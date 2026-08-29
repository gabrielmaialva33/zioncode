use std::{
    collections::HashMap,
    io::{Cursor, Read},
    path::Component,
};

use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive};
use zstd::zstd_safe::CParameter;

use crate::{
    canonical::canonical_books,
    constants::{
        EXPECTED_BOOK_COUNT, EXPECTED_CHAPTER_COUNT, EXPECTED_LARGEST_RAW_ZSTD6_BOOK_BYTES,
        EXPECTED_LARGEST_RAW_ZSTD6_BOOK_ID, EXPECTED_RAW_USFM_BYTES,
        EXPECTED_RAW_USFM_ZSTD6_CHECKSUM_BYTES, EXPECTED_VERSE_COUNT, MANIFEST_SCHEMA,
        MAX_AGGREGATE_BYTES, MAX_ENTRY_BYTES, MAX_ZIP_ENTRIES, NORMALIZATION_NOTICE,
        PINNED_ZIP_BYTES, PINNED_ZIP_SHA256, REFERENCE_A57_SAFE_MAX_COMPRESSED_BYTES,
    },
    error::CorpusError,
    model::{
        BookArtifact, BookManifestEntry, BookMetrics, CanonicalBook, CorpusManifest, CorpusPackage,
        CorpusTotals, SourceMetadata,
    },
    usfm,
};

/// Verifies the exact byte length and SHA-256 of the immutable BLIVRE release asset.
///
/// # Errors
///
/// Returns [`CorpusError::SourceZipLength`] or [`CorpusError::SourceZipHash`]
/// when the bytes differ from the pinned asset.
pub fn verify_pinned_zip(zip_bytes: &[u8]) -> Result<(), CorpusError> {
    if zip_bytes.len() != PINNED_ZIP_BYTES {
        return Err(CorpusError::SourceZipLength {
            actual: zip_bytes.len(),
            expected: PINNED_ZIP_BYTES,
        });
    }
    if sha256_hex(zip_bytes) != PINNED_ZIP_SHA256 {
        return Err(CorpusError::SourceZipHash);
    }
    Ok(())
}

/// Imports the exact pinned BLIVRE 2018.2.0 Textus Receptus ZIP.
///
/// No network access occurs. The returned package retains the original ZIP and
/// every raw USFM entry byte-for-byte alongside deterministic JSON payloads.
///
/// # Errors
///
/// Returns a typed [`CorpusError`] if source identity, archive bounds, raw
/// hashes, USFM invariants, serialization, or compression validation fails.
pub fn import_pinned_blivre(zip_bytes: &[u8]) -> Result<CorpusPackage, CorpusError> {
    verify_pinned_zip(zip_bytes)?;
    let books = canonical_books()?;
    import_verified_archive(zip_bytes, &books, true)
}

pub(crate) fn import_verified_archive(
    zip_bytes: &[u8],
    books: &[CanonicalBook],
    enforce_pinned_totals: bool,
) -> Result<CorpusPackage, CorpusError> {
    let raw_books = read_entries(zip_bytes, books)?;
    let mut artifacts = Vec::with_capacity(books.len());

    for book in books {
        let raw = raw_books
            .get(book.source_file.as_str())
            .ok_or_else(|| CorpusError::MissingEntry(book.source_file.clone()))?
            .clone();
        artifacts.push(build_artifact(book, raw)?);
    }

    build_package(zip_bytes, artifacts, enforce_pinned_totals)
}

