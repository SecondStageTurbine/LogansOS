// SPDX-License-Identifier: MPL-2.0
//! S6b's drift KAT, half 2 (`Plans/DISPATCH-2026-11-12-seat-c-setup-s6b-language.md`):
//! the code that draws the wizard, the intro, help, power and clock cards,
//! Home's clock verbs and (F1a) the Files app's core holds no user-visible
//! literal outside the catalog.
//!
//! The scan reads each source file (`include_str!`) and lists every string
//! literal outside comments, attributes and test modules. A literal is
//! user-visible if, with its `{…}` slots removed, it still holds a word (two
//! letters in a row). Byte strings (`b"…"`) are the serial log's and are
//! skipped; so are `expect("…")` messages (a panic, not a row) and
//! `cfg!(feature = "…")` names. What is left
//! must be on [`ALLOWED`], each entry with its reason: the command words the
//! parsers read, zone names, log tags.

extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use std::format;

const SETUP_CORE: &str = include_str!("../../setup-core/src/lib.rs");
const KEYBOARD: &str = include_str!("../../setup-core/src/keyboard.rs");
const SETUP_VIEW: &str = include_str!("../../neo-init/src/desktop_session/setup.rs");
const DESKTOP: &str = include_str!("../../neo-init/src/desktop_session.rs");
const INTRO: &str = include_str!("../../neo-atrium/src/intro.rs");
const CLOCK: &str = include_str!("../../neo-atrium/src/clock.rs");
const POWER: &str = include_str!("../../neo-atrium/src/power.rs");
const SCENE: &str = include_str!("../../neo-atrium/src/scene.rs");
const CIVIL: &str = include_str!("../../libneo/src/civil.rs");
// F1a: Files' core, every module but the tests' fake volume.
const FILES_FS: &str = include_str!("../../files-core/src/fs.rs");
const FILES_NAME: &str = include_str!("../../files-core/src/name.rs");
const FILES_OPEN_WITH: &str = include_str!("../../files-core/src/open_with.rs");
const FILES_FILES: &str = include_str!("../../files-core/src/files.rs");
const FILES_VIEW: &str = include_str!("../../files-core/src/view.rs");

/// Literals that hold a word but are not the user's language, and why.
const ALLOWED: &[(&str, &str)] = &[
    // Zone names are IANA's, in every language; "UTC" is a standard's name.
    ("UTC", "a zone name (the Time zone page's short list)"),
    ("America/New_York", "a zone name"),
    ("Europe/London", "a zone name"),
    ("Australia/Sydney", "a zone name"),
    ("Etc/UTC", "a zone name"),
    (
        "{abbr}, UTC{sign}{h}",
        "a zone's offset label (EDT, UTC-4): abbreviation and standard",
    ),
    ("{abbr}, UTC{sign}{h}:{m:02}", "a zone's offset label"),
    (
        "{}, UTC{}{:02}:{:02}",
        "a zone's offset label (EST, UTC-05:00)",
    ),
    // The command words the parsers read stay as typed (S6a's rule).
    ("help", "the help card's command word"),
    (
        "  timezone {n}",
        "a `timezone` command to type, with a zone name",
    ),
    ("shut down", "a power verb's trigger word"),
    ("shutdown", "a power verb's trigger word"),
    ("power off", "a power verb's trigger word"),
    ("turn off", "a power verb's trigger word"),
    ("restart", "a power verb's trigger word, and its log tag"),
    ("reboot", "a power verb's trigger word"),
    // The serial log's tags and values (`DESKTOP setup page=…`, `done
    // action=…`, the action log's choices), read by the gates.
    ("shut-down", "the power verb's log tag"),
    ("welcome", "a page's log name"),
    ("skip", "a page's log name, and the done line's action"),
    ("language", "a page's log name"),
    ("keyboard", "a page's log name"),
    ("network", "a page's log name"),
    ("timezone", "a page's log name"),
    ("comfort", "a page's log name"),
    ("try", "a page's log name, and the done line's action"),
    ("done", "the action log's end record"),
    ("unknown", "the done line's zone value"),
    ("unset", "the done line's and the action log's zone value"),
    ("begin", "the action log's Welcome choice"),
    ("continue", "the action log's choice on a page without one"),
    ("rm={} rt={}", "the action log's Comfort choice"),
    ("hidden", "the Time zone page's log state"),
    ("review", "the Time zone page's log state"),
    ("search", "the Time zone page's log state"),
    ("none", "the Network page's log value for no driver"),
    // F1a: the extensions the editor opens (`files_core::open_with`): file
    // names' suffixes, the same in every language.
    ("txt", "a file extension the editor opens"),
    ("md", "a file extension the editor opens"),
    ("log", "a file extension the editor opens"),
    ("csv", "a file extension the editor opens"),
    ("json", "a file extension the editor opens"),
    ("toml", "a file extension the editor opens"),
    ("yaml", "a file extension the editor opens"),
    ("yml", "a file extension the editor opens"),
    ("ini", "a file extension the editor opens"),
    ("conf", "a file extension the editor opens"),
    ("cfg", "a file extension the editor opens"),
    ("rs", "a file extension the editor opens"),
    ("py", "a file extension the editor opens"),
    ("sh", "a file extension the editor opens"),
    ("js", "a file extension the editor opens"),
    ("css", "a file extension the editor opens"),
    ("html", "a file extension the editor opens"),
    ("xml", "a file extension the editor opens"),
    (
        "neom",
        "the model file extension (Arthur's, opened by no app)",
    ),
];

