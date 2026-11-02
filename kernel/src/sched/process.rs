// SPDX-License-Identifier: MPL-2.0
//! Process abstraction.
//!
//! Phase 1.2/1.3 had only Threads. Phase 1.4 introduces Process: a Pmap
//! plus a name plus a pid. Threads gain a `pid` field; the scheduler uses
//! it to decide whether a context switch crosses a process boundary
//! (requiring CR3 reload + TSS.rsp0 update).
//!
//! Process composition:
//!   - pid 0: the implicit "kernel process". Pmap is None — kernel threads
//!     run on the kernel's PML4 directly. All Phase 1.2/1.3 threads
//!     (main, ping, pong, hog) belong here.
//!   - pid 1+: user processes. Each owns a `Pmap`. Phase 1.4 spawns
//!     exactly one (neo-init); Phase 1.5 will support multiple.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering,
};

use spin::{Lazy, Mutex};
use x86_64::VirtAddr;
use x86_64::registers::control::Cr3Flags;
use x86_64::structures::paging::PhysFrame;

use crate::ai::model::Model;
use crate::arch::x86_64::without_interrupts;
use crate::fs::{FileRights, OpenFile};
use crate::ipc::{Channel, EndpointSide};
use crate::loader::elf::{Elf, ElfError, allocate_user_stack};
use crate::mm::pmap::Pmap;
use crate::net::socket::Socket;
use crate::net::tcp::{TcpListener, TcpSocket};
use crate::serial_println;
#[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
use crate::sync::ticket_mutex::{TicketMutex, TicketMutexGuard};
use crate::sync::{IrqSafeMutex, SwitchMutex};

use super::file_cap_logic;
#[cfg(feature = "net-dial-cap")]
use super::net_dial_cap_logic;
use super::scheduler;
use super::thread::Thread;

// I-15 honesty guard: the pure `file_cap_logic` mapping mirrors the authoritative
// `FileRights` (fs) and `meta_rights` (below) bit values as local consts so the
// host KAT crate can `#[path]`-mount it kernel-free. These asserts tie the two
// together — any drift in either definition is a compile error here, not a silent
// oracle skew. (Grep-checkable guard for I-15; see docs/complexity-governance.md.)
const _: () = {
    assert!(file_cap_logic::FR_READ == FileRights::READ.bits());
    assert!(file_cap_logic::FR_WRITE == FileRights::WRITE.bits());
    assert!(file_cap_logic::FR_LOOKUP == FileRights::LOOKUP.bits());
    assert!(file_cap_logic::FR_READDIR == FileRights::READDIR.bits());
    assert!(file_cap_logic::FR_EXECUTE == FileRights::EXECUTE.bits());
    assert!(file_cap_logic::FR_MMAP == FileRights::MMAP_RESERVED.bits());
    assert!(file_cap_logic::FR_GRANT == FileRights::GRANT_RESERVED.bits());
    assert!(file_cap_logic::FR_DERIVE == FileRights::DERIVE_RESERVED.bits());
    assert!(file_cap_logic::FR_MOUNT == FileRights::MOUNT.bits());
    assert!(file_cap_logic::M_USE == meta_rights::USE);
    assert!(file_cap_logic::M_DERIVE == meta_rights::DERIVE);
    assert!(file_cap_logic::M_GRANT == meta_rights::GRANT);
    assert!(file_cap_logic::M_REVOKE == meta_rights::REVOKE);
};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct ProcessId(pub u32);

/// pid 0 reserved for the implicit kernel process.
pub const KERNEL_PID: ProcessId = ProcessId(0);

static NEXT_PID: AtomicU32 = AtomicU32::new(1);
const EXIT_CODE_RUNNING: i32 = i32::MIN;
const EXIT_STATE_RUNNING: u8 = 0;
const EXIT_STATE_EXITING_NATURAL: u8 = 1;
const EXIT_STATE_EXITING_FORCED: u8 = 2;
const EXIT_STATE_EXITED: u8 = 3;
const PROCESS_WAIT_QUEUE_CAPACITY: usize = 4;

pub const WAIT_FLAG_WNOHANG: u64 = 1 << 0;
pub const WAIT_EXITED_TAG: i64 = 1_i64 << 32;
pub const PROCESS_INSPECT_STATE_RUNNING: u32 = 0;
pub const PROCESS_INSPECT_STATE_EXITED: u32 = 1;

pub const fn encode_wait_exit_code(code: i32) -> i64 {
    WAIT_EXITED_TAG | ((code as u32) as i64)
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct CapId(pub u64);

/// CapId(0) is reserved as the null / no-cap sentinel.
pub const NULL_CAP_ID: CapId = CapId(0);

static NEXT_CAP_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a kernel-unique capability id. BUILD-1 uses a single global
/// atomic because cap allocation is not a hot path; Phase 2 SMP work may
/// switch this to per-CPU banks without changing the API surface.
pub fn alloc_cap_id() -> CapId {
    let id = NEXT_CAP_ID.fetch_add(1, Ordering::Relaxed);
    debug_assert_ne!(id, NULL_CAP_ID.0, "CapId 0 is reserved");
    CapId(id)
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CapRights {
    pub meta: u8,
    pub semantic: u8,
}

impl CapRights {
    pub const fn new(meta: u8, semantic: u8) -> Self {
        Self { meta, semantic }
    }

    pub const fn is_subset_of(self, parent: Self) -> bool {
        (self.meta & parent.meta) == self.meta && (self.semantic & parent.semantic) == self.semantic
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self {
            meta: self.meta & other.meta,
            semantic: self.semantic & other.semantic,
        }
    }

    pub const fn has_meta(self, bit: u8) -> bool {
        (self.meta & bit) == bit
    }

    pub const fn has_semantic(self, bit: u8) -> bool {
        (self.semantic & bit) == bit
    }
}

pub mod meta_rights {
    pub const USE: u8 = 1 << 0;
    pub const DERIVE: u8 = 1 << 1;
    pub const GRANT: u8 = 1 << 2;
    pub const REVOKE: u8 = 1 << 3;
}

pub mod channel_rights {
    pub const SEND: u8 = 1 << 0;
    pub const RECV: u8 = 1 << 1;
    pub const CLOSE: u8 = 1 << 2;
}

pub mod process_rights {
    pub const KILL: u8 = 1 << 0;
    pub const WAIT: u8 = 1 << 1;
    pub const INSPECT: u8 = 1 << 2;
}

pub mod socket_rights {
    pub const SEND: u8 = 1 << 0;
    pub const RECV: u8 = 1 << 1;
    pub const BIND: u8 = 1 << 2;
    pub const CLOSE: u8 = 1 << 3;
    pub const READ: u8 = 1 << 4;
    pub const WRITE: u8 = 1 << 5;
}

pub mod tensor_rights {
    /// Read access to the tensor (query shape/bytes, use as an inference input).
    pub const USE: u8 = 1 << 0;
    /// Mutate the tensor's bytes (Phase 7 BUILD-3). A read-only share derives a cap
    /// **without** this bit; a mutating op by such a grantee is denied at the op
    /// boundary, independent of the tensor's own object-level `mutable` flag.
    pub const WRITE: u8 = 1 << 1;
}

pub mod model_rights {
    pub const USE: u8 = 1 << 0;
}

pub mod net_dial_rights {
    /// Authorise one outbound dial within the immutable handle scope.
    pub const DIAL: u8 = 1 << 0;
    /// OT-61 rung 5b (`net-dial-public-only`): the dial may reach non-public
    /// addresses (loopback, private, link-local; `net_dial_cap_logic::
    /// v4_public`/`v6_public`). Root caps carry it, so they dial as before; a
    /// cap derived without it reaches only public addresses, checked after
    /// the kernel's own resolution (the fetch broker's per-request caps).
    pub const NON_PUBLIC: u8 = 1 << 1;
}

/// OT-61 C1.1 — semantic rights for the `BrowserRuntime` cap (the thread/VM
/// authority of the browser host runtime, the `NetDial` pattern: minted only
/// by the kernel root path, no self-mint, fail-closed `EPERM` without it).
#[cfg(feature = "browser-runtime")]
pub mod browser_runtime_rights {
    /// Authorise `SYS_THREAD_CREATE` — the one C1.1 operation that mints a
    /// new execution context. The other thread/futex syscalls act only on
    /// the caller's own process state and confer no new authority.
    pub const THREAD_SPAWN: u8 = 1 << 0;
}

/// OT-87 R4.4: semantic rights for the `Installer` cap, the authority to
/// list install targets and run an install (`SYS_INSTALL_LIST`,
/// `SYS_INSTALL_RUN`). Minted only by the kernel root path.
#[cfg(feature = "installer")]
pub mod installer_rights {
    pub const INSTALL: u8 = 1 << 0;
}

/// OT-66 L1: semantic rights for the `SystemStore` cap, the authority to open
/// a mount's `.system/` (the sign-in record). Minted only by the kernel root
/// path.
#[cfg(feature = "system-store")]
pub mod system_store_rights {
    pub const STORE: u8 = 1 << 0;
}

/// Dexter #187 item 3a (J.c., option A): semantic rights for the
/// `MountAuthority` cap, the authority to mount and unmount volumes. Minted
/// only by the kernel root path.
#[cfg(feature = "mount-authority")]
pub mod mount_authority_rights {
    pub const MOUNT: u8 = 1 << 0;
}

/// L1a.1 step 2a: semantic rights of a pipe handle. A read end carries
/// `READ`, a write end `WRITE`; a handle may only do what its end allows.
#[cfg(feature = "pipe")]
pub mod pipe_rights {
    pub const READ: u8 = 1 << 0;
    pub const WRITE: u8 = 1 << 1;
}

/// Full rights for a new pipe end: `USE`, `GRANT`, `DERIVE`, and the end's
/// own semantic right.
#[cfg(feature = "pipe")]
pub fn full_pipe_rights(end: crate::ipc::pipe_logic::End) -> CapRights {
    let semantic = match end {
        crate::ipc::pipe_logic::End::Read => pipe_rights::READ,
        crate::ipc::pipe_logic::End::Write => pipe_rights::WRITE,
    };
    CapRights::new(
        meta_rights::USE | meta_rights::GRANT | meta_rights::DERIVE,
        semantic,
    )
}

/// OT-63 Rung 1 Slice 2 — semantic rights for the scarce `GpuSubmit` cap.
#[cfg(feature = "atrium-compositor-g1")]
pub mod gpu_submit_rights {
    /// Authorise a raw-scanout present on either Software or HardwareFlip.
    pub const SUBMIT: u8 = 1 << 0;
}

/// DI-1a (THINK §8.2): the input-system authority. Claim the pointer route,
/// forward/drop pointer records, take SAK chords.
pub mod input_system_rights {
    pub const SYSTEM: u8 = 1 << 0;
}

/// GPU P1 Slice 6 — semantic rights for the post-Slice-5 userland compute
/// ABI.  These rights deliberately split the three authority shapes: a buffer
/// range is not a queue, and a completion fence is never a submission right.
#[cfg(feature = "atrium-compositor-g1")]
pub mod gpu_compute_rights {
    /// Read the immutable source range of one compositor-owned GPU buffer.
    pub const BUFFER_READ: u8 = 1 << 0;
    /// Write the immutable destination range of one compositor-owned GPU buffer.
    pub const BUFFER_WRITE: u8 = 1 << 1;
    /// Submit the one pinned glass-blur manifest within the queue quota.
    pub const QUEUE_SUBMIT: u8 = 1 << 2;
    /// Await a completed compute fence.  This carries no submit authority.
    pub const FENCE_WAIT: u8 = 1 << 3;
    /// Query a completed compute fence.  This carries no submit authority.
    pub const FENCE_QUERY: u8 = 1 << 4;
}

pub const fn full_meta_rights() -> u8 {
    meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT | meta_rights::REVOKE
}

pub const fn full_channel_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        channel_rights::SEND | channel_rights::RECV | channel_rights::CLOSE,
    )
}

pub const fn full_process_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        process_rights::KILL | process_rights::WAIT | process_rights::INSPECT,
    )
}

pub const fn full_socket_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        socket_rights::SEND | socket_rights::RECV | socket_rights::BIND | socket_rights::CLOSE,
    )
}

pub const fn full_tcp_socket_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        socket_rights::READ | socket_rights::WRITE | socket_rights::CLOSE,
    )
}

pub const fn full_tcp_listener_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        socket_rights::READ | socket_rights::CLOSE,
    )
}

pub const fn full_tensor_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        tensor_rights::USE | tensor_rights::WRITE,
    )
}

/// Read-only tensor rights (Phase 7 BUILD-3): full meta-rights (so the grantee may
/// itself re-derive/grant/revoke the RO cap) but **no** `WRITE` — the canonical shape
/// for a shared/deduped immutable tensor capability.
pub const fn readonly_tensor_rights() -> CapRights {
    CapRights::new(full_meta_rights(), tensor_rights::USE)
}

pub const fn full_model_rights() -> CapRights {
    CapRights::new(full_meta_rights(), model_rights::USE)
}

/// Full (client/root) shared-pixel-buffer rights: the creator can map RW,
/// query, commit, and present. Bit values live in `atrium::logic` so the
/// host logic-tests sweep them without kernel types.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_buffer_rights() -> CapRights {
    use crate::atrium::logic::buffer_rights as b;
    CapRights::new(
        full_meta_rights(),
        b::MAP_WRITE | b::MAP_READ | b::QUERY | b::PRESENT | b::COMMIT,
    )
}

/// Client-owned root: RECEIVE authority, never ACTIVATE authority.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn client_input_focus_rights() -> CapRights {
    use crate::atrium::logic::focus_rights as f;
    CapRights::new(full_meta_rights(), f::RECEIVE)
}

/// DI-1a: the pointer route's root. RECEIVE with `USE` only: the raw mouse
/// stream cannot be derived or granted to another process.
#[cfg(feature = "atrium-pointer-forward")]
pub const fn pointer_route_rights() -> CapRights {
    use crate::atrium::logic::focus_rights as f;
    CapRights::new(meta_rights::USE, f::RECEIVE)
}

/// Compositor sibling: transferable ACTIVATE authority, but neither RECEIVE
/// nor DERIVE, so attenuation can never manufacture a receive-capable child.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn compositor_input_focus_rights() -> CapRights {
    use crate::atrium::logic::focus_rights as f;
    CapRights::new(meta_rights::USE | meta_rights::GRANT, f::ACTIVATE)
}

/// Full browser-runtime rights: THREAD_SPAWN + full meta. Root-mint only.
#[cfg(feature = "browser-runtime")]
pub const fn full_browser_runtime_rights() -> CapRights {
    CapRights::new(full_meta_rights(), browser_runtime_rights::THREAD_SPAWN)
}

/// The installer authority's only rights: `USE`, `GRANT` (so neo-init can
/// hand it to the shell and the installer screen at spawn) and `INSTALL`.
/// No `DERIVE`: there is nothing narrower to derive.
#[cfg(feature = "installer")]
pub const fn installer_cap_rights() -> CapRights {
    CapRights::new(
        meta_rights::USE | meta_rights::GRANT,
        installer_rights::INSTALL,
    )
}

/// The system store's only rights: `USE` and `STORE`. No `DERIVE` and no
/// `GRANT`: it stays in the session root (Dexter #186 item 4).
#[cfg(feature = "system-store")]
pub const fn system_store_cap_rights() -> CapRights {
    CapRights::new(meta_rights::USE, system_store_rights::STORE)
}

/// The mount authority's only rights: `USE` and `MOUNT`. No `DERIVE` and no
/// `GRANT`: it stays in the session root (Dexter #187 item 3a).
#[cfg(feature = "mount-authority")]
pub const fn mount_authority_cap_rights() -> CapRights {
    CapRights::new(meta_rights::USE, mount_authority_rights::MOUNT)
}

#[cfg(not(feature = "net-dial-public-only"))]
pub const fn full_net_dial_rights() -> CapRights {
    CapRights::new(full_meta_rights(), net_dial_rights::DIAL)
}

/// With `net-dial-public-only`, a root cap also carries `NON_PUBLIC`, so every
/// existing holder dials exactly as without the feature.
#[cfg(feature = "net-dial-public-only")]
pub const fn full_net_dial_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        net_dial_rights::DIAL | net_dial_rights::NON_PUBLIC,
    )
}

/// Full (compositor root) GPU-submit rights: SUBMIT + full meta. Minted only by
/// a privileged path (the compositor-root grant); a client leaf is the
/// DERIVE-time attenuation. The scarcity is the point — an un-granted process
/// holds no `GpuSubmit` cap, so with enforcement armed it cannot submit a flip.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_gpu_submit_rights() -> CapRights {
    CapRights::new(full_meta_rights(), gpu_submit_rights::SUBMIT)
}

/// The input-system authority's ONLY rights: `USE` + `SYSTEM`. No `DERIVE`,
/// no `GRANT`: it cannot be delegated, so holding it is exact (Jarvis F3).
#[cfg(feature = "atrium-compositor-g1")]
pub const fn input_system_rights() -> CapRights {
    CapRights::new(meta_rights::USE, input_system_rights::SYSTEM)
}

/// Full trusted-compositor queue authority.  No userspace syscall creates this
/// cap; it is installed only by the kernel after the graphics-domain gate is
/// active.  Derivation can only attenuate it.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_gpu_compute_queue_rights() -> CapRights {
    CapRights::new(full_meta_rights(), gpu_compute_rights::QUEUE_SUBMIT)
}

/// Full source-buffer authority for one fixed GPU buffer range.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_gpu_compute_read_buffer_rights() -> CapRights {
    CapRights::new(full_meta_rights(), gpu_compute_rights::BUFFER_READ)
}

/// Full destination-buffer authority for one fixed GPU buffer range.
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_gpu_compute_write_buffer_rights() -> CapRights {
    CapRights::new(full_meta_rights(), gpu_compute_rights::BUFFER_WRITE)
}

/// A submitted fence is wait/query-only.  It may be transferred or revoked but
/// cannot be attenuated into a queue or buffer capability because the handle
/// variant is preserved by [`Handle::with_cap`].
#[cfg(feature = "atrium-compositor-g1")]
pub const fn full_gpu_compute_fence_rights() -> CapRights {
    CapRights::new(
        full_meta_rights(),
        gpu_compute_rights::FENCE_WAIT | gpu_compute_rights::FENCE_QUERY,
    )
}

/// Allocate the next pid (>= 1). pid 0 is reserved for the kernel and is
/// never returned by this function.
pub fn alloc_pid() -> ProcessId {
    ProcessId(NEXT_PID.fetch_add(1, Ordering::Relaxed))
}

/// A process. Owns its address space (`pmap`) and a per-process IPC
/// handle table; threads live in the scheduler's queues and reference
/// the process by pid.
///
/// Phase 1.5b: pmap is `Arc<Pmap>` so an IPC channel endpoint can hold a
/// translate-capable reference to a peer's address space across the
/// rendezvous (the receiver translates the sender's user buffer without
/// switching CR3). Pmap is built mutably during ELF load, then handed
/// to the Process constructor which wraps it in Arc; after that point
/// it is read-only until BUILD-4 (sys_brk) introduces interior mutability.
pub struct Process {
    pub pid: ProcessId,
    pub name: &'static str,
    /// `None` for the kernel process. `Some` for user processes.
    pub pmap: Option<Arc<Pmap>>,
    /// Per-process IPC handle table. Phase 1.5b: only IPC channel handles
    /// live here. Phase 2 extends with capability-permission bits and
    /// other kernel-object handles (tensors, models, files).
    /// Process-context capability state: syscall/process cleanup paths may
    /// acquire under explicit IF-off discipline, but no ISR acquires it.
    pub handles: SwitchMutex<HandleTable>,
    /// Per-process derivation log. Maps child CapId to parent CapId; `None`
    /// means root-minted by the kernel. Revoke walks these logs across the
    /// process registry. Every read and write takes the caller's `CapGraph`
    /// (K1b, Muse's #166 item 5): only a holder of `CAP_GRAPH` can see or
    /// change an edge. No interrupt handler walks or mutates it.
    pub derivations: edges::Derivations,
    /// Set before teardown drains the handle table. Capability install paths
    /// check this under the recipient handle lock so grants cannot land in a
    /// process that is already being torn down.
    exiting: AtomicBool,
    /// B1c AAG J3/J4: threads of this process inside an inference syscall
    /// (marked participants). Terminate's accounting counts them as handled:
    /// they exit themselves at the syscall boundary.
    #[cfg(feature = "inference-stack")]
    pub(crate) inference_participants: AtomicU32,
    /// OT-61 C1.1 (blocker C): a ring-3 #PF fault-killed this process. Set from
    /// the exception frame (`fault_kill_current_user`), which does only the
    /// minimum there. The heavy sibling drain (IPC-parked + process-wait-parked
    /// threads) is deferred to the REAPER's normal kernel context
    /// (`on_thread_reap_threaded`), where walking the handle table / wait
    /// queues is safe — never from the fragile #PF exception frame.
    #[cfg(feature = "browser-runtime")]
    fault_killing: AtomicBool,
    /// OT-93: `terminate` (a force-kill) claimed this process. Its threads
    /// that the kill did not zombify (Ready inside a syscall, or running) read
    /// this at their next boundary and exit there (`scheduler::leave_syscall`).
    /// Counted in [`FORCED_ALIVE`] until the process drops.
    force_killed: AtomicBool,
    /// OT-93 (the PM's #204 bounce, item 1): how many threads anchor this
    /// process (`thread::ProcessAnchor`): every user thread, from its creation
    /// to its `Thread::free` at the reap. A kill completes, its waiters woken
    /// and EXITED published, only once this is 0 and `terminate`'s teardown is
    /// done (`finish_kill`), whatever state each thread was in when the kill
    /// looked, so no snapshot of where the threads are decides it.
    live_threads: AtomicUsize,
    /// OT-61 C2.2b: what this process's spawn charged to its parent's
    /// account (the process, its first thread and kernel stack). Released
    /// when the `Process` drops, after its last thread is freed.
    #[cfg(feature = "obj-quota")]
    obj_spawn: SwitchMutex<Option<crate::mm::mem_account_logic::ObjCharge>>,
    /// One-shot process-exit owner. Natural exit and forced kill CAS this
    /// from RUNNING to their own in-progress state; the loser leaves cleanup
    /// to the winner.
    exit_state: AtomicU8,
    /// Process-visible IPv6 addresses assigned by the kernel SLAAC table.
    /// BUILD-2 owns mutation; userland consumes these through socket bind
    /// and later stats/query APIs.
    pub ipv6_addresses: SwitchMutex<Vec<[u8; 16]>>,
    /// Exit status recorded by `sys_exit`. Phase 1.5c's test harness uses
    /// the root process status to prevent leak-clean-but-semantically-red
    /// boots from passing. Future `sys_wait` can consume the same field.
    exit_code: AtomicI32,
    /// Threads blocked in `sys_wait` on this process, plus the one-shot exit
    /// completion state. Capacity is reserved at construction so parking a
    /// waiter cannot allocate while the scheduler lock is held.
    ///
    /// Lock type: `IrqSafeMutex<ProcessWaitQueue>`. This state is touched from
    /// IF=0 scheduler/reaper paths and is held across
    /// `block_current_into_with_guard`; `IrqSafeGuard` provides the same
    /// context-switch release token while preserving the IF-off discipline.
    waiters: IrqSafeMutex<ProcessWaitQueue>,
    /// OT-61 C1.1: per-process thread registry — the thread refcount (the
    /// `Pmap`/`Process` drops only on the LAST thread's reap), the
    /// join/detach/exit-status records, and each thread's user-stack bounds
    /// (`thread_get_info`). Lock type `IrqSafeMutex`: `thread_join` parks
    /// holding this guard (`try_block_current_with_token_and_guard` needs the
    /// context-switch release token), and exit paths touch it under IF=0.
    /// Leaf lock; ordered before the scheduler lock, never after (CP-1).
    #[cfg(feature = "browser-runtime")]
    threads: IrqSafeMutex<ThreadRegistry>,
}

/// OT-61 C1.1: user-stack bounds retained per thread (downward stack:
/// `usable_low..usable_low+usable_len` is mapped; the guard page, when
/// present, is `guard_low..guard_low+guard_len` immediately below). Valid
/// from creation through exit/reap (the record lives until the reaper
/// removes it).
#[cfg(feature = "browser-runtime")]
#[derive(Copy, Clone)]
pub struct ThreadStackBounds {
    pub usable_low: u64,
    pub usable_len: u64,
    pub guard_low: u64,
    pub guard_len: u64,
}

#[cfg(feature = "browser-runtime")]
pub const THREAD_NAME_CAP: usize = 32;

#[cfg(feature = "browser-runtime")]
struct ThreadRecord {
    tid: u64,
    done: bool,
    exit_code: i32,
    detached: bool,
    /// A joiner is parked (or the exit status has been consumed) — a second
    /// join/detach on the same target is `EINVAL` (single-joiner, POSIX).
    join_claimed: bool,
    /// The thread object has been reaped. A joinable-but-unconsumed record
    /// survives reap as a ZOMBIE record (the POSIX shape: the exit status is
    /// collectable until joined or the process ends); a later join collects
    /// the status and drops the record.
    reaped: bool,
    /// A joiner has read the exit status. The record is dropped only once it is
    /// BOTH `reaped` (so the reaper has unmapped the created stack — blocker B)
    /// AND `collected`/`detached` (so nobody still needs the status). This
    /// decoupling makes the stack-unmap happen exactly once at reap regardless
    /// of whether join collects before or after the reap runs.
    collected: bool,
    /// OT-61 C1.1 (blocker F): the frozen per-thread **done-word**. `0` while
    /// the thread runs, `1` once it exits. Boxed so its address is stable
    /// across registry `Vec` reallocation; a `thread_join` parks on the futex
    /// queue key `join_key(tid)` and re-checks THIS word (lock-free, under the
    /// futex queue guard) for the lost-wakeup discipline, and `thread_exit`
    /// stores `1` here then `futex::wake(join_key(tid))`. There is no private
    /// join-token store — join and the user futex share one wait mechanism.
    done_word: Box<AtomicU32>,
    /// OT-61 C1.1 (blocker B): the created thread's user stack, freed exactly
    /// once at reap when the process survives (the whole-`Pmap` drop handles
    /// the last-thread case). `false` for the adopted INITIAL process thread,
    /// whose stack is `spawn_user`'s and must never be unmapped here.
    created: bool,
    stack: ThreadStackBounds,
    name: [u8; THREAD_NAME_CAP],
    name_len: usize,
}

/// Bounded per-process thread count. Well above the C1 batteries; a full
/// registry fails `thread_create` closed with `-EAGAIN`. Capacity is
/// reserved at construction so record insertion never allocates under the
/// registry spinlock (the `ProcessWaitQueue` discipline).
#[cfg(feature = "browser-runtime")]
const MAX_THREADS_PER_PROCESS: usize = 16;

#[cfg(feature = "browser-runtime")]
pub struct ThreadRegistry {
    records: Vec<ThreadRecord>,
    /// Live (not-yet-reaped) threads. Reaching 0 on a reap triggers the
    /// process teardown that pre-C1.1 ran on ANY reap.
    live: u32,
}

#[cfg(feature = "browser-runtime")]
impl ThreadRegistry {
    fn new() -> Self {
        Self {
            records: Vec::with_capacity(MAX_THREADS_PER_PROCESS),
            live: 0,
        }
    }

    fn find(&mut self, tid: u64) -> Option<&mut ThreadRecord> {
        self.records.iter_mut().find(|r| r.tid == tid)
    }

    fn insert(&mut self, tid: u64, stack: ThreadStackBounds, created: bool) -> Result<(), ()> {
        if self.records.len() >= MAX_THREADS_PER_PROCESS {
            return Err(());
        }
        self.records.push(ThreadRecord {
            tid,
            done: false,
            exit_code: 0,
            detached: false,
            join_claimed: false,
            reaped: false,
            collected: false,
            done_word: Box::new(AtomicU32::new(0)),
            created,
            stack,
            name: [0; THREAD_NAME_CAP],
            name_len: 0,
        });
        self.live += 1;
        Ok(())
    }

    /// Roll back a record for a thread that never started (push-ready
    /// failure). Distinct from `note_reaped` so the "last thread" teardown
    /// cannot fire from a rollback.
    fn remove_unstarted(&mut self, tid: u64) {
        if let Some(pos) = self.records.iter().position(|r| r.tid == tid) {
            self.records.remove(pos);
            self.live = self.live.saturating_sub(1);
        }
    }

    /// Account a reaped thread. Returns `(last, free_stack)`:
    /// - `last` is true when this was the LAST live thread (⇒ run the process
    ///   teardown; a tid without a record — a process built outside
    ///   `spawn_user` — preserves the pre-C1.1 any-reap-tears-down behavior).
    /// - `free_stack` is `Some(bounds)` when the caller must unmap this
    ///   thread's created user stack now (blocker B): only for a `created`
    ///   thread when the process SURVIVES (`!last`). On the last thread the
    ///   whole `Pmap` drops and frees every stack, so per-stack unmap is
    ///   skipped; the initial (`!created`) thread's stack is `spawn_user`'s
    ///   and is never unmapped here.
    ///
    /// Record lifetime (blocker F): a DETACHED record is dropped here (nobody
    /// collects it); a joinable record (claimed or not) SURVIVES as a zombie
    /// so a `thread_join` can still read the exit status from it — the joiner
    /// (or process teardown) drops it. Un-collected zombies are bounded by
    /// `MAX_THREADS_PER_PROCESS`. The `done_word` Box lives with the record,
    /// so a joiner parked on it keeps a valid pointer until it collects.
    fn note_reaped(&mut self, tid: u64) -> (bool, Option<ThreadStackBounds>) {
        let mut free_stack = None;
        if let Some(pos) = self.records.iter().position(|r| r.tid == tid) {
            self.live -= 1;
            let last = self.live == 0;
            let rec = &mut self.records[pos];
            rec.reaped = true;
            // Blocker B: the reap is the single point that unmaps the created
            // stack — the bounds live in the record, which must therefore
            // survive until here (join no longer removes an un-reaped record).
            if rec.created && !last {
                free_stack = Some(rec.stack);
            }
            // Drop the record now only if nobody still needs the exit status:
            // detached, or a joiner already read it (collected). Otherwise keep
            // it as a zombie so a future/pending join can collect the status.
            if rec.detached || rec.collected {
                self.records.remove(pos);
            }
            return (last, free_stack);
        }
        (self.live == 0, free_stack)
    }
}

impl Process {
    /// Teardown has begun (OT-22: set before any wait-table purge; CP-66: set
    /// before `admission::cancel_pid`).
    pub fn is_exiting(&self) -> bool {
        self.exiting.load(Ordering::Acquire)
    }

    pub fn new_kernel(name: &'static str) -> Self {
        Self {
            pid: KERNEL_PID,
            name,
            pmap: None,
            handles: SwitchMutex::new(HandleTable::new()),
            derivations: edges::Derivations::new(),
            exiting: AtomicBool::new(false),
            #[cfg(feature = "inference-stack")]
            inference_participants: AtomicU32::new(0),
            #[cfg(feature = "browser-runtime")]
            fault_killing: AtomicBool::new(false),
            force_killed: AtomicBool::new(false),
            live_threads: AtomicUsize::new(0),
            #[cfg(feature = "obj-quota")]
            obj_spawn: SwitchMutex::new(None),
            exit_state: AtomicU8::new(EXIT_STATE_RUNNING),
            ipv6_addresses: SwitchMutex::new(Vec::new()),
            exit_code: AtomicI32::new(EXIT_CODE_RUNNING),
            waiters: IrqSafeMutex::new(ProcessWaitQueue::new()),
            #[cfg(feature = "browser-runtime")]
            threads: IrqSafeMutex::new(ThreadRegistry::new()),
        }
    }

