use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_stego::{extract_file, load_png_rgb};

#[derive(ClapArgs)]
pub struct Args {
    /// Stego PNG photos.
    pub stego: Vec<PathBuf>,

    /// Output file.
    #[arg(short = 'o', long)]
    pub output: PathBuf,

    /// Passphrase directly (insecure — prefer --prompt).
    #[arg(long, conflicts_with_all = ["passphrase_file", "prompt"])]
    pub passphrase: Option<String>,

    /// Read passphrase from a file (first line).
    #[arg(long, conflicts_with_all = ["passphrase", "prompt"])]
    pub passphrase_file: Option<PathBuf>,

    /// Read passphrase interactively (no echo).
    #[arg(long, conflicts_with_all = ["passphrase", "passphrase_file"])]
    pub prompt: bool,
}

/// Runs the `zion stego-extract` subcommand.
///
/// # Errors
/// Propagates I/O or extract-pipeline errors.
pub fn run(args: Args) -> Result<()> {
    anyhow::ensure!(!args.stego.is_empty(), "provide at least one stego photo");

    let passphrase = read_passphrase(
        &args.passphrase,
        args.passphrase_file.as_deref(),
        args.prompt,
    )?;

    let mut images = Vec::with_capacity(args.stego.len());
    for path in &args.stego {
        let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let img = load_png_rgb(file)
            .map_err(|e| anyhow::anyhow!("reading {} as PNG: {e}", path.display()))?;
        images.push(img);
    }

    let out =
        extract_file(&images, &passphrase).map_err(|e| anyhow::anyhow!("extract failed: {e}"))?;

    fs::write(&args.output, &out.file_bytes)
        .with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!(
        "file recovered: {} ({} bytes) file_id={}",
        args.output.display(),
        out.file_bytes.len(),
        uuid::Uuid::from_bytes(out.file_id)
    );
    Ok(())
}

fn read_passphrase(
    direct: &Option<String>,
    file: Option<&std::path::Path>,
    prompt: bool,
) -> Result<String> {
    if let Some(p) = direct {
        return Ok(p.clone());
    }
    if let Some(f) = file {
        let content = fs::read_to_string(f).with_context(|| format!("reading {}", f.display()))?;
        return Ok(content.lines().next().unwrap_or("").to_string());
    }
    if prompt {
        let p = rpassword::prompt_password("passphrase: ")
            .context("reading passphrase interactively")?;
        return Ok(p);
    }
    anyhow::bail!("provide --passphrase, --passphrase-file, or --prompt");
}
