//! Integration roundtrip tests.
//! Priority test: render(bytes) -> PNG -> extract(PNG) == bytes for [1, 8192, 1_000_000] bytes.

use zion_optical::{RenderParams, extract, render};

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
