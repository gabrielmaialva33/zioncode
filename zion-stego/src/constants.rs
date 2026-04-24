//! Constantes trancadas pela v1. Ver spec Appendix A.

/// Magic bytes no início do plaintext header.
pub const MAGIC: [u8; 4] = *b"ZSTG";

/// Versão do formato v1.
pub const VERSION: u8 = 0x01;

/// Tamanho do plaintext header (32 bytes = 256 bits).
pub const PLAINTEXT_HEADER_LEN: usize = 32;

/// Número de canais row-major ocupados pelo plaintext header.
pub const PLAINTEXT_HEADER_CHANNELS: usize = 256;

/// Argon2id: memória em KiB (64 MiB).
pub const ARGON2ID_MEMORY_KIB: u32 = 65_536;

/// Argon2id: iterações.
pub const ARGON2ID_ITERATIONS: u32 = 3;

/// Argon2id: paralelismo.
pub const ARGON2ID_PARALLELISM: u32 = 4;

/// Argon2id: tamanho de output em bytes.
pub const ARGON2ID_OUTPUT_LEN: usize = 32;

/// Prefix do salt do Argon2id (concatena com `file_id` antes de BLAKE3).
pub const ARGON2ID_SALT_PREFIX: &str = "zioncode-stego-v1:salt-derivation";

/// Context do `blake3::derive_key` para `aead_key`.
pub const KDF_AEAD_CONTEXT: &str = "zioncode-stego-v1 aead key v1";

/// Context do `blake3::derive_key` para `permutation_seed` (per-símbolo).
pub const KDF_PERMUTATION_CONTEXT: &str = "zioncode-stego-v1 permutation seed v1";

/// Context do `blake3::derive_key` para nonce (per-símbolo).
pub const KDF_NONCE_CONTEXT: &str = "zioncode-stego-v1 nonce v1";

/// `XChaCha20-Poly1305`: nonce em bytes.
pub const AEAD_NONCE_LEN: usize = 24;

/// `XChaCha20-Poly1305`: tag Poly1305 em bytes.
pub const AEAD_TAG_LEN: usize = 16;

/// `XChaCha20-Poly1305`: chave em bytes.
pub const AEAD_KEY_LEN: usize = 32;

/// Density default (fração de canais permutáveis usados para embedding).
pub const EMBEDDING_DENSITY_DEFAULT: f32 = 0.33;

/// Implementation limit do `zion-codec` (`MAX_SYMBOL_BYTES` / `RS_N`).
/// `K_global` do B é clamped a esse teto.
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
