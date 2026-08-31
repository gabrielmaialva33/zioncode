# Zion Art algorithm review

Date: 2026-08-30

Status: architectural research and first implementation decision. This document
does not define a stable wire format.

## Research question

How can zioncode move from “bytes hidden in an unrelated cover image” to an
information-to-art system in which the content shapes the artwork, while exact
recovery remains cryptographically verifiable and eventually survives a real
screen-capture channel?

The review used Exa for semantic discovery and the web search path for direct
primary-source verification. Secondary surveys and product claims were not
used to make architecture decisions.

## The capacity/robustness boundary

The literature separates into two regimes that should not be conflated:

1. Robust learned watermarking survives crop, resize, JPEG, display/camera, or
   localized editing, but normally carries tens or hundreds of bits.
2. High-capacity generative steganography reports much larger digital payloads,
   but exact recovery normally assumes the generated PNG or a narrow digital
   perturbation model rather than an unregistered screenshot or camera image.

Primary evidence:

- [HiDDeN](https://openaccess.thecvf.com/content_ECCV_2018/html/Jiren_Zhu_HiDDeN_Hiding_Data_ECCV_2018_paper.html)
  established end-to-end encoder/noise/decoder training against crop, dropout,
  blur, and differentiable JPEG approximations.
- [StegaStamp](https://openaccess.thecvf.com/content_CVPR_2020/html/Tancik_StegaStamp_Invisible_Hyperlinks_in_Physical_Photographs_CVPR_2020_paper.html)
  trains through perspective, blur, color, noise, and JPEG transformations. It
  robustly retrieves a 56-bit post-ECC identifier in the demonstrated physical
  pipeline, not a book-sized payload.
- [PIMoG](https://doi.org/10.1145/3503161.3548049) identifies perspective,
  illumination, moiré, and residual Gaussian noise as the important
  differentiable screen-shooting components. Its published code is
  [GPL-3.0](https://github.com/FangHanNUS/PIMoG-An-Effective-Screen-shooting-Noise-Layer-Simulation-for-Deep-Learning-Based-Watermarking-Netw),
  so zioncode must not copy that implementation into its MIT/Apache workspace.
- [Watermark Anything](https://arxiv.org/abs/2411.07231) reframes extraction as
  segmentation and demonstrates localized 32-bit messages. This is relevant to
  crop detection and repeated beacons, not bulk content transport.
- [RoSteALS](https://openaccess.thecvf.com/content/CVPR2023W/WMF/html/Bui_RoSteALS_Robust_Steganography_Using_Autoencoder_Latent_Space_CVPRW_2023_paper.html)
  injects a typically 100-bit secret into a frozen autoencoder latent and can
  synthesize a cover from noise or a text-conditioned diffusion process.
- [Generative Steganography Diffusion](https://arxiv.org/abs/2305.03472) and
  [Diffusion-Stego](https://arxiv.org/abs/2305.18726) demonstrate that secret
  data can participate directly in image generation. Their digital-domain
  reversibility is promising for a research branch but is not evidence of
  book-sized recovery after screen resampling.
- [RaptorQ, RFC 6330](https://datatracker.ietf.org/doc/html/rfc6330) can generate
  an open-ended sequence of repair symbols and reconstruct a source block from
  almost any sufficient subset. It is appropriate across a collection of
  artworks, not as a substitute for per-image symbol correction.

The architecture inference is therefore: use learned robustness for locating
and synchronizing an artifact; do not force the neural watermark to carry the
entire encrypted book.

## Audit of the current algorithms

| Current component | Decision | Reason |
|---|---|---|
| zstd bounded compression | Preserve | Independent from the visual channel and already measured on the corpus. |
| Argon2id, BLAKE3, XChaCha20-Poly1305 | Preserve | The artwork must not become the authentication or secrecy mechanism. |
| RS(255,k) plus column interleaving | Preserve as inner FEC | It repairs localized byte damage before AEAD verification. |
| ARC v1 keyed LSB matching | Freeze for compatibility | Excellent exact-PNG density; structurally incapable of surviving resampling. |
| Screen lab 2×2 differential cells | Keep only as a measurement baseline | It proves full-resolution capacity under exact geometry but has no registration, scale, or crop recovery. |
| Repeated CRC header | Replace with a robust beacon | Repetition helps erasures but does not solve geometric synchronization. |
| One independent book per image | Keep as a simple profile | It is understandable and bounded, but not the only collection topology. |
| Fountain coding across images | Research next | RaptorQ-like repair can make a collection order-independent and tolerant of missing artworks. |
| End-to-end neural bulk payload | Reject as the sole channel | Current robust physical methods do not demonstrate the required payload scale. |
| Diffusion inversion as the stable decoder | Research-only | Model weight, scheduler, quantization, and inversion drift are too large a compatibility surface today. |

## Visual grammar candidates

The first value-noise renderer is a deterministic baseline, not the visual
destination. Primary research points to a stronger grammar assembled from
small, versionable algorithms:

- [Differentiable programming of reaction-diffusion patterns](https://doi.org/10.1162/isal_a_00429)
  shows that emergent textures can be optimized toward target structures while
  retaining an explicit dynamical system. A Gray-Scott-style field is a good
  candidate for content-derived growth, density, and tension.
- [Dendry](https://doi.org/10.1145/3306131.3317020) demonstrates differential
  growth as an interactive procedural system. Its branching and contour
  language is a better candidate for paragraph and narrative structure than
  undifferentiated noise.
- [Anisotropic noise](https://doi.org/10.1145/1360612.1360653) and
  [improved Gabor noise](https://doi.org/10.1109/TVCG.2010.238) provide
  controllable orientation and spectral energy. Those properties are useful
  both artistically and for keeping visual texture away from frequencies later
  reserved for a robust transport channel.
- [CLIPDraw](https://arxiv.org/abs/2106.14843) shows that semantic guidance can
  optimize a compact set of vector strokes. Zion can use this idea in an
  optional semantic frontend, but must store only the resulting bounded genome
  rather than make a changing model part of the decoder contract.

The v0.1 visual experiment should therefore combine a bounded
reaction-diffusion field, divergence-free flow strokes, and differential-growth
contours. Semantic axes select parameters; content structure selects stable
initial conditions and composition. Gabor-like spectral masks remain a
separate candidate for the registered data channel. This preserves exact
determinism while giving the content a more legible visual syntax than generic
procedural noise.

## Selected architecture

The project will use four independently testable layers:

```text
meaning profile + content structure
            │
            ▼
deterministic macro artwork       exact compressed/encrypted bytes
            │                                  │
            ├──────── visual composition ──────┤
            │                                  │
     robust localized beacon           high-capacity channel
       (identity + geometry)          (registered block symbols)
            │                                  │
            └──────── screenshot/capture ──────┘
                               │
                         authenticated text
```

### 1. Macro artwork

The content hash, structural cadence, and a model-independent semantic profile
determine a versioned visual genome. The renderer is integer-only and produces
the same RGB raster for the same genome. A semantic model is an optional
compiler frontend, not part of decoding exact bytes.

### 2. Robust beacon

A small repeated/localized signal will carry only an artifact identifier,
grammar/channel versions, coarse orientation, and calibration. Its design can
borrow the published PIMoG distortion categories and WAM-style localization
without copying incompatible source code.

### 3. High-capacity registered channel

After registration, payload symbols should move from individual RGB LSBs to
mesoscopic blocks. The first non-neural baseline should compare luminance/chroma
block-transform quantization and differential coefficient modulation against
the existing 2×2 carrier. A learned residual encoder can then optimize the same
channel under an explicit distortion simulator.

### 4. Collection repair

Reed–Solomon remains the inner per-artwork code. A later fountain layer can
distribute authenticated source shards across many artworks so the collection
is recoverable from an unordered sufficient subset.

## Rejected shortcuts

- Semantic classification alone cannot recover exact prose.
- A content hash used as a diffusion seed makes content-derived art, but does
  not make the generated pixels a recoverable encoding of that content.
- A high digital bits-per-pixel result is not a screen-capture result.
- PSNR alone is insufficient; future evaluation needs SSIM/LPIPS, bit and frame
  recovery, false-positive rate, crop/scale envelopes, and real-device trials.
- A single AI checkpoint cannot silently become the format. Model identity,
  preprocessing, quantization, and semantic-axis output must be versioned.

## Implementation consequence

`zion-art` is now a separate crate. It owns only the visual genome and
deterministic renderer. ARC v1 and the screenshot laboratory remain unchanged.
This separation keeps research iteration from mutating existing wire formats.

The next experimental gate is not a larger neural payload. It is two parallel,
separately measured prototypes: the v0.1 reaction-diffusion/flow grammar, and a
robust localized 96–256-bit beacon with automatic geometry recovery on real
A57 screenshots. Only after registration is measured should the project
replace the high-capacity cell modulation.
