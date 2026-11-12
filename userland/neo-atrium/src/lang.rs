// SPDX-License-Identifier: MPL-2.0
//! F11 S6b: the session's language. The WM sets it from the settings store's
//! `Language` (neo-init's `i18n-catalog`); the intro, help, power and clock
//! cards and Home's hint read it. Never set, it is English, so a build
//! without `i18n-catalog` draws what it drew before S6b.
//!
//! Each card's words also have a `_in(lang, ..)` form, pure, which the host
//! KATs use; the forms without `_in` take the session's language.

use core::sync::atomic::{AtomicU8, Ordering};
pub use i18n_core::Lang;

static LANG: AtomicU8 = AtomicU8::new(0);

/// Set the session's language.
pub fn set(lang: Lang) {
    let i = Lang::ALL.iter().position(|&l| l == lang).unwrap_or(0);
    LANG.store(i as u8, Ordering::Relaxed);
}

/// The session's language.
pub fn get() -> Lang {
    Lang::from_index(usize::from(LANG.load(Ordering::Relaxed))).unwrap_or_default()
}
