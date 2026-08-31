//! Experimental exact-geometry Android screenshot transport laboratory.
//!
//! This module is deliberately separate from ARC v1. It does not change or
//! extend the v1 wire format, and its framing is not stable. The current lab
//! assumes an RGB8, lossless screenshot captured at the exact carrier
//! dimensions. Crop, resize, camera capture, printing, and JPEG remain outside
//! its demonstrated contract.

use crc32c::crc32c;
use thiserror::Error;
use zion_stego::RgbImage;

use crate::carrier;

const SCREEN_MAGIC: [u8; 4] = *b"ZSCP";
const SCREEN_VERSION: u8 = 1;
const CELL_EDGE: usize = 2;
const BITS_PER_CELL: usize = 2;
const CELLS_PER_BYTE: usize = 8 / BITS_PER_CELL;
const HEADER_LEN: usize = 16;
const HEADER_REPEATS: usize = 9;
const HEADER_SYMBOLS: usize = HEADER_LEN * CELLS_PER_BYTE;
const HEADER_CELLS: usize = HEADER_SYMBOLS * HEADER_REPEATS;
const DIFFERENCE_MARGIN: i32 = 16;

/// Failures produced by the experimental screenshot transport.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ScreenError {
    #[error("invalid RGB8 PNG")]
    InvalidPng,
    #[error("invalid screenshot dimensions")]
    InvalidDimensions,
    #[error("screenshot transport arithmetic overflow")]
    ArithmeticOverflow,
    #[error("payload does not fit in the screenshot carrier")]
    CapacityExceeded,
    #[error("screenshot framing header is invalid")]
    InvalidHeader,
    #[error("screenshot symbol could not be recovered")]
    UnrecoverableSymbol,
    #[error("PNG encoding failed")]
    PngEncodeFailed,
}

/// Diagnostics for one experimental screen embedding.
#[derive(Debug)]
pub struct ScreenEncodeOutput {
    pub png_bytes: Vec<u8>,
    pub capacity_bytes: usize,
    pub changed_channels: usize,
    pub psnr_db: f64,
}

#[derive(Debug, Clone, Copy)]
struct Geometry {
    width: usize,
    cell_columns: usize,
    total_cells: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Header {
    payload_len: usize,
}

impl Header {
    fn serialize(self) -> Result<[u8; HEADER_LEN], ScreenError> {
        let payload_len =
            u32::try_from(self.payload_len).map_err(|_| ScreenError::CapacityExceeded)?;
        let mut bytes = [0u8; HEADER_LEN];
        bytes[0..4].copy_from_slice(&SCREEN_MAGIC);
        bytes[4] = SCREEN_VERSION;
        bytes[5] = u8::try_from(CELL_EDGE).map_err(|_| ScreenError::ArithmeticOverflow)?;
        bytes[6] = u8::try_from(BITS_PER_CELL).map_err(|_| ScreenError::ArithmeticOverflow)?;
        bytes[7] = u8::try_from(HEADER_REPEATS).map_err(|_| ScreenError::ArithmeticOverflow)?;
        bytes[8..12].copy_from_slice(&payload_len.to_be_bytes());
        let checksum = crc32c(&bytes[..12]);
        bytes[12..16].copy_from_slice(&checksum.to_be_bytes());
        Ok(bytes)
    }

