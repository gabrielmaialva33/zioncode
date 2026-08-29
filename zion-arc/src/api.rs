use zeroize::{Zeroize, Zeroizing};
use zion_stego::RgbImage;

use crate::bootstrap::Bootstrap;
use crate::carrier::{self, CarrierError};
use crate::constants::{AEAD_TAG_LEN, BOOTSTRAP_LEN, MAX_CODEWORDS, MAX_ORIGINAL_LEN, RS_N};
use crate::error::{CollectionKeyError, OpenError, SealError};
use crate::kdf::{CollectionKey, derive_capsule_values, validate_passphrase};
use crate::types::{
    Capacity, CorruptionFixtureOutput, ItemMetadata, OpenOutput, SealConfig, SealOutput,
};

struct Prepared {
    original_len: usize,
    compressed: Zeroizing<Vec<u8>>,
    content_hash: [u8; 32],
    padding_len: usize,
    ciphertext_len: usize,
    codeword_count: usize,
}

struct SealCoreOutput {
    output: SealOutput,
    #[cfg(test)]
    artifacts: TestArtifacts,
}

#[cfg(test)]
pub(crate) struct TestArtifacts {
    pub compressed: Vec<u8>,
    pub content_hash: [u8; 32],
    pub argon2_salt: [u8; 16],
    pub collection_key: [u8; 32],
    pub aead_key: [u8; 32],
    pub nonce: [u8; 24],
    pub permutation_seed: [u8; 32],
    pub matching_seed: [u8; 32],
    pub padding_len: usize,
    pub envelope: Vec<u8>,
    pub bootstrap: [u8; BOOTSTRAP_LEN],
    pub ciphertext: Vec<u8>,
    pub interleaved: Vec<u8>,
    pub positions: Vec<u32>,
    pub flat_raster: Vec<u8>,
}

/// Computes the exact ARC v1 capacity for an RGB8 carrier's dimensions.
///
/// # Errors
/// Returns a typed geometry or capacity error when the dimensions cannot form
/// a conforming ARC carrier.
pub fn capacity(width: u32, height: u32) -> Result<Capacity, SealError> {
    carrier::checked_capacity(width, height).map_err(map_carrier_seal)
}

/// Seals one content item into a lossless RGB8 PNG using a fresh collection.
///
/// # Errors
/// Returns [`SealError`] when any input, capacity, randomness, cryptographic,
/// ECC, or PNG-output check fails. No partial output is returned.
pub fn seal_png_bytes(
    content: &[u8],
    cover_png: &[u8],
    passphrase: &[u8],
    metadata: &ItemMetadata,
    config: SealConfig,
) -> Result<SealOutput, SealError> {
    validate_passphrase(passphrase).map_err(map_collection_key_seal)?;
    validate_inputs(content, metadata)?;
    let (image, capacity) = carrier::decode_png(cover_png).map_err(map_carrier_seal)?;
    let prepared = prepare(content, metadata, config, capacity)?;
    let collection_key = CollectionKey::new(passphrase).map_err(map_collection_key_seal)?;
    let capsule_id = collection_key
        .fresh_capsule_id()
        .map_err(map_collection_key_seal)?;
    let padding = random_padding(prepared.padding_len)?;
    Ok(seal_image(
        image,
        capacity,
        metadata,
        config,
        &collection_key,
        capsule_id,
        prepared,
        &padding,
    )?
    .output)
}

/// Seals one item with a cached collection key while still generating a fresh capsule ID.
///
/// # Errors
/// Returns [`SealError`] when any input, capacity, randomness, cryptographic,
/// ECC, or PNG-output check fails. No partial output is returned.
pub fn seal_png_bytes_with_collection_key(
    content: &[u8],
    cover_png: &[u8],
    collection_key: &CollectionKey,
    metadata: &ItemMetadata,
    config: SealConfig,
) -> Result<SealOutput, SealError> {
    validate_inputs(content, metadata)?;
    let (image, capacity) = carrier::decode_png(cover_png).map_err(map_carrier_seal)?;
    let prepared = prepare(content, metadata, config, capacity)?;
    let capsule_id = collection_key
        .fresh_capsule_id()
        .map_err(map_collection_key_seal)?;
    let padding = random_padding(prepared.padding_len)?;
    Ok(seal_image(
        image,
        capacity,
        metadata,
        config,
        collection_key,
        capsule_id,
        prepared,
        &padding,
    )?
    .output)
}

