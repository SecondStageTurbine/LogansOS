// SPDX-License-Identifier: MPL-2.0
//! Names: the check before rename (and new folder), the sort order, and the
//! numbered name for a new folder or a pasted copy.

use crate::fs::NAME_MAX;
use alloc::format;
use alloc::string::String;
use core::cmp::Ordering;

/// Why a name can't be used. F0's own check (`dir_ops_logic::check_name`)
/// refuses the same names, except [`NameError::Control`], which is Files'
/// (F0 refuses NUL; Files refuses every control character, which can't be
/// typed or drawn).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameError {
    Empty,
    /// Over [`NAME_MAX`] bytes of UTF-8.
    TooLong,
    Slash,
    /// NUL or another control character.
    Control,
    /// `.` or `..`.
    Dot,
}

/// Whether `name` can be a file or folder's name.
pub fn check(name: &str) -> Result<(), NameError> {
    if name.is_empty() {
        Err(NameError::Empty)
    } else if name.len() > NAME_MAX {
        Err(NameError::TooLong)
    } else if name.contains('/') {
        Err(NameError::Slash)
    } else if name.chars().any(char::is_control) {
        Err(NameError::Control)
    } else if name == "." || name == ".." {
        Err(NameError::Dot)
    } else {
        Ok(())
    }
}

/// The listing's order for two names of the same group (folders, or files):
/// by the names with ASCII `A`-`Z` folded to `a`-`z`, compared byte by byte;
/// then, for names equal under folding, by their raw bytes (so `B` before
/// `b`). Bytes past ASCII compare by value: no locale collation in the MVP.
pub fn order(a: &str, b: &str) -> Ordering {
    fn fold(s: &str) -> impl Iterator<Item = u8> + '_ {
        s.bytes().map(|c| c.to_ascii_lowercase())
    }
    fold(a)
        .cmp(fold(b))
        .then_with(|| a.as_bytes().cmp(b.as_bytes()))
}

/// The first of `base`, `base 2`, `base 3`, ... that `taken` doesn't hold.
/// For a file the number goes before the extension (`notes 2.txt`). `None`
/// when every candidate up to the folder's limit is taken or too long.
pub fn numbered(base: &str, is_file: bool, taken: impl Fn(&str) -> bool) -> Option<String> {
    let (stem, ext) = match base.rfind('.') {
        // A leading dot is part of the name (`.hidden`), not an extension.
        Some(dot) if is_file && dot > 0 => base.split_at(dot),
        _ => (base, ""),
    };
    // A folder holds at most 64 entries, so 65 candidates always find a free
    // name unless the length rule rules them out.
    (1..=crate::fs::FOLDER_MAX + 1).find_map(|n| {
        let name = if n == 1 {
            String::from(base)
        } else {
            format!("{stem} {n}{ext}")
        };
        (check(&name).is_ok() && !taken(&name)).then_some(name)
    })
}
