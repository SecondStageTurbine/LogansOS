// SPDX-License-Identifier: MPL-2.0
//! F11 S5: the Keyboard page (setup-menu THINK §3.2, §5 S5).
//!
//! With `keyboard-layouts` the page lists the layouts (`keymap_layout_logic.rs`'s
//! names), marks the chosen one `[x]` and has a test field: the WM decodes
//! each key in the layout chosen on the page and hands it in as that layout
//! types it, any legend or a dead key ([`Input::Layout`](crate::Input)); the
//! page composes dead keys ([`Composer`]), so the user sees what the keys
//! type, `ñ` and `á` included, before leaving the screen. The field is the
//! only text in the session that is not ASCII yet: Home and Arthur take the
//! ASCII part of the layout. Choosing a layout clears the field (its text
//! was typed in the other layout). Without the feature the page is S1's
//! stub, shown only with `stub_pages`.

use crate::{Outcome, Page, Wizard};
use alloc::string::String;
use alloc::vec::Vec;
use i18n_core::Msg;

#[cfg(not(feature = "keyboard-layouts"))]
pub use off::*;
#[cfg(feature = "keyboard-layouts")]
pub use on::*;

#[cfg(not(feature = "keyboard-layouts"))]
mod off {
    use super::*;

    /// The page is S1's stub: shown only with `stub_pages`.
    pub const SHOWN: bool = false;

    pub fn options(w: &Wizard, o: &mut Vec<String>) {
        o.push(w.t(Msg::SetupContinue));
    }

    pub fn body(w: &Wizard, b: &mut Vec<String>) {
        b.push(w.t(Msg::SetupBodyKeyboard));
    }

    pub fn summary(_w: &Wizard, _s: &mut Vec<String>) {}

    pub fn choose(w: &mut Wizard, _focus: usize) -> Outcome {
        w.show(w.next(Page::Keyboard))
    }
}

#[cfg(feature = "keyboard-layouts")]
mod on {
    use super::*;
    use crate::mark;
    use alloc::format;

    #[allow(dead_code)]
    mod keymap {
        include!("../../../kernel/src/drivers/keymap_us_logic.rs");
        include!("../../../kernel/src/drivers/keymap_layout_logic.rs");
    }

    pub use keymap::{Composer, Dead, LAYOUT_COUNT, LAYOUT_NAMES, Typed};

    /// The layouts' names in the catalog, in `LAYOUT_NAMES`' order (a KAT
    /// holds the English entries equal to `LAYOUT_NAMES`).
    pub const LAYOUT_MSGS: [Msg; LAYOUT_COUNT as usize] =
        [Msg::LayoutEnglishUs, Msg::LayoutSpanishSpain];

    /// The page is shown.
    pub const SHOWN: bool = true;

    /// The test field's length: what fits the card's row after its label.
    pub const FIELD_MAX: usize = 24;

    /// The test field: printable text (any legend, dead keys composed),
    /// bounded in characters.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Field {
        text: String,
        composer: Composer,
    }

    impl Field {
        pub fn as_str(&self) -> &str {
            &self.text
        }
    }

    /// One row per layout, then Continue.
    pub fn options(w: &Wizard, o: &mut Vec<String>) {
        for (i, &name) in LAYOUT_MSGS.iter().enumerate() {
            o.push(format!(
                "{} {}",
                mark(i == w.choices.layout as usize),
                w.t(name)
            ));
        }
        o.push(w.t(Msg::SetupContinue));
    }

    pub fn body(w: &Wizard, b: &mut Vec<String>) {
        b.push(w.t(Msg::SetupKeyboardBodyChoose));
        b.push(w.f(Msg::SetupKeyboardTryIt, &[w.field.as_str()]));
    }

    pub fn summary(w: &Wizard, s: &mut Vec<String>) {
        s.push(w.f(
            Msg::SetupSummaryKeyboard,
            &[&w.t(LAYOUT_MSGS[w.choices.layout as usize])],
        ));
    }

    /// Enter on a layout row chooses it; on Continue, the next page.
    pub fn choose(w: &mut Wizard, focus: usize) -> Outcome {
        if focus < LAYOUT_COUNT as usize {
            w.choices.layout = focus as u8;
            w.field = Field::default();
            Outcome::Redraw
        } else {
            w.show(w.next(Page::Keyboard))
        }
    }

    /// A key (`Some`) or Backspace (`None`) for the test field, on the
    /// Keyboard page ([`Wizard::on`] routes them here). A dead key waits for
    /// the next key ([`Composer`]); Backspace drops a waiting dead key, else
    /// the last character. A key that changes nothing (a dead key waiting,
    /// past the field's end, Backspace on an empty field) is
    /// [`Outcome::Nothing`].
    pub fn typed(w: &mut Wizard, key: Option<Typed>) -> Outcome {
        let f = &mut w.field;
        let Some(key) = key else {
            if f.composer.pending().is_some() {
                f.composer.feed(Typed::None);
                return Outcome::Nothing;
            }
            return match f.text.pop() {
                Some(_) => Outcome::Redraw,
                None => Outcome::Nothing,
            };
        };
        let out = f.composer.feed(key);
        let mut changed = false;
        for &c in out.as_slice() {
            if !c.is_control() && f.text.chars().count() < FIELD_MAX {
                f.text.push(c);
                changed = true;
            }
        }
        if changed {
            Outcome::Redraw
        } else {
            Outcome::Nothing
        }
    }

    /// The test field's text (the WM logs it).
    pub fn field(w: &Wizard) -> &str {
        w.field.as_str()
    }
}
