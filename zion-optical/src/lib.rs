//! zion-optical — visual-digital lossless container for zioncode v1.
//!
//! See spec: docs/superpowers/specs/2026-04-24-zioncode-optical-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod constants;
mod error;

pub use constants::{BYTES_PER_PIXEL, K_DEFAULT, MAX_PAYLOAD_BYTES_V1, ZSTD_LEVEL_DEFAULT};
pub use error::{ExtractError, PngImageError, RenderError};

/// Expert-facing wire constants.
pub mod wire {
    pub use crate::constants::{MAGIC, OPTICAL_HEADER_LEN, PNG_BIT_DEPTH, VERSION};
}
