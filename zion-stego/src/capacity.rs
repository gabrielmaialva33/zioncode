//! Capacity calculation per photo and `K_global` for the set.
//! See spec §4 "Density target" + `MAX_K` clamp.

use crate::constants::{AEAD_TAG_LEN, PLAINTEXT_HEADER_CHANNELS, ZION_CODEC_MAX_K};

/// Returns `K_per_photo` (natural, before clamping).
///
/// ```text
/// total_channels        = W × H × 3
/// permutable            = total_channels - 256
/// embeddable_bits       = floor(permutable × density)
/// embeddable_bytes      = floor(embeddable_bits / 8)
/// symbol_capacity_bytes = embeddable_bytes - AEAD_TAG_LEN
/// K_per_photo           = floor(symbol_capacity_bytes / 255)
/// ```
#[must_use]
pub fn photo_k_per_photo(width: u32, height: u32, density: f32) -> u16 {
    let total_channels = (width as usize) * (height as usize) * 3;
    if total_channels <= PLAINTEXT_HEADER_CHANNELS {
        return 0;
    }
    let permutable = total_channels - PLAINTEXT_HEADER_CHANNELS;
    // Integer math via (density * 100_000) and divide by 100_000 to avoid float drift.
    // density=0.33 → 33_000.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "density validated in [0.0, 1.0]; scaling to u64 is safe"
    )]
    let density_scaled = (density * 100_000.0) as u64;
    let embeddable_bits = (permutable as u64) * density_scaled / 100_000;
    let embeddable_bytes = (embeddable_bits / 8) as usize;
    if embeddable_bytes <= AEAD_TAG_LEN {
        return 0;
    }
    let symbol_capacity_bytes = embeddable_bytes - AEAD_TAG_LEN;
    let k = symbol_capacity_bytes / 255;
    u16::try_from(k).unwrap_or(u16::MAX)
}

/// Returns `K_global` — the min of `K_per_photo`, clamped to `ZION_CODEC_MAX_K`.
///
/// Returns `None` if any photo cannot carry at least one codeword (`K=0`) or if
/// `dims` is empty.
#[must_use]
pub fn compute_k_global(dims: &[(u32, u32)], density: f32) -> Option<u16> {
    if dims.is_empty() {
        return None;
    }
    let mut min_k = u16::MAX;
    for &(w, h) in dims {
        let k = photo_k_per_photo(w, h, density);
        if k == 0 {
            return None;
        }
        if k < min_k {
            min_k = k;
        }
    }
    Some(min_k.min(ZION_CODEC_MAX_K))
}

/// Returns the payload size in bytes for a given `K` (ciphertext without tag).
#[must_use]
pub const fn symbol_payload_bytes(k: u16) -> usize {
    (k as usize) * 255
}

/// Returns the ciphertext size in bytes for a given `K` (plaintext + Poly1305 tag).
#[must_use]
pub const fn ciphertext_bytes(k: u16) -> usize {
    symbol_payload_bytes(k) + AEAD_TAG_LEN
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::EMBEDDING_DENSITY_DEFAULT;

    #[test]
    fn k_per_photo_1080p() {
        // Per spec Appendix B: 1920×1080 @ density 0.33 → K=1006.
        let k = photo_k_per_photo(1920, 1080, EMBEDDING_DENSITY_DEFAULT);
        assert_eq!(k, 1006);
    }

    #[test]
    fn k_per_photo_4k() {
        // Per spec Appendix B: 3840×2160 @ 0.33.
        // NOTE: spec table states K=4024 but the explicit formula evaluates to 4025:
        //   floor((1_026_421 - 16) / 255) = floor(1_026_405 / 255) = 4025
        // (255 * 4025 = 1_026_375, and 1_026_405 - 1_026_375 = 30 > 0 ⇒ 4025 codewords fit).
        // The computation matches the formula, so this test reflects the correct integer math.
        let k = photo_k_per_photo(3840, 2160, EMBEDDING_DENSITY_DEFAULT);
        assert_eq!(k, 4025);
    }

    #[test]
    fn k_per_photo_8k_natural() {
        // Per spec: 7680×4320 @ 0.33 → K_natural=16100 (clamped to 4112 in compute_k_global).
        let k = photo_k_per_photo(7680, 4320, EMBEDDING_DENSITY_DEFAULT);
        assert_eq!(k, 16_100);
    }

    #[test]
    fn k_global_clamps_to_max_k() {
        let dims = [(7680, 4320)]; // 8K natural K=16100
        let k = compute_k_global(&dims, EMBEDDING_DENSITY_DEFAULT).unwrap();
        assert_eq!(k, ZION_CODEC_MAX_K); // 4112
    }

    #[test]
    fn k_global_takes_min() {
        let dims = [(1920, 1080), (3840, 2160)]; // K_per = [1006, 4024]
        let k = compute_k_global(&dims, EMBEDDING_DENSITY_DEFAULT).unwrap();
        assert_eq!(k, 1006);
    }

    #[test]
    fn k_global_empty_returns_none() {
        assert!(compute_k_global(&[], EMBEDDING_DENSITY_DEFAULT).is_none());
    }

    #[test]
    fn k_global_tiny_photo_returns_none() {
        // 1×1 pixel has only 3 channels; much less than plaintext header.
        assert!(compute_k_global(&[(1, 1)], EMBEDDING_DENSITY_DEFAULT).is_none());
    }

    #[test]
    fn ciphertext_bytes_formula() {
        assert_eq!(ciphertext_bytes(1006), 1006 * 255 + 16);
    }
}