    fn parse(bytes: &[u8; HEADER_LEN]) -> Result<Self, ScreenError> {
        if bytes[0..4] != SCREEN_MAGIC
            || bytes[4] != SCREEN_VERSION
            || usize::from(bytes[5]) != CELL_EDGE
            || usize::from(bytes[6]) != BITS_PER_CELL
            || usize::from(bytes[7]) != HEADER_REPEATS
        {
            return Err(ScreenError::InvalidHeader);
        }
        let expected = u32::from_be_bytes(
            bytes[12..16]
                .try_into()
                .map_err(|_| ScreenError::InvalidHeader)?,
        );
        if crc32c(&bytes[..12]) != expected {
            return Err(ScreenError::InvalidHeader);
        }
        let payload_len = u32::from_be_bytes(
            bytes[8..12]
                .try_into()
                .map_err(|_| ScreenError::InvalidHeader)?,
        );
        Ok(Self {
            payload_len: usize::try_from(payload_len).map_err(|_| ScreenError::InvalidHeader)?,
        })
    }
}

/// Returns the current lab payload capacity for an exact-size RGB8 screenshot.
///
/// # Errors
///
/// Returns [`ScreenError`] when dimensions violate the bounded ARC raster
/// contract or checked capacity arithmetic fails.
pub fn screen_capacity(width: u32, height: u32) -> Result<usize, ScreenError> {
    let geometry = geometry(width, height)?;
    geometry
        .total_cells
        .checked_sub(HEADER_CELLS)
        .map(|cells| cells / CELLS_PER_BYTE)
        .ok_or(ScreenError::InvalidDimensions)
}

/// Embeds opaque bytes in a cover using the experimental screen cell carrier.
///
/// # Errors
///
/// Returns [`ScreenError`] for invalid RGB8 PNG input, invalid dimensions,
/// capacity overflow, failed symbol placement, or failed PNG encoding.
pub fn encode_screen_payload(
    payload: &[u8],
    cover_png: &[u8],
) -> Result<ScreenEncodeOutput, ScreenError> {
    let (mut image, _) = carrier::decode_png(cover_png).map_err(|_| ScreenError::InvalidPng)?;
    let capacity_bytes = screen_capacity(image.width, image.height)?;
    if payload.is_empty() || payload.len() > capacity_bytes {
        return Err(ScreenError::CapacityExceeded);
    }
    let original = image.rgb_data.clone();
    let changed_channels = embed_image(&mut image, payload)?;
    let psnr_db = psnr(&original, &image.rgb_data)?;
    let png_bytes =
        carrier::encode_png_checked(&image).map_err(|_| ScreenError::PngEncodeFailed)?;
    Ok(ScreenEncodeOutput {
        png_bytes,
        capacity_bytes,
        changed_channels,
        psnr_db,
    })
}

/// Extracts opaque bytes from an exact-size experimental screen carrier.
///
/// # Errors
///
/// Returns [`ScreenError`] when PNG framing, dimensions, repeated header, or
/// any required differential symbol cannot be recovered.
pub fn decode_screen_payload(screenshot_png: &[u8]) -> Result<Vec<u8>, ScreenError> {
    let (image, _) = carrier::decode_png(screenshot_png).map_err(|_| ScreenError::InvalidPng)?;
    extract_image(&image)
}

fn geometry(width: u32, height: u32) -> Result<Geometry, ScreenError> {
    carrier::checked_capacity(width, height).map_err(|_| ScreenError::InvalidDimensions)?;
    let width = usize::try_from(width).map_err(|_| ScreenError::ArithmeticOverflow)?;
    let height = usize::try_from(height).map_err(|_| ScreenError::ArithmeticOverflow)?;
    let cell_columns = width / CELL_EDGE;
    let cell_rows = height / CELL_EDGE;
    let total_cells = cell_columns
        .checked_mul(cell_rows)
        .ok_or(ScreenError::ArithmeticOverflow)?;
    if total_cells <= HEADER_CELLS {
        return Err(ScreenError::InvalidDimensions);
    }
    Ok(Geometry {
        width,
        cell_columns,
        total_cells,
    })
}

fn embed_image(image: &mut RgbImage, payload: &[u8]) -> Result<usize, ScreenError> {
    let geometry = geometry(image.width, image.height)?;
    let capacity = (geometry.total_cells - HEADER_CELLS) / CELLS_PER_BYTE;
    if payload.is_empty() || payload.len() > capacity {
        return Err(ScreenError::CapacityExceeded);
    }
    let header = Header {
        payload_len: payload.len(),
    }
    .serialize()?;
    let mut changed_channels = 0;
    for copy in 0..HEADER_REPEATS {
        let start_cell = header_start_cell(geometry, copy)?;
        changed_channels += embed_bytes(image, geometry, &header, |symbol_offset| {
            start_cell
                .checked_add(symbol_offset)
                .ok_or(ScreenError::ArithmeticOverflow)
        })?;
    }
    changed_channels += embed_bytes(image, geometry, payload, |symbol_offset| {
        payload_physical_cell(geometry, symbol_offset)
    })?;
    Ok(changed_channels)
}

fn extract_image(image: &RgbImage) -> Result<Vec<u8>, ScreenError> {
    let geometry = geometry(image.width, image.height)?;
    let header = Header::parse(&extract_repeated_header(image, geometry)?)?;
    let capacity = (geometry.total_cells - HEADER_CELLS) / CELLS_PER_BYTE;
    if header.payload_len == 0 || header.payload_len > capacity {
        return Err(ScreenError::InvalidHeader);
    }
    extract_bytes(image, geometry, header.payload_len, |symbol_offset| {
        payload_physical_cell(geometry, symbol_offset)
    })
}

fn embed_bytes<CellForSymbol>(
    image: &mut RgbImage,
    geometry: Geometry,
    bytes: &[u8],
    cell_for_symbol: CellForSymbol,
) -> Result<usize, ScreenError>
where
    CellForSymbol: Fn(usize) -> Result<usize, ScreenError>,
{
    let required_cells = bytes
        .len()
        .checked_mul(CELLS_PER_BYTE)
        .ok_or(ScreenError::ArithmeticOverflow)?;
    if required_cells > geometry.total_cells - HEADER_CELLS {
        return Err(ScreenError::CapacityExceeded);
    }
    let mut changed_channels = 0;
    for (byte_index, &byte) in bytes.iter().enumerate() {
        for symbol_index in 0..CELLS_PER_BYTE {
            let shift = 6 - symbol_index * BITS_PER_CELL;
            let symbol = (byte >> shift) & 0b11;
            let logical_cell = byte_index * CELLS_PER_BYTE + symbol_index;
            let cell = cell_for_symbol(logical_cell)?;
            changed_channels += write_symbol(image, geometry, cell, symbol)?;
        }
    }
    Ok(changed_channels)
}

fn extract_bytes<CellForSymbol>(
    image: &RgbImage,
    geometry: Geometry,
    byte_len: usize,
    cell_for_symbol: CellForSymbol,
) -> Result<Vec<u8>, ScreenError>
where
    CellForSymbol: Fn(usize) -> Result<usize, ScreenError>,
{
    let required_cells = byte_len
        .checked_mul(CELLS_PER_BYTE)
        .ok_or(ScreenError::ArithmeticOverflow)?;
    if required_cells > geometry.total_cells - HEADER_CELLS {
        return Err(ScreenError::InvalidHeader);
    }
    let mut bytes = vec![0u8; byte_len];
    for (byte_index, byte) in bytes.iter_mut().enumerate() {
        for symbol_index in 0..CELLS_PER_BYTE {
            let logical_cell = byte_index * CELLS_PER_BYTE + symbol_index;
            let cell = cell_for_symbol(logical_cell)?;
            let symbol = read_symbol(image, geometry, cell)?;
            let shift = 6 - symbol_index * BITS_PER_CELL;
            *byte |= symbol << shift;
        }
    }
    Ok(bytes)
}

fn extract_repeated_header(
    image: &RgbImage,
    geometry: Geometry,
) -> Result<[u8; HEADER_LEN], ScreenError> {
    let mut bytes = [0u8; HEADER_LEN];
    for symbol_offset in 0..HEADER_SYMBOLS {
        let mut counts = [0u8; 4];
        for copy in 0..HEADER_REPEATS {
            let cell = header_start_cell(geometry, copy)? + symbol_offset;
            if let Ok(symbol) = read_symbol(image, geometry, cell) {
                counts[usize::from(symbol)] += 1;
            }
        }
        let symbol = unique_maximum(counts)?;
        let byte_index = symbol_offset / CELLS_PER_BYTE;
        let shift = 6 - (symbol_offset % CELLS_PER_BYTE) * BITS_PER_CELL;
        bytes[byte_index] |= symbol << shift;
    }
    Ok(bytes)
}

fn header_start_cell(geometry: Geometry, copy: usize) -> Result<usize, ScreenError> {
    if copy >= HEADER_REPEATS {
        return Err(ScreenError::ArithmeticOverflow);
    }
    let stride = geometry.total_cells / HEADER_REPEATS;
    let start = copy
        .checked_mul(stride)
        .ok_or(ScreenError::ArithmeticOverflow)?;
    if start
        .checked_add(HEADER_SYMBOLS)
        .ok_or(ScreenError::ArithmeticOverflow)?
        > geometry.total_cells
    {
        return Err(ScreenError::InvalidDimensions);
    }
    Ok(start)
}

fn payload_physical_cell(geometry: Geometry, logical_cell: usize) -> Result<usize, ScreenError> {
    if logical_cell >= geometry.total_cells - HEADER_CELLS {
        return Err(ScreenError::CapacityExceeded);
    }
    let mut physical = logical_cell;
    for copy in 0..HEADER_REPEATS {
        let header_start = header_start_cell(geometry, copy)?;
        if physical >= header_start {
            physical = physical
                .checked_add(HEADER_SYMBOLS)
                .ok_or(ScreenError::ArithmeticOverflow)?;
        }
    }
    if physical >= geometry.total_cells {
        return Err(ScreenError::CapacityExceeded);
    }
    Ok(physical)
}

fn unique_maximum(counts: [u8; 4]) -> Result<u8, ScreenError> {
    let maximum = counts.iter().copied().max().unwrap_or(0);
    let matching_maximums = counts
        .map(|count| u8::from(count == maximum))
        .into_iter()
        .sum::<u8>();
    if maximum == 0 || matching_maximums != 1 {
        return Err(ScreenError::UnrecoverableSymbol);
    }
    counts
        .iter()
        .position(|&count| count == maximum)
        .and_then(|index| u8::try_from(index).ok())
        .ok_or(ScreenError::UnrecoverableSymbol)
}

fn cell_channel_indices(geometry: Geometry, cell: usize) -> Result<[[usize; 3]; 4], ScreenError> {
    if cell >= geometry.total_cells {
        return Err(ScreenError::CapacityExceeded);
    }
    let cell_x = cell % geometry.cell_columns;
    let cell_y = cell / geometry.cell_columns;
    let x = cell_x * CELL_EDGE;
    let y = cell_y * CELL_EDGE;
    let pixel = |pixel_x: usize, pixel_y: usize| -> Result<[usize; 3], ScreenError> {
        let base = pixel_y
            .checked_mul(geometry.width)
            .and_then(|value| value.checked_add(pixel_x))
            .and_then(|value| value.checked_mul(3))
            .ok_or(ScreenError::ArithmeticOverflow)?;
        Ok([base, base + 1, base + 2])
    };
    Ok([
        pixel(x, y)?,
        pixel(x + 1, y)?,
        pixel(x, y + 1)?,
        pixel(x + 1, y + 1)?,
    ])
}

fn write_symbol(
    image: &mut RgbImage,
    geometry: Geometry,
    cell: usize,
    symbol: u8,
) -> Result<usize, ScreenError> {
    let indices = cell_channel_indices(geometry, cell)?;
    let horizontal_positive = symbol & 0b10 != 0;
    let vertical_positive = symbol & 0b01 != 0;
    let red_left = [indices[0][0], indices[2][0]];
    let red_right = [indices[1][0], indices[3][0]];
    let blue_top = [indices[0][2], indices[1][2]];
    let blue_bottom = [indices[2][2], indices[3][2]];
    let red_changed = force_group_difference(
        &mut image.rgb_data,
        red_left,
        red_right,
        horizontal_positive,
    )?;
    let blue_changed = force_group_difference(
        &mut image.rgb_data,
        blue_top,
        blue_bottom,
        vertical_positive,
    )?;
    Ok(red_changed + blue_changed)
}

fn read_symbol(image: &RgbImage, geometry: Geometry, cell: usize) -> Result<u8, ScreenError> {
    let indices = cell_channel_indices(geometry, cell)?;
    let left_red =
        u16::from(image.rgb_data[indices[0][0]]) + u16::from(image.rgb_data[indices[2][0]]);
    let right_red =
        u16::from(image.rgb_data[indices[1][0]]) + u16::from(image.rgb_data[indices[3][0]]);
    let top_blue =
        u16::from(image.rgb_data[indices[0][2]]) + u16::from(image.rgb_data[indices[1][2]]);
    let bottom_blue =
        u16::from(image.rgb_data[indices[2][2]]) + u16::from(image.rgb_data[indices[3][2]]);
    if left_red == right_red || top_blue == bottom_blue {
        return Err(ScreenError::UnrecoverableSymbol);
    }
    Ok((u8::from(left_red > right_red) << 1) | u8::from(top_blue > bottom_blue))
}

fn force_group_difference(
    data: &mut [u8],
    first: [usize; 2],
    second: [usize; 2],
    first_is_higher: bool,
) -> Result<usize, ScreenError> {
    let indices = [first[0], first[1], second[0], second[1]];
    let original = indices.map(|index| {
        data.get(index)
            .copied()
            .ok_or(ScreenError::InvalidDimensions)
    });
    let original = [original[0]?, original[1]?, original[2]?, original[3]?];
    let (higher, lower) = if first_is_higher {
        (first, second)
    } else {
        (second, first)
    };
    for _ in 0..=u8::MAX {
        let higher_sum = i32::from(data[higher[0]]) + i32::from(data[higher[1]]);
        let lower_sum = i32::from(data[lower[0]]) + i32::from(data[lower[1]]);
        if higher_sum - lower_sum >= DIFFERENCE_MARGIN {
            return Ok(indices
                .iter()
                .zip(original)
                .filter(|(index, before)| data[**index] != *before)
                .count());
        }
        let mut progressed = false;
        for &index in &higher {
            if data[index] < u8::MAX {
                data[index] += 1;
                progressed = true;
            }
        }
        for &index in &lower {
            if data[index] > 0 {
                data[index] -= 1;
                progressed = true;
            }
        }
        if !progressed {
            return Err(ScreenError::UnrecoverableSymbol);
        }
    }
    Err(ScreenError::UnrecoverableSymbol)
}

#[allow(clippy::cast_precision_loss)]
fn psnr(original: &[u8], encoded: &[u8]) -> Result<f64, ScreenError> {
    if original.len() != encoded.len() || original.is_empty() {
        return Err(ScreenError::InvalidDimensions);
    }
    let squared_error =
        original
            .iter()
            .zip(encoded)
            .try_fold(0u64, |accumulator, (&before, &after)| {
                let difference = i32::from(before) - i32::from(after);
                let square = u64::try_from(difference * difference)
                    .map_err(|_| ScreenError::ArithmeticOverflow)?;
                accumulator
                    .checked_add(square)
                    .ok_or(ScreenError::ArithmeticOverflow)
            })?;
    if squared_error == 0 {
        return Ok(f64::INFINITY);
    }
    let sample_count =
        u32::try_from(original.len()).map_err(|_| ScreenError::ArithmeticOverflow)?;
    let mse = squared_error as f64 / f64::from(sample_count);
    Ok(10.0 * ((255.0 * 255.0) / mse).log10())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EccProfile, ecc};

