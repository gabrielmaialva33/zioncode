//! Raw file blocking and per-block compression.

use crate::constants::{BLOCK_SIZE_RAW, MAX_TOTAL_BLOCKS};
use crate::error::EncodeError;
use crate::format::BlockEntry;
use crate::zstd_layer::encode_block;

/// Split the file into 8192-byte blocks, compressing each block with
/// `compressed < raw` fallback.
///
/// # Errors
/// - `EncodeError::EmptyInput` if `raw_file` is empty.
/// - `EncodeError::TooManyBlocks` if the implementation limit is exceeded.
/// - `EncodeError::ZstdEncodeFailed` if zstd fails for any block.
pub fn split_file_into_blocks(
    raw_file: &[u8],
    zstd_level: i32,
) -> Result<Vec<BlockEntry>, EncodeError> {
    if raw_file.is_empty() {
        return Err(EncodeError::EmptyInput);
    }

    let total_blocks = raw_file.len().div_ceil(BLOCK_SIZE_RAW);
    if total_blocks > MAX_TOTAL_BLOCKS as usize {
        return Err(EncodeError::TooManyBlocks {
            got: total_blocks,
            max: MAX_TOTAL_BLOCKS,
        });
    }

    let mut blocks = Vec::with_capacity(total_blocks);
    for (i, chunk) in raw_file.chunks(BLOCK_SIZE_RAW).enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "total_blocks <= MAX_TOTAL_BLOCKS = 2^20 fits u32"
        )]
        let entry = encode_block(chunk, zstd_level, i as u32)?;
        blocks.push(entry);
    }
    Ok(blocks)
}
