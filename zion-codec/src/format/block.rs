//! Block entry inside a symbol. Fixed v1 layout (7-byte header + payload).
//! See spec section 4.3.

use crate::constants::{BLOCK_HEADER_LEN, BLOCK_SIZE_RAW};
use crate::crc::crc32c;
use crate::error::BlockError;

pub const FLAG_COMPRESSED_BIT: u8 = 0b0000_0001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEntry {
    pub compressed: bool,
    pub payload: Vec<u8>,
}

impl BlockEntry {
    /// Serialize the block in wire format: 1B flags + 2B `payload_size` (LE) + 4B CRC32C + payload.
    ///
    /// # Panics
    /// Panics if `self.payload.len()` does not fit in `u16` (larger than 65535 bytes).
    /// Callers must guarantee the payload is at most `BLOCK_SIZE_RAW` (8192) bytes.
    #[must_use]
    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BLOCK_HEADER_LEN + self.payload.len());
        let flags = if self.compressed {
            FLAG_COMPRESSED_BIT
        } else {
            0
        };
        out.push(flags);
        let size = u16::try_from(self.payload.len()).expect("payload_size fits u16");
        out.extend_from_slice(&size.to_le_bytes());
        let crc = crc32c(&self.payload);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// Parse a block from `bytes` (which must start at the flags byte).
    /// Returns `(entry, bytes_consumed)` where `bytes_consumed` = 7 + `payload_size`.
    ///
    /// # Errors
    /// - `BlockError::UnknownFlag` if any reserved bit is set.
    /// - `BlockError::PayloadSizeTooLarge` if `payload_size > 8192` or the buffer is too short.
    /// - `BlockError::BlockCrcMismatch` if the payload CRC32C does not match.
    ///
    /// # Panics
    /// Does not panic: `bytes[1..3]` and `bytes[3..7]` are protected by the
    /// top-level size check (`bytes.len() < BLOCK_HEADER_LEN`), and `try_into`
    /// converts fixed-size `&[u8]` slices into arrays, so it cannot fail here.
    pub fn parse(bytes: &[u8], block_index: u32) -> Result<(Self, usize), BlockError> {
        if bytes.len() < BLOCK_HEADER_LEN {
            return Err(BlockError::PayloadSizeTooLarge {
                block_index,
                got: 0,
            });
        }

        let flags = bytes[0];
        if flags & !FLAG_COMPRESSED_BIT != 0 {
            return Err(BlockError::UnknownFlag {
                block_index,
                flag_byte: flags,
            });
        }

        let payload_size = u16::from_le_bytes(bytes[1..3].try_into().unwrap());
        if payload_size as usize > BLOCK_SIZE_RAW {
            return Err(BlockError::PayloadSizeTooLarge {
                block_index,
                got: payload_size,
            });
        }

        let stored_crc = u32::from_le_bytes(bytes[3..7].try_into().unwrap());
        let total = BLOCK_HEADER_LEN + payload_size as usize;
        if bytes.len() < total {
            return Err(BlockError::PayloadSizeTooLarge {
                block_index,
                got: payload_size,
            });
        }

        let payload = bytes[BLOCK_HEADER_LEN..total].to_vec();
        let computed_crc = crc32c(&payload);
        if stored_crc != computed_crc {
            return Err(BlockError::BlockCrcMismatch { block_index });
        }

        Ok((
            Self {
                compressed: flags & FLAG_COMPRESSED_BIT != 0,
                payload,
            },
            total,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_raw_block() {
        let payload = vec![0x42u8; 100];
        let entry = BlockEntry {
            compressed: false,
            payload: payload.clone(),
        };
        let bytes = entry.serialize();
        assert_eq!(bytes.len(), BLOCK_HEADER_LEN + 100);
        assert_eq!(bytes[0], 0);
        let (parsed, consumed) = BlockEntry::parse(&bytes, 0).unwrap();
        assert_eq!(consumed, BLOCK_HEADER_LEN + 100);
        assert_eq!(parsed, entry);
    }

    #[test]
    fn roundtrip_compressed_block() {
        let payload = vec![0x11u8; 50];
        let entry = BlockEntry {
            compressed: true,
            payload,
        };
        let bytes = entry.serialize();
        assert_eq!(bytes[0] & FLAG_COMPRESSED_BIT, FLAG_COMPRESSED_BIT);
        let (parsed, _) = BlockEntry::parse(&bytes, 7).unwrap();
        assert_eq!(parsed, entry);
    }

    #[test]
    fn block_crc_mismatch_detected() {
        let entry = BlockEntry {
            compressed: false,
            payload: vec![0xAAu8; 10],
        };
        let mut bytes = entry.serialize();
        bytes[BLOCK_HEADER_LEN + 5] ^= 0xFF; // corrupt payload
        match BlockEntry::parse(&bytes, 42) {
            Err(BlockError::BlockCrcMismatch { block_index }) => {
                assert_eq!(block_index, 42);
            }
            other => panic!("expected BlockCrcMismatch, got {other:?}"),
        }
    }

    #[test]
    fn unknown_flag_rejected() {
        let bytes = [0b1000_0000u8, 0, 0, 0, 0, 0, 0];
        assert!(matches!(
            BlockEntry::parse(&bytes, 0),
            Err(BlockError::UnknownFlag { .. })
        ));
    }

    #[test]
    fn payload_too_large_rejected() {
        let mut bytes = vec![0u8; 7];
        bytes[1..3].copy_from_slice(&9000u16.to_le_bytes());
        assert!(matches!(
            BlockEntry::parse(&bytes, 0),
            Err(BlockError::PayloadSizeTooLarge { .. })
        ));
    }
}
