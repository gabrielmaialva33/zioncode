use std::{
    collections::BTreeSet,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};
use clap::{Args as ClapArgs, Subcommand, ValueEnum};
use zeroize::Zeroizing;
use zion_arc::{
    Capacity, CollectionKey, EccProfile, ItemMetadata, SealConfig, capacity,
    make_bounded_corruption_fixture, open_png_bytes, seal_png_bytes_with_collection_key,
};
use zion_corpus::{CorpusPackage, PINNED_ZIP_BYTES, import_pinned_blivre};

const MAX_ARC_CONTENT_BYTES: usize = 16_777_216;
const MAX_ARC_PNG_BYTES: usize = 134_217_728;
const MAX_ARC_PASSPHRASE_BYTES: usize = 1_024;
const MAX_ARC_ATTRIBUTION_BYTES: usize = 4_096;
const ENVELOPE_HEADER_BYTES: usize = 64;
const AEAD_TAG_BYTES: usize = 16;
const RS_CODEWORD_BYTES: usize = 255;
const MAX_BATCH_COVER_DIRECTORY_ENTRIES: usize = 96;

const BLIVRE_MEDIA_TYPE: &str = "application/vnd.zion.blivre+json";
const BLIVRE_ATTRIBUTION: &str = "Todas as Escrituras em português citadas são da Bíblia Livre (BLIVRE), Copyright © Diego Santos, Mario Sérgio, e Marco Teles, http://sites.google.com/site/biblialivre/ - fevereiro de 2018. Licença Creative Commons Atribuição 3.0 Brasil (https://creativecommons.org/licenses/by/3.0/br/). The normalized JSON retains byte-exact, exportable source USFM and its pinned provenance.";

#[derive(ClapArgs)]
pub struct Args {
    #[command(subcommand)]
    command: ArcCommand,
}

#[derive(Subcommand)]
enum ArcCommand {
    /// Report exact ARC v1 capacity for RGB8 carrier dimensions.
    Capacity(CapacityArgs),
    /// Seal one opaque content file into one exact RGB8 PNG.
    Seal(SealArgs),
    /// Open one ARC PNG and write the authenticated content bytes.
    Open(OpenArgs),
    /// Create a deliberately damaged fixture at the exact ECC correction bound.
    CorruptFixture(CorruptFixtureArgs),
    /// Import, analyze, or seal the pinned BLIVRE 2018.2.0 corpus.
    Blivre(BlivreArgs),
}

#[derive(ClapArgs)]
struct BlivreArgs {
    #[command(subcommand)]
    command: BlivreCommand,
}

#[derive(Subcommand)]
enum BlivreCommand {
    /// Verify and materialize the pinned corpus with separate provenance.
    Import(BlivreImportArgs),
    /// Report every normalized book's measured ARC fit.
    Analyze(BlivreAnalyzeArgs),
    /// Seal all 66 books with one cached collection key.
    SealBatch(BlivreSealBatchArgs),
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum ProfileArg {
    /// RS(255,223), correcting up to 16 unknown bytes per codeword.
    #[default]
    Safe,
    /// RS(255,239), correcting up to 8 unknown bytes per codeword.
    Balanced,
    /// RS(255,247), correcting up to 4 unknown bytes per codeword.
    Dense,
}

impl ProfileArg {
    const fn name(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Balanced => "balanced",
            Self::Dense => "dense",
        }
    }
}

impl From<ProfileArg> for EccProfile {
    fn from(profile: ProfileArg) -> Self {
        match profile {
            ProfileArg::Safe => Self::Safe,
            ProfileArg::Balanced => Self::Balanced,
            ProfileArg::Dense => Self::Dense,
        }
    }
}

#[derive(ClapArgs)]
struct PassphraseArgs {
    /// Read opaque passphrase bytes from a bounded owner-only file.
    ///
    /// On Unix, group/world-accessible files are rejected. One trailing
    /// CRLF, LF, or CR terminator is removed; all other bytes are preserved.
    #[arg(long, value_name = "FILE", conflicts_with = "prompt")]
    passphrase_file: Option<PathBuf>,

    /// Read the passphrase interactively without terminal echo.
    ///
    /// Interactive input is also the fallback when no passphrase file is set.
    #[arg(long, conflicts_with = "passphrase_file")]
    prompt: bool,
}

#[derive(ClapArgs)]
struct CapacityArgs {
    /// RGB8 carrier width in pixels.
    #[arg(long)]
    width: u32,
    /// RGB8 carrier height in pixels.
    #[arg(long)]
    height: u32,
    /// Reed-Solomon protection profile.
    #[arg(long, value_enum, default_value_t)]
    profile: ProfileArg,
    /// Optional already-compressed payload bytes to evaluate.
    #[arg(long)]
    payload_bytes: Option<usize>,
    /// UTF-8 item-name bytes used by the payload estimate.
    #[arg(long, default_value_t = 1)]
    name_bytes: usize,
    /// UTF-8 media-type bytes used by the payload estimate.
    #[arg(long, default_value_t = 1)]
    media_type_bytes: usize,
    /// UTF-8 attribution bytes used by the payload estimate.
    #[arg(long, default_value_t = 0)]
    attribution_bytes: usize,
}

#[derive(ClapArgs)]
struct SealArgs {
    /// Opaque content file to seal.
    content: PathBuf,
    /// Exact 8-bit RGB cover PNG.
    cover: PathBuf,
    /// New ARC PNG output path; existing paths are refused.
    #[arg(short = 'o', long)]
    output: PathBuf,
    /// Encrypted item name. Defaults to the content file name.
    #[arg(long)]
    name: Option<String>,
    /// Encrypted item media type.
    #[arg(long, default_value = "application/octet-stream")]
    media_type: String,
    /// Bounded UTF-8 file containing encrypted attribution text.
    #[arg(long)]
    attribution_file: Option<PathBuf>,
    /// Reed-Solomon protection profile.
    #[arg(long, value_enum, default_value_t)]
    profile: ProfileArg,
    #[command(flatten)]
    passphrase: PassphraseArgs,
}

