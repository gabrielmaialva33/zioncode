//! Narrow, byte-oriented Android bridge for authenticated ARC book capsules.

#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::any::Any;

use jni::{
    Env, EnvUnowned,
    errors::{ErrorPolicy, Result as JniResult},
    jni_mangle, jni_str,
    objects::{JByteArray, JObject},
    strings::JNIStr,
};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};
use zion_arc::{OpenError, OpenOutput};
use zion_corpus::{BookDocument, Marker, MarkerRecord};

const BOOK_MEDIA_TYPE: &str = "application/vnd.zion.blivre+json";
const BOOK_SCHEMA: u8 = 1;
const BOOK_LANGUAGE: &str = "pt-BR";
const MAX_BOOK_ID_BYTES: usize = 3;
const MAX_TITLE_BYTES: usize = 255;
const MAX_SOURCE_TEXT_BYTES: usize = 4_096;
const MAX_AUTHORS: usize = 32;
const MAX_CHAPTERS: usize = 1_024;
const MAX_JNI_PNG_BYTES: usize = 134_217_728;
const MAX_JNI_PASSPHRASE_BYTES: usize = 1_024;

/// User-safe failures returned by the pure mobile decoder.
///
/// The variants intentionally retain no underlying error or decrypted value.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum MobileError {
    #[error("invalid ARC image")]
    InvalidImage,
    #[error("invalid passphrase")]
    InvalidPassphrase,
    #[error("capsule recovery failed")]
    RecoveryFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InvalidBook;

struct ZeroizingBookDocument(BookDocument);

impl Drop for ZeroizingBookDocument {
    fn drop(&mut self) {
        zeroize_book_document(&mut self.0);
    }
}

/// Opens one exact PNG byte stream, authenticates ARC, and validates its book JSON.
///
/// The returned allocation is zeroized when the caller drops it. Callers should
/// copy it only into the final presentation boundary.
///
/// # Errors
///
/// Returns a user-safe [`MobileError`] for an invalid image or passphrase, or a
/// generic recovery failure. Every failure after ARC successfully opens maps to
/// that same generic recovery result.
pub fn decode_book_png(
    png_bytes: &[u8],
    passphrase_bytes: &[u8],
) -> Result<Zeroizing<Vec<u8>>, MobileError> {
    let OpenOutput {
        content,
        metadata,
        collection_id: _,
        capsule_id: _,
        profile: _,
    } = zion_arc::open_png_bytes(png_bytes, passphrase_bytes)
        .map_err(|error| map_open_error(&error))?;

    let content = Zeroizing::new(content);
    if metadata.media_type != BOOK_MEDIA_TYPE {
        return Err(MobileError::RecoveryFailed);
    }

    validate_and_reserialize_book_json(&content).map_err(|_| MobileError::RecoveryFailed)
}

fn validate_and_reserialize_book_json(
    json_bytes: &[u8],
) -> Result<Zeroizing<Vec<u8>>, InvalidBook> {
    let document: BookDocument = serde_json::from_slice(json_bytes).map_err(|_| InvalidBook)?;
    let document = ZeroizingBookDocument(document);
    validate_document(&document.0)?;
    let json = serde_json::to_vec(&document.0).map_err(|_| InvalidBook)?;
    Ok(Zeroizing::new(json))
}

fn map_open_error(error: &OpenError) -> MobileError {
    match error {
        OpenError::InvalidPassphraseLength => MobileError::InvalidPassphrase,
        OpenError::KdfFailed | OpenError::RecoveryFailed => MobileError::RecoveryFailed,
        OpenError::InputTooLarge
        | OpenError::InvalidPng
        | OpenError::UnsupportedPixelFormat
        | OpenError::InvalidDimensions
        | OpenError::CarrierTooSmall
        | OpenError::BadMagic
        | OpenError::UnsupportedVersion
        | OpenError::InvalidBootstrapLength
        | OpenError::BootstrapCrcMismatch
        | OpenError::UnsupportedProfile
        | OpenError::UnsupportedFlags
        | OpenError::UnsupportedSuite
        | OpenError::UnsupportedDensity
        | OpenError::InvalidIdentifier
        | OpenError::InvalidLength
        | OpenError::CapacityExceeded => MobileError::InvalidImage,
    }
}

