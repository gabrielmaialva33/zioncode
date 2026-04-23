//! Domain types used by the public codec API.

use std::fmt;
use std::ops::{Deref, DerefMut};

/// Opaque file identity stored in every symbol header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId([u8; 16]);

impl FileId {
    #[must_use]
    pub fn new_random() -> Self {
        Self(*uuid::Uuid::new_v4().as_bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        self.0
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", uuid::Uuid::from_bytes(self.0))
    }
}

impl PartialEq<[u8; 16]> for FileId {
    fn eq(&self, other: &[u8; 16]) -> bool {
        self.0 == *other
    }
}

/// BLAKE3-256 hash of the original file bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalHash([u8; 32]);

impl GlobalHash {
    #[must_use]
    pub fn digest(bytes: &[u8]) -> Self {
        Self(*blake3::hash(bytes).as_bytes())
    }

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl PartialEq<[u8; 32]> for GlobalHash {
    fn eq(&self, other: &[u8; 32]) -> bool {
        self.0 == *other
    }
}

/// Encoded bytes for one transmitted `.zbin` symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolBytes(Vec<u8>);

impl SymbolBytes {
    #[must_use]
    pub const fn from_vec(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for SymbolBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl Deref for SymbolBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_bytes()
    }
}

impl DerefMut for SymbolBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