#[derive(ClapArgs)]
struct OpenArgs {
    /// ARC PNG to open.
    input: PathBuf,
    /// New content output path; existing paths are refused.
    #[arg(short = 'o', long)]
    output: PathBuf,
    #[command(flatten)]
    passphrase: PassphraseArgs,
}

#[derive(ClapArgs)]
struct CorruptFixtureArgs {
    /// Authenticated ARC PNG to damage in bounded payload positions.
    input: PathBuf,
    /// New recoverable PNG fixture path; existing paths are refused.
    #[arg(short = 'o', long)]
    output: PathBuf,
    #[command(flatten)]
    passphrase: PassphraseArgs,
}

#[derive(ClapArgs)]
struct BlivreImportArgs {
    /// Exact pinned usfm-blivre-tr.zip asset.
    source_zip: PathBuf,
    /// New package directory for books, manifest, raw source, and provenance.
    #[arg(short = 'o', long)]
    output_dir: PathBuf,
}

#[derive(ClapArgs)]
struct BlivreAnalyzeArgs {
    /// Exact pinned usfm-blivre-tr.zip asset.
    source_zip: PathBuf,
    /// RGB8 carrier width in pixels.
    #[arg(long)]
    width: u32,
    /// RGB8 carrier height in pixels.
    #[arg(long)]
    height: u32,
    /// Reed-Solomon protection profile.
    #[arg(long, value_enum, default_value_t)]
    profile: ProfileArg,
}

