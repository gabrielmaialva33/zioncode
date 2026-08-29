use std::{
    collections::HashSet,
    fs::{self, DirBuilder, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;

use crate::{
    archive::{import_verified_archive, verify_pinned_zip},
    canonical::canonical_books,
    constants::PINNED_ASSET_NAME,
    error::CorpusError,
    model::{CanonicalBook, CorpusManifest, CorpusPackage, SourceMetadata, SourceZipIdentity},
};

const UPSTREAM_README: &[u8] = include_bytes!("../data/provenance/UPSTREAM_README.md");
const UPSTREAM_LICENSE: &[u8] = include_bytes!("../data/provenance/UPSTREAM_LICENSE.md");
const STAGING_ATTEMPTS: usize = 16;

#[derive(Serialize)]
struct ProvenanceDocument<'a> {
    schema: u8,
    source: &'a SourceMetadata,
    source_zip: &'a SourceZipIdentity,
    normalization_notice: &'a str,
}

#[derive(Default)]
struct MaterializationControl {
    fail_after_files: Option<usize>,
    files_written: usize,
}

impl MaterializationControl {
    fn before_file(&mut self) -> Result<(), CorpusError> {
        if self.fail_after_files == Some(self.files_written) {
            return Err(CorpusError::Io(std::io::Error::other(
                "injected materialization failure",
            )));
        }
        self.files_written = self
            .files_written
            .checked_add(1)
            .ok_or_else(|| invalid_package("materialized file count overflow"))?;
        Ok(())
    }
}

impl CorpusPackage {
    /// Materializes the deterministic package in a new directory.
    ///
    /// Before creating any filesystem entry, this revalidates the retained ZIP
    /// against the pinned release and reconstructs the complete canonical
    /// package to verify every manifest, artifact, payload, hash, and path.
    /// Files are prepared in a private sibling directory and published with an
    /// atomic no-replace rename. Unsupported platforms fail closed after
    /// validation but before any filesystem entry is queried or created.
    ///
    /// Normalized payloads live in `books/`. The immutable ZIP, exact upstream
    /// notices, and byte-exact raw USFM live separately under `provenance/`.
    ///
    /// # Errors
    ///
    /// Returns source or canonical validation errors for an untrusted retained
    /// ZIP, or [`CorpusError::InvalidPackage`] when package layers disagree.
    /// Returns [`CorpusError::OutputExists`] if the destination exists and
    /// [`CorpusError::Io`] with `Unsupported` when atomic no-replace publication
    /// is unavailable. Other typed I/O, serialization, parsing, or compression
    /// errors are returned if validation or materialization cannot complete.
    pub fn write_to_directory(&self, output: impl AsRef<Path>) -> Result<(), CorpusError> {
        let canonical = canonical_books()?;
        verify_pinned_zip(self.source_zip())?;
        self.write_transaction(
            output.as_ref(),
            &canonical,
            self.source_zip(),
            true,
            MaterializationControl::default(),
            platform_atomic_publication_capability(),
        )
    }

    #[cfg(test)]
    pub(crate) fn write_to_directory_for_test(
        &self,
        output: impl AsRef<Path>,
        canonical: &[CanonicalBook],
        expected_zip: &[u8],
        fail_after_files: Option<usize>,
    ) -> Result<(), CorpusError> {
        self.write_transaction(
            output.as_ref(),
            canonical,
            expected_zip,
            false,
            MaterializationControl {
                fail_after_files,
                files_written: 0,
            },
            platform_atomic_publication_capability(),
        )
    }

    #[cfg(test)]
    fn write_to_directory_with_unsupported_publication_for_test(
        &self,
        output: impl AsRef<Path>,
        canonical: &[CanonicalBook],
        expected_zip: &[u8],
    ) -> Result<(), CorpusError> {
        self.write_transaction(
            output.as_ref(),
            canonical,
            expected_zip,
            false,
            MaterializationControl::default(),
            AtomicPublicationCapability::Unsupported,
        )
    }

    fn write_transaction(
        &self,
        output: &Path,
        canonical: &[CanonicalBook],
        expected_zip: &[u8],
        enforce_pinned_totals: bool,
        mut control: MaterializationControl,
        publication_capability: AtomicPublicationCapability,
    ) -> Result<(), CorpusError> {
        let provenance_json =
            self.validate_for_materialization(canonical, expected_zip, enforce_pinned_totals)?;
        ensure_atomic_publication_supported(publication_capability)?;
        if path_entry_exists(output)? {
            return Err(CorpusError::OutputExists(output.to_path_buf()));
        }

        let mut staging = StagingDirectory::create_for(output)?;
        if let Err(error) = self.write_staged(staging.path(), &provenance_json, &mut control) {
            return staging.abort(error);
        }
        staging.publish(output)
    }

    fn write_staged(
        &self,
        output: &Path,
        provenance_json: &[u8],
        control: &mut MaterializationControl,
    ) -> Result<(), CorpusError> {
        let books_directory = output.join("books");
        let provenance_directory = output.join("provenance");
        let raw_directory = provenance_directory.join("raw-usfm");

        create_private_directory(&books_directory)?;
        create_private_directory(&provenance_directory)?;
        create_private_directory(&raw_directory)?;

        write_controlled(&output.join("manifest.json"), self.manifest_json(), control)?;
        write_controlled(
            &provenance_directory.join(PINNED_ASSET_NAME),
            self.source_zip(),
            control,
        )?;
        write_controlled(
            &provenance_directory.join("UPSTREAM_README.md"),
            UPSTREAM_README,
            control,
        )?;
        write_controlled(
            &provenance_directory.join("UPSTREAM_LICENSE.md"),
            UPSTREAM_LICENSE,
            control,
        )?;
        write_controlled(
            &provenance_directory.join("SOURCE.json"),
            provenance_json,
            control,
        )?;

        for book in self.books() {
            let payload_name = format!("{:02}-{}.json", book.canonical.ordinal, book.canonical.id);
            write_controlled(
                &books_directory.join(payload_name),
                &book.payload_json,
                control,
            )?;
            write_controlled(
                &raw_directory.join(&book.canonical.source_file),
                &book.raw_usfm,
                control,
            )?;
        }
        Ok(())
    }

    fn validate_for_materialization(
        &self,
        canonical: &[CanonicalBook],
        expected_zip: &[u8],
        enforce_pinned_totals: bool,
    ) -> Result<Vec<u8>, CorpusError> {
        validate_manifest_paths(self.manifest())?;
        validate_artifact_names(self.books())?;
        if self.source_zip() != expected_zip {
            return Err(invalid_package(
                "retained source ZIP differs from its trusted identity",
            ));
        }

        let expected = import_verified_archive(expected_zip, canonical, enforce_pinned_totals)?;
        if self.manifest() != expected.manifest() {
            return Err(invalid_package(
                "manifest differs from the verified canonical package",
            ));
        }
        if self.manifest_json() != expected.manifest_json() {
            return Err(invalid_package(
                "serialized manifest differs from the verified canonical manifest",
            ));
        }
        if self.books() != expected.books() {
            return Err(invalid_package(
                "book artifacts differ from the verified canonical package",
            ));
        }

        serde_json::to_vec(&ProvenanceDocument {
            schema: 1,
            source: &expected.manifest().source,
            source_zip: &expected.manifest().source_zip,
            normalization_notice: &expected.manifest().provenance.normalization_notice,
        })
        .map_err(CorpusError::from)
    }
}

fn validate_manifest_paths(manifest: &CorpusManifest) -> Result<(), CorpusError> {
    validate_safe_leaf(&manifest.source_zip.file, "source ZIP filename")?;
    let mut paths = HashSet::new();
    for (path, label) in [
        (
            &manifest.provenance.source_zip,
            "provenance source ZIP path",
        ),
        (&manifest.provenance.upstream_readme, "upstream README path"),
        (
            &manifest.provenance.upstream_license,
            "upstream license path",
        ),
        (
            &manifest.provenance.raw_usfm_directory,
            "raw USFM directory",
        ),
    ] {
        validate_safe_relative_path(path, label)?;
        if !paths.insert(path.as_str()) {
            return Err(invalid_package(format!("duplicate package path `{path}`")));
        }
    }

    let mut ids = HashSet::new();
    let mut source_files = HashSet::new();
    for book in &manifest.books {
        validate_safe_leaf(&book.id, "manifest book identifier")?;
        validate_safe_leaf(&book.source_file, "manifest source filename")?;
        validate_safe_relative_path(&book.payload_file, "manifest payload path")?;
        validate_safe_relative_path(&book.raw_usfm_file, "manifest raw USFM path")?;
        if !ids.insert(book.id.as_str()) {
            return Err(invalid_package(format!(
                "duplicate manifest book identifier `{}`",
                book.id
            )));
        }
        if !source_files.insert(book.source_file.as_str()) {
            return Err(invalid_package(format!(
                "duplicate manifest source filename `{}`",
                book.source_file
            )));
        }
        for path in [&book.payload_file, &book.raw_usfm_file] {
            if !paths.insert(path.as_str()) {
                return Err(invalid_package(format!("duplicate package path `{path}`")));
            }
        }
    }
    Ok(())
}

fn validate_artifact_names(books: &[crate::model::BookArtifact]) -> Result<(), CorpusError> {
    let mut ids = HashSet::new();
    let mut source_files = HashSet::new();
    for book in books {
        validate_safe_leaf(&book.canonical.id, "artifact book identifier")?;
        validate_safe_leaf(&book.canonical.source_file, "artifact source filename")?;
        if !ids.insert(book.canonical.id.as_str()) {
            return Err(invalid_package(format!(
                "duplicate artifact book identifier `{}`",
                book.canonical.id
            )));
        }
        if !source_files.insert(book.canonical.source_file.as_str()) {
            return Err(invalid_package(format!(
                "duplicate artifact source filename `{}`",
                book.canonical.source_file
            )));
        }
    }
    Ok(())
}

fn validate_safe_leaf(value: &str, label: &str) -> Result<(), CorpusError> {
    let mut components = Path::new(value).components();
    if value.is_empty()
        || value.contains(['/', '\\', '\0'])
        || matches!(value, "." | "..")
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(invalid_package(format!("unsafe {label} `{value}`")));
    }
    Ok(())
}

