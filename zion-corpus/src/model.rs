use std::fmt;

use serde::{Deserialize, Serialize};

use crate::constants::{
    LANGUAGE, LICENSE_NAME, LICENSE_URL, NORMALIZATION_NOTICE, PAYLOAD_SCHEMA, PINNED_ASSET_NAME,
    PINNED_ASSET_URL, PINNED_LICENSE_SHA256, PINNED_README_SHA256, PINNED_RELEASE_URL,
    PINNED_TAG_COMMIT, PINNED_VERSION, PINNED_ZIP_BYTES, PINNED_ZIP_SHA256,
};

/// One checked-in canonical book mapping and its pinned raw-source identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanonicalBook {
    pub ordinal: u8,
    pub id: String,
    pub title: String,
    pub source_file: String,
    pub chapter_count: u16,
    pub verse_count: u32,
    pub raw_bytes: u32,
    pub raw_sha256: String,
}

/// A marker in the deliberately small BLIVRE 2018.2.0 USFM vocabulary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Marker {
    #[serde(rename = "id")]
    Id,
    #[serde(rename = "ide")]
    Ide,
    #[serde(rename = "h")]
    Header,
    #[serde(rename = "toc1")]
    Toc1,
    #[serde(rename = "toc2")]
    Toc2,
    #[serde(rename = "toc3")]
    Toc3,
    #[serde(rename = "mt")]
    MainTitle,
    #[serde(rename = "mt1")]
    MainTitle1,
    #[serde(rename = "p")]
    Paragraph,
    #[serde(rename = "v")]
    Verse,
    #[serde(rename = "d")]
    DescriptiveTitle,
    #[serde(rename = "add")]
    Addition,
    #[serde(rename = "add*")]
    AdditionEnd,
    #[serde(rename = "f")]
    Footnote,
    #[serde(rename = "f*")]
    FootnoteEnd,
    #[serde(rename = "fr")]
    FootnoteReference,
    #[serde(rename = "fq")]
    FootnoteQuote,
    #[serde(rename = "ft")]
    FootnoteText,
    #[serde(rename = "rq")]
    ReferenceQuote,
    #[serde(rename = "rq*")]
    ReferenceQuoteEnd,
}

/// One normalized marker event. Closing and paragraph markers normally omit text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarkerRecord {
    pub marker: Marker,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u16>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

/// One canonical chapter with marker order retained exactly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chapter {
    pub number: u16,
    pub markers: Vec<MarkerRecord>,
}

/// Attribution and immutable upstream identity included in every book payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceMetadata {
    pub name: String,
    pub version: String,
    pub version_date: String,
    pub release_url: String,
    pub asset_url: String,
    pub tag_commit: String,
    pub license: String,
    pub license_url: String,
    pub authors: Vec<String>,
}

impl SourceMetadata {
    #[must_use]
    pub fn pinned_blivre() -> Self {
        Self {
            name: "Bíblia Livre".to_owned(),
            version: PINNED_VERSION.to_owned(),
            version_date: "2018-02".to_owned(),
            release_url: PINNED_RELEASE_URL.to_owned(),
            asset_url: PINNED_ASSET_URL.to_owned(),
            tag_commit: PINNED_TAG_COMMIT.to_owned(),
            license: LICENSE_NAME.to_owned(),
            license_url: LICENSE_URL.to_owned(),
            authors: vec![
                "Diego Santos".to_owned(),
                "Mario Sérgio".to_owned(),
                "Marco Teles".to_owned(),
            ],
        }
    }
}

/// Deterministic, compact, versioned JSON document sealed into one ARC item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookDocument {
    pub schema: u8,
    pub id: String,
    pub ordinal: u8,
    pub title: String,
    pub language: String,
    pub headers: Vec<MarkerRecord>,
    pub chapters: Vec<Chapter>,
    /// Complete verified source USFM, including its UTF-8 BOM and CRLF bytes.
    pub raw_usfm: String,
    pub source: SourceMetadata,
    pub normalization: String,
}

impl BookDocument {
    pub(crate) fn new(
        book: &CanonicalBook,
        headers: Vec<MarkerRecord>,
        chapters: Vec<Chapter>,
        raw_usfm: String,
    ) -> Self {
        Self {
            schema: PAYLOAD_SCHEMA,
            id: book.id.clone(),
            ordinal: book.ordinal,
            title: book.title.clone(),
            language: LANGUAGE.to_owned(),
            headers,
            chapters,
            raw_usfm,
            source: SourceMetadata::pinned_blivre(),
            normalization: NORMALIZATION_NOTICE.to_owned(),
        }
    }
}

/// Exact source and normalized-payload measurements for one book.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookMetrics {
    pub chapter_count: u16,
    pub verse_records: u32,
    pub marker_records: u32,
    pub raw_usfm_bytes: u64,
    pub raw_usfm_sha256: String,
    /// zstd 1.5.7 level 6, with the frame checksum explicitly enabled.
    pub raw_usfm_zstd6_checksum_bytes: u64,
    pub payload_json_bytes: u64,
    pub payload_json_sha256: String,
    /// ARC-equivalent zstd level 6 metric (frame checksum disabled).
    pub payload_json_zstd6_bytes: u64,
}