#[derive(ClapArgs)]
struct BlivreSealBatchArgs {
    /// Exact pinned usfm-blivre-tr.zip asset.
    source_zip: PathBuf,
    /// Directory containing one `<ordinal>-<USFM_ID>.png` RGB8 cover per book.
    ///
    /// Examples: `01-GEN.png`, `19-PSA.png`, and `66-REV.png`.
    #[arg(long)]
    covers_dir: PathBuf,
    /// New directory for the 66 `<ordinal>-<USFM_ID>.arc.png` outputs.
    #[arg(short = 'o', long)]
    output_dir: PathBuf,
    /// Reed-Solomon protection profile.
    #[arg(long, value_enum, default_value_t)]
    profile: ProfileArg,
    #[command(flatten)]
    passphrase: PassphraseArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FitEstimate {
    codewords: usize,
    ciphertext_bytes: usize,
    interleaved_bytes: usize,
    required_bits: usize,
    fits: bool,
}

type CoverSnapshot = [u8; 32];

/// Run the nested `zion arc` command tree.
///
/// # Errors
/// Propagates bounded I/O, corpus, capacity, KDF, sealing, or opening errors.
pub fn run(args: Args) -> Result<()> {
    match args.command {
        ArcCommand::Capacity(args) => run_capacity(args),
        ArcCommand::Seal(args) => run_seal(args),
        ArcCommand::Open(args) => run_open(args),
        ArcCommand::CorruptFixture(args) => run_corrupt_fixture(args),
        ArcCommand::Blivre(args) => match args.command {
            BlivreCommand::Import(args) => run_blivre_import(args),
            BlivreCommand::Analyze(args) => run_blivre_analyze(args),
            BlivreCommand::SealBatch(args) => run_blivre_seal_batch(args),
        },
    }
}

fn run_capacity(args: CapacityArgs) -> Result<()> {
    validate_estimate_metadata_lengths(
        args.name_bytes,
        args.media_type_bytes,
        args.attribution_bytes,
    )?;
    let profile = EccProfile::from(args.profile);
    let result = capacity(args.width, args.height).context("computing ARC capacity")?;

    println!(
        "dimensions                 : {}x{} RGB8",
        args.width, args.height
    );
    println!("profile                    : {}", args.profile.name());
    println!("channels                   : {}", result.channels());
    println!("candidate_bits             : {}", result.candidate_bits());
    println!("usable_bits                : {}", result.usable_bits());
    println!("max_codewords              : {}", result.max_codewords());
    println!("data_bytes_per_codeword    : {}", profile.data_len());
    println!("parity_bytes_per_codeword  : {}", profile.parity_len());
    println!(
        "correction_bytes_per_word  : {}",
        profile.correction_budget()
    );
    println!(
        "max_ciphertext_bytes       : {}",
        result.max_ciphertext_bytes(profile)
    );
    println!(
        "max_aead_plaintext_bytes   : {}",
        result.max_aead_plaintext_bytes(profile)
    );

    if let Some(payload_bytes) = args.payload_bytes {
        let estimate = estimate_fit(
            result,
            profile,
            payload_bytes,
            args.name_bytes,
            args.media_type_bytes,
            args.attribution_bytes,
        )?;
        println!("estimated_payload_bytes    : {payload_bytes}");
        println!("required_codewords         : {}", estimate.codewords);
        println!("required_ciphertext_bytes  : {}", estimate.ciphertext_bytes);
        println!(
            "required_interleaved_bytes : {}",
            estimate.interleaved_bytes
        );
        println!("required_payload_bits      : {}", estimate.required_bits);
        println!("fits                       : {}", yes_no(estimate.fits));
    }
    Ok(())
}

fn run_seal(args: SealArgs) -> Result<()> {
    ensure_new_file_path(&args.output)?;
    let content = Zeroizing::new(read_bounded(
        &args.content,
        MAX_ARC_CONTENT_BYTES,
        "ARC content",
    )?);
    let cover = read_bounded(&args.cover, MAX_ARC_PNG_BYTES, "cover PNG")?;
    let name = match args.name {
        Some(name) => name,
        None => args
            .content
            .file_name()
            .and_then(|name| name.to_str())
            .context("content path has no UTF-8 file name; provide --name")?
            .to_owned(),
    };
    let attribution = read_optional_utf8(
        args.attribution_file.as_deref(),
        MAX_ARC_ATTRIBUTION_BYTES,
        "attribution",
    )?;
    let metadata = ItemMetadata::new(name, args.media_type, attribution);
    let passphrase = read_passphrase(&args.passphrase)?;
    let key_derivation_started = Instant::now();
    let collection_key =
        CollectionKey::new(passphrase.as_slice()).context("deriving ARC collection key")?;
    let key_derivation_wall_ms = key_derivation_started.elapsed().as_secs_f64() * 1_000.0;
    drop(passphrase);
    let sealed = seal_png_bytes_with_collection_key(
        content.as_slice(),
        &cover,
        &collection_key,
        &metadata,
        SealConfig::with_profile(args.profile.into()),
    )
    .context("sealing ARC capsule")?;

    write_new(&args.output, &sealed.png_bytes)?;
    eprintln!(
        "sealed: {} ({} bytes, profile={}, codewords={}, ciphertext_bytes={}, key_derivation_wall_ms={:.3}, collection_id={}, capsule_id={})",
        args.output.display(),
        sealed.png_bytes.len(),
        args.profile.name(),
        sealed.codeword_count,
        sealed.ciphertext_len,
        key_derivation_wall_ms,
        hex::encode(sealed.collection_id),
        hex::encode(sealed.capsule_id),
    );
    Ok(())
}

fn run_open(args: OpenArgs) -> Result<()> {
    ensure_new_file_path(&args.output)?;
    let input = read_bounded(&args.input, MAX_ARC_PNG_BYTES, "ARC PNG")?;
    let passphrase = read_passphrase(&args.passphrase)?;
    let opened = open_png_bytes(&input, passphrase.as_slice()).context("opening ARC capsule")?;
    drop(passphrase);

    let content = Zeroizing::new(opened.content);
    write_private_new(&args.output, content.as_slice())?;
    eprintln!(
        "opened: {} ({} bytes, profile={}, collection_id={}, capsule_id={})",
        args.output.display(),
        content.len(),
        profile_name(opened.profile),
        hex::encode(opened.collection_id),
        hex::encode(opened.capsule_id),
    );
    Ok(())
}

fn run_corrupt_fixture(args: CorruptFixtureArgs) -> Result<()> {
    ensure_new_file_path(&args.output)?;
    let input = read_bounded(&args.input, MAX_ARC_PNG_BYTES, "ARC PNG")?;
    let passphrase = read_passphrase(&args.passphrase)?;
    let fixture = make_bounded_corruption_fixture(&input, passphrase.as_slice())
        .context("creating bounded ARC corruption fixture")?;
    drop(passphrase);

    write_new(&args.output, &fixture.png_bytes)?;
    eprintln!(
        "corruption fixture: {} (profile={}, codewords={}, corrupted_bytes_per_codeword={}, total_corrupted_payload_bytes={}, changed_channels={})",
        args.output.display(),
        profile_name(fixture.profile),
        fixture.codeword_count,
        fixture.corrupted_bytes_per_codeword,
        fixture.total_corrupted_payload_bytes,
        fixture.changed_channels,
    );
    Ok(())
}

fn run_blivre_import(args: BlivreImportArgs) -> Result<()> {
    let package = read_blivre_package(&args.source_zip)?;
    package
        .write_to_directory(&args.output_dir)
        .with_context(|| {
            format!(
                "materializing corpus package at {}",
                args.output_dir.display()
            )
        })?;
    eprintln!(
        "imported BLIVRE {}: {} books, {} chapters, {} verse records into {}",
        package.manifest().source.version,
        package.manifest().totals.books,
        package.manifest().totals.chapters,
        package.manifest().totals.verse_records,
        args.output_dir.display(),
    );
    Ok(())
}

fn run_blivre_analyze(args: BlivreAnalyzeArgs) -> Result<()> {
    let package = read_blivre_package(&args.source_zip)?;
    let profile = EccProfile::from(args.profile);
    let carrier = capacity(args.width, args.height).context("computing ARC capacity")?;
    let mut all_fit = true;

    println!(
        "BLIVRE {} on {}x{} RGB8, profile={} (max_codewords={})",
        package.manifest().source.version,
        args.width,
        args.height,
        args.profile.name(),
        carrier.max_codewords(),
    );
    println!(
        "ordinal\tid\ttitle\tpayload_json_bytes\tpayload_zstd6_bytes\tcodewords\trequired_bits\tfit"
    );
    for book in package.books() {
        let metadata = blivre_metadata(book.canonical.ordinal, &book.canonical.title);
        let estimate = estimate_fit(
            carrier,
            profile,
            usize::try_from(book.metrics.payload_json_zstd6_bytes)
                .context("payload zstd metric does not fit usize")?,
            metadata.name.len(),
            metadata.media_type.len(),
            metadata.attribution.len(),
        )?;
        all_fit &= estimate.fits;
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            book.canonical.ordinal,
            book.canonical.id,
            book.canonical.title,
            book.metrics.payload_json_bytes,
            book.metrics.payload_json_zstd6_bytes,
            estimate.codewords,
            estimate.required_bits,
            yes_no(estimate.fits),
        );
    }
    println!("all_books_fit\t{}", yes_no(all_fit));
    anyhow::ensure!(all_fit, "one or more BLIVRE books do not fit this carrier");
    Ok(())
}

fn run_blivre_seal_batch(args: BlivreSealBatchArgs) -> Result<()> {
    ensure_new_directory_path(&args.output_dir)?;
    anyhow::ensure!(
        args.covers_dir.is_dir(),
        "covers directory does not exist or is not a directory: {}",
        args.covers_dir.display()
    );
    let package = read_blivre_package(&args.source_zip)?;
    let config = SealConfig::with_profile(args.profile.into());
    let cover_snapshots = preflight_batch(&package, &args.covers_dir, config.profile)?;

    let passphrase = read_passphrase(&args.passphrase)?;
    let collection_key = CollectionKey::new(passphrase.as_slice())
        .context("deriving one ARC collection key for the batch")?;
    drop(passphrase);
    let staging = StagingDirectory::create_for(&args.output_dir)?;

    for (book, expected_cover_snapshot) in package.books().iter().zip(&cover_snapshots) {
        let cover_path =
            batch_cover_path(&args.covers_dir, book.canonical.ordinal, &book.canonical.id);
        let output_path =
            batch_output_path(staging.path(), book.canonical.ordinal, &book.canonical.id);
        let cover = read_verified_batch_cover(&cover_path, expected_cover_snapshot)?;
        let metadata = blivre_metadata(book.canonical.ordinal, &book.canonical.title);
        let sealed = seal_png_bytes_with_collection_key(
            &book.payload_json,
            &cover,
            &collection_key,
            &metadata,
            config,
        )
        .with_context(|| format!("sealing BLIVRE book {}", book.canonical.id))?;
        write_new(&output_path, &sealed.png_bytes)?;
        eprintln!(
            "prepared {:02}/66 {} (codewords={}, ciphertext_bytes={})",
            book.canonical.ordinal, book.canonical.id, sealed.codeword_count, sealed.ciphertext_len,
        );
    }

    staging.publish(&args.output_dir)?;
    eprintln!(
        "sealed BLIVRE collection: 66 capsules in {} (profile={}, collection_id={})",
        args.output_dir.display(),
        args.profile.name(),
        hex::encode(collection_key.collection_id()),
    );
    Ok(())
}