/// `src` with comments blanked (line comments, block comments), keeping
/// string literals intact.
fn strip_comments(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let end = literal_end(b, i);
                out.push_str(&src[i..end]);
                i = end;
            }
            b'\'' if i + 2 < b.len() && (b[i + 2] == b'\'' || b[i + 1] == b'\\') => {
                // A char literal ('"', '\''): copy it whole.
                let end = src[i + 1..].find('\'').map_or(b.len(), |j| {
                    let j = i + 1 + j;
                    if b[i + 1] == b'\\' && j == i + 2 {
                        i + 4
                    } else {
                        j + 1
                    }
                });
                out.push_str(&src[i..end]);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let end = src[i..].find('\n').map_or(b.len(), |j| i + j);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let end = src[i + 2..].find("*/").map_or(b.len(), |j| i + 2 + j + 2);
                i = end;
            }
            _ => {
                let c = src[i..].chars().next().unwrap();
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    out
}

/// The index just past the literal starting at `b[start] == '"'`.
fn literal_end(b: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    b.len()
}

/// `src` without its `#[cfg(test)]` modules and `mod tests` blocks.
fn without_tests(src: &str) -> String {
    let mut out = String::new();
    let mut rest = src;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        // Skip the item: to its first `{` and the matching `}`, or `;`.
        let open = after.find('{');
        let semi = after.find(';');
        let end = match (open, semi) {
            (Some(o), Some(s)) if s < o => s + 1,
            (Some(o), _) => matching(after, o),
            (None, Some(s)) => s + 1,
            (None, None) => after.len(),
        };
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// The index past the `}` matching the `{` at `open`.
fn matching(src: &str, open: usize) -> usize {
    let mut depth = 0usize;
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + i + 1;
                }
            }
            _ => {}
        }
    }
    src.len()
}

/// The block of `src` that starts at `marker` (a `mod name {` line).
fn block<'a>(src: &'a str, marker: &str) -> &'a str {
    let at = src
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} not found"));
    let open = at + src[at..].find('{').unwrap();
    &src[at..matching(src, open)]
}

/// Every string literal in `code` that holds a word, with its line.
fn worded_literals(code: &str) -> Vec<(usize, String)> {
    let code = without_tests(&strip_comments(code));
    let b = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        let end = literal_end(b, i);
        let prefix = &code[..i];
        let line_start = prefix.rfind('\n').map_or(0, |n| n + 1);
        let line = &code[line_start..i];
        let byte_string = prefix.ends_with('b') || prefix.ends_with("br");
        let attribute = line.trim_start().starts_with("#[") || line.trim_start().starts_with("#![");
        let not_shown = prefix.trim_end().ends_with("expect(")
            || prefix.trim_end().ends_with("feature =")
            || prefix.trim_end().ends_with("include!(")
            || prefix.trim_end().ends_with("include_str!(")
            || prefix.trim_end().ends_with("panic!(");
        let lit = &code[i + 1..end - 1];
        if !byte_string && !attribute && !not_shown && has_word(lit) {
            out.push((code[..i].matches('\n').count() + 1, String::from(lit)));
        }
        i = end;
    }
    out
}

/// Whether `lit`, without its `{…}` slots, holds two letters in a row.
fn has_word(lit: &str) -> bool {
    let mut bare = String::new();
    let mut depth = 0;
    for c in lit.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => bare.push(c),
            _ => {}
        }
    }
    bare.chars()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|w| w[0].is_alphabetic() && w[1].is_alphabetic())
}

#[test]
fn no_user_visible_literal_outside_the_catalog() {
    let sources: [(&str, &str); 15] = [
        ("setup-core/src/lib.rs", SETUP_CORE),
        ("setup-core/src/keyboard.rs", KEYBOARD),
        ("neo-init/src/desktop_session/setup.rs", SETUP_VIEW),
        (
            "neo-init desktop_session.rs mod zone_verb",
            block(DESKTOP, "mod zone_verb {"),
        ),
        (
            "neo-init desktop_session.rs mod clock",
            block(DESKTOP, "\nmod clock {"),
        ),
        ("neo-atrium/src/intro.rs", INTRO),
        ("neo-atrium/src/clock.rs", CLOCK),
        ("neo-atrium/src/power.rs", POWER),
        (
            "neo-atrium/src/scene.rs fn paint_intro_text",
            block(SCENE, "fn paint_intro_text("),
        ),
        ("libneo/src/civil.rs", CIVIL),
        ("files-core/src/fs.rs", FILES_FS),
        ("files-core/src/name.rs", FILES_NAME),
        ("files-core/src/open_with.rs", FILES_OPEN_WITH),
        ("files-core/src/files.rs", FILES_FILES),
        ("files-core/src/view.rs", FILES_VIEW),
    ];
    let mut stray = Vec::new();
    for (name, src) in sources {
        for (line, lit) in worded_literals(src) {
            if !ALLOWED.iter().any(|&(a, _)| a == lit) {
                stray.push(format!("{name}:{line}: {lit:?}"));
            }
        }
    }
    assert!(
        stray.is_empty(),
        "user-visible literals outside the catalog (move them to i18n-core, or allow-list a command word or log tag with its reason):\n{}",
        stray.join("\n")
    );
}
