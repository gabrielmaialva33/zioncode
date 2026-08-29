use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::constants::{
    BOOTSTRAP_CHANNELS, DENSITY_DENOMINATOR, DENSITY_NUMERATOR, MAX_CODEWORDS, MAX_DIMENSION,
    MAX_PERMUTATION_INDICES, MAX_PIXELS, MAX_RASTER_BYTES, RS_N,
};
use crate::error::SealError;

/// Reed-Solomon protection profile used by ARC v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EccProfile {
    /// RS(255,223), correcting up to 16 unknown bytes per codeword.
    #[default]
    Safe,
    /// RS(255,239), correcting up to 8 unknown bytes per codeword.
    Balanced,
    /// RS(255,247), correcting up to 4 unknown bytes per codeword.
    Dense,
}

impl EccProfile {
    #[must_use]
    pub const fn data_len(self) -> usize {
        match self {
            Self::Safe => 223,
            Self::Balanced => 239,
            Self::Dense => 247,
        }
    }

    #[must_use]
    pub const fn parity_len(self) -> usize {
        RS_N - self.data_len()
    }

    #[must_use]
    pub const fn correction_budget(self) -> usize {
        self.parity_len() / 2
    }

    pub(crate) const fn wire_code(self) -> u8 {
        match self {
            Self::Safe => 0,
            Self::Balanced => 1,
            Self::Dense => 2,
        }
    }

    pub(crate) const fn from_wire_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Safe),
            1 => Some(Self::Balanced),
            2 => Some(Self::Dense),
            _ => None,
        }
    }

    pub(crate) const fn codec_profile(self) -> zion_codec::EccProfile {
        match self {
            Self::Safe => zion_codec::EccProfile::Safe,
            Self::Balanced => zion_codec::EccProfile::Balanced,
            Self::Dense => zion_codec::EccProfile::Dense,
        }
    }
}

/// Metadata encrypted inside one ARC capsule.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct ItemMetadata {
    pub name: String,
    pub media_type: String,
    pub attribution: String,
}

impl ItemMetadata {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        media_type: impl Into<String>,
        attribution: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            media_type: media_type.into(),
            attribution: attribution.into(),
        }
    }
}

/// Configuration for one sealing operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct SealConfig {
    pub profile: EccProfile,
}

impl SealConfig {
    /// Creates the default ARC v1 sealing configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            profile: EccProfile::Safe,
        }
    }

    /// Selects the Reed-Solomon protection profile.
    #[must_use]
    pub const fn with_profile(profile: EccProfile) -> Self {
        Self { profile }
    }
}

/// Exact carrier capacity under ARC v1's 33-percent payload ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Capacity {
    width: u32,
    height: u32,
    channels: usize,
    candidate_bits: usize,
    usable_bits: usize,
    max_codewords: usize,
}

impl Capacity {
    pub(crate) fn checked(width: u32, height: u32) -> Result<Self, SealError> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(SealError::InvalidDimensions);
        }
        let pixels = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .ok_or(SealError::ArithmeticOverflow)?;
        if pixels > MAX_PIXELS {
            return Err(SealError::InvalidDimensions);
        }
        let channels = pixels.checked_mul(3).ok_or(SealError::ArithmeticOverflow)?;
        if channels > MAX_RASTER_BYTES {
            return Err(SealError::InvalidDimensions);
        }
        let candidate_bits = channels
            .checked_sub(BOOTSTRAP_CHANNELS)
            .ok_or(SealError::CarrierTooSmall)?;
        if candidate_bits > MAX_PERMUTATION_INDICES {
            return Err(SealError::InvalidDimensions);
        }
        let usable_bits = candidate_bits
            .checked_mul(DENSITY_NUMERATOR)
            .ok_or(SealError::ArithmeticOverflow)?
            / DENSITY_DENOMINATOR;
        let bits_per_codeword = RS_N.checked_mul(8).ok_or(SealError::ArithmeticOverflow)?;
        let max_codewords = (usable_bits / bits_per_codeword).min(MAX_CODEWORDS);
        if max_codewords == 0 {
            return Err(SealError::CarrierTooSmall);
        }
        Ok(Self {
            width,
            height,
            channels,
            candidate_bits,
            usable_bits,
            max_codewords,
        })
    }

    /// Carrier width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Carrier height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// Exact number of RGB channel bytes in the decoded raster.
    #[must_use]
    pub const fn channels(self) -> usize {
        self.channels
    }

    /// Channel positions eligible for payload placement after the bootstrap.
    #[must_use]
    pub const fn candidate_bits(self) -> usize {
        self.candidate_bits
    }

    /// Payload bits allowed by ARC v1's 33-percent density ceiling.
    #[must_use]
    pub const fn usable_bits(self) -> usize {
        self.usable_bits
    }

    /// Maximum whole RS(255,k) codewords that fit in this carrier.
    #[must_use]
    pub const fn max_codewords(self) -> usize {
        self.max_codewords
    }

    #[must_use]
    pub const fn max_ciphertext_bytes(self, profile: EccProfile) -> usize {
        self.max_codewords * profile.data_len()
    }

    /// Maximum authenticated-encryption plaintext length for this carrier.
    ///
    /// This is the encrypted envelope size, not the maximum original content
    /// size: its fixed header, metadata, compressed payload, and authenticated
    /// alignment padding all consume bytes within this limit.
    #[must_use]
    pub const fn max_aead_plaintext_bytes(self, profile: EccProfile) -> usize {
        self.max_ciphertext_bytes(profile)
            .saturating_sub(crate::constants::AEAD_TAG_LEN)
    }
}