fn read_entries(
    zip_bytes: &[u8],
    books: &[CanonicalBook],
) -> Result<HashMap<String, Vec<u8>>, CorpusError> {
    let expected: HashMap<&str, &CanonicalBook> = books
        .iter()
        .map(|book| (book.source_file.as_str(), book))
        .collect();
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(CorpusError::TooManyEntries {
            actual: archive.len(),
            limit: MAX_ZIP_ENTRIES,
        });
    }
    if archive.len() != books.len() {
        return Err(CorpusError::CorpusInvariant {
            name: "zip_entry_count",
            actual: archive.len(),
            expected: books.len(),
        });
    }
    if archive.offset() != 0 || !archive.comment().is_empty() {
        return Err(CorpusError::InvalidEntryName);
    }

    let mut decoded = HashMap::with_capacity(archive.len());
    let mut aggregate = 0_usize;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let path = entry.enclosed_name().ok_or(CorpusError::InvalidEntryName)?;
        let mut components = path.components();
        let Some(Component::Normal(component)) = components.next() else {
            return Err(CorpusError::InvalidEntryName);
        };
        if components.next().is_some() {
            return Err(CorpusError::InvalidEntryName);
        }
        let name = component
            .to_str()
            .filter(|name| name.is_ascii() && name.as_bytes() == entry.name_raw())
            .ok_or(CorpusError::InvalidEntryName)?
            .to_owned();
        let canonical = expected
            .get(name.as_str())
            .ok_or_else(|| CorpusError::UnexpectedEntry(name.clone()))?;

        if !entry.is_file()
            || entry.encrypted()
            || entry.compression() != CompressionMethod::Deflated
            || entry.compressed_size() > zip_bytes.len() as u64
        {
            return Err(CorpusError::UnsupportedEntry(name));
        }
        let declared = usize::try_from(entry.size()).map_err(|_| CorpusError::EntryTooLarge {
            name: name.clone(),
            actual: entry.size(),
            limit: MAX_ENTRY_BYTES,
        })?;
        if declared > MAX_ENTRY_BYTES {
            return Err(CorpusError::EntryTooLarge {
                name: name.clone(),
                actual: entry.size(),
                limit: MAX_ENTRY_BYTES,
            });
        }
        aggregate = aggregate
            .checked_add(declared)
            .ok_or(CorpusError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            })?;
        if aggregate > MAX_AGGREGATE_BYTES {
            return Err(CorpusError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            });
        }

        let mut raw = Vec::with_capacity(declared);
        (&mut entry)
            .take((MAX_ENTRY_BYTES + 1) as u64)
            .read_to_end(&mut raw)?;
        if raw.len() != declared || raw.len() != canonical.raw_bytes as usize {
            return Err(CorpusError::EntrySizeMismatch {
                name,
                actual: raw.len(),
                expected: canonical.raw_bytes as usize,
            });
        }
        if sha256_hex(&raw) != canonical.raw_sha256 {
            return Err(CorpusError::RawHashMismatch(canonical.id.clone()));
        }
        if decoded.insert(name.clone(), raw).is_some() {
            return Err(CorpusError::DuplicateEntry(name));
        }
    }

    for book in books {
        if !decoded.contains_key(book.source_file.as_str()) {
            return Err(CorpusError::MissingEntry(book.source_file.clone()));
        }
    }
    Ok(decoded)
}

pub(crate) fn build_artifact(
    book: &CanonicalBook,
    raw: Vec<u8>,
) -> Result<BookArtifact, CorpusError> {
    let parsed = usfm::parse(&raw, book)?;
    let payload_json = serde_json::to_vec(&parsed.document)?;
    let raw_zstd = compress_with_checksum(&raw)?;
    let payload_zstd = zstd::bulk::compress(&payload_json, 6).map_err(CorpusError::Zstd)?;
    if payload_zstd.len() > REFERENCE_A57_SAFE_MAX_COMPRESSED_BYTES {
        return Err(CorpusError::PayloadCapacityExceeded {
            book: book.id.clone(),
            actual: payload_zstd.len(),
            limit: REFERENCE_A57_SAFE_MAX_COMPRESSED_BYTES,
        });
    }

    let metrics = BookMetrics {
        chapter_count: u16::try_from(parsed.document.chapters.len())
            .map_err(|_| malformed_metric(book, "chapter count"))?,
        verse_records: u32::try_from(parsed.verse_count)
            .map_err(|_| malformed_metric(book, "verse count"))?,
        marker_records: u32::try_from(parsed.marker_count)
            .map_err(|_| malformed_metric(book, "marker count"))?,
        raw_usfm_bytes: u64::try_from(raw.len())
            .map_err(|_| malformed_metric(book, "raw byte count"))?,
        raw_usfm_sha256: sha256_hex(&raw),
        raw_usfm_zstd6_checksum_bytes: u64::try_from(raw_zstd.len())
            .map_err(|_| malformed_metric(book, "raw zstd byte count"))?,
        payload_json_bytes: u64::try_from(payload_json.len())
            .map_err(|_| malformed_metric(book, "payload byte count"))?,
        payload_json_sha256: sha256_hex(&payload_json),
        payload_json_zstd6_bytes: u64::try_from(payload_zstd.len())
            .map_err(|_| malformed_metric(book, "payload zstd byte count"))?,
    };

    Ok(BookArtifact {
        canonical: book.clone(),
        raw_usfm: raw,
        document: parsed.document,
        payload_json,
        metrics,
    })
}