fn read_blivre_package(path: &Path) -> Result<CorpusPackage> {
    let source = read_bounded(path, PINNED_ZIP_BYTES, "pinned BLIVRE ZIP")?;
    import_pinned_blivre(&source)
        .with_context(|| format!("importing pinned BLIVRE ZIP {}", path.display()))
}

fn preflight_batch(
    package: &CorpusPackage,
    covers_dir: &Path,
    profile: EccProfile,
) -> Result<Vec<CoverSnapshot>> {
    anyhow::ensure!(
        package.books().len() == 66,
        "BLIVRE package does not contain 66 books"
    );
    let expected_names: BTreeSet<String> = package
        .books()
        .iter()
        .map(|book| format!("{:02}-{}.png", book.canonical.ordinal, book.canonical.id))
        .collect();
    anyhow::ensure!(
        expected_names.len() == package.books().len(),
        "BLIVRE cover filenames are not unique"
    );
    validate_batch_cover_set(covers_dir, &expected_names)?;

    let mut cover_snapshots = Vec::with_capacity(package.books().len());
    for book in package.books() {
        let path = batch_cover_path(covers_dir, book.canonical.ordinal, &book.canonical.id);
        let bytes = read_batch_cover(&path, "required batch cover")?;
        let dimensions = png_dimensions_rgb8(&bytes)
            .with_context(|| format!("preflighting required cover {}", path.display()))?;
        let carrier = capacity(dimensions.0, dimensions.1)
            .with_context(|| format!("computing capacity for {}", path.display()))?;
        let decoded = zion_stego::load_png_rgb(Cursor::new(&bytes)).map_err(|error| {
            anyhow::anyhow!("decoding required cover {}: {error}", path.display())
        })?;
        anyhow::ensure!(
            decoded.width == dimensions.0
                && decoded.height == dimensions.1
                && decoded.has_valid_shape(),
            "decoded cover geometry is inconsistent: {}",
            path.display()
        );
        let metadata = blivre_metadata(book.canonical.ordinal, &book.canonical.title);
        let estimate = estimate_fit(
            carrier,
            profile,
            usize::try_from(book.metrics.payload_json_zstd6_bytes)
                .context("payload zstd metric does not fit usize")?,
            metadata.name.len(),
            metadata.media_type.len(),
            metadata.attribution.len(),
        )?;
        anyhow::ensure!(
            estimate.fits,
            "book {} does not fit required cover {}",
            book.canonical.id,
            path.display()
        );
        cover_snapshots.push(snapshot_cover(&bytes));
    }
    Ok(cover_snapshots)
}

fn read_verified_batch_cover(path: &Path, expected: &CoverSnapshot) -> Result<Vec<u8>> {
    let bytes = read_batch_cover(path, "batch cover PNG")?;
    anyhow::ensure!(
        snapshot_cover(&bytes) == *expected,
        "batch cover changed after preflight: {}",
        path.display()
    );
    Ok(bytes)
}

fn snapshot_cover(bytes: &[u8]) -> CoverSnapshot {
    *blake3::hash(bytes).as_bytes()
}

fn validate_batch_cover_set(covers_dir: &Path, expected: &BTreeSet<String>) -> Result<()> {
    let entries = fs::read_dir(covers_dir)
        .with_context(|| format!("enumerating covers directory {}", covers_dir.display()))?;
    let mut missing = expected.clone();
    let mut count = 0_usize;

    for entry in entries {
        count = count
            .checked_add(1)
            .context("cover directory entry count overflow")?;
        anyhow::ensure!(
            count <= MAX_BATCH_COVER_DIRECTORY_ENTRIES,
            "covers directory exceeds the {MAX_BATCH_COVER_DIRECTORY_ENTRIES}-entry limit"
        );
        let entry =
            entry.with_context(|| format!("reading covers directory {}", covers_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("reading cover entry type in {}", covers_dir.display()))?;
        anyhow::ensure!(
            file_type.is_file() && !file_type.is_symlink(),
            "covers directory contains a symlink, directory, or non-file entry"
        );
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("covers directory contains a non-UTF-8 entry name"))?;
        anyhow::ensure!(
            missing.remove(&name),
            "covers directory contains an extra or noncanonical entry: {name}"
        );
    }

    anyhow::ensure!(
        missing.is_empty() && count == expected.len(),
        "covers directory is missing one or more canonical BLIVRE covers"
    );
    Ok(())
}

fn png_dimensions_rgb8(bytes: &[u8]) -> Result<(u32, u32)> {
    anyhow::ensure!(bytes.len() >= 33, "PNG is shorter than its IHDR");
    anyhow::ensure!(
        bytes[..8] == [137, 80, 78, 71, 13, 10, 26, 10],
        "invalid PNG signature"
    );
    anyhow::ensure!(
        bytes[8..12] == 13_u32.to_be_bytes() && &bytes[12..16] == b"IHDR",
        "PNG does not begin with a canonical IHDR"
    );
    anyhow::ensure!(
        bytes[24] == 8
            && bytes[25] == 2
            && bytes[26] == 0
            && bytes[27] == 0
            && matches!(bytes[28], 0 | 1),
        "cover must be an 8-bit RGB PNG"
    );
    validate_png_chunk_layout(bytes)?;
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("fixed-width slice"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("fixed-width slice"));
    Ok((width, height))
}

