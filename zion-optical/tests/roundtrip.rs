//! Integration roundtrip tests.
//! Priority test: render(bytes) -> PNG -> extract(PNG) == bytes for [1, 8192, 1_000_000] bytes.
//! Plus negative-path tests for header CRC, padding zero invariant, and truncated images.

use zion_optical::{ExtractError, RenderParams, RgbImage, decode_png, encode_png, extract, render};

fn roundtrip(payload: &[u8]) {
    let out = render(payload, &RenderParams::default()).unwrap();
    let rec = extract(&out.png_bytes).unwrap();
    assert_eq!(rec.file_bytes, payload, "roundtrip mismatch");
    assert_eq!(rec.file_id, out.file_id);
    assert_eq!(rec.k_global, out.k_global);
    assert_eq!(rec.total_symbols, out.total_symbols);
    assert_eq!(rec.payload_len, out.payload_len);
}

#[test]
fn roundtrip_one_byte() {
    roundtrip(&[0x42]);
}

#[test]
fn roundtrip_8192_bytes() {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "deterministic byte pattern wraps intentionally"
    )]
    let payload: Vec<u8> = (0..8192u32).map(|i| (i & 0xff) as u8).collect();
    roundtrip(&payload);
}

#[test]
fn roundtrip_1_million_bytes() {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "deterministic byte pattern wraps intentionally"
    )]
    let payload: Vec<u8> = (0..1_000_000u32)
        .map(|i| (i.wrapping_mul(31) & 0xff) as u8)
        .collect();
    roundtrip(&payload);
}

/// Take a freshly-rendered PNG, decode to flat bytes, mutate, re-encode, and try to extract.
fn render_mutate_extract<F>(payload: &[u8], mutate: F) -> Result<Vec<u8>, ExtractError>
where
    F: FnOnce(&mut [u8]),
{
    let out = render(payload, &RenderParams::default()).unwrap();
    let mut img = decode_png(&out.png_bytes).unwrap();
    mutate(&mut img.rgb_data);
    let mutated_png = encode_png(&img).unwrap();
    extract(&mutated_png).map(|o| o.file_bytes)
}

#[test]
fn corrupt_header_byte_fails_crc() {
    // Flip a bit in k_global (bytes [8..10) of the optical header).
    let err = render_mutate_extract(b"hello", |buf| {
        buf[8] ^= 0x01;
    })
    .unwrap_err();
    assert!(
        matches!(err, ExtractError::HeaderCrcMismatch),
        "got {err:?}"
    );
}

#[test]
fn corrupt_padding_byte_fails() {
    // Tiny payload -> image padded with zeros; flip the last byte to land in the padding region.
    let err = render_mutate_extract(b"x", |buf| {
        let last = buf.len() - 1;
        buf[last] = 0xFF;
    })
    .unwrap_err();
    assert!(
        matches!(err, ExtractError::PaddingNotZero { .. }),
        "got {err:?}"
    );
}

#[test]
fn truncated_image_fails() {
    // Construct a tiny but valid 1×1 RGB image (3 bytes); too small for the optical header.
    let img = RgbImage::from_flat(vec![0u8; 3], 1, 1).unwrap();
    let png_bytes = encode_png(&img).unwrap();
    let err = extract(&png_bytes).unwrap_err();
    assert!(
        matches!(err, ExtractError::ImageTooSmall { .. }),
        "got {err:?}"
    );
}
