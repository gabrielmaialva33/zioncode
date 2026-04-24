//! Full extract pipeline: stego images → bytes → A decoder → original file.
//! See spec §4.

use crate::aead::open;
use crate::constants::{PLAINTEXT_HEADER_CHANNELS, PLAINTEXT_HEADER_LEN};
use crate::error::ExtractError;
use crate::kdf::{derive_aead_key, derive_master_key, derive_nonce, derive_permutation_seed};
use crate::lsb::{bits_to_bytes, extract_bits_at};
use crate::permutation::permute_range;
use crate::plaintext_header::PlaintextHeader;
use crate::png_io::RgbImage;

#[derive(Debug)]
pub struct ExtractOutput {
    pub file_bytes: Vec<u8>,
    pub file_id: [u8; 16],
}

/// Public metadata stored in the passphrase-free plaintext stego header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StegoMetadata {
    pub width: u32,
    pub height: u32,
    pub file_id: [u8; 16],
    pub k_global: u16,
    pub symbol_index: u16,
    pub total_symbols: u16,
    pub ciphertext_len: u32,
}

/// Inspect a stego image's plaintext header without a passphrase.
///
/// # Errors
/// Returns `ExtractError` if the image dimensions are inconsistent, the image
/// is too small, or the plaintext header is invalid.
pub fn inspect_stego_image(image: &RgbImage, index: usize) -> Result<StegoMetadata, ExtractError> {
    let header = read_plaintext_header(image, index)?;
    Ok(StegoMetadata {
        width: image.width,
        height: image.height,
        file_id: header.file_id,
        k_global: header.k_global,
        symbol_index: header.symbol_index,
        total_symbols: header.total_symbols,
        ciphertext_len: header.ciphertext_len,
    })
}

/// Extracts the original file from a set of stego images.
///
/// # Errors
/// - `NoStegoImages`, `InvalidStegoImage`, `BadMagic`, `UnsupportedVersion`
/// - `AeadAuthFailed` (wrong passphrase or tampered image)
/// - `InconsistentFileId` (images from different files mixed)
/// - `ReassemblyFailed` (codec A reassembly error)
/// - `SymbolDecodeError` (codec A symbol-level error)
///
/// # Panics
/// Panics only on internal invariants (fixed-size byte conversions whose
/// bounds are statically guaranteed by `PLAINTEXT_HEADER_LEN`).
pub fn extract_file(
    stego_images: &[RgbImage],
    passphrase: &str,
) -> Result<ExtractOutput, ExtractError> {
    if stego_images.is_empty() {
        return Err(ExtractError::NoStegoImages);
    }

    // 1. Read plaintext header from each stego image (file_id + symbol_index).
    let mut headers: Vec<(usize, PlaintextHeader)> = Vec::with_capacity(stego_images.len());
    for (i, img) in stego_images.iter().enumerate() {
        headers.push((i, read_plaintext_header(img, i)?));
    }

    // 2. Consistency: all headers must share the same file_id.
    let file_id = headers[0].1.file_id;
    for (_, h) in &headers[1..] {
        if h.file_id != file_id {
            return Err(ExtractError::InconsistentFileId);
        }
    }

    // 3. Derive master_key from passphrase + file_id.
    let master_key =
        derive_master_key(passphrase.as_bytes(), &file_id).map_err(ExtractError::KdfFailed)?;
    let aead_key = derive_aead_key(&master_key);

    // 4. For each stego image, decrypt payload and feed to A decoder.
    let mut reasm = zion_codec::FileReassembler::new();

    for (img_idx, header) in &headers {
        let img = &stego_images[*img_idx];

        let total_channels =
            u32::try_from(img.rgb_data.len()).expect("fits u32 for practical image sizes");
        let perm_seed = derive_permutation_seed(&master_key, &file_id, header.symbol_index);
        let permutation = permute_range(
            u32::try_from(PLAINTEXT_HEADER_CHANNELS).expect("PLAINTEXT_HEADER_CHANNELS fits u32"),
            total_channels,
            &perm_seed,
        );

        let needed_bits = (header.ciphertext_len as usize) * 8;
        if needed_bits > permutation.len() {
            return Err(ExtractError::InvalidStegoImage {
                index: *img_idx,
                expected: needed_bits,
                got: permutation.len(),
            });
        }

        let cipher_bits = extract_bits_at(&img.rgb_data, &permutation[..needed_bits]);
        let ciphertext = bits_to_bytes(&cipher_bits);

        let nonce = derive_nonce(&master_key, header.symbol_index);
        let symbol_bytes =
            open(&aead_key, &nonce, &ciphertext).map_err(|_| ExtractError::AeadAuthFailed {
                symbol_index: header.symbol_index,
            })?;

        let decoded = zion_codec::decode_symbol(&symbol_bytes)?;
        reasm.add_symbol(decoded)?;
    }

    let file_bytes = reasm.finalize()?;
    Ok(ExtractOutput {
        file_bytes,
        file_id,
    })
}