fn validate_document(document: &BookDocument) -> Result<(), InvalidBook> {
    if document.schema != BOOK_SCHEMA
        || !valid_book_id(&document.id)
        || document.ordinal == 0
        || document.title.is_empty()
        || document.title.len() > MAX_TITLE_BYTES
        || contains_forbidden_control(&document.title)
        || document.language != BOOK_LANGUAGE
        || document.chapters.is_empty()
        || document.chapters.len() > MAX_CHAPTERS
        || document.raw_usfm.is_empty()
        || document.raw_usfm.len() > zion_corpus::MAX_ENTRY_BYTES
        || !document.raw_usfm.starts_with('\u{feff}')
        || document.normalization.is_empty()
        || document.normalization.len() > MAX_SOURCE_TEXT_BYTES
    {
        return Err(InvalidBook);
    }

    validate_headers(&document.headers)?;
    validate_source(document)?;

    let mut total_markers = document.headers.len();
    for (index, chapter) in document.chapters.iter().enumerate() {
        if usize::from(chapter.number) != index + 1 || chapter.markers.is_empty() {
            return Err(InvalidBook);
        }
        total_markers = total_markers
            .checked_add(chapter.markers.len())
            .ok_or(InvalidBook)?;
        if total_markers > zion_corpus::MAX_MARKERS_PER_BOOK {
            return Err(InvalidBook);
        }
        validate_body_markers(&chapter.markers)?;
    }

    Ok(())
}

