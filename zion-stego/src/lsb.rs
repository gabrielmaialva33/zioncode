//! LSB matching embedding (±1 random) with saturation at extremes.
//! See spec §4 "Embedding rule".

use rand_chacha::ChaCha20Rng;
use rand_core::Rng;

/// Applies LSB matching to a channel: if `channel & 1 == bit`, no-op;
/// otherwise adjusts `channel` by ±1 (random), with saturation at the extremes.
#[must_use]
pub fn apply_lsb_matching(channel: u8, bit: u8, rng: &mut ChaCha20Rng) -> u8 {
    debug_assert!(bit == 0 || bit == 1);
    if channel & 1 == bit {
        return channel;
    }
    if channel == 0 {
        return 1;
    }
    if channel == 255 {
        return 254;
    }
    // Random ±1.
    if rng.next_u32() & 1 == 0 {
        channel - 1
    } else {
        channel + 1
    }
}

/// Extracts the LSB of a channel.
#[must_use]
pub const fn extract_lsb(channel: u8) -> u8 {
    channel & 1
}

/// Writes `bits` to the channels of `data` pointed to by `positions`, via LSB matching.
/// `rng` is used for the ±1 decision at intermediate channels.
///
/// # Panics
/// Panics if `positions.len() != bits.len()`.
pub fn embed_bits_at(data: &mut [u8], positions: &[u32], bits: &[u8], rng: &mut ChaCha20Rng) {
    assert_eq!(positions.len(), bits.len(), "positions.len != bits.len");
    for (pos, &bit) in positions.iter().zip(bits) {
        let idx = *pos as usize;
        data[idx] = apply_lsb_matching(data[idx], bit, rng);
    }
}

/// Extracts `positions.len()` bits from the channels in `data` (via LSB).
#[must_use]
pub fn extract_bits_at(data: &[u8], positions: &[u32]) -> Vec<u8> {
    positions
        .iter()
        .map(|&pos| extract_lsb(data[pos as usize]))
        .collect()
}

/// Converts a byte slice into a `Vec<u8>` where each entry is 0 or 1 (MSB-first per byte).
#[must_use]
pub fn bytes_to_bits(bytes: &[u8]) -> Vec<u8> {
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    for &byte in bytes {
        for shift in (0..8).rev() {
            bits.push((byte >> shift) & 1);
        }
    }
    bits
}

/// Converts bits (0/1) back into bytes (MSB-first).
/// `bits.len()` must be a multiple of 8; trailing bits are ignored.
#[must_use]
pub fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(bits.len() / 8);
    for chunk in bits.chunks_exact(8) {
        let mut byte = 0u8;
        for &bit in chunk {
            byte = (byte << 1) | (bit & 1);
        }
        bytes.push(byte);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::from_seed([0x33; 32])
    }

    #[test]
    fn lsb_match_noop_when_bit_matches() {
        let mut r = rng();
        assert_eq!(apply_lsb_matching(100, 0, &mut r), 100); // 100 & 1 == 0
        assert_eq!(apply_lsb_matching(101, 1, &mut r), 101); // 101 & 1 == 1
    }

    #[test]
    fn lsb_match_zero_goes_up() {
        let mut r = rng();
        assert_eq!(apply_lsb_matching(0, 1, &mut r), 1);
    }

    #[test]
    fn lsb_match_255_goes_down() {
        let mut r = rng();
        assert_eq!(apply_lsb_matching(255, 0, &mut r), 254);
    }

    #[test]
    fn lsb_match_intermediate_pm1() {
        let mut r = rng();
        let out = apply_lsb_matching(128, 1, &mut r); // 128 & 1 == 0, need 1
        assert!(out == 127 || out == 129);
        assert_eq!(out & 1, 1);
    }

    #[test]
    fn extract_lsb_works() {
        assert_eq!(extract_lsb(0), 0);
        assert_eq!(extract_lsb(1), 1);
        assert_eq!(extract_lsb(254), 0);
        assert_eq!(extract_lsb(255), 1);
    }

    #[test]
    fn bytes_to_bits_msb_first() {
        assert_eq!(bytes_to_bits(&[0b1010_1100]), vec![1, 0, 1, 0, 1, 1, 0, 0]);
        assert_eq!(
            bytes_to_bits(&[0, 0xFF]),
            vec![0; 8].into_iter().chain(vec![1; 8]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn bytes_bits_roundtrip() {
        let bytes = vec![0x01, 0x42, 0xAB, 0xFF];
        let bits = bytes_to_bits(&bytes);
        assert_eq!(bits_to_bytes(&bits), bytes);
    }

    #[test]
    fn embed_extract_roundtrip() {
        let mut data = vec![100u8; 256];
        let positions: Vec<u32> = (0..256).collect();
        let payload: Vec<u8> = (0..32u8).map(|i| i.wrapping_mul(7)).collect();
        let bits = bytes_to_bits(&payload);
        assert_eq!(bits.len(), 256);
        embed_bits_at(&mut data, &positions, &bits, &mut rng());
        let recovered = extract_bits_at(&data, &positions);
        assert_eq!(recovered, bits);
    }
}
