pub const PINNED_VERSION: &str = "2018.2.0";
pub const PINNED_TAG_COMMIT: &str = "a386942daee9984c654ebc8cea95ec9d3661b183";
pub const PINNED_RELEASE_URL: &str = "https://github.com/blivre/BibliaLivre/releases/tag/2018.2.0";
pub const PINNED_ASSET_URL: &str =
    "https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip";
pub const PINNED_ASSET_NAME: &str = "usfm-blivre-tr.zip";
pub const PINNED_ZIP_BYTES: usize = 1_364_020;
pub const PINNED_ZIP_SHA256: &str =
    "83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead";
pub const PINNED_README_SHA256: &str =
    "2c9188d1031b419ec4c59288f3aa41368bbfd5a4b646edaf6ea74169e1a46114";
pub const PINNED_LICENSE_SHA256: &str =
    "6ff396843c629e408e3dbad8b16653acbfed6448fe4011bbde4e13baaae706b7";

pub const EXPECTED_BOOK_COUNT: usize = 66;
pub const EXPECTED_CHAPTER_COUNT: usize = 1_189;
pub const EXPECTED_VERSE_COUNT: usize = 31_102;
pub const EXPECTED_RAW_USFM_BYTES: usize = 4_276_761;
pub const EXPECTED_RAW_USFM_ZSTD6_CHECKSUM_BYTES: usize = 1_391_039;
pub const EXPECTED_LARGEST_RAW_ZSTD6_BOOK_ID: &str = "PSA";
pub const EXPECTED_LARGEST_RAW_ZSTD6_BOOK_BYTES: usize = 80_236;

pub const MAX_ZIP_ENTRIES: usize = 96;
pub const MAX_ENTRY_BYTES: usize = 512 * 1024;
pub const MAX_AGGREGATE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 4 * 1024;
pub const MAX_MARKERS_PER_BOOK: usize = 8_192;
pub(crate) const MAX_MARKER_NAME_BYTES: usize = 8;
pub(crate) const PAYLOAD_SCHEMA: u8 = 1;
pub(crate) const MANIFEST_SCHEMA: u8 = 1;
pub(crate) const LANGUAGE: &str = "pt-BR";
pub(crate) const LICENSE_NAME: &str = "CC BY 3.0 BR";
pub(crate) const LICENSE_URL: &str = "https://creativecommons.org/licenses/by/3.0/br/";
pub(crate) const NORMALIZATION_NOTICE: &str = "The structured marker projection omits the UTF-8 BOM and normalizes CRLF to LF. The raw_usfm field preserves the verified source UTF-8 bytes exactly, including BOM and CRLF, and is exportable byte-for-byte.";
pub const REFERENCE_A57_SAFE_MAX_COMPRESSED_BYTES: usize = 273_316;
