# BLIVRE 2018.2.0 ARC fit results

Date: 2026-08-29

Status: measured on a fresh final-hardening import. All 66 normalized books fit a 1080x2340 RGB8 carrier with the ARC `safe` profile. This is a capacity and byte-roundtrip result, not a claim of visual undetectability or audited cryptographic security.

## Reproducible source

- Release: [BLIVRE 2018.2.0](https://github.com/blivre/BibliaLivre/releases/tag/2018.2.0)
- Asset: [`usfm-blivre-tr.zip`](https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip)
- Tagged commit: `a386942daee9984c654ebc8cea95ec9d3661b183`
- Verified asset size: `1,364,020` bytes
- Verified asset SHA-256: `83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead`
- Import result: 66 books, 1,189 chapters, and 31,102 verse records
- Manifest: 34,734 bytes; SHA-256 `84f99c4af8f4e0c63b39deb91c9928118123935f9f83127b9475abab544777cc`

### Reproducible command and evidence record

The primary source acquisition was a fresh official download under `/tmp/zion-blivre-phase3.1N0inE`. Size and hash were checked before extraction:

```bash
FRESH_DIR=/tmp/zion-blivre-phase3.1N0inE
mkdir -m 700 "$FRESH_DIR"
curl --fail --location \
  --output "$FRESH_DIR/usfm-blivre-tr.zip" \
  https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip
stat -c '%s %n' "$FRESH_DIR/usfm-blivre-tr.zip"
sha256sum "$FRESH_DIR/usfm-blivre-tr.zip"
mkdir -p "$FRESH_DIR/extracted"
/usr/sbin/bsdtar -xf "$FRESH_DIR/usfm-blivre-tr.zip" \
  -C "$FRESH_DIR/extracted"
```

```text
1364020 /tmp/zion-blivre-phase3.1N0inE/usfm-blivre-tr.zip
83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead  /tmp/zion-blivre-phase3.1N0inE/usfm-blivre-tr.zip
```

The later private-output-mode patch run was a separate copied-and-reverified rerun, not another fresh download. It used `/tmp/zion-phase3-postpatch.4k17ht` and preceded the final staging-publication and cover-TOCTOU hardening:

```bash
POSTPATCH_DIR=/tmp/zion-phase3-postpatch.4k17ht
mkdir -m 700 "$POSTPATCH_DIR"
cp /tmp/zion-phase3-validation.QxoXjF/usfm-blivre-tr.zip \
  "$POSTPATCH_DIR/usfm-blivre-tr.zip"
stat -c '%s %n' "$POSTPATCH_DIR/usfm-blivre-tr.zip"
sha256sum "$POSTPATCH_DIR/usfm-blivre-tr.zip"
cargo build -p zion-cli

(
  umask 000
  ./target/debug/zion arc blivre import \
    "$POSTPATCH_DIR/usfm-blivre-tr.zip" \
    --output-dir "$POSTPATCH_DIR/corpus"
)

./target/debug/zion arc blivre analyze \
  "$POSTPATCH_DIR/usfm-blivre-tr.zip" \
  --width 1080 --height 2340 --profile safe
```

Import reported `66 books, 1189 chapters, 31102 verse records`; analyze ended with `all_books_fit yes`. The security-mode evidence was:

```bash
find "$POSTPATCH_DIR/corpus" -type d ! -perm 0700 -print
find "$POSTPATCH_DIR/corpus" -type f ! -perm 0600 -print
find "$POSTPATCH_DIR/corpus" -type d | wc -l
find "$POSTPATCH_DIR/corpus" -type f | wc -l
```

Both violation searches produced no output; the counts were four directories and 137 files.

The real batch-equivalent Psalms seal/open sequence was:

```bash
./target/debug/zion optical-render Cargo.toml \
  --output "$POSTPATCH_DIR/cover-1080x2340.png" \
  --width 1080 --height 2340
cp /tmp/zion-phase3-validation.QxoXjF/batch-attribution.txt \
  "$POSTPATCH_DIR/batch-attribution.txt"
openssl rand -out "$POSTPATCH_DIR/passphrase.bin" 32
chmod 600 "$POSTPATCH_DIR/passphrase.bin"

(
  umask 000
  ./target/debug/zion arc seal \
    "$POSTPATCH_DIR/corpus/books/19-PSA.json" \
    "$POSTPATCH_DIR/cover-1080x2340.png" \
    --output "$POSTPATCH_DIR/19-PSA.batch.arc.png" \
    --name '19 — Salmos' \
    --media-type 'application/vnd.zion.blivre+json' \
    --attribution-file "$POSTPATCH_DIR/batch-attribution.txt" \
    --profile safe \
    --passphrase-file "$POSTPATCH_DIR/passphrase.bin"
  ./target/debug/zion arc open \
    "$POSTPATCH_DIR/19-PSA.batch.arc.png" \
    --output "$POSTPATCH_DIR/19-PSA.batch.opened.json" \
    --passphrase-file "$POSTPATCH_DIR/passphrase.bin"
)

stat -c '%s %a %n' \
  "$POSTPATCH_DIR/corpus/books/19-PSA.json" \
  "$POSTPATCH_DIR/19-PSA.batch.opened.json"
sha256sum \
  "$POSTPATCH_DIR/corpus/books/19-PSA.json" \
  "$POSTPATCH_DIR/19-PSA.batch.opened.json" \
  "$POSTPATCH_DIR/19-PSA.batch.arc.png"
rhash --blake3 \
  "$POSTPATCH_DIR/corpus/books/19-PSA.json" \
  "$POSTPATCH_DIR/19-PSA.batch.opened.json" \
  "$POSTPATCH_DIR/19-PSA.batch.arc.png"
cmp -s \
  "$POSTPATCH_DIR/corpus/books/19-PSA.json" \
  "$POSTPATCH_DIR/19-PSA.batch.opened.json"
```

Seal reported 556 codewords, 123,988 ciphertext bytes, and a 666,592-byte PNG. Input and opened output were each 657,274 bytes in mode `0600` with identical SHA-256 `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f` and BLAKE3 `860fc75947f898f1561565309988239ec13365040741c72010de3ba1238bf477`; `cmp` returned zero. The ARC PNG hashes were SHA-256 `b4cb04f4defa2a379720a445cc8c0dc88686fd35dad237c31b02c98baa3400ed` and BLAKE3 `239668f5c634d06730be9ed65137db9101e39d008c72937eabf1e298fc9c36f3`.

No generated ZIP, corpus, passphrase, plaintext, RGB dump, or ARC PNG is checked into the repository.

## Aggregate corpus measurements

| Metric | Bytes |
|---|---:|
| Raw USFM | 4,276,761 |
| Raw USFM, zstd level 6 with frame checksum | 1,391,039 |
| Normalized JSON | 10,043,674 |
| Normalized JSON, ARC zstd level 6 | 1,874,794 |

The two zstd columns intentionally measure different inputs. `raw_usfm_zstd6_checksum_bytes` measures the byte-exact source USFM with a zstd frame checksum. `payload_json_zstd6_bytes` measures the deterministic normalized JSON with the same no-explicit-checksum zstd-6 path used by ARC.

### Pinned marker audit

The extracted 2018.2.0 asset uses `\mt` in all 66 files. It contains no `\mt1` and no `\fr` markers; those are parser compatibility variants, not observed-source claims. `DEU` 32:4 is `\v 4 \add Ele é\add* ...`: the `v 4` marker has an empty base-text segment, followed by a preserved `add` segment rather than silently merged or dropped text.

## Per-book fit

Parameters: 1080x2340 RGB8, ARC v1 `safe` profile, RS(255,223), 32 parity bytes and correction of up to 16 unknown erroneous bytes per codeword. The batch-equivalent encrypted metadata is included in every estimate.

| Ordinal | ID | Title | JSON bytes | zstd-6 bytes | Codewords | Required payload bits | Fits |
|---:|---|---|---:|---:|---:|---:|:---:|
| 1 | GEN | Gênesis | 437436 | 79570 | 360 | 734400 | yes |
| 2 | EXO | Êxodo | 363808 | 62085 | 281 | 573240 | yes |
| 3 | LEV | Levítico | 267809 | 42600 | 194 | 395760 | yes |
| 4 | NUM | Números | 372256 | 59178 | 268 | 546720 | yes |
| 5 | DEU | Deuteronômio | 308039 | 53986 | 245 | 499800 | yes |
| 6 | JOS | Josué | 213321 | 36500 | 166 | 338640 | yes |
| 7 | JDG | Juízes | 211125 | 38337 | 175 | 357000 | yes |
| 8 | RUT | Rute | 28955 | 6469 | 32 | 65280 | yes |
| 9 | 1SA | 1 Samuel | 277968 | 51478 | 234 | 477360 | yes |
| 10 | 2SA | 2 Samuel | 229445 | 42903 | 195 | 397800 | yes |
| 11 | 1KI | 1 Reis | 276275 | 49147 | 223 | 454920 | yes |
| 12 | 2KI | 2 Reis | 255194 | 43813 | 199 | 405960 | yes |
| 13 | 1CH | 1 Crônicas | 257210 | 47104 | 214 | 436560 | yes |
| 14 | 2CH | 2 Crônicas | 295506 | 51456 | 234 | 477360 | yes |
| 15 | EZR | Esdras | 93664 | 17751 | 82 | 167280 | yes |
| 16 | NEH | Neemias | 133427 | 25450 | 117 | 238680 | yes |
| 17 | EST | Ester | 70765 | 13400 | 63 | 128520 | yes |
| 18 | JOB | Jó | 272171 | 54427 | 247 | 503880 | yes |
| 19 | PSA | Salmos | 657274 | 123349 | 556 | 1134240 | yes |
| 20 | PRO | Provérbios | 233105 | 44021 | 200 | 408000 | yes |
| 21 | ECC | Eclesiastes | 77856 | 15834 | 74 | 150960 | yes |
| 22 | SNG | Cantares | 42681 | 8970 | 43 | 87720 | yes |
| 23 | ISA | Isaías | 506793 | 97311 | 439 | 895560 | yes |
| 24 | JER | Jeremias | 536507 | 95427 | 431 | 879240 | yes |
| 25 | LAM | Lamentações | 49782 | 10931 | 52 | 106080 | yes |
| 26 | EZK | Ezequiel | 486784 | 82719 | 374 | 762960 | yes |
| 27 | DAN | Daniel | 145584 | 26580 | 122 | 248880 | yes |
| 28 | HOS | Oseias | 70894 | 15124 | 71 | 144840 | yes |
| 29 | JOL | Joel | 26009 | 5820 | 29 | 59160 | yes |
| 30 | AMO | Amós | 53519 | 11116 | 53 | 108120 | yes |
| 31 | OBA | Obadias | 9747 | 2648 | 15 | 30600 | yes |
| 32 | JON | Jonas | 17834 | 4249 | 22 | 44880 | yes |
| 33 | MIC | Miqueias | 41932 | 9405 | 45 | 91800 | yes |
| 34 | NAM | Naum | 18531 | 4697 | 24 | 48960 | yes |
| 35 | HAB | Habacuque | 20228 | 5131 | 26 | 53040 | yes |
| 36 | ZEP | Sofonias | 21840 | 5129 | 26 | 53040 | yes |
| 37 | HAG | Ageu | 15192 | 3397 | 18 | 36720 | yes |
| 38 | ZEC | Zacarias | 77942 | 15513 | 72 | 146880 | yes |
| 39 | MAL | Malaquias | 23272 | 5267 | 26 | 53040 | yes |
| 40 | MAT | Mateus | 331152 | 63472 | 287 | 585480 | yes |
| 41 | MRK | Marcos | 196004 | 38694 | 176 | 359040 | yes |
| 42 | LUK | Lucas | 339230 | 65317 | 296 | 603840 | yes |
| 43 | JHN | João | 242275 | 45668 | 208 | 424320 | yes |
| 44 | ACT | Atos | 344767 | 65775 | 298 | 607920 | yes |
| 45 | ROM | Romanos | 146064 | 28550 | 131 | 267240 | yes |
| 46 | 1CO | 1 Coríntios | 135594 | 26132 | 120 | 244800 | yes |
| 47 | 2CO | 2 Coríntios | 89211 | 18261 | 85 | 173400 | yes |
| 48 | GAL | Gálatas | 48260 | 10488 | 50 | 102000 | yes |
| 49 | EPH | Efésios | 48467 | 10367 | 49 | 99960 | yes |
| 50 | PHP | Filipenses | 32793 | 7410 | 36 | 73440 | yes |
| 51 | COL | Colossenses | 32481 | 7282 | 35 | 71400 | yes |
| 52 | 1TH | 1 Tessalonicenses | 28918 | 6368 | 31 | 63240 | yes |
| 53 | 2TH | 2 Tessalonicenses | 16531 | 3939 | 20 | 40800 | yes |
| 54 | 1TI | 1 Timóteo | 36713 | 8375 | 40 | 81600 | yes |
| 55 | 2TI | 2 Timóteo | 26578 | 6355 | 31 | 63240 | yes |
| 56 | TIT | Tito | 16649 | 4164 | 21 | 42840 | yes |
| 57 | PHM | Filemom | 8611 | 2378 | 13 | 26520 | yes |
| 58 | HEB | Hebreus | 117230 | 23961 | 110 | 224400 | yes |
| 59 | JAS | Tiago | 34165 | 8019 | 39 | 79560 | yes |
| 60 | 1PE | 1 Pedro | 42069 | 9329 | 45 | 91800 | yes |
| 61 | 2PE | 2 Pedro | 23647 | 5610 | 28 | 57120 | yes |
| 62 | 1JN | 1 João | 31647 | 6216 | 31 | 63240 | yes |
| 63 | 2JN | 2 João | 5343 | 1669 | 10 | 20400 | yes |
| 64 | 3JN | 3 João | 5976 | 1851 | 11 | 22440 | yes |
| 65 | JUD | Judas | 10557 | 2904 | 16 | 32640 | yes |
| 66 | REV | Apocalipse | 155062 | 27948 | 128 | 261120 | yes |

The largest normalized payload is Psalms (`PSA`): 657,274 JSON bytes and 123,349 zstd-6 bytes. The often-cited 80,236-byte value is a different metric: byte-exact raw Psalms USFM compressed with zstd-6 and a frame checksum. It must not be used as the ARC normalized-payload size.

## Carrier and ARC capacity

A fresh reference carrier was rendered for this run and independently inspected as a non-interlaced 1080x2340, 8-bit/color RGB PNG (`rgb24`).

| Measurement | Value |
|---|---:|
| RGB channels | 7,581,600 |
| Candidate bits after bootstrap reservation | 7,581,088 |
| Usable placement bits | 2,501,759 |
| Maximum codewords | 1,226 |
| Maximum ciphertext bytes | 273,398 |
| Maximum AEAD plaintext bytes | 273,382 |
| Maximum interleaved bytes | 312,630 |

The reference cover was 15,392 bytes with SHA-256 `bb61bb0fa62bc9d7793f135ca1a0b2aa15a64812e8ca6c36315018e0e896c17a`. Its decoded RGB bytes were 7,581,600 bytes with SHA-256 `5d586f30621a98c9c376b87139ff6c270d6344601efbc281c19a9f1d25a297d3`. `zion optical-render` assigns a fresh file identifier, so a rerendered carrier need not have the same PNG hash.

## Canonical Psalms seal

The real seal used exactly the metadata used by `zion arc blivre seal-batch`:

| Field | Exact value | UTF-8 bytes |
|---|---|---:|
| Name | `19 — Salmos` | 13 |
| Media type | `application/vnd.zion.blivre+json` | 32 |
| Attribution | Exact text recorded in the provenance document | 384 |

The attribution SHA-256 was `459e4e5585e1ee8692f07aa947fe08b200f5e40afc09e46961fda42c5d5d070d` and the file had no trailing line terminator.

| Geometry | Bytes or count |
|---|---:|
| Envelope header + name + media type + attribution + compressed content | 123,842 bytes |
| Random authenticated alignment padding (deterministic length) | 130 bytes |
| AEAD plaintext | 123,972 bytes |
| AEAD tag | 16 bytes |
| Ciphertext | 123,988 bytes |
| RS codewords | 556 |
| RS parity | 17,792 bytes |
| Interleaved stream | 141,780 bytes |
| Required payload bits | 1,134,240 |
| Bootstrap plus payload positions touched | 1,134,752 |

ARC pads the encrypted envelope so ciphertext ends on an RS data-codeword boundary. The alignment rule deterministically selects the 130-byte length for this envelope, while the padding bytes themselves come from the operating system CSPRNG and are authenticated. For these exact metadata lengths, the largest compressed content that can occupy all 1,226 codewords is 272,889 bytes.

| Margin | Value |
|---|---:|
| Codewords | 670 |
| Ciphertext bytes | 149,410 |
| Interleaved bytes | 170,850 |
| Usable placement bits | 1,367,519 |
| Compressed content bytes with the same metadata | 149,540 |
| Codeword utilization | 45.350734095% |
| Usable-bit utilization | 45.337700394% |
| RS expansion over ciphertext | 14.349775785% |
| Normalized JSON compression reduction | 81.233245192% |

## Reference seal/open proof before final staging hardening

These earlier reference commands ran under `umask 000` after rebuilding `zion-cli`.

- ARC PNG: 666,592 bytes; SHA-256 `b4cb04f4defa2a379720a445cc8c0dc88686fd35dad237c31b02c98baa3400ed`
- ARC PNG BLAKE3: `239668f5c634d06730be9ed65137db9101e39d008c72937eabf1e298fc9c36f3`
- Input Psalms JSON: 657,274 bytes; SHA-256 `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f`
- Opened Psalms JSON: 657,274 bytes; SHA-256 `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f`
- Input and opened Psalms JSON BLAKE3: `860fc75947f898f1561565309988239ec13365040741c72010de3ba1238bf477`
- Byte comparison: identical (`cmp` exit status 0)
- Opened plaintext mode on Unix: `0600`, created at open time even under the permissive umask
- Imported package under the same umask: all four directories were `0700` and all 137 files were `0600`

The measured collection ID was `dd6d64bc5608072450492361ea94a6d8` and capsule ID was `354c93c64d05297e192de322f1849700`. These IDs, the salt, nonce, ciphertext, changed pixels, and final PNG hash are intentionally nondeterministic on reseal.

## Final security-hardening rerun

After adding atomic `0700` staging creation, atomic no-replace publication, and no-follow cover snapshots, a new copied-and-reverified run used `/tmp/zion-phase3-hardening.Psdwo8`. The ZIP remained 1,364,020 bytes with SHA-256 `83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead`.

```bash
HARDENING_DIR=/tmp/zion-phase3-hardening.Psdwo8
(
  umask 000
  ./target/debug/zion arc blivre import \
    "$HARDENING_DIR/usfm-blivre-tr.zip" \
    --output-dir "$HARDENING_DIR/corpus"
)
find "$HARDENING_DIR" -maxdepth 1 -name '.corpus.zion-staging-*' -print
find "$HARDENING_DIR/corpus" -type d ! -perm 0700 -print
find "$HARDENING_DIR/corpus" -type f ! -perm 0600 -print

(
  umask 000
  ./target/debug/zion arc seal \
    "$HARDENING_DIR/corpus/books/19-PSA.json" \
    "$HARDENING_DIR/cover-1080x2340.png" \
    --output "$HARDENING_DIR/19-PSA.arc.png" \
    --name '19 — Salmos' \
    --media-type 'application/vnd.zion.blivre+json' \
    --attribution-file "$HARDENING_DIR/batch-attribution.txt" \
    --profile safe \
    --passphrase-file "$HARDENING_DIR/passphrase.bin"
  ./target/debug/zion arc open \
    "$HARDENING_DIR/19-PSA.arc.png" \
    --output "$HARDENING_DIR/19-PSA.opened.json" \
    --passphrase-file "$HARDENING_DIR/passphrase.bin"
)
```

Import reported 66 books, 1,189 chapters, and 31,102 verse records. No staging entry or mode-violation search produced output; the published package contained four `0700` directories and 137 `0600` files. This real import exercised the private staging directory and Linux atomic no-replace publication path.

The final-hardening seal reported 556 codewords and 123,988 ciphertext bytes. Its nondeterministic ARC PNG was 667,007 bytes, with SHA-256 `74c943ebc0514ee615d51a72388f13cefb2a73e6f84814043fc0098c2c675c8f` and BLAKE3 `f89c23a3cac22eb40eab1edca157ae164dd19dcc3952567566da3d1abc670ff3`. Input and opened JSON were each 657,274 bytes in mode `0600`, with identical SHA-256 `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f` and BLAKE3 `860fc75947f898f1561565309988239ec13365040741c72010de3ba1238bf477`; `cmp` returned zero. The measured collection ID was `2c09090b253da66c9e0d6de275cac5f3` and capsule ID was `afa4c731435f7a942fdacb24b9b88b6e`.

## Current transactional-boundary acceptance rerun

This rerun supersedes the earlier acceptance evidence above for the final `CorpusPackage::write_to_directory` validation and transaction boundary. It used `/tmp/zion-phase3-final.xquGFX`. The pinned ZIP was copied from the previously verified hardening workspace and reverified locally; it was not downloaded from the network again.

```bash
FINAL_DIR=/tmp/zion-phase3-final.xquGFX
mkdir -m 700 "$FINAL_DIR"
cp /tmp/zion-phase3-hardening.Psdwo8/usfm-blivre-tr.zip \
  "$FINAL_DIR/usfm-blivre-tr.zip"
stat -c '%s %n' "$FINAL_DIR/usfm-blivre-tr.zip"
sha256sum "$FINAL_DIR/usfm-blivre-tr.zip"
cargo build -p zion-cli

(
  umask 000
  ./target/debug/zion arc blivre import \
    "$FINAL_DIR/usfm-blivre-tr.zip" \
    --output-dir "$FINAL_DIR/corpus"
)
./target/debug/zion arc blivre analyze \
  "$FINAL_DIR/usfm-blivre-tr.zip" \
  --width 1080 --height 2340 --profile safe

find "$FINAL_DIR/corpus" -type d ! -perm 0700 -print
find "$FINAL_DIR/corpus" -type f ! -perm 0600 -print
find "$FINAL_DIR/corpus" -type d | wc -l
find "$FINAL_DIR/corpus" -type f | wc -l
find "$FINAL_DIR" -maxdepth 1 -name '.zion-corpus-staging-*' -print
sha256sum "$FINAL_DIR/corpus/manifest.json"

./target/debug/zion optical-render Cargo.toml \
  --output "$FINAL_DIR/cover-1080x2340.png" \
  --width 1080 --height 2340
cp /tmp/zion-phase3-hardening.Psdwo8/batch-attribution.txt \
  "$FINAL_DIR/batch-attribution.txt"
openssl rand -out "$FINAL_DIR/passphrase.bin" 32
chmod 600 "$FINAL_DIR/passphrase.bin"

(
  umask 000
  ./target/debug/zion arc seal \
    "$FINAL_DIR/corpus/books/19-PSA.json" \
    "$FINAL_DIR/cover-1080x2340.png" \
    --output "$FINAL_DIR/19-PSA.arc.png" \
    --name '19 — Salmos' \
    --media-type 'application/vnd.zion.blivre+json' \
    --attribution-file "$FINAL_DIR/batch-attribution.txt" \
    --profile safe \
    --passphrase-file "$FINAL_DIR/passphrase.bin"
  ./target/debug/zion arc open \
    "$FINAL_DIR/19-PSA.arc.png" \
    --output "$FINAL_DIR/19-PSA.opened.json" \
    --passphrase-file "$FINAL_DIR/passphrase.bin"
)
stat -c '%s %a %n' \
  "$FINAL_DIR/corpus/books/19-PSA.json" \
  "$FINAL_DIR/19-PSA.opened.json"
sha256sum \
  "$FINAL_DIR/corpus/books/19-PSA.json" \
  "$FINAL_DIR/19-PSA.opened.json" \
  "$FINAL_DIR/19-PSA.arc.png"
cmp -s \
  "$FINAL_DIR/corpus/books/19-PSA.json" \
  "$FINAL_DIR/19-PSA.opened.json"
```

The ZIP measured 1,364,020 bytes and SHA-256 `83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead`. Import reported 66 books, 1,189 chapters, and 31,102 verse records; analysis ended with `all_books_fit yes`. Both mode-violation searches and the staging-residue search produced no output. The published corpus contained four directories, all mode `0700`, and 137 files, all mode `0600`. Its 34,734-byte manifest retained SHA-256 `84f99c4af8f4e0c63b39deb91c9928118123935f9f83127b9475abab544777cc`.

The Psalms seal reported 556 codewords and 123,988 ciphertext bytes. The 657,274-byte input and opened JSON were both mode `0600`, both had SHA-256 `0b2f3d28602f0d945a6166f746c70d11f55c8a1528bf17998beb9d8570b8db6f`, and `cmp` returned zero. The ARC PNG was 666,347 bytes with SHA-256 `1df8dc09a785e299f11425187f2f197dae44f77c9a0e512d29cb640f045637b7`. That PNG size and hash are non-normative: salts, nonces, identifiers, authenticated padding bytes, and placement vary on every valid reseal.

## Reference image delta

| Measurement | Value |
|---|---:|
| Sealed decoded RGB SHA-256 | `172bae8b88611408e98430a8847f041ff855097597139c7ef3dbe9b9ad99778e` |
| Changed RGB channels | 566,957 of 7,581,600 (7.478065316%) |
| Changed among touched positions | 49.963075632% |
| Maximum channel delta | 1 |
| Mean absolute error | 0.074780653 |
| Mean squared error | 0.074780653 |
| PSNR | 59.392911 dB |
| SSIM | 0.939872 |

The RGB differences were measured after decoding both PNGs to `rgb24`. The reference is a high-contrast codec test carrier, not polished artwork. SSIM and apparent quality are carrier-dependent; these measurements do not establish resistance to statistical steganalysis.

## Validation environment and limits

- Rust/Cargo 1.95.0
- zstd 1.5.7
- FFmpeg 9.0.1
- Linux 7.1.9-zen, x86_64

ARC recovery requires the original dimensions and RGB8 pixel samples in a valid lossless PNG. Container bytes may differ after a lossless PNG re-encode without changing recoverability. Screenshots, lossy recompression, resizing, cropping, color conversion, and social-media processing are outside this result. The `safe` profile corrects bounded byte errors after extraction; it does not make arbitrary image transformations recoverable. The implementation has not received an independent cryptographic audit or a dedicated steganalysis evaluation. Legal redistribution duties remain those of the BLIVRE license described in the provenance document.
