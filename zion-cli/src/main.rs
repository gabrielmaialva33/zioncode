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
    /// Seal, open, and package Zion ARC exact-PNG capsules.
    Arc(cmd::arc::Args),
    /// Compile UTF-8 content into deterministic generative art.
    ArtRender(cmd::art_render::Args),
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
    /// Render a file as a 1-image PNG container (RGB 24 bpp lossless).
    OpticalRender(cmd::optical_render::Args),
    /// Extract a file from a zion-optical PNG.
    OpticalExtract(cmd::optical_extract::Args),
    /// Show optical header metadata. Use --deep to also decode the first codec-A symbol.
    OpticalInspect(cmd::optical_inspect::Args),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Arc(args) => cmd::arc::run(args),
        Command::ArtRender(args) => cmd::art_render::run(args),
        Command::Encode(args) => cmd::encode::run(args),
        Command::Decode(args) => cmd::decode::run(args),
        Command::Inspect(args) => cmd::inspect::run(args),
        Command::StegoEmbed(args) => cmd::stego_embed::run(args),
        Command::StegoExtract(args) => cmd::stego_extract::run(args),
        Command::StegoInspect(args) => cmd::stego_inspect::run(args),
        Command::OpticalRender(args) => cmd::optical_render::run(args),
        Command::OpticalExtract(args) => cmd::optical_extract::run(args),
        Command::OpticalInspect(args) => cmd::optical_inspect::run(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_command_tree_parses_all_required_subcommands() {
        for arguments in [
            vec![
                "zion", "arc", "capacity", "--width", "1080", "--height", "2340",
            ],
            vec![
                "zion",
                "arc",
                "seal",
                "book.json",
                "cover.png",
                "--output",
                "book.arc.png",
                "--passphrase-file",
                "passphrase.bin",
            ],
            vec![
                "zion",
                "arc",
                "open",
                "book.arc.png",
                "--output",
                "book.json",
                "--prompt",
            ],
            vec![
                "zion",
                "arc",
                "corrupt-fixture",
                "book.arc.png",
                "--output",
                "book.damaged.arc.png",
                "--passphrase-file",
                "passphrase.bin",
            ],
            vec![
                "zion",
                "arc",
                "blivre",
                "import",
                "source.zip",
                "--output-dir",
                "corpus",
            ],
            vec![
                "zion",
                "arc",
                "blivre",
                "analyze",
                "source.zip",
                "--width",
                "1080",
                "--height",
                "2340",
            ],
            vec![
                "zion",
                "arc",
                "blivre",
                "seal-batch",
                "source.zip",
                "--covers-dir",
                "covers",
                "--output-dir",
                "art",
                "--passphrase-file",
                "passphrase.bin",
            ],
        ] {
            Cli::try_parse_from(arguments).expect("required ARC command must parse");
        }
    }

    #[test]
    fn arc_never_accepts_a_direct_passphrase_option() {
        let result = Cli::try_parse_from([
            "zion",
            "arc",
            "seal",
            "book.json",
            "cover.png",
            "--output",
            "book.arc.png",
            "--passphrase",
            "must-not-be-accepted",
        ]);
        let error = match result {
            Ok(_) => panic!("a direct passphrase option must not exist"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("unexpected argument '--passphrase'")
        );
    }
}
