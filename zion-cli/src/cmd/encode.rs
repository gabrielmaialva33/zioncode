use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_codec::constants::ZSTD_LEVEL_DEFAULT;
use zion_codec::encode::encode_file;

#[derive(ClapArgs)]
pub struct Args {
    /// Input file.
    pub input: PathBuf,

    /// Output prefix. Generated files: `<prefix>_000.zbin`, `<prefix>_001.zbin`, etc.
    #[arg(short = 'o', long = "output-prefix")]
    pub output_prefix: PathBuf,

    /// Number of RS codewords per symbol.
    #[arg(long, default_value_t = 148)]
    pub k: u16,

    /// zstd compression level (1-22).
    #[arg(long, default_value_t = ZSTD_LEVEL_DEFAULT)]
    pub zstd_level: i32,
}

/// Run the `zion encode` command.
///
/// # Errors
/// Propagates I/O or encoding errors.
pub fn run(args: Args) -> Result<()> {
    let raw = fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let encoded = encode_file(&raw, args.k, args.zstd_level).context("encode_file")?;

    let prefix_file_name = args
        .output_prefix
        .file_name()
        .context("output-prefix has no file name")?
        .to_string_lossy()
        .into_owned();

    for (i, sym) in encoded.symbols.iter().enumerate() {
        let filename = format!("{prefix_file_name}_{i:03}.zbin");
        let path = args.output_prefix.with_file_name(filename);
        fs::write(&path, sym).with_context(|| format!("writing {}", path.display()))?;
        eprintln!("written: {}", path.display());
    }

    eprintln!(
        "{} symbols generated (file_id={})",
        encoded.symbols.len(),
        uuid::Uuid::from_bytes(encoded.file_id)
    );
    Ok(())
}
