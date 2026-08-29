<p align="center">
  <img src=".github/assets/zioncode-hero.svg" alt="zioncode: resilient offline transports">
</p>

<h1 align="center">zioncode</h1>

<p align="center">
  <b>Knowledge that travels as art.</b><br>
  <sub>Versioned Rust transports for resilient files, lossless images, and offline authenticated capsules.</sub>
</p>

<p align="center">
  <a href="rust-toolchain.toml"><img alt="Rust" src="https://img.shields.io/badge/Rust-1.95.0-CE422B?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="Cargo.toml"><img alt="Edition" src="https://img.shields.io/badge/Edition-2024-000000?style=for-the-badge&logo=rust&logoColor=white"></a>
  <a href="LICENSE"><img alt="Code license" src="https://img.shields.io/badge/Code-MIT%20OR%20Apache--2.0-4F46E5?style=for-the-badge"></a>
  <img alt="Status" src="https://img.shields.io/badge/Status-ARC%20hackathon%20MVP-10B981?style=for-the-badge">
</p>

`zioncode` is a seven-crate Rust workspace plus a native Android MVP. It keeps
four transport concerns distinct:

- `ZION` codec v1: resilient `.zbin` symbols with compression, ECC, and
  integrity;
- `ZSTG` stego v1: the pre-ARC photo embedding format, retained for
  compatibility;
- Zion Optical v1: an exact RGB PNG container for codec output; and
- `ZARC` / Zion ARC v1: one compressed, authenticated, ECC-protected item in
  one exact RGB8 PNG artwork.

ARC is a new transport protocol, not a new cipher. It composes Argon2id,
BLAKE3, XChaCha20-Poly1305, zstd, Reed–Solomon, interleaving, and keyed LSB
matching. The protocol is public; a strong passphrase is the secret.

## Current status

The desktop ARC path, pinned BLIVRE importer, carrier-art workflow, Rust Android
bridge, Android unit tests/lint, arm64 native build, debug APK, and unsigned
release APK pass on the development workstation. A physical Android device or
emulator was not available for the recorded run, so launch, UX, and performance
on a Galaxy A57-class phone remain pending.

Measured on the 1080×2340 RGB8 reference carrier:

| Result | Measurement |
|---|---:|
| Safe-profile maximum | 1,226 codewords / 273,398 ciphertext bytes |
| Pinned BLIVRE corpus | 66 books, 1,189 chapters, 31,102 verse records |
| Books that fit independently | 66 of 66 |
| Largest normalized book | Psalms, 657,274 JSON bytes |
| Psalms after ARC zstd-6 | 123,349 bytes |
| Psalms capsule | 556 codewords / 123,988 ciphertext bytes |
| Fresh sealed artwork | 3,972,714 PNG bytes |
| Lossless compression-level-0 encoding | 7,607,311 PNG bytes |
| Sealed/re-encoded RGB identity | Exact, all 7,581,600 samples |
| Clean/sealed image quality | 59.387568 dB PSNR / 0.998874 SSIM |
| Both authenticated opens | Exact 657,274-byte JSON |

The 3.97 MB and 7.61 MB files contain the same sealed RGB raster. PNG byte size
is not ARC capacity: encoder filtering, compression, and ancillary chunks can
change container bytes without changing dimensions or RGB identity.

Full evidence: [carrier-art report](docs/results/2026-08-29-carrier-art.md) and
[66-book fit report](docs/results/2026-08-29-blivre-arc-fit.md).

## Workspace map

| Path | Responsibility |
|---|---|
| [`zion-codec/`](zion-codec/) | Frozen `ZION` codec: block compression/raw fallback, CRC32C, Reed–Solomon profiles, interleaving, and typed reassembly |
| [`zion-stego/`](zion-stego/) | Frozen `ZSTG` photo-stego format and exact RGB PNG I/O used by existing integrations |
| [`zion-optical/`](zion-optical/) | One-image lossless RGB container for codec symbols |
| [`zion-arc/`](zion-arc/) | ARC v1 exact-PNG core, bounded parsers, KDF/AEAD/ECC/carrier pipeline, capacity API, and frozen vector |
| [`zion-corpus/`](zion-corpus/) | Pinned, bounded BLIVRE 2018.2.0 verifier/importer and canonical book model |
| [`zion-android/`](zion-android/) | Narrow byte-oriented Rust/JNI bridge for one authenticated ARC book |
| [`zion-cli/`](zion-cli/) | `zion` command tree for all four transports and corpus tooling |
| [`android/`](android/) | Kotlin/Jetpack Compose single-book reader using Android's document picker |
| [`fuzz/`](fuzz/) | Separate cargo-fuzz workspace for existing codec, stego, and optical attacker-facing parsers |
| [`scripts/`](scripts/) | Android setup/build and reproducible carrier-art demo workflows |

The four wire families are not interchangeable. ARC uses `ZARC`/`ARCE` and
does not mutate or reinterpret `ZION` or `ZSTG` v1.

## ARC quickstart

