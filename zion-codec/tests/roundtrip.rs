use zion_codec::constants::ZSTD_LEVEL_DEFAULT;
use zion_codec::decode::decode_symbol;
use zion_codec::ecc::{interleave_column_major, rs_encode_codeword};
use zion_codec::encode::encode_file;
use zion_codec::reassemble::FileReassembler;

fn encode_decode_roundtrip(raw: &[u8], k: u16) {
    let encoded = encode_file(raw, k, ZSTD_LEVEL_DEFAULT).unwrap();
    let mut reasm = FileReassembler::new();
    for sym in encoded.symbols {
        let decoded = decode_symbol(&sym).unwrap();
        reasm.add_symbol(decoded).unwrap();
    }
    let recovered = reasm.finalize().unwrap();
    assert_eq!(recovered, raw);
}

#[test]
fn roundtrip_small() {
    encode_decode_roundtrip(b"hello world from zion", 38);
}

#[test]
fn roundtrip_one_block_full() {
    let raw = vec![0x42u8; 8192];
    encode_decode_roundtrip(&raw, 38);
}

#[test]
fn roundtrip_four_blocks() {
    #[allow(clippy::cast_possible_truncation, reason = "test data, i < 32768")]
    let raw: Vec<u8> = (0..32768).map(|i| i as u8).collect();
    encode_decode_roundtrip(&raw, 148);
}

#[test]
fn roundtrip_boundary_sizes() {
    for &size in &[1usize, 8191, 8192, 8193, 16384, 16385, 100_000] {
        #[allow(clippy::cast_possible_truncation, reason = "test data, bounded")]
        let raw: Vec<u8> = (0..size).map(|i| (i * 31) as u8).collect();
        encode_decode_roundtrip(&raw, 148);
    }
}

#[test]
fn missing_symbol_reports_block_range() {
    let raw = vec![0u8; 40_000];
    let encoded = encode_file(&raw, 148, ZSTD_LEVEL_DEFAULT).unwrap();
    assert!(encoded.symbols.len() >= 2);

    let mut reasm = FileReassembler::new();
    // Add only the first symbol
    let decoded = decode_symbol(&encoded.symbols[0]).unwrap();
    reasm.add_symbol(decoded).unwrap();

    match reasm.finalize() {
        Err(zion_codec::error::FileError::MissingBlocks { ranges }) => {
            assert!(!ranges.is_empty());
        }
        other => panic!("expected MissingBlocks, got {other:?}"),
    }
}

#[test]
fn out_of_order_symbols_work() {
    let raw = vec![0u8; 40_000];
    let encoded = encode_file(&raw, 148, ZSTD_LEVEL_DEFAULT).unwrap();

    let mut reasm = FileReassembler::new();
    // Add in reverse order
    for sym in encoded.symbols.iter().rev() {
        let decoded = decode_symbol(sym).unwrap();
        reasm.add_symbol(decoded).unwrap();
    }
    let recovered = reasm.finalize().unwrap();
    assert_eq!(recovered, raw);
}

#[test]
fn duplicate_symbol_idempotent() {
    let raw = vec![0u8; 5000];
    let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
    assert_eq!(encoded.symbols.len(), 1);

    let mut reasm = FileReassembler::new();
    reasm
        .add_symbol(decode_symbol(&encoded.symbols[0]).unwrap())
        .unwrap();
    // Add it again: idempotent in v1
    reasm
        .add_symbol(decode_symbol(&encoded.symbols[0]).unwrap())
        .unwrap();

    let recovered = reasm.finalize().unwrap();
    assert_eq!(recovered, raw);
}

#[test]
fn recaptured_symbol_can_fill_missing_blocks() {
    let raw = vec![0x5Au8; 5000];
    let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
    assert_eq!(encoded.symbols.len(), 1);

    let good = decode_symbol(&encoded.symbols[0]).unwrap();
    let header = good.header.clone();
    let block = good.blocks[0].as_ref().unwrap().clone();

    let mut bad_block_bytes = block.serialize();
    bad_block_bytes[7] ^= 0xFF;
    let bad_symbol = build_symbol_with_raw_blocks(&header, &[bad_block_bytes], 38);
    let bad = decode_symbol(&bad_symbol).unwrap();
    assert!(bad.blocks[0].is_err());

    let mut reasm = FileReassembler::new();
    reasm.add_symbol(bad).unwrap();
    reasm.add_symbol(good).unwrap();

    let recovered = reasm.finalize().unwrap();
    assert_eq!(recovered, raw);
}

#[test]
fn invalid_zero_length_file_metadata_is_rejected() {
    let raw = vec![0x33u8; 100];
    let encoded = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
    let mut decoded = decode_symbol(&encoded.symbols[0]).unwrap();
    decoded.header.file_size = 0;

    let mut reasm = FileReassembler::new();
    let result = reasm.add_symbol(decoded);
    assert!(matches!(
        result,
        Err(zion_codec::error::FileError::InconsistentFileMetadata { field: "file_size" })
    ));
}

#[test]
fn mixed_file_ids_rejected() {
    let raw = vec![0u8; 5000];
    let encoded_a = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
    let encoded_b = encode_file(&raw, 38, ZSTD_LEVEL_DEFAULT).unwrap();
    // UUIDv4 garante que encoded_a.file_id != encoded_b.file_id (com probabilidade ~1).

    let mut reasm = FileReassembler::new();
    reasm
        .add_symbol(decode_symbol(&encoded_a.symbols[0]).unwrap())
        .unwrap();
    let result = reasm.add_symbol(decode_symbol(&encoded_b.symbols[0]).unwrap());
    assert!(matches!(
        result,
        Err(zion_codec::error::FileError::SymbolFileIdMismatch { .. })
    ));
}

fn build_symbol_with_raw_blocks(
    header: &zion_codec::format::SymbolHeader,
    block_bytes: &[Vec<u8>],
    k: usize,
) -> Vec<u8> {
    use zion_codec::constants::RS_K;

    let mut pre_ecc = Vec::with_capacity(k * RS_K);
    pre_ecc.extend_from_slice(&header.serialize_v1());
    for block in block_bytes {
        pre_ecc.extend_from_slice(block);
    }
    pre_ecc.resize(k * RS_K, 0);

    let codewords: Vec<[u8; 255]> = pre_ecc
        .chunks_exact(RS_K)
        .map(|chunk| rs_encode_codeword(chunk.try_into().unwrap()))
        .collect();
    interleave_column_major(&codewords)
}
