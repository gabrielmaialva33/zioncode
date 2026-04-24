//! zion-stego — LSB matching steganography + AEAD for zioncode v1.
//!
//! See spec: docs/superpowers/specs/2026-04-23-zioncode-stego-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod aead;
mod capacity;
mod constants;
mod embed;
mod error;
mod extract;
mod kdf;
mod lsb;
mod permutation;
mod plaintext_header;
mod png_io;
mod types;

pub use constants::EMBEDDING_DENSITY_DEFAULT;
pub use embed::{EmbedOutput, EmbedParams, embed_file};
pub use error::{EmbedError, ExtractError};
pub use extract::{ExtractOutput, StegoMetadata, extract_file, inspect_stego_image};
pub use png_io::{RgbImage, load_png_rgb, save_png_rgb};
pub use types::EmbeddingDensity;

/// Expert-facing plaintext wire header for fuzzing and diagnostics.
pub mod wire {
    pub use crate::constants::{PLAINTEXT_HEADER_CHANNELS, PLAINTEXT_HEADER_LEN};
    pub use crate::plaintext_header::PlaintextHeader;
}

/// Expert-facing low-level steganography primitives.
pub mod low_level {
    pub use crate::aead::{ciphertext_len, open, seal};
    pub use crate::capacity::{
        ciphertext_bytes, compute_k_global, photo_k_per_photo, symbol_payload_bytes,
    };
    pub use crate::constants::{
        AEAD_KEY_LEN, AEAD_NONCE_LEN, AEAD_TAG_LEN, ARGON2ID_ITERATIONS, ARGON2ID_MEMORY_KIB,
        ARGON2ID_OUTPUT_LEN, ARGON2ID_PARALLELISM, ARGON2ID_SALT_PREFIX, KDF_AEAD_CONTEXT,
        KDF_NONCE_CONTEXT, KDF_PERMUTATION_CONTEXT, MAGIC, VERSION, ZION_CODEC_MAX_K,
    };
    pub use crate::kdf::{
        derive_aead_key, derive_master_key, derive_nonce, derive_permutation_seed, derive_salt,
    };
    pub use crate::lsb::{
        apply_lsb_matching, bits_to_bytes, bytes_to_bits, embed_bits_at, extract_bits_at,
        extract_lsb,
    };
    pub use crate::permutation::permute_range;
}
