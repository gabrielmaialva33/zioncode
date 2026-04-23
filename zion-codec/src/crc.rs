//! Wrapper fino sobre crc32c (Castagnoli).

#[inline]
#[must_use]
pub fn crc32c(data: &[u8]) -> u32 {
    crc32c::crc32c(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical value for "123456789" under Castagnoli (RFC 3720, iSCSI).
    #[test]
    fn canonical_vector() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
    }

    #[test]
    fn empty_input() {
        assert_eq!(crc32c(b""), 0);
    }
}
