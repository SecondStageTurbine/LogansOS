// SPDX-License-Identifier: MPL-2.0
//! NeoFS v1 mount support for Phase 3 BUILD-2.
//!
//! This module consumes the 2A on-disk format exactly as specified in
//! `docs/specs/neofs-format.md`. 2B validated read-only mkfs images; 2C adds
//! the first narrow runtime mutation path for small Regular files through the
//! synchronous block-device API and the BUILD-2 bump allocator.

mod alloc_logic;
mod cache;
// The commit order on a write-back disk (pure; host-tested).
mod commit_logic;
mod format;
mod inode;
mod mount;
// NFS-W1: the block tree of a file written in one piece (pure).
#[cfg(feature = "neofs-large-write")]
mod tree_logic;

#[cfg(feature = "neo-test")]
pub use cache::{CacheStats, WriteBackWorkerStopReport};
#[allow(unused_imports)] // exported ABI tag for callers that inspect mounted volume kind
pub use format::NeoFsKind;
#[cfg(feature = "power-clean")]
pub use inode::ShutdownReport;
#[allow(unused_imports)] // write/GC diagnostics are consumed only by selected scenarios
pub use inode::{FreeMapStats, GcStats};
#[cfg(feature = "neo-test-phase3-umount-drop-outside-mounts")]
pub(crate) use inode::{pause_next_state_drop_for_test, state_drop_paused_for_test};
pub use mount::{MountReport, NeoFs};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum NeoFsError {
    NoBlockDevice,
    BlockDevice,
    InvalidDeviceGeometry,
    NoValidSuperblock,
    InvalidSuperblock,
    InvalidInode,
    InvalidDirectory,
    UnsupportedType,
    ContentHashMismatch,
    RootHashMismatch,
    RootNotDirectory,
    DirectoryTypeMismatch,
    FileTooLarge,
    NoSpace,
    NotFound,
    ReadOnly,
    /// A writable mount needs a device that can flush its write cache: the
    /// commit order rests on flushes (`commit_logic`).
    NoDurableFlush,
}

/// NFS-W1: the next large write stops before its commit (the scenario's cut).
#[cfg(feature = "neo-test-neofs-large-write")]
pub fn debug_stop_next_large_write_before_commit() {
    inode::LARGE_WRITE_STOP_BEFORE_COMMIT.store(true, core::sync::atomic::Ordering::Release);
}

/// NFS-W1: the next large write fails its first data write.
#[cfg(feature = "neo-test-neofs-large-write")]
pub fn debug_fail_next_large_write() {
    inode::LARGE_WRITE_FAIL_FIRST_WRITE.store(true, core::sync::atomic::Ordering::Release);
}

/// NFS-W1: the last large write's run: (first block, block count).
#[cfg(feature = "neo-test-neofs-large-write")]
pub fn debug_last_large_write_run() -> (u64, u64) {
    let v = inode::LAST_LARGE_WRITE_RUN.load(core::sync::atomic::Ordering::Acquire);
    (v >> 32, v & 0xffff_ffff)
}
