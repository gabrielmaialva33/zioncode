//! Deterministic information-to-art compiler for zioncode.
//!
//! This crate owns the low-frequency visual identity of an artifact. It does
//! not hide or recover the exact payload: cryptographic and robust transport
//! layers remain separate so that visual meaning is never confused with byte
//! integrity.

#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeSet;

use thiserror::Error;
use zion_stego::{RgbImage, save_png_rgb};

/// Version of the experimental deterministic visual grammar.
pub const ART_GRAMMAR_VERSION: u8 = 0;
/// Maximum input accepted by one compiler invocation.
pub const MAX_CONTENT_BYTES: usize = 16 * 1024 * 1024;
/// Smallest supported raster edge.
pub const MIN_DIMENSION: u32 = 64;
/// Largest supported raster edge.
pub const MAX_DIMENSION: u32 = 4_096;

const AXIS_LIMIT: i16 = 1_000;
const GENOME_CONTEXT: &str = "org.zioncode.art.genome.v0";

/// Stable semantic dimensions that control macro composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MeaningAxis {
    Warmth = 0,
    Motion = 1,
    Density = 2,
    Tension = 3,
    Intimacy = 4,
    Transcendence = 5,
    Order = 6,
    Radiance = 7,
}

/// Quantized semantic input, independent from any particular AI model.
///
/// Each axis is in `-1000..=1000`. A future local model can produce this
/// compact contract while the renderer remains deterministic and auditable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeaningProfile {
    axes: [i16; 8],
}

impl MeaningProfile {
    /// Creates a checked semantic profile.
    ///
    /// # Errors
    ///
    /// Returns [`ArtError::MeaningOutOfRange`] when any axis lies outside the
    /// stable quantized range.
    pub fn new(axes: [i16; 8]) -> Result<Self, ArtError> {
        if axes
            .iter()
            .any(|value| !(-AXIS_LIMIT..=AXIS_LIMIT).contains(value))
        {
            return Err(ArtError::MeaningOutOfRange);
        }
        Ok(Self { axes })
    }

    /// A model-free profile for structural experiments.
    #[must_use]
    pub const fn neutral() -> Self {
        Self { axes: [0; 8] }
    }

    /// Returns one stable semantic axis.
    #[must_use]
    pub const fn value(self, axis: MeaningAxis) -> i16 {
        self.axes[axis as usize]
    }

    /// Returns all axes in their wire-independent canonical order.
    #[must_use]
    pub const fn values(self) -> [i16; 8] {
        self.axes
    }
}

impl Default for MeaningProfile {
    fn default() -> Self {
        Self::neutral()
    }
}

/// Deterministic text-shape measurements used by the visual grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentStructure {
    pub bytes: u32,
    pub characters: u32,
    pub words: u32,
    pub sentences: u32,
    pub lines: u32,
    pub paragraphs: u32,
    pub lexical_diversity_per_10k: u16,
    pub punctuation_per_10k: u16,
    pub cadence: [u16; 8],
}

/// Four-color palette emitted by the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: [u8; 3],
    pub shadow: [u8; 3],
    pub primary: [u8; 3],
    pub accent: [u8; 3],
}

/// Stable, inspectable description of one content-derived artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtGenome {
    pub version: u8,
    pub content_hash: [u8; 32],
    pub visual_seed: [u8; 32],
    pub meaning: MeaningProfile,
    pub structure: ContentStructure,
    pub palette: Palette,
    pub flow_direction: u8,
    pub layer_count: u8,
    pub texture_scale: u16,
    pub flow_period: u16,
}

/// Raster configuration for the deterministic renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderParams {
    pub width: u32,
    pub height: u32,
    pub meaning: MeaningProfile,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            width: 1_080,
            height: 2_340,
            meaning: MeaningProfile::neutral(),
        }
    }
}

/// Result of compiling and rendering content as art.
#[derive(Debug)]
pub struct RenderOutput {
    pub png_bytes: Vec<u8>,
    pub genome: ArtGenome,
    pub raster_hash: [u8; 32],
}

#[derive(Debug, Clone, Copy)]
struct PixelSignals {
    field: u32,
    ordered_field: u16,
    band: u16,
    cadence: i64,
    river_distance: u64,
    river_width: u32,
    edge_x: u32,
    edge_y: u32,
    width: u32,
    height: u32,
}

