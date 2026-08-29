use std::{collections::HashSet, fmt, sync::Mutex};

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::{Zeroize, Zeroizing};

use crate::constants::{
    AEAD_KEY_CONTEXT, ARGON2_ITERATIONS, ARGON2_MEMORY_KIB, ARGON2_OUTPUT_LEN, ARGON2_PARALLELISM,
    ARGON2_SALT_CONTEXT, MATCHING_CONTEXT, MAX_PASSPHRASE_LEN, NONCE_CONTEXT, PERMUTATION_CONTEXT,
};
use crate::error::CollectionKeyError;

/// Reusable, in-memory collection key. Its key bytes are redacted and zeroized.
pub struct CollectionKey {
    collection_id: [u8; 16],
    key: Zeroizing<[u8; 32]>,
    used_capsule_ids: Mutex<HashSet<[u8; 16]>>,
}

impl CollectionKey {
    /// Creates a new collection with an OS-random nonzero identifier.
    ///
    /// # Errors
    /// Returns an error for an invalid passphrase, unavailable OS randomness,
    /// or failed Argon2id derivation.
    pub fn new(passphrase: &[u8]) -> Result<Self, CollectionKeyError> {
        validate_passphrase(passphrase)?;
        let collection_id = random_nonzero_id()?;
        Self::derive(passphrase, collection_id)
    }

    /// Re-derives the key for an existing public collection identifier.
    ///
    /// # Errors
    /// Returns an error for an invalid passphrase or identifier, or when
    /// Argon2id derivation fails.
    pub fn derive(passphrase: &[u8], collection_id: [u8; 16]) -> Result<Self, CollectionKeyError> {
        validate_passphrase(passphrase)?;
        if collection_id == [0; 16] {
            return Err(CollectionKeyError::InvalidIdentifier);
        }
        let salt = derive_salt(&collection_id);
        let params = Params::new(
            ARGON2_MEMORY_KIB,
            ARGON2_ITERATIONS,
            ARGON2_PARALLELISM,
            Some(ARGON2_OUTPUT_LEN),
        )
        .map_err(|_| CollectionKeyError::KdfFailed)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; ARGON2_OUTPUT_LEN]);
        argon2
            .hash_password_into(passphrase, &salt, key.as_mut())
            .map_err(|_| CollectionKeyError::KdfFailed)?;
        Ok(Self {
            collection_id,
            key,
            used_capsule_ids: Mutex::new(HashSet::new()),
        })
    }

    #[must_use]
    pub const fn collection_id(&self) -> [u8; 16] {
        self.collection_id
    }

    pub(crate) fn key_bytes(&self) -> &[u8; 32] {
        &self.key
    }

    pub(crate) fn fresh_capsule_id(&self) -> Result<[u8; 16], CollectionKeyError> {
        loop {
            let id = random_nonzero_id()?;
            let mut used = self
                .used_capsule_ids
                .lock()
                .map_err(|_| CollectionKeyError::RandomFailed)?;
            if used.insert(id) {
                return Ok(id);
            }
        }
    }
}

impl fmt::Debug for CollectionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionKey")
            .field("collection_id", &self.collection_id)
            .field("key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

pub(crate) struct CapsuleValues {
    pub aead_key: Zeroizing<[u8; 32]>,
    pub nonce: [u8; 24],
    pub permutation_seed: Zeroizing<[u8; 32]>,
    pub matching_seed: Zeroizing<[u8; 32]>,
}

impl Drop for CapsuleValues {
    fn drop(&mut self) {
        self.nonce.zeroize();
    }
}

pub(crate) fn validate_passphrase(passphrase: &[u8]) -> Result<(), CollectionKeyError> {
    if passphrase.is_empty() || passphrase.len() > MAX_PASSPHRASE_LEN {
        return Err(CollectionKeyError::InvalidPassphraseLength);
    }
    Ok(())
}

#[must_use]
pub(crate) fn derive_salt(collection_id: &[u8; 16]) -> [u8; 16] {
    let full = blake3::derive_key(ARGON2_SALT_CONTEXT, collection_id);
    full[..16].try_into().expect("fixed slice")
}

#[must_use]
pub(crate) fn derive_capsule_values(
    collection_key: &CollectionKey,
    capsule_id: &[u8; 16],
    width: u32,
    height: u32,
) -> CapsuleValues {
    let mut key_material = Zeroizing::new([0u8; 64]);
    key_material[..32].copy_from_slice(collection_key.key_bytes());
    key_material[32..48].copy_from_slice(&collection_key.collection_id);
    key_material[48..64].copy_from_slice(capsule_id);

    let mut carrier_material = Zeroizing::new([0u8; 72]);
    carrier_material[..64].copy_from_slice(key_material.as_ref());
    carrier_material[64..68].copy_from_slice(&width.to_le_bytes());
    carrier_material[68..72].copy_from_slice(&height.to_le_bytes());

    let mut nonce_material = Zeroizing::new([0u8; 32]);
    nonce_material[..16].copy_from_slice(&collection_key.collection_id);
    nonce_material[16..].copy_from_slice(capsule_id);
    let nonce_full = Zeroizing::new(blake3::derive_key(NONCE_CONTEXT, nonce_material.as_ref()));

    CapsuleValues {
        aead_key: Zeroizing::new(blake3::derive_key(AEAD_KEY_CONTEXT, key_material.as_ref())),
        nonce: nonce_full[..24].try_into().expect("fixed slice"),
        permutation_seed: Zeroizing::new(blake3::derive_key(
            PERMUTATION_CONTEXT,
            carrier_material.as_ref(),
        )),
        matching_seed: Zeroizing::new(blake3::derive_key(
            MATCHING_CONTEXT,
            carrier_material.as_ref(),
        )),
    }
}

pub(crate) fn random_nonzero_id() -> Result<[u8; 16], CollectionKeyError> {
    loop {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id).map_err(|_| CollectionKeyError::RandomFailed)?;
        if id != [0; 16] {
            return Ok(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_redacts_key_material() {
        let key = CollectionKey {
            collection_id: [1; 16],
            key: Zeroizing::new([0xAB; 32]),
            used_capsule_ids: Mutex::new(HashSet::new()),
        };
        let rendered = format!("{key:?}");
        assert!(rendered.contains("REDACTED"));
        assert!(!rendered.contains("abababababababab"));
        assert!(!rendered.contains("171, 171, 171"));
    }

    #[test]
    fn public_constructor_errors_are_typed_and_validate_before_derivation() {
        assert!(matches!(
            CollectionKey::new(b""),
            Err(CollectionKeyError::InvalidPassphraseLength)
        ));
        assert!(matches!(
            CollectionKey::derive(b"valid", [0; 16]),
            Err(CollectionKeyError::InvalidIdentifier)
        ));
    }

    #[test]
    fn contexts_are_pairwise_distinct() {
        let contexts = [
            ARGON2_SALT_CONTEXT,
            AEAD_KEY_CONTEXT,
            NONCE_CONTEXT,
            PERMUTATION_CONTEXT,
            MATCHING_CONTEXT,
        ];
        for (index, left) in contexts.iter().enumerate() {
            for right in &contexts[index + 1..] {
                assert_ne!(left, right);
            }
        }
    }
}