/// Result of a successful sealing operation.
#[derive(Clone, PartialEq, Eq)]
pub struct SealOutput {
    pub png_bytes: Vec<u8>,
    pub collection_id: [u8; 16],
    pub capsule_id: [u8; 16],
    pub profile: EccProfile,
    pub capacity: Capacity,
    pub ciphertext_len: usize,
    pub codeword_count: usize,
}

/// Recoverable payload-corruption artifact intended for demonstrations and
/// decoder acceptance tests.
///
/// The mutation changes one payload bit in the maximum correctable number of
/// distinct bytes in every Reed-Solomon codeword. It does not change the
/// public bootstrap or make arbitrary image transforms recoverable.
#[derive(Clone, PartialEq, Eq)]
pub struct CorruptionFixtureOutput {
    pub png_bytes: Vec<u8>,
    pub profile: EccProfile,
    pub codeword_count: usize,
    pub corrupted_bytes_per_codeword: usize,
    pub total_corrupted_payload_bytes: usize,
    pub changed_channels: usize,
}

impl fmt::Debug for CorruptionFixtureOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CorruptionFixtureOutput")
            .field("png_bytes", &"[REDACTED]")
            .field("profile", &self.profile)
            .field("codeword_count", &self.codeword_count)
            .field(
                "corrupted_bytes_per_codeword",
                &self.corrupted_bytes_per_codeword,
            )
            .field(
                "total_corrupted_payload_bytes",
                &self.total_corrupted_payload_bytes,
            )
            .field("changed_channels", &self.changed_channels)
            .finish()
    }
}

impl fmt::Debug for SealOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealOutput")
            .field("png_bytes", &"[REDACTED]")
            .field("collection_id", &self.collection_id)
            .field("capsule_id", &self.capsule_id)
            .field("profile", &self.profile)
            .field("capacity", &self.capacity)
            .field("ciphertext_len", &self.ciphertext_len)
            .field("codeword_count", &self.codeword_count)
            .finish()
    }
}

/// Result of a fully authenticated and verified opening operation.
#[derive(Clone, PartialEq, Eq)]
pub struct OpenOutput {
    pub content: Vec<u8>,
    pub metadata: ItemMetadata,
    pub collection_id: [u8; 16],
    pub capsule_id: [u8; 16],
    pub profile: EccProfile,
}

impl fmt::Debug for OpenOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenOutput")
            .field("content", &"[REDACTED]")
            .field("metadata", &"[REDACTED]")
            .field("collection_id", &self.collection_id)
            .field("capsule_id", &self.capsule_id)
            .field("profile", &self.profile)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_debug_implementations_redact_all_payload_values() {
        let png_sentinel = b"ARC_PNG_SENTINEL_7c932a".to_vec();
        let content_sentinel = b"ARC_CONTENT_SENTINEL_e184bf".to_vec();
        let metadata = ItemMetadata::new(
            "ARC_NAME_SENTINEL_a91b",
            "ARC_MEDIA_SENTINEL_b82c",
            "ARC_ATTRIBUTION_SENTINEL_c73d",
        );
        let seal = SealOutput {
            png_bytes: png_sentinel,
            collection_id: [1; 16],
            capsule_id: [2; 16],
            profile: EccProfile::Safe,
            capacity: Capacity::checked(64, 64).unwrap(),
            ciphertext_len: 223,
            codeword_count: 1,
        };
        let open = OpenOutput {
            content: content_sentinel,
            metadata,
            collection_id: [1; 16],
            capsule_id: [2; 16],
            profile: EccProfile::Safe,
        };
        let corruption = CorruptionFixtureOutput {
            png_bytes: b"ARC_CORRUPTION_PNG_SENTINEL_b9f2".to_vec(),
            profile: EccProfile::Safe,
            codeword_count: 1,
            corrupted_bytes_per_codeword: 16,
            total_corrupted_payload_bytes: 16,
            changed_channels: 16,
        };

        let seal_debug = format!("{seal:?}");
        assert!(seal_debug.contains("png_bytes: \"[REDACTED]\""));
        assert!(!seal_debug.contains("ARC_PNG_SENTINEL"));
        assert!(!seal_debug.contains("65, 82, 67, 95, 80, 78, 71"));

        let open_debug = format!("{open:?}");
        assert!(open_debug.contains("content: \"[REDACTED]\""));
        assert!(open_debug.contains("metadata: \"[REDACTED]\""));
        for sentinel in [
            "ARC_CONTENT_SENTINEL",
            "ARC_NAME_SENTINEL",
            "ARC_MEDIA_SENTINEL",
            "ARC_ATTRIBUTION_SENTINEL",
            "65, 82, 67, 95, 67, 79, 78, 84, 69, 78, 84",
        ] {
            assert!(!open_debug.contains(sentinel));
        }

        let corruption_debug = format!("{corruption:?}");
        assert!(corruption_debug.contains("png_bytes: \"[REDACTED]\""));
        assert!(!corruption_debug.contains("ARC_CORRUPTION_PNG_SENTINEL"));
    }

    #[test]
    fn item_metadata_zeroizes_all_string_buffers() {
        let mut metadata = ItemMetadata::new(
            "ARC_NAME_ZEROIZE_SENTINEL",
            "ARC_MEDIA_ZEROIZE_SENTINEL",
            "ARC_ATTRIBUTION_ZEROIZE_SENTINEL",
        );

        metadata.zeroize();

        assert!(metadata.name.is_empty());
        assert!(metadata.media_type.is_empty());
        assert!(metadata.attribution.is_empty());
    }
}
