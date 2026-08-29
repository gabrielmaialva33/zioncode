# Zion ARC carrier-art evidence

Date: 2026-08-29

Status: three generated-and-normalized clean carrier pairs have each passed the
complete release-build desktop seal/open, lossless PNG re-encode, second open,
and coded image-quality measurement path with Genesis, Psalms, and Revelation.
The desktop-to-Android device demonstration has not been completed, and no
physical-device result is claimed here.

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
| First authenticated open | Byte-exact `cmp` |
| Open after lossless re-encode | Byte-exact `cmp` |
| PSNR, clean vs sealed | 59.393034 dB |
| SSIM, clean vs sealed | 0.998873 |

The PNG container measurements for that run are run-specific and non-normative;
resealing is intentionally nondeterministic. Plaintext fingerprints are not
published by the hardened demo workflow.

## Script acceptance rerun

The hardened executable workflow was built in release mode and run three times
from the repository root into `/tmp/zion-three-carrier.eby2It`. Each book
path's parent package supplied the required pinned companion `manifest.json`.
For example, the Psalms/river invocation was:

```bash
cargo build --release -p zion-cli
ZION_BIN="$PWD/target/release/zion" \
  ./scripts/build-art-demo.sh \
  /tmp/zion-phase3-final.xquGFX/corpus/books/19-PSA.json \
  /tmp/zion-phase3-final.xquGFX/passphrase.bin \
  /tmp/zion-phase3-final.xquGFX/batch-attribution.txt \
  assets/carriers/zion-arc-river-1080x2340.png \
  /tmp/zion-three-carrier.eby2It/psalms
```

The directory is an unversioned local validation artifact and must not be
committed or treated as a distributable package. Its completed state contains
only the owner-readable clean cover, two sealed PNG encodings, and public
metrics. Private book and attribution snapshots, transient metric diagnostics,
decoded RGB, and recovered plaintext lived only in the `0700` work directory
and were removed by explicit path before success. The original owner-only
passphrase file stayed outside the output; the script reopened, revalidated,
and pinned it through an inherited read-only descriptor only around each
`zion` invocation, and created no second passphrase file.

The separately prepared bounded-corruption workspace was not one of those
three cleaned script output directories. After its metrics were recorded,
`verified.json`, two decoded-RGB files, and the CLI logs were explicitly
removed. Only the encrypted corruption-fixture PNG remained, mode `0600`,
under its private parent directory.

| Release-run measurement | Genesis / dunes | Psalms / river | Revelation / forest |
|---|---:|---:|---:|
| Clean PNG bytes | 3,871,473 | 4,064,369 | 4,267,490 |
| Sealed PNG bytes | 3,672,542 | 3,972,476 | 4,172,242 |
| Sealed PNG SHA-256 | `088846355cab0266e771d649d68c3cdea15cc67c26dda3b9d15cbc813c762cb5` | `78947eff4439d4fcf8b73f225f6082655a44ccce59bcf009f8a51c371884dd1b` | `eaeca1e763da572fe7aa5e95b49b22284bb2fa2768475ff17afed43642eed621` |
| Compression-level-0 PNG bytes | 7,607,311 | 7,607,311 | 7,607,311 |
| Compression-level-0 PNG SHA-256 | `bfaec121bcdc0c77ee4eed75f31633525af9b7fc2d603c9954326dda18f19afc` | `ee7be46650bcee03f6270f3be8d62137ddda603a669768a7c7661cde21f3b1ba` | `68df3103468dcd855147534fbc1395b42df03940aa78169dd703a233a830b535` |
| Sealed/re-encoded raw RGB SHA-256 | `2e7343252bb94130bb948d1224232f30f0bd22dc50ebeb37f9343d361060cfce` | `7e2576bbab68902f65f524aeaad59bd93d895619d1da16322f6b8c4c36bb0a61` | `ce456b5ea9a7dbb53841c4060672e769a19de6872cefe2f9fb9b969cfd9ea533` |
| Safe codewords | 360 | 556 | 128 |
| Compressed payload bytes | 79,570 | 123,349 | 27,948 |
| Ciphertext bytes | 80,280 | 123,988 | 28,544 |
| RS parity bytes | 11,520 | 17,792 | 4,096 |
| Interleaved ECC bytes | 91,800 | 141,780 | 32,640 |
| Changed RGB channels | 367,419 | 567,057 | 130,990 |
| Changed-channel percentage | 4.846193416% | 7.479384299% | 1.727735570% |
| Mean absolute channel delta | 0.048461934156 | 0.074793842988 | 0.017277355703 |
| Maximum channel delta | 1 | 1 | 1 |
| PSNR | 61.276796 dB | 59.392145 dB | 65.756031 dB |
| SSIM | 0.999600 | 0.998846 | 0.999828 |
| Key derivation wall time | 66.859 ms | 60.855 ms | 66.535 ms |
| Seal wall time | 382.023 ms | 321.093 ms | 333.019 ms |
| First open wall time | 187.477 ms | 174.772 ms | 197.077 ms |
| Re-encoded open wall time | 188.340 ms | 158.563 ms | 182.178 ms |
| First and second `cmp` | Exact | Exact | Exact |
| Sealed/re-encoded RGB identity | Exact | Exact | Exact |
| Script stderr | 0 bytes | 0 bytes | 0 bytes |

