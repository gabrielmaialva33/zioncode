# Zion ARC Hackathon Implementation Plan

Date: 2026-08-29

Status: ready for execution

Working name: **Zion ARC — Adaptive Resilient Capsule**

Tagline: **Knowledge that travels as art.**

## Outcome and cut line

The hackathon demo must prove one complete, offline path:

```text
BLIVRE USFM book
  -> whole-book zstd
  -> authenticated encryption
  -> outer Reed-Solomon protection
  -> keyed placement in an exact PNG artwork
  -> Android import
  -> passphrase unlock
  -> readable book
```

The must-have cut line is:

1. A versioned ARC protocol specification and threat model.
2. A Rust core that seals and opens one complete book per PNG.
3. A reproducible BLIVRE 2018.2.0 importer and 66-book manifest.
4. A desktop CLI that produces the artwork collection.
5. An arm64 Android reader that imports PNG bytes and opens a book offline.
6. A live demo of correct password, wrong password, bounded corruption recovery, and pixel-quality metrics.

Texture-aware placement, 66 AI-generated final artworks, search, persistence, iOS, camera capture, JPEG survival, and
forensic-deniability work are stretch goals. They must not delay the cut line.

## Honest product and security claim

Zion ARC is a new **cryptographic transport protocol**, not a new cipher. Its defensible contribution is the ordering,
framing, adaptive capacity, independent book capsules, and artwork carrier:

```text
normalize -> compress -> AEAD seal -> ECC -> interleave -> keyed placement
```

The implementation may claim:

- offline confidentiality against an attacker who lacks a strong passphrase;
- authenticated integrity through XChaCha20-Poly1305;
- a measured Reed-Solomon correction budget;
- one independently recoverable capsule per book;
- exact-PNG round trips and measured visual similarity;
- no cloud, account, analytics, or AI model required by the reader.

It must not claim:

- a novel cipher, military-grade, unbreakable, post-quantum, audited, or formally verified cryptography;
- forensic invisibility, plausible deniability, or resistance to modern steganalysis;
- survival through JPEG conversion, resize, screenshot, print, camera capture, or social-media recompression;
- legal permission to bypass inspection or import rules.

## Phase 0 — Documentation discovery and allowed APIs

### Repository findings

The checkout contains four workspace crates in `Cargo.toml:1-4`, although `AGENTS.md`, `README.md`, `CLAUDE.md`, and
`CONTRIBUTING.md` still describe a two-crate or codec-only project. All referenced historical files under
`docs/superpowers/` are absent from both the working tree and `HEAD`. The ARC spec created in Phase 1 is therefore the
first available source of truth for the new protocol.

The current pipeline is documented by these implementation sources:

- `zion-stego/src/embed.rs:77-165` — current file-to-host orchestration.
- `zion-stego/src/embed.rs:171-236` — public header, AEAD, permutation, and LSB order.
- `zion-stego/src/extract.rs` — inverse pipeline.
- `zion-stego/src/kdf.rs:31-88` — Argon2id and BLAKE3 sub-key derivation pattern.
- `zion-stego/src/aead.rs:15-39` — XChaCha20-Poly1305 wrapper pattern.
- `zion-stego/src/png_io.rs` — exact RGB PNG decoding and encoding.
- `zion-codec/src/ecc.rs:23-75` — ECC profiles.
- `zion-codec/src/ecc.rs:100-208` — fallible Reed-Solomon encode/decode APIs.
- `zion-codec/src/ecc.rs:217-269` — column-major interleave/deinterleave APIs.
- `zion-codec/src/constants.rs:7-27` — v1 block and symbol limits.
- `zion-codec/src/encode/packing.rs:33-89` — the current four-block packing policy.

### Allowed Rust APIs to copy or call

Use the existing, documented primitives instead of inventing replacements:

```rust
zion_stego::load_png_rgb(reader)
zion_stego::save_png_rgb(&image, writer)
zion_stego::RgbImage

zion_codec::EccProfile::{Safe, Balanced, Dense}
zion_codec::low_level::try_rs_encode_codeword_with_profile(data, profile)
zion_codec::low_level::rs_decode_codeword_with_profile(codeword, profile, index)
zion_codec::low_level::interleave_column_major(codewords)
zion_codec::low_level::try_deinterleave_column_major(bytes, k)

chacha20poly1305::XChaCha20Poly1305
chacha20poly1305::aead::{Aead, KeyInit, Payload}
argon2::Argon2::hash_password_into(...)
blake3::derive_key(...)
zstd::bulk::{compress, decompress}
```

