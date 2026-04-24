use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_stego::constants::{PLAINTEXT_HEADER_CHANNELS, PLAINTEXT_HEADER_LEN};
use zion_stego::lsb::{bits_to_bytes, extract_bits_at};
use zion_stego::plaintext_header::PlaintextHeader;
use zion_stego::png_io::load_png_rgb;

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

    let expected = (img.width as usize) * (img.height as usize) * 3;
    anyhow::ensure!(
        img.rgb_data.len() == expected,
        "dimension mismatch: expected {expected} bytes, got {}",
        img.rgb_data.len()
    );
    anyhow::ensure!(
        img.rgb_data.len() >= PLAINTEXT_HEADER_CHANNELS,
        "image too small to contain a plaintext header"
    );

    let header_positions: Vec<u32> = (0..PLAINTEXT_HEADER_CHANNELS as u32).collect();
    let header_bits = extract_bits_at(&img.rgb_data, &header_positions);
    let header_bytes_vec = bits_to_bytes(&header_bits);
    let header_bytes: [u8; PLAINTEXT_HEADER_LEN] = header_bytes_vec
        .as_slice()
        .try_into()
        .expect("256 bits = 32 bytes");

    let h = PlaintextHeader::parse(&header_bytes, 0)
        .map_err(|e| anyhow::anyhow!("parsing header: {e}"))?;

    println!("stego             : {}", args.stego.display());
    println!("dimensions        : {}×{} RGB", img.width, img.height);
    println!("file_id           : {}", uuid::Uuid::from_bytes(h.file_id));
    println!("k_global          : {}", h.k_global);
    println!("symbol_index      : {}", h.symbol_index);
    println!("total_symbols     : {}", h.total_symbols);
    println!("ciphertext_len    : {}", h.ciphertext_len);
    Ok(())
}
