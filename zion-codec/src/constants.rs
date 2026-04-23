//! Constantes trancadas pela v1. Ver spec Appendix A.

pub const MAGIC: [u8; 4] = *b"ZION";
pub const VERSION: u8 = 0x01;
pub const HEADER_LEN_V1: usize = 82;
pub const BLOCK_HEADER_LEN: usize = 7;
pub const BLOCK_SIZE_RAW: usize = 8192;

pub const RS_N: usize = 255;
pub const RS_K: usize = 223;
pub const RS_PARITY: usize = RS_N - RS_K; // 32

pub const TARGET_BLOCKS_PER_SYMBOL: usize = 4;

pub const ZSTD_LEVEL_FAST: i32 = 3;
pub const ZSTD_LEVEL_BALANCED: i32 = 6;
pub const ZSTD_LEVEL_MAX: i32 = 9;
pub const ZSTD_LEVEL_DEFAULT: i32 = ZSTD_LEVEL_BALANCED;

/// Implementation limit: maximum blocks per symbol.
pub const MAX_BLOCK_COUNT: u16 = 64;

/// Implementation limit: a symbol may not exceed 1 MiB of transmitted bytes.
pub const MAX_SYMBOL_BYTES: usize = 1 << 20;

/// Derivado de `MAX_SYMBOL_BYTES` / `RS_N`.
pub const MAX_K: usize = MAX_SYMBOL_BYTES / RS_N; // 4112

/// Implementation limit: maximum total blocks per file.
pub const MAX_TOTAL_BLOCKS: u32 = 1 << 20;

/// Implementation limit: maximum total symbols per file.
pub const MAX_TOTAL_SYMBOLS: u16 = 1 << 14;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_values() {
        assert_eq!(MAGIC, *b"ZION");
        assert_eq!(VERSION, 0x01);
        assert_eq!(HEADER_LEN_V1, 82);
        assert_eq!(BLOCK_HEADER_LEN, 7);
        assert_eq!(BLOCK_SIZE_RAW, 8192);
        assert_eq!(RS_N, 255);
        assert_eq!(RS_K, 223);
        assert_eq!(RS_PARITY, 32);
        assert_eq!(TARGET_BLOCKS_PER_SYMBOL, 4);
        assert_eq!(ZSTD_LEVEL_FAST, 3);
        assert_eq!(ZSTD_LEVEL_BALANCED, 6);
        assert_eq!(ZSTD_LEVEL_MAX, 9);
        assert_eq!(ZSTD_LEVEL_DEFAULT, ZSTD_LEVEL_BALANCED);
        assert_eq!(MAX_BLOCK_COUNT, 64);
        assert_eq!(MAX_SYMBOL_BYTES, 1 << 20);
        assert_eq!(MAX_TOTAL_BLOCKS, 1 << 20);
        assert_eq!(MAX_TOTAL_SYMBOLS, 1 << 14);
    }

    #[test]
    fn max_k_derived() {
        assert_eq!(MAX_K, 4112);
    }
}
