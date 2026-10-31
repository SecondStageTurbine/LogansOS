// SPDX-License-Identifier: MPL-2.0
//! Production-stable lock owner diagnostics.
//!
//! `LockOwner` is intentionally small and NMI-readable. Writers use a seqlock
//! counter so diagnostic readers can reject in-flight updates without taking
//! any locks of their own.

use core::panic::Location;

#[cfg(feature = "kernel-diagnostic")]
use core::sync::atomic::{AtomicU32, Ordering};

#[allow(dead_code)]
pub const UNOWNED: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockOwnerSnapshot {
    pub cpu: u32,
    pub thread_id: u32,
    pub acquisition_site: u32,
}

#[cfg(feature = "kernel-diagnostic")]
pub struct LockOwner {
    cpu: AtomicU32,
    thread_id: AtomicU32,
    acquisition_site: AtomicU32,
    seq: AtomicU32,
}

#[cfg(not(feature = "kernel-diagnostic"))]
pub struct LockOwner;

#[allow(dead_code)]
impl LockOwner {
    #[inline(always)]
    pub const fn new() -> Self {
        #[cfg(feature = "kernel-diagnostic")]
        {
            Self {
                cpu: AtomicU32::new(UNOWNED),
                thread_id: AtomicU32::new(UNOWNED),
                acquisition_site: AtomicU32::new(0),
                seq: AtomicU32::new(0),
            }
        }
        #[cfg(not(feature = "kernel-diagnostic"))]
        {
            Self
        }
    }

    #[inline(always)]
    pub fn acquire(&self, cpu: u32, thread_id: u32, site: u32) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::AcqRel);
            self.cpu.store(cpu, Ordering::Relaxed);
            self.thread_id.store(thread_id, Ordering::Relaxed);
            self.acquisition_site.store(site, Ordering::Relaxed);
            self.seq.fetch_add(1, Ordering::Release);
        }
        #[cfg(not(feature = "kernel-diagnostic"))]
        {
            let _ = (cpu, thread_id, site);
        }
    }

    #[inline(always)]
    pub fn release(&self) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::AcqRel);
            self.cpu.store(UNOWNED, Ordering::Relaxed);
            self.thread_id.store(UNOWNED, Ordering::Relaxed);
            self.acquisition_site.store(0, Ordering::Relaxed);
            self.seq.fetch_add(1, Ordering::Release);
        }
    }

    #[inline(always)]
    pub fn read_consistent(&self) -> Option<LockOwnerSnapshot> {
        #[cfg(feature = "kernel-diagnostic")]
        {
            let seq1 = self.seq.load(Ordering::Acquire);
            if seq1 & 1 != 0 {
                return None;
            }

            let cpu = self.cpu.load(Ordering::Relaxed);
            let thread_id = self.thread_id.load(Ordering::Relaxed);
            let acquisition_site = self.acquisition_site.load(Ordering::Relaxed);

            let seq2 = self.seq.load(Ordering::Acquire);
            if seq1 != seq2 {
                return None;
            }

            Some(LockOwnerSnapshot {
                cpu,
                thread_id,
                acquisition_site,
            })
        }
        #[cfg(not(feature = "kernel-diagnostic"))]
        {
            None
        }
    }
}

impl Default for LockOwner {
    fn default() -> Self {
        Self::new()
    }
}

#[track_caller]
#[allow(dead_code)]
pub fn caller_site() -> u32 {
    let location = Location::caller();
    hash_site(location.file(), location.line())
}

#[allow(dead_code)]
pub const fn hash_site(file: &str, line: u32) -> u32 {
    let bytes = file.as_bytes();
    let mut hash = 0x811c_9dc5u32;
    let mut idx = 0;
    while idx < bytes.len() {
        hash ^= bytes[idx] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        idx += 1;
    }

    let mut shift = 0;
    while shift < 32 {
        hash ^= (line >> shift) & 0xff;
        hash = hash.wrapping_mul(0x0100_0193);
        shift += 8;
    }
    hash
}

#[macro_export]
macro_rules! lock_site {
    () => {
        $crate::sync::lock_owner::hash_site(file!(), line!())
    };
}
