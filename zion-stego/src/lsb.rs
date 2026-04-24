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
}
