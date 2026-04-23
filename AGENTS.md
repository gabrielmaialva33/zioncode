# Repository Guidelines

## Project Structure & Module Organization

`zioncode` is a Rust workspace with two crates:

- `zion-codec/`: core codec library. Main modules live in `src/` (`encode.rs`, `decode.rs`, `reassemble.rs`, `ecc.rs`, `format/`, `zstd_layer.rs`).
- `zion-cli/`: `zion` command-line wrapper around the library.
- `zion-codec/tests/`: integration and property tests (`roundtrip.rs`, `properties.rs`).
- `zion-codec/benches/`: Criterion benchmarks (`throughput.rs`, `ecc.rs`).
- `fuzz/`: cargo-fuzz targets for parser and decoder hardening.
- `docs/superpowers/specs/` and `docs/superpowers/plans/`: format spec and implementation plan; check these before changing wire format or invariants.

## Build, Test, and Development Commands

- `cargo build --workspace`: build both crates.
- `cargo test`: run unit, integration, and property tests.
- `cargo clippy --all-targets --all-features`: run the main lint gate.
- `cargo fmt --all`: format the workspace with `rustfmt`.
- `cargo bench -p zion-codec`: run Criterion benchmarks for codec hot paths.
- `cargo fuzz run parse_header` from `fuzz/`: exercise a fuzz target locally.

Use the pinned toolchain from `rust-toolchain.toml` (`1.95.0` with `rustfmt` and `clippy`).

## Coding Style & Naming Conventions

Write code, comments, and developer-facing strings in English. Follow `rustfmt` defaults (4-space indentation, trailing commas where formatter wants them). Prefer small focused modules and typed errors over stringly-typed failures. Use `snake_case` for files, modules, functions, and tests; `PascalCase` for structs and enums. Keep format constants in `constants.rs` and wire-format parsing inside `format/`.

## Testing Guidelines

Every behavioral change should come with tests in the closest layer:

- unit tests next to the module for parsing, ECC, CRC, and packing logic;
- integration tests in `zion-codec/tests/` for end-to-end roundtrips and reassembly;
- property tests for invariants and corruption handling.

Name tests after the expected behavior, for example `missing_symbol_reports_block_range`.

## Commit & Pull Request Guidelines

This checkout does not expose usable git history, so no verified local commit convention could be extracted. Use short, imperative subjects with a scope prefix, for example `codec: reject invalid last-block sizes` or `cli: honor force-write-corrupt`.

PRs should include:

- a short problem/solution summary;
- linked issue or spec section when relevant;
- notes on format-compatibility risk;
- the commands you ran (`cargo test`, `cargo clippy`, etc.).
