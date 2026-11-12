// SPDX-License-Identifier: MPL-2.0
//! files-core's KATs, against the in-memory volume in `fake.rs`.

extern crate std;

use crate::fake::{FakeFs, Op, p};
use crate::files::{Effect, Files, Input, Mode, Note, error_note, location, name_note};
use crate::fs::{COPY_MAX, FOLDER_MAX, FsError, Kind, NAME_MAX, Rights};
use crate::name::{self, NameError};
use crate::open_with::{
    EDITOR_RIGHTS, MODULE_EDITOR, OpenRequest, OpenWith, TEXT_EXTENSIONS, open_with,
};
use crate::view::{Dialog, size_text, view};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use i18n_core::{Lang, Msg};
use std::format;

fn names(files: &Files<FakeFs>) -> Vec<&str> {
    files.rows().iter().map(|r| r.name.as_str()).collect()
}

fn at(files: &Files<FakeFs>, name: &str) -> usize {
    files
        .rows()
        .iter()
        .position(|r| r.name == name)
        .unwrap_or_else(|| panic!("{name:?} not listed: {:?}", names(files)))
}

/// Select `name`, by moving the selection as a user would.
fn select(files: &mut Files<FakeFs>, name: &str) {
    let want = at(files, name);
    files.handle(Input::First);
    for _ in 0..want {
        files.handle(Input::Down);
    }
    assert_eq!(files.selected_name(), Some(name));
}

fn note(files: &Files<FakeFs>) -> Option<Msg> {
    files.note().map(|n| n.msg)
}

fn files_with(setup: impl FnOnce(&mut FakeFs)) -> Files<FakeFs> {
    let mut fs = FakeFs::new();
    setup(&mut fs);
    Files::new(fs, Lang::En)
}

fn type_name(files: &mut Files<FakeFs>, name: &str) {
    for c in name.chars() {
        files.handle(Input::Char(c));
    }
}

// ---- 1. The directory trait's errors ----

#[test]
fn errnos_map_to_f0s_error_set() {
    let table = [
        (-116, FsError::Stale),
        (-20, FsError::Stale),
        (-17, FsError::Exists),
        (-39, FsError::NotEmpty),
        (-13, FsError::Refused),
        (-1, FsError::Refused),
        (-21, FsError::Refused),
        (-18, FsError::Refused),
        (-2, FsError::NotFound),
        (-16, FsError::Busy),
        (-11, FsError::Retry),
        (-5, FsError::Io),
        (-22, FsError::Invalid),
        (-38, FsError::Other(-38)),
    ];
    for (errno, want) in table {
        assert_eq!(FsError::from_errno(errno), want, "errno {errno}");
    }
}

#[test]
fn every_error_has_its_own_message_in_both_languages() {
    let all = [
        FsError::Stale,
        FsError::Exists,
        FsError::NotEmpty,
        FsError::NoSpace,
        FsError::Refused,
        FsError::NameTooLong,
        FsError::FolderFull,
        FsError::NotFound,
        FsError::Busy,
        FsError::Retry,
        FsError::Io,
        FsError::Invalid,
        FsError::Other(-38),
    ];
    let mut seen = Vec::new();
    for e in all {
        let n = error_note(e);
        assert!(!seen.contains(&n.msg), "{e:?} shares a message");
        seen.push(n.msg);
        for lang in Lang::ALL {
            let t = n.text(lang);
            assert!(!t.contains('{'), "{e:?} {lang:?}: unfilled {t:?}");
        }
    }
    assert_eq!(
        error_note(FsError::FolderFull).text(Lang::En),
        "This folder is full: 64 items at most."
    );
    assert_eq!(
        error_note(FsError::NameTooLong).text(Lang::Es),
        "Un nombre puede tener como máximo 47 bytes."
    );
    assert_eq!(
        error_note(FsError::Other(-38)).text(Lang::En),
        "That didn't work (error -38)."
    );
}

#[test]
fn rights_are_libneos_bits() {
    assert_eq!(Rights::READ.0, 1);
    assert_eq!(Rights::WRITE.0, 2);
    assert_eq!(Rights::LOOKUP.0, 4);
    assert_eq!(Rights::READDIR.0, 8);
    assert_eq!(crate::fs::DIR_RIGHTS.0, 2 | 4 | 8);
    assert_eq!(EDITOR_RIGHTS.0, 1 | 2);
    for raw in 1..=4 {
        assert!(Kind::from_raw(raw).is_some());
    }
    assert_eq!(Kind::from_raw(0), None);
    assert_eq!(Kind::from_raw(5), None);
}

// ---- 2. The listing ----

#[test]
fn folders_come_first_then_files_each_by_the_stated_order() {
    let files = files_with(|fs| {
        // The disk's order is the order they were made: deliberately shuffled.
        for f in [
            "b.txt",
            "Zeta.md",
            "a.txt",
            "B.txt",
            "_x",
            "10.txt",
            "9.txt",
            "éclair.txt",
        ] {
            fs.ext_file(f, b"x");
        }
        for d in ["music", "Docs", "apps", "docs"] {
            fs.ext_mkdir(d);
        }
    });
    assert_eq!(
        names(&files),
        [
            // Folders: folded ASCII, then raw bytes (`Docs` before `docs`).
            "apps",
            "Docs",
            "docs",
            "music",
            // Files: digits, `_` (0x5F) before letters? No: folded, `_` is
            // 0x5F and letters fold to 0x61.., so `_x` sorts before `a`.
            // Past ASCII, bytes: `é` (0xC3..) after every ASCII letter.
            "10.txt",
            "9.txt",
            "_x",
            "a.txt",
            "B.txt",
            "b.txt",
            "Zeta.md",
            "éclair.txt",
        ]
    );
}

#[test]
fn the_order_is_folded_ascii_then_bytes() {
    use core::cmp::Ordering::*;
    assert_eq!(name::order("a", "B"), Less);
    assert_eq!(
        name::order("B", "b"),
        Less,
        "equal folded: raw bytes, so B first"
    );
    assert_eq!(name::order("b", "B"), Greater);
    assert_eq!(name::order("abc", "ABD"), Less);
    assert_eq!(name::order("a", "a"), Equal);
    assert_eq!(name::order("a", "ab"), Less);
    assert_eq!(name::order("Z", "é"), Less, "non-ASCII by byte value");
    assert_eq!(name::order("É", "é"), Less, "only ASCII folds");
}

