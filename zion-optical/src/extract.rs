//! Extract pipeline: PNG -> flat stream -> optical header validate -> split symbols -> reassemble.
//! Also provides `inspect` (shallow) and `inspect_deep` (decodes first symbol).
//! See spec section 2 (decoder pipeline) and section 10 (inspect).

use crate::constants::OPTICAL_HEADER_LEN;
use crate::error::ExtractError;
use crate::optical_header::OpticalHeader;
use crate::pixel_io::{RgbImage, decode_png};

#[derive(Debug)]
pub struct ExtractOutput {
    pub file_bytes: Vec<u8>,
    pub file_id: [u8; 16],
    pub k_global: u16,
    pub total_symbols: u16,
    pub payload_len: u32,
}

/// Extract the original file from a `zion-optical` v1 PNG.
///
/// # Errors
/// Any of the variants of `ExtractError` -- header issues, PNG format mismatch,
/// padding non-zero, codec errors, etc.
pub fn extract(png_bytes: &[u8]) -> Result<ExtractOutput, ExtractError> {
    let (image, header) = decode_container(png_bytes)?;
    let flat_stream = image.rgb_data;

    // Defense in depth: re-validate declared payload_len == k_global * total_symbols * 255.
    // `OpticalHeader::parse` already enforces this, but we keep the check explicit here so a
    // future relaxation in the header layer cannot silently bypass extract validation.
    let computed: u64 = u64::from(header.total_symbols)
        * u64::from(header.k_global)
        * zion_codec::low_level::RS_N as u64;
    if u64::from(header.payload_len) != computed {
        return Err(ExtractError::InconsistentPayloadLength {
            declared: header.payload_len,
            computed,
        });
    }

    // Length check.
    let payload_end = OPTICAL_HEADER_LEN
        .checked_add(usize::try_from(header.payload_len).map_err(|_| {
            ExtractError::PayloadTruncated {
                declared: header.payload_len,
                available: flat_stream.len().saturating_sub(OPTICAL_HEADER_LEN),
            }
        })?)
        .ok_or(ExtractError::PayloadTruncated {
            declared: header.payload_len,
            available: flat_stream.len().saturating_sub(OPTICAL_HEADER_LEN),
        })?;
    if payload_end > flat_stream.len() {
        return Err(ExtractError::PayloadTruncated {
            declared: header.payload_len,
            available: flat_stream.len() - OPTICAL_HEADER_LEN,
        });
    }
    let payload_bytes = &flat_stream[OPTICAL_HEADER_LEN..payload_end];

    // Padding: all bytes after payload must be zero.
    for (rel_offset, &b) in flat_stream[payload_end..].iter().enumerate() {
        if b != 0 {
            return Err(ExtractError::PaddingNotZero {
                offset: payload_end + rel_offset,
            });
        }
    }

    // Split payload into symbols and feed FileReassembler.
    let symbol_size = usize::from(header.k_global) * zion_codec::low_level::RS_N;
    let mut reassembler = zion_codec::FileReassembler::new();
    let mut file_id_opt: Option<[u8; 16]> = None;

    for sym_idx in 0..usize::from(header.total_symbols) {
        let start = sym_idx * symbol_size;
        let end = start + symbol_size;
        let sym_bytes = &payload_bytes[start..end];
        let decoded = zion_codec::decode_symbol(sym_bytes)?;
        if file_id_opt.is_none() {
            file_id_opt = Some(decoded.metadata.file_id.to_bytes());
        }
        reassembler.add_symbol(decoded)?;
    }

    let file_bytes = reassembler.finalize()?;
    let file_id = file_id_opt.ok_or(ExtractError::EmptyContainer)?;

    Ok(ExtractOutput {
        file_bytes,
        file_id,
        k_global: header.k_global,
        total_symbols: header.total_symbols,
        payload_len: header.payload_len,
    })
}

