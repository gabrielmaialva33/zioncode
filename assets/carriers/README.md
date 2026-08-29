# Zion ARC carrier artwork provenance

Date recorded: 2026-08-29

These six files are three clean source/phone carrier pairs for exact-PNG ARC
demonstrations. They are studio inputs, not sealed capsules. The reader needs
neither the image generator nor a GPU. Their separate rights notice is
[`LICENSE.md`](LICENSE.md); the repository's source-code licenses do not cover
these assets.

## File and decoded-raster manifest

`PNG bytes` and `PNG SHA-256` identify the exact containers produced in this
run. `Raw RGB bytes` and `raw RGB SHA-256` identify the decoded row-major RGB8
samples that ARC actually indexes. Those identities are deliberately separate:
a lossless PNG re-encode can change the first pair while preserving the second.

| Pair / file | Role | Dimensions / format | PNG bytes | PNG SHA-256 | Raw RGB bytes | Raw RGB SHA-256 | Clean bootstrap bytes |
|---|---|---|---:|---|---:|---|---|
| River — `zion-arc-river-source.png` | Image-generation result | 852×1846 RGB8 | 2,721,196 | `853b33cc6c2fd3ee19db20e872dc7b727bdb78ba4144a3124c6507878711be07` | 4,718,376 | `fa2a2713f45001d86fdb5e792b7f2650cb078e7673882198b0af5ae288d024af` | `61836da0` |
| River — `zion-arc-river-1080x2340.png` | Normalized phone carrier | 1080×2340 RGB8 | 4,064,369 | `42f2312025af5acc2c2dba1f2a6c8ff642ea25ab390d15726695dc626f31e272` | 7,581,600 | `7a7c39b60bfb78da6bca8cc11fde000b3ef4c39bc0373ff85ce5d0c578f384a1` | `61b0b7b6` |
| Dunes — `zion-arc-dunes-source.png` | Image-generation result | 852×1846 RGB8 | 2,595,348 | `60d5d2acd9cda6f2e3593abe06c16c90098cadc635eba62db5c97a72ff28f976` | 4,718,376 | `ed8de8516957cb3b84b4002fbeb43dcc9484c63fae3934789273ffcd30cee844` | `5398ae32` |
| Dunes — `zion-arc-dunes-1080x2340.png` | Normalized phone carrier | 1080×2340 RGB8 | 3,871,473 | `cd39b437a6e3d4de2f3f44453b74b5a4a49d6c3ff1c944ae1b293659817e18b0` | 7,581,600 | `3b76b976a97a78ec25dda866c9b4fcdf5d84d20e6b5e7faf25616e959b5595c3` | `57f35eb8` |
| Forest — `zion-arc-forest-source.png` | Image-generation result | 852×1846 RGB8 | 2,849,521 | `604041ade6bf4627a0bdc015fe32538917972cea609ebf12a44404f62f415b95` | 4,718,376 | `26362334f83bf273b2dfbc2b0440791ba95ee73dbd50896cea1dc6f11d018ea0` | `6915d829` |
| Forest — `zion-arc-forest-1080x2340.png` | Normalized phone carrier | 1080×2340 RGB8 | 4,267,490 | `af2aad825ced318d8361d89577a34a75145af21a0b5537dfca16debb41df4d86` | 7,581,600 | `87bf79f9d055bc06d5a29a27bc1fecfa6b58bf2cae49d25afa21364d0e8f090c` | `68a2e760` |

The final column is the first four bytes decoded MSB-first from the first 32
RGB-channel least-significant bits. None equals ARC's public `ZARC` magic
(`5a415243`). “Clean” has only that narrow, testable meaning: each file precedes
ARC sealing and lacks the public magic at the bootstrap position. It is not a
claim about steganalysis, provenance detection, or arbitrary coincidental data
elsewhere in the raster.

## Image-generation provenance

All three source images were requested as brand-new generations on 2026-08-29
through OpenAI's built-in `imagegen` capability in the Codex environment. No
input image, reference image, or external checkpoint was supplied. The
interface exposed no stable model build identifier, seed, sampler, or
deterministic regeneration contract, so those fields are unavailable rather
than guessed. The phone variants were normalized locally and were not sent
back through an image model.

This provenance is factual, not a claim that the artwork is copyright-free,
public domain, exclusive, non-infringing, or covered by the code license. See
the [carrier rights notice](LICENSE.md).

## River prompt, verbatim

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

## Dunes prompt, verbatim