fn validate_png_chunk_layout(bytes: &[u8]) -> Result<()> {
    let mut cursor = 8_usize;
    let mut saw_iend = false;
    while cursor < bytes.len() {
        let header_end = cursor.checked_add(8).context("PNG chunk offset overflow")?;
        anyhow::ensure!(header_end <= bytes.len(), "truncated PNG chunk header");
        let length = usize::try_from(u32::from_be_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .expect("fixed-width slice"),
        ))
        .context("PNG chunk length does not fit usize")?;
        let chunk_end = header_end
            .checked_add(length)
            .and_then(|end| end.checked_add(4))
            .context("PNG chunk length overflow")?;
        anyhow::ensure!(chunk_end <= bytes.len(), "truncated PNG chunk");
        let chunk_type = &bytes[cursor + 4..cursor + 8];
        anyhow::ensure!(chunk_type != b"acTL", "APNG covers are not supported");
        if chunk_type == b"IEND" {
            anyhow::ensure!(length == 0, "invalid PNG IEND length");
            anyhow::ensure!(chunk_end == bytes.len(), "bytes follow PNG IEND");
            saw_iend = true;
            break;
        }
        cursor = chunk_end;
    }
    anyhow::ensure!(saw_iend, "PNG has no terminal IEND");
    Ok(())
}

fn batch_cover_path(directory: &Path, ordinal: u8, id: &str) -> PathBuf {
    directory.join(format!("{ordinal:02}-{id}.png"))
}

fn batch_output_path(directory: &Path, ordinal: u8, id: &str) -> PathBuf {
    directory.join(format!("{ordinal:02}-{id}.arc.png"))
}

fn blivre_metadata(ordinal: u8, title: &str) -> ItemMetadata {
    ItemMetadata::new(
        format!("{ordinal:02} — {title}"),
        BLIVRE_MEDIA_TYPE,
        BLIVRE_ATTRIBUTION,
    )
}

fn estimate_fit(
    capacity: Capacity,
    profile: EccProfile,
    payload_bytes: usize,
    name_bytes: usize,
    media_type_bytes: usize,
    attribution_bytes: usize,
) -> Result<FitEstimate> {
    validate_estimate_metadata_lengths(name_bytes, media_type_bytes, attribution_bytes)?;
    let base_bytes = ENVELOPE_HEADER_BYTES
        .checked_add(name_bytes)
        .and_then(|bytes| bytes.checked_add(media_type_bytes))
        .and_then(|bytes| bytes.checked_add(attribution_bytes))
        .and_then(|bytes| bytes.checked_add(payload_bytes))
        .context("capacity estimate overflow")?;
    let tagged_bytes = base_bytes
        .checked_add(AEAD_TAG_BYTES)
        .context("capacity estimate overflow")?;
    let codewords = tagged_bytes.div_ceil(profile.data_len());
    let ciphertext_bytes = codewords
        .checked_mul(profile.data_len())
        .context("capacity estimate overflow")?;
    let interleaved_bytes = codewords
        .checked_mul(RS_CODEWORD_BYTES)
        .context("capacity estimate overflow")?;
    let required_bits = interleaved_bytes
        .checked_mul(8)
        .context("capacity estimate overflow")?;
    Ok(FitEstimate {
        codewords,
        ciphertext_bytes,
        interleaved_bytes,
        required_bits,
        fits: codewords <= capacity.max_codewords() && required_bits <= capacity.usable_bits(),
    })
}

fn validate_estimate_metadata_lengths(
    name_bytes: usize,
    media_type_bytes: usize,
    attribution_bytes: usize,
) -> Result<()> {
    anyhow::ensure!(
        (1..=255).contains(&name_bytes),
        "name bytes must be in 1..=255"
    );
    anyhow::ensure!(
        (1..=127).contains(&media_type_bytes),
        "media-type bytes must be in 1..=127"
    );
    anyhow::ensure!(
        attribution_bytes <= MAX_ARC_ATTRIBUTION_BYTES,
        "attribution bytes must be at most {MAX_ARC_ATTRIBUTION_BYTES}"
    );
    Ok(())
}

fn read_passphrase(args: &PassphraseArgs) -> Result<Zeroizing<Vec<u8>>> {
    if let Some(path) = args.passphrase_file.as_deref() {
        return read_passphrase_file(path);
    }
    let _explicit_or_fallback = args.prompt || args.passphrase_file.is_none();
    let prompted = Zeroizing::new(
        rpassword::prompt_password("ARC passphrase: ")
            .context("reading ARC passphrase interactively")?,
    );
    let passphrase = Zeroizing::new(prompted.as_bytes().to_vec());
    validate_passphrase_bytes(&passphrase)?;
    Ok(passphrase)
}

fn read_passphrase_file(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    validate_passphrase_file_permissions(&file, path)?;
    let raw_limit = MAX_ARC_PASSPHRASE_BYTES + 2;
    let mut passphrase = Zeroizing::new(Vec::new());
    file.take((raw_limit + 1) as u64)
        .read_to_end(&mut passphrase)
        .with_context(|| format!("reading bounded passphrase file {}", path.display()))?;
    anyhow::ensure!(
        passphrase.len() <= raw_limit,
        "passphrase file exceeds the bounded ARC limit"
    );
    trim_one_trailing_terminator(&mut passphrase);
    validate_passphrase_bytes(&passphrase)?;
    Ok(passphrase)
}

