<p align="center">
  <img src=".github/assets/zioncode-hero.svg" alt="zioncode: offline codec for hostile channels">
</p>

<p align="center">
  <a href="rust-toolchain.toml"><img alt="Rust 1.95.0" src="https://img.shields.io/badge/Rust-1.95.0-f4b860?style=for-the-badge&logo=rust&logoColor=1b1f23"></a>
  <a href="LICENSE"><img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-c7f9b8?style=for-the-badge"></a>
  <img alt="Status: codec v1" src="https://img.shields.io/badge/Status-Codec%20v1-9ee7d8?style=for-the-badge">
</p>

<p align="center">
  <b>Encode files into resilient offline symbols. Decode them back with strong integrity checks.</b>
  <br>
  Built for air-gapped transfer, noisy capture paths, and explicit recovery diagnostics.
</p>

---

## Signal For Air Gaps

`zioncode` is a Rust codec for moving files through hostile offline channels.

The current repository implements **subsystem A: the offline byte codec**. It turns a file into one or more `.zbin` symbols with compression, ECC, interleaving, CRC validation, and typed reassembly errors. Optical rendering and camera decoding are intentionally separate future layers.

```text
file bytes
  -> zion-codec
  -> .zbin symbols
  -> offline/optical transport layer
  -> captured symbols
  -> zion-codec
  -> restored file
```

## What Makes It Robust

| Layer | What it protects |
| --- | --- |
| **zstd with raw fallback** | Compression is used only when it helps; incompressible blocks stay raw. |
| **CRC32C per block** | Corrupt payloads are detected locally before final reassembly. |
| **Reed-Solomon ECC** | Fixed-size codewords recover from bounded symbol damage. |
| **Column-major interleaving** | Burst errors are spread across codewords instead of concentrated. |
| **BLAKE3 global hash** | The final restored file is verified end-to-end. |
| **Typed reassembler** | Missing, duplicate, divergent, or inconsistent symbols produce actionable errors. |

## Quick Demo

Encode a file:

```sh
cargo run -p zion-cli -- encode ./input.bin --output-prefix ./out/input
```

Inspect a generated symbol:

```sh
cargo run -p zion-cli -- inspect ./out/input_000.zbin
```

Decode symbols back into the original file:

```sh
cargo run -p zion-cli -- decode ./out/input_000.zbin ./out/input_001.zbin --output ./restored.bin
```

Symbols can be passed in any order. `--force-write-corrupt` exists for forensic recovery only, when the global hash does not match.

## Architecture

| Path | Responsibility |
| --- | --- |
| `zion-codec/` | Core library: format, compression, ECC, encode, decode, reassembly. |
| `zion-cli/` | Thin `zion` CLI wrapper around the library. |
| `zion-codec/tests/` | Integration and property tests for roundtrip and corruption invariants. |
| `zion-codec/benches/` | Criterion benchmarks for hot paths. |
| `fuzz/` | cargo-fuzz targets for parser and decoder hardening. |
| `docs/superpowers/specs/` | Source of truth for the v1 wire format and invariants. |

## Codec Pipeline

```text
raw file
  -> 8 KiB logical blocks
  -> zstd-or-raw payload decision
  -> deterministic contiguous symbol packing
  -> versioned header + block metadata
  -> CRC32C validation data
  -> RS(255, 223) ECC
  -> column-major interleaving
  -> fixed-size .zbin symbols
```

The inverse path decodes each symbol independently, validates per-block data, merges valid blocks, rejects divergent duplicates, and checks the final file hash.

## Build And Test

The workspace uses the pinned toolchain in `rust-toolchain.toml`:

```text
Rust 1.95.0 + rustfmt + clippy
```

Common commands:

```sh
cargo build --workspace
cargo test
cargo clippy --all-targets --all-features
cargo fmt --all --check
```

Benchmark codec hot paths:

```sh
cargo bench -p zion-codec
```

Run fuzz targets from `fuzz/`:

```sh
cargo fuzz run parse_header
```

## Format Contract

The v1 binary format is specified in:

```text
docs/superpowers/specs/2026-04-23-zioncode-codec-offline-design.md
```

Read that spec before changing header layout, ECC parameters, interleaving, block packing, validation invariants, error semantics, or compatibility behavior.

## Contributing

See `CONTRIBUTING.md` for workflow, style, testing expectations, and PR requirements. Security-sensitive parser or decoder changes should include malformed-input coverage and, when relevant, fuzz target updates.

## License

Licensed under either the Apache License, Version 2.0 or the MIT license, at your option.
