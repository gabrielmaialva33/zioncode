//! Reed-Solomon RS(255, K) over GF(256) plus column-major interleaving.
//! See spec section 6.
//!
//! Implemented through the `reed-solomon` crate (BCH over GF(256)).
//! `reed-solomon-simd` does not fit here because it operates on GF(2^16) and
//! requires even `shard_bytes >= 2`, which breaks the 255-byte codeword contract.

use crate::constants::{RS_K, RS_N};
use crate::error::{EncodeError, SymbolError};
use reed_solomon::{Decoder, Encoder};

/// Reed-Solomon density/robustness profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EccProfile {
    /// RS(255,223): 32 parity bytes, corrects up to 16 unknown bad bytes.
    Safe,
    /// RS(255,239): 16 parity bytes, corrects up to 8 unknown bad bytes.
    Balanced,
    /// RS(255,247): 8 parity bytes, corrects up to 4 unknown bad bytes.
    Dense,
}

impl EccProfile {
    #[must_use]
    pub const fn data_len(self) -> usize {
        match self {
            Self::Safe => 223,
            Self::Balanced => 239,
            Self::Dense => 247,
        }
    }

    #[must_use]
    pub const fn parity_len(self) -> usize {
        RS_N - self.data_len()
    }

    #[must_use]
    pub const fn correction_budget(self) -> usize {
        self.parity_len() / 2
    }

    #[must_use]
    pub const fn header_flags(self) -> u16 {
        match self {
            Self::Safe => 0,
            Self::Balanced => 1,
            Self::Dense => 2,
        }
    }

    #[must_use]
    pub const fn from_header_flags(flags: u16) -> Option<Self> {
        match flags {
            0 => Some(Self::Safe),
            1 => Some(Self::Balanced),
            2 => Some(Self::Dense),
            _ => None,
        }
    }

    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Safe, Self::Balanced, Self::Dense]
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Balanced => "balanced",
            Self::Dense => "dense",
        }
    }
}

/// Encode 223 data bytes into 255 bytes (223 + 32 parity).
///
/// Output: `data[0..223]` followed by 32 parity bytes (Reed-Solomon BCH, GF(256)).
#[must_use]
pub fn rs_encode_codeword(data: &[u8; RS_K]) -> [u8; RS_N] {
    rs_encode_codeword_with_profile(data, EccProfile::Safe)
}

/// Encode one codeword using the selected profile.
///
/// # Panics
/// Panics if `data.len() != profile.data_len()`.
#[must_use]
pub fn rs_encode_codeword_with_profile(data: &[u8], profile: EccProfile) -> [u8; RS_N] {
    try_rs_encode_codeword_with_profile(data, profile)
        .expect("caller must provide data matching the selected RS profile")
}

/// Encode one codeword using the selected profile.
///
/// # Errors
/// Returns `EncodeError::InvalidCodewordDataLength` if `data.len()` does not
/// match the profile's data length.
pub fn try_rs_encode_codeword_with_profile(
    data: &[u8],
    profile: EccProfile,
) -> Result<[u8; RS_N], EncodeError> {
    if data.len() != profile.data_len() {
        return Err(EncodeError::InvalidCodewordDataLength {
            profile: profile.name(),
            got: data.len(),
            expected: profile.data_len(),
        });
    }
    let encoder = Encoder::new(profile.parity_len());
    let codeword = encoder.encode(data);

    let mut out = [0u8; RS_N];
    let data_len = profile.data_len();
    out[..data_len].copy_from_slice(&codeword[..data_len]);
    out[data_len..].copy_from_slice(&codeword[data_len..RS_N]);
    Ok(out)
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
    let data =
        rs_decode_codeword_with_profile_and_erasures(code, EccProfile::Safe, &[], codeword_index)?;
    let mut out = [0u8; RS_K];
    out.copy_from_slice(&data);
    Ok(out)
}

/// Decode 255 bytes back into the original 223 data bytes with known erasures.
///
/// Erasure positions are byte offsets inside the 255-byte codeword. A known
/// erasure costs one parity byte, while an unknown error costs two.
///
/// # Errors
/// Returns `SymbolError::TooManyErasures` if erasures exceed the parity budget,
/// or `SymbolError::RsDecodeFailed` if remaining damage is not recoverable.
pub fn rs_decode_codeword_with_erasures(
    code: &[u8; RS_N],
    erasures: &[u8],
    codeword_index: u16,
) -> Result<[u8; RS_K], SymbolError> {
    let data = rs_decode_codeword_with_profile_and_erasures(
        code,
        EccProfile::Safe,
        erasures,
        codeword_index,
    )?;
    let mut out = [0u8; RS_K];
    out.copy_from_slice(&data);
    Ok(out)
}

/// Decode one codeword using the selected profile.
///
/// # Errors
/// Returns `SymbolError::RsDecodeFailed` if RS cannot correct the codeword.
pub fn rs_decode_codeword_with_profile(
    code: &[u8; RS_N],
    profile: EccProfile,
    codeword_index: u16,
) -> Result<Vec<u8>, SymbolError> {
    rs_decode_codeword_with_profile_and_erasures(code, profile, &[], codeword_index)
}

