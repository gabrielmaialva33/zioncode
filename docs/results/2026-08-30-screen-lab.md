# ARC Screen laboratory result

Date: 2026-08-30

Status: host prototype passed. This is not an ARC v1 extension and is not yet
an Android feature or a real-device claim.

## Objective

Test whether one lossless, exact-geometry Android screenshot can carry the
measured Safe-profile Psalms frame while preserving the existing outer
Reed–Solomon repair layer. ARC v1 remains unchanged and continues to reject any
claim of screenshot survival.

## Experimental carrier

The `zion_arc::screen_lab` module divides an RGB8 raster into 2×2 cells. Each
cell carries two differential bits:

- the red-channel left/right group difference carries one bit;
- the blue-channel top/bottom group difference carries one bit;
- a minimum signed difference is forced while preserving existing within-group
  texture where possible; and
- green is not used by this laboratory carrier.

The 16-byte `ZSCP` laboratory header contains a version, cell geometry, header
repeat count, payload length, and CRC32C. Nine complete header copies are spread
through the raster instead of being adjacent. Majority recovery passed after
two complete copies were deliberately overwritten.

The framing is intentionally marked unstable. It does not reinterpret the
normative `ZARC` bootstrap or alter ARC v1's frozen density and placement rules.

## A57 capacity

For the 1080×2340 reference raster:

```text
2x2 cells                 = 540 * 1170 = 631,800
repeated-header cells     = 16 * 4 * 9 = 576
payload cells             = 631,224
two-bit payload capacity  = 631,224 / 4 = 157,806 bytes
measured Psalms frame     = 64 + 556 * 255 = 141,844 bytes
remaining laboratory room= 15,962 bytes
```

The 64-byte prefix in the current test is a synthetic stand-in for the ARC
bootstrap. The following 141,780 bytes are a real column-major Safe-profile
RS interleaving produced by the existing ARC ECC implementation from 556
codewords.

## Measured host test

The full-size test measured:

```text
payload bytes      = 141,844
capacity bytes     = 157,806
changed channels   = 4,511,636 of 7,581,600
embedding PSNR     = 33.505 dB on the deterministic synthetic textured cover
```

The test then applied a deterministic compositor model with 2% channel gain,
per-channel offsets of `+2/-1/+3`, and per-sample noise in `[-1,+1]`. It encoded
the transformed raster as a lossless RGB PNG, decoded it through the public
screen-lab API, and deliberately inverted eight additional payload symbols.
The extracted interleaving differed, but the existing Safe-profile
Reed–Solomon decoder repaired it and reproduced the original synthetic
ciphertext exactly.

The public PNG API also passed a separate opaque 4,096-byte roundtrip. The
laboratory test set and strict lint gate pass with:

```bash
cargo test -p zion-arc screen_lab -- --nocapture
cargo clippy -p zion-arc --all-targets --all-features -- -D warnings
```

## Visual before/after artifact

The project includes a full-size phone-art comparison generated through the
public laboratory API:

- [`screen-lab-before.png`](assets/screen-lab-before.png): 3,653,826-byte
  clean 1080×2340 RGB8 cover;
- [`screen-lab-after.png`](assets/screen-lab-after.png): 4,382,678-byte
  encoded 1080×2340 RGB8 cover carrying 141,844 deterministic opaque bytes.

On this continuous-tone artwork the embedding measured 35.161 dB PSNR, changed
4,178,875 of 7,581,600 channels, retained the same dimensions, and decoded the
payload exactly. The example can reproduce the second artifact with:

```bash
cargo run -p zion-arc --example screen_lab_demo -- \
  docs/results/assets/screen-lab-before.png \
  docs/results/assets/screen-lab-after.png \
  141844
```

These images demonstrate the carrier appearance and exact-raster roundtrip.
They are not evidence of recovery from a physical A57 screenshot.

## What this proves

- The measured Psalms RS frame fits in a full 1080×2340 2×2 differential
  carrier.
- A monotonic, exact-geometry digital compositor transform need not destroy the
  two-bit cell decisions.
- Existing outer Reed–Solomon repair remains useful after screen-symbol errors.
- Header replication can survive localized copy destruction when copies are
  spatially distributed.

## What remains unproven

- Real Galaxy A57 screenshot capture and OEM color-management behavior.
- Finder/registration, crop recovery, subpixel translation, resize, rotation,
  or aspect-ratio change.
- Status/navigation-bar overlays or capture from an arbitrary image viewer.
- JPEG, messaging-app recompression, camera photos, paper printing, or social
  media.
- Integration of the real authenticated ARC bootstrap, encryption pipeline,
  Android JNI, full-screen renderer, and mobile reader.
- Acceptable visual quality on the three licensed demo artworks. The measured
  33.505 dB is a laboratory result, not a perceptual-quality guarantee.

## Next engineering gate

The next gate is a registration layer and deterministic render contract:

1. render the encoded art edge-to-edge at one physical pixel per source pixel;
2. add distributed calibration/finder structure that does not resemble a QR
   payload grid;
3. recover translation and bounded crop before reading 2×2 symbols;
4. integrate the real ARC bootstrap plus interleaved frame; and
5. capture and open an actual screenshot on the Galaxy A57.

Until that device gate passes, product language must say “experimental
exact-geometry screenshot profile,” not “works with screenshots” without a
qualifier.