#[test]
fn rows_carry_kind_and_size_and_readdir_is_read_to_the_end() {
    let mut files = files_with(|fs| {
        fs.batch = 2;
        fs.ext_mkdir("d");
        fs.ext_file("d/one", b"1");
        fs.ext_file("d/two", b"2");
        fs.ext_file("f.txt", &[0; 1536]);
        fs.ext_node("model.neom", Kind::Model, &[0; 10]);
        fs.ext_node("link", Kind::Symlink, b"");
        fs.ext_mkdir("empty");
    });
    let rows = files.rows().to_vec();
    assert_eq!(rows.len(), 5);
    assert_eq!(
        (rows[0].name.as_str(), rows[0].kind, rows[0].size),
        ("d", Kind::Directory, 2)
    );
    assert_eq!((rows[1].name.as_str(), rows[1].size), ("empty", 0));
    assert_eq!(
        (rows[2].name.as_str(), rows[2].kind, rows[2].size),
        ("f.txt", Kind::Regular, 1536)
    );
    assert_eq!(rows[3].kind, Kind::Symlink);
    assert_eq!(rows[4].kind, Kind::Model);
    let v = view(&files);
    assert_eq!(v.rows[0].label, "d/");
    assert_eq!(v.rows[0].size, "2 items");
    assert_eq!(v.rows[1].size, "0 items");
    assert_eq!(v.rows[2].size, "1.5 KB");
    assert_eq!(v.rows[2].kind, "File");
    assert_eq!(v.rows[3].kind, "Link");
    assert_eq!(v.rows[4].kind, "Model");
    files.set_lang(Lang::Es);
    let v = view(&files);
    assert_eq!(v.rows[2].size, "1,5 KB");
    assert_eq!(v.rows[0].kind, "Carpeta");
    assert_eq!(v.rows[0].size, "2 elementos");
}

#[test]
fn an_entry_gone_between_readdir_and_stat_is_left_out() {
    let mut files = files_with(|fs| {
        fs.ext_file("a", b"");
        fs.ext_file("b", b"");
    });
    files.fs_mut().fail_next(Op::Stat, FsError::NotFound);
    files.handle(Input::Refresh);
    assert_eq!(names(&files), ["b"]);
    assert_eq!(note(&files), None);
}

#[test]
fn a_failed_read_leaves_no_listing_behind() {
    let mut files = files_with(|fs| fs.ext_file("a", b""));
    files.fs_mut().fail_next(Op::Readdir, FsError::Io);
    files.handle(Input::Refresh);
    assert!(files.rows().is_empty());
    assert_eq!(files.selected(), None);
    assert_eq!(note(&files), Some(Msg::FilesErrIo));
    files.handle(Input::Refresh);
    assert_eq!(names(&files), ["a"]);
}

#[test]
fn the_64_entry_limit_is_shown_and_enforced_before_the_call() {
    let mut files = files_with(|fs| {
        for i in 0..FOLDER_MAX {
            fs.ext_file(&format!("f{i:02}"), b"x");
        }
        fs.ext_mkdir_in_root_for_copy();
    });
    // 64 entries (the helper adds none at the root).
    assert_eq!(files.rows().len(), FOLDER_MAX);
    assert!(files.full());
    let v = view(&files);
    assert_eq!(
        v.footer.as_deref(),
        Some("This folder holds 64 items, the most it can.")
    );
    let before = files.fs().log.len();
    files.handle(Input::NewFolder);
    assert_eq!(note(&files), Some(Msg::FilesErrFolderFull));
    select(&mut files, "f00");
    files.handle(Input::Copy);
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesErrFolderFull));
    assert_eq!(files.fs().log.len(), before, "no call was made");
    files.set_lang(Lang::Es);
    assert_eq!(
        view(&files).footer.as_deref(),
        Some("Esta carpeta tiene 64 elementos, el máximo posible.")
    );
}

impl FakeFs {
    /// Nothing: keeps the 64-entry KAT's setup readable.
    fn ext_mkdir_in_root_for_copy(&mut self) {}
}

#[test]
fn an_empty_folder_says_so() {
    let files = files_with(|_| {});
    let v = view(&files);
    assert!(v.rows.is_empty());
    assert_eq!(v.selected, None);
    assert_eq!(v.footer.as_deref(), Some("This folder is empty."));
    assert_eq!(v.location, "/");
    assert_eq!(v.title, "Files");
}

// ---- 3. Navigation ----

#[test]
fn a_folder_opens_in_place_and_up_returns_to_it() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_mkdir("a/b");
        fs.ext_file("a/b/n.txt", b"hi");
        fs.ext_file("z.txt", b"");
    });
    select(&mut files, "a");
    files.handle(Input::Open);
    assert_eq!(files.path(), p("a"));
    files.handle(Input::Open);
    assert_eq!(files.path(), p("a/b"));
    assert_eq!(view(&files).location, "/a/b");
    assert_eq!(names(&files), ["n.txt"]);
    files.handle(Input::Parent);
    assert_eq!(files.path(), p("a"));
    assert_eq!(
        files.selected_name(),
        Some("b"),
        "Up selects the folder it left"
    );
    files.handle(Input::Parent);
    assert_eq!(files.selected_name(), Some("a"));
    files.handle(Input::Parent);
    assert!(files.path().is_empty(), "never above the root");
    assert_eq!(
        files.fs().open_handles(),
        1,
        "one handle: the current folder"
    );
}

#[test]
fn the_selection_is_kept_by_name_across_a_refresh() {
    let mut files = files_with(|fs| {
        for f in ["b", "d", "f"] {
            fs.ext_file(f, b"");
        }
    });
    select(&mut files, "d");
    // Another process adds entries before it: the index moves, the name stays.
    files.fs_mut().ext_file("a", b"");
    files.fs_mut().ext_file("c", b"");
    files.handle(Input::Refresh);
    assert_eq!(files.selected_name(), Some("d"));
    // The selected name is removed: its neighbour takes its place.
    files.fs_mut().ext_remove("d");
    files.handle(Input::Refresh);
    assert_eq!(names(&files), ["a", "b", "c", "f"]);
    assert_eq!(files.selected_name(), Some("f"), "the row now at its index");
    files.fs_mut().ext_remove("f");
    files.handle(Input::Refresh);
    assert_eq!(
        files.selected_name(),
        Some("c"),
        "past the end: the last row"
    );
    for f in ["a", "b", "c"] {
        files.fs_mut().ext_remove(f);
    }
    files.handle(Input::Refresh);
    assert_eq!(files.selected(), None);
}