    pub fn new_user(name: &'static str, pmap: Pmap) -> Self {
        // OT-61 C2.2b: the process's handles charge its own account.
        #[cfg(feature = "obj-quota")]
        let handles = HandleTable::with_account(pmap.account().cloned());
        #[cfg(not(feature = "obj-quota"))]
        let handles = HandleTable::new();
        Self {
            pid: alloc_pid(),
            name,
            pmap: Some(Arc::new(pmap)),
            handles: SwitchMutex::new(handles),
            derivations: edges::Derivations::new(),
            exiting: AtomicBool::new(false),
            #[cfg(feature = "inference-stack")]
            inference_participants: AtomicU32::new(0),
            #[cfg(feature = "browser-runtime")]
            fault_killing: AtomicBool::new(false),
            force_killed: AtomicBool::new(false),
            live_threads: AtomicUsize::new(0),
            #[cfg(feature = "obj-quota")]
            obj_spawn: SwitchMutex::new(None),
            exit_state: AtomicU8::new(EXIT_STATE_RUNNING),
            ipv6_addresses: SwitchMutex::new(Vec::new()),
            exit_code: AtomicI32::new(EXIT_CODE_RUNNING),
            waiters: IrqSafeMutex::new(ProcessWaitQueue::new()),
            #[cfg(feature = "browser-runtime")]
            threads: IrqSafeMutex::new(ThreadRegistry::new()),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ProcessWaitCompletion {
    Running,
    ExitedUnconsumed(i32),
    Consumed,
}

struct ProcessWaitQueue {
    #[allow(clippy::vec_box)]
    // Box gives pointer-stable, alloc-free ownership handoff between the wait queue and scheduler batch; take_all must not allocate under the waiters spinlock.
    waiters: Vec<Box<Thread>>,
    completion: ProcessWaitCompletion,
}

impl ProcessWaitQueue {
    fn new() -> Self {
        Self {
            waiters: Vec::with_capacity(PROCESS_WAIT_QUEUE_CAPACITY),
            completion: ProcessWaitCompletion::Running,
        }
    }

    fn is_full(&self) -> bool {
        self.waiters.len() >= self.waiters.capacity()
    }

    fn push(&mut self, waiter: Box<Thread>) {
        debug_assert!(
            self.waiters.len() < self.waiters.capacity(),
            "sys_wait queue push would allocate"
        );
        self.waiters.push(waiter);
    }

    fn consume_completion(&mut self) -> Option<i32> {
        match self.completion {
            ProcessWaitCompletion::ExitedUnconsumed(code) => {
                self.completion = ProcessWaitCompletion::Consumed;
                Some(code)
            }
            _ => None,
        }
    }

    fn is_consumed(&self) -> bool {
        self.completion == ProcessWaitCompletion::Consumed
    }

    fn complete(&mut self, code: Option<i32>) {
        self.completion = match code {
            Some(code) if self.waiters.is_empty() => ProcessWaitCompletion::ExitedUnconsumed(code),
            _ => ProcessWaitCompletion::Consumed,
        };
    }

    fn take_all(&mut self) -> ProcessWaiterBatch {
        let mut batch = ProcessWaiterBatch::new();
        while !self.waiters.is_empty() {
            batch.push(self.waiters.remove(0));
        }
        batch
    }

    /// OT-61 C1.1 (blocker C): remove and return one waiter owned by `pid`, if
    /// any. Used by fault/forced kill to extract a sibling that is blocked in
    /// `sys_wait` on THIS (target) process, so the killed process's live-thread
    /// count can reach zero and its teardown completes. OT-98: and by
    /// `terminate`, in every build.
    fn extract_by_pid(&mut self, pid: ProcessId) -> Option<Box<Thread>> {
        let pos = self.waiters.iter().position(|w| w.pid() == pid)?;
        Some(self.waiters.remove(pos))
    }
}

struct ProcessWaiterBatch {
    waiters: [Option<Box<Thread>>; PROCESS_WAIT_QUEUE_CAPACITY],
    len: usize,
}

impl ProcessWaiterBatch {
    fn new() -> Self {
        Self {
            waiters: core::array::from_fn(|_| None),
            len: 0,
        }
    }

    fn push(&mut self, waiter: Box<Thread>) {
        if self.len < PROCESS_WAIT_QUEUE_CAPACITY {
            self.waiters[self.len] = Some(waiter);
            self.len += 1;
        }
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn len(&self) -> usize {
        self.len
    }
}

pub struct ProcessInspectSnapshot {
    pub state: u32,
    pub pid: ProcessId,
    pub exit_code: i32,
    pub thread_count: u32,
    pub handle_count: u32,
    pub channel_handle_count: u32,
    /// Live handles conferring network authority (`NetDial`/`Socket`/
    /// `TcpSocket`/`TcpListener`). Zero ⇒ the process holds no network cap —
    /// the I-14(a) containment witness for a secret-holding subsystem.
    pub network_handle_count: u32,
    pub name: &'static str,
}

// === Handle table (Phase 1.5b) =========================================

/// One entry in a process's handle table. Phase 1.5b ships two variants:
///   - `Channel` — one endpoint of an IPC channel (BUILD-3, ADR-004).
///   - `Process` — capability to terminate a spawned child (BUILD-5,
///     ADR-007). `sys_spawn` installs one in the parent's table; the
///     parent uses it with `sys_kill`.
///   - `File` — Phase 3 BUILD-1 file capability. It carries `FileRights`
///     rather than the legacy `CapRights` byte pair; generic derive/grant
///     semantics for files remain deferred.
///
/// **Phase 2 capability extension point — do not flatten.** New
/// capability types (Tensor, Model, InferenceTicket, File per
/// ANSPEC-001 §3.7) will land as additional variants here, NOT as
/// parallel handle tables. The EBADF-on-wrong-variant gating in
/// `ipc_lookup` / `process_handle_lookup` is what makes the enum
/// useful — without it, the type system can't catch
/// `sys_send(process_handle, ...)` mix-ups. Phase 2 also adds
/// permission bits (READ/WRITE/SHARE/KILL) per-variant.
#[derive(Clone)]
pub enum Handle {
    Channel {
        channel: Arc<Channel>,
        side: EndpointSide,
        cap_id: CapId,
        rights: CapRights,
    },
    /// ProcessHandle — capability to terminate `target`. Issued by
    /// `sys_spawn` to the parent. Per ADR-007 §Q1, the handle IS the
    /// authorization; there is no PID-keyed kill path.
    Process {
        target: ProcessId,
        cap_id: CapId,
        rights: CapRights,
    },
    File {
        file: Arc<OpenFile>,
        cap_id: CapId,
        rights: FileRights,
    },
    Socket {
        socket: Arc<Socket>,
        cap_id: CapId,
        rights: CapRights,
    },
    TcpSocket {
        socket: Arc<TcpSocket>,
        cap_id: CapId,
        rights: CapRights,
    },
    TcpListener {
        listener: Arc<TcpListener>,
        cap_id: CapId,
        rights: CapRights,
    },
    /// Outbound-network authority. The owner/root cap uses `*`:0; every
    /// user-facing grant is a concrete `(host, port)` attenuation matching
    /// `Grant::NetDial`. Scope is immutable, so syscall checks snapshot it by
    /// cloning the Arc while holding the handle-table lock and then drop the
    /// lock before DNS/TCP work.
    NetDial {
        host: Arc<str>,
        port: u16,
        cap_id: CapId,
        rights: CapRights,
    },
    /// BUILD-O1b (ADR-043): the handle NAMES the tensor; the arena owns it.
    /// `slot` is the generational key (stale ⇒ every use fails safe, I-13);
    /// `meta` caches the immutable identity so metadata paths never mint.
    /// Table membership carries the handle count: `HandleTable` install
    /// retains, remove/revoke/drain release (the GEM `handle_count` model);
    /// transient `Handle` copies carry no ownership at all.
    Tensor {
        slot: crate::ai::arena::SlotKey,
        meta: crate::ai::tensor::TensorMeta,
        cap_id: CapId,
        rights: CapRights,
    },
    Model {
        model: Arc<Model>,
        cap_id: CapId,
        rights: CapRights,
    },
    /// Atrium G1 (Rung 0): the shared pixel-buffer capability. The handle
    /// holds the object `Arc` directly — dropping the last handle (and the
    /// last mapping pin) frees the frames. Semantic rights bits are
    /// `atrium::logic::buffer_rights`; the compositor role is the
    /// MAP_READ-without-MAP_WRITE attenuation (hardware RO/NX at map time).
    #[cfg(feature = "atrium-compositor-g1")]
    Buffer {
        buffer: Arc<crate::atrium::buffer::SharedPixelBuffer>,
        cap_id: CapId,
        rights: CapRights,
    },
    /// Atrium G1 (Rung 0): the kernel-routed input-focus capability.
    /// Semantic rights bits are `atrium::logic::focus_rights` (ACTIVATE =
    /// the compositor root; RECEIVE = the client leaf).
    #[cfg(feature = "atrium-compositor-g1")]
    InputFocus {
        focus: Arc<crate::atrium::focus::InputFocus>,
        cap_id: CapId,
        rights: CapRights,
    },
    /// Atrium G1 (OT-63 Rung 1 Slice 2 + showcase AAG): the scarce raw-scanout
    /// authority. The handle carries no object — it IS the authority to drive a
    /// Software framebuffer blit or `HardwareFlip` present when enforcement is
    /// armed. Semantic rights are `gpu_submit_rights` (SUBMIT).
    /// Minted only by a privileged path (`full_gpu_submit_rights`), so a
    /// non-grantee cannot self-mint one and bypass submit enforcement.
    #[cfg(feature = "atrium-compositor-g1")]
    GpuSubmit { cap_id: CapId, rights: CapRights },
    /// DI-1a: the input-system authority (THINK §8.2). No object — it IS the
    /// authority. Minted only by the kernel into the session root, with
    /// `input_system_rights()` (no DERIVE/GRANT), so it cannot be delegated
    /// and there is no create syscall.
    #[cfg(feature = "atrium-compositor-g1")]
    InputSystem { cap_id: CapId, rights: CapRights },
    /// OT-61 C1.1: the browser-runtime thread/VM authority (the `NetDial`
    /// mint pattern). The handle carries no object — it IS the authority to
    /// create threads (`browser_runtime_rights::THREAD_SPAWN`). Minted only
    /// by `root_browser_runtime_cap()` (kernel root path, installed into the
    /// trusted init process at spawn); there is deliberately no create
    /// syscall, so a process cannot self-mint thread authority.
    #[cfg(feature = "browser-runtime")]
    BrowserRuntime { cap_id: CapId, rights: CapRights },
    /// OT-87 R4.4: the installer authority. No object: it IS the authority
    /// to list install targets and run an install. Minted only by
    /// `root_installer_cap()` into neo-init at spawn; no create syscall.
    #[cfg(feature = "installer")]
    Installer { cap_id: CapId, rights: CapRights },
    /// OT-66 L1: the system-store authority. No object: it IS the authority
    /// to open a mount's `.system/`. Minted only by `root_system_store_cap()`
    /// into neo-init at spawn; no create syscall, never inherited or granted.
    #[cfg(feature = "system-store")]
    SystemStore { cap_id: CapId, rights: CapRights },
    /// Dexter #187 item 3a: the mount authority. No object: it IS the
    /// authority to `SYS_MOUNT` and `SYS_UMOUNT`. Minted only by
    /// `root_mount_authority_cap()` into neo-init at spawn; no create
    /// syscall, never inherited or granted.
    #[cfg(feature = "mount-authority")]
    MountAuthority { cap_id: CapId, rights: CapRights },
    /// L1a.1 step 2a: one end of a byte pipe. Endpoints are counted per
    /// table entry (`retain_pipe` / `release_pipe`), so a granted copy keeps
    /// its end open after the original closes. Semantic rights:
    /// `pipe_rights::READ` on a read end, `pipe_rights::WRITE` on a write end.
    #[cfg(feature = "pipe")]
    Pipe {
        pipe: Arc<crate::ipc::pipe::PipeObject>,
        end: crate::ipc::pipe_logic::End,
        cap_id: CapId,
        rights: CapRights,
    },
    /// GPU P1 Slice 6 — post-Slice-5 userland compute capabilities.  The
    /// nested kind realizes the concrete three-cap split without adding a
    /// second handle table: `Buffer` is a generation-tagged immutable range,
    /// `Queue` is the manifest/quota submit authority, and `Fence` is
    /// wait/query only.  A `GpuCompute` handle is minted only by trusted kernel
    /// paths (root grant or successful submit); there is deliberately no
    /// `SYS_GPU_COMPUTE_CREATE`.
    #[cfg(feature = "atrium-compositor-g1")]
    GpuCompute {
        cap: GpuComputeCap,
        cap_id: CapId,
        rights: CapRights,
    },
}

/// The object carried by [`Handle::GpuCompute`].  Generation is monotonically
/// allocated by the kernel and makes an old buffer/fence identity impossible to
/// confuse with a later one even if a handle-table index is reused.
#[cfg(feature = "atrium-compositor-g1")]
#[derive(Clone)]
pub enum GpuComputeCap {
    Buffer {
        buffer: Arc<crate::atrium::buffer::SharedPixelBuffer>,
        generation: u64,
        offset: u64,
        len: u64,
    },
    Queue {
        /// Upper bound for one pinned-manifest source/destination range.
        byte_quota: u64,
    },
    Fence {
        token: crate::drivers::accel::DispatchToken,
        generation: u64,
    },
}

impl Handle {
    pub fn cap_id(&self) -> CapId {
        match self {
            Handle::Channel { cap_id, .. }
            | Handle::Process { cap_id, .. }
            | Handle::File { cap_id, .. }
            | Handle::Socket { cap_id, .. }
            | Handle::TcpSocket { cap_id, .. }
            | Handle::TcpListener { cap_id, .. }
            | Handle::NetDial { cap_id, .. }
            | Handle::Tensor { cap_id, .. }
            | Handle::Model { cap_id, .. } => *cap_id,
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { cap_id, .. } => *cap_id,
            #[cfg(feature = "installer")]
            Handle::Installer { cap_id, .. } => *cap_id,
            #[cfg(feature = "system-store")]
            Handle::SystemStore { cap_id, .. } => *cap_id,
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { cap_id, .. } => *cap_id,
            #[cfg(feature = "pipe")]
            Handle::Pipe { cap_id, .. } => *cap_id,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { cap_id, .. }
            | Handle::InputFocus { cap_id, .. }
            | Handle::GpuSubmit { cap_id, .. }
            | Handle::InputSystem { cap_id, .. }
            | Handle::GpuCompute { cap_id, .. } => *cap_id,
        }
    }

    pub fn rights(&self) -> CapRights {
        match self {
            Handle::Channel { rights, .. }
            | Handle::Process { rights, .. }
            | Handle::Socket { rights, .. }
            | Handle::TcpSocket { rights, .. }
            | Handle::TcpListener { rights, .. }
            | Handle::NetDial { rights, .. }
            | Handle::Tensor { rights, .. }
            | Handle::Model { rights, .. } => *rights,
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { rights, .. } => *rights,
            #[cfg(feature = "installer")]
            Handle::Installer { rights, .. } => *rights,
            #[cfg(feature = "system-store")]
            Handle::SystemStore { rights, .. } => *rights,
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { rights, .. } => *rights,
            #[cfg(feature = "pipe")]
            Handle::Pipe { rights, .. } => *rights,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { rights, .. }
            | Handle::InputFocus { rights, .. }
            | Handle::GpuSubmit { rights, .. }
            | Handle::InputSystem { rights, .. }
            | Handle::GpuCompute { rights, .. } => *rights,
            // File handles are first-class caps (I-15): their `FileRights` map
            // into `CapRights` so `has_meta`/`is_subset_of` gate file
            // derivation/granting. With `file-caps` off this is `(0, 0)` — the
            // pre-unification behavior, bit-for-bit (the Rule-1 kill-switch).
            Handle::File { rights, .. } => {
                let (meta, semantic) = file_cap_logic::file_rights_to_cap(
                    rights.bits(),
                    file_cap_logic::FILE_CAPS_ENABLED,
                );
                CapRights::new(meta, semantic)
            }
        }
    }

    /// OT-61 C2.1: this handle's `SYS_HANDLE_INFO` record (kind and rights;
    /// never the object itself).
    #[cfg(any(
        feature = "browser-runtime",
        feature = "cap-rooted-open",
        feature = "installer"
    ))]
    pub fn handle_info(&self) -> super::handle_info_logic::HandleInfo {
        use super::handle_info_logic as hi;
        let kind = match self {
            Handle::Channel { .. } => hi::KIND_CHANNEL,
            Handle::Process { .. } => hi::KIND_PROCESS,
            Handle::File { .. } => hi::KIND_FILE,
            Handle::Socket { .. } => hi::KIND_SOCKET,
            Handle::TcpSocket { .. } => hi::KIND_TCP_SOCKET,
            Handle::TcpListener { .. } => hi::KIND_TCP_LISTENER,
            Handle::NetDial { .. } => hi::KIND_NET_DIAL,
            Handle::Tensor { .. } => hi::KIND_TENSOR,
            Handle::Model { .. } => hi::KIND_MODEL,
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { .. } => hi::KIND_BROWSER_RUNTIME,
            #[cfg(feature = "installer")]
            Handle::Installer { .. } => hi::KIND_INSTALLER,
            #[cfg(feature = "system-store")]
            Handle::SystemStore { .. } => hi::KIND_SYSTEM_STORE,
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { .. } => hi::KIND_MOUNT_AUTHORITY,
            #[cfg(feature = "pipe")]
            Handle::Pipe { .. } => hi::KIND_PIPE,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { .. } => hi::KIND_BUFFER,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::InputFocus { .. } => hi::KIND_INPUT_FOCUS,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::GpuSubmit { .. } => hi::KIND_GPU_SUBMIT,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::InputSystem { .. } => hi::KIND_INPUT_SYSTEM,
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::GpuCompute { .. } => hi::KIND_GPU_COMPUTE,
        };
        let rights = self.rights();
        #[cfg(feature = "cap-rooted-open")]
        let flags = match self {
            Handle::File { file, .. } if file.is_vfs_root() => hi::FLAG_ROOT_DIR,
            _ => 0,
        };
        #[cfg(not(feature = "cap-rooted-open"))]
        let flags = 0;
        hi::HandleInfo {
            kind,
            file_rights: self.file_rights().map_or(0, |r| r.bits()),
            cap_meta: rights.meta,
            cap_semantic: rights.semantic,
            flags,
        }
    }

    pub fn file_rights(&self) -> Option<FileRights> {
        match self {
            Handle::File { rights, .. } => Some(*rights),
            _ => None,
        }
    }

    pub fn with_cap(self, cap_id: CapId, rights: CapRights) -> Self {
        match self {
            Handle::Channel { channel, side, .. } => Handle::Channel {
                channel,
                side,
                cap_id,
                rights,
            },
            Handle::Process { target, .. } => Handle::Process {
                target,
                cap_id,
                rights,
            },
            Handle::File {
                file,
                rights: file_rights,
                ..
            } => {
                // I-15: attenuate the file rights to the requested subset (child ⊆
                // parent). With `file-caps` off this ignores `rights` and keeps the
                // original `FileRights` — the pre-unification `with_cap`, bit-for-bit.
                let new_bits = file_cap_logic::attenuate_file_rights(
                    file_rights.bits(),
                    rights.meta,
                    rights.semantic,
                    file_cap_logic::FILE_CAPS_ENABLED,
                );
                Handle::File {
                    file,
                    cap_id,
                    rights: FileRights::from_bits(new_bits),
                }
            }
            Handle::Socket { socket, .. } => Handle::Socket {
                socket,
                cap_id,
                rights,
            },
            Handle::TcpSocket { socket, .. } => Handle::TcpSocket {
                socket,
                cap_id,
                rights,
            },
            Handle::TcpListener { listener, .. } => Handle::TcpListener {
                listener,
                cap_id,
                rights,
            },
            Handle::NetDial { host, port, .. } => Handle::NetDial {
                host,
                port,
                cap_id,
                rights,
            },
            Handle::Tensor { slot, meta, .. } => Handle::Tensor {
                slot,
                meta,
                cap_id,
                rights,
            },
            Handle::Model { model, .. } => Handle::Model {
                model,
                cap_id,
                rights,
            },
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { .. } => Handle::BrowserRuntime { cap_id, rights },
            #[cfg(feature = "installer")]
            Handle::Installer { .. } => Handle::Installer { cap_id, rights },
            #[cfg(feature = "system-store")]
            Handle::SystemStore { .. } => Handle::SystemStore { cap_id, rights },
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { .. } => Handle::MountAuthority { cap_id, rights },
            #[cfg(feature = "pipe")]
            Handle::Pipe { pipe, end, .. } => Handle::Pipe {
                pipe,
                end,
                cap_id,
                rights,
            },
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { buffer, .. } => Handle::Buffer {
                buffer,
                cap_id,
                rights,
            },
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::InputFocus { focus, .. } => Handle::InputFocus {
                focus,
                cap_id,
                rights,
            },
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::GpuSubmit { .. } => Handle::GpuSubmit { cap_id, rights },
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::InputSystem { .. } => Handle::InputSystem { cap_id, rights },
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::GpuCompute { cap, .. } => Handle::GpuCompute {
                cap,
                cap_id,
                rights,
            },
        }
    }
}

/// OT-66 K1a: the RootDirCap's rights: every file right the root form can
/// grant, plus DERIVE and GRANT so init can hand derived copies on.
#[cfg(feature = "cap-rooted-open")]
pub const ROOT_DIR_CAP_RIGHTS: FileRights = FileRights::from_bits(
    FileRights::KNOWN_BUILD4B.bits()
        | FileRights::GRANT_RESERVED.bits()
        | FileRights::DERIVE_RESERVED.bits(),
);

/// OT-66 K1a: mint the RootDirCap, an ordinary directory file handle on "/".
/// Kernel-only, no syscall twin: installed into init at spawn (the
/// `root_net_dial_cap` pattern), after its other root caps, so their indices
/// don't move. It moves only by derive, grant or spawn inheritance.
#[cfg(feature = "cap-rooted-open")]
pub fn root_dir_cap() -> Handle {
    let dir_rights = FileRights::from_bits(
        FileRights::LOOKUP.bits() | FileRights::READDIR.bits() | FileRights::MOUNT.bits(),
    );
    let file = crate::fs::open_root_temporary("/", crate::fs::OPEN_ROOT_TEMPORARY, dir_rights)
        .expect("the VFS root opens for the RootDirCap");
    Handle::File {
        file,
        cap_id: alloc_cap_id(),
        rights: ROOT_DIR_CAP_RIGHTS,
    }
}

/// OT-66 K1a/K1b: the directory cap through which `proc` may make a
/// root-form open of `path` (normalized, as `fs::normalize_absolute_path`
/// gives it) for `requested`: a held directory handle whose scope (the path
/// it was opened at) contains `path` and whose rights cover `requested`. The
/// RootDirCap is the scope "/"; K1b gives a client only the scopes it needs.
/// `EPERM` when no held scope contains `path`; `EACCES` when every one that
/// does is too narrow.
#[cfg(feature = "cap-rooted-open")]
pub fn root_dir_cap_for(
    proc: &Process,
    path: &str,
    requested: FileRights,
) -> Result<(CapId, FileRights), i64> {
    without_interrupts(|| {
        let handles = proc.handles.lock();
        let mut held = false;
        for handle in handles.entries.iter().flatten() {
            if let Handle::File {
                file,
                cap_id,
                rights,
            } = handle
                && file
                    .scope()
                    .is_some_and(|scope| crate::fs::scope_contains(scope, path))
            {
                held = true;
                if requested.is_subset_of(*rights) {
                    return Ok((*cap_id, *rights));
                }
            }
        }
        Err(if held {
            crate::errno::EACCES
        } else {
            crate::errno::EPERM
        })
    })
}

/// OT-66 K1a: spawn inheritance admits a file handle only if it is a
/// directory: the M1 own-directory cap (exactly LOOKUP|READDIR), or any
/// directory cap carrying GRANT (a derived RootDirCap). Never a plain file.
/// Called under the caller's IF-off handles lock, so `is_directory` reads the
/// kind recorded at open and takes no inode lock (CP-37, CP-56).
#[cfg(feature = "cap-rooted-open")]
pub fn is_spawn_inheritable_dir_cap(handle: &Handle) -> bool {
    let Handle::File { file, rights, .. } = handle else {
        return false;
    };
    let own_dir = rights.bits() == (FileRights::LOOKUP.bits() | FileRights::READDIR.bits());
    file.is_directory() && (own_dir || rights.contains(FileRights::GRANT_RESERVED))
}

/// OT-61 C1.1: mint the root browser-runtime (thread) authority. Kernel-only
/// constructor, no syscall twin — installed into the trusted init process at
/// spawn (the `root_net_dial_cap` pattern); a process without it gets `EPERM`
/// from `SYS_THREAD_CREATE` (fail-closed, no self-mint).
#[cfg(feature = "browser-runtime")]
pub fn root_browser_runtime_cap() -> Handle {
    Handle::BrowserRuntime {
        cap_id: alloc_cap_id(),
        rights: full_browser_runtime_rights(),
    }
}

/// OT-87 R4.4: mint the installer authority. Kernel-only, no syscall twin;
/// installed into neo-init at spawn. Without it the installer syscalls are
/// `EPERM`.
#[cfg(feature = "installer")]
pub fn root_installer_cap() -> Handle {
    Handle::Installer {
        cap_id: alloc_cap_id(),
        rights: installer_cap_rights(),
    }
}

/// OT-66 L1: mint the system-store authority. Kernel-only, no syscall twin;
/// installed into neo-init at spawn. Without it a `.system/` open is `EPERM`.
#[cfg(feature = "system-store")]
pub fn root_system_store_cap() -> Handle {
    Handle::SystemStore {
        cap_id: alloc_cap_id(),
        rights: system_store_cap_rights(),
    }
}

/// Dexter #187 item 3a: mint the mount authority. Kernel-only, no syscall
/// twin; installed into neo-init at spawn. Without it `SYS_MOUNT` and
/// `SYS_UMOUNT` are `EPERM`.
#[cfg(feature = "mount-authority")]
pub fn root_mount_authority_cap() -> Handle {
    Handle::MountAuthority {
        cap_id: alloc_cap_id(),
        rights: mount_authority_cap_rights(),
    }
}

#[cfg(feature = "net-dial-cap")]
pub fn root_net_dial_cap() -> Handle {
    Handle::NetDial {
        host: Arc::<str>::from(net_dial_cap_logic::ROOT_HOST_PATTERN),
        port: net_dial_cap_logic::ROOT_PORT,
        cap_id: alloc_cap_id(),
        rights: full_net_dial_rights(),
    }
}

/// Mint the compositor root's scarce scanout authority. There is deliberately
/// no userspace create syscall: the kernel installs this only into a trusted
/// compositor root, which may choose whether to derive or inherit it onward.
#[cfg(feature = "atrium-compositor-g1")]
pub fn root_gpu_submit_cap() -> Handle {
    Handle::GpuSubmit {
        cap_id: alloc_cap_id(),
        rights: full_gpu_submit_rights(),
    }
}

/// Mint the input-system authority (THINK §8.2). Kernel-only, installed
/// once into the session root at spawn; no syscall twin.
#[cfg(feature = "atrium-compositor-g1")]
pub fn root_input_system_cap() -> Handle {
    Handle::InputSystem {
        cap_id: alloc_cap_id(),
        rights: input_system_rights(),
    }
}

/// Fixed per-submit glass quota.  It is intentionally a kernel policy, not a
/// user parameter: the only exposed pipeline is the pinned separable blur, and
/// oversized ranges fail closed before any allocation or driver submit.
#[cfg(feature = "atrium-compositor-g1")]
pub const GPU_GLASS_QUEUE_BYTE_QUOTA: u64 = 64 * 1024 * 1024;

#[cfg(feature = "atrium-compositor-g1")]
static NEXT_GPU_COMPUTE_GENERATION: AtomicU64 = AtomicU64::new(1);

#[cfg(feature = "atrium-compositor-g1")]
fn alloc_gpu_compute_generation() -> u64 {
    NEXT_GPU_COMPUTE_GENERATION.fetch_add(1, Ordering::Relaxed)
}

/// Mint the three post-Slice-5 compute roots for a trusted compositor-owned
/// source/destination pair.  This is the only production constructor for
/// `Handle::GpuCompute`; it is kernel-internal and has no syscall twin.  The
/// caller must first establish the graphics-domain sequencing gate — the
/// function refuses while Slice-5 narrowing is not active.
#[cfg(feature = "atrium-compositor-g1")]
pub fn root_gpu_compute_caps(
    src: Arc<crate::atrium::buffer::SharedPixelBuffer>,
    dst: Arc<crate::atrium::buffer::SharedPixelBuffer>,
) -> Option<[Handle; 3]> {
    use crate::drivers::graphics_arena;

    if !graphics_arena::narrowing_enabled()
        || Arc::ptr_eq(&src, &dst)
        || src.desc() != dst.desc()
        || src.desc().bytes > GPU_GLASS_QUEUE_BYTE_QUOTA
    {
        return None;
    }
    let bytes = src.desc().bytes;
    Some([
        Handle::GpuCompute {
            cap: GpuComputeCap::Buffer {
                buffer: src,
                generation: alloc_gpu_compute_generation(),
                offset: 0,
                len: bytes,
            },
            cap_id: alloc_cap_id(),
            rights: full_gpu_compute_read_buffer_rights(),
        },
        Handle::GpuCompute {
            cap: GpuComputeCap::Buffer {
                buffer: dst,
                generation: alloc_gpu_compute_generation(),
                offset: 0,
                len: bytes,
            },
            cap_id: alloc_cap_id(),
            rights: full_gpu_compute_write_buffer_rights(),
        },
        Handle::GpuCompute {
            cap: GpuComputeCap::Queue {
                byte_quota: GPU_GLASS_QUEUE_BYTE_QUOTA,
            },
            cap_id: alloc_cap_id(),
            rights: full_gpu_compute_queue_rights(),
        },
    ])
}

/// Slice-6 QEMU root installation.  This is deliberately test-feature-gated:
/// production callers must supply their already-owned compositor surfaces to
/// [`root_gpu_compute_caps`], while the scenario receives exactly its three
/// inherited handles (READ source, WRITE destination, queue) and no constructor
/// syscall exists for untrusted code to imitate this step.
#[cfg(feature = "neo-test-gpu-glass-slice6")]
pub fn root_gpu_compute_test_caps() -> Option<[Handle; 3]> {
    use crate::atrium::buffer::SharedPixelBuffer;
    use crate::atrium::logic::{BufferDescV1, FORMAT_XRGB8888_LE};

    let desc = BufferDescV1 {
        width: 8,
        height: 8,
        stride: 32,
        format: FORMAT_XRGB8888_LE,
        bytes: 256,
    };
    let src = SharedPixelBuffer::create_gpu(desc).ok()?;
    let dst = SharedPixelBuffer::create_gpu(desc).ok()?;
    root_gpu_compute_caps(src, dst)
}

/// Mint one wait/query-only fence after a successful compute-service submit.
/// This is not self-minting: it is the result object of an already-authorized
/// queue operation, analogous to a present epoch but intentionally separate
/// from every scanout primitive.
#[cfg(feature = "atrium-compositor-g1")]
pub fn completed_gpu_compute_fence(token: crate::drivers::accel::DispatchToken) -> Handle {
    Handle::GpuCompute {
        cap: GpuComputeCap::Fence {
            token,
            generation: alloc_gpu_compute_generation(),
        },
        cap_id: alloc_cap_id(),
        rights: full_gpu_compute_fence_rights(),
    }
}

#[derive(Clone)]
pub struct InheritedHandle {
    pub handle: Handle,
    pub parent: Option<CapId>,
}

/// fds the file I/O syscalls hardwire: serial stdout (fd 1), and stdin (fd 0)
/// where the input path owns it. A `Handle::File` must never be installed on one
/// of these — the syscall layer special-cases them, so a file cap landing there is
/// unusable (read ⇒ EBADF/stdin, write ⇒ serial). Mirror of `syscall::RESERVED_FILE_FDS`
/// (kept here because `HandleTable::install` is where derive/grant land file caps);
/// 0 = stdin, 1 = stdout per the stdio fd ABI.
#[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
const RESERVED_FILE_FDS: &[u32] = &[0, 1];
#[cfg(not(any(feature = "serial-stdin", feature = "usb-hid")))]
const RESERVED_FILE_FDS: &[u32] = &[1];

/// Per-process handle table. Lowest-free-index allocation guarantees that
/// a freshly constructed table fills 0..n on initial inheritance — this is
/// the deterministic-ordering ABI clause from ADR-004 §Q2.
pub struct HandleTable {
    entries: Vec<Option<Handle>>,
    /// OT-61 C2.2b: the account this table's handles charge (`None`: the
    /// kernel's table, and test tables, charge nothing).
    #[cfg(feature = "obj-quota")]
    account: Option<Arc<crate::mm::mem_account_logic::MemAccount>>,
    /// What each entry charged, by index (0 for an uncharged install).
    #[cfg(feature = "obj-quota")]
    weights: Vec<u64>,
    /// Charged by `charge_next`, recorded by the next install.
    #[cfg(feature = "obj-quota")]
    pending: u64,
    /// #165 F2: how many of `entries`' slots have their [`obj_weights::SLOT`]
    /// charged. A slot's backing outlives the handle in it (a close keeps
    /// the `Vec`'s capacity), so its charge is held until the slots are freed
    /// (`drain`, the drop), not released with the handle.
    #[cfg(feature = "obj-quota")]
    slots: usize,
}

/// OT-61 C2.2b: record the charge `charge_next` took for the entry just
/// installed at `i` (0 when none was taken). Takes the two fields, not the
/// table, so the installs can call it while they hold `entries`.
#[cfg(feature = "obj-quota")]
fn record_weight(weights: &mut Vec<u64>, pending: &mut u64, i: usize) {
    let w = core::mem::take(pending);
    if weights.len() <= i {
        weights.resize(i + 1, 0);
    }
    weights[i] = w;
}

/// #165 F2: where an install skipping `excluded` lands, as
/// `HandleTable::install_excluding` (and `install`, with none) picks it: the
/// lowest free slot not excluded, else the first index past the end that
/// isn't.
#[cfg(feature = "obj-quota")]
fn landing(entries: &[Option<Handle>], excluded: &[u32]) -> usize {
    if let Some(i) =
        (0..entries.len()).find(|&i| entries[i].is_none() && !excluded.contains(&(i as u32)))
    {
        return i;
    }
    let mut idx = entries.len();
    while excluded.contains(&(idx as u32)) {
        idx += 1;
    }
    idx
}

/// The fds `HandleTable::install_minted` keeps `h` off.
#[cfg(feature = "obj-quota")]
fn minted_excluded(h: &Handle) -> &'static [u32] {
    if matches!(h, Handle::File { .. }) {
        RESERVED_FILE_FDS
    } else {
        &[]
    }
}