/// Opens and fully authenticates one ARC PNG using passphrase bytes.
///
/// # Errors
/// Returns typed pre-KDF parsing errors. After successful key derivation every
/// placement, ECC, AEAD, envelope, zstd, length, and hash error is returned as
/// [`OpenError::RecoveryFailed`].
pub fn open_png_bytes(stego_png: &[u8], passphrase: &[u8]) -> Result<OpenOutput, OpenError> {
    let (image, capacity) = carrier::decode_png(stego_png).map_err(map_carrier_open)?;
    let bootstrap_bytes =
        carrier::extract_bootstrap(&image.rgb_data).map_err(|()| OpenError::CarrierTooSmall)?;
    let bootstrap = Bootstrap::parse(&bootstrap_bytes)?;
    validate_bootstrap_capacity(&bootstrap, capacity)?;
    validate_passphrase(passphrase).map_err(map_collection_key_open)?;
    let collection_key = CollectionKey::derive(passphrase, bootstrap.collection_id)
        .map_err(map_collection_key_open)?;
    recover_image(
        &image,
        capacity,
        bootstrap,
        &bootstrap_bytes,
        &collection_key,
    )
    .map_err(|()| OpenError::RecoveryFailed)
}

/// Opens a capsule using a cached key for the same public collection identifier.
///
/// # Errors
/// Returns typed carrier/bootstrap errors before recovery and
/// [`OpenError::RecoveryFailed`] for every recovery-stage failure.
pub fn open_png_bytes_with_collection_key(
    stego_png: &[u8],
    collection_key: &CollectionKey,
) -> Result<OpenOutput, OpenError> {
    let (image, capacity) = carrier::decode_png(stego_png).map_err(map_carrier_open)?;
    let bootstrap_bytes =
        carrier::extract_bootstrap(&image.rgb_data).map_err(|()| OpenError::CarrierTooSmall)?;
    let bootstrap = Bootstrap::parse(&bootstrap_bytes)?;
    validate_bootstrap_capacity(&bootstrap, capacity)?;
    if bootstrap.collection_id != collection_key.collection_id() {
        return Err(OpenError::RecoveryFailed);
    }
    recover_image(
        &image,
        capacity,
        bootstrap,
        &bootstrap_bytes,
        collection_key,
    )
    .map_err(|()| OpenError::RecoveryFailed)
}

