# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`zioncode` is an air-gap file transport system between networkless machines, using static photos of grayscale optical symbols. Flow: file encoded at origin (print or screen) → captured by camera at destination → reassembled.

The project is designed as **three independent subsystems** with byte-stream contracts between them:

| Subsystem | Responsibility | Status |
|---|---|---|
| **A — Offline codec** | File ↔ post-ECC byte stream | ✅ v1 implemented |
| **B — Optical renderer** | Byte stream → grayscale image | 🔜 future |
| **C — Optical decoder** | Image → byte stream | 🔜 future |

This repository currently implements **only Subsystem A** (`zion-codec` library + `zion-cli` binary). B and C are future specs; the codec is an independently deliverable unit.

## Authority: spec and plan

**Always consult these two documents before changing format, ECC, or pipeline code:**

- **v1 spec (FROZEN):** `docs/superpowers/specs/2026-04-23-zioncode-codec-offline-design.md` — binary format, invariants, error model, sanity limits. Structural changes require bumping `version` in the header or updating the spec.
- **Implementation plan:** `docs/superpowers/plans/2026-04-23-zioncode-codec-offline-implementation.md` — 22 tasks across 3 milestones (M1/M2/M3).

## Build, test, run

```bash
# Build
cargo build --workspace

# All tests (unit + integration + property)
cargo test --workspace

# Codec only
cargo test -p zion-codec

# Specific test by name
cargo test -p zion-codec format::header::roundtrip_serialize_parse

# Property tests
cargo test -p zion-codec --test properties

# Integration tests (multi-symbol roundtrip)
cargo test -p zion-codec --test roundtrip

# Lints (pedantic enabled on zion-codec)
cargo clippy --workspace --all-targets -- -D warnings

# Formatting
cargo fmt --check        # verify
cargo fmt                # apply
```

### Benchmarks (criterion)

```bash
cargo bench -p zion-codec --bench ecc
cargo bench -p zion-codec --bench throughput
cargo bench -p zion-codec --bench ecc -- --quick     # fast smoke
```

Benchmarks produce HTML reports under `target/criterion/`.

### Fuzzing

**cargo-fuzz requires nightly toolchain**, even though `rust-toolchain.toml` pins 1.95.0 stable. Use:

```bash
cd fuzz
rustup run nightly cargo fuzz run decode_full -- -max_total_time=30
rustup run nightly cargo fuzz run parse_header -- -max_total_time=30
rustup run nightly cargo fuzz run parse_block -- -max_total_time=30
```

Available targets: `decode_full` (full symbol), `parse_header` (header alone), `parse_block` (block entry alone). Any panic is a bug.

### CLI (`zion`)

```bash
# Encode
cargo run -p zion-cli -- encode file.bin -o prefix --k 148 --zstd-level 6
# Produces prefix_000.zbin, prefix_001.zbin, ...

# Decode (arbitrary order OK, FileReassembler handles it)
cargo run -p zion-cli -- decode prefix_*.zbin -o restored.bin

# Inspect metadata
cargo run -p zion-cli -- inspect prefix_000.zbin
```

## Codec architecture (Subsystem A)

### End-to-end pipeline

```
file.bin
    ↓  split_file_into_blocks (8 KiB chunks)
    ↓  zstd encode per block, raw fallback if C >= R
Vec<BlockEntry>
    ↓  pack_blocks_into_symbols (greedy contiguous, up to 4 blocks/symbol)
Vec<SymbolPacking>  +  SymbolHeader per symbol
    ↓  encode_single_symbol: header + blocks → pre-ECC, zero-pad K*223
    ↓  RS(255,223) GF(256) + column-major interleave (K×255 matrix)
Vec<Vec<u8>>  (each symbol = K*255 bytes)
    ↓  written to .zbin files (or handed to Subsystem B in the future)

... reverse path on decode:
.zbin bytes
    ↓  decode_symbol (len % 255 == 0 is a hard precondition, K = len/255)
    ↓  deinterleave + RS decode per codeword (tolerates 16 errors/codeword)
    ↓  parse header + parse blocks + validate CRC32C per block
DecodedSymbol
    ↓  FileReassembler::add_symbol × N, validates file_id/metadata/global_hash
    ↓  concatenates decompressed blocks, validates global BLAKE3-256
file.bin
```

