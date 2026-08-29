use std::mem::MaybeUninit;

use zeroize::{Zeroize, Zeroizing};
use zstd::zstd_safe::zstd_sys;

#[cfg(test)]
use crate::constants::PINNED_DSTREAM_ESTIMATE;
use crate::constants::{MAX_ORIGINAL_LEN, MAX_ZSTD_DECODER_BYTES, ZSTD_WINDOW_LOG_MAX};

pub(crate) struct FramePreflight {
    pub window_size: usize,
    pub estimated_workspace: usize,
}

/// Performs the two libzstd checks that `zstd-safe` 7.2.4 does not expose:
/// frame-window inspection and a pre-allocation `DStream` workspace estimate.
pub(crate) fn preflight_frame(frame: &[u8]) -> Result<FramePreflight, ()> {
    let estimated_workspace = estimate_max_workspace()?;
    let mut header = MaybeUninit::<zstd_sys::ZSTD_FrameHeader>::uninit();

    // SAFETY: `frame.as_ptr()` is valid for exactly `frame.len()` bytes for the
    // duration of this call, and `header` points to writable storage of the
    // exact C `ZSTD_FrameHeader` type. We only assume initialization below when
    // libzstd returns zero, which its API defines as a completely filled header.
    let result = unsafe {
        zstd_sys::ZSTD_getFrameHeader(header.as_mut_ptr(), frame.as_ptr().cast(), frame.len())
    };
    if is_zstd_error(result) || result != 0 {
        return Err(());
    }
    // SAFETY: guarded by the successful zero return from ZSTD_getFrameHeader.
    let header = unsafe { header.assume_init() };
    if header.frameType != zstd_sys::ZSTD_FrameType_e::ZSTD_frame {
        return Err(());
    }
    let window_size = usize::try_from(header.windowSize).map_err(|_| ())?;
    if window_size > MAX_ORIGINAL_LEN {
        return Err(());
    }
    Ok(FramePreflight {
        window_size,
        estimated_workspace,
    })
}

fn estimate_max_workspace() -> Result<usize, ()> {
    // SAFETY: this pure libzstd query accepts one integer and dereferences no
    // caller pointer. It performs no context or frame allocation.
    let estimate = unsafe { zstd_sys::ZSTD_estimateDStreamSize(MAX_ORIGINAL_LEN) };
    if is_zstd_error(estimate) || estimate > MAX_ZSTD_DECODER_BYTES {
        return Err(());
    }
    Ok(estimate)
}

/// Decompresses one already-preflighted frame with a statically allocated
/// `ZSTD_DStream`. The backing `u64` allocation guarantees the 8-byte
/// alignment required by libzstd, remains alive for the entire C call, and is
/// zeroized on every exit. A static context cannot allocate or resize its
/// decoder workspace; an undersized workspace therefore fails closed.
pub(crate) fn decompress_static_frame(
    frame: &[u8],
    original_len: usize,
    estimated_workspace: usize,
) -> Result<Vec<u8>, ()> {
    let workspace_words = estimated_workspace.checked_add(7).ok_or(())? / 8;
    let workspace_bytes = workspace_words.checked_mul(size_of::<u64>()).ok_or(())?;
    if estimated_workspace == 0 || workspace_bytes > MAX_ZSTD_DECODER_BYTES {
        return Err(());
    }

    // A boxed slice has no spare capacity, so the entire writable allocation
    // exposed to libzstd is exactly `workspace_bytes` rather than merely a
    // lower bound on a `Vec` allocation.
    let mut workspace = Zeroizing::new(vec![0u64; workspace_words].into_boxed_slice());
    let mut output = vec![0u8; original_len];

    // SAFETY: `workspace` is 8-byte aligned by its `u64` element type and is
    // exactly `workspace_bytes` long. It and `output` remain allocated and
    // immovable for the duration of the call. `frame` is a valid immutable
    // slice. The helper never retains a Rust pointer after returning.
    let result = unsafe {
        decompress_into_static_workspace(
            frame,
            &mut output,
            workspace.as_mut_ptr().cast(),
            workspace_bytes,
        )
    };
    if result.is_err() {
        output.zeroize();
        return Err(());
    }
    Ok(output)
}