fn valid_book_id(id: &str) -> bool {
    id.len() == MAX_BOOK_ID_BYTES
        && id
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

fn contains_forbidden_control(value: &str) -> bool {
    value
        .chars()
        .any(|character| character == '\0' || (character.is_control() && character != '\n'))
}

fn validate_headers(headers: &[MarkerRecord]) -> Result<(), InvalidBook> {
    let markers: Vec<Marker> = headers.iter().map(|record| record.marker).collect();
    let expected_mt = [
        Marker::Id,
        Marker::Ide,
        Marker::Header,
        Marker::Toc1,
        Marker::Toc2,
        Marker::Toc3,
        Marker::MainTitle,
    ];
    let expected_mt1 = [
        Marker::Id,
        Marker::Ide,
        Marker::Header,
        Marker::Toc1,
        Marker::Toc2,
        Marker::Toc3,
        Marker::MainTitle1,
    ];
    if markers.as_slice() != expected_mt && markers.as_slice() != expected_mt1 {
        return Err(InvalidBook);
    }
    if headers.iter().any(|record| {
        record.number.is_some()
            || record.text.len() > zion_corpus::MAX_ENTRY_BYTES
            || contains_forbidden_control(&record.text)
    }) {
        return Err(InvalidBook);
    }
    Ok(())
}

fn validate_body_markers(markers: &[MarkerRecord]) -> Result<(), InvalidBook> {
    let mut addition = false;
    let mut footnote = false;
    let mut reference_quote = false;
    let mut last_verse = 0_u16;

    for record in markers {
        if record.text.len() > zion_corpus::MAX_ENTRY_BYTES
            || contains_forbidden_control(&record.text)
        {
            return Err(InvalidBook);
        }
        match record.marker {
            Marker::Id
            | Marker::Ide
            | Marker::Header
            | Marker::Toc1
            | Marker::Toc2
            | Marker::Toc3
            | Marker::MainTitle
            | Marker::MainTitle1 => return Err(InvalidBook),
            Marker::Verse => {
                let number = record.number.ok_or(InvalidBook)?;
                if number == 0 || number <= last_verse {
                    return Err(InvalidBook);
                }
                last_verse = number;
            }
            Marker::Addition if addition => return Err(InvalidBook),
            Marker::Addition => addition = true,
            Marker::AdditionEnd if !addition => return Err(InvalidBook),
            Marker::AdditionEnd => addition = false,
            Marker::Footnote if footnote => return Err(InvalidBook),
            Marker::Footnote => footnote = true,
            Marker::FootnoteEnd if !footnote => return Err(InvalidBook),
            Marker::FootnoteEnd => footnote = false,
            Marker::FootnoteReference | Marker::FootnoteQuote | Marker::FootnoteText
                if !footnote =>
            {
                return Err(InvalidBook);
            }
            Marker::ReferenceQuote if reference_quote => return Err(InvalidBook),
            Marker::ReferenceQuote => reference_quote = true,
            Marker::ReferenceQuoteEnd if !reference_quote => {
                return Err(InvalidBook);
            }
            Marker::ReferenceQuoteEnd => reference_quote = false,
            Marker::Paragraph
            | Marker::DescriptiveTitle
            | Marker::FootnoteReference
            | Marker::FootnoteQuote
            | Marker::FootnoteText => {}
        }
        if record.marker != Marker::Verse && record.number.is_some() {
            return Err(InvalidBook);
        }
    }

    if addition || footnote || reference_quote || last_verse == 0 {
        return Err(InvalidBook);
    }
    Ok(())
}

fn validate_source(document: &BookDocument) -> Result<(), InvalidBook> {
    let source = &document.source;
    let required = [
        source.name.as_str(),
        source.version.as_str(),
        source.version_date.as_str(),
        source.release_url.as_str(),
        source.asset_url.as_str(),
        source.tag_commit.as_str(),
        source.license.as_str(),
        source.license_url.as_str(),
    ];
    if source.authors.is_empty()
        || source.authors.len() > MAX_AUTHORS
        || required
            .iter()
            .any(|value| value.is_empty() || value.len() > MAX_SOURCE_TEXT_BYTES)
        || source.authors.iter().any(|author| {
            author.is_empty()
                || author.len() > MAX_TITLE_BYTES
                || contains_forbidden_control(author)
        })
    {
        return Err(InvalidBook);
    }
    Ok(())
}

fn zeroize_book_document(document: &mut BookDocument) {
    document.id.zeroize();
    document.title.zeroize();
    document.language.zeroize();
    zeroize_marker_records(&mut document.headers);
    for chapter in &mut document.chapters {
        zeroize_marker_records(&mut chapter.markers);
    }
    document.raw_usfm.zeroize();
    document.source.name.zeroize();
    document.source.version.zeroize();
    document.source.version_date.zeroize();
    document.source.release_url.zeroize();
    document.source.asset_url.zeroize();
    document.source.tag_commit.zeroize();
    document.source.license.zeroize();
    document.source.license_url.zeroize();
    for author in &mut document.source.authors {
        author.zeroize();
    }
    document.normalization.zeroize();
}

fn zeroize_marker_records(records: &mut [MarkerRecord]) {
    for record in records {
        record.text.zeroize();
    }
}

fn validate_jni_input_lengths(
    png_length: usize,
    passphrase_length: usize,
) -> Result<(), MobileError> {
    if png_length > MAX_JNI_PNG_BYTES {
        return Err(MobileError::InvalidImage);
    }
    if !(1..=MAX_JNI_PASSPHRASE_BYTES).contains(&passphrase_length) {
        return Err(MobileError::InvalidPassphrase);
    }
    Ok(())
}

fn mobile_error_contract(error: MobileError) -> (&'static JNIStr, &'static JNIStr) {
    match error {
        MobileError::InvalidImage => (
            jni_str!("java/lang/IllegalArgumentException"),
            jni_str!("ZION_INVALID_IMAGE"),
        ),
        MobileError::InvalidPassphrase => (
            jni_str!("java/lang/IllegalArgumentException"),
            jni_str!("ZION_INVALID_PASSPHRASE"),
        ),
        MobileError::RecoveryFailed => (
            jni_str!("java/lang/IllegalStateException"),
            jni_str!("ZION_RECOVERY_FAILED"),
        ),
    }
}

#[derive(Debug, Error)]
enum BridgeError {
    #[error("native bridge failure")]
    Jni(#[from] jni::errors::Error),
    #[error("{0}")]
    Mobile(#[from] MobileError),
}

struct SanitizedExceptionPolicy;

impl<T: Default> ErrorPolicy<T, BridgeError> for SanitizedExceptionPolicy {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        error: BridgeError,
    ) -> JniResult<T> {
        if env.exception_check() {
            return Ok(T::default());
        }
        let result = match error {
            BridgeError::Mobile(error) => {
                let (class, code) = mobile_error_contract(error);
                env.throw_new(class, code)
            }
            BridgeError::Jni(_) => env.throw_new(
                jni_str!("java/lang/IllegalStateException"),
                jni_str!("ZION_NATIVE_FAILURE"),
            ),
        };
        let _ = result;
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut Env<'unowned_env_local>,
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn Any + Send + 'static>,
    ) -> JniResult<T> {
        std::mem::forget(payload);
        if !env.exception_check() {
            let _ = env.throw_new(
                jni_str!("java/lang/IllegalStateException"),
                jni_str!("ZION_NATIVE_FAILURE"),
            );
        }
        Ok(T::default())
    }

    fn on_internal_jni_error<'unowned_env_local: 'native_method, 'native_method>(
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        _error: jni::errors::Error,
    ) -> T {
        T::default()
    }

    fn on_internal_panic<'unowned_env_local: 'native_method, 'native_method>(
        _captures: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn Any + Send + 'static>,
    ) -> T {
        std::mem::forget(payload);
        T::default()
    }
}

