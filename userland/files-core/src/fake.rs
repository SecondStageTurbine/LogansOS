// SPDX-License-Identifier: MPL-2.0
//! An in-memory volume with F0's rules, for the KATs.
//!
//! - **A handle binds its path** and resolves it at every call, as F0's do
//!   (`fs/mod.rs:656-659`): rename a folder away and its handle is stale (a
//!   file at the path: `ENOTDIR`, also stale); make a new folder at the old
//!   path and the handle acts on the new one.
//! - A folder holds at most 64 entries, names are at most 47 bytes, rename
//!   never replaces, `unlink_at` takes only a file or an empty folder, and a
//!   model can't be unlinked (`EISDIR`).
//! - `readdir` returns entries in the order they were made (the disk's, not
//!   sorted), `batch` at a time.
//! - Any call can be made to fail once ([`FakeFs::fail_next`]), and a rename
//!   can report `ESTALE` after it commits ([`FakeFs::stale_after_rename`]),
//!   as Muse's observation 2 describes.

use crate::fs::{COPY_MAX, DirEntry, FOLDER_MAX, Fs, FsError, Kind, NAME_MAX, Rights, Stat};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Dir(Vec<(String, Node)>),
    File { kind: Kind, bytes: Vec<u8> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    OpenDir,
    Readdir,
    Stat,
    Mkdir,
    Unlink,
    Rename,
    OpenFile,
    Read,
    Write,
}

pub struct FakeFs {
    root: Node,
    dirs: Vec<Option<Vec<String>>>,
    files: Vec<Option<(Vec<String>, usize)>>,
    pub batch: usize,
    fails: Vec<(Op, FsError)>,
    pub stale_after_rename: bool,
    /// Bytes the volume can still take (for `NoSpace`).
    pub space: usize,
    /// Paths `unlink_at` and `rename_at` refuse with `EBUSY`.
    pub busy: Vec<Vec<String>>,
    /// Every mutating call made, for KATs that check nothing else happened.
    pub log: Vec<String>,
}

pub fn p(s: &str) -> Vec<String> {
    s.split('/')
        .filter(|c| !c.is_empty())
        .map(String::from)
        .collect()
}

fn children(node: &Node) -> Option<&Vec<(String, Node)>> {
    match node {
        Node::Dir(c) => Some(c),
        Node::File { .. } => None,
    }
}

impl FakeFs {
    pub fn new() -> FakeFs {
        FakeFs {
            root: Node::Dir(Vec::new()),
            dirs: Vec::new(),
            files: Vec::new(),
            batch: 3,
            fails: Vec::new(),
            stale_after_rename: false,
            space: usize::MAX,
            busy: Vec::new(),
            log: Vec::new(),
        }
    }

    pub fn fail_next(&mut self, op: Op, e: FsError) {
        self.fails.push((op, e));
    }

    fn injected(&mut self, op: Op) -> Result<(), FsError> {
        match self.fails.iter().position(|&(o, _)| o == op) {
            Some(i) => Err(self.fails.remove(i).1),
            None => Ok(()),
        }
    }

    fn node(&self, path: &[String]) -> Option<&Node> {
        let mut node = &self.root;
        for c in path {
            node = &children(node)?.iter().find(|(n, _)| n == c)?.1;
        }
        Some(node)
    }

    fn node_mut(&mut self, path: &[String]) -> Option<&mut Node> {
        let mut node = &mut self.root;
        for c in path {
            node = match node {
                Node::Dir(kids) => &mut kids.iter_mut().find(|(n, _)| n == c)?.1,
                Node::File { .. } => return None,
            };
        }
        Some(node)
    }

    /// The entries of the folder at `path`: stale if it isn't one.
    fn dir_mut(&mut self, path: &[String]) -> Result<&mut Vec<(String, Node)>, FsError> {
        match self.node_mut(path) {
            Some(Node::Dir(kids)) => Ok(kids),
            _ => Err(FsError::Stale),
        }
    }

    fn handle_path(&self, dir: &usize) -> Vec<String> {
        self.dirs[*dir].clone().expect("a closed handle used")
    }

    // ---- Another process's changes ----

    pub fn ext_mkdir(&mut self, path: &str) {
        let mut path = p(path);
        let name = path.pop().unwrap();
        self.dir_mut(&path)
            .expect("parent")
            .push((name, Node::Dir(Vec::new())));
    }

    pub fn ext_file(&mut self, path: &str, bytes: &[u8]) {
        self.ext_node(path, Kind::Regular, bytes);
    }

    pub fn ext_node(&mut self, path: &str, kind: Kind, bytes: &[u8]) {
        let mut path = p(path);
        let name = path.pop().unwrap();
        self.dir_mut(&path).expect("parent").push((
            name,
            Node::File {
                kind,
                bytes: bytes.to_vec(),
            },
        ));
    }

    pub fn ext_remove(&mut self, path: &str) -> Node {
        let mut path = p(path);
        let name = path.pop().unwrap();
        let kids = self.dir_mut(&path).expect("parent");
        let at = kids.iter().position(|(n, _)| *n == name).expect("entry");
        kids.remove(at).1
    }

