//! Single-symbol encoder. Combines header + blocks + padding + RS + interleave.
//! See spec section 6.2.

use crate::constants::{
    BLOCK_HEADER_LEN, BLOCK_SIZE_RAW, HEADER_LEN_V1, MAX_K, MAX_TOTAL_BLOCKS, MAX_TOTAL_SYMBOLS,
    RS_K, RS_N, TARGET_BLOCKS_PER_SYMBOL,
};
use crate::ecc::{interleave_column_major, rs_encode_codeword};
use crate::error::EncodeError;
use crate::format::{BlockEntry, SymbolHeader};
use crate::zstd_layer::encode_block;
use uuid::Uuid;

/// Pack the header plus blocks into the pre-ECC buffer, zero-pad to `K*223`,
/// then apply RS and interleave.
///
/// # Panics
/// Panics if `pre_ecc_stream.len() > k * RS_K`. The caller must ensure the
/// header plus blocks fit within the `k * RS_K` budget before calling.
#[must_use]
pub fn encode_single_symbol(header: &SymbolHeader, blocks: &[BlockEntry], k: usize) -> Vec<u8> {
    debug_assert!(k > 0);

    let mut pre_ecc = Vec::with_capacity(k * RS_K);
    pre_ecc.extend_from_slice(&header.serialize_v1());
    for block in blocks {
        pre_ecc.extend_from_slice(&block.serialize());
    }
    assert!(
        pre_ecc.len() <= k * RS_K,
        "pre_ecc overflow: {} > {}*223",
        pre_ecc.len(),
        k
    );
    pre_ecc.resize(k * RS_K, 0);

    let mut codewords: Vec<[u8; RS_N]> = Vec::with_capacity(k);
    for chunk in pre_ecc.chunks_exact(RS_K) {
        let data: &[u8; RS_K] = chunk.try_into().unwrap();
        codewords.push(rs_encode_codeword(data));
    }

    interleave_column_major(&codewords)
}

/// Split the file into 8192-byte blocks (the last one may be smaller),
/// compress each block with `C < R` fallback, and return serializable
/// `BlockEntry`s.
///
/// # Errors
/// - `EncodeError::EmptyInput` if `raw_file` is empty (unsupported in v1).
/// - `EncodeError::ZstdEncodeFailed` if zstd fails for any block.
pub fn split_file_into_blocks(
    raw_file: &[u8],
    zstd_level: i32,
) -> Result<Vec<BlockEntry>, EncodeError> {
    if raw_file.is_empty() {
        return Err(EncodeError::EmptyInput);
    }

    let mut blocks = Vec::with_capacity(raw_file.len().div_ceil(BLOCK_SIZE_RAW));
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
    let available_capacity = usize::from(k) * RS_K;
    let mut symbols = Vec::new();
    let mut current_start: u32 = 0;
    let mut current_count: u16 = 0;
    let mut current_size: usize = HEADER_LEN_V1;
    let mut next_block: u32 = 0;

    while (next_block as usize) < blocks.len() {
        let block_cost = BLOCK_HEADER_LEN + blocks[next_block as usize].payload.len();
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
                u16::try_from((HEADER_LEN_V1 + block_cost).div_ceil(RS_K)).unwrap_or(u16::MAX);
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

/// Full encoder output: symbols ready for subsystem B plus metadata for
/// tracing and tests.
#[derive(Debug)]
pub struct EncodedFile {
    pub file_id: [u8; 16],
    pub global_hash: [u8; 32],
    pub symbols: Vec<Vec<u8>>,
}

/// Full encoder pipeline: split -> pack -> build per-symbol header -> serialize -> RS.
///
/// # Errors
/// Propagates `EncodeError::EmptyInput`, `ZstdEncodeFailed`, or
/// `InsufficientSymbolCapacity` as appropriate.
///
/// # Panics
/// Panics if `raw_file.len()` does not fit in u64 (impossible on 64-bit
/// targets), or if `blocks.len()` exceeds u32, or if `symbols.len()` exceeds
/// u16. These cases exceed the implementation limits
/// (`MAX_TOTAL_BLOCKS=2^20`, `MAX_TOTAL_SYMBOLS=2^14`) and should have been
/// rejected by higher layers.
pub fn encode_file(raw_file: &[u8], k: u16, zstd_level: i32) -> Result<EncodedFile, EncodeError> {
    if raw_file.is_empty() {
        return Err(EncodeError::EmptyInput);
    }
    validate_k(k)?;
    validate_raw_file_limits(raw_file.len())?;

    let file_id = *Uuid::new_v4().as_bytes();
    let global_hash: [u8; 32] = blake3::hash(raw_file).as_bytes().to_owned();

    let blocks = split_file_into_blocks(raw_file, zstd_level)?;
    let packings = pack_blocks_into_symbols(&blocks, k)?;
    validate_symbol_count(packings.len())?;

    let total_blocks = u32::try_from(blocks.len()).expect("blocks.len() fits u32");
    let total_symbols = u16::try_from(packings.len()).expect("packings.len() fits u16");

    let mut symbols = Vec::with_capacity(packings.len());
    for (idx, packing) in packings.iter().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "HEADER_LEN_V1 = 82 fits u8; idx < total_symbols fits u16"
        )]
        let header = SymbolHeader {
            header_len: HEADER_LEN_V1 as u8,
            flags: 0,
            file_id,
            file_size: raw_file.len() as u64,
            total_blocks,
            total_symbols,
            symbol_index: u16::try_from(idx).expect("idx fits u16"),
            block_start: packing.block_start,
            block_count: packing.block_count,
            global_hash,
        };
        let start = packing.block_start as usize;
        let end = start + packing.block_count as usize;
        let symbol_blocks = &blocks[start..end];
        let symbol_bytes = encode_single_symbol(&header, symbol_blocks, usize::from(k));
        symbols.push(symbol_bytes);
    }

    Ok(EncodedFile {
        file_id,
        global_hash,
        symbols,
    })
}