fn validate_safe_relative_path(value: &str, label: &str) -> Result<(), CorpusError> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains(['\\', '\0'])
        || path.is_absolute()
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_package(format!("unsafe {label} `{value}`")));
    }
    Ok(())
}

fn write_controlled(
    path: &Path,
    bytes: &[u8],
    control: &mut MaterializationControl,
) -> Result<(), CorpusError> {
    control.before_file()?;
    write_private_file(path, bytes)?;
    Ok(())
}

struct StagingDirectory {
    path: PathBuf,
    active: bool,
}

impl StagingDirectory {
    fn create_for(destination: &Path) -> Result<Self, CorpusError> {
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));
        for _ in 0..STAGING_ATTEMPTS {
            let mut random = [0_u8; 8];
            getrandom::fill(&mut random).map_err(|error| {
                CorpusError::Io(std::io::Error::other(format!(
                    "generating private staging name failed: {error}"
                )))
            })?;
            let path = parent.join(format!(".zion-corpus-staging-{}", hex::encode(random)));
            match create_private_directory(&path) {
                Ok(()) => {
                    return Ok(Self { path, active: true });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(CorpusError::Io(error)),
            }
        }
        Err(CorpusError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique private corpus staging directory",
        )))
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn abort<T>(&mut self, original: CorpusError) -> Result<T, CorpusError> {
        match fs::remove_dir_all(&self.path) {
            Ok(()) => {
                self.active = false;
                Err(original)
            }
            Err(cleanup) if cleanup.kind() == std::io::ErrorKind::NotFound => {
                self.active = false;
                Err(original)
            }
            Err(cleanup) => Err(CorpusError::Io(std::io::Error::new(
                cleanup.kind(),
                format!("{original}; staging cleanup also failed: {cleanup}"),
            ))),
        }
    }

    fn publish(mut self, destination: &Path) -> Result<(), CorpusError> {
        match rename_directory_noreplace(&self.path, destination) {
            Ok(()) => {
                self.active = false;
                Ok(())
            }
            Err(error) => self.abort(error),
        }
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Copy)]
enum AtomicPublicationCapability {
    Supported,
    Unsupported,
}

