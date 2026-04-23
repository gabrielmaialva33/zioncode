//! zion-codec — codec offline do zioncode v1.
//!
//! Ver spec: docs/superpowers/specs/2026-04-23-zioncode-codec-offline-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod constants;
pub mod crc;
pub mod error;
pub mod format;
