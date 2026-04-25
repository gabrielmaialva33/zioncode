//! PNG RGB 24bpp I/O and flat-bytes ↔ pixel-buffer mapping.
//!
//! Mapping is row-major, R-G-B order per pixel, scanning X first then Y.
//! See spec §3.

use std::io::{Cursor, Read, Write};

use crate::constants::{BYTES_PER_PIXEL, PNG_BIT_DEPTH};
use crate::error::PngImageError;

/// Plain RGB 24bpp image buffer.
#[derive(Debug, Clone)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    /// Row-major, `width × height × 3` bytes, R-G-B per pixel.
    pub rgb_data: Vec<u8>,
}

impl RgbImage {
    /// Construct an `RgbImage` from a flat byte stream of length exactly `width × height × 3`.
    /// The flat stream maps directly to `rgb_data` — no transformation.
    ///
    /// # Errors
    /// Returns an error if dimensions overflow the platform `usize` or if
    /// `flat.len()` does not match the requested RGB shape.
    pub fn from_flat(flat: Vec<u8>, width: u32, height: u32) -> Result<Self, PngImageError> {
        let expected = expected_rgb_len(width, height)?;
        if flat.len() != expected {
            return Err(PngImageError::InvalidRgbDataLen {
                expected,
                got: flat.len(),
            });
        }

        Ok(Self {
            width,
            height,
            rgb_data: flat,
        })
    }

    /// Return the inner flat byte stream by value (consumes the image).
    #[must_use]
    pub fn into_flat(self) -> Vec<u8> {
        self.rgb_data
    }

    #[must_use]
    pub fn expected_rgb_len(&self) -> Option<usize> {
        expected_rgb_len(self.width, self.height).ok()
    }

    #[must_use]
    pub fn has_valid_shape(&self) -> bool {
        self.expected_rgb_len() == Some(self.rgb_data.len())
    }
}

/// Encode an `RgbImage` as PNG RGB 24bpp into a `Vec<u8>`.
///
/// # Errors
/// Returns an error if the RGB buffer shape is invalid or PNG encoding fails.
pub fn encode_png(image: &RgbImage) -> Result<Vec<u8>, PngImageError> {
    let expected = expected_rgb_len(image.width, image.height)?;
    if image.rgb_data.len() != expected {
        return Err(PngImageError::InvalidRgbDataLen {
            expected,
            got: image.rgb_data.len(),
        });
    }
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, image.width, image.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(match PNG_BIT_DEPTH {
            8 => png::BitDepth::Eight,
            _ => unreachable!("PNG_BIT_DEPTH is locked to 8 in v1"),
        });
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&image.rgb_data)?;
        writer.finish()?;
    }
    Ok(buf)
}

/// Decode PNG bytes as RGB 24bpp. Rejects any other format (RGBA, grayscale, 16-bit, etc.).
///
/// # Errors
/// Returns an error if PNG decoding fails, the format is not RGB 8-bit, or the
/// decoded buffer shape is invalid.
pub fn decode_png(png_bytes: &[u8]) -> Result<RgbImage, PngImageError> {
    let decoder = png::Decoder::new(Cursor::new(png_bytes));
    let mut reader = decoder.read_info()?;
    let info = reader.info().clone();
    if info.color_type != png::ColorType::Rgb {
        return Err(PngImageError::UnsupportedFormat {
            color_type: info.color_type,
            bit_depth: info.bit_depth,
        });
    }
    if info.bit_depth != png::BitDepth::Eight {
        return Err(PngImageError::UnsupportedFormat {
            color_type: info.color_type,
            bit_depth: info.bit_depth,
        });
    }
    let output_buffer_size = reader
        .output_buffer_size()
        .ok_or(PngImageError::OutputBufferSizeUnavailable)?;
    let mut buf = vec![0u8; output_buffer_size];
    let frame = reader.next_frame(&mut buf)?;
    buf.truncate(frame.buffer_size());
    let expected = expected_rgb_len(info.width, info.height)?;
    if buf.len() != expected {
        return Err(PngImageError::InvalidRgbDataLen {
            expected,
            got: buf.len(),
        });
    }
    Ok(RgbImage {
        width: info.width,
        height: info.height,
        rgb_data: buf,
    })
}