    const A57_WIDTH: u32 = 1_080;
    const A57_HEIGHT: u32 = 2_340;
    const PSALMS_CODEWORDS: usize = 556;

    fn synthetic_cover(width: u32, height: u32) -> RgbImage {
        let mut rgb_data = Vec::with_capacity(
            usize::try_from(width).unwrap() * usize::try_from(height).unwrap() * 3,
        );
        for y in 0..height {
            for x in 0..width {
                let texture = u8::try_from((x ^ y) & 31).unwrap();
                rgb_data.push(u8::try_from((x * 3 + y * 2) % 224).unwrap() + texture);
                rgb_data.push(u8::try_from((x + y * 5) % 224).unwrap() + texture);
                rgb_data.push(u8::try_from((x * 7 + y) % 224).unwrap() + texture);
            }
        }
        RgbImage {
            width,
            height,
            rgb_data,
        }
    }

    fn simulate_srgb_compositor(image: &RgbImage) -> RgbImage {
        let mut transformed = image.clone();
        let offsets = [2i16, -1, 3];
        for (index, value) in transformed.rgb_data.iter_mut().enumerate() {
            let corrected = (u16::from(*value) * 102 + 50) / 100;
            let corrected = i16::try_from(corrected).unwrap();
            let noise = i16::try_from((index * 17 + 5) % 3).unwrap() - 1;
            let channel = index % 3;
            *value = u8::try_from((corrected + offsets[channel] + noise).clamp(0, 255)).unwrap();
        }
        transformed
    }