fn validate_k(k: u16) -> Result<(), EncodeError> {
    if usize::from(k) > MAX_K {
        return Err(EncodeError::KTooLarge { got: k, max: MAX_K });
    }
    Ok(())
}

fn validate_raw_file_limits(raw_len: usize) -> Result<(), EncodeError> {
    let total_blocks = raw_len.div_ceil(BLOCK_SIZE_RAW);
    if total_blocks > MAX_TOTAL_BLOCKS as usize {
        return Err(EncodeError::TooManyBlocks {
            got: total_blocks,
            max: MAX_TOTAL_BLOCKS,
        });
    }
    Ok(())
}

fn validate_symbol_count(total_symbols: usize) -> Result<(), EncodeError> {
    if total_symbols > usize::from(MAX_TOTAL_SYMBOLS) {
        return Err(EncodeError::TooManySymbols {
            got: total_symbols,
            max: MAX_TOTAL_SYMBOLS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_length_is_k_times_n() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "HEADER_LEN_V1 = 82 fits u8"
        )]
        let header = SymbolHeader {
            header_len: HEADER_LEN_V1 as u8,
            flags: 0,
            file_id: [0; 16],
            file_size: 100,
            total_blocks: 1,
            total_symbols: 1,
            symbol_index: 0,
            block_start: 0,
            block_count: 1,
            global_hash: [0; 32],
        };
        let block = BlockEntry {
            compressed: false,
            payload: vec![0x55u8; 100],
        };
        let output = encode_single_symbol(&header, &[block], 10);
        assert_eq!(output.len(), 10 * 255);
    }

    #[test]
    fn k_above_decoder_limit_is_rejected() {
        let raw = [0x42u8; 1];
        let too_large_k = u16::try_from(MAX_K + 1).unwrap();
        assert!(matches!(
            encode_file(&raw, too_large_k, crate::constants::ZSTD_LEVEL_DEFAULT),
            Err(EncodeError::KTooLarge { got, max })
                if got == too_large_k && max == MAX_K
        ));
    }

    #[test]
    fn too_many_blocks_is_rejected_before_allocation() {
        let raw_len = (MAX_TOTAL_BLOCKS as usize * BLOCK_SIZE_RAW) + 1;
        assert!(matches!(
            validate_raw_file_limits(raw_len),
            Err(EncodeError::TooManyBlocks { got, max })
                if got == MAX_TOTAL_BLOCKS as usize + 1 && max == MAX_TOTAL_BLOCKS
        ));
    }

    #[test]
    fn too_many_symbols_is_rejected() {
        let total_symbols = usize::from(MAX_TOTAL_SYMBOLS) + 1;
        assert!(matches!(
            validate_symbol_count(total_symbols),
            Err(EncodeError::TooManySymbols { got, max })
                if got == total_symbols && max == MAX_TOTAL_SYMBOLS
        ));
    }
}

#[cfg(test)]
mod split_tests {
    use super::*;
    use crate::constants::ZSTD_LEVEL_DEFAULT;

