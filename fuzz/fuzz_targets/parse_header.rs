#![no_main]

use libfuzzer_sys::fuzz_target;
use zion_codec::format::SymbolHeader;

fuzz_target!(|data: &[u8]| {
    // Header parser must never panic on any input.
    // Allocations bounded by header_len field (u8 → max 255 bytes).
    let _ = SymbolHeader::parse(data);
});