#[test]
fn the_selection_moves_within_the_rows() {
    let mut files = files_with(|fs| {
        for f in ["a", "b", "c"] {
            fs.ext_file(f, b"");
        }
    });
    assert_eq!(files.selected(), Some(0));
    files.handle(Input::Up);
    assert_eq!(files.selected(), Some(0));
    files.handle(Input::Last);
    assert_eq!(files.selected(), Some(2));
    files.handle(Input::Down);
    assert_eq!(files.selected(), Some(2));
    files.handle(Input::First);
    assert_eq!(files.selected(), Some(0));
}

#[test]
fn a_folder_gone_before_it_opens_says_so_and_relists() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_file("b", b"");
    });
    select(&mut files, "a");
    files.fs_mut().ext_remove("a");
    files.handle(Input::Open);
    assert!(files.path().is_empty());
    assert_eq!(note(&files), Some(Msg::FilesErrNotFound));
    assert_eq!(names(&files), ["b"]);
    // Replaced by a file of the same name: the same.
    files.fs_mut().ext_mkdir("c");
    files.handle(Input::Refresh);
    select(&mut files, "c");
    files.fs_mut().ext_remove("c");
    files.fs_mut().ext_file("c", b"");
    files.handle(Input::Open);
    assert!(files.path().is_empty());
    assert_eq!(note(&files), Some(Msg::FilesErrNotFound));
}

#[test]
fn a_root_that_wont_open_is_shown_and_a_refresh_retries() {
    let mut fs = FakeFs::new();
    fs.ext_file("a", b"");
    fs.fail_next(Op::OpenDir, FsError::Io);
    let mut files = Files::new(fs, Lang::En);
    assert!(!files.available());
    assert_eq!(note(&files), Some(Msg::FilesRootUnavailable));
    assert_eq!(view(&files).footer, None, "not 'empty': unknown");
    files.handle(Input::NewFolder);
    assert_eq!(note(&files), Some(Msg::FilesRootUnavailable));
    assert!(files.fs().log.is_empty());
    files.handle(Input::Refresh);
    assert!(files.available());
    assert_eq!(names(&files), ["a"]);
}

// ---- 4. Operations ----

#[test]
fn new_folder_numbers_its_name_and_selects_it() {
    let mut files = files_with(|fs| fs.ext_file("zz", b""));
    files.handle(Input::NewFolder);
    assert_eq!(files.selected_name(), Some("New folder"));
    files.handle(Input::NewFolder);
    assert_eq!(files.selected_name(), Some("New folder 2"));
    files.handle(Input::NewFolder);
    assert_eq!(
        names(&files),
        ["New folder", "New folder 2", "New folder 3", "zz"]
    );
    files.set_lang(Lang::Es);
    files.handle(Input::NewFolder);
    assert_eq!(files.selected_name(), Some("Carpeta nueva"));
    assert!(files.fs().exists("/Carpeta nueva"));
}

#[test]
fn numbered_names() {
    let none = |_: &str| false;
    assert_eq!(
        name::numbered("New folder", false, none).as_deref(),
        Some("New folder")
    );
    let taken = |n: &str| ["notes.txt", "notes 2.txt"].contains(&n);
    assert_eq!(
        name::numbered("notes.txt", true, taken).as_deref(),
        Some("notes 3.txt")
    );
    let taken = |n: &str| n == ".hidden";
    assert_eq!(
        name::numbered(".hidden", true, taken).as_deref(),
        Some(".hidden 2")
    );
    let taken = |n: &str| n == "a.b";
    assert_eq!(
        name::numbered("a.b", false, taken).as_deref(),
        Some("a.b 2"),
        "a folder has no extension"
    );
    let long = "x".repeat(NAME_MAX);
    assert_eq!(
        name::numbered(&long, false, |n| n == long),
        None,
        "every candidate too long"
    );
    let all = |_: &str| true;
    assert_eq!(name::numbered("a", false, all), None);
}

#[test]
fn the_name_check_is_f0s() {
    assert_eq!(name::check(""), Err(NameError::Empty));
    assert_eq!(name::check(&"x".repeat(NAME_MAX)), Ok(()));
    assert_eq!(
        name::check(&"x".repeat(NAME_MAX + 1)),
        Err(NameError::TooLong)
    );
    // 47 bytes, not 47 characters: 24 two-byte characters are 48 bytes.
    assert_eq!(name::check(&"é".repeat(24)), Err(NameError::TooLong));
    assert_eq!(name::check(&"é".repeat(23)), Ok(()));
    assert_eq!(name::check("a/b"), Err(NameError::Slash));
    assert_eq!(name::check("a\0b"), Err(NameError::Control));
    assert_eq!(name::check("a\nb"), Err(NameError::Control));
    assert_eq!(name::check("."), Err(NameError::Dot));
    assert_eq!(name::check(".."), Err(NameError::Dot));
    assert_eq!(name::check("..."), Ok(()));
    assert_eq!(name::check(".hidden"), Ok(()));
    assert_eq!(name::check(" spaced "), Ok(()));
}

