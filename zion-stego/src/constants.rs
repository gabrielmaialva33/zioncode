//! Constants frozen by v1. See spec Appendix A.

/// Magic bytes at the start of the plaintext header.
pub const MAGIC: [u8; 4] = *b"ZSTG";

/// Format version v1.
pub const VERSION: u8 = 0x01;

/// Plaintext header size (32 bytes = 256 bits).
pub const PLAINTEXT_HEADER_LEN: usize = 32;

/// Number of row-major channels occupied by the plaintext header.
pub const PLAINTEXT_HEADER_CHANNELS: usize = 256;

/// Argon2id: memory in KiB (64 MiB).
pub const ARGON2ID_MEMORY_KIB: u32 = 65_536;

/// Argon2id: iterations.
pub const ARGON2ID_ITERATIONS: u32 = 3;

/// Argon2id: parallelism.
pub const ARGON2ID_PARALLELISM: u32 = 4;

/// Argon2id: output length in bytes.
pub const ARGON2ID_OUTPUT_LEN: usize = 32;

/// Argon2id salt prefix (concatenated with `file_id` before BLAKE3).
pub const ARGON2ID_SALT_PREFIX: &str = "zioncode-stego-v1:salt-derivation";

/// `blake3::derive_key` context for `aead_key`.
pub const KDF_AEAD_CONTEXT: &str = "zioncode-stego-v1 aead key v1";

/// `blake3::derive_key` context for `permutation_seed` (per-symbol).
pub const KDF_PERMUTATION_CONTEXT: &str = "zioncode-stego-v1 permutation seed v1";

/// `blake3::derive_key` context for nonce (per-symbol).
pub const KDF_NONCE_CONTEXT: &str = "zioncode-stego-v1 nonce v1";

/// `XChaCha20-Poly1305`: nonce length in bytes.
pub const AEAD_NONCE_LEN: usize = 24;

/// `XChaCha20-Poly1305`: Poly1305 tag length in bytes.
pub const AEAD_TAG_LEN: usize = 16;

/// `XChaCha20-Poly1305`: key length in bytes.
pub const AEAD_KEY_LEN: usize = 32;

/// Default density (fraction of permutable channels used for embedding).
pub const EMBEDDING_DENSITY_DEFAULT: f32 = 0.33;

/// `zion-codec` implementation limit (`MAX_SYMBOL_BYTES` / `RS_N`).
/// B's `K_global` is clamped to this ceiling.
pub const ZION_CODEC_MAX_K: u16 = 4112;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_values() {
        assert_eq!(MAGIC, *b"ZSTG");
        assert_eq!(VERSION, 0x01);
        assert_eq!(PLAINTEXT_HEADER_LEN, 32);
        assert_eq!(PLAINTEXT_HEADER_CHANNELS, 256);
        assert_eq!(ARGON2ID_MEMORY_KIB, 65_536);
        assert_eq!(ARGON2ID_ITERATIONS, 3);
        assert_eq!(ARGON2ID_PARALLELISM, 4);
        assert_eq!(AEAD_NONCE_LEN, 24);
        assert_eq!(AEAD_TAG_LEN, 16);
        assert_eq!(AEAD_KEY_LEN, 32);
        assert!((EMBEDDING_DENSITY_DEFAULT - 0.33).abs() < 1e-6);
        assert_eq!(ZION_CODEC_MAX_K, 4112);
    }

    #[test]
    fn max_k_matches_zion_codec_impl_limit() {
        assert_eq!(usize::from(ZION_CODEC_MAX_K), zion_codec::low_level::MAX_K);
    }
}