Copy the typed-error, fallible-parser, and bounds-checking conventions from `zion-codec/src/error.rs`,
`zion-codec/src/format/header.rs`, and `zion-stego/src/error.rs`. Copy PNG byte-oriented tests from
`zion-stego/tests/roundtrip.rs`; do not pass decoded Android `Bitmap` pixels through the bridge.

### Corpus source and measured capacity

Use the immutable upstream BLIVRE release for the hackathon:

- Release: `https://github.com/blivre/BibliaLivre/releases/tag/2018.2.0`
- Asset: `https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip`
- Tag commit: `a386942daee9984c654ebc8cea95ec9d3661b183`
- ZIP bytes: `1,364,020`
- ZIP SHA-256: `83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead`
- License: CC BY 3.0 Brazil, as declared by that tagged source.
- Corpus: 66 books, `4,276,761` raw USFM bytes.
- Independently compressed at zstd level 6: `1,391,039` bytes total.
- Largest compressed book: Psalms, `80,236` bytes.

The immutable release avoids the unresolved license-version and provenance mismatch between the upstream CC BY 3.0
declaration and the newer eBible mirror's CC BY 4.0 declaration.

At 1080x2340, the existing 0.33 carrier formula yields `K=1226`, 312,630 post-ECC bytes, and approximately 273,398
Safe-profile pre-ECC bytes. The measured largest book therefore has substantial margin, even after encrypted metadata and
ECC.

### Android path

Use native Jetpack Compose and a tiny direct JNI bridge for the hackathon reader. The app has only three important
screens and benefits from Android's native Storage Access Framework. UniFFI and Tauri remain valid later options, but
their generated-binding/JNA or WebView/IPC/plugin surfaces are unnecessary for the first two bridge calls.

Official references to copy during implementation:

- Android `ActivityResultContracts.OpenDocument`:
  `https://developer.android.com/reference/androidx/activity/result/contract/ActivityResultContracts.OpenDocument`
- Android Storage Access Framework streams:
  `https://developer.android.com/training/data-storage/shared/documents-files`
- Android native-library packaging:
  `https://developer.android.com/studio/projects/gradle-external-native-builds`
- AGP 9.0.1 compatibility matrix:
  `https://developer.android.com/build/releases/agp-9-0-0-release-notes`
- `cargo-ndk` build/output pattern:
  `https://github.com/bbqsrc/cargo-ndk/blob/main/README.md`
- `jni` crate documentation for the exact selected release; pin it before copying any macro or signature.

Live workstation gaps discovered on 2026-08-29:

- Rust 1.95.0 and `adb` are present.
- Only `x86_64-unknown-linux-gnu` is installed.
- Android SDK, NDK, command-line tools, Gradle, Android Studio, and `cargo-ndk` are absent.
- The active JDK is 25; the Android build should use a pinned JDK 17.

### Phase 0 anti-pattern guards

- Do not alter the frozen codec/stego v1 wire formats to impersonate ARC.
- Do not merely raise `TARGET_BLOCKS_PER_SYMBOL` and call the result cryptographically resilient; v1 still places AEAD
  outside ECC.
- Do not expose book title, canonical order, or corpus name in the public carrier header.
- Do not depend on a secret algorithm. The passphrase/key is the secret.
- Do not derive Argon2id independently 66 times when a collection key can be cached in memory.
- Do not infer an API because it sounds plausible; open the exact selected dependency documentation first.

## Phase 1 — Freeze the ARC v1 protocol and test vectors

### What to implement

Create `docs/specs/2026-08-29-zion-arc-v1.md` before protocol code. Define:

1. Threat model and exact-PNG transport contract.
2. Versioned public bootstrap layout with random collection and capsule identifiers, bounded lengths, profile, and CRC.
3. Encrypted envelope layout with generic item metadata, original length, compressed length, BLAKE3 content hash, UTF-8
   name/media type, attribution, and payload.
4. Argon2id collection-key parameters and BLAKE3 domain-separated sub-keys.
5. Unique XChaCha20-Poly1305 nonce derivation from the random capsule identifier.
6. Bootstrap bytes as AEAD associated data so parameter tampering is authenticated.
7. Whole-item zstd level 6 before encryption.
8. Random padding to an integral Reed-Solomon data-codeword boundary.
9. Reed-Solomon after AEAD, followed by existing column-major interleaving.
10. Keyed channel placement and the exact LSB mutation rule.
11. Decoder bounds checked before allocation.
12. Compatibility and version-rejection rules.

