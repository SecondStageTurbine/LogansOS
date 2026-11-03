// SPDX-License-Identifier: MPL-2.0
//! F11 S1: the first-boot setup menu's page machine, with no I/O
//! (`docs/THINK-2026-11-02-setup-menu.md` §2.1, §4.2, §4.5, §5 S1).
//!
//! The WM's view (`neo-init/src/desktop_session/setup.rs`) only draws the
//! rows this crate gives it, turns keys into [`Input`]s and logs. Every
//! decision is here, so the host KATs run the whole walk without QEMU:
//!
//! - the pages this build shows, in order ([`Wizard::pages`]): Welcome, then
//!   (behind `stub_pages`) Language and Keyboard, then Time zone, Comfort
//!   and Try. There is no Network page until S4 (§3.3);
//! - the key map: Up/Down and Tab move, Enter or Space chooses, Esc goes
//!   back (on Welcome it asks before skipping, §4.2);
//! - the rows: a title (with `N of M`, counting the pages after Welcome),
//!   a body, the options and a hint, never more than [`MAX_ROWS`]. The
//!   focused option is marked `>` and a chosen one `[x]`, so no state is
//!   colour-only (§4.3);
//! - the choices, held until the wizard ends. S1 writes nothing: the store
//!   (S2), the clock confirm (S3) and the Install action (S7) come later.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The card's row budget (`neo-atrium` `CARD_MAX_ROWS`).
pub const MAX_ROWS: usize = 8;

/// The Time zone page's short list: a pointer alone can pick from it
/// (§4.2). Typing a name in the field is S3's.
pub const ZONES: [&str; 4] = [
    "UTC",
    "America/New_York",
    "Europe/London",
    "Australia/Sydney",
];

/// The last row of every page.
pub const HINT: &str = "Up/Down to move, Enter to choose, Esc to go back.";

/// A page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Welcome,
    /// "Skip setup and use the defaults?" (Esc on Welcome).
    SkipConfirm,
    /// A stub until a second language exists (§3.1).
    Language,
    /// A stub until a second layout exists (§3.2).
    Keyboard,
    TimeZone,
    Comfort,
    /// The summary and "Try LogansOS" (Install is S7's).
    Try,
}

impl Page {
    /// The name the log uses (`DESKTOP setup page=<name>`).
    pub const fn name(self) -> &'static str {
        match self {
            Page::Welcome => "welcome",
            Page::SkipConfirm => "skip",
            Page::Language => "language",
            Page::Keyboard => "keyboard",
            Page::TimeZone => "timezone",
            Page::Comfort => "comfort",
            Page::Try => "try",
        }
    }
}

/// What a key does here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Up,
    /// Down, and Tab.
    Down,
    /// Enter, and Space.
    Choose,
    /// Esc.
    Back,
}

/// How the wizard ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    /// Try LogansOS: the session starts with the choices.
    Try,
    /// Skip setup: the defaults.
    Skipped,
}

/// What an input did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The focus moved, or a choice changed: redraw this page.
    Redraw,
    /// A new page is shown.
    Page(Page),
    /// The wizard is over.
    Done(Finish),
}

/// What the user chose. The defaults are the store's (`settings-core`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    /// An index into [`ZONES`].
    pub zone: usize,
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
}

/// The wizard: the page shown, the focused option, the choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wizard {
    stub_pages: bool,
    page: Page,
    focus: usize,
    choices: Choices,
}

impl Wizard {
    /// A wizard on Welcome. `stub_pages` shows the Language and Keyboard
    /// stubs (the `setup-stub-pages` build).
    pub fn new(stub_pages: bool) -> Self {
        Self {
            stub_pages,
            page: Page::Welcome,
            focus: 0,
            choices: Choices::default(),
        }
    }

    pub fn page(&self) -> Page {
        self.page
    }

    pub fn focus(&self) -> usize {
        self.focus
    }

    pub fn choices(&self) -> Choices {
        self.choices
    }

    /// The pages this build shows, in order (Skip's confirmation is not one).
    pub fn pages(&self) -> Vec<Page> {
        let mut pages = Vec::new();
        pages.push(Page::Welcome);
        if self.stub_pages {
            pages.push(Page::Language);
            pages.push(Page::Keyboard);
        }
        pages.push(Page::TimeZone);
        pages.push(Page::Comfort);
        pages.push(Page::Try);
        pages
    }

    /// The options of the page shown, as their labels (without marks).
    pub fn options(&self) -> Vec<String> {
        let mut o = Vec::new();
        match self.page {
            Page::Welcome => {
                o.push(String::from("Begin"));
                o.push(String::from("Skip setup"));
            }
            Page::SkipConfirm => {
                o.push(String::from("Skip setup and use the defaults"));
                o.push(String::from("Back"));
            }
            Page::Language | Page::Keyboard => o.push(String::from("Continue")),
            Page::TimeZone => {
                for (i, z) in ZONES.iter().enumerate() {
                    o.push(format!("{} {}", mark(i == self.choices.zone), z));
                }
                o.push(String::from("Continue"));
            }
            Page::Comfort => {
                o.push(format!(
                    "{} Reduce motion",
                    mark(self.choices.reduce_motion)
                ));
                o.push(format!(
                    "{} Reduce transparency",
                    mark(self.choices.reduce_transparency)
                ));
                o.push(String::from("Continue"));
            }
            Page::Try => o.push(String::from("Try LogansOS")),
        }
        o
    }

