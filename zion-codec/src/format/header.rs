//! Symbol header. Fixed v1 layout (82 bytes).
//! See spec sections 4.2 and 4.5.

use crate::constants::{HEADER_LEN_V1, MAGIC, VERSION};
use crate::crc::crc32c;
use crate::ecc::EccProfile;
use crate::error::SymbolError;
use crate::types::{FileId, GlobalHash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolHeader {
    header_len: u8,
    flags: u16,
    file_id: [u8; 16],
    file_size: u64,
    total_blocks: u32,
    total_symbols: u16,
    symbol_index: u16,
    block_start: u32,
    block_count: u16,
    global_hash: [u8; 32],
}

impl SymbolHeader {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "v1 header layout is fixed by the wire format fields"
    )]
    pub fn new_v1(
        ecc_profile: EccProfile,
        file_id: FileId,
        file_size: u64,
        total_blocks: u32,
        total_symbols: u16,
        symbol_index: u16,
        block_start: u32,
        block_count: u16,
        global_hash: GlobalHash,
    ) -> Self {
        Self {
            #[allow(
                clippy::cast_possible_truncation,
                reason = "HEADER_LEN_V1 is a fixed v1 constant = 82"
            )]
            header_len: HEADER_LEN_V1 as u8,
            flags: ecc_profile.header_flags(),
            file_id: file_id.to_bytes(),
            file_size,
            total_blocks,
            total_symbols,
            symbol_index,
            block_start,
            block_count,
            global_hash: global_hash.to_bytes(),
        }
    }

    #[must_use]
    pub const fn header_len(&self) -> u8 {
        self.header_len
    }

    #[must_use]
    pub fn ecc_profile(&self) -> Option<EccProfile> {
        EccProfile::from_header_flags(self.flags)
    }

    #[must_use]
    pub const fn flags(&self) -> u16 {
        self.flags
    }

    #[must_use]
    pub const fn file_id(&self) -> FileId {
        FileId::from_bytes(self.file_id)
    }

    #[must_use]
    pub const fn file_size(&self) -> u64 {
        self.file_size
    }

    #[must_use]
    pub const fn total_blocks(&self) -> u32 {
        self.total_blocks
    }

    #[must_use]
    pub const fn total_symbols(&self) -> u16 {
        self.total_symbols
    }

    #[must_use]
    pub const fn symbol_index(&self) -> u16 {
        self.symbol_index
    }

    #[must_use]
    pub const fn block_start(&self) -> u32 {
        self.block_start
    }

    #[must_use]
    pub const fn block_count(&self) -> u16 {
        self.block_count
    }

    #[must_use]
    pub const fn global_hash(&self) -> GlobalHash {
        GlobalHash::from_bytes(self.global_hash)
    }

    /// Serialize to exactly 82 bytes (v1).
    ///
    /// # Panics
    /// In debug builds, panics if `self.header_len != HEADER_LEN_V1`.
    /// An inconsistent struct state is a caller bug.
    #[must_use]
    pub fn serialize_v1(&self) -> [u8; HEADER_LEN_V1] {
        self.try_serialize_v1()
            .expect("caller must provide a valid v1 header")
    }

    /// Fallible variant of [`serialize_v1`](Self::serialize_v1).
    ///
    /// # Errors
    /// Returns `SymbolError::HeaderLengthInvalid` if the header does not carry
    /// the expected v1 length.
    pub fn try_serialize_v1(&self) -> Result<[u8; HEADER_LEN_V1], SymbolError> {
        if self.header_len as usize != HEADER_LEN_V1 {
            return Err(SymbolError::HeaderLengthInvalid {
                got: self.header_len,
            });
        }
        let mut out = [0u8; HEADER_LEN_V1];

        out[0..4].copy_from_slice(&MAGIC);
        out[4] = VERSION;
        out[5] = self.header_len;
        out[6..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..24].copy_from_slice(&self.file_id);
        out[24..32].copy_from_slice(&self.file_size.to_le_bytes());
        out[32..36].copy_from_slice(&self.total_blocks.to_le_bytes());
        out[36..38].copy_from_slice(&self.total_symbols.to_le_bytes());
        out[38..40].copy_from_slice(&self.symbol_index.to_le_bytes());
        out[40..44].copy_from_slice(&self.block_start.to_le_bytes());
        out[44..46].copy_from_slice(&self.block_count.to_le_bytes());
        out[46..78].copy_from_slice(&self.global_hash);

        let crc = crc32c(&out[0..78]);
        out[78..82].copy_from_slice(&crc.to_le_bytes());

        Ok(out)
    }

    /// Parse the initial bytes as a header. Consumes `header_len` total bytes.
    /// Compatible with `header_len > 82` by skipping extra bytes after CRC validation.
    ///
    /// # Errors
    /// Returns `SymbolError::BadMagic`, `UnsupportedVersion`,
    /// `HeaderLengthInvalid`, or `HeaderCrcMismatch` depending on the failure.
    ///
    /// # Unvalidated Invariants
    /// This method only validates the wire format (magic, version, `header_len`,
    /// CRC32C). Semantic invariants from spec §4.4 such as `block_count >= 1`,
    /// `symbol_index < total_symbols`, `block_start + block_count <= total_blocks`,
    /// and `total_blocks == ceil(file_size / 8192)` are enforced by the upper
    /// layer (`FileReassembler`), which returns the appropriate `FileError`.
    ///
    /// # Panics
    /// Does not panic because every `try_into().unwrap()` operates on slices
    /// whose size has already been validated.
    pub fn parse(bytes: &[u8]) -> Result<(Self, usize), SymbolError> {
        if bytes.len() < HEADER_LEN_V1 {
            return Err(SymbolError::HeaderLengthInvalid {
                got: u8::try_from(bytes.len()).unwrap_or(u8::MAX),
            });
        }

        let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
        if magic != MAGIC {
            return Err(SymbolError::BadMagic { got: magic });
        }

        let version = bytes[4];
        if version != VERSION {
            return Err(SymbolError::UnsupportedVersion {
                got: version,
                max_supported: VERSION,
            });
        }

        let header_len = bytes[5];
        if (header_len as usize) < HEADER_LEN_V1 {
            return Err(SymbolError::HeaderLengthInvalid { got: header_len });
        }
        if bytes.len() < header_len as usize {
            return Err(SymbolError::HeaderLengthInvalid { got: header_len });
        }

        // CRC32C over header[0 .. header_len-4]
        let crc_end = header_len as usize - 4;
        let stored_crc = u32::from_le_bytes(bytes[crc_end..crc_end + 4].try_into().unwrap());
        let computed_crc = crc32c(&bytes[0..crc_end]);
        if stored_crc != computed_crc {
            return Err(SymbolError::HeaderCrcMismatch);
        }

        let flags = u16::from_le_bytes(bytes[6..8].try_into().unwrap());
        if EccProfile::from_header_flags(flags).is_none() {
            return Err(SymbolError::UnsupportedHeaderFlags { flags });
        }
        let file_id: [u8; 16] = bytes[8..24].try_into().unwrap();
        let file_size = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
        let total_blocks = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        let total_symbols = u16::from_le_bytes(bytes[36..38].try_into().unwrap());
        let symbol_index = u16::from_le_bytes(bytes[38..40].try_into().unwrap());
        let block_start = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
        let block_count = u16::from_le_bytes(bytes[44..46].try_into().unwrap());
        let global_hash: [u8; 32] = bytes[46..78].try_into().unwrap();

        Ok((
            Self {
                header_len,
                flags,
                file_id,
                file_size,
                total_blocks,
                total_symbols,
                symbol_index,
                block_start,
                block_count,
                global_hash,
            },
            header_len as usize,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_header() -> SymbolHeader {
        SymbolHeader::new_v1(
            EccProfile::Safe,
            FileId::from_bytes([0xaa; 16]),
            32768,
            4,
            1,
            0,
            0,
            4,
            GlobalHash::from_bytes([0xbb; 32]),
        )
    }

    #[test]
    fn roundtrip_serialize_parse() {
        let h = sample_header();
        let bytes = h.serialize_v1();
        let (parsed, consumed) = SymbolHeader::parse(&bytes).unwrap();
        assert_eq!(parsed, h);
        assert_eq!(consumed, HEADER_LEN_V1);
    }

    #[test]
    fn magic_and_version_at_expected_offsets() {
        let h = sample_header();
        let bytes = h.serialize_v1();
        assert_eq!(&bytes[0..4], b"ZION");
        assert_eq!(bytes[4], 0x01);
        assert_eq!(bytes[5], 82);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[0] = 0xFF;
        match SymbolHeader::parse(&bytes) {
            Err(SymbolError::BadMagic { got }) => assert_eq!(got[0], 0xFF),
            other => panic!("expected BadMagic, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_version_rejected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[4] = 0x99;
        // Recompute CRC to isolate the version error
        let crc = crc32c(&bytes[0..78]);
        bytes[78..82].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            SymbolHeader::parse(&bytes),
            Err(SymbolError::UnsupportedVersion { got: 0x99, .. })
        ));
    }

    #[test]
    fn version_zero_is_rejected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[4] = 0x00;
        let crc = crc32c(&bytes[0..78]);
        bytes[78..82].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            SymbolHeader::parse(&bytes),
            Err(SymbolError::UnsupportedVersion { got: 0, .. })
        ));
    }

    #[test]
    fn supported_ecc_profile_flags_are_accepted() {
        let mut bytes = sample_header().serialize_v1();
        bytes[6..8].copy_from_slice(&1u16.to_le_bytes());
        let crc = crc32c(&bytes[0..78]);
        bytes[78..82].copy_from_slice(&crc.to_le_bytes());
        let (parsed, _) = SymbolHeader::parse(&bytes).unwrap();
        assert_eq!(parsed.flags(), 1);
    }

    #[test]
    fn reserved_header_flags_are_rejected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[6..8].copy_from_slice(&3u16.to_le_bytes());
        let crc = crc32c(&bytes[0..78]);
        bytes[78..82].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            SymbolHeader::parse(&bytes),
            Err(SymbolError::UnsupportedHeaderFlags { flags: 3 })
        ));
    }

    #[test]
    fn header_crc_mismatch_detected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[10] ^= 0x01;
        assert!(matches!(
            SymbolHeader::parse(&bytes),
            Err(SymbolError::HeaderCrcMismatch)
        ));
    }

    #[test]
    fn header_len_too_small_rejected() {
        let mut bytes = sample_header().serialize_v1();
        bytes[5] = 50;
        let crc = crc32c(&bytes[0..46]);
        bytes[46..50].copy_from_slice(&crc.to_le_bytes());
        assert!(matches!(
            SymbolHeader::parse(&bytes),
            Err(SymbolError::HeaderLengthInvalid { .. })
        ));
    }

    /// Forward compatibility: `header_len` = 90 (future version with 8 extra bytes before the CRC).
    #[test]
    fn forward_compat_larger_header() {
        let mut bytes = vec![0u8; 90];
        let base = sample_header();
        let v1_bytes = base.serialize_v1();
        bytes[0..82].copy_from_slice(&v1_bytes);
        bytes[5] = 90;
        // bytes 82..86 stay zero (future fields)
        let crc = crc32c(&bytes[0..86]);
        bytes[86..90].copy_from_slice(&crc.to_le_bytes());

        let (parsed, consumed) = SymbolHeader::parse(&bytes).unwrap();
        assert_eq!(consumed, 90);
        assert_eq!(parsed.header_len(), 90);
        assert_eq!(parsed.file_size(), base.file_size());
    }
}
