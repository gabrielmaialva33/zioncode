use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use zion_optical::{RenderParams, extract, render};

fn payload_random(size: usize) -> Vec<u8> {
    // Deterministic PRNG via LCG; high entropy mimics typical codec-A output.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    (0..size)
        .map(|_| {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            #[allow(
                clippy::cast_possible_truncation,
                reason = "LCG output truncated to byte intentionally"
            )]
            let b = (x >> 24) as u8;
            b
        })
        .collect()
}

fn bench_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("optical_render");
    group.sample_size(10);
    for &size in &[1_000usize, 100_000, 1_000_000] {
        let payload = payload_random(size);
        let throughput_bytes = payload.len() as u64;
        group.throughput(Throughput::Bytes(throughput_bytes));
        group.bench_function(format!("{size}_bytes"), |b| {
            b.iter(|| render(black_box(&payload), black_box(&RenderParams::default())).unwrap());
        });
    }
    group.finish();
}

fn bench_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("optical_extract");
    group.sample_size(10);
    for &size in &[1_000usize, 100_000, 1_000_000] {
        let payload = payload_random(size);
        let png_bytes = render(&payload, &RenderParams::default())
            .unwrap()
            .png_bytes;
        let throughput_bytes = payload.len() as u64;
        group.throughput(Throughput::Bytes(throughput_bytes));
        group.bench_function(format!("{size}_bytes"), |b| {
            b.iter(|| extract(black_box(&png_bytes)).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_render, bench_extract);
criterion_main!(benches);