/// Produces a deliberately damaged PNG at the selected profile's exact
/// per-codeword Reed-Solomon correction bound.
///
/// This helper exists for demonstrations and decoder acceptance tests. It
/// changes one embedded payload bit in `profile.correction_budget()` distinct
/// bytes of every codeword, verifies that the resulting PNG still opens and
/// authenticates, and reports the exact mutation count. It does not model
/// screenshots, JPEG, resizing, printing, camera capture, or random damage.
///
/// # Errors
/// Returns the same bounded parsing and generic recovery errors as
/// [`open_png_bytes`]. No fixture is returned unless the damaged capsule has
/// been successfully Reed-Solomon repaired and authenticated.
pub fn make_bounded_corruption_fixture(
    stego_png: &[u8],
    passphrase: &[u8],
) -> Result<CorruptionFixtureOutput, OpenError> {
    let (mut image, capacity) = carrier::decode_png(stego_png).map_err(map_carrier_open)?;
    let bootstrap_bytes =
        carrier::extract_bootstrap(&image.rgb_data).map_err(|()| OpenError::CarrierTooSmall)?;
    let bootstrap = Bootstrap::parse(&bootstrap_bytes)?;
    validate_bootstrap_capacity(&bootstrap, capacity)?;
    validate_passphrase(passphrase).map_err(map_collection_key_open)?;
    let collection_key = CollectionKey::derive(passphrase, bootstrap.collection_id)
        .map_err(map_collection_key_open)?;
    let values = derive_capsule_values(
        &collection_key,
        &bootstrap.capsule_id,
        image.width,
        image.height,
    );
    let interleaved_len = bootstrap
        .codeword_count
        .checked_mul(RS_N)
        .ok_or(OpenError::InvalidLength)?;
    let required_bits = interleaved_len
        .checked_mul(8)
        .ok_or(OpenError::InvalidLength)?;
    let positions = carrier::payload_positions(
        image.rgb_data.len(),
        &values.permutation_seed,
        required_bits,
    )
    .map_err(|()| OpenError::RecoveryFailed)?;
    let corrupted_bytes_per_codeword = bootstrap.profile.correction_budget();
    let total_corrupted_payload_bytes = corrupted_bytes_per_codeword
        .checked_mul(bootstrap.codeword_count)
        .ok_or(OpenError::InvalidLength)?;

    // ARC interleaves column-major: byte `row * codeword_count + word` is
    // byte `row` of codeword `word`. Flipping the first embedded bit of each
    // selected byte therefore creates the exact documented error count in
    // every codeword without touching the bootstrap.
    for row in 0..corrupted_bytes_per_codeword {
        for word in 0..bootstrap.codeword_count {
            let byte_index = row
                .checked_mul(bootstrap.codeword_count)
                .and_then(|offset| offset.checked_add(word))
                .ok_or(OpenError::InvalidLength)?;
            let bit_index = byte_index.checked_mul(8).ok_or(OpenError::InvalidLength)?;
            let channel_index =
                usize::try_from(*positions.get(bit_index).ok_or(OpenError::RecoveryFailed)?)
                    .map_err(|_| OpenError::RecoveryFailed)?;
            let channel = image
                .rgb_data
                .get_mut(channel_index)
                .ok_or(OpenError::RecoveryFailed)?;
            *channel ^= 1;
        }
    }
    drop(positions);
    drop(values);

    let png_bytes = carrier::encode_png_checked(&image).map_err(map_carrier_open)?;
    let mut verified = open_png_bytes_with_collection_key(&png_bytes, &collection_key)?;
    verified.content.zeroize();
    verified.metadata.zeroize();

    Ok(CorruptionFixtureOutput {
        png_bytes,
        profile: bootstrap.profile,
        codeword_count: bootstrap.codeword_count,
        corrupted_bytes_per_codeword,
        total_corrupted_payload_bytes,
        changed_channels: total_corrupted_payload_bytes,
    })
}

fn validate_inputs(content: &[u8], metadata: &ItemMetadata) -> Result<(), SealError> {
    if content.len() > MAX_ORIGINAL_LEN {
        return Err(SealError::ContentTooLarge);
    }
    crate::envelope::validate_metadata(metadata)
}

