// SPDX-License-Identifier: MPL-2.0
//! F11 S5: the Keyboard page (setup-menu THINK §3.2, §5 S5).
//!
//! With `keyboard-layouts` the page lists the layouts (`keymap_layout_logic.rs`'s
//! names), marks the chosen one `[x]` and has a test field: the WM decodes
//! each key in the layout chosen on the page and hands the character in
//! ([`Input::Char`](crate::Input)), so the user sees what the keys type
//! before leaving the screen. Choosing a layout clears the field (its text
//! was typed in the other layout). Without the feature the page is S1's
//! stub, shown only with `stub_pages`.

use crate::{Outcome, Page, Wizard};
use alloc::string::String;
use alloc::vec::Vec;

#[cfg(not(feature = "keyboard-layouts"))]
pub use off::*;
#[cfg(feature = "keyboard-layouts")]
pub use on::*;

#[cfg(not(feature = "keyboard-layouts"))]
mod off {
    use super::*;

    /// The page is S1's stub: shown only with `stub_pages`.
    pub const SHOWN: bool = false;

    pub fn options(_w: &Wizard, o: &mut Vec<String>) {
        o.push(String::from("Continue"));
    }

    pub fn body(_w: &Wizard, b: &mut Vec<String>) {
        b.push(String::from("US is the only keyboard layout for now."));
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

    pub use keymap::{LAYOUT_COUNT, LAYOUT_NAMES};

    /// The page is shown.
    pub const SHOWN: bool = true;

    /// The test field's length: what fits the card's row after its label.
    pub const FIELD_MAX: usize = 24;

    /// The test field: printable ASCII, bounded.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Field {
        text: String,
    }

    impl Field {
        pub fn as_str(&self) -> &str {
            &self.text
        }
    }

    /// One row per layout, then Continue.
    pub fn options(w: &Wizard, o: &mut Vec<String>) {
        for (i, name) in LAYOUT_NAMES.iter().enumerate() {
            o.push(format!("{} {}", mark(i == w.choices.layout as usize), name));
        }
        o.push(String::from("Continue"));
    }

    pub fn body(w: &Wizard, b: &mut Vec<String>) {
        b.push(String::from("Choose the layout printed on your keys."));
        b.push(format!("Try it: {}_", w.field.as_str()));
    }

    pub fn summary(w: &Wizard, s: &mut Vec<String>) {
        s.push(format!(
            "Keyboard: {}",
            LAYOUT_NAMES[w.choices.layout as usize]
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

    /// A character (`Some`) or Backspace (`None`) for the test field. Off the
    /// Keyboard page, and past the field's end, nothing changes.
    pub fn typed(w: &mut Wizard, c: Option<u8>) -> Outcome {
        if w.page == Page::Keyboard {
            match c {
                Some(c) if (0x20..=0x7e).contains(&c) && w.field.text.len() < FIELD_MAX => {
                    w.field.text.push(c as char)
                }
                None => {
                    w.field.text.pop();
                }
                _ => {}
            }
        }
        Outcome::Redraw
    }

    /// The test field's text (the WM logs it).
    pub fn field(w: &Wizard) -> &str {
        w.field.as_str()
    }
}