#[test]
fn rename_validates_first_and_refuses_a_taken_name() {
    let mut files = files_with(|fs| {
        fs.ext_file("a.txt", b"A");
        fs.ext_file("b.txt", b"B");
    });
    select(&mut files, "a.txt");
    files.handle(Input::Rename);
    assert_eq!(
        files.mode(),
        &Mode::Naming {
            from: "a.txt".into(),
            text: "a.txt".into()
        }
    );
    let cases: [(&str, Msg); 5] = [
        ("", Msg::FilesNameEmpty),
        ("a/b", Msg::FilesNameSlash),
        ("..", Msg::FilesNameDot),
        ("x\u{7}", Msg::FilesNameControl),
        ("b.txt", Msg::FilesErrExists),
    ];
    for (name, msg) in cases {
        while matches!(files.mode(), Mode::Naming { text, .. } if !text.is_empty()) {
            files.handle(Input::Erase);
        }
        type_name(&mut files, name);
        files.handle(Input::Confirm);
        assert_eq!(note(&files), Some(msg), "{name:?}");
        assert!(
            matches!(files.mode(), Mode::Naming { .. }),
            "{name:?}: still naming"
        );
    }
    // Over 47 bytes.
    type_name(&mut files, &"x".repeat(NAME_MAX));
    files.handle(Input::Confirm);
    assert_eq!(
        files.note().map(|n| n.text(Lang::En)).as_deref(),
        Some("A name can be at most 47 bytes long.")
    );
    assert!(files.fs().log.is_empty(), "no rename reached the volume");
    files.handle(Input::Cancel);
    assert_eq!(files.mode(), &Mode::Browse);
    assert_eq!(names(&files), ["a.txt", "b.txt"]);
}

#[test]
fn rename_moves_the_entry_and_selects_its_new_name() {
    let mut files = files_with(|fs| {
        fs.ext_file("a.txt", b"A");
        fs.ext_file("m.txt", b"M");
    });
    select(&mut files, "a.txt");
    files.handle(Input::Rename);
    files.handle(Input::Erase);
    files.handle(Input::Erase);
    files.handle(Input::Erase);
    files.handle(Input::Erase);
    type_name(&mut files, "z.md");
    let v = view(&files);
    assert_eq!(
        v.dialog,
        Some(Dialog::Rename {
            title: "Rename".into(),
            field: "az.md".into(),
            hint: "Type a name. Enter to save, Esc to cancel.".into(),
        })
    );
    assert!(v.hints.is_empty());
    files.handle(Input::Confirm);
    assert_eq!(files.mode(), &Mode::Browse);
    assert_eq!(names(&files), ["az.md", "m.txt"]);
    assert_eq!(files.selected_name(), Some("az.md"));
    assert_eq!(files.fs().bytes("/az.md"), Some(&b"A"[..]));
    // The same name: nothing happens.
    files.handle(Input::Rename);
    files.handle(Input::Confirm);
    assert_eq!(files.fs().log.len(), 1);
}

#[test]
fn rename_errors_from_the_volume_are_shown_and_the_folder_relisted() {
    let mut files = files_with(|fs| {
        fs.ext_file("hello.txt", b"");
        fs.busy.push(p("hello.txt"));
    });
    files.handle(Input::Rename);
    type_name(&mut files, "2");
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrBusy));
    assert_eq!(files.mode(), &Mode::Browse);
    assert_eq!(files.selected_name(), Some("hello.txt"));
    // A name taken since the listing: F0 refuses it.
    files.fs_mut().ext_file("hello.txt2", b"");
    files.fs_mut().busy.clear();
    files.handle(Input::First);
    // The listing now has it, so Files refuses before the call...
    files.handle(Input::Rename);
    type_name(&mut files, "2");
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrExists));
    files.handle(Input::Cancel);
    // ...and when it doesn't (taken after the listing), F0 does.
    files.fs_mut().ext_file("late", b"");
    select(&mut files, "hello.txt");
    files.handle(Input::Rename);
    for _ in 0.."hello.txt".len() {
        files.handle(Input::Erase);
    }
    type_name(&mut files, "late");
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrExists));
    assert!(files.rows().iter().any(|r| r.name == "late"), "relisted");
}

#[test]
fn delete_asks_once_and_any_other_key_goes_back() {
    let mut files = files_with(|fs| {
        fs.ext_file("a.txt", b"A");
        fs.ext_file("b.txt", b"B");
    });
    files.handle(Input::Delete);
    let v = view(&files);
    assert_eq!(
        v.dialog,
        Some(Dialog::ConfirmDelete {
            question: "Delete this file? This can't be undone.".into(),
            name: "a.txt".into(),
            hint: "Enter to delete; any other key goes back.".into(),
        })
    );
    files.handle(Input::Down);
    assert_eq!(files.mode(), &Mode::Browse);
    assert_eq!(
        files.selected_name(),
        Some("a.txt"),
        "the key only closed the question"
    );
    assert!(files.fs().exists("/a.txt"));
    files.handle(Input::Delete);
    files.handle(Input::Confirm);
    assert!(!files.fs().exists("/a.txt"));
    assert_eq!(names(&files), ["b.txt"]);
    assert_eq!(files.selected_name(), Some("b.txt"));
}

#[test]
fn a_non_empty_folder_is_refused_without_asking() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("full");
        fs.ext_file("full/x", b"");
        fs.ext_mkdir("hollow");
    });
    files.handle(Input::Delete);
    assert_eq!(files.mode(), &Mode::Browse);
    assert_eq!(note(&files), Some(Msg::FilesErrNotEmpty));
    assert_eq!(
        files.note().unwrap().text(Lang::Es),
        "Esa carpeta no está vacía. Vacíala primero."
    );
    select(&mut files, "hollow");
    files.handle(Input::Delete);
    assert!(matches!(
        view(&files).dialog,
        Some(Dialog::ConfirmDelete { question, .. }) if question == "Delete this empty folder?"
    ));
    // Filled by someone else before the answer: F0 refuses it.
    files.fs_mut().ext_file("hollow/late", b"");
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrNotEmpty));
    assert!(files.fs().exists("/hollow/late"));
    assert_eq!(files.rows()[at(&files, "hollow")].size, 1, "relisted");
}

#[test]
fn models_and_busy_entries_are_refused() {
    let mut files = files_with(|fs| {
        fs.ext_node("model.neom", Kind::Model, b"m");
        fs.ext_file("hello.txt", b"");
        fs.busy.push(p("hello.txt"));
    });
    select(&mut files, "model.neom");
    files.handle(Input::Delete);
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrRefused));
    select(&mut files, "hello.txt");
    files.handle(Input::Delete);
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrBusy));
    assert_eq!(names(&files), ["hello.txt", "model.neom"]);
}

