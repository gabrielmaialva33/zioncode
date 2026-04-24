#![no_main]

use libfuzzer_sys::fuzz_target;
use zion_codec::wire::BlockEntry;

fuzz_target!(|data: &[u8]| {
    // Block parser must never panic. Payload_size is bounded at u16 + validated
    // against BLOCK_SIZE_RAW (8192) in the parse routine.
    let _ = BlockEntry::parse(data, 0);
});
