use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use zion_codec::{Decoder, Encoder, EncoderConfig};

fn text_corpus() -> Vec<u8> {
    b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(2000)
}

fn random_corpus() -> Vec<u8> {
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let mut v = vec![0u8; 100_000];
    rng.fill_bytes(&mut v);
    v
}

fn bench_encode(c: &mut Criterion) {
    let text = text_corpus();
    let rand = random_corpus();

    let mut group = c.benchmark_group("encode");
    let encoder = Encoder::new(EncoderConfig::fixed_k(148).unwrap());
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("text_100k", |b| {
        b.iter(|| encoder.encode(&text).unwrap());
    });
    group.throughput(Throughput::Bytes(rand.len() as u64));
    group.bench_function("random_100k", |b| {
        b.iter(|| encoder.encode(&rand).unwrap());
    });
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let text = text_corpus();
    let encoded = Encoder::new(EncoderConfig::fixed_k(148).unwrap())
        .encode(&text)
        .unwrap();
    let decoder = Decoder::default();

    let mut group = c.benchmark_group("decode");
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("text_100k_full_pipeline", |b| {
        b.iter(|| decoder.decode_file(&encoded.symbols).unwrap());
    });
    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode);
criterion_main!(benches);
