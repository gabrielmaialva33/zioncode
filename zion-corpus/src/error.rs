use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CorpusError {
    #[error("source ZIP length is {actual} bytes; expected exactly {expected}")]
    SourceZipLength { actual: usize, expected: usize },
    #[error("source ZIP SHA-256 does not match the pinned BLIVRE release")]
    SourceZipHash,
    #[error("invalid ZIP archive: {0}")]
    InvalidZip(#[from] zip::result::ZipError),
    #[error("ZIP contains {actual} entries; limit is {limit}")]
    TooManyEntries { actual: usize, limit: usize },
    #[error("ZIP entry name is unsafe or non-canonical")]
    InvalidEntryName,
    #[error("unexpected ZIP entry `{0}`")]
    UnexpectedEntry(String),
    #[error("duplicate ZIP entry `{0}`")]
    DuplicateEntry(String),
    #[error("required ZIP entry `{0}` is missing")]
    MissingEntry(String),
    #[error(
        "ZIP entry `{0}` is encrypted, a directory, a symlink, or uses unsupported compression"
    )]
    UnsupportedEntry(String),
    #[error("ZIP entry `{name}` declares {actual} bytes; limit is {limit}")]
    EntryTooLarge {
        name: String,
        actual: u64,
        limit: usize,
    },
    #[error("aggregate uncompressed ZIP size exceeds {limit} bytes")]
    AggregateTooLarge { limit: usize },
    #[error("ZIP entry `{name}` decoded to {actual} bytes; expected {expected}")]
    EntrySizeMismatch {
        name: String,
        actual: usize,
        expected: usize,
    },
    #[error("raw SHA-256 mismatch for `{0}`")]
    RawHashMismatch(String),
    #[error("checked-in canonical book data is invalid: {0}")]
    InvalidCanonicalData(String),
    #[error("`{book}` is not valid UTF-8 USFM")]
    InvalidUtf8 { book: String },
    #[error("`{book}` contains a line of {actual} bytes; limit is {limit}")]
    LineTooLong {
        book: String,
        actual: usize,
        limit: usize,
    },
    #[error("`{book}` contains more than {limit} USFM markers")]
    TooManyMarkers { book: String, limit: usize },
    #[error("unsupported USFM marker `{marker}` in `{book}`")]
    UnsupportedMarker { book: String, marker: String },
    #[error("malformed USFM in `{book}`: {detail}")]
    MalformedUsfm { book: String, detail: String },
    #[error("corpus invariant `{name}` is {actual}; expected {expected}")]
    CorpusInvariant {
        name: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error(
        "normalized payload for `{book}` compresses to {actual} bytes; reference capacity is {limit}"
    )]
    PayloadCapacityExceeded {
        book: String,
        actual: usize,
        limit: usize,
    },
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zstd level-6 compression failed: {0}")]
    Zstd(#[source] io::Error),
    #[error("output path already exists: {0}")]
    OutputExists(PathBuf),
    #[error("corpus package failed materialization validation: {0}")]
    InvalidPackage(String),
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}