/// A table dropped without a drain (a spawn that failed after installing its
/// inherited handles) releases what its entries hold, as a drain would: the
/// tensor, socket and exit-code counts (#171 F2), and, with `obj-quota`, what
/// they charged and the slots (#165 F2). A drained table is empty, so a
/// process torn down as usual releases nothing twice.
impl Drop for HandleTable {
    fn drop(&mut self) {
        #[cfg(not(feature = "neo-test-abandoned-table-control"))]
        drop(self.drain());
        #[cfg(feature = "neo-test-abandoned-table-control")]
        {
            #[cfg(feature = "obj-quota")]
            for i in 0..self.weights.len() {
                self.release_weight(i);
            }
            #[cfg(feature = "obj-quota")]
            self.release_slots();
        }
    }
}

/// OT-61 C2.2b (I-18): the declared kernel-heap weight, in bytes, of each
/// charged object. Each is built from `size_of` of the structures it pays
/// for plus their queue bounds, so a weight can only over-count; an object
/// shared by several handles is counted once per handle. The `const`
/// block at the end of this module checks each weight covers the structure
/// it names (#165: the comment used to promise a check nobody wrote).
#[cfg(feature = "obj-quota")]
pub mod obj_weights {
    use super::{CapId, Handle, Process, ProcessId};
    use crate::sched::thread::{STACK_SIZE, Thread};
    use core::mem::size_of;

    /// An `Arc` allocation's header (two counters).
    const ARC: usize = 2 * size_of::<usize>();

    /// A table slot: its entry and its per-slot weight, each counted four
    /// times. Charged when the slot is created and held while it exists, not
    /// while a handle is in it (#165 F2). Four, not three (#229.1a, Chad
    /// #183): a `Vec`'s first allocation holds four elements of these sizes
    /// (`RawVec`'s minimum non-zero capacity), so a one-slot table holds four
    /// entries' backing; and a doubling `Vec` of `L` slots holds at most
    /// `max(4, 2L)`, plus the old buffer (at most `L`) while it grows. Four
    /// per slot covers both at every `L`, and keeps every slot's cost the
    /// same (the userland handle loop checks that each pair costs alike).
    pub const SLOT: u64 = (4 * (size_of::<Option<Handle>>() + size_of::<u64>())) as u64;
    /// A handle's derivation edge (a `BTreeMap` entry, counted at twice its
    /// size for the node).
    pub const EDGE: u64 = (2 * size_of::<(CapId, Option<CapId>)>()) as u64;
    /// Any handle in a fresh slot: the slot and the edge.
    pub const HANDLE_BASE: u64 = SLOT + EDGE;

    /// #165 F1: an exit code someone can still claim. OT-91 keeps an entry in
    /// `EXIT_CLAIMS` and one in `COMPLETED_EXIT_CODES` only while a process
    /// handle names the pid or its spawn is in flight, so a process handle
    /// and a spawn each pay for both (counted at twice their size for the
    /// node).
    pub const EXIT_CODE: u64 = (2 * 2 * size_of::<(ProcessId, i32)>()) as u64;

    const CHANNEL: u64 = (ARC + size_of::<crate::ipc::Channel>()) as u64;
    const FILE: u64 = (ARC + size_of::<crate::fs::OpenFile>()) as u64;
    /// A UDP socket with its receive queue full.
    const SOCKET: u64 = (ARC
        + size_of::<crate::net::socket::Socket>()
        + crate::net::socket::RX_QUEUE_LIMIT * (crate::net::socket::MAX_DATAGRAM + 64))
        as u64;
    /// A TCP connection with its receive queue full (the send side is
    /// bounded by the same window).
    const TCP_SOCKET: u64 = (ARC
        + size_of::<crate::net::tcp::TcpSocket>()
        + 2 * crate::net::tcp::RX_QUEUE_LIMIT) as u64;
    const TCP_LISTENER: u64 = (ARC + size_of::<crate::net::tcp::TcpListener>()) as u64;
    /// #165 F1: what a listener's queues can hold, per place of `backlog`:
    /// one half-open and one accepted connection (separate queues, each up
    /// to `backlog`), each a TCP connection with its receive queue full, and
    /// its queue entry.
    const LISTENER_PLACE: u64 =
        2 * (TCP_SOCKET + size_of::<alloc::sync::Arc<crate::net::tcp::TcpSocket>>() as u64 + 8);
    /// A dial capability's host name (at most a DNS name).
    const NET_DIAL: u64 = (ARC + crate::net::dns::MAX_NAME_LEN) as u64;
    const MODEL: u64 = (ARC + size_of::<crate::ai::model::Model>()) as u64;

    /// What installing `h` charges, a fresh slot's [`SLOT`] included.
    pub fn handle(h: &Handle) -> u64 {
        SLOT + object(h)
    }

    /// What `h` charges while it is in a table: everything but its slot.
    pub fn object(h: &Handle) -> u64 {
        EDGE + match h {
            Handle::Channel { .. } => CHANNEL,
            Handle::Process { .. } => EXIT_CODE,
            Handle::File { .. } => FILE,
            Handle::Socket { .. } => SOCKET,
            Handle::TcpSocket { .. } => TCP_SOCKET,
            Handle::TcpListener { listener, .. } => {
                TCP_LISTENER + listener.backlog() as u64 * LISTENER_PLACE
            }
            Handle::NetDial { .. } => NET_DIAL,
            Handle::Model { .. } => MODEL,
            // L1a.1 step 2a: the object and its storage. A pipe shared by
            // two ends is counted once per handle, like a channel.
            #[cfg(feature = "pipe")]
            Handle::Pipe { pipe, .. } => {
                (ARC + size_of::<crate::ipc::pipe::PipeObject>() + pipe.capacity()) as u64
            }
            _ => 0,
        }
    }

    /// A spawn, charged to the parent: the `Process`, its first `Thread`
    /// and that thread's kernel stack, and its exit code's entries while the
    /// spawn's claim holds them (#165 F1).
    pub const SPAWN: u64 =
        (ARC + size_of::<Process>() + size_of::<Thread>() + STACK_SIZE) as u64 + EXIT_CODE;

    /// A `SYS_THREAD_CREATE`, charged to the creator: the `Thread` and its
    /// kernel stack.
    pub const THREAD: u64 = (size_of::<Thread>() + STACK_SIZE) as u64;

    /// A `SYS_BUFFER_MAP`, charged to the mapper: its entry in the Pmap's pin
    /// `Vec`, counted four times. The `Vec`'s first allocation holds four
    /// entries; after that it holds at most twice its live entries, and three
    /// times while it grows (the old buffer and the new). The pin's `Arc` and
    /// the charge's account `Arc` are clones, which allocate nothing; the
    /// page tables are frames, charged as frames. `obj_quota`'s kernel check
    /// measures the heap a pin `Vec` takes as it grows to `MAX_BORROWED_PINS`
    /// against this.
    #[cfg(feature = "atrium-compositor-g1")]
    pub const PIN: u64 = (4 * size_of::<crate::mm::pmap::BorrowedPin>()) as u64;

    // #165: each weight covers at least the structure it names (they are
    // built from these sizes, so this fails only if one is rewritten short).
    const _: () = {
        assert!(SLOT >= (size_of::<Option<Handle>>() + size_of::<u64>()) as u64);
        // #229.1a: one slot covers a first allocation's four entries.
        assert!(SLOT >= 4 * (size_of::<Option<Handle>>() + size_of::<u64>()) as u64);
        assert!(EDGE >= size_of::<(CapId, Option<CapId>)>() as u64);
        assert!(EXIT_CODE >= 2 * size_of::<(ProcessId, i32)>() as u64);
        assert!(CHANNEL >= size_of::<crate::ipc::Channel>() as u64);
        assert!(FILE >= size_of::<crate::fs::OpenFile>() as u64);
        assert!(SOCKET >= size_of::<crate::net::socket::Socket>() as u64);
        assert!(TCP_SOCKET >= size_of::<crate::net::tcp::TcpSocket>() as u64);
        assert!(TCP_LISTENER >= size_of::<crate::net::tcp::TcpListener>() as u64);
        assert!(LISTENER_PLACE >= 2 * TCP_SOCKET);
        assert!(MODEL >= size_of::<crate::ai::model::Model>() as u64);
        assert!(SPAWN >= (size_of::<Process>() + size_of::<Thread>() + STACK_SIZE) as u64);
        assert!(THREAD >= (size_of::<Thread>() + STACK_SIZE) as u64);
    };
}

/// Handles taken out of a table (a close, a revoke, a teardown). A last
/// tensor release can park on a Reserved slot (`ai::reservation`), so the
/// tensor counts are released only when this drops or is unwrapped, and every
/// `CAP_GRAPH` holder keeps it alive past its unlock (Janus F2). Dropping it
/// also drops the handles themselves (channel and buffer `Arc`s) outside the
/// lock. Under `neo-test`, a tensor release while this CPU holds `CAP_GRAPH`
/// panics (`release_tensor`).
#[must_use = "dropping it releases tensor counts; keep it past the CAP_GRAPH section"]
pub struct Detached(Vec<Handle>);

impl Detached {
    pub fn handles(&self) -> &[Handle] {
        &self.0
    }

    /// Release the tensor counts now and hand back the handles.
    pub fn into_handles(mut self) -> Vec<Handle> {
        self.release();
        core::mem::take(&mut self.0)
    }

    fn release(&mut self) {
        for handle in &self.0 {
            HandleTable::release_tensor(handle);
            HandleTable::release_net(handle);
            HandleTable::release_process(handle);
            HandleTable::release_pipe(handle);
        }
    }

    /// Release the socket handles now, ahead of the rest. Teardown calls it
    /// before it wakes the process's waiters, so a parent that waited on the
    /// child finds the child's ports free. The socket handles leave the set,
    /// so the drop doesn't release them twice.
    pub fn close_net(&mut self) {
        self.0.retain(|handle| {
            if HandleTable::is_net(handle) {
                HandleTable::release_net(handle);
                false
            } else {
                true
            }
        });
    }
}

impl Drop for Detached {
    fn drop(&mut self) {
        self.release();
    }
}

impl HandleTable {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            #[cfg(feature = "obj-quota")]
            account: None,
            #[cfg(feature = "obj-quota")]
            weights: Vec::new(),
            #[cfg(feature = "obj-quota")]
            pending: 0,
            #[cfg(feature = "obj-quota")]
            slots: 0,
        }
    }

    /// OT-61 C2.2b: a table whose installs charge `account` (I-18).
    #[cfg(feature = "obj-quota")]
    pub fn with_account(account: Option<Arc<crate::mm::mem_account_logic::MemAccount>>) -> Self {
        let mut table = Self::new();
        table.account = account;
        table
    }

    /// OT-61 C2.2b: charge the weight of `h` to this table's account, for
    /// the [`Self::install`] that follows under the same lock. Refused past a
    /// limit on the account chain; nothing is installed then.
    #[cfg(feature = "obj-quota")]
    pub fn charge_next(
        &mut self,
        h: &Handle,
    ) -> Result<(), crate::mm::mem_account_logic::QuotaExceeded> {
        self.charge_for(h, &[])
    }

    /// As [`Self::charge_next`], for an [`Self::install_minted`].
    #[cfg(feature = "obj-quota")]
    pub fn charge_next_minted(
        &mut self,
        h: &Handle,
    ) -> Result<(), crate::mm::mem_account_logic::QuotaExceeded> {
        self.charge_for(h, minted_excluded(h))
    }

    /// As [`Self::charge_next`], for an [`Self::install_excluding`].
    #[cfg(feature = "obj-quota")]
    pub fn charge_next_excluding(
        &mut self,
        h: &Handle,
        excluded: &[u32],
    ) -> Result<(), crate::mm::mem_account_logic::QuotaExceeded> {
        self.charge_for(h, excluded)
    }

    /// `h`'s object weight, plus the [`obj_weights::SLOT`] of every slot the
    /// install will create (#165 F2): none when it reuses a free one, else
    /// up to its index, the skipped `excluded` ones included. The slots stay
    /// charged ([`Self::release_slots`]); the object goes with the handle.
    #[cfg(feature = "obj-quota")]
    fn charge_for(
        &mut self,
        h: &Handle,
        excluded: &[u32],
    ) -> Result<(), crate::mm::mem_account_logic::QuotaExceeded> {
        debug_assert_eq!(self.pending, 0, "charge_next without an install");
        if let Some(account) = self.account.as_ref() {
            let at = landing(&self.entries, excluded);
            let new_slots = (at + 1).saturating_sub(self.entries.len());
            let w = obj_weights::object(h);
            account.charge_obj(w + new_slots as u64 * obj_weights::SLOT)?;
            self.pending = w;
            self.slots += new_slots;
        }
        Ok(())
    }

    #[cfg(feature = "obj-quota")]
    fn release_weight(&mut self, idx: usize) {
        if let Some(w) = self.weights.get_mut(idx) {
            let w = core::mem::take(w);
            if let Some(account) = self.account.as_ref() {
                let _ = account.release_obj(w);
            }
        }
    }

    /// #165 F2: release the slots' charge once their backing is gone.
    #[cfg(feature = "obj-quota")]
    fn release_slots(&mut self) {
        let n = core::mem::take(&mut self.slots) as u64;
        if let Some(account) = self.account.as_ref() {
            let _ = account.release_obj(n * obj_weights::SLOT);
        }
    }

    /// A tensor handle entering a table owns one arena handle count; leaving
    /// releases it (free-on-last — the O1b lifecycle chokepoint; `entries`
    /// is private, so install/remove/revoke/drain below are exhaustive).
    fn retain_tensor(h: &Handle) {
        if let Handle::Tensor { slot, .. } = h {
            // A live source entry pins the slot, so this can only refuse for
            // a handle forged against a freed slot — refuse loudly in debug.
            let retained = crate::ai::arena::retain(*slot);
            debug_assert!(retained, "installed a tensor handle with a dead slot");
        }
    }

    fn release_tensor(h: &Handle) {
        if let Handle::Tensor { slot, .. } = h {
            // K1a-0 F2 witness: a last release can park (a Reserved slot), and
            // a CAP_GRAPH holder must never park.
            #[cfg(feature = "neo-test")]
            assert!(
                !cap_graph_held_here(),
                "K1a-0 F2: a tensor count released while this CPU holds CAP_GRAPH"
            );
            crate::ai::arena::release(*slot);
        }
    }

    /// OT-91: a process handle in a table is a claim on that pid's exit code.
    fn retain_process(h: &Handle) {
        if let Handle::Process { target, .. } = h {
            update_exit_claims(*target, ClaimOp::AddHandle);
        }
    }

    fn release_process(h: &Handle) {
        if let Handle::Process { target, .. } = h {
            update_exit_claims(*target, ClaimOp::DropHandle);
        }
    }

    fn is_net(h: &Handle) -> bool {
        matches!(
            h,
            Handle::Socket { .. } | Handle::TcpSocket { .. } | Handle::TcpListener { .. }
        )
    }

    /// A socket, TCP connection or listener handle entering a table counts
    /// one on the object (`net::HandleCount`); same chokepoints as the
    /// tensor count.
    fn retain_net(h: &Handle) {
        match h {
            Handle::Socket { socket, .. } => socket.handles.retain(),
            Handle::TcpSocket { socket, .. } => socket.handles.retain(),
            Handle::TcpListener { listener, .. } => listener.handles.retain(),
            _ => {}
        }
    }

    /// The last handle leaving closes the object, as `SYS_SOCKET_CLOSE`
    /// does: before this, an exit, kill or revoke left the port bound and the
    /// `Arc` in the net tables for good. Runs where `Detached` releases, after
    /// the `CAP_GRAPH` section; the close takes the net table locks.
    fn release_net(h: &Handle) {
        let last = match h {
            Handle::Socket { socket, .. } => socket.handles.release(),
            Handle::TcpSocket { socket, .. } => socket.handles.release(),
            Handle::TcpListener { listener, .. } => listener.handles.release(),
            _ => return,
        };
        if !last {
            return;
        }
        #[cfg(feature = "neo-test")]
        assert!(
            !cap_graph_held_here(),
            "a socket closed while this CPU holds CAP_GRAPH"
        );
        without_interrupts(|| match h {
            Handle::Socket { socket, .. } => socket.close(),
            Handle::TcpSocket { socket, .. } => crate::net::tcp::close_connection(socket),
            Handle::TcpListener { listener, .. } => crate::net::tcp::close_listener(listener),
            _ => {}
        });
    }

    /// L1a.1 step 2a: a pipe handle entering a table is one more endpoint on
    /// its end (the socket count's pattern; same chokepoints).
    #[cfg_attr(not(feature = "pipe"), allow(unused_variables))]
    fn retain_pipe(h: &Handle) {
        #[cfg(feature = "pipe")]
        if let Handle::Pipe { pipe, end, .. } = h {
            pipe.retain(*end);
        }
    }

    /// The pipe handle left: one endpoint fewer. The last one of an end wakes
    /// everyone parked on the other side (EOF for readers, EPIPE for
    /// writers). Runs where `Detached` releases, after the `CAP_GRAPH`
    /// section; the wakes take the scheduler lock.
    #[cfg_attr(not(feature = "pipe"), allow(unused_variables))]
    fn release_pipe(h: &Handle) {
        #[cfg(feature = "pipe")]
        if let Handle::Pipe { pipe, end, .. } = h {
            pipe.release(*end);
        }
    }

    /// OT-22 (Jarvis rung-1 F1): a channel handle leaving a table wakes any
    /// `SYS_WAIT_ANY` waiter watching what it named, so a wait on a closed,
    /// revoked or drained handle re-resolves it (EBADF) instead of sleeping
    /// on a key nothing will fire. Same exhaustive chokepoints as the tensor
    /// release (`entries` is private).
    #[cfg_attr(not(feature = "wait-multiplex"), allow(unused_variables))]
    fn notify_removed(h: &Handle) {
        #[cfg(feature = "wait-multiplex")]
        if let Handle::Channel { channel, side, .. } = h {
            crate::sched::wait_any::fire_handle_gone(channel.wait_key(*side));
        }
        // L1a.1 step 2a, CP-61: a pipe handle leaving fires its key too, so a
        // wait on a closed or reused slot re-resolves.
        #[cfg(feature = "pipe")]
        if let Handle::Pipe { pipe, end, .. } = h {
            crate::sched::wait_any::fire_handle_gone(pipe.wait_key(*end));
        }
    }

    /// Install `h` at the lowest unused index. Returns the assigned index.
    pub fn install(&mut self, h: Handle) -> u32 {
        Self::retain_tensor(&h);
        Self::retain_net(&h);
        Self::retain_process(&h);
        Self::retain_pipe(&h);
        for (i, slot) in self.entries.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(h);
                #[cfg(feature = "obj-quota")]
                record_weight(&mut self.weights, &mut self.pending, i);
                return i as u32;
            }
        }
        self.entries.push(Some(h));
        #[cfg(feature = "obj-quota")]
        record_weight(&mut self.weights, &mut self.pending, self.entries.len() - 1);
        (self.entries.len() - 1) as u32
    }

    /// Allocate a fresh fd for a handle that the cap graph just minted (a
    /// `derive`/`grant` result). Identical to [`install`] for every handle kind
    /// **except** `Handle::File`, which is routed through [`install_excluding`] so
    /// a derived/granted file cap never lands on a hardwired stdio fd (I-15).
    ///
    /// Before the file-cap unification a file handle could not be derived/granted
    /// (it EPERM'd), so this case never arose; now that it can, a file cap on the
    /// stdout fd would be **unusable** (`sys_read` ⇒ EBADF, `sys_write` ⇒ serial) —
    /// it would not "work", violating ISC-4. This is NOT used by the spawn-inherit
    /// 0..n fill, which must place handles at exact indices (ADR-004 §Q2) and uses
    /// plain `install`; only fresh-allocation (derive/grant) routes through here.
    pub fn install_minted(&mut self, h: Handle) -> u32 {
        if matches!(h, Handle::File { .. }) {
            return self.install_excluding(h, RESERVED_FILE_FDS);
        }
        self.install(h)
    }

    /// The slots the charged table has (#165 F2's discriminator).
    #[cfg(feature = "neo-test-obj-quota")]
    pub fn charged_slots(&self) -> usize {
        self.slots
    }

    /// The object weight charged but not yet installed (Chad #198): a refused
    /// charge must leave it as it was.
    #[cfg(feature = "neo-test-obj-quota")]
    pub fn pending_weight(&self) -> u64 {
        self.pending
    }

    /// The bytes the table's two `Vec`s hold now (#229.1a's discriminator):
    /// what the slots' charge must cover.
    #[cfg(feature = "neo-test-obj-quota")]
    pub fn backing_bytes(&self) -> u64 {
        (self.entries.capacity() * core::mem::size_of::<Option<Handle>>()
            + self.weights.capacity() * core::mem::size_of::<u64>()) as u64
    }

    /// Install `h` at the lowest unused index not named in `excluded`.
    /// Used by file opens to keep fd 1 permanently reserved for serial
    /// stdout compatibility without changing IPC inheritance ordering.
    pub fn install_excluding(&mut self, h: Handle, excluded: &[u32]) -> u32 {
        Self::retain_tensor(&h);
        Self::retain_net(&h);
        Self::retain_process(&h);
        Self::retain_pipe(&h);
        for (i, slot) in self.entries.iter_mut().enumerate() {
            if slot.is_none() && !excluded.contains(&(i as u32)) {
                *slot = Some(h);
                #[cfg(feature = "obj-quota")]
                record_weight(&mut self.weights, &mut self.pending, i);
                return i as u32;
            }
        }
        let mut idx = self.entries.len() as u32;
        while excluded.contains(&idx) {
            self.entries.push(None);
            idx += 1;
        }
        self.entries.push(Some(h));
        #[cfg(feature = "obj-quota")]
        record_weight(&mut self.weights, &mut self.pending, idx as usize);
        idx
    }

    /// Look up a handle by index. Returns None for free or out-of-range.
    pub fn get(&self, idx: u32) -> Option<Handle> {
        self.entries.get(idx as usize).and_then(|o| o.clone())
    }

    pub fn get_mut(&mut self, idx: u32) -> Option<&mut Handle> {
        self.entries
            .get_mut(idx as usize)
            .and_then(|slot| slot.as_mut())
    }

    /// Take handle `idx` out. Its tensor count is released when the returned
    /// [`Detached`] drops (after the caller's `CAP_GRAPH` section).
    pub fn remove(&mut self, idx: u32) -> Option<Detached> {
        let removed = self
            .entries
            .get_mut(idx as usize)
            .and_then(|slot| slot.take());
        #[cfg(feature = "obj-quota")]
        if removed.is_some() {
            self.release_weight(idx as usize);
        }
        removed.map(|handle| {
            Self::notify_removed(&handle);
            Detached(alloc::vec![handle])
        })
    }

    /// Look up a live handle by CapId. Used by grant rendezvous completion
    /// so a cap revoked while the grantor was blocked is detected.
    pub fn get_by_cap_id(&self, cap_id: CapId) -> Option<Handle> {
        self.entries
            .iter()
            .flatten()
            .find(|h| h.cap_id() == cap_id)
            .cloned()
    }

    /// OT-63 Rung 1 Slice 2 — does this table hold a usable `GpuSubmit` cap?
    /// True iff some live handle is a `Handle::GpuSubmit` carrying `USE` meta +
    /// the `SUBMIT` semantic. The scarce authority a HardwareFlip submit needs
    /// when enforcement is armed; the caller passes the result into
    /// `present(.., caller_holds_gpu_submit)`.
    #[cfg(feature = "atrium-compositor-g1")]
    pub fn holds_gpu_submit(&self) -> bool {
        self.entries.iter().flatten().any(|h| {
            matches!(h, Handle::GpuSubmit { .. })
                && h.rights().has_meta(meta_rights::USE)
                && h.rights().has_semantic(gpu_submit_rights::SUBMIT)
        })
    }

    /// DI-1a: does this table hold the input-system authority? Exact: the
    /// kind, `USE` and `SYSTEM`. No enforcement toggle can open it.
    #[cfg(feature = "atrium-compositor-g1")]
    pub fn holds_input_system(&self) -> bool {
        self.entries.iter().flatten().any(|h| {
            matches!(h, Handle::InputSystem { .. })
                && h.rights().has_meta(meta_rights::USE)
                && h.rights().has_semantic(input_system_rights::SYSTEM)
        })
    }

    /// Take every handle in `revoked` out. Their tensor counts are released
    /// (and the handles dropped) when the returned [`Detached`] drops, after
    /// the caller's `CAP_GRAPH` section.
    #[cfg(not(feature = "obj-quota"))]
    pub fn remove_cap_ids(&mut self, revoked: &BTreeSet<CapId>) -> Detached {
        let mut removed = Vec::new();
        for slot in &mut self.entries {
            if let Some(handle) = slot
                && revoked.contains(&handle.cap_id())
            {
                Self::notify_removed(handle);
                removed.extend(slot.take());
            }
        }
        Detached(removed)
    }

    /// As above, releasing each removed entry's charge (OT-61 C2.2b).
    #[cfg(feature = "obj-quota")]
    pub fn remove_cap_ids(&mut self, revoked: &BTreeSet<CapId>) -> Detached {
        let mut removed = Vec::new();
        let mut freed: Vec<usize> = Vec::new();
        for (i, slot) in self.entries.iter_mut().enumerate() {
            if let Some(handle) = slot
                && revoked.contains(&handle.cap_id())
            {
                Self::notify_removed(handle);
                removed.extend(slot.take());
                freed.push(i);
            }
        }
        for i in freed {
            self.release_weight(i);
        }
        Detached(removed)
    }

    /// Drain all handles. Used during process exit cleanup so the caller
    /// can walk and signal each peer endpoint (see `on_thread_reap`).
    pub fn drain(&mut self) -> Detached {
        #[cfg(feature = "obj-quota")]
        for i in 0..self.weights.len() {
            self.release_weight(i);
        }
        let drained: Vec<Handle> = self.entries.drain(..).flatten().collect();
        // #165 F2: free the slots' backing too (a drained table keeps no
        // handles), then release their charge.
        #[cfg(feature = "obj-quota")]
        {
            self.entries = Vec::new();
            self.weights = Vec::new();
            self.release_slots();
        }
        for handle in &drained {
            Self::notify_removed(handle);
        }
        Detached(drained)
    }

    pub fn live_count(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    /// OT-61 C1.1 (blocker C): snapshot up to `out.len()` (channel, side) pairs
    /// into a caller-provided fixed array so a caller can extract parked
    /// siblings WITHOUT draining/closing the handles AND WITHOUT allocating.
    /// The fault-kill drain that uses this runs in the REAPER's kernel context
    /// (not the #PF exception frame — see `on_thread_reap_threaded`), so the
    /// heap would in fact be available; it stays alloc-free anyway because the
    /// scan happens under the handle lock, where the rest of the scheduler's
    /// locked paths are alloc-free too. Clones the channel `Arc`s (a refcount
    /// bump, no heap); the caller drops them before returning (CP-30).
    ///
    /// **Paged, so the scan is COMPLETE.** `entries` is an unbounded `Vec`, so a
    /// single fixed-size snapshot could silently miss channels past the array —
    /// and a missed IPC-parked sibling stalls fault teardown forever (blocker C
    /// requires retiring EVERY sibling, not most of them). Scanning resumes at
    /// `start` (an index into `entries`) and returns `(filled, next)`, where
    /// `next` is the entry index to resume from; the caller loops until `next`
    /// reaches the table length. Truncation therefore costs another page, never
    /// a lost sibling.
    #[cfg(feature = "browser-runtime")]
    fn snapshot_channel_endpoints(
        &self,
        start: usize,
        out: &mut [Option<(Arc<Channel>, EndpointSide)>],
    ) -> (usize, usize) {
        let mut n = 0;
        let mut idx = start;
        while idx < self.entries.len() {
            if let Some(Handle::Channel { channel, side, .. }) = &self.entries[idx] {
                if n >= out.len() {
                    return (n, idx);
                }
                out[n] = Some((channel.clone(), *side));
                n += 1;
            }
            idx += 1;
        }
        (n, idx)
    }

    /// Number of handle slots — the paging bound for `snapshot_channel_endpoints`.
    #[cfg(feature = "browser-runtime")]
    fn entry_slots(&self) -> usize {
        self.entries.len()
    }

    /// OT-61 C1.1: does this table hold a usable browser-runtime thread
    /// authority? The `SYS_THREAD_CREATE` gate (the `NetDial` scan shape —
    /// no handle-index argument pollutes the frozen thread ABI).
    #[cfg(feature = "browser-runtime")]
    pub fn has_browser_runtime_thread_authority(&self) -> bool {
        self.entries.iter().flatten().any(|handle| {
            matches!(
                handle,
                Handle::BrowserRuntime { rights, .. }
                    if rights.has_meta(meta_rights::USE)
                        && rights.has_semantic(browser_runtime_rights::THREAD_SPAWN)
            )
        })
    }

    /// OT-87 R4.4: does this table hold the installer authority (`USE` and
    /// `INSTALL`)?
    #[cfg(feature = "installer")]
    pub fn holds_installer(&self) -> bool {
        self.entries.iter().flatten().any(|handle| {
            matches!(
                handle,
                Handle::Installer { rights, .. }
                    if rights.has_meta(meta_rights::USE)
                        && rights.has_semantic(installer_rights::INSTALL)
            )
        })
    }

    /// OT-66 L1: does this table hold the system-store authority (`USE` and
    /// `STORE`)?
    #[cfg(feature = "system-store")]
    pub fn holds_system_store(&self) -> bool {
        self.entries.iter().flatten().any(|handle| {
            matches!(
                handle,
                Handle::SystemStore { rights, .. }
                    if rights.has_meta(meta_rights::USE)
                        && rights.has_semantic(system_store_rights::STORE)
            )
        })
    }

    /// Dexter #187 item 3a: does this table hold the mount authority (`USE`
    /// and `MOUNT`)?
    #[cfg(feature = "mount-authority")]
    pub fn holds_mount_authority(&self) -> bool {
        self.entries.iter().flatten().any(|handle| {
            matches!(
                handle,
                Handle::MountAuthority { rights, .. }
                    if rights.has_meta(meta_rights::USE)
                        && rights.has_semantic(mount_authority_rights::MOUNT)
            )
        })
    }

    pub fn channel_count(&self) -> usize {
        self.entries
            .iter()
            .flatten()
            .filter(|handle| matches!(handle, Handle::Channel { .. }))
            .count()
    }

    /// Count the live handles that confer **network** authority, in any form:
    /// the outbound dial capability (`NetDial`) and any live socket/listener
    /// object (`Socket`/`TcpSocket`/`TcpListener`). Surfaced through
    /// `SYS_INSPECT` so a supervisor can prove a subsystem holds *no* network
    /// authority directly from the live handle table — the I-14(a) containment
    /// property for a secret-holding process such as the cap-bounded vault
    /// (ADR-075 §2.4: "Tests must inspect the actual child handle set. Grep is
    /// supporting evidence, not the proof."). A zero here means the process
    /// cannot reach the network and therefore cannot exfiltrate.
    pub fn network_count(&self) -> usize {
        self.entries
            .iter()
            .flatten()
            .filter(|handle| {
                matches!(
                    handle,
                    Handle::NetDial { .. }
                        | Handle::Socket { .. }
                        | Handle::TcpSocket { .. }
                        | Handle::TcpListener { .. }
                )
            })
            .count()
    }

    /// Whether this table references physical tensor `tensor_id` (Phase 7 BUILD-3) —
    /// either directly as a tensor capability (`Handle::Tensor`) or indirectly as a
    /// weight of a loaded model (`Handle::Model`). Used by the cross-process reader
    /// scan that derives the shared-tensor reader set without storing it.
    pub fn references_tensor_id(&self, tensor_id: u64) -> bool {
        self.entries.iter().flatten().any(|handle| match handle {
            Handle::Tensor { meta, .. } => meta.id == tensor_id,
            Handle::Model { model, .. } => model.references_tensor_id(tensor_id),
            _ => false,
        })
    }
}

