//! Plaintext header (32 bytes = 256 bits) at the start of a stego image.
//! Lives in the fixed row-major region (channels `[0..256)`) so the decoder
//! can read it without the passphrase. See spec §4 "Plaintext header".

use crate::constants::{MAGIC, PLAINTEXT_HEADER_LEN, VERSION};
use crate::error::ExtractError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaintextHeader {
    pub k_global: u16,
    pub file_id: [u8; 16],
    pub symbol_index: u16,
    pub total_symbols: u16,
    pub ciphertext_len: u32,
}

impl PlaintextHeader {
    /// Serializes the header into exactly 32 bytes (v1 layout).
    #[must_use]
    pub fn serialize_v1(&self) -> [u8; PLAINTEXT_HEADER_LEN] {
        let mut out = [0u8; PLAINTEXT_HEADER_LEN];
        out[0..4].copy_from_slice(&MAGIC);
        out[4] = VERSION;
        out[5] = 0; // reserved
        out[6..8].copy_from_slice(&self.k_global.to_le_bytes());
        out[8..24].copy_from_slice(&self.file_id);
        out[24..26].copy_from_slice(&self.symbol_index.to_le_bytes());
        out[26..28].copy_from_slice(&self.total_symbols.to_le_bytes());
        out[28..32].copy_from_slice(&self.ciphertext_len.to_le_bytes());
        out
    }

    /// Parses 32 bytes. Validates magic and version.
    ///
    /// # Errors
    /// - [`ExtractError::BadMagic`] if the first 4 bytes are not `"ZSTG"`.
    /// - [`ExtractError::UnsupportedVersion`] if version > [`VERSION`].
    ///
    /// # Panics
    /// Never panics in practice: the input is a fixed-size array of
    /// [`PLAINTEXT_HEADER_LEN`] bytes, so every slice-to-array conversion
    /// is statically guaranteed to succeed.
    pub fn parse(bytes: &[u8; PLAINTEXT_HEADER_LEN], index: usize) -> Result<Self, ExtractError> {
        let magic: [u8; 4] = bytes[0..4].try_into().expect("4 bytes");
        if magic != MAGIC {
            return Err(ExtractError::BadMagic { index, got: magic });
        }
        let version = bytes[4];
        if version > VERSION {
            return Err(ExtractError::UnsupportedVersion {
                index,
                got: version,
                max_supported: VERSION,
            });
        }
        // byte 5 reserved, ignored
        let k_global = u16::from_le_bytes(bytes[6..8].try_into().expect("2 bytes"));
        let file_id: [u8; 16] = bytes[8..24].try_into().expect("16 bytes");
        let symbol_index = u16::from_le_bytes(bytes[24..26].try_into().expect("2 bytes"));
        let total_symbols = u16::from_le_bytes(bytes[26..28].try_into().expect("2 bytes"));
        let ciphertext_len = u32::from_le_bytes(bytes[28..32].try_into().expect("4 bytes"));

        Ok(Self {
            k_global,
            file_id,
            symbol_index,
            total_symbols,
            ciphertext_len,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PlaintextHeader {
        PlaintextHeader {
            k_global: 1006,
            file_id: [0xAB; 16],
            symbol_index: 3,
            total_symbols: 6,
            ciphertext_len: 256_546,
        }
    }

    #[test]
    fn roundtrip() {
        let h = sample();
        let bytes = h.serialize_v1();
        let parsed = PlaintextHeader::parse(&bytes, 0).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn magic_and_version_at_expected_offsets() {
        let h = sample();
        let bytes = h.serialize_v1();
        assert_eq!(&bytes[0..4], b"ZSTG");
        assert_eq!(bytes[4], 0x01);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = sample().serialize_v1();
        bytes[0] = 0xFF;
        match PlaintextHeader::parse(&bytes, 7) {
            Err(ExtractError::BadMagic { index, got }) => {
                assert_eq!(index, 7);
                assert_eq!(got[0], 0xFF);
            }
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut bytes = sample().serialize_v1();
        bytes[4] = 0x99;
        assert!(matches!(
            PlaintextHeader::parse(&bytes, 0),
            Err(ExtractError::UnsupportedVersion {
                got: 0x99,
                max_supported: 0x01,
                ..
            })
        ));
    }

    #[test]
    fn byte_layout_matches_spec() {
        // Specific field offsets per spec §4.
        let h = PlaintextHeader {
            k_global: 0x1234,
            file_id: [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10,
            ],
            symbol_index: 0x5678,
            total_symbols: 0x9ABC,
            ciphertext_len: 0xDEAD_BEEF,
        };
        let bytes = h.serialize_v1();
        // k_global LE at [6..8]
        assert_eq!(bytes[6], 0x34);
        assert_eq!(bytes[7], 0x12);
        // file_id at [8..24]
        assert_eq!(bytes[8], 0x01);
        assert_eq!(bytes[23], 0x10);
        // symbol_index LE at [24..26]
        assert_eq!(bytes[24], 0x78);
        assert_eq!(bytes[25], 0x56);
        // total_symbols LE at [26..28]
        assert_eq!(bytes[26], 0xBC);
        assert_eq!(bytes[27], 0x9A);
        // ciphertext_len LE at [28..32]
        assert_eq!(bytes[28], 0xEF);
        assert_eq!(bytes[29], 0xBE);
        assert_eq!(bytes[30], 0xAD);
        assert_eq!(bytes[31], 0xDE);
    }
}