/// JNI boundary used by `org.zioncode.app.NativeBridge.decodeBookPng`.
///
/// `EnvUnowned::with_env` catches Rust panics before they can unwind across JNI.
#[jni_mangle("org.zioncode.app.NativeBridge", "decodeBookPng")]
#[must_use]
pub fn decode_book_png_jni<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _this: JObject<'local>,
    png_bytes: JByteArray<'local>,
    passphrase_bytes: JByteArray<'local>,
) -> JByteArray<'local> {
    unowned_env
        .with_env(|env| -> Result<_, BridgeError> {
            let png_length = png_bytes.len(env)?;
            let passphrase_length = passphrase_bytes.len(env)?;
            validate_jni_input_lengths(png_length, passphrase_length)?;
            let png_bytes = env.convert_byte_array(&png_bytes)?;
            let passphrase_bytes = Zeroizing::new(env.convert_byte_array(&passphrase_bytes)?);
            let json = decode_book_png(&png_bytes, &passphrase_bytes)?;
            Ok(env.byte_array_from_slice(&json)?)
        })
        .resolve::<SanitizedExceptionPolicy>()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use zion_arc::{EccProfile, ItemMetadata, SealConfig, seal_png_bytes};
    use zion_corpus::{BookDocument, Marker};
    use zion_stego::{RgbImage, save_png_rgb};

    use super::*;

    const TEST_PASSPHRASE: &[u8] = b"host-only synthetic test passphrase";
    const BOOK_DOCUMENT_CONTRACT: &[u8] =
        include_bytes!("../../testdata/contracts/book_document_v1.json");
    const EXPECTED_RAW_USFM: &str = "\u{feff}\\id TST\r\n\\ide UTF-8\r\n\\h Guia de Horta\r\n\\toc1 Guia de Horta\r\n\\toc2 Guia de Horta\r\n\\toc3 TST\r\n\\mt Guia de Horta\r\n\\c 1\r\n\\p\r\n\\d Preparação\r\n\\v 1 Prepare o vaso.\\add com cuidado\\add*\r\n\\v 2 Registre o resultado.\r\n\\c 2\r\n\\d Revisão\r\n\\p\r\n\\v 1 Observe o crescimento.\\rq consulte a ficha\\rq*\r\n";

    fn synthetic_document() -> BookDocument {
        serde_json::from_slice(BOOK_DOCUMENT_CONTRACT)
            .expect("shared BookDocument contract must deserialize")
    }

    fn synthetic_cover() -> Vec<u8> {
        let image = RgbImage {
            width: 128,
            height: 128,
            rgb_data: (0_usize..128 * 128 * 3)
                .map(|index| {
                    u8::try_from((index * 37 + 19) % 256)
                        .expect("value reduced modulo 256 must fit in u8")
                })
                .collect(),
        };
        let mut png = Vec::new();
        save_png_rgb(&image, Cursor::new(&mut png)).expect("encode synthetic cover");
        png
    }

    fn seal_fixture(content: &[u8], media_type: &str) -> Vec<u8> {
        seal_png_bytes(
            content,
            &synthetic_cover(),
            TEST_PASSPHRASE,
            &ItemMetadata::new("fixture", media_type, "host-only synthetic fixture"),
            SealConfig::new(),
        )
        .expect("seal synthetic fixture")
        .png_bytes
    }

    fn assert_production_recovery_contract(result: Result<Zeroizing<Vec<u8>>, MobileError>) {
        let error = result.expect_err("fixture must fail closed");
        assert_eq!(error, MobileError::RecoveryFailed);
        assert_eq!(error.to_string(), "capsule recovery failed");
        let (class, code) = mobile_error_contract(error);
        assert_eq!(class.to_string(), "java/lang/IllegalStateException");
        assert_eq!(code.to_string(), "ZION_RECOVERY_FAILED");
    }

    #[test]
    fn synthetic_arc_book_round_trips_through_mobile_decoder() {
        let document = synthetic_document();
        let json = serde_json::to_vec(&document).expect("serialize fixture");
        let output = seal_png_bytes(
            &json,
            &synthetic_cover(),
            TEST_PASSPHRASE,
            &ItemMetadata::new(
                "Synthetic garden manual",
                BOOK_MEDIA_TYPE,
                "Host-only synthetic fixture",
            ),
            SealConfig::with_profile(EccProfile::Safe),
        )
        .expect("seal synthetic fixture");

        let recovered =
            decode_book_png(&output.png_bytes, TEST_PASSPHRASE).expect("open synthetic fixture");
        assert_eq!(recovered.as_slice(), json);
        let decoded: BookDocument = serde_json::from_slice(&recovered).expect("decode JSON");
        assert_eq!(decoded.raw_usfm.as_bytes(), document.raw_usfm.as_bytes());
    }

    #[test]
    fn shared_book_document_contract_matches_rust_shape_and_mobile_validation() {
        let document: BookDocument = serde_json::from_slice(BOOK_DOCUMENT_CONTRACT)
            .expect("shared fixture must match zion_corpus::BookDocument");

        assert_eq!(document.id, "TST");
        assert_eq!(document.ordinal, 7);
        assert_eq!(document.title, "Guia de Horta");
        assert_eq!(document.source.name, "Coleção Sintética");
        assert_eq!(document.source.version, "2026.08-contract");
        assert_eq!(document.chapters.len(), 2);
        assert_eq!(document.chapters[0].markers[0].marker, Marker::Paragraph);
        assert_eq!(
            document.chapters[0].markers[1].marker,
            Marker::DescriptiveTitle
        );
        assert_eq!(document.chapters[0].markers[1].text, "Preparação");
        assert_eq!(document.chapters[0].markers[2].marker, Marker::Verse);
        assert_eq!(document.chapters[0].markers[2].number, Some(1));
        assert_eq!(document.chapters[0].markers[2].text, "Prepare o vaso.");
        assert_eq!(document.chapters[0].markers[3].marker, Marker::Addition);
        assert_eq!(document.chapters[0].markers[3].text, "com cuidado");
        assert_eq!(document.chapters[0].markers[4].marker, Marker::AdditionEnd);
        assert!(document.chapters[0].markers[4].text.is_empty());
        assert_eq!(
            document.chapters[1].markers[3].marker,
            Marker::ReferenceQuote
        );
        assert_eq!(document.chapters[1].markers[3].text, "consulte a ficha");
        assert_eq!(
            document.chapters[1].markers[4].marker,
            Marker::ReferenceQuoteEnd
        );
        assert!(document.chapters[1].markers[4].text.is_empty());
        assert_eq!(document.raw_usfm, EXPECTED_RAW_USFM);
        assert_eq!(document.raw_usfm.as_bytes()[..3], [0xef, 0xbb, 0xbf]);
        assert!(!document.raw_usfm.replace("\r\n", "").contains('\n'));

        let canonical = serde_json::to_vec(&document).expect("canonical serde serialization");
        let validated = validate_and_reserialize_book_json(BOOK_DOCUMENT_CONTRACT)
            .expect("shared fixture must pass the production structural validator");
        assert_eq!(validated.as_slice(), canonical);
        let reparsed: BookDocument =
            serde_json::from_slice(&canonical).expect("canonical JSON must deserialize");
        assert_eq!(reparsed, document);
    }

    #[test]
    fn authenticated_invalid_payloads_and_wrong_password_share_recovery_contract() {
        let valid_json = serde_json::to_vec(&synthetic_document()).expect("serialize fixture");
        let valid_png = seal_fixture(&valid_json, BOOK_MEDIA_TYPE);
        let malformed_png = seal_fixture(b"{not-json", BOOK_MEDIA_TYPE);
        let wrong_media_png = seal_fixture(&valid_json, "application/json");

        let mut invalid_document = synthetic_document();
        invalid_document.chapters[0].number = 2;
        let invalid_json =
            serde_json::to_vec(&invalid_document).expect("serialize invalid fixture");
        let invalid_structure_png = seal_fixture(&invalid_json, BOOK_MEDIA_TYPE);

        assert_production_recovery_contract(decode_book_png(
            &valid_png,
            b"incorrect test passphrase",
        ));
        assert_production_recovery_contract(decode_book_png(&malformed_png, TEST_PASSPHRASE));
        assert_production_recovery_contract(decode_book_png(&wrong_media_png, TEST_PASSPHRASE));
        assert_production_recovery_contract(decode_book_png(
            &invalid_structure_png,
            TEST_PASSPHRASE,
        ));
    }

    #[test]
    fn structurally_invalid_book_diagnostics_remain_internal() {
        let mut document = synthetic_document();
        document.chapters[0].number = 2;
        let json = serde_json::to_vec(&document).expect("serialize fixture");
        assert_eq!(
            validate_and_reserialize_book_json(&json).map(|_| ()),
            Err(InvalidBook)
        );
    }

    #[test]
    fn document_zeroizer_clears_all_owned_strings() {
        let mut document = synthetic_document();
        zeroize_book_document(&mut document);

        assert!(document.id.is_empty());
        assert!(document.title.is_empty());
        assert!(document.language.is_empty());
        assert!(document.raw_usfm.is_empty());
        assert!(document.normalization.is_empty());
        assert!(document.headers.iter().all(|record| record.text.is_empty()));
        assert!(
            document
                .chapters
                .iter()
                .flat_map(|chapter| &chapter.markers)
                .all(|record| record.text.is_empty())
        );
        assert!(document.source.name.is_empty());
        assert!(document.source.version.is_empty());
        assert!(document.source.version_date.is_empty());
        assert!(document.source.release_url.is_empty());
        assert!(document.source.asset_url.is_empty());
        assert!(document.source.tag_commit.is_empty());
        assert!(document.source.license.is_empty());
        assert!(document.source.license_url.is_empty());
        assert!(document.source.authors.iter().all(String::is_empty));
    }

    #[test]
    fn jni_png_length_boundary_is_checked_before_copy() {
        assert_eq!(validate_jni_input_lengths(0, 1), Ok(()));
        assert_eq!(validate_jni_input_lengths(MAX_JNI_PNG_BYTES, 1), Ok(()));
        assert_eq!(
            validate_jni_input_lengths(MAX_JNI_PNG_BYTES + 1, 1),
            Err(MobileError::InvalidImage)
        );
    }

    #[test]
    fn jni_passphrase_length_boundary_is_checked_before_copy() {
        assert_eq!(
            validate_jni_input_lengths(1, 0),
            Err(MobileError::InvalidPassphrase)
        );
        assert_eq!(validate_jni_input_lengths(1, 1), Ok(()));
        assert_eq!(
            validate_jni_input_lengths(1, MAX_JNI_PASSPHRASE_BYTES),
            Ok(())
        );
        assert_eq!(
            validate_jni_input_lengths(1, MAX_JNI_PASSPHRASE_BYTES + 1),
            Err(MobileError::InvalidPassphrase)
        );
    }
}
