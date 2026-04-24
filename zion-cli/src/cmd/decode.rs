use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::fs;
use std::path::PathBuf;
use zion_codec::{Decoder, FileError, FileReassembler, FinalizedFile, HashStatus};

#[derive(ClapArgs)]
pub struct Args {
    /// `.zbin` files in any order.
    pub symbols: Vec<PathBuf>,

    /// Output file.
    #[arg(short = 'o', long)]
    pub output: PathBuf,

    /// Write the output even when the global hash does not match (forensics only).
    #[arg(long)]
    pub force_write_corrupt: bool,
}

/// Run the `zion decode` command.
///
/// # Errors
/// Propagates I/O, symbol decode, or reassembly errors.
pub fn run(args: Args) -> Result<()> {
    anyhow::ensure!(!args.symbols.is_empty(), "provide at least one symbol");

    let decoder = Decoder::default();
    let mut reasm = FileReassembler::new();
    for path in &args.symbols {
        let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let decoded = decoder
            .decode_symbol(&bytes)
            .with_context(|| format!("decode_symbol for {}", path.display()))?;
        reasm
            .add_symbol(decoded)
            .with_context(|| format!("add_symbol for {}", path.display()))?;
        eprintln!("ok: {}", path.display());
    }

    let finalized = match reasm.finalize_report() {
        Ok(report) => report,
        Err(e) => {
            eprintln!("reassembly failed: {e}");
            return Err(e.into());
        }
    };
    let FinalizedFile {
        bytes: raw,
        hash_status,
    } = finalized;
    if let HashStatus::Mismatch { expected, got } = hash_status {
        let err = FileError::GlobalHashMismatch { expected, got };
        eprintln!("reassembly failed: {err}");
        if !args.force_write_corrupt {
            return Err(err.into());
        }
        eprintln!("writing output despite hash mismatch because --force-write-corrupt was set");
    }

    fs::write(&args.output, &raw).with_context(|| format!("writing {}", args.output.display()))?;
    eprintln!(
        "reassembled file: {} ({} bytes)",
        args.output.display(),
        raw.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zion_codec::{
        ZSTD_LEVEL_DEFAULT, decode_symbol,
        low_level::{decode_block, encode_file, encode_single_symbol},
        wire::BlockEntry,
    };

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("zion-cli-{name}-{unique}"))
    }

    #[test]
    fn force_write_corrupt_writes_reassembled_bytes() {
        let mut raw = vec![0u8; 4096];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(31).wrapping_add(7);
        }

        let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
        let decoded = decode_symbol(&encoded.symbols[0]).unwrap();
        let header = decoded.metadata.to_header();
        let mut blocks: Vec<BlockEntry> = decoded.blocks.into_iter().map(Result::unwrap).collect();
        let expected_raw_size = u16::try_from(raw.len()).unwrap();
        let mut mutated = decode_block(&blocks[0], 0, expected_raw_size).unwrap();
        mutated[0] ^= 0xFF;
        blocks[0] = BlockEntry::raw(mutated.clone()).unwrap();
        let corrupt_symbol = encode_single_symbol(&header, &blocks, 38);

        let symbol_path = temp_path("symbol");
        let output_path = temp_path("output");
        fs::write(&symbol_path, corrupt_symbol).unwrap();

        let result = run(Args {
            symbols: vec![symbol_path.clone()],
            output: output_path.clone(),
            force_write_corrupt: true,
        });

        assert!(result.is_ok());
        let written = fs::read(&output_path).unwrap();
        assert_eq!(written, mutated);
        assert_ne!(written, raw);

        let _ = fs::remove_file(symbol_path);
        let _ = fs::remove_file(output_path);
    }
}
