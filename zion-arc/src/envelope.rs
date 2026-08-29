use zeroize::Zeroizing;

use crate::constants::{
    AEAD_TAG_LEN, ARC_VERSION, ENVELOPE_HEADER_LEN, ENVELOPE_MAGIC, MAX_ATTRIBUTION_LEN,
    MAX_COMPRESSED_LEN, MAX_MEDIA_TYPE_LEN, MAX_NAME_LEN, MAX_ORIGINAL_LEN,
};
use crate::error::SealError;
use crate::types::{EccProfile, ItemMetadata};

pub(crate) struct EnvelopeView<'a> {
    pub original_len: usize,
    pub content_hash: [u8; 32],
    pub metadata: ItemMetadata,
    pub compressed: &'a [u8],
}

pub(crate) fn validate_metadata(metadata: &ItemMetadata) -> Result<(), SealError> {
    let name = metadata.name.as_bytes();
    let media_type = metadata.media_type.as_bytes();
    let attribution = metadata.attribution.as_bytes();
    if name.is_empty()
        || name.len() > MAX_NAME_LEN
        || name.contains(&0)
        || media_type.is_empty()
        || media_type.len() > MAX_MEDIA_TYPE_LEN
        || media_type.contains(&0)
        || attribution.len() > MAX_ATTRIBUTION_LEN
    {
        return Err(SealError::InvalidMetadata);
    }
    Ok(())
}

pub(crate) fn padding_len(
    metadata: &ItemMetadata,
    compressed_len: usize,
    profile: EccProfile,
) -> Result<usize, SealError> {
    validate_metadata(metadata)?;
    if compressed_len == 0 || compressed_len > MAX_COMPRESSED_LEN {
        return Err(SealError::CompressedTooLarge);
    }
    let base_len = ENVELOPE_HEADER_LEN
        .checked_add(metadata.name.len())
        .and_then(|length| length.checked_add(metadata.media_type.len()))
        .and_then(|length| length.checked_add(metadata.attribution.len()))
        .and_then(|length| length.checked_add(compressed_len))
        .ok_or(SealError::ArithmeticOverflow)?;
    let k = profile.data_len();
    let tagged_len = base_len
        .checked_add(AEAD_TAG_LEN)
        .ok_or(SealError::ArithmeticOverflow)?;
    Ok((k - (tagged_len % k)) % k)
}

pub(crate) fn serialize(
    original_len: usize,
    content_hash: &[u8; 32],
    metadata: &ItemMetadata,
    compressed: &[u8],
    padding: &[u8],
    profile: EccProfile,
) -> Result<Zeroizing<Vec<u8>>, SealError> {
    if original_len > MAX_ORIGINAL_LEN || u64::try_from(original_len).is_err() {
        return Err(SealError::ContentTooLarge);
    }
    let expected_padding = padding_len(metadata, compressed.len(), profile)?;
    if padding.len() != expected_padding || padding.len() >= profile.data_len() {
        return Err(SealError::ArithmeticOverflow);
    }
    let total_len = ENVELOPE_HEADER_LEN
        .checked_add(metadata.name.len())
        .and_then(|length| length.checked_add(metadata.media_type.len()))
        .and_then(|length| length.checked_add(metadata.attribution.len()))
        .and_then(|length| length.checked_add(compressed.len()))
        .and_then(|length| length.checked_add(padding.len()))
        .ok_or(SealError::ArithmeticOverflow)?;
    let mut out = Zeroizing::new(Vec::with_capacity(total_len));
    out.extend_from_slice(&ENVELOPE_MAGIC);
    out.push(ARC_VERSION);
    out.push(64);
    out.push(1); // zstd level-6 frame
    out.push(0); // flags
    out.extend_from_slice(
        &u64::try_from(original_len)
            .map_err(|_| SealError::ContentTooLarge)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u32::try_from(compressed.len())
            .map_err(|_| SealError::CompressedTooLarge)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u16::try_from(metadata.name.len())
            .map_err(|_| SealError::InvalidMetadata)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u16::try_from(metadata.media_type.len())
            .map_err(|_| SealError::InvalidMetadata)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u16::try_from(metadata.attribution.len())
            .map_err(|_| SealError::InvalidMetadata)?
            .to_le_bytes(),
    );
    out.extend_from_slice(
        &u16::try_from(padding.len())
            .map_err(|_| SealError::ArithmeticOverflow)?
            .to_le_bytes(),
    );
    out.extend_from_slice(content_hash);
    out.extend_from_slice(&[0; 4]);
    debug_assert_eq!(out.len(), ENVELOPE_HEADER_LEN);
    out.extend_from_slice(metadata.name.as_bytes());
    out.extend_from_slice(metadata.media_type.as_bytes());
    out.extend_from_slice(metadata.attribution.as_bytes());
    out.extend_from_slice(compressed);
    out.extend_from_slice(padding);
    debug_assert_eq!(out.len(), total_len);
    Ok(out)
}