/// Failures produced by the experimental art compiler.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum ArtError {
    #[error("content is empty")]
    EmptyContent,
    #[error("content exceeds the compiler limit")]
    ContentTooLarge,
    #[error("content is not valid UTF-8")]
    InvalidUtf8,
    #[error("semantic axis is outside -1000..=1000")]
    MeaningOutOfRange,
    #[error("invalid render dimensions")]
    InvalidDimensions,
    #[error("render arithmetic overflow")]
    ArithmeticOverflow,
    #[error("PNG encoding failed")]
    PngEncodeFailed,
}

/// Compiles UTF-8 content and a semantic profile into a stable visual genome.
///
/// # Errors
///
/// Rejects empty, oversized, or invalid UTF-8 content.
pub fn compile(content: &[u8], meaning: MeaningProfile) -> Result<ArtGenome, ArtError> {
    if content.is_empty() {
        return Err(ArtError::EmptyContent);
    }
    if content.len() > MAX_CONTENT_BYTES {
        return Err(ArtError::ContentTooLarge);
    }
    let text = std::str::from_utf8(content).map_err(|_| ArtError::InvalidUtf8)?;
    let content_hash = *blake3::hash(content).as_bytes();
    let structure = measure_structure(text)?;
    let visual_seed = derive_visual_seed(content_hash, meaning, structure);
    let palette = derive_palette(visual_seed, meaning);
    let density = i32::from(meaning.value(MeaningAxis::Density));
    let motion = i32::from(meaning.value(MeaningAxis::Motion));
    let layer_count = u8::try_from((6 + density.div_euclid(250)).clamp(3, 10))
        .map_err(|_| ArtError::ArithmeticOverflow)?;
    let texture_scale = u16::try_from(
        (96 - density.div_euclid(20) + i32::from(visual_seed[4] % 24)).clamp(32, 192),
    )
    .map_err(|_| ArtError::ArithmeticOverflow)?;
    let flow_period =
        u16::try_from((180 - motion.div_euclid(12) + i32::from(visual_seed[5])).clamp(72, 360))
            .map_err(|_| ArtError::ArithmeticOverflow)?;

    Ok(ArtGenome {
        version: ART_GRAMMAR_VERSION,
        content_hash,
        visual_seed,
        meaning,
        structure,
        palette,
        flow_direction: visual_seed[3] % 8,
        layer_count,
        texture_scale,
        flow_period,
    })
}

/// Compiles content and renders its deterministic RGB artwork as PNG.
///
/// # Errors
///
/// Returns a typed error for invalid input, dimensions, arithmetic, or PNG
/// encoding.
pub fn render(content: &[u8], params: RenderParams) -> Result<RenderOutput, ArtError> {
    validate_dimensions(params.width, params.height)?;
    let genome = compile(content, params.meaning)?;
    let image = render_rgb(genome, params.width, params.height)?;
    let raster_hash = *blake3::hash(&image.rgb_data).as_bytes();
    let mut png_bytes = Vec::new();
    save_png_rgb(&image, &mut png_bytes).map_err(|_| ArtError::PngEncodeFailed)?;
    Ok(RenderOutput {
        png_bytes,
        genome,
        raster_hash,
    })
}

