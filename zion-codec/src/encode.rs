//! Single-symbol encoder. Combines header + blocks + padding + RS + interleave.
//! See spec section 6.2.

mod blocks;
mod packing;

pub use blocks::split_file_into_blocks;
pub use packing::{SymbolPacking, pack_blocks_into_symbols, pack_blocks_into_symbols_with_profile};

use crate::constants::{BLOCK_SIZE_RAW, MAX_K, MAX_TOTAL_BLOCKS, MAX_TOTAL_SYMBOLS, RS_N};
use crate::ecc::{EccProfile, interleave_column_major, try_rs_encode_codeword_with_profile};
use crate::error::{BlockError, EncodeError, SymbolError};
use crate::format::{BlockEntry, SymbolHeader};
use crate::types::{FileId, GlobalHash, SymbolBytes};

/// Pack the header plus blocks into the pre-ECC buffer, zero-pad to `K*223`,
/// then apply RS and interleave.
///
/// # Panics
/// Panics if `pre_ecc_stream.len() > k * RS_K`. The caller must ensure the
/// header plus blocks fit within the `k * RS_K` budget before calling.
#[must_use]
pub fn encode_single_symbol(header: &SymbolHeader, blocks: &[BlockEntry], k: usize) -> Vec<u8> {
    encode_single_symbol_with_profile(header, blocks, k, EccProfile::Safe)
}

/// Fallible variant of [`encode_single_symbol`].
///
/// # Errors
/// Returns `EncodeError::InvalidK` for `K=0` or
/// `EncodeError::SymbolPayloadTooLarge` when the header and blocks do not fit
/// the selected symbol capacity.
pub fn try_encode_single_symbol(
    header: &SymbolHeader,
    blocks: &[BlockEntry],
    k: usize,
) -> Result<SymbolBytes, EncodeError> {
    try_encode_single_symbol_with_profile(header, blocks, k, EccProfile::Safe)
}

/// Pack, pad, RS-encode, and interleave one symbol with the selected ECC profile.
///
/// # Panics
/// Panics if `pre_ecc_stream.len() > k * profile.data_len()`. The caller must
/// ensure the header plus blocks fit within the profile capacity.
#[must_use]
pub fn encode_single_symbol_with_profile(
    header: &SymbolHeader,
    blocks: &[BlockEntry],
    k: usize,
    profile: EccProfile,
) -> Vec<u8> {
    try_encode_single_symbol_with_profile(header, blocks, k, profile)
        .expect("caller must provide a valid K and enough symbol capacity")
        .into_vec()
}

/// Fallible variant of [`encode_single_symbol_with_profile`].
///
/// # Errors
/// Returns `EncodeError::InvalidK` for `K=0` or
/// `EncodeError::SymbolPayloadTooLarge` when the header and blocks do not fit
/// the selected symbol capacity.
pub fn try_encode_single_symbol_with_profile(
    header: &SymbolHeader,
    blocks: &[BlockEntry],
    k: usize,
    profile: EccProfile,
) -> Result<SymbolBytes, EncodeError> {
    debug_assert!(k > 0);
    if k == 0 {
        return Err(EncodeError::InvalidK { got: 0 });
    }

    let data_len = profile.data_len();
    let mut pre_ecc = Vec::with_capacity(k * data_len);
    let header_bytes = header.try_serialize_v1().map_err(|err| match err {
        SymbolError::HeaderLengthInvalid { got } => EncodeError::InvalidHeaderLength {
            got,
            expected: crate::constants::HEADER_LEN_V1,
        },
        other => unreachable!("unexpected header serialization error: {other:?}"),
    })?;
    pre_ecc.extend_from_slice(&header_bytes);
    for block in blocks {
        let block_bytes = block.try_serialize().map_err(|err| match err {
            BlockError::PayloadSizeOutOfRange { got, max } => {
                EncodeError::BlockPayloadTooLarge { got, max }
            }
            other => unreachable!("unexpected block serialization error: {other:?}"),
        })?;
        pre_ecc.extend_from_slice(&block_bytes);
    }
    let capacity = k * data_len;
    if pre_ecc.len() > capacity {
        return Err(EncodeError::SymbolPayloadTooLarge {
            got: pre_ecc.len(),
            max: capacity,
        });
    }
    pre_ecc.resize(k * data_len, 0);

    let mut codewords: Vec<[u8; RS_N]> = Vec::with_capacity(k);
    for chunk in pre_ecc.chunks_exact(data_len) {
        codewords.push(try_rs_encode_codeword_with_profile(chunk, profile)?);
    }

    Ok(SymbolBytes::from_vec(interleave_column_major(&codewords)))
}