/// Process registry — maps pid → Process. The scheduler queries this on
/// cold process operations. Scheduler context switches do not read this
/// table after SC-4 Phase A; `Thread` carries its own `Arc<Pmap>`.
///
/// SC-4 forward marker (Phase 2+): if a future syscall handler needs
/// CR3-by-PID without `&Thread` access, the Phase B fallback is an
/// `AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` CR3-data-only table
/// using the arc-swap crate pattern. Cerebro D5 §10 documents this deferred
/// path. Do not introduce it until a concrete non-scheduler hot path exists.
///
/// SC-4 forward marker (Phase 2+): `spin::RwLock` is documented-pathological
/// at the 4-64 CPU range for short critical sections due to reader-counter
/// cache-line ping-pong and writer-starvation pressure. Do not reach for it
/// as a default upgrade from `spin::Mutex` on PROCESSES.
static PROCESSES: Lazy<Mutex<BTreeMap<ProcessId, Arc<Process>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

/// OT-93 F2 (Janus #149; #202 F2, Chad): a force-killed process between its
/// removal from `PROCESSES` and the completion of its waiters. `terminate`
/// moves it here in the same section as the removal (the registration still
/// goes first, for `close_peer`), and a `sys_wait` that arrives meanwhile finds
/// it here and parks. Completion (waiters woken with ESRCH, EXITED published)
/// waits for `terminate` to finish its teardown (handles closed, sockets
/// released) and, when the kill left a thread inside the kernel (a survivor,
/// an admission caller, an inference participant), for the reap of its last
/// thread, whose syscall could still take effect until then. Whichever comes
/// last completes it (`finish_kill`). Lock order: `PROCESSES`, then `DYING`.
static DYING: Lazy<Mutex<BTreeMap<ProcessId, DyingProcess>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

struct DyingProcess {
    process: Arc<Process>,
    /// `terminate` has not finished its teardown yet.
    teardown_pending: bool,
    /// Leg E's control (`neo-test-kill-at-exit-control-e`): the pre-count
    /// rule. The kill waits for a reap only if its survivor count saw a
    /// thread, and only a reap of a process already here counts.
    #[cfg(feature = "neo-test-kill-at-exit-control-e")]
    reap_pending: bool,
}

/// What `finish_kill` is told has happened.
#[derive(Clone, Copy)]
enum KillStep {
    TeardownDone,
    /// A thread of it was reaped (its anchor released): check again.
    ThreadReaped,
}

/// OT-93 (the PM's #204 bounce, item 1): the reaper freed a thread of `pid`
/// (its anchor is released). A killed process in `DYING` completes here once
/// that was its last thread and its teardown is done.
pub fn thread_reaped(pid: ProcessId) {
    if pid != KERNEL_PID {
        finish_kill(pid, KillStep::ThreadReaped);
    }
}

/// OT-98 (CP-98, Janus #174 F2; the PM's bounce, item 2): after
/// `terminate`'s scans, whether the killed `pid`'s thread left the kernel on
/// its own while they ran. A waiter in `sys_wait` whose target completed
/// between the first survivor count and the `sys_wait` scan is in no queue
/// those read: it was woken Ready inside its syscall (under its target's
/// `waiters` lock, so never in neither), and can run on to its return, read
/// `force_killed` and exit before the recount. From there it is in a zombie
/// ring, then popped by the reaper and being freed (`scheduler::reaping`),
/// then freed (its anchor released, `Process::live_threads` 0). Those come in
/// the order these checks read them, and each move is published before the
/// next check reads it (the ring under its CPU's scheduler lock, the reaping
/// mark SeqCst before that lock is released, the count SeqCst before the mark
/// is cleared), so a thread that left is found in one of them. Only a
/// diagnostic (`kill_unlocated`) depends on it: the kill's completion is the
/// count's. The controls leave out the zombie and reaping checks
/// (`neo-test-kill-at-exit-control-d`) or the reaping one (`-control-f`).
fn killed_thread_left_kernel(pid: ProcessId, proc_arc: &Process) -> bool {
    if cfg!(feature = "neo-test-kill-at-exit-control-d") {
        return false;
    }
    scheduler::zombie_pending(pid)
        || (!cfg!(feature = "neo-test-kill-at-exit-control-f") && scheduler::reaping() == Some(pid))
        || proc_arc.live_threads() == 0
}

/// OT-98 (the PM's bounce, items 2 and 3): kills whose scans found no thread
/// of their target in any state they read. Since the completion counts
/// threads (`finish_kill`), such a kill is still correct; the count is a
/// diagnostic, which `kill_at_exit` holds at zero wherever the scans can find
/// the thread. It replaces the `debug_assert` that panicked the kernel (the
/// product keeps debug assertions on) in windows no snapshot can read.
static KILL_UNLOCATED: AtomicU64 = AtomicU64::new(0);

fn note_kill_unlocated(pid: ProcessId) {
    KILL_UNLOCATED.fetch_add(1, Ordering::AcqRel);
    serial_println!(
        "[ kernel ] proc: kill of pid={} found no thread in its scans; it completes at its last reap",
        pid.0
    );
}

/// How many kills found no thread of their target (`note_kill_unlocated`).
#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn kill_unlocated_count() -> u64 {
    KILL_UNLOCATED.load(Ordering::Acquire)
}

/// A registered process, or a force-killed one still in `DYING`.
fn get_live_or_dying(pid: ProcessId) -> Option<Arc<Process>> {
    without_interrupts(|| {
        let procs = PROCESSES.lock();
        procs
            .get(&pid)
            .cloned()
            .or_else(|| DYING.lock().get(&pid).map(|d| d.process.clone()))
    })
}

/// OT-93 F2: record `step` for a killed process in `DYING`; once its teardown
/// is done and no thread of it is left (`Process::live_threads`, the PM's
/// #204 bounce, item 1), wake its waiters (with ESRCH, a kill's result) and
/// publish EXITED. A pid not in `DYING` is a no-op.
///
/// The thread count decides it, not what `terminate`'s scans saw: a survivor
/// can be reaped before `terminate` inserts the process here (its reap then
/// finds the process still registered, and only the count records it), and a
/// thread can be between two places no scan reads. Every reap releases its
/// thread's anchor (the count) and then calls this; `terminate` calls it when
/// its teardown is done. Whichever is last completes the kill: the count is
/// SeqCst, and each caller reads it after its own step, under `DYING`.
/// The control (`neo-test-kill-wait-early-control`) completes at the
/// teardown, whatever the count, as before F2.
fn finish_kill(pid: ProcessId, step: KillStep) {
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    if matches!(step, KillStep::ThreadReaped) && pid.0 == KILL_VICTIM.load(Ordering::Acquire) {
        KILL_SURVIVOR_REAPED.store(true, Ordering::Release);
    }
    let done = without_interrupts(|| {
        let mut dying = DYING.lock();
        let entry = dying.get_mut(&pid)?;
        if matches!(step, KillStep::TeardownDone) {
            entry.teardown_pending = false;
        }
        let threads_left = !cfg!(feature = "neo-test-kill-at-exit-control-e")
            && !cfg!(feature = "neo-test-kill-wait-early-control")
            && entry.process.live_threads() > 0;
        #[cfg(feature = "neo-test-kill-at-exit-control-e")]
        let threads_left = {
            if matches!(step, KillStep::ThreadReaped) {
                entry.reap_pending = false;
            }
            entry.reap_pending
        };
        if entry.teardown_pending || threads_left {
            return None;
        }
        dying.remove(&pid).map(|d| d.process)
    });
    let Some(proc_arc) = done else {
        drop_deferred_processes_if_interruptible();
        return;
    };
    complete_process_waiters(&proc_arc, pid, None);
    proc_arc
        .exit_state
        .store(EXIT_STATE_EXITED, Ordering::Release);
    if matches!(step, KillStep::ThreadReaped) {
        serial_println!(
            "[ kernel ] proc: kill of pid={} completed at its last reap",
            pid.0
        );
    }
    // CP-91 (Janus #174 F3): this can be the process's last `Arc`, and
    // `terminate` (its teardown completing the kill) runs inside `SYS_KILL`'s
    // interrupts-off section: hand it to the reaper there.
    release_last_process(proc_arc);
    drop_deferred_processes_if_interruptible();
}

/// OT-98 (Janus #174 F3): drop `proc` here if interrupts are on; with them
/// off, or inside a nested reap (under NeoFS locks: the #226 bounce), a last
/// `Arc` goes to the reaper (`release_scan_pin`).
pub(super) fn release_last_process(proc: Arc<Process>) {
    if crate::arch::x86_64::interrupts_enabled() && !crate::sched::scheduler::in_nested_reap() {
        drop(proc);
    } else {
        release_scan_pin(proc);
    }
}

/// The exit codes of reaped processes that someone can still claim. OT-91:
/// an entry exists only while `EXIT_CLAIMS` has one for its pid, so the map
/// is bounded by the live process handles plus the spawns in flight (it used
/// to keep every unwaited child's code for good).
static COMPLETED_EXIT_CODES: Lazy<Mutex<BTreeMap<ProcessId, i32>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

/// OT-91: who can still claim a pid's exit code. `handles` counts the
/// `Handle::Process` entries naming it in any table (the `sys_wait` and
/// `SYS_INSPECT` paths need one), plus the spawner's claim, taken before the
/// child can run: a syscall spawn drops it once the parent's handle is
/// installed (or failed to be); a kernel spawner keeps it until it reads the
/// code. Stored as `2 * handles + spawn` in the same map type as
/// `COMPLETED_EXIT_CODES`, which keeps the kernel from carrying a second
/// BTree's code. Lock order: a process's `waiters`, then `EXIT_CLAIMS`, then
/// `COMPLETED_EXIT_CODES`.
static EXIT_CLAIMS: Lazy<Mutex<BTreeMap<ProcessId, i32>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

#[derive(Clone, Copy)]
enum ClaimOp {
    AddHandle,
    DropHandle,
    SetSpawn,
    ClearSpawn,
}

/// Change `pid`'s claims; when none are left, drop the claims and any exit
/// code waiting for them (nobody can ever read it).
#[inline(never)]
fn update_exit_claims(pid: ProcessId, op: ClaimOp) {
    without_interrupts(|| {
        let mut claims = EXIT_CLAIMS.lock();
        let entry = claims.entry(pid).or_insert(0);
        match op {
            ClaimOp::AddHandle => *entry = entry.saturating_add(2),
            ClaimOp::DropHandle => {
                debug_assert!(*entry >= 2, "released a process handle claim that was zero");
                *entry = entry.saturating_sub(2).max(*entry & 1);
            }
            ClaimOp::SetSpawn => *entry |= 1,
            ClaimOp::ClearSpawn => *entry &= !1,
        }
        if *entry == 0 {
            claims.remove(&pid);
            COMPLETED_EXIT_CODES.lock().remove(&pid);
        }
    });
}

/// #171 F1: a claim on `pid`'s exit code held by a syscall in flight. A
/// `sys_wait` or `SYS_INSPECT` takes one under its caller's table lock, from
/// the process handle that authorized it, so a revoke of that handle before
/// the call reads the code doesn't drop the code from under it. Counted as
/// a handle; released on drop.
#[must_use = "dropping it releases the claim"]
pub struct ExitClaim(ProcessId);

impl ExitClaim {
    /// Take a claim on `pid`. Call it with the table lock held that holds
    /// the process handle naming `pid` (that handle's claim covers it until
    /// this one exists).
    pub fn pin(pid: ProcessId) -> Self {
        update_exit_claims(pid, ClaimOp::AddHandle);
        ExitClaim(pid)
    }
}

impl Drop for ExitClaim {
    fn drop(&mut self) {
        update_exit_claims(self.0, ClaimOp::DropHandle);
    }
}

/// Drop the spawner's claim on `pid`'s exit code (see `EXIT_CLAIMS`).
pub fn release_spawn_claim(pid: ProcessId) {
    update_exit_claims(pid, ClaimOp::ClearSpawn);
}

/// The kernel reads the exit code of a process it spawned; once it has the
/// code it drops its spawn claim (a poll before the exit keeps it).
pub fn take_spawned_exit_code(pid: ProcessId) -> Option<i32> {
    let code = take_completed_exit_code(pid);
    if code.is_some() {
        release_spawn_claim(pid);
    }
    code
}

/// Test-only: the sizes of the exit-code and claims maps.
#[cfg(feature = "neo-test")]
pub fn exit_code_table_sizes() -> (usize, usize) {
    without_interrupts(|| (COMPLETED_EXIT_CODES.lock().len(), EXIT_CLAIMS.lock().len()))
}

/// Register a process so the scheduler can look it up by pid.
pub fn register(proc: Process) -> Arc<Process> {
    without_interrupts(|| {
        let pid = proc.pid;
        let arc = Arc::new(proc);
        PROCESSES.lock().insert(pid, arc.clone());
        arc
    })
}

fn unregister_unstarted(pid: ProcessId) {
    without_interrupts(|| {
        PROCESSES.lock().remove(&pid);
    });
}

/// K1a-0 F1 leg (`cap_graph_revoke`, and `mem_quota`'s deferred-drop leg):
/// a scan under `CAP_GRAPH`, as `cap_is_live` runs it, and an unregister, as
/// the reaper's.
#[cfg(any(feature = "neo-test-cap-graph-revoke", feature = "neo-test-mem-quota"))]
pub fn cap_is_live_for_test(g: &mut CapGraph, cap_id: CapId) -> bool {
    cap_is_live(g, cap_id)
}
#[cfg(any(feature = "neo-test-cap-graph-revoke", feature = "neo-test-mem-quota"))]
pub fn deferred_process_drops_for_test() -> usize {
    without_interrupts(|| DEFERRED_PROCESS_DROPS.lock().len())
}
/// How many queued `Process`es are named `name`.
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub fn deferred_named_for_test(name: &str) -> usize {
    without_interrupts(|| {
        DEFERRED_PROCESS_DROPS
            .lock()
            .iter()
            .filter(|p| p.name == name)
            .count()
    })
}
#[cfg(any(feature = "neo-test-cap-graph-revoke", feature = "neo-test-mem-quota"))]
pub fn unregister_for_test(pid: ProcessId) {
    unregister_unstarted(pid);
}

/// Fetch a `Process` Arc by pid. Used by the IPC syscalls to obtain the
/// caller's `Arc<Pmap>` for the rendezvous-time translate.
/// OT-98 discriminator: how many threads are parked in `sys_wait` on `pid`.
#[cfg(feature = "neo-test-kill-at-exit")]
pub fn waiter_count_for_test(pid: ProcessId) -> usize {
    get_live_or_dying(pid).map_or(0, |p| without_interrupts(|| p.waiters.lock().waiters.len()))
}

pub fn get(pid: ProcessId) -> Option<Arc<Process>> {
    without_interrupts(|| PROCESSES.lock().get(&pid).cloned())
}

/// B1c AAG J1: has `pid`'s teardown begun (or finished)? A thread the kill
/// path skipped because it held the inference stack checks this at the
/// syscall boundary and exits.
#[cfg(feature = "inference-stack")]
pub fn torn_down(pid: ProcessId) -> bool {
    get(pid).is_none_or(|proc| proc.exiting.load(Ordering::Acquire))
}

pub fn get_user_by_active_cr3(active_cr3: u64) -> Option<Arc<Process>> {
    if active_cr3 == 0 {
        return None;
    }
    without_interrupts(|| {
        PROCESSES.lock().values().find_map(|proc| {
            if proc.pid == KERNEL_PID {
                return None;
            }
            let pmap = proc.pmap.as_ref()?;
            let (frame, flags) = pmap.cr3();
            let cr3 = crate::arch::x86_64::per_cpu::encode_cr3(frame, flags);
            if cr3 == active_cr3 {
                Some(proc.clone())
            } else {
                None
            }
        })
    })
}

fn processes_snapshot() -> Vec<Arc<Process>> {
    without_interrupts(|| PROCESSES.lock().values().cloned().collect())
}

/// The set of principals currently referencing physical tensor `tensor_id` (Phase 7
/// BUILD-3) — its shared-tensor reader set, derived by scanning every process's
/// capability table. Returns sorted, de-duplicated pids. Holds no AI-subsystem lock
/// (the caller in `ai::sharing` snapshots its registry before calling this), so the
/// `PROCESSES`/handles locks taken here never invert against the sharing registry.
pub fn tensor_reader_pids(tensor_id: u64) -> Vec<ProcessId> {
    let mut pids: Vec<ProcessId> = without_interrupts(|| {
        processes_snapshot()
            .into_iter()
            .filter(|proc| proc.handles.lock().references_tensor_id(tensor_id))
            .map(|proc| proc.pid)
            .collect()
    });
    pids.sort_unstable();
    pids.dedup();
    pids
}

// === K1a-0: the capability-graph lock (CP-87, CP-88) =========================
//
// A cap's derivation edge (`child -> parent`) lives in its holder's
// `derivations` map. Two defects followed from that, both closed here:
//
// - **CP-87, install vs revoke.** Revoke walks every process's map, then
//   removes. A child installed in between (a derive, a grant, a spawn
//   inheritance, any install naming a parent) was never in the walk's set and
//   survived the revoke. Now every install that names a parent, every revoke,
//   every close and every process teardown holds `CAP_GRAPH`, and an install
//   re-checks under it that its parent is still live.
// - **CP-88, laundering by close or exit.** Closing a cap, or its holder
//   exiting, deleted its edge, so its children lost their path to every
//   ancestor and a later revoke of the ancestor missed them (derive D'' from
//   D', close D', and D'' outlives `revoke(D)`). Now a closed or torn-down cap
//   that has children hands them to its own parent first (`retire`).
//
// The lock: a FIFO ticket spin lock (`sync::ticket_mutex`), not IRQ-safe by
// itself. It is always taken with interrupts off, and no interrupt path takes
// it (CP-37). Its holders never park, block or wait on another CPU, so its
// waiters spin with interrupts off. Two hidden ways a holder could (Janus
// F1/F2, CP-91/CP-92) are closed by construction: a scan's `Arc<Process>`
// may be the last, and its `Pmap` drop waits on a TLB shootdown, so a last
// one is handed to the reaper (`CapGraph::drop`), never dropped under the
// lock or in its caller's interrupts-off block; and a removed handle's tensor count is
// released after the unlock (`Detached`), since a last release can park on a
// Reserved slot. `neo-test` witnesses panic if either runs under the lock. FIFO
// order bounds a revoke's wait behind a stream of derives and closes to the
// lockers already queued ahead of it, one critical section each.
//
// Lock order: `Channel.state` (a grant completes under it) -> `CAP_GRAPH` ->
// `PROCESSES` -> a process's `handles` -> its `derivations` -> the wait-any
// table and the scheduler lock (a removal fires wait keys, which wakes).
// Nothing holding any lock after `CAP_GRAPH` in that order takes it: it is
// taken only from syscall bodies, the reaper (after the scheduler lock is
// released) and `terminate`.
//
// The set it guards holds the caps that MAY have children: a parent is added
// when a child names it and is not removed when its last child goes (an entry
// can outlive its children; a stale entry only costs one scan). So it is an
// over-approximation, never a miss, and closing a cap that never had children
// stays one lookup.
//
// Kill-switch: `neo-test-cap-graph-unlocked-control` restores the previous
// layer: no lock, no parent re-check, no re-parenting (test only: its disabled
// state is the known escape).

#[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
static CAP_GRAPH: TicketMutex<BTreeSet<CapId>> = TicketMutex::new(BTreeSet::new());

/// The CPU holding `CAP_GRAPH`, plus one (0: none). The holder never parks or
/// migrates, so "this CPU holds it" is well defined; the witness behind
/// [`cap_graph_held_here`].
#[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
static CAP_GRAPH_HOLDER: AtomicU32 = AtomicU32::new(0);

/// Whether this CPU holds `CAP_GRAPH`. Used by the `neo-test` witnesses on the
/// paths that must never run under it: a `Pmap` drop (a TLB shootdown), a
/// tensor release (a possible park), a reservation park.
pub fn cap_graph_held_here() -> bool {
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    {
        CAP_GRAPH_HOLDER.load(Ordering::Acquire)
            == crate::arch::x86_64::per_cpu::current_cpu_id() + 1
    }
    #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
    false
}

/// Whether the cap-graph rules are on (off only under the control feature).
pub const CAP_GRAPH_ENFORCED: bool = !cfg!(feature = "neo-test-cap-graph-unlocked-control");

/// K1b (Muse, #166 review item 5): a process's derivation edges, behind an API
/// whose every read and write takes the caller's `CapGraph` guard. The map is
/// private to this module, so no code elsewhere in the kernel can reach it,
/// and inside `process` no path can touch it without holding `CAP_GRAPH`:
/// this closes the edge gate's named misses (a macro, an alias, a helper in
/// another module, a reassignment) by construction.
pub mod edges {
    use super::{CapGraph, CapId};
    use crate::sync::SwitchMutex;
    use alloc::collections::{BTreeMap, BTreeSet};

    pub struct Derivations(SwitchMutex<BTreeMap<CapId, Option<CapId>>>);

    impl Derivations {
        pub(super) fn new() -> Self {
            Self(SwitchMutex::new(BTreeMap::new()))
        }

        pub(super) fn insert(&self, _g: &CapGraph, child: CapId, parent: Option<CapId>) {
            self.0.lock().insert(child, parent);
        }

        /// The edge of `cap`, removed: `Some(parent)` if it had one.
        pub(super) fn remove(&self, _g: &CapGraph, cap: CapId) -> Option<Option<CapId>> {
            self.0.lock().remove(&cap)
        }

        pub(super) fn remove_all(&self, _g: &CapGraph, caps: &BTreeSet<CapId>) {
            let mut map = self.0.lock();
            for cap in caps {
                map.remove(cap);
            }
        }

        /// Every edge, removed (process teardown).
        pub(super) fn take(&self, _g: &CapGraph) -> BTreeMap<CapId, Option<CapId>> {
            core::mem::take(&mut *self.0.lock())
        }

        /// Rewrite each edge's parent in place (the re-parent of CP-88).
        pub(super) fn reparent(&self, _g: &CapGraph, mut f: impl FnMut(&mut Option<CapId>)) {
            for parent in self.0.lock().values_mut() {
                f(parent);
            }
        }

        /// Visit every edge `(child, parent)` (the revoke walk).
        pub(super) fn for_each(&self, _g: &CapGraph, mut f: impl FnMut(CapId, Option<CapId>)) {
            for (child, parent) in self.0.lock().iter() {
                f(*child, *parent);
            }
        }

        /// The q8a negative gate's deliberately broken install, which skips
        /// the lock. Returns the map's size before the insert, as before.
        #[cfg(feature = "neo-test-missing-cap-lock-negative")]
        pub(super) fn insert_without_cap_graph(&self, child: CapId, parent: Option<CapId>) -> u32 {
            let mut map = self.0.lock();
            let idx = map.len() as u32;
            map.insert(child, parent);
            idx
        }

        #[cfg(feature = "neo-test")]
        pub fn is_empty_for_test(&self) -> bool {
            self.0.lock().is_empty()
        }

        #[cfg(feature = "neo-test")]
        pub fn parent_for_test(&self, cap: CapId) -> Option<Option<CapId>> {
            self.0.lock().get(&cap).copied()
        }
    }
}

/// Test-only: a handle the test removed from `proc`'s table by hand also
/// leaves the graph, as a close would (its edge goes and its children are
/// re-parented). Takes `CAP_GRAPH`, so call it with no handle lock held.
#[cfg(feature = "neo-test")]
pub fn retire_edge_for_test(proc: &Process, cap_id: CapId) {
    without_interrupts(|| lock_cap_graph().retire_one(proc, cap_id));
}

/// Proof that `CAP_GRAPH` is held (a no-op under the control feature).
pub struct CapGraph {
    /// `Some` while held; `Drop` takes it to release the lock first.
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    has_children: Option<TicketMutexGuard<'static, BTreeSet<CapId>>>,
    /// Every `Arc<Process>` a scan under the lock took (Janus F1). One of
    /// them can be a process's last reference (the reaper unregisters and
    /// drops its own outside the lock), and that drop runs `Pmap::drop`, a TLB
    /// shootdown that would wait on a CPU spinning for this lock. They are
    /// dropped in `Drop`, after the lock is released.
    pinned: Vec<Arc<Process>>,
}

/// Take `CAP_GRAPH`. Call with interrupts off (see the lock order above).
pub fn lock_cap_graph() -> CapGraph {
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    let guard = CAP_GRAPH.lock();
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    CAP_GRAPH_HOLDER.store(
        crate::arch::x86_64::per_cpu::current_cpu_id() + 1,
        Ordering::Release,
    );
    CapGraph {
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        has_children: Some(guard),
        pinned: Vec::new(),
    }
}

impl Drop for CapGraph {
    /// Release the lock, then let go of what the scans pinned (F1).
    ///
    /// Not a plain drop. Every `CAP_GRAPH` section runs inside its caller's
    /// interrupts-off block, which can still hold other state (spawn's
    /// shootdown quiesce, a grant's `Channel.state`). A `Pmap` drop there
    /// would send a shootdown with interrupts off, and a contended
    /// `SHOOTDOWN_LOCK` turns them on briefly (`acquire_shootdown_lock`).
    /// So each pin goes through `Arc::into_inner`: it decrements, and only
    /// when that pin was the last reference does it hand the `Process` back
    /// instead of dropping it. Those (rare: the reaper unregistered and
    /// dropped its own first) wait on [`DEFERRED_PROCESS_DROPS`] for the
    /// reaper, which drops them where it drops every process.
    fn drop(&mut self) {
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        {
            CAP_GRAPH_HOLDER.store(0, Ordering::Release);
            self.has_children = None;
        }
        #[cfg(feature = "neo-test")]
        RELEASING_PINS.store(
            crate::arch::x86_64::per_cpu::current_cpu_id() + 1,
            Ordering::Release,
        );
        for pin in core::mem::take(&mut self.pinned) {
            if let Some(last) = Arc::into_inner(pin) {
                defer_process_drop(last);
            }
        }
        #[cfg(feature = "neo-test")]
        RELEASING_PINS.store(0, Ordering::Release);
    }
}

/// Processes whose last reference was a `CAP_GRAPH` scan's pin (F1). Dropped
/// by the reaper ([`drop_deferred_processes`]), never inside a `CAP_GRAPH`
/// caller's interrupts-off block. Rare: it takes a scan racing a reap.
static DEFERRED_PROCESS_DROPS: IrqSafeMutex<Vec<Process>> = IrqSafeMutex::new(Vec::new());
/// Set after a push to [`DEFERRED_PROCESS_DROPS`], so the reaper's ordinary
/// pass ([`drop_deferred_processes_if_any`]) drains it with one atomic swap.
static DEFERRED_PENDING: AtomicBool = AtomicBool::new(false);

/// Hand the reaper a `Process` whose last reference was dropped where its
/// `Drop` mustn't run (under `CAP_GRAPH`, or a caller's IF-off section). The
/// only producer path (Janus #182 item 4): it always publishes
/// [`DEFERRED_PENDING`], so the next top-level reap drains it even when no
/// teardown follows. Every producer goes through here: a `CapGraph` pin's
/// release, and a kill scan's (`release_scan_pin`, OT-98).
pub fn defer_process_drop(last: Process) {
    DEFERRED_PROCESS_DROPS.lock().push(last);
    // The control (`neo-test-deferred-pending-control`) skips the publish
    // only for F1d's enqueue, so it fails F1d itself (Janus #188 recheck 2,
    // item 5), not an earlier leg.
    #[cfg(feature = "neo-test-deferred-pending-control")]
    if PENDING_CONTROL_ARMED.swap(false, Ordering::AcqRel) {
        return;
    }
    DEFERRED_PENDING.store(true, Ordering::Release);
}

/// Armed by F1d just before its enqueue (the pending control's scope).
#[cfg(feature = "neo-test-deferred-pending-control")]
pub static PENDING_CONTROL_ARMED: AtomicBool = AtomicBool::new(false);

/// The CPU (plus one) inside `CapGraph::drop`'s pin release: the F1 witness
/// in `Pmap::drop` panics if a `Pmap` drops there.
#[cfg(feature = "neo-test")]
static RELEASING_PINS: AtomicU32 = AtomicU32::new(0);

/// Whether this CPU is releasing `CapGraph` pins (a `neo-test` witness).
#[cfg(feature = "neo-test")]
pub fn releasing_cap_graph_pins_here() -> bool {
    RELEASING_PINS.load(Ordering::Acquire) == crate::arch::x86_64::per_cpu::current_cpu_id() + 1
}

/// OT-98 (Janus #174 F3): let go of a scan's clone of a process. When it is
/// the last reference, the `Process` goes to the reaper ([`defer_process_drop`],
/// which publishes it: Janus #182 item 4) instead of dropping here, so its
/// `Pmap` never runs a shootdown with interrupts off, in any caller (`SYS_KILL`
/// runs `terminate` with them off), and a scan released after the last
/// teardown's drain is still freed by the next top-level reap.
fn release_scan_pin(pin: Arc<Process>) {
    if let Some(last) = Arc::into_inner(pin) {
        defer_process_drop(last);
    }
}

