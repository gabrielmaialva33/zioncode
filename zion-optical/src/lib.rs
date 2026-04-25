//! zion-optical — visual-digital lossless container for zioncode v1.
//!
//! See spec: docs/superpowers/specs/2026-04-24-zioncode-optical-design.md

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod constants;
mod error;
mod optical_header;
mod pixel_io;

pub use constants::{BYTES_PER_PIXEL, K_DEFAULT, MAX_PAYLOAD_BYTES_V1, ZSTD_LEVEL_DEFAULT};
pub use error::{ExtractError, PngImageError, RenderError};
pub use optical_header::OpticalHeader;
pub use pixel_io::{RgbImage, decode_png, encode_png, read_png, write_png};

/// Expert-facing wire constants and header codec.
pub mod wire {
    pub use crate::constants::{MAGIC, OPTICAL_HEADER_LEN, PNG_BIT_DEPTH, VERSION};
    pub use crate::optical_header::OpticalHeader;
}
