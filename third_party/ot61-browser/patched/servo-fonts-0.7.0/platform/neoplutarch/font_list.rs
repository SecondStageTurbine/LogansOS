// SPDX-License-Identifier: MPL-2.0
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The font list for `target_os = "neoplutarch"`: the fonts `neo-font-store`
//! bundles, in place of fontconfig's (shaped like the OHOS fixed list).

use style::Atom;
use style::values::computed::font::GenericFontFamily;
use style::values::computed::{
    FontStyle as StyleFontStyle, FontWeight as StyleFontWeight, FontWidth as StyleFontWidth,
};

use crate::{
    FallbackFontSelectionOptions, FontIdentifier, FontTemplate, FontTemplateDescriptor,
    LocalFontIdentifier, LowercaseFontFamilyName,
};

pub(crate) fn for_each_available_family<F>(mut callback: F)
where
    F: FnMut(String),
{
    let mut seen: Vec<&str> = Vec::new();
    for font in neo_font_store::fonts() {
        if !seen.contains(&font.family) {
            seen.push(font.family);
            callback(font.family.to_owned());
        }
    }
}

pub(crate) fn for_each_variation<F>(family_name: &str, mut callback: F)
where
    F: FnMut(FontTemplate),
{
    for font in neo_font_store::fonts() {
        if !font.family.eq_ignore_ascii_case(family_name) {
            continue;
        }
        let local_font_identifier = LocalFontIdentifier {
            path: Atom::from(font.path),
            face_index: 0,
            named_instance_index: 0,
        };
        let style = if font.italic {
            StyleFontStyle::ITALIC
        } else {
            StyleFontStyle::NORMAL
        };
        let descriptor = FontTemplateDescriptor::new(
            StyleFontWeight::from_float(font.weight),
            StyleFontWidth::NORMAL,
            style,
        );
        callback(FontTemplate::new(
            FontIdentifier::Local(local_font_identifier),
            descriptor,
            None,
        ));
    }
}

/// Every bundled family, whatever the character: there are too few to choose
/// between by Unicode block.
pub fn fallback_font_families(_options: FallbackFontSelectionOptions) -> Vec<&'static str> {
    let mut families: Vec<&'static str> = Vec::new();
    for font in neo_font_store::fonts() {
        if !families.contains(&font.family) {
            families.push(font.family);
        }
    }
    families
}

/// Every CSS generic family resolves to the default bundled family.
pub(crate) fn default_system_generic_font_family(
    _generic: GenericFontFamily,
) -> LowercaseFontFamilyName {
    neo_font_store::DEFAULT_FAMILY.into()
}