/// `cap_graph_revoke` F1e: [`release_scan_pin`], as a kill scan runs it.
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub fn release_scan_pin_for_test(pin: Arc<Process>) {
    release_scan_pin(pin);
}

/// [`drop_deferred_processes`], only where interrupts are on and no reap is
/// nested. `terminate` and `finish_kill` also run inside `SYS_KILL`'s
/// interrupts-off section, and a thread reap can be nested in the NeoFS retry
/// paths, under their locks (`sched::reap_nested`, the #226 bounce); there
/// they leave the list to the reaper's top level (`sched::reap_and_drain`;
/// every enqueue publishes `DEFERRED_PENDING`).
fn drop_deferred_processes_if_interruptible() {
    if crate::arch::x86_64::interrupts_enabled() && !crate::sched::scheduler::in_nested_reap() {
        drop_deferred_processes();
    }
}

/// Drop the processes a `CAP_GRAPH` scan was left holding last (F1). Called
/// by the reaper, at the point where it drops its own process.
pub fn drop_deferred_processes() {
    let deferred = without_interrupts(|| core::mem::take(&mut *DEFERRED_PROCESS_DROPS.lock()));
    drop(deferred);
}

/// [`drop_deferred_processes`], from the reaper's top level
/// (`sched::reap_and_drain`; Janus #127, #182 item 2). A pin can be released
/// after the last process teardown has drained the queue, and an ordinary
/// pass reaps nothing; without this the process (and its frames) would wait
/// for the next teardown, which may never come.
///
/// Only from a context that holds no other lock, with interrupts on: a
/// `Process` drop runs `Pmap::drop` (a TLB shootdown). So never from a reap
/// nested in the NeoFS retry paths (`inode.rs`, `cache.rs`), which hold
/// `entries` and the cache's `drain` guard: those call `sched::reap_nested`.
pub fn drop_deferred_processes_if_any() {
    assert!(
        crate::arch::x86_64::interrupts_enabled(),
        "deferred Process drops drained with interrupts off"
    );
    if DEFERRED_PENDING.swap(false, Ordering::AcqRel) {
        drop_deferred_processes();
    }
}

impl CapGraph {
    /// [`get`], pinned until this guard is released (Janus #127 F1). A plain
    /// lookup's `Arc` can be the process's last reference by the time it drops
    /// (a reap unregistered and dropped its own meanwhile), and that drop would
    /// run `Pmap::drop` under the lock.
    fn get_pinned(&mut self, pid: ProcessId) -> Option<Arc<Process>> {
        let proc = get(pid)?;
        self.pinned.push(proc.clone());
        Some(proc)
    }

    /// A snapshot of every registered process, pinned until this guard is
    /// released (F1). Iterate it by reference.
    fn snapshot(&mut self) -> &[Arc<Process>] {
        let start = self.pinned.len();
        self.pinned.extend(processes_snapshot());
        &self.pinned[start..]
    }

    /// The has-children set (the lock's payload).
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    fn set(&mut self) -> &mut BTreeSet<CapId> {
        self.has_children
            .as_mut()
            .expect("CapGraph used after its lock was released")
    }

    /// Record that `parent` may have a child (never cleared when the child
    /// goes; see the set's comment).
    fn note_parent(&mut self, parent: Option<CapId>) {
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        if let Some(p) = parent {
            self.set().insert(p);
        }
        #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
        let _ = parent;
    }

    /// Forget revoked caps (they no longer exist anywhere).
    fn forget(&mut self, revoked: &BTreeSet<CapId>) {
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        for c in revoked {
            self.set().remove(c);
        }
        #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
        let _ = revoked;
    }

    /// Retire `dying` (each cap's edge, `cap -> parent`, already taken out of
    /// its holder's map): every child of a dying cap, in any process, is
    /// re-parented to its nearest ancestor that is not dying (CP-88).
    fn retire(&mut self, dying: &BTreeMap<CapId, Option<CapId>>) {
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        {
            if !dying.keys().any(|c| self.set().contains(c)) {
                return;
            }
            let mut ups = Vec::new();
            // Clones of Arcs the guard has pinned, so none of them is the
            // last when this Vec drops under the lock (CP-91).
            let processes: Vec<Arc<Process>> = self.snapshot().to_vec();
            for proc in &processes {
                proc.derivations.reparent(self, |parent| {
                    if let Some(Some(up)) = super::cap_graph_logic::reparent(parent, dying) {
                        ups.push(up);
                    }
                });
            }
            let set = self.set();
            set.extend(ups);
            for c in dying.keys() {
                set.remove(c);
            }
        }
        #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
        let _ = dying;
    }

    /// A handle and its edge leave `proc` (a close): re-parent its children.
    pub fn retire_one(&mut self, proc: &Process, cap_id: CapId) {
        let parent = proc.derivations.remove(self, cap_id);
        if let Some(parent) = parent {
            let mut dying = BTreeMap::new();
            dying.insert(cap_id, parent);
            self.retire(&dying);
        }
    }

    /// Process teardown: drain `proc`'s handles and retire every edge it
    /// held. The caller drops (and peer-closes) the returned handles after
    /// releasing the lock.
    pub fn teardown(&mut self, proc: &Process) -> Detached {
        let drained = proc.handles.lock().drain();
        let dying = proc.derivations.take(self);
        // The window between the edges leaving and the re-parent: a revoke
        // here must see the old tree or the new one (the exit and kill legs).
        #[cfg(feature = "neo-test-cap-graph-revoke")]
        cap_graph_race::in_teardown();
        self.retire(&dying);
        drained
    }
}

/// Whether `cap_id` is still installed in some registered process (an
/// inherited handle's parent at spawn). Under `CAP_GRAPH`.
fn cap_is_live(g: &mut CapGraph, cap_id: CapId) -> bool {
    g.snapshot()
        .iter()
        .any(|proc| proc.handles.lock().get_by_cap_id(cap_id).is_some())
}

/// Remove handle `idx` from `proc` if `accept` takes it, and retire its edge
/// (the close path of every handle kind). `Err(EBADF)` for an empty slot or a
/// handle `accept` refuses.
pub fn close_handle(
    proc: &Process,
    idx: u32,
    accept: impl FnOnce(&Handle) -> bool,
) -> Result<Handle, i64> {
    // The closure's `CapGraph` drops at its end; the detached handle's tensor
    // count is released only after that (F2).
    let detached = without_interrupts(|| {
        let mut g = lock_cap_graph();
        let removed = {
            let mut handles = proc.handles.lock();
            let handle = handles.get(idx).ok_or(crate::errno::EBADF)?;
            if !accept(&handle) {
                return Err(crate::errno::EBADF);
            }
            handles.remove(idx).ok_or(crate::errno::EBADF)?
        };
        g.retire_one(proc, removed.handles()[0].cap_id());
        Ok(removed)
    })?;
    detached.into_handles().pop().ok_or(crate::errno::EBADF)
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum InstallCapError {
    RecipientGone,
    /// The named parent is no longer in the table (closed or revoked).
    ParentGone,
    /// OT-61 C2.2b: the handle's weight would pass an object limit on the
    /// recipient's account chain.
    #[cfg(feature = "obj-quota")]
    Quota,
}

pub fn install_cap(
    proc: &Process,
    handle: Handle,
    parent: Option<CapId>,
) -> Result<u32, InstallCapError> {
    // A refused handle can be its object's last reference (the minted
    // ACTIVATE sibling of a revoked focus), so it is moved out here and
    // dropped after `CAP_GRAPH`: an object's `Drop` may take its own locks
    // (`InputFocus` takes `DELIVERY`).
    #[cfg_attr(
        any(
            feature = "neo-test-missing-cap-lock-negative",
            feature = "neo-test-install-cap-drop-control"
        ),
        allow(unused_mut)
    )]
    let mut refused: Option<Handle> = None;
    let installed = without_interrupts(|| {
        #[cfg(feature = "neo-test-missing-cap-lock-negative")]
        {
            let cap_id = handle.cap_id();
            // Deliberately broken q8a negative gate: model the failure mode
            // where the handle-table acquisition is omitted. The scenario must
            // detect the resulting derivation/handle mismatch.
            if proc.exiting.load(Ordering::Acquire) {
                return Err(InstallCapError::RecipientGone);
            }
            Ok(proc.derivations.insert_without_cap_graph(cap_id, parent))
        }

        #[cfg(not(feature = "neo-test-missing-cap-lock-negative"))]
        {
            let mut g = lock_cap_graph();
            let r = install_minted_locked(&mut g, proc, handle, parent, true, &mut refused);
            // The control drops it under the lock, as before.
            #[cfg(feature = "neo-test-install-cap-drop-control")]
            drop(refused.take());
            r
        }
    });
    drop(refused);
    installed
}

/// `install_cap`'s body under `CAP_GRAPH`. With `parent_in_proc`, the named
/// parent must still be in `proc`'s own table (CP-87); `complete_grant`
/// checks its parent in the grantor instead and passes false. A refused
/// handle is moved to `refused`, for the caller to drop after the lock.
fn install_minted_locked(
    g: &mut CapGraph,
    proc: &Process,
    handle: Handle,
    parent: Option<CapId>,
    parent_in_proc: bool,
    refused: &mut Option<Handle>,
) -> Result<u32, InstallCapError> {
    let cap_id = handle.cap_id();
    let mut handles = proc.handles.lock();
    if proc.exiting.load(Ordering::Acquire) {
        *refused = Some(handle);
        return Err(InstallCapError::RecipientGone);
    }
    if CAP_GRAPH_ENFORCED
        && parent_in_proc
        && let Some(p) = parent
        && handles.get_by_cap_id(p).is_none()
    {
        // Usually the parent is in `proc` itself (tensor share, TCP accept,
        // GPU fence); otherwise it must be live elsewhere. The scan takes
        // other tables' locks, so drop this one first and re-take it.
        drop(handles);
        if !cap_is_live(g, p) {
            *refused = Some(handle);
            return Err(InstallCapError::ParentGone);
        }
        handles = proc.handles.lock();
        if proc.exiting.load(Ordering::Acquire) {
            *refused = Some(handle);
            return Err(InstallCapError::RecipientGone);
        }
    }
    #[cfg(feature = "obj-quota")]
    if handles.charge_next_minted(&handle).is_err() {
        *refused = Some(handle);
        return Err(InstallCapError::Quota);
    }
    // Fresh-allocate (grant/AI-mint path): route File caps off the
    // hardwired stdio fds (I-15). Identical to `install` for other kinds.
    let idx = handles.install_minted(handle);
    proc.derivations.insert(g, cap_id, parent);
    g.note_parent(parent);
    Ok(idx)
}

pub fn install_cap_excluding(
    proc: &Process,
    handle: Handle,
    parent: Option<CapId>,
    excluded: &[u32],
) -> Result<u32, InstallCapError> {
    // A refused handle drops after `CAP_GRAPH` (see `install_cap`).
    let mut refused = None;
    let installed = without_interrupts(|| {
        let mut g = lock_cap_graph();
        let cap_id = handle.cap_id();
        let mut handles = proc.handles.lock();
        if proc.exiting.load(Ordering::Acquire) {
            refused = Some(handle);
            return Err(InstallCapError::RecipientGone);
        }
        if CAP_GRAPH_ENFORCED
            && let Some(p) = parent
            && handles.get_by_cap_id(p).is_none()
        {
            drop(handles);
            if !cap_is_live(&mut g, p) {
                refused = Some(handle);
                return Err(InstallCapError::ParentGone);
            }
            handles = proc.handles.lock();
            if proc.exiting.load(Ordering::Acquire) {
                refused = Some(handle);
                return Err(InstallCapError::RecipientGone);
            }
        }
        #[cfg(feature = "obj-quota")]
        if handles.charge_next_excluding(&handle, excluded).is_err() {
            refused = Some(handle);
            return Err(InstallCapError::Quota);
        }
        let idx = handles.install_excluding(handle, excluded);
        proc.derivations.insert(&g, cap_id, parent);
        g.note_parent(parent);
        Ok(idx)
    });
    drop(refused);
    installed
}

pub fn derive_handle(pid: ProcessId, h: u32, child_rights: CapRights) -> Result<u32, i64> {
    without_interrupts(|| {
        use crate::errno::{EBADF, EINVAL, EPERM};

        // CP-87: the parent's presence below is checked under CAP_GRAPH, so
        // a revoke of an ancestor either finished first (the parent is gone)
        // or starts after this child's edge exists (its walk finds it).
        let mut g = lock_cap_graph();
        let proc = g.get_pinned(pid).ok_or(EINVAL)?;
        let mut handles = proc.handles.lock();
        if proc.exiting.load(Ordering::Acquire) {
            return Err(EINVAL);
        }
        let parent = handles.get(h).ok_or(EBADF)?;
        let parent_rights = parent.rights();
        if !parent_rights.has_meta(meta_rights::DERIVE) {
            return Err(EPERM);
        }
        if !child_rights.is_subset_of(parent_rights) {
            return Err(EPERM);
        }

        let parent_cap_id = parent.cap_id();
        let child_cap_id = alloc_cap_id();
        let child = parent.with_cap(child_cap_id, child_rights);
        #[cfg(feature = "obj-quota")]
        handles
            .charge_next_minted(&child)
            .map_err(|_| crate::errno::ENOMEM)?;
        // Fresh-allocate the derived fd off the hardwired stdio fds for File caps
        // (I-15) — a derived file cap on the stdout fd would be unusable.
        let idx = handles.install_minted(child);
        proc.derivations
            .insert(&g, child_cap_id, Some(parent_cap_id));
        g.note_parent(Some(parent_cap_id));
        Ok(idx)
    })
}

#[cfg(feature = "net-dial-cap")]
pub fn derive_net_dial_cap(
    pid: ProcessId,
    h: u32,
    child_host: &str,
    child_port: u16,
) -> Result<u32, i64> {
    without_interrupts(|| {
        use crate::errno::{EACCES, EBADF, EINVAL, EPERM};

        let mut g = lock_cap_graph();
        let proc = g.get_pinned(pid).ok_or(EINVAL)?;
        let mut handles = proc.handles.lock();
        if proc.exiting.load(Ordering::Acquire) {
            return Err(EINVAL);
        }
        let parent = handles.get(h).ok_or(EBADF)?;
        let (parent_host, parent_port, parent_cap_id, parent_rights) = match parent {
            Handle::NetDial {
                host,
                port,
                cap_id,
                rights,
            } => (host, port, cap_id, rights),
            _ => return Err(EBADF),
        };
        if !parent_rights.has_meta(meta_rights::DERIVE) {
            return Err(EPERM);
        }
        if !net_dial_cap_logic::can_attenuate(&parent_host, parent_port, child_host, child_port) {
            return Err(EACCES);
        }

        let child_cap_id = alloc_cap_id();
        let child = Handle::NetDial {
            host: Arc::<str>::from(child_host),
            port: child_port,
            cap_id: child_cap_id,
            // Scope derivation must not amplify a rights-attenuated parent.
            // The syscall narrows `(host, port)` while preserving the existing
            // rights ceiling; generic `derive_handle` remains available when
            // the caller also wants to reduce that rights set.
            rights: parent_rights,
        };
        #[cfg(feature = "obj-quota")]
        handles
            .charge_next_minted(&child)
            .map_err(|_| crate::errno::ENOMEM)?;
        let idx = handles.install_minted(child);
        proc.derivations
            .insert(&g, child_cap_id, Some(parent_cap_id));
        g.note_parent(Some(parent_cap_id));
        Ok(idx)
    })
}

pub fn revoke_handle(pid: ProcessId, h: u32) -> Result<(), i64> {
    without_interrupts(|| -> Result<(), i64> {
        use crate::errno::{EBADF, EINVAL, EPERM};

        // Lock acquisition order under SMP per SC-4 D6:
        //   1. PROCESSES (snapshot only, released before per-process locks;
        //      outermost after CAP_GRAPH, step 0 below)
        //   2. Per-process locks acquired in ascending ProcessId.0 order
        //   3. Within a single process: handles before derivations
        //   4. Channel state (innermost; matches SC-3B SwitchMutex ordering)
        //
        // processes_snapshot() clones Arc<Process> values out of a BTreeMap.
        // BTreeMap iteration is sorted by key, and ProcessId derives Ord over
        // its u32 pid, so the walk below is monotonic by ProcessId. Concurrent
        // revokes on different CPUs follow the same rule, preventing AB-BA.
        //
        // Citation precision: FreeBSD process walks use a monotonic table walk
        // with per-process locked handoff, and Linux PID-IDR mutation is guarded
        // by pidmap_lock, not tasklist_lock. See
        // Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md.
        //   0. CAP_GRAPH (K1a-0, CP-87): held across the walk AND the
        //      removal, so no install naming a doomed parent lands between.
        //      Not the global outermost: a grant takes it under
        //      `Channel.state` (ADR-010); a revoke holds no channel lock.
        //
        // Declared before the guard, so they drop after it (K1a-0 F1/F2): the
        // caller's Arc, and the removed handles with their tensor releases.
        let caller = get(pid).ok_or(EINVAL)?;
        let mut detached: Vec<Detached> = Vec::new();
        let mut g = lock_cap_graph();
        let mut revoked = BTreeSet::new();
        {
            let mut caller_handles = caller.handles.lock();
            let root = caller_handles.get(h).ok_or(EBADF)?;
            if !root.rights().has_meta(meta_rights::REVOKE) {
                return Err(EPERM);
            }
            revoked.insert(root.cap_id());
            detached.push(caller_handles.remove_cap_ids(&revoked));
        }

        // Pinned in the guard until after the unlock (F1).
        let processes: Vec<Arc<Process>> = g.snapshot().to_vec();
        loop {
            let mut changed = false;
            for proc in &processes {
                let _handles = proc.handles.lock();
                proc.derivations.for_each(&g, |child, parent| {
                    if let Some(parent_id) = parent
                        && revoked.contains(&parent_id)
                        && !revoked.contains(&child)
                    {
                        revoked.insert(child);
                        changed = true;
                    }
                });
            }
            if !changed {
                break;
            }
        }

        #[cfg(feature = "neo-test-cap-graph-revoke")]
        cap_graph_race::between_walk_and_removal();

        for proc in &processes {
            detached.push(proc.handles.lock().remove_cap_ids(&revoked));
            proc.derivations.remove_all(&g, &revoked);
        }
        g.forget(&revoked);
        // `processes` holds clones of Arcs the guard has pinned, so none of
        // them is a last reference here; `g`, then `detached`, drop next.
        drop(processes);

        Ok(())
    })?;
    #[cfg(feature = "neo-test-cap-graph-revoke")]
    cap_graph_race::after_revoke();
    Ok(())
}

/// K1a-0 test hooks (`cap_graph_revoke`). Each places, deterministically, the
/// interleaving a racing CPU would produce:
///
/// - **Revoke window** (kinds `DERIVE`, `GRANT`, `SPAWN`; CP-87): an install
///   naming a doomed descendant, between a revoke's walk and its removal.
///   With the lock on, that install would wait for `CAP_GRAPH`: the hook
///   proves the revoke holds it, then runs the install after the revoke
///   returns, where it must find its parent gone. The spawn twin clones the
///   inherited handle inside the window (as `sys_spawn` clones before calling
///   `spawn_user`) and spawns afterwards.
/// - **Teardown window** (kind `TEARDOWN`; CP-88): a revoke of the caller's
///   handle `h` between a dying process's edges leaving and the re-parent.
///   With the lock on it waits and runs after the teardown, on the new tree.
///
/// Under the control feature each runs inside its window, and the cap it
/// should have killed survives.
#[cfg(feature = "neo-test-cap-graph-revoke")]
/// Janus #188 recheck 2, item 2 (`cap_graph_revoke` F1g): on a reap of an
/// armed pid, right after `on_thread_reap`'s lookup, unregister the process
/// and drop the test's own `Arc`, as a kill completing on another CPU would.
/// The reap's clone is then the last reference.
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub mod reap_race {
    use super::*;

    static ARMED: Mutex<Option<Arc<Process>>> = Mutex::new(None);

    pub fn arm(proc: Arc<Process>) {
        *ARMED.lock() = Some(proc);
    }

    pub(super) fn after_lookup(pid: ProcessId) {
        let mut armed = ARMED.lock();
        if armed.as_ref().is_some_and(|p| p.pid == pid) {
            without_interrupts(|| PROCESSES.lock().remove(&pid));
            drop(armed.take());
        }
    }

    /// `on_thread_reap` of `pid`, as the reaper calls it.
    pub fn reap(pid: ProcessId) {
        on_thread_reap(pid);
    }

    /// A claimed exit (a kill's), so the reap takes its early return.
    pub fn claim_forced(proc: &Process) {
        proc.exit_state.store(EXIT_STATE_EXITED, Ordering::Release);
    }
}

pub mod cap_graph_race {
    use super::*;
    use core::sync::atomic::AtomicI64;

    pub const DERIVE: u32 = 0;
    pub const GRANT: u32 = 1;
    pub const SPAWN: u32 = 2;
    pub const TEARDOWN: u32 = 3;
    /// A hook ran while `CAP_GRAPH` was free.
    pub const LOCK_NOT_HELD: i64 = -1000;

    #[derive(Clone)]
    struct Armed {
        kind: u32,
        pid: ProcessId,
        h: u32,
    }

    enum Deferred {
        Install(Armed),
        Spawn(InheritedHandle),
        Revoke(Armed),
    }

    static ARMED: Mutex<Option<Armed>> = Mutex::new(None);
    static DEFERRED: Mutex<Option<Deferred>> = Mutex::new(None);
    static RESULT: AtomicI64 = AtomicI64::new(i64::MIN);
    /// The spawn twin's child, for `result` to inspect.
    static SPAWNED: AtomicU32 = AtomicU32::new(0);

    pub fn arm(pid: ProcessId, h: u32, kind: u32) {
        RESULT.store(i64::MIN, Ordering::SeqCst);
        SPAWNED.store(0, Ordering::SeqCst);
        *ARMED.lock() = Some(Armed { kind, pid, h });
    }

    /// The armed operation's result: a handle index or errno for `DERIVE`
    /// and `GRANT`; for `SPAWN`, 1 if the child still holds the inherited
    /// cap, 0 if it was spawned without it, or the spawn's errno; for
    /// `TEARDOWN`, the revoke's return.
    pub fn result() -> i64 {
        let pid = SPAWNED.swap(0, Ordering::SeqCst);
        if pid != 0 {
            let held = get(ProcessId(pid))
                .is_some_and(|p| without_interrupts(|| p.handles.lock().get(0).is_some()));
            // The child parks forever holding the cap; end it once read.
            without_interrupts(|| terminate(ProcessId(pid)));
            RESULT.store(held as i64, Ordering::SeqCst);
        }
        RESULT.load(Ordering::SeqCst)
    }

    fn take(kinds: &[u32]) -> Option<Armed> {
        let mut armed = ARMED.lock();
        if armed.as_ref().is_some_and(|a| kinds.contains(&a.kind)) {
            armed.take()
        } else {
            None
        }
    }

    fn rights() -> CapRights {
        CapRights::new(meta_rights::USE | meta_rights::DERIVE, 0)
    }

    fn install_now(a: &Armed) -> i64 {
        let r = if a.kind == DERIVE {
            derive_handle(a.pid, a.h, rights())
        } else {
            let source = get(a.pid)
                .and_then(|p| without_interrupts(|| p.handles.lock().get(a.h)))
                .map(|h| h.cap_id());
            match source {
                Some(cap_id) => {
                    complete_grant(a.pid, cap_id, rights(), a.pid, rights()).map_err(|e| match e {
                        GrantInstallError::GrantorBadCap => crate::errno::EBADF,
                        _ => crate::errno::EINVAL,
                    })
                }
                None => Err(crate::errno::EBADF),
            }
        };
        match r {
            Ok(idx) => idx as i64,
            Err(e) => e,
        }
    }

    fn spawn_now(inherited: InheritedHandle) {
        match spawn_user_by_name("worker", &[inherited]) {
            Ok(pid) => SPAWNED.store(pid.0, Ordering::SeqCst),
            Err(SpawnError::ParentGone) => RESULT.store(crate::errno::EBADF, Ordering::SeqCst),
            Err(_) => RESULT.store(crate::errno::EINVAL, Ordering::SeqCst),
        }
    }

    /// The inherited copy `sys_spawn` would have cloned from `h`.
    fn clone_inherited(a: &Armed) -> Option<InheritedHandle> {
        let h = get(a.pid).and_then(|p| without_interrupts(|| p.handles.lock().get(a.h)))?;
        let parent = h.cap_id();
        let rights = h.rights();
        Some(InheritedHandle {
            handle: h.with_cap(alloc_cap_id(), rights),
            parent: Some(parent),
        })
    }

    /// Whether this hook runs inside `CAP_GRAPH` (it must, unless the
    /// control removed the lock).
    #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
    fn lock_is_held() -> bool {
        CAP_GRAPH.try_lock().is_none()
    }

    pub(super) fn between_walk_and_removal() {
        let Some(a) = take(&[DERIVE, GRANT, SPAWN]) else {
            return;
        };
        let spawn = (a.kind == SPAWN).then(|| clone_inherited(&a)).flatten();
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        {
            if !lock_is_held() {
                RESULT.store(LOCK_NOT_HELD, Ordering::SeqCst);
                return;
            }
            *DEFERRED.lock() = Some(match spawn {
                Some(inherited) => Deferred::Spawn(inherited),
                None => Deferred::Install(a),
            });
        }
        #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
        match spawn {
            Some(inherited) => spawn_now(inherited),
            None => RESULT.store(install_now(&a), Ordering::SeqCst),
        }
    }

    pub(super) fn in_teardown() {
        let Some(a) = take(&[TEARDOWN]) else {
            return;
        };
        #[cfg(not(feature = "neo-test-cap-graph-unlocked-control"))]
        {
            if !lock_is_held() {
                RESULT.store(LOCK_NOT_HELD, Ordering::SeqCst);
                return;
            }
            *DEFERRED.lock() = Some(Deferred::Revoke(a));
        }
        #[cfg(feature = "neo-test-cap-graph-unlocked-control")]
        RESULT.store(revoke_now(&a), Ordering::SeqCst);
    }

    fn revoke_now(a: &Armed) -> i64 {
        match revoke_handle(a.pid, a.h) {
            Ok(()) => 0,
            Err(e) => e,
        }
    }

    fn run_deferred() {
        let deferred = DEFERRED.lock().take();
        match deferred {
            Some(Deferred::Install(a)) => RESULT.store(install_now(&a), Ordering::SeqCst),
            Some(Deferred::Spawn(inherited)) => spawn_now(inherited),
            Some(Deferred::Revoke(a)) => RESULT.store(revoke_now(&a), Ordering::SeqCst),
            None => {}
        }
    }

    pub(super) fn after_revoke() {
        run_deferred();
    }

    pub(super) fn after_teardown() {
        run_deferred();
    }

    // K1a-0 #183 item 6: the two-CPU grant leg. A grant that reaches the
    // barrier holds CAP_GRAPH between its source lookup and its install, and
    // waits there until the test releases it, so another CPU's revoke can be
    // started inside that window and seen to wait for the lock.
    static GRANT_BARRIER_ARMED: AtomicU32 = AtomicU32::new(0);
    static GRANT_BARRIER_REACHED: AtomicU32 = AtomicU32::new(0);
    static GRANT_BARRIER_RELEASE: AtomicU32 = AtomicU32::new(0);
    static GRANT_BARRIER_HELD: AtomicU32 = AtomicU32::new(0);

    /// Arm the barrier for the next `complete_grant`.
    pub fn arm_grant_barrier() {
        GRANT_BARRIER_REACHED.store(0, Ordering::SeqCst);
        GRANT_BARRIER_RELEASE.store(0, Ordering::SeqCst);
        GRANT_BARRIER_HELD.store(0, Ordering::SeqCst);
        GRANT_BARRIER_ARMED.store(1, Ordering::SeqCst);
    }

    /// Whether a grant is waiting at the barrier, and whether it held
    /// CAP_GRAPH when it got there.
    pub fn grant_barrier_reached() -> (bool, bool) {
        (
            GRANT_BARRIER_REACHED.load(Ordering::Acquire) == 1,
            GRANT_BARRIER_HELD.load(Ordering::Acquire) == 1,
        )
    }

    pub fn release_grant_barrier() {
        GRANT_BARRIER_RELEASE.store(1, Ordering::Release);
    }

    /// Janus #127 F1 leg: the recipient's other reference, held here until
    /// `complete_grant` has looked the recipient up.
    static RECIPIENT_LAST: Mutex<Option<Arc<Process>>> = Mutex::new(None);

    pub fn arm_recipient_last(proc: Arc<Process>) {
        *RECIPIENT_LAST.lock() = Some(proc);
    }

    /// Called by `complete_grant` just after its recipient lookup: the
    /// recipient exits and is unregistered, and its other reference drops (a
    /// reap's interleaving), so the grant's lookup is the last one.
    pub(super) fn after_recipient_lookup(pid: ProcessId) {
        let Some(proc) = RECIPIENT_LAST.lock().take_if(|p| p.pid == pid) else {
            return;
        };
        proc.exiting.store(true, Ordering::Release);
        unregister_unstarted(pid);
        drop(proc);
    }

