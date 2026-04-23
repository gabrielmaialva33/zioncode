# Contributing

## Development Setup

Install the Rust toolchain declared in `rust-toolchain.toml`. The workspace expects Rust `1.95.0` with `rustfmt` and `clippy`.

Build and test before opening a pull request:

```sh
cargo build --workspace
cargo test
cargo clippy --all-targets --all-features
cargo fmt --all --check
```

Run benchmarks only when changing hot paths:

```sh
cargo bench -p zion-codec
```

Run fuzz targets from `fuzz/` when changing parsers, ECC, decoding, or reassembly:

```sh
cargo fuzz run parse_header
```

## Project Boundaries

`zioncode` currently implements the offline codec only. Keep the boundaries clear:

- `zion-codec/` owns the byte-level format, encoding, decoding, ECC, and reassembly.
- `zion-cli/` should stay a thin wrapper over `zion-codec`.
- `format/` should contain wire-format parsing and serialization, not high-level orchestration.
- `docs/superpowers/specs/` is the source of truth for format changes.

Do not change the wire format, packing invariants, ECC layout, or compatibility behavior without updating the relevant spec or adding a focused design note.

## Code Style

- Write code, comments, docs, and developer-facing strings in English.
- Keep modules focused and errors typed.
- Prefer explicit invariants over implicit assumptions.
- Keep format constants in `constants.rs`.
- Use `snake_case` for modules, functions, files, and tests.
- Use `PascalCase` for structs and enums.
- Let `rustfmt` own formatting.

## Testing Expectations

Every behavioral change should include tests at the closest useful layer:

- Unit tests for parsing, CRC, ECC, compression fallback, and packing helpers.
- Integration tests in `zion-codec/tests/` for encode/decode/reassembly flows.
- Property tests for invariants, malformed inputs, corruption handling, and recovery behavior.
- Fuzzing for parser and decoder hardening when the input surface changes.

Name tests after the behavior being protected, for example:

```text
missing_symbol_reports_block_range
duplicate_symbol_with_divergent_payload_is_rejected
```

## Pull Requests

PRs should include:

- Problem and solution summary.
- Format compatibility notes when relevant.
- Tests or fuzz targets added or updated.
- Commands run locally.
- Any known limitations or follow-up work.

Use short, imperative commit subjects with a scope prefix:

```text
codec: reject invalid last-block sizes
cli: honor force-write-corrupt
docs: document fuzzing workflow
```
