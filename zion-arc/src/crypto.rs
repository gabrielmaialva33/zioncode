use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::Zeroizing;

pub(crate) fn seal(
    key: &[u8; 32],
    nonce: &[u8; 24],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, ()> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| ())?;
    cipher
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| ())
}

pub(crate) fn open(
    key: &[u8; 32],
    nonce: &[u8; 24],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, ()> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| ())?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| ())
}
