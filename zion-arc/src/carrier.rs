use std::io::Cursor;

use zeroize::Zeroize;
use zion_stego::RgbImage;

use crate::constants::{
    BOOTSTRAP_CHANNELS, BOOTSTRAP_LEN, MAX_PERMUTATION_INDICES, MAX_PNG_BYTES,
    MAX_PNG_DECODER_BYTES,
};
use crate::types::Capacity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CarrierError {
    InputTooLarge,
    InvalidPng,
    UnsupportedPixelFormat,
    InvalidDimensions,
    CarrierTooSmall,
}

pub(crate) fn decode_png(png_bytes: &[u8]) -> Result<(RgbImage, Capacity), CarrierError> {
    let (width, height, raster_len) = preflight_png(png_bytes)?;
    let decoder = png::Decoder::new_with_limits(
        Cursor::new(png_bytes),
        png::Limits {
            bytes: MAX_PNG_DECODER_BYTES,
        },
    );
    let mut reader = decoder.read_info().map_err(|_| CarrierError::InvalidPng)?;
    if reader.info().width != width || reader.info().height != height {
        return Err(CarrierError::InvalidPng);
    }
    if reader.output_color_type() != (png::ColorType::Rgb, png::BitDepth::Eight) {
        return Err(CarrierError::UnsupportedPixelFormat);
    }
    if reader.output_buffer_size() != Some(raster_len) {
        return Err(CarrierError::InvalidDimensions);
    }
    let mut rgb_data = vec![0u8; raster_len];
    let output = reader
        .next_frame(&mut rgb_data)
        .map_err(|_| CarrierError::InvalidPng)?;
    if output.width != width
        || output.height != height
        || output.color_type != png::ColorType::Rgb
        || output.bit_depth != png::BitDepth::Eight
        || output.buffer_size() != raster_len
    {
        return Err(CarrierError::InvalidPng);
    }
    reader.finish().map_err(|_| CarrierError::InvalidPng)?;
    let image = RgbImage {
        width,
        height,
        rgb_data,
    };
    let capacity = checked_capacity(width, height)?;
    if image.rgb_data.len() != capacity.channels() {
        return Err(CarrierError::InvalidDimensions);
    }
    Ok((image, capacity))
}

pub(crate) fn encode_png_checked(image: &RgbImage) -> Result<Vec<u8>, CarrierError> {
    let capacity = checked_capacity(image.width, image.height)?;
    if image.rgb_data.len() != capacity.channels() {
        return Err(CarrierError::InvalidDimensions);
    }
    let mut png_bytes = Vec::new();
    zion_stego::save_png_rgb(image, &mut png_bytes).map_err(|_| CarrierError::InvalidPng)?;
    if png_bytes.len() > MAX_PNG_BYTES {
        return Err(CarrierError::InputTooLarge);
    }
    let (decoded, _) = decode_png(&png_bytes)?;
    if decoded.width != image.width
        || decoded.height != image.height
        || decoded.rgb_data != image.rgb_data
    {
        return Err(CarrierError::InvalidPng);
    }
    Ok(png_bytes)
}

pub(crate) fn checked_capacity(width: u32, height: u32) -> Result<Capacity, CarrierError> {
    Capacity::checked(width, height).map_err(|error| match error {
        crate::error::SealError::CarrierTooSmall => CarrierError::CarrierTooSmall,
        _ => CarrierError::InvalidDimensions,
    })
}

