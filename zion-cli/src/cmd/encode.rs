use anyhow::{Context, Result};
use clap::{Args as ClapArgs, ValueEnum};
use std::fs;
use std::path::PathBuf;
use zion_codec::{EccProfile, Encoder, EncoderConfig};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ProfileArg {
    /// RS(255,223), strongest default profile.
    Safe,
    /// RS(255,239), lower overhead with less correction budget.
    Balanced,
    /// RS(255,247), lowest overhead with the smallest correction budget.
    Dense,
}

impl From<ProfileArg> for EccProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Safe => Self::Safe,
            ProfileArg::Balanced => Self::Balanced,
            ProfileArg::Dense => Self::Dense,
        }
    }
}

#[derive(ClapArgs)]
pub struct Args {
    /// Input file.
    pub input: PathBuf,

    /// Output prefix. Generated files: `<prefix>_000.zbin`, `<prefix>_001.zbin`, etc.
    #[arg(short = 'o', long = "output-prefix")]
    pub output_prefix: PathBuf,

    /// Number of RS codewords per symbol. If omitted, the encoder picks the smallest output.
    #[arg(long)]
    pub k: Option<u16>,

    /// ECC density profile.
    #[arg(long, value_enum, default_value_t = ProfileArg::Safe)]
    pub profile: ProfileArg,

    /// zstd compression level (1-22).
    #[arg(long, default_value_t = EncoderConfig::default().zstd_level)]
    pub zstd_level: i32,
}

/// Run the `zion encode` command.
///
/// # Errors
/// Propagates I/O or encoding errors.
pub fn run(args: Args) -> Result<()> {
    let raw = fs::read(&args.input).with_context(|| format!("reading {}", args.input.display()))?;
    let config = EncoderConfig {
        k: args.k,
        zstd_level: args.zstd_level,
        ecc_profile: EccProfile::from(args.profile),
    };
    let encoded = Encoder::new(config).encode(&raw).context("encode_file")?;

    let prefix_file_name = args
        .output_prefix
        .file_name()
        .context("output-prefix has no file name")?
        .to_string_lossy()
        .into_owned();

    for (i, sym) in encoded.symbols.iter().enumerate() {
        let filename = format!("{prefix_file_name}_{i:03}.zbin");
        let path = args.output_prefix.with_file_name(filename);
        fs::write(&path, sym).with_context(|| format!("writing {}", path.display()))?;
        eprintln!("written: {}", path.display());
    }

    eprintln!(
        "{} symbols generated (file_id={}, k={}, profile={})",
        encoded.symbols.len(),
        encoded.file_id,
        encoded.k,
        encoded.ecc_profile.name()
    );
    Ok(())
}