The seal totals include CLI, PNG, ECC, and cryptographic work; the KDF row is
the CLI's separately instrumented collection-key derivation time. Every output
directory was mode `0700` and every retained file was mode `0600`.

The compressed payload is the zstd-6 book content before the authenticated ARC
envelope. Ciphertext then occupies the data portion of the Safe codewords; RS
parity is added after encryption, and ciphertext plus parity forms the
interleaved ECC stream. The separate capacity table above states the carrier's
maximums, not the bytes used by any one run.

The source and sealed PNG byte hashes differ by design. The sealed and
compression-level-0 PNG byte hashes also differ even though their decoded
dimensions and all 7,581,600 RGB8 samples are identical. PNG filtering,
compression, and ancillary metadata are not the ARC identity.

## Bounded-corruption fixture

The same release run exercised the explicit `zion arc corrupt-fixture` test
path against the Psalms/river capsule. It changed exactly 16 reconstructed
payload bytes in each of 556 Safe codewords: 8,896 altered payload bytes and
8,896 RGB channels, each with maximum absolute delta 1. This is the Safe
profile's documented unknown-error boundary per codeword. RS decoded the
fixture, repairing that measured damage, and the subsequent AEAD check and
byte-exact plaintext `cmp` passed. The purpose-built corrupted PNG was 3,973,198
bytes with run-specific SHA-256
`85f1638f864c361e91fb94b439c9be439632b3ae848e15922f0c7ead3f4daffe`.

This is a controlled exact-dimension/RGB8 protocol fixture, not evidence for
random image editing, JPEG, screenshots, resizing, printing, cameras, or
social-media recompression.

## Measurement method

[`scripts/build-art-demo.sh`](../../scripts/build-art-demo.sh) performs the
measurements rather than copying slide estimates. It:

1. infers the companion manifest from `corpus/books/NN-ID.json`, snapshots it
   privately, and verifies the exact pinned BLIVRE source, 66-entry canonical
   mapping, selected payload identity, raw USFM, complete renderable
   `BookDocument`, and exact canonical attribution before invoking `zion`;
2. snapshots the clean RGB8 cover, then reopens, revalidates, and pins the
   original owner-only passphrase file through an inherited read-only descriptor
   only around each `zion` invocation; it creates no second passphrase file and
   never prints or hashes the passphrase bytes;
3. derives the encrypted `NN — title` metadata only from the verified book
   snapshot, retains capacity and seal diagnostics only long enough to parse
   public metrics, suppresses open diagnostics, seals and opens through
   `zion arc`, and requires byte-exact `cmp` without publishing plaintext
   metadata or fingerprints;
4. losslessly re-encodes the sealed PNG with FFmpeg
   `-pix_fmt rgb24 -compression_level 0`, requires 5,000,000–9,000,000 bytes,
   opens it again, and requires the same plaintext;
5. decodes all three images to raw RGB8, requires exact sealed/re-encoded
   raster identity, and computes only public carrier/stego raster SHA-256;
6. counts changed channels and absolute deltas with bounded chunked Python
   standard-library code; and
7. runs FFmpeg's `psnr` and `ssim` filters and parses their measured output.

`bash -n`, `shellcheck`, and `git diff --check` pass for the script. With
`ZION_BIN=/usr/bin/false`, three independent fixtures—a fake package containing
a traversal manifest entry and mutated book, a mutated book beside the real
manifest, and wrong attribution—each failed with the same generic verification
error before the CLI could run. Each produced zero stdout bytes and left no
output directory. The script uses neither a cleanup glob nor recursive
deletion.

An unavoidable abrupt-termination limit was also tested: the monitor observed
the real release `zion` executable running `arc seal`, then `SIGKILL` of its
process group produced exit 137 before the shell trap could run. It left a
private `0700` output with
the clean cover and a `.metrics-work` directory containing owner-only book,
attribution, manifest, mapping, and transient log snapshots. It left no
passphrase filename, passphrase copy, or byte-identical passphrase artifact;
the per-invocation descriptor closed when the process group died. The test
directory was removed explicitly after inspection. An operator must likewise
remove an incomplete output directory after a power loss or untrappable termination;
depending on when that happens, any private work file that existed at that
moment—including recovered plaintext or decoded RGB—can remain until that
manual cleanup.

## Limits and claim boundary

This proves an exact lossless PNG path for three covers and three whole books on
the measured desktop release build. It does
not prove visual indetectability, resistance to steganalysis, JPEG/screenshot/
resize survival, print or camera recovery, a three-cover Android demo, physical
Galaxy A57 performance, independent cryptographic audit, or post-quantum
security. The public bootstrap makes ARC existence detectable. The visible
cover, upper seven bits of RGB channels, unused RGB channels (and alpha, if an
external noncanonical tool introduces it), and PNG metadata are not
authenticated; authenticated data is limited to the protocol bootstrap and
dimensions as AAD plus the encrypted envelope and recovered content.