fn preflight_png(png_bytes: &[u8]) -> Result<(u32, u32, usize), CarrierError> {
    if png_bytes.len() > MAX_PNG_BYTES {
        return Err(CarrierError::InputTooLarge);
    }
    if png_bytes.len() < 33 || png_bytes[..8] != *b"\x89PNG\r\n\x1a\n" {
        return Err(CarrierError::InvalidPng);
    }
    if u32::from_be_bytes(
        png_bytes[8..12]
            .try_into()
            .map_err(|_| CarrierError::InvalidPng)?,
    ) != 13
        || png_bytes[12..16] != *b"IHDR"
    {
        return Err(CarrierError::InvalidPng);
    }
    let width = u32::from_be_bytes(
        png_bytes[16..20]
            .try_into()
            .map_err(|_| CarrierError::InvalidPng)?,
    );
    let height = u32::from_be_bytes(
        png_bytes[20..24]
            .try_into()
            .map_err(|_| CarrierError::InvalidPng)?,
    );
    if png_bytes[24] != 8
        || png_bytes[25] != 2
        || png_bytes[26] != 0
        || png_bytes[27] != 0
        || png_bytes[28] > 1
    {
        return Err(CarrierError::UnsupportedPixelFormat);
    }
    let capacity = checked_capacity(width, height)?;

    let mut offset = 8usize;
    let mut saw_idat = false;
    let mut saw_iend = false;
    while offset < png_bytes.len() {
        let header_end = offset.checked_add(8).ok_or(CarrierError::InvalidPng)?;
        if header_end > png_bytes.len() {
            return Err(CarrierError::InvalidPng);
        }
        let chunk_len = u32::from_be_bytes(
            png_bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| CarrierError::InvalidPng)?,
        ) as usize;
        let chunk_type = &png_bytes[offset + 4..offset + 8];
        let chunk_end = header_end
            .checked_add(chunk_len)
            .and_then(|end| end.checked_add(4))
            .ok_or(CarrierError::InvalidPng)?;
        if chunk_end > png_bytes.len() {
            return Err(CarrierError::InvalidPng);
        }
        if chunk_type == b"acTL" || chunk_type == b"fcTL" || chunk_type == b"fdAT" {
            return Err(CarrierError::InvalidPng);
        }
        if chunk_type == b"IDAT" {
            saw_idat = true;
        }
        if chunk_type == b"IEND" {
            if chunk_len != 0 || chunk_end != png_bytes.len() {
                return Err(CarrierError::InvalidPng);
            }
            saw_iend = true;
            break;
        }
        offset = chunk_end;
    }
    if !saw_idat || !saw_iend {
        return Err(CarrierError::InvalidPng);
    }
    Ok((width, height, capacity.channels()))
}

pub(crate) fn embed_bootstrap(
    channels: &mut [u8],
    bootstrap: &[u8; BOOTSTRAP_LEN],
) -> Result<(), ()> {
    if channels.len() < BOOTSTRAP_CHANNELS {
        return Err(());
    }
    for (bit_index, channel) in channels[..BOOTSTRAP_CHANNELS].iter_mut().enumerate() {
        let bit = (bootstrap[bit_index / 8] >> (7 - bit_index % 8)) & 1;
        if (*channel & 1) != bit {
            *channel = if *channel == 255 { 254 } else { *channel + 1 };
        }
    }
    Ok(())
}

pub(crate) fn extract_bootstrap(channels: &[u8]) -> Result<[u8; BOOTSTRAP_LEN], ()> {
    if channels.len() < BOOTSTRAP_CHANNELS {
        return Err(());
    }
    let mut bytes = [0u8; BOOTSTRAP_LEN];
    for (bit_index, &channel) in channels[..BOOTSTRAP_CHANNELS].iter().enumerate() {
        bytes[bit_index / 8] |= (channel & 1) << (7 - bit_index % 8);
    }
    Ok(bytes)
}