#[test]
fn small_copy_pastes_into_another_folder() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("dst");
        fs.ext_file("note.txt", b"hello");
    });
    select(&mut files, "note.txt");
    files.handle(Input::Copy);
    assert_eq!(note(&files), Some(Msg::FilesCopied));
    assert_eq!(files.clipboard(), Some(&p("note.txt")[..]));
    select(&mut files, "dst");
    files.handle(Input::Open);
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesPasted));
    assert_eq!(files.fs().bytes("/dst/note.txt"), Some(&b"hello"[..]));
    assert_eq!(files.selected_name(), Some("note.txt"));
    // Again in the same folder: a numbered name.
    files.handle(Input::Paste);
    assert_eq!(files.selected_name(), Some("note 2.txt"));
    assert_eq!(files.fs().bytes("/dst/note 2.txt"), Some(&b"hello"[..]));
    assert_eq!(files.fs().open_handles(), 1, "every file handle closed");
}

#[test]
fn copy_is_for_files_up_to_32_kib() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("dir");
        fs.ext_file("big", &vec![7; COPY_MAX + 1]);
        fs.ext_file("edge", &vec![7; COPY_MAX]);
        fs.ext_node("m.neom", Kind::Model, b"");
    });
    files.handle(Input::Copy);
    assert_eq!(note(&files), Some(Msg::FilesCopyNotFile));
    select(&mut files, "m.neom");
    files.handle(Input::Copy);
    assert_eq!(note(&files), Some(Msg::FilesCopyNotFile));
    select(&mut files, "big");
    files.handle(Input::Copy);
    assert_eq!(
        files.note().unwrap().text(Lang::En),
        "Only files up to 32 KB copy for now. Nothing was copied."
    );
    assert_eq!(files.clipboard(), None);
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesNothingToPaste));
    select(&mut files, "edge");
    files.handle(Input::Copy);
    files.handle(Input::Paste);
    assert_eq!(files.fs().bytes("/edge 2"), Some(&vec![7; COPY_MAX][..]));
}

#[test]
fn a_source_that_grew_or_went_away_copies_nothing() {
    let mut files = files_with(|fs| fs.ext_file("s.txt", b"small"));
    files.handle(Input::Copy);
    files.fs_mut().ext_remove("s.txt");
    files.fs_mut().ext_file("s.txt", &vec![1; COPY_MAX + 5]);
    let before = files.fs().snapshot();
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesCopyTooBig));
    assert_eq!(files.fs().snapshot(), before);
    files.fs_mut().ext_remove("s.txt");
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesErrNotFound));
    assert_eq!(files.fs().open_handles(), 1);
}

#[test]
fn a_copy_that_runs_out_of_space_leaves_no_half_copy() {
    let mut files = files_with(|fs| fs.ext_file("s.txt", &vec![3; 9000]));
    files.fs_mut().space = 5000;
    files.handle(Input::Copy);
    files.handle(Input::Paste);
    assert_eq!(note(&files), Some(Msg::FilesErrNoSpace));
    assert_eq!(names(&files), ["s.txt"]);
    assert_eq!(files.fs().open_handles(), 1);
}

// ---- 5. Open with ----

#[test]
fn open_with_maps_text_to_the_editor_and_models_to_nothing() {
    for ext in TEXT_EXTENSIONS {
        assert_eq!(
            open_with(&format!("a.{ext}"), Kind::Regular),
            OpenWith::Editor,
            "{ext}"
        );
        let upper = ext.to_ascii_uppercase();
        assert_eq!(
            open_with(&format!("a.{upper}"), Kind::Regular),
            OpenWith::Editor,
            "{upper}"
        );
    }
    assert_eq!(open_with("model.neom", Kind::Regular), OpenWith::Model);
    assert_eq!(open_with("weights", Kind::Model), OpenWith::Model);
    assert_eq!(open_with("photo.png", Kind::Regular), OpenWith::Nothing);
    assert_eq!(
        open_with("README", Kind::Regular),
        OpenWith::Nothing,
        "no extension"
    );
    assert_eq!(
        open_with(".txt", Kind::Regular),
        OpenWith::Nothing,
        "a dot file has no extension"
    );
    assert_eq!(open_with("a.txt", Kind::Symlink), OpenWith::Nothing);
    assert_eq!(
        open_with("a.txt.bak", Kind::Regular),
        OpenWith::Nothing,
        "the last extension counts"
    );
    assert_eq!(MODULE_EDITOR, 7);
}

#[test]
fn opening_a_file_returns_the_request_and_holds_nothing_for_it() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("docs");
        fs.ext_file("docs/plan.md", b"# plan");
        fs.ext_file("docs/pic.png", b"");
        fs.ext_node("docs/m.neom", Kind::Model, b"");
    });
    files.handle(Input::Open);
    select(&mut files, "plan.md");
    let before = files.fs().open_handles();
    let effect = files.handle(Input::Open);
    assert_eq!(
        effect,
        Effect::OpenWith(OpenRequest {
            path: p("docs/plan.md"),
            module_id: MODULE_EDITOR,
            rights: EDITOR_RIGHTS,
        })
    );
    assert_eq!(
        files.fs().open_handles(),
        before,
        "Files opened nothing for the target"
    );
    select(&mut files, "pic.png");
    assert_eq!(files.handle(Input::Open), Effect::None);
    assert_eq!(files.note().unwrap().text(Lang::En), "No app opens this.");
    select(&mut files, "m.neom");
    assert_eq!(files.handle(Input::Open), Effect::None);
    assert_eq!(
        files.note().unwrap().text(Lang::Es),
        "Los modelos son de Arthur; ninguna app los abre."
    );
}

// ---- 6. The path binds the handle (Muse on #106, observation 1) ----

