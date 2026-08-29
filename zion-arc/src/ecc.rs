use crate::constants::RS_N;
use crate::types::EccProfile;

pub(crate) fn encode(ciphertext: &[u8], profile: EccProfile) -> Result<Vec<u8>, ()> {
    let k = profile.data_len();
    if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(k) {
        return Err(());
    }
    let mut codewords = Vec::with_capacity(ciphertext.len() / k);
    for chunk in ciphertext.chunks_exact(k) {
        codewords.push(
            zion_codec::low_level::try_rs_encode_codeword_with_profile(
                chunk,
                profile.codec_profile(),
            )
            .map_err(|_| ())?,
        );
    }
    Ok(zion_codec::low_level::interleave_column_major(&codewords))
}

pub(crate) fn decode(
    interleaved: &[u8],
    codeword_count: usize,
    profile: EccProfile,
) -> Result<Vec<u8>, ()> {
    if codeword_count == 0 || interleaved.len() != codeword_count.checked_mul(RS_N).ok_or(())? {
        return Err(());
    }
    let codewords =
        zion_codec::low_level::try_deinterleave_column_major(interleaved, codeword_count)
            .map_err(|_| ())?;
    let expected = codeword_count.checked_mul(profile.data_len()).ok_or(())?;
    let mut ciphertext = Vec::with_capacity(expected);
    for (index, codeword) in codewords.iter().enumerate() {
        let decoded = zion_codec::low_level::rs_decode_codeword_with_profile(
            codeword,
            profile.codec_profile(),
            u16::try_from(index).map_err(|_| ())?,
        )
        .map_err(|_| ())?;
        if decoded.len() != profile.data_len() {
            return Err(());
        }
        ciphertext.extend_from_slice(&decoded);
    }
    if ciphertext.len() != expected {
        return Err(());
    }
    Ok(ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_profile_budget_before_aead() {
        let profile = EccProfile::Safe;
        let input = vec![0x5A; profile.data_len() * 2];
        let mut encoded = encode(&input, profile).unwrap();
        for byte in encoded.iter_mut().take(profile.correction_budget() * 2) {
            *byte ^= 0x80;
        }
        assert_eq!(decode(&encoded, 2, profile).unwrap(), input);
    }
}