/// Decode one codeword using the selected profile and known erasures.
///
/// # Errors
/// Returns `SymbolError::TooManyErasures` if erasures exceed the selected
/// profile's parity budget, or `SymbolError::RsDecodeFailed` if RS cannot
/// correct the codeword.
pub fn rs_decode_codeword_with_profile_and_erasures(
    code: &[u8; RS_N],
    profile: EccProfile,
    erasures: &[u8],
    codeword_index: u16,
) -> Result<Vec<u8>, SymbolError> {
    let erasures = normalized_erasures(erasures);
    if erasures.len() > profile.parity_len() {
        return Err(SymbolError::TooManyErasures {
            codeword_index,
            got: erasures.len(),
            max: profile.parity_len(),
        });
    }

    let decoder = Decoder::new(profile.parity_len());
    let erase_pos = if erasures.is_empty() {
        None
    } else {
        Some(erasures.as_slice())
    };
    let buffer = decoder
        .correct(code, erase_pos)
        .map_err(|_| SymbolError::RsDecodeFailed { codeword_index })?;

    let data = buffer.data();
    debug_assert_eq!(data.len(), profile.data_len());
    Ok(data.to_vec())
}

fn normalized_erasures(erasures: &[u8]) -> Vec<u8> {
    let mut normalized = erasures.to_vec();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
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
    try_deinterleave_column_major(symbol_bytes, k)
        .expect("caller must provide a K*255 interleaved buffer")
}

/// Fallible inverse of [`interleave_column_major`].
///
/// # Errors
/// Returns `SymbolError::InvalidInterleaveGeometry` when `k == 0` or
/// `symbol_bytes.len() != k * 255`.
pub fn try_deinterleave_column_major(
    symbol_bytes: &[u8],
    k: usize,
) -> Result<Vec<[u8; RS_N]>, SymbolError> {
    if k == 0 || symbol_bytes.len() != k * RS_N {
        return Err(SymbolError::InvalidInterleaveGeometry {
            symbol_len: symbol_bytes.len(),
            k,
        });
    }
    let mut out = vec![[0u8; RS_N]; k];
    for r in 0..RS_N {
        for (j, cw) in out.iter_mut().enumerate() {
            cw[r] = symbol_bytes[r * k + j];
        }
    }
    Ok(out)
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

    #[test]
    fn profiles_have_expected_dimensions() {
        assert_eq!(EccProfile::Safe.data_len(), 223);
        assert_eq!(EccProfile::Safe.parity_len(), 32);
        assert_eq!(EccProfile::Balanced.data_len(), 239);
        assert_eq!(EccProfile::Balanced.parity_len(), 16);
        assert_eq!(EccProfile::Dense.data_len(), 247);
        assert_eq!(EccProfile::Dense.parity_len(), 8);
    }

    #[test]
    fn fallible_encode_rejects_wrong_profile_data_length() {
        assert!(matches!(
            try_rs_encode_codeword_with_profile(&[0u8; 10], EccProfile::Safe),
            Err(EncodeError::InvalidCodewordDataLength {
                profile: "safe",
                got: 10,
                expected: 223,
            })
        ));
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;

    #[test]
    fn rs_roundtrip_clean() {
        for profile in EccProfile::all() {
            let data = vec![0x42u8; profile.data_len()];
            let code = rs_encode_codeword_with_profile(&data, profile);
            let recovered = rs_decode_codeword_with_profile(&code, profile, 0).unwrap();
            assert_eq!(recovered, data);
        }
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
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test index is always < 256 and fits in u8"
    )]
    fn rs_tolerates_up_to_32_known_erasures() {
        let data: [u8; RS_K] = std::array::from_fn(|i| i as u8);
        let mut code = rs_encode_codeword(&data);
        let erasures: Vec<u8> = (0u8..32).collect();

        for &position in &erasures {
            code[usize::from(position)] ^= 0xA5;
        }

        let recovered = rs_decode_codeword_with_erasures(&code, &erasures, 0).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn rs_rejects_erasures_above_parity_budget() {
        let data = [0u8; RS_K];
        let code = rs_encode_codeword(&data);
        let erasures: Vec<u8> = (0u8..33).collect();

        assert!(matches!(
            rs_decode_codeword_with_erasures(&code, &erasures, 7),
            Err(SymbolError::TooManyErasures {
                codeword_index: 7,
                got: 33,
                max: 32,
            })
        ));
    }

    #[test]
    fn duplicate_erasures_do_not_consume_extra_budget() {
        let data = [0x11u8; RS_K];
        let mut code = rs_encode_codeword(&data);
        code[4] ^= 0xFF;

        let recovered = rs_decode_codeword_with_erasures(&code, &[4, 4, 4], 0).unwrap();
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
    fn fallible_deinterleave_rejects_invalid_geometry() {
        assert!(matches!(
            try_deinterleave_column_major(&[0u8; 254], 1),
            Err(SymbolError::InvalidInterleaveGeometry {
                symbol_len: 254,
                k: 1,
            })
        ));
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