    /// Called by `complete_grant` after the source lookup, before the
    /// install. Bounded: a test that never releases it fails on its own
    /// checks rather than hanging the kernel.
    pub(super) fn grant_barrier() {
        if GRANT_BARRIER_ARMED.swap(0, Ordering::SeqCst) == 0 {
            return;
        }
        GRANT_BARRIER_HELD.store(cap_graph_held_here() as u32, Ordering::SeqCst);
        GRANT_BARRIER_REACHED.store(1, Ordering::Release);
        for _ in 0..2_000_000_000u64 {
            if GRANT_BARRIER_RELEASE.load(Ordering::Acquire) == 1 {
                return;
            }
            core::hint::spin_loop();
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GrantInstallError {
    GrantorBadCap,
    GrantorMissingGrant,
    RecipientGone,
    EmptyIntersection,
}

pub fn complete_grant(
    grantor_pid: ProcessId,
    source_cap_id: CapId,
    offered: CapRights,
    recipient_pid: ProcessId,
    max_rights: CapRights,
) -> Result<u32, GrantInstallError> {
    // A refused child drops after `CAP_GRAPH` (see `install_cap`).
    let mut refused = None;
    let installed = without_interrupts(|| {
        // SC-4 D6: grants never hold two Process locks at once. We read the
        // grantor handle, release that lock, then install into the recipient.
        // That release-before-reacquire shape composes with the ascending-pid
        // rule used by revoke walks and avoids cross-process AB-BA cycles.
        //
        // K1a-0 (CP-87): CAP_GRAPH is held from the source lookup through
        // the recipient install, so a revoke of the source cannot fall
        // between them. The install below does not re-check the source, so
        // this section must not be split (`cap_graph_revoke`'s two-CPU grant
        // leg fails if it is).
        let mut g = lock_cap_graph();
        let grantor = g
            .get_pinned(grantor_pid)
            .ok_or(GrantInstallError::GrantorBadCap)?;
        let source = grantor
            .handles
            .lock()
            .get_by_cap_id(source_cap_id)
            .ok_or(GrantInstallError::GrantorBadCap)?;
        #[cfg(feature = "neo-test-cap-graph-revoke")]
        cap_graph_race::grant_barrier();
        let source_rights = source.rights();
        if !source_rights.has_meta(meta_rights::GRANT) {
            return Err(GrantInstallError::GrantorMissingGrant);
        }

        let actual = source_rights.intersection(offered).intersection(max_rights);
        if actual.meta == 0 {
            return Err(GrantInstallError::EmptyIntersection);
        }

        let recipient = g
            .get_pinned(recipient_pid)
            .ok_or(GrantInstallError::RecipientGone)?;
        #[cfg(feature = "neo-test-cap-graph-revoke")]
        cap_graph_race::after_recipient_lookup(recipient_pid);
        let child_cap_id = alloc_cap_id();
        // Phase 7 BUILD-3: capture the shared physical tensor's SLOT before
        // `source` is moved into the child handle. The mint itself happens
        // OUTSIDE this without_interrupts block (Forge M-1): mint can park
        // across an in-flight daemon migration, and parking IRQ-off is the
        // recorded P4a deadlock class. A key copy is enough here — the
        // recipient's freshly-installed entry pins the slot live past this
        // block, so the later mint cannot race the slot's death.
        let shared_slot = match &source {
            Handle::Tensor { slot, .. } => Some(*slot),
            _ => None,
        };
        let child = source.with_cap(child_cap_id, actual);
        let installed = install_minted_locked(
            &mut g,
            &recipient,
            child,
            Some(source_cap_id),
            false,
            &mut refused,
        )
        .map_err(|_| GrantInstallError::RecipientGone)?;
        Ok((installed, shared_slot))
    });
    drop(refused);
    let (installed, shared_slot) = installed?;
    // A granted tensor capability is a cross-process RO share — N capability
    // handles, one physical `Arc<Tensor>`. Register it as shared (owner =
    // grantor; residency counted once) and emit the ownership trace. The
    // reader set (including this recipient) is derived by scanning handle
    // tables, so revoke/drop need no further hook. Minted IRQ-on, dropped
    // at block end (the I-9 mint/drop discipline).
    if let Some(slot) = shared_slot
        && let Some(tensor) = crate::ai::arena::mint(slot)
    {
        crate::ai::sharing::on_tensor_shared(&tensor, grantor_pid, recipient_pid);
    }
    Ok(installed)
}

/// Record the current process's `sys_exit` code before its thread moves to
/// Zombie. The reaper transfers this into `COMPLETED_EXIT_CODES` before it
/// deregisters the process and drops the Pmap.
///
/// OT-98: this is also where the process commits to its natural exit. It
/// claims `exit_state` here, not at the reap: between the two its thread is
/// on its way to the zombie queue (or already in it, until the BSP reaps), a
/// state no kill scan can find. A `terminate` that loses this claim returns
/// ESRCH, and the reap delivers `code` to the waiters. A kill that claimed
/// first keeps its claim; the thread still exits.
///
/// OT-98 (Janus #174 F1): only the winning claim writes `code`. Two exit
/// producers can both get here (two threads in `SYS_EXIT` past the entry
/// check, or a fault exit racing one); the loser changes nothing, so the
/// waiters get the winner's code. The CAS and the store are one section under
/// `PROCESSES`, and the store precedes, in the winner's program order, its
/// thread's move to Zombie, which it publishes under its CPU's scheduler lock.
/// The reaper reads the code (`on_thread_reap`) only after it has taken that
/// lock to pop the winner's zombie (with `browser-runtime`, at the process's
/// last reap, which comes after every thread's, the winner's included), so the
/// store happens-before the read. Returns whether this call won the claim.
/// The control (`neo-test-kill-at-exit-control-f1`) stores before the claim,
/// whatever its outcome, as before.
pub fn record_exit_code(pid: ProcessId, code: i32) -> bool {
    #[cfg(feature = "neo-test-kill-at-exit")]
    crate::test_harness::kae_exit_hold(pid);
    let won = without_interrupts(|| {
        if pid == KERNEL_PID {
            return false;
        }
        let procs = PROCESSES.lock();
        let Some(proc_arc) = procs.get(&pid) else {
            return false;
        };
        // The control (`neo-test-kill-at-exit-control`) claims at the reap
        // again, and stores unconditionally, as before OT-98.
        if cfg!(feature = "neo-test-kill-at-exit-control") {
            proc_arc.exit_code.store(code, Ordering::Release);
            return false;
        }
        if cfg!(feature = "neo-test-kill-at-exit-control-f1") {
            proc_arc.exit_code.store(code, Ordering::Release);
        }
        let won = proc_arc
            .exit_state
            .compare_exchange(
                EXIT_STATE_RUNNING,
                EXIT_STATE_EXITING_NATURAL,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if won {
            proc_arc.exit_code.store(code, Ordering::Release);
        }
        won
    });
    #[cfg(feature = "neo-test-kill-at-exit")]
    crate::test_harness::kae_exit_recorded(pid, code, won);
    won
}

/// CP-66: whether the current thread's process is being torn down (no longer
/// registered, or `exiting`). The `SYS_INFER_RUN` boundary exits such a thread
/// instead of returning it to ring 3. Kernel threads are never torn down.
pub fn current_torn_down() -> bool {
    let pid = crate::sched::current_pid();
    if pid == KERNEL_PID {
        return false;
    }
    get(pid).is_none_or(|proc| proc.exiting.load(Ordering::Acquire))
}

/// Consume a completed process exit code after the process has been reaped.
pub fn take_completed_exit_code(pid: ProcessId) -> Option<i32> {
    without_interrupts(|| COMPLETED_EXIT_CODES.lock().remove(&pid))
}

fn peek_completed_exit_code(pid: ProcessId) -> Option<i32> {
    without_interrupts(|| COMPLETED_EXIT_CODES.lock().get(&pid).copied())
}

/// `claim` is the in-flight claim `sys_wait` took from its handle (#171 F1),
/// or `None`. It covers a code already kept for a reaped target; a target
/// still registered or dying delivers its code through `waiters` instead, so
/// the claim is released before the wait can park (see `wait_for_exit`).
pub fn wait_for_exit_abi(target_pid: ProcessId, flags: u64, claim: Option<ExitClaim>) -> i64 {
    let ret = wait_for_exit(target_pid, flags, claim);
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    kill_hold_note_wait(target_pid);
    ret
}

fn wait_for_exit(target_pid: ProcessId, flags: u64, claim: Option<ExitClaim>) -> i64 {
    use crate::errno::{EAGAIN, EINVAL, ESRCH};

    if flags & !WAIT_FLAG_WNOHANG != 0 {
        return EINVAL;
    }

    // OT-93 F2: a force-killed process still in `DYING` is waited on like a
    // live one, until its last thread is reaped.
    let proc_arc = match get_live_or_dying(target_pid) {
        Some(proc_arc) => proc_arc,
        None => {
            // The kept code, which `claim` held for us (#171 F1).
            let ret = match take_completed_exit_code(target_pid) {
                Some(code) => encode_wait_exit_code(code),
                None => ESRCH,
            };
            drop(claim);
            return ret;
        }
    };
    // The target is registered or dying, so its code comes through its
    // `waiters` (the completion is recorded there before any teardown drops
    // its `Arc`), not the kept-code map. The claim isn't needed, and it must
    // go before the park: a parked waiter can be zombified raw (CP-30,
    // OT-98), and a raw free runs no `Drop`, which would leak the claim and
    // the pid's map entries for good (the PM's #238 bounce).
    #[cfg(not(feature = "neo-test-wait-claim-park-control"))]
    drop(claim);
    // The control holds it across the park, as before.
    #[cfg(feature = "neo-test-wait-claim-park-control")]
    let _held_across_park = claim;

    // An early return hands the `Arc` back, so it is dropped below, outside
    // the waiters lock. It can be the last one, and a syscall runs with
    // interrupts off (the PM's bounce, note 1), so it goes through
    // `release_last_process` (CP-91).
    let (ret, unparked) = without_interrupts(move || {
        // SAFETY: `proc_arc` keeps the target alive until it is dropped below,
        // which happens only while `guard` holds the target's `waiters` lock
        // with no completion recorded; see the CP-30 note at that drop.
        let target: &Process = unsafe { &*Arc::as_ptr(&proc_arc) };
        let mut guard = target.waiters.lock();
        if let Some(code) = guard.consume_completion() {
            COMPLETED_EXIT_CODES.lock().remove(&target_pid);
            drop(guard);
            return (encode_wait_exit_code(code), Some(proc_arc));
        }
        if guard.is_consumed() {
            drop(guard);
            return (ESRCH, Some(proc_arc));
        }
        let exit_state = target.exit_state.load(Ordering::Acquire);
        // OT-93 F2: a process being force-killed is waited on too; its kill
        // completes the waiters (at its teardown, or at its last reap).
        if exit_state == EXIT_STATE_EXITED {
            drop(guard);
            return (ESRCH, Some(proc_arc));
        }
        if flags & WAIT_FLAG_WNOHANG != 0 {
            drop(guard);
            return (EAGAIN, Some(proc_arc));
        }
        if guard.is_full() {
            drop(guard);
            return (EAGAIN, Some(proc_arc));
        }
        // CP-30 (OT-98): a `sys_wait`-parked thread can be zombified raw, by a
        // kill of its own process (`terminate`) or a fault-kill of a sibling
        // (OT-61 C1.1), and a raw free runs no Drop. So the waiter parks owning
        // nothing: drop the owned `Arc` now, under the target's `waiters` lock.
        // It is never the last one. Every teardown records the completion
        // under this lock (`complete_process_waiters`) before it drops its own
        // `Arc`, and none is recorded, so a teardown holder (the registry,
        // `DYING`, or `terminate`'s and `on_thread_reap`'s locals) still holds
        // one, and it
        // can't drop it before it takes this lock and wakes the waiter. The
        // woken waiter returns without touching the target.
        drop(proc_arc);
        let ret = scheduler::block_current_into_with_guard(guard, |queue, waiter| {
            queue.push(waiter);
        });
        (ret, None)
    });
    if let Some(proc_arc) = unparked {
        release_last_process(proc_arc);
    }
    ret
}

pub fn inspect_process(target_pid: ProcessId) -> Result<ProcessInspectSnapshot, i64> {
    use crate::errno::ESRCH;

    if let Some(proc_arc) = get(target_pid) {
        let state = if proc_arc.exit_state.load(Ordering::Acquire) == EXIT_STATE_RUNNING {
            PROCESS_INSPECT_STATE_RUNNING
        } else {
            PROCESS_INSPECT_STATE_EXITED
        };
        let exit_code = if state == PROCESS_INSPECT_STATE_RUNNING {
            0
        } else {
            let code = proc_arc.exit_code.load(Ordering::Acquire);
            if code == EXIT_CODE_RUNNING { 0 } else { code }
        };
        let (handle_count, channel_handle_count, network_handle_count) = without_interrupts(|| {
            let handles = proc_arc.handles.lock();
            (
                handles.live_count() as u32,
                handles.channel_count() as u32,
                handles.network_count() as u32,
            )
        });
        // OT-61 C1.1: report the live-thread refcount (pre-C1.1 this was a
        // hardcoded 1 — every process had exactly one thread). Registry-less
        // processes (test-harness-constructed) keep reporting 1.
        //
        // The OFF arm is written as the original inline `thread_count: 1`
        // literal rather than a local, so the default build emits byte-identical
        // code (a local costs 2 bytes at opt-level 0 — measured).
        #[cfg(feature = "browser-runtime")]
        let thread_count = proc_arc.threads.lock().live.max(1);
        return Ok(ProcessInspectSnapshot {
            state,
            pid: target_pid,
            exit_code,
            #[cfg(feature = "browser-runtime")]
            thread_count,
            #[cfg(not(feature = "browser-runtime"))]
            thread_count: 1,
            handle_count,
            channel_handle_count,
            network_handle_count,
            name: proc_arc.name,
        });
    }

    match peek_completed_exit_code(target_pid) {
        Some(code) => Ok(ProcessInspectSnapshot {
            state: PROCESS_INSPECT_STATE_EXITED,
            pid: target_pid,
            exit_code: code,
            thread_count: 0,
            handle_count: 0,
            channel_handle_count: 0,
            network_handle_count: 0,
            name: "",
        }),
        None => Err(ESRCH),
    }
}

fn complete_process_waiters(proc_arc: &Process, pid: ProcessId, exit_code: Option<i32>) {
    // OT-98 (CP-98, FAIL-B): the waiters are woken under the `waiters` lock
    // (the waiters → scheduler order the park already uses). Taken out and
    // woken later, a waiter is in no queue for a while, and a kill of its
    // process then finds no thread of it at all. The control
    // (`neo-test-kill-at-exit-control-b`) wakes them after the lock, as before.
    let wake_under_lock = !cfg!(feature = "neo-test-kill-at-exit-control-b");
    let mut waiters = without_interrupts(|| {
        let mut guard = proc_arc.waiters.lock();
        // The completion is recorded before the waiters are taken: with a
        // waiter queued, the code goes to it and the completion is consumed.
        // Recorded after (an empty queue), it stayed unconsumed as well, so a
        // later `sys_wait` that still found the process registered (between
        // this and the deregistration) got the code a second time
        // (`sys_wait_multiple_waiters`, intermittent; on main too).
        guard.complete(exit_code);
        let mut waiters = guard.take_all();
        // OT-91: keep the code only if someone can claim it; the claims lock
        // orders this against a last handle's release.
        if waiters.is_empty()
            && let Some(code) = exit_code
        {
            let claims = EXIT_CLAIMS.lock();
            if claims.contains_key(&pid) {
                COMPLETED_EXIT_CODES.lock().insert(pid, code);
            }
        }
        if wake_under_lock {
            wake_process_waiters(&mut waiters, exit_code);
        }
        drop(guard);
        waiters
    });
    #[cfg(feature = "neo-test-kill-at-exit")]
    crate::test_harness::kae_after_waiters_taken(pid);
    wake_process_waiters(&mut waiters, exit_code);
}

/// Wake each waiter in `waiters`: the first with `exit_code`, the rest (or
/// all, after a kill) with ESRCH. Leaves `waiters` empty.
fn wake_process_waiters(waiters: &mut ProcessWaiterBatch, exit_code: Option<i32>) {
    let mut delivered = exit_code.is_none();
    for i in 0..waiters.len() {
        if let Some(waiter) = waiters.waiters[i].take() {
            match exit_code {
                Some(code) if !delivered => {
                    delivered = true;
                    scheduler::wake_thread(waiter, encode_wait_exit_code(code));
                }
                _ => scheduler::wake_thread(waiter, crate::errno::ESRCH),
            }
        }
    }
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_mark_exiting_for_test(pid: ProcessId) -> bool {
    match get(pid) {
        Some(proc_arc) => {
            proc_arc.exiting.store(true, Ordering::Release);
            true
        }
        None => false,
    }
}

#[cfg(feature = "neo-test")]
#[allow(dead_code)]
pub fn debug_exit_state_for_test(pid: ProcessId) -> Option<u8> {
    get(pid).map(|proc_arc| proc_arc.exit_state.load(Ordering::Acquire))
}

/// Top of the user stack. Sits high in the canonical lower half so it
/// doesn't collide with any reasonable ELF segment placement.
const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;
// Phase 4 rustls handshakes need more userspace call-stack headroom than the
// original IPC/VFS smoke programs.
const USER_STACK_SIZE: usize = 64 * 1024;

/// Errors propagated out of `spawn_user`.
#[derive(Debug)]
pub enum SpawnError {
    Elf(ElfError),
    Pmap,
    SchedulerFull,
    /// An inherited handle's parent was closed or revoked before the child
    /// was registered (K1a-0, CP-87).
    ParentGone,
    /// OT-61 C2.2b: the spawn's weight (charged to the parent) or an
    /// inherited handle's (charged to the child) would pass an object limit.
    #[cfg(feature = "obj-quota")]
    ObjQuota,
}

impl From<ElfError> for SpawnError {
    fn from(e: ElfError) -> Self {
        SpawnError::Elf(e)
    }
}

/// The body of [`spawn_user`], shared with the OT-61 C2.2 budgeted spawn.
/// A macro rather than a function so that, with `mem-quota` off,
/// `spawn_user` compiles exactly as before (the closure below is still
/// `spawn_user`'s, not a helper's, so no symbol in the kernel moves).
macro_rules! spawn_user_body {
    ($name:expr, $elf_bytes:expr, $inherited:expr, $new_pmap:expr) => {{
    let shootdown_quiesce = crate::mm::tlb_shootdown::quiesce_local_cpu();
    let result = without_interrupts(|| {
        let elf = Elf::parse($elf_bytes)?;
        let pmap = $new_pmap.map_err(|_| SpawnError::Pmap)?;
        let entry = elf.load_into(&pmap)?;

        // Allocate user stack at the top of the user half.
        let stack_top = VirtAddr::new(USER_STACK_TOP);
        allocate_user_stack(&pmap, stack_top, USER_STACK_SIZE)?;
        // SysV ABI: at function entry, `(RSP+8) % 16 == 0`. Initial RSP must
        // satisfy that. stack_top is page-aligned (4096), so stack_top - 8
        // gives RSP ≡ 8 mod 16.
        let initial_user_rsp = USER_STACK_TOP - 8;

        let process = Process::new_user($name, pmap);
        let pid = process.pid;
        let pmap_for_thread = process
            .pmap
            .as_ref()
            .cloned()
            .expect("user process must carry pmap");

        // K1a-0 (CP-87): the inherited handles were cloned from the caller
        // before this call, and the child is invisible to a revoke walk until
        // it is registered. Under CAP_GRAPH, check every parent is still live,
        // then install and register, so a revoke either finished first (and
        // the spawn is refused) or walks a registered child.
        let mut g = lock_cap_graph();
        if CAP_GRAPH_ENFORCED
            && $inherited
                .iter()
                .any(|h| h.parent.is_some_and(|p| !cap_is_live(&mut g, p)))
        {
            return Err(SpawnError::ParentGone);
        }

        // Install inherited handles at indices 0..n in the order given —
        // ADR-004 §Q2 deterministic-ordering ABI clause. HandleTable::new()
        // is empty; install() picks the lowest free slot, so the i-th
        // handle lands at child handle index i exactly.
        {
            let mut table = process.handles.lock();
            for (i, h) in $inherited.iter().enumerate() {
                let cap_id = h.handle.cap_id();
                // OT-61 C2.2b: the child's handles charge the child's account.
                #[cfg(feature = "obj-quota")]
                table
                    .charge_next(&h.handle)
                    .map_err(|_| SpawnError::ObjQuota)?;
                let idx = table.install(h.handle.clone());
                process.derivations.insert(&g, cap_id, h.parent);
                g.note_parent(h.parent);
                debug_assert_eq!(
                    idx as usize, i,
                    "$inherited handle ordering violated: i={} got idx={}",
                    i, idx
                );
            }
        }

        let proc_arc = register(process);
        drop(g);

        let mut thread = Thread::new_user($name, pid, entry, initial_user_rsp, pmap_for_thread);
        thread.set_process_anchor(proc_arc.clone());

        // OT-61 C1.1: record the INITIAL thread in the registry too (the P2
        // amendment: `thread_get_info` must serve the first process thread,
        // not only `thread_create` children). Its stack is the fixed
        // `[USER_STACK_TOP - USER_STACK_SIZE, USER_STACK_TOP)` window; the
        // page below is unmapped by construction (nothing else is placed
        // there), so it is reported as the guard page — a downward overflow
        // faults into it exactly like an allocated guard.
        #[cfg(feature = "browser-runtime")]
        if let Some(proc_arc) = get(pid) {
            let usable_low = USER_STACK_TOP - USER_STACK_SIZE as u64;
            let _ = proc_arc.threads.lock().insert(
                thread.id.0,
                ThreadStackBounds {
                    usable_low,
                    usable_len: USER_STACK_SIZE as u64,
                    guard_low: usable_low - 0x1000,
                    guard_len: 0x1000,
                },
                // created=false: the initial thread's stack is spawn_user's,
                // freed by the Pmap drop at process exit — never per-thread.
                false,
            );
        }

        // OT-91: claim the exit code before the child can run, so a child
        // that exits before its spawner installs a handle keeps its code.
        update_exit_claims(pid, ClaimOp::SetSpawn);
        if let Err(thread) = scheduler::try_push_ready(thread) {
            unregister_unstarted(pid);
            Thread::free(thread);
            // OT-93 (the PM's re-review, note 2): a kill that landed since the
            // anchor was set is waiting for this thread's count; its free
            // re-checks, as a reap does.
            thread_reaped(pid);
            release_spawn_claim(pid);
            return Err(SpawnError::SchedulerFull);
        }

        #[cfg(feature = "neo-test-swapgs-irq-ring3")]
        crate::test_harness::wait_for_swapgs_publication_filter_probe();

        serial_println!(
            "[ kernel ] proc: spawned '{}' (pid={}) entry=0x{:016x} user_rsp=0x{:016x} handles_inherited={}",
            $name,
            pid.0,
            entry,
            initial_user_rsp,
            $inherited.len()
        );
        crate::mm::frame_audit("post-spawn");

        Ok(pid)
    });

    // Keep the current thread on-CPU while resume temporarily enables IF to
    // wait for the sender lock. The lock serializes the final local flush and
    // readiness publication against every remote mapping update.
    let _preemption_guard = scheduler::disable_preemption();
    shootdown_quiesce.resume();
    #[cfg(feature = "neo-test-swapgs-irq-ring3")]
    crate::test_harness::wait_for_swapgs_rejoin_ack_probe();
    result
    }};
}

/// Spawn a new user process. Loads `elf_bytes` (a Limine module) into a
/// fresh Pmap, allocates and maps a user stack, registers the process,
/// installs any inherited handles at indices 0..n in the deterministic
/// order ADR-004 §Q2 specifies, creates the first thread, and pushes it
/// onto the scheduler ready queue.
///
/// Runs the construction transaction with interrupts masked, but temporarily
/// removes this CPU from remote TLB-shootdown target sets first. Any mappings
/// changed while quiesced are covered by a full local flush before readiness
/// is restored. `spawn_user` returns with IF enabled so the republished CPU is
/// immediately able to acknowledge remote shootdowns.
pub fn spawn_user(
    name: &'static str,
    elf_bytes: &[u8],
    inherited: &[InheritedHandle],
) -> Result<ProcessId, SpawnError> {
    #[cfg(not(feature = "mem-quota"))]
    {
        spawn_user_body!(name, elf_bytes, inherited, Pmap::new())
    }
    // OT-61 C2.2: an unbudgeted child shares its spawner's memory account.
    #[cfg(feature = "mem-quota")]
    {
        spawn_user_in(name, elf_bytes, inherited, caller_mem_account())
    }
}

/// OT-61 C2.2: the memory account of the calling process: its Pmap's, or the
/// unlimited root when the caller is the kernel or a kernel-built fixture.
#[cfg(feature = "mem-quota")]
pub fn caller_mem_account() -> Arc<crate::mm::mem_account_logic::MemAccount> {
    get(scheduler::current_pid())
        .and_then(|p| p.pmap.as_ref().and_then(|pm| pm.account().cloned()))
        .unwrap_or_else(root_mem_account)
}

/// OT-61 C2.2b: the part of the kernel heap user objects can never claim
/// (drivers, the NeoFS cache, IPC, the kernel's own processes). Half the heap
/// on the 4 MiB default; the root's object limit is the rest.
#[cfg(feature = "obj-quota")]
pub const OBJ_HEAP_RESERVE: usize = crate::mm::heap::HEAP_SIZE / 2;

/// OT-61 C2.2: the unlimited root account (init's).
#[cfg(feature = "mem-quota")]
pub fn root_mem_account() -> Arc<crate::mm::mem_account_logic::MemAccount> {
    static ROOT: spin::Once<Arc<crate::mm::mem_account_logic::MemAccount>> = spin::Once::new();
    #[cfg(not(feature = "obj-quota"))]
    let root = ROOT.call_once(crate::mm::mem_account_logic::MemAccount::root);
    // OT-61 C2.2b: every user object is charged under the root, whose limit
    // leaves the kernel's own allocations a reserve of the heap.
    #[cfg(feature = "obj-quota")]
    let root = ROOT.call_once(|| {
        crate::mm::mem_account_logic::MemAccount::root_with_obj_limit(
            (crate::mm::heap::HEAP_SIZE - OBJ_HEAP_RESERVE) as u64,
        )
    });
    root.clone()
}

/// OT-61 C2.2: [`spawn_user_by_name`] whose child gets its own account of
/// `limit_frames`, nested under the caller's. The child's PML4, image and
/// stack are charged to it, so a budget too small to load the module fails
/// here (`SpawnError::Elf(OutOfFrames)` or `SpawnError::Pmap`), and the
/// dropped Pmap releases whatever was charged (CP-89).
#[cfg(feature = "mem-quota")]
pub fn spawn_user_by_name_budgeted(
    name: &str,
    inherited: &[InheritedHandle],
    limit_frames: u64,
) -> Result<ProcessId, SpawnError> {
    let mut path_suffix = alloc::string::String::with_capacity(name.len() + 5);
    path_suffix.push_str("/neo-");
    path_suffix.push_str(name);
    let bytes = crate::find_module_by_path_suffix(&path_suffix)
        .ok_or(SpawnError::Elf(ElfError::TooSmall))?;
    let account =
        crate::mm::mem_account_logic::MemAccount::child(&caller_mem_account(), limit_frames);
    spawn_user_in(name_to_static(name), bytes, inherited, account)
}

/// OT-61 C2.2: [`spawn_user`] with the child's Pmap charged to `account`.
#[cfg(feature = "mem-quota")]
fn spawn_user_in(
    name: &'static str,
    elf_bytes: &[u8],
    inherited: &[InheritedHandle],
    account: Arc<crate::mm::mem_account_logic::MemAccount>,
) -> Result<ProcessId, SpawnError> {
    // OT-61 C2.2b: the parent pays for the spawn, before anything is
    // allocated (so a spawn loop trips its own budget). A failed spawn drops
    // the charge; a spawned child holds it until its `Process` drops.
    #[cfg(feature = "obj-quota")]
    let charge =
        crate::mm::mem_account_logic::ObjCharge::new(&caller_mem_account(), obj_weights::SPAWN)
            .map_err(|_| SpawnError::ObjQuota)?;
    let spawned = spawn_user_body!(name, elf_bytes, inherited, Pmap::new_charged(account));
    #[cfg(feature = "obj-quota")]
    if let Ok(pid) = spawned
        && let Some(child) = get(pid)
    {
        *child.obj_spawn.lock() = Some(charge);
    }
    spawned
}

/// OT-61 C2.2b: [`spawn_user_by_name_budgeted`] with an object budget too.
/// Either limit may be `UNLIMITED`; both only narrow.
#[cfg(feature = "obj-quota")]
pub fn spawn_user_by_name_budgeted2(
    name: &str,
    inherited: &[InheritedHandle],
    limit_frames: u64,
    obj_limit: u64,
) -> Result<ProcessId, SpawnError> {
    let mut path_suffix = alloc::string::String::with_capacity(name.len() + 5);
    path_suffix.push_str("/neo-");
    path_suffix.push_str(name);
    let bytes = crate::find_module_by_path_suffix(&path_suffix)
        .ok_or(SpawnError::Elf(ElfError::TooSmall))?;
    let account = crate::mm::mem_account_logic::MemAccount::child_with_obj_limit(
        &caller_mem_account(),
        limit_frames,
        obj_limit,
    );
    spawn_user_in(name_to_static(name), bytes, inherited, account)
}

/// Phase 1.5 sys_spawn entry: look up a Limine module whose path ends
/// with `/neo-<name>` and ELF-load it as a new user process. Returns
/// the new process's pid on success, propagates SpawnError otherwise.
///
/// Per Bible Article 5.5 (spawn-only, no fork). The hybrid model is in
/// effect: main.rs spawns neo-init from a Limine module via the existing
/// `spawn_user`; neo-init spawns workers via THIS function (driven from
/// the SYS_SPAWN dispatcher in arch::x86_64::syscall). All ELF data
/// comes from Limine modules; Phase 3+ NeoFS will add disk-backed paths.
pub fn spawn_user_by_name(
    name: &str,
    inherited: &[InheritedHandle],
) -> Result<ProcessId, SpawnError> {
    // Match against `/neo-<name>` so callers pass "worker" and find
    // `/modules/neo-worker`. Limited concatenation; small bounded alloc.
    let mut path_suffix = alloc::string::String::with_capacity(name.len() + 5);
    path_suffix.push_str("/neo-");
    path_suffix.push_str(name);
    let bytes = crate::find_module_by_path_suffix(&path_suffix)
        .ok_or(SpawnError::Elf(ElfError::TooSmall))?;
    spawn_user(name_to_static(name), bytes, inherited)
}

/// Phase 1.5 stop-gap: Process::name is `&'static str` but `sys_spawn`
/// receives a runtime user-supplied name. Until Phase 1.6 makes Process
/// names heap-owned (Box<str>), we map the user-supplied name to a small
/// known-static set. Smoke test only uses "worker"; future workers add
/// more arms here. Intentional ugliness — flagged for cleanup.
fn name_to_static(name: &str) -> &'static str {
    match name {
        "worker" => "worker",
        "init" => "init",
        _ => "user-process",
    }
}

/// Look up a process's name by pid. Returns None if the pid isn't
/// registered (kernel pid, or a process that's been on_thread_reap'd away).
/// Used by main's spawn_neo_init wait loop and by sys_exit_impl for the
/// exit log line.
pub fn name_of(pid: ProcessId) -> Option<&'static str> {
    without_interrupts(|| {
        if pid == KERNEL_PID {
            return None;
        }
        PROCESSES.lock().get(&pid).map(|p| p.name)
    })
}

/// True iff any non-kernel process is currently registered. Phase 1.4
/// uses this from main's wait loop to detect when neo-init has been
/// fully reaped. O(N) walk; N ≤ 1 in Phase 1.4.
/// OT-90 P3a (K1): the pids of every user process registered now.
#[cfg(feature = "power-clean")]
pub fn user_pids() -> Vec<ProcessId> {
    let n = without_interrupts(|| PROCESSES.lock().len());
    let mut out = Vec::with_capacity(n + 4);
    without_interrupts(|| {
        for pid in PROCESSES.lock().keys() {
            if *pid != KERNEL_PID && out.len() < out.capacity() {
                out.push(*pid);
            }
        }
    });
    out
}

pub fn any_user_process() -> bool {
    without_interrupts(|| PROCESSES.lock().values().any(|p| p.pid != KERNEL_PID))
}

/// Aggregate-only process registry count for `sys_sysstat`.
///
/// Deliberately excludes pid/name/handle details: this is not the deferred
/// process-enumeration observability lane.
pub fn user_process_count() -> u64 {
    without_interrupts(|| {
        PROCESSES
            .lock()
            .values()
            .filter(|p| p.pid != KERNEL_PID)
            .count() as u64
    })
}

/// Called from `scheduler::reap` when a thread is dropped. If this was the
/// last thread in a user process, deregister the Process (which drops its
/// `Pmap`). Phase 1.4: every user process has exactly one thread, so any
/// reap removes the Process. Phase 1.5+ will refcount threads per process.
///
/// Phase 1.5b: walks the exiting process's handle table and signals each
/// peer endpoint per ADR-004 §"Process-exit cleanup" (a blocked peer
/// wakes with `-EPIPE`; an idle peer transitions to `Closed` so its next
/// operation returns `-EPIPE`), then deregisters.
///
/// **Two PROCESSES.lock() acquisitions, intentional.** A "single
/// remove-then-cleanup" shape is tempting but interacts subtly with
/// the rendezvous Drop chain when the registry's Arc is the only
/// remaining reference — remove() must not transfer the Arc into a
/// local that's then used to drive close_peer's wake path, because the
/// rendezvous-time Drop sequence becomes order-sensitive in a way that
/// has surfaced as a deadlock. The clone-cleanup-remove pattern keeps
/// the registry's Arc alive across the cleanup phase and only drops
/// the Process at end-of-fn, which is the safe ordering. Phase 2's
/// SMP-safe restructure may consolidate these once the rendezvous path
/// stops needing the staged-drop guarantee.
pub fn on_thread_reap(pid: ProcessId) {
    if pid == KERNEL_PID {
        return;
    }

    let proc_arc = match without_interrupts(|| PROCESSES.lock().get(&pid).cloned()) {
        Some(p) => p,
        // OT-93 F2: a killed process's last thread is reaped.
        None => return finish_kill(pid, KillStep::ThreadReaped),
    };
    #[cfg(feature = "neo-test-cap-graph-revoke")]
    reap_race::after_lookup(pid);
    // OT-98: `record_exit_code` usually claimed the natural exit already; a
    // thread that ended without one (no `sys_exit`) is claimed here.
    match proc_arc.exit_state.compare_exchange(
        EXIT_STATE_RUNNING,
        EXIT_STATE_EXITING_NATURAL,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) | Err(EXIT_STATE_EXITING_NATURAL) => proc_arc.exiting.store(true, Ordering::Release),
        Err(state) => {
            // A kill claimed it and has not moved it to `DYING` yet: this
            // reap released its thread's anchor (`Process::live_threads`),
            // which `terminate`'s completion reads (the PM's #204 bounce,
            // item 1).
            serial_println!(
                "[ kernel ] proc: natural reap skipped for pid={} state={}",
                pid.0,
                state
            );
            // The kill can complete meanwhile on another CPU and release
            // its references, leaving this clone the last: hand it to the
            // reaper when this reap is nested or IF is off (Janus #188
            // recheck 2, item 2).
            release_last_process(proc_arc);
            return;
        }
    }
    let exit_code = proc_arc.exit_code.load(Ordering::Acquire);

    // K1a-0 (CP-88): retire this process's edges under CAP_GRAPH, so the
    // children it granted or passed on keep a path to their ancestors.
    let mut drained = without_interrupts(|| lock_cap_graph().teardown(&proc_arc));
    #[cfg(feature = "neo-test-cap-graph-revoke")]
    cap_graph_race::after_teardown();
    close_channel_peers(drained.handles(), pid);
    drained.close_net();

    complete_process_waiters(
        &proc_arc,
        pid,
        (exit_code != EXIT_CODE_RUNNING).then_some(exit_code),
    );
    proc_arc
        .exit_state
        .store(EXIT_STATE_EXITED, Ordering::Release);
    without_interrupts(|| {
        PROCESSES.lock().remove(&pid);
    });

    let name = proc_arc.name;
    let pid_for_log = pid.0;
    // Explicit drop so Pmap::Drop's frame reclaim happens BEFORE the
    // frame_audit print, mirroring `terminate`'s ordering. Inside a nested
    // reap (NeoFS locks held) the process goes to the reaper instead.
    drop(drained);
    release_last_process(proc_arc);
    // K1a-0 F1: processes a CAP_GRAPH scan held last, dropped here too,
    // unless this reap is nested (then the reaper's top level).
    drop_deferred_processes_if_interruptible();

    serial_println!(
        "[ kernel ] proc: deregistered '{}' (pid={})",
        name,
        pid_for_log
    );
    crate::mm::frame_audit("post-natural-exit");
}

/// OT-61 C1.1: refcounted reap accounting. Every thread reap decrements the
/// owning process's live-thread count; only the LAST thread's reap runs the
/// classic `on_thread_reap` teardown (which drops the shared `Pmap`). The
/// `Arc<Pmap>` held by each remaining `Thread` makes an early drop
/// structurally impossible; this gate keeps the PROCESS-level teardown
/// (handle drain, peer EPIPE, waiter completion, deregistration) from firing
/// while sibling threads still run — the CP-19…CP-28 teardown-race class.
#[cfg(feature = "browser-runtime")]
pub fn on_thread_reap_threaded(pid: ProcessId, tid: u64) {
    if pid == KERNEL_PID {
        return;
    }
    // OT-93 F2: a killed process's threads are still accounted, so its last
    // reap can complete the kill.
    let proc_arc = match get_live_or_dying(pid) {
        Some(p) => p,
        // Already removed (forced kill / fault kill tore it down): the
        // registry died with the Process; nothing to account.
        None => return,
    };
    // Blocker C: if this process was ring-3 fault-killed, drain its remaining
    // IPC-parked and process-wait-parked siblings HERE, in the reaper's normal
    // kernel context (safe to walk the handle table / wait queues — unlike the
    // #PF exception frame). Each drained sibling becomes a zombie and reaps in
    // turn, re-entering this path until the live count reaches zero. Idempotent
    // (extracts only what is still parked); runs before note_reaped so a
    // just-drained sibling is not miscounted as this reap.
    if proc_arc.fault_killing.load(Ordering::Acquire) {
        zombify_ipc_parked_siblings(&proc_arc, pid);
        zombify_wait_parked_siblings(pid);
        // OT-22 (AAG F1): a sibling that entered SYS_WAIT_ANY after the #PF
        // frame's purge is token-parked; drain it here too (wait_any also
        // refuses to park an exiting process, so this only closes the race).
        #[cfg(feature = "wait-multiplex")]
        {
            crate::sched::wait_any::purge_pid(pid);
            while scheduler::zombify_parked_thread_by_pid(pid) {}
        }
    }

    let (last, free_stack) = proc_arc.threads.lock().note_reaped(tid);

    // Blocker B: free the reaped thread's created stack now, at the correct
    // teardown boundary — the thread is off both its user stack (it exited to
    // ring 0 and is being reaped) and its kernel stack (already `Thread::free`d
    // by the reaper before this call). Only when the process SURVIVES; on the
    // last thread the `Pmap` drop below frees every stack in one walk. Done
    // AFTER releasing the registry lock (unmap does a TLB shootdown; never hold
    // the registry spinlock across it). No owned Arc crosses a block here — the
    // reaper is not the reaped thread, so CP-30 does not apply.
    if let Some(bounds) = free_stack
        && let Some(pmap) = proc_arc.pmap.as_ref()
    {
        let _ = pmap.unmap_user_range(VirtAddr::new(bounds.usable_low), bounds.usable_len as usize);
        crate::arch::x86_64::flush_local_tlb();
    }

    // This can be the process's last `Arc` (a cross-CPU kill completed
    // meanwhile), and the reap can be nested under NeoFS locks: hand it to
    // the reaper then (the PM's #226 follow-up).
    release_last_process(proc_arc);
    if last {
        on_thread_reap(pid);
    }
}

/// OT-61 C1.1 (P2): `SYS_THREAD_CREATE` body. Allocates the new thread's
/// user stack (guarded on request) from the CALLER's `Pmap` via the C1.0
/// mmap machinery, registers the thread record (bounds retained for
/// `thread_get_info`), builds the ring-3 thread with `arg` in RDI, and
/// enqueues it. Returns the new tid or a negative errno. The cap gate
/// (`BrowserRuntime`/THREAD_SPAWN) runs at the syscall layer.
#[cfg(feature = "browser-runtime")]
pub fn thread_create_for_current(entry: u64, arg: u64, stack_size: usize, guard: bool) -> i64 {
    thread_create_for_current_placed(entry, arg, stack_size, guard, scheduler::try_push_ready)
}

/// OT-61 C1.2 test-only: `thread_create_for_current`, but the new thread starts
/// on `cpu` (`place_thread_on_cpu`, which takes CP-35's reservation there), so
/// `threads_tls` can have a waker on another CPU wake a thread across CPUs.
/// Panics if the placement fails: this scenario's target ring is never full.
#[cfg(any(
    feature = "neo-test-threads-tls",
    feature = "neo-test-threads-exit-group",
    feature = "neo-test-user-copy-fault"
))]
pub fn thread_create_for_current_on_cpu(
    entry: u64,
    arg: u64,
    stack_size: usize,
    guard: bool,
    cpu: u32,
) -> i64 {
    thread_create_for_current_placed(entry, arg, stack_size, guard, |thread| {
        if let Err(e) = scheduler::place_thread_on_cpu(thread, cpu) {
            panic!("threads_tls: place_thread_on_cpu({cpu}) failed: {e}");
        }
        Ok(())
    })
}

