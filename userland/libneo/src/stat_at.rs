// SPDX-License-Identifier: MPL-2.0
//! F0: decoding the `SYS_STAT_AT` reply, `StatAtV1` (16 bytes; the layout is
//! `kernel/src/fs/neofs/dir_ops_logic.rs`). Written independently of the
//! kernel's encoder; `tools/neofs-dir-ops-logic-tests` checks both against
//! hand-laid bytes.
//!
//! | Bytes | Field |
//! |---|---|
//! | 0..4 | `kind`, u32 LE: 1 directory, 2 regular, 3 symlink, 4 model |
//! | 4..8 | `flags`, u32 LE: bit 0, an empty directory |
//! | 8..16 | `size`, u64 LE: a file's bytes, a directory's entry count |

/// The reply's length.
pub const LEN: usize = 16;

/// `flags` bit 0: a directory with no entries.
pub const FLAG_EMPTY_DIRECTORY: u32 = 1 << 0;

/// What an entry is, by the reply's `kind` (the `NeoDirentV1` numbering).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Directory,
    Regular,
    Symlink,
    Model,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatAt {
    pub kind: Kind,
    /// A directory with no entries: the one `unlink_at` removes.
    pub empty_directory: bool,
    /// A file's length in bytes; a directory's entry count.
    pub size: u64,
}

impl StatAt {
    /// `None` for a reply of the wrong length, an unknown kind, or a flag
    /// bit this version doesn't know.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != LEN {
            return None;
        }
        let word = |at: usize| {
            u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };
        let kind = match word(0) {
            1 => Kind::Directory,
            2 => Kind::Regular,
            3 => Kind::Symlink,
            4 => Kind::Model,
            _ => return None,
        };
        let flags = word(4);
        if flags & !FLAG_EMPTY_DIRECTORY != 0 {
            return None;
        }
        let mut size = [0u8; 8];
        size.copy_from_slice(&bytes[8..16]);
        Some(Self {
            kind,
            empty_directory: flags & FLAG_EMPTY_DIRECTORY != 0,
            size: u64::from_le_bytes(size),
        })
    }
}