    pub fn ext_rename(&mut self, from: &str, to: &str) {
        let node = self.ext_remove(from);
        let mut to = p(to);
        let name = to.pop().unwrap();
        self.dir_mut(&to).expect("parent").push((name, node));
    }

    // ---- What the KATs read ----

    pub fn exists(&self, path: &str) -> bool {
        self.node(&p(path)).is_some()
    }

    pub fn bytes(&self, path: &str) -> Option<&[u8]> {
        match self.node(&p(path))? {
            Node::File { bytes, .. } => Some(bytes),
            Node::Dir(_) => None,
        }
    }

    /// Every path, with its node's kind and size (a folder's entry count).
    pub fn snapshot(&self) -> BTreeMap<String, (Kind, Vec<u8>)> {
        fn walk(prefix: &str, node: &Node, out: &mut BTreeMap<String, (Kind, Vec<u8>)>) {
            if let Node::Dir(kids) = node {
                for (name, kid) in kids {
                    let path = alloc::format!("{prefix}/{name}");
                    match kid {
                        Node::Dir(_) => {
                            out.insert(path.clone(), (Kind::Directory, Vec::new()));
                        }
                        Node::File { kind, bytes } => {
                            out.insert(path.clone(), (*kind, bytes.clone()));
                        }
                    }
                    walk(&path, kid, out);
                }
            }
        }
        let mut out = BTreeMap::new();
        walk("", &self.root, &mut out);
        out
    }

    /// Handles not yet closed.
    pub fn open_handles(&self) -> usize {
        self.dirs.iter().flatten().count() + self.files.iter().flatten().count()
    }

    fn check_name(name: &str) -> Result<(), FsError> {
        if name.len() > NAME_MAX {
            Err(FsError::NameTooLong)
        } else if name.is_empty()
            || name.contains('/')
            || name.contains('\0')
            || name == "."
            || name == ".."
        {
            Err(FsError::Invalid)
        } else {
            Ok(())
        }
    }
}

fn stat_of(node: &Node) -> Stat {
    match node {
        Node::Dir(kids) => Stat {
            kind: Kind::Directory,
            empty_directory: kids.is_empty(),
            size: kids.len() as u64,
        },
        Node::File { kind, bytes } => Stat {
            kind: *kind,
            empty_directory: false,
            size: bytes.len() as u64,
        },
    }
}

impl Fs for FakeFs {
    type Dir = usize;
    type File = usize;

    fn open_dir(&mut self, path: &[String], _rights: Rights) -> Result<usize, FsError> {
        self.injected(Op::OpenDir)?;
        match self.node(path) {
            Some(Node::Dir(_)) => {}
            Some(Node::File { .. }) => return Err(FsError::Stale),
            None => return Err(FsError::NotFound),
        }
        self.dirs.push(Some(path.to_vec()));
        Ok(self.dirs.len() - 1)
    }

    fn readdir(
        &mut self,
        dir: &usize,
        cursor: &mut u64,
        out: &mut Vec<DirEntry>,
    ) -> Result<usize, FsError> {
        self.injected(Op::Readdir)?;
        let path = self.handle_path(dir);
        let batch = self.batch;
        let kids = self.dir_mut(&path)?;
        let start = *cursor as usize;
        let taken: Vec<DirEntry> = kids
            .iter()
            .skip(start)
            .take(batch)
            .map(|(name, node)| DirEntry {
                name: name.clone(),
                kind: stat_of(node).kind,
            })
            .collect();
        *cursor += taken.len() as u64;
        let n = taken.len();
        out.extend(taken);
        Ok(n)
    }

    fn stat_at(&mut self, dir: &usize, name: &str) -> Result<Stat, FsError> {
        self.injected(Op::Stat)?;
        let path = self.handle_path(dir);
        let kids = self.dir_mut(&path)?;
        kids.iter()
            .find(|(n, _)| n == name)
            .map(|(_, node)| stat_of(node))
            .ok_or(FsError::NotFound)
    }

    fn mkdir_at(&mut self, dir: &usize, name: &str) -> Result<(), FsError> {
        self.injected(Op::Mkdir)?;
        let path = self.handle_path(dir);
        Self::check_name(name)?;
        let kids = self.dir_mut(&path)?;
        if kids.iter().any(|(n, _)| n == name) {
            return Err(FsError::Exists);
        }
        if kids.len() >= FOLDER_MAX {
            return Err(FsError::FolderFull);
        }
        kids.push((String::from(name), Node::Dir(Vec::new())));
        self.log.push(alloc::format!("mkdir {path:?} {name}"));
        Ok(())
    }

    fn unlink_at(&mut self, dir: &usize, name: &str) -> Result<(), FsError> {
        self.injected(Op::Unlink)?;
        let mut path = self.handle_path(dir);
        path.push(String::from(name));
        if self.busy.contains(&path) {
            return Err(FsError::Busy);
        }
        path.pop();
        let kids = self.dir_mut(&path)?;
        let at = kids
            .iter()
            .position(|(n, _)| n == name)
            .ok_or(FsError::NotFound)?;
        match &kids[at].1 {
            Node::Dir(sub) if !sub.is_empty() => return Err(FsError::NotEmpty),
            Node::File {
                kind: Kind::Model | Kind::Symlink,
                ..
            } => return Err(FsError::Refused),
            _ => {}
        }
        kids.remove(at);
        self.log.push(alloc::format!("unlink {path:?} {name}"));
        Ok(())
    }

