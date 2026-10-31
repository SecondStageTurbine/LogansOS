// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.2 (A2, I-18): memory accounts — the per-content-process frame quota.
//!
//! An account counts the buddy frames its processes caused to be allocated.
//! Accounts nest: a budgeted spawn's account has the spawner's as its parent,
//! so a charge must fit every limit up the chain. A browser launched with a
//! budget therefore bounds all of its content processes together, and each
//! content process is bounded by its own.
//!
//! Lock-free: a charge is one compare-and-swap per level, so it is safe at
//! IF=0, under `Pmap::inner` or `FRAME_ALLOC`, and on any CPU (CP-37 N/A).
//! Pure, so the host KAT crate (`tools/mem-account-logic-tests`) mounts it via
//! `#[path]`.

use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

/// No limit at this level.
pub const UNLIMITED: u64 = u64::MAX;

/// A charge that would take some account on the chain over its limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotaExceeded;

/// A release of more frames than were charged (a bookkeeping bug, never a
/// user action). The counter is left unchanged rather than wrapped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Underflow;

pub struct MemAccount {
    limit: u64,
    used: AtomicU64,
    /// Strong: a child account keeps its parent alive, so the chain walk never
    /// reaches a freed account. Accounts form a tree, so this cannot cycle.
    parent: Option<Arc<MemAccount>>,
    /// True when this account or any ancestor has a limit.
    budgeted: bool,
    /// Releases refused as `Underflow` (always 0 unless there is a bug).
    underflows: AtomicU64,
    /// OT-61 C2.2b (I-18): the kernel-heap object budget, in bytes of
    /// declared weight, charged on the same chain as the frames.
    #[cfg(feature = "obj-quota")]
    obj_limit: u64,
    #[cfg(feature = "obj-quota")]
    obj_used: AtomicU64,
}

impl MemAccount {
    /// The unlimited root (init's account).
    pub fn root() -> Arc<Self> {
        Arc::new(Self {
            limit: UNLIMITED,
            used: AtomicU64::new(0),
            parent: None,
            budgeted: false,
            underflows: AtomicU64::new(0),
            #[cfg(feature = "obj-quota")]
            obj_limit: UNLIMITED,
            #[cfg(feature = "obj-quota")]
            obj_used: AtomicU64::new(0),
        })
    }