#[cfg(feature = "browser-runtime")]
fn thread_create_for_current_placed(
    entry: u64,
    arg: u64,
    stack_size: usize,
    guard: bool,
    place: impl FnOnce(alloc::boxed::Box<Thread>) -> Result<(), alloc::boxed::Box<Thread>>,
) -> i64 {
    use crate::errno::{EAGAIN, EINVAL, ENOMEM};

    let pid = scheduler::current_pid();
    let Some(proc_arc) = get(pid) else {
        return EINVAL;
    };
    let Some(pmap) = proc_arc.pmap.as_ref().cloned() else {
        return EINVAL;
    };

    // OT-61 C2.2b: the creator's account pays for the thread and its kernel
    // stack, before either is allocated; `Thread::free` releases it.
    #[cfg(feature = "obj-quota")]
    let charge = match pmap.account() {
        Some(account) => {
            match crate::mm::mem_account_logic::ObjCharge::new(account, obj_weights::THREAD) {
                Ok(c) => Some(c),
                Err(_) => return EAGAIN,
            }
        }
        None => None,
    };

    let rounded = (stack_size + 0xFFF) & !0xFFF;
    let base = match pmap.allocate_user_region_guarded(
        rounded,
        crate::mm::pmap::flags::USER_DATA_RW,
        guard,
    ) {
        Ok(base) => base,
        Err(_) => return ENOMEM,
    };
    crate::arch::x86_64::flush_local_tlb();

    // SysV: at fn entry `(RSP+8) % 16 == 0`; base is page-aligned so
    // `top - 8` gives RSP ≡ 8 mod 16 (the spawn_user discipline).
    let user_rsp = base + rounded as u64 - 8;
    let bounds = ThreadStackBounds {
        usable_low: base,
        usable_len: rounded as u64,
        guard_low: if guard { base - 0x1000 } else { 0 },
        guard_len: if guard { 0x1000 } else { 0 },
    };

    let mut thread = Thread::new_user_with_arg("bthread", pid, entry, user_rsp, arg, pmap.clone());
    #[cfg(feature = "obj-quota")]
    thread.set_obj_charge(charge);
    // Finding-2 anchor: the scheduler now owns an `Arc<Process>` for the life of
    // this thread, so a joiner resumed into the Ready→Running window cannot
    // outlive the registry / done-word it holds raw pointers to.
    thread.set_process_anchor(proc_arc.clone());
    let tid = thread.id.0;

    // Register BEFORE push: the child can run and exit immediately after the
    // push, and its exit path needs the record to exist. created=true: this
    // stack is unmapped at the thread's reap when the process survives (B).
    if proc_arc.threads.lock().insert(tid, bounds, true).is_err() {
        Thread::free(thread);
        // OT-93: a freed anchored thread re-checks a pending kill (as a reap).
        thread_reaped(proc_arc.pid);
        let _ = pmap.unmap_user_range(VirtAddr::new(base), rounded);
        return EAGAIN;
    }

    if let Err(thread) = place(thread) {
        proc_arc.threads.lock().remove_unstarted(tid);
        Thread::free(thread);
        thread_reaped(proc_arc.pid);
        let _ = pmap.unmap_user_range(VirtAddr::new(base), rounded);
        return EAGAIN;
    }
    tid as i64
}

/// OT-61 C1.1 (P2): `SYS_THREAD_EXIT` body. Publishes the exit status into
/// the caller's record, sets the frozen **done-word** to 1 (Release), and
/// wakes any joiner parked on the futex queue key `join_key(tid)` — the same
/// wait/wake mechanism the user futex uses (blocker F), NOT a private
/// join-token store. The wake happens OUTSIDE the registry lock (CP-16). The
/// PROCESS survives until the last thread's reap (`on_thread_reap_threaded`).
#[cfg(feature = "browser-runtime")]
pub fn thread_exit_current(code: i32) -> ! {
    let pid = scheduler::current_pid();
    let tid = crate::arch::x86_64::per_cpu::current_thread_id();
    if let Some(proc_arc) = get(pid) {
        {
            let mut reg = proc_arc.threads.lock();
            if let Some(rec) = reg.find(tid) {
                rec.exit_code = code;
                rec.done = true;
                // Release: a joiner that observes done_word==1 in its
                // under-guard re-check must also see exit_code (published
                // above). The registry lock already orders this store before
                // the wake, but the done-word is read lock-free by the joiner.
                rec.done_word.store(1, Ordering::Release);
            }
            // OT-98 (CP-98): the last thread out commits the process to its
            // natural exit here, as `record_exit_code` does for `SYS_EXIT`; a
            // kill that loses returns ESRCH. No thread can be added once
            // every one is done (a create needs a running thread).
            if reg.records.iter().all(|r| r.done || r.reaped) {
                let _ = proc_arc.exit_state.compare_exchange(
                    EXIT_STATE_RUNNING,
                    EXIT_STATE_EXITING_NATURAL,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
        }
        // Wake the joiner on the shared futex queue (join_key is derived from
        // tid; the joiner parks on the same key). At most one joiner exists
        // (single-joiner), but wake-all is harmless and keeps the path simple.
        let _ = crate::sched::futex::wake(crate::sched::futex::join_key(tid), u32::MAX);
        // CP-30: drop the owned Arc<Process> before the diverging exit() — the
        // faulting/exiting thread's stack is freed raw by the reaper, so an
        // owned Arc on it would leak.
        drop(proc_arc);
    }
    scheduler::exit();
}

/// OT-61 C1.1 (P2 / blocker F): `SYS_THREAD_JOIN` body. Single-joiner
/// (POSIX): a second joiner, a detached target, or self-join is `EINVAL`.
/// The delayed join blocks on the frozen **per-thread done-word** via the P1
/// futex — the SAME wait/wake mechanism the user futex uses, keyed by
/// `join_key(target_tid)`; there is no private join-token store. Returns the
/// ENCODED exit status (`WAIT_EXITED_TAG | code`) or a negative errno; the
/// syscall layer decodes and writes the user out-param.
#[cfg(feature = "browser-runtime")]
///
/// `deliver` receives the exit code under the registry lock, BEFORE the
/// record is collected (CP-99): `SYS_THREAD_JOIN` writes the caller's status
/// word there. If it fails (EFAULT), its error is returned and the record is
/// left as it was, still done and joinable, so a faulting status pointer
/// commits nothing.
pub fn thread_join_for_current(
    target_tid: u64,
    deliver: impl FnOnce(i32) -> Result<(), i64>,
) -> i64 {
    use crate::errno::{EINVAL, ESRCH};

    let pid = scheduler::current_pid();
    let self_tid = crate::arch::x86_64::per_cpu::current_thread_id();
    if target_tid == self_tid {
        return EINVAL; // self-join deadlock
    }
    // Capture a raw pointer to the registry lock and DROP the owned
    // Arc<Process> BEFORE any block (CP-30): a thread force-zombified while
    // parked in join has its stack freed raw (no Drop), so an owned
    // Arc<Process> on the stack would leak and strand the whole process +
    // Pmap. The scheduler-owned process anchor on THIS thread keeps the
    // Process and its registry alive for the whole blocked-to-running window.
    let reg_mutex: *const IrqSafeMutex<ThreadRegistry> = {
        let Some(proc_arc) = get(pid) else {
            return EINVAL;
        };
        &proc_arc.threads as *const _
        // proc_arc Arc dropped here; PROCESSES keeps the Process alive.
    };
    // SAFETY: the registry outlives this blocked thread (see above).
    let reg_lock = unsafe { &*reg_mutex };

    // Phase 1: validate + claim under the registry lock, and capture a raw
    // pointer to the target's done-word (a heap-stable Box, alive as long as
    // the record — join_claimed keeps a joinable record from being dropped at
    // reap). No owned resource is retained across the block below.
    let done_word_ptr: *const AtomicU32 = {
        let mut reg = reg_lock.lock();
        let Some(pos) = reg.records.iter().position(|r| r.tid == target_tid) else {
            return ESRCH; // never existed, or already collected
        };
        let rec = &reg.records[pos];
        if rec.detached || rec.join_claimed {
            return EINVAL;
        }
        if rec.done {
            // Already exited: collect the status now (no need to park). Drop
            // the record only if the reaper already ran (stack unmapped);
            // otherwise mark it collected and let the reaper unmap+remove — so
            // the created-stack unmap (blocker B) still happens exactly once.
            let code = rec.exit_code;
            let reaped = rec.reaped;
            if let Err(e) = deliver(code) {
                return e;
            }
            if reaped {
                reg.records.remove(pos);
            } else {
                reg.records[pos].collected = true;
            }
            return encode_wait_exit_code(code);
        }
        reg.records[pos].join_claimed = true;
        &*reg.records[pos].done_word as *const AtomicU32
        // registry lock released here.
    };

    // Phase 2: park on the futex done-word until the target sets it (blocker
    // F). `wait_on_key` re-checks `done_word == 0` UNDER the futex queue guard
    // (the lost-wakeup discipline — a wake between the check and the park is
    // observed and refuses to park). Spurious returns just re-loop.
    //
    // ESRCH is terminal (Jarvis, C1.3b review finding 1): `wait_on_key` refuses
    // to park a thread of a process being retired, and nobody will set the
    // done-word of a process that exits by SYS_EXIT. Retrying would spin here
    // in ring 0 with IF=0 forever, never reaching the syscall-return check that
    // retires this thread. Return to that boundary instead.
    // SAFETY: `done_word_ptr` targets the Box owned by the (join_claimed,
    // hence retained) record; valid until this join collects it below.
    loop {
        if unsafe { (*done_word_ptr).load(Ordering::Acquire) } != 0 {
            break;
        }
        let r = crate::sched::futex::wait_on_key(
            crate::sched::futex::join_key(target_tid),
            || Ok(unsafe { (*done_word_ptr).load(Ordering::Acquire) } == 0),
            crate::sched::futex::DEADLINE_INFINITE,
        );
        if r == ESRCH {
            return ESRCH;
        }
    }

    // Phase 3: collect the exit status. Drop the record only if the reaper
    // already ran (created stack already unmapped); otherwise mark it collected
    // and let the reaper unmap+remove — the created-stack unmap (blocker B)
    // stays tied to the reap and happens exactly once. The record is still
    // present (join_claimed kept it).
    let mut reg = reg_lock.lock();
    if let Some(pos) = reg.records.iter().position(|r| r.tid == target_tid) {
        let code = reg.records[pos].exit_code;
        if let Err(e) = deliver(code) {
            // Un-claim: the record is done, so the next join collects it.
            reg.records[pos].join_claimed = false;
            return e;
        }
        if reg.records[pos].reaped {
            reg.records.remove(pos);
        } else {
            reg.records[pos].collected = true;
        }
        encode_wait_exit_code(code)
    } else {
        ESRCH
    }
}

/// OT-61 C1.1 (P2): `SYS_THREAD_DETACH` body. A detached thread's record is
/// dropped at reap with no status kept; joining it afterwards is refused.
#[cfg(feature = "browser-runtime")]
pub fn thread_detach_for_current(target_tid: u64) -> i64 {
    use crate::errno::{EINVAL, ESRCH};

    let pid = scheduler::current_pid();
    let Some(proc_arc) = get(pid) else {
        return EINVAL;
    };
    let mut reg = proc_arc.threads.lock();
    let Some(pos) = reg.records.iter().position(|r| r.tid == target_tid) else {
        return ESRCH;
    };
    if reg.records[pos].detached || reg.records[pos].join_claimed {
        return EINVAL;
    }
    if reg.records[pos].reaped {
        // Already exited + reaped: detaching a zombie record just discards it.
        reg.records.remove(pos);
    } else {
        reg.records[pos].detached = true;
    }
    0
}

/// OT-61 C1.1 (P2 amendment): `SYS_THREAD_GET_INFO` lookup — self
/// (`tid == 0` or the caller's own tid) or any same-process tid. Valid
/// through exit/reap: the record (and therefore the bounds) survives until
/// the reaper removes it.
#[cfg(feature = "browser-runtime")]
pub fn thread_get_info_for_current(target_tid: u64) -> Result<(u64, ThreadStackBounds), i64> {
    use crate::errno::{EINVAL, ESRCH};

    let pid = scheduler::current_pid();
    let self_tid = crate::arch::x86_64::per_cpu::current_thread_id();
    let tid = if target_tid == 0 {
        self_tid
    } else {
        target_tid
    };
    let Some(proc_arc) = get(pid) else {
        return Err(EINVAL);
    };
    let mut reg = proc_arc.threads.lock();
    match reg.find(tid) {
        Some(rec) => Ok((rec.tid, rec.stack)),
        None => Err(ESRCH),
    }
}

/// OT-61 C1.1 (P2): `SYS_THREAD_SET_NAME` — stores the (truncated) name in
/// the caller's own registry record. Purely observability.
#[cfg(feature = "browser-runtime")]
pub fn thread_set_name_for_current(name: &[u8]) -> i64 {
    use crate::errno::ESRCH;

    let pid = scheduler::current_pid();
    let self_tid = crate::arch::x86_64::per_cpu::current_thread_id();
    let Some(proc_arc) = get(pid) else {
        return ESRCH;
    };
    let mut reg = proc_arc.threads.lock();
    let Some(rec) = reg.find(self_tid) else {
        return ESRCH;
    };
    let n = core::cmp::min(name.len(), THREAD_NAME_CAP);
    rec.name[..n].copy_from_slice(&name[..n]);
    rec.name_len = n;
    0
}

/// OT-61 C1.1: exit code delivered to `sys_wait`ers of a page-fault-killed
/// process — 128 + SIGSEGV(11), the conventional "killed by segfault" code.
pub const FAULT_KILL_EXIT_CODE: i32 = crate::user_fault_logic::Vector::PageFault.exit_code();

/// OT-61 C1.1 (I-14b): a ring-3 page fault (e.g. a thread stack overflowing
/// into its guard page) kills the FAULTING PROCESS ONLY. Exit code 139. See
/// [`fault_kill_current_user_exception`], which every ring-3 exception uses.
pub fn fault_kill_current_user(cr2: u64, rip: u64) -> ! {
    fault_kill_current_user_exception(crate::user_fault_logic::Vector::PageFault, rip, cr2)
}

/// OT-61 C1.1 / OT-83 (I-14b): a ring-3 CPU exception kills the FAULTING
/// PROCESS ONLY — the fault degrades within the process; the kernel, the
/// scheduler, and every governance invariant stay live. The process exits with
/// 128 + the signal for `vector` (`user_fault_logic`): #UD 132, #DE/#MF/#XM/#NM
/// 136, #SS/#NP/#AC 135, #GP/#PF/#BR/#OF 139. `detail` is the fault address
/// (CR2) for #PF and the error code for error-code vectors (0 otherwise).
/// Never returns.
///
/// Unconditional (OT-83): before it, only a ring-3 #PF under `browser-runtime`
/// was contained, and every other ring-3 exception — and every ring-3 #PF in a
/// build without `browser-runtime` — halted the whole machine. Without
/// `browser-runtime` a process has exactly one thread (`SYS_THREAD_CREATE` is
/// absent), so the sibling drain below is a no-op there and the normal reap of
/// this thread runs the full `on_thread_reap` teardown.
///
/// Runs in the EXCEPTION context (IF=0, on the faulting thread's kernel stack,
/// GS already swapped to the kernel base by the caller) — a fragile place to
/// run the full process-teardown machinery. So it does the MINIMUM here:
/// record the exit code, purge the futex queue, and reap every SIBLING
/// thread (Ready + token-parked) so the process refcount can reach zero;
/// then `scheduler::exit()` retires the faulting thread. The heavy teardown
/// (handle drain, channel-peer EPIPE, waiter completion delivering the exit
/// code, `PROCESSES` deregistration, `Pmap` drop) then runs on the LAST
/// thread's reap through the EXACT same `on_thread_reap` path natural exit
/// uses — from the reaper's clean kernel context, not the exception frame.
///
/// The faulting thread was in ring 3, so it holds no kernel lock and is in no
/// kill-protected kernel section (CP-63/CP-65/CP-66/CP-67 are kernel-mode
/// states); only its siblings can be, and those are handled by the same
/// guarded scans every kill uses.
///
/// Allocation-free. No log-sink wait and no block I/O (OT-83 F1, CP-71): every
/// line logged here, the admission withdrawal traces included, is staged or
/// dropped (counted) in a stage-only section, and the reaper flushes it. Locks
/// taken: the futex queue (and, with
/// `wait-multiplex`, the wait-any table), the admission `QUEUES` for CP-66's
/// `admission::cancel_pid` (then the scheduler lock when it wakes a withdrawn
/// caller — `QUEUES → scheduler`, the order `complete` uses; this exception
/// path cannot already hold `QUEUES`), and the per-CPU scheduler locks. No
/// other AI-governance lock (`INFLIGHT`, `RECENCY`, `ARENA`, `TABLE`, the
/// reservation) is taken.
pub fn fault_kill_current_user_exception(
    vector: crate::user_fault_logic::Vector,
    rip: u64,
    detail: u64,
) -> ! {
    let pid = scheduler::current_pid();
    if pid == KERNEL_PID {
        // A CPL-3 fault cannot belong to the kernel process; this is a kernel
        // bug, not a containable user fault — fail loud.
        panic!(
            "fault_kill_current_user: ring-3 {} attributed to KERNEL_PID",
            vector.name()
        );
    }
    let exit_code = vector.exit_code();
    // OT-83 F1 (CP-71): nothing on this IF=0 path may wait for the log sink or
    // do block I/O — a `SINK` holder preempted on this CPU could never run to
    // release it. Every line logged from here to just before `scheduler::exit()`
    // is staged or dropped (counted); the reaper writes it out from normal
    // context.
    let stage_only = crate::log::enter_stage_only();
    serial_println!(
        "[ kernel ] proc: ring-3 {} pid={} rip=0x{:016x} detail=0x{:x} — fault-killing process, exit {} (I-14b)",
        vector.name(),
        pid.0,
        rip,
        detail,
        exit_code
    );

    retire_process_from_current_thread(pid, exit_code, stage_only)
}

/// OT-98 discriminator (Janus #174 F1): the fault kill's tail, from a
/// kernel-built stand-in thread, so a fault exit can race a `SYS_EXIT` of the
/// same process. With interrupts off, as in the exception frame.
#[cfg(feature = "neo-test-kill-at-exit")]
pub fn fault_exit_current_for_test(exit_code: i32) -> ! {
    crate::arch::x86_64::disable_interrupts();
    let pid = scheduler::current_pid();
    let stage_only = crate::log::enter_stage_only();
    retire_process_from_current_thread(pid, exit_code, stage_only)
}

/// The shared tail of [`fault_kill_current_user_exception`] and
/// [`exit_group_current`]: record `exit_code` for `pid`'s waiters, mark the
/// process exiting (and fault-killing, so its reap drains IPC- and
/// wait-parked siblings), purge its futex/join queue, yank its Ready and
/// token-parked siblings, then retire the calling thread. The process's
/// heavy teardown runs on the LAST thread's reap. Runs with interrupts off.
fn retire_process_from_current_thread(pid: ProcessId, exit_code: i32, stage_only: bool) -> ! {
    // Deliver the exit code to any sys_waiter: on_thread_reap reads this and
    // completes waiters with it.
    record_exit_code(pid, exit_code);

    // Mark the process fault-killed so the REAPER (normal kernel context) does
    // the heavy sibling drain — walking the handle table / wait queues is
    // unsafe from this #PF exception frame (blocker C; the AAG's "heavy
    // teardown deferred to safe off-stack reap"). Also set `exiting` so no new
    // capability install lands mid-teardown.
    if let Some(proc_arc) = get(pid) {
        #[cfg(feature = "browser-runtime")]
        if !proc_arc.fault_killing.swap(true, Ordering::AcqRel) {
            // C1.3b: one more process mid-retire (the boundary checks' fast
            // path); `Process::drop` gives it back once every thread is freed.
            // The swap publishes the flag BEFORE this increment, so a
            // boundary in between can read 0 and skip its check. That is
            // safe: the increment comes before the drain and the IPIs below,
            // which reach any thread that skipped (CP-86).
            RETIRING.fetch_add(1, Ordering::AcqRel);
        }
        proc_arc.exiting.store(true, Ordering::Release);
    }

    // The MINIMUM safe in the exception frame: purge the futex/join queue (so a
    // racing wake cannot re-ready a sibling) and yank Ready + token-parked
    // siblings. IPC-parked and process-wait-parked siblings are drained by the
    // reaper once this thread reaps. No owned Arc is held across exit() (CP-30).
    #[cfg(feature = "browser-runtime")]
    crate::sched::futex::purge_pid(pid);
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::purge_pid(pid);
    // CP-66: siblings inside the accelerator admission section are cancelled
    // (they unwind and exit at the syscall boundary), never zombified raw.
    crate::ai::admission::cancel_pid(pid);
    loop {
        let killed = scheduler::kill_thread_by_pid(pid);
        let zombified = scheduler::zombify_parked_thread_by_pid(pid);
        if !killed && !zombified {
            break;
        }
    }
    // C1.3b (CP-86): a sibling Running on another CPU is in none of the
    // queues drained above. Interrupt every CPU that last switched in a
    // thread of `pid`; its CPL-3 return path retires the sibling. The scan
    // runs after the drain took each CPU's scheduler lock, and a CPU publishes
    // its pid under that lock before switching, so a sibling the drain did not
    // yank is visible here (or will cross a syscall boundary, which checks).
    #[cfg(feature = "browser-runtime")]
    kick_cpus_running(pid);

    crate::log::leave_stage_only(stage_only);

    // Retire the faulting thread. Its reap drives on_thread_reap_threaded,
    // which (seeing `fault_killing`) drains the remaining IPC/wait siblings
    // from the reaper's context; each drained sibling reaps in turn until the
    // LAST reap runs the full on_thread_reap teardown.
    scheduler::exit();
}

/// OT-61 C1.3: `SYS_EXIT` from a process that has other live threads ends the
/// WHOLE process with `code` (POSIX `exit`/`exit_group` semantics), through the
/// same sibling drain the ring-3 fault kill uses. Without it, `SYS_EXIT` on a
/// spawned thread retired only that thread: a std worker thread's
/// `process::exit` or panic-abort (`abort_internal` → `SYS_EXIT(134)`) left the
/// process alive, its joiner parked forever on a record that never completed.
/// Returns (and the caller retires the single thread as before) when the caller
/// is the process's only live thread, so a single-threaded process's exit path
/// is unchanged.
#[cfg(feature = "browser-runtime")]
pub fn exit_group_current(code: i32) {
    let pid = scheduler::current_pid();
    if pid == KERNEL_PID {
        return;
    }
    let threaded = get(pid).is_some_and(|proc_arc| proc_arc.threads.lock().live > 1);
    // The Arc is dropped before the no-return path below (CP-30). While the
    // caller is the only live thread nothing can add one, so `threaded` stays
    // false; once it is true, the drain below handles every sibling.
    if !threaded {
        return;
    }
    without_interrupts(|| {
        let stage_only = crate::log::enter_stage_only();
        serial_println!(
            "[ kernel ] proc: pid={} SYS_EXIT({}) with sibling threads — ending the process",
            pid.0,
            code
        );
        retire_process_from_current_thread(pid, code, stage_only)
    })
}

/// OT-61 C1.3b: processes mid-retire (the fault kill or the exit group has
/// begun and some thread of the process still exists). The ring-3 boundary
/// checks ([`retire_current_if_dying`]) read only this while it is 0.
#[cfg(feature = "browser-runtime")]
static RETIRING: AtomicU32 = AtomicU32::new(0);

/// `kill_ready_in_kernel` (OT-93): the handoff hold. The harness arms it; the
/// first rendezvous sender takes it and stays Ready inside its send until
/// `terminate` has killed a process; that kill records the victim's pmap.
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_HOLD_ARMED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_HOLD_TAKEN: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_HOLD_RELEASED: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILLED_PMAP: SwitchMutex<Option<alloc::sync::Weak<Pmap>>> = SwitchMutex::new(None);
/// OT-93 F2: the pid the kill ended; set once its last thread is reaped; and
/// whether a `sys_wait` on it returned only after that.
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_VICTIM: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_SURVIVOR_REAPED: AtomicBool = AtomicBool::new(false);
/// 0: no wait on the victim returned; 1: one returned after its last reap;
/// 2: one returned before it (the F2 defect).
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
static KILL_WAIT_ORDER: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "neo-test-kill-ready-in-kernel")]
fn kill_hold_note_wait(target: ProcessId) {
    let victim = KILL_VICTIM.load(Ordering::Acquire);
    if victim == 0 || target.0 != victim {
        return;
    }
    let order = if KILL_SURVIVOR_REAPED.load(Ordering::Acquire) {
        1
    } else {
        2
    };
    // The first wait to return decides; a later one can't hide an early one.
    let _ = KILL_WAIT_ORDER.compare_exchange(0, order, Ordering::AcqRel, Ordering::Acquire);
}

