//! Stable high-level API for encoding and decoding zion symbols.

use crate::constants::ZSTD_LEVEL_DEFAULT;
use crate::decode::{DecodedSymbol, decode_symbol, decode_symbol_with_erasures};
use crate::ecc::EccProfile;
use crate::encode::{EncodedFile, encode_file_auto_k, encode_file_with_profile};
use crate::error::{DecodeFileError, EncodeError, SymbolError};
use crate::reassemble::FileReassembler;

/// Encoder configuration for the full file-to-symbol pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderConfig {
    /// `None` lets the encoder choose the smallest transmitted output.
    pub k: Option<u16>,
    pub zstd_level: i32,
    pub ecc_profile: EccProfile,
}

impl EncoderConfig {
    #[must_use]
    pub const fn auto() -> Self {
        Self {
            k: None,
            zstd_level: ZSTD_LEVEL_DEFAULT,
            ecc_profile: EccProfile::Safe,
        }
    }

    #[must_use]
    pub const fn fixed_k(k: u16) -> Self {
        Self {
            k: Some(k),
            zstd_level: ZSTD_LEVEL_DEFAULT,
            ecc_profile: EccProfile::Safe,
        }
    }

    #[must_use]
    pub const fn with_profile(mut self, ecc_profile: EccProfile) -> Self {
        self.ecc_profile = ecc_profile;
        self
    }

    #[must_use]
    pub const fn with_zstd_level(mut self, zstd_level: i32) -> Self {
        self.zstd_level = zstd_level;
        self
    }
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self::auto()
    }
}

/// Full file encoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoder {
    config: EncoderConfig,
}

impl Encoder {
    #[must_use]
    pub const fn new(config: EncoderConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> EncoderConfig {
        self.config
    }

    /// Encode one file into transport symbols.
    ///
    /// # Errors
    /// Returns `EncodeError` when the input is empty, too large, impossible to
    /// pack with the selected `K`, or compression fails.
    pub fn encode(&self, raw_file: &[u8]) -> Result<EncodedFile, EncodeError> {
        match self.config.k {
            Some(k) => encode_file_with_profile(
                raw_file,
                k,
                self.config.zstd_level,
                self.config.ecc_profile,
            ),
            None => encode_file_auto_k(raw_file, self.config.zstd_level, self.config.ecc_profile),
        }
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new(EncoderConfig::default())
    }
}

/// Decoder configuration reserved for future capture policies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DecoderConfig;

/// Borrowed symbol capture from a transport or optical layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolCapture<'a> {
    pub bytes: &'a [u8],
    pub erased_positions: &'a [usize],
}

impl<'a> SymbolCapture<'a> {
    #[must_use]
    pub const fn from_bytes(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            erased_positions: &[],
        }
    }

    #[must_use]
    pub const fn with_erasures(bytes: &'a [u8], erased_positions: &'a [usize]) -> Self {
        Self {
            bytes,
            erased_positions,
        }
    }
}

/// Full symbol decoder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Decoder {
    config: DecoderConfig,
}

impl Decoder {
    #[must_use]
    pub const fn new(config: DecoderConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub const fn config(&self) -> DecoderConfig {
        self.config
    }

    /// Decode a symbol without known erasures.
    ///
    /// # Errors
    /// Returns `SymbolError` when size validation, ECC, or header validation
    /// fails.
    pub fn decode_symbol(&self, symbol_bytes: &[u8]) -> Result<DecodedSymbol, SymbolError> {
        decode_symbol(symbol_bytes)
    }

    /// Decode a symbol capture with optional known erasures.
    ///
    /// # Errors
    /// Returns `SymbolError` when size validation, erasure validation, ECC, or
    /// header validation fails.
    pub fn decode_capture(&self, capture: SymbolCapture<'_>) -> Result<DecodedSymbol, SymbolError> {
        if capture.erased_positions.is_empty() {
            decode_symbol(capture.bytes)
        } else {
            decode_symbol_with_erasures(capture.bytes, capture.erased_positions)
        }
    }

    /// Decode and reassemble a complete file from `.zbin` symbols in any order.
    ///
    /// # Errors
    /// Returns `DecodeFileError` when any symbol cannot be decoded or when the
    /// reassembler rejects the symbol set.
    pub fn decode_file<I, B>(&self, symbols: I) -> Result<Vec<u8>, DecodeFileError>
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        let mut reassembler = FileReassembler::new();
        for (symbol_index, symbol) in symbols.into_iter().enumerate() {
            let decoded =
                self.decode_symbol(symbol.as_ref())
                    .map_err(|source| DecodeFileError::Symbol {
                        symbol_index,
                        source,
                    })?;
            reassembler.add_symbol(decoded)?;
        }
        Ok(reassembler.finalize()?)
    }

    /// Decode and reassemble a complete file from captures with optional erasures.
    ///
    /// # Errors
    /// Returns `DecodeFileError` when any capture cannot be decoded or when the
    /// reassembler rejects the symbol set.
    pub fn decode_captures<'a, I>(&self, captures: I) -> Result<Vec<u8>, DecodeFileError>
    where
        I: IntoIterator<Item = SymbolCapture<'a>>,
    {
        let mut reassembler = FileReassembler::new();
        for (symbol_index, capture) in captures.into_iter().enumerate() {
            let decoded =
                self.decode_capture(capture)
                    .map_err(|source| DecodeFileError::Symbol {
                        symbol_index,
                        source,
                    })?;
            reassembler.add_symbol(decoded)?;
        }
        Ok(reassembler.finalize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_level_roundtrip_uses_only_facade_types() {
        let raw = b"architectural boundary test".repeat(64);

        let encoded = Encoder::new(EncoderConfig::fixed_k(38))
            .encode(&raw)
            .unwrap();
        let recovered = Decoder::default().decode_file(&encoded.symbols).unwrap();

        assert_eq!(recovered, raw);
    }

    #[test]
    fn capture_api_recovers_known_erasures() {
        let raw = b"erasure-aware public decoder".repeat(32);
        let encoded = Encoder::new(EncoderConfig::fixed_k(38))
            .encode(&raw)
            .unwrap();
        let mut symbol = encoded.symbols[0].clone();
        let erasures: Vec<usize> = (0..32).map(|row| row * 38).collect();

        for &position in &erasures {
            symbol[position] ^= 0xA5;
        }

        let capture = SymbolCapture::with_erasures(symbol.as_bytes(), &erasures);
        let decoded = Decoder::default().decode_captures([capture]).unwrap();

        assert_eq!(decoded, raw);
    }
}
