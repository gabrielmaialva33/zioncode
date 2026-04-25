//! Optical header (24 bytes): the on-image preamble that identifies a v1 stream.
//! See spec §4.

use crate::constants::{MAGIC, OPTICAL_HEADER_LEN, VERSION};
use crate::error::ExtractError;

/// Mutable fields of the optical header. Magic, version, and `header_len` are constant in v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpticalHeader {
    pub flags: u16,
    pub k_global: u16,
    pub total_symbols: u16,
    pub payload_len: u32,
}

impl OpticalHeader {
    /// Serialize the v1 header layout into exactly 24 bytes.
    ///
    /// Layout (little-endian, see spec §4):
    ///   `[0..4)`   magic = "ZOPT"
    ///   `[4]`      version = 0x01
    ///   `[5]`      `header_len` = 24
    ///   `[6..8)`   flags
    ///   `[8..10)`  `k_global`
    ///   `[10..12)` `total_symbols`
    ///   `[12..16)` reserved (zero)
    ///   `[16..20)` `payload_len`
    ///   `[20..24)` `header_crc32c` (CRC32C over bytes `[0..20)`)
    #[must_use]
    pub fn serialize_v1(&self) -> [u8; OPTICAL_HEADER_LEN] {
        let mut out = [0u8; OPTICAL_HEADER_LEN];
        out[0..4].copy_from_slice(&MAGIC);
        out[4] = VERSION;
        #[allow(
            clippy::cast_possible_truncation,
            reason = "OPTICAL_HEADER_LEN = 24 fits u8"
        )]
        {
            out[5] = OPTICAL_HEADER_LEN as u8;
        }
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..10].copy_from_slice(&self.k_global.to_le_bytes());
        out[10..12].copy_from_slice(&self.total_symbols.to_le_bytes());
        // [12..16) reserved zeros (already by initialization)
        out[16..20].copy_from_slice(&self.payload_len.to_le_bytes());
        let crc = crc32c::crc32c(&out[0..20]);
        out[20..24].copy_from_slice(&crc.to_le_bytes());
        out
    }

    /// Parse 24+ bytes as an optical header. Validates magic, version, `header_len`, and CRC32C.
    ///
    /// # Errors
    /// - `ImageTooSmall` if `bytes.len() < OPTICAL_HEADER_LEN`.
    /// - `BadMagic` if first 4 bytes are not "ZOPT".
    /// - `UnsupportedVersion` if version != 0x01.
    /// - `InvalidHeaderLen` if `header_len` != 24 in v1.
    /// - `HeaderCrcMismatch` if the stored CRC doesn't match.
    ///
    /// # Panics
    /// Does not panic in practice: every fixed-size slice extraction is guarded by the
    /// length check at the top of this function (`OPTICAL_HEADER_LEN = 24`). The
    /// `expect` calls are unreachable.
    pub fn parse(bytes: &[u8]) -> Result<Self, ExtractError> {
        if bytes.len() < OPTICAL_HEADER_LEN {
            return Err(ExtractError::ImageTooSmall {
                got: bytes.len(),
                min: OPTICAL_HEADER_LEN,
            });
        }
        let magic: [u8; 4] = bytes[0..4].try_into().expect("4 bytes");
        if magic != MAGIC {
            return Err(ExtractError::BadMagic { got: magic });
        }
        let version = bytes[4];
        if version != VERSION {
            return Err(ExtractError::UnsupportedVersion {
                got: version,
                max_supported: VERSION,
            });
        }
        let header_len = bytes[5];
        #[allow(
            clippy::cast_possible_truncation,
            reason = "OPTICAL_HEADER_LEN = 24 fits u8"
        )]
        let expected = OPTICAL_HEADER_LEN as u8;
        if header_len != expected {
            return Err(ExtractError::InvalidHeaderLen { got: header_len });
        }
        let stored_crc = u32::from_le_bytes(bytes[20..24].try_into().expect("4 bytes"));
        let computed_crc = crc32c::crc32c(&bytes[0..20]);
        if stored_crc != computed_crc {
            return Err(ExtractError::HeaderCrcMismatch);
        }
        for (offset, byte) in bytes[12..16].iter().enumerate() {
            if *byte != 0 {
                return Err(ExtractError::PaddingNotZero {
                    offset: 12 + offset,
                });
            }
        }

        let header = Self {
            flags: u16::from_le_bytes(bytes[6..8].try_into().expect("2 bytes")),
            k_global: u16::from_le_bytes(bytes[8..10].try_into().expect("2 bytes")),
            total_symbols: u16::from_le_bytes(bytes[10..12].try_into().expect("2 bytes")),
            payload_len: u32::from_le_bytes(bytes[16..20].try_into().expect("4 bytes")),
        };
        header.validate_payload_len()?;
        Ok(header)
    }

    fn validate_payload_len(&self) -> Result<(), ExtractError> {
        let computed = u64::from(self.k_global)
            * u64::from(self.total_symbols)
            * zion_codec::low_level::RS_N as u64;
        let declared = u64::from(self.payload_len);
        if declared != computed {
            return Err(ExtractError::InconsistentPayloadLength {
                declared: self.payload_len,
                computed,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OpticalHeader {
        OpticalHeader {
            flags: 0,
            k_global: 4112,
            total_symbols: 14,
            payload_len: 14_679_840,
        }
    }

    #[test]
    fn roundtrip() {
        let h = sample();
        let bytes = h.serialize_v1();
        let parsed = OpticalHeader::parse(&bytes).unwrap();
        assert_eq!(parsed, h);
    }

    #[test]
    fn magic_and_version_at_expected_offsets() {
        let bytes = sample().serialize_v1();
        assert_eq!(&bytes[0..4], b"ZOPT");
        assert_eq!(bytes[4], 0x01);
        assert_eq!(bytes[5], 24);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = sample().serialize_v1();
        bytes[0] = 0xFF;
        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::BadMagic { .. })
        ));
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut bytes = sample().serialize_v1();
        bytes[4] = 0x99;
        // Recompute CRC so we isolate version error.
        let crc = crc32c::crc32c(&bytes[0..20]);
        bytes[20..24].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::UnsupportedVersion { got: 0x99, .. })
        ));
    }

    #[test]
    fn invalid_header_len_rejected() {
        let mut bytes = sample().serialize_v1();
        bytes[5] = 16;
        let crc = crc32c::crc32c(&bytes[0..20]);
        bytes[20..24].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::InvalidHeaderLen { got: 16 })
        ));
    }

    #[test]
    fn crc_mismatch_detected() {
        let mut bytes = sample().serialize_v1();
        bytes[8] ^= 0x01; // flip a bit in k_global; CRC no longer matches
        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::HeaderCrcMismatch)
        ));
    }

    #[test]
    fn reserved_bytes_must_be_zero() {
        let mut bytes = sample().serialize_v1();
        bytes[12] = 1;
        let crc = crc32c::crc32c(&bytes[0..20]);
        bytes[20..24].copy_from_slice(&crc.to_le_bytes());

        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::PaddingNotZero { offset: 12 })
        ));
    }

    #[test]
    fn inconsistent_payload_len_rejected() {
        let h = OpticalHeader {
            payload_len: 1,
            ..sample()
        };
        let bytes = h.serialize_v1();

        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::InconsistentPayloadLength {
                declared: 1,
                computed: 14_679_840,
            })
        ));
    }

    #[test]
    fn too_short_rejected() {
        let bytes = vec![0u8; 10];
        assert!(matches!(
            OpticalHeader::parse(&bytes),
            Err(ExtractError::ImageTooSmall { got: 10, min: 24 })
        ));
    }

    #[test]
    fn byte_layout_matches_spec() {
        let h = OpticalHeader {
            flags: 0x1234,
            k_global: 0x5678,
            total_symbols: 0x9ABC,
            payload_len: 0xDEAD_BEEF,
        };
        let b = h.serialize_v1();
        // flags LE at [6..8]
        assert_eq!(b[6], 0x34);
        assert_eq!(b[7], 0x12);
        // k_global LE at [8..10]
        assert_eq!(b[8], 0x78);
        assert_eq!(b[9], 0x56);
        // total_symbols LE at [10..12]
        assert_eq!(b[10], 0xBC);
        assert_eq!(b[11], 0x9A);
        // reserved zeros at [12..16]
        assert_eq!(&b[12..16], &[0, 0, 0, 0]);
        // payload_len LE at [16..20]
        assert_eq!(b[16], 0xEF);
        assert_eq!(b[17], 0xBE);
        assert_eq!(b[18], 0xAD);
        assert_eq!(b[19], 0xDE);
    }
}