#[test]
fn rename_away_falls_back_to_the_nearest_ancestor() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_mkdir("a/b");
        fs.ext_file("a/b/x.txt", b"x");
    });
    files.handle(Input::Open);
    files.handle(Input::Open);
    assert_eq!(files.path(), p("a/b"));
    files.fs_mut().ext_rename("/a/b", "/a/c");
    files.handle(Input::Refresh);
    assert_eq!(files.path(), p("a"), "fell back to the parent");
    assert_eq!(names(&files), ["c"]);
    assert_eq!(note(&files), Some(Msg::FilesErrStale));
    assert_eq!(
        view(&files).message.as_deref(),
        Some("That folder moved or was removed. This is the nearest one above it.")
    );
    assert_eq!(files.fs().open_handles(), 1);
    // Any operation finds it the same way, not just a refresh.
    files.handle(Input::Open);
    files.fs_mut().ext_rename("/a/c", "/a/d");
    files.handle(Input::NewFolder);
    assert_eq!(files.path(), p("a"));
    assert_eq!(note(&files), Some(Msg::FilesErrStale));
    assert!(
        !files.fs().exists("/a/New folder"),
        "nothing made in the parent"
    );
    assert!(files.fs().exists("/a/d/x.txt"));
}

#[test]
fn rename_away_then_a_new_folder_at_the_old_path_is_never_shown_the_old_listing() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_mkdir("a/b");
        fs.ext_file("a/b/x.txt", b"precious");
    });
    files.handle(Input::Open);
    files.handle(Input::Open);
    select(&mut files, "x.txt");
    files.handle(Input::Delete);
    // While the question is up, another process moves the folder away and
    // makes a new one at the old path. The handle now names the new folder.
    files.fs_mut().ext_rename("/a/b", "/a/c");
    files.fs_mut().ext_mkdir("/a/b");
    files.fs_mut().ext_file("/a/b/x.txt", b"other");
    files.handle(Input::Confirm);
    // The delete acted on the folder now at the path: its own x.txt. The
    // moved folder's file is untouched.
    assert_eq!(files.fs().bytes("/a/c/x.txt"), Some(&b"precious"[..]));
    assert!(!files.fs().exists("/a/b/x.txt"));
    // And the listing is the new folder's, read after the operation.
    assert_eq!(files.path(), p("a/b"));
    assert!(files.rows().is_empty());
    // Without a same-named file there, the delete finds nothing and says so.
    files.fs_mut().ext_file("/a/b/y.txt", b"");
    files.handle(Input::Refresh);
    files.handle(Input::Delete);
    files.fs_mut().ext_rename("/a/b", "/a/e");
    files.fs_mut().ext_mkdir("/a/b");
    files.handle(Input::Confirm);
    assert_eq!(note(&files), Some(Msg::FilesErrNotFound));
    assert!(files.fs().exists("/a/e/y.txt"));
    assert!(
        files.rows().is_empty(),
        "the new folder's listing, not the cached one"
    );
}

#[test]
fn an_ancestor_rename_falls_back_to_the_root() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_mkdir("a/b");
        fs.ext_mkdir("a/b/c");
        fs.ext_file("top.txt", b"");
    });
    for _ in 0..3 {
        files.handle(Input::Open);
    }
    assert_eq!(files.path(), p("a/b/c"));
    files.fs_mut().ext_rename("/a", "/z");
    files.handle(Input::Refresh);
    assert!(
        files.path().is_empty(),
        "every ancestor below the root is gone"
    );
    assert_eq!(names(&files), ["z", "top.txt"]);
    assert_eq!(note(&files), Some(Msg::FilesErrStale));
    assert_eq!(files.fs().open_handles(), 1);
    // A folder replaced by a file at its path is stale too (ENOTDIR).
    files.handle(Input::Open);
    files.fs_mut().ext_remove("/z");
    files.fs_mut().ext_file("/z", b"");
    files.handle(Input::Refresh);
    assert!(files.path().is_empty());
    assert_eq!(note(&files), Some(Msg::FilesErrStale));
}

#[test]
fn files_own_rename_reopens_the_folder_by_its_new_name() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_mkdir("a/b");
        fs.ext_file("a/b/in.txt", b"");
    });
    files.handle(Input::Open);
    files.handle(Input::Rename);
    files.handle(Input::Erase);
    type_name(&mut files, "e");
    files.handle(Input::Confirm);
    assert_eq!(files.selected_name(), Some("e"));
    files.handle(Input::Open);
    assert_eq!(files.path(), p("a/e"));
    assert_eq!(names(&files), ["in.txt"]);
    assert_eq!(files.fs().open_handles(), 1);
}

#[test]
fn a_rename_that_committed_but_replied_stale_is_read_back() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("a");
        fs.ext_file("a/old.txt", b"");
        fs.stale_after_rename = true;
    });
    files.handle(Input::Open);
    files.handle(Input::Rename);
    for _ in 0..7 {
        files.handle(Input::Erase);
    }
    type_name(&mut files, "new.txt");
    files.handle(Input::Confirm);
    assert_eq!(files.path(), p("a"), "the folder reopened by its path");
    assert_eq!(names(&files), ["new.txt"]);
    assert_eq!(files.selected_name(), Some("new.txt"));
    assert_eq!(note(&files), None, "it committed: no error");
    assert_eq!(files.fs().open_handles(), 1);
    // A stale reply that did not commit (the folder itself went away).
    files.fs_mut().stale_after_rename = false;
    files.handle(Input::Rename);
    type_name(&mut files, "2");
    files.fs_mut().ext_rename("/a", "/gone");
    files.handle(Input::Confirm);
    assert!(files.path().is_empty());
    assert_eq!(note(&files), Some(Msg::FilesErrStale));
    assert!(files.fs().exists("/gone/new.txt"));
}

// ---- 7. The render model ----

#[test]
fn sizes_read_as_the_column_shows_them() {
    let cases: [(u64, &str, &str); 9] = [
        (0, "0 B", "0 B"),
        (1023, "1023 B", "1023 B"),
        (1024, "1.0 KB", "1,0 KB"),
        (1536, "1.5 KB", "1,5 KB"),
        (10 * 1024 - 52, "9.9 KB", "9,9 KB"),
        (10 * 1024 - 51, "10 KB", "10 KB"),
        (32 * 1024, "32 KB", "32 KB"),
        (1024 * 1024, "1.0 MB", "1,0 MB"),
        (25 * 1024 * 1024, "25 MB", "25 MB"),
    ];
    for (bytes, en, es) in cases {
        assert_eq!(size_text(Lang::En, bytes), en, "{bytes}");
        assert_eq!(size_text(Lang::Es, bytes), es, "{bytes}");
    }
    // Just under 1 MiB rounds to 1.0 MB, not "1024 KB".
    assert_eq!(size_text(Lang::En, 1024 * 1024 - 1), "1.0 MB");
    assert_eq!(size_text(Lang::En, 1024 * 1024 - 513), "1023 KB");
}