/// OT-93 F2: whether a `sys_wait` on the victim returned, and whether only
/// after its last thread was reaped (`(returned, after_reap)`).
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_wait_report() -> (bool, bool) {
    let order = KILL_WAIT_ORDER.load(Ordering::Acquire);
    (order != 0, order == 1)
}

#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_arm() {
    KILL_HOLD_ARMED.store(true, Ordering::Release);
}

/// Whether the running thread is the worker the hold is armed for.
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_waiting_worker() -> bool {
    KILL_HOLD_ARMED.load(Ordering::Acquire)
        && get(scheduler::current_pid()).is_some_and(|p| p.name == "worker")
}

/// The first sender of a process named "worker" after `kill_hold_arm` gets
/// `true` (init's own sends never take it).
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_take() -> bool {
    if !get(scheduler::current_pid()).is_some_and(|p| p.name == "worker") {
        return false;
    }
    let took = KILL_HOLD_ARMED.swap(false, Ordering::AcqRel);
    if took {
        KILL_HOLD_TAKEN.store(true, Ordering::Release);
    }
    took
}

#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_released() -> bool {
    KILL_HOLD_RELEASED.load(Ordering::Acquire)
}

/// Whether a sender took the hold, and whether the pmap of the process the
/// kill ended is gone (every `Arc<Pmap>`, the held send's included, dropped).
#[cfg(feature = "neo-test-kill-ready-in-kernel")]
pub fn kill_hold_report() -> (bool, bool) {
    let freed = without_interrupts(|| KILLED_PMAP.lock().as_ref().map(|w| w.strong_count() == 0));
    (KILL_HOLD_TAKEN.load(Ordering::Acquire), freed == Some(true))
}

/// OT-93: force-killed processes that still exist. Each of their threads
/// holds an `Arc<Process>` until `Thread::free`, so this stays above zero
/// while a thread the kill left running can still reach a boundary; the
/// ring-3 interrupt checks read only this while it is 0.
static FORCED_ALIVE: AtomicU32 = AtomicU32::new(0);

/// Whether some force-killed process still has a thread (the IRQ-path checks'
/// fast path).
pub fn forced_kill_in_flight() -> bool {
    FORCED_ALIVE.load(Ordering::Acquire) != 0
}

impl Process {
    /// OT-93: a force-kill has claimed this process.
    pub fn is_force_killed(&self) -> bool {
        self.force_killed.load(Ordering::Acquire)
    }

    /// OT-93: a thread installs its anchor on this process (`ProcessAnchor`).
    pub(super) fn anchor_thread(&self) {
        self.live_threads.fetch_add(1, Ordering::SeqCst);
    }

    /// OT-93: a thread's anchor is released (`Thread::free`). SeqCst, paired
    /// with `finish_kill`'s read: the reaper decrements, then takes `DYING`
    /// to re-check; `terminate` marks its teardown done under `DYING`, then
    /// reads. Whichever comes second sees both.
    pub(super) fn unanchor_thread(&self) {
        self.live_threads.fetch_sub(1, Ordering::SeqCst);
    }

    /// OT-93: threads still anchored to this process.
    pub fn live_threads(&self) -> usize {
        self.live_threads.load(Ordering::SeqCst)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // The last Arc goes when the last thread is freed (each thread holds
        // one until `Thread::free`), so the retire or the kill is over.
        #[cfg(feature = "browser-runtime")]
        if *self.fault_killing.get_mut() {
            RETIRING.fetch_sub(1, Ordering::AcqRel);
        }
        if *self.force_killed.get_mut() {
            FORCED_ALIVE.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// C1.3b review test-only: the thread armed by `SYS_TEST_TAIL_HOLD` and its
/// user flag.
#[cfg(feature = "neo-test-threads-exit-group")]
static TAIL_HOLD_TID: AtomicU64 = AtomicU64::new(u64::MAX);
#[cfg(feature = "neo-test-threads-exit-group")]
static TAIL_HOLD_FLAG: AtomicU64 = AtomicU64::new(0);

/// `SYS_TEST_TAIL_HOLD(flag)`: arm the caller, then return with interrupts
/// on, as `spawn_user` does (the syscall return's tail then runs with IF=1
/// unless it disables them itself).
#[cfg(feature = "neo-test-threads-exit-group")]
pub fn arm_tail_hold_for_test(flag: u64) -> i64 {
    TAIL_HOLD_FLAG.store(flag, Ordering::Release);
    TAIL_HOLD_TID.store(
        crate::arch::x86_64::per_cpu::current_thread_id(),
        Ordering::Release,
    );
    crate::arch::x86_64::enable_interrupts();
    0
}

/// For the armed thread only (once), after the syscall return's final retire
/// check: set the user flag (the parent's go), then wait, without touching
/// IF, until this process is being retired and 3 more ticks, so its retire
/// IPI lands here. Bounded at 1,000 ticks.
#[cfg(feature = "neo-test-threads-exit-group")]
pub fn tail_hold_for_test() {
    let me = crate::arch::x86_64::per_cpu::current_thread_id();
    if TAIL_HOLD_TID
        .compare_exchange(me, u64::MAX, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let flag = TAIL_HOLD_FLAG.load(Ordering::Acquire);
    // `flag` was validated as a 4-byte user range (and checked writable) when
    // armed, and this is the arming thread, so its address space is the
    // active one. The write goes through the fault-tolerant copy (CP-99): if
    // the page was unmapped since, nobody can see the go, so don't hold.
    if crate::arch::x86_64::syscall::write_user(flag as *mut u32, 1u32).is_err() {
        return;
    }
    let pid = scheduler::current_pid();
    let ticks = || crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
    let start = ticks();
    let mut seen: Option<u64> = None;
    loop {
        let now = ticks();
        if now.saturating_sub(start) > 1_000 {
            break;
        }
        match seen {
            Some(at) if now >= at + 3 => break,
            Some(_) => {}
            None => {
                if get(pid).is_some_and(|p| p.is_retiring()) {
                    seen = Some(now);
                }
            }
        }
        core::hint::spin_loop();
    }
}

/// OT-61 C1.3b (CP-86): send the reschedule IPI to every other CPU that last
/// switched in a thread of `pid`.
#[cfg(feature = "browser-runtime")]
fn kick_cpus_running(pid: ProcessId) {
    let me = crate::arch::x86_64::per_cpu::current_cpu_id() as usize;
    for cpu in 0..crate::arch::x86_64::per_cpu::MAX_CPUS {
        if cpu == me
            || crate::arch::x86_64::per_cpu::PER_CPU[cpu]
                .scheduler_ptr
                .load(Ordering::Acquire)
                .is_null()
        {
            continue;
        }
        if crate::arch::x86_64::per_cpu::current_pid_of(cpu) == u64::from(pid.0) {
            crate::arch::x86_64::apic::send_reschedule_ipi(cpu as u32);
        }
    }
}

/// OT-61 C1.3b (CP-86): if the calling user thread belongs to a process that
/// is being retired (a fault kill or an exit group has begun), retire it now.
/// Called only at ring-3 boundaries, where the thread holds no kernel lock and
/// no owned `Arc` (CP-63/CP-65/CP-67): syscall entry, syscall return, and the
/// timer and reschedule-IPI handlers when they interrupted CPL 3. One relaxed
/// load while no process is retiring.
#[cfg(feature = "browser-runtime")]
#[inline]
pub fn retire_current_if_dying() {
    if RETIRING.load(Ordering::Acquire) == 0 {
        return;
    }
    retire_current_if_dying_slow();
}

#[cfg(feature = "browser-runtime")]
#[inline(never)]
fn retire_current_if_dying_slow() {
    let pid = scheduler::current_pid();
    if pid == KERNEL_PID {
        return;
    }
    let dying = get(pid).is_some_and(|proc_arc| proc_arc.is_retiring());
    // The Arc is dropped before the no-return path (CP-30).
    if dying {
        // The process's exit code is already recorded; the reap of this
        // thread drains it like any other sibling.
        scheduler::exit();
    }
}

#[cfg(feature = "browser-runtime")]
impl Process {
    /// OT-61 C1.3b (CP-86): a fault kill or an exit group has begun for this
    /// process. Stored before the retire's futex purge, so the futex park,
    /// reading it under the same queue lock, either is purged or sees it.
    pub fn is_retiring(&self) -> bool {
        self.fault_killing.load(Ordering::Acquire)
    }
}

/// OT-61 C1.1 (blocker C): zombify every sibling of `pid` blocked in IPC on a
/// channel in this process's own handle table (send/recv/grant park the thread
/// on its own endpoint). Extracts the parked `Box<Thread>` only — it does NOT
/// close the channel or wake peers (that heavy teardown is deferred to
/// `on_thread_reap`, which drains the handles and EPIPEs the peers). Returns
/// the number extracted.
#[cfg(feature = "browser-runtime")]
fn zombify_ipc_parked_siblings(proc_arc: &Process, pid: ProcessId) -> usize {
    // Alloc-free under the handle lock (matching the scheduler's other locked
    // paths — this runs in the reaper's context, not the #PF frame), and PAGED
    // so the scan is complete: the handle table is an unbounded Vec, so a single
    // fixed snapshot could miss channels past the array, and a missed IPC-parked
    // sibling stalls fault teardown forever. Take a page under the lock, drop
    // the lock, extract + zombify, then resume where the page ended.
    const PAGE: usize = 32;
    without_interrupts(|| {
        let mut n = 0;
        let mut start = 0usize;
        loop {
            let mut endpoints: [Option<(Arc<Channel>, EndpointSide)>; PAGE] =
                core::array::from_fn(|_| None);
            let (count, next) = {
                let handles = proc_arc.handles.lock();
                let slots = handles.entry_slots();
                if start >= slots {
                    break;
                }
                handles.snapshot_channel_endpoints(start, &mut endpoints)
            };
            for slot in endpoints.iter_mut().take(count) {
                if let Some((channel, side)) = slot.take() {
                    while let Some(thread) = channel.extract_parked_if_owned(side, pid) {
                        scheduler::zombify_thread(thread);
                        n += 1;
                    }
                }
            }
            // `next` strictly advances (it is the entry index the page stopped
            // at, and a full page stops on an unconsumed channel entry), so this
            // terminates; the `<= start` guard is a belt-and-braces backstop.
            if next <= start {
                break;
            }
            start = next;
        }
        n
    })
}

/// OT-61 C1.1 (blocker C): zombify every sibling of `pid` blocked in `sys_wait`
/// on ANY process. A `sys_wait`er is parked in the TARGET process's `waiters`
/// queue (not its own), so this scans every registered process. Returns the
/// number extracted. OT-98: `terminate` uses it too, in every build, for a
/// killed process's thread parked in `sys_wait`; the waiter parks owning
/// nothing (`wait_for_exit_abi`), so the raw free leaks nothing (CP-30).
fn zombify_wait_parked_siblings(pid: ProcessId) -> usize {
    // Alloc-free under PROCESSES.lock() (matching the scheduler's other locked
    // paths — this runs in the reaper's context, not the #PF frame), and PAGED
    // BY KEY so the scan is complete. PROCESSES is an unbounded BTreeMap, so a
    // single fixed-size snapshot could miss a process, and a missed wait-parked
    // sibling stalls fault teardown forever. Paging by *key* rather than by
    // index is what makes it robust: teardown deregisters processes while this
    // runs, which would shift index-based pages and skip an entry, whereas
    // "the next pids strictly greater than the last one seen" is stable under
    // concurrent insertion and removal.
    // OT-98: only the registry read runs with interrupts off; the `waiters`
    // lock masks them itself. `terminate` now runs this on any CPU, so a
    // page's clone can be the last `Arc` (its process torn down meanwhile on
    // another CPU), and its `Pmap`'s shootdown must not run with interrupts
    // off (CP-91). Running the scan with them on is not enough: `SYS_KILL`
    // calls `terminate` inside `without_interrupts` (Janus #174 F3). So each
    // clone goes through `release_scan_pin`, which hands a last one to the
    // reaper, as `CapGraph`'s pins do.
    //
    // OT-98 (CP-98, FAIL-A): the waiter's target may be mid-`terminate`,
    // moved from `PROCESSES` to `DYING` (in one section, so it is always in
    // one of them), so both are scanned. The control
    // (`neo-test-kill-at-exit-control-a`) scans `PROCESSES` only, as before.
    const PAGE: usize = 64;
    let mut n = 0;
    let scan_dying = !cfg!(feature = "neo-test-kill-at-exit-control-a");
    for dying in [false, true] {
        if dying && !scan_dying {
            break;
        }
        let mut cursor: Option<ProcessId> = None;
        loop {
            let mut procs: [Option<Arc<Process>>; PAGE] = core::array::from_fn(|_| None);
            let (count, last) = without_interrupts(|| {
                if dying {
                    fill_process_page(&DYING.lock(), cursor, &mut procs, |d| d.process.clone())
                } else {
                    fill_process_page(&PROCESSES.lock(), cursor, &mut procs, Arc::clone)
                }
            });
            if count == 0 {
                break;
            }
            for slot in procs.iter_mut().take(count) {
                if let Some(proc_arc) = slot.take() {
                    while let Some(thread) = proc_arc.waiters.lock().extract_by_pid(pid) {
                        scheduler::zombify_thread(thread);
                        n += 1;
                    }
                    // CP-91 (Janus #174 F3): never the drop of a last `Arc`
                    // here, whatever the caller's interrupt state.
                    release_scan_pin(proc_arc);
                }
            }
            // A short page means the map is exhausted; otherwise resume
            // strictly after the last pid seen.
            if count < PAGE {
                break;
            }
            match last {
                Some(k) => cursor = Some(k),
                None => break,
            }
        }
    }
    n
}

/// One page of `table` strictly after `cursor`, into `procs`: how many, and
/// the last pid. Alloc-free (run under the table's lock).
fn fill_process_page<V, const N: usize>(
    table: &BTreeMap<ProcessId, V>,
    cursor: Option<ProcessId>,
    procs: &mut [Option<Arc<Process>>; N],
    process: impl Fn(&V) -> Arc<Process>,
) -> (usize, Option<ProcessId>) {
    let iter: &mut dyn Iterator<Item = (&ProcessId, &V)> = match cursor {
        None => &mut table.iter(),
        Some(after) => &mut table.range((
            core::ops::Bound::Excluded(after),
            core::ops::Bound::Unbounded,
        )),
    };
    let mut count = 0;
    let mut last = None;
    for (k, v) in iter {
        if count >= N {
            break;
        }
        procs[count] = Some(process(v));
        last = Some(*k);
        count += 1;
    }
    (count, last)
}

/// Forced-kill path triggered by `sys_kill`.
///
/// Per ADR-007 §Q3 cleanup discipline:
///   1. Claim forced-exit ownership with `exit_state`, then yank the target's
///      thread from the scheduler ready queue when it is Ready. If the thread
///      is Blocked in IPC, the drained handle table is walked below to extract
///      and zombify its parked `Box<Thread>`.
///   2. Remove the `Process` Arc from `PROCESSES` BEFORE any handle drops
///      — see ADR-007 §Implementation note 3 and the load-bearing comment
///      on `on_thread_reap` above for the BUILD-3 Chesterton's Fence.
///   3. Drain the handle table and signal channel peers via the shared
///      `close_channel_peers` helper (same path `on_thread_reap` uses).
///   4. Drop the `Process` Arc, triggering `Pmap::Drop` and the per-
///      process frame walker.
///
/// Returns `0` on success, negative errno on failure. Caller (the
/// `sys_kill` syscall body) is responsible for the self-kill check.
pub fn terminate(target_pid: ProcessId) -> i64 {
    use crate::errno::{EINVAL, ESRCH};
    if target_pid == KERNEL_PID {
        return EINVAL; // refuse to kill the implicit kernel "process"
    }

    let proc_for_claim = match without_interrupts(|| PROCESSES.lock().get(&target_pid).cloned()) {
        Some(proc_arc) => proc_arc,
        None => return ESRCH,
    };
    // CP-91 (Janus #174 F3): every early return below lets go of this clone
    // through `release_last_process`; the process can be reaped meanwhile on
    // another CPU, which makes it the last `Arc`, and `SYS_KILL` has
    // interrupts off.
    if proc_for_claim.exit_state.load(Ordering::Acquire) != EXIT_STATE_RUNNING {
        release_last_process(proc_for_claim);
        return ESRCH;
    }
    if proc_for_claim
        .exit_state
        .compare_exchange(
            EXIT_STATE_RUNNING,
            EXIT_STATE_EXITING_FORCED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        release_last_process(proc_for_claim);
        return ESRCH;
    }
    proc_for_claim.exiting.store(true, Ordering::Release);
    // OT-93: published before the scans below. A thread the scans leave
    // (Ready inside a syscall, or running) reads it at its next boundary.
    if !proc_for_claim.force_killed.swap(true, Ordering::AcqRel) {
        FORCED_ALIVE.fetch_add(1, Ordering::AcqRel);
    }
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    KILL_VICTIM.store(target_pid.0, Ordering::Release);

    // OT-22: drop the target's SYS_WAIT_ANY entries first, so a racing
    // channel fire or tick cannot re-ready a thread being torn down; a
    // thread parked there is token-parked and zombified below.
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::purge_pid(target_pid);

    // CP-66 (J3-P): a thread inside the accelerator admission section (parked
    // behind a holder, or a holder preempted mid-dispatch) owns the request's
    // `Arc`s on its stack. Withdraw its request and wake it with the cancel
    // verdict; the scans below skip it, it unwinds and exits at the syscall
    // boundary. Counted as handled.
    let in_admission = crate::ai::admission::cancel_pid(target_pid);

    #[cfg(not(feature = "browser-runtime"))]
    let kill_result = scheduler::kill_thread_by_pid(target_pid);
    // OT-61 C1.1: a multi-threaded target needs EVERY Ready and token-parked
    // thread yanked, not just one. Purge the futex queue first so a racing
    // futex_wake cannot re-ready a sibling mid-yank.
    #[cfg(feature = "browser-runtime")]
    let kill_result = {
        crate::sched::futex::purge_pid(target_pid);
        let mut any = false;
        loop {
            let killed = scheduler::kill_thread_by_pid(target_pid);
            let zombified = scheduler::zombify_parked_thread_by_pid(target_pid);
            if killed || zombified {
                any = true;
            } else {
                break;
            }
        }
        any
    };
    // OT-93 (CP-95): the scans above zombify only a Ready thread that resumes
    // in ring 3; one inside a syscall, or running, is left to exit at its
    // boundary when it reads `force_killed`. Interrupt the CPUs running one:
    // the IPI ends a ring-3 runner, and a ring-0 one exits at its syscall
    // return (flag stored above, before this scan; the return checks with
    // interrupts off, CP-93).
    let survivors = scheduler::kick_kill_survivors(target_pid);
    // OT-98 leg C's control (`neo-test-kill-at-exit-control-c`): no survivor
    // count, as before OT-93, so a target running on another CPU panics.
    let survivors = if cfg!(feature = "neo-test-kill-at-exit-control-c") {
        0
    } else {
        survivors
    };
    // OT-93 F2 (Janus #149): while a thread of the target is still inside the
    // kernel (a survivor; an admission caller woken to cancel; an inference
    // participant, which exits itself), its syscall can still take effect. So
    // the waiters complete only once no thread of it is left
    // (`Process::live_threads`, at its last reap) and the teardown below is
    // done (`finish_kill`), not on what these scans saw: a survivor can be
    // reaped before the process reaches `DYING` (the PM's #204 bounce, item 1).
    // OT-98 leg E: the window between this count and the `DYING` insert.
    #[cfg(feature = "neo-test-kill-at-exit")]
    crate::test_harness::kae_after_survivor_count(target_pid);

    #[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
    debug_pause_for_concurrent_reap_after_terminate(target_pid);

    // Registry-removal-FIRST per ADR-007 §Implementation note 3: any
    // `close_peer` invocation below (or any handle-drop ripple at scope
    // end) must not be able to find this pid in `PROCESSES`. The
    // debug_assert below tightens the invariant statically.
    // K1a-0 (CP-88): unregister and retire the edges in one CAP_GRAPH
    // section; a revoke never sees the process gone with its edges unretired.
    // OT-93 F2: the process moves to `DYING` under the same `PROCESSES` lock,
    // so no wait or reap can find it in neither.
    let (proc_arc, mut drained) = match without_interrupts(|| {
        let mut g = lock_cap_graph();
        let mut procs = PROCESSES.lock();
        let proc_arc = procs.remove(&target_pid)?;
        DYING.lock().insert(
            target_pid,
            DyingProcess {
                process: proc_arc.clone(),
                teardown_pending: true,
                #[cfg(feature = "neo-test-kill-at-exit-control-e")]
                reap_pending: survivors > 0,
            },
        );
        drop(procs);
        let drained = g.teardown(&proc_arc);
        Some((proc_arc, drained))
    }) {
        Some(a) => a,
        None => {
            // Race with natural exit between the checks. A waiter may have
            // parked on the claim (OT-93 F2); it must not wait forever.
            complete_process_waiters(&proc_for_claim, target_pid, None);
            proc_for_claim
                .exit_state
                .store(EXIT_STATE_EXITED, Ordering::Release);
            release_last_process(proc_for_claim);
            return ESRCH;
        }
    };
    drop(proc_for_claim);
    // OT-98 discriminator: the FAIL-A and #202-F2 window, between the
    // removal and the completion.
    #[cfg(feature = "neo-test-kill-at-exit")]
    crate::test_harness::kae_in_terminate(target_pid);
    debug_assert!(
        without_interrupts(|| PROCESSES.lock().get(&target_pid).is_none()),
        "killed Process must be unmapped from PROCESSES before HandleTable drop"
    );
    #[cfg(feature = "neo-test-cap-graph-revoke")]
    cap_graph_race::after_teardown();

    #[cfg(not(feature = "browser-runtime"))]
    if !kill_result && in_admission == 0 {
        let mut found_blocked = false;
        // INV-IPC-8/F-005: extraction from a parked channel side is pid-aware.
        // A granted handle can park a foreign process on one of the target's
        // sides; only a waiter owned by target_pid may be zombified here.
        for handle in drained.handles() {
            if let Handle::Channel { channel, side, .. } = handle
                && let Some(parked) = channel.extract_parked_if_owned(*side, target_pid)
            {
                scheduler::zombify_thread(parked);
                found_blocked = true;
                break;
            }
        }
        if !found_blocked && scheduler::zombify_parked_thread_by_pid(target_pid) {
            found_blocked = true;
        }
        // OT-98: parked in `sys_wait`, in the waited-on process's queue. The
        // control skips it, as before OT-98.
        #[cfg(not(feature = "neo-test-kill-at-exit-control"))]
        if !found_blocked && zombify_wait_parked_siblings(target_pid) > 0 {
            found_blocked = true;
        }
        // OT-98 (CP-98): a park that completed after the first survivor
        // count (its target's waiters were woken meanwhile) left the thread
        // Ready inside its syscall, in no queue the scans above read: count
        // again, and kick it. Such a thread exits at its boundary.
        let late = if found_blocked || cfg!(feature = "neo-test-kill-at-exit-control-c") {
            0
        } else {
            scheduler::kick_kill_survivors(target_pid)
        };
        // B1c AAG J3/J4: an inference participant (holder or waiter) is
        // skipped by the kill path and exits itself at the syscall boundary.
        #[cfg(feature = "inference-stack")]
        let found_blocked =
            found_blocked || proc_arc.inference_participants.load(Ordering::Acquire) > 0;
        // OT-98 (CP-98): the claim above succeeded, so the target had not
        // committed to exit (`record_exit_code` claims first). So its one
        // thread is Ready (zombified, or a survivor inside a syscall), running
        // (a survivor), parked in a channel, a token slot or a `sys_wait`
        // queue, an inference or admission participant, or (Janus #174 F2) it
        // was woken from `sys_wait` during the scans and has since left the
        // kernel. Or it is between two places none of these read (an IPC
        // handoff, the PM's bounce, item 3): the completion doesn't depend on
        // finding it (`finish_kill` counts threads), so that is a diagnostic,
        // not a panic.
        if !(found_blocked
            || survivors > 0
            || late > 0
            || killed_thread_left_kernel(target_pid, &proc_arc))
        {
            note_kill_unlocated(target_pid);
        }
        // OT-98 leg F: the scans and checks are done.
        #[cfg(feature = "neo-test-kill-at-exit")]
        crate::test_harness::kae_after_scans(target_pid);
    }
    // OT-61 C1.1: extract EVERY channel-parked thread the target owns (a
    // multi-threaded process can have several parked across its endpoints),
    // plus any token-parked stragglers that landed after the pre-drain yank.
    #[cfg(feature = "browser-runtime")]
    {
        let mut found_blocked = false;
        for handle in drained.handles() {
            if let Handle::Channel { channel, side, .. } = handle {
                while let Some(parked) = channel.extract_parked_if_owned(*side, target_pid) {
                    scheduler::zombify_thread(parked);
                    found_blocked = true;
                }
            }
        }
        while scheduler::zombify_parked_thread_by_pid(target_pid) {
            found_blocked = true;
        }
        // OT-98: threads parked in `sys_wait`, in other processes' queues.
        if zombify_wait_parked_siblings(target_pid) > 0 {
            found_blocked = true;
        }
        // OT-98 (CP-98): a park that completed after the first survivor count
        // left its thread Ready inside its syscall: count again, and kick it.
        let late = scheduler::kick_kill_survivors(target_pid);
        #[cfg(feature = "inference-stack")]
        let found_blocked =
            found_blocked || proc_arc.inference_participants.load(Ordering::Acquire) > 0;
        // As in the default arm: a diagnostic, not a panic.
        if !(kill_result
            || found_blocked
            || in_admission != 0
            || survivors > 0
            || late > 0
            || killed_thread_left_kernel(target_pid, &proc_arc))
        {
            note_kill_unlocated(target_pid);
        }
        // OT-98 leg F: the scans and checks are done.
        #[cfg(feature = "neo-test-kill-at-exit")]
        crate::test_harness::kae_after_scans(target_pid);
    }
    // OT-22 (AAG F1): a waiter that re-registered between the purge above
    // and its zombify left an entry behind; drop it now that no thread of
    // the target can run again.
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::purge_pid(target_pid);
    close_channel_peers(drained.handles(), target_pid);
    drained.close_net();
    // The handles are closed and the sockets released: the kill completes
    // here, or at its last thread's reap if that is still pending.
    let name = proc_arc.name;
    let pid_for_log = target_pid.0;
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    {
        if let Some(pmap) = proc_arc.pmap.as_ref() {
            *KILLED_PMAP.lock() = Some(Arc::downgrade(pmap));
        }
    }
    // Explicit drops release the Process-owned references before the audit.
    // The killed Thread also carries an Arc<Pmap>; that final reference is
    // released when the zombie queue is reaped, so post-kill can still show
    // in-flight frames while final-post-reap returns to baseline.
    drop(drained);
    drop(proc_arc);
    finish_kill(target_pid, KillStep::TeardownDone);
    // K1a-0 F1: processes a CAP_GRAPH scan held last, dropped here too, unless
    // `SYS_KILL` has interrupts off (then the reaper's next drain, CP-91).
    drop_deferred_processes_if_interruptible();

    serial_println!(
        "[ kernel ] proc: terminated '{}' (pid={}) via sys_kill",
        name,
        pid_for_log
    );
    crate::mm::frame_audit("post-kill");
    #[cfg(feature = "neo-test-kill-ready-in-kernel")]
    KILL_HOLD_RELEASED.store(true, Ordering::Release);

    0
}

#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
static F004_HOOK_TARGET_PID: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
static F004_HOOK_STAGE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
static F004_HOOK_RACER_DONE: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
pub fn debug_arm_concurrent_reap_after_terminate(pid: ProcessId) {
    F004_HOOK_TARGET_PID.store(pid.0, Ordering::Release);
    F004_HOOK_STAGE.store(0, Ordering::Release);
    F004_HOOK_RACER_DONE.store(0, Ordering::Release);
}

#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
pub fn debug_concurrent_reap_after_terminate_stage() -> u32 {
    F004_HOOK_STAGE.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
pub fn debug_mark_concurrent_reap_after_terminate_racer_done() {
    F004_HOOK_RACER_DONE.store(1, Ordering::Release);
}

#[cfg(feature = "neo-test-smp-concurrent-reap-after-terminate")]
fn debug_pause_for_concurrent_reap_after_terminate(pid: ProcessId) {
    if F004_HOOK_TARGET_PID.load(Ordering::Acquire) != pid.0 {
        return;
    }

    F004_HOOK_STAGE.store(1, Ordering::Release);
    for _ in 0..100_000 {
        if F004_HOOK_RACER_DONE.load(Ordering::Acquire) == 1 {
            break;
        }
        scheduler::yield_now();
    }
    F004_HOOK_STAGE.store(2, Ordering::Release);
}

// PHASE-4-FOLLOWUP: cap-passing-over-IPC required to construct this case.
// Once a process can receive a `Handle::Process` referring to *itself*
// (via Phase 4 capability-grant IPC), the self-kill EINVAL guard in
// `sys_kill_impl` becomes runtime-reachable and needs an actual test —
// not just code review. The test would: (1) spawn child with parent's
// own ProcessHandle inherited, (2) child calls sys_kill on it, (3)
// verify -EINVAL and that parent is still alive. Cannot be expressed
// in Phase 1.5b because `sys_spawn` only lets the parent install
// handles in the CHILD's table, not handles to itself. The guard at
// `sys_kill_impl` is correct today; this comment tags the moment the
// test becomes constructable.

/// Walk a freshly-drained handle vector and signal each channel-endpoint
/// peer per ADR-004 §"Process-exit cleanup". `Handle::Process` variants
/// need no per-handle exit cleanup — the registry entry for that pid
/// lives independently of the (now-dropped) handle, and Phase 1.5b does
/// not attempt to revoke handles outstanding in OTHER processes when a
/// target dies.
///
/// Shape is "give me the drained vec" rather than "give me the proc and
/// I'll drain" because the two callers (`on_thread_reap`, `terminate`)
/// have different lock-acquisition orderings around the drain (see the
/// load-bearing comment on `on_thread_reap` for the BUILD-3 lesson) —
/// the helper must not refight that battle.
fn close_channel_peers(drained: &[Handle], exiting_pid: ProcessId) {
    for handle in drained {
        if let Handle::Channel { channel, side, .. } = handle {
            channel.close_peer(*side, exiting_pid);
        }
    }
}

/// The kernel's CR3 (used when switching to a kernel thread from a user
/// thread): the PML4 frame CR3 held at `mm::init`, before any user process
/// existed.
///
/// CP-60: this used to be a `Lazy` over `Cr3::read`, captured on first call.
/// The first call is the first switch from a user-process thread to a kernel
/// thread, while that process's PML4 is still loaded. Every kernel thread then
/// ran on that PML4, and when the process died its freed PML4 was reused by
/// the next `Pmap::new`, which zeroed the live page table (triple fault) or
/// corrupted the buddy allocator. `token_park_teardown_churn` is the
/// discriminator; `neo-test-kernel-cr3-lazy-control` restores the old capture.
pub fn kernel_cr3() -> (PhysFrame, Cr3Flags) {
    #[cfg(feature = "neo-test-kernel-cr3-lazy-control")]
    {
        static LAZY: Lazy<(PhysFrame, Cr3Flags)> = Lazy::new(x86_64::registers::control::Cr3::read);
        return *LAZY;
    }
    #[cfg(not(feature = "neo-test-kernel-cr3-lazy-control"))]
    crate::mm::kernel_cr3()
}