fn read_plaintext_header(image: &RgbImage, index: usize) -> Result<PlaintextHeader, ExtractError> {
    let expected = image.expected_rgb_len();
    if image.rgb_data.len() != expected {
        return Err(ExtractError::InvalidStegoImage {
            index,
            expected,
            got: image.rgb_data.len(),
        });
    }
    if image.rgb_data.len() < PLAINTEXT_HEADER_CHANNELS {
        return Err(ExtractError::InvalidStegoImage {
            index,
            expected: PLAINTEXT_HEADER_CHANNELS,
            got: image.rgb_data.len(),
        });
    }
    let header_positions: Vec<u32> = (0..u32::try_from(PLAINTEXT_HEADER_CHANNELS)
        .expect("PLAINTEXT_HEADER_CHANNELS fits u32"))
        .collect();
    let header_bits = extract_bits_at(&image.rgb_data, &header_positions);
    let header_bytes_vec = bits_to_bytes(&header_bits);
    let header_bytes: [u8; PLAINTEXT_HEADER_LEN] = header_bytes_vec
        .as_slice()
        .try_into()
        .expect("256 bits = 32 bytes");
    PlaintextHeader::parse(&header_bytes, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{EmbedParams, embed_file};

    fn make_host(w: u32, h: u32, seed: u8) -> RgbImage {
        let len = (w as usize) * (h as usize) * 3;
        let base = seed | 1;
        let data: Vec<u8> = (0..len)
            .map(|i| {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "intentional truncation for test fixture pattern"
                )]
                let v = i as u8;
                v.wrapping_mul(base)
            })
            .collect();
        RgbImage {
            width: w,
            height: h,
            rgb_data: data,
        }
    }

    #[test]
    fn empty_stego_list_rejected() {
        let err = extract_file(&[], "x").unwrap_err();
        assert!(matches!(err, ExtractError::NoStegoImages));
    }

    #[test]
    fn inspect_reads_public_metadata() {
        let host = make_host(512, 512, 17);
        let params = EmbedParams {
            passphrase: "light rain".into(),
            ..Default::default()
        };
        let out = embed_file(b"payload", vec![host], &params).unwrap();
        let metadata = inspect_stego_image(&out.stego_images[0], 0).unwrap();

        assert_eq!(metadata.file_id, out.file_id);
        assert_eq!(metadata.k_global, out.k_global);
        assert_eq!(metadata.symbol_index, 0);
        assert_eq!(
            metadata.total_symbols,
            u16::try_from(out.symbols_used).unwrap()
        );
    }

    #[test]
    fn roundtrip_single_photo() {
        let host = make_host(512, 512, 17);
        let params = EmbedParams {
            passphrase: "light rain".into(),
            ..Default::default()
        };
        let payload = b"forbidden verse";
        let out = embed_file(payload, vec![host], &params).unwrap();
        let stego = &out.stego_images[..out.symbols_used];
        let rec = extract_file(stego, "light rain").unwrap();
        assert_eq!(rec.file_bytes, payload);
        assert_eq!(rec.file_id, out.file_id);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let host = make_host(512, 512, 17);
        let params = EmbedParams {
            passphrase: "right phrase".into(),
            ..Default::default()
        };
        let payload = b"verse";
        let out = embed_file(payload, vec![host], &params).unwrap();
        let stego = &out.stego_images[..out.symbols_used];
        let err = extract_file(stego, "wrong phrase").unwrap_err();
        assert!(matches!(err, ExtractError::AeadAuthFailed { .. }));
    }

    #[test]
    fn tampered_stego_fails() {
        let host = make_host(512, 512, 17);
        let params = EmbedParams {
            passphrase: "password".into(),
            ..Default::default()
        };
        let out = embed_file(b"data", vec![host], &params).unwrap();
        let mut stego = out.stego_images[0].clone();
        // Flip the LSB of every channel past the plaintext header region.
        // Guarantees that every ciphertext bit we extract is inverted, so the
        // AEAD tag will fail to authenticate.
        for byte in &mut stego.rgb_data[PLAINTEXT_HEADER_CHANNELS..] {
            *byte ^= 0x01;
        }
        let err = extract_file(&[stego], "password").unwrap_err();
        assert!(matches!(err, ExtractError::AeadAuthFailed { .. }));
    }
}
