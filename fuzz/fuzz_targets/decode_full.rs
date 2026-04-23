#![no_main]

use libfuzzer_sys::fuzz_target;
use zion_codec::decode::decode_symbol;

fuzz_target!(|data: &[u8]| {
    // decoder must never panic nor allocate beyond MAX_SYMBOL_BYTES.
    // Any error path is fine; success with valid input is fine too.
    let _ = decode_symbol(data);
});
