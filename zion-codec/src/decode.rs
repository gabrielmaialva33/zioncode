//! Single-symbol decoder. See spec section 6.3.

use crate::constants::{BLOCK_HEADER_LEN, MAX_SYMBOL_BYTES, RS_N};
use crate::ecc::{EccProfile, deinterleave_column_major, rs_decode_codeword_with_profile};
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

    let k = symbol_bytes.len() / RS_N;
    let mut first_error: Option<SymbolError> = None;
    for profile in EccProfile::all() {
        match decode_symbol_with_profile(symbol_bytes, k, profile) {
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
) -> Result<Option<DecodedSymbol>, SymbolError> {
    let codewords = deinterleave_column_major(symbol_bytes, k);
    let mut pre_ecc = Vec::with_capacity(k * profile.data_len());
    for (j, cw) in codewords.iter().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "k <= MAX_K = 4112 fits u16"
        )]
        let chunk = rs_decode_codeword_with_profile(cw, profile, j as u16)?;
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
}