Create a deterministic test-vector appendix using fixed IDs, fixed passphrase, fixed carrier pixels, and expected hashes.
Randomness injection must be explicit in the test-only API; production continues to use the OS RNG.

### Documentation references

- Copy field-validation style from `zion-codec/src/format/header.rs`.
- Copy ECC dimensions and correction budgets from `zion-codec/src/ecc.rs:23-75`.
- Copy KDF domain-separation structure from `zion-stego/src/kdf.rs:52-88`, but use new ARC-specific contexts.
- Copy the XChaCha wrapper pattern from `zion-stego/src/aead.rs:15-39`, adding `Payload { msg, aad }` from the selected
  RustCrypto API.

### Verification checklist

- Every field has byte offset, width, endianness, and maximum.
- Nonce uniqueness requirements are stated and tested by construction.
- AEAD is inside ECC in both diagrams and pseudocode.
- The correction claim is expressed per 255-byte codeword.
- The spec explicitly excludes JPEG, resize, screenshot, print, and camera support.
- A reviewer can implement a decoder without reading the Rust code.

### Anti-pattern guards

- No home-grown cipher, MAC, stream generator, or password hashing.
- No unauthenticated semantic metadata.
- No allocation from unchecked `ciphertext_len`, `k`, or string lengths.
- No implicit nonce reuse when resealing the same book or carrier.

## Phase 2 — Implement the Rust ARC core and CLI

### What to implement

Add a separate `zion-arc/` workspace crate so ARC does not silently mutate v1. Build these focused modules:

```text
zion-arc/src/
  api.rs          seal_png_bytes/open_png_bytes/capacity
  bootstrap.rs    public fixed-size parser/serializer
  envelope.rs     encrypted item parser/serializer
  kdf.rs          Argon2id collection key + ARC sub-keys
  crypto.rs       XChaCha20-Poly1305 with AAD
  ecc.rs          ciphertext padding + RS + interleave adapter
  carrier.rs      capacity and keyed channel positions
  error.rs        typed seal/open errors
  lib.rs          narrow public facade
```

The stable facade should operate on bytes for CLI and JNI callers:

```rust
pub fn seal_png_bytes(
    content: &[u8],
    cover_png: &[u8],
    passphrase: &[u8],
    metadata: &ItemMetadata,
    config: SealConfig,
) -> Result<SealOutput, SealError>;

pub fn open_png_bytes(
    stego_png: &[u8],
    passphrase: &[u8],
) -> Result<OpenOutput, OpenError>;
```

Support a reusable in-memory `CollectionKey` so a batch of 66 books performs Argon2id once. Keep raw passphrases and
keys in `Zeroizing` storage where the dependency permits it.

Add thin CLI commands following `zion-cli/src/cmd/stego_embed.rs` and `stego_extract.rs`:

```text
zion arc seal <content> <cover.png> --output <art.png> --passphrase-file ...
zion arc open <art.png> --output <content> --passphrase-file ...
zion arc analyze <cover.png> --payload-bytes <n>
```

The analyzer reports dimensions, required/available channels, ECC overhead, estimated changed-channel count, and
pixel-quality metrics after sealing when an output is available.

### Documentation references

- Reuse `zion_stego::{load_png_rgb, save_png_rgb, RgbImage}` rather than adding a second PNG implementation.
- Copy fallible RS calls and interleaving from `zion-codec/src/ecc.rs:100-269`.
- Copy low-level LSB and permutation test structure from `zion-stego/src/lsb.rs` and `permutation.rs`.
- Copy CLI argument/password-file handling from `zion-cli/src/cmd/stego_embed.rs`.

### Verification checklist

- Clean byte-identical round trip for payload sizes around every header/codeword boundary.
- Wrong passphrase fails authentication without returning plaintext.
- Header tampering fails CRC or AEAD AAD verification.
- Random corruption within each selected ECC profile's budget recovers.
- Corruption beyond the budget returns a typed failure, never unauthenticated bytes.
- Reusing a collection key still yields distinct capsule IDs, nonces, and ciphertexts.
- ARC leaves existing v1 golden behavior and tests unchanged.
- `cargo fmt --all --check`, `cargo test --workspace`, and strict clippy pass.

### Anti-pattern guards

