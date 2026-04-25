//! Full render pipeline: file bytes → zion-codec encode → flat stream → PNG.
//! See spec §2 (encoder pipeline) and §4 (header).

use crate::constants::{K_DEFAULT, MAX_PAYLOAD_BYTES_V1, OPTICAL_HEADER_LEN, ZSTD_LEVEL_DEFAULT};
use crate::error::RenderError;
use crate::layout::{ImageShape, compute_dimensions};
use crate::optical_header::OpticalHeader;
use crate::pixel_io::{RgbImage, encode_png};

#[derive(Debug, Clone, Copy)]
pub struct RenderParams {
    pub symbol_width: zion_codec::SymbolWidth,
    pub shape: ImageShape,
    pub zstd_level: i32,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            symbol_width: zion_codec::SymbolWidth::new(K_DEFAULT)
                .expect("K_DEFAULT is a valid zion-codec symbol width"),
            shape: ImageShape::Square,
            zstd_level: ZSTD_LEVEL_DEFAULT,
        }
    }
}

impl RenderParams {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the codec symbol width used by the optical payload.
    ///
    /// # Errors
    /// Returns a zion-codec encode error if `k` is zero or exceeds the codec
    /// implementation limit.
    pub fn with_k(mut self, k: u16) -> Result<Self, zion_codec::EncodeError> {
        self.symbol_width = zion_codec::SymbolWidth::new(k)?;
        Ok(self)
    }

    #[must_use]
    pub const fn with_shape(mut self, shape: ImageShape) -> Self {
        self.shape = shape;
        self
    }

    #[must_use]
    pub const fn with_zstd_level(mut self, zstd_level: i32) -> Self {
        self.zstd_level = zstd_level;
        self
    }
}

#[derive(Debug)]
pub struct RenderOutput {
    pub png_bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub k_global: u16,
    pub total_symbols: u16,
    pub payload_len: u32,
    pub file_id: [u8; 16],
    pub global_hash: [u8; 32],
}

/// Render a file as a PNG RGB 24bpp visual-digital container.
///
/// # Errors
/// - `EmptyInput` if `file_bytes` is empty.
/// - `PayloadTooLargeForV1` if total payload would overflow the v1 u32 `payload_len`.
/// - `InsufficientImageCapacity` if the chosen `shape` cannot fit the stream.
/// - `InvalidDimensions` if shape parameters are zero or overflow.
/// - `CodecError` if zion-codec encode fails.
/// - `PngError` if PNG encoding fails.
pub fn render(file_bytes: &[u8], params: &RenderParams) -> Result<RenderOutput, RenderError> {
    if file_bytes.is_empty() {
        return Err(RenderError::EmptyInput);
    }

    // 1. zion-codec encode_file.
    let k = params.symbol_width.get();
    let encoded = zion_codec::low_level::encode_file(file_bytes, k, params.zstd_level)?;
    let total_symbols =
        u16::try_from(encoded.symbols.len()).map_err(|_| RenderError::TooManySymbols {
            got: encoded.symbols.len(),
        })?;

    // 2. Concatenate symbols into payload bytes.
    let payload_bytes_len =
        u64::from(total_symbols) * u64::from(k) * zion_codec::low_level::RS_N as u64;
    if payload_bytes_len > MAX_PAYLOAD_BYTES_V1 {
        return Err(RenderError::PayloadTooLargeForV1 {
            got: payload_bytes_len,
        });
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "validated <= u32::MAX above"
    )]
    let payload_len_u32 = payload_bytes_len as u32;

    let payload_bytes_usize =
        usize::try_from(payload_bytes_len).map_err(|_| RenderError::PayloadTooLargeForV1 {
            got: payload_bytes_len,
        })?;
    let flat_stream_len = OPTICAL_HEADER_LEN.checked_add(payload_bytes_usize).ok_or(
        RenderError::PayloadTooLargeForV1 {
            got: payload_bytes_len,
        },
    )?;
    let mut flat_stream: Vec<u8> = Vec::with_capacity(flat_stream_len);

    // 3. Serialize header into the flat stream.
    let header = OpticalHeader {
        flags: 0,
        k_global: k,
        total_symbols,
        payload_len: payload_len_u32,
    };
    flat_stream.extend_from_slice(&header.serialize_v1());

    // 4. Concatenate symbols.
    for sym in &encoded.symbols {
        flat_stream.extend_from_slice(sym.as_bytes());
    }

    // 5. Compute dimensions, pad with zeros to fill capacity.
    let dims = compute_dimensions(params.shape, flat_stream.len())?;
    let capacity = usize::try_from(dims.capacity_bytes()).map_err(|_| {
        RenderError::InvalidDimensions("image capacity exceeds platform usize".into())
    })?;
    flat_stream.resize(capacity, 0);

    // 6. Encode as PNG RGB 24bpp.
    let image = RgbImage::from_flat(flat_stream, dims.width, dims.height)?;
    let png_bytes = encode_png(&image)?;

    Ok(RenderOutput {
        png_bytes,
        width: dims.width,
        height: dims.height,
        k_global: k,
        total_symbols,
        payload_len: payload_len_u32,
        file_id: encoded.file_id.to_bytes(),
        global_hash: encoded.global_hash.to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_rejected() {
        let err = render(&[], &RenderParams::default()).unwrap_err();
        assert!(matches!(err, RenderError::EmptyInput));
    }

    #[test]
    fn tiny_input_succeeds() {
        let out = render(b"hello", &RenderParams::default()).unwrap();
        assert!(!out.png_bytes.is_empty());
        assert_eq!(out.k_global, K_DEFAULT);
        assert!(out.total_symbols >= 1);
        assert!(out.width >= 1 && out.height >= 1);
    }

    #[test]
    fn dimensions_are_square_by_default() {
        let payload = vec![0u8; 8192];
        let out = render(&payload, &RenderParams::default()).unwrap();
        assert_eq!(out.width, out.height);
    }
}
