// SPDX-License-Identifier: MPL-2.0
//! WCAG 2.x contrast ratio (relative luminance of sRGB colours).

fn linear(c8: u32) -> f64 {
    let c = c8 as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

/// Relative luminance of an XRGB8888 colour.
pub fn luminance(rgb: u32) -> f64 {
    0.2126 * linear((rgb >> 16) & 0xFF)
        + 0.7152 * linear((rgb >> 8) & 0xFF)
        + 0.0722 * linear(rgb & 0xFF)
}

/// `(L_light + 0.05) / (L_dark + 0.05)`.
pub fn ratio(a: u32, b: u32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}