    #[test]
    fn a57_screen_capacity_fits_measured_psalms_interleaving() {
        assert_eq!(157_806, screen_capacity(A57_WIDTH, A57_HEIGHT).unwrap());
        assert!(64 + PSALMS_CODEWORDS * 255 <= screen_capacity(A57_WIDTH, A57_HEIGHT).unwrap());
    }

    #[test]
    fn repeated_header_survives_two_destroyed_copies() {
        let mut image = synthetic_cover(160, 160);
        embed_image(&mut image, b"screen header majority").unwrap();
        let geometry = geometry(160, 160).unwrap();
        for copy in 0..2 {
            let start = header_start_cell(geometry, copy).unwrap();
            for symbol in 0..HEADER_SYMBOLS {
                write_symbol(&mut image, geometry, start + symbol, 0).unwrap();
            }
        }
        assert_eq!(
            b"screen header majority",
            extract_image(&image).unwrap().as_slice()
        );
    }

    #[test]
    fn public_png_api_roundtrips_opaque_payload() {
        let cover = synthetic_cover(320, 320);
        let cover_png = carrier::encode_png_checked(&cover).unwrap();
        let payload: Vec<u8> = (0..4_096)
            .map(|index| u8::try_from((index * 29 + 7) % 256).unwrap())
            .collect();

        let encoded = encode_screen_payload(&payload, &cover_png).unwrap();

        assert_eq!(6_256, encoded.capacity_bytes);
        assert!(encoded.changed_channels > payload.len() * 4);
        assert!(encoded.psnr_db > 24.0);
        assert_eq!(payload, decode_screen_payload(&encoded.png_bytes).unwrap());
    }

