use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use zion_stego::embed::{EmbedParams, embed_file};
use zion_stego::extract::extract_file;
use zion_stego::kdf::derive_master_key;
use zion_stego::png_io::RgbImage;

fn make_host(w: u32, h: u32) -> RgbImage {
    let len = (w as usize) * (h as usize) * 3;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "deterministic byte fixture for bench setup"
    )]
    let data: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
    RgbImage {
        width: w,
        height: h,
        rgb_data: data,
    }
}

fn bench_argon2id(c: &mut Criterion) {
    let file_id = [0xAAu8; 16];
    let passphrase = b"light rain on the roof";
    c.bench_function("argon2id_master_key", |b| {
        b.iter(|| derive_master_key(passphrase, &file_id).unwrap());
    });
}

fn bench_embed(c: &mut Criterion) {
    let host = make_host(1920, 1080);
    let payload: Vec<u8> = (0..50_000).map(|i| (i as u8).wrapping_mul(13)).collect();
    let params = EmbedParams {
        passphrase: "test".into(),
        ..Default::default()
    };
    let mut group = c.benchmark_group("embed");
    group.sample_size(10); // Argon2id makes each run ~0.5s
    group.throughput(Throughput::Bytes(payload.len() as u64));
    group.bench_function("1080p_50k_payload", |b| {
        b.iter(|| embed_file(&payload, vec![host.clone(), host.clone()], &params).unwrap());
    });
    group.finish();
}

fn bench_extract(c: &mut Criterion) {
    let host = make_host(1920, 1080);
    let payload: Vec<u8> = (0..50_000).map(|i| (i as u8).wrapping_mul(13)).collect();
    let params = EmbedParams {
        passphrase: "test".into(),
        ..Default::default()
    };
    let out = embed_file(&payload, vec![host.clone(), host], &params).unwrap();
    let stego = out.stego_images[..out.symbols_used].to_vec();

    let mut group = c.benchmark_group("extract");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(payload.len() as u64));
    group.bench_function("1080p_50k_payload", |b| {
        b.iter(|| extract_file(&stego, "test").unwrap());
    });
    group.finish();
}

criterion_group!(benches, bench_argon2id, bench_embed, bench_extract);
criterion_main!(benches);