/// Full encoder output: symbols ready for subsystem B plus metadata for
/// tracing and tests.
#[derive(Debug)]
pub struct EncodedFile {
    pub file_id: FileId,
    pub global_hash: GlobalHash,
    pub k: u16,
    pub ecc_profile: EccProfile,
    pub symbols: Vec<SymbolBytes>,
}

/// Full encoder pipeline: split -> pack -> build per-symbol header -> serialize -> RS.
///
/// # Errors
/// Propagates `EncodeError::EmptyInput`, `ZstdEncodeFailed`, or
/// `InsufficientSymbolCapacity` as appropriate.
pub fn encode_file(raw_file: &[u8], k: u16, zstd_level: i32) -> Result<EncodedFile, EncodeError> {
    encode_file_with_profile(raw_file, k, zstd_level, EccProfile::Safe)
}

/// Full encoder pipeline with a selected ECC profile.
///
/// # Errors
/// Propagates `EncodeError::EmptyInput`, `ZstdEncodeFailed`,
/// `InsufficientSymbolCapacity`, or implementation-limit errors as appropriate.
pub fn encode_file_with_profile(
    raw_file: &[u8],
    k: u16,
    zstd_level: i32,
    profile: EccProfile,
) -> Result<EncodedFile, EncodeError> {
    if raw_file.is_empty() {
        return Err(EncodeError::EmptyInput);
    }
    validate_k(k)?;
    validate_raw_file_limits(raw_file.len())?;

    let blocks = split_file_into_blocks(raw_file, zstd_level)?;
    encode_prepared_blocks(raw_file, &blocks, k, profile)
}

/// Encode with an automatically selected `K` that minimizes total emitted bytes.
///
/// # Errors
/// Propagates the same errors as [`encode_file_with_profile`].
pub fn encode_file_auto_k(
    raw_file: &[u8],
    zstd_level: i32,
    profile: EccProfile,
) -> Result<EncodedFile, EncodeError> {
    if raw_file.is_empty() {
        return Err(EncodeError::EmptyInput);
    }
    validate_raw_file_limits(raw_file.len())?;

    let blocks = split_file_into_blocks(raw_file, zstd_level)?;
    let k = choose_auto_k_for_blocks(&blocks, profile)?;
    encode_prepared_blocks(raw_file, &blocks, k, profile)
}