fn measure_structure(text: &str) -> Result<ContentStructure, ArtError> {
    let characters = text.chars().count();
    let tokens = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    let words = tokens.len();
    let unique = tokens
        .iter()
        .map(|token| token.to_lowercase())
        .collect::<BTreeSet<_>>()
        .len();
    let sentence_lengths = text
        .split(['.', '?', '!', ';'])
        .map(|sentence| sentence.split_whitespace().count())
        .filter(|&length| length > 0)
        .collect::<Vec<_>>();
    let sentences = sentence_lengths.len().max(usize::from(!tokens.is_empty()));
    let lines = text.lines().filter(|line| !line.trim().is_empty()).count();
    let paragraphs = text
        .split("\n\n")
        .filter(|paragraph| !paragraph.trim().is_empty())
        .count();
    let punctuation = text.chars().filter(char::is_ascii_punctuation).count();
    let mut cadence_sum = [0u64; 8];
    let mut cadence_count = [0u64; 8];
    for (index, length) in sentence_lengths.iter().copied().enumerate() {
        let bin = index % 8;
        cadence_sum[bin] = cadence_sum[bin]
            .checked_add(u64::try_from(length).map_err(|_| ArtError::ArithmeticOverflow)?)
            .ok_or(ArtError::ArithmeticOverflow)?;
        cadence_count[bin] += 1;
    }
    let averages = std::array::from_fn(|index| cadence_sum[index] / cadence_count[index].max(1));
    let maximum = averages.iter().copied().max().unwrap_or(0).max(1);
    let cadence = averages.map(|average| {
        u16::try_from(average.saturating_mul(u64::from(u16::MAX)) / maximum).unwrap_or(u16::MAX)
    });

    Ok(ContentStructure {
        bytes: to_u32(text.len())?,
        characters: to_u32(characters)?,
        words: to_u32(words)?,
        sentences: to_u32(sentences)?,
        lines: to_u32(lines.max(1))?,
        paragraphs: to_u32(paragraphs.max(1))?,
        lexical_diversity_per_10k: ratio_per_10k(unique, words.max(1))?,
        punctuation_per_10k: ratio_per_10k(punctuation, characters.max(1))?,
        cadence,
    })
}

fn derive_visual_seed(
    content_hash: [u8; 32],
    meaning: MeaningProfile,
    structure: ContentStructure,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key(GENOME_CONTEXT);
    hasher.update(&content_hash);
    for value in meaning.values() {
        hasher.update(&value.to_be_bytes());
    }
    for value in structure.cadence {
        hasher.update(&value.to_be_bytes());
    }
    hasher.update(&structure.words.to_be_bytes());
    hasher.update(&structure.sentences.to_be_bytes());
    *hasher.finalize().as_bytes()
}

fn derive_palette(seed: [u8; 32], meaning: MeaningProfile) -> Palette {
    let warmth = i32::from(meaning.value(MeaningAxis::Warmth));
    let tension = i32::from(meaning.value(MeaningAxis::Tension));
    let radiance = i32::from(meaning.value(MeaningAxis::Radiance));
    let hash_hue = i32::from(u16::from_be_bytes([seed[0], seed[1]]) % 360);
    let warm_target = if warmth >= 0 { 38 } else { 192 };
    let warmth_weight = warmth.unsigned_abs().min(1_000);
    let base_hue = blend_hue(hash_hue, warm_target, warmth_weight);
    let accent_shift = if tension >= 0 { 148 } else { 54 };
    let light = radiance.div_euclid(25);
    Palette {
        background: hsv_to_rgb(base_hue, 78, clamp_u8(24 + light.div_euclid(4))),
        shadow: hsv_to_rgb(
            (base_hue + 22) % 360,
            64,
            clamp_u8(42 + light.div_euclid(3)),
        ),
        primary: hsv_to_rgb((base_hue + 348) % 360, 72, clamp_u8(166 + light)),
        accent: hsv_to_rgb(
            (base_hue + accent_shift) % 360,
            clamp_u8(68 + tension.div_euclid(40)),
            clamp_u8(212 + light),
        ),
    }
}

