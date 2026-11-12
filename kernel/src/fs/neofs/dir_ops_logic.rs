// SPDX-License-Identifier: MPL-2.0
//! F0 (`neofs-dir-ops`): the rules of the four directory operations, pure.
//! The kernel (`inode/dir_ops.rs`) and the host model
//! (`tools/neofs-dir-ops-logic-tests`, which mounts this file through
//! `#[path]`) use the same functions, so the planner the model cuts power
//! under is the kernel's.
//!
//! - A name is 1 to 47 bytes of UTF-8 (the format's directory entry), with
//!   no `/` or NUL, and is not `.` or `..`.
//! - A tree is at most [`MAX_DEPTH`] directories deep: mount marks the live
//!   set to that depth and refuses a volume past it.
//! - The root's `hello.txt` and `model.neom` are pinned: mount refuses a
//!   volume without them, so no operation removes or renames them.
//! - A rename never replaces: the destination name must be free. It can't
//!   move a directory into itself or beneath itself.
//! - `stat_at`'s reply is [`StatAtV1`], 16 bytes.

use alloc::string::String;
use alloc::vec::Vec;

/// The longest name a directory entry holds (bytes 0..48, NUL-terminated).
pub const NAME_MAX: usize = 47;

/// How deep mount walks the tree (`mark_inode_tree` refuses depth > 16).
/// The root is depth 0, its entries depth 1; nothing may sit deeper than
/// this.
pub const MAX_DEPTH: usize = 16;

/// The root entries mount requires (`validate_children`).
pub const PINNED_AT_ROOT: [&str; 2] = ["hello.txt", "model.neom"];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Slash,
    Nul,
    Dot,
    NotUtf8,
}

/// `name` as a directory entry's name, or why it can't be one.
pub fn check_name(name: &[u8]) -> Result<&str, NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > NAME_MAX {
        return Err(NameError::TooLong);
    }
    if name.contains(&b'/') {
        return Err(NameError::Slash);
    }
    if name.contains(&0) {
        return Err(NameError::Nul);
    }
    if name == b"." || name == b".." {
        return Err(NameError::Dot);
    }
    core::str::from_utf8(name).map_err(|_| NameError::NotUtf8)
}

/// The components of a directory's mount-relative path (`None`: the root).
pub fn components(dir: Option<&str>) -> Vec<&str> {
    match dir {
        None => Vec::new(),
        Some(path) => path.split('/').collect(),
    }
}

/// `name` in the directory at `dir` (`None`: the root), mount-relative.
pub fn join(dir: Option<&str>, name: &str) -> String {
    match dir {
        None => String::from(name),
        Some(path) => {
            let mut out = String::with_capacity(path.len() + 1 + name.len());
            out.push_str(path);
            out.push('/');
            out.push_str(name);
            out
        }
    }
}

/// Is `name` in the directory `dir` one mount requires?
pub fn is_pinned(dir: &[&str], name: &str) -> bool {
    dir.is_empty() && PINNED_AT_ROOT.contains(&name)
}

/// Does an entry whose subtree is `height` directories tall (0: a file or
/// an empty directory) fit in the directory `dir`? The entry sits at depth
/// `dir.len() + 1` and its deepest descendant at that plus `height`.
pub fn depth_fits(dir: &[&str], height: usize) -> bool {
    dir.len() + 1 + height <= MAX_DEPTH
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenameRefusal {
    /// The destination is the source, or beneath it.
    IntoItself,
}

/// Check a rename of `src_name` in `src_dir` to `dst_name` in `dst_dir`
/// (component lists from the root), and return the length of the two
/// parents' common prefix: the directories both chains share, which the
/// commit rewrites once.
pub fn rename_check(
    src_dir: &[&str],
    src_name: &str,
    dst_dir: &[&str],
    dst_name: &str,
) -> Result<usize, RenameRefusal> {
    // The source as a path: the destination may not be it or beneath it.
    // Moving a file "into" itself is impossible (a file has no entries), but
    // the check is on names, so it refuses that too, before any lookup.
    let src_len = src_dir.len() + 1;
    let dst_len = dst_dir.len() + 1;
    let src_at = |i: usize| {
        if i < src_dir.len() {
            src_dir[i]
        } else {
            src_name
        }
    };
    let dst_at = |i: usize| {
        if i < dst_dir.len() {
            dst_dir[i]
        } else {
            dst_name
        }
    };
    if dst_len >= src_len && (0..src_len).all(|i| src_at(i) == dst_at(i)) {
        return Err(RenameRefusal::IntoItself);
    }
    Ok(src_dir
        .iter()
        .zip(dst_dir.iter())
        .take_while(|(a, b)| a == b)
        .count())
}

/// Remove the entry named `name`, keeping the others' order. `None` when
/// there is none.
pub fn remove_named<E>(
    entries: &mut Vec<E>,
    name: &str,
    name_of: impl Fn(&E) -> &str,
) -> Option<E> {
    let at = entries.iter().position(|e| name_of(e) == name)?;
    Some(entries.remove(at))
}

/// The ABI kind numbers, as `NeoDirentV1` (`SYS_READDIR`) uses them.
pub const KIND_DIRECTORY: u32 = 1;
pub const KIND_REGULAR: u32 = 2;
pub const KIND_SYMLINK: u32 = 3;
pub const KIND_MODEL: u32 = 4;

/// `StatAtV1.flags`: a directory with no entries (the one `unlink_at`
/// removes).
pub const STAT_EMPTY_DIRECTORY: u32 = 1 << 0;

/// `SYS_STAT_AT`'s reply: 16 bytes, little-endian.
///
/// | Bytes | Field | |
/// |---|---|---|
/// | 0..4 | `kind` | [`KIND_DIRECTORY`] .. [`KIND_MODEL`] |
/// | 4..8 | `flags` | [`STAT_EMPTY_DIRECTORY`]; other bits zero |
/// | 8..16 | `size` | a file's bytes; a directory's entry count |
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StatAtV1 {
    pub kind: u32,
    pub flags: u32,
    pub size: u64,
}

pub const STAT_AT_V1_LEN: usize = 16;

impl StatAtV1 {
    pub fn encode(&self) -> [u8; STAT_AT_V1_LEN] {
        let mut out = [0u8; STAT_AT_V1_LEN];
        out[0..4].copy_from_slice(&self.kind.to_le_bytes());
        out[4..8].copy_from_slice(&self.flags.to_le_bytes());
        out[8..16].copy_from_slice(&self.size.to_le_bytes());
        out
    }

    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != STAT_AT_V1_LEN {
            return None;
        }
        Some(Self {
            kind: u32::from_le_bytes(bytes[0..4].try_into().ok()?),
            flags: u32::from_le_bytes(bytes[4..8].try_into().ok()?),
            size: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        })
    }
}