/// Stream sink for callers who want PNG written directly to an arbitrary writer.
///
/// # Errors
/// Returns an error if PNG encoding or writer I/O fails.
pub fn write_png<W: Write>(image: &RgbImage, writer: W) -> Result<(), PngImageError> {
    let bytes = encode_png(image)?;
    let mut writer = writer;
    writer.write_all(&bytes)?;
    Ok(())
}

/// Stream source for callers who want to decode PNG from an arbitrary reader.
///
/// # Errors
/// Returns an error if reader I/O or PNG decoding fails.
pub fn read_png<R: Read>(mut reader: R) -> Result<RgbImage, PngImageError> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    decode_png(&buf)
}

fn expected_rgb_len(width: u32, height: u32) -> Result<usize, PngImageError> {
    let len = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(BYTES_PER_PIXEL as u64))
        .ok_or(PngImageError::ImageTooLarge { width, height })?;
    usize::try_from(len).map_err(|_| PngImageError::ImageTooLarge { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_flat(w: u32, h: u32) -> Vec<u8> {
        let len = (w as usize) * (h as usize) * 3;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "test fixture: deterministic byte pattern wraps intentionally"
        )]
        (0..len).map(|i| (i as u8).wrapping_mul(31)).collect()
    }

    #[test]
    fn flat_to_image_to_flat_roundtrip() {
        let flat = sample_flat(16, 12);
        let img = RgbImage::from_flat(flat.clone(), 16, 12).unwrap();
        assert_eq!(img.width, 16);
        assert_eq!(img.height, 12);
        assert_eq!(img.into_flat(), flat);
    }

    #[test]
    fn encode_decode_png_roundtrip() {
        let flat = sample_flat(32, 24);
        let img = RgbImage::from_flat(flat.clone(), 32, 24).unwrap();
        let png_bytes = encode_png(&img).unwrap();
        let recovered = decode_png(&png_bytes).unwrap();
        assert_eq!(recovered.width, 32);
        assert_eq!(recovered.height, 24);
        assert_eq!(recovered.rgb_data, flat);
    }

    #[test]
    fn decode_rejects_grayscale() {
        // Encode a grayscale PNG manually to test rejection.
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, 4, 4);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0u8; 16]).unwrap();
            writer.finish().unwrap();
        }
        let err = decode_png(&buf).unwrap_err();
        assert!(matches!(err, PngImageError::UnsupportedFormat { .. }));
    }

    #[test]
    fn decode_rejects_rgba() {
        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, 4, 4);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[0u8; 64]).unwrap();
            writer.finish().unwrap();
        }
        let err = decode_png(&buf).unwrap_err();
        assert!(matches!(err, PngImageError::UnsupportedFormat { .. }));
    }

    #[test]
    fn first_pixel_byte_layout_is_rgb() {
        let mut flat = vec![0u8; 6];
        flat[0] = 0xAA; // pixel(0,0).R
        flat[1] = 0xBB; // pixel(0,0).G
        flat[2] = 0xCC; // pixel(0,0).B
        flat[3] = 0x11; // pixel(1,0).R
        flat[4] = 0x22; // pixel(1,0).G
        flat[5] = 0x33; // pixel(1,0).B
        let img = RgbImage::from_flat(flat.clone(), 2, 1).unwrap();
        let png_bytes = encode_png(&img).unwrap();
        let recovered = decode_png(&png_bytes).unwrap();
        assert_eq!(recovered.rgb_data, flat);
    }

    #[test]
    fn dimension_mismatch_rejected_on_encode() {
        let img = RgbImage {
            width: 10,
            height: 10,
            rgb_data: vec![0u8; 50], // should be 300
        };
        assert!(encode_png(&img).is_err());
    }

    #[test]
    fn from_flat_rejects_shape_mismatch() {
        let err = RgbImage::from_flat(vec![0u8; 2], 1, 1).unwrap_err();
        assert!(matches!(
            err,
            PngImageError::InvalidRgbDataLen {
                expected: 3,
                got: 2
            }
        ));
    }
}
