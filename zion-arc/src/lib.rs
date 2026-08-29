//! Zion ARC v1 exact-PNG cryptographic transport core.

#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod api;
mod bootstrap;
mod carrier;
mod compression;
mod constants;
mod crypto;
mod ecc;
mod envelope;
mod error;
mod kdf;
mod types;
#[allow(unsafe_code)]
mod zstd_ffi;

pub use api::{
    capacity, make_bounded_corruption_fixture, open_png_bytes,
    open_png_bytes_with_collection_key, seal_png_bytes, seal_png_bytes_with_collection_key,
};
pub use error::{CollectionKeyError, OpenError, SealError};
pub use kdf::CollectionKey;
pub use types::{
    Capacity, CorruptionFixtureOutput, EccProfile, ItemMetadata, OpenOutput, SealConfig, SealOutput,
};

#[cfg(test)]
mod vector_tests;