fn render_rgb(genome: ArtGenome, width: u32, height: u32) -> Result<RgbImage, ArtError> {
    let pixels = usize::try_from(width)
        .ok()
        .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
        .ok_or(ArtError::ArithmeticOverflow)?;
    let channels = pixels.checked_mul(3).ok_or(ArtError::ArithmeticOverflow)?;
    let mut rgb_data = Vec::with_capacity(channels);
    let seed = u64::from_be_bytes(
        genome.visual_seed[..8]
            .try_into()
            .map_err(|_| ArtError::ArithmeticOverflow)?,
    );
    let directions = [
        (3i64, 1i64),
        (2, 2),
        (1, 3),
        (-1, 3),
        (-3, 1),
        (-2, -2),
        (1, -3),
        (3, -1),
    ];
    let (direction_x, direction_y) = directions[usize::from(genome.flow_direction)];
    let order = i32::from(genome.meaning.value(MeaningAxis::Order));
    let intimacy = i32::from(genome.meaning.value(MeaningAxis::Intimacy));
    let transcendence = i32::from(genome.meaning.value(MeaningAxis::Transcendence));
    let base_scale = u32::from(genome.texture_scale);
    let base_period = i64::from(genome.flow_period);
    let river_width = (width / u32::from(5 + genome.layer_count / 2)).max(24);

    for y in 0..height {
        let cadence = i64::from(cadence_at(genome.structure.cadence, y, height));
        let period = (base_period + cadence / 1_024 - i64::from(intimacy) / 20).max(32);
        let river_noise = value_noise(seed.rotate_left(41), 0, y, (height / 7).max(16), 7);
        let river_offset = (i64::from(river_noise) - 32_768) * i64::from(width) / 98_304;
        let river_center =
            (i64::from(width) / 2 + river_offset).clamp(0, i64::from(width.saturating_sub(1)));
        for x in 0..width {
            let coarse = value_noise(seed, x, y, base_scale.max(8), 0);
            let medium = value_noise(seed.rotate_left(17), x, y, (base_scale / 2).max(4), 1);
            let fine = value_noise(seed.rotate_left(33), x, y, (base_scale / 4).max(2), 2);
            let field = (u32::from(coarse) * 4 + u32::from(medium) * 2 + u32::from(fine)) / 7;
            let warp = i64::from(field) / 96;
            let phase = (i64::from(x) * direction_x
                + i64::from(y) * direction_y
                + warp
                + i64::from(transcendence) * i64::from(y) / i64::from(height))
            .rem_euclid(period);
            let triangle = (phase - period / 2).unsigned_abs() * 2;
            let band = u16::try_from(
                65_535u64.saturating_sub(triangle.saturating_mul(65_535) / period.cast_unsigned()),
            )
            .unwrap_or(0);
            let ordered_field = if order >= 0 {
                blend_u16(
                    u16::try_from(field).unwrap_or(u16::MAX),
                    band,
                    order.cast_unsigned(),
                )
            } else {
                let disorder = lattice(seed.rotate_right(11), x, y, 9);
                blend_u16(
                    u16::try_from(field).unwrap_or(u16::MAX),
                    disorder,
                    order.unsigned_abs(),
                )
            };
            let distance = (i64::from(x) - river_center).unsigned_abs();
            let color = compose_color(
                genome,
                PixelSignals {
                    field,
                    ordered_field,
                    band,
                    cadence,
                    river_distance: distance,
                    river_width,
                    edge_x: x.min(width - 1 - x),
                    edge_y: y.min(height - 1 - y),
                    width,
                    height,
                },
            );
            let grain = i16::from(u8::try_from(lattice(seed, x, y, 13) & 7).unwrap()) - 3;
            for channel in color {
                rgb_data.push(u8::try_from((i16::from(channel) + grain).clamp(0, 255)).unwrap());
            }
        }
    }
    Ok(RgbImage {
        width,
        height,
        rgb_data,
    })
}

fn compose_color(genome: ArtGenome, signals: PixelSignals) -> [u8; 3] {
    let shaped_field =
        u16::try_from(u32::from(signals.ordered_field) * u32::from(signals.ordered_field) / 65_535)
            .unwrap_or(u16::MAX);
    let mut color = mix_color(
        genome.palette.background,
        genome.palette.primary,
        shaped_field,
    );
    if signals.river_distance < u64::from(signals.river_width) {
        let river_weight = u16::try_from(
            (u64::from(signals.river_width) - signals.river_distance) * 48_000
                / u64::from(signals.river_width),
        )
        .unwrap_or(48_000);
        color = mix_color(color, genome.palette.primary, river_weight);
    }
    let highlight_threshold = 59_000u16.saturating_sub(
        u16::try_from(signals.cadence / 24)
            .unwrap_or(3_000)
            .min(3_000),
    );
    if signals.band > highlight_threshold {
        let strength = signals
            .band
            .saturating_sub(highlight_threshold)
            .saturating_mul(2);
        color = mix_color(color, genome.palette.accent, strength);
    } else if signals.field < 12_000 {
        color = mix_color(
            color,
            genome.palette.shadow,
            24_000 - u16::try_from(signals.field).unwrap_or(u16::MAX),
        );
    }
    let edge = u64::from(signals.edge_x) * u64::from(signals.edge_y) * 65_535
        / (u64::from(signals.width / 2).max(1) * u64::from(signals.height / 2).max(1));
    let vignette = u16::try_from(edge.min(65_535)).unwrap_or(u16::MAX);
    mix_color(genome.palette.background, color, 24_000 + vignette / 2)
}

