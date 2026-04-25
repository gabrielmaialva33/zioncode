use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_optical::extract;

#[derive(ClapArgs)]
pub struct Args {
    /// Input PNG (zion-optical v1).
    pub input: PathBuf,

    /// Output file path.
    #[arg(short = 'o', long)]
    pub output: PathBuf,
}

/// Run `zion optical-extract`.
///
/// # Errors
/// Propagates I/O or extract-pipeline errors.
pub fn run(args: Args) -> Result<()> {
    let png_bytes =
        fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let out = extract(&png_bytes).context("extract failed")?;
    fs::write(&args.output, &out.file_bytes)
        .with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!(
        "recovered: {} ({} bytes, file_id={}, k_global={}, total_symbols={})",
        args.output.display(),
        out.file_bytes.len(),
        uuid::Uuid::from_bytes(out.file_id),
        out.k_global,
        out.total_symbols
    );
    Ok(())
}
