//! Symbol packing policy for the file encoder.

use crate::constants::{BLOCK_HEADER_LEN, HEADER_LEN_V1, TARGET_BLOCKS_PER_SYMBOL};
use crate::ecc::EccProfile;
use crate::error::EncodeError;
use crate::format::BlockEntry;

/// Contiguous group of blocks that will occupy one symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPacking {
    pub block_start: u32,
    pub block_count: u16,
}

/// Implements the contiguous greedy algorithm from spec 3.3.
///
/// # Errors
/// Returns `EncodeError::InsufficientSymbolCapacity` if any single block does
/// not fit in `k * RS_K - HEADER_LEN_V1` bytes. The caller (subsystem B) must
/// increase `k` or reduce physical symbol density.
pub fn pack_blocks_into_symbols(
    blocks: &[BlockEntry],
    k: u16,
) -> Result<Vec<SymbolPacking>, EncodeError> {
    pack_blocks_into_symbols_with_profile(blocks, k, EccProfile::Safe)
}

/// Implements the contiguous greedy algorithm for a selected ECC profile.
///
/// # Errors
/// Returns `EncodeError::InsufficientSymbolCapacity` if any single block does
/// not fit in `k * profile.data_len() - HEADER_LEN_V1` bytes.
pub fn pack_blocks_into_symbols_with_profile(
    blocks: &[BlockEntry],
    k: u16,
    profile: EccProfile,
) -> Result<Vec<SymbolPacking>, EncodeError> {
    let available_capacity = usize::from(k) * profile.data_len();
    let mut symbols = Vec::new();
    let mut current_start: u32 = 0;
    let mut current_count: u16 = 0;
    let mut current_size: usize = HEADER_LEN_V1;
    let mut next_block: u32 = 0;

    while (next_block as usize) < blocks.len() {
        let block_cost = BLOCK_HEADER_LEN + blocks[next_block as usize].payload().len();
        let fits = current_size + block_cost <= available_capacity;
        let full = current_count as usize == TARGET_BLOCKS_PER_SYMBOL;

        if full {
            symbols.push(SymbolPacking {
                block_start: current_start,
                block_count: current_count,
            });
            current_start = next_block;
            current_count = 0;
            current_size = HEADER_LEN_V1;
        } else if fits {
            current_count += 1;
            current_size += block_cost;
            next_block += 1;
        } else if current_count == 0 {
            let min_k_required =
                u16::try_from((HEADER_LEN_V1 + block_cost).div_ceil(profile.data_len()))
                    .unwrap_or(u16::MAX);
            return Err(EncodeError::InsufficientSymbolCapacity {
                k,
                min_k_required,
                block_index: next_block,
            });
        } else {
            symbols.push(SymbolPacking {
                block_start: current_start,
                block_count: current_count,
            });
            current_start = next_block;
            current_count = 0;
            current_size = HEADER_LEN_V1;
        }
    }

    if current_count > 0 {
        symbols.push(SymbolPacking {
            block_start: current_start,
            block_count: current_count,
        });
    }

    Ok(symbols)
}