fn prepare(
    content: &[u8],
    metadata: &ItemMetadata,
    config: SealConfig,
    capacity: Capacity,
) -> Result<Prepared, SealError> {
    let compressed = crate::compression::compress(content).map_err(|error| match error {
        crate::compression::CompressError::Failed => SealError::CompressionFailed,
        crate::compression::CompressError::TooLarge => SealError::CompressedTooLarge,
    })?;
    let padding_len = crate::envelope::padding_len(metadata, compressed.len(), config.profile)?;
    let envelope_len = crate::constants::ENVELOPE_HEADER_LEN
        .checked_add(metadata.name.len())
        .and_then(|length| length.checked_add(metadata.media_type.len()))
        .and_then(|length| length.checked_add(metadata.attribution.len()))
        .and_then(|length| length.checked_add(compressed.len()))
        .and_then(|length| length.checked_add(padding_len))
        .ok_or(SealError::ArithmeticOverflow)?;
    let ciphertext_len = envelope_len
        .checked_add(AEAD_TAG_LEN)
        .ok_or(SealError::ArithmeticOverflow)?;
    if !ciphertext_len.is_multiple_of(config.profile.data_len()) {
        return Err(SealError::ArithmeticOverflow);
    }
    let codeword_count = ciphertext_len / config.profile.data_len();
    if codeword_count == 0
        || codeword_count > MAX_CODEWORDS
        || codeword_count > capacity.max_codewords()
    {
        return Err(SealError::CapacityExceeded);
    }
    Ok(Prepared {
        original_len: content.len(),
        compressed,
        content_hash: *blake3::hash(content).as_bytes(),
        padding_len,
        ciphertext_len,
        codeword_count,
    })
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the linear sealing pipeline retains test-only interoperability artifacts"
)]
fn seal_image(
    mut image: RgbImage,
    capacity: Capacity,
    metadata: &ItemMetadata,
    config: SealConfig,
    collection_key: &CollectionKey,
    capsule_id: [u8; 16],
    prepared: Prepared,
    padding: &[u8],
) -> Result<SealCoreOutput, SealError> {
    let Prepared {
        original_len,
        compressed,
        content_hash,
        padding_len,
        ciphertext_len,
        codeword_count,
    } = prepared;
    if padding.len() != padding_len {
        return Err(SealError::RandomFailed);
    }
    let envelope = crate::envelope::serialize(
        original_len,
        &content_hash,
        metadata,
        &compressed,
        padding,
        config.profile,
    )?;
    if envelope.len().checked_add(AEAD_TAG_LEN) != Some(ciphertext_len) {
        return Err(SealError::ArithmeticOverflow);
    }
    let bootstrap = Bootstrap::new(
        config.profile,
        collection_key.collection_id(),
        capsule_id,
        ciphertext_len,
        codeword_count,
    )?;
    let bootstrap_bytes = bootstrap.serialize();
    let values = derive_capsule_values(collection_key, &capsule_id, image.width, image.height);
    let aad = build_aad(&bootstrap_bytes, image.width, image.height);
    let ciphertext = crate::crypto::seal(&values.aead_key, &values.nonce, &envelope, &aad)
        .map_err(|()| SealError::CryptoFailed)?;
    if ciphertext.len() != ciphertext_len {
        return Err(SealError::CryptoFailed);
    }

    #[cfg(test)]
    let artifact_compressed = compressed.to_vec();
    #[cfg(test)]
    let artifact_envelope = envelope.to_vec();
    #[cfg(test)]
    let artifact_aead_key = *values.aead_key;
    #[cfg(test)]
    let artifact_nonce = values.nonce;
    #[cfg(test)]
    let artifact_permutation_seed = *values.permutation_seed;
    #[cfg(test)]
    let artifact_matching_seed = *values.matching_seed;
    #[cfg(test)]
    let artifact_ciphertext = ciphertext.clone();
    drop(envelope);
    drop(compressed);

    let interleaved =
        crate::ecc::encode(&ciphertext, config.profile).map_err(|()| SealError::EccFailed)?;
    #[cfg(test)]
    let artifact_interleaved = interleaved.clone();
    drop(ciphertext);
    let required_bits = interleaved
        .len()
        .checked_mul(8)
        .ok_or(SealError::ArithmeticOverflow)?;
    if required_bits > capacity.usable_bits() {
        return Err(SealError::CapacityExceeded);
    }
    let positions = carrier::payload_positions(
        image.rgb_data.len(),
        &values.permutation_seed,
        required_bits,
    )
    .map_err(|()| SealError::CapacityExceeded)?;
    carrier::embed_payload(
        &mut image.rgb_data,
        &positions,
        &interleaved,
        &values.matching_seed,
    )
    .map_err(|()| SealError::CapacityExceeded)?;
    #[cfg(test)]
    let artifact_positions = positions.clone();
    drop(positions);
    drop(interleaved);
    drop(values);
    carrier::embed_bootstrap(&mut image.rgb_data, &bootstrap_bytes)
        .map_err(|()| SealError::CarrierTooSmall)?;

    #[cfg(test)]
    let flat_raster = image.rgb_data.clone();
    let png_bytes = carrier::encode_png_checked(&image).map_err(|_| SealError::PngEncodeFailed)?;
    let output = SealOutput {
        png_bytes,
        collection_id: collection_key.collection_id(),
        capsule_id,
        profile: config.profile,
        capacity,
        ciphertext_len,
        codeword_count,
    };

    Ok(SealCoreOutput {
        output,
        #[cfg(test)]
        artifacts: TestArtifacts {
            compressed: artifact_compressed,
            content_hash,
            argon2_salt: crate::kdf::derive_salt(&collection_key.collection_id()),
            collection_key: *collection_key.key_bytes(),
            aead_key: artifact_aead_key,
            nonce: artifact_nonce,
            permutation_seed: artifact_permutation_seed,
            matching_seed: artifact_matching_seed,
            padding_len,
            envelope: artifact_envelope,
            bootstrap: bootstrap_bytes,
            ciphertext: artifact_ciphertext,
            interleaved: artifact_interleaved,
            positions: artifact_positions,
            flat_raster,
        },
    })
}

