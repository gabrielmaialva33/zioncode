//! KDF: Argon2id (passphrase → `master_key`) + `blake3::derive_key` (`master_key` → sub-keys).
//! Ver spec §3.

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use crate::constants::{
    AEAD_KEY_LEN, AEAD_NONCE_LEN, ARGON2ID_ITERATIONS, ARGON2ID_MEMORY_KIB,
    ARGON2ID_OUTPUT_LEN, ARGON2ID_PARALLELISM, ARGON2ID_SALT_PREFIX, KDF_AEAD_CONTEXT,
    KDF_NONCE_CONTEXT, KDF_PERMUTATION_CONTEXT,
};

/// Deriva o salt do Argon2id a partir do `file_id`.
///
/// `salt = BLAKE3(SALT_PREFIX || file_id)[..16]`
#[must_use]
pub fn derive_salt(file_id: &[u8; 16]) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ARGON2ID_SALT_PREFIX.as_bytes());
    hasher.update(file_id);
    let full = hasher.finalize();
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&full.as_bytes()[..16]);
    salt
}

/// Deriva a `master_key` via Argon2id (64 MiB, 3 iter, 4 parallel).
///
/// # Errors
/// Retorna erro se o Argon2id falhar (params inválidos ou out-of-memory).
pub fn derive_master_key(
    passphrase: &[u8],
    file_id: &[u8; 16],
) -> Result<Zeroizing<[u8; 32]>, String> {
    let salt = derive_salt(file_id);
    let params = Params::new(
        ARGON2ID_MEMORY_KIB,
        ARGON2ID_ITERATIONS,
        ARGON2ID_PARALLELISM,
        Some(ARGON2ID_OUTPUT_LEN),
    )
    .map_err(|e| e.to_string())?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut master = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase, &salt, master.as_mut())
        .map_err(|e| e.to_string())?;
    Ok(master)
}

/// `aead_key = blake3::derive_key(KDF_AEAD_CONTEXT, master_key)`
#[must_use]
pub fn derive_aead_key(master_key: &[u8; 32]) -> Zeroizing<[u8; AEAD_KEY_LEN]> {
    Zeroizing::new(blake3::derive_key(KDF_AEAD_CONTEXT, master_key))
}

/// `permutation_seed_i = blake3::derive_key(`
///    `KDF_PERMUTATION_CONTEXT,`
///    `master_key || file_id || symbol_index.to_le_bytes()`
/// `)`
#[must_use]
pub fn derive_permutation_seed(
    master_key: &[u8; 32],
    file_id: &[u8; 16],
    symbol_index: u16,
) -> Zeroizing<[u8; 32]> {
    let mut input = Zeroizing::new([0u8; 32 + 16 + 2]);
    input[..32].copy_from_slice(master_key);
    input[32..48].copy_from_slice(file_id);
    input[48..50].copy_from_slice(&symbol_index.to_le_bytes());
    Zeroizing::new(blake3::derive_key(KDF_PERMUTATION_CONTEXT, input.as_ref()))
}

/// `nonce_i = blake3::derive_key(`
///    `KDF_NONCE_CONTEXT,`
///    `master_key || symbol_index.to_le_bytes()`
/// `)[..24]`
#[must_use]
pub fn derive_nonce(master_key: &[u8; 32], symbol_index: u16) -> [u8; AEAD_NONCE_LEN] {
    let mut input = Zeroizing::new([0u8; 32 + 2]);
    input[..32].copy_from_slice(master_key);
    input[32..34].copy_from_slice(&symbol_index.to_le_bytes());
    let full = blake3::derive_key(KDF_NONCE_CONTEXT, input.as_ref());
    let mut nonce = [0u8; AEAD_NONCE_LEN];
    nonce.copy_from_slice(&full[..AEAD_NONCE_LEN]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_ID_A: [u8; 16] = [0xAA; 16];
    const FILE_ID_B: [u8; 16] = [0xBB; 16];

    #[test]
    fn salt_deterministic_per_file_id() {
        let s1 = derive_salt(&FILE_ID_A);
        let s2 = derive_salt(&FILE_ID_A);
        assert_eq!(s1, s2);
    }

    #[test]
    fn salt_differs_per_file_id() {
        let s1 = derive_salt(&FILE_ID_A);
        let s2 = derive_salt(&FILE_ID_B);
        assert_ne!(s1, s2);
    }

    #[test]
    fn master_key_deterministic() {
        let m1 = derive_master_key(b"senha", &FILE_ID_A).unwrap();
        let m2 = derive_master_key(b"senha", &FILE_ID_A).unwrap();
        assert_eq!(*m1, *m2);
    }

    #[test]
    fn master_key_changes_with_file_id() {
        let m1 = derive_master_key(b"senha", &FILE_ID_A).unwrap();
        let m2 = derive_master_key(b"senha", &FILE_ID_B).unwrap();
        assert_ne!(*m1, *m2);
    }

    #[test]
    fn master_key_changes_with_passphrase() {
        let m1 = derive_master_key(b"senha1", &FILE_ID_A).unwrap();
        let m2 = derive_master_key(b"senha2", &FILE_ID_A).unwrap();
        assert_ne!(*m1, *m2);
    }

    #[test]
    fn sub_keys_distinct_from_master_and_each_other() {
        let master = [0x55u8; 32];
        let aead = derive_aead_key(&master);
        let perm = derive_permutation_seed(&master, &FILE_ID_A, 0);
        let nonce = derive_nonce(&master, 0);

        assert_ne!(*aead, master);
        assert_ne!(*perm, master);
        assert_ne!(aead.as_slice(), perm.as_slice());
        assert_ne!(&aead[..24], nonce);
    }

    #[test]
    fn permutation_seed_per_symbol() {
        let master = [0x77u8; 32];
        let s0 = derive_permutation_seed(&master, &FILE_ID_A, 0);
        let s1 = derive_permutation_seed(&master, &FILE_ID_A, 1);
        assert_ne!(*s0, *s1);
    }

    #[test]
    fn nonce_per_symbol() {
        let master = [0x11u8; 32];
        let n0 = derive_nonce(&master, 0);
        let n1 = derive_nonce(&master, 1);
        assert_ne!(n0, n1);
    }
}
