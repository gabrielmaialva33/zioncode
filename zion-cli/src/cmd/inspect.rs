use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_codec::decode::decode_symbol;
use zion_codec::ecc::EccProfile;

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
    let decoded = decode_symbol(&bytes)
        .with_context(|| format!("decode_symbol for {}", args.symbol.display()))?;
    let h = &decoded.header;

    println!("symbol            : {}", args.symbol.display());
    println!("bytes_transmitted : {}", bytes.len());
    println!(
        "ecc_profile       : {}",
        EccProfile::from_header_flags(h.flags)
            .map(EccProfile::name)
            .unwrap_or("unknown")
    );
    println!("file_id           : {}", uuid::Uuid::from_bytes(h.file_id));
    println!("file_size         : {}", h.file_size);
    println!("total_blocks      : {}", h.total_blocks);
    println!("total_symbols     : {}", h.total_symbols);
    println!("symbol_index      : {}", h.symbol_index);
    println!("block_start       : {}", h.block_start);
    println!("block_count       : {}", h.block_count);
    println!("global_hash       : {}", hex::encode(h.global_hash));

    let ok = decoded.blocks.iter().filter(|r| r.is_ok()).count();
    let err = decoded.blocks.len() - ok;
    println!("blocks_ok         : {ok}");
    println!("blocks_error      : {err}");
    Ok(())
}
