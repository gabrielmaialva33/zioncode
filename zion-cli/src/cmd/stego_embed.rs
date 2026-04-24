use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_stego::EmbeddingDensity;
use zion_stego::constants::EMBEDDING_DENSITY_DEFAULT;
use zion_stego::embed::{EmbedParams, embed_file};
use zion_stego::png_io::{load_png_rgb, save_png_rgb};

#[derive(ClapArgs)]
pub struct Args {
    /// Input file to embed.
    pub input: PathBuf,

    /// Host PNG photos (one or more).
    #[arg(short = 'H', long = "host", num_args = 1.., required = true)]
    pub hosts: Vec<PathBuf>,

    /// Output directory (produces stego_000.png, stego_001.png, ...).
    #[arg(short = 'o', long)]
    pub output_dir: PathBuf,

    /// Passphrase directly on command line (insecure — prefer --prompt).
    #[arg(long, conflicts_with_all = ["passphrase_file", "prompt"])]
    pub passphrase: Option<String>,

    /// Read passphrase from a file (first line).
    #[arg(long, conflicts_with_all = ["passphrase", "prompt"])]
    pub passphrase_file: Option<PathBuf>,

    /// Read passphrase interactively (no echo).
    #[arg(long, conflicts_with_all = ["passphrase", "passphrase_file"])]
    pub prompt: bool,

    /// Embedding density (default 0.33).
    #[arg(long, default_value_t = EMBEDDING_DENSITY_DEFAULT)]
    pub density: f32,

    /// zstd compression level.
    #[arg(long, default_value_t = zion_codec::ZSTD_LEVEL_DEFAULT)]
    pub zstd_level: i32,
}

/// Runs the `zion stego-embed` subcommand.
///
/// # Errors
/// Propagates I/O or embed-pipeline errors.
pub fn run(args: Args) -> Result<()> {
    let passphrase = read_passphrase(
        &args.passphrase,
        args.passphrase_file.as_deref(),
        args.prompt,
    )?;

    let file_bytes =
        fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;

    let mut hosts = Vec::with_capacity(args.hosts.len());
    for path in &args.hosts {
        let data = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let img = load_png_rgb(data)
            .map_err(|e| anyhow::anyhow!("reading {} as PNG: {e}", path.display()))?;
        hosts.push(img);
    }

    let params = EmbedParams {
        passphrase,
        density: EmbeddingDensity::new(args.density).context("invalid embedding density")?,
        zstd_level: args.zstd_level,
    };
    let out = embed_file(&file_bytes, hosts, &params)
        .map_err(|e| anyhow::anyhow!("embed failed: {e}"))?;

    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("creating {}", args.output_dir.display()))?;
    for (i, img) in out.stego_images.iter().enumerate() {
        let path = args.output_dir.join(format!("stego_{i:03}.png"));
        let file =
            fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
        save_png_rgb(img, file).map_err(|e| anyhow::anyhow!("saving {}: {e}", path.display()))?;
        eprintln!("wrote: {}", path.display());
    }

    eprintln!(
        "embed complete: file_id={} k_global={} symbols={} hosts_provided={}",
        uuid::Uuid::from_bytes(out.file_id),
        out.k_global,
        out.symbols_used,
        out.hosts_provided
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
