//! Single-symbol decoder. See spec section 6.3.

use crate::constants::{BLOCK_HEADER_LEN, MAX_SYMBOL_BYTES, RS_N};
use crate::ecc::{
    EccProfile, deinterleave_column_major, rs_decode_codeword_with_profile_and_erasures,
};
use crate::error::{BlockError, SymbolError};
use crate::format::{BlockEntry, SymbolHeader};

/// Result of decoding one standalone symbol.
#[derive(Debug)]
pub struct DecodedSymbol {
    pub header: SymbolHeader,
    /// Blocks that passed RS decoding, in order.
    /// Some may still fail at block level; see `block_failures`.
    pub blocks: Vec<Result<BlockEntry, BlockError>>,
}

/// Decode a full symbol from `symbol_bytes`.
///
/// # Errors
/// Returns `SymbolError` if ECC, magic, version, `header_len`, or CRC checks
/// fail. Block-level failures are reported inside `DecodedSymbol.blocks` as
/// `Err(_)` without failing the whole symbol.
pub fn decode_symbol(symbol_bytes: &[u8]) -> Result<DecodedSymbol, SymbolError> {
    decode_symbol_with_optional_erasures(symbol_bytes, None)
}

/// Decode a full symbol while marking known-bad byte offsets as erasures.
///
/// `erased_positions` are offsets in the interleaved symbol byte stream. This
/// lets optical/camera layers pass damaged module positions directly without
/// needing to understand the internal RS codeword layout.
///
/// # Errors
/// Returns `SymbolError` if size validation, erasure validation, ECC, magic,
/// version, `header_len`, or CRC checks fail. Block-level failures are reported
/// inside `DecodedSymbol.blocks` as `Err(_)` without failing the whole symbol.
pub fn decode_symbol_with_erasures(
    symbol_bytes: &[u8],
    erased_positions: &[usize],
) -> Result<DecodedSymbol, SymbolError> {
    let k = validate_symbol_bytes(symbol_bytes)?;
    let erasures_by_codeword = build_erasure_map(symbol_bytes.len(), k, erased_positions)?;
    decode_symbol_inner(symbol_bytes, k, Some(&erasures_by_codeword))
}

fn decode_symbol_with_optional_erasures(
    symbol_bytes: &[u8],
    erasures_by_codeword: Option<&[Vec<u8>]>,
) -> Result<DecodedSymbol, SymbolError> {
    let k = validate_symbol_bytes(symbol_bytes)?;
    decode_symbol_inner(symbol_bytes, k, erasures_by_codeword)
}

fn validate_symbol_bytes(symbol_bytes: &[u8]) -> Result<usize, SymbolError> {
    if symbol_bytes.is_empty() || !symbol_bytes.len().is_multiple_of(RS_N) {
        return Err(SymbolError::BadSize {
            got: symbol_bytes.len(),
        });
    }
    if symbol_bytes.len() > MAX_SYMBOL_BYTES {
        return Err(SymbolError::SymbolByteLengthTooLarge {
            got: symbol_bytes.len(),
            max: MAX_SYMBOL_BYTES,
        });
    }

    Ok(symbol_bytes.len() / RS_N)
}

fn build_erasure_map(
    symbol_len: usize,
    k: usize,
    erased_positions: &[usize],
) -> Result<Vec<Vec<u8>>, SymbolError> {
    let mut erasures_by_codeword = vec![Vec::new(); k];
    for &position in erased_positions {
        if position >= symbol_len {
            return Err(SymbolError::ErasurePositionOutOfRange {
                position,
                symbol_len,
            });
        }
        let codeword_index = position % k;
        let codeword_position = position / k;
        debug_assert!(codeword_position < RS_N);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "codeword_position < RS_N = 255 fits u8"
        )]
        erasures_by_codeword[codeword_index].push(codeword_position as u8);
    }

    Ok(erasures_by_codeword)
}

