// SPDX-License-Identifier: MPL-2.0
//! The Files app's state: where it is, what it lists, what is selected, and
//! the operations, as an explicit state machine ([`Mode`]) driven by
//! [`Input`]s.
//!
//! Three rules hold throughout (Muse on #106, observation 1, and the F1a
//! dispatch item 6):
//! - **No listing outlives an operation.** Every operation, successful or
//!   not, ends with a fresh `readdir` of the current folder ([`Files::relist`]),
//!   and so does every stale error.
//! - **A stale folder falls back to the nearest ancestor that still opens**,
//!   says so ([`Msg::FilesErrStale`]), and lists it.
//! - **Files' own renames reopen the folder by its path** afterwards, so a
//!   rename's post-commit reload error (Muse observation 2) can't leave a
//!   handle behind; a stale reply to a rename that did commit is read back
//!   from the fresh listing.

use crate::fs::{COPY_MAX, DIR_RIGHTS, FOLDER_MAX, Fs, FsError, Kind, NAME_MAX, Rights};
use crate::name::{self, NameError};
use crate::open_with::{EDITOR_RIGHTS, MODULE_EDITOR, OpenRequest, OpenWith, open_with};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use i18n_core::Lang;
use i18n_core::files::{Msg, text};

/// The most `readdir` batches one listing reads before it gives up: a folder
/// holds at most 64 entries, so a well-behaved volume ends far sooner.
const READDIR_BATCHES: usize = 4 * FOLDER_MAX;

/// One row of the listing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Row {
    pub name: String,
    pub kind: Kind,
    /// A file's bytes; a folder's entry count (`stat_at`'s size).
    pub size: u64,
}

/// What the user asks for. F1b maps keys and clicks to these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Input {
    /// Move the selection.
    Up,
    Down,
    First,
    Last,
    /// Open the selection: a folder in place, a file with its app. In a
    /// dialog, the same as [`Input::Confirm`].
    Open,
    /// Go to the parent folder (never above Files' root).
    Parent,
    NewFolder,
    Rename,
    Delete,
    Copy,
    Paste,
    /// List the folder again.
    Refresh,
    /// Typing in the rename field.
    Char(char),
    Erase,
    Confirm,
    Cancel,
}

/// Where the state machine is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Browse,
    /// Renaming `from`; `text` is the field.
    Naming {
        from: String,
        text: String,
    },
    /// Asking once before deleting `name`.
    ConfirmDelete {
        name: String,
        kind: Kind,
    },
}

/// The message line: one catalog row and its argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub msg: Msg,
    pub arg: Option<String>,
}

impl Note {
    const fn of(msg: Msg) -> Note {
        Note { msg, arg: None }
    }

    fn with(msg: Msg, arg: impl ToString) -> Note {
        Note {
            msg,
            arg: Some(arg.to_string()),
        }
    }
}

/// What F1b must do after an input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    None,
    /// Open the file and hand it to the app ([`crate::open_with`]).
    OpenWith(OpenRequest),
}

/// The message for an error from F0's calls.
pub fn error_note(e: FsError) -> Note {
    match e {
        FsError::Stale => Note::of(Msg::FilesErrStale),
        FsError::Exists => Note::of(Msg::FilesErrExists),
        FsError::NotEmpty => Note::of(Msg::FilesErrNotEmpty),
        FsError::NoSpace => Note::of(Msg::FilesErrNoSpace),
        FsError::Refused => Note::of(Msg::FilesErrRefused),
        FsError::NameTooLong => Note::with(Msg::FilesNameTooLong, NAME_MAX),
        FsError::FolderFull => Note::with(Msg::FilesErrFolderFull, FOLDER_MAX),
        FsError::NotFound => Note::of(Msg::FilesErrNotFound),
        FsError::Busy => Note::of(Msg::FilesErrBusy),
        FsError::Retry => Note::of(Msg::FilesErrRetry),
        FsError::Io => Note::of(Msg::FilesErrIo),
        FsError::Invalid => Note::of(Msg::FilesErrInvalid),
        FsError::Other(errno) => Note::with(Msg::FilesErrOther, errno),
    }
}

