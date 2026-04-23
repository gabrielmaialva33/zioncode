//! Reed-Solomon RS(255, 223) over GF(256) plus column-major interleaving.
//! See spec section 6.
//!
//! Implemented through the `reed-solomon` crate (BCH over GF(256)).
//! `reed-solomon-simd` does not fit here because it operates on GF(2^16) and
//! requires even `shard_bytes >= 2`, which breaks the 255-byte codeword contract.

use crate::constants::{RS_K, RS_N, RS_PARITY};
use crate::error::SymbolError;
use reed_solomon::{Decoder, Encoder};

/// Encode 223 data bytes into 255 bytes (223 + 32 parity).
///
/// Output: `data[0..223]` followed by 32 parity bytes (Reed-Solomon BCH, GF(256)).
#[must_use]
pub fn rs_encode_codeword(data: &[u8; RS_K]) -> [u8; RS_N] {
    let encoder = Encoder::new(RS_PARITY);
    let codeword = encoder.encode(data);

    let mut out = [0u8; RS_N];
    out[..RS_K].copy_from_slice(&codeword[..RS_K]);
    out[RS_K..].copy_from_slice(&codeword[RS_K..RS_N]);
    out
}

/// Decode 255 bytes back into the original 223 data bytes.
/// Tolerates up to 16 bad bytes (half of the 32 parity bytes).
///
/// # Errors
/// Returns `SymbolError::RsDecodeFailed` if RS cannot correct the codeword
/// (that is, when errors exceed the correction budget).
pub fn rs_decode_codeword(
    code: &[u8; RS_N],
    codeword_index: u16,
) -> Result<[u8; RS_K], SymbolError> {
    let decoder = Decoder::new(RS_PARITY);
    let buffer = decoder
        .correct(code, None)
        .map_err(|_| SymbolError::RsDecodeFailed { codeword_index })?;

    let data = buffer.data();
    debug_assert_eq!(data.len(), RS_K);

    let mut out = [0u8; RS_K];
    out.copy_from_slice(data);
    Ok(out)
}

/// Column-major interleave: given K codewords of `RS_N` bytes each, produce a
/// `K * RS_N` bytes where `output[r * K + j] = codewords[j][r]`.
///
/// Equivalent to `output[i] = codewords[i % K][i / K]`.
/// The goal is to spread a burst of consecutive channel bytes into one
/// corrupted byte per codeword after deinterleave, keeping the burst
/// within the RS budget.
#[must_use]
pub fn interleave_column_major(codewords: &[[u8; RS_N]]) -> Vec<u8> {
    let k = codewords.len();
    let mut out = vec![0u8; k * RS_N];
    for r in 0..RS_N {
        for (j, cw) in codewords.iter().enumerate() {
            out[r * k + j] = cw[r];
        }
    }
    out
}

/// Inverse of [`interleave_column_major`]. Given a `K * RS_N` byte buffer,
/// returns K codewords of `RS_N` bytes.
///
/// # Panics
/// Panics if `symbol_bytes.len() != k * RS_N`.
#[must_use]
pub fn deinterleave_column_major(symbol_bytes: &[u8], k: usize) -> Vec<[u8; RS_N]> {
    assert_eq!(symbol_bytes.len(), k * RS_N);
    let mut out = vec![[0u8; RS_N]; k];
    for r in 0..RS_N {
        for (j, cw) in out.iter_mut().enumerate() {
            cw[r] = symbol_bytes[r * k + j];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_preserves_data_prefix() {
        let data = [0x42u8; RS_K];
        let code = rs_encode_codeword(&data);
        assert_eq!(&code[..RS_K], &data[..]);
    }

    #[test]
    fn encode_different_data_produces_different_parity() {
        let code_a = rs_encode_codeword(&[0u8; RS_K]);
        let mut data_b = [0u8; RS_K];
        data_b[0] = 1;
        let code_b = rs_encode_codeword(&data_b);
        assert_ne!(&code_a[RS_K..], &code_b[RS_K..]);
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    #[test]
    fn rs_roundtrip_clean() {
        let data = [0x42u8; RS_K];
        let code = rs_encode_codeword(&data);
        let recovered = rs_decode_codeword(&code, 0).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test index is always < 256 and fits in u8"
    )]
    fn rs_tolerates_up_to_16_errors() {
        let data: [u8; RS_K] = std::array::from_fn(|i| i as u8);
        let mut code = rs_encode_codeword(&data);
        // Corrupt 16 bytes
        for i in 0..16 {
            code[i * 10] ^= 0xFF;
        }
        let recovered = rs_decode_codeword(&code, 0).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn rs_fails_above_correction_budget() {
        let data = [0u8; RS_K];
        let mut code = rs_encode_codeword(&data);
        // 17 errors, above the correction budget
        for i in 0..17 {
            code[i * 5] ^= 0xFF;
        }
        match rs_decode_codeword(&code, 42) {
            Err(SymbolError::RsDecodeFailed { codeword_index: 42 }) => {}
            Ok(recovered) if recovered != data => {
                // acceptable miscorrection: RS may "correct" to the wrong data
            }
            Ok(recovered) if recovered == data => {
                panic!("decoder returned correct data with 17 errors, which should be impossible");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test values are always < 256 and fit in u8"
    )]
    fn interleave_roundtrip() {
        let codewords: Vec<[u8; RS_N]> = (0..10)
            .map(|j| std::array::from_fn(|r| (j * 100 + r) as u8))
            .collect();
        let interleaved = interleave_column_major(&codewords);
        assert_eq!(interleaved.len(), 10 * RS_N);
        let back = deinterleave_column_major(&interleaved, 10);
        assert_eq!(back, codewords);
    }

    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "j is always < 148 and fits in u16"
    )]
    fn burst_error_spread_across_codewords() {
        let codewords: Vec<[u8; RS_N]> = (0..148)
            .map(|_| {
                let data = [0xAAu8; RS_K];
                rs_encode_codeword(&data)
            })
            .collect();
        let mut interleaved = interleave_column_major(&codewords);
        // A 100-byte consecutive burst should become 1 bad byte in each
        // of 100 codewords after deinterleave.
        for byte in &mut interleaved[1000..1100] {
            *byte ^= 0xFF;
        }
        let deinterleaved = deinterleave_column_major(&interleaved, 148);
        for (j, cw) in deinterleaved.iter().enumerate() {
            let recovered = rs_decode_codeword(cw, j as u16).unwrap();
            assert_eq!(recovered, [0xAAu8; RS_K]);
        }
    }
}
