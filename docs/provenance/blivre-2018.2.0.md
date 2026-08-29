# BLIVRE 2018.2.0 license and provenance

Date recorded: 2026-08-29

This document applies to the BLIVRE sample corpus. It does not change the license of the Zion source code, ARC format implementation, carrier artwork, or other bundled material.

## Immutable upstream identity

| Field | Pinned value |
|---|---|
| Work | Bíblia Livre (BLIVRE), Textus Receptus USFM release |
| Version | 2018.2.0 (February 2018) |
| Authors credited upstream | Diego Santos, Mario Sérgio, Marco Teles |
| Release | [`2018.2.0`](https://github.com/blivre/BibliaLivre/releases/tag/2018.2.0) |
| Release asset | [`usfm-blivre-tr.zip`](https://github.com/blivre/BibliaLivre/releases/download/2018.2.0/usfm-blivre-tr.zip) |
| Tagged commit | [`a386942daee9984c654ebc8cea95ec9d3661b183`](https://github.com/blivre/BibliaLivre/tree/a386942daee9984c654ebc8cea95ec9d3661b183) |
| Asset length | 1,364,020 bytes |
| Asset SHA-256 | `83e5435706b2d640d0b35b690b0cf0a4508d910dd26b895d155c37a670729ead` |
| Content license | Creative Commons Attribution 3.0 Brazil (CC BY 3.0 BR) |
| License deed/legal code | [creativecommons.org/licenses/by/3.0/br](https://creativecommons.org/licenses/by/3.0/br/) |

The importer accepts only that exact asset length and SHA-256. It checks both before ZIP extraction, applies bounded ZIP reads, rejects unexpected corpus structure, and requires the checked-in 66-book canonical mapping in `zion-corpus/data/books.json`. It does not infer canonical order from upstream filenames.

The tag-specific notices are available upstream as the [README](https://github.com/blivre/BibliaLivre/blob/2018.2.0/README.md) and [license](https://github.com/blivre/BibliaLivre/blob/2018.2.0/LICENCA.md). Exact checked-in copies are preserved by the importer:

| Notice | Bytes | SHA-256 |
|---|---:|---|
| `UPSTREAM_README.md` | 2,566 | `2c9188d1031b419ec4c59288f3aa41368bbfd5a4b646edaf6ea74169e1a46114` |
| `UPSTREAM_LICENSE.md` | 16,646 | `6ff396843c629e408e3dbad8b16653acbfed6448fe4011bbde4e13baaae706b7` |

## Attribution carried by ARC

`zion arc blivre seal-batch` encrypts this exact 384-byte UTF-8 attribution with each book:

> Todas as Escrituras em português citadas são da Bíblia Livre (BLIVRE), Copyright © Diego Santos, Mario Sérgio, e Marco Teles, http://sites.google.com/site/biblialivre/ - fevereiro de 2018. Licença Creative Commons Atribuição 3.0 Brasil (https://creativecommons.org/licenses/by/3.0/br/). The normalized JSON retains byte-exact, exportable source USFM and its pinned provenance.

The text has no trailing line terminator and SHA-256 `459e4e5585e1ee8692f07aa947fe08b200f5e40afc09e46961fda42c5d5d070d`. Every normalized book also contains structured source metadata: work name, version, version date, release and asset URLs, tagged commit, license name and URL, and author names.

Redistributors should present attribution where a recipient can reasonably see it, preserve applicable copyright and license information, link or include the license, identify this as the February 2018 version, and identify material transformations. The upstream license text controls; this document is an engineering record, not legal advice.

## Transformation and export record

The deterministic importer performs a format projection, not a new translation:

1. It verifies the pinned ZIP before extraction and matches all 66 source files to the checked-in canonical IDs, titles, ordinals, chapter counts, verse-record counts, lengths, and hashes.
2. It parses only the deliberately bounded marker vocabulary present in or needed for this release: `id`, `ide`, `h`, `toc1`, `toc2`, `toc3`, `mt`, `mt1`, `p`, `v`, `d`, `add`/`add*`, `f`/`f*`, `fr`, `fq`, `ft`, and `rq`/`rq*`.
3. It removes the UTF-8 BOM and changes CRLF to LF only in the structured marker projection. Marker order, inline additions, headings, references, and footnote records remain explicit. Unsupported markers and unbalanced inline state are errors rather than silently dropped text.
4. It retains the complete verified source USFM in each JSON document's `raw_usfm` field, including BOM and CRLF when reconstructed as UTF-8 bytes. It also writes every original USFM file separately under `provenance/raw-usfm/` so it can be exported byte-for-byte.
5. It serializes compact deterministic UTF-8 JSON and records raw and normalized hashes and size metrics in `manifest.json`.
6. ARC compresses that JSON with zstd level 6, then encrypts and authenticates it. Opening a capsule returns the exact JSON bytes supplied to sealing.

The materialized package is deliberately separate from generated ARC images:

```text
package/
  manifest.json
  books/01-GEN.json ... books/66-REV.json
  provenance/
    SOURCE.json
    UPSTREAM_README.md
    UPSTREAM_LICENSE.md
    usfm-blivre-tr.zip
    raw-usfm/<66 exact source files>
```

On Unix, each package directory is created as `0700` and every package file as `0600` at creation time, even with a permissive umask. Existing destinations are refused. These privacy modes are operational defaults, not a substitute for carrying the license and attribution when the content is distributed.

## License boundaries

- The BLIVRE scripture text and the source-derived normalized documents are governed by the upstream CC BY 3.0 BR terms.
- Zion source code is separately dual-licensed under `MIT OR Apache-2.0`; see [`LICENSE`](../../LICENSE), [`LICENSE-MIT`](../../LICENSE-MIT), and [`LICENSE-APACHE`](../../LICENSE-APACHE). CC BY does not automatically apply to the code.
- Carrier artwork, fonts, prompts, models, and generation inputs need their own provenance and licenses. The BLIVRE license does not grant rights to an unrelated cover image.
- Packaging multiple items together does not erase their separate notices or imply endorsement by the BLIVRE authors.

CC BY 3.0 BR allows licensed reuse and adaptation subject to its conditions. If this transformed corpus or a capsule containing it is distributed, retain the attribution, license reference, version, source identity, and normalization notice. Give intended recipients both the means to open/export the content and the license information. Do not use the encryption layer as an effective access restriction that prevents recipients from exercising rights granted by the content license.

## Operational and security limits

Passphrases are read from an owner-only bounded file or a no-echo prompt, never from a dedicated command-line passphrase option. Opened plaintext is created as `0600` on Unix with exclusive creation; existing paths are refused. Passphrases must be shared separately through an appropriate channel and are not recoverable from the provenance data.

ARC authentication detects a wrong passphrase or tampering, and Reed-Solomon coding handles only its stated bounded error budget. Recovery requires the original dimensions and RGB8 pixel samples in a valid lossless PNG; the container bytes may differ after a lossless PNG re-encode. Screenshots, resizing, cropping, palette or color conversion, lossy recompression, and social-media processing are not supported. ARC has not received an independent cryptographic audit or a dedicated steganalysis evaluation, and encryption does not itself make transport lawful in every jurisdiction. Users remain responsible for applicable export, import, content, and local laws.
