//! zion-codec — offline zioncode codec v1.
//!
//! See spec: docs/superpowers/specs/2026-04-23-zioncode-codec-offline-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod api;

pub use api::{Decoder, DecoderConfig, Encoder, EncoderConfig, SymbolCapture};
pub use decode::DecodedSymbol;
pub use ecc::EccProfile;
pub use encode::EncodedFile;
pub use error::{BlockError, DecodeFileError, EncodeError, FileError, SymbolError};
pub use reassemble::{FileReassembler, FinalizedFile, HashStatus};
pub use types::{FileId, GlobalHash, SymbolBytes};

#[doc(hidden)]
pub mod constants;
#[doc(hidden)]
pub mod crc;
#[doc(hidden)]
pub mod decode;
#[doc(hidden)]
pub mod ecc;
#[doc(hidden)]
pub mod encode;
pub mod error;
#[doc(hidden)]
pub mod format;
#[doc(hidden)]
pub mod reassemble;
mod types;
#[doc(hidden)]
pub mod zstd_layer;
