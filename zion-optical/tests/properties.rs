//! Property-based tests for the optical render/extract roundtrip.

use proptest::prelude::*;
use zion_optical::{ImageShape, RenderParams, extract, render};

proptest! {
    // zstd-22 + codec-A roundtrip is a few hundred ms per case at 1 MB.
    // Keep cases small so the suite finishes in <1min.
    #![proptest_config(ProptestConfig::with_cases(8))]

    /// Roundtrip preserves the exact bytes for any non-empty payload up to ~64 KiB.
    #[test]
    fn roundtrip_preserves_bytes(
        payload in prop::collection::vec(any::<u8>(), 1..65_536),
    ) {
        let out = render(&payload, &RenderParams::default()).unwrap();
        let rec = extract(&out.png_bytes).unwrap();
        prop_assert_eq!(rec.file_bytes, payload);
    }

    /// Image dimensions are always Square by default.
    #[test]
    fn square_default_dimensions_are_equal(
        payload in prop::collection::vec(any::<u8>(), 1..16_384),
    ) {
        let out = render(&payload, &RenderParams::default()).unwrap();
        prop_assert_eq!(out.width, out.height);
    }

    /// Custom WidthHeight that fits the payload always succeeds and preserves dimensions.
    #[test]
    fn explicit_shape_with_enough_capacity_works(
        payload in prop::collection::vec(any::<u8>(), 1..8_192),
        side in 200u32..400u32,
    ) {
        let params = RenderParams::default()
            .with_k(38)
            .unwrap()
            .with_shape(ImageShape::WidthHeight { width: side, height: side });
        let out = render(&payload, &params).unwrap();
        let rec = extract(&out.png_bytes).unwrap();

        prop_assert_eq!(out.width, side);
        prop_assert_eq!(out.height, side);
        prop_assert_eq!(rec.file_bytes, payload);
    }
}