- Do not call current `zion_stego::embed_file()` from ARC; its cryptographic ordering is wrong for this claim.
- Do not return partially decrypted or hash-mismatched content.
- Do not serialize a Rust struct by memory layout; use explicit wire encoding.
- Do not put a passphrase in command arguments by default, shell history, logs, or debug output.

## Phase 3 — Build the legal PT-BR scripture packer

### What to implement

Add a reproducible importer under `tools/blivre/` or as a focused CLI command. It must:

1. Download or accept the pinned `usfm-blivre-tr.zip` asset.
2. Verify the exact ZIP SHA-256 before extraction.
3. Preserve the source ZIP, release/tag metadata, README, and license in a provenance directory outside generated ARC
   output.
4. Parse all 66 USFM files and derive canonical ordinals independent of the source filenames.
5. Normalize each book to a compact UTF-8 JSON document suitable for the reader.
6. Include source, authors, version date, license, and a clear normalization notice.
7. Produce a manifest with raw hash, normalized hash, raw bytes, compressed bytes, book ID/title/ordinal, and source.
8. Pair each normalized book with exactly one cover and call the ARC batch API with one cached collection key.

Suggested document shape:

```json
{
  "schema": 1,
  "id": "GEN",
  "ordinal": 1,
  "title": "Gênesis",
  "language": "pt-BR",
  "chapters": [[{"verse": 1, "text": "..."}]],
  "source": {
    "name": "Bíblia Livre",
    "version": "2018.2.0",
    "license": "CC BY 3.0 BR",
    "authors": ["Diego Santos", "Mario Sérgio", "Marco Teles"]
  }
}
```

### Documentation references

- Copy the upstream `convert_to_usfm.js` semantics only after comparing its markers with the pinned release.
- Parse the observed marker subset (`id`, `ide`, `h`, `toc*`, `mt1`, `c`, `p`, `v`, `d`, `add`, `f`, `rq`) instead of
  inventing full USFM support.
- Keep canonical mappings in a checked-in data file, not inferred from eBible numeric prefixes.

### Verification checklist

- Exactly 66 books, 1,189 chapters, and the expected verse-record count for the selected release.
- Every book has contiguous canonical ordinal 1 through 66.
- Source ZIP hash and per-book hashes match the manifest.
- Largest zstd-6 book remains below the measured ARC capacity on 1080x2340.
- Attribution is visible in the app About screen and present in exported/decoded documents.
- The reader can export decoded content; the sample corpus is not presented as proprietary Zion content.

### Anti-pattern guards

- Do not use NVI, NAA, ACF, or an unlicensed GitHub JSON dump.
- Do not label the eBible mirror's newer license discrepancy as resolved.
- Do not silently remove or rewrite verses, footnotes, additions, or headings.
- Do not make the corpus license apply to the ARC code; keep code and content notices separate.

## Phase 4 — Build the offline Android reader

### What to implement

The implemented hackathon cut is an arm64-first Android project plus the small `zion-android` `cdylib`/`rlib` bridge.
It pins `jni` 0.22.4 and exposes exactly one byte-oriented JNI call:

```text
decodeBookPng(pngBytes: ByteArray, passphraseBytes: ByteArray) -> ByteArray
```

The Compose MVP deliberately contains:

1. A single-document locked/import state rather than a gallery.
2. `OpenDocument` filtered to `image/png`.
3. One bounded `ContentResolver.openInputStream(uri)` read of the original PNG bytes.
4. A passphrase dialog and background decode call.
5. A book/chapter reader for the decoded JSON.
6. An About/Licenses screen with BLIVRE attribution and ARC limitations.
7. No internet permission, analytics, account, ads, or remote model.

`inspectArcPng`, a persistent artwork gallery, and multiple-document import are explicitly deferred. They would expand
the JNI/API and lifecycle surface without helping the one-book offline demo, and they must not be inferred from this plan
as implemented behavior.

Build only `arm64-v8a` for the Galaxy A57 demo first. Package the Rust shared library under the Android project's
`jniLibs/arm64-v8a` path using the exact `cargo-ndk` output convention.

### Documentation references

- Copy the activity-result launcher from Android's `OpenDocument` reference.
- Copy SAF stream handling from Android's shared-document training page.
- Copy native library packaging from the official Android Gradle documentation.
- Pin the Android build to AGP 9.0.1's documented JDK 17, Gradle 9.1, Build Tools 36.0, and default NDK
  28.2.13676358 unless the generated project proves a different supported matrix.

### Verification checklist

