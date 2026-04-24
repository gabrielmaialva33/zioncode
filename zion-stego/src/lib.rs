//! zion-stego — esteganografia LSB matching + AEAD para zioncode v1.
//!
//! Ver spec: docs/superpowers/specs/2026-04-23-zioncode-stego-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod constants;
pub mod error;
pub mod kdf;
pub mod png_io;
