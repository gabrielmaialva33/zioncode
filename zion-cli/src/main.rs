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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Encode(args) => cmd::encode::run(args),
        Command::Decode(args) => cmd::decode::run(args),
        Command::Inspect(args) => cmd::inspect::run(args),
    }
}