pub(crate) fn build_package(
    zip_bytes: &[u8],
    artifacts: Vec<BookArtifact>,
    enforce_pinned_totals: bool,
) -> Result<CorpusPackage, CorpusError> {
    let totals = CorpusTotals {
        books: u16::try_from(artifacts.len())
            .map_err(|_| CorpusError::InvalidCanonicalData("book count overflow".to_owned()))?,
        chapters: sum_u16(artifacts.iter().map(|book| book.metrics.chapter_count))?,
        verse_records: sum_u32(artifacts.iter().map(|book| book.metrics.verse_records))?,
        raw_usfm_bytes: sum_u64(artifacts.iter().map(|book| book.metrics.raw_usfm_bytes))?,
        raw_usfm_zstd6_checksum_bytes: sum_u64(
            artifacts
                .iter()
                .map(|book| book.metrics.raw_usfm_zstd6_checksum_bytes),
        )?,
        payload_json_bytes: sum_u64(artifacts.iter().map(|book| book.metrics.payload_json_bytes))?,
        payload_json_zstd6_bytes: sum_u64(
            artifacts
                .iter()
                .map(|book| book.metrics.payload_json_zstd6_bytes),
        )?,
    };
    if enforce_pinned_totals {
        validate_pinned_totals(&totals, &artifacts)?;
    }

    let entries = artifacts
        .iter()
        .map(|book| BookManifestEntry {
            ordinal: book.canonical.ordinal,
            id: book.canonical.id.clone(),
            title: book.canonical.title.clone(),
            source_file: book.canonical.source_file.clone(),
            payload_file: format!(
                "books/{:02}-{}.json",
                book.canonical.ordinal, book.canonical.id
            ),
            raw_usfm_file: format!("provenance/raw-usfm/{}", book.canonical.source_file),
            metrics: book.metrics.clone(),
        })
        .collect();
    let source_zip = if enforce_pinned_totals {
        CorpusManifest::source_zip_identity()
    } else {
        crate::model::SourceZipIdentity {
            file: crate::constants::PINNED_ASSET_NAME.to_owned(),
            bytes: u64::try_from(zip_bytes.len()).map_err(|_| {
                CorpusError::InvalidCanonicalData("source ZIP byte count overflow".to_owned())
            })?,
            sha256: sha256_hex(zip_bytes),
        }
    };
    let manifest = CorpusManifest {
        schema: MANIFEST_SCHEMA,
        source: SourceMetadata::pinned_blivre(),
        source_zip,
        provenance: CorpusManifest::provenance_layout(NORMALIZATION_NOTICE),
        totals,
        books: entries,
    };
    let manifest_json = serde_json::to_vec(&manifest)?;

    Ok(CorpusPackage::from_parts(
        manifest,
        manifest_json,
        artifacts,
        zip_bytes.to_vec(),
    ))
}