/// Imported book with both byte-exact USFM and deterministic normalized JSON.
#[derive(Clone, PartialEq, Eq)]
pub struct BookArtifact {
    pub canonical: CanonicalBook,
    pub raw_usfm: Vec<u8>,
    pub document: BookDocument,
    pub payload_json: Vec<u8>,
    pub metrics: BookMetrics,
}

impl fmt::Debug for BookArtifact {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BookArtifact")
            .field("canonical", &self.canonical)
            .field("raw_usfm_bytes", &self.raw_usfm.len())
            .field("payload_json_bytes", &self.payload_json.len())
            .field("metrics", &self.metrics)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BookManifestEntry {
    pub ordinal: u8,
    pub id: String,
    pub title: String,
    pub source_file: String,
    pub payload_file: String,
    pub raw_usfm_file: String,
    #[serde(flatten)]
    pub metrics: BookMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusTotals {
    pub books: u16,
    pub chapters: u16,
    pub verse_records: u32,
    pub raw_usfm_bytes: u64,
    pub raw_usfm_zstd6_checksum_bytes: u64,
    pub payload_json_bytes: u64,
    pub payload_json_zstd6_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceZipIdentity {
    pub file: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceLayout {
    pub source_zip: String,
    pub upstream_readme: String,
    pub upstream_readme_sha256: String,
    pub upstream_license: String,
    pub upstream_license_sha256: String,
    pub raw_usfm_directory: String,
    pub normalization_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusManifest {
    pub schema: u8,
    pub source: SourceMetadata,
    pub source_zip: SourceZipIdentity,
    pub provenance: ProvenanceLayout,
    pub totals: CorpusTotals,
    pub books: Vec<BookManifestEntry>,
}

impl CorpusManifest {
    pub(crate) fn source_zip_identity() -> SourceZipIdentity {
        SourceZipIdentity {
            file: PINNED_ASSET_NAME.to_owned(),
            bytes: PINNED_ZIP_BYTES as u64,
            sha256: PINNED_ZIP_SHA256.to_owned(),
        }
    }

    pub(crate) fn provenance_layout(normalization_notice: &str) -> ProvenanceLayout {
        ProvenanceLayout {
            source_zip: format!("provenance/{PINNED_ASSET_NAME}"),
            upstream_readme: "provenance/UPSTREAM_README.md".to_owned(),
            upstream_readme_sha256: PINNED_README_SHA256.to_owned(),
            upstream_license: "provenance/UPSTREAM_LICENSE.md".to_owned(),
            upstream_license_sha256: PINNED_LICENSE_SHA256.to_owned(),
            raw_usfm_directory: "provenance/raw-usfm".to_owned(),
            normalization_notice: normalization_notice.to_owned(),
        }
    }
}

/// A complete reproducible import, ready to materialize without network access.
#[derive(Clone, PartialEq, Eq)]
pub struct CorpusPackage {
    manifest: CorpusManifest,
    manifest_json: Vec<u8>,
    books: Vec<BookArtifact>,
    source_zip: Vec<u8>,
}

impl CorpusPackage {
    pub(crate) fn from_parts(
        manifest: CorpusManifest,
        manifest_json: Vec<u8>,
        books: Vec<BookArtifact>,
        source_zip: Vec<u8>,
    ) -> Self {
        Self {
            manifest,
            manifest_json,
            books,
            source_zip,
        }
    }

    /// Returns the validated deterministic manifest.
    #[must_use]
    pub const fn manifest(&self) -> &CorpusManifest {
        &self.manifest
    }

    /// Returns the deterministic serialized manifest bytes.
    #[must_use]
    pub fn manifest_json(&self) -> &[u8] {
        &self.manifest_json
    }

    /// Returns the imported books in canonical order.
    #[must_use]
    pub fn books(&self) -> &[BookArtifact] {
        &self.books
    }

    pub(crate) fn source_zip(&self) -> &[u8] {
        &self.source_zip
    }

    #[cfg(test)]
    pub(crate) fn manifest_mut_for_test(&mut self) -> &mut CorpusManifest {
        &mut self.manifest
    }

    #[cfg(test)]
    pub(crate) fn manifest_json_mut_for_test(&mut self) -> &mut Vec<u8> {
        &mut self.manifest_json
    }

    #[cfg(test)]
    pub(crate) fn books_mut_for_test(&mut self) -> &mut Vec<BookArtifact> {
        &mut self.books
    }

    #[cfg(test)]
    pub(crate) fn source_zip_mut_for_test(&mut self) -> &mut Vec<u8> {
        &mut self.source_zip
    }
}

impl fmt::Debug for CorpusPackage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CorpusPackage")
            .field("manifest", &self.manifest)
            .field("manifest_json_bytes", &self.manifest_json.len())
            .field("book_count", &self.books.len())
            .field("source_zip_bytes", &self.source_zip.len())
            .finish_non_exhaustive()
    }
}