/// The message for a name the check refuses.
pub fn name_note(e: NameError) -> Note {
    match e {
        NameError::Empty => Note::of(Msg::FilesNameEmpty),
        NameError::TooLong => Note::with(Msg::FilesNameTooLong, NAME_MAX),
        NameError::Slash => Note::of(Msg::FilesNameSlash),
        NameError::Control => Note::of(Msg::FilesNameControl),
        NameError::Dot => Note::of(Msg::FilesNameDot),
    }
}

/// The Files app over a volume `F`.
pub struct Files<F: Fs> {
    fs: F,
    lang: Lang,
    /// The current folder, below Files' root.
    path: Vec<String>,
    /// A handle on `path`; `None` when not even the root opens.
    dir: Option<F::Dir>,
    rows: Vec<Row>,
    selected: Option<usize>,
    mode: Mode,
    note: Option<Note>,
    /// The copied file's path, until it is pasted or replaced.
    clipboard: Option<Vec<String>>,
}

impl<F: Fs> Files<F> {
    /// Files at its root, listed.
    pub fn new(fs: F, lang: Lang) -> Files<F> {
        let mut files = Files {
            fs,
            lang,
            path: Vec::new(),
            dir: None,
            rows: Vec::new(),
            selected: None,
            mode: Mode::Browse,
            note: None,
            clipboard: None,
        };
        files.open_root();
        files
    }

    pub fn fs(&self) -> &F {
        &self.fs
    }

