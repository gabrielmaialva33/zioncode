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
fn roundtrip_short_payload_single_photo() {
    let host = make_host(640, 480, 1);
    let params = EmbedParams {
        passphrase: "p1".into(),
        ..Default::default()
    };
    let payload = b"hello world short payload";
    let out = embed_file(payload, vec![host], &params).unwrap();
    let stego = &out.stego_images[..out.symbols_used];
    let rec = extract_file(stego, "p1").unwrap();
    assert_eq!(rec.file_bytes, payload);
}

#[test]
fn roundtrip_larger_payload() {
    // 100 KiB payload spans multiple codec-A symbols (TARGET_BLOCKS_PER_SYMBOL=4
    // caps plaintext at ~32 KiB per symbol), so we provide enough hosts.
    let hosts: Vec<RgbImage> = (0..4).map(|i| make_host(1920, 1080, 2 + i)).collect();
    let params = EmbedParams {
        passphrase: "another phrase".into(),
        ..Default::default()
    };
    let payload: Vec<u8> = (0..100_000).map(|i| (i as u8).wrapping_mul(13)).collect();
    let out = embed_file(&payload, hosts, &params).unwrap();
    let stego = &out.stego_images[..out.symbols_used];
    let rec = extract_file(stego, "another phrase").unwrap();
    assert_eq!(rec.file_bytes, payload);
}

#[test]
fn excess_hosts_unchanged() {
    // Provide 3 hosts, only 1 needed.
    let host0 = make_host(1920, 1080, 10);
    let host1 = make_host(1920, 1080, 11);
    let host2 = make_host(1920, 1080, 12);
    let originals = vec![host0.clone(), host1.clone(), host2.clone()];
    let params = EmbedParams {
        passphrase: "p".into(),
        ..Default::default()
    };
    let out = embed_file(b"small", originals, &params).unwrap();
    assert_eq!(out.stego_images.len(), 3);
    assert_eq!(out.symbols_used, 1);
    // Hosts 1 and 2 should be unchanged bit-for-bit.
    assert_eq!(out.stego_images[1].rgb_data, host1.rgb_data);
    assert_eq!(out.stego_images[2].rgb_data, host2.rgb_data);
    // Host 0 should be modified (LSBs changed).
    assert_ne!(out.stego_images[0].rgb_data, host0.rgb_data);
}
