//! Fisher-Yates shuffle over channel indices, seeded by per-symbol `permutation_seed`.
//! See spec §4 "Channel permutation".

use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};

/// Returns the Fisher-Yates permutation of indices `[start..end)`.
/// `seed` is the per-symbol `permutation_seed_i` (32 bytes) from the KDF.
///
/// The output is a `Vec<u32>` of length `end - start` containing each index
/// exactly once in shuffled order.
///
/// # Panics
///
/// Panics if `end < start`.
#[must_use]
pub fn permute_range(start: u32, end: u32, seed: &[u8; 32]) -> Vec<u32> {
    assert!(end >= start, "end < start");
    let len = (end - start) as usize;
    let mut indices: Vec<u32> = (start..end).collect();

    let mut rng = ChaCha20Rng::from_seed(*seed);
    // Fisher-Yates: for i in (1..len).rev(), j = rand(0..=i), swap.
    for i in (1..len).rev() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "i < len <= u32::MAX by construction (start/end are u32)"
        )]
        let bound = i as u32 + 1;
        let j = bounded_u32(&mut rng, bound) as usize;
        indices.swap(i, j);
    }
    indices
}

/// Generates a uniform u32 in `0..bound` via rejection sampling.
fn bounded_u32<R: Rng>(rng: &mut R, bound: u32) -> u32 {
    let threshold = (u32::MAX - bound + 1) % bound;
    loop {
        let x = rng.next_u32();
        if x >= threshold {
            return x % bound;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 32] = [0x42; 32];

    #[test]
    fn empty_range() {
        let p = permute_range(5, 5, &SEED);
        assert!(p.is_empty());
    }

    #[test]
    fn single_element() {
        let p = permute_range(7, 8, &SEED);
        assert_eq!(p, vec![7]);
    }

    #[test]
    fn permutation_length_correct() {
        let p = permute_range(256, 1000, &SEED);
        assert_eq!(p.len(), 1000 - 256);
    }

    #[test]
    fn permutation_is_bijection() {
        let p = permute_range(0, 100, &SEED);
        let mut sorted = p.clone();
        sorted.sort_unstable();
        let expected: Vec<u32> = (0..100).collect();
        assert_eq!(sorted, expected);
    }

    #[test]
    fn permutation_deterministic_same_seed() {
        let p1 = permute_range(0, 500, &SEED);
        let p2 = permute_range(0, 500, &SEED);
        assert_eq!(p1, p2);
    }

    #[test]
    fn permutation_differs_per_seed() {
        let seed2 = [0x43; 32];
        let p1 = permute_range(0, 500, &SEED);
        let p2 = permute_range(0, 500, &seed2);
        assert_ne!(p1, p2);
    }

    #[test]
    fn permutation_is_not_identity() {
        let p = permute_range(0, 1000, &SEED);
        let identity: Vec<u32> = (0..1000).collect();
        assert_ne!(p, identity);
    }
}
