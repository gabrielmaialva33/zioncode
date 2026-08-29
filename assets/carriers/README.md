# Zion ARC carrier artwork provenance

Date recorded: 2026-08-29

These two files are studio inputs for an exact-PNG ARC demonstration. They are
clean artwork, not sealed capsules, and the reader does not need either the
image generator or a GPU.

| File | Role | Dimensions / format | Bytes | SHA-256 | Pre-seal bootstrap check |
|---|---|---|---:|---|---|
| `zion-arc-river-source.png` | Image-generation result | 852×1846 RGB8 PNG | 2,721,196 | `853b33cc6c2fd3ee19db20e872dc7b727bdb78ba4144a3124c6507878711be07` | First four LSB-decoded bytes `61836da0`, not `ZARC` |
| `zion-arc-river-1080x2340.png` | Normalized phone carrier | 1080×2340 RGB8 PNG | 4,064,369 | `42f2312025af5acc2c2dba1f2a6c8ff642ea25ab390d15726695dc626f31e272` | First four LSB-decoded bytes `61b0b7b6`, not `ZARC` |

“Clean” here has a narrow, testable meaning: these files precede ARC sealing,
and the public bootstrap positions do not decode to ARC's `ZARC` magic
(`5a415243`). It is not a claim about steganalysis, provenance detection, or the
absence of arbitrary coincidental data elsewhere in the pixels.

## Image-generation provenance

The source image was produced on 2026-08-29 with OpenAI's built-in `imagegen`
capability available in the Codex environment, using the prompt below. No
external checkpoint was supplied. The interface did not expose a stable model
build identifier, seed, sampler, or deterministic regeneration contract, so
those values are recorded as unavailable rather than guessed. The normalized
file was made locally with FFmpeg and was not sent back through an image model.

The repository makes no claim that this artwork is copyright-free, public
domain, or covered by the source-code `MIT OR Apache-2.0` license. Anyone
redistributing or commercializing it must evaluate the applicable OpenAI terms,
project policy, and local law separately.

## Final prompt, verbatim

```text
Use case: stylized-concept
Asset type: premium Android phone wallpaper and steganographic carrier art for a hackathon demo
Primary request: create a serene, sophisticated abstract digital landscape that looks like an ordinary high-end phone wallpaper, with a luminous winding path or river moving through layered mineral canyons toward a distant horizon under a deep night sky
Scene/backdrop: an expansive otherworldly canyon with fine geological textures, subtle stars, atmospheric depth, and small natural luminous accents
Style/medium: polished cinematic digital matte painting, elegant and believable, high detail, rich organic microtexture throughout the entire image
Composition/framing: extra-tall portrait phone wallpaper, 9:19.5 aspect ratio, edge-to-edge composition, strong visual flow from lower foreground to upper horizon, no important details near crop edges
Lighting/mood: calm, hopeful, discreet, moonlit indigo and teal with restrained warm amber highlights
Color palette: deep navy, indigo, teal, muted cyan, warm amber
Materials/textures: detailed rock strata, fine mist, water reflections, subtle grain and natural variation; avoid large flat color areas and visible banding
Constraints: no text, no letters, no numbers, no logo, no QR code, no UI, no frame, no watermark, no faces, no people, no books, no cross, no religious symbols; must read simply as beautiful abstract wallpaper; high visual complexity without looking noisy
```

## Reproducible normalization

The literal command used for the phone-sized asset was:

```bash
ffmpeg -hide_banner -loglevel error -i "$source_copy" -vf scale=1080:2340:flags=lanczos -pix_fmt rgb24 -compression_level 6 "$phone_png"
```

Here `source_copy` is a byte-identical local copy of
`zion-arc-river-source.png`, and `phone_png` is the new normalized output path.
The command enforces the target dimensions and decoded `rgb24` pixel format;
the two PNG hashes above identify the exact files from this run.

PNG container bytes are not ARC's stable identity. Lossless encoders may choose
different filters, compression, or ancillary chunks and therefore produce a
different PNG byte count and SHA-256. ARC recovery depends on the exact
dimensions and row-major RGB8 samples. A lossless re-encode is acceptable only
when those values compare exactly; JPEG, resize, screenshot, print/camera, and
color conversion are unsupported.