The authoritative literal copy is
[`prompts/zion-arc-dunes.txt`](prompts/zion-arc-dunes.txt).

```text
Use case: stylized-concept
Asset type: second premium Android phone wallpaper and texture-rich steganographic carrier artwork for the Zion ARC hackathon demo collection
Primary request: create a serene abstract desert landscape of flowing mineral dunes and wind-carved stone ribbons leading toward a small luminous horizon beneath a vast predawn sky; it must look like an ordinary sophisticated phone wallpaper
Scene/backdrop: layered sandstone, fine granular dunes, distant misty ridges, subtle atmospheric particles and faint stars
Style/medium: polished cinematic digital matte painting, elegant, believable, highly detailed, rich organic microtexture across the full frame
Composition/framing: extra-tall portrait phone wallpaper, 9:19.5 aspect ratio, edge-to-edge, strong S-curve from the lower foreground to upper horizon, no important details near crop edges
Lighting/mood: quiet, contemplative and hopeful; cool violet-blue predawn with restrained copper and rose-gold highlights
Color palette: midnight blue, violet, muted mauve, sandstone copper, pale rose-gold
Materials/textures: fine sand grains, stratified rock, soft mist, subtle natural grain and tonal variation; avoid large flat areas, visible banding, harsh noise, or repeated patterns
Collection consistency: visually distinct from a river canyon while matching the same premium cinematic Zion ARC wallpaper family
Constraints: no text, no letters, no numbers, no logo, no QR code, no UI, no frame, no watermark, no faces, no people, no books, no cross, no religious symbols; must read simply as beautiful abstract wallpaper; high visual complexity without looking noisy
```

## Forest prompt, verbatim

The authoritative literal copy is
[`prompts/zion-arc-forest.txt`](prompts/zion-arc-forest.txt).

```text
Use case: stylized-concept
Asset type: third premium Android phone wallpaper and texture-rich steganographic carrier artwork for the Zion ARC hackathon demo collection
Primary request: create a tranquil abstract cloud-forest landscape with layered emerald terraces, delicate mist, and a narrow luminous trail winding upward toward a distant moonlit ridge; it must look like an ordinary sophisticated phone wallpaper
Scene/backdrop: deep botanical valley, mossy stone terraces, fine foliage, drifting fog, faint rain particles and a distant cool horizon
Style/medium: polished cinematic digital matte painting, elegant, believable, highly detailed, rich organic microtexture across the full frame
Composition/framing: extra-tall portrait phone wallpaper, 9:19.5 aspect ratio, edge-to-edge, strong visual flow from lower foreground to upper distance, no important details near crop edges
Lighting/mood: calm, restorative and discreet; blue-hour moonlight with restrained jade, cyan and soft amber bioluminescent accents
Color palette: deep forest green, jade, petrol blue, muted cyan, charcoal, very subtle amber
Materials/textures: intricate leaves, moss, wet rock, fine mist, water sheen and natural grain; avoid large flat areas, visible banding, harsh noise, or repeated patterns
Collection consistency: visually distinct from river canyon and mineral desert while matching the same premium cinematic Zion ARC wallpaper family
Constraints: no text, no letters, no numbers, no logo, no QR code, no UI, no frame, no watermark, no faces, no people, no books, no cross, no religious symbols; must read simply as beautiful abstract wallpaper; high visual complexity without looking noisy
```

## Reproducible normalization

Each source/phone pair used the same literal command with the variables bound to
that pair:

```bash
ffmpeg -hide_banner -loglevel error -i "$source_copy" -vf scale=1080:2340:flags=lanczos -pix_fmt rgb24 -compression_level 6 "$phone_png"
```

The recorded evidence used FFmpeg `n9.0.1`. Re-running that command with the
three source assets and this build produced byte-identical phone PNGs. This is
run evidence, not a portable promise: another scaler or build may change RGB
samples. A different lossless PNG encoding of the same raster may instead
change filters, compressed bytes, PNG size, SHA-256, or ancillary chunks while
preserving every RGB8 sample.

ARC's baseline lossless guarantee requires exact dimensions and decoded RGB8
channel values. Separately, the published per-codeword ECC bound covers
controlled payload-byte errors such as the measured fixture; it does not cover
arbitrary raster edits. A lossless re-encode is acceptable when the dimensions
and decoded RGB8 samples compare exactly. JPEG, resize, screenshot,
print/camera, color correction, and social-media recompression are unsupported.
