// SPDX-License-Identifier: MPL-2.0
//! The rules, case by case, and `StatAtV1` against hand-laid bytes: the
//! kernel's encoder and libneo's decoder, each written on its own.

use crate::dir_ops_logic::{
    self as logic, NameError, RenameRefusal, STAT_AT_V1_LEN, STAT_EMPTY_DIRECTORY, StatAtV1,
};
use crate::libneo_stat_at::{self as libneo, Kind, StatAt};

#[test]
fn names_follow_the_directory_entry() {
    assert_eq!(logic::check_name(b"notes.txt"), Ok("notes.txt"));
    assert_eq!(logic::check_name("año".as_bytes()), Ok("año"));
    assert_eq!(logic::check_name(&[b'n'; 47]).map(str::len), Ok(47));
    assert_eq!(logic::check_name(&[b'n'; 48]), Err(NameError::TooLong));
    assert_eq!(logic::check_name(b""), Err(NameError::Empty));
    assert_eq!(logic::check_name(b"a/b"), Err(NameError::Slash));
    assert_eq!(logic::check_name(b"/"), Err(NameError::Slash));
    assert_eq!(logic::check_name(b"a\0"), Err(NameError::Nul));
    assert_eq!(logic::check_name(b"."), Err(NameError::Dot));
    assert_eq!(logic::check_name(b".."), Err(NameError::Dot));
    assert_eq!(logic::check_name(b"..."), Ok("..."));
    assert_eq!(logic::check_name(b".hidden"), Ok(".hidden"));
    assert_eq!(logic::check_name(&[0xff, 0xfe]), Err(NameError::NotUtf8));
    // 47 bytes of UTF-8 that ends mid-character is not a name.
    let mut cut = [b'n'; 47];
    cut[46] = 0xc3;
    assert_eq!(logic::check_name(&cut), Err(NameError::NotUtf8));
}

#[test]
fn paths_join_from_the_root() {
    assert_eq!(logic::join(None, "a"), "a");
    assert_eq!(logic::join(Some("a/b"), "c"), "a/b/c");
    assert!(logic::components(None).is_empty());
    assert_eq!(logic::components(Some("a/b")), ["a", "b"]);
}

#[test]
fn only_the_roots_required_entries_are_pinned() {
    assert!(logic::is_pinned(&[], "hello.txt"));
    assert!(logic::is_pinned(&[], "model.neom"));
    assert!(!logic::is_pinned(&["a"], "hello.txt"));
    assert!(!logic::is_pinned(&[], "subdir"));
}

#[test]
fn depth_leaves_room_for_the_entries_below() {
    let fifteen = ["d"; 15];
    let fourteen = ["d"; 14];
    // A file at depth 16 is the deepest mount accepts.
    assert!(logic::depth_fits(&fifteen, 0));
    assert!(!logic::depth_fits(&["d"; 16], 0));
    // An empty directory (height 1) may sit at depth 15, not 16.
    assert!(logic::depth_fits(&fourteen, 1));
    assert!(!logic::depth_fits(&fifteen, 1));
    assert!(logic::depth_fits(&[], logic::MAX_DEPTH - 1));
    assert!(!logic::depth_fits(&[], logic::MAX_DEPTH));
}

#[test]
fn a_rename_never_goes_into_itself() {
    // In place, and across: the length of the shared prefix.
    assert_eq!(logic::rename_check(&["a"], "b", &["a"], "c"), Ok(1));
    assert_eq!(logic::rename_check(&["a", "x"], "n", &["c"], "m"), Ok(0));
    assert_eq!(logic::rename_check(&["a", "b"], "n", &["a"], "n"), Ok(1));
    assert_eq!(logic::rename_check(&[], "c", &["a", "b"], "c"), Ok(0));
    // Onto itself, into itself, beneath itself.
    assert_eq!(
        logic::rename_check(&["a"], "b", &["a"], "b"),
        Err(RenameRefusal::IntoItself)
    );
    assert_eq!(
        logic::rename_check(&[], "a", &["a"], "x"),
        Err(RenameRefusal::IntoItself)
    );
    assert_eq!(
        logic::rename_check(&["p"], "a", &["p", "a", "b"], "x"),
        Err(RenameRefusal::IntoItself)
    );
    // A sibling that shares a prefix of the name is not beneath it.
    assert_eq!(logic::rename_check(&[], "a", &["ab"], "x"), Ok(0));
}

#[test]
fn remove_keeps_the_others_in_order() {
    let mut entries = vec!["a", "b", "c"];
    assert_eq!(logic::remove_named(&mut entries, "b", |e| e), Some("b"));
    assert_eq!(entries, ["a", "c"]);
    assert_eq!(logic::remove_named(&mut entries, "z", |e| e), None);
}

/// The byte oracle: replies laid out here by hand (kind u32 LE at 0, flags
/// u32 LE at 4, size u64 LE at 8), not by either side's code.
#[test]
fn stat_at_v1_matches_hand_laid_bytes() {
    let empty_dir: [u8; 16] = [
        0x01, 0x00, 0x00, 0x00, // kind 1, a directory
        0x01, 0x00, 0x00, 0x00, // flags: empty
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 0 entries
    ];
    let file: [u8; 16] = [
        0x02, 0x00, 0x00, 0x00, // kind 2, a regular file
        0x00, 0x00, 0x00, 0x00, // no flags
        0x34, 0x12, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // 0x1_0000_1234 bytes
    ];
    let model: [u8; 16] = [
        0x04, 0, 0, 0, 0, 0, 0, 0, 0x00, 0x00, 0x00, 0x40, 0, 0, 0, 0,
    ];
    assert_eq!(STAT_AT_V1_LEN, 16);
    assert_eq!(libneo::LEN, 16);
    let cases = [
        (
            empty_dir,
            StatAtV1 {
                kind: logic::KIND_DIRECTORY,
                flags: STAT_EMPTY_DIRECTORY,
                size: 0,
            },
            StatAt {
                kind: Kind::Directory,
                empty_directory: true,
                size: 0,
            },
        ),
        (
            file,
            StatAtV1 {
                kind: logic::KIND_REGULAR,
                flags: 0,
                size: 0x1_0000_1234,
            },
            StatAt {
                kind: Kind::Regular,
                empty_directory: false,
                size: 0x1_0000_1234,
            },
        ),
        (
            model,
            StatAtV1 {
                kind: logic::KIND_MODEL,
                flags: 0,
                size: 0x4000_0000,
            },
            StatAt {
                kind: Kind::Model,
                empty_directory: false,
                size: 0x4000_0000,
            },
        ),
    ];
    for (bytes, kernel, user) in cases {
        assert_eq!(kernel.encode(), bytes);
        assert_eq!(StatAtV1::decode(&bytes), Some(kernel));
        assert_eq!(StatAt::decode(&bytes), Some(user));
    }
    assert_eq!(libneo::FLAG_EMPTY_DIRECTORY, STAT_EMPTY_DIRECTORY);
}

#[test]
fn libneo_refuses_what_it_does_not_know() {
    let mut bytes = [0u8; 16];
    bytes[0] = 2;
    assert!(StatAt::decode(&bytes).is_some());
    assert_eq!(StatAt::decode(&bytes[..15]), None);
    bytes[0] = 5;
    assert_eq!(StatAt::decode(&bytes), None);
    bytes[0] = 0;
    assert_eq!(StatAt::decode(&bytes), None);
    bytes[0] = 2;
    bytes[4] = 0x02;
    assert_eq!(StatAt::decode(&bytes), None);
}
