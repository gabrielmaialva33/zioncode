//! zion-codec — offline zioncode codec v1.
//!
//! See spec: docs/superpowers/specs/2026-04-23-zioncode-codec-offline-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod api;
mod constants;
mod crc;
mod decode;
mod ecc;
mod encode;
mod error;
mod format;
mod reassemble;
mod types;
mod zstd_layer;

pub use api::{Decoder, DecoderConfig, Encoder, EncoderConfig, SymbolCapture};
pub use constants::{BLOCK_SIZE_RAW, ZSTD_LEVEL_DEFAULT, ZSTD_LEVEL_FAST, ZSTD_LEVEL_MAX};
pub use decode::{DecodedSymbol, SymbolMetadata, decode_symbol, decode_symbol_with_erasures};
pub use ecc::EccProfile;
pub use encode::EncodedFile;
pub use error::{BlockError, DecodeFileError, EncodeError, FileError, SymbolError};
pub use reassemble::{FileReassembler, FinalizedFile, HashStatus};
pub use types::{FileId, GlobalHash, SymbolBytes, SymbolWidth};

/// Expert-facing wire-format types.
pub mod wire {
    pub use crate::format::{BlockEntry, SymbolHeader};
}

/// Expert-facing low-level helpers for benchmarks, fuzzing, and forensics.
pub mod low_level {
    pub use crate::constants::{HEADER_LEN_V1, MAX_K, RS_K, RS_N, RS_PARITY};
    pub use crate::ecc::{
        deinterleave_column_major, interleave_column_major, rs_decode_codeword,
        rs_decode_codeword_with_erasures, rs_decode_codeword_with_profile,
        rs_decode_codeword_with_profile_and_erasures, rs_encode_codeword,
        rs_encode_codeword_with_profile, try_deinterleave_column_major,
        try_rs_encode_codeword_with_profile,
    };
    pub use crate::encode::{
        SymbolPacking, encode_file, encode_file_auto_k, encode_file_with_profile,
        encode_single_symbol, encode_single_symbol_with_profile, pack_blocks_into_symbols,
        pack_blocks_into_symbols_with_profile, split_file_into_blocks, try_encode_single_symbol,
        try_encode_single_symbol_with_profile,
    };
    pub use crate::zstd_layer::decode_block;
}
