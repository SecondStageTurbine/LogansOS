// SPDX-License-Identifier: MPL-2.0
// F11 S2 (`setup-keys`): how many languages and keyboard layouts exist, so
// the `Language` and `KeyboardLayout` range checks accept only real ids
// (setup-menu THINK §2.2). One of each today: English (0) and US (0). The
// catalog (S6) and the layout table (S5) generate these counts when they
// land, as `cargo xtask tzdata` generates `TZ_ZONE_COUNT`.

/// The languages: a `Language` id is below it.
pub const LANGUAGE_COUNT: u32 = 1;
/// The keyboard layouts: a `KeyboardLayout` id is below it. With
/// `keyboard-layouts`, US and Spanish (`keymap_layout_logic.rs`'s `LAYOUT_COUNT`;
/// neo-init asserts the two agree at compile time).
#[cfg(not(feature = "keyboard-layouts"))]
pub const LAYOUT_COUNT: u32 = 1;
#[cfg(feature = "keyboard-layouts")]
pub const LAYOUT_COUNT: u32 = 2;
