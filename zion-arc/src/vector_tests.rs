use hex_literal::hex;
use zion_stego::RgbImage;

use crate::api::seal_rgb_deterministic;
use crate::kdf::{CollectionKey, derive_capsule_values};
use crate::{
    EccProfile, ItemMetadata, OpenError, SealConfig, open_png_bytes,
    open_png_bytes_with_collection_key,
};

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one frozen vector keeps all required interoperability anchors together"
)]
fn vector_a_matches_all_frozen_intermediates_and_outcomes() {
    let passphrase = b"arc-vector-passphrase";
    let collection_id = hex!("000102030405060708090a0b0c0d0e0f");
    let capsule_id = hex!("101112131415161718191a1b1c1d1e1f");
    let content = b"Knowledge that travels as art.\n";
    let metadata = ItemMetadata::new(
        "ARC sample",
        "text/plain; charset=utf-8",
        "Zion ARC deterministic test fixture",
    );
    let mut rgb_data = Vec::with_capacity(64 * 64 * 3);
    for index in 0..64 * 64 * 3 {
        rgb_data
            .push(u8::try_from(19usize.wrapping_add(73usize.wrapping_mul(index)) & 0xFF).unwrap());
    }
    let image = RgbImage {
        width: 64,
        height: 64,
        rgb_data,
    };
    let padding: Vec<u8> = (0..33)
        .map(|index| 0xA5u8.wrapping_add(29u8.wrapping_mul(index)))
        .collect();

    let (sealed, artifacts) = seal_rgb_deterministic(
        content,
        image,
        passphrase,
        &metadata,
        SealConfig::default(),
        collection_id,
        capsule_id,
        &padding,
    )
    .unwrap();

    assert_eq!(artifacts.compressed.len(), 40);
    assert_eq!(
        artifacts.compressed,
        hex!(
            "28b52ffd201ff900004b6e6f776c6564676520746861742074726176656c7320
             6173206172742e0a"
        )
    );
    assert_eq!(
        artifacts.content_hash,
        hex!("3881db30bc3c880c582d38ca432ad40263fde21b2ef190f5c552ccc4c1eb0bc6")
    );
    assert_eq!(
        artifacts.argon2_salt,
        hex!("355433abf2fb7c096fbdd5330af07a59")
    );
    assert_eq!(
        artifacts.collection_key,
        hex!("6467e8a6f456a4950c4c1a02563fecc10791486b43f37da0276bb41d0fa6d33a")
    );
    assert_eq!(
        artifacts.aead_key,
        hex!("bbc1a34174e29f141d17bde406233528ef12948eb89f51d95a563420acb1d60e")
    );
    assert_eq!(
        artifacts.nonce,
        hex!("2ebbc9768ad9b8936e15fbd37eaa4982aaaaf9c0cedc404d")
    );
    assert_eq!(
        artifacts.permutation_seed,
        hex!("57c5822368b3c108730c0cbbae929d05a062487e1c9b63070fd129913686f1a8")
    );
    assert_eq!(
        artifacts.matching_seed,
        hex!("9538906fe4a9e235eb59bc7bd988f4fc2ca66d873c180c10a82837bfd3dac128")
    );
    assert_eq!(artifacts.padding_len, 33);
    assert_eq!(artifacts.envelope.len(), 207);
    assert_eq!(
        *blake3::hash(&artifacts.envelope).as_bytes(),
        hex!("1b5071c2eb024b2dcac59bbbad272650d6d53e9da1faeeecab8f3a36bcfa2084")
    );
    assert_eq!(
        artifacts.bootstrap,
        hex!(
            "5a41524301400000000102030405060708090a0b0c0d0e0f1011121314151617
             18191a1b1c1d1e1fdf00000001000101000000000000000000000000c2f9d3ee"
        )
    );
    assert_eq!(sealed.ciphertext_len, 223);
    assert_eq!(
        *blake3::hash(&artifacts.ciphertext).as_bytes(),
        hex!("b50a65b4209b27bfdeef431975e9f9ba7e71bd532243abeb0b50d0eb562879f4")
    );
    let expected_codeword = hex!(
        "f2fef29353e201a38928f3e514502421a92c8703b56f1da10437284a0ece35d1
         f7e51648b72115a73e5d23419e08b789cca6525c4556fa9c74b6bb672a7b761d
         ac79e17789725a5cc1020bdd1877f475623f1ae9217686311f12f5bb085eaa9a
         fa2823aa6092d56b483b265db83c45047fd9b81035362378f985ec5db53c57a8
         42562d717d184035b73617a6e2a8bf1829d4813eb756ee2073e70763cc8de074
         eadad1b5fd5d94a276338452f6e4720d840bb9b2790bfc93d7c4cf2bb74081d
         8afdabd261c01404f1ed319af66255707d48082c81819f46cc0ba438dc280ffa
         962d8c4572ee48dc62c0b1893cf52b18372cf82a9409398d39b36204b3dcf64"
    );
    assert_eq!(artifacts.interleaved, expected_codeword);
    assert_eq!(
        *blake3::hash(&artifacts.interleaved).as_bytes(),
        hex!("69f0f7cd914d4b93307244d40404d1afea6af0933d7d9fb46d7f9adf77503659")
    );
    assert_eq!(
        &artifacts.positions[..32],
        &[
            566, 12_048, 1_123, 8_010, 1_252, 2_730, 2_917, 1_948, 3_871, 5_136, 11_465, 8_318,
            10_041, 6_153, 12_275, 8_861, 9_299, 3_545, 11_431, 1_410, 10_603, 12_002, 8_036,
            1_574, 5_207, 11_489, 8_527, 11_668, 12_005, 3_380, 10_349, 3_115,
        ]
    );
    assert_eq!(
        *blake3::hash(&artifacts.flat_raster).as_bytes(),
        hex!("bc9e230cfba6118c501932c3d8eb46937489d5531f0f9b37d77ea886da8d8115")
    );

    let opened = open_png_bytes(&sealed.png_bytes, passphrase).unwrap();
    assert_eq!(opened.content, content);
    assert_eq!(opened.metadata, metadata);

    let mut within_budget = artifacts.interleaved.clone();
    for byte in within_budget.iter_mut().take(16) {
        *byte ^= 0x80;
    }
    assert_eq!(
        crate::ecc::decode(&within_budget, 1, EccProfile::Safe).unwrap(),
        artifacts.ciphertext
    );
    let mut beyond_budget = artifacts.interleaved.clone();
    for byte in beyond_budget.iter_mut().take(17) {
        *byte ^= 0x80;
    }
    assert!(crate::ecc::decode(&beyond_budget, 1, EccProfile::Safe).is_err());

    // Vector A's fixed 17-byte corruption fixture is the normative
    // over-budget outcome: unlike arbitrary budget+1 patterns, it is pinned
    // to fail rather than merely forbidden from yielding wrong content.
    let collection_key = CollectionKey::derive(passphrase, collection_id).unwrap();
    let mut beyond_budget_image = RgbImage {
        width: 64,
        height: 64,
        rgb_data: artifacts.flat_raster.clone(),
    };
    for byte_index in 0..17 {
        let position = usize::try_from(artifacts.positions[byte_index * 8]).unwrap();
        beyond_budget_image.rgb_data[position] ^= 1;
    }
    let beyond_budget_png = crate::carrier::encode_png_checked(&beyond_budget_image).unwrap();
    assert_eq!(
        open_png_bytes_with_collection_key(&beyond_budget_png, &collection_key),
        Err(OpenError::RecoveryFailed)
    );

    let mut tampered_bootstrap = artifacts.bootstrap;
    tampered_bootstrap[24] ^= 1;
    let crc = crc32c::crc32c(&tampered_bootstrap[..60]);
    tampered_bootstrap[60..64].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(crc, 0xDD1F_427E);
    let original_values = derive_capsule_values(&collection_key, &capsule_id, 64, 64);
    let mut tampered_aad = [0u8; 72];
    tampered_aad[..64].copy_from_slice(&tampered_bootstrap);
    tampered_aad[64..68].copy_from_slice(&64u32.to_le_bytes());
    tampered_aad[68..72].copy_from_slice(&64u32.to_le_bytes());
    assert!(
        crate::crypto::open(
            &original_values.aead_key,
            &original_values.nonce,
            &artifacts.ciphertext,
            &tampered_aad,
        )
        .is_err()
    );
}