#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "the test-only vector fixture makes every randomness input explicit"
)]
pub(crate) fn seal_rgb_deterministic(
    content: &[u8],
    image: RgbImage,
    passphrase: &[u8],
    metadata: &ItemMetadata,
    config: SealConfig,
    collection_id: [u8; 16],
    capsule_id: [u8; 16],
    padding: &[u8],
) -> Result<(SealOutput, TestArtifacts), SealError> {
    validate_passphrase(passphrase).map_err(map_collection_key_seal)?;
    validate_inputs(content, metadata)?;
    let capacity =
        carrier::checked_capacity(image.width, image.height).map_err(map_carrier_seal)?;
    if image.rgb_data.len() != capacity.channels() {
        return Err(SealError::InvalidDimensions);
    }
    let prepared = prepare(content, metadata, config, capacity)?;
    let collection_key =
        CollectionKey::derive(passphrase, collection_id).map_err(map_collection_key_seal)?;
    let sealed = seal_image(
        image,
        capacity,
        metadata,
        config,
        &collection_key,
        capsule_id,
        prepared,
        padding,
    )?;
    Ok((sealed.output, sealed.artifacts))
}

fn recover_image(
    image: &RgbImage,
    capacity: Capacity,
    bootstrap: Bootstrap,
    bootstrap_bytes: &[u8; BOOTSTRAP_LEN],
    collection_key: &CollectionKey,
) -> Result<OpenOutput, ()> {
    let values = derive_capsule_values(
        collection_key,
        &bootstrap.capsule_id,
        image.width,
        image.height,
    );
    let interleaved_len = bootstrap.codeword_count.checked_mul(RS_N).ok_or(())?;
    let required_bits = interleaved_len.checked_mul(8).ok_or(())?;
    if required_bits > capacity.usable_bits() {
        return Err(());
    }
    let positions = carrier::payload_positions(
        image.rgb_data.len(),
        &values.permutation_seed,
        required_bits,
    )?;
    let interleaved = carrier::extract_payload(&image.rgb_data, &positions, interleaved_len)?;
    drop(positions);
    let ciphertext = crate::ecc::decode(&interleaved, bootstrap.codeword_count, bootstrap.profile)?;
    drop(interleaved);
    if ciphertext.len() != bootstrap.ciphertext_len {
        return Err(());
    }
    let aad = build_aad(bootstrap_bytes, image.width, image.height);
    let plaintext = crate::crypto::open(&values.aead_key, &values.nonce, &ciphertext, &aad)?;
    drop(ciphertext);
    drop(values);
    let (content, metadata) = {
        let envelope = crate::envelope::parse(&plaintext, bootstrap.profile)?;
        let mut content =
            crate::compression::decompress_one_frame(envelope.compressed, envelope.original_len)?;
        if blake3::hash(&content).as_bytes() != &envelope.content_hash {
            content.zeroize();
            return Err(());
        }
        (content, envelope.metadata)
    };
    drop(plaintext);
    Ok(OpenOutput {
        content,
        metadata,
        collection_id: bootstrap.collection_id,
        capsule_id: bootstrap.capsule_id,
        profile: bootstrap.profile,
    })
}

fn validate_bootstrap_capacity(bootstrap: &Bootstrap, capacity: Capacity) -> Result<(), OpenError> {
    if bootstrap.codeword_count > capacity.max_codewords() {
        return Err(OpenError::CapacityExceeded);
    }
    let interleaved_len = bootstrap
        .codeword_count
        .checked_mul(RS_N)
        .ok_or(OpenError::InvalidLength)?;
    let encoded_bits = interleaved_len
        .checked_mul(8)
        .ok_or(OpenError::InvalidLength)?;
    if encoded_bits > capacity.usable_bits() {
        return Err(OpenError::CapacityExceeded);
    }
    Ok(())
}

fn random_padding(length: usize) -> Result<Zeroizing<Vec<u8>>, SealError> {
    let mut padding = Zeroizing::new(vec![0u8; length]);
    getrandom::fill(&mut padding).map_err(|_| SealError::RandomFailed)?;
    Ok(padding)
}

