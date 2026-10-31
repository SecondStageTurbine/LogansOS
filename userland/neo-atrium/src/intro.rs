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
//! `tests.rs`: ASCII only, no `!`, the length bounds, each row inside the
//! card, and the model-state sentences exactly B1b's.
//!
//! Pure: the WM decides the variant and draws the rows with
//! [`crate::BaseScene::set_card`]; nothing here touches the system.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Line 1 (variant A): Bible 29.1's name, read as part of the greeting.
pub const GREETING: &str =
    "Good day. I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.";
/// `desktop-clock` (OT-85): with a wall clock the greeting moves to its own
/// row with the time ([`crate::clock::greeting_row`]), and the name follows
/// as this row. (One row would be 93 bytes, past the 90 the variant-A
/// greeting is allowed; split, both rows are within the 80 of new rows.)
pub const NAME: &str =
    "I'm Arthur: Assistant for Real-time Tasks, Home automation and User Requests.";
/// Line 2: the household side named as what Arthur is for, not promised.
pub const HOUSEHOLD: &str =
    "The household side comes later; for now, I look after this computer for you.";
/// Line 3 (PL Q2 option b): a description of Arthur's behaviour.
pub const PRIVACY: &str =
    "I work entirely on this machine, and I don't send what you tell me anywhere.";
/// DECISION D5: on a USB-only machine nothing persists, so the intro repeats.
pub const USB_ONLY: &str = "On this machine I can't keep anything once it's switched off, not yet.";
pub const ASK: &str = "Ask in your own words: \"What does a CPU do?\", for instance.";
pub const LIST: &str = "\"list /\" shows you the files on this machine.";
/// The no-model variants: the Lens is the one thing that needs no model.
pub const LIST_STILL: &str = "\"list /\" still shows you the files on this machine.";
/// The exit (persona rule 9): the help card, and Esc back to Home.
pub const EXIT: &str = "Type help for what you can do yourself. Esc always brings you back here.";

/// The help card: only what is true of the 0.1 desktop without the model.
pub const HELP: [&str; 6] = [
    "What you can do yourself, without asking me:",
    "Esc sets me aside and brings you back to Home.",
    "\"list /\" opens the Lens: a read-only view of the files on this machine.",
    "The chips on the left switch between me and the Lens; Tab reaches them.",
    "Drag a window by its title bar to move it, or by an edge to resize it.",
    "Start typing to ask me something, or press Esc to close this card.",
];

/// The visible affordance on Home while no card shows (typed or clicked).
pub const HINT: &str = "Type help, or click here, for what you can do yourself.";

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
    match state {
        ModelState::Ready | ModelState::TooLittleMemory { .. } => None,
        ModelState::Absent => Some(
            "I can hear you, but my model isn't on this system yet. Boot the LogansOS USB it came with.",
        ),
        ModelState::Damaged => Some(
            "My model file is damaged, so I won't use it. Re-create the USB from the original image.",
        ),
        ModelState::LoadFailed => {
            Some("My model didn't load, so I'm running without it. The details are in /model.")
        }
    }
}

/// The introduction's rows for `model`; `usb_only` adds the D5 line.
pub fn intro_lines(model: ModelState, usb_only: bool) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let mut push = |s: &str| rows.push(String::from(s));
    push(GREETING);
    push(HOUSEHOLD);
    match model {
        ModelState::Ready => {
            push(PRIVACY);
            if usb_only {
                push(USB_ONLY);
            }
            push(ASK);
            push(LIST);
        }
        ModelState::TooLittleMemory { have_gb, need_gb } => {
            if usb_only {
                push(USB_ONLY);
            }
            rows.push(format!(
                "This computer has {have_gb} GB of memory; my smallest brain needs {need_gb} GB."
            ));
            rows.push(String::from(
                "I can't think yet, but \"list /\" still shows you the files on this machine.",
            ));
        }
        _ => {
            if usb_only {
                push(USB_ONLY);
            }
            if let Some(row) = state_row(model) {
                push(row);
            }
            push(LIST_STILL);
        }
    }
    rows.push(String::from(EXIT));
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
    let mut rows = intro_lines(model, usb_only);
    rows[0] = crate::clock::greeting_row(now);
    rows.insert(1, String::from(NAME));
    rows
}

/// The help card's rows.
pub fn help_lines() -> Vec<String> {
    HELP.iter().map(|s| String::from(*s)).collect()
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
/// least 16 spaces after `go`'s label), drawn as one line. Its labels are
/// what [`crate::BaseScene::card_buttons`] hit-tests.
pub fn button_row(go: &str) -> String {
    let label = format!("[ {go} ]");
    let gap = 27usize.saturating_sub(label.len()).max(16);
    format!("{label}{:gap$}[ Back ]", "")
}
