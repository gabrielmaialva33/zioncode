use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_codec::Decoder;

#[derive(ClapArgs)]
pub struct Args {
    /// `.zbin` file to inspect.
    pub symbol: PathBuf,
}

/// Run the `zion inspect` command and print symbol metadata.
///
/// # Errors
/// Propagates I/O or symbol decode errors.
pub fn run(args: Args) -> Result<()> {
    let bytes =
        fs::read(&args.symbol).with_context(|| format!("reading {}", args.symbol.display()))?;
    let decoded = Decoder::default()
        .decode_symbol(&bytes)
        .with_context(|| format!("decode_symbol for {}", args.symbol.display()))?;
    let metadata = &decoded.metadata;

    println!("symbol            : {}", args.symbol.display());
    println!("bytes_transmitted : {}", bytes.len());
    println!("ecc_profile       : {}", metadata.ecc_profile.name());
    println!("file_id           : {}", metadata.file_id);
    println!("file_size         : {}", metadata.file_size);
    println!("total_blocks      : {}", metadata.total_blocks);
    println!("total_symbols     : {}", metadata.total_symbols);
    println!("symbol_index      : {}", metadata.symbol_index);
    println!("block_start       : {}", metadata.block_start);
    println!("block_count       : {}", metadata.block_count);
    println!(
        "global_hash       : {}",
        hex::encode(metadata.global_hash.to_bytes())
    );

    let ok = decoded.blocks_ok();
    let err = decoded.blocks_error();
    println!("blocks_ok         : {ok}");
    println!("blocks_error      : {err}");
    Ok(())
}