- Rust core cross-compiles for `aarch64-linux-android`, including `zstd-sys`.
- APK installs and launches on a physical arm64 Android device or emulator.
- Import uses original PNG bytes and never re-encodes through `Bitmap`.
- Correct password opens at least Genesis, Psalms, and Revelation offline.
- Wrong password shows a generic authentication error without crashing.
- The passphrase and key are not persisted; temporary byte arrays are cleared where practical.
- App works in airplane mode and declares no network permission.

### Anti-pattern guards

- Do not send 66 full decoded `RgbImage` buffers through JNI at once.
- Do not perform Argon2, PNG decode, or RS work on the main UI thread.
- Do not log passphrases, keys, decrypted verses, or source document URIs.
- Do not use Android `Bitmap.compress()` on carrier images.
- Do not add database/search/persistence until the one-book live path works.

## Phase 5 — Use the RTX 4090 for carrier creation and demo metrics

### What to implement

The RTX 4090 is a studio-side advantage, not a reader dependency. Build a local workflow that generates diverse,
texture-rich 1080x2340 artwork with a consistent visual collection identity. Start with three polished demo covers, then
scale to 66 only after the complete app path passes.

Add a carrier-quality report that records:

- cover and output PNG bytes;
- dimensions and ARC payload capacity;
- compressed payload and ECC overhead;
- changed channels and percentage;
- mean absolute channel delta;
- PSNR and, if a maintained dependency is justified, SSIM;
- seal/open time and Argon2 time;
- number of deliberately corrupted bytes recovered in the demo.

### Documentation references

- Use the ARC analyzer from Phase 2 as the acceptance gate for generated covers.
- Keep generation prompts/models/seeds in a separate provenance manifest.
- Preserve the exact final PNG after sealing; never feed the sealed image back through an image model or JPEG encoder.

### Verification checklist

- Three covers pass the full desktop-to-Android path.
- Original and sealed images can be shown in a blink/difference demo without visible gross artifacts.
- Each metric is produced by code, not a slide estimate.
- Removing the 4090 and all AI assets does not affect decoding.

### Anti-pattern guards

- Do not claim the neural generator performs the encryption.
- Do not require the same model, prompt, or GPU to recover a book.
- Do not optimize only for high entropy; visually noisy carriers may be conspicuous.
- Do not call a generated image copyright-free without recording the model/license provenance.

## Phase 6 — Final verification, demo, and pitch

### What to implement

Create `docs/hackathon/demo-script.md` and `docs/hackathon/pitch.md` with a repeatable five-minute story:

1. Show three ordinary artwork files in the device's system Files/gallery app, then select one through Zion ARC's
   single-document picker.
2. Explain ARC's five stages without claiming a new cipher.
3. Open Psalms on Android with the correct password and airplane mode enabled.
4. Show wrong-password rejection.
5. Use a prepared, bounded-corruption image and recover it successfully.
6. Reveal the 66-book capacity/compression table and exact measured metrics.
7. Close with the broader use case: offline transport of educational, archival, emergency, and cultural knowledge.

### Full verification checklist

```sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Additionally:

- Run ARC parser fuzz targets for bootstrap, envelope, and full open.
- Grep for stale ARC magic/version duplicates and passphrase logging.
- Verify the BLIVRE source hash from a clean checkout.
- Rebuild all three demo capsules from source and compare their decoded normalized hashes.
- Build the arm64 debug/release APK from documented commands.
- Run the demo from a clean app install in airplane mode.
- Record known limitations in both README and About.

### Final anti-pattern guards

- No unsupported security superlatives in code, README, UI, or slides.
- No staged corruption that exceeds the documented budget while claiming recovery.
- No dependency on cached plaintext or network access during the live demo.
- No release claim until a cryptographic review and steganalysis evaluation occur.

## Recommended 48-hour order

| Window | Deliverable |
|---|---|
| 0–4 h | ARC spec, threat model, deterministic vector |
| 4–14 h | Rust ARC core, roundtrip/corruption tests, CLI |
| 14–20 h | Pinned BLIVRE importer, 66-book manifest, batch pack |
| 20–32 h | Android toolchain, JNI bridge, three-screen reader |
| 32–40 h | Three final artworks, capacity/quality metrics |
| 40–48 h | Physical-device validation, demo script, pitch, contingency recording |

If the Android toolchain blocks after the core and corpus are working, use the desktop CLI to preserve the cryptographic
demo and continue the Android bridge in parallel. Do not weaken or bypass protocol verification merely to produce an APK.
