#![no_main]

use libfuzzer_sys::fuzz_target;
use zion_stego::{RgbImage, extract_file};

fuzz_target!(|data: &[u8]| {
    // Fuzz the extract pipeline. Build a tiny RgbImage from the input and
    // feed it to extract_file with a fixed passphrase. It must never panic.
    let width = 16u32;
    let height = 16u32;
    let expected = (width * height * 3) as usize;
    if data.len() < expected {
        return;
    }
    let img = RgbImage {
        width,
        height,
        rgb_data: data[..expected].to_vec(),
    };
    let _ = extract_file(&[img], "fuzz-pass");
});
