// SPDX-License-Identifier: MPL-2.0
//! L1.3a (`desktop-actions`): the actions Arthur takes on the desktop, from
//! an explicit command line only. The serial grammar's keyword fallback
//! (`agentic_action_logic`) reads "show", "update" or "state" anywhere in a
//! line as an action; a chat line on the desktop must never become one by
//! accident, so here the line has to *start* with the verb and name an
//! absolute path:
//!
//! - `read <path>`: the file's bytes;
//! - `list <path>`: the directory's entries;
//! - `write <path> <text>`: replace the file with `<text>` (only when the
//!   WM's session allows writes; read-only by default, THINK D7).
//!
//! Anything else is not an action and goes to the model as before.

/// One desktop action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopAction<'a> {
    Read { path: &'a str },
    List { path: &'a str },
    Write { path: &'a str, text: &'a str },
}

/// The atrium `OP_PERMISSION` action ids (`PERMISSION_READ` and so on).
pub const READ: u16 = 1;
pub const LIST: u16 = 2;
pub const WRITE: u16 = 3;

impl<'a> DesktopAction<'a> {
    pub fn path(&self) -> &'a str {
        match *self {
            Self::Read { path } | Self::List { path } | Self::Write { path, .. } => path,
        }
    }

    /// The `OP_PERMISSION` action id.
    pub fn kind(&self) -> u16 {
        match self {
            Self::Read { .. } => READ,
            Self::List { .. } => LIST,
            Self::Write { .. } => WRITE,
        }
    }
}

/// The action a line asks for, or `None` (not an action: chat).
pub fn parse(line: &str) -> Option<DesktopAction<'_>> {
    let line = line.trim();
    let (verb, rest) = line.split_once(' ')?;
    let rest = rest.trim_start();
    let (path, tail) = match rest.split_once(' ') {
        Some((path, tail)) => (path, tail.trim_start()),
        None => (rest, ""),
    };
    if !path.starts_with('/') || path.len() < 2 {
        return None;
    }
    match verb {
        "read" if tail.is_empty() => Some(DesktopAction::Read { path }),
        "list" if tail.is_empty() => Some(DesktopAction::List { path }),
        "write" if !tail.is_empty() => Some(DesktopAction::Write { path, text: tail }),
        _ => None,
    }
}
