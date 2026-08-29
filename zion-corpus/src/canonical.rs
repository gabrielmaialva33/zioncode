use std::{collections::HashSet, ffi::OsStr, path::Path};

use crate::{
    constants::{
        EXPECTED_BOOK_COUNT, EXPECTED_CHAPTER_COUNT, EXPECTED_RAW_USFM_BYTES, EXPECTED_VERSE_COUNT,
    },
    error::CorpusError,
    model::CanonicalBook,
};

const BOOKS_JSON: &str = include_str!("../data/books.json");

/// Loads and validates the checked-in canonical Protestant 66-book mapping.
///
/// # Errors
///
/// Returns [`CorpusError`] if the embedded JSON or any identity, ordinal,
/// aggregate, filename, or hash invariant is invalid.
pub fn canonical_books() -> Result<Vec<CanonicalBook>, CorpusError> {
    let books: Vec<CanonicalBook> = serde_json::from_str(BOOKS_JSON)?;
    validate(&books)?;
    Ok(books)
}

fn validate(books: &[CanonicalBook]) -> Result<(), CorpusError> {
    invariant("book_count", books.len(), EXPECTED_BOOK_COUNT)?;

    let mut ids = HashSet::with_capacity(books.len());
    let mut files = HashSet::with_capacity(books.len());
    let mut chapters = 0_usize;
    let mut verses = 0_usize;
    let mut raw_bytes = 0_usize;

    for (index, book) in books.iter().enumerate() {
        let ordinal = usize::from(book.ordinal);
        if ordinal != index + 1 {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "ordinal {ordinal} appears at position {}",
                index + 1
            )));
        }
        if book.id.is_empty()
            || book
                .id
                .bytes()
                .any(|byte| !byte.is_ascii_uppercase() && !byte.is_ascii_digit())
        {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "invalid USFM identifier `{}`",
                book.id
            )));
        }
        if book.title.is_empty() {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "empty title for `{}`",
                book.id
            )));
        }
        if Path::new(&book.source_file).extension() != Some(OsStr::new("txt"))
            || book.source_file.contains('/')
            || book.source_file.contains('\\')
            || book.source_file.contains('\0')
        {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "unsafe source file `{}`",
                book.source_file
            )));
        }
        if book.raw_sha256.len() != 64
            || book
                .raw_sha256
                .bytes()
                .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
        {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "invalid raw SHA-256 for `{}`",
                book.id
            )));
        }
        if !ids.insert(book.id.as_str()) || !files.insert(book.source_file.as_str()) {
            return Err(CorpusError::InvalidCanonicalData(format!(
                "duplicate identifier or file for `{}`",
                book.id
            )));
        }

        chapters = chapters
            .checked_add(usize::from(book.chapter_count))
            .ok_or_else(|| {
                CorpusError::InvalidCanonicalData("chapter total overflow".to_owned())
            })?;
        verses = verses
            .checked_add(book.verse_count as usize)
            .ok_or_else(|| CorpusError::InvalidCanonicalData("verse total overflow".to_owned()))?;
        raw_bytes = raw_bytes
            .checked_add(book.raw_bytes as usize)
            .ok_or_else(|| {
                CorpusError::InvalidCanonicalData("raw-byte total overflow".to_owned())
            })?;
    }

    invariant("chapter_count", chapters, EXPECTED_CHAPTER_COUNT)?;
    invariant("verse_count", verses, EXPECTED_VERSE_COUNT)?;
    invariant("raw_usfm_bytes", raw_bytes, EXPECTED_RAW_USFM_BYTES)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_mapping_is_complete_contiguous_and_pinned() {
        let books = canonical_books().expect("checked-in mapping must validate");
        assert_eq!(books.len(), 66);
        assert_eq!(books.first().map(|book| book.id.as_str()), Some("GEN"));
        assert_eq!(books.last().map(|book| book.id.as_str()), Some("REV"));
        assert_eq!(books[18].title, "Salmos");
        assert_eq!(books[18].raw_bytes, 258_558);
    }
}
