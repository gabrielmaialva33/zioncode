//! Full embedding pipeline: file → A codec → symbols → B (encrypt + permute + LSB).
//! See spec §4.

use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;
use zeroize::Zeroizing;

use crate::aead::seal;
use crate::capacity::{ciphertext_bytes, compute_k_global};
use crate::constants::PLAINTEXT_HEADER_CHANNELS;
use crate::error::EmbedError;
use crate::kdf::{derive_aead_key, derive_master_key, derive_nonce, derive_permutation_seed};
use crate::lsb::{bytes_to_bits, embed_bits_at};
use crate::permutation::permute_range;
use crate::plaintext_header::PlaintextHeader;
use crate::png_io::RgbImage;
use crate::types::EmbeddingDensity;

/// User-facing parameters for the embed pipeline.
pub struct EmbedParams {
    pub passphrase: String,
    pub density: EmbeddingDensity,
    pub zstd_level: i32,
}

impl Default for EmbedParams {
    fn default() -> Self {
        Self {
            passphrase: String::new(),
            density: EmbeddingDensity::default(),
            zstd_level: zion_codec::ZSTD_LEVEL_DEFAULT,
        }
    }
}

/// Result of a full embed: one stego image per host, plus bookkeeping fields.
#[derive(Debug)]
pub struct EmbedOutput {
    pub stego_images: Vec<RgbImage>,
    pub file_id: [u8; 16],
    pub global_hash: [u8; 32],
    pub k_global: u16,
    pub symbols_used: usize,
    pub hosts_provided: usize,
}

