//! Wrapper mínimo sobre `png` 0.17 pra ler/escrever PNG em RGB 8-bit.
//! Converte outros formatos (RGBA, grayscale) pra RGB conforme necessário.

use std::io::{Read, Write};

use crate::error::{EmbedError, ExtractError};

/// Imagem RGB 8-bit plana.
#[derive(Debug, Clone)]
pub struct RgbImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, row-major, cada pixel = (R, G, B).
    pub rgb_data: Vec<u8>,
}

/// Lê um PNG e converte pra RGB 8-bit.
///
/// # Errors
/// Retorna erro se o PNG for inválido ou se a conversão falhar.
pub fn load_png_rgb<R: Read>(reader: R) -> Result<RgbImage, String> {
    let decoder = png::Decoder::new(reader);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());

    let rgb_data = match info.color_type {
        png::ColorType::Rgb => {
            if info.bit_depth != png::BitDepth::Eight {
                return Err(format!("PNG RGB depth != 8: {:?}", info.bit_depth));
            }
            buf
        }
        png::ColorType::Rgba => {
            if info.bit_depth != png::BitDepth::Eight {
                return Err(format!("PNG RGBA depth != 8: {:?}", info.bit_depth));
            }
            #[allow(
                clippy::cast_possible_truncation,
                reason = "width*height*3 fits in usize on 64-bit targets"
            )]
            let cap = (info.width as usize) * (info.height as usize) * 3;
            let mut rgb = Vec::with_capacity(cap);
            for px in buf.chunks_exact(4) {
                rgb.extend_from_slice(&px[..3]);
            }
            rgb
        }
        png::ColorType::Grayscale => {
            if info.bit_depth != png::BitDepth::Eight {
                return Err(format!("PNG Grayscale depth != 8: {:?}", info.bit_depth));
            }
            #[allow(
                clippy::cast_possible_truncation,
                reason = "width*height*3 fits in usize on 64-bit targets"
            )]
            let cap = (info.width as usize) * (info.height as usize) * 3;
            let mut rgb = Vec::with_capacity(cap);
            for &g in &buf {
                rgb.extend_from_slice(&[g, g, g]);
            }
            rgb
        }
        other => {
            return Err(format!("color_type não suportado: {other:?}"));
        }
    };

    Ok(RgbImage {
        width: info.width,
        height: info.height,
        rgb_data,
    })
}

/// Escreve um PNG RGB 8-bit.
///
/// # Errors
/// Retorna erro se a encodificação falhar.
pub fn save_png_rgb<W: Write>(image: &RgbImage, writer: W) -> Result<(), String> {
    let expected_len = (image.width as usize) * (image.height as usize) * 3;
    if image.rgb_data.len() != expected_len {
        return Err(format!(
            "rgb_data len {}, expected {}",
            image.rgb_data.len(),
            expected_len
        ));
    }
    let mut encoder = png::Encoder::new(writer, image.width, image.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut w = encoder.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&image.rgb_data)
        .map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())?;
    Ok(())
}

impl From<String> for EmbedError {
    fn from(e: String) -> Self {
        EmbedError::PngError(e)
    }
}

impl From<String> for ExtractError {
    fn from(e: String) -> Self {
        ExtractError::PngError(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_rgb(w: u32, h: u32) -> RgbImage {
        let len = (w * h * 3) as usize;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "test fixture: byte pattern from index intentionally wraps"
        )]
        let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(17)).collect();
        RgbImage {
            width: w,
            height: h,
            rgb_data: data,
        }
    }

    #[test]
    fn roundtrip_rgb() {
        let img = sample_rgb(64, 48);
        let mut buf = Vec::new();
        save_png_rgb(&img, &mut buf).unwrap();

        let recovered = load_png_rgb(Cursor::new(&buf)).unwrap();
        assert_eq!(recovered.width, 64);
        assert_eq!(recovered.height, 48);
        assert_eq!(recovered.rgb_data, img.rgb_data);
    }

    #[test]
    fn rgba_input_strips_alpha() {
        let w = 16u32;
        let h = 8u32;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "test fixture: byte pattern from index intentionally wraps"
        )]
        let rgba: Vec<u8> = (0..(w * h * 4) as usize)
            .map(|i| (i as u8).wrapping_mul(13))
            .collect();

        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&rgba).unwrap();
        }

        let img = load_png_rgb(Cursor::new(&buf)).unwrap();
        assert_eq!(img.rgb_data.len(), (w * h * 3) as usize);
        for (i, &byte) in img.rgb_data.iter().enumerate() {
            let pixel = i / 3;
            let channel = i % 3;
            let rgba_idx = pixel * 4 + channel;
            assert_eq!(byte, rgba[rgba_idx]);
        }
    }

    #[test]
    fn grayscale_input_triples() {
        let w = 8u32;
        let h = 4u32;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "test fixture: small w*h, byte pattern from index"
        )]
        let gray: Vec<u8> = (0..(w * h) as usize).map(|i| i as u8).collect();

        let mut buf = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut buf, w, h);
            encoder.set_color(png::ColorType::Grayscale);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&gray).unwrap();
        }

        let img = load_png_rgb(Cursor::new(&buf)).unwrap();
        for (pixel, &g) in gray.iter().enumerate() {
            assert_eq!(img.rgb_data[pixel * 3], g);
            assert_eq!(img.rgb_data[pixel * 3 + 1], g);
            assert_eq!(img.rgb_data[pixel * 3 + 2], g);
        }
    }
}
