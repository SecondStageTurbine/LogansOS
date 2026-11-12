// SPDX-License-Identifier: MPL-2.0
//! A2 (`atrium-launcher`): the Home intents and Arthur lines for the apps that
//! have no module yet. Files comes with F1 and Settings with S1; until then
//! their intent (Home's `files` or `settings`, or the Spine chip clicked in
//! Home, which the WM sends as that intent) and `open files` / `open settings`
//! are answered, and nothing is launched. When F1 or S1 adds the module, its
//! name leaves [`UNAVAILABLE`] (the one switch it flips) and its intent
//! launches like `terminal`.

/// The apps with no module yet: their intent words.
pub const UNAVAILABLE: &[&[u8]] = &[b"files", b"settings"];

/// The app an intent names, if it is one with no module yet: the bare word or
/// `open <word>`, ignoring surrounding spaces.
pub fn unavailable(line: &[u8]) -> Option<&'static [u8]> {
    let line = line.trim_ascii();
    let word = match line.strip_prefix(b"open ") {
        Some(rest) => rest.trim_ascii(),
        None => line,
    };
    UNAVAILABLE.iter().copied().find(|app| *app == word)
}

/// What Arthur answers for one of them.
pub fn unavailable_reply(app: &[u8]) -> &'static [u8] {
    match app {
        b"files" => b"Files isn't available yet.",
        b"settings" => b"Settings isn't available yet.",
        _ => b"That app isn't available yet.",
    }
}
