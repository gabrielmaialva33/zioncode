//! Reproducible, bounded importer for the pinned BLIVRE 2018.2.0 corpus.

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

mod archive;
mod canonical;
mod constants;
mod error;
mod model;
mod package;
mod usfm;

pub use archive::{import_pinned_blivre, verify_pinned_zip};
pub use canonical::canonical_books;
pub use constants::{
    EXPECTED_BOOK_COUNT, EXPECTED_CHAPTER_COUNT, EXPECTED_LARGEST_RAW_ZSTD6_BOOK_BYTES,
    EXPECTED_LARGEST_RAW_ZSTD6_BOOK_ID, EXPECTED_RAW_USFM_BYTES,
    EXPECTED_RAW_USFM_ZSTD6_CHECKSUM_BYTES, EXPECTED_VERSE_COUNT, MAX_AGGREGATE_BYTES,
    MAX_ENTRY_BYTES, MAX_LINE_BYTES, MAX_MARKERS_PER_BOOK, MAX_ZIP_ENTRIES, PINNED_ASSET_NAME,
    PINNED_ASSET_URL, PINNED_LICENSE_SHA256, PINNED_README_SHA256, PINNED_RELEASE_URL,
    PINNED_TAG_COMMIT, PINNED_VERSION, PINNED_ZIP_BYTES, PINNED_ZIP_SHA256,
    REFERENCE_A57_SAFE_MAX_COMPRESSED_BYTES,
};
pub use error::CorpusError;
pub use model::{
    BookArtifact, BookDocument, BookManifestEntry, BookMetrics, CanonicalBook, Chapter,
    CorpusManifest, CorpusPackage, CorpusTotals, Marker, MarkerRecord, ProvenanceLayout,
    SourceMetadata, SourceZipIdentity,
};
pub use package::{upstream_license_bytes, upstream_readme_bytes};