pub(crate) fn payload_positions(
    channels: usize,
    seed: &[u8; 32],
    required_bits: usize,
) -> Result<Vec<u32>, ()> {
    let candidate_count = channels.checked_sub(BOOTSTRAP_CHANNELS).ok_or(())?;
    if candidate_count > MAX_PERMUTATION_INDICES || required_bits > candidate_count {
        return Err(());
    }
    let end = u32::try_from(channels).map_err(|_| ())?;
    let start = u32::try_from(BOOTSTRAP_CHANNELS).map_err(|_| ())?;
    let mut indices: Vec<u32> = (start..end).collect();
    let mut rng = ArcPrng::new(seed);
    for index in (1..indices.len()).rev() {
        let bound = u64::try_from(index).map_err(|_| ())? + 1;
        let threshold = u32::try_from((1u64 << 32) % bound).map_err(|_| ())?;
        let random = loop {
            let candidate = rng.next_u32()?;
            if candidate >= threshold {
                break candidate;
            }
        };
        let swap_index = usize::try_from(u64::from(random) % bound).map_err(|_| ())?;
        indices.swap(index, swap_index);
    }
    indices.truncate(required_bits);
    Ok(indices.into_boxed_slice().into_vec())
}

pub(crate) fn embed_payload(
    channels: &mut [u8],
    positions: &[u32],
    bytes: &[u8],
    matching_seed: &[u8; 32],
) -> Result<(), ()> {
    let required_bits = bytes.len().checked_mul(8).ok_or(())?;
    if positions.len() != required_bits {
        return Err(());
    }
    let mut rng = ArcPrng::new(matching_seed);
    for (bit_index, &position) in positions.iter().enumerate() {
        let direction = rng.next_u32()?;
        let target = (bytes[bit_index / 8] >> (7 - bit_index % 8)) & 1;
        let channel = channels.get_mut(position as usize).ok_or(())?;
        if (*channel & 1) != target {
            *channel = match *channel {
                0 => 1,
                255 => 254,
                value if direction & 1 == 0 => value - 1,
                value => value + 1,
            };
        }
    }
    Ok(())
}

pub(crate) fn extract_payload(
    channels: &[u8],
    positions: &[u32],
    byte_len: usize,
) -> Result<Vec<u8>, ()> {
    if positions.len() != byte_len.checked_mul(8).ok_or(())? {
        return Err(());
    }
    let mut bytes = vec![0u8; byte_len];
    for (bit_index, &position) in positions.iter().enumerate() {
        let channel = channels.get(position as usize).ok_or(())?;
        bytes[bit_index / 8] |= (channel & 1) << (7 - bit_index % 8);
    }
    Ok(bytes)
}

pub(crate) struct ArcPrng {
    state: [u32; 16],
    block: [u32; 16],
    next_word: usize,
    exhausted: bool,
}

impl ArcPrng {
    pub(crate) fn new(seed: &[u8; 32]) -> Self {
        let mut state = [0u32; 16];
        state[..4].copy_from_slice(&[0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574]);
        for (index, chunk) in seed.chunks_exact(4).enumerate() {
            state[4 + index] = u32::from_le_bytes(chunk.try_into().expect("fixed slice"));
        }
        Self {
            state,
            block: [0; 16],
            next_word: 16,
            exhausted: false,
        }
    }

    pub(crate) fn next_u32(&mut self) -> Result<u32, ()> {
        if self.next_word == 16 {
            self.refill()?;
        }
        let value = self.block[self.next_word];
        self.next_word += 1;
        Ok(value)
    }

    fn refill(&mut self) -> Result<(), ()> {
        if self.exhausted {
            return Err(());
        }
        let mut working = self.state;
        for _ in 0..10 {
            quarter_round(&mut working, 0, 4, 8, 12);
            quarter_round(&mut working, 1, 5, 9, 13);
            quarter_round(&mut working, 2, 6, 10, 14);
            quarter_round(&mut working, 3, 7, 11, 15);
            quarter_round(&mut working, 0, 5, 10, 15);
            quarter_round(&mut working, 1, 6, 11, 12);
            quarter_round(&mut working, 2, 7, 8, 13);
            quarter_round(&mut working, 3, 4, 9, 14);
        }
        for (output, (&work, &state)) in self
            .block
            .iter_mut()
            .zip(working.iter().zip(self.state.iter()))
        {
            *output = work.wrapping_add(state);
        }
        working.zeroize();
        self.next_word = 0;
        if self.state[12] == u32::MAX {
            self.exhausted = true;
        } else {
            self.state[12] += 1;
        }
        Ok(())
    }
}

