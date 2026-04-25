use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_optical::{inspect, inspect_deep};

#[derive(ClapArgs)]
pub struct Args {
    /// Input PNG.
    pub input: PathBuf,

    /// Decode the first codec-A symbol to also show file_id/global_hash/file_size.
    #[arg(long)]
    pub deep: bool,
}

/// Run `zion optical-inspect [--deep]`.
///
/// # Errors
/// I/O or header validation errors.
pub fn run(args: Args) -> Result<()> {
    let png_bytes =
        fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;

    if args.deep {
        let m = inspect_deep(&png_bytes).context("inspect-deep failed")?;
        let o = m.optical;
        println!("optical            : {}", args.input.display());
        println!("dimensions         : {}x{}, RGB 24 bpp", o.width, o.height);
        println!("version            : 0x{:02x}", o.version);
        println!("header_len         : {}", o.header_len);
        println!("flags              : 0x{:04x}", o.flags);
        println!("k_global           : {}", o.k_global);
        println!("total_symbols      : {}", o.total_symbols);
        println!("payload_len        : {}", o.payload_len);
        println!("--- deep (from first symbol) ---");
        println!("file_id            : {}", uuid::Uuid::from_bytes(m.file_id));
        println!("global_hash        : {}", hex::encode(m.global_hash));
        println!("total_blocks       : {}", m.total_blocks);
        println!("file_size          : {}", m.file_size);
    } else {
        let o = inspect(&png_bytes).context("inspect failed")?;
        println!("optical            : {}", args.input.display());
        println!("dimensions         : {}x{}, RGB 24 bpp", o.width, o.height);
        println!("version            : 0x{:02x}", o.version);
        println!("header_len         : {}", o.header_len);
        println!("flags              : 0x{:04x}", o.flags);
        println!("k_global           : {}", o.k_global);
        println!("total_symbols      : {}", o.total_symbols);
        println!("payload_len        : {}", o.payload_len);
    }
    Ok(())
}