    /// The card's rows for the page shown: title, body, options (the
    /// focused one marked `>`), hint.
    pub fn rows(&self) -> Vec<String> {
        let mut rows = Vec::new();
        rows.push(self.title());
        for line in self.body() {
            rows.push(line);
        }
        for (i, label) in self.options().into_iter().enumerate() {
            let focus = if i == self.focus { ">" } else { " " };
            rows.push(format!("{focus} {label}"));
        }
        rows.push(String::from(HINT));
        rows
    }

    fn title(&self) -> String {
        let name = match self.page {
            Page::Welcome => return String::from("Set up LogansOS"),
            Page::SkipConfirm => return String::from("Skip setup?"),
            Page::Language => "Language",
            Page::Keyboard => "Keyboard",
            Page::TimeZone => "Time zone",
            Page::Comfort => "Comfort",
            Page::Try => "Try LogansOS",
        };
        let pages = self.pages();
        let total = pages.len() - 1;
        let n = pages.iter().position(|&p| p == self.page).unwrap_or(0);
        format!("{name} ({n} of {total})")
    }

    fn body(&self) -> Vec<String> {
        let mut b = Vec::new();
        match self.page {
            Page::Welcome => {
                b.push(String::from(
                    "A few short screens. You can change any of these later.",
                ));
            }
            Page::SkipConfirm => {
                b.push(String::from(
                    "The defaults are UTC, with full motion and transparency.",
                ));
            }
            Page::Language => b.push(String::from("English is the only language for now.")),
            Page::Keyboard => b.push(String::from("US is the only keyboard layout for now.")),
            Page::TimeZone => b.push(String::from("Where are you? At Home: timezone Area/City.")),
            Page::Comfort => b.push(String::from("Make the screen calmer, if you like.")),
            Page::Try => {
                for line in self.summary() {
                    b.push(line);
                }
            }
        }
        b
    }

    /// The Try page's summary: what the session will start with.
    pub fn summary(&self) -> Vec<String> {
        let mut s = Vec::new();
        s.push(format!("Time zone: {}", ZONES[self.choices.zone]));
        s.push(format!(
            "Reduce motion: {}. Reduce transparency: {}.",
            on_off(self.choices.reduce_motion),
            on_off(self.choices.reduce_transparency)
        ));
        s
    }

    /// Apply one input.
    pub fn on(&mut self, input: Input) -> Outcome {
        let count = self.options().len();
        match input {
            Input::Up => {
                self.focus = if self.focus == 0 {
                    count - 1
                } else {
                    self.focus - 1
                };
                Outcome::Redraw
            }
            Input::Down => {
                self.focus = (self.focus + 1) % count;
                Outcome::Redraw
            }
            Input::Back => match self.page {
                Page::Welcome => self.show(Page::SkipConfirm),
                Page::SkipConfirm => self.show(Page::Welcome),
                page => self.show(self.previous(page)),
            },
            Input::Choose => self.choose(),
        }
    }

    fn choose(&mut self) -> Outcome {
        match (self.page, self.focus) {
            (Page::Welcome, 0) => self.show(self.next(Page::Welcome)),
            (Page::Welcome, _) => self.show(Page::SkipConfirm),
            (Page::SkipConfirm, 0) => Outcome::Done(Finish::Skipped),
            (Page::SkipConfirm, _) => self.show(Page::Welcome),
            (Page::Language | Page::Keyboard, _) => self.show(self.next(self.page)),
            (Page::TimeZone, i) if i < ZONES.len() => {
                self.choices.zone = i;
                Outcome::Redraw
            }
            (Page::TimeZone, _) => self.show(self.next(Page::TimeZone)),
            (Page::Comfort, 0) => {
                self.choices.reduce_motion = !self.choices.reduce_motion;
                Outcome::Redraw
            }
            (Page::Comfort, 1) => {
                self.choices.reduce_transparency = !self.choices.reduce_transparency;
                Outcome::Redraw
            }
            (Page::Comfort, _) => self.show(self.next(Page::Comfort)),
            (Page::Try, _) => Outcome::Done(Finish::Try),
        }
    }

    fn show(&mut self, page: Page) -> Outcome {
        self.page = page;
        self.focus = 0;
        Outcome::Page(page)
    }

    fn next(&self, page: Page) -> Page {
        let pages = self.pages();
        let i = pages.iter().position(|&p| p == page).unwrap_or(0);
        pages.get(i + 1).copied().unwrap_or(Page::Try)
    }

    fn previous(&self, page: Page) -> Page {
        let pages = self.pages();
        let i = pages.iter().position(|&p| p == page).unwrap_or(0);
        pages[i.saturating_sub(1)]
    }
}

fn mark(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

#[cfg(test)]
mod tests;
