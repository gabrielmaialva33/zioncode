use zeroize::Zeroizing;

use crate::constants::{MAX_COMPRESSED_LEN, MAX_ORIGINAL_LEN, MAX_ZSTD_DECODER_BYTES, ZSTD_LEVEL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompressError {
    Failed,
    TooLarge,
}

pub(crate) fn compress(content: &[u8]) -> Result<Zeroizing<Vec<u8>>, CompressError> {
    let compressed = Zeroizing::new(
        zstd::bulk::compress(content, ZSTD_LEVEL).map_err(|_| CompressError::Failed)?,
    );
    if compressed.is_empty() {
        return Err(CompressError::Failed);
    }
    if compressed.len() > MAX_COMPRESSED_LEN {
        return Err(CompressError::TooLarge);
    }
    Ok(compressed)
}

pub(crate) fn decompress_one_frame(compressed: &[u8], original_len: usize) -> Result<Vec<u8>, ()> {
    if compressed.is_empty()
        || compressed.len() > MAX_COMPRESSED_LEN
        || original_len > MAX_ORIGINAL_LEN
        || compressed.get(..4) != Some(&[0x28, 0xB5, 0x2F, 0xFD])
    {
        return Err(());
    }
    let frame_preflight = crate::zstd_ffi::preflight_frame(compressed)?;
    if frame_preflight.window_size > MAX_ORIGINAL_LEN
        || frame_preflight.estimated_workspace > MAX_ZSTD_DECODER_BYTES
    {
        return Err(());
    }
    let frame_len = zstd::zstd_safe::find_frame_compressed_size(compressed).map_err(|_| ())?;
    if frame_len != compressed.len() {
        return Err(());
    }
    let declared_size = zstd::zstd_safe::get_frame_content_size(compressed).map_err(|_| ())?;
    if declared_size.is_some_and(|size| size != original_len as u64) {
        return Err(());
    }

    crate::zstd_ffi::decompress_static_frame(
        compressed,
        original_len,
        frame_preflight.estimated_workspace,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_frame_roundtrip_and_trailing_frame_rejection() {
        for content in [b"".as_slice(), b"bounded zstd".as_slice()] {
            let frame = compress(content).unwrap();
            assert_eq!(
                decompress_one_frame(&frame, content.len()).unwrap(),
                content
            );
        }
        let frame = compress(b"bounded zstd").unwrap();
        let mut two = frame.clone();
        two.extend_from_slice(&frame);
        assert!(decompress_one_frame(&two, b"bounded zstd".len()).is_err());
    }
}