Build the CLI and create a private demo workspace. ARC deliberately has no
direct `--passphrase` option; use an owner-only file or the no-echo prompt.

```bash
cargo build --release -p zion-cli

ARC_DEMO="$(mktemp -d /tmp/zion-arc-quickstart.XXXXXX)"
chmod 700 "$ARC_DEMO"
openssl rand -out "$ARC_DEMO/passphrase.bin" 32
chmod 600 "$ARC_DEMO/passphrase.bin"

./target/release/zion arc capacity \
  --width 1080 \
  --height 2340 \
  --profile safe

./target/release/zion arc seal \
  README.md \
  assets/carriers/zion-arc-river-1080x2340.png \
  --output "$ARC_DEMO/readme.arc.png" \
  --name 'zioncode README' \
  --media-type 'text/markdown; charset=utf-8' \
  --profile safe \
  --passphrase-file "$ARC_DEMO/passphrase.bin"

./target/release/zion arc open \
  "$ARC_DEMO/readme.arc.png" \
  --output "$ARC_DEMO/readme.opened.md" \
  --passphrase-file "$ARC_DEMO/passphrase.bin"

cmp README.md "$ARC_DEMO/readme.opened.md"
```

`seal` and `open` refuse existing output paths. On Unix, recovered plaintext is
created as `0600`. A wrong passphrase returns a generic recovery failure and no
partial plaintext.

### Reproduce the 5–9 MB artwork demo

The carrier workflow accepts exactly one canonical book JSON, one owner-only
passphrase file, one UTF-8 attribution file, one clean 1080×2340 RGB8 cover,
and one new output directory:

```bash
ZION_BIN="$PWD/target/release/zion" \
  ./scripts/build-art-demo.sh \
  ./corpus/books/19-PSA.json \
  ./passphrase.bin \
  ./blivre-attribution.txt \
  assets/carriers/zion-arc-river-1080x2340.png \
  /tmp/zion-psalms-art
```

The script seals and opens, requires an exact plaintext comparison, re-encodes
the sealed PNG losslessly with FFmpeg compression level 0 into the inclusive
5,000,000–9,000,000-byte demonstration band, checks every RGB sample, opens and
compares again, and computes hashes, channel deltas, PSNR, SSIM, and wall times.
It never prints or hashes passphrase bytes. See the
[carrier provenance](assets/carriers/README.md) and
[coded metrics](docs/results/2026-08-29-carrier-art.md).

### Pinned BLIVRE corpus

The repository does not vendor scripture text or the upstream ZIP. Acquire the
exact release asset, then let the importer verify its 1,364,020-byte length and
SHA-256 before parsing:

```bash
./target/release/zion arc blivre import \
  ./usfm-blivre-tr.zip \
  --output-dir ./corpus

./target/release/zion arc blivre analyze \
  ./usfm-blivre-tr.zip \
  --width 1080 \
  --height 2340 \
  --profile safe
```

The expected ZIP SHA-256 is
`83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead`.
BLIVRE content is attributed to Diego Santos, Mario Sérgio, and Marco Teles
under CC BY 3.0 Brazil. The importer preserves byte-exact source USFM and
records provenance separately from generated ARC output. See the
[provenance record](docs/provenance/blivre-2018.2.0.md) and
[repeatable CLI demo](docs/hackathon/2026-08-29-cli-demo.md).

## ARC architecture and threat boundary

```text
content bytes
  -> whole-item zstd level 6
  -> encrypted metadata + content hash + authenticated random padding
  -> XChaCha20-Poly1305 (bootstrap and dimensions are AAD)
  -> RS(255,K) codewords
  -> column-major interleave
  -> passphrase-keyed RGB channel placement and LSB matching
  -> exact lossless RGB8 PNG
```

Safe uses RS(255,223) and corrects at most 16 unknown byte errors per individual
codeword; Balanced corrects 8 and Dense corrects 4. This is a per-codeword
bound, not a whole-image corruption percentage.

The public 64-byte `ZARC` bootstrap makes capsule existence detectable and
allows collection correlation. ARC authenticates that bootstrap and dimensions
as associated data plus the encrypted envelope and recovered content. It does
not authenticate the visible cover, the upper seven bits of RGB channels,
unused channels, alpha, or PNG metadata. Changing unauthenticated presentation
data can alter the picture without making open fail.

ARC has not been independently audited, formally verified, or externally
validated for interoperability. It is not post-quantum and does not claim
forensic invisibility or resistance to steganalysis. It does not promise
survival through JPEG, resize, crop, screenshot, print, camera capture, color
correction, or social-media recompression. Recovery requires the original
dimensions and RGB8 samples in a valid lossless PNG. The software provides no
legal authorization to bypass inspection or import rules.

The normative contract is the
[ARC v1 specification and threat model](docs/specs/2026-08-29-zion-arc-v1.md).

## Android MVP

The Compose application imports one original PNG through
`OpenDocument("image/png")`, performs bounded stream I/O, passes bytes—not a
decoded `Bitmap`—through one JNI method, opens and validates one book off the UI
thread, renders chapters, exposes BLIVRE attribution, and can export preserved
raw USFM. It declares no internet permission, analytics, account, ads, database,
or remote model.