### v1 format decisions (frozen)

All come from spec §4 and §8.1 — **do not change without updating the spec**:

- `block_size_raw = 8192` bytes (only accepted value in v1)
- `RS(255, 223)` in GF(256), 32 parity bytes, corrects up to 16 errors/codeword
- `HEADER_LEN_V1 = 82` bytes, little-endian, CRC32C over `bytes[0..header_len-4]`
- `TARGET_BLOCKS_PER_SYMBOL = 4` (nominal; the last symbol may carry fewer)
- **Column-major** interleaving: `output[r*K + j] = codewords[j][r]`
- `K` (codewords per symbol) is an **input parameter**, NOT a header field — decoder derives it from the byte stream length
- Global hash: BLAKE3-256 of the raw file, repeated in every symbol's header
- Multi-symbol: self-contained fat indexed (each symbol carries `file_id` + `symbol_index` + `block_start` + `block_count`)

### Layered error model

`zion-codec/src/error.rs` defines 4 enums, used in discovery order:

- `EncodeError` — encoder side: `EmptyInput`, `InsufficientSymbolCapacity`, `ZstdEncodeFailed`
- `SymbolError` — optical/ECC layer: `BadSize`, `RsDecodeFailed`, `BadMagic`, `UnsupportedVersion`, `HeaderLengthInvalid`, `HeaderCrcMismatch`, `SymbolByteLengthTooLarge`
- `BlockError` — block layer: `BlockCrcMismatch`, `ZstdDecodeFailed`, `UnknownFlag`, `PayloadSizeTooLarge`, `RawBlockSizeMismatch`, `CompressedPayloadNotSmaller`
- `FileError` — reassembly: `MissingBlocks { ranges: Vec<(u32,u32)> }`, `SymbolFileIdMismatch`, `InconsistentFileMetadata`, `GlobalHashMismatch`, `SymbolIndexOutOfRange`, `BlockRangeExceedsTotal`, `DuplicateSymbol`

Partial recovery policy: `BlockCrcMismatch` does NOT invalidate the whole symbol (the block is marked lost and ends up in `MissingBlocks` at the end). `RsDecodeFailed` / `HeaderCrcMismatch` invalidate the whole symbol. `GlobalHashMismatch` rejects the output by default.

### Crate layout

```
zioncode/
├── zion-codec/src/            # pure library, #![forbid(unsafe_code)]
│   ├── constants.rs           # frozen v1 values (spec Appendix A)
│   ├── error.rs               # 4 typed enums (thiserror)
│   ├── crc.rs                 # wrapper over crc32c (Castagnoli)
│   ├── format/{header,block}.rs  # wire-format serialize/parse
│   ├── zstd_layer.rs          # per-block encode/decode + C<R fallback
│   ├── ecc.rs                 # RS(255,223) + interleave/deinterleave
│   ├── encode.rs              # split, pack, encode_single_symbol, encode_file
│   ├── decode.rs              # decode_symbol → DecodedSymbol
│   └── reassemble.rs          # multi-symbol FileReassembler
├── zion-cli/src/              # `zion` binary (clap + anyhow)
│   └── cmd/{encode,decode,inspect}.rs
├── fuzz/                      # cargo-fuzz, EXCLUDED from workspace
│   └── fuzz_targets/{decode_full,parse_header,parse_block}.rs
├── testdata/                  # corpus + goldens (empty for now)
└── docs/superpowers/{specs,plans}/
```

## Conventions that matter

### Lints and `#[allow(...)]`

`zion-codec/src/lib.rs` declares:

```rust
#![forbid(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
```

**Never suppress lints at the crate level.** When pedantic complains (e.g. `cast_possible_truncation`, `missing_errors_doc`), add `#[allow(clippy::X, reason = "explicit justification")]` at the exact **site**, with a `reason` explaining why it is safe. Example from the codebase:

```rust
#[allow(clippy::cast_possible_truncation, reason = "HEADER_LEN_V1 = 82 fits u8")]
let header_len = HEADER_LEN_V1 as u8;
```