    /// A child account limited to `limit` frames, nested under `parent`.
    pub fn child(parent: &Arc<Self>, limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicU64::new(0),
            parent: Some(parent.clone()),
            budgeted: limit != UNLIMITED || parent.budgeted,
            underflows: AtomicU64::new(0),
            #[cfg(feature = "obj-quota")]
            obj_limit: UNLIMITED,
            #[cfg(feature = "obj-quota")]
            obj_used: AtomicU64::new(0),
        })
    }

    /// The root with an object limit (the kernel heap less its reserve).
    #[cfg(feature = "obj-quota")]
    pub fn root_with_obj_limit(obj_limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit: UNLIMITED,
            used: AtomicU64::new(0),
            parent: None,
            budgeted: false,
            underflows: AtomicU64::new(0),
            obj_limit,
            obj_used: AtomicU64::new(0),
        })
    }

    /// A child account with a frame limit and an object limit (either may
    /// be `UNLIMITED`), nested under `parent`.
    #[cfg(feature = "obj-quota")]
    pub fn child_with_obj_limit(parent: &Arc<Self>, limit: u64, obj_limit: u64) -> Arc<Self> {
        Arc::new(Self {
            limit,
            used: AtomicU64::new(0),
            parent: Some(parent.clone()),
            budgeted: limit != UNLIMITED || obj_limit != UNLIMITED || parent.budgeted,
            underflows: AtomicU64::new(0),
            obj_limit,
            obj_used: AtomicU64::new(0),
        })
    }

    pub fn limit(&self) -> u64 {
        self.limit
    }

    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Acquire)
    }

    /// This account or an ancestor has a limit.
    pub fn is_budgeted(&self) -> bool {
        self.budgeted
    }

    /// Charge `n` frames at every level, or none: if a level refuses, the
    /// levels already charged are released before returning.
    pub fn charge(&self, n: u64) -> Result<(), QuotaExceeded> {
        if n == 0 {
            return Ok(());
        }
        let mut level: Option<&MemAccount> = Some(self);
        let mut charged = 0usize;
        while let Some(acct) = level {
            let limit = acct.limit;
            let ok = acct
                .used
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |u| {
                    u.checked_add(n).filter(|&v| v <= limit)
                })
                .is_ok();
            if !ok {
                // Roll back the `charged` levels below this one.
                let mut undo: Option<&MemAccount> = Some(self);
                for _ in 0..charged {
                    let a = undo.expect("rollback stays inside the charged levels");
                    a.used.fetch_sub(n, Ordering::AcqRel);
                    undo = a.parent.as_deref();
                }
                return Err(QuotaExceeded);
            }
            charged += 1;
            level = acct.parent.as_deref();
        }
        Ok(())
    }

    /// Release `n` frames at every level. A level holding fewer than `n` is
    /// left unchanged and counted in `underflows`; the walk continues so the
    /// other levels stay consistent with each other.
    pub fn release(&self, n: u64) -> Result<(), Underflow> {
        if n == 0 {
            return Ok(());
        }
        let mut result = Ok(());
        let mut level: Option<&MemAccount> = Some(self);
        while let Some(acct) = level {
            if acct
                .used
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |u| u.checked_sub(n))
                .is_err()
            {
                acct.underflows.fetch_add(1, Ordering::AcqRel);
                result = Err(Underflow);
            }
            level = acct.parent.as_deref();
        }
        result
    }

    pub fn underflows(&self) -> u64 {
        self.underflows.load(Ordering::Acquire)
    }

    #[cfg(feature = "obj-quota")]
    pub fn obj_limit(&self) -> u64 {
        self.obj_limit
    }

    #[cfg(feature = "obj-quota")]
    pub fn obj_used(&self) -> u64 {
        self.obj_used.load(Ordering::Acquire)
    }

    /// Charge `n` bytes of object weight at every level, or none: the same
    /// walk and rollback as [`charge`](Self::charge), on the second counter.
    #[cfg(feature = "obj-quota")]
    pub fn charge_obj(&self, n: u64) -> Result<(), QuotaExceeded> {
        if n == 0 {
            return Ok(());
        }
        let mut level: Option<&MemAccount> = Some(self);
        let mut charged = 0usize;
        while let Some(acct) = level {
            let limit = acct.obj_limit;
            let ok = acct
                .obj_used
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |u| {
                    u.checked_add(n).filter(|&v| v <= limit)
                })
                .is_ok();
            if !ok {
                let mut undo: Option<&MemAccount> = Some(self);
                for _ in 0..charged {
                    let a = undo.expect("rollback stays inside the charged levels");
                    a.obj_used.fetch_sub(n, Ordering::AcqRel);
                    undo = a.parent.as_deref();
                }
                return Err(QuotaExceeded);
            }
            charged += 1;
            level = acct.parent.as_deref();
        }
        Ok(())
    }

    /// Release `n` bytes of object weight at every level (as
    /// [`release`](Self::release)).
    #[cfg(feature = "obj-quota")]
    pub fn release_obj(&self, n: u64) -> Result<(), Underflow> {
        if n == 0 {
            return Ok(());
        }
        let mut result = Ok(());
        let mut level: Option<&MemAccount> = Some(self);
        while let Some(acct) = level {
            if acct
                .obj_used
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |u| u.checked_sub(n))
                .is_err()
            {
                acct.underflows.fetch_add(1, Ordering::AcqRel);
                result = Err(Underflow);
            }
            level = acct.parent.as_deref();
        }
        result
    }
}

/// OT-61 C2.2b: one object's charge, in bytes of declared weight. Released
/// exactly once, when it drops; it never re-derives a size.
#[cfg(feature = "obj-quota")]
pub struct ObjCharge {
    account: Arc<MemAccount>,
    bytes: u64,
}

#[cfg(feature = "obj-quota")]
impl ObjCharge {
    /// Charge `bytes` to `account`'s chain, or refuse.
    pub fn new(account: &Arc<MemAccount>, bytes: u64) -> Result<Self, QuotaExceeded> {
        account.charge_obj(bytes)?;
        Ok(Self {
            account: account.clone(),
            bytes,
        })
    }

    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[cfg(feature = "obj-quota")]
impl Drop for ObjCharge {
    fn drop(&mut self) {
        let _ = self.account.release_obj(self.bytes);
    }
}

/// What one owner (a `Pmap`, an Atrium buffer) has charged to its account.
/// The owner charges and releases only through its ledger, and `release_all`
/// at teardown returns exactly what is still held, so the account can never
/// be released for frames the owner did not charge.
pub struct ChargeLedger {
    account: Arc<MemAccount>,
    held: AtomicU64,
}

impl ChargeLedger {
    pub fn new(account: Arc<MemAccount>) -> Self {
        Self {
            account,
            held: AtomicU64::new(0),
        }
    }

