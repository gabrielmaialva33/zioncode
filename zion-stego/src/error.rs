//! Error types by layer. See spec §5.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("empty input is not supported")]
    EmptyInput,

    #[error("no host images provided")]
    NoHostImages,

    #[error("invalid host image {index}: expected {expected} RGB bytes, got {got}")]
    InvalidHostImage {
        index: usize,
        expected: usize,
        got: usize,
    },

    #[error("insufficient capacity: needs {needed} bytes, available {available} across photos")]
    InsufficientCapacity { needed: usize, available: usize },

    #[error("invalid embedding density: {got} (expected 0.0 < density <= 1.0)")]
    InvalidDensity { got: f32 },

    #[error("Argon2id failed: {0}")]
    KdfFailed(String),

    #[error("AEAD encrypt failed: {0}")]
    AeadFailed(String),

    #[error("PNG I/O failed: {0}")]
    PngError(String),

    #[error("zion-codec subsystem error: {0}")]
    CodecError(#[from] zion_codec::EncodeError),
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("no stego images provided")]
    NoStegoImages,

    #[error("invalid stego image {index}: expected {expected} RGB bytes, got {got}")]
    InvalidStegoImage {
        index: usize,
        expected: usize,
        got: usize,
    },

    #[error("invalid plaintext header in image {index}: magic is not ZSTG")]
    BadMagic { index: usize, got: [u8; 4] },

    #[error("unsupported version in image {index}: got={got}, max_supported={max_supported}")]
    UnsupportedVersion {
        index: usize,
        got: u8,
        max_supported: u8,
    },

    #[error(
        "AEAD authentication failed on symbol {symbol_index} — wrong passphrase or tampered image"
    )]
    AeadAuthFailed { symbol_index: u16 },

    #[error("Argon2id failed: {0}")]
    KdfFailed(String),

    #[error("file_id diverges between stego images (mixed sets)")]
    InconsistentFileId,

    #[error("PNG I/O failed: {0}")]
    PngError(String),

    #[error("zion-codec reassembly error: {0}")]
    ReassemblyFailed(#[from] zion_codec::FileError),

    #[error("symbol decode error: {0}")]
    SymbolDecodeError(#[from] zion_codec::SymbolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_error_display_includes_fields() {
        let e = EmbedError::InsufficientCapacity {
            needed: 1000,
            available: 500,
        };
        let s = format!("{e}");
        assert!(s.contains("1000"));
        assert!(s.contains("500"));
    }

    #[test]
    fn extract_error_display_includes_fields() {
        let e = ExtractError::AeadAuthFailed { symbol_index: 3 };
        assert!(format!("{e}").contains('3'));
    }
}