#[test]
fn the_view_is_plain_data_in_the_sessions_language() {
    let mut files = files_with(|fs| {
        fs.ext_mkdir("d");
        fs.ext_file("d/x", b"");
        fs.ext_file("f", b"");
    });
    let v = view(&files);
    assert_eq!(
        v.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        ["d/", "f"]
    );
    assert_eq!(v.rows[0].size, "1 item");
    assert!(v.rows[0].folder);
    assert_eq!(v.selected, Some(0));
    assert_eq!(v.footer, None);
    assert_eq!(v.dialog, None);
    assert_eq!(v.message, None);
    assert_eq!(
        v.hints,
        [
            "Enter opens. Backspace goes up. N makes a folder. F2 renames.",
            "Del deletes. C copies. V pastes."
        ]
    );
    files.set_lang(Lang::Es);
    files.handle(Input::Delete);
    let v = view(&files);
    assert_eq!(v.title, "Archivos");
    assert_eq!(v.rows[0].size, "1 elemento");
    assert_eq!(
        v.message.as_deref(),
        Some("Esa carpeta no está vacía. Vacíala primero.")
    );
    select(&mut files, "f");
    files.handle(Input::Delete);
    assert!(matches!(
        view(&files).dialog,
        Some(Dialog::ConfirmDelete { question, hint, .. })
            if question == "¿Eliminar este archivo? No se puede deshacer."
                && hint == "Enter para eliminar; cualquier otra tecla vuelve atrás."
    ));
    assert_eq!(location(&p("a/b")), "/a/b");
}

#[test]
fn every_name_message_has_its_own_row() {
    let all = [
        NameError::Empty,
        NameError::TooLong,
        NameError::Slash,
        NameError::Control,
        NameError::Dot,
    ];
    let msgs: Vec<Msg> = all.iter().map(|&e| name_note(e).msg).collect();
    for (i, m) in msgs.iter().enumerate() {
        assert!(!msgs[i + 1..].contains(m));
    }
    assert_eq!(
        Note {
            msg: Msg::FilesNameTooLong,
            arg: Some("47".into())
        }
        .text(Lang::Es),
        "Un nombre puede tener como máximo 47 bytes."
    );
}

// ---- 8. A randomized sequence against a model of the tree ----

/// A small, seeded generator (xorshift64*), so a failure replays.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

type Tree = BTreeMap<String, (Kind, Vec<u8>)>;

fn join(dir: &str, name: &str) -> String {
    format!("{dir}/{name}")
}

fn kids<'a>(tree: &'a Tree, dir: &str) -> Vec<(&'a str, &'a (Kind, Vec<u8>))> {
    let prefix = format!("{dir}/");
    tree.iter()
        .filter_map(|(k, v)| {
            let rest = k.strip_prefix(&prefix)?;
            (!rest.contains('/')).then_some((rest, v))
        })
        .collect()
}

fn is_dir(tree: &Tree, dir: &str) -> bool {
    dir.is_empty() || matches!(tree.get(dir), Some((Kind::Directory, _)))
}

/// Move `from` (and everything under it) to `to`.
fn move_subtree(tree: &mut Tree, from: &str, to: &str) {
    let moved: Vec<String> = tree
        .keys()
        .filter(|k| *k == from || k.starts_with(&format!("{from}/")))
        .cloned()
        .collect();
    for k in moved {
        let v = tree.remove(&k).unwrap();
        tree.insert(format!("{to}{}", &k[from.len()..]), v);
    }
}

/// What a Files input should do to the tree, from the tree before it (the
/// volume's truth) and what Files shows (its listing, its mode). Written
/// from the rules, not from `files.rs`.
fn predict(files: &Files<FakeFs>, tree: &Tree, input: Input) -> Tree {
    let mut out = tree.clone();
    let here = location(files.path());
    let here = if here == "/" { String::new() } else { here };
    if !files.available() || !is_dir(tree, &here) {
        return out; // A stale folder: the operation falls back, changes nothing.
    }
    let listed = |n: &str| files.rows().iter().any(|r| r.name == n);
    let entries = kids(tree, &here);
    let truth = |n: &str| entries.iter().any(|(k, _)| *k == n);
    let room = entries.len() < FOLDER_MAX;
    match (files.mode().clone(), input) {
        (Mode::Browse, Input::NewFolder) if !files.full() => {
            let name = name::numbered("New folder", false, listed).unwrap();
            if !truth(&name) && room {
                out.insert(join(&here, &name), (Kind::Directory, Vec::new()));
            }
        }
        (Mode::Browse, Input::Paste) if !files.full() => {
            if let Some(src) = files.clipboard() {
                let src_path = location(src);
                if let Some((Kind::Regular, bytes)) = tree.get(&src_path) {
                    let name = name::numbered(src.last().unwrap(), true, listed).unwrap();
                    if bytes.len() <= COPY_MAX && !truth(&name) && room {
                        out.insert(join(&here, &name), (Kind::Regular, bytes.clone()));
                    }
                }
            }
        }
        (Mode::ConfirmDelete { name, .. }, Input::Confirm | Input::Open) => {
            let path = join(&here, &name);
            let removable = match tree.get(&path) {
                Some((Kind::Regular, _)) => true,
                Some((Kind::Directory, _)) => kids(tree, &path).is_empty(),
                _ => false,
            };
            if removable {
                out.remove(&path);
            }
        }
        (Mode::Naming { from, text }, Input::Confirm | Input::Open)
            if name::check(&text).is_ok()
                && text != from
                && !listed(&text)
                && truth(&from)
                && !truth(&text) =>
        {
            move_subtree(&mut out, &join(&here, &from), &join(&here, &text));
        }
        _ => {}
    }
    out
}