const fn platform_atomic_publication_capability() -> AtomicPublicationCapability {
    if cfg!(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple"
    )) {
        AtomicPublicationCapability::Supported
    } else {
        AtomicPublicationCapability::Unsupported
    }
}

fn ensure_atomic_publication_supported(
    capability: AtomicPublicationCapability,
) -> Result<(), CorpusError> {
    match capability {
        AtomicPublicationCapability::Supported => Ok(()),
        AtomicPublicationCapability::Unsupported => Err(unsupported_publication_error()),
    }
}

fn unsupported_publication_error() -> CorpusError {
    CorpusError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace corpus publication is unsupported on this platform",
    ))
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn rename_directory_noreplace(source: &Path, destination: &Path) -> Result<(), CorpusError> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE).map_err(|error| {
        if error == rustix::io::Errno::EXIST {
            CorpusError::OutputExists(destination.to_path_buf())
        } else {
            CorpusError::Io(std::io::Error::from(error))
        }
    })
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn rename_directory_noreplace(_source: &Path, _destination: &Path) -> Result<(), CorpusError> {
    Err(unsupported_publication_error())
}

fn path_entry_exists(path: &Path) -> Result<bool, CorpusError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CorpusError::Io(error)),
    }
}

fn invalid_package(detail: impl Into<String>) -> CorpusError {
    CorpusError::InvalidPackage(detail.into())
}