#[cfg(unix)]
fn validate_passphrase_file_permissions(file: &File, path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .with_context(|| format!("reading passphrase file metadata {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "passphrase path is not a regular file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.mode() & 0o077 == 0,
        "passphrase file must not be accessible by group or other users: {}",
        path.display()
    );
    Ok(())
}

#[cfg(not(unix))]
fn validate_passphrase_file_permissions(file: &File, path: &Path) -> Result<()> {
    anyhow::ensure!(
        file.metadata()
            .with_context(|| format!("reading passphrase file metadata {}", path.display()))?
            .is_file(),
        "passphrase path is not a regular file: {}",
        path.display()
    );
    Ok(())
}

fn trim_one_trailing_terminator(bytes: &mut Vec<u8>) {
    if bytes.ends_with(b"\r\n") {
        bytes.truncate(bytes.len() - 2);
    } else if matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
}

fn validate_passphrase_bytes(bytes: &[u8]) -> Result<()> {
    anyhow::ensure!(!bytes.is_empty(), "ARC passphrase must not be empty");
    anyhow::ensure!(
        bytes.len() <= MAX_ARC_PASSPHRASE_BYTES,
        "ARC passphrase exceeds {MAX_ARC_PASSPHRASE_BYTES} bytes"
    );
    Ok(())
}

fn read_optional_utf8(path: Option<&Path>, max: usize, label: &str) -> Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };
    let bytes = read_bounded(path, max, label)?;
    String::from_utf8(bytes)
        .with_context(|| format!("{label} file is not UTF-8: {}", path.display()))
}

fn read_bounded(path: &Path, max: usize, label: &str) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("opening {label} {}", path.display()))?;
    read_bounded_file(file, path, max, label)
}

fn read_batch_cover(path: &Path, label: &str) -> Result<Vec<u8>> {
    let file = open_batch_cover(path, label)?;
    read_bounded_file(file, path, MAX_ARC_PNG_BYTES, label)
}

#[cfg(unix)]
fn open_batch_cover(path: &Path, label: &str) -> Result<File> {
    use rustix::fs::{CWD, Mode, OFlags, openat};

    let fd = openat(
        CWD,
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .with_context(|| format!("opening {label} without following links {}", path.display()))?;
    let file = File::from(fd);
    anyhow::ensure!(
        file.metadata()
            .with_context(|| format!("reading {label} metadata {}", path.display()))?
            .is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    Ok(file)
}

#[cfg(not(unix))]
fn open_batch_cover(path: &Path, label: &str) -> Result<File> {
    let file = File::open(path).with_context(|| format!("opening {label} {}", path.display()))?;
    anyhow::ensure!(
        file.metadata()
            .with_context(|| format!("reading {label} metadata {}", path.display()))?
            .is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    Ok(file)
}

fn read_bounded_file(file: File, path: &Path, max: usize, label: &str) -> Result<Vec<u8>> {
    let read_limit = max.checked_add(1).context("bounded read limit overflow")?;
    let mut bytes = Vec::new();
    file.take(read_limit as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {label} {}", path.display()))?;
    anyhow::ensure!(
        bytes.len() <= max,
        "{label} exceeds the {max}-byte limit: {}",
        path.display()
    );
    Ok(bytes)
}

fn ensure_new_file_path(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path_entry_exists(path)?,
        "output path already exists: {}",
        path.display()
    );
    Ok(())
}

fn ensure_new_directory_path(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path_entry_exists(path)?,
        "output directory already exists: {}",
        path.display()
    );
    Ok(())
}

fn path_entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("checking output path {}", path.display()))
        }
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("creating new output {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("writing output {}", path.display()))?;
    file.flush()
        .with_context(|| format!("flushing output {}", path.display()))?;
    Ok(())
}

fn write_private_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("creating private output {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("writing private output {}", path.display()))?;
    file.flush()
        .with_context(|| format!("flushing private output {}", path.display()))?;
    Ok(())
}

struct StagingDirectory {
    path: PathBuf,
    published: bool,
}

impl StagingDirectory {
    fn create_for(final_path: &Path) -> Result<Self> {
        let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
        let stem = final_path
            .file_name()
            .and_then(|name| name.to_str())
            .context("batch output directory has no UTF-8 file name")?;
        for _ in 0..16 {
            let mut random = [0_u8; 8];
            getrandom::fill(&mut random)
                .map_err(|_| anyhow::anyhow!("creating private staging name failed"))?;
            let path = parent.join(format!(".{stem}.zion-staging-{}", hex::encode(random)));
            match create_private_staging_directory(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("creating private staging directory {}", path.display())
                    });
                }
            }
        }
        anyhow::bail!("could not allocate a unique private staging directory")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(mut self, final_path: &Path) -> Result<()> {
        rename_directory_noreplace(&self.path, final_path).with_context(|| {
            format!(
                "publishing completed batch {} to {}",
                self.path.display(),
                final_path.display()
            )
        })?;
        self.published = true;
        Ok(())
    }
}

