use crate::constants::{
    ARC_VERSION, BOOTSTRAP_LEN, BOOTSTRAP_MAGIC, DENSITY_ID, MAX_CODEWORDS, SUITE_ID,
};
use crate::error::{OpenError, SealError};
use crate::types::EccProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bootstrap {
    pub profile: EccProfile,
    pub collection_id: [u8; 16],
    pub capsule_id: [u8; 16],
    pub ciphertext_len: usize,
    pub codeword_count: usize,
}

impl Bootstrap {
    pub(crate) fn new(
        profile: EccProfile,
        collection_id: [u8; 16],
        capsule_id: [u8; 16],
        ciphertext_len: usize,
        codeword_count: usize,
    ) -> Result<Self, SealError> {
        if collection_id == [0; 16] || capsule_id == [0; 16] {
            return Err(SealError::RandomFailed);
        }
        let expected_ciphertext_len = codeword_count
            .checked_mul(profile.data_len())
            .ok_or(SealError::ArithmeticOverflow)?;
        if codeword_count == 0
            || codeword_count > MAX_CODEWORDS
            || ciphertext_len != expected_ciphertext_len
            || u32::try_from(ciphertext_len).is_err()
            || u16::try_from(codeword_count).is_err()
        {
            return Err(SealError::ArithmeticOverflow);
        }
        Ok(Self {
            profile,
            collection_id,
            capsule_id,
            ciphertext_len,
            codeword_count,
        })
    }

    #[must_use]
    pub(crate) fn serialize(&self) -> [u8; BOOTSTRAP_LEN] {
        let mut out = [0u8; BOOTSTRAP_LEN];
        out[0..4].copy_from_slice(&BOOTSTRAP_MAGIC);
        out[4] = ARC_VERSION;
        out[5] = 64;
        out[6] = self.profile.wire_code();
        out[7] = 0;
        out[8..24].copy_from_slice(&self.collection_id);
        out[24..40].copy_from_slice(&self.capsule_id);
        out[40..44].copy_from_slice(
            &u32::try_from(self.ciphertext_len)
                .expect("bootstrap constructor validated ciphertext length")
                .to_le_bytes(),
        );
        out[44..46].copy_from_slice(
            &u16::try_from(self.codeword_count)
                .expect("bootstrap constructor validated codeword count")
                .to_le_bytes(),
        );
        out[46] = SUITE_ID;
        out[47] = DENSITY_ID;
        let crc = crc32c::crc32c(&out[..60]);
        out[60..64].copy_from_slice(&crc.to_le_bytes());
        out
    }

    pub(crate) fn parse(bytes: &[u8; BOOTSTRAP_LEN]) -> Result<Self, OpenError> {
        if bytes[0..4] != BOOTSTRAP_MAGIC {
            return Err(OpenError::BadMagic);
        }
        if bytes[4] != ARC_VERSION {
            return Err(OpenError::UnsupportedVersion);
        }
        if bytes[5] != 64 {
            return Err(OpenError::InvalidBootstrapLength);
        }
        let stored_crc = u32::from_le_bytes(bytes[60..64].try_into().expect("fixed slice"));
        if crc32c::crc32c(&bytes[..60]) != stored_crc {
            return Err(OpenError::BootstrapCrcMismatch);
        }
        let profile = EccProfile::from_wire_code(bytes[6]).ok_or(OpenError::UnsupportedProfile)?;
        if bytes[7] != 0 {
            return Err(OpenError::UnsupportedFlags);
        }
        if bytes[46] != SUITE_ID {
            return Err(OpenError::UnsupportedSuite);
        }
        if bytes[47] != DENSITY_ID {
            return Err(OpenError::UnsupportedDensity);
        }
        if bytes[48..60] != [0; 12] {
            return Err(OpenError::UnsupportedFlags);
        }

        let collection_id = bytes[8..24].try_into().expect("fixed slice");
        let capsule_id = bytes[24..40].try_into().expect("fixed slice");
        if collection_id == [0; 16] || capsule_id == [0; 16] {
            return Err(OpenError::InvalidIdentifier);
        }
        let ciphertext_len =
            u32::from_le_bytes(bytes[40..44].try_into().expect("fixed slice")) as usize;
        let codeword_count =
            u16::from_le_bytes(bytes[44..46].try_into().expect("fixed slice")) as usize;
        let expected_ciphertext_len = codeword_count
            .checked_mul(profile.data_len())
            .ok_or(OpenError::InvalidLength)?;
        if codeword_count == 0
            || codeword_count > MAX_CODEWORDS
            || ciphertext_len == 0
            || ciphertext_len != expected_ciphertext_len
        {
            return Err(OpenError::InvalidLength);
        }
        Ok(Self {
            profile,
            collection_id,
            capsule_id,
            ciphertext_len,
            codeword_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn vector_a_bootstrap_matches_spec_anchor() {
        let bootstrap = Bootstrap::new(
            EccProfile::Safe,
            hex!("000102030405060708090a0b0c0d0e0f"),
            hex!("101112131415161718191a1b1c1d1e1f"),
            223,
            1,
        )
        .unwrap();
        let expected = hex!(
            "5a41524301400000000102030405060708090a0b0c0d0e0f1011121314151617
             18191a1b1c1d1e1fdf00000001000101000000000000000000000000c2f9d3ee"
        );
        assert_eq!(bootstrap.serialize(), expected);
        assert_eq!(Bootstrap::parse(&expected).unwrap(), bootstrap);
    }

    #[test]
    fn malformed_length_is_rejected_before_kdf() {
        let mut bytes = Bootstrap::new(EccProfile::Safe, [1; 16], [2; 16], 223, 1)
            .unwrap()
            .serialize();
        bytes[40..44].copy_from_slice(&222u32.to_le_bytes());
        let crc = crc32c::crc32c(&bytes[..60]);
        bytes[60..64].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(Bootstrap::parse(&bytes), Err(OpenError::InvalidLength));
    }

    #[test]
    fn crc_damage_is_rejected_before_field_use() {
        let mut bytes = Bootstrap::new(EccProfile::Safe, [1; 16], [2; 16], 223, 1)
            .unwrap()
            .serialize();
        bytes[40] ^= 1;
        assert_eq!(
            Bootstrap::parse(&bytes),
            Err(OpenError::BootstrapCrcMismatch)
        );
    }
}
