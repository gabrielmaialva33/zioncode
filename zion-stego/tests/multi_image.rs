use zion_stego::embed::{EmbedParams, embed_file};
use zion_stego::extract::extract_file;
use zion_stego::png_io::RgbImage;

fn make_host(w: u32, h: u32, seed: u64) -> RgbImage {
    let len = (w as usize) * (h as usize) * 3;
    let mut data = Vec::with_capacity(len);
    let mut x = seed;
    for _ in 0..len {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        data.push((x >> 24) as u8);
    }
    RgbImage {
        width: w,
        height: h,
        rgb_data: data,
    }
}

#[test]
fn payload_spans_multiple_symbols() {
    // Tiny hosts + large payload forces the codec to produce multiple symbols.
    let hosts: Vec<RgbImage> = (0..5).map(|i| make_host(400, 400, 100 + i)).collect();
    let params = EmbedParams {
        passphrase: "set".into(),
        ..Default::default()
    };
    let payload: Vec<u8> = (0..60_000).map(|i| (i as u8).wrapping_mul(7)).collect();
    let out = embed_file(&payload, hosts, &params).unwrap();
    assert!(out.symbols_used >= 2, "expected multi-symbol");

    let stego = &out.stego_images[..out.symbols_used];
    let rec = extract_file(stego, "set").unwrap();
    assert_eq!(rec.file_bytes, payload);
}

#[test]
fn extract_accepts_shuffled_order() {
    let hosts: Vec<RgbImage> = (0..4).map(|i| make_host(400, 400, 200 + i)).collect();
    let params = EmbedParams {
        passphrase: "ord".into(),
        ..Default::default()
    };
    let payload: Vec<u8> = (0..40_000).map(|i| (i as u8).wrapping_add(1)).collect();
    let out = embed_file(&payload, hosts, &params).unwrap();
    assert!(out.symbols_used >= 2);

    // Reverse the order of stego images.
    let mut shuffled: Vec<RgbImage> = out.stego_images[..out.symbols_used].to_vec();
    shuffled.reverse();
    let rec = extract_file(&shuffled, "ord").unwrap();
    assert_eq!(rec.file_bytes, payload);
}

#[test]
fn mixed_file_ids_detected() {
    let host_a = make_host(400, 400, 300);
    let host_b = make_host(400, 400, 301);
    let params_a = EmbedParams {
        passphrase: "a".into(),
        ..Default::default()
    };
    let params_b = EmbedParams {
        passphrase: "b".into(),
        ..Default::default()
    };
    let out_a = embed_file(b"file A", vec![host_a], &params_a).unwrap();
    let out_b = embed_file(b"file B", vec![host_b], &params_b).unwrap();

    // Mix one image from each file.
    let mixed = vec![out_a.stego_images[0].clone(), out_b.stego_images[0].clone()];
    // Using passphrase "a": the plaintext headers are read first and detect
    // that file_ids differ between the two images, before any decrypt attempt.
    let err = extract_file(&mixed, "a").unwrap_err();
    assert!(matches!(
        err,
        zion_stego::error::ExtractError::InconsistentFileId
    ));
}