fn validate_pinned_totals(
    totals: &CorpusTotals,
    artifacts: &[BookArtifact],
) -> Result<(), CorpusError> {
    invariant("book_count", usize::from(totals.books), EXPECTED_BOOK_COUNT)?;
    invariant(
        "chapter_count",
        usize::from(totals.chapters),
        EXPECTED_CHAPTER_COUNT,
    )?;
    invariant(
        "verse_count",
        totals.verse_records as usize,
        EXPECTED_VERSE_COUNT,
    )?;
    invariant(
        "raw_usfm_bytes",
        usize::try_from(totals.raw_usfm_bytes).unwrap_or(usize::MAX),
        EXPECTED_RAW_USFM_BYTES,
    )?;
    invariant(
        "raw_usfm_zstd6_checksum_bytes",
        usize::try_from(totals.raw_usfm_zstd6_checksum_bytes).unwrap_or(usize::MAX),
        EXPECTED_RAW_USFM_ZSTD6_CHECKSUM_BYTES,
    )?;
    let largest = artifacts
        .iter()
        .max_by_key(|book| book.metrics.raw_usfm_zstd6_checksum_bytes)
        .ok_or(CorpusError::CorpusInvariant {
            name: "largest_raw_zstd_book",
            actual: 0,
            expected: EXPECTED_LARGEST_RAW_ZSTD6_BOOK_BYTES,
        })?;
    if largest.canonical.id != EXPECTED_LARGEST_RAW_ZSTD6_BOOK_ID {
        return Err(CorpusError::InvalidCanonicalData(
            "largest raw zstd-6 book is not Psalms".to_owned(),
        ));
    }
    invariant(
        "largest_raw_zstd_book_bytes",
        usize::try_from(largest.metrics.raw_usfm_zstd6_checksum_bytes).unwrap_or(usize::MAX),
        EXPECTED_LARGEST_RAW_ZSTD6_BOOK_BYTES,
    )
}

fn compress_with_checksum(bytes: &[u8]) -> Result<Vec<u8>, CorpusError> {
    let mut compressor = zstd::bulk::Compressor::new(6).map_err(CorpusError::Zstd)?;
    compressor
        .set_parameter(CParameter::ChecksumFlag(true))
        .map_err(CorpusError::Zstd)?;
    compressor.compress(bytes).map_err(CorpusError::Zstd)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn invariant(name: &'static str, actual: usize, expected: usize) -> Result<(), CorpusError> {
    if actual != expected {
        return Err(CorpusError::CorpusInvariant {
            name,
            actual,
            expected,
        });
    }
    Ok(())
}

fn sum_u16(mut values: impl Iterator<Item = u16>) -> Result<u16, CorpusError> {
    values.try_fold(0_u16, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| CorpusError::InvalidCanonicalData("u16 total overflow".to_owned()))
    })
}

fn sum_u32(mut values: impl Iterator<Item = u32>) -> Result<u32, CorpusError> {
    values.try_fold(0_u32, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| CorpusError::InvalidCanonicalData("u32 total overflow".to_owned()))
    })
}

fn sum_u64(mut values: impl Iterator<Item = u64>) -> Result<u64, CorpusError> {
    values.try_fold(0_u64, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| CorpusError::InvalidCanonicalData("u64 total overflow".to_owned()))
    })
}