### Pinned dependencies — rationale

- **`reed-solomon = "0.2"`** (NOT `reed-solomon-simd`): the spec requires classic GF(256). `reed-solomon-simd` uses FFT-based GF(2^16) and requires `shard_bytes ≥ 2`, which is incompatible.
- **`zstd = "0.13"`**: official C binding. Frame configured with `contentSizeFlag=0`, `checksumFlag=0`, `dictID=0` to minimize per-block overhead.
- **`rand = "0.9"`** in dev-deps: uses the `rand::rng()` API (not 0.8's `thread_rng()`).

### `Cargo.lock` is committed

The workspace contains a binary (`zion-cli`), so the lockfile goes in the repo per Cargo convention.

### File extensions

- `.zbin` — v1 intermediate artifact (post-ECC bytes of one symbol). Used in fixtures, debug, round-trip tests, CLI encoder output.
- `.zion` — **reserved**. To be adopted once B/C exist and the final container format (PNG + metadata) is consolidated. Do not use `.zion` in v1.

### Header forward compatibility

The v1 decoder accepts `header_len > 82` as long as `header_crc32c` covers all bytes (`bytes[0..header_len-4]`). Extra bytes are ignored. See spec §4.5 and the `forward_compat_larger_header` test in `format/header.rs`.

## Commit message convention

Commit messages should be written in **English**, prefixed with a **gitmoji** that matches the intent. Keep the subject short and focused on *why*, not *what* — the diff already shows *what*.

### Common gitmojis used in this project

| Emoji | Meaning                                                 | Example                                                     |
|-------|---------------------------------------------------------|-------------------------------------------------------------|
| 🏗️   | Project structure / build system                        | `🏗️ build: initial Rust workspace (zion-codec + zion-cli)` |
| ✨     | New feature or capability                               | `✨ codec: block entry serialize/parse with CRC32C`          |
| 🐛    | Bug fix                                                 | `🐛 codec: reject block_count=0 before parsing`             |
| ♻️    | Refactor (no behavior change)                           | `♻️ codec: extract interleave into helper`                  |
| ✅     | Add or fix tests                                        | `✅ codec: property tests for roundtrip`                     |
| 🧪    | Fuzzing or experimental testing                         | `🧪 fuzz: target header parser (smoke OK)`                  |
| ⚡     | Performance improvement                                 | `⚡ ecc: SIMD interleave kernel`                             |
| 📝    | Documentation (spec, plan, CLAUDE.md, rustdoc)          | `📝 spec: freeze v1 of offline codec subsystem`             |
| 🔒    | Safety / invariant / hardening                          | `🔒 decode: enforce M % 255 == 0 precondition`              |
| 🔧    | Config or tooling                                       | `🔧 workspace: commit Cargo.lock (binary crate)`            |
| 🚚    | Move or rename                                          | `🚚 format: split header and block into submodules`         |
| 🔥    | Remove code                                             | `🔥 codec: drop unused FORMAT_MARKER`                       |
| ⬆️    | Dependency bump                                         | `⬆️ deps: bump zstd 0.13 → 0.14`                            |
| 🚧    | Work in progress (avoid on `master` unless unavoidable) | `🚧 renderer: scaffolding for subsystem B`                  |

### Subject-line format

```
<gitmoji> <scope>: <short imperative summary in English>
```

Scopes used so far: `build`, `spec`, `plan`, `codec`, `cli`, `fuzz`, `bench`, `docs`.

### Body (optional)

If the reasoning is non-obvious, add a body explaining *why* (a past incident, a spec constraint, a design tradeoff). Keep it terse — one short paragraph at most.

## Task workflow

This project was built via the superpowers workflow. When adding new functionality:

1. If it requires a format or invariant change → update the spec first.
2. If it is a large new feature → use `superpowers:brainstorming` for design, then `superpowers:writing-plans`, then `superpowers:subagent-driven-development` for execution.
3. Prefer TDD order where applicable (failing test → minimal impl → passing test → commit).
4. One commit per logical step. Commit messages follow the gitmoji convention above.