fn cadence_at(cadence: [u16; 8], y: u32, height: u32) -> u16 {
    let position = u64::from(y) * 8 * 65_535 / u64::from(height);
    let index = usize::try_from(position / 65_535).unwrap_or(7).min(7);
    let next = (index + 1).min(7);
    let fraction = u16::try_from(position % 65_535).unwrap_or(0);
    lerp_u16(cadence[index], cadence[next], smoothstep(fraction))
}

fn value_noise(seed: u64, x: u32, y: u32, scale: u32, octave: u8) -> u16 {
    let grid_x = x / scale;
    let grid_y = y / scale;
    let fraction_x = u16::try_from(u64::from(x % scale) * 65_535 / u64::from(scale)).unwrap();
    let fraction_y = u16::try_from(u64::from(y % scale) * 65_535 / u64::from(scale)).unwrap();
    let smooth_x = smoothstep(fraction_x);
    let smooth_y = smoothstep(fraction_y);
    let top = lerp_u16(
        lattice(seed, grid_x, grid_y, octave),
        lattice(seed, grid_x + 1, grid_y, octave),
        smooth_x,
    );
    let bottom = lerp_u16(
        lattice(seed, grid_x, grid_y + 1, octave),
        lattice(seed, grid_x + 1, grid_y + 1, octave),
        smooth_x,
    );
    lerp_u16(top, bottom, smooth_y)
}

