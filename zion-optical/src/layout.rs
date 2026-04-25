//! Image shape calculation and padding for the optical container.
//! See spec §5.

use crate::constants::BYTES_PER_PIXEL;
use crate::error::RenderError;

/// User-facing image shape choice.
#[derive(Debug, Clone, Copy)]
pub enum ImageShape {
    /// Smallest square that fits the flat byte stream.
    Square,
    /// Explicit dimensions; must be large enough for the stream.
    WidthHeight { width: u32, height: u32 },
    /// Aspect ratio (W:H); computes the smallest matching dimensions.
    Aspect { w: u32, h: u32 },
}

/// Computed image dimensions in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    /// Total bytes the image holds: `width × height × 3`.
    #[must_use]
    pub fn capacity_bytes(self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * BYTES_PER_PIXEL as u64
    }
}

/// Resolve a shape choice into concrete dimensions, validating capacity against `flat_stream_len`.
///
/// # Invariants
/// - `flat_stream_len` MUST be > 0. Callers (e.g. `render`) reject empty input
///   upstream via `RenderError::EmptyInput`. Passing `0` here is a logic bug:
///   the function does not error, but returns trivial `1×1` dimensions that
///   waste capacity.
///
/// # Errors
/// - `InvalidDimensions` if any dimension is zero or arithmetic overflows.
/// - `InsufficientImageCapacity` if `width × height × 3 < flat_stream_len`.
pub fn compute_dimensions(
    shape: ImageShape,
    flat_stream_len: usize,
) -> Result<Dimensions, RenderError> {
    match shape {
        ImageShape::Square => square_dimensions(flat_stream_len),
        ImageShape::WidthHeight { width, height } => {
            explicit_dimensions(width, height, flat_stream_len)
        }
        ImageShape::Aspect { w, h } => aspect_dimensions(w, h, flat_stream_len),
    }
}

fn square_dimensions(flat_stream_len: usize) -> Result<Dimensions, RenderError> {
    // pixel_count = ceil(flat_stream_len / 3)
    let pixel_count = flat_stream_len.div_ceil(BYTES_PER_PIXEL);
    // side = ceil(sqrt(pixel_count))
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "pixel_count <= u32::MAX in practice (max payload 4 GiB / 3 ≈ 1.43e9 pixels); sqrt of non-negative f64 is non-negative and bounded"
    )]
    let side_u64 = {
        let side_f = (pixel_count as f64).sqrt().ceil();
        side_f as u64
    };
    let side = u32::try_from(side_u64)
        .map_err(|_| RenderError::InvalidDimensions(format!("side {side_u64} > u32::MAX")))?;
    let side = side.max(1);
    let dims = Dimensions {
        width: side,
        height: side,
    };
    if dims.capacity_bytes() < flat_stream_len as u64 {
        return Err(RenderError::InsufficientImageCapacity {
            needed: flat_stream_len,
            available: capacity_to_usize(dims.capacity_bytes()),
        });
    }
    Ok(dims)
}

fn explicit_dimensions(
    width: u32,
    height: u32,
    flat_stream_len: usize,
) -> Result<Dimensions, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidDimensions(format!(
            "width={width}, height={height}"
        )));
    }
    let dims = Dimensions { width, height };
    let capacity = dims.capacity_bytes();
    if capacity < flat_stream_len as u64 {
        return Err(RenderError::InsufficientImageCapacity {
            needed: flat_stream_len,
            available: capacity_to_usize(capacity),
        });
    }
    Ok(dims)
}

