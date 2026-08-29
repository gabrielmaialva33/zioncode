use zion_arc::{
    CollectionKey, EccProfile, ItemMetadata, OpenError, SealConfig, capacity, open_png_bytes,
    seal_png_bytes, seal_png_bytes_with_collection_key,
};
use zion_stego::RgbImage;

const PASSPHRASE: &[u8] = b"hackathon-strength-test-passphrase";

fn cover_png() -> Vec<u8> {
    let rgb_data = (0..64 * 64 * 3)
        .map(|index| (index as u8).wrapping_mul(73).wrapping_add(19))
        .collect();
    let image = RgbImage {
        width: 64,
        height: 64,
        rgb_data,
    };
    let mut bytes = Vec::new();
    zion_stego::save_png_rgb(&image, &mut bytes).unwrap();
    bytes
}

fn metadata() -> ItemMetadata {
    ItemMetadata::new(
        "ARC public API fixture",
        "text/plain; charset=utf-8",
        "Test attribution",
    )
}

#[test]
fn exact_png_roundtrip_and_wrong_password_is_generic() {
    let content = b"Offline knowledge survives an exact lossless PNG round trip.\n";
    let sealed = seal_png_bytes(
        content,
        &cover_png(),
        PASSPHRASE,
        &metadata(),
        SealConfig::default(),
    )
    .unwrap();
    let opened = open_png_bytes(&sealed.png_bytes, PASSPHRASE).unwrap();
    assert_eq!(opened.content, content);
    assert_eq!(opened.metadata, metadata());
    assert_eq!(opened.collection_id, sealed.collection_id);
    assert_eq!(opened.capsule_id, sealed.capsule_id);
    assert_eq!(opened.profile, EccProfile::Safe);

    assert_eq!(
        open_png_bytes(&sealed.png_bytes, b"definitely the wrong password"),
        Err(OpenError::RecoveryFailed)
    );
}

#[test]
fn cached_collection_key_reseal_uses_fresh_capsules() {
    let key = CollectionKey::new(PASSPHRASE).unwrap();
    let cover = cover_png();
    let first = seal_png_bytes_with_collection_key(
        b"same bytes",
        &cover,
        &key,
        &metadata(),
        SealConfig::default(),
    )
    .unwrap();
    let second = seal_png_bytes_with_collection_key(
        b"same bytes",
        &cover,
        &key,
        &metadata(),
        SealConfig::default(),
    )
    .unwrap();
    assert_eq!(first.collection_id, second.collection_id);
    assert_ne!(first.capsule_id, second.capsule_id);
    assert_ne!(first.png_bytes, second.png_bytes);
}

#[test]
fn galaxy_a57_reference_capacity_is_exact() {
    let result = capacity(1080, 2340).unwrap();
    assert_eq!(result.max_codewords(), 1_226);
    assert_eq!(result.usable_bits(), 2_501_759);
    assert_eq!(result.max_ciphertext_bytes(EccProfile::Safe), 273_398);
}
