// SPDX-License-Identifier: MPL-2.0
use super::*;

/// The byte oracle, laid out by hand: a 4x4 SVG with a pixel-aligned 2x2
/// Electric Blue square at (1, 1). Every pixel is either fully inside the
/// square (coverage 1: `3c 8e f4 ff`) or fully outside (`00 00 00 00`), so
/// the expected bytes need no knowledge of the rasteriser's anti-aliasing.
#[test]
fn a_pixel_aligned_square_rasterises_to_the_hand_laid_bytes() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4" viewBox="0 0 4 4"><rect x="1" y="1" width="2" height="2" fill="#3c8ef4"/></svg>"##;
    const O: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    const B: [u8; 4] = [0x3c, 0x8e, 0xf4, 0xff];
    let want: Vec<u8> = [
        O, O, O, O, //
        O, B, B, O, //
        O, B, B, O, //
        O, O, O, O, //
    ]
    .concat();
    assert_eq!(rasterize(svg, 4).unwrap(), want);
}

/// Straight alpha, not premultiplied: a pixel-aligned column at half
/// opacity keeps its colour bytes and carries the opacity in alpha alone
/// (0.5 x 255 = 127.5, which tiny-skia rounds to 128).
#[test]
fn blobs_are_straight_alpha() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2" viewBox="0 0 2 2"><rect x="0" y="0" width="1" height="2" fill="#ffffff" fill-opacity="0.5"/></svg>"##;
    const H: [u8; 4] = [0xff, 0xff, 0xff, 0x80];
    const O: [u8; 4] = [0x00, 0x00, 0x00, 0x00];
    assert_eq!(rasterize(svg, 2).unwrap(), [H, O, H, O].concat());
}

/// The pins: each committed blob's BLAKE3. A rasteriser or SVG change that
/// moves a pixel fails here (and in `committed_blobs_are_current`) instead
/// of reaching the Spine silently. Regenerate with `cargo xtask icons`, then
/// update these from `cargo run -p icon-raster -- check`'s output.
const PINS: [(&str, u32, &str); 21] = [
    (
        "arthur",
        40,
        "4567855a288ec03e2974fe8db3154483e81f81d448eeccefcde651d2d7d3b675",
    ),
    (
        "arthur",
        24,
        "a422e2ea83409fe9eaf261d145428d976602990571dd62afeb690f84c2c75f43",
    ),
    (
        "arthur",
        16,
        "2bf7a0b7b14fbb62b132094ae53cb671a847dd23141ab0fd7acf3a9f099f7f06",
    ),
    (
        "lens",
        40,
        "885c81c5a9b03a063da3172c215f8243c27b863773ca91449f218671e5335029",
    ),
    (
        "lens",
        24,
        "cfc2b9e49d54b40171cd31089af0447315d9f2cc474c6f36d570892935689181",
    ),
    (
        "lens",
        16,
        "6397007387f07446a6f5f3899c25e866ae2bbcc501890c7a58c5bc2b1872830d",
    ),
    (
        "terminal",
        40,
        "5de89425996c2c00fced43a5bd3585782af10ad736e178cc50c44d10c93ad82f",
    ),
    (
        "terminal",
        24,
        "967f52da69832230e2ab692c39c6adcdcfe2b3f24e7821248ca665e37c0cc611",
    ),
    (
        "terminal",
        16,
        "cc8a76944f6448441c3091382c8931ed21397ba4b5f11866deda92a7a43594de",
    ),
    (
        "files",
        40,
        "fae313568873f8057bc3d29247b627fb51de1d05d7c9298b34f299366b4200ec",
    ),
    (
        "files",
        24,
        "17e659f8372a353b1f01ef8873207926eb844dd8d81d81db0746ff863ab3ca87",
    ),
    (
        "files",
        16,
        "4620b2c10ccb3b768e9dbc3c0deb707a286c0d4578ee9dc3b2cf30bdcdd84ded",
    ),
    (
        "settings",
        40,
        "85a3668c4d7d347718b02ad75b538caf4b37ec2a74de15c6772c51846118aa6c",
    ),
    (
        "settings",
        24,
        "3d80f6aede924e206f4c688f27abfb157cb330f5649b806e7c615ce89b31451c",
    ),
    (
        "settings",
        16,
        "f24f7c28d8f4b6c6707e94c4ab29d70f551e264e1abe0091173bdf9f8b86f438",
    ),
    (
        "install",
        40,
        "67752d9e244bf944ec88043589a4428bc602e5205c65882a59ac5c45e3fd8527",
    ),
    (
        "install",
        24,
        "694f2904ba0451b98517ba7b52cbcb35a119fb692443c6132aeed8d596ed84cb",
    ),
    (
        "install",
        16,
        "969745e14efdc2669c3ec5ec3a9c19fd385a9bf98e8b702fba24ea28be44b3b4",
    ),
    (
        "browser",
        40,
        "f2a9b869d8e4cd4cb3d87483ba5359f58561ba8c3e1d7eae7094bbf1fe26052a",
    ),
    (
        "browser",
        24,
        "78ed8cc5cb07e2776ce2a52fd8ae3a49a5ad9ab24f0550f1fd1657d4a66bd069",
    ),
    (
        "browser",
        16,
        "82d5188dfb8192764c25903cfbf6fbcbf1298c9ae79da09ddb28cb8fc48645a3",
    ),
];

#[test]
fn committed_blobs_match_their_pins() {
    for (name, size, pin) in PINS {
        let blob = std::fs::read(blob_path(name, size)).expect("committed blob");
        assert_eq!(blob.len(), (size * size * 4) as usize, "{name}-{size}");
        assert_eq!(blake3_hex(&blob), pin, "{name}-{size}");
    }
}

/// The committed blobs are what this rasteriser makes from the committed
/// SVGs today (`cargo xtask icons` regenerates them).
#[test]
fn committed_blobs_are_current() {
    for name in NAMES {
        for size in SIZES {
            let svg = std::fs::read(svg_path(name, size)).expect("svg");
            let blob = std::fs::read(blob_path(name, size)).expect("blob");
            assert!(
                rasterize(&svg, size).unwrap() == blob,
                "{name}-{size} is stale"
            );
        }
    }
}

/// The 2 px safe margin of every 40 px master is fully transparent, so the
/// Spine chip may place the icon 2 px above its label box and clip to the
/// box without cutting any ink.
#[test]
fn every_master_keeps_its_two_pixel_margin_clear() {
    for name in NAMES {
        let blob = std::fs::read(blob_path(name, 40)).expect("blob");
        for y in 0..40usize {
            for x in 0..40usize {
                if (2..38).contains(&x) && (2..38).contains(&y) {
                    continue;
                }
                assert_eq!(blob[(y * 40 + x) * 4 + 3], 0, "{name} ink at ({x},{y})");
            }
        }
    }
}
