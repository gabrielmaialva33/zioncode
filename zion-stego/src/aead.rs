//! Wrapper over XChaCha20-Poly1305 (chacha20poly1305 crate).
//! See spec §3 AEAD.

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};

use crate::constants::{AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN};

/// Encrypts `plaintext` returning `ciphertext || tag` (total = `plaintext.len()` + 16).
///
/// # Errors
/// Returns error if AEAD fails (e.g. input too large).
pub fn seal(
    key: &[u8; AEAD_KEY_LEN],
    nonce: &[u8; AEAD_NONCE_LEN],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    let xnonce = XNonce::from_slice(nonce);
    cipher.encrypt(xnonce, plaintext).map_err(|e| e.to_string())
}

/// Decrypts `ciphertext || tag` returning the plaintext, or fails if the tag does not match.
///
/// # Errors
/// Returns error if the Poly1305 tag fails to authenticate.
pub fn open(
    key: &[u8; AEAD_KEY_LEN],
    nonce: &[u8; AEAD_NONCE_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    let xnonce = XNonce::from_slice(nonce);
    cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|e| e.to_string())
}

/// Ciphertext length given the plaintext length.
#[must_use]
pub const fn ciphertext_len(plaintext_len: usize) -> usize {
    plaintext_len + AEAD_TAG_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [0xAA; 32];
    const NONCE: [u8; 24] = [0xBB; 24];

    #[test]
    fn roundtrip() {
        let plaintext = b"secret bible verse";
        let ct = seal(&KEY, &NONCE, plaintext).unwrap();
        assert_eq!(ct.len(), ciphertext_len(plaintext.len()));
        let pt = open(&KEY, &NONCE, &ct).unwrap();
        assert_eq!(pt, plaintext);
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let plaintext = b"sensitive payload";
        let mut ct = seal(&KEY, &NONCE, plaintext).unwrap();
        ct[0] ^= 0xFF;
        assert!(open(&KEY, &NONCE, &ct).is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let plaintext = b"x";
        let ct = seal(&KEY, &NONCE, plaintext).unwrap();
        let wrong_key = [0xCC; 32];
        assert!(open(&wrong_key, &NONCE, &ct).is_err());
    }

    #[test]
    fn wrong_nonce_fails() {
        let plaintext = b"x";
        let ct = seal(&KEY, &NONCE, plaintext).unwrap();
        let wrong_nonce = [0xDD; 24];
        assert!(open(&KEY, &wrong_nonce, &ct).is_err());
    }

    #[test]
    fn empty_plaintext_works() {
        let ct = seal(&KEY, &NONCE, b"").unwrap();
        assert_eq!(ct.len(), AEAD_TAG_LEN);
        let pt = open(&KEY, &NONCE, &ct).unwrap();
        assert_eq!(pt, b"");
    }
}
