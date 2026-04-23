//! Error types by layer. See spec section 7.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum EncodeError {
    #[error("empty input is not supported in v1")]
    EmptyInput,

    #[error(
        "insufficient capacity: K={k}, minimum required={min_k_required} at block {block_index}"
    )]
    InsufficientSymbolCapacity {
        k: u16,
        min_k_required: u16,
        block_index: u32,
    },

    #[error("zstd encode failed at block {block_index}: {zstd_err}")]
    ZstdEncodeFailed { block_index: u32, zstd_err: String },

    #[error("K exceeds implementation limit: {got} > {max}")]
    KTooLarge { got: u16, max: usize },

    #[error("too many blocks for v1 implementation: {got} > {max}")]
    TooManyBlocks { got: usize, max: u32 },

    #[error("too many symbols for v1 implementation: {got} > {max}")]
    TooManySymbols { got: usize, max: u16 },
}

#[derive(Debug, Error)]
pub enum SymbolError {
    #[error("invalid symbol size: {got} bytes (not a multiple of 255)")]
    BadSize { got: usize },

    #[error("RS decode failed for codeword {codeword_index}")]
    RsDecodeFailed { codeword_index: u16 },

    #[error("invalid magic: {got:?}")]
    BadMagic { got: [u8; 4] },

    #[error("unsupported version: got={got}, max_supported={max_supported}")]
    UnsupportedVersion { got: u8, max_supported: u8 },

    #[error("unsupported header flags: {flags:#x}")]
    UnsupportedHeaderFlags { flags: u16 },

    #[error("invalid header_len: {got}")]
    HeaderLengthInvalid { got: u8 },

    #[error("header CRC32C mismatch")]
    HeaderCrcMismatch,

    #[error("symbol exceeds maximum allowed size: {got} > {max}")]
    SymbolByteLengthTooLarge { got: usize, max: usize },

    #[error("block range overflows u32: block_start={block_start}, block_count={block_count}")]
    BlockRangeOverflow { block_start: u32, block_count: u16 },
}

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("block {block_index} CRC32C mismatch")]
    BlockCrcMismatch { block_index: u32 },

    #[error("zstd decode failed at block {block_index}: {zstd_err}")]
    ZstdDecodeFailed { block_index: u32, zstd_err: String },

    #[error("unknown flag in block {block_index}: {flag_byte:#x}")]
    UnknownFlag { block_index: u32, flag_byte: u8 },

    #[error("block {block_index} payload_size exceeds 8192: {got}")]
    PayloadSizeTooLarge { block_index: u32, got: u16 },

    #[error("raw block {block_index} has wrong size: expected {expected}, got {got}")]
    RawBlockSizeMismatch {
        block_index: u32,
        expected: u16,
        got: u16,
    },

    #[error(
        "compressed block {block_index} is not smaller than raw: raw_expected={raw_expected}, compressed={compressed}"
    )]
    CompressedPayloadNotSmaller {
        block_index: u32,
        raw_expected: u16,
        compressed: u16,
    },
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("missing blocks: {ranges:?}")]
    MissingBlocks { ranges: Vec<(u32, u32)> },

    #[error("file_id mismatch between symbols")]
    SymbolFileIdMismatch { expected: [u8; 16], got: [u8; 16] },

    #[error("metadata mismatch: {field}")]
    InconsistentFileMetadata { field: &'static str },

    #[error("global file hash mismatch")]
    GlobalHashMismatch { expected: [u8; 32], got: [u8; 32] },

    #[error("symbol_index {got} >= total_symbols {total}")]
    SymbolIndexOutOfRange { got: u16, total: u16 },

    #[error(
        "block range exceeds total_blocks: block_start={block_start}, count={block_count}, total={total_blocks}"
    )]
    BlockRangeExceedsTotal {
        block_start: u32,
        block_count: u16,
        total_blocks: u32,
    },

    #[error("duplicate symbol {symbol_index} (divergent={divergent})")]
    DuplicateSymbol { symbol_index: u16, divergent: bool },

    #[error("overlapping block range: symbol_index={symbol_index}, block_index={block_index}")]
    OverlappingBlockRange { symbol_index: u16, block_index: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_shows_fields() {
        let e = SymbolError::BadSize { got: 100 };
        assert!(format!("{e}").contains("100"));
    }

    #[test]
    fn missing_blocks_debug() {
        let e = FileError::MissingBlocks {
            ranges: vec![(10, 12), (20, 20)],
        };
        let s = format!("{e}");
        assert!(s.contains("10"));
    }
}