fn decode_symbol_inner(
    symbol_bytes: &[u8],
    k: usize,
    erasures_by_codeword: Option<&[Vec<u8>]>,
) -> Result<DecodedSymbol, SymbolError> {
    let mut first_error: Option<SymbolError> = None;
    for profile in EccProfile::all() {
        match decode_symbol_with_profile(symbol_bytes, k, profile, erasures_by_codeword) {
            Ok(Some(decoded)) => return Ok(decoded),
            Ok(None) => {}
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }

    Err(first_error.unwrap_or(SymbolError::HeaderCrcMismatch))
}

fn decode_symbol_with_profile(
    symbol_bytes: &[u8],
    k: usize,
    profile: EccProfile,
    erasures_by_codeword: Option<&[Vec<u8>]>,
) -> Result<Option<DecodedSymbol>, SymbolError> {
    let codewords = deinterleave_column_major(symbol_bytes, k);
    let mut pre_ecc = Vec::with_capacity(k * profile.data_len());
    for (j, cw) in codewords.iter().enumerate() {
        let erasures = erasures_by_codeword
            .and_then(|by_codeword| by_codeword.get(j))
            .map_or(&[][..], Vec::as_slice);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "k <= MAX_K = 4112 fits u16"
        )]
        let chunk = rs_decode_codeword_with_profile_and_erasures(cw, profile, erasures, j as u16)?;
        pre_ecc.extend_from_slice(&chunk);
    }

    let (header, header_bytes_consumed) = SymbolHeader::parse(&pre_ecc)?;
    if EccProfile::from_header_flags(header.flags) != Some(profile) {
        return Ok(None);
    }
    if header
        .block_start
        .checked_add(u32::from(header.block_count))
        .is_none()
    {
        return Err(SymbolError::BlockRangeOverflow {
            block_start: header.block_start,
            block_count: header.block_count,
        });
    }

    let mut offset = header_bytes_consumed;
    let mut blocks: Vec<Result<BlockEntry, BlockError>> =
        Vec::with_capacity(header.block_count as usize);
    for i in 0..header.block_count {
        let global_idx = header.block_start + u32::from(i);
        if offset + BLOCK_HEADER_LEN > pre_ecc.len() {
            blocks.push(Err(BlockError::PayloadSizeTooLarge {
                block_index: global_idx,
                got: 0,
            }));
            break;
        }
        match BlockEntry::parse(&pre_ecc[offset..], global_idx) {
            Ok((entry, consumed)) => {
                offset += consumed;
                blocks.push(Ok(entry));
            }
            Err(e) => {
                blocks.push(Err(e));
                break;
            }
        }
    }

    Ok(Some(DecodedSymbol { header, blocks }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::HEADER_LEN_V1;
    use crate::encode::{encode_single_symbol, encode_single_symbol_with_profile};

    #[test]
    fn roundtrip_single_symbol() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "HEADER_LEN_V1 = 82 fits u8"
        )]
        let header = SymbolHeader {
            header_len: HEADER_LEN_V1 as u8,
            flags: 0,
            file_id: [0xAA; 16],
            file_size: 50,
            total_blocks: 1,
            total_symbols: 1,
            symbol_index: 0,
            block_start: 0,
            block_count: 1,
            global_hash: [0xBB; 32],
        };
        let block = BlockEntry {
            compressed: false,
            payload: (0u8..50).collect(),
        };
        let symbol_bytes = encode_single_symbol(&header, std::slice::from_ref(&block), 5);
        let decoded = decode_symbol(&symbol_bytes).unwrap();
        assert_eq!(decoded.header.file_size, 50);
        assert_eq!(decoded.blocks.len(), 1);
        assert_eq!(decoded.blocks[0].as_ref().unwrap(), &block);
    }

    #[test]
    fn bad_size_rejected() {
        let bytes = vec![0u8; 100]; // not a multiple of 255
        assert!(matches!(
            decode_symbol(&bytes),
            Err(SymbolError::BadSize { got: 100 })
        ));
    }

    #[test]
    fn empty_input_rejected() {
        let bytes = vec![];
        assert!(matches!(
            decode_symbol(&bytes),
            Err(SymbolError::BadSize { got: 0 })
        ));
    }

    #[test]
    fn too_large_rejected() {
        // Buffer is a multiple of RS_N, but larger than MAX_SYMBOL_BYTES.
        // MAX_K * RS_N <= MAX_SYMBOL_BYTES, so (MAX_K + 1) * RS_N exceeds it.
        use crate::constants::MAX_K;
        let bytes = vec![0u8; (MAX_K + 1) * RS_N];
        assert!(matches!(
            decode_symbol(&bytes),
            Err(SymbolError::SymbolByteLengthTooLarge { .. })
        ));
    }

    #[test]
    fn overflowing_block_range_is_rejected_without_panic() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "HEADER_LEN_V1 = 82 fits u8"
        )]
        let header = SymbolHeader {
            header_len: HEADER_LEN_V1 as u8,
            flags: 0,
            file_id: [0xAA; 16],
            file_size: 1,
            total_blocks: u32::MAX,
            total_symbols: 1,
            symbol_index: 0,
            block_start: u32::MAX,
            block_count: 1,
            global_hash: [0xBB; 32],
        };

        let symbol_bytes = encode_single_symbol(&header, &[], 1);
        assert!(matches!(
            decode_symbol(&symbol_bytes),
            Err(SymbolError::BlockRangeOverflow {
                block_start: u32::MAX,
                block_count: 1,
            })
        ));
    }

    #[test]
    fn decodes_all_ecc_profiles() {
        for profile in EccProfile::all() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "HEADER_LEN_V1 = 82 fits u8"
            )]
            let header = SymbolHeader {
                header_len: HEADER_LEN_V1 as u8,
                flags: profile.header_flags(),
                file_id: [0xAA; 16],
                file_size: 50,
                total_blocks: 1,
                total_symbols: 1,
                symbol_index: 0,
                block_start: 0,
                block_count: 1,
                global_hash: [0xBB; 32],
            };
            let block = BlockEntry {
                compressed: false,
                payload: (0u8..50).collect(),
            };
            let symbol_bytes = encode_single_symbol_with_profile(
                &header,
                std::slice::from_ref(&block),
                5,
                profile,
            );
            let decoded = decode_symbol(&symbol_bytes).unwrap();
            assert_eq!(decoded.header.flags, profile.header_flags());
            assert_eq!(decoded.blocks[0].as_ref().unwrap(), &block);
        }
    }

    #[test]
    fn decodes_symbol_with_known_erasures() {
        for profile in EccProfile::all() {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "HEADER_LEN_V1 = 82 fits u8"
            )]
            let header = SymbolHeader {
                header_len: HEADER_LEN_V1 as u8,
                flags: profile.header_flags(),
                file_id: [0xAA; 16],
                file_size: 50,
                total_blocks: 1,
                total_symbols: 1,
                symbol_index: 0,
                block_start: 0,
                block_count: 1,
                global_hash: [0xBB; 32],
            };
            let block = BlockEntry {
                compressed: false,
                payload: (0u8..50).collect(),
            };
            let k = 5;
            let mut symbol_bytes = encode_single_symbol_with_profile(
                &header,
                std::slice::from_ref(&block),
                k,
                profile,
            );
            let erasures: Vec<usize> = (0..profile.parity_len()).map(|r| r * k).collect();

            for &position in &erasures {
                symbol_bytes[position] ^= 0xA5;
            }

            let decoded = decode_symbol_with_erasures(&symbol_bytes, &erasures).unwrap();
            assert_eq!(decoded.header.flags, profile.header_flags());
            assert_eq!(decoded.blocks[0].as_ref().unwrap(), &block);
        }
    }

    #[test]
    fn rejects_erasure_position_outside_symbol() {
        let bytes = vec![0u8; RS_N];

        assert!(matches!(
            decode_symbol_with_erasures(&bytes, &[RS_N]),
            Err(SymbolError::ErasurePositionOutOfRange {
                position,
                symbol_len: RS_N,
            }) if position == RS_N
        ));
    }

    #[test]
    fn rejects_too_many_symbol_erasures_in_one_codeword() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "HEADER_LEN_V1 = 82 fits u8"
        )]
        let header = SymbolHeader {
            header_len: HEADER_LEN_V1 as u8,
            flags: EccProfile::Safe.header_flags(),
            file_id: [0xAA; 16],
            file_size: 50,
            total_blocks: 1,
            total_symbols: 1,
            symbol_index: 0,
            block_start: 0,
            block_count: 1,
            global_hash: [0xBB; 32],
        };
        let block = BlockEntry {
            compressed: false,
            payload: (0u8..50).collect(),
        };
        let k = 5;
        let symbol_bytes = encode_single_symbol(&header, &[block], k);
        let erasures: Vec<usize> = (0..=EccProfile::Safe.parity_len()).map(|r| r * k).collect();

        assert!(matches!(
            decode_symbol_with_erasures(&symbol_bytes, &erasures),
            Err(SymbolError::TooManyErasures {
                codeword_index: 0,
                got: 33,
                max: 32,
            })
        ));
    }
}