/// Whether `input` ends with a fresh listing (by the rules: every operation
/// and every move between folders re-lists; a selection move, a dialog key
/// or a message-only outcome doesn't).
fn relists(files: &Files<FakeFs>, input: Input) -> bool {
    let sel_dir = files
        .selected()
        .is_some_and(|i| files.rows()[i].kind == Kind::Directory);
    match (files.mode(), input) {
        (Mode::Browse, Input::Refresh | Input::NewFolder | Input::Paste) => true,
        (Mode::Browse, Input::Parent) => !files.path().is_empty(),
        (Mode::Browse, Input::Open | Input::Confirm) => sel_dir,
        (Mode::ConfirmDelete { .. }, Input::Confirm | Input::Open) => true,
        (Mode::Naming { from, text }, Input::Confirm | Input::Open) => {
            name::check(text).is_ok()
                && text != from
                && !files.rows().iter().any(|r| &r.name == text)
        }
        _ => false,
    }
}

const NAMES: [&str; 7] = ["a", "b", "New folder", "New folder 2", "x.txt", "y.md", "z"];

/// Another process's change, at random, applied to the volume.
fn external(rng: &mut Rng, fs: &mut FakeFs) {
    let tree = fs.snapshot();
    let dirs: Vec<String> = core::iter::once(String::new())
        .chain(
            tree.iter()
                .filter(|(_, v)| v.0 == Kind::Directory)
                .map(|(k, _)| k.clone()),
        )
        .collect();
    let dir = rng.pick(&dirs).clone();
    let name = *rng.pick(&NAMES);
    let path = join(&dir, name);
    let free = !tree.contains_key(&path) && kids(&tree, &dir).len() < FOLDER_MAX;
    match rng.below(4) {
        0 if free => fs.ext_mkdir(&path),
        1 if free => {
            let len = rng.below(3) * 20_000;
            fs.ext_file(&path, &vec![rng.below(250) as u8; len]);
        }
        2 if tree.contains_key(&path) => {
            fs.ext_remove(&path);
        }
        3 if tree.contains_key(&path) => {
            let to_dir = rng.pick(&dirs).clone();
            let to = join(&to_dir, rng.pick(&NAMES));
            let into_itself = to_dir == path || to_dir.starts_with(&format!("{path}/"));
            if !tree.contains_key(&to) && !into_itself {
                fs.ext_rename(&path, &to);
            }
        }
        _ => {}
    }
}

fn random_input(rng: &mut Rng) -> Input {
    let inputs = [
        Input::Up,
        Input::Down,
        Input::Open,
        Input::Open,
        Input::Parent,
        Input::NewFolder,
        Input::Rename,
        Input::Delete,
        Input::Confirm,
        Input::Confirm,
        Input::Copy,
        Input::Paste,
        Input::Refresh,
        Input::Erase,
        Input::Cancel,
    ];
    if rng.below(5) == 0 {
        Input::Char(*rng.pick(&['a', 'b', '2', '.', '/', ' ']))
    } else {
        *rng.pick(&inputs)
    }
}

#[test]
fn a_random_operation_sequence_matches_the_model() {
    // What the run covered, so a generator that stops reaching the
    // operations fails here rather than passing quietly.
    let (mut changed, mut stale, mut opened, mut checked) = (0, 0, 0, 0);
    for seed in 1..=40u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut fs = FakeFs::new();
        fs.batch = 1 + rng.below(5);
        fs.ext_mkdir("a");
        fs.ext_file("x.txt", b"seed");
        let mut files = Files::new(fs, Lang::En);
        for step in 0..400 {
            if rng.below(6) == 0 {
                external(&mut rng, files.fs_mut());
            }
            let input = random_input(&mut rng);
            let before = files.fs().snapshot();
            let want = predict(&files, &before, input);
            let relists = relists(&files, input);
            let effect = files.handle(input);
            let got = files.fs().snapshot();
            let ctx = format!("seed {seed} step {step} {input:?} at {:?}", files.path());
            assert_eq!(got, want, "{ctx}: the volume differs from the model");
            changed += usize::from(got != before);
            stale += usize::from(files.note().map(|n| n.msg) == Some(Msg::FilesErrStale));
            opened += usize::from(matches!(effect, Effect::OpenWith(_)));
            // After every input that ends an operation or moves, what Files
            // shows is the folder's truth, read after the input.
            let quiet = matches!(
                files.note().map(|n| n.msg),
                Some(Msg::FilesNothingToPaste | Msg::FilesErrFolderFull)
            );
            if files.available() && relists && !quiet {
                let here = location(files.path());
                let here = if here == "/" { String::new() } else { here };
                assert!(
                    is_dir(&got, &here),
                    "{ctx}: Files is in a folder that isn't one"
                );
                let mut truth: Vec<(String, Kind)> = kids(&got, &here)
                    .into_iter()
                    .map(|(n, v)| (String::from(n), v.0))
                    .collect();
                truth.sort_by(|a, b| {
                    (a.1 != Kind::Directory)
                        .cmp(&(b.1 != Kind::Directory))
                        .then_with(|| name::order(&a.0, &b.0))
                });
                let shown: Vec<(String, Kind)> = files
                    .rows()
                    .iter()
                    .map(|r| (r.name.clone(), r.kind))
                    .collect();
                assert_eq!(shown, truth, "{ctx}: the listing is not the folder's");
                checked += 1;
            }
            if let Some(i) = files.selected() {
                assert!(i < files.rows().len(), "{ctx}: selection past the rows");
            } else {
                assert!(files.rows().is_empty(), "{ctx}: rows but no selection");
            }
            assert!(files.rows().len() <= FOLDER_MAX);
            assert!(files.fs().open_handles() <= 1, "{ctx}: a handle leaked");
            if let Effect::OpenWith(req) = effect {
                assert_eq!(req.module_id, MODULE_EDITOR, "{ctx}");
                // What Files lists (another process may have removed it
                // since; F1b's open then fails and says so).
                assert_eq!(&req.path[..req.path.len() - 1], files.path(), "{ctx}");
                assert!(
                    files
                        .rows()
                        .iter()
                        .any(|r| Some(&r.name) == req.path.last()),
                    "{ctx}: opens what is listed"
                );
            }
        }
    }
    std::eprintln!("changed {changed}, stale {stale}, opened {opened}, listings checked {checked}");
    assert!(changed > 500 && stale > 20 && opened > 20 && checked > 2000);
}