impl Drop for ArcPrng {
    fn drop(&mut self) {
        self.state.zeroize();
        self.block.zeroize();
    }
}

fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    let (mut av, mut bv, mut cv, mut dv) = (state[a], state[b], state[c], state[d]);
    av = av.wrapping_add(bv);
    dv = (dv ^ av).rotate_left(16);
    cv = cv.wrapping_add(dv);
    bv = (bv ^ cv).rotate_left(12);
    av = av.wrapping_add(bv);
    dv = (dv ^ av).rotate_left(8);
    cv = cv.wrapping_add(dv);
    bv = (bv ^ cv).rotate_left(7);
    state[a] = av;
    state[b] = bv;
    state[c] = cv;
    state[d] = dv;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::MAX_DIMENSION;
    use hex_literal::hex;

    fn valid_rgb_png() -> Vec<u8> {
        let image = RgbImage {
            width: 64,
            height: 64,
            rgb_data: vec![0x7F; 64 * 64 * 3],
        };
        encode_png_checked(&image).unwrap()
    }

    #[test]
    fn arc_prng_zero_seed_matches_normative_block() {
        let mut rng = ArcPrng::new(&[0; 32]);
        let mut bytes = Vec::with_capacity(64);
        for _ in 0..16 {
            bytes.extend_from_slice(&rng.next_u32().unwrap().to_le_bytes());
        }
        assert_eq!(
            bytes,
            hex!(
                "76b8e0ada0f13d90405d6ae55386bd28
                 bdd219b8a08ded1aa836efcc8b770dc7
                 da41597c5157488d7724e03fb8d84a37
                 6a43b8f41518a11cc387b669b2ee6586"
            )
        );
    }

    #[test]
    fn a57_capacity_is_exact() {
        let capacity = checked_capacity(1080, 2340).unwrap();
        assert_eq!(capacity.channels(), 7_581_600);
        assert_eq!(capacity.usable_bits(), 2_501_759);
        assert_eq!(capacity.max_codewords(), 1_226);
    }

    #[test]
    fn dimension_validation_is_centralized_and_globally_bounded() {
        assert_eq!(
            checked_capacity(0, 64),
            Err(CarrierError::InvalidDimensions)
        );
        assert_eq!(
            checked_capacity(MAX_DIMENSION + 1, 1),
            Err(CarrierError::InvalidDimensions)
        );
        assert_eq!(
            checked_capacity(4_096, 4_096),
            Err(CarrierError::InvalidDimensions)
        );
        assert_eq!(checked_capacity(1, 1), Err(CarrierError::CarrierTooSmall));
    }

    #[test]
    fn permutation_result_has_no_candidate_workspace_slack() {
        let positions = payload_positions(8_192, &[0x42; 32], 257).unwrap();
        assert_eq!(positions.len(), 257);
        assert_eq!(positions.capacity(), positions.len());
    }

    #[test]
    fn png_preflight_rejects_non_rgb8_apng_and_bytes_after_iend() {
        let mut grayscale = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut grayscale, 64, 64);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&vec![0x55; 64 * 64]).unwrap();
        }
        assert!(matches!(
            decode_png(&grayscale),
            Err(CarrierError::UnsupportedPixelFormat)
        ));

        let mut apng = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut apng, 64, 64);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_animated(1, 0).unwrap();
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&vec![0x22; 64 * 64 * 3]).unwrap();
        }
        assert!(matches!(decode_png(&apng), Err(CarrierError::InvalidPng)));

        let mut trailing = valid_rgb_png();
        trailing.extend_from_slice(b"ARC_TRAILING_SENTINEL");
        assert!(matches!(
            decode_png(&trailing),
            Err(CarrierError::InvalidPng)
        ));
    }
}
