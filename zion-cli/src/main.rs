use anyhow::Result;
use clap::{Parser, Subcommand};

mod cmd;

#[derive(Parser)]
#[command(name = "zion", version, about = "zioncode codec CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Encode a file into `.zbin` symbols.
    Encode(cmd::encode::Args),
    /// Decode `.zbin` symbols back into a file.
    Decode(cmd::decode::Args),
    /// Show metadata for a `.zbin` symbol.
    Inspect(cmd::inspect::Args),
    /// Hide a file inside PNG photos (steganography).
    StegoEmbed(cmd::stego_embed::Args),
    /// Extract a hidden file from stego photos.
    StegoExtract(cmd::stego_extract::Args),
    /// Show stego photo metadata (without passphrase).
    StegoInspect(cmd::stego_inspect::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Encode(args) => cmd::encode::run(args),
        Command::Decode(args) => cmd::decode::run(args),
        Command::Inspect(args) => cmd::inspect::run(args),
        Command::StegoEmbed(args) => cmd::stego_embed::run(args),
        Command::StegoExtract(args) => cmd::stego_extract::run(args),
        Command::StegoInspect(args) => cmd::stego_inspect::run(args),
    }
}
