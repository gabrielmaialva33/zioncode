//! zion-stego — LSB matching steganography + AEAD for zioncode v1.
//!
//! See spec: docs/superpowers/specs/2026-04-23-zioncode-stego-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod aead;
pub mod capacity;
pub mod constants;
pub mod embed;
pub mod error;
pub mod kdf;
pub mod lsb;
pub mod permutation;
pub mod plaintext_header;
pub mod png_io;