/// Performs the raw libzstd streaming calls against caller-owned storage.
///
/// # Safety
/// `workspace` must point to `workspace_bytes` writable bytes aligned to at
/// least 8 bytes, and that storage must remain valid for this entire call.
unsafe fn decompress_into_static_workspace(
    frame: &[u8],
    output: &mut [u8],
    workspace: *mut core::ffi::c_void,
    workspace_bytes: usize,
) -> Result<(), ()> {
    // SAFETY: upheld by this function's contract. libzstd documents that a
    // static DStream uses only this workspace and performs no allocation.
    let stream = unsafe { zstd_sys::ZSTD_initStaticDStream(workspace, workspace_bytes) };
    if stream.is_null() {
        return Err(());
    }

    // Configure the normative 24-bit maximum window before starting the
    // session. `ZSTD_initDStream` resets session state but preserves parameters.
    let window_log_max = i32::try_from(ZSTD_WINDOW_LOG_MAX).map_err(|_| ())?;
    // SAFETY: `stream` is non-null and remains backed by the live workspace.
    let parameter_result = unsafe {
        zstd_sys::ZSTD_DCtx_setParameter(
            stream,
            zstd_sys::ZSTD_dParameter::ZSTD_d_windowLogMax,
            window_log_max,
        )
    };
    if is_zstd_error(parameter_result) {
        return Err(());
    }
    // SAFETY: same valid static stream; this starts a dictionary-free session.
    let init_result = unsafe { zstd_sys::ZSTD_initDStream(stream) };
    if is_zstd_error(init_result) {
        return Err(());
    }
    // SAFETY: classification/query only on the valid stream pointer.
    let stream_size = unsafe { zstd_sys::ZSTD_sizeof_DStream(stream) };
    if stream_size == 0 || stream_size > workspace_bytes || stream_size > MAX_ZSTD_DECODER_BYTES {
        return Err(());
    }

    let mut input = zstd_sys::ZSTD_inBuffer {
        src: frame.as_ptr().cast(),
        size: frame.len(),
        pos: 0,
    };
    let mut destination = zstd_sys::ZSTD_outBuffer {
        dst: output.as_mut_ptr().cast(),
        size: output.len(),
        pos: 0,
    };
    loop {
        let previous_input = input.pos;
        let previous_output = destination.pos;
        // SAFETY: all three pointers refer to live storage with the sizes
        // recorded in their C buffer descriptors.
        let remaining = unsafe {
            zstd_sys::ZSTD_decompressStream(stream, &raw mut destination, &raw mut input)
        };
        if is_zstd_error(remaining) {
            return Err(());
        }
        if remaining == 0 {
            return if input.pos == input.size && destination.pos == destination.size {
                Ok(())
            } else {
                Err(())
            };
        }
        if input.pos == input.size
            || destination.pos == destination.size
            || (input.pos == previous_input && destination.pos == previous_output)
        {
            return Err(());
        }
    }
}

fn is_zstd_error(code: usize) -> bool {
    // SAFETY: ZSTD_isError is a pure classification function for a size code.
    unsafe { zstd_sys::ZSTD_isError(code) != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_libzstd_workspace_estimate_is_within_budget() {
        assert_eq!(estimate_max_workspace().unwrap(), PINNED_DSTREAM_ESTIMATE);
    }

    #[test]
    fn frame_declaring_32_mib_window_is_rejected_before_decode() {
        // Standard frame magic, non-single-segment descriptor, and window
        // descriptor 0x78 (windowLog=25, exactly 32 MiB). A complete block is
        // unnecessary because ZSTD_getFrameHeader consumes only these 6 bytes.
        let header = [0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x78];
        assert!(preflight_frame(&header).is_err());
    }
}