fn lattice(seed: u64, x: u32, y: u32, octave: u8) -> u16 {
    let value = seed
        ^ u64::from(x).wrapping_mul(0x9e37_79b1_85eb_ca87)
        ^ u64::from(y).wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
        ^ u64::from(octave).wrapping_mul(0x1656_67b1_9e37_79f9);
    u16::try_from(mix64(value) >> 48).unwrap()
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn smoothstep(value: u16) -> u16 {
    let value = u64::from(value);
    let squared = value * value / 65_535;
    u16::try_from(squared * (3 * 65_535 - 2 * value) / 65_535).unwrap_or(u16::MAX)
}

fn lerp_u16(first: u16, second: u16, weight: u16) -> u16 {
    let first = i64::from(first);
    let difference = i64::from(second) - first;
    u16::try_from(first + difference * i64::from(weight) / 65_535).unwrap_or(0)
}

fn blend_u16(first: u16, second: u16, weight_per_1k: u32) -> u16 {
    let weight = u16::try_from(weight_per_1k.min(1_000) * 65_535 / 1_000).unwrap();
    lerp_u16(first, second, weight)
}

fn mix_color(first: [u8; 3], second: [u8; 3], weight: u16) -> [u8; 3] {
    std::array::from_fn(|index| {
        u8::try_from(
            lerp_u16(
                u16::from(first[index]) * 257,
                u16::from(second[index]) * 257,
                weight,
            ) / 257,
        )
        .unwrap()
    })
}

fn hsv_to_rgb(hue: i32, saturation: u8, value: u8) -> [u8; 3] {
    let hue = hue.rem_euclid(360);
    let region = hue / 60;
    let remainder = (hue % 60) * 255 / 60;
    let value_i = i32::from(value);
    let saturation_i = i32::from(saturation);
    let p = value_i * (255 - saturation_i) / 255;
    let q = value_i * (255 - saturation_i * remainder / 255) / 255;
    let t = value_i * (255 - saturation_i * (255 - remainder) / 255) / 255;
    let values = match region {
        0 => [value_i, t, p],
        1 => [q, value_i, p],
        2 => [p, value_i, t],
        3 => [p, q, value_i],
        4 => [t, p, value_i],
        _ => [value_i, p, q],
    };
    values.map(|component| u8::try_from(component).unwrap())
}

fn blend_hue(source: i32, target: i32, weight_per_1k: u32) -> i32 {
    let delta = (target - source + 540).rem_euclid(360) - 180;
    (source + delta * i32::try_from(weight_per_1k).unwrap() / 1_000).rem_euclid(360)
}

fn clamp_u8(value: i32) -> u8 {
    u8::try_from(value.clamp(0, 255)).unwrap()
}

fn ratio_per_10k(numerator: usize, denominator: usize) -> Result<u16, ArtError> {
    let numerator = u64::try_from(numerator).map_err(|_| ArtError::ArithmeticOverflow)?;
    let denominator = u64::try_from(denominator).map_err(|_| ArtError::ArithmeticOverflow)?;
    u16::try_from(numerator.saturating_mul(10_000) / denominator.max(1))
        .map_err(|_| ArtError::ArithmeticOverflow)
}

fn to_u32(value: usize) -> Result<u32, ArtError> {
    u32::try_from(value).map_err(|_| ArtError::ArithmeticOverflow)
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ArtError> {
    if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&width)
        || !(MIN_DIMENSION..=MAX_DIMENSION).contains(&height)
    {
        return Err(ArtError::InvalidDimensions);
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > u64::from(MAX_DIMENSION) * u64::from(MAX_DIMENSION) {
        return Err(ArtError::InvalidDimensions);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zion_stego::load_png_rgb;

    const TEXT: &[u8] = "No princípio havia ritmo.\n\nE o ritmo encontrou luz!".as_bytes();

    #[test]
    fn meaning_profile_enforces_quantized_contract() {
        assert_eq!(
            MeaningProfile::neutral(),
            MeaningProfile::new([0; 8]).unwrap()
        );
        assert_eq!(
            ArtError::MeaningOutOfRange,
            MeaningProfile::new([0, 0, 0, 0, 0, 0, 0, 1_001]).unwrap_err()
        );
    }

    #[test]
    fn compiler_is_deterministic_and_content_sensitive() {
        let first = compile(TEXT, MeaningProfile::neutral()).unwrap();
        let second = compile(TEXT, MeaningProfile::neutral()).unwrap();
        let changed = compile(
            b"No principio havia outro ritmo.",
            MeaningProfile::neutral(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_ne!(first.content_hash, changed.content_hash);
        assert_ne!(first.visual_seed, changed.visual_seed);
    }

    #[test]
    fn semantic_profile_changes_macro_art_not_content_identity() {
        let warm = MeaningProfile::new([900, 300, 0, 0, 0, 400, 0, 300]).unwrap();
        let cool = MeaningProfile::new([-900, -300, 0, 0, 0, -400, 0, -300]).unwrap();
        let warm = compile(TEXT, warm).unwrap();
        let cool = compile(TEXT, cool).unwrap();
        assert_eq!(warm.content_hash, cool.content_hash);
        assert_ne!(warm.palette, cool.palette);
        assert_ne!(warm.visual_seed, cool.visual_seed);
    }

    #[test]
    fn renderer_png_roundtrips_exact_raster() {
        let params = RenderParams {
            width: 96,
            height: 128,
            meaning: MeaningProfile::neutral(),
        };
        let first = render(TEXT, params).unwrap();
        let second = render(TEXT, params).unwrap();
        assert_eq!(
            [
                0xb0, 0x51, 0x3e, 0xab, 0x3c, 0x26, 0x69, 0x3d, 0x01, 0xa5, 0x95, 0x5f, 0xf8, 0xe3,
                0xf8, 0xee, 0xd7, 0xe8, 0xb6, 0xed, 0x8b, 0xbc, 0x48, 0xea, 0xa4, 0x89, 0x77, 0x0e,
                0xff, 0xa5, 0xc0, 0x07,
            ],
            first.raster_hash
        );
        assert_eq!(first.raster_hash, second.raster_hash);
        assert_eq!(first.png_bytes, second.png_bytes);
        let decoded = load_png_rgb(first.png_bytes.as_slice()).unwrap();
        assert_eq!(decoded.width, params.width);
        assert_eq!(decoded.height, params.height);
        assert_eq!(
            *blake3::hash(&decoded.rgb_data).as_bytes(),
            first.raster_hash
        );
    }

    #[test]
    fn invalid_inputs_are_rejected_before_rendering() {
        assert_eq!(
            ArtError::EmptyContent,
            compile(&[], MeaningProfile::neutral()).unwrap_err()
        );
        assert_eq!(
            ArtError::InvalidUtf8,
            compile(&[0xff], MeaningProfile::neutral()).unwrap_err()
        );
        assert_eq!(
            ArtError::InvalidDimensions,
            render(
                TEXT,
                RenderParams {
                    width: 63,
                    height: 128,
                    meaning: MeaningProfile::neutral(),
                }
            )
            .unwrap_err()
        );
    }
}
