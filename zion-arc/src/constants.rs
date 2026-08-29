pub const ARC_VERSION: u8 = 1;
pub const BOOTSTRAP_LEN: usize = 64;
pub const BOOTSTRAP_CHANNELS: usize = BOOTSTRAP_LEN * 8;
pub const ENVELOPE_HEADER_LEN: usize = 64;
pub const AEAD_TAG_LEN: usize = 16;
pub const RS_N: usize = 255;
pub const MAX_CODEWORDS: usize = 4_096;
pub const DENSITY_NUMERATOR: usize = 33;
pub const DENSITY_DENOMINATOR: usize = 100;

pub const MAX_PNG_BYTES: usize = 134_217_728;
pub const MAX_PIXELS: usize = 4_194_304;
pub const MAX_DIMENSION: u32 = 16_384;
pub const MAX_RASTER_BYTES: usize = 12_582_912;
pub const MAX_PNG_DECODER_BYTES: usize = 16_777_216;
pub const MAX_ZSTD_DECODER_BYTES: usize = 20_971_520;
pub const MAX_PERMUTATION_INDICES: usize = 12_582_400;
pub const MAX_PERMUTATION_BYTES: usize = 50_329_600;
pub const MAX_ORIGINAL_LEN: usize = 16_777_216;
pub const MAX_COMPRESSED_LEN: usize = 1_000_000;
pub const MAX_NAME_LEN: usize = 255;
pub const MAX_MEDIA_TYPE_LEN: usize = 127;
pub const MAX_ATTRIBUTION_LEN: usize = 4_096;
pub const MAX_PASSPHRASE_LEN: usize = 1_024;

pub const SUITE_ID: u8 = 1;
pub const DENSITY_ID: u8 = 1;
pub const ZSTD_LEVEL: i32 = 6;
pub const ZSTD_WINDOW_LOG_MAX: u32 = 24;

pub const ARGON2_MEMORY_KIB: u32 = 65_536;
pub const ARGON2_ITERATIONS: u32 = 3;
pub const ARGON2_PARALLELISM: u32 = 4;
pub const ARGON2_OUTPUT_LEN: usize = 32;

pub const ARGON2_SALT_CONTEXT: &str = "zioncode ARC v1 Argon2id salt 2026-08-29";
pub const AEAD_KEY_CONTEXT: &str = "zioncode ARC v1 XChaCha20-Poly1305 key 2026-08-29";
pub const NONCE_CONTEXT: &str = "zioncode ARC v1 XChaCha20-Poly1305 nonce 2026-08-29";
pub const PERMUTATION_CONTEXT: &str = "zioncode ARC v1 carrier permutation seed 2026-08-29";
pub const MATCHING_CONTEXT: &str = "zioncode ARC v1 LSB matching seed 2026-08-29";

pub const BOOTSTRAP_MAGIC: [u8; 4] = *b"ZARC";
pub const ENVELOPE_MAGIC: [u8; 4] = *b"ARCE";

// libzstd 1.5.7's ZSTD_estimateDStreamSize(16 MiB). The actual context size is
// checked while decoding as well. This anchor is frozen by the ARC v1 spec.
pub const PINNED_DSTREAM_ESTIMATE: usize = 17_266_488;

const _: () = assert!(PINNED_DSTREAM_ESTIMATE <= MAX_ZSTD_DECODER_BYTES);
const _: () = assert!(MAX_PERMUTATION_INDICES * size_of::<u32>() == MAX_PERMUTATION_BYTES);
