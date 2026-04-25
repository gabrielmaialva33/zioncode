//! Constants frozen by spec v1. See spec Appendix A.

/// Magic bytes that mark a `zion-optical` v1 stream: ASCII "ZOPT".
pub const MAGIC: [u8; 4] = *b"ZOPT";

/// Format version v1.
pub const VERSION: u8 = 0x01;

/// Total length of the optical header in bytes (24 in v1).
pub const OPTICAL_HEADER_LEN: usize = 24;

/// Bytes per pixel for PNG RGB 24 bpp (3 channels × 1 byte).
pub const BYTES_PER_PIXEL: usize = 3;

/// PNG bit depth for v1 (8 bits per channel).
pub const PNG_BIT_DEPTH: u8 = 8;

/// Default `K` (codewords per symbol) — mirrors `zion_codec` `MAX_K` so each symbol fills 1 MiB.
#[allow(
    clippy::cast_possible_truncation,
    reason = "MAX_K = 4112 fits u16 (verified by k_default_matches_zion_codec_max_k test)"
)]
pub const K_DEFAULT: u16 = zion_codec::low_level::MAX_K as u16;

/// Default zstd compression level: maximum (22) for archival density.
pub const ZSTD_LEVEL_DEFAULT: i32 = 22;

/// Maximum payload size the v1 `payload_len` field (u32) can express, in bytes.
#[allow(
    clippy::cast_lossless,
    reason = "`u64::from` is not const fn on stable; widening u32->u64 is infallible"
)]
pub const MAX_PAYLOAD_BYTES_V1: u64 = u32::MAX as u64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_values_locked() {
        assert_eq!(MAGIC, *b"ZOPT");
        assert_eq!(VERSION, 0x01);
        assert_eq!(OPTICAL_HEADER_LEN, 24);
        assert_eq!(BYTES_PER_PIXEL, 3);
        assert_eq!(PNG_BIT_DEPTH, 8);
        assert_eq!(ZSTD_LEVEL_DEFAULT, 22);
        assert_eq!(MAX_PAYLOAD_BYTES_V1, u64::from(u32::MAX));
    }

    #[test]
    fn k_default_matches_zion_codec_max_k() {
        assert_eq!(usize::from(K_DEFAULT), zion_codec::low_level::MAX_K);
    }
}