    #[test]
    fn a57_fullscreen_srgb_compositor_roundtrip_repairs_outer_rs_errors() {
        let profile = EccProfile::Safe;
        let ciphertext_len = PSALMS_CODEWORDS * profile.data_len();
        let ciphertext: Vec<u8> = (0..ciphertext_len)
            .map(|index| u8::try_from((index * 73 + 19) % 256).unwrap())
            .collect();
        let interleaved = ecc::encode(&ciphertext, profile).unwrap();
        assert_eq!(PSALMS_CODEWORDS * 255, interleaved.len());
        let mut frame = vec![0xA5; 64];
        frame.extend_from_slice(&interleaved);

        let mut image = synthetic_cover(A57_WIDTH, A57_HEIGHT);
        let original = image.rgb_data.clone();
        let changed_channels = embed_image(&mut image, &frame).unwrap();
        let measured_psnr = psnr(&original, &image.rgb_data).unwrap();
        eprintln!(
            "screen_lab payload={} capacity={} changed_channels={} psnr_db={measured_psnr:.3}",
            frame.len(),
            screen_capacity(A57_WIDTH, A57_HEIGHT).unwrap(),
            changed_channels,
        );
        assert!(changed_channels > frame.len() * 4);
        assert!(measured_psnr > 24.0, "measured PSNR: {measured_psnr}");

        let mut screenshot = simulate_srgb_compositor(&image);
        let geometry = geometry(A57_WIDTH, A57_HEIGHT).unwrap();
        for byte_offset in [0usize, 1, 2, 100, 1_000, 10_000, 80_000, 120_000] {
            let frame_byte = 64 + byte_offset;
            let cell = payload_physical_cell(geometry, frame_byte * CELLS_PER_BYTE).unwrap();
            let symbol = read_symbol(&screenshot, geometry, cell).unwrap();
            write_symbol(&mut screenshot, geometry, cell, symbol ^ 0b11).unwrap();
        }

        let screenshot_png = carrier::encode_png_checked(&screenshot).unwrap();
        let recovered_frame = decode_screen_payload(&screenshot_png).unwrap();
        assert_eq!(&[0xA5; 64], &recovered_frame[..64]);
        assert_ne!(interleaved, recovered_frame[64..]);
        let recovered = ecc::decode(&recovered_frame[64..], PSALMS_CODEWORDS, profile).unwrap();
        assert_eq!(ciphertext, recovered);
    }
}