#[derive(Debug, Clone, Copy)]
pub struct OpticalMetadata {
    pub width: u32,
    pub height: u32,
    pub version: u8,
    pub header_len: u8,
    pub flags: u16,
    pub k_global: u16,
    pub total_symbols: u16,
    pub payload_len: u32,
}

/// Inspect the optical header without touching the payload (no codec calls).
///
/// # Errors
/// PNG decode or header validation errors.
pub fn inspect(png_bytes: &[u8]) -> Result<OpticalMetadata, ExtractError> {
    let (image, header) = decode_container(png_bytes)?;
    Ok(metadata_from_parts(&image, &header))
}

#[derive(Debug, Clone)]
pub struct OpticalDeepMetadata {
    pub optical: OpticalMetadata,
    pub file_id: [u8; 16],
    pub global_hash: [u8; 32],
    pub total_blocks: u32,
    pub file_size: u64,
}

/// Inspect optical header AND decode the first symbol to extract codec-A metadata.
///
/// # Errors
/// PNG decode, header validation, or `zion-codec` decode failures.
pub fn inspect_deep(png_bytes: &[u8]) -> Result<OpticalDeepMetadata, ExtractError> {
    let (image, header) = decode_container(png_bytes)?;
    let optical = metadata_from_parts(&image, &header);
    let symbol_size = usize::from(optical.k_global) * zion_codec::low_level::RS_N;
    let start = OPTICAL_HEADER_LEN;
    let end = start + symbol_size;
    if end > image.rgb_data.len() {
        return Err(ExtractError::PayloadTruncated {
            declared: optical.payload_len,
            available: image.rgb_data.len().saturating_sub(start),
        });
    }
    let first_sym = &image.rgb_data[start..end];
    let decoded = zion_codec::decode_symbol(first_sym)?;
    let m = decoded.metadata;
    Ok(OpticalDeepMetadata {
        optical,
        file_id: m.file_id.to_bytes(),
        global_hash: m.global_hash.to_bytes(),
        total_blocks: m.total_blocks,
        file_size: m.file_size,
    })
}

fn decode_container(png_bytes: &[u8]) -> Result<(RgbImage, OpticalHeader), ExtractError> {
    let image = decode_png(png_bytes)?;
    if image.rgb_data.len() < OPTICAL_HEADER_LEN {
        return Err(ExtractError::ImageTooSmall {
            got: image.rgb_data.len(),
            min: OPTICAL_HEADER_LEN,
        });
    }
    let header = OpticalHeader::parse(&image.rgb_data[0..OPTICAL_HEADER_LEN])?;
    Ok((image, header))
}

fn metadata_from_parts(image: &RgbImage, header: &OpticalHeader) -> OpticalMetadata {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "OPTICAL_HEADER_LEN = 24 fits u8"
    )]
    let header_len_u8 = OPTICAL_HEADER_LEN as u8;
    OpticalMetadata {
        width: image.width,
        height: image.height,
        version: crate::wire::VERSION,
        header_len: header_len_u8,
        flags: header.flags,
        k_global: header.k_global,
        total_symbols: header.total_symbols,
        payload_len: header.payload_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderParams, render};

    #[test]
    fn inspect_returns_correct_metadata() {
        let payload = b"inspect-test-content";
        let out = render(payload, &RenderParams::default()).unwrap();
        let meta = inspect(&out.png_bytes).unwrap();
        assert_eq!(meta.k_global, out.k_global);
        assert_eq!(meta.total_symbols, out.total_symbols);
        assert_eq!(meta.payload_len, out.payload_len);
        assert_eq!(meta.width, out.width);
        assert_eq!(meta.height, out.height);
    }

    #[test]
    fn inspect_deep_returns_codec_metadata() {
        let payload = b"inspect-deep-test";
        let out = render(payload, &RenderParams::default()).unwrap();
        let meta = inspect_deep(&out.png_bytes).unwrap();
        assert_eq!(meta.file_id, out.file_id);
        assert_eq!(meta.global_hash, out.global_hash);
        assert_eq!(meta.file_size, u64::try_from(payload.len()).unwrap());
    }
}
