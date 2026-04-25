use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_optical::{ImageShape, K_DEFAULT, RenderParams, ZSTD_LEVEL_DEFAULT, render};

#[derive(ClapArgs)]
pub struct Args {
    /// Input file to render as a single PNG.
    pub input: PathBuf,

    /// Output PNG path.
    #[arg(short = 'o', long)]
    pub output: PathBuf,

    /// K (codewords per symbol). Defaults to ZION_CODEC_MAX_K (4112).
    #[arg(long)]
    pub k: Option<u16>,

    /// Force explicit width and height (in pixels). Mutually exclusive with --aspect.
    #[arg(long, requires = "height", conflicts_with = "aspect")]
    pub width: Option<u32>,
    #[arg(long, requires = "width", conflicts_with = "aspect")]
    pub height: Option<u32>,

    /// Aspect ratio "W:H" (e.g. "16:9", "4:3"). Mutually exclusive with --width/--height.
    #[arg(long, conflicts_with_all = ["width", "height"])]
    pub aspect: Option<String>,

    /// zstd compression level (1..=22). Defaults to 22 (max density).
    #[arg(long)]
    pub zstd_level: Option<i32>,
}

/// Run `zion optical-render`.
///
/// # Errors
/// Propagates I/O or render-pipeline errors.
pub fn run(args: Args) -> Result<()> {
    let file_bytes =
        fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;

    let shape = match (args.width, args.height, args.aspect.as_deref()) {
        (Some(w), Some(h), _) => ImageShape::WidthHeight {
            width: w,
            height: h,
        },
        (_, _, Some(a)) => parse_aspect(a)?,
        _ => ImageShape::Square,
    };

    let k = args.k.unwrap_or(K_DEFAULT);
    let params = RenderParams::new()
        .with_k(k)
        .map_err(|e| anyhow::anyhow!("invalid k={k}: {e}"))?
        .with_shape(shape)
        .with_zstd_level(args.zstd_level.unwrap_or(ZSTD_LEVEL_DEFAULT));
    let out = render(&file_bytes, &params).context("render failed")?;

    fs::write(&args.output, &out.png_bytes)
        .with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!(
        "rendered: {} ({}x{}, k_global={}, total_symbols={}, payload_len={}, file_id={})",
        args.output.display(),
        out.width,
        out.height,
        out.k_global,
        out.total_symbols,
        out.payload_len,
        uuid::Uuid::from_bytes(out.file_id)
    );
    Ok(())
}

fn parse_aspect(s: &str) -> Result<ImageShape> {
    let (w_raw, h_raw) = s
        .split_once(':')
        .with_context(|| format!("aspect must be 'W:H', got {s:?}"))?;
    anyhow::ensure!(
        !w_raw.is_empty() && !h_raw.is_empty(),
        "aspect must be 'W:H', got {s:?}"
    );
    let w: u32 = w_raw
        .parse()
        .with_context(|| format!("aspect width in {s:?}"))?;
    let h: u32 = h_raw
        .parse()
        .with_context(|| format!("aspect height in {s:?}"))?;
    Ok(ImageShape::Aspect { w, h })
}