pub(crate) fn parse(plaintext: &[u8], profile: EccProfile) -> Result<EnvelopeView<'_>, ()> {
    if plaintext.len() < ENVELOPE_HEADER_LEN || plaintext[0..4] != ENVELOPE_MAGIC {
        return Err(());
    }
    if plaintext[4] != ARC_VERSION
        || plaintext[5] != 64
        || plaintext[6] != 1
        || plaintext[7] != 0
        || plaintext[60..64] != [0; 4]
    {
        return Err(());
    }
    let original_len = usize::try_from(u64::from_le_bytes(
        plaintext[8..16].try_into().map_err(|_| ())?,
    ))
    .map_err(|_| ())?;
    let compressed_len = u32::from_le_bytes(plaintext[16..20].try_into().map_err(|_| ())?) as usize;
    let name_len = u16::from_le_bytes(plaintext[20..22].try_into().map_err(|_| ())?) as usize;
    let media_type_len = u16::from_le_bytes(plaintext[22..24].try_into().map_err(|_| ())?) as usize;
    let attribution_len =
        u16::from_le_bytes(plaintext[24..26].try_into().map_err(|_| ())?) as usize;
    let declared_padding_len =
        u16::from_le_bytes(plaintext[26..28].try_into().map_err(|_| ())?) as usize;
    if original_len > MAX_ORIGINAL_LEN
        || compressed_len == 0
        || compressed_len > MAX_COMPRESSED_LEN
        || name_len == 0
        || name_len > MAX_NAME_LEN
        || media_type_len == 0
        || media_type_len > MAX_MEDIA_TYPE_LEN
        || attribution_len > MAX_ATTRIBUTION_LEN
        || declared_padding_len >= profile.data_len()
    {
        return Err(());
    }
    let exact_len = ENVELOPE_HEADER_LEN
        .checked_add(name_len)
        .and_then(|length| length.checked_add(media_type_len))
        .and_then(|length| length.checked_add(attribution_len))
        .and_then(|length| length.checked_add(compressed_len))
        .and_then(|length| length.checked_add(declared_padding_len))
        .ok_or(())?;
    if exact_len != plaintext.len() {
        return Err(());
    }

    let name_start = ENVELOPE_HEADER_LEN;
    let media_start = name_start.checked_add(name_len).ok_or(())?;
    let attribution_start = media_start.checked_add(media_type_len).ok_or(())?;
    let compressed_start = attribution_start.checked_add(attribution_len).ok_or(())?;
    let compressed_end = compressed_start.checked_add(compressed_len).ok_or(())?;

    let name = std::str::from_utf8(&plaintext[name_start..media_start]).map_err(|_| ())?;
    let media_type =
        std::str::from_utf8(&plaintext[media_start..attribution_start]).map_err(|_| ())?;
    let attribution =
        std::str::from_utf8(&plaintext[attribution_start..compressed_start]).map_err(|_| ())?;
    if name.as_bytes().contains(&0) || media_type.as_bytes().contains(&0) {
        return Err(());
    }
    let metadata = ItemMetadata::new(name, media_type, attribution);
    let expected_padding = padding_len(&metadata, compressed_len, profile).map_err(|_| ())?;
    if declared_padding_len != expected_padding {
        return Err(());
    }
    let content_hash = plaintext[28..60].try_into().map_err(|_| ())?;
    Ok(EnvelopeView {
        original_len,
        content_hash,
        metadata,
        compressed: &plaintext[compressed_start..compressed_end],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_variable_lengths_are_rejected_without_slicing() {
        let metadata = ItemMetadata::new("n", "m", "");
        let padding = vec![0; padding_len(&metadata, 4, EccProfile::Safe).unwrap()];
        let mut encoded = serialize(
            3,
            &[4; 32],
            &metadata,
            &[1, 2, 3, 4],
            &padding,
            EccProfile::Safe,
        )
        .unwrap();
        encoded[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&encoded, EccProfile::Safe).is_err());
    }
}