fn malformed_metric(book: &CanonicalBook, detail: &str) -> CorpusError {
    CorpusError::MalformedUsfm {
        book: book.id.clone(),
        detail: detail.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;

    fn raw_fixture() -> Vec<u8> {
        b"\xef\xbb\xbf\\id TST fixture\r\n\\ide UTF-8\r\n\\h Teste\r\n\\toc1 Teste\r\n\\toc2 Teste\r\n\\toc3 Ts\r\n\\mt Teste\r\n\\c 1\r\n\\p\r\n\\v 1 Texto \\add fornecido\\add* .\r\nConteudo textual livre: \xce\xa9 \xf0\x9f\x93\x96 \0 fim.\r\n"
            .to_vec()
    }

    fn fixture_book(raw: &[u8]) -> CanonicalBook {
        CanonicalBook {
            ordinal: 1,
            id: "TST".to_owned(),
            title: "Teste".to_owned(),
            source_file: "test.txt".to_owned(),
            chapter_count: 1,
            verse_count: 1,
            raw_bytes: u32::try_from(raw.len()).expect("fixture fits"),
            raw_sha256: sha256_hex(raw),
        }
    }

    fn zip_fixture_with_method(entries: &[(&str, &[u8])], method: CompressionMethod) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(method);
        for (name, raw) in entries {
            writer.start_file(*name, options).expect("start fixture");
            writer.write_all(raw).expect("write fixture");
        }
        writer.finish().expect("finish fixture").into_inner()
    }

    fn zip_fixture(name: &str, raw: &[u8]) -> Vec<u8> {
        zip_fixture_with_method(&[(name, raw)], CompressionMethod::Deflated)
    }

    #[test]
    fn generated_zip_fixture_import_is_offline_deterministic_and_raw_exact() {
        let raw = raw_fixture();
        let book = fixture_book(&raw);
        let zip = zip_fixture("test.txt", &raw);
        let first = import_verified_archive(&zip, std::slice::from_ref(&book), false)
            .expect("fixture imports");
        let second = import_verified_archive(&zip, std::slice::from_ref(&book), false)
            .expect("fixture imports again");
        assert_eq!(first.books()[0].raw_usfm, raw);
        let decoded: crate::BookDocument =
            serde_json::from_slice(&first.books()[0].payload_json).expect("deserialize payload");
        assert_eq!(decoded.raw_usfm.as_bytes(), raw);
        assert_eq!(
            first.books()[0].payload_json,
            second.books()[0].payload_json
        );
        assert_eq!(first.manifest_json(), second.manifest_json());
        let unchecked = zstd::bulk::compress(&raw, 6).expect("unchecked metric");
        assert_eq!(
            first.books()[0].metrics.raw_usfm_zstd6_checksum_bytes,
            u64::try_from(unchecked.len() + 4).expect("metric fits")
        );
        assert!(first.books()[0].metrics.payload_json_zstd6_bytes > 0);

        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("corpus");
        first
            .write_to_directory_for_test(&output, std::slice::from_ref(&book), &zip, None)
            .expect("materialize fixture");
        assert_eq!(
            std::fs::read(output.join("provenance/raw-usfm/test.txt")).expect("read raw"),
            raw
        );
        assert_eq!(
            std::fs::read(output.join("provenance/usfm-blivre-tr.zip")).expect("read ZIP"),
            zip
        );
        assert_eq!(
            std::fs::read(output.join("books/01-TST.json")).expect("read payload"),
            first.books()[0].payload_json
        );
        assert!(matches!(
            first.write_to_directory_for_test(&output, std::slice::from_ref(&book), &zip, None),
            Err(CorpusError::OutputExists(_))
        ));
    }

    #[test]
    fn strict_verifier_rejects_before_zip_parsing() {
        assert!(matches!(
            verify_pinned_zip(b"not a zip"),
            Err(CorpusError::SourceZipLength { .. })
        ));
        let wrong = vec![0_u8; PINNED_ZIP_BYTES];
        assert!(matches!(
            verify_pinned_zip(&wrong),
            Err(CorpusError::SourceZipHash)
        ));
    }

    #[test]
    fn unsafe_entry_name_is_rejected_without_extraction() {
        let raw = raw_fixture();
        let book = fixture_book(&raw);
        let zip = zip_fixture("../test.txt", &raw);
        assert!(matches!(
            import_verified_archive(&zip, &[book], false),
            Err(CorpusError::InvalidEntryName)
        ));
    }

    #[test]
    fn entry_count_size_compression_and_aggregate_limits_are_enforced() {
        let empty_names: Vec<String> = (0..=MAX_ZIP_ENTRIES)
            .map(|index| format!("{index}.txt"))
            .collect();
        let empty_entries: Vec<(&str, &[u8])> = empty_names
            .iter()
            .map(|name| (name.as_str(), &[][..]))
            .collect();
        let too_many = zip_fixture_with_method(&empty_entries, CompressionMethod::Deflated);
        assert!(matches!(
            read_entries(&too_many, &[]),
            Err(CorpusError::TooManyEntries { .. })
        ));

        let oversized = vec![0_u8; MAX_ENTRY_BYTES + 1];
        let oversized_book = fixture_book(&oversized);
        let oversized_zip = zip_fixture("test.txt", &oversized);
        assert!(matches!(
            read_entries(&oversized_zip, &[oversized_book]),
            Err(CorpusError::EntryTooLarge { .. })
        ));

        let raw = raw_fixture();
        let stored = zip_fixture_with_method(&[("test.txt", &raw)], CompressionMethod::Stored);
        assert!(matches!(
            read_entries(&stored, &[fixture_book(&raw)]),
            Err(CorpusError::UnsupportedEntry(_))
        ));

        let aggregate_raw = vec![0_u8; 500_000];
        let aggregate_hash = sha256_hex(&aggregate_raw);
        let names: Vec<String> = (0..11).map(|index| format!("b{index}.txt")).collect();
        let entries: Vec<(&str, &[u8])> = names
            .iter()
            .map(|name| (name.as_str(), aggregate_raw.as_slice()))
            .collect();
        let books: Vec<CanonicalBook> = names
            .iter()
            .enumerate()
            .map(|(index, name)| CanonicalBook {
                ordinal: u8::try_from(index + 1).expect("ordinal fits"),
                id: format!("B{index}"),
                title: format!("Book {index}"),
                source_file: name.clone(),
                chapter_count: 1,
                verse_count: 1,
                raw_bytes: u32::try_from(aggregate_raw.len()).expect("raw size fits"),
                raw_sha256: aggregate_hash.clone(),
            })
            .collect();
        let aggregate_zip = zip_fixture_with_method(&entries, CompressionMethod::Deflated);
        assert!(matches!(
            read_entries(&aggregate_zip, &books),
            Err(CorpusError::AggregateTooLarge { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn materialized_corpus_is_owner_only_under_a_permissive_umask() {
        use std::{os::unix::fs::MetadataExt, path::PathBuf, process::Command};

        const CHILD_PATH_ENV: &str = "ZION_CORPUS_PRIVATE_MODE_CHILD_PATH";
        if let Some(path) = std::env::var_os(CHILD_PATH_ENV) {
            let output = PathBuf::from(path);
            let raw = raw_fixture();
            let book = fixture_book(&raw);
            let zip = zip_fixture("test.txt", &raw);
            let package = import_verified_archive(&zip, std::slice::from_ref(&book), false)
                .expect("fixture imports in child");
            package
                .write_to_directory_for_test(&output, std::slice::from_ref(&book), &zip, None)
                .expect("materialize private corpus");

            for directory in [
                output.clone(),
                output.join("books"),
                output.join("provenance"),
                output.join("provenance/raw-usfm"),
            ] {
                let metadata = std::fs::metadata(&directory).expect("directory metadata");
                assert!(metadata.is_dir());
                assert_eq!(metadata.mode() & 0o777, 0o700, "{}", directory.display());
            }
            for file in [
                output.join("manifest.json"),
                output.join("books/01-TST.json"),
                output.join("provenance/usfm-blivre-tr.zip"),
                output.join("provenance/UPSTREAM_README.md"),
                output.join("provenance/UPSTREAM_LICENSE.md"),
                output.join("provenance/SOURCE.json"),
                output.join("provenance/raw-usfm/test.txt"),
            ] {
                let metadata = std::fs::metadata(&file).expect("file metadata");
                assert!(metadata.is_file());
                assert_eq!(metadata.mode() & 0o777, 0o600, "{}", file.display());
            }
            return;
        }

        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("private-corpus");
        let executable = std::env::current_exe().expect("current test executable");
        let status = Command::new("sh")
            .arg("-c")
            .arg(
                "umask 000; exec \"$1\" --exact \
                 archive::tests::materialized_corpus_is_owner_only_under_a_permissive_umask",
            )
            .arg("sh")
            .arg(executable)
            .env(CHILD_PATH_ENV, &output)
            .status()
            .expect("spawn isolated umask test");
        assert!(status.success());
        assert!(output.is_dir());
    }
}
