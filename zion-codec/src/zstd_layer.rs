//! Per-block zstd compression with raw fallback. See spec section 5.
//!
//! Rule: if `compressed.len() < raw.len()` we keep the compressed payload
//! (`flag.bit0 = 1`). Otherwise we fall back to raw (`flag.bit0 = 0`) to
//! guarantee we never make the payload larger.
//!
//! zstd frame config: `contentSizeFlag = 0`, `checksumFlag = 0`,
//! `dictID = 0`. This trims ~5-12 bytes/block of frame overhead because:
//! - contentSize: we already carry the payload size in the block header.
//! - checksum: we already have CRC32C in the block header.
//! - dictID: v1 does not use dictionaries.

use crate::constants::BLOCK_SIZE_RAW;
use crate::error::{BlockError, EncodeError};
use crate::format::BlockEntry;

/// Compress `raw`. If `compressed.len() >= raw.len()`, return raw instead.
///
/// # Errors
/// Returns `EncodeError::ZstdEncodeFailed` if zstd fails.
pub fn encode_block(raw: &[u8], level: i32, block_index: u32) -> Result<BlockEntry, EncodeError> {
    debug_assert!(raw.len() <= BLOCK_SIZE_RAW);

    let compressed =
        zstd_compress(raw, level).map_err(|zstd_err| EncodeError::ZstdEncodeFailed {
            block_index,
            zstd_err,
        })?;

    if compressed.len() < raw.len() {
        BlockEntry::compressed(compressed).map_err(|err| match err {
            BlockError::PayloadSizeOutOfRange { got, max } => {
                EncodeError::BlockPayloadTooLarge { got, max }
            }
            other => unreachable!("unexpected block construction error: {other:?}"),
        })
    } else {
        BlockEntry::raw(raw.to_vec()).map_err(|err| match err {
            BlockError::PayloadSizeOutOfRange { got, max } => {
                EncodeError::BlockPayloadTooLarge { got, max }
            }
            other => unreachable!("unexpected block construction error: {other:?}"),
        })
    }
}

/// Decode a stored payload while honoring the `compressed` flag.
///
/// # Errors
/// Returns `BlockError::ZstdDecodeFailed`, `RawBlockSizeMismatch`, or
/// `CompressedPayloadNotSmaller` if the block violates v1 invariants.
pub fn decode_block(
    entry: &BlockEntry,
    block_index: u32,
    expected_raw_size: u16,
) -> Result<Vec<u8>, BlockError> {
    if entry.is_compressed() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "payload_size <= 8192 by parse invariant"
        )]
        let compressed = entry.payload().len() as u16;
        if compressed >= expected_raw_size {
            return Err(BlockError::CompressedPayloadNotSmaller {
                block_index,
                raw_expected: expected_raw_size,
                compressed,
            });
        }

        let raw =
            zstd_decompress(entry.payload()).map_err(|zstd_err| BlockError::ZstdDecodeFailed {
                block_index,
                zstd_err,
            })?;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "decoded payload size is bounded to BLOCK_SIZE_RAW <= u16::MAX"
        )]
        let got = raw.len() as u16;
        if got != expected_raw_size {
            return Err(BlockError::RawBlockSizeMismatch {
                block_index,
                expected: expected_raw_size,
                got,
            });
        }
        Ok(raw)
    } else {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "payload_size <= 8192 by parse invariant"
        )]
        let got = entry.payload().len() as u16;
        if got != expected_raw_size {
            return Err(BlockError::RawBlockSizeMismatch {
                block_index,
                expected: expected_raw_size,
                got,
            });
        }
        Ok(entry.payload().to_vec())
    }
}

/// Compress with `zstd::bulk::Compressor` and disable contentSize, checksum, and dictID.
fn zstd_compress(raw: &[u8], level: i32) -> Result<Vec<u8>, String> {
    use zstd::zstd_safe::CParameter;

    let mut compressor = zstd::bulk::Compressor::new(level).map_err(|e| e.to_string())?;
    compressor
        .set_parameter(CParameter::ContentSizeFlag(false))
        .map_err(|e| e.to_string())?;
    compressor
        .set_parameter(CParameter::ChecksumFlag(false))
        .map_err(|e| e.to_string())?;
    compressor
        .set_parameter(CParameter::DictIdFlag(false))
        .map_err(|e| e.to_string())?;
    compressor.compress(raw).map_err(|e| e.to_string())
}

/// Decompress with `zstd::bulk::Decompressor`. The output buffer is sized to
/// `BLOCK_SIZE_RAW` because each block has at most 8192 original bytes.
fn zstd_decompress(compressed: &[u8]) -> Result<Vec<u8>, String> {
    let mut decompressor = zstd::bulk::Decompressor::new().map_err(|e| e.to_string())?;
    decompressor
        .decompress(compressed, BLOCK_SIZE_RAW)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::ZSTD_LEVEL_DEFAULT;

    #[test]
    fn highly_compressible_picks_compressed() {
        let raw = vec![0u8; BLOCK_SIZE_RAW];
        let entry = encode_block(&raw, ZSTD_LEVEL_DEFAULT, 0).unwrap();
        assert!(entry.is_compressed());
        assert!(entry.payload().len() < raw.len());
    }

    #[test]
    fn incompressible_falls_back_to_raw() {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut raw = vec![0u8; BLOCK_SIZE_RAW];
        rng.fill_bytes(&mut raw);
        let entry = encode_block(&raw, ZSTD_LEVEL_DEFAULT, 0).unwrap();
        assert!(!entry.is_compressed());
        assert_eq!(entry.payload(), raw);
    }

    #[test]
    fn roundtrip_compressible() {
        let raw = b"Lorem ipsum dolor sit amet ".repeat(200);
        let entry = encode_block(&raw, ZSTD_LEVEL_DEFAULT, 0).unwrap();
        let decoded = decode_block(&entry, 0, u16::try_from(raw.len()).unwrap()).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn roundtrip_short_random() {
        let raw = vec![0xAB, 0xCD, 0x42, 0x99];
        let entry = encode_block(&raw, ZSTD_LEVEL_DEFAULT, 0).unwrap();
        let decoded = decode_block(&entry, 0, u16::try_from(raw.len()).unwrap()).unwrap();
        assert_eq!(decoded, raw);
    }

    #[test]
    fn raw_block_size_mismatch_rejected() {
        let entry = BlockEntry::raw(vec![0x55; 32]).unwrap();
        assert!(matches!(
            decode_block(&entry, 7, 31),
            Err(BlockError::RawBlockSizeMismatch {
                block_index: 7,
                expected: 31,
                got: 32,
            })
        ));
    }

    #[test]
    fn compressed_payload_must_be_smaller_than_raw() {
        let entry = BlockEntry::compressed(vec![0xAA; 10]).unwrap();
        assert!(matches!(
            decode_block(&entry, 3, 10),
            Err(BlockError::CompressedPayloadNotSmaller {
                block_index: 3,
                raw_expected: 10,
                compressed: 10,
            })
        ));
    }
}