fn aspect_dimensions(
    w_in: u32,
    h_in: u32,
    flat_stream_len: usize,
) -> Result<Dimensions, RenderError> {
    if w_in == 0 || h_in == 0 {
        return Err(RenderError::InvalidDimensions(format!(
            "aspect w={w_in}, h={h_in}"
        )));
    }
    let g = gcd_u32(w_in, h_in);
    let rw = u64::from(w_in / g);
    let rh = u64::from(h_in / g);
    // scale = ceil(sqrt(flat_stream_len / (3 * rw * rh)))
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "ratio components are u32-derived and flat_stream_len ≤ 4 GiB in v1; sqrt of non-negative f64 is non-negative and bounded"
    )]
    let scale = {
        let scale_f = (flat_stream_len as f64 / (BYTES_PER_PIXEL as f64 * rw as f64 * rh as f64))
            .sqrt()
            .ceil();
        (scale_f as u64).max(1)
    };
    let width_u64 = rw
        .checked_mul(scale)
        .ok_or_else(|| RenderError::InvalidDimensions("aspect scale overflow".into()))?;
    let height_u64 = rh
        .checked_mul(scale)
        .ok_or_else(|| RenderError::InvalidDimensions("aspect scale overflow".into()))?;
    let width = u32::try_from(width_u64)
        .map_err(|_| RenderError::InvalidDimensions("aspect width > u32::MAX".into()))?;
    let height = u32::try_from(height_u64)
        .map_err(|_| RenderError::InvalidDimensions("aspect height > u32::MAX".into()))?;
    let dims = Dimensions { width, height };
    let capacity = dims.capacity_bytes();
    if capacity < flat_stream_len as u64 {
        return Err(RenderError::InsufficientImageCapacity {
            needed: flat_stream_len,
            available: capacity_to_usize(capacity),
        });
    }
    Ok(dims)
}

fn gcd_u32(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd_u32(b, a % b) }
}

fn capacity_to_usize(bytes: u64) -> usize {
    usize::try_from(bytes).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_for_small_payload() {
        let dims = compute_dimensions(ImageShape::Square, 100).unwrap();
        // ceil(100/3) = 34 pixels, sqrt(34) ≈ 5.83, ceil = 6
        assert_eq!(dims.width, 6);
        assert_eq!(dims.height, 6);
        assert!(dims.capacity_bytes() >= 100);
    }

    #[test]
    fn square_for_one_byte() {
        let dims = compute_dimensions(ImageShape::Square, 1).unwrap();
        assert_eq!(dims.width, 1);
        assert_eq!(dims.height, 1);
        assert_eq!(dims.capacity_bytes(), 3);
    }

    #[test]
    fn square_for_bible_example() {
        // 14 symbols × 4112 × 255 = 14 679 840 + 24 header = 14 679 864 bytes
        let n = 14_679_864;
        let dims = compute_dimensions(ImageShape::Square, n).unwrap();
        // ceil(n/3) = 4 893 288 px → ceil(sqrt(...)) = 2213
        assert_eq!(dims.width, 2213);
        assert_eq!(dims.height, 2213);
        assert!(dims.capacity_bytes() >= n as u64);
    }

    #[test]
    fn explicit_width_height_ok() {
        let dims = compute_dimensions(
            ImageShape::WidthHeight {
                width: 100,
                height: 100,
            },
            1000,
        )
        .unwrap();
        assert_eq!(dims.width, 100);
        assert_eq!(dims.height, 100);
    }

    #[test]
    fn explicit_zero_rejected() {
        let err = compute_dimensions(
            ImageShape::WidthHeight {
                width: 0,
                height: 100,
            },
            10,
        )
        .unwrap_err();
        assert!(matches!(err, RenderError::InvalidDimensions(_)));
    }

    #[test]
    fn explicit_too_small_rejected() {
        let err = compute_dimensions(
            ImageShape::WidthHeight {
                width: 10,
                height: 10,
            },
            10_000,
        )
        .unwrap_err();
        assert!(matches!(err, RenderError::InsufficientImageCapacity { .. }));
    }

    #[test]
    fn aspect_16_9() {
        let n = 1_920 * 1_080 * 3;
        let dims = compute_dimensions(ImageShape::Aspect { w: 16, h: 9 }, n).unwrap();
        assert!(dims.capacity_bytes() >= n as u64);
        assert_eq!(dims.width % 16, 0);
        assert_eq!(dims.height % 9, 0);
        assert_eq!(dims.width / 16, dims.height / 9);
    }

    #[test]
    fn aspect_zero_rejected() {
        let err = compute_dimensions(ImageShape::Aspect { w: 0, h: 9 }, 10).unwrap_err();
        assert!(matches!(err, RenderError::InvalidDimensions(_)));
    }

    #[test]
    fn aspect_reduces_via_gcd() {
        let dims_a = compute_dimensions(ImageShape::Aspect { w: 1920, h: 1080 }, 100).unwrap();
        let dims_b = compute_dimensions(ImageShape::Aspect { w: 16, h: 9 }, 100).unwrap();
        assert_eq!(dims_a, dims_b);
    }

    #[test]
    fn capacity_bytes_correct() {
        let d = Dimensions {
            width: 100,
            height: 200,
        };
        assert_eq!(d.capacity_bytes(), 100 * 200 * 3);
    }
}