    #[test]
    fn split_exact_multiple() {
        let raw = vec![0u8; 16384]; // 2 full blocks
        let blocks = split_file_into_blocks(&raw, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn split_with_partial_last() {
        let raw = vec![0u8; 12000]; // 1 full block + 1 partial
        let blocks = split_file_into_blocks(&raw, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(
            split_file_into_blocks(&[], ZSTD_LEVEL_DEFAULT),
            Err(EncodeError::EmptyInput)
        ));
    }

    #[test]
    fn single_byte_file() {
        let blocks = split_file_into_blocks(&[0x42], ZSTD_LEVEL_DEFAULT).unwrap();
        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn large_file_has_expected_block_count() {
        let raw = vec![0u8; 100_000]; // 12 full + 1 partial = 13 blocks
        let blocks = split_file_into_blocks(&raw, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_eq!(blocks.len(), 13);
    }
}

#[cfg(test)]
mod pack_tests {
    use super::*;

    fn blocks_of_size(n: usize, payload_size: usize) -> Vec<BlockEntry> {
        (0..n)
            .map(|_| BlockEntry {
                compressed: false,
                payload: vec![0u8; payload_size],
            })
            .collect()
    }

    #[test]
    fn four_raw_blocks_fit_one_symbol() {
        let blocks = blocks_of_size(4, 8192);
        // 82 + 4*(7+8192) = 32878. K=148 * 223 = 33004. Cabe.
        let packings = pack_blocks_into_symbols(&blocks, 148).unwrap();
        assert_eq!(packings.len(), 1);
        assert_eq!(packings[0].block_count, 4);
    }

    #[test]
    fn splits_when_target_reached() {
        let blocks = blocks_of_size(9, 100); // pequenos, sempre cabem
        let packings = pack_blocks_into_symbols(&blocks, 148).unwrap();
        // TARGET = 4 → 9 blocos = [4, 4, 1]
        assert_eq!(packings.len(), 3);
        assert_eq!(packings[0].block_count, 4);
        assert_eq!(packings[1].block_count, 4);
        assert_eq!(packings[2].block_count, 1);
    }

    #[test]
    fn insufficient_capacity_errors() {
        let blocks = blocks_of_size(1, 8192);
        // K=30 → 30*223=6690 < 82+7+8192=8281
        match pack_blocks_into_symbols(&blocks, 30) {
            Err(EncodeError::InsufficientSymbolCapacity {
                k: 30,
                min_k_required,
                block_index: 0,
            }) => {
                assert_eq!(min_k_required, 38);
            }
            other => panic!("expected InsufficientSymbolCapacity, got {other:?}"),
        }
    }

    #[test]
    fn single_block_fits_k_38() {
        let blocks = blocks_of_size(1, 8192);
        let packings = pack_blocks_into_symbols(&blocks, 38).unwrap();
        assert_eq!(packings.len(), 1);
        assert_eq!(packings[0].block_count, 1);
    }

    #[test]
    fn determinism() {
        let blocks = blocks_of_size(20, 2000);
        let a = pack_blocks_into_symbols(&blocks, 50).unwrap();
        let b = pack_blocks_into_symbols(&blocks, 50).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn block_start_is_contiguous() {
        let blocks = blocks_of_size(10, 100);
        let packings = pack_blocks_into_symbols(&blocks, 148).unwrap();
        let mut expected_start: u32 = 0;
        for p in &packings {
            assert_eq!(p.block_start, expected_start);
            expected_start += u32::from(p.block_count);
        }
        assert_eq!(expected_start, 10);
    }
}

#[cfg(test)]
mod encode_file_tests {
    use super::*;
    use crate::constants::ZSTD_LEVEL_DEFAULT;

    #[test]
    fn encode_small_file_one_symbol() {
        let raw = b"hello world".repeat(100);
        let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_eq!(encoded.symbols.len(), 1);
        assert_eq!(encoded.symbols[0].len(), 38 * 255);
    }

    #[test]
    fn encode_large_file_multiple_symbols() {
        let raw = vec![0u8; 40_000]; // 5 blocks; target=4 -> at least 2 symbols
        let encoded = encode_file(&raw, 148, ZSTD_LEVEL_DEFAULT).unwrap();
        assert!(encoded.symbols.len() >= 2);
        for sym in &encoded.symbols {
            assert_eq!(sym.len(), 148 * 255);
        }
    }

    #[test]
    fn empty_input_rejected() {
        assert!(matches!(
            encode_file(&[], 148, ZSTD_LEVEL_DEFAULT),
            Err(EncodeError::EmptyInput)
        ));
    }

    #[test]
    fn file_id_and_hash_populated() {
        let raw = b"test content".repeat(50);
        let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_ne!(encoded.file_id, [0u8; 16]);
        let expected_hash: [u8; 32] = *blake3::hash(&raw).as_bytes();
        assert_eq!(encoded.global_hash, expected_hash);
    }

    #[test]
    fn file_id_unique_per_call() {
        let raw = b"same data";
        let a = encode_file(raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
        let b = encode_file(raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
        assert_ne!(a.file_id, b.file_id); // UUIDv4 random
        assert_eq!(a.global_hash, b.global_hash); // same content -> same hash
    }
}