#[must_use]
fn build_aad(bootstrap: &[u8; BOOTSTRAP_LEN], width: u32, height: u32) -> [u8; 72] {
    let mut aad = [0u8; 72];
    aad[..BOOTSTRAP_LEN].copy_from_slice(bootstrap);
    aad[64..68].copy_from_slice(&width.to_le_bytes());
    aad[68..72].copy_from_slice(&height.to_le_bytes());
    aad
}

fn map_carrier_seal(error: CarrierError) -> SealError {
    match error {
        CarrierError::InputTooLarge => SealError::InputTooLarge,
        CarrierError::InvalidPng => SealError::InvalidPng,
        CarrierError::UnsupportedPixelFormat => SealError::UnsupportedPixelFormat,
        CarrierError::InvalidDimensions => SealError::InvalidDimensions,
        CarrierError::CarrierTooSmall => SealError::CarrierTooSmall,
    }
}

fn map_carrier_open(error: CarrierError) -> OpenError {
    match error {
        CarrierError::InputTooLarge => OpenError::InputTooLarge,
        CarrierError::InvalidPng => OpenError::InvalidPng,
        CarrierError::UnsupportedPixelFormat => OpenError::UnsupportedPixelFormat,
        CarrierError::InvalidDimensions => OpenError::InvalidDimensions,
        CarrierError::CarrierTooSmall => OpenError::CarrierTooSmall,
    }
}

fn map_collection_key_seal(error: CollectionKeyError) -> SealError {
    match error {
        CollectionKeyError::InvalidPassphraseLength => SealError::InvalidPassphraseLength,
        CollectionKeyError::RandomFailed => SealError::RandomFailed,
        CollectionKeyError::InvalidIdentifier | CollectionKeyError::KdfFailed => {
            SealError::KdfFailed
        }
    }
}

