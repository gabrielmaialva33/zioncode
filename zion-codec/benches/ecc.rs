use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use zion_codec::constants::{RS_K, RS_N};
use zion_codec::ecc::{
    deinterleave_column_major, interleave_column_major, rs_decode_codeword, rs_encode_codeword,
};

fn bench_rs_encode(c: &mut Criterion) {
    let data = [0x42u8; RS_K];
    let mut group = c.benchmark_group("rs");
    group.throughput(Throughput::Bytes(RS_K as u64));
    group.bench_function("encode_codeword", |b| {
        b.iter(|| rs_encode_codeword(&data));
    });
    group.finish();
}

fn bench_rs_decode(c: &mut Criterion) {
    let data = [0x42u8; RS_K];
    let code = rs_encode_codeword(&data);
    let mut group = c.benchmark_group("rs");
    group.throughput(Throughput::Bytes(RS_N as u64));
    group.bench_function("decode_codeword_clean", |b| {
        b.iter(|| rs_decode_codeword(&code, 0).unwrap());
    });
    group.finish();
}

fn bench_interleave(c: &mut Criterion) {
    let codewords: Vec<[u8; RS_N]> = (0..148)
        .map(|_| rs_encode_codeword(&[0x55u8; RS_K]))
        .collect();
    let interleaved = interleave_column_major(&codewords);

    let mut group = c.benchmark_group("interleave");
    group.throughput(Throughput::Bytes((148 * RS_N) as u64));
    group.bench_function("k148_encode", |b| {
        b.iter(|| interleave_column_major(&codewords));
    });
    group.bench_function("k148_decode", |b| {
        b.iter(|| deinterleave_column_major(&interleaved, 148));
    });
    group.finish();
}

criterion_group!(benches, bench_rs_encode, bench_rs_decode, bench_interleave);
criterion_main!(benches);
