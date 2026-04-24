//! Domain types used by the public stego API.

use crate::constants::EMBEDDING_DENSITY_DEFAULT;
use crate::error::EmbedError;

const DENSITY_SCALE_F32: f32 = 100_000.0;

/// Fraction of permutable RGB channels used for embedding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EmbeddingDensity {
    scaled_parts: u32,
}

impl EmbeddingDensity {
    /// Validate and create an embedding density.
    ///
    /// # Errors
    /// Returns `EmbedError::InvalidDensity` if the value is not finite, is zero
    /// or negative, is greater than 1.0, or is too small to survive integer
    /// scaling.
    pub fn new(value: f32) -> Result<Self, EmbedError> {
        if !value.is_finite() || value <= 0.0 || value > 1.0 {
            return Err(EmbedError::InvalidDensity { got: value });
        }

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "value is finite and validated in (0.0, 1.0]"
        )]
        let scaled_parts = (value * DENSITY_SCALE_F32).floor() as u32;
        if scaled_parts == 0 {
            return Err(EmbedError::InvalidDensity { got: value });
        }

        Ok(Self { scaled_parts })
    }

    #[must_use]
    pub const fn scaled_parts_per_100k(self) -> u32 {
        self.scaled_parts
    }

    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "scaled_parts is <= 100_000, which f32 represents exactly"
    )]
    pub fn as_f32(self) -> f32 {
        self.scaled_parts as f32 / DENSITY_SCALE_F32
    }
}

impl Default for EmbeddingDensity {
    fn default() -> Self {
        Self::new(EMBEDDING_DENSITY_DEFAULT).expect("EMBEDDING_DENSITY_DEFAULT is a valid density")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_constant() {
        let density = EmbeddingDensity::default();
        assert!((density.as_f32() - EMBEDDING_DENSITY_DEFAULT).abs() < f32::EPSILON);
        assert_eq!(density.scaled_parts_per_100k(), 33_000);
    }

    #[test]
    fn rejects_non_finite_values() {
        assert!(matches!(
            EmbeddingDensity::new(f32::NAN),
            Err(EmbedError::InvalidDensity { .. })
        ));
        assert!(matches!(
            EmbeddingDensity::new(f32::INFINITY),
            Err(EmbedError::InvalidDensity { .. })
        ));
    }

    #[test]
    fn rejects_out_of_range_values() {
        for value in [-1.0, 0.0, 1.01] {
            assert!(matches!(
                EmbeddingDensity::new(value),
                Err(EmbedError::InvalidDensity { got }) if got.to_bits() == value.to_bits()
            ));
        }
    }

    #[test]
    fn accepts_full_density() {
        let density = EmbeddingDensity::new(1.0).unwrap();
        assert_eq!(density.scaled_parts_per_100k(), 100_000);
    }
}
