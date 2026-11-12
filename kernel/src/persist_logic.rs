// SPDX-License-Identifier: MPL-2.0
//! OT-100 R3: the persistence area's pure decisions
//! (`docs/THINK-2026-11-10-ot100-usb-persistence.md` §2.3, §4). Kernel-free:
//! the kernel's `persist` module and the host KATs in
//! `tools/persist-logic-tests` mount this file by `#[path]`, so it stays free
//! of `crate::` paths, feature cfgs and allocation.
//!
//! - [`parse_cmdline`]: "do we write this boot?" from the Limine command line.
//!   Off is the default; only the exact token `persist=on` turns it on.

/// Whether this boot may write the persistence area (THINK §2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Persist {
    On,
    Off,
}

/// The command-line key, with its `=`.
pub const PERSIST_KEY: &[u8] = b"persist=";

/// The longest command line read; a longer one (or one with no NUL in reach)
/// is not read at all, which is `Off`.
pub const CMDLINE_MAX: usize = 4096;

/// The persistent image's two Limine entries pass `persist=on` and
/// `persist=off`. `On` only for exactly one `persist=` token whose value is
/// exactly `on`. Off for: no command line, no `persist=` token, any other
/// value (`ON`, `1`, `on,` ...), or two `persist=` tokens, even if both say
/// `on`. Tokens are separated by ASCII whitespace; the bytes need not be
/// UTF-8.
pub fn parse_cmdline(cmdline: Option<&[u8]>) -> Persist {
    let Some(line) = cmdline else {
        return Persist::Off;
    };
    let mut found: Option<&[u8]> = None;
    for token in line
        .split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
    {
        if let Some(value) = token.strip_prefix(PERSIST_KEY) {
            if found.is_some() {
                return Persist::Off;
            }
            found = Some(value);
        }
    }
    if found == Some(b"on".as_slice()) {
        Persist::On
    } else {
        Persist::Off
    }
}

/// The NUL-terminated string at the start of `bytes`, or `None` when no NUL
/// lies within the first [`CMDLINE_MAX`] bytes (the kernel reads Limine's
/// pointer through this, so a missing terminator is `Off`, never an overrun).
pub fn c_string(bytes: &[u8]) -> Option<&[u8]> {
    let reach = &bytes[..bytes.len().min(CMDLINE_MAX)];
    reach.iter().position(|&b| b == 0).map(|n| &reach[..n])
}