    pub fn account(&self) -> &Arc<MemAccount> {
        &self.account
    }

    pub fn held(&self) -> u64 {
        self.held.load(Ordering::Acquire)
    }

    /// Charge `n` frames before allocating them.
    pub fn charge(&self, n: u64) -> Result<(), QuotaExceeded> {
        self.account.charge(n)?;
        self.held.fetch_add(n, Ordering::AcqRel);
        Ok(())
    }

    /// Release `n` frames this owner charged: after a failed allocation, or
    /// when it frees or hands back a frame. More than it holds is refused
    /// (`Underflow`) and changes nothing.
    pub fn release(&self, n: u64) -> Result<(), Underflow> {
        if n == 0 {
            return Ok(());
        }
        if self
            .held
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |h| h.checked_sub(n))
            .is_err()
        {
            self.account.underflows.fetch_add(1, Ordering::AcqRel);
            return Err(Underflow);
        }
        self.account.release(n)
    }

    /// Release everything still held (the owner's teardown). Idempotent.
    pub fn release_all(&self) -> Result<(), Underflow> {
        let n = self.held.swap(0, Ordering::AcqRel);
        self.account.release(n)
    }
}

/// A dropped ledger releases whatever it still holds, so an owner's early
/// return (an allocation failing after its charge) cannot leak a charge.
/// Owners that want the release checked call `release_all` first; this is
/// then a no-op.
impl Drop for ChargeLedger {
    fn drop(&mut self) {
        let _ = self.release_all();
    }
}

/// `SYS_MEM_INFO` v1: 32 bytes, little-endian.
///
/// | Bytes | Field |
/// |---|---|
/// | 0..4 | version (1) |
/// | 4..8 | flags (bit 0: this process has its own budget) |
/// | 8..16 | used frames |
/// | 16..24 | limit frames (`UNLIMITED` = none) |
/// | 24..32 | frames in use system-wide |
pub const MEM_INFO_V1_LEN: usize = 32;
pub const MEM_INFO_VERSION: u32 = 1;
pub const MEM_INFO_FLAG_OWN_BUDGET: u32 = 1 << 0;

/// `SYS_MEM_INFO` v2 (OT-61 C2.2b): v1's 32 bytes, then
///
/// | Bytes | Field |
/// |---|---|
/// | 32..40 | object bytes used (declared weights) |
/// | 40..48 | object byte limit (`UNLIMITED` = none) |
///
/// with version 2. A caller asks for it by passing 48 as the length.
#[cfg(feature = "obj-quota")]
pub const MEM_INFO_V2_LEN: usize = 48;
#[cfg(feature = "obj-quota")]
pub const MEM_INFO_VERSION_2: u32 = 2;

#[cfg(feature = "obj-quota")]
pub fn encode_mem_info_v2(
    flags: u32,
    used: u64,
    limit: u64,
    global_used: u64,
    obj_used: u64,
    obj_limit: u64,
) -> [u8; MEM_INFO_V2_LEN] {
    let mut out = [0u8; MEM_INFO_V2_LEN];
    out[..MEM_INFO_V1_LEN].copy_from_slice(&encode_mem_info(flags, used, limit, global_used));
    out[0..4].copy_from_slice(&MEM_INFO_VERSION_2.to_le_bytes());
    out[32..40].copy_from_slice(&obj_used.to_le_bytes());
    out[40..48].copy_from_slice(&obj_limit.to_le_bytes());
    out
}

pub fn encode_mem_info(
    flags: u32,
    used: u64,
    limit: u64,
    global_used: u64,
) -> [u8; MEM_INFO_V1_LEN] {
    let mut out = [0u8; MEM_INFO_V1_LEN];
    out[0..4].copy_from_slice(&MEM_INFO_VERSION.to_le_bytes());
    out[4..8].copy_from_slice(&flags.to_le_bytes());
    out[8..16].copy_from_slice(&used.to_le_bytes());
    out[16..24].copy_from_slice(&limit.to_le_bytes());
    out[24..32].copy_from_slice(&global_used.to_le_bytes());
    out
}

/// Frames for a byte budget, rounded up. `None` for 0 (no budget is not a
/// budget of 0) and for a value that would round past `UNLIMITED`.
pub fn budget_frames(bytes: u64) -> Option<u64> {
    if bytes == 0 {
        return None;
    }
    let frames = bytes.div_ceil(4096);
    (frames != UNLIMITED).then_some(frames)
}
