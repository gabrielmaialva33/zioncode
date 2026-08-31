use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use zion_art::{MAX_CONTENT_BYTES, MeaningProfile, RenderParams, render};

#[derive(ClapArgs)]
pub struct Args {
    /// UTF-8 content whose structure and identity generate the artwork.
    pub input: PathBuf,

    /// New PNG path. Existing files are never replaced.
    #[arg(short = 'o', long)]
    pub output: PathBuf,

    /// Output width in pixels.
    #[arg(long, default_value_t = 1_080)]
    pub width: u32,

    /// Output height in pixels.
    #[arg(long, default_value_t = 2_340)]
    pub height: u32,

    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub warmth: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub motion: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub density: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub tension: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub intimacy: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub transcendence: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub order: i16,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub radiance: i16,
}

/// Runs the experimental deterministic information-to-art compiler.
///
/// # Errors
///
/// Propagates bounded input, semantic-profile, rendering, and create-new
/// output failures.
pub fn run(args: Args) -> Result<()> {
    let metadata = fs::metadata(&args.input)
        .with_context(|| format!("reading metadata for {}", args.input.display()))?;
    anyhow::ensure!(
        metadata.len() <= u64::try_from(MAX_CONTENT_BYTES).unwrap(),
        "input exceeds the {}-byte art compiler limit",
        MAX_CONTENT_BYTES
    );
    let content =
        fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let meaning = MeaningProfile::new([
        args.warmth,
        args.motion,
        args.density,
        args.tension,
        args.intimacy,
        args.transcendence,
        args.order,
        args.radiance,
    ])
    .context("invalid semantic profile")?;
    let output = render(
        &content,
        RenderParams {
            width: args.width,
            height: args.height,
            meaning,
        },
    )
    .context("art compilation failed")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args.output)
        .with_context(|| format!("creating {}", args.output.display()))?;
    file.write_all(&output.png_bytes)
        .with_context(|| format!("writing {}", args.output.display()))?;
    file.sync_all()
        .with_context(|| format!("syncing {}", args.output.display()))?;

    eprintln!(
        "art-rendered: {} ({}x{}, grammar=v{}, words={}, raster_hash={})",
        args.output.display(),
        args.width,
        args.height,
        output.genome.version,
        output.genome.structure.words,
        hex::encode(output.raster_hash),
    );
    Ok(())
}