fn create_private_directory(path: &Path) -> Result<(), std::io::Error> {
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        builder.mode(0o700);
    }
    builder.create(path)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.flush()
}

/// Exact README bytes from upstream tag `2018.2.0`.
#[must_use]
pub const fn upstream_readme_bytes() -> &'static [u8] {
    UPSTREAM_README
}

/// Exact CC BY 3.0 Brazil license bytes from upstream tag `2018.2.0`.
#[must_use]
pub const fn upstream_license_bytes() -> &'static [u8] {
    UPSTREAM_LICENSE
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

    use crate::constants::{PINNED_LICENSE_SHA256, PINNED_README_SHA256};

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
            raw_sha256: crate::archive::sha256_hex(raw),
        }
    }

    fn zip_fixture(raw: &[u8]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        writer
            .start_file("test.txt", options)
            .expect("start fixture");
        writer.write_all(raw).expect("write fixture");
        writer.finish().expect("finish fixture").into_inner()
    }

    fn fixture_package() -> (CanonicalBook, Vec<u8>, CorpusPackage) {
        let raw = raw_fixture();
        let book = fixture_book(&raw);
        let zip = zip_fixture(&raw);
        let package = import_verified_archive(&zip, std::slice::from_ref(&book), false)
            .expect("fixture imports");
        (book, zip, package)
    }

    fn assert_no_staging(parent: &Path) {
        let residue: Vec<_> = fs::read_dir(parent)
            .expect("enumerate fixture parent")
            .map(|entry| entry.expect("read fixture entry").file_name())
            .filter(|name| name.to_string_lossy().starts_with(".zion-corpus-staging-"))
            .collect();
        assert!(residue.is_empty(), "staging residue: {residue:?}");
    }

    fn assert_rejected_without_output(
        temporary: &TempDir,
        package: &CorpusPackage,
        book: &CanonicalBook,
        zip: &[u8],
    ) {
        let output = temporary.path().join("corpus");
        assert!(matches!(
            package.write_to_directory_for_test(&output, std::slice::from_ref(book), zip, None,),
            Err(CorpusError::InvalidPackage(_))
        ));
        assert!(!output.exists());
        assert_no_staging(temporary.path());
    }

    #[test]
    fn checked_in_upstream_notices_are_distinct_and_attributed() {
        assert!(
            std::str::from_utf8(upstream_readme_bytes())
                .expect("README is UTF-8")
                .contains("Diego Santos, Mario Sérgio, e Marco Teles")
        );
        assert!(
            std::str::from_utf8(upstream_license_bytes())
                .expect("license is UTF-8")
                .contains("Creative Commons Atribuição 3.0 Brasil")
        );
        assert_ne!(upstream_readme_bytes(), upstream_license_bytes());
        assert_eq!(
            hex::encode(Sha256::digest(upstream_readme_bytes())),
            PINNED_README_SHA256
        );
        assert_eq!(
            hex::encode(Sha256::digest(upstream_license_bytes())),
            PINNED_LICENSE_SHA256
        );
    }

    #[test]
    fn unsafe_manifest_paths_are_rejected_before_filesystem_creation() {
        for unsafe_path in [
            "../escaped.json",
            "/absolute/escaped.json",
            "books/./escaped.json",
            "books/../escaped.json",
            "books\\escaped.json",
        ] {
            let temporary = tempfile::tempdir().expect("temporary directory");
            let (book, zip, mut package) = fixture_package();
            package.manifest_mut_for_test().books[0].payload_file = unsafe_path.to_owned();
            assert_rejected_without_output(&temporary, &package, &book, &zip);
            assert!(!temporary.path().join("escaped.json").exists());
        }
    }

    #[test]
    fn production_materializer_rejects_unpinned_source_before_creation() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("corpus");
        let (_, _, package) = fixture_package();
        assert!(matches!(
            package.write_to_directory(&output),
            Err(CorpusError::SourceZipLength { .. })
        ));
        assert!(!output.exists());
        assert_no_staging(temporary.path());
    }

    #[test]
    fn unsupported_publication_is_rejected_after_validation_before_filesystem_access() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("corpus");
        let (book, zip, package) = fixture_package();
        let error = package
            .write_to_directory_with_unsupported_publication_for_test(
                &output,
                std::slice::from_ref(&book),
                &zip,
            )
            .expect_err("unsupported publication must fail closed");
        assert!(matches!(
            error,
            CorpusError::Io(ref source) if source.kind() == std::io::ErrorKind::Unsupported
        ));
        assert!(!output.exists());
        assert_no_staging(temporary.path());

        let occupied = temporary.path().join("occupied");
        fs::create_dir(&occupied).expect("create occupied destination");
        fs::write(occupied.join("sentinel"), b"keep").expect("write sentinel");
        let error = package
            .write_to_directory_with_unsupported_publication_for_test(
                &occupied,
                std::slice::from_ref(&book),
                &zip,
            )
            .expect_err("capability check must precede destination lookup");
        assert!(matches!(
            error,
            CorpusError::Io(ref source) if source.kind() == std::io::ErrorKind::Unsupported
        ));
        assert_eq!(
            fs::read(occupied.join("sentinel")).expect("read sentinel"),
            b"keep"
        );
        assert_no_staging(temporary.path());

        let invalid_output = temporary.path().join("invalid");
        let (_, _, mut invalid_package) = fixture_package();
        invalid_package.manifest_mut_for_test().books[0].payload_file = "../escape".to_owned();
        assert!(matches!(
            invalid_package.write_to_directory_with_unsupported_publication_for_test(
                &invalid_output,
                std::slice::from_ref(&book),
                &zip,
            ),
            Err(CorpusError::InvalidPackage(_))
        ));
        assert!(!invalid_output.exists());
        assert_no_staging(temporary.path());
    }

    #[test]
    fn absolute_and_traversing_provenance_paths_cannot_escape() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let escaped = temporary.path().join("escaped.zip");
        let (book, zip, mut package) = fixture_package();
        package.manifest_mut_for_test().provenance.source_zip =
            escaped.to_string_lossy().into_owned();
        assert_rejected_without_output(&temporary, &package, &book, &zip);
        assert!(!escaped.exists());

        let (book, zip, mut package) = fixture_package();
        package.manifest_mut_for_test().provenance.upstream_readme =
            "provenance/../../escaped.md".to_owned();
        assert_rejected_without_output(&temporary, &package, &book, &zip);
        assert!(!temporary.path().join("escaped.md").exists());
    }

    #[test]
    fn mutated_manifest_artifacts_and_source_zip_are_rejected() {
        type PackageMutation = Box<dyn FnOnce(&mut CorpusPackage)>;

        let mutations: Vec<PackageMutation> = vec![
            Box::new(|package| package.manifest_mut_for_test().totals.books = 2),
            Box::new(|package| package.books_mut_for_test()[0].payload_json.push(b' ')),
            Box::new(|package| package.books_mut_for_test()[0].canonical.id = "BAD".to_owned()),
            Box::new(|package| {
                package.books_mut_for_test()[0].canonical.source_file = "other.txt".to_owned();
            }),
            Box::new(|package| package.source_zip_mut_for_test()[0] ^= 1),
            Box::new(|package| package.manifest_json_mut_for_test().push(b' ')),
        ];

        for mutate in mutations {
            let temporary = tempfile::tempdir().expect("temporary directory");
            let (book, zip, mut package) = fixture_package();
            mutate(&mut package);
            assert_rejected_without_output(&temporary, &package, &book, &zip);
        }
    }

    #[test]
    fn duplicate_and_cross_layer_inconsistent_books_are_rejected() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let (book, zip, mut package) = fixture_package();
        let duplicate = package.manifest().books[0].clone();
        package.manifest_mut_for_test().books.push(duplicate);
        assert_rejected_without_output(&temporary, &package, &book, &zip);

        let (book, zip, mut package) = fixture_package();
        let duplicate = package.books()[0].clone();
        package.books_mut_for_test().push(duplicate);
        assert_rejected_without_output(&temporary, &package, &book, &zip);

        let (book, zip, mut package) = fixture_package();
        package.manifest_mut_for_test().books[0].title = "Different title".to_owned();
        assert_rejected_without_output(&temporary, &package, &book, &zip);
    }

    #[test]
    fn injected_mid_write_failure_leaves_no_partial_or_staging_residue() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let output = temporary.path().join("corpus");
        let sibling = temporary.path().join("sentinel");
        fs::write(&sibling, b"keep").expect("write sibling sentinel");
        let (book, zip, package) = fixture_package();

        assert!(matches!(
            package.write_to_directory_for_test(
                &output,
                std::slice::from_ref(&book),
                &zip,
                Some(3),
            ),
            Err(CorpusError::Io(_))
        ));
        assert!(!output.exists());
        assert_eq!(fs::read(&sibling).expect("read sibling sentinel"), b"keep");
        assert_no_staging(temporary.path());
    }

    #[test]
    fn existing_destination_and_symlink_are_never_replaced() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let (book, zip, package) = fixture_package();
        let output = temporary.path().join("corpus");
        fs::create_dir(&output).expect("create destination");
        fs::write(output.join("sentinel"), b"keep").expect("write sentinel");
        assert!(matches!(
            package.write_to_directory_for_test(&output, std::slice::from_ref(&book), &zip, None,),
            Err(CorpusError::OutputExists(_))
        ));
        assert_eq!(
            fs::read(output.join("sentinel")).expect("read sentinel"),
            b"keep"
        );
        assert_no_staging(temporary.path());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = temporary.path().join("target");
            fs::create_dir(&target).expect("create symlink target");
            fs::write(target.join("sentinel"), b"target-keep").expect("write target sentinel");
            let link = temporary.path().join("corpus-link");
            symlink(&target, &link).expect("create destination symlink");
            assert!(matches!(
                package
                    .write_to_directory_for_test(&link, std::slice::from_ref(&book), &zip, None,),
                Err(CorpusError::OutputExists(_))
            ));
            assert_eq!(
                fs::read(target.join("sentinel")).expect("read target sentinel"),
                b"target-keep"
            );
            assert!(link.is_symlink());
            assert_no_staging(temporary.path());
        }
    }

    #[test]
    fn atomic_publish_noreplace_preserves_racing_destination() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let destination = temporary.path().join("corpus");
        let staging = StagingDirectory::create_for(&destination).expect("create staging");
        let staging_path = staging.path().to_owned();
        write_private_file(&staging_path.join("prepared"), b"prepared").expect("write staged file");
        fs::create_dir(&destination).expect("create racing destination");
        fs::write(destination.join("sentinel"), b"keep").expect("write sentinel");

        assert!(matches!(
            staging.publish(&destination),
            Err(CorpusError::OutputExists(_))
        ));
        assert_eq!(
            fs::read(destination.join("sentinel")).expect("read sentinel"),
            b"keep"
        );
        assert!(!staging_path.exists());
        assert_no_staging(temporary.path());
    }
}
