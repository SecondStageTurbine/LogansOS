// SPDX-License-Identifier: MPL-2.0
//! "Open with" (THINK OT-88 §4.3): the MVP's fixed mapping from an entry to
//! the app that opens it. Files returns an [`OpenRequest`]; F1b opens the
//! file by its path with the request's rights, from Files' own directory cap,
//! and sends `OP_OPEN_WITH {module_id, rights}` with that one file handle. The
//! core never holds a handle for the target app.

use crate::fs::{Kind, Rights};
use alloc::string::String;
use alloc::vec::Vec;

/// The editor's module id (OT-67 E2: `MODULE_EDITOR = 7`, THINK OT-88 §2).
/// Files only names it; the broker checks it against the manifest.
pub const MODULE_EDITOR: u16 = 7;

/// The rights the editor's manifest asks for on the file it is given.
pub const EDITOR_RIGHTS: Rights = Rights(Rights::READ.0 | Rights::WRITE.0);

/// The extensions the editor opens, compared without ASCII case. A file with
/// none of these (or no extension) opens with nothing. The editor still
/// checks the bytes (`neo_edit_core::format`), so a binary file with a text
/// extension is refused there.
pub const TEXT_EXTENSIONS: &[&str] = &[
    "txt", "md", "log", "csv", "json", "toml", "yaml", "yml", "ini", "conf", "cfg", "rs", "c", "h",
    "py", "sh", "js", "css", "html", "xml",
];

/// What opens an entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenWith {
    /// The editor, with [`EDITOR_RIGHTS`].
    Editor,
    /// A model (`.neom`, or the Model kind): Arthur's, not an app's.
    Model,
    /// No app opens it.
    Nothing,
}

/// The app for an entry named `name` of `kind`. Folders open in place and
/// never reach here.
pub fn open_with(name: &str, kind: Kind) -> OpenWith {
    let ext = match name.rfind('.') {
        Some(dot) if dot > 0 => &name[dot + 1..],
        _ => "",
    };
    if kind == Kind::Model || ext.eq_ignore_ascii_case("neom") {
        return OpenWith::Model;
    }
    if kind == Kind::Regular && TEXT_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
        OpenWith::Editor
    } else {
        OpenWith::Nothing
    }
}

/// Files' request to the broker: open `path` (below Files' root) with
/// `rights`, and hand that one handle to `module_id`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRequest {
    pub path: Vec<String>,
    pub module_id: u16,
    pub rights: Rights,
}
