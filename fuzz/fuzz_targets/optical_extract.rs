#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz the optical extract pipeline: PNG decode + optical header parse +
    // post-ECC byte stream recovery. It must never panic; any error is fine.
    let _ = zion_optical::extract(data);
});