    fn rename_at(
        &mut self,
        dir: &usize,
        name: &str,
        to: &usize,
        to_name: &str,
    ) -> Result<(), FsError> {
        self.injected(Op::Rename)?;
        let from_dir = self.handle_path(dir);
        let to_dir = self.handle_path(to);
        Self::check_name(to_name)?;
        let mut src = from_dir.clone();
        src.push(String::from(name));
        if self.busy.contains(&src) {
            return Err(FsError::Busy);
        }
        // Resolve both folders before changing anything.
        self.dir_mut(&to_dir)?;
        let kids = self.dir_mut(&from_dir)?;
        if !kids.iter().any(|(n, _)| n == name) {
            return Err(FsError::NotFound);
        }
        let mut dst = to_dir.clone();
        dst.push(String::from(to_name));
        if dst.starts_with(&src) && dst != src {
            return Err(FsError::Invalid);
        }
        let dst_kids = self.dir_mut(&to_dir)?;
        if dst_kids.iter().any(|(n, _)| n == to_name) {
            return Err(FsError::Exists);
        }
        if from_dir != to_dir && dst_kids.len() >= FOLDER_MAX {
            return Err(FsError::FolderFull);
        }
        let kids = self.dir_mut(&from_dir)?;
        let at = kids.iter().position(|(n, _)| n == name).unwrap();
        let (_, node) = kids.remove(at);
        self.dir_mut(&to_dir)?.push((String::from(to_name), node));
        self.log.push(alloc::format!(
            "rename {from_dir:?} {name} {to_dir:?} {to_name}"
        ));
        if self.stale_after_rename {
            // Committed, but the source handle's reload failed (Muse obs. 2).
            return Err(FsError::Stale);
        }
        Ok(())
    }

    fn open_file(
        &mut self,
        path: &[String],
        create: bool,
        rights: Rights,
    ) -> Result<usize, FsError> {
        self.injected(Op::OpenFile)?;
        let (name, parent) = path.split_last().ok_or(FsError::Refused)?;
        let kids = match self.node_mut(parent) {
            Some(Node::Dir(kids)) => kids,
            Some(Node::File { .. }) => return Err(FsError::Stale),
            None => return Err(FsError::NotFound),
        };
        match kids.iter().find(|(n, _)| n == name) {
            Some((_, Node::Dir(_))) => return Err(FsError::Refused),
            Some(_) if create => return Err(FsError::Exists),
            Some(_) => {}
            None if !create => return Err(FsError::NotFound),
            None => {
                Self::check_name(name)?;
                if kids.len() >= FOLDER_MAX {
                    return Err(FsError::FolderFull);
                }
                if !rights.contains(Rights::WRITE) {
                    return Err(FsError::Refused);
                }
                kids.push((
                    name.clone(),
                    Node::File {
                        kind: Kind::Regular,
                        bytes: Vec::new(),
                    },
                ));
                self.log.push(alloc::format!("create {path:?}"));
            }
        }
        self.files.push(Some((path.to_vec(), 0)));
        Ok(self.files.len() - 1)
    }

    fn read(&mut self, file: &usize, buf: &mut [u8]) -> Result<usize, FsError> {
        self.injected(Op::Read)?;
        let (path, pos) = self.files[*file].clone().expect("a closed file used");
        let Some(Node::File { bytes, .. }) = self.node(&path) else {
            return Err(FsError::Stale);
        };
        let n = buf.len().min(bytes.len().saturating_sub(pos)).min(4096);
        buf[..n].copy_from_slice(&bytes[pos..pos + n]);
        self.files[*file].as_mut().unwrap().1 += n;
        Ok(n)
    }

    fn write(&mut self, file: &usize, buf: &[u8]) -> Result<usize, FsError> {
        self.injected(Op::Write)?;
        let (path, pos) = self.files[*file].clone().expect("a closed file used");
        let space = self.space;
        let Some(Node::File { bytes, .. }) = self.node_mut(&path) else {
            return Err(FsError::Stale);
        };
        // The ordinary write path: 32 KiB a file, and the volume's room.
        let n = buf
            .len()
            .min(4096)
            .min(COPY_MAX.saturating_sub(pos))
            .min(space);
        if n == 0 && !buf.is_empty() {
            return Err(FsError::NoSpace);
        }
        bytes.truncate(pos);
        bytes.extend_from_slice(&buf[..n]);
        self.space -= n;
        self.files[*file].as_mut().unwrap().1 += n;
        Ok(n)
    }

    fn close_dir(&mut self, dir: usize) {
        assert!(self.dirs[dir].take().is_some(), "a folder closed twice");
    }

    fn close_file(&mut self, file: usize) {
        assert!(self.files[file].take().is_some(), "a file closed twice");
    }
}
