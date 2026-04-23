use proptest::prelude::*;
use zion_codec::constants::{BLOCK_SIZE_RAW, HEADER_LEN_V1, RS_K, ZSTD_LEVEL_DEFAULT};
use zion_codec::decode::decode_symbol;
use zion_codec::encode::encode_file;
use zion_codec::reassemble::FileReassembler;

/// Compute the minimum K that can hold a file of `n` bytes in the worst case
/// (one raw block up to 8192 bytes plus the v1 header).
fn min_k_for_bytes(n: usize) -> u16 {
    let payload = n.min(BLOCK_SIZE_RAW);
    let bytes = HEADER_LEN_V1 + 7 + payload;
    u16::try_from(bytes.div_ceil(RS_K))
        .unwrap_or(u16::MAX)
        .max(38)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Basic roundtrip: any non-empty `Vec<u8>` up to 10 KiB must survive
    /// encode->decode->reassemble with a large enough K.
    #[test]
    fn roundtrip_preserves_bytes(
        bytes in prop::collection::vec(any::<u8>(), 1..10_000),
        k_offset in 0u16..50,
    ) {
        let k = min_k_for_bytes(bytes.len()).saturating_add(k_offset);
        let encoded = encode_file(&bytes, k, ZSTD_LEVEL_DEFAULT).unwrap();
        let mut reasm = FileReassembler::new();
        for sym in encoded.symbols {
            let decoded = decode_symbol(&sym).unwrap();
            reasm.add_symbol(decoded).unwrap();
        }
        let recovered = reasm.finalize().unwrap();
        prop_assert_eq!(recovered, bytes);
    }

    /// Invariant: every symbol has exactly K*255 bytes.
    #[test]
    fn symbol_length_is_k_times_255(
        bytes in prop::collection::vec(any::<u8>(), 1..5000),
        k_offset in 0u16..20,
    ) {
        let k = min_k_for_bytes(bytes.len()).saturating_add(k_offset);
        let encoded = encode_file(&bytes, k, ZSTD_LEVEL_DEFAULT).unwrap();
        for sym in &encoded.symbols {
            prop_assert_eq!(sym.len() % 255, 0);
            prop_assert_eq!(sym.len(), usize::from(k) * 255);
        }
    }

    /// Canonical rule (spec §10): corruption above the RS budget must never
    /// produce silent success with an accepted file. It must fail in some layer:
    /// RS decode, header CRC, block CRC, reassembly metadata, ou global hash.
    ///
    /// Strategy: interleave is column-major (`output[r*k + j] = cw[j][r]`),
    /// so to exceed the RS budget (16 errors/codeword) we corrupt 17 bytes in
    /// the same codeword by choosing positions `r*k + j` with fixed `j`.
    #[test]
    fn corruption_above_budget_never_silent_success(
        bytes in prop::collection::vec(any::<u8>(), 100..500),
        k_offset in 0u16..10,
        corruption_seed in 1u64..10_000,
    ) {
        let k = min_k_for_bytes(bytes.len()).saturating_add(k_offset);
        let encoded = encode_file(&bytes, k, ZSTD_LEVEL_DEFAULT).unwrap();
        let mut sym = encoded.symbols[0].clone();
        let k_usize = usize::from(k);

        // Pick a target codeword and corrupt 17 bytes in it (17 > 16 RS budget).
        let mut rng = corruption_seed;
        rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        #[allow(
            clippy::cast_possible_truncation,
            reason = "rng % k_usize fits in usize for all proptest-generated K"
        )]
        let target_j = (rng as usize) % k_usize;
        for r in 0..17 {
            let idx = r * k_usize + target_j;
            sym[idx] ^= 0xFF;
        }

        let silent_success = (|| -> bool {
            let decoded = match decode_symbol(&sym) {
                Err(_) => return false,
                Ok(d) => d,
            };
            let mut reasm = FileReassembler::new();
            if reasm.add_symbol(decoded).is_err() {
                return false;
            }
            // Add the remaining uncorrupted symbols.
            for sym_ok in &encoded.symbols[1..] {
                if let Ok(d) = decode_symbol(sym_ok) {
                    let _ = reasm.add_symbol(d);
                }
            }
            match reasm.finalize() {
                Err(_) => false,
                Ok(recovered) => recovered == bytes,
            }
        })();
        prop_assert!(
            !silent_success,
            "17-byte corruption in the same codeword produced silent success"
        );
    }
}
