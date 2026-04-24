use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_stego::{inspect_stego_image, load_png_rgb};

#[derive(ClapArgs)]
pub struct Args {
    /// Stego PNG photo to inspect.
    pub stego: PathBuf,
}

/// Runs the `zion stego-inspect` subcommand (no passphrase required).
///
/// Reads the public plaintext header of a stego photo and prints its metadata.
///
/// # Errors
/// I/O errors or invalid header.
pub fn run(args: Args) -> Result<()> {
    let file =
        fs::File::open(&args.stego).with_context(|| format!("opening {}", args.stego.display()))?;
    let img =
        load_png_rgb(file).map_err(|e| anyhow::anyhow!("reading {}: {e}", args.stego.display()))?;

    let metadata =
        inspect_stego_image(&img, 0).map_err(|e| anyhow::anyhow!("parsing header: {e}"))?;

    println!("stego             : {}", args.stego.display());
    println!(
        "dimensions        : {}x{} RGB",
        metadata.width, metadata.height
    );
    println!(
        "file_id           : {}",
        uuid::Uuid::from_bytes(metadata.file_id)
    );
    println!("k_global          : {}", metadata.k_global);
    println!("symbol_index      : {}", metadata.symbol_index);
    println!("total_symbols     : {}", metadata.total_symbols);
    println!("ciphertext_len    : {}", metadata.ciphertext_len);
    Ok(())
}