#[cfg(unix)]
fn create_private_staging_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let mut builder = DirBuilder::new();
    builder.mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_staging_directory(path: &Path) -> std::io::Result<()> {
    DirBuilder::new().create(path)
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
fn rename_directory_noreplace(source: &Path, destination: &Path) -> Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};

    renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE)
        .context("atomically renaming directory without replacement")
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
fn rename_directory_noreplace(_source: &Path, _destination: &Path) -> Result<()> {
    anyhow::bail!("atomic no-replace directory publication is unsupported on this platform")
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if !self.published
            && self.path.exists()
            && let Err(error) = fs::remove_dir_all(&self.path)
        {
            eprintln!(
                "warning: could not clean private output staging directory {}: {error}",
                self.path.display()
            );
        }
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

const fn profile_name(profile: EccProfile) -> &'static str {
    match profile {
        EccProfile::Safe => "safe",
        EccProfile::Balanced => "balanced",
        EccProfile::Dense => "dense",
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use sha2::{Digest, Sha256};

    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "zion-cli-arc-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    fn make_owner_only(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .expect("set owner-only permissions");
    }

    #[cfg(not(unix))]
    fn make_owner_only(_path: &Path) {}

    #[test]
    fn blivre_attribution_bytes_are_pinned() {
        assert_eq!(BLIVRE_ATTRIBUTION.len(), 384);
        assert_eq!(
            hex::encode(Sha256::digest(BLIVRE_ATTRIBUTION.as_bytes())),
            "459e4e5585e1ee8692f07aa947fe08b200f5e40afc09e46961fda42c5d5d070d"
        );
    }

    #[test]
    fn passphrase_file_trims_exactly_one_trailing_terminator() {
        for (raw, expected) in [
            (&b"opaque\r\n"[..], &b"opaque"[..]),
            (&b"opaque\n"[..], &b"opaque"[..]),
            (&b"opaque\r"[..], &b"opaque"[..]),
            (&b"opaque\r\n\r\n"[..], &b"opaque\r\n"[..]),
            (&b"op\0aque"[..], &b"op\0aque"[..]),
        ] {
            let path = temp_path("passphrase");
            fs::write(&path, raw).expect("write fixture");
            make_owner_only(&path);
            let passphrase = read_passphrase_file(&path).expect("read passphrase");
            assert_eq!(passphrase.as_slice(), expected);
            fs::remove_file(path).expect("remove fixture");
        }
    }

    #[test]
    fn passphrase_and_generic_reads_are_bounded() {
        let passphrase_path = temp_path("long-passphrase");
        fs::write(&passphrase_path, vec![b'x'; MAX_ARC_PASSPHRASE_BYTES + 3])
            .expect("write fixture");
        make_owner_only(&passphrase_path);
        assert!(read_passphrase_file(&passphrase_path).is_err());
        fs::remove_file(passphrase_path).expect("remove fixture");

        let input_path = temp_path("long-input");
        fs::write(&input_path, b"12345").expect("write fixture");
        assert!(read_bounded(&input_path, 4, "fixture").is_err());
        fs::remove_file(input_path).expect("remove fixture");
    }

    #[cfg(unix)]
    #[test]
    fn passphrase_file_rejects_group_or_world_access() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("permissions");
        fs::write(&path, b"not-reported-in-error").expect("write fixture");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("set unsafe permissions");
        let error = read_passphrase_file(&path).expect_err("unsafe mode must be rejected");
        assert!(!error.to_string().contains("not-reported-in-error"));
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn create_new_output_refuses_overwrite() {
        let path = temp_path("output");
        write_new(&path, b"first").expect("create output");
        assert!(write_new(&path, b"second").is_err());
        assert_eq!(fs::read(&path).expect("read output"), b"first");
        fs::remove_file(path).expect("remove fixture");
    }

    #[cfg(unix)]
    #[test]
    fn private_output_is_owner_only_under_a_permissive_umask() {
        use std::{os::unix::fs::MetadataExt, process::Command};

        const CHILD_PATH_ENV: &str = "ZION_CLI_PRIVATE_MODE_CHILD_PATH";
        if let Some(path) = std::env::var_os(CHILD_PATH_ENV) {
            let path = PathBuf::from(path);
            write_private_new(&path, b"private fixture").expect("write private output");
            let mode = fs::metadata(&path).expect("output metadata").mode() & 0o777;
            assert_eq!(mode, 0o600);
            fs::remove_file(path).expect("remove child fixture");
            return;
        }

        let path = temp_path("private-mode");
        let executable = std::env::current_exe().expect("current test executable");
        let status = Command::new("sh")
            .arg("-c")
            .arg(
                "umask 000; exec \"$1\" --exact \
                 cmd::arc::tests::private_output_is_owner_only_under_a_permissive_umask",
            )
            .arg("sh")
            .arg(executable)
            .env(CHILD_PATH_ENV, &path)
            .status()
            .expect("spawn isolated umask test");
        assert!(status.success());
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn staging_directory_is_owner_only_at_creation_under_a_permissive_umask() {
        use std::{os::unix::fs::MetadataExt, process::Command};

        const CHILD_PATH_ENV: &str = "ZION_CLI_STAGING_MODE_CHILD_PATH";
        if let Some(final_path) = std::env::var_os(CHILD_PATH_ENV) {
            let final_path = PathBuf::from(final_path);
            let staging = StagingDirectory::create_for(&final_path).expect("create staging");
            let staging_path = staging.path().to_owned();
            let mode = fs::metadata(&staging_path)
                .expect("staging metadata immediately after creation")
                .mode()
                & 0o777;
            assert_eq!(mode, 0o700);
            drop(staging);
            assert!(!staging_path.exists());
            return;
        }

        let final_path = temp_path("staging-private-mode");
        let executable = std::env::current_exe().expect("current test executable");
        let status = Command::new("sh")
            .arg("-c")
            .arg(
                "umask 000; exec \"$1\" --exact \
                 cmd::arc::tests::staging_directory_is_owner_only_at_creation_under_a_permissive_umask",
            )
            .arg("sh")
            .arg(executable)
            .env(CHILD_PATH_ENV, &final_path)
            .status()
            .expect("spawn isolated staging umask test");
        assert!(status.success());
        assert!(!final_path.exists());
    }

    #[test]
    fn staging_is_not_visible_until_publish_and_cleans_on_failure() {
        let final_path = temp_path("final-batch");
        let staging_path = {
            let staging = StagingDirectory::create_for(&final_path).expect("create staging");
            let path = staging.path().to_owned();
            write_new(&path.join("01-GEN.arc.png"), b"prepared").expect("write staging");
            assert!(!final_path.exists());
            path
        };
        assert!(!staging_path.exists());

        let staging = StagingDirectory::create_for(&final_path).expect("create staging");
        write_new(&staging.path().join("01-GEN.arc.png"), b"prepared").expect("write staging");
        staging.publish(&final_path).expect("publish");
        assert_eq!(
            fs::read(final_path.join("01-GEN.arc.png")).expect("read published"),
            b"prepared"
        );
        fs::remove_dir_all(final_path).expect("remove published fixture");
    }

    #[test]
    fn staging_publish_refuses_an_existing_final_directory() {
        let final_path = temp_path("occupied-final-batch");
        let staging = StagingDirectory::create_for(&final_path).expect("create staging");
        let staging_path = staging.path().to_owned();
        write_new(&staging_path.join("01-GEN.arc.png"), b"prepared").expect("write staging");
        fs::create_dir(&final_path).expect("create competing final");
        fs::write(final_path.join("sentinel"), b"keep").expect("write sentinel");

        assert!(staging.publish(&final_path).is_err());
        assert_eq!(
            fs::read(final_path.join("sentinel")).expect("read sentinel"),
            b"keep"
        );
        assert!(!staging_path.exists());
        fs::remove_dir_all(final_path).expect("remove final fixture");
    }

    #[cfg(unix)]
    #[test]
    fn staging_publish_noreplace_preserves_an_existing_empty_directory() {
        use std::os::unix::fs::MetadataExt;

        let final_path = temp_path("occupied-empty-final-batch");
        let staging = StagingDirectory::create_for(&final_path).expect("create staging");
        let staging_path = staging.path().to_owned();
        write_new(&staging_path.join("01-GEN.arc.png"), b"prepared").expect("write staging");
        fs::create_dir(&final_path).expect("create competing empty final");
        let before = fs::metadata(&final_path).expect("final metadata before publish");

        let error = staging
            .publish(&final_path)
            .expect_err("no-replace publish must reject an existing empty directory");
        assert!(!error.to_string().is_empty());
        #[cfg(any(target_os = "linux", target_os = "android"))]
        assert_eq!(
            error.downcast_ref::<rustix::io::Errno>(),
            Some(&rustix::io::Errno::EXIST)
        );
        let after = fs::metadata(&final_path).expect("final metadata after rejected publish");
        assert_eq!((after.dev(), after.ino()), (before.dev(), before.ino()));
        assert!(
            fs::read_dir(&final_path)
                .expect("read preserved final")
                .next()
                .is_none()
        );
        fs::write(final_path.join("sentinel"), b"keep").expect("write sentinel after rejection");
        assert_eq!(
            fs::read(final_path.join("sentinel")).expect("read sentinel"),
            b"keep"
        );
        assert!(!staging_path.exists());
        fs::remove_dir_all(final_path).expect("remove final fixture");
    }

    #[test]
    fn galaxy_a57_capacity_fits_measured_normalized_psalms_payload() {
        let carrier = capacity(1080, 2340).expect("reference capacity");
        let metadata = blivre_metadata(19, "Salmos");
        let estimate = estimate_fit(
            carrier,
            EccProfile::Safe,
            123_349,
            metadata.name.len(),
            metadata.media_type.len(),
            metadata.attribution.len(),
        )
        .expect("estimate");
        assert!(estimate.fits);
        assert_eq!(estimate.codewords, 556);
        assert!(estimate.codewords < carrier.max_codewords());
        assert_eq!(estimate.ciphertext_bytes % EccProfile::Safe.data_len(), 0);
    }

    fn canonical_cover_names() -> BTreeSet<String> {
        zion_corpus::canonical_books()
            .expect("canonical books")
            .into_iter()
            .map(|book| format!("{:02}-{}.png", book.ordinal, book.id))
            .collect()
    }

    #[test]
    fn batch_cover_set_accepts_exactly_the_66_canonical_files() {
        let directory = temp_path("exact-cover-set");
        fs::create_dir(&directory).expect("create covers directory");
        let expected = canonical_cover_names();
        for name in &expected {
            fs::write(directory.join(name), b"fixture").expect("write cover-name fixture");
        }

        validate_batch_cover_set(&directory, &expected).expect("exact set must validate");
        fs::remove_dir_all(directory).expect("remove fixture");
    }

    #[test]
    fn batch_cover_set_rejects_any_extra_entry() {
        let directory = temp_path("extra-cover");
        fs::create_dir(&directory).expect("create covers directory");
        let expected = canonical_cover_names();
        for name in &expected {
            fs::write(directory.join(name), b"fixture").expect("write cover-name fixture");
        }
        fs::write(directory.join("README.txt"), b"extra").expect("write extra fixture");

        assert!(validate_batch_cover_set(&directory, &expected).is_err());
        fs::remove_dir_all(directory).expect("remove fixture");
    }

    #[test]
    fn batch_cover_mutation_after_preflight_is_rejected_without_publish() {
        let covers_directory = temp_path("mutated-cover-directory");
        fs::create_dir(&covers_directory).expect("create covers directory");
        let cover_path = covers_directory.join("01-GEN.png");
        fs::write(&cover_path, b"preflight cover bytes").expect("write preflight cover");
        let expected = snapshot_cover(
            &read_batch_cover(&cover_path, "test cover").expect("read preflight cover"),
        );
        fs::write(&cover_path, b"mutated cover bytes").expect("mutate cover");

        let final_path = temp_path("mutated-cover-final");
        let staging = StagingDirectory::create_for(&final_path).expect("create staging");
        let staging_path = staging.path().to_owned();
        assert!(read_verified_batch_cover(&cover_path, &expected).is_err());
        drop(staging);

        assert!(!final_path.exists());
        assert!(!staging_path.exists());
        fs::remove_dir_all(covers_directory).expect("remove covers fixture");
    }

    #[cfg(unix)]
    #[test]
    fn batch_cover_symlink_swap_after_preflight_is_rejected_without_publish() {
        use std::os::unix::fs::symlink;

        let covers_directory = temp_path("symlink-cover-directory");
        fs::create_dir(&covers_directory).expect("create covers directory");
        let cover_path = covers_directory.join("01-GEN.png");
        let replacement_path = covers_directory.join("replacement.png");
        fs::write(&cover_path, b"preflight cover bytes").expect("write preflight cover");
        fs::write(&replacement_path, b"preflight cover bytes").expect("write replacement cover");
        let expected = snapshot_cover(
            &read_batch_cover(&cover_path, "test cover").expect("read preflight cover"),
        );
        fs::remove_file(&cover_path).expect("remove preflight cover");
        symlink(&replacement_path, &cover_path).expect("replace cover with symlink");

        let final_path = temp_path("symlink-cover-final");
        let staging = StagingDirectory::create_for(&final_path).expect("create staging");
        let staging_path = staging.path().to_owned();
        assert!(read_verified_batch_cover(&cover_path, &expected).is_err());
        drop(staging);

        assert!(!final_path.exists());
        assert!(!staging_path.exists());
        fs::remove_dir_all(covers_directory).expect("remove covers fixture");
    }

    #[test]
    fn batch_file_contract_is_canonical_and_distinct() {
        let directory = Path::new("covers");
        assert_eq!(
            batch_cover_path(directory, 1, "GEN"),
            Path::new("covers/01-GEN.png")
        );
        assert_eq!(
            batch_output_path(directory, 66, "REV"),
            Path::new("covers/66-REV.arc.png")
        );
    }
}