/// Runs the full embed pipeline: codec A + B (encrypt + permute + LSB).
///
/// # Errors
/// Propagates codec A, Argon2id, AEAD, or capacity-check errors.
///
/// # Panics
/// Panics only on internal invariants (checked `u16`/`u32` conversions whose
/// bounds are verified up-front by capacity/clamp logic).
pub fn embed_file(
    file_bytes: &[u8],
    host_images: Vec<RgbImage>,
    params: &EmbedParams,
) -> Result<EmbedOutput, EmbedError> {
    if file_bytes.is_empty() {
        return Err(EmbedError::EmptyInput);
    }
    if host_images.is_empty() {
        return Err(EmbedError::NoHostImages);
    }
    for (i, img) in host_images.iter().enumerate() {
        let expected = (img.width as usize) * (img.height as usize) * 3;
        if img.rgb_data.len() != expected {
            return Err(EmbedError::InvalidHostImage {
                index: i,
                expected,
                got: img.rgb_data.len(),
            });
        }
    }

    // 1. Compute K_global from host dimensions, density, and MAX_K clamp.
    let dims: Vec<(u32, u32)> = host_images.iter().map(|h| (h.width, h.height)).collect();
    let k_global =
        compute_k_global(&dims, params.density).ok_or(EmbedError::InsufficientCapacity {
            needed: 0,
            available: 0,
        })?;

    // 2. Invoke codec A with K_global.
    let encoded = zion_codec::low_level::encode_file(file_bytes, k_global, params.zstd_level)?;
    let symbols_used = encoded.symbols.len();
    let hosts_provided = host_images.len();

    if symbols_used > hosts_provided {
        return Err(EmbedError::InsufficientCapacity {
            needed: symbols_used,
            available: hosts_provided,
        });
    }

    let file_id = encoded.file_id.to_bytes();
    let global_hash = encoded.global_hash.to_bytes();

    // 3. Derive master_key (Argon2id).
    let master_key =
        derive_master_key(params.passphrase.as_bytes(), &file_id).map_err(EmbedError::KdfFailed)?;
    let aead_key = derive_aead_key(&master_key);

    // 4. For each symbol i, embed into host_images[i]; leftover hosts remain unchanged.
    let mut stego_images: Vec<RgbImage> = Vec::with_capacity(hosts_provided);
    let mut host_iter = host_images.into_iter();

    for (i, symbol_bytes) in encoded.symbols.iter().enumerate() {
        let mut host = host_iter
            .next()
            .expect("hosts_provided >= symbols_used verified above");
        let symbol_index = u16::try_from(i).expect("i < total_symbols fits u16");
        let total_symbols = u16::try_from(symbols_used).expect("symbols_used fits u16");

        embed_into_host(
            &mut host,
            symbol_bytes.as_bytes(),
            symbol_index,
            total_symbols,
            k_global,
            &file_id,
            &master_key,
            &aead_key,
        )?;

        stego_images.push(host);
    }

    // 5. Remaining hosts: append unchanged.
    for remaining in host_iter {
        stego_images.push(remaining);
    }

    Ok(EmbedOutput {
        stego_images,
        file_id,
        global_hash,
        k_global,
        symbols_used,
        hosts_provided,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "internal helper, call site is local"
)]
fn embed_into_host(
    host: &mut RgbImage,
    symbol_bytes: &[u8],
    symbol_index: u16,
    total_symbols: u16,
    k_global: u16,
    file_id: &[u8; 16],
    master_key: &Zeroizing<[u8; 32]>,
    aead_key: &Zeroizing<[u8; 32]>,
) -> Result<(), EmbedError> {
    // 1. Plaintext header (32 bytes) at channels [0..256) row-major.
    let header = PlaintextHeader {
        k_global,
        file_id: *file_id,
        symbol_index,
        total_symbols,
        ciphertext_len: u32::try_from(ciphertext_bytes(k_global))
            .expect("ciphertext_bytes fits u32 for k <= ZION_CODEC_MAX_K"),
    };
    let header_bytes = header.serialize_v1();
    let header_bits = bytes_to_bits(&header_bytes);
    debug_assert_eq!(header_bits.len(), PLAINTEXT_HEADER_CHANNELS);

    let header_positions: Vec<u32> =
        (0..u32::try_from(PLAINTEXT_HEADER_CHANNELS).expect("256 fits u32")).collect();

    // Deterministic LSB ±1 RNG seeded by (master_key, symbol_index).
    let mut lsb_rng = lsb_rng(master_key, symbol_index);
    embed_bits_at(
        &mut host.rgb_data,
        &header_positions,
        &header_bits,
        &mut lsb_rng,
    );

    // 2. Encrypt symbol_bytes with XChaCha20-Poly1305.
    let nonce = derive_nonce(master_key, symbol_index);
    let ciphertext = seal(aead_key, &nonce, symbol_bytes).map_err(EmbedError::AeadFailed)?;
    debug_assert_eq!(ciphertext.len(), ciphertext_bytes(k_global));

    // 3. Permute channels [256..total_channels) with per-symbol seed.
    let total_channels =
        u32::try_from(host.rgb_data.len()).expect("fits u32 for practical image sizes");
    let perm_seed = derive_permutation_seed(master_key, file_id, symbol_index);
    let permutation = permute_range(
        u32::try_from(PLAINTEXT_HEADER_CHANNELS).expect("256 fits u32"),
        total_channels,
        &perm_seed,
    );

    let cipher_bits = bytes_to_bits(&ciphertext);
    assert!(
        cipher_bits.len() <= permutation.len(),
        "ciphertext bits {} > permutable channels {}",
        cipher_bits.len(),
        permutation.len()
    );

    embed_bits_at(
        &mut host.rgb_data,
        &permutation[..cipher_bits.len()],
        &cipher_bits,
        &mut lsb_rng,
    );

    Ok(())
}

/// RNG for ±1 decisions in LSB matching. Deterministic for (`master_key`, `symbol_index`).
fn lsb_rng(master_key: &[u8; 32], symbol_index: u16) -> ChaCha20Rng {
    let mut seed_material = Zeroizing::new([0u8; 32 + 2]);
    seed_material[..32].copy_from_slice(master_key);
    seed_material[32..34].copy_from_slice(&symbol_index.to_le_bytes());
    let seed = blake3::derive_key("zioncode-stego-v1 lsb rng v1", seed_material.as_ref());
    ChaCha20Rng::from_seed(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_host(w: u32, h: u32) -> RgbImage {
        let len = (w as usize) * (h as usize) * 3;
        let data: Vec<u8> = (0..len)
            .map(|i| {
                #[allow(
                    clippy::cast_possible_truncation,
                    reason = "test-only deterministic pattern"
                )]
                let byte = i as u8;
                byte.wrapping_mul(31)
            })
            .collect();
        RgbImage {
            width: w,
            height: h,
            rgb_data: data,
        }
    }

    #[test]
    fn empty_input_rejected() {
        let hosts = vec![make_host(256, 256)];
        let params = EmbedParams {
            passphrase: "x".into(),
            ..Default::default()
        };
        let err = embed_file(&[], hosts, &params).unwrap_err();
        assert!(matches!(err, EmbedError::EmptyInput));
    }

    #[test]
    fn no_host_rejected() {
        let params = EmbedParams {
            passphrase: "x".into(),
            ..Default::default()
        };
        let err = embed_file(b"hello", vec![], &params).unwrap_err();
        assert!(matches!(err, EmbedError::NoHostImages));
    }

    #[test]
    fn invalid_host_shape_rejected() {
        let bad = RgbImage {
            width: 10,
            height: 10,
            rgb_data: vec![0u8; 5], // should be 300
        };
        let params = EmbedParams {
            passphrase: "x".into(),
            ..Default::default()
        };
        let err = embed_file(b"payload", vec![bad], &params).unwrap_err();
        assert!(matches!(err, EmbedError::InvalidHostImage { .. }));
    }

    #[test]
    fn small_file_embeds_into_one_symbol() {
        // 1920×1080 host → K=1006 → 1 symbol carries plenty.
        let host = make_host(1920, 1080);
        let params = EmbedParams {
            passphrase: "light rain".into(),
            ..Default::default()
        };
        let payload = b"bible content here";
        let out = embed_file(payload, vec![host.clone()], &params).unwrap();
        assert_eq!(out.stego_images.len(), 1);
        assert_eq!(out.symbols_used, 1);
        assert_eq!(out.hosts_provided, 1);
        assert_eq!(out.stego_images[0].width, host.width);
        assert_eq!(out.stego_images[0].height, host.height);
    }
}
