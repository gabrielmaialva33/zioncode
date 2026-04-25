//! Error types for the optical render and extract pipelines.

use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PngImageError {
    #[error("invalid RGB data length: got {got} bytes, expected {expected}")]
    InvalidRgbDataLen { expected: usize, got: usize },

    #[error("image dimensions are too large: width={width}, height={height}")]
    ImageTooLarge { width: u32, height: u32 },

    #[error("unsupported PNG format: got {color_type:?} {bit_depth:?}, expected RGB 8-bit")]
    UnsupportedFormat {
        color_type: png::ColorType,
        bit_depth: png::BitDepth,
    },

    #[error("PNG output buffer size is unavailable or exceeds limits")]
    OutputBufferSizeUnavailable,

    #[error("PNG encode error: {0}")]
    Encode(#[from] png::EncodingError),

    #[error("PNG decode error: {0}")]
    Decode(#[from] png::DecodingError),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("empty input is not supported")]
    EmptyInput,

    #[error("insufficient image capacity: needed {needed} bytes, available {available} bytes")]
    InsufficientImageCapacity { needed: usize, available: usize },

    #[error("invalid image dimensions: {0}")]
    InvalidDimensions(String),

    #[error("payload too large for u32 payload_len field: {got} bytes")]
    PayloadTooLargeForV1 { got: u64 },

    #[error("too many symbols for v1 header: {got} symbols")]
    TooManySymbols { got: usize },

    #[error("zion-codec encode error: {0}")]
    CodecError(#[from] zion_codec::EncodeError),

    #[error("PNG image error: {0}")]
    PngError(#[from] PngImageError),
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("PNG image error: {0}")]
    PngError(#[from] PngImageError),

    #[error("unsupported PNG format: got {got}, expected RGB 8-bit")]
    UnsupportedPngFormat { got: String },

    #[error("image too small: got {got} bytes, min {min}")]
    ImageTooSmall { got: usize, min: usize },

    #[error("bad magic at optical header: got {got:?}, expected ZOPT")]
    BadMagic { got: [u8; 4] },

    #[error("unsupported optical version: got {got}, max supported {max_supported}")]
    UnsupportedVersion { got: u8, max_supported: u8 },

    #[error("optical header CRC mismatch")]
    HeaderCrcMismatch,

    #[error("invalid optical header_len: got {got}, expected 24 in v1")]
    InvalidHeaderLen { got: u8 },

    #[error("empty container: total_symbols is zero")]
    EmptyContainer,

    #[error("invalid optical k_global: got {got}")]
    InvalidKGlobal { got: u16 },

    #[error(
        "inconsistent payload length: header declared {declared}, computed from k×total {computed}"
    )]
    InconsistentPayloadLength { declared: u32, computed: u64 },

    #[error("padding byte not zero at offset {offset}")]
    PaddingNotZero { offset: usize },

    #[error("payload truncated: declared {declared} bytes, available {available}")]
    PayloadTruncated { declared: u32, available: usize },

    #[error("zion-codec reassembly error: {0}")]
    ReassemblyFailed(#[from] zion_codec::FileError),

    #[error("zion-codec symbol decode error: {0}")]
    SymbolDecodeError(#[from] zion_codec::SymbolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_error_display_includes_fields() {
        let e = RenderError::InsufficientImageCapacity {
            needed: 1000,
            available: 500,
        };
        let s = format!("{e}");
        assert!(s.contains("1000") && s.contains("500"));
    }

    #[test]
    fn extract_error_display_includes_fields() {
        let e = ExtractError::InvalidHeaderLen { got: 99 };
        assert!(format!("{e}").contains("99"));
    }
}
