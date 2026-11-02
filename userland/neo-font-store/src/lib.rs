// SPDX-License-Identifier: MPL-2.0
//! The fonts a LogansOS content process has (OT-61 L1).
//!
//! Each font is compiled into the binary and named by a `path` under
//! [`PREFIX`], which no file system resolves: `servo-fonts-traits`'
//! `LocalFontIdentifier` carries that name, and its `font_data_and_index`
//! asks [`bytes`] for the font. The font files themselves are recorded
//! (licence, upstream, commit, SHA-256) in `third_party/ot61-swgl/VENDORED.md`.

#![no_std]

/// The prefix of every bundled font's name.
pub const PREFIX: &str = "neo-font:/";

/// A bundled font file.
#[derive(Debug)]
pub struct BundledFont {
    /// Its family name, as CSS `font-family` names it.
    pub family: &'static str,
    /// The name `LocalFontIdentifier::path` carries for it.
    pub path: &'static str,
    /// Its CSS weight. A variable font is listed at its default instance.
    pub weight: f32,
    /// Whether it is an italic face.
    pub italic: bool,
    /// The font file.
    pub bytes: &'static [u8],
}

/// Roboto Slab, variable (`wght` 100-900), Apache-2.0: `third_party/ot61-swgl/fonts/robotoslab`.
const ROBOTO_SLAB: &[u8] =
    include_bytes!("../../../third_party/ot61-swgl/fonts/robotoslab/RobotoSlab-wght.ttf");

static FONTS: &[BundledFont] = &[BundledFont {
    family: "Roboto Slab",
    path: "neo-font:/robotoslab/RobotoSlab-wght.ttf",
    weight: 400.0,
    italic: false,
    bytes: ROBOTO_SLAB,
}];

/// Every bundled font.
pub fn fonts() -> &'static [BundledFont] {
    FONTS
}

/// The font named `path`, if it is bundled.
pub fn bytes(path: &str) -> Option<&'static [u8]> {
    FONTS.iter().find(|f| f.path == path).map(|f| f.bytes)
}

/// The family every CSS generic family (`serif`, `sans-serif`, `monospace`,
/// ...) resolves to: the one bundled family, until there are more.
pub const DEFAULT_FAMILY: &str = "Roboto Slab";

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn every_font_is_found_by_its_path_and_is_a_truetype_file() {
        for f in fonts() {
            assert!(f.path.starts_with(PREFIX));
            let b = bytes(f.path).expect("listed font is found");
            // sfnt version 0x00010000 (TrueType outlines).
            assert_eq!(&b[..4], &[0x00, 0x01, 0x00, 0x00]);
        }
        assert!(bytes("neo-font:/missing.ttf").is_none());
    }
}
