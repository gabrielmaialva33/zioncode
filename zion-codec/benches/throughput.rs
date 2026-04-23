use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use zion_codec::constants::ZSTD_LEVEL_DEFAULT;
use zion_codec::decode::decode_symbol;
use zion_codec::encode::encode_file;
use zion_codec::reassemble::FileReassembler;

fn text_corpus() -> Vec<u8> {
    b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(2000)
}

fn random_corpus() -> Vec<u8> {
    use rand::{RngCore, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let mut v = vec![0u8; 100_000];
    rng.fill_bytes(&mut v);
    v
}

fn bench_encode(c: &mut Criterion) {
    let text = text_corpus();
    let rand = random_corpus();

    let mut group = c.benchmark_group("encode");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("text_100k", |b| {
        b.iter(|| encode_file(&text, 148, ZSTD_LEVEL_DEFAULT).unwrap());
    });
    group.throughput(Throughput::Bytes(rand.len() as u64));
    group.bench_function("random_100k", |b| {
        b.iter(|| encode_file(&rand, 148, ZSTD_LEVEL_DEFAULT).unwrap());
    });
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let text = text_corpus();
    let encoded = encode_file(&text, 148, ZSTD_LEVEL_DEFAULT).unwrap();

    let mut group = c.benchmark_group("decode");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("text_100k_full_pipeline", |b| {
        b.iter(|| {
            let mut reasm = FileReassembler::new();
            for sym in &encoded.symbols {
                let decoded = decode_symbol(sym).unwrap();
                reasm.add_symbol(decoded).unwrap();
            }
            reasm.finalize().unwrap()
        });
    });
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
