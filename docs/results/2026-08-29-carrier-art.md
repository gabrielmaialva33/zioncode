# Zion ARC carrier-art evidence

Date: 2026-08-29

Status: one generated-and-normalized cover has passed the complete desktop
Psalms seal/open, lossless PNG re-encode, second open, and coded image-quality
measurement path. The planned three-cover desktop-to-Android demonstration has
not been completed, and no physical-device result is claimed here.

## What was measured

The clean carrier is
[`assets/carriers/zion-arc-river-1080x2340.png`](../../assets/carriers/zion-arc-river-1080x2340.png).
Its generation prompt, provenance, normalization command, hashes, and clean-art
status are recorded in the [carrier asset manifest](../../assets/carriers/README.md).
The real payload was the canonical normalized BLIVRE Psalms document produced
by the pinned importer.

| Input | Measurement |
|---|---:|
| Clean carrier | 1080×2340, RGB8 (`rgb24`) |
| Clean carrier PNG | 4,064,369 bytes |
| Clean carrier SHA-256 | `42f2312025af5acc2c2dba1f2a6c8ff642ea25ab390d15726695dc626f31e272` |
| Clean carrier bootstrap-position bytes | `61b0b7b6` (not `ZARC`) |
| Normalized Psalms JSON | 657,274 bytes |
| Normalized Psalms SHA-256 | `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f` |
| ARC zstd-6 content | 123,349 bytes |
| Encrypted item name | `19 — Salmos` (13 UTF-8 bytes) |
| Encrypted media type | `application/vnd.zion.blivre+json` (32 bytes) |
| Encrypted attribution | 384 UTF-8 bytes |

The 123,349-byte value is the normalized JSON compressed for ARC. It must not
be confused with the separately measured 80,236-byte checksum-zstd size of raw
Psalms USFM.

## Capacity is not PNG file size

At 1080×2340, Safe profile capacity is determined by the number of usable RGB
channel positions and Reed–Solomon geometry. It is unrelated to whether a PNG
encoder happens to produce a 4 MB or 7.6 MB container.

| ARC geometry | Value |
|---|---:|
| RGB channels | 7,581,600 |
| Candidate bits after the public bootstrap | 7,581,088 |
| Usable placement bits (33%) | 2,501,759 |
| Maximum Safe codewords | 1,226 |
| Maximum Safe ciphertext | 273,398 bytes |
| Maximum AEAD plaintext | 273,382 bytes |
| Maximum interleaved stream | 312,630 bytes |

The Psalms capsule used 556 Safe RS(255,223) codewords, 123,988 ciphertext
bytes, 17,792 parity bytes, a 141,780-byte interleaved stream, and 1,134,240
payload-placement bits. Its 670-codeword margin is protocol capacity. Raising
PNG container size with `compression_level 0` does not create more ARC capacity
and does not change any RGB sample.

## Supplied reference run

The earlier real workspace `/tmp/zion-art-demo.3fJG0d` produced:

| Measurement | Value |
|---|---:|
| Sealed PNG | 3,972,259 bytes |
| FFmpeg compression-level-0 PNG | 7,607,311 bytes |
| First authenticated open | Exact, SHA-256 `0b2f3d…db6f` |
| Open after lossless re-encode | Exact, SHA-256 `0b2f3d…db6f` |
| PSNR, clean vs sealed | 59.393034 dB |
| SSIM, clean vs sealed | 0.998873 |

The abbreviated plaintext hash above denotes the complete Psalms SHA-256
recorded in the input table. Any PNG hashes for that run are run-specific and
non-normative; resealing is intentionally nondeterministic.

## Script acceptance rerun

The new executable workflow was then run from the repository root into a fresh
private directory:

```bash
cargo build -p zion-cli
ZION_BIN="$PWD/target/debug/zion" \
  ./scripts/build-art-demo.sh \
  /tmp/zion-phase3-final.xquGFX/corpus/books/19-PSA.json \
  /tmp/zion-phase3-final.xquGFX/passphrase.bin \
  /tmp/zion-phase3-final.xquGFX/batch-attribution.txt \
  assets/carriers/zion-arc-river-1080x2340.png \
  /tmp/zion-art-phase5-final.9G8ZgU/demo
```

