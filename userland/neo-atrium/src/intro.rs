// SPDX-License-Identifier: MPL-2.0
//! Arthur's first-boot introduction and the help card (feature
//! `desktop-intro` in the WM): the words, which variant shows, and when.
//!
//! Wording authority: `Plans/NOTE-2026-10-20-arthur-first-boot-intro-s2.md`
//! as the PL decided it (variant A with the name; the privacy line as a
//! description; "Good day", the clock being ticks since boot; the USB-only
//! line; a deterministic help card the WM answers, never the model). Register:
//! `docs/VISION-arthur-persona.md` (no exclamations or emoji, economy, the
//! exit always offered). Every line is checked by the persona KAT in
//! `tests.rs`: every character in the face, no `!` (nor `¡`), the length
//! bounds, each row inside the card, and the model-state sentences exactly
//! B1b's.
//!
//! **S6b:** the words are `i18n-core`'s catalog; the constants below are its
//! English entries. Each function has an `_in(lang, ..)` form; the plain form
//! takes the session's language ([`crate::lang`]).
//!
//! Pure: the WM decides the variant and draws the rows with
//! [`crate::BaseScene::set_card`]; nothing here touches the system.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use i18n_core::{Lang, Msg, text};

const fn en(m: Msg) -> &'static str {
    text(Lang::En, m)
}

/// Line 1 (variant A): Bible 29.1's name, read as part of the greeting.
pub const GREETING: &str = en(Msg::IntroGreeting);
/// `desktop-clock` (OT-85): with a wall clock the greeting moves to its own
/// row with the time ([`crate::clock::greeting_row`]), and the name follows
/// as this row. (One row would be 93 bytes, past the 90 the variant-A
/// greeting is allowed; split, both rows are within the 80 of new rows.)
pub const NAME: &str = en(Msg::IntroName);
/// Line 2: the household side named as what Arthur is for, not promised.
pub const HOUSEHOLD: &str = en(Msg::IntroHousehold);
/// Line 3 (PL Q2 option b): a description of Arthur's behaviour.
pub const PRIVACY: &str = en(Msg::IntroPrivacy);
/// DECISION D5: on a USB-only machine nothing persists, so the intro repeats.
pub const USB_ONLY: &str = en(Msg::IntroUsbOnly);
pub const ASK: &str = en(Msg::IntroAsk);
pub const LIST: &str = en(Msg::IntroList);
/// The no-model variants: the Lens is the one thing that needs no model.
pub const LIST_STILL: &str = en(Msg::IntroListStill);
/// The exit (persona rule 9): the help card, and Esc back to Home.
pub const EXIT: &str = en(Msg::IntroExit);

/// The help card: only what is true of the 0.1 desktop without the model.
pub const HELP: [&str; 6] = [
    en(Msg::HelpTitle),
    en(Msg::HelpEsc),
    en(Msg::HelpList),
    en(Msg::HelpChips),
    en(Msg::HelpDrag),
    en(Msg::HelpType),
];

/// The help card's ids, in [`HELP`]'s order.
const HELP_MSGS: [Msg; 6] = [
    Msg::HelpTitle,
    Msg::HelpEsc,
    Msg::HelpList,
    Msg::HelpChips,
    Msg::HelpDrag,
    Msg::HelpType,
];

/// S6b: Home's hint in `lang`.
pub const fn hint_in(lang: Lang) -> &'static str {
    text(lang, Msg::IntroHint)
}

/// Home's hint in the session's language.
pub fn hint() -> &'static str {
    hint_in(crate::lang::get())
}

/// The visible affordance on Home while no card shows (typed or clicked).
pub const HINT: &str = en(Msg::IntroHint);

/// The word that opens the help card from the intent field.
pub const HELP_COMMAND: &str = "help";

/// The model's state as the intro words it. The first four are B1b's
/// (`neo-arthur/src/model_state_logic.rs`); `TooLittleMemory` is B2's tier
/// rule (not produced in 0.1; the words are ready for it).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelState {
    Ready,
    Absent,
    Damaged,
    LoadFailed,
    TooLittleMemory { have_gb: u32, need_gb: u32 },
}

/// B1b's beginner sentence for `state`, verbatim, as one card row (each fits
/// the card at its size; the KAT measures them and compares them with B1b,
/// so the card never disagrees with Arthur's later reply). `None` with a
/// model, or for B2's state (its own rows).
pub const fn state_row(state: ModelState) -> Option<&'static str> {
    state_row_in(Lang::En, state)
}

