// SPDX-License-Identifier: MPL-2.0
//! The render model: what Files shows, as plain data in the session's
//! language, so F1b paints it without deciding anything. It follows the Lens
//! (`neo-worker/src/lens.rs`): a location line, then one row per entry with a
//! folder's name ending in `/`, then a footer for an empty or full folder.
//! Files adds the kind and size columns, the selection, a dialog and the
//! message line.

use crate::files::{Files, Mode, Row, location};
use crate::fs::{FOLDER_MAX, Fs, Kind};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use i18n_core::files::{Msg, text};
use i18n_core::{Lang, fill};

/// One row as drawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowView {
    /// The name, with `/` after a folder's (the Lens's mark).
    pub label: String,
    pub kind: String,
    pub size: String,
    pub folder: bool,
}

/// A dialog over the listing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Dialog {
    /// The rename field: its title, what is typed, and the hint.
    Rename {
        title: String,
        field: String,
        hint: String,
    },
    /// The one confirmation before a delete: the question, the name, the hint.
    ConfirmDelete {
        question: String,
        name: String,
        hint: String,
    },
}

/// Everything Files shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    pub title: String,
    /// `/`, `/docs`, ...
    pub location: String,
    pub rows: Vec<RowView>,
    pub selected: Option<usize>,
    /// "This folder is empty." or the full folder's note, under the rows.
    pub footer: Option<String>,
    pub dialog: Option<Dialog>,
    /// The last input's outcome.
    pub message: Option<String>,
    /// The key hints, one per line.
    pub hints: Vec<String>,
}

/// A size as the column shows it: bytes under 1 KB, else KB or MB with one
/// decimal under 10 and none from 10 up (1.5 KB, 12 KB), rounded half up.
pub fn size_text(lang: Lang, bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    if bytes < KIB {
        return fill(text(lang, Msg::FilesSizeBytes), &[&format!("{bytes}")]);
    }
    let (unit, msg) = if bytes < MIB - KIB / 2 {
        (KIB, Msg::FilesSizeKb)
    } else {
        (MIB, Msg::FilesSizeMb)
    };
    let tenths = (bytes * 10 + unit / 2) / unit;
    let number = if tenths < 100 {
        format!(
            "{}{}{}",
            tenths / 10,
            text(lang, Msg::FilesDecimalMark),
            tenths % 10
        )
    } else {
        format!("{}", (bytes + unit / 2) / unit)
    };
    fill(text(lang, msg), &[&number])
}

/// A row's kind column.
pub fn kind_text(lang: Lang, kind: Kind) -> &'static str {
    text(
        lang,
        match kind {
            Kind::Directory => Msg::FilesKindFolder,
            Kind::Regular => Msg::FilesKindFile,
            Kind::Model => Msg::FilesKindModel,
            Kind::Symlink => Msg::FilesKindLink,
        },
    )
}

/// A row's size column: a folder's entry count, a file's size.
pub fn row_size(lang: Lang, row: &Row) -> String {
    match row.kind {
        Kind::Directory if row.size == 1 => String::from(text(lang, Msg::FilesItemsOne)),
        Kind::Directory => fill(text(lang, Msg::FilesItemsMany), &[&format!("{}", row.size)]),
        _ => size_text(lang, row.size),
    }
}

/// Files' view now, in its language.
pub fn view<F: Fs>(files: &Files<F>) -> View {
    let lang = files.lang();
    let rows: Vec<RowView> = files
        .rows()
        .iter()
        .map(|r| {
            let folder = r.kind == Kind::Directory;
            RowView {
                label: if folder {
                    format!("{}/", r.name)
                } else {
                    r.name.clone()
                },
                kind: String::from(kind_text(lang, r.kind)),
                size: row_size(lang, r),
                folder,
            }
        })
        .collect();
    let footer = if !files.available() {
        None
    } else if rows.is_empty() {
        Some(String::from(text(lang, Msg::FilesEmpty)))
    } else if files.full() {
        Some(fill(
            text(lang, Msg::FilesFolderFullNote),
            &[&format!("{FOLDER_MAX}")],
        ))
    } else {
        None
    };
    let (dialog, hints) = match files.mode() {
        Mode::Browse => (
            None,
            alloc::vec![
                String::from(text(lang, Msg::FilesHintBrowse)),
                String::from(text(lang, Msg::FilesHintBrowse2)),
            ],
        ),
        Mode::Naming { text: field, .. } => (
            Some(Dialog::Rename {
                title: String::from(text(lang, Msg::FilesRenameTitle)),
                field: field.clone(),
                hint: String::from(text(lang, Msg::FilesHintNaming)),
            }),
            Vec::new(),
        ),
        Mode::ConfirmDelete { name, kind } => (
            Some(Dialog::ConfirmDelete {
                question: String::from(text(
                    lang,
                    if *kind == Kind::Directory {
                        Msg::FilesConfirmDeleteFolder
                    } else {
                        Msg::FilesConfirmDeleteFile
                    },
                )),
                name: name.clone(),
                hint: String::from(text(lang, Msg::FilesConfirmHint)),
            }),
            Vec::new(),
        ),
    };
    View {
        title: String::from(text(lang, Msg::FilesTitle)),
        location: location(files.path()),
        rows,
        selected: files.selected(),
        footer,
        dialog,
        message: files.note().map(|n| n.text(lang)),
        hints,
    }
}