The directory is an unversioned local validation artifact. It contains private
plaintext copies and must not be committed or treated as a distributable
package.

| Measurement | Fresh run value |
|---|---:|
| Sealed PNG | 3,972,714 bytes |
| Sealed PNG SHA-256 | `b2b04150ecd1d5129477b8a99b2b5d9e464308533940ff116f0e14fc571a977e` |
| FFmpeg compression-level-0 PNG | 7,607,311 bytes |
| Compression-level-0 PNG SHA-256 | `c2d12092a499e5987183889d8203afd916380092b7be4fdddcb01405f1e5cbd0` |
| Sealed raw RGB SHA-256 | `6dc118d65b6c81511687e6b3a1004a68065b599e80d8644b729e55ea88ec51b1` |
| Re-encoded raw RGB SHA-256 | `6dc118d65b6c81511687e6b3a1004a68065b599e80d8644b729e55ea88ec51b1` |
| Sealed/re-encoded geometry and RGB samples | Exact |
| First JSON open and `cmp` | Exact |
| Re-encoded JSON open and `cmp` | Exact |
| Changed RGB channels | 567,655 of 7,581,600 (7.487271816%) |
| Mean absolute channel delta | 0.074872718160 over all RGB channels |
| Maximum channel delta | 1 |
| PSNR, clean vs sealed | 59.387568 dB |
| SSIM, clean vs sealed | 0.998874 |
| Seal wall time | 4,299.525 ms |
| First open wall time | 2,497.325 ms |
| Re-encoded open wall time | 2,477.146 ms |

The wall times came from this unoptimized host debug binary and include the
CLI, PNG, ECC, and cryptographic work. Argon2 time is not separately
instrumented by the CLI, so no isolated Argon2 number is asserted. No deliberate
corruption fixture was introduced in this carrier-art run; the protocol's
per-codeword recovery bounds and core corruption tests must not be restated as a
measured artwork-corruption count.

The source and sealed PNG byte hashes differ by design. The sealed and
compression-level-0 PNG byte hashes also differ even though their decoded
dimensions and all 7,581,600 RGB8 samples are identical. PNG filtering,
compression, and ancillary metadata are not the ARC identity.

## Measurement method

[`scripts/build-art-demo.sh`](../../scripts/build-art-demo.sh) performs the
measurements rather than copying slide estimates. It:

1. validates and snapshots the canonical JSON, attribution, and clean RGB8
   cover inside a newly created `0700` output directory;
2. validates an owner-only passphrase file without printing or hashing its
   contents;
3. seals and opens through `zion arc`, then requires byte-exact `cmp`;
4. losslessly re-encodes the sealed PNG with FFmpeg
   `-pix_fmt rgb24 -compression_level 0`, requires 5,000,000–9,000,000 bytes,
   opens it again, and requires the same plaintext;
5. decodes all three images to raw RGB8, requires exact sealed/re-encoded
   raster identity, and computes SHA-256;
6. counts changed channels and absolute deltas with bounded chunked Python
   standard-library code; and
7. runs FFmpeg's `psnr` and `ssim` filters and parses their measured output.

`bash -n` and `shellcheck` pass for the script. A failed first trial also proved
that its explicit-path cleanup removed the incomplete output directory; it uses
neither a cleanup glob nor recursive deletion.

## Limits and claim boundary

This proves an exact lossless PNG path for one cover and one whole book. It does
not prove visual indetectability, resistance to steganalysis, JPEG/screenshot/
resize survival, print or camera recovery, a three-cover Android demo, physical
Galaxy A57 performance, independent cryptographic audit, or post-quantum
security. The public bootstrap makes ARC existence detectable. The visible
cover, upper seven bits of RGB channels, unused pixels, and PNG metadata are not
authenticated; authenticated data is limited to the protocol bootstrap and
dimensions as AAD plus the encrypted envelope and recovered content.