/// S6b: [`state_row`] in `lang`.
pub const fn state_row_in(lang: Lang, state: ModelState) -> Option<&'static str> {
    match state {
        ModelState::Ready | ModelState::TooLittleMemory { .. } => None,
        ModelState::Absent => Some(text(lang, Msg::IntroModelAbsent)),
        ModelState::Damaged => Some(text(lang, Msg::IntroModelDamaged)),
        ModelState::LoadFailed => Some(text(lang, Msg::IntroModelLoadFailed)),
    }
}

/// The introduction's rows for `model`, in the session's language;
/// `usb_only` adds the D5 line.
pub fn intro_lines(model: ModelState, usb_only: bool) -> Vec<String> {
    intro_lines_in(crate::lang::get(), model, usb_only)
}

/// S6b: [`intro_lines`] in `lang`.
pub fn intro_lines_in(lang: Lang, model: ModelState, usb_only: bool) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let t = |m: Msg| text(lang, m);
    let mut push = |s: &str| rows.push(String::from(s));
    push(t(Msg::IntroGreeting));
    push(t(Msg::IntroHousehold));
    match model {
        ModelState::Ready => {
            push(t(Msg::IntroPrivacy));
            if usb_only {
                push(t(Msg::IntroUsbOnly));
            }
            push(t(Msg::IntroAsk));
            push(t(Msg::IntroList));
        }
        ModelState::TooLittleMemory { have_gb, need_gb } => {
            if usb_only {
                push(t(Msg::IntroUsbOnly));
            }
            push(&i18n_core::format(
                lang,
                Msg::IntroTooLittleMemory,
                &[&format!("{have_gb}"), &format!("{need_gb}")],
            ));
            push(t(Msg::IntroCantThink));
        }
        _ => {
            if usb_only {
                push(t(Msg::IntroUsbOnly));
            }
            if let Some(row) = state_row_in(lang, model) {
                push(row);
            }
            push(t(Msg::IntroListStill));
        }
    }
    push(t(Msg::IntroExit));
    rows
}

/// `desktop-clock` (OT-85): the introduction's rows with the wall clock:
/// [`intro_lines`] with its "Good day" greeting replaced by the greeting and
/// time row and the [`NAME`] row (one row more; still at most
/// `CARD_MAX_ROWS`). Everything after line 1 is unchanged.
pub fn intro_lines_now(
    model: ModelState,
    usb_only: bool,
    now: &crate::clock::Now<'_>,
) -> Vec<String> {
    intro_lines_now_in(crate::lang::get(), model, usb_only, now)
}

/// S6b: [`intro_lines_now`] in `lang` (`now`'s texts already in it).
pub fn intro_lines_now_in(
    lang: Lang,
    model: ModelState,
    usb_only: bool,
    now: &crate::clock::Now<'_>,
) -> Vec<String> {
    let mut rows = intro_lines_in(lang, model, usb_only);
    rows[0] = crate::clock::greeting_row_in(lang, now);
    rows.insert(1, String::from(text(lang, Msg::IntroName)));
    rows
}

/// The help card's rows, in the session's language.
pub fn help_lines() -> Vec<String> {
    help_lines_in(crate::lang::get())
}

/// S6b: [`help_lines`] in `lang`.
pub fn help_lines_in(lang: Lang) -> Vec<String> {
    HELP_MSGS
        .iter()
        .map(|&m| String::from(text(lang, m)))
        .collect()
}

/// Whether the intro shows this session: always on a machine that cannot
/// keep anything (`persistent == false`, D5), else until it has been seen
/// once (the persisted flag, `introduced`).
pub const fn show_intro(persistent: bool, introduced: bool) -> bool {
    !(persistent && introduced)
}

/// Whether an intent field line asks for the help card: `help`, any ASCII
/// case, surrounding spaces ignored. The WM answers it; it never reaches
/// the model.
pub fn is_help(line: &[u8]) -> bool {
    let start = line.iter().position(|&b| b != b' ').unwrap_or(line.len());
    let end = line
        .iter()
        .rposition(|&b| b != b' ')
        .map_or(start, |i| i + 1);
    line[start..end].eq_ignore_ascii_case(HELP_COMMAND.as_bytes())
}

/// A card's button row: `[ go ]`, then `[ Back ]` from column 27 (at
/// least 16 spaces after `go`'s label), drawn as one line, in the session's
/// language. Its labels are what [`crate::BaseScene::card_buttons`]
/// hit-tests.
pub fn button_row(go: &str) -> String {
    button_row_in(crate::lang::get(), go)
}

/// S6b: [`button_row`] with `lang`'s Back (`[ Volver ]`). Columns count
/// characters, so a label with an accent lines up as an ASCII one does.
pub fn button_row_in(lang: Lang, go: &str) -> String {
    let label = format!("[ {go} ]");
    let gap = 27usize.saturating_sub(label.chars().count()).max(16);
    format!("{label}{:gap$}{}", "", text(lang, Msg::ButtonBack))
}
