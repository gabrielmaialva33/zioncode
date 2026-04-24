#![no_main]

use libfuzzer_sys::fuzz_target;
use zion_stego::wire::{PLAINTEXT_HEADER_LEN, PlaintextHeader};

fuzz_target!(|data: &[u8]| {
    // Fuzz the plaintext header parser with arbitrary bytes.
    // It must never panic, only return Ok/Err.
    if data.len() >= PLAINTEXT_HEADER_LEN {
        let bytes: [u8; PLAINTEXT_HEADER_LEN] = data[..PLAINTEXT_HEADER_LEN]
            .try_into()
            .unwrap();
        let _ = PlaintextHeader::parse(&bytes, 0);
    }
});
