use thiserror::Error;

/// Errors produced while creating or re-deriving a reusable collection key.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum CollectionKeyError {
    #[error("passphrase length is outside the ARC limit")]
    InvalidPassphraseLength,
    #[error("collection identifier must be nonzero")]
    InvalidIdentifier,
    #[error("operating-system randomness is unavailable")]
    RandomFailed,
    #[error("collection-key derivation failed")]
    KdfFailed,
}

/// Errors produced while sealing a capsule.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SealError {
    #[error("input exceeds the ARC limit")]
    InputTooLarge,
    #[error("invalid PNG")]
    InvalidPng,
    #[error("PNG must be an exact 8-bit RGB image")]
    UnsupportedPixelFormat,
    #[error("invalid carrier dimensions")]
    InvalidDimensions,
    #[error("carrier is too small for ARC")]
    CarrierTooSmall,
    #[error("passphrase length is outside the ARC limit")]
    InvalidPassphraseLength,
    #[error("item metadata is invalid or too large")]
    InvalidMetadata,
    #[error("content exceeds the ARC limit")]
    ContentTooLarge,
    #[error("compressed content exceeds the ARC limit")]
    CompressedTooLarge,
    #[error("capsule does not fit in the carrier")]
    CapacityExceeded,
    #[error("checked arithmetic overflow")]
    ArithmeticOverflow,
    #[error("operating-system randomness is unavailable")]
    RandomFailed,
    #[error("collection-key derivation failed")]
    KdfFailed,
    #[error("zstd compression failed")]
    CompressionFailed,
    #[error("authenticated encryption failed")]
    CryptoFailed,
    #[error("Reed-Solomon encoding failed")]
    EccFailed,
    #[error("PNG encoding failed")]
    PngEncodeFailed,
}

/// Errors produced while opening a capsule.
///
/// Once collection-key derivation succeeds, all recovery-stage failures are
/// deliberately collapsed to [`OpenError::RecoveryFailed`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OpenError {
    #[error("input exceeds the ARC limit")]
    InputTooLarge,
    #[error("invalid PNG")]
    InvalidPng,
    #[error("PNG must be an exact 8-bit RGB image")]
    UnsupportedPixelFormat,
    #[error("invalid carrier dimensions")]
    InvalidDimensions,
    #[error("carrier is too small for ARC")]
    CarrierTooSmall,
    #[error("ARC bootstrap magic is invalid")]
    BadMagic,
    #[error("ARC version is unsupported")]
    UnsupportedVersion,
    #[error("ARC bootstrap length is invalid")]
    InvalidBootstrapLength,
    #[error("ARC bootstrap CRC does not match")]
    BootstrapCrcMismatch,
    #[error("ARC ECC profile is unsupported")]
    UnsupportedProfile,
    #[error("ARC flags are unsupported")]
    UnsupportedFlags,
    #[error("ARC suite is unsupported")]
    UnsupportedSuite,
    #[error("ARC density is unsupported")]
    UnsupportedDensity,
    #[error("ARC identifier is invalid")]
    InvalidIdentifier,
    #[error("ARC bootstrap length relation is invalid")]
    InvalidLength,
    #[error("capsule exceeds carrier capacity")]
    CapacityExceeded,
    #[error("passphrase length is outside the ARC limit")]
    InvalidPassphraseLength,
    #[error("collection-key derivation failed")]
    KdfFailed,
    #[error("capsule recovery failed")]
    RecoveryFailed,
}