/// Choose the `K` that minimizes total transmitted bytes for already prepared blocks.
///
/// Ties prefer the smaller `K`, which is better for small files and physical symbols.
///
/// # Errors
/// Returns capacity or symbol-limit errors when no valid K can encode the blocks.
pub fn choose_auto_k_for_blocks(
    blocks: &[BlockEntry],
    profile: EccProfile,
) -> Result<u16, EncodeError> {
    let max_k = u16::try_from(MAX_K).unwrap_or(u16::MAX);
    let mut best: Option<(u16, usize)> = None;
    let mut last_error: Option<EncodeError> = None;

    for k in 1..=max_k {
        match pack_blocks_into_symbols_with_profile(blocks, k, profile) {
            Ok(packings) => {
                if let Err(err) = validate_symbol_count(packings.len()) {
                    last_error = Some(err);
                    continue;
                }
                let total_bytes = packings.len() * usize::from(k) * RS_N;
                if best.is_none_or(|(_, best_bytes)| total_bytes < best_bytes) {
                    best = Some((k, total_bytes));
                }
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }

    best.map(|(k, _)| k).ok_or_else(|| {
        last_error.unwrap_or(EncodeError::TooManySymbols {
            got: usize::MAX,
            max: MAX_TOTAL_SYMBOLS,
        })
    })
}

fn encode_prepared_blocks(
    raw_file: &[u8],
    blocks: &[BlockEntry],
    k: u16,
    profile: EccProfile,
) -> Result<EncodedFile, EncodeError> {
    validate_k(k)?;
    let packings = pack_blocks_into_symbols_with_profile(blocks, k, profile)?;
    validate_symbol_count(packings.len())?;

    let total_blocks = u32::try_from(blocks.len()).map_err(|_| EncodeError::TooManyBlocks {
        got: blocks.len(),
        max: MAX_TOTAL_BLOCKS,
    })?;
    let total_symbols = u16::try_from(packings.len()).map_err(|_| EncodeError::TooManySymbols {
        got: packings.len(),
        max: MAX_TOTAL_SYMBOLS,
    })?;
    let file_id = FileId::new_random();
    let global_hash = GlobalHash::digest(raw_file);

    let mut symbols = Vec::with_capacity(packings.len());
    for (idx, packing) in packings.iter().enumerate() {
        let header = SymbolHeader::new_v1(
            profile,
            file_id,
            raw_file.len() as u64,
            total_blocks,
            total_symbols,
            u16::try_from(idx).map_err(|_| EncodeError::TooManySymbols {
                got: packings.len(),
                max: MAX_TOTAL_SYMBOLS,
            })?,
            packing.block_start,
            packing.block_count,
            global_hash,
        );
        let start = packing.block_start as usize;
        let end = start + packing.block_count as usize;
        let symbol_blocks = &blocks[start..end];
        let symbol_bytes =
            try_encode_single_symbol_with_profile(&header, symbol_blocks, usize::from(k), profile)?;
        symbols.push(symbol_bytes);
    }

    Ok(EncodedFile {
        file_id,
        global_hash,
        k,
        ecc_profile: profile,
        symbols,
    })
}

fn validate_k(k: u16) -> Result<(), EncodeError> {
    if k == 0 {
        return Err(EncodeError::InvalidK { got: k });
    }
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
        let header = SymbolHeader::new_v1(
            EccProfile::Safe,
            FileId::from_bytes([0; 16]),
            100,
            1,
            1,
            0,
            0,
            1,
            GlobalHash::from_bytes([0; 32]),
        );
        let block = BlockEntry::raw(vec![0x55u8; 100]).unwrap();
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

    #[test]
    fn auto_k_picks_tiny_symbol_for_tiny_file() {
        let raw = [0x42u8; 1];
        let encoded =
            encode_file_auto_k(&raw, crate::constants::ZSTD_LEVEL_DEFAULT, EccProfile::Safe)
                .unwrap();
        assert_eq!(encoded.symbols.len(), 1);
        assert_eq!(encoded.symbols[0].len(), RS_N);
    }

    #[test]
    fn dense_profile_uses_smaller_auto_symbol_than_safe_for_single_raw_block() {
        let blocks = vec![BlockEntry::raw(vec![0x42; BLOCK_SIZE_RAW]).unwrap()];
        let safe_k = choose_auto_k_for_blocks(&blocks, EccProfile::Safe).unwrap();
        let dense_k = choose_auto_k_for_blocks(&blocks, EccProfile::Dense).unwrap();
        assert!(dense_k < safe_k);
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
            .map(|_| BlockEntry::raw(vec![0u8; payload_size]).unwrap())
            .collect()
    }

    #[test]
    fn four_raw_blocks_fit_one_symbol() {
        let blocks = blocks_of_size(4, 8192);
        // 82 + 4*(7+8192) = 32878. K=148 * 223 = 33004. Fits.
        let packings = pack_blocks_into_symbols(&blocks, 148).unwrap();
        assert_eq!(packings.len(), 1);
        assert_eq!(packings[0].block_count, 4);
    }

    #[test]
    fn splits_when_target_reached() {
        let blocks = blocks_of_size(9, 100); // Small blocks always fit.
        let packings = pack_blocks_into_symbols(&blocks, 148).unwrap();
        // TARGET = 4 -> 9 blocks = [4, 4, 1]
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