fn map_collection_key_open(error: CollectionKeyError) -> OpenError {
    match error {
        CollectionKeyError::InvalidPassphraseLength => OpenError::InvalidPassphraseLength,
        CollectionKeyError::InvalidIdentifier | CollectionKeyError::KdfFailed => {
            OpenError::KdfFailed
        }
        CollectionKeyError::RandomFailed => OpenError::KdfFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EccProfile;

    const PASSPHRASE: &[u8] = b"correct horse battery staple";

    fn metadata() -> ItemMetadata {
        ItemMetadata::new("fixture", "application/octet-stream", "")
    }

    fn cover_image() -> RgbImage {
        cover_image_with_dimensions(64, 64)
    }

    fn cover_image_with_dimensions(width: u32, height: u32) -> RgbImage {
        let channel_count = usize::try_from(width).unwrap() * usize::try_from(height).unwrap() * 3;
        let rgb_data = (0usize..channel_count)
            .map(|index| index.to_le_bytes()[0].wrapping_mul(37).wrapping_add(11))
            .collect();
        RgbImage {
            width,
            height,
            rgb_data,
        }
    }

    fn cover_png() -> Vec<u8> {
        carrier::encode_png_checked(&cover_image()).unwrap()
    }

    fn flip_interleaved_bytes(png_bytes: &[u8], key: &CollectionKey, byte_count: usize) -> Vec<u8> {
        let (mut image, _) = carrier::decode_png(png_bytes).unwrap();
        let bootstrap_bytes = carrier::extract_bootstrap(&image.rgb_data).unwrap();
        let bootstrap = Bootstrap::parse(&bootstrap_bytes).unwrap();
        assert_eq!(bootstrap.codeword_count, 1);
        let values = derive_capsule_values(key, &bootstrap.capsule_id, image.width, image.height);
        let payload_bytes = bootstrap.codeword_count * RS_N;
        let positions = carrier::payload_positions(
            image.rgb_data.len(),
            &values.permutation_seed,
            payload_bytes * 8,
        )
        .unwrap();
        for byte_index in 0..byte_count {
            let position = usize::try_from(positions[byte_index * 8]).unwrap();
            image.rgb_data[position] ^= 1;
        }
        carrier::encode_png_checked(&image).unwrap()
    }

    #[test]
    fn carrier_corruption_is_recovered_only_within_safe_budget() {
        let key = CollectionKey::derive(PASSPHRASE, [9; 16]).unwrap();
        for profile in [EccProfile::Safe, EccProfile::Balanced, EccProfile::Dense] {
            let sealed = seal_png_bytes_with_collection_key(
                b"pre-AEAD corruption fixture",
                &cover_png(),
                &key,
                &metadata(),
                SealConfig::with_profile(profile),
            )
            .unwrap();
            assert_eq!(sealed.codeword_count, 1);

            let within =
                flip_interleaved_bytes(&sealed.png_bytes, &key, profile.correction_budget());
            let opened = open_png_bytes_with_collection_key(&within, &key).unwrap();
            assert_eq!(opened.content, b"pre-AEAD corruption fixture");

            let beyond =
                flip_interleaved_bytes(&sealed.png_bytes, &key, profile.correction_budget() + 1);
            match open_png_bytes_with_collection_key(&beyond, &key) {
                Ok(opened) => {
                    assert_eq!(opened.content, b"pre-AEAD corruption fixture");
                    assert_eq!(opened.metadata, metadata());
                }
                Err(OpenError::RecoveryFailed) => {}
                Err(other) => panic!("unexpected public recovery error: {other:?}"),
            }
        }
    }

    #[test]
    fn public_corruption_fixture_hits_exact_safe_bound_and_still_authenticates() {
        let key = CollectionKey::derive(PASSPHRASE, [10; 16]).unwrap();
        let sealed = seal_png_bytes_with_collection_key(
            b"public bounded corruption fixture",
            &cover_png(),
            &key,
            &metadata(),
            SealConfig::default(),
        )
        .unwrap();
        assert_eq!(sealed.codeword_count, 1);

        let fixture = make_bounded_corruption_fixture(&sealed.png_bytes, PASSPHRASE).unwrap();
        assert_eq!(fixture.profile, EccProfile::Safe);
        assert_eq!(fixture.codeword_count, 1);
        assert_eq!(fixture.corrupted_bytes_per_codeword, 16);
        assert_eq!(fixture.total_corrupted_payload_bytes, 16);
        assert_eq!(fixture.changed_channels, 16);

        let opened = open_png_bytes_with_collection_key(&fixture.png_bytes, &key).unwrap();
        assert_eq!(opened.content, b"public bounded corruption fixture");
        assert_eq!(opened.metadata, metadata());

        let (original_image, _) = carrier::decode_png(&sealed.png_bytes).unwrap();
        let (fixture_image, _) = carrier::decode_png(&fixture.png_bytes).unwrap();
        let changed_channels = original_image
            .rgb_data
            .iter()
            .zip(&fixture_image.rgb_data)
            .filter(|(left, right)| left != right)
            .count();
        assert_eq!(changed_channels, fixture.changed_channels);
        assert!(
            original_image
                .rgb_data
                .iter()
                .zip(&fixture_image.rgb_data)
                .all(|(left, right)| left.abs_diff(*right) <= 1)
        );

        assert_eq!(
            make_bounded_corruption_fixture(&sealed.png_bytes, b"wrong passphrase"),
            Err(OpenError::RecoveryFailed)
        );
    }

    #[test]
    fn cached_collection_mismatch_is_a_generic_recovery_failure() {
        let sealing_key = CollectionKey::derive(PASSPHRASE, [5; 16]).unwrap();
        let wrong_collection_key = CollectionKey::derive(PASSPHRASE, [6; 16]).unwrap();
        let sealed = seal_png_bytes_with_collection_key(
            b"collection mismatch fixture",
            &cover_png(),
            &sealing_key,
            &metadata(),
            SealConfig::default(),
        )
        .unwrap();
        assert_eq!(
            open_png_bytes_with_collection_key(&sealed.png_bytes, &wrong_collection_key),
            Err(OpenError::RecoveryFailed)
        );
    }

    #[test]
    fn resealing_changes_every_per_capsule_cryptographic_artifact() {
        let key = CollectionKey::derive(PASSPHRASE, [7; 16]).unwrap();
        let image = cover_image();
        let capacity = carrier::checked_capacity(image.width, image.height).unwrap();
        let metadata = metadata();
        let config = SealConfig::default();

        let first_prepared = prepare(b"same reseal bytes", &metadata, config, capacity).unwrap();
        let first_id = key.fresh_capsule_id().unwrap();
        let first_padding = random_padding(first_prepared.padding_len).unwrap();
        let first = seal_image(
            image,
            capacity,
            &metadata,
            config,
            &key,
            first_id,
            first_prepared,
            &first_padding,
        )
        .unwrap();

        let second_image = cover_image();
        let second_prepared = prepare(b"same reseal bytes", &metadata, config, capacity).unwrap();
        let second_id = key.fresh_capsule_id().unwrap();
        let second_padding = random_padding(second_prepared.padding_len).unwrap();
        let second = seal_image(
            second_image,
            capacity,
            &metadata,
            config,
            &key,
            second_id,
            second_prepared,
            &second_padding,
        )
        .unwrap();

        assert_eq!(first.output.collection_id, second.output.collection_id);
        assert_ne!(first.output.capsule_id, second.output.capsule_id);
        assert_ne!(first.artifacts.nonce, second.artifacts.nonce);
        assert_ne!(first.artifacts.aead_key, second.artifacts.aead_key);
        assert_ne!(first.artifacts.ciphertext, second.artifacts.ciphertext);
        assert_ne!(first.artifacts.positions, second.artifacts.positions);
        assert_ne!(first.artifacts.flat_raster, second.artifacts.flat_raster);
        assert_ne!(first.output.png_bytes, second.output.png_bytes);
    }

    #[test]
    fn cached_key_roundtrips_across_codeword_boundaries() {
        let key = CollectionKey::derive(PASSPHRASE, [4; 16]).unwrap();
        let image = cover_image_with_dimensions(96, 96);
        let cover = carrier::encode_png_checked(&image).unwrap();
        let capacity = carrier::checked_capacity(image.width, image.height).unwrap();
        let metadata = metadata();
        let config = SealConfig::default();

        let content_for = |len: usize| {
            let mut content = vec![0u8; len];
            let mut hasher = blake3::Hasher::new();
            hasher.update(b"ARC boundary test content v1");
            hasher.finalize_xof().fill(&mut content);
            content
        };
        let mut previous_codewords = 0;
        let mut transitions = Vec::new();
        for length in 1..1_024 {
            let content = content_for(length);
            let codewords = prepare(&content, &metadata, config, capacity)
                .unwrap()
                .codeword_count;
            if previous_codewords != 0 && codewords != previous_codewords {
                transitions.push(length);
                if transitions.len() == 2 {
                    break;
                }
            }
            previous_codewords = codewords;
        }
        assert_eq!(transitions, [104, 326]);

        for boundary in transitions {
            for length in [boundary - 1, boundary, boundary + 1] {
                let content = content_for(length);
                let sealed =
                    seal_png_bytes_with_collection_key(&content, &cover, &key, &metadata, config)
                        .unwrap();
                let opened = open_png_bytes_with_collection_key(&sealed.png_bytes, &key).unwrap();
                assert_eq!(opened.content, content);
                assert_eq!(opened.metadata, metadata);
            }
        }
    }

    #[test]
    fn malformed_bootstrap_lengths_fail_before_kdf() {
        let key = CollectionKey::derive(PASSPHRASE, [8; 16]).unwrap();
        let sealed = seal_png_bytes_with_collection_key(
            b"header fixture",
            &cover_png(),
            &key,
            &metadata(),
            SealConfig::default(),
        )
        .unwrap();
        let (mut image, _) = carrier::decode_png(&sealed.png_bytes).unwrap();
        let mut bootstrap = carrier::extract_bootstrap(&image.rgb_data).unwrap();
        bootstrap[40..44].copy_from_slice(&222u32.to_le_bytes());
        let crc = crc32c::crc32c(&bootstrap[..60]);
        bootstrap[60..64].copy_from_slice(&crc.to_le_bytes());
        carrier::embed_bootstrap(&mut image.rgb_data, &bootstrap).unwrap();
        let malformed = carrier::encode_png_checked(&image).unwrap();
        assert_eq!(
            open_png_bytes(&malformed, PASSPHRASE),
            Err(OpenError::InvalidLength)
        );
    }
}