    pub fn fs_mut(&mut self) -> &mut F {
        &mut self.fs
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Whether a folder is open (false only when not even the root opens).
    pub fn available(&self) -> bool {
        self.dir.is_some()
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.selected.map(|i| self.rows[i].name.as_str())
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn note(&self) -> Option<&Note> {
        self.note.as_ref()
    }

    pub fn clipboard(&self) -> Option<&[String]> {
        self.clipboard.as_deref()
    }

    /// Whether the folder is at its 64-entry limit (shown, not hidden).
    pub fn full(&self) -> bool {
        self.rows.len() >= FOLDER_MAX
    }

    /// One input. The message line is cleared first, so it shows only the
    /// last input's outcome.
    pub fn handle(&mut self, input: Input) -> Effect {
        self.note = None;
        match core::mem::replace(&mut self.mode, Mode::Browse) {
            Mode::Browse => self.browse(input),
            Mode::Naming { from, text } => {
                self.naming(from, text, input);
                Effect::None
            }
            Mode::ConfirmDelete { name, kind } => {
                if matches!(input, Input::Confirm | Input::Open) {
                    self.delete(&name, kind);
                }
                // Any other input goes back, as the power card does.
                Effect::None
            }
        }
    }

    fn browse(&mut self, input: Input) -> Effect {
        if self.dir.is_none() {
            // Nothing opens: only a refresh can help.
            if input == Input::Refresh {
                self.open_root();
            } else {
                self.note = Some(Note::of(Msg::FilesRootUnavailable));
            }
            return Effect::None;
        }
        let last = self.rows.len().checked_sub(1);
        match input {
            Input::Up => self.selected = self.selected.map(|i| i.saturating_sub(1)),
            Input::Down => self.selected = self.selected.map(|i| (i + 1).min(last.unwrap_or(0))),
            Input::First => self.selected = last.map(|_| 0),
            Input::Last => self.selected = last,
            Input::Open | Input::Confirm => return self.open_selected(),
            Input::Parent => self.parent(),
            Input::NewFolder => self.new_folder(),
            Input::Rename => {
                if let Some(name) = self.selected_name() {
                    let name = String::from(name);
                    self.mode = Mode::Naming {
                        text: name.clone(),
                        from: name,
                    };
                }
            }
            Input::Delete => self.ask_delete(),
            Input::Copy => self.copy(),
            Input::Paste => self.paste(),
            Input::Refresh => self.relist(None),
            Input::Char(_) | Input::Erase | Input::Cancel => {}
        }
        Effect::None
    }

    // ---- Listing and navigation ----

    /// Read the current folder: `readdir` to the end, then `stat_at` each
    /// entry for its size. An entry gone between the two is skipped.
    fn read_listing(&mut self) -> Result<Vec<Row>, FsError> {
        let dir = self.dir.as_ref().ok_or(FsError::Stale)?;
        let mut entries = Vec::new();
        let mut cursor = 0u64;
        let mut done = false;
        for _ in 0..READDIR_BATCHES {
            if self.fs.readdir(dir, &mut cursor, &mut entries)? == 0 {
                done = true;
                break;
            }
        }
        if !done {
            return Err(FsError::Io);
        }
        let mut rows = Vec::with_capacity(entries.len());
        for entry in entries {
            match self.fs.stat_at(dir, &entry.name) {
                Ok(stat) => rows.push(Row {
                    name: entry.name,
                    kind: stat.kind,
                    size: stat.size,
                }),
                Err(FsError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        rows.sort_by(|a, b| {
            let folder = |r: &Row| r.kind != Kind::Directory;
            folder(a)
                .cmp(&folder(b))
                .then_with(|| name::order(&a.name, &b.name))
        });
        Ok(rows)
    }

    /// List the current folder again, keeping the selection by name: `keep`
    /// if given, else the selected row's name; when that name is gone, the
    /// row now at its place (or the last). A stale folder falls back.
    fn relist(&mut self, keep: Option<&str>) {
        let old_index = self.selected;
        let want: Option<String> = keep
            .map(String::from)
            .or_else(|| self.selected_name().map(String::from));
        match self.read_listing() {
            Ok(rows) => {
                self.rows = rows;
                self.selected = self.find(want.as_deref(), old_index);
            }
            Err(FsError::Stale) => self.recover(),
            Err(e) => {
                // No listing survives a failed read.
                self.rows.clear();
                self.selected = None;
                self.set_error(e);
            }
        }
    }

    /// The row named `want`, or the neighbour at `old_index`.
    fn find(&self, want: Option<&str>, old_index: Option<usize>) -> Option<usize> {
        let last = self.rows.len().checked_sub(1)?;
        want.and_then(|w| self.rows.iter().position(|r| r.name == w))
            .or_else(|| old_index.map(|i| i.min(last)))
            .or(Some(0))
    }

    fn set_error(&mut self, e: FsError) {
        // A stale note from a fallback is the more useful of the two.
        if self.note.is_none() {
            self.note = Some(error_note(e));
        }
    }

    fn close_dir(&mut self) {
        if let Some(dir) = self.dir.take() {
            self.fs.close_dir(dir);
        }
    }

    fn open_root(&mut self) {
        self.close_dir();
        self.path.clear();
        self.go_to(None);
    }

    /// Open `self.path` and list it, selecting `select`. On a stale folder,
    /// fall back.
    fn go_to(&mut self, select: Option<&str>) {
        self.close_dir();
        match self.fs.open_dir(&self.path, DIR_RIGHTS) {
            Ok(dir) => {
                self.dir = Some(dir);
                self.selected = None;
                self.relist(select);
            }
            Err(_) if !self.path.is_empty() => self.recover(),
            Err(_) => self.unavailable(),
        }
    }

    fn unavailable(&mut self) {
        self.close_dir();
        self.rows.clear();
        self.selected = None;
        self.note = Some(Note::of(Msg::FilesRootUnavailable));
    }

    /// The current folder is stale: walk up to the nearest ancestor that
    /// opens and lists, say so, and select the child we came from if it is
    /// still there.
    fn recover(&mut self) {
        self.close_dir();
        loop {
            let Some(came_from) = self.path.pop() else {
                // The root itself is gone: nothing to fall back to.
                self.unavailable();
                return;
            };
            let Ok(dir) = self.fs.open_dir(&self.path, DIR_RIGHTS) else {
                continue;
            };
            self.dir = Some(dir);
            match self.read_listing() {
                Ok(rows) => {
                    self.rows = rows;
                    self.selected = self.find(Some(&came_from), None);
                    self.note = Some(Note::of(Msg::FilesErrStale));
                    return;
                }
                Err(_) => self.close_dir(),
            }
        }
    }

    fn open_selected(&mut self) -> Effect {
        let Some(i) = self.selected else {
            return Effect::None;
        };
        let row = self.rows[i].clone();
        if row.kind == Kind::Directory {
            self.enter(&row.name);
            return Effect::None;
        }
        match open_with(&row.name, row.kind) {
            OpenWith::Editor => {
                let mut path = self.path.clone();
                path.push(row.name);
                Effect::OpenWith(OpenRequest {
                    path,
                    module_id: MODULE_EDITOR,
                    rights: EDITOR_RIGHTS,
                })
            }
            OpenWith::Model => {
                self.note = Some(Note::of(Msg::FilesModelArthur));
                Effect::None
            }
            OpenWith::Nothing => {
                self.note = Some(Note::of(Msg::FilesNoApp));
                Effect::None
            }
        }
    }

    /// Open the child folder `name` in place.
    fn enter(&mut self, name: &str) {
        let mut child = self.path.clone();
        child.push(String::from(name));
        match self.fs.open_dir(&child, DIR_RIGHTS) {
            Ok(dir) => {
                self.close_dir();
                self.path = child;
                self.dir = Some(dir);
                self.selected = None;
                self.relist(None);
            }
            Err(e) => {
                // The folder went away (or turned into a file) since the
                // listing: say so and list again.
                self.note = Some(error_note(match e {
                    FsError::Stale => FsError::NotFound,
                    e => e,
                }));
                self.relist(None);
            }
        }
    }

    fn parent(&mut self) {
        let Some(name) = self.path.pop() else {
            return; // Never above the root cap.
        };
        self.go_to(Some(&name));
    }

    // ---- Operations ----

    fn new_folder(&mut self) {
        if self.full() {
            self.note = Some(error_note(FsError::FolderFull));
            return;
        }
        let base = text(self.lang, Msg::FilesNewFolderName);
        let Some(name) = name::numbered(base, false, |n| self.rows.iter().any(|r| r.name == n))
        else {
            self.note = Some(error_note(FsError::FolderFull));
            return;
        };
        let result = match self.dir.as_ref() {
            Some(dir) => self.fs.mkdir_at(dir, &name),
            None => Err(FsError::Stale),
        };
        self.finish(result, Some(&name));
    }

    fn naming(&mut self, from: String, mut field: String, input: Input) {
        match input {
            Input::Char(c) => field.push(c),
            Input::Erase => {
                field.pop();
            }
            Input::Cancel => return,
            Input::Confirm | Input::Open => {
                if let Err(e) = name::check(&field) {
                    self.note = Some(name_note(e));
                } else if field == from {
                    return;
                } else if self.rows.iter().any(|r| r.name == field) {
                    // F0 refuses a rename over a name; say so before asking.
                    self.note = Some(error_note(FsError::Exists));
                } else {
                    self.rename(&from, &field);
                    return;
                }
            }
            _ => {}
        }
        self.mode = Mode::Naming { from, text: field };
    }

    fn rename(&mut self, from: &str, to: &str) {
        let result = match self.dir.as_ref() {
            Some(dir) => self.fs.rename_at(dir, from, dir, to),
            None => Err(FsError::Stale),
        };
        // Reopen the folder by its path: no handle survives the rename.
        let path = self.path.clone();
        self.close_dir();
        match self.fs.open_dir(&path, DIR_RIGHTS) {
            Ok(dir) => self.dir = Some(dir),
            Err(_) => {
                self.recover();
                return;
            }
        }
        match result {
            Ok(()) => self.relist(Some(to)),
            Err(FsError::Stale) => {
                // The reply was stale, but the rename may have committed
                // before the reload: the fresh listing says which.
                self.relist(Some(from));
                let has = |n: &str| self.rows.iter().any(|r| r.name == n);
                if self.dir.is_some() && has(to) && !has(from) {
                    self.selected = self.find(Some(to), self.selected);
                } else if self.note.is_none() {
                    self.note = Some(error_note(FsError::Stale));
                }
            }
            Err(e) => {
                self.relist(Some(from));
                self.set_error(e);
            }
        }
    }

    fn ask_delete(&mut self) {
        let Some(i) = self.selected else {
            return;
        };
        let row = &self.rows[i];
        if row.kind == Kind::Directory && row.size > 0 {
            // Refused with its own message, without asking.
            self.note = Some(error_note(FsError::NotEmpty));
            return;
        }
        self.mode = Mode::ConfirmDelete {
            name: row.name.clone(),
            kind: row.kind,
        };
    }

    fn delete(&mut self, name: &str, _kind: Kind) {
        let result = match self.dir.as_ref() {
            Some(dir) => self.fs.unlink_at(dir, name),
            None => Err(FsError::Stale),
        };
        self.finish(result, None);
    }

    fn copy(&mut self) {
        let Some(i) = self.selected else {
            return;
        };
        let row = &self.rows[i];
        if row.kind != Kind::Regular {
            self.note = Some(Note::of(Msg::FilesCopyNotFile));
        } else if row.size > COPY_MAX as u64 {
            self.note = Some(Note::with(Msg::FilesCopyTooBig, COPY_MAX / 1024));
        } else {
            let mut path = self.path.clone();
            path.push(row.name.clone());
            self.clipboard = Some(path);
            self.note = Some(Note::of(Msg::FilesCopied));
        }
    }

    fn paste(&mut self) {
        let Some(src) = self.clipboard.clone() else {
            self.note = Some(Note::of(Msg::FilesNothingToPaste));
            return;
        };
        if self.full() {
            self.note = Some(error_note(FsError::FolderFull));
            return;
        }
        let src_name = src.last().cloned().unwrap_or_default();
        let Some(name) = name::numbered(&src_name, true, |n| self.rows.iter().any(|r| r.name == n))
        else {
            self.note = Some(error_note(FsError::FolderFull));
            return;
        };
        let result = self.copy_file(&src, &name);
        if result == Ok(true) {
            self.note = Some(Note::of(Msg::FilesPasted));
        }
        self.finish(result.map(|_| ()), Some(&name));
    }

    /// Read `src` (at most 32 KiB) and write it as `name` here: `Ok(false)`
    /// when it is over the limit and nothing was written.
    fn copy_file(&mut self, src: &[String], name: &str) -> Result<bool, FsError> {
        let file = self.fs.open_file(src, false, Rights::READ)?;
        let mut bytes = vec![0u8; COPY_MAX + 1];
        let mut len = 0;
        let read = loop {
            match self.fs.read(&file, &mut bytes[len..]) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    len += n;
                    if len > COPY_MAX {
                        break Ok(());
                    }
                }
                Err(e) => break Err(e),
            }
        };
        self.fs.close_file(file);
        read?;
        if len > COPY_MAX {
            // It grew since it was copied: say so, write nothing.
            self.note = Some(Note::with(Msg::FilesCopyTooBig, COPY_MAX / 1024));
            return Ok(false);
        }
        let mut dst = self.path.clone();
        dst.push(String::from(name));
        let file = self.fs.open_file(&dst, true, Rights::WRITE)?;
        let mut at = 0;
        let wrote = loop {
            if at == len {
                break Ok(());
            }
            match self.fs.write(&file, &bytes[at..len]) {
                Ok(0) => break Err(FsError::NoSpace),
                Ok(n) => at += n,
                Err(e) => break Err(e),
            }
        };
        self.fs.close_file(file);
        if wrote.is_err() {
            // Leave no half copy behind.
            if let Some(dir) = self.dir.as_ref() {
                let _ = self.fs.unlink_at(dir, name);
            }
        }
        wrote.map(|()| true)
    }

    /// End an operation: say what failed, and list the folder again (always).
    fn finish(&mut self, result: Result<(), FsError>, select: Option<&str>) {
        match result {
            Ok(()) => self.relist(select),
            Err(FsError::Stale) => self.recover(),
            Err(e) => {
                self.note = Some(error_note(e));
                self.relist(None);
            }
        }
    }
}

impl Note {
    /// The message in `lang`, its argument filled in.
    pub fn text(&self, lang: Lang) -> String {
        match &self.arg {
            Some(arg) => i18n_core::files::format(lang, self.msg, &[arg.as_str()]),
            None => String::from(text(lang, self.msg)),
        }
    }
}

/// The folder path as it reads in the title: `/`, `/docs`, `/docs/notes`.
pub fn location(path: &[String]) -> String {
    if path.is_empty() {
        String::from("/")
    } else {
        path.iter().fold(String::new(), |s, c| format!("{s}/{c}"))
    }
}
