use proptest::prelude::*;
use zion_stego::{EmbedParams, RgbImage, embed_file, extract_file};

fn arb_host(size: usize) -> RgbImage {
    let len = size * size * 3;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "deterministic byte pattern for test fixtures"
    )]
    let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(29)).collect();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "test helper: size is bounded and known small"
    )]
    let dim = size as u32;
    RgbImage {
        width: dim,
        height: dim,
        rgb_data: data,
    }
}

proptest! {
    // Fewer cases: each embed/extract runs Argon2id (~0.5s) + codec + LSB work.
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// Roundtrip holds: any non-empty byte payload round-trips through embed+extract
    /// with the right passphrase.
    #[test]
    fn roundtrip_preserves_bytes(
        payload in prop::collection::vec(any::<u8>(), 1..5_000),
        passphrase in "[a-z ]{5,20}",
    ) {
        let host = arb_host(640);
        let params = EmbedParams {
            passphrase: passphrase.clone(),
            ..Default::default()
        };
        let out = embed_file(&payload, vec![host], &params).unwrap();
        let stego = &out.stego_images[..out.symbols_used];
        let rec = extract_file(stego, &passphrase).unwrap();
        prop_assert_eq!(rec.file_bytes, payload);
    }

    /// Wrong passphrase never silently returns the correct bytes.
    #[test]
    fn wrong_passphrase_never_returns_correct_bytes(
        payload in prop::collection::vec(any::<u8>(), 1..2_000),
        passphrase in "[a-z]{5,15}",
        wrong in "[A-Z]{5,15}",
    ) {
        prop_assume!(passphrase != wrong);
        let host = arb_host(512);
        let params = EmbedParams {
            passphrase: passphrase.clone(),
            ..Default::default()
        };
        let out = embed_file(&payload, vec![host], &params).unwrap();
        let stego = &out.stego_images[..out.symbols_used];
        let result = extract_file(stego, &wrong);
        match result {
            Err(_) => {},
            Ok(r) if r.file_bytes != payload => {},
            Ok(_) => prop_assert!(false, "silent success with wrong passphrase"),
        }
    }

    /// Flipping the LSB of any channel carrying the `file_id` bits (channels
    /// [64..192), which correspond to plaintext-header bytes [8..24)) must
    /// never silently return the correct bytes. Changing the `file_id`
    /// invalidates the Argon2-derived master key, so AEAD authentication
    /// always fails. Other plaintext-header regions (e.g., `total_symbols`
    /// on single-image decodes, or `version` bit 0) can be tolerated by
    /// design, so we restrict to the region guaranteed to be detected.
    #[test]
    fn tampered_file_id_never_silent_success(
        payload in prop::collection::vec(any::<u8>(), 500..2_000),
        passphrase in "[a-z]{5,10}",
        flip_index in 64usize..192,
    ) {
        let host = arb_host(512);
        let params = EmbedParams {
            passphrase: passphrase.clone(),
            ..Default::default()
        };
        let out = embed_file(&payload, vec![host], &params).unwrap();
        let mut stego = out.stego_images[0].clone();
        // Flip the LSB in a channel carrying a file_id bit.
        stego.rgb_data[flip_index] ^= 0x01;

        let result = extract_file(&[stego], &passphrase);
        match result {
            Err(_) => {},
            Ok(r) if r.file_bytes != payload => {},
            Ok(_) => prop_assert!(false, "silent success with tampered file_id"),
        }
    }
}