Install the pinned local Android toolchain and build:

```bash
./scripts/setup-android.sh
./scripts/build-android.sh
```

Generated artifacts include:

```text
android/app/build/outputs/apk/debug/app-debug.apk
android/app/build/outputs/apk/release/app-release-unsigned.apk
```

The debug APK is locally signed for installation. The release APK is
intentionally unsigned. Generated APK/AAB/native-library files are ignored and
must not be committed. Toolchain pins, architecture, validation results, JNI
error policy, JVM zeroization limits, and the pending physical-device status are
documented in the [Android MVP record](docs/android/2026-08-29-android-mvp.md).

AI and an RTX 4090 are optional studio-side aids for creating and evaluating
carrier artwork. They do not perform encryption and are not present in the
reader path. Removing the model, prompts, generated covers, and GPU does not
change ARC decoding.

## Existing codec, stego, and optical commands

These formats remain useful and supported separately from ARC.

### ZION resilient symbols

```bash
./target/release/zion encode ./input.bin \
  --output-prefix ./out/input \
  --profile safe

./target/release/zion inspect ./out/input_000.zbin

./target/release/zion decode ./out/input_*.zbin \
  --output ./restored.bin
```

Symbols may arrive out of order. The codec provides per-block CRC32C, whole-file
BLAKE3, typed missing/duplicate/divergent diagnostics, and an explicit
`--force-write-corrupt` forensic path. Integrity is not confidentiality.

### Zion Optical exact PNG container

```bash
./target/release/zion optical-render ./input.bin \
  --output ./input.optical.png \
  --width 1080 \
  --height 2340

./target/release/zion optical-inspect ./input.optical.png --deep

./target/release/zion optical-extract ./input.optical.png \
  --output ./restored.bin
```

Optical v1 is a lossless digital RGB container. These commands do not establish
a screenshot, print, or camera-recovery guarantee.

### Legacy ZSTG photo embedding

```bash
./target/release/zion stego-embed ./input.bin \
  --host ./cover-a.png ./cover-b.png \
  --output-dir ./stego-output \
  --prompt

./target/release/zion stego-inspect ./stego-output/stego_000.png

./target/release/zion stego-extract ./stego-output/stego_*.png \
  --output ./restored.bin \
  --prompt
```

`ZSTG` v1 predates ARC and has a different wire format and cryptographic/ECC
ordering. Its CLI retains a direct passphrase option for compatibility; avoid it
because process arguments and shell history can expose secrets. Use `--prompt`
or a protected file.

## Build, tests, benchmarks, and fuzzing

The workspace is pinned to Rust 1.95.0:

```bash
cargo build --workspace
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo bench -p zion-codec
cargo bench -p zion-stego
cargo bench -p zion-optical
```

The separate fuzz workspace currently covers existing codec, stego, and optical
entry points:

```bash
cd fuzz
cargo +nightly fuzz run parse_header
cargo +nightly fuzz run parse_block
cargo +nightly fuzz run decode_full
cargo +nightly fuzz run stego_plaintext_header
cargo +nightly fuzz run stego_extract
cargo +nightly fuzz run optical_extract
```

Dedicated ARC bootstrap/envelope/full-open fuzz targets remain a Phase 6 task;
do not describe the ARC parser as fuzz-complete yet.

## Documentation

- [ARC v1 specification](docs/specs/2026-08-29-zion-arc-v1.md)
- [Hackathon implementation plan](docs/plans/2026-08-29-zion-arc-hackathon.md)
- [BLIVRE source and license provenance](docs/provenance/blivre-2018.2.0.md)
- [BLIVRE all-book capacity evidence](docs/results/2026-08-29-blivre-arc-fit.md)
- [Carrier artwork provenance](assets/carriers/README.md)
- [Carrier-art measurements](docs/results/2026-08-29-carrier-art.md)
- [Desktop CLI demo](docs/hackathon/2026-08-29-cli-demo.md)
- [Three-minute pitch and 90-second demo](docs/hackathon/2026-08-29-pitch.md)
- [Android MVP build and limitations](docs/android/2026-08-29-android-mvp.md)

## Licensing

Workspace code is dual-licensed under Apache-2.0 or MIT; see
[`LICENSE`](LICENSE), [`LICENSE-APACHE`](LICENSE-APACHE), and
[`LICENSE-MIT`](LICENSE-MIT).

That code license does not automatically cover BLIVRE content or generated
carrier artwork. BLIVRE has its own CC BY 3.0 Brazil attribution and provenance.
The carrier manifest records OpenAI image-generation provenance and makes no
claim that the artwork is copyright-free or public domain.

## Contributing

Preserve the three frozen legacy format identities and ARC's separate versioned
contract. Protocol changes require a spec/version review and tests at the
closest layer. Before a PR, run formatting, workspace tests, and strict clippy;
include compatibility risk and the exact commands executed. See
[`CONTRIBUTING.md`](CONTRIBUTING.md).
