// SPDX-License-Identifier: MPL-2.0
//! NeoPlutarch userland library.
//!
//! Phase 1.4 ships the syscall-wrapper layer: every NEO userspace program
//! links libneo and calls `sys_*` functions instead of issuing raw SYSCALL
//! instructions. Phase 2.5 will add the AI client API on top (`neo_infer`,
//! `neo_embed`, `neo_classify`, conversation handles, intent bus).
//!
//! Syscall ABI (mirror of `kernel/src/arch/x86_64/syscall.rs`):
//!   - SYSCALL instruction; kernel transitions to ring 0 via STAR/LSTAR.
//!   - RAX = syscall number
//!   - RDI / RSI / RDX / R10 / R8 / R9 = args 0..5 (R10 substitutes for RCX which the
//!     CPU clobbers with saved user RIP)
//!   - Return value in RAX (negative = errno; non-negative = success)
//!   - Clobbers per the SYSCALL instruction: RCX (saved RIP), R11 (saved
//!     RFLAGS). All other GP regs preserved by the kernel's syscall path.

#![no_std]
// The handler exists only when libneo owns the allocator; with the host-only
// `image-decode` feature std owns it, so gate the unstable feature too (an
// unused `feature` gate is a warning, and the close gate denies warnings).
#![cfg_attr(not(feature = "image-decode"), feature(alloc_error_handler))]

extern crate alloc;

// Lane 2b: the image-decode implementation is std-only, so its `image-decode`
// feature must never be enabled for the OS target. The re-export below is the
// public API; the compile_error keeps a mistaken OS build loud.
#[cfg(all(feature = "image-decode", target_os = "none"))]
compile_error!(
    "libneo's `image-decode` feature is host-only: the vendored `png` 0.18 decoder requires std"
);

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::ptr::{self, NonNull};

use linked_list_allocator::Heap;
use spin::Mutex;

pub mod action;
#[cfg(feature = "atrium-yield")]
pub mod atrium_client;
#[cfg(any(feature = "atrium-configure", test))]
pub mod atrium_configure;
pub mod atrium_desktop;
#[cfg(feature = "atrium-yield")]
pub mod atrium_handoff;
#[cfg(feature = "atrium-launch")]
pub mod atrium_launch;
/// PR #15 re-check: a cap lent over a client's channel by tries (pure).
pub mod atrium_lend;
pub mod atrium_protocol;
pub mod atrium_wm;
/// L7.1: the `SYS_BOOT_TIMES` record and the boot-time lines, one source
/// with the kernel's encoder.
#[path = "../../../kernel/src/boot_time_logic.rs"]
#[allow(dead_code)]
pub mod boot_time_logic;
/// OT-85: epoch → civil date/time, and its text (pure).
pub mod civil;
pub mod grant_store;
/// OT-61 C2.1: the `SYS_HANDLE_INFO` record (pure).
pub mod handle_info;
pub mod tz;
/// Lane 2b: host-only image decode (`png` + `zune-jpeg`), bytes in, RGBA out.
/// The implementation is the `neo-image-decode` crate; this re-export is the
/// libneo-level API the OS's future image consumers call.
#[cfg(feature = "image-decode")]
pub use neo_image_decode as image_decode;
/// OT-87 R4.4: the installer's disk list and run (kernel `installer`).
pub mod install;
/// OT-87 R4.4: the installer's ABI and text (pure).
pub mod install_abi;
/// OT-61 C2.2: the `SYS_MEM_INFO` record (pure).
pub mod mem_info;
pub mod nsync;
pub mod nthread;
pub mod ntls;
pub mod tls;
pub mod wait_abi;

getrandom::register_custom_getrandom!(neo_getrandom);

fn neo_getrandom(out: &mut [u8]) -> Result<(), getrandom::Error> {
    if out.is_empty() {
        return Ok(());
    }

    // Try kernel DRBG first (Phase 10.3). When the kernel was built without
    // `kernel-rng`, SYS_GETRANDOM returns ENOSYS (-38) and we fall back to
    // the direct RDRAND path so rustls and other getrandom consumers still
    // work in scenarios that don't enable the kernel RNG feature.
    let enosys: i64 = -38;
    let ret = unsafe { raw3(SYS_GETRANDOM, out.as_mut_ptr() as u64, out.len() as u64, 0) };
    if ret >= 0 {
        return Ok(());
    }
    if ret != enosys {
        return Err(getrandom::Error::UNSUPPORTED);
    }

    // Fallback: direct RDRAND (pre-10.3 path). Preserves rustls TLS
    // constructor for scenarios where kernel-rng is not enabled.
    for chunk in out.chunks_mut(core::mem::size_of::<u64>()) {
        let mut value = 0u64;
        let mut ok: u8 = 0;
        for _ in 0..64 {
            unsafe {
                asm!(
                    "rdrand {value}",
                    "setc {ok}",
                    value = out(reg) value,
                    ok = out(reg_byte) ok,
                    options(nomem, nostack)
                );
            }
            if ok != 0 {
                break;
            }
        }
        if ok == 0 {
            return Err(getrandom::Error::UNSUPPORTED);
        }
        let bytes = value.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    Ok(())
}

// === Syscall numbers (must match kernel/src/arch/x86_64/syscall.rs) ===
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_SPAWN: u64 = 2;
pub const SYS_CHAN_CREATE: u64 = 3;
pub const SYS_SEND: u64 = 4;
pub const SYS_RECV: u64 = 5;
pub const SYS_MAP_REGION: u64 = 6;
pub const SYS_KILL: u64 = 7;
pub const SYS_DERIVE: u64 = 8;
pub const SYS_REVOKE: u64 = 9;
pub const SYS_GRANT: u64 = 10;
pub const SYS_RECV_GRANT: u64 = 11;
pub const SYS_OPEN: u64 = 12;
pub const SYS_READ: u64 = 13;
pub const SYS_CLOSE: u64 = 14;
pub const SYS_FSYNC: u64 = 15;
pub const SYS_READDIR: u64 = 16;
pub const SYS_MOUNT: u64 = 17;
pub const SYS_UMOUNT: u64 = 18;
pub const SYS_WAIT: u64 = 19;
pub const SYS_INSPECT: u64 = 20;
pub const SYS_SYSSTAT: u64 = 21;
pub const SYS_OBSERVE: u64 = 22;
pub const SYS_FTRUNCATE: u64 = 23;
/// OT-67 W1: a file's whole content replaced in one commit (kernel feature
/// `neofs-file-replace`; without it, the number is unknown).
pub const SYS_FILE_REPLACE: u64 = 24;
// OT-61 browser C1 host-runtime block (0x40..0x4F). C1.0 = VM + clock.
pub const SYS_VM_MAP: u64 = 0x40;
pub const SYS_VM_UNMAP: u64 = 0x41;
pub const SYS_VM_PROTECT: u64 = 0x42;
pub const SYS_CLOCK_GETTIME: u64 = 0x43;
/// `rtc-offset` (THINK-2026-11-02-time-zones §2.1): confirm realtime as UTC.
pub const SYS_CLOCK_CONFIRM_REALTIME: u64 = 0x1C8;
/// `rtc-offset`: whether realtime is confirmed UTC.
pub const SYS_CLOCK_REALTIME_STATUS: u64 = 0x1C9;
/// POSIX-canonical protection bits for [`sys_vm_map`]/[`sys_vm_protect`].
pub const PROT_READ: u64 = 1;
pub const PROT_WRITE: u64 = 2;
pub const PROT_EXEC: u64 = 4;
/// Clock ids for [`sys_clock_gettime`].
pub const CLOCK_MONOTONIC: u64 = 1;
pub const CLOCK_REALTIME: u64 = 2;
// OT-61 C1.1 (P1+P2): futex + thread lifecycle; C1.2 (P3) TLS at 0x4F.
pub const SYS_FUTEX_WAIT: u64 = 0x44;
pub const SYS_FUTEX_WAKE: u64 = 0x45;
pub const SYS_THREAD_CREATE: u64 = 0x46;
pub const SYS_THREAD_EXIT: u64 = 0x47;
pub const SYS_THREAD_JOIN: u64 = 0x48;
pub const SYS_THREAD_DETACH: u64 = 0x49;
pub const SYS_SCHED_YIELD: u64 = 0x4A;
pub const SYS_THREAD_SELF: u64 = 0x4B;
pub const SYS_THREAD_SET_NAME: u64 = 0x4C;
pub const SYS_THREAD_GET_INFO: u64 = 0x4D;
/// C1.2 (P3): set the calling thread's FS.base. 0x4F, the reserved C1.2 slot
/// (the test-only queue-length probe keeps 0x4E).
pub const SYS_THREAD_SET_TLS: u64 = 0x4F;
/// OT-61 C2.1: the calling process's pid, from any of its threads
/// (`browser-runtime` kernels).
pub const SYS_PROCESS_SELF: u64 = 0x50;
/// OT-61 C2.1: `SYS_HANDLE_INFO(idx, out, 16)` writes one handle's kind and
/// rights as a [`handle_info`] record (`browser-runtime` kernels).
pub const SYS_HANDLE_INFO: u64 = 0x51;
/// OT-61 C2.2: `SYS_SPAWN_BUDGETED(name, len, handles, n, budget_bytes)` is
/// `SYS_SPAWN` whose child gets its own memory account, nested under the
/// caller's (`mem-quota` kernels).
pub const SYS_SPAWN_BUDGETED: u64 = 0x52;
/// OT-61 C2.2: `SYS_MEM_INFO(out, 32)` writes the caller's memory account as
/// a [`mem_info`] record (`mem-quota` kernels).
pub const SYS_MEM_INFO: u64 = 0x53;
/// OT-61 C2.2b: `SYS_SPAWN_BUDGETED` with an object budget too.
pub const SYS_SPAWN_BUDGETED2: u64 = 0x54;
/// L7.1: `SYS_BOOT_TIMES(out, 80)` writes the [`boot_time_logic`] record
/// (`boot-time` kernels; ENOSYS otherwise).
pub const SYS_BOOT_TIMES: u64 = 0x55;
/// C1.2 test-only: `SYS_TEST_TLS(op, out)` — op 0 probes `{cpu, recorded
/// FS.base, live IA32_FS_BASE}`, op 1 parks the caller, op 2 wakes the parked
/// thread onto the caller's CPU (`threads_tls` kernels only). Must NOT ship.
#[cfg(feature = "neo-test")]
pub const SYS_TEST_TLS: u64 = 0xF04;
/// C1.2 test-only: `SYS_THREAD_CREATE` whose thread starts on a given CPU
/// (`threads_tls` kernels only). Must NOT ship in production.
#[cfg(feature = "neo-test")]
pub const SYS_TEST_THREAD_CREATE_ON: u64 = 0xF05;
/// C1.3b test-only: arm the calling thread's next `SYS_FUTEX_WAIT` to park only
/// after its process is being retired (`threads_exit_group` kernels only).
#[cfg(feature = "neo-test")]
pub const SYS_TEST_FUTEX_LATE_PARK: u64 = 0xF06;
/// C1.3b test-only: spin in ring 0 for about `ms` milliseconds
/// (`threads_exit_group` kernels only).
#[cfg(feature = "neo-test")]
pub const SYS_TEST_KERNEL_SPIN: u64 = 0xF07;
/// C1.3b review test-only: see [`sys_test_tail_hold`].
#[cfg(feature = "neo-test")]
pub const SYS_TEST_TAIL_HOLD: u64 = 0xF09;
/// C1.1 test-only: queue-length probe. Must NOT ship in production.
#[cfg(feature = "neo-test")]
pub const SYS_FUTEX_QUEUE_LEN: u64 = 0x4E;
/// K1a-0 test-only: the `cap_graph_revoke` race hook. Must NOT ship.
#[cfg(feature = "neo-test")]
pub const SYS_TEST_CAP_GRAPH_RACE: u64 = 0xF08;
/// CP-41 test-only: kernel float probe (`syscall_mxcsr_entry` kernels only).
/// Must NOT ship in production.
#[cfg(feature = "neo-test")]
pub const SYS_TEST_FLOAT_PROBE: u64 = 0xF00;
/// OT-22 test-only (kernel `neo-test-desktop-idle`; ENOSYS elsewhere): the
/// ticks the BSP has spent in its idle thread since boot.
pub const SYS_TEST_IDLE_TICKS: u64 = 0xF03;
/// OT-66 L1 test-only (kernel `neo-test-login-cut`; ENOSYS elsewhere).
pub const SYS_TEST_BLOCK_CUT: u64 = 0xF0A;
/// `sys_futex_wait` deadline sentinels (ABSOLUTE monotonic ns — the C1.0
/// clock's timebase; `pthread_cond_timedwait` semantics, never relative).
pub const FUTEX_DEADLINE_INFINITE: u64 = u64::MAX;
pub const FUTEX_DEADLINE_EXPIRED: u64 = 0;
pub const SYS_TENSOR_CREATE: u64 = 0x100;
pub const SYS_TENSOR_QUERY: u64 = 0x101;
pub const SYS_TENSOR_SHARE: u64 = 0x102;
pub const SYS_TENSOR_DROP: u64 = 0x103;
pub const SYS_MODEL_LOAD: u64 = 0x110;
pub const SYS_MODEL_QUERY: u64 = 0x111;
pub const SYS_MODEL_DROP: u64 = 0x112;
pub const SYS_INFER_RUN: u64 = 0x120;
pub const SYS_GENERATE: u64 = 0x121;
pub const SYS_GENERATE_CONSTRAINED: u64 = 0x122;
pub const SYS_GENERATE_MASKED: u64 = 0x123;
pub const SYS_SOCKET_CREATE: u64 = 0x180;
pub const SYS_SOCKET_BIND: u64 = 0x181;
pub const SYS_SOCKET_SEND: u64 = 0x182;
pub const SYS_SOCKET_RECV: u64 = 0x183;
pub const SYS_SOCKET_CLOSE: u64 = 0x184;
pub const SYS_NET_STATS: u64 = 0x185;
pub const SYS_SOCKET_SENDTO: u64 = 0x186;
pub const SYS_SOCKET_RECVFROM: u64 = 0x187;
pub const SYS_NET_CONNECT_TCP: u64 = 0x188;
pub const SYS_NET_LISTEN_TCP: u64 = 0x189;
pub const SYS_NET_ACCEPT_TCP: u64 = 0x18A;
pub const SYS_NET_SHUTDOWN_TCP: u64 = 0x18B;
pub const SYS_NET_RESOLVE: u64 = 0x18C;
pub const SYS_NET_DIAL: u64 = 0x18D;
pub const SYS_NET_TCP_ENABLE_TLS: u64 = 0x18E;
pub const SYS_NET_DIAL_CAP_DERIVE: u64 = 0x18F;
/// Phase 10.3: userland randomness via the kernel CSPRNG DRBG.
pub const SYS_GETRANDOM: u64 = 0x190;
// Atrium G1 compositor range (0x1A0..0x1AF) — mirror of the kernel numbers.
// ENOSYS unless the kernel was built with `atrium-compositor-g1` AND the
// runtime switch is on (the two-layer kill-switch).
pub const SYS_BUFFER_CREATE: u64 = 0x1A0;
pub const SYS_BUFFER_MAP: u64 = 0x1A1;
pub const SYS_BUFFER_QUERY: u64 = 0x1A2;
pub const SYS_BUFFER_COMMIT: u64 = 0x1A3;
pub const SYS_FB_PRESENT: u64 = 0x1A4;
pub const SYS_BUFFER_COMMIT_DAMAGE: u64 = 0x1AD;
pub const SYS_PRESENT_FENCE_WAIT: u64 = 0x1A5;
/// GPU P1 Slice 6: submit the one kernel-pinned glass blur manifest using a
/// READ GpuBufferCap, WRITE GpuBufferCap, and quota-bound GpuQueueCap.  There
/// is intentionally no create syscall for these scarce roots.
pub const SYS_GPU_COMPUTE_SUBMIT_BLUR: u64 = 0x1AE;
/// GPU P1 Slice 6: wait/query a GpuFenceCap.  It never implies a present.
pub const SYS_GPU_COMPUTE_FENCE_WAIT: u64 = 0x1AF;
pub const SYS_INPUT_FOCUS_CREATE: u64 = 0x1A6;
pub const SYS_INPUT_FOCUS_ACTIVATE: u64 = 0x1A7;
pub const SYS_INPUT_FOCUS_DEACTIVATE: u64 = 0x1A8;
pub const SYS_INPUT_FOCUS_POLL: u64 = 0x1A9;
pub const SYS_INPUT_FOCUS_RECV: u64 = 0x1AA;
pub const SYS_INPUT_FOCUS_MINT_ACTIVATOR: u64 = 0x1AB;
pub const SYS_INPUT_FOCUS_TEST_INJECT: u64 = 0x1AC;
/// Desktop S1: the scanout output size, for the GpuSubmit holder only
/// (kernel `desktop-session` builds; ENOSYS elsewhere).
pub const SYS_FB_OUTPUT_QUERY: u64 = 0x1B0;
/// DI-1a B-ref (kernel `atrium-pointer-forward`; ENOSYS elsewhere). Input-
/// system authority only: claim the pointer route, forward/drop a record,
/// take the SAK chords. `SYS_TRY_RECV` is `SYS_RECV` that returns EAGAIN
/// instead of parking.
pub const SYS_INPUT_POINTER_CLAIM: u64 = 0x1B1;
pub const SYS_INPUT_POINTER_FORWARD: u64 = 0x1B2;
pub const SYS_INPUT_SAK_TAKE: u64 = 0x1B3;
pub const SYS_TRY_RECV: u64 = 0x1B4;
/// OT-88 A0k: `SYS_SEND` / `SYS_GRANT` that complete only with the peer
/// already parked in its receive; EAGAIN otherwise (ENOSYS on a kernel
/// without `ipc-try-send`). They never park.
pub const SYS_TRY_SEND: u64 = 0x1B7;
pub const SYS_TRY_GRANT: u64 = 0x1B8;
pub const SYS_TRY_RECV_GRANT: u64 = 0x1B9;
/// OT-22 (kernel `wait-multiplex`; ENOSYS elsewhere): park until any of up
/// to [`WAIT_ITEMS_MAX`] items is ready or a monotonic deadline passes.
pub const SYS_WAIT_ANY: u64 = 0x1B5;
/// Chad #177 item 4: unmap a borrowed buffer mapping (`sys_buffer_unmap`).
pub const SYS_BUFFER_UNMAP: u64 = 0x1B6;
pub use wait_abi::{WAIT_ITEMS_MAX, WAIT_KIND_CHANNEL, WAIT_KIND_INPUT, WAIT_KIND_SAK, WaitItem};

pub const INPUT_EVENT_V1_LEN: usize = 8;
/// Input event kinds (ADR-074 + the DI-1a amendment).
pub const INPUT_EVENT_KEY: u8 = 1;
pub const INPUT_EVENT_POINTER: u8 = 2;
/// A forwarded pointer event for a client: `flags` = buttons, `code` =
/// phase, `x`/`y` = surface-local.
pub const INPUT_EVENT_POINTER_ABS: u8 = 3;
/// A kernel-minted pointer record for the WM: `flags` bits 0-4 = buttons,
/// bit 7 = RESYNC; `code` = the tag to forward; `x`/`y` = relative motion.
pub const INPUT_EVENT_POINTER_REC: u8 = 4;
pub const POINTER_RESYNC: u8 = 0x80;
/// Bit 6 of a record's `flags`: the route is spent (every tag used); reclaim
/// a new one with `sys_input_pointer_claim(true)`. Its tag is never valid.
pub const POINTER_ROUTE_EXPIRED: u8 = 0x40;
/// A9-C2, bit 5 of a record's `flags`: a wheel record. `x` is the wheel
/// (i16, positive away from the user); it moves nothing and carries the
/// current buttons. Forward it to the window under the pointer.
pub const POINTER_WHEEL: u8 = 0x20;
/// A9-C2: a forwarded wheel event for a client: `flags` = buttons, `code` =
/// the wheel (i16), `x`/`y` = where the pointer is on the surface.
pub const INPUT_EVENT_POINTER_WHEEL: u8 = 5;
/// SAK chord bits from [`sys_input_sak_take`].
pub const SAK_TAB: u8 = 1;
pub const SAK_SPACE: u8 = 2;
pub const SAK_ESCAPE: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputEventV1 {
    pub kind: u8,
    pub flags: u8,
    pub code: u16,
    pub x: i16,
    pub y: i16,
}

// === Errno constants (must match kernel/src/errno.rs) ===
pub const EPERM: i64 = -1;
pub const ENOENT: i64 = -2;
pub const ESRCH: i64 = -3;
pub const E2BIG: i64 = -7;
pub const EBADF: i64 = -9;
/// `-EIO` — the storage device failed the operation.
pub const EIO: i64 = -5;
pub const EACCES: i64 = -13;
pub const ENOMEM: i64 = -12;
pub const EAGAIN: i64 = -11;
/// `-ETIMEDOUT` — a bounded wait (present-fence) reached its deadline.
pub const ETIMEDOUT: i64 = -110;
pub const EFAULT: i64 = -14;
pub const EBUSY: i64 = -16;
pub const EEXIST: i64 = -17;
pub const ENOTDIR: i64 = -20;
pub const EISDIR: i64 = -21;
pub const EINVAL: i64 = -22;
pub const EPIPE: i64 = -32;
pub const ENOTCONN: i64 = -107;
pub const ENOSYS: i64 = -38;
pub const ENOTEMPTY: i64 = -39;
pub const ESTALE: i64 = -116;
/// `-EBADMSG` — a boot-module model failed its pinned-digest check (B1b).
pub const EBADMSG: i64 = -74;
/// `-EALREADY` — a one-shot operation was already done (a DI-1a pointer record
/// forwarded or dropped twice).
pub const EALREADY: i64 = -114;

pub const WAIT_FLAG_WNOHANG: u64 = 1 << 0;
pub const WAIT_EXITED_TAG: i64 = 1_i64 << 32;
pub const PROCESS_SNAPSHOT_V1_VERSION: u32 = 1;
pub const PROCESS_SNAPSHOT_NAME_LEN: usize = 32;
pub const SYSSTAT_V1_VERSION: u32 = 1;
pub const KERNEL_OBSERVABILITY_V1_VERSION: u32 = 3;
pub const KERNEL_OBSERVABILITY_MAX_CPUS: usize = 64;
pub const KERNEL_OBSERVABILITY_TRACE_EVENTS: usize = 16;
pub const TRACE_KIND_DISPATCH: u32 = 1;
pub const TRACE_KIND_PAGE_FAULT: u32 = 2;
pub const TRACE_KIND_SYSCALL: u32 = 3;
pub const PROCESS_STATE_RUNNING: u32 = 0;
pub const PROCESS_STATE_EXITED: u32 = 1;
pub const TLS_KEY_MATERIAL_LEN: usize = 88;
pub const TENSOR_MAX_RANK: usize = 8;
pub const TENSOR_DTYPE_F32: u32 = 0;
pub const TENSOR_DTYPE_F16: u32 = 1;
pub const TENSOR_DTYPE_BF16: u32 = 2;
pub const TENSOR_DTYPE_I32: u32 = 3;
pub const TENSOR_DTYPE_I8: u32 = 4;
pub const TENSOR_DTYPE_U8: u32 = 5;
pub const TENSOR_DTYPE_BOOL: u32 = 6;
pub const TENSOR_DEVICE_CPU: u32 = 0;
pub const MODEL_QUERY_VERSION: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ProcessSnapshotV1 {
    pub version: u32,
    pub state: u32,
    pub pid: u32,
    pub exit_code: i32,
    pub thread_count: u32,
    pub handle_count: u32,
    pub channel_handle_count: u32,
    /// Count of live handles conferring network authority (`NetDial`/`Socket`/
    /// `TcpSocket`/`TcpListener`). Zero proves the process holds no network cap
    /// — the direct I-14(a) handle-set witness (ADR-075 §2.4). Mirror of the
    /// kernel `ProcessSnapshotV1`; the layout must stay identical.
    pub network_handle_count: u32,
    pub name_len: u32,
    pub name: [u8; PROCESS_SNAPSHOT_NAME_LEN],
}

// Mirror of the kernel-side I-12 byte-oracle for the SYS_INSPECT result ABI.
// If this fails to compile, the userland and kernel layouts have diverged.
const _: () = {
    assert!(core::mem::size_of::<ProcessSnapshotV1>() == 68);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, network_handle_count) == 28);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, name_len) == 32);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, name) == 36);
};

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SysStatV1 {
    pub version: u32,
    pub _pad0: u32,
    pub process_count: u64,
    pub frames_used: u64,
    pub memory_used_bytes: u64,
    pub cores_online: u32,
    pub _pad1: u32,
    pub uptime_ticks: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct SchedulerStatsV1 {
    pub cpu_id: u32,
    pub _pad0: u32,
    pub context_switches: u64,
    pub dispatches: u64,
    pub ready_queue_depth: u64,
    pub run_ticks: u64,
    pub idle_ticks: u64,
    pub timer_preemptions: u64,
    pub voluntary_yields: u64,
    pub ready_queue_high_water: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TraceEventV1 {
    pub seq: u64,
    pub ticks: u64,
    pub kind: u32,
    pub cpu_id: u32,
    pub arg0: u64,
    pub arg1: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct KernelObservabilityV1 {
    pub version: u32,
    pub cpu_count: u32,
    pub trace_capacity: u32,
    pub trace_count: u32,
    pub heap_current_bytes: u64,
    pub heap_high_water_bytes: u64,
    pub heap_alloc_failures: u64,
    pub syscall_total: u64,
    pub syscall_ipc: u64,
    pub syscall_file: u64,
    pub syscall_ai: u64,
    pub syscall_net: u64,
    pub syscall_other: u64,
    pub frames_used: u64,
    pub buddy_free_pages: u64,
    pub buddy_largest_free_order: u32,
    pub _pad0: u32,
    pub sched: [SchedulerStatsV1; KERNEL_OBSERVABILITY_MAX_CPUS],
    pub trace: [TraceEventV1; KERNEL_OBSERVABILITY_TRACE_EVENTS],
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TensorCreateDesc {
    pub rank: u32,
    pub dtype: u32,
    pub mutable: u32,
    pub _pad: u32,
    pub dims: [u64; TENSOR_MAX_RANK],
    pub data_ptr: u64,
    pub data_len: u64,
}

impl TensorCreateDesc {
    pub fn new(
        rank: u32,
        dtype: u32,
        mutable: bool,
        dims: [u64; TENSOR_MAX_RANK],
        data: &[u8],
    ) -> Self {
        Self {
            rank,
            dtype,
            mutable: u32::from(mutable),
            _pad: 0,
            dims,
            data_ptr: data.as_ptr() as u64,
            data_len: data.len() as u64,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TensorQueryDesc {
    pub rank: u32,
    pub dtype: u32,
    pub device: u32,
    pub mutable: u32,
    pub refcount: u64,
    pub dims: [u64; TENSOR_MAX_RANK],
    pub byte_len: u64,
}

impl TensorQueryDesc {
    pub const fn empty() -> Self {
        Self {
            rank: 0,
            dtype: 0,
            device: 0,
            mutable: 0,
            refcount: 0,
            dims: [0; TENSOR_MAX_RANK],
            byte_len: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ModelQueryDesc {
    pub version: u32,
    pub tensor_count: u32,
    pub node_count: u32,
    pub input_count: u32,
    pub output_count: u32,
    pub value_count: u32,
    pub first_tensor_dtype: u32,
    pub first_tensor_rank: u32,
    pub first_tensor_dims: [u64; TENSOR_MAX_RANK],
    pub first_tensor_byte_len: u64,
}

impl ModelQueryDesc {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            tensor_count: 0,
            node_count: 0,
            input_count: 0,
            output_count: 0,
            value_count: 0,
            first_tensor_dtype: 0,
            first_tensor_rank: 0,
            first_tensor_dims: [0; TENSOR_MAX_RANK],
            first_tensor_byte_len: 0,
        }
    }
}

impl ProcessSnapshotV1 {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            state: 0,
            pid: 0,
            exit_code: 0,
            thread_count: 0,
            handle_count: 0,
            channel_handle_count: 0,
            network_handle_count: 0,
            name_len: 0,
            name: [0; PROCESS_SNAPSHOT_NAME_LEN],
        }
    }
}

impl SysStatV1 {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            _pad0: 0,
            process_count: 0,
            frames_used: 0,
            memory_used_bytes: 0,
            cores_online: 0,
            _pad1: 0,
            uptime_ticks: 0,
        }
    }
}

impl SchedulerStatsV1 {
    pub const fn empty() -> Self {
        Self {
            cpu_id: 0,
            _pad0: 0,
            context_switches: 0,
            dispatches: 0,
            ready_queue_depth: 0,
            run_ticks: 0,
            idle_ticks: 0,
            timer_preemptions: 0,
            voluntary_yields: 0,
            ready_queue_high_water: 0,
        }
    }
}

impl TraceEventV1 {
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            ticks: 0,
            kind: 0,
            cpu_id: 0,
            arg0: 0,
            arg1: 0,
        }
    }
}

impl KernelObservabilityV1 {
    pub const fn empty() -> Self {
        Self {
            version: 0,
            cpu_count: 0,
            trace_capacity: KERNEL_OBSERVABILITY_TRACE_EVENTS as u32,
            trace_count: 0,
            heap_current_bytes: 0,
            heap_high_water_bytes: 0,
            heap_alloc_failures: 0,
            syscall_total: 0,
            syscall_ipc: 0,
            syscall_file: 0,
            syscall_ai: 0,
            syscall_net: 0,
            syscall_other: 0,
            frames_used: 0,
            buddy_free_pages: 0,
            buddy_largest_free_order: 0,
            _pad0: 0,
            sched: [SchedulerStatsV1::empty(); KERNEL_OBSERVABILITY_MAX_CPUS],
            trace: [TraceEventV1::empty(); KERNEL_OBSERVABILITY_TRACE_EVENTS],
        }
    }
}

// === Handles (Phase 1.5b) ===============================================

/// A capability-style integer handle into the calling process's handle
/// table, referring to one endpoint of an anonymous IPC channel.
/// Phase 2 extends with permission bits per ANSPEC-001 §3.7.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct Handle(pub u32);

/// Kernel-enforced outbound dial authority. Its immutable scope lives in the
/// kernel handle object; userland carries only this opaque handle index.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct NetDialCap(pub Handle);

/// Explicit negative-control sentinel. It can never name an installed handle.
pub const NO_NET_DIAL_CAP: NetDialCap = NetDialCap(Handle(u32::MAX));

/// A capability to operate on a spawned child process. Returned by
/// `sys_spawn` alongside the child's pid; Phase 2 activates `sys_kill`,
/// `sys_wait`, and `sys_inspect` through per-operation rights.
/// Type-distinct from `Handle` so the compiler refuses
/// `sys_send(process_handle, ...)` and similar mix-ups.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct ProcessHandle(pub u32);

/// A capability-shaped file handle returned by `sys_open`.
/// BUILD-1 reserves fd 1 for serial stdout; `sys_open` will not return it.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct FileHandle(pub u32);

pub const STDIN_FD: u32 = 0;
pub const STDOUT_FD: u32 = 1;

/// A capability-shaped UDP socket handle returned by `sys_socket_create`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct SocketHandle(pub u32);

/// A capability-shaped TCP socket handle returned by network TCP syscalls.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TcpSocketHandle(pub u32);

/// A capability-shaped TCP listener handle returned by `sys_net_listen_tcp`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TcpListenerHandle(pub u32);

/// A capability-shaped tensor handle returned by AI tensor syscalls.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct TensorHandle(pub u32);

/// A capability-shaped model handle returned by AI model syscalls.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct ModelHandle(pub u32);

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct GenerateConstrainedSequenceDesc {
    pub offset: u32,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct GenerateConstrainedDesc {
    pub max_new: u32,
    pub eos_token_id: u32,
    pub sequence_count: u32,
    pub _reserved0: u32,
    pub sequences_ptr: u64,
    pub tokens_ptr: u64,
    pub tokens_len: u32,
    pub _reserved1: u32,
}

pub const GENERATE_MASK_FLAG_DISABLED: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct GenerateMaskDesc {
    pub mask_ptr: u64,
    pub mask_bits: u32,
    pub mask_bytes: u32,
    pub flags: u32,
    pub _reserved0: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct NetStatsV1 {
    pub packets_in: u64,
    pub packets_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct NetCandidate {
    pub family: u16,
    pub reserved: u16,
    pub ttl: u32,
    pub addr: [u8; 16],
}

pub const AF_INET: u32 = 2;
pub const AF_INET6: u32 = 10;
pub const SOCK_DGRAM: u32 = 2;
pub const SOCKADDR_INET_LEN: usize = core::mem::size_of::<SockAddrIn>();
pub const SOCKADDR_INET6_LEN: usize = core::mem::size_of::<SockAddrIn6>();

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CapRights {
    pub meta: u8,
    pub semantic: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(transparent)]
pub struct FileRights(pub u32);

impl FileRights {
    pub const READ: Self = Self(1 << 0);
    pub const WRITE: Self = Self(1 << 1);
    pub const LOOKUP: Self = Self(1 << 2);
    pub const READDIR: Self = Self(1 << 3);

    // Reserved future bits: MMAP, GRANT, DERIVE, RETYPE. BUILD-4A activated
    // LOOKUP/READDIR; BUILD-4B activates MOUNT. *_RESERVED aliases remain for
    // source compatibility.
    pub const LOOKUP_RESERVED: Self = Self::LOOKUP;
    pub const READDIR_RESERVED: Self = Self::READDIR;
    pub const EXECUTE: Self = Self(1 << 4);
    pub const EXECUTE_RESERVED: Self = Self::EXECUTE;
    pub const MMAP_RESERVED: Self = Self(1 << 5);
    pub const GRANT_RESERVED: Self = Self(1 << 24);
    pub const DERIVE_RESERVED: Self = Self(1 << 25);
    pub const RETYPE_RESERVED: Self = Self(1 << 26);
    pub const MOUNT: Self = Self(1 << 27);
    pub const MOUNT_RESERVED: Self = Self::MOUNT;

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

// === BUILD-4B mount ABI ================================================

pub const MOUNT_REQUEST_V1_VERSION: u32 = 1;
pub const MOUNT_REQUEST_V1_HEADER_LEN: usize = 32;

/// `fs_type` values for `MountRequestV1` in BUILD-4B.
pub const MOUNT_FS_TYPE_NEOFS_DEVICE0_RO: u32 = 1;
pub const MOUNT_FS_TYPE_NEOFS_DEVICE0_RW: u32 = 2;
pub const MOUNT_FS_TYPE_FAT32_DEVICE1_RO: u32 = 3;
pub const MOUNT_FS_TYPE_EXT4_DEVICE2_RO: u32 = 4;

/// Mount flag bit 0: allow executable opens on a foreign filesystem mount.
pub const MOUNT_FLAG_EXEC_ALLOWED: u32 = 0x0000_0001;

/// Build a `MountRequestV1` byte buffer with `source` and `target` strings
/// packed after the 32-byte header. Returns the encoded bytes.
pub fn encode_mount_request_v1(fs_type: u32, source: &str, target: &str) -> alloc::vec::Vec<u8> {
    let mut buf =
        alloc::vec::Vec::with_capacity(MOUNT_REQUEST_V1_HEADER_LEN + source.len() + target.len());
    buf.extend_from_slice(&MOUNT_REQUEST_V1_VERSION.to_le_bytes());
    buf.extend_from_slice(&fs_type.to_le_bytes());
    let source_off = MOUNT_REQUEST_V1_HEADER_LEN as u32;
    let source_len = source.len() as u32;
    let target_off = source_off + source_len;
    let target_len = target.len() as u32;
    buf.extend_from_slice(&source_off.to_le_bytes());
    buf.extend_from_slice(&source_len.to_le_bytes());
    buf.extend_from_slice(&target_off.to_le_bytes());
    buf.extend_from_slice(&target_len.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved0
    buf.extend_from_slice(&0u32.to_le_bytes()); // reserved1
    buf.extend_from_slice(source.as_bytes());
    buf.extend_from_slice(target.as_bytes());
    buf
}

/// Mount through directory FileCap `dir`. `req` is a `MountRequestV1` byte
/// buffer (see `encode_mount_request_v1`). Returns 0 on success or a negative
/// errno.
#[inline]
pub fn sys_mount(dir: FileHandle, req: &[u8], flags: u32) -> i64 {
    unsafe {
        raw4(
            SYS_MOUNT,
            dir.0 as u64,
            req.as_ptr() as u64,
            req.len() as u64,
            flags as u64,
        )
    }
}

/// Unmount the child named `name` under directory FileCap `dir`. Returns 0 on
/// success or a negative errno (`EBUSY` while any OpenFile references the
/// mount, `ENOENT` if no such child, `EACCES` without `MOUNT` rights,
/// `ENOTDIR` if `dir` is not a directory).
#[inline]
pub fn sys_umount(dir: FileHandle, name: &str, flags: u32) -> i64 {
    unsafe {
        raw4(
            SYS_UMOUNT,
            dir.0 as u64,
            name.as_ptr() as u64,
            name.len() as u64,
            flags as u64,
        )
    }
}

pub const OPEN_ROOT_TEMPORARY: u32 = 0;
pub const OPEN_CREATE_REGULAR: u32 = 1 << 0;
/// OT-66 K1b (`cap-rooted-open` kernels): open a directory as a scope, a cap
/// whose rights (file rights included) bound the root-form opens at or
/// beneath it. `EINVAL` on a kernel without the feature.
pub const OPEN_SCOPE_DIR: u32 = 1 << 1;

pub const NEO_DIRENT_KIND_DIRECTORY: u8 = 1;
pub const NEO_DIRENT_KIND_REGULAR: u8 = 2;
pub const NEO_DIRENT_KIND_SYMLINK: u8 = 3;
pub const NEO_DIRENT_KIND_MODEL: u8 = 4;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct NeoDirentV1<'a> {
    pub record_len: u32,
    pub name_len: u16,
    pub kind: u8,
    pub flags: u8,
    pub next_cursor: u64,
    pub name: &'a [u8],
}

impl CapRights {
    pub const fn new(meta: u8, semantic: u8) -> Self {
        Self { meta, semantic }
    }

    pub const fn pack(self) -> u16 {
        ((self.meta as u16) << 8) | self.semantic as u16
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

pub mod net_dial_rights {
    pub const DIAL: u8 = 1 << 0;
    /// OT-61 rung 5b: the cap may reach non-public addresses (root caps carry
    /// it on a `net-dial-public-only` kernel; a cap derived without it dials
    /// only public addresses).
    pub const NON_PUBLIC: u8 = 1 << 1;
}

/// GPU P1 Slice 6 semantic rights.  These mirror the kernel's typed
/// `GpuComputeCap` algebra so an application can attenuate an inherited cap;
/// they are not a capability constructor and cannot mint a GPU queue.
pub mod gpu_compute_rights {
    pub const BUFFER_READ: u8 = 1 << 0;
    pub const BUFFER_WRITE: u8 = 1 << 1;
    pub const QUEUE_SUBMIT: u8 = 1 << 2;
    pub const FENCE_WAIT: u8 = 1 << 3;
    pub const FENCE_QUERY: u8 = 1 << 4;
}

pub const FULL_META_RIGHTS: u8 =
    meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT | meta_rights::REVOKE;
pub const FULL_CHANNEL_RIGHTS: CapRights = CapRights::new(
    FULL_META_RIGHTS,
    channel_rights::SEND | channel_rights::RECV | channel_rights::CLOSE,
);
pub const FULL_PROCESS_RIGHTS: CapRights = CapRights::new(
    FULL_META_RIGHTS,
    process_rights::KILL | process_rights::WAIT | process_rights::INSPECT,
);

/// Raw 3-argument SYSCALL wrapper. Returns the kernel's i64 return value
/// (negative = errno, non-negative = success).
///
/// # Safety
/// The caller must pass arguments valid for the given syscall number: any
/// pointer args must reference live, correctly-sized user memory for the
/// syscall's contract. The kernel validates pointers/lengths defensively, but
/// this wrapper performs no checks of its own.
#[inline]
pub unsafe fn raw3(num: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    let ret: i64;
    // SAFETY: caller upholds the raw syscall ABI contract documented above.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            // SYSCALL clobbers RCX (saved user RIP) and R11 (saved user RFLAGS).
            lateout("rcx") _,
            lateout("r11") _,
            // The kernel's syscall path saves and restores all other GP regs,
            // so no other clobbers need to be declared.
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Raw 4-argument SYSCALL wrapper. Phase 1.5b: needed by the extended
/// SYS_SPAWN that takes (name_ptr, name_len, handles_ptr, handles_count).
/// The 4th arg lands in R10, mirroring the kernel's ABI (R10 substitutes
/// for RCX which the SYSCALL instruction clobbers with the saved user RIP).
///
/// # Safety
/// Same contract as [`raw3`]: the caller must pass arguments valid for the
/// given syscall number, including any pointer args referencing live,
/// correctly-sized user memory.
#[inline]
pub unsafe fn raw4(num: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    // SAFETY: caller upholds the raw syscall ABI contract documented above.
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Raw 6-argument SYSCALL wrapper. Phase 4 BUILD-2 uses this for
/// destination-bearing UDP datagrams.
///
/// # Safety
/// Same contract as [`raw3`]: the caller must pass arguments valid for the
/// given syscall number, including any pointer args referencing live,
/// correctly-sized user memory.
#[inline]
pub unsafe fn raw6(num: u64, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") num => ret,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            in("r8") a4,
            in("r9") a5,
            lateout("rcx") _,
            lateout("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// Write `len` bytes from `buf` to file descriptor `fd`. Phase 1.4 only
/// supports fd=1 (stdout, routed to the kernel serial console). Returns the
/// number of bytes written, or a negative errno on failure.
#[inline]
pub fn sys_write(fd: u32, buf: *const u8, len: usize) -> i64 {
    // SAFETY: kernel validates buf+len fits in user half (and doesn't wrap).
    unsafe { raw3(SYS_WRITE, fd as u64, buf as u64, len as u64) }
}

/// Temporary BUILD-1 root-open wrapper:
/// `sys_open(path_ptr, path_len, flags, requested_rights_u32)`.
///
/// The target model is capability-rooted open:
/// `sys_open(dir_cap, path, flags, requested_rights)`.
#[inline]
pub fn sys_open(path: &str, flags: u32, requested: FileRights) -> Result<FileHandle, i64> {
    let r = unsafe {
        raw4(
            SYS_OPEN,
            path.as_ptr() as u64,
            path.len() as u64,
            flags as u64,
            requested.bits() as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(FileHandle(r as u32))
    }
}

#[inline]
pub fn sys_read(handle: FileHandle, buf: &mut [u8]) -> i64 {
    sys_read_fd(handle.0, buf)
}

#[inline]
pub fn sys_read_fd(fd: u32, buf: &mut [u8]) -> i64 {
    unsafe {
        raw3(
            SYS_READ,
            fd as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

#[inline]
pub fn sys_close(handle: FileHandle) -> i64 {
    unsafe { raw3(SYS_CLOSE, handle.0 as u64, 0, 0) }
}

/// Explicitly drop an IPC endpoint. The peer's blocked or next channel
/// operation observes `EPIPE`, matching process-exit cleanup.
#[inline]
pub fn sys_channel_close(handle: Handle) -> i64 {
    unsafe { raw3(SYS_CLOSE, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_fsync(handle: FileHandle) -> i64 {
    unsafe { raw3(SYS_FSYNC, handle.0 as u64, 0, 0) }
}

/// OT-61 C2.1: the calling process's pid, or a negative errno.
#[inline]
pub fn sys_process_self() -> i64 {
    unsafe { raw3(SYS_PROCESS_SELF, 0, 0, 0) }
}

/// OT-66 K1a: this process's widest RootDirCap (a file handle on the VFS
/// root), found by `SYS_HANDLE_INFO`; the widest, because a root-form open of
/// "/" is a RootDirCap too, often a narrow one. `None` without one, or on a
/// kernel without the syscall.
pub fn root_dir_cap() -> Option<Handle> {
    (0..256u32)
        .filter_map(|idx| match sys_handle_info(idx) {
            Ok(info)
                if info.kind == handle_info::HandleKind::File
                    && info.flags & handle_info::FLAG_ROOT_DIR != 0 =>
            {
                Some((info.file_rights.count_ones(), idx))
            }
            _ => None,
        })
        .max_by_key(|&(width, idx)| (width, core::cmp::Reverse(idx)))
        .map(|(_, idx)| Handle(idx))
}

/// OT-66 K1a: the rights a spawned client's RootDirCap carries: every file
/// right but MOUNT, plus DERIVE and GRANT (to pass it on) and REVOKE.
#[cfg(feature = "cap-rooted-open")]
pub const CLIENT_ROOT_CAP_RIGHTS: CapRights = CapRights::new(
    meta_rights::USE | meta_rights::DERIVE | meta_rights::GRANT | meta_rights::REVOKE,
    (FileRights::READ.bits()
        | FileRights::WRITE.bits()
        | FileRights::LOOKUP.bits()
        | FileRights::READDIR.bits()
        | FileRights::EXECUTE.bits()) as u8,
);

/// OT-66 K1a (transitional, S2 point 4): spawn `name` with `inherited` and,
/// after them (so their indices don't move), a RootDirCap derived from this
/// process's own. The derived cap stays in this table, so the spawner can
/// revoke it, and with it every file the child opened through it. K1b
/// replaces the whole-root cap with directory caps scoped to the client.
/// Without the feature, or without a RootDirCap to derive from, this is
/// `sys_spawn`.
pub fn sys_spawn_client(name: &str, inherited: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
    #[cfg(feature = "cap-rooted-open")]
    if let Some(root) = root_dir_cap()
        && let Ok(client) = sys_derive(root, CLIENT_ROOT_CAP_RIGHTS)
    {
        let mut all = alloc::vec::Vec::with_capacity(inherited.len() + 1);
        all.extend_from_slice(inherited);
        all.push(client);
        return sys_spawn(name, &all);
    }
    sys_spawn(name, inherited)
}

/// OT-66 K1b: the scopes Arthur is spawned with: his own state, read-write
/// (memory, grants, undo), and the boot modules, read-only (the model and
/// its vocabulary). Nothing else on the volume opens for him.
pub const ARTHUR_SCOPES: &[(&str, FileRights)] = &[
    (
        "/neofs/.arthur",
        FileRights::READ
            .union(FileRights::WRITE)
            .union(FileRights::LOOKUP)
            .union(FileRights::READDIR),
    ),
    (
        "/boot",
        FileRights::READ
            .union(FileRights::LOOKUP)
            .union(FileRights::READDIR),
    ),
];

/// OT-66 K1b: spawn `name` with `inherited` and, after them (so their
/// indices don't move), a directory cap for each of `scopes` that opens:
/// the child's root-form opens then reach only those subtrees, within each
/// one's rights. A scope that doesn't open (no such directory) is left out.
/// This process's copies are closed after the spawn; the child's stay
/// revocable through the RootDirCap they were opened under. Without the
/// feature this is `sys_spawn` (the root form is ambient).
pub fn sys_spawn_scoped(
    name: &str,
    inherited: &[Handle],
    #[cfg_attr(not(feature = "cap-rooted-open"), allow(unused_variables))] scopes: &[(
        &str,
        FileRights,
    )],
) -> Result<(u32, ProcessHandle), i64> {
    #[cfg(feature = "cap-rooted-open")]
    {
        let mut all = alloc::vec::Vec::with_capacity(inherited.len() + scopes.len());
        all.extend_from_slice(inherited);
        let first_scope = all.len();
        for &(path, rights) in scopes {
            if let Ok(dir) = sys_open(path, OPEN_SCOPE_DIR, rights) {
                all.push(Handle(dir.0));
            }
        }
        let spawned = sys_spawn(name, &all);
        for &scope in &all[first_scope..] {
            let _ = sys_close(FileHandle(scope.0));
        }
        spawned
    }
    #[cfg(not(feature = "cap-rooted-open"))]
    sys_spawn(name, inherited)
}

/// OT-61 C2.1: the kind and rights of handle `idx`; `Err` carries the
/// negative errno (EBADF for an empty slot).
pub fn sys_handle_info(idx: u32) -> Result<handle_info::HandleInfo, i64> {
    let mut rec = [0u8; handle_info::RECORD_LEN];
    let ret = unsafe {
        raw3(
            SYS_HANDLE_INFO,
            idx as u64,
            rec.as_mut_ptr() as u64,
            rec.len() as u64,
        )
    };
    if ret < 0 {
        return Err(ret);
    }
    handle_info::HandleInfo::decode(&rec).ok_or(-22)
}

#[inline]
pub fn sys_ftruncate(handle: FileHandle, new_len: u64) -> i64 {
    unsafe { raw3(SYS_FTRUNCATE, handle.0 as u64, new_len, 0) }
}

/// OT-67 W1: `bytes` become the file's whole content, in one commit. 0, or
/// a negative errno: `EFAULT` (`bytes` not user memory), `EACCES` (no
/// WRITE), `EISDIR` (not a regular file), `EINVAL` (past 4 MiB, or no
/// space), `ESTALE` (another handle replaced it first), `EIO` (the device
/// failed).
#[inline]
pub fn sys_file_replace(handle: FileHandle, bytes: &[u8]) -> i64 {
    unsafe {
        raw3(
            SYS_FILE_REPLACE,
            handle.0 as u64,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
        )
    }
}

#[inline]
pub fn sys_readdir(handle: FileHandle, out: &mut [u8], cursor: &mut u64) -> i64 {
    unsafe {
        raw4(
            SYS_READDIR,
            handle.0 as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            cursor as *mut u64 as u64,
        )
    }
}

pub fn parse_dirent_v1(buf: &[u8]) -> Result<NeoDirentV1<'_>, i64> {
    if buf.len() < 16 {
        return Err(EINVAL);
    }
    let record_len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let name_len = u16::from_le_bytes([buf[4], buf[5]]);
    let needed = 16usize.checked_add(name_len as usize).ok_or(EINVAL)?;
    if record_len as usize != needed || buf.len() < needed {
        return Err(EINVAL);
    }
    let next_cursor = u64::from_le_bytes([
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
    ]);
    Ok(NeoDirentV1 {
        record_len,
        name_len,
        kind: buf[6],
        flags: buf[7],
        next_cursor,
        name: &buf[16..needed],
    })
}

/// Spawn a new user process by name, optionally giving it a list of
/// handles. The kernel looks up a Limine module whose path ends with
/// `/neo-<name>` and ELF-loads it. The child receives `handles[i]` at
/// its own handle index `i` (deterministic ordering, ADR-004 §Q2).
///
/// On success returns `(child_pid, ProcessHandle)`. The
/// `ProcessHandle` is the capability to terminate the child via
/// `sys_kill` (ADR-007 §Q1). On failure returns a negative errno
/// (e.g., `-2 ENOENT`, `-9 EBADF` for a stale inherited-handle index).
///
/// Phase 1.5b: spawn is the only mechanism by which a handle can cross
/// a process boundary. Phase 2 capability-grant adds in-flight passing.
#[inline]
pub fn sys_spawn(name: &str, handles: &[Handle]) -> Result<(u32, ProcessHandle), i64> {
    // SAFETY: name + handles are valid slices; kernel validates buf+len.
    let r = unsafe {
        raw4(
            SYS_SPAWN,
            name.as_ptr() as u64,
            name.len() as u64,
            handles.as_ptr() as u64,
            handles.len() as u64,
        )
    };
    if r < 0 {
        return Err(r);
    }
    // Packing matches kernel sys_spawn_impl: low 32 = pid, high 32 = handle.
    let packed = r as u64;
    let pid = (packed & 0xFFFF_FFFF) as u32;
    let handle = ProcessHandle((packed >> 32) as u32);
    Ok((pid, handle))
}

/// OT-61 C2.2: [`sys_spawn`] whose child gets its own memory account of
/// `budget_bytes` (rounded up to 4 KiB frames), nested under the caller's, so
/// it is also bounded by every budget above it. The child's own image, stack
/// and page tables count against it: a budget too small to load the module
/// fails with `ENOMEM`. A `mem-quota` kernel only.
#[inline]
pub fn sys_spawn_budgeted(
    name: &str,
    handles: &[Handle],
    budget_bytes: u64,
) -> Result<(u32, ProcessHandle), i64> {
    // SAFETY: name + handles are valid slices; kernel validates buf+len.
    let r = unsafe {
        raw6(
            SYS_SPAWN_BUDGETED,
            name.as_ptr() as u64,
            name.len() as u64,
            handles.as_ptr() as u64,
            handles.len() as u64,
            budget_bytes,
            0,
        )
    };
    if r < 0 {
        return Err(r);
    }
    let packed = r as u64;
    Ok((
        (packed & 0xFFFF_FFFF) as u32,
        ProcessHandle((packed >> 32) as u32),
    ))
}

/// OT-61 C2.2b: [`sys_spawn_budgeted`] with an object budget: the child's
/// account may charge at most `obj_bytes` of kernel-heap objects (handles,
/// spawns, threads, at their declared weights), nested under the caller's.
/// `frame_bytes` 0 gives the child no frame budget of its own. An
/// `obj-quota` kernel only.
#[inline]
pub fn sys_spawn_budgeted2(
    name: &str,
    handles: &[Handle],
    frame_bytes: u64,
    obj_bytes: u64,
) -> Result<(u32, ProcessHandle), i64> {
    // SAFETY: name + handles are valid slices; kernel validates buf+len.
    let r = unsafe {
        raw6(
            SYS_SPAWN_BUDGETED2,
            name.as_ptr() as u64,
            name.len() as u64,
            handles.as_ptr() as u64,
            handles.len() as u64,
            frame_bytes,
            obj_bytes,
        )
    };
    if r < 0 {
        return Err(r);
    }
    let packed = r as u64;
    Ok((
        (packed & 0xFFFF_FFFF) as u32,
        ProcessHandle((packed >> 32) as u32),
    ))
}

/// OT-61 C2.2b: the caller's account with its object counter (the v2
/// record). An `obj-quota` kernel only.
#[inline]
pub fn sys_mem_info_v2() -> Result<mem_info::MemInfoV2, i64> {
    let mut rec = [0u8; mem_info::RECORD_V2_LEN];
    // SAFETY: `rec` is a writable buffer of exactly the v2 record length.
    let r = unsafe { raw3(SYS_MEM_INFO, rec.as_mut_ptr() as u64, rec.len() as u64, 0) };
    if r < 0 {
        return Err(r);
    }
    mem_info::decode_v2(&rec).ok_or(EINVAL)
}

/// L7.1: the kernel's boot-time record. A `boot-time` kernel only.
pub fn sys_boot_times() -> Result<boot_time_logic::BootTimes, i64> {
    let mut rec = [0u8; boot_time_logic::BOOT_TIMES_V1_LEN];
    // SAFETY: `rec` is a writable buffer of exactly the record length.
    let r = unsafe { raw3(SYS_BOOT_TIMES, rec.as_mut_ptr() as u64, rec.len() as u64, 0) };
    if r < 0 {
        return Err(r);
    }
    boot_time_logic::decode(&rec).ok_or(EINVAL)
}

/// L7.1: `what`'s boot-time line now, e.g. `Sign-in 9.87 s after power-on
/// (firmware 4.12 s).`, or `None` when the kernel has no `boot-time`.
pub fn boot_milestone_line(what: &str) -> Option<alloc::string::String> {
    let times = sys_boot_times().ok()?;
    // SAFETY: RDTSC is unprivileged (CR4.TSD is clear) and has no side
    // effects.
    let now = unsafe { core::arch::x86_64::_rdtsc() };
    let mut line = alloc::string::String::new();
    boot_time_logic::write_milestone(&mut line, what, &times, now).ok()?;
    Some(line)
}

/// L7.1: the boot summary ending with `what` now, e.g. `From power-on:
/// firmware 4.12 s, desktop 7.95 s, Arthur ready 21.70 s.`, or `None` when
/// the kernel has no `boot-time`.
pub fn boot_summary_line(what: &str) -> Option<alloc::string::String> {
    let times = sys_boot_times().ok()?;
    // SAFETY: as in `boot_milestone_line`.
    let now = unsafe { core::arch::x86_64::_rdtsc() };
    let mut line = alloc::string::String::new();
    boot_time_logic::write_summary(&mut line, what, &times, now).ok()?;
    Some(line)
}

/// OT-61 C2.2: the caller's memory account. A `mem-quota` kernel only.
#[inline]
pub fn sys_mem_info() -> Result<mem_info::MemInfo, i64> {
    let mut rec = [0u8; mem_info::RECORD_LEN];
    // SAFETY: `rec` is a writable buffer of exactly the record length.
    let r = unsafe { raw3(SYS_MEM_INFO, rec.as_mut_ptr() as u64, rec.len() as u64, 0) };
    if r < 0 {
        return Err(r);
    }
    mem_info::decode(&rec).ok_or(EINVAL)
}

/// Terminate the process named by `handle` (ADR-007 §Q1, capability-
/// mediated kill). Forced — a Ready target is yanked from the scheduler
/// ready queue; a Blocked-in-IPC target is extracted from its channel
/// endpoint and zombified. No graceful unwind on the killed side.
///
/// Returns `0` on success, negative errno on failure: `-9 EBADF` if
/// the handle index is stale or refers to a non-Process variant;
/// `-22 EINVAL` if the resolved target is the calling process
/// (self-kill is illegal — ADR-007 §Implementation note 1).
#[inline]
pub fn sys_kill(handle: ProcessHandle) -> i64 {
    // SAFETY: SYS_KILL takes one arg (handle index), returns 0 or errno.
    unsafe { raw3(SYS_KILL, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_wait(handle: ProcessHandle, flags: u64) -> Result<i32, i64> {
    // SAFETY: SYS_WAIT takes a process handle plus scalar flags.
    let r = unsafe { raw3(SYS_WAIT, handle.0 as u64, flags, 0) };
    if r < 0 {
        return Err(r);
    }
    if (r & WAIT_EXITED_TAG) == 0 {
        return Err(EINVAL);
    }
    Ok((r as u32) as i32)
}

#[inline]
pub fn sys_inspect(handle: ProcessHandle) -> Result<ProcessSnapshotV1, i64> {
    let mut snapshot = ProcessSnapshotV1::empty();
    // SAFETY: `snapshot` is a valid out pointer for the fixed snapshot ABI.
    let r = unsafe {
        raw3(
            SYS_INSPECT,
            handle.0 as u64,
            (&mut snapshot as *mut ProcessSnapshotV1) as u64,
            core::mem::size_of::<ProcessSnapshotV1>() as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(snapshot) }
}

#[inline]
pub fn sys_sysstat() -> Result<SysStatV1, i64> {
    let mut stats = SysStatV1::empty();
    // SAFETY: `stats` is a valid out pointer for the fixed aggregate ABI.
    let r = unsafe {
        raw3(
            SYS_SYSSTAT,
            (&mut stats as *mut SysStatV1) as u64,
            core::mem::size_of::<SysStatV1>() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(stats) }
}

#[inline]
pub fn sys_observe() -> Result<KernelObservabilityV1, i64> {
    let mut stats = KernelObservabilityV1::empty();
    // SAFETY: `stats` is a valid out pointer for the fixed observability ABI.
    let r = unsafe {
        raw3(
            SYS_OBSERVE,
            (&mut stats as *mut KernelObservabilityV1) as u64,
            core::mem::size_of::<KernelObservabilityV1>() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(stats) }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SockAddrIn {
    pub family: u16,
    pub port_be: u16,
    pub addr: [u8; 4],
}

impl SockAddrIn {
    pub const fn new_loopback(port: u16) -> Self {
        Self {
            family: AF_INET as u16,
            port_be: port.to_be(),
            addr: [127, 0, 0, 1],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SockAddrIn6 {
    pub family: u16,
    pub port_be: u16,
    pub addr: [u8; 16],
}

impl SockAddrIn6 {
    pub const fn new(addr: [u8; 16], port: u16) -> Self {
        Self {
            family: AF_INET6 as u16,
            port_be: port.to_be(),
            addr,
        }
    }

    pub const fn new_loopback(port: u16) -> Self {
        Self::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], port)
    }
}

#[derive(Copy, Clone)]
pub enum SockAddr {
    Inet(SockAddrIn),
    Inet6(SockAddrIn6),
}

impl SockAddr {
    // The wire length of a socket address is a fixed non-zero constant per
    // family; an `is_empty` would always be false and is meaningless here.
    #[allow(clippy::len_without_is_empty)]
    pub const fn len(&self) -> usize {
        match self {
            SockAddr::Inet(_) => SOCKADDR_INET_LEN,
            SockAddr::Inet6(_) => SOCKADDR_INET6_LEN,
        }
    }

    pub const fn as_ptr(&self) -> *const u8 {
        match self {
            SockAddr::Inet(addr) => addr as *const SockAddrIn as *const u8,
            SockAddr::Inet6(addr) => addr as *const SockAddrIn6 as *const u8,
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        match self {
            SockAddr::Inet(addr) => addr as *mut SockAddrIn as *mut u8,
            SockAddr::Inet6(addr) => addr as *mut SockAddrIn6 as *mut u8,
        }
    }
}

#[inline]
pub fn sys_socket_create(family: u32, socktype: u32) -> Result<SocketHandle, i64> {
    let r = unsafe { raw3(SYS_SOCKET_CREATE, family as u64, socktype as u64, 0) };
    if r < 0 {
        Err(r)
    } else {
        Ok(SocketHandle(r as u32))
    }
}

#[inline]
pub fn sys_socket_bind_in(handle: SocketHandle, addr: &SockAddrIn) -> i64 {
    unsafe {
        raw3(
            SYS_SOCKET_BIND,
            handle.0 as u64,
            (addr as *const SockAddrIn) as u64,
            core::mem::size_of::<SockAddrIn>() as u64,
        )
    }
}

#[inline]
pub fn sys_socket_bind_in6(handle: SocketHandle, addr: &SockAddrIn6) -> i64 {
    unsafe {
        raw3(
            SYS_SOCKET_BIND,
            handle.0 as u64,
            (addr as *const SockAddrIn6) as u64,
            core::mem::size_of::<SockAddrIn6>() as u64,
        )
    }
}

#[inline]
pub fn sys_socket_send(handle: SocketHandle, buf: &[u8]) -> i64 {
    unsafe {
        raw4(
            SYS_SOCKET_SEND,
            handle.0 as u64,
            buf.as_ptr() as u64,
            buf.len() as u64,
            0,
        )
    }
}

#[inline]
pub fn sys_socket_sendto(handle: SocketHandle, buf: &[u8], addr: &SockAddr) -> i64 {
    unsafe {
        raw6(
            SYS_SOCKET_SENDTO,
            handle.0 as u64,
            buf.as_ptr() as u64,
            buf.len() as u64,
            0,
            addr.as_ptr() as u64,
            addr.len() as u64,
        )
    }
}

#[inline]
pub fn sys_socket_recv(handle: SocketHandle, buf: &mut [u8]) -> i64 {
    unsafe {
        raw4(
            SYS_SOCKET_RECV,
            handle.0 as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
        )
    }
}

#[inline]
pub fn sys_socket_recvfrom(handle: SocketHandle, buf: &mut [u8], addr_out: &mut SockAddr) -> i64 {
    let mut addr_len = addr_out.len();
    unsafe {
        raw6(
            SYS_SOCKET_RECVFROM,
            handle.0 as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
            addr_out.as_mut_ptr() as u64,
            (&mut addr_len as *mut usize) as u64,
        )
    }
}

#[inline]
pub fn sys_socket_close(handle: SocketHandle) -> i64 {
    unsafe { raw3(SYS_SOCKET_CLOSE, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_tcp_send(handle: TcpSocketHandle, buf: &[u8]) -> i64 {
    unsafe {
        raw4(
            SYS_SOCKET_SEND,
            handle.0 as u64,
            buf.as_ptr() as u64,
            buf.len() as u64,
            0,
        )
    }
}

#[inline]
pub fn sys_tcp_recv(handle: TcpSocketHandle, buf: &mut [u8]) -> i64 {
    unsafe {
        raw4(
            SYS_SOCKET_RECV,
            handle.0 as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
            0,
        )
    }
}

#[inline]
pub fn sys_tcp_close(handle: TcpSocketHandle) -> i64 {
    unsafe { raw3(SYS_SOCKET_CLOSE, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_net_listen_tcp(addr: &SockAddr, backlog: usize) -> Result<TcpListenerHandle, i64> {
    let r = unsafe {
        raw3(
            SYS_NET_LISTEN_TCP,
            addr.as_ptr() as u64,
            addr.len() as u64,
            backlog as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(TcpListenerHandle(r as u32))
    }
}

/// Create a listener while `net-dial-cap` confinement is enabled.  Listener
/// creation requires the process's root network authority; accepted sockets
/// are subsequently authorized by the listener/socket capability lineage.
#[inline]
pub fn sys_net_listen_tcp_with_cap(
    cap: NetDialCap,
    addr: &SockAddr,
    backlog: usize,
) -> Result<TcpListenerHandle, i64> {
    let r = unsafe {
        raw4(
            SYS_NET_LISTEN_TCP,
            addr.as_ptr() as u64,
            addr.len() as u64,
            backlog as u64,
            cap.0.0 as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(TcpListenerHandle(r as u32))
    }
}

#[inline]
pub fn sys_net_accept_tcp(handle: TcpListenerHandle) -> Result<TcpSocketHandle, i64> {
    let r = unsafe { raw3(SYS_NET_ACCEPT_TCP, handle.0 as u64, 0, 0) };
    if r < 0 {
        Err(r)
    } else {
        Ok(TcpSocketHandle(r as u32))
    }
}

#[inline]
pub fn sys_net_stats(handle: SocketHandle) -> Result<NetStatsV1, i64> {
    let mut stats = NetStatsV1::default();
    let r = unsafe {
        raw3(
            SYS_NET_STATS,
            handle.0 as u64,
            (&mut stats as *mut NetStatsV1) as u64,
            core::mem::size_of::<NetStatsV1>() as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(stats) }
}

#[inline]
pub fn sys_net_resolve(name: &str, family_hint: u32, out: &mut [NetCandidate]) -> i64 {
    unsafe {
        raw6(
            SYS_NET_RESOLVE,
            name.as_ptr() as u64,
            name.len() as u64,
            family_hint as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

#[inline]
pub fn sys_net_dial(cap: NetDialCap, name: &str, port: u16) -> Result<TcpSocketHandle, i64> {
    let r = unsafe {
        raw4(
            SYS_NET_DIAL,
            name.as_ptr() as u64,
            name.len() as u64,
            port as u64,
            cap.0.0 as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(TcpSocketHandle(r as u32))
    }
}

/// Attenuate `parent` to one concrete `(host, port)` scope. The kernel rejects
/// widening and records the child in the normal derivation/revocation graph.
#[inline]
pub fn sys_net_dial_cap_derive(
    parent: NetDialCap,
    host: &str,
    port: u16,
) -> Result<NetDialCap, i64> {
    let r = unsafe {
        raw4(
            SYS_NET_DIAL_CAP_DERIVE,
            parent.0.0 as u64,
            host.as_ptr() as u64,
            host.len() as u64,
            port as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(NetDialCap(Handle(r as u32)))
    }
}

#[inline]
pub fn sys_net_tcp_enable_tls(
    handle: TcpSocketHandle,
    key_material: &mut [u8; TLS_KEY_MATERIAL_LEN],
) -> i64 {
    unsafe {
        raw3(
            SYS_NET_TCP_ENABLE_TLS,
            handle.0 as u64,
            key_material.as_mut_ptr() as u64,
            key_material.len() as u64,
        )
    }
}

#[inline]
pub fn sys_tensor_create(desc: &TensorCreateDesc) -> Result<TensorHandle, i64> {
    let r = unsafe {
        raw3(
            SYS_TENSOR_CREATE,
            (desc as *const TensorCreateDesc) as u64,
            core::mem::size_of::<TensorCreateDesc>() as u64,
            0,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(TensorHandle(r as u32))
    }
}

#[inline]
pub fn sys_tensor_query(handle: TensorHandle, out: &mut TensorQueryDesc) -> i64 {
    unsafe {
        raw3(
            SYS_TENSOR_QUERY,
            handle.0 as u64,
            (out as *mut TensorQueryDesc) as u64,
            core::mem::size_of::<TensorQueryDesc>() as u64,
        )
    }
}

#[inline]
pub fn sys_tensor_share(handle: TensorHandle) -> Result<TensorHandle, i64> {
    let r = unsafe { raw3(SYS_TENSOR_SHARE, handle.0 as u64, 0, 0) };
    if r < 0 {
        Err(r)
    } else {
        Ok(TensorHandle(r as u32))
    }
}

#[inline]
pub fn sys_tensor_drop(handle: TensorHandle) -> i64 {
    unsafe { raw3(SYS_TENSOR_DROP, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_model_load(path: &str, flags: u32) -> Result<ModelHandle, i64> {
    let r = unsafe {
        raw3(
            SYS_MODEL_LOAD,
            path.as_ptr() as u64,
            path.len() as u64,
            flags as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(ModelHandle(r as u32))
    }
}

#[inline]
pub fn sys_model_query(handle: ModelHandle, out: &mut ModelQueryDesc) -> i64 {
    unsafe {
        raw3(
            SYS_MODEL_QUERY,
            handle.0 as u64,
            (out as *mut ModelQueryDesc) as u64,
            core::mem::size_of::<ModelQueryDesc>() as u64,
        )
    }
}

#[inline]
pub fn sys_model_drop(handle: ModelHandle) -> i64 {
    unsafe { raw3(SYS_MODEL_DROP, handle.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_infer_run(
    model: ModelHandle,
    inputs: &[TensorHandle],
    outputs: &mut [TensorHandle],
    flags: u32,
) -> Result<usize, i64> {
    let r = unsafe {
        raw6(
            SYS_INFER_RUN,
            model.0 as u64,
            inputs.as_ptr() as u64,
            inputs.len() as u64,
            outputs.as_mut_ptr() as u64,
            outputs.len() as u64,
            flags as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(r as usize) }
}

/// OT-37: autoregressive generation. Runs the proven forward + greedy sampler in
/// the kernel, looping until EOS or `max_new` tokens, and writes the generated
/// token IDs into `out`. Returns the count written. `eos = None` disables EOS
/// stopping (encoded as the u32::MAX sentinel over the ABI).
pub fn sys_generate(
    model: ModelHandle,
    prompt: &[u32],
    out: &mut [u32],
    max_new: u32,
    eos: Option<u32>,
) -> Result<usize, i64> {
    let eos_raw = eos.unwrap_or(u32::MAX);
    let packed = (u64::from(eos_raw) << 32) | u64::from(max_new);
    let r = unsafe {
        raw6(
            SYS_GENERATE,
            model.0 as u64,
            prompt.as_ptr() as u64,
            prompt.len() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            packed,
        )
    };
    if r < 0 { Err(r) } else { Ok(r as usize) }
}

/// Agentic-action constrained generation. The kernel runs the same forward pass
/// as `sys_generate`, but the sampler can only select next tokens that continue
/// one of the caller-provided candidate sequences.
pub fn sys_generate_constrained(
    model: ModelHandle,
    prompt: &[u32],
    out: &mut [u32],
    max_new: u32,
    eos: Option<u32>,
    sequences: &[GenerateConstrainedSequenceDesc],
    tokens: &[u32],
) -> Result<usize, i64> {
    if sequences.len() > u32::MAX as usize || tokens.len() > u32::MAX as usize {
        return Err(EINVAL);
    }
    let desc = GenerateConstrainedDesc {
        max_new,
        eos_token_id: eos.unwrap_or(u32::MAX),
        sequence_count: sequences.len() as u32,
        _reserved0: 0,
        sequences_ptr: sequences.as_ptr() as u64,
        tokens_ptr: tokens.as_ptr() as u64,
        tokens_len: tokens.len() as u32,
        _reserved1: 0,
    };
    let r = unsafe {
        raw6(
            SYS_GENERATE_CONSTRAINED,
            model.0 as u64,
            prompt.as_ptr() as u64,
            prompt.len() as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            (&desc as *const GenerateConstrainedDesc) as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(r as usize) }
}

/// One-token grammar-guided generation. Userland owns the grammar and supplies a
/// little-endian validity bitmask for the current decode step; the kernel only
/// masks logits before greedy sampling.
pub fn sys_generate_masked_step(
    model: ModelHandle,
    prompt: &[u32],
    out: &mut u32,
    mask: &[u8],
    mask_bits: u32,
    flags: u32,
) -> Result<u32, i64> {
    if mask.len() > u32::MAX as usize {
        return Err(EINVAL);
    }
    let desc = GenerateMaskDesc {
        mask_ptr: mask.as_ptr() as u64,
        mask_bits,
        mask_bytes: mask.len() as u32,
        flags,
        _reserved0: 0,
    };
    let r = unsafe {
        raw6(
            SYS_GENERATE_MASKED,
            model.0 as u64,
            prompt.as_ptr() as u64,
            prompt.len() as u64,
            (out as *mut u32) as u64,
            (&desc as *const GenerateMaskDesc) as u64,
            0,
        )
    };
    if r < 0 {
        Err(r)
    } else if r == 1 {
        Ok(*out)
    } else {
        Err(EINVAL)
    }
}

#[inline]
pub fn sys_derive(h: Handle, rights: CapRights) -> Result<Handle, i64> {
    // SAFETY: SYS_DERIVE takes a handle index plus two scalar rights bytes.
    let r = unsafe {
        raw3(
            SYS_DERIVE,
            h.0 as u64,
            rights.meta as u64,
            rights.semantic as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

#[inline]
pub fn sys_derive_process(h: ProcessHandle, rights: CapRights) -> Result<ProcessHandle, i64> {
    let r = unsafe {
        raw3(
            SYS_DERIVE,
            h.0 as u64,
            rights.meta as u64,
            rights.semantic as u64,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(ProcessHandle(r as u32))
    }
}

#[inline]
pub fn sys_revoke(h: Handle) -> i64 {
    // SAFETY: SYS_REVOKE takes one handle index.
    unsafe { raw3(SYS_REVOKE, h.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_grant(channel: Handle, source: Handle, offered: CapRights) -> i64 {
    // SAFETY: SYS_GRANT takes channel/source handle indices and packed rights.
    unsafe {
        raw3(
            SYS_GRANT,
            channel.0 as u64,
            source.0 as u64,
            offered.pack() as u64,
        )
    }
}

#[inline]
pub fn sys_recv_grant(channel: Handle, max_rights: CapRights) -> Result<Handle, i64> {
    // SAFETY: SYS_RECV_GRANT takes a channel handle and packed max rights.
    let r = unsafe {
        raw3(
            SYS_RECV_GRANT,
            channel.0 as u64,
            max_rights.pack() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

// === IPC wrappers (Phase 1.5b) ==========================================

/// Create a fresh anonymous IPC channel. Returns `(h_a, h_b)` — the two
/// endpoint handles installed in the calling process's table. The caller
/// distributes them: typically one is kept and the other is passed to a
/// child via `sys_spawn(name, &[other])`.
///
/// On success the i64 from the kernel encodes both handles packed:
/// low 32 bits = `h_a`, high 32 bits = `h_b`. This wrapper unpacks.
/// Negative return = errno.
#[inline]
pub fn sys_chan_create() -> Result<(Handle, Handle), i64> {
    // SAFETY: SYS_CHAN_CREATE takes no args and only reads kernel state.
    let r = unsafe { raw3(SYS_CHAN_CREATE, 0, 0, 0) };
    if r < 0 {
        return Err(r);
    }
    let packed = r as u64;
    let h_a = Handle((packed & 0xFFFF_FFFF) as u32);
    let h_b = Handle((packed >> 32) as u32);
    Ok((h_a, h_b))
}

/// Send `buf` synchronously on `h`. Blocks until a matching `sys_recv`
/// arrives on the peer endpoint. Returns bytes sent on success, or
/// negative errno (e.g., -32 EPIPE if the peer process exited, -9 EBADF
/// for a stale handle).
#[inline]
pub fn sys_send(h: Handle, buf: &[u8]) -> i64 {
    // SAFETY: buf is a valid slice; kernel validates user-half bounds.
    unsafe { raw3(SYS_SEND, h.0 as u64, buf.as_ptr() as u64, buf.len() as u64) }
}

/// Receive synchronously into `buf` on `h`. Blocks until a matching
/// `sys_send` arrives. Returns bytes received (≤ `buf.len()`) on
/// success. If the sender's payload exceeded `buf.len()`, the return
/// value equals `buf.len()` and the message was truncated — the caller
/// can retry with a larger buffer if it cares (ADR-004 §"Truncation
/// policy"). Negative return = errno.
#[inline]
pub fn sys_recv(h: Handle, buf: &mut [u8]) -> i64 {
    // SAFETY: buf is a valid mutable slice; kernel validates user-half bounds.
    unsafe {
        raw3(
            SYS_RECV,
            h.0 as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// Receive into a raw user virtual address rather than a Rust slice.
///
/// # Safety
/// `buf_user_va`/`len` are passed to the kernel verbatim. The caller
/// intentionally supplies the raw VA — this exists for scenarios that verify
/// kernel-side translation-fault handling without manufacturing an invalid
/// Rust slice, so the VA need not reference valid memory; the kernel handles
/// the fault. For ordinary receives, prefer the slice-based `sys_recv`.
#[inline]
pub unsafe fn sys_recv_raw(h: Handle, buf_user_va: u64, len: usize) -> i64 {
    unsafe { raw3(SYS_RECV, h.0 as u64, buf_user_va, len as u64) }
}

/// Fill `buf` with cryptographically secure random bytes from the kernel
/// CSPRNG DRBG (Phase 10.3). Returns the number of bytes filled on success
/// (always `buf.len()`), or a negative errno on failure (`EFAULT` for bad
/// buffer, `EINVAL` for oversized request, `ENOMEM` if the DRBG is
/// uninitialized).
#[inline]
pub fn sys_getrandom(buf: &mut [u8]) -> i64 {
    unsafe { raw3(SYS_GETRANDOM, buf.as_mut_ptr() as u64, buf.len() as u64, 0) }
}

/// Terminate the calling process with the given exit code. Never returns.
/// The kernel reaps the process on the next yield; its heap-backed kernel
/// stack and Pmap are released.
#[inline]
pub fn sys_exit(code: i32) -> ! {
    // SAFETY: SYS_EXIT cannot fail.
    unsafe { raw3(SYS_EXIT, code as u64, 0, 0) };
    // If the syscall ever returns (it shouldn't — kernel calls sched::exit
    // which doesn't return), spin instead of issuing a syscall storm.
    // `spin_loop()` hints the CPU to back off (PAUSE instruction).
    loop {
        core::hint::spin_loop();
    }
}

// === sys_map_region + GlobalAlloc (Phase 1.5b BUILD-4) ==================

/// Allocate `size` bytes of fresh anonymous user-half memory. Per ADR-006:
/// kernel picks the address (monotonic per-process cursor starting at
/// `0x0000_4000_0000_0000`); pages are zeroed; permissions are R+W (NX).
/// Returns the base pointer on success or a negative errno (`-ENOMEM` on
/// frame-allocator exhaustion or cursor exhaustion, `-EINVAL` for size 0
/// or > 16 MiB).
///
/// No `unmap` in Phase 1.5b — regions persist for the process's
/// lifetime. Process exit frees everything via the kernel's `Pmap::Drop`
/// walker.
#[inline]
pub fn sys_map_region(size: usize) -> Result<*mut u8, i64> {
    // SAFETY: SYS_MAP_REGION takes one arg, returns user VA or errno.
    let r = unsafe { raw3(SYS_MAP_REGION, size as u64, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r as *mut u8) }
}

// ── OT-61 browser C1.0 host-runtime shims (VM map/unmap/protect + clock) ──────

/// C1.0 (P5): map `size` bytes of fresh anonymous memory with POSIX `prot`
/// (`PROT_READ|WRITE|EXEC`; W+X is refused with `-EINVAL` — W^X). `guard=true`
/// leaves an unmapped guard page below the region (a downward overflow faults).
/// Returns the base pointer or a negative errno.
#[inline]
pub fn sys_vm_map(size: usize, prot: u64, guard: bool) -> Result<*mut u8, i64> {
    // SAFETY: SYS_VM_MAP(size, prot, guard) → user VA or errno.
    let r = unsafe { raw3(SYS_VM_MAP, size as u64, prot, guard as u64) };
    if r < 0 { Err(r) } else { Ok(r as *mut u8) }
}

/// C1.0 (P5): unmap `[va, va+size)` (page-granular). Fail-closed: an unmapped
/// page in the range is `-EINVAL`. Returns `Ok(())` or a negative errno.
#[inline]
pub fn sys_vm_unmap(va: *mut u8, size: usize) -> Result<(), i64> {
    // SAFETY: SYS_VM_UNMAP(va, size) → 0 or errno.
    let r = unsafe { raw3(SYS_VM_UNMAP, va as u64, size as u64, 0) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.0 (P5): change the permissions of `[va, va+size)` in place. **W^X**: a
/// request for `PROT_WRITE|PROT_EXEC` is refused (`-EINVAL`). An unmapped page
/// is `-EINVAL`.
#[inline]
pub fn sys_vm_protect(va: *mut u8, size: usize, prot: u64) -> Result<(), i64> {
    // SAFETY: SYS_VM_PROTECT(va, size, prot) → 0 or errno.
    let r = unsafe { raw3(SYS_VM_PROTECT, va as u64, size as u64, prot) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.0 (P4): read a clock in nanoseconds. `CLOCK_MONOTONIC` is nondecreasing
/// (10 ms resolution). Returns the nanosecond value or a negative errno.
#[inline]
pub fn sys_clock_gettime(clock_id: u64) -> Result<u64, i64> {
    // SAFETY: SYS_CLOCK_GETTIME(clock_id) → ns or errno.
    let r = unsafe { raw3(SYS_CLOCK_GETTIME, clock_id, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// `rtc-offset`: confirm `CLOCK_REALTIME` as UTC, once, with the RTC's
/// offset from UTC in seconds (0 when the RTC holds UTC). The input-system
/// authority (the WM) only. Errors: `EPERM`, `EINVAL` (past ±24 h),
/// `EALREADY`, `EAGAIN` (no RTC epoch), `ENOSYS` (kernel without the rung).
#[inline]
pub fn sys_clock_confirm_realtime(offset_s: i64) -> Result<(), i64> {
    // SAFETY: SYS_CLOCK_CONFIRM_REALTIME(offset_s) → 0 or errno; no memory.
    let r = unsafe { raw3(SYS_CLOCK_CONFIRM_REALTIME, offset_s as u64, 0, 0) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// `rtc-offset`: whether `CLOCK_REALTIME` is confirmed UTC (`ENOSYS` from a
/// kernel without the rung).
#[inline]
pub fn sys_clock_realtime_status() -> Result<bool, i64> {
    // SAFETY: SYS_CLOCK_REALTIME_STATUS() → 0 | 1 or errno; no memory.
    let r = unsafe { raw3(SYS_CLOCK_REALTIME_STATUS, 0, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r == 1) }
}

/// C1.1 (P2): thread-info record returned by [`sys_thread_get_info`]
/// (v1, 40 bytes, must match the kernel's `ThreadInfoV1`). Downward-stack
/// semantics: `usable_low..usable_low+usable_len` is the mapped stack; the
/// guard page (when present) is `guard_low..guard_low+guard_len` below it.
#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct ThreadInfoV1 {
    pub tid: u64,
    pub usable_low: u64,
    pub usable_len: u64,
    pub guard_low: u64,
    pub guard_len: u64,
}

/// C1.1 (P1): wait on the 4-aligned futex word at `uaddr` while it still
/// holds `expected`, until woken or the ABSOLUTE monotonic `deadline_ns`
/// passes (`FUTEX_DEADLINE_INFINITE` = no timeout; `0` = already expired).
/// `Ok(())` = woken (spurious wakes permitted — re-check the predicate);
/// `-ETIMEDOUT`; `-EAGAIN` = the word no longer held `expected` at entry.
#[inline]
pub fn sys_futex_wait(uaddr: *const u32, expected: u32, deadline_ns: u64) -> Result<(), i64> {
    // SAFETY: SYS_FUTEX_WAIT(uaddr, expected, deadline_ns) → 0 or errno.
    let r = unsafe { raw3(SYS_FUTEX_WAIT, uaddr as u64, expected as u64, deadline_ns) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.1 (P1): wake up to `count` waiters on the futex word at `uaddr`.
/// Returns the number actually woken (preserved — the rwlock writer→reader
/// fallback consumes it).
#[inline]
pub fn sys_futex_wake(uaddr: *const u32, count: u32) -> Result<u32, i64> {
    // SAFETY: SYS_FUTEX_WAKE(uaddr, count) → #woken or errno.
    let r = unsafe { raw3(SYS_FUTEX_WAKE, uaddr as u64, count as u64, 0) };
    if r < 0 { Err(r) } else { Ok(r as u32) }
}

/// C1.1 test-only: return the number of waiters parked on the futex queue
/// for the key derived from `uaddr`. Ground truth from the kernel queue.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_futex_queue_len(uaddr: *const u32) -> usize {
    let r = unsafe { raw3(SYS_FUTEX_QUEUE_LEN, uaddr as u64, 0, 0) };
    if r < 0 { 0 } else { r as usize }
}

/// C1.1 (P2): create a thread. The KERNEL allocates the new thread's stack
/// (`stack_size` bytes, R+W+NX, an unmapped guard page below when
/// `guard=true`) from this process's address space; `entry(arg)` runs on it
/// in ring 3 and MUST end in [`sys_thread_exit`]. Requires the
/// browser-runtime capability (`-EPERM` without it). Returns the new tid.
/// Prefer [`nthread::spawn`], which wraps the raw contract.
#[inline]
pub fn sys_thread_create(
    entry: extern "C" fn(usize) -> !,
    arg: usize,
    stack_size: usize,
    guard: bool,
) -> Result<u64, i64> {
    // C1.3: the process's first extra thread switches the key layer to FS.base
    // (`ntls::ensure_initial_installed`), so no two threads ever share the
    // initial thread's static block.
    ntls::ensure_initial_installed()?;
    // SAFETY: SYS_THREAD_CREATE(entry, arg, stack_size, guard) → tid or errno.
    let r = unsafe {
        raw4(
            SYS_THREAD_CREATE,
            entry as usize as u64,
            arg as u64,
            stack_size as u64,
            guard as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// C1.3b test-only: [`SYS_TEST_FUTEX_LATE_PARK`]. 0, or -errno (`ENOSYS` on
/// any other kernel).
#[cfg(feature = "neo-test")]
pub fn sys_test_futex_late_park() -> i64 {
    // SAFETY: no arguments.
    unsafe { raw3(SYS_TEST_FUTEX_LATE_PARK, 0, 0, 0) }
}

/// C1.3b test-only: [`SYS_TEST_KERNEL_SPIN`]. 0, or -errno (`ENOSYS` on any
/// other kernel).
#[cfg(feature = "neo-test")]
pub fn sys_test_kernel_spin(ms: u64) -> i64 {
    // SAFETY: one scalar argument.
    unsafe { raw3(SYS_TEST_KERNEL_SPIN, ms, 0, 0) }
}

/// C1.2 (P3): set the calling thread's FS.base to `base` (0 clears it). The
/// thread's key layer ([`ntls`]) finds its control block through it. Requires
/// the browser-runtime capability (`-EPERM`); `-EFAULT` for a kernel-half
/// address. Prefer the `nthread` bootstrap, which installs it.
#[inline]
pub fn sys_thread_set_tls(base: u64) -> Result<(), i64> {
    // SAFETY: SYS_THREAD_SET_TLS(base) → 0 or errno; the kernel never
    // dereferences `base`.
    let r = unsafe { raw3(SYS_THREAD_SET_TLS, base, 0, 0) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.2 test-only: `[cpu, recorded FS.base, live IA32_FS_BASE]` for the
/// calling thread, or the errno.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_tls_probe() -> Result<[u64; 3], i64> {
    let mut out = [0u64; 3];
    // SAFETY: SYS_TEST_TLS(0, out) writes 24 bytes to `out`.
    let r = unsafe { raw3(SYS_TEST_TLS, 0, out.as_mut_ptr() as u64, 0) };
    if r < 0 { Err(r) } else { Ok(out) }
}

/// K1a-0 test-only: arm a `cap_graph_revoke` hook on `h`. Kinds 0, 1 and 2
/// derive from, grant from and spawn-inherit `h` inside the next revoke's
/// window; kind 3 revokes `h` inside the next process teardown's window.
#[cfg(feature = "neo-test")]
pub fn sys_test_cap_graph_race_arm(h: Handle, kind: u32) {
    // SAFETY: SYS_TEST_CAP_GRAPH_RACE(0, h, kind) takes no pointer arguments.
    unsafe { raw3(SYS_TEST_CAP_GRAPH_RACE, 0, h.0 as u64, kind as u64) };
}

/// K1a-0 test-only: the armed operation's result (see the kernel's
/// `cap_graph_race::result`; `i64::MIN` if the hook never ran).
#[cfg(feature = "neo-test")]
pub fn sys_test_cap_graph_race_result() -> i64 {
    // SAFETY: SYS_TEST_CAP_GRAPH_RACE(1) takes no pointer arguments.
    unsafe { raw3(SYS_TEST_CAP_GRAPH_RACE, 1, 0, 0) }
}

/// C1.2 test-only: park the calling thread until [`sys_test_tls_unpark`]
/// wakes it (onto the waker's CPU). Returns the wake value or the errno.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_tls_park() -> i64 {
    // SAFETY: SYS_TEST_TLS(1) blocks the caller; no pointer arguments.
    unsafe { raw3(SYS_TEST_TLS, 1, 0, 0) }
}

/// C1.3b review test-only: `SYS_TEST_TAIL_HOLD(flag)`. Returns (with the
/// kernel having set `*flag = 1` mid-return) once this process is being
/// retired, or after the hold's bound. `threads_exit_group` kernels only.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_tail_hold(flag: &core::sync::atomic::AtomicU32) -> i64 {
    // SAFETY: SYS_TEST_TAIL_HOLD(flag) writes one u32 at `flag`.
    unsafe { raw3(SYS_TEST_TAIL_HOLD, flag.as_ptr() as u64, 0, 0) }
}

/// C1.2 test-only: block in the kernel for a tick and return (on the BSP, a
/// switch to the idle thread and back on the same CPU).
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_tls_sleep() -> i64 {
    // SAFETY: SYS_TEST_TLS(3) takes no pointer arguments.
    unsafe { raw3(SYS_TEST_TLS, 3, 0, 0) }
}

/// C1.2 test-only: wake the parked thread onto this CPU. `true` if one was
/// parked.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_tls_unpark() -> bool {
    // SAFETY: SYS_TEST_TLS(2) takes no pointer arguments.
    (unsafe { raw3(SYS_TEST_TLS, 2, 0, 0) }) == 1
}

/// C1.2 test-only: create a thread (guard page on) that starts on CPU `cpu`.
/// Same contract as [`sys_thread_create`]; `-EINVAL` for a CPU that isn't up.
#[cfg(feature = "neo-test")]
#[inline]
pub fn sys_test_thread_create_on(
    entry: extern "C" fn(usize) -> !,
    arg: usize,
    stack_size: usize,
    cpu: u32,
) -> Result<u64, i64> {
    ntls::ensure_initial_installed()?;
    // SAFETY: SYS_TEST_THREAD_CREATE_ON(entry, arg, stack_size, cpu).
    let r = unsafe {
        raw4(
            SYS_TEST_THREAD_CREATE_ON,
            entry as usize as u64,
            arg as u64,
            stack_size as u64,
            u64::from(cpu),
        )
    };
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// C1.1 (P2): exit the calling thread with `code`, waking its joiner. The
/// process survives until its LAST thread exits.
#[inline]
pub fn sys_thread_exit(code: i32) -> ! {
    // SAFETY: SYS_THREAD_EXIT(code) diverges.
    unsafe {
        raw3(SYS_THREAD_EXIT, code as u32 as u64, 0, 0);
        core::hint::unreachable_unchecked()
    }
}

/// C1.1 (P2): join `tid` (same process), blocking until it exits. Returns
/// its exit code. Single-joiner: a second join, a detached target, or
/// self-join is `-EINVAL`; an unknown/already-reaped tid is `-ESRCH`.
#[inline]
pub fn sys_thread_join(tid: u64) -> Result<i32, i64> {
    let mut code: i32 = 0;
    // SAFETY: SYS_THREAD_JOIN(tid, &code) → 0 or errno; the kernel writes
    // the 4-byte exit code through the validated pointer.
    let r = unsafe { raw3(SYS_THREAD_JOIN, tid, &mut code as *mut i32 as u64, 0) };
    if r < 0 { Err(r) } else { Ok(code) }
}

/// C1.1 (P2): detach `tid` — its exit status is discarded at reap and it can
/// no longer be joined.
#[inline]
pub fn sys_thread_detach(tid: u64) -> Result<(), i64> {
    // SAFETY: SYS_THREAD_DETACH(tid) → 0 or errno.
    let r = unsafe { raw3(SYS_THREAD_DETACH, tid, 0, 0) };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.1 (P2): yield the CPU.
#[inline]
pub fn sys_sched_yield() {
    // SAFETY: SYS_SCHED_YIELD() → 0; no arguments.
    let _ = unsafe { raw3(SYS_SCHED_YIELD, 0, 0, 0) };
}

/// C1.1 (P2): the calling thread's tid.
#[inline]
pub fn sys_thread_self() -> u64 {
    // SAFETY: SYS_THREAD_SELF() → tid; no arguments.
    (unsafe { raw3(SYS_THREAD_SELF, 0, 0, 0) }) as u64
}

/// C1.1 (P2): set the calling thread's (truncated) debug name.
#[inline]
pub fn sys_thread_set_name(name: &str) -> Result<(), i64> {
    let bytes = name.as_bytes();
    // SAFETY: SYS_THREAD_SET_NAME(ptr, len) → 0 or errno; `bytes` outlives
    // the call.
    let r = unsafe {
        raw3(
            SYS_THREAD_SET_NAME,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(()) }
}

/// C1.1 (P2 amendment): query a thread's stack bounds — `tid = 0` for self,
/// or any same-process tid (valid through exit/reap). The SpiderMonkey
/// stack-limit seam (`JS_SetNativeStackQuota` ← `pthread_attr_getstack`)
/// binds here.
#[inline]
pub fn sys_thread_get_info(tid: u64) -> Result<ThreadInfoV1, i64> {
    let mut info = ThreadInfoV1::default();
    // SAFETY: SYS_THREAD_GET_INFO(tid, out, out_len) → 0 or errno; the
    // kernel writes exactly `size_of::<ThreadInfoV1>()` bytes.
    let r = unsafe {
        raw3(
            SYS_THREAD_GET_INFO,
            tid,
            &mut info as *mut ThreadInfoV1 as u64,
            core::mem::size_of::<ThreadInfoV1>() as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(info) }
}

/// Page size assumed by `sys_map_region` (kernel uses 4 KiB throughout).
pub const PAGE_SIZE: usize = 4096;

/// Round `n` up to the next multiple of `PAGE_SIZE`. Used by `LibneoHeap`
/// when sizing its `sys_map_region` requests, and exported for any future
/// libneo consumer that needs the same arithmetic (e.g., a Phase 5 tensor
/// buffer wrapper).
#[inline]
pub const fn page_round_up(n: usize) -> usize {
    (n + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// Initial heap region requested on first allocation. 256 KiB covers the
/// Phase 1.5b workloads (small `Vec<u8>`s, IPC payload buffers) without
/// triggering an immediate extension. Exported so consumers can read the
/// tunable for tests / measurement.
pub const BOOTSTRAP_HEAP_SIZE: usize = 256 * 1024;
/// Minimum extension chunk when the heap fills. We round each growth up
/// to this so we don't issue many tiny `sys_map_region` calls.
pub const HEAP_GROWTH_CHUNK: usize = 64 * 1024;
/// The kernel's per-call `sys_map_region` ceiling (mirror of
/// `MAX_MAP_REGION_SIZE` in the syscall layer). A heap growth larger than this
/// is split into several contiguous maps.
pub const MAX_MAP_REGION_SIZE: usize = 16 * 1024 * 1024;

/// How many separate regions the heap can span (see [`LibneoHeap`]).
pub const MAX_HEAP_ARENAS: usize = 64;

/// Userspace global allocator backing `alloc::*` types (`Box`, `Vec`,
/// `String`, etc.) for any libneo-using binary.
///
/// Lazy bootstrap: the first allocation requests a 256 KiB region from the
/// kernel via `sys_map_region`. When the heap is exhausted it maps another
/// `max(layout.size(), 64 KiB)`-rounded chunk. **The new region is not
/// assumed to abut the heap:** `sys_map_region` shares the process's region
/// cursor with every other mapping (`sys_buffer_map` planes, shared
/// surfaces), so a region mapped after a buffer lands *beyond* it. Extending
/// the heap blindly would hand out memory that aliases that buffer (DI-1b
/// caught exactly this: the WM's first post-boot blur allocation overwrote
/// its own output plane). A region that starts at the last arena's top
/// extends that arena (the old behaviour, unchanged for the contiguous case);
/// any other region starts a new arena, up to [`MAX_HEAP_ARENAS`].
///
/// Every growth that follows another mapping (a buffer map, `sys_vm_map`, a
/// thread stack) costs an arena slot, and slots are never reclaimed; growth
/// is therefore geometric (at least a quarter of the heap so far), so the
/// number of growths — and of arenas — grows with the log of the heap size.
/// Running out of slots is an allocation failure, never an alias.
///
/// The one `spin::Mutex` serializes every arena operation, including the
/// whole map loop, so concurrent threads (`nthread`) are safe.
pub struct LibneoHeap {
    inner: Mutex<Arenas>,
}

struct Arenas {
    heaps: [Heap; MAX_HEAP_ARENAS],
    count: usize,
}

impl Arenas {
    /// Take `region` (`len` bytes, fresh from `sys_map_region`): extend the
    /// last arena if it starts exactly at that arena's top, else open a new
    /// arena. `false` when every arena slot is used.
    ///
    /// # Safety
    /// `region..region + len` must be a fresh mapping owned by this process
    /// for its whole lifetime.
    unsafe fn add_region(&mut self, region: *mut u8, len: usize) -> bool {
        if let Some(last) = self.count.checked_sub(1)
            && self.heaps[last].top() == region
        {
            // SAFETY: the region starts at this arena's top, as `extend`
            // requires, and is owned by the process (caller contract).
            unsafe { self.heaps[last].extend(len) };
            return true;
        }
        if self.count == MAX_HEAP_ARENAS {
            return false;
        }
        // SAFETY: a fresh, exclusively-owned region (caller contract).
        unsafe { self.heaps[self.count].init(region, len) };
        self.count += 1;
        true
    }

    /// Bytes the arenas own.
    fn owned(&self) -> usize {
        self.heaps[..self.count].iter().map(|h| h.size()).sum()
    }

    fn allocate(&mut self, layout: Layout) -> *mut u8 {
        for heap in &mut self.heaps[..self.count] {
            if let Ok(p) = heap.allocate_first_fit(layout) {
                return p.as_ptr();
            }
        }
        ptr::null_mut()
    }
}

impl Default for LibneoHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl LibneoHeap {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(Arenas {
                heaps: [const { Heap::empty() }; MAX_HEAP_ARENAS],
                count: 0,
            }),
        }
    }
}

unsafe impl GlobalAlloc for LibneoHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut h = self.inner.lock();
        // Fast path: an existing arena satisfies the request.
        let p = h.allocate(layout);
        if !p.is_null() {
            return p;
        }
        // Slow path: bootstrap (no arena yet) or grow.
        let need = layout.size().max(HEAP_GROWTH_CHUNK);
        let chunk = if h.count == 0 {
            // First-touch bootstrap. Use the larger of bootstrap size and
            // what we actually need.
            BOOTSTRAP_HEAP_SIZE.max(need)
        } else {
            // Geometric: each growth may cost an arena slot (see the type).
            need.max((h.owned() / 4).min(MAX_MAP_REGION_SIZE))
        };
        // Grow by the request plus a chunk of slack so `allocate_first_fit`
        // always has room for its own bookkeeping (an exact-fit grow can fail
        // by the allocator's metadata otherwise).
        let total = page_round_up(chunk).saturating_add(HEAP_GROWTH_CHUNK);
        // The kernel caps a single `sys_map_region` at `MAX_MAP_REGION_SIZE`
        // (16 MiB), so a large allocation — e.g. Argon2id's 64 MiB memory
        // matrix — is satisfied by mapping several regions. Back-to-back maps
        // abut and extend one arena unless another thread maps in between
        // (then each piece opens its own arena and the request fails cleanly).
        // A partial failure leaves the earlier maps installed (harmless: they
        // just enlarge the heap).
        let mut remaining = total;
        while remaining > 0 {
            let this = remaining.min(MAX_MAP_REGION_SIZE);
            let region = match sys_map_region(this) {
                Ok(p) => p,
                Err(_) => return ptr::null_mut(),
            };
            // SAFETY: `region` is a fresh kernel-allocated user-half range
            // owned exclusively by this process; `this` matches the request.
            if !unsafe { h.add_region(region, this) } {
                return ptr::null_mut();
            }
            remaining -= this;
        }
        // Retry now that the heap can satisfy the request.
        h.allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(p) = NonNull::new(ptr) {
            let mut h = self.inner.lock();
            let count = h.count;
            let addr = ptr as usize;
            if let Some(heap) = h.heaps[..count]
                .iter_mut()
                .find(|heap| (heap.bottom() as usize..heap.top() as usize).contains(&addr))
            {
                // SAFETY: the caller is required by GlobalAlloc to pass a
                // pointer + layout previously returned by `alloc`, and it lies
                // inside this arena.
                unsafe { heap.deallocate(p, layout) };
            }
        }
    }
}

// With `image-decode` (a host-only feature) std is linked and owns the global
// allocator; the kernel-backed `LibneoHeap` would issue a `sys_map_region` at
// the first host allocation and fail. Every OS-target build has the feature
// off, so this is the same allocator as before.
#[cfg(not(feature = "image-decode"))]
#[global_allocator]
static HEAP: LibneoHeap = LibneoHeap::new();

/// Required by Rust when there's a global_allocator. Called when an
/// allocation cannot be satisfied; we surface the failure on stderr (well,
/// stdout — Phase 1.5b's only fd) and exit. Userspace `Vec::push` and
/// friends call this on OOM.
#[cfg(not(feature = "image-decode"))]
#[alloc_error_handler]
fn on_alloc_error(layout: Layout) -> ! {
    const MSG: &[u8] = b"[ libneo ] allocation failed (OOM); exiting\n";
    let _ = sys_write(1, MSG.as_ptr(), MSG.len());
    let _ = layout;
    sys_exit(137)
}

// === Atrium G1 compositor slice-1 wrappers ===

/// `BufferDescV1` wire length (4 x u32 + 1 x u64, little-endian).
pub const BUFFER_DESC_V1_LEN: usize = 24;
/// `PresentResultV1` wire length.
pub const PRESENT_RESULT_V1_LEN: usize = 32;
/// DRM fourcc 'XR24' — the one slice-1 pixel format.
pub const FORMAT_XRGB8888_LE: u32 = 0x3432_5258;

/// Encode a `BufferDescV1` for `sys_buffer_create`.
pub fn encode_buffer_desc(
    width: u32,
    height: u32,
    stride: u32,
    format: u32,
    bytes: u64,
) -> [u8; BUFFER_DESC_V1_LEN] {
    let mut out = [0u8; BUFFER_DESC_V1_LEN];
    out[0..4].copy_from_slice(&width.to_le_bytes());
    out[4..8].copy_from_slice(&height.to_le_bytes());
    out[8..12].copy_from_slice(&stride.to_le_bytes());
    out[12..16].copy_from_slice(&format.to_le_bytes());
    out[16..24].copy_from_slice(&bytes.to_le_bytes());
    out
}

#[inline]
pub fn sys_buffer_create(desc: &[u8; BUFFER_DESC_V1_LEN]) -> Result<Handle, i64> {
    // SAFETY: pointer + exact length of the descriptor wire form.
    let r = unsafe {
        raw3(
            SYS_BUFFER_CREATE,
            desc.as_ptr() as u64,
            desc.len() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

/// `role` 0 = client (RW+NX; needs MAP_WRITE), 1 = compositor (hardware
/// RO+NX; needs MAP_READ). Returns the mapped base VA.
#[inline]
pub fn sys_buffer_map(h: Handle, role: u32) -> Result<*mut u8, i64> {
    // SAFETY: scalar args only.
    let r = unsafe { raw3(SYS_BUFFER_MAP, h.0 as u64, role as u64, 0) };
    if r < 0 {
        Err(r)
    } else {
        Ok(r as u64 as *mut u8)
    }
}

/// Unmap the buffer mapping `sys_buffer_map` returned at `ptr`, releasing
/// its pin (and charge). The caller must hold no reference into it.
#[inline]
pub fn sys_buffer_unmap(ptr: *const u8) -> i64 {
    // SAFETY: scalar arg only; the kernel checks the mapping exists.
    unsafe { raw3(SYS_BUFFER_UNMAP, ptr as u64, 0, 0) }
}

#[inline]
pub fn sys_buffer_query(h: Handle, out: &mut [u8; BUFFER_DESC_V1_LEN]) -> i64 {
    // SAFETY: pointer + exact length of the descriptor wire form.
    unsafe {
        raw3(
            SYS_BUFFER_QUERY,
            h.0 as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

#[inline]
pub fn sys_buffer_commit(h: Handle) -> i64 {
    // SAFETY: scalar arg only.
    unsafe { raw3(SYS_BUFFER_COMMIT, h.0 as u64, 0, 0) }
}

pub const DAMAGE_RECT_V1_LEN: usize = 8;

pub fn encode_damage_rect(x: u16, y: u16, width: u16, height: u16) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&x.to_le_bytes());
    out[2..4].copy_from_slice(&y.to_le_bytes());
    out[4..6].copy_from_slice(&width.to_le_bytes());
    out[6..8].copy_from_slice(&height.to_le_bytes());
    out
}

#[inline]
pub fn sys_buffer_commit_damage(h: Handle, rect: &[u8; DAMAGE_RECT_V1_LEN]) -> i64 {
    unsafe {
        raw3(
            SYS_BUFFER_COMMIT_DAMAGE,
            h.0 as u64,
            rect.as_ptr() as u64,
            rect.len() as u64,
        )
    }
}

/// Composite the buffer at `(dst_x, dst_y)`; returns the present epoch. The
/// buffer must carry `PRESENT`; when scanout enforcement is armed, the calling
/// process must independently hold a `GpuSubmitCap`.
#[inline]
pub fn sys_fb_present(h: Handle, dst_x: usize, dst_y: usize) -> i64 {
    // SAFETY: scalar args only.
    unsafe { raw3(SYS_FB_PRESENT, h.0 as u64, dst_x as u64, dst_y as u64) }
}

/// Submit the sole trusted G1 glass-blur manifest.  `src` and `dst` are typed
/// `GpuBufferCap` handles (READ and WRITE respectively); `queue` is the scarce
/// `GpuQueueCap`.  A successful return is a new `GpuFenceCap` handle, not a
/// display epoch.  To scan out the resulting G1 surface, the compositor must
/// separately call [`sys_fb_present`] and satisfy `GpuSubmitCap` enforcement.
#[inline]
pub fn sys_gpu_compute_submit_blur(src: Handle, dst: Handle, queue: Handle) -> Result<Handle, i64> {
    // SAFETY: scalar handles only; no client pointer or GPU address enters this ABI.
    let r = unsafe {
        raw3(
            SYS_GPU_COMPUTE_SUBMIT_BLUR,
            src.0 as u64,
            dst.0 as u64,
            queue.0 as u64,
        )
    };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

/// Query the completed sequence of a `GpuFenceCap`.  The v1 glass service
/// returns its cap only after the bounded compute wait resolves, so this is a
/// completion/right-separation witness; it cannot submit compute or present.
#[inline]
pub fn sys_gpu_compute_fence_wait(fence: Handle) -> i64 {
    // SAFETY: scalar handle only.
    unsafe { raw3(SYS_GPU_COMPUTE_FENCE_WAIT, fence.0 as u64, 0, 0) }
}

/// Bounded wait for present `epoch`; fills `out` with `PresentResultV1`
/// (present_id u64, outcome u32, flags u32, submit_tick u64, complete_tick
/// u64 — little-endian). `timeout_ticks` 0 = non-blocking probe.
#[inline]
pub fn sys_present_fence_wait(
    h: Handle,
    epoch: u64,
    out: &mut [u8; PRESENT_RESULT_V1_LEN],
    timeout_ticks: u64,
) -> i64 {
    // SAFETY: pointer + exact length of the result wire form.
    unsafe {
        raw6(
            SYS_PRESENT_FENCE_WAIT,
            h.0 as u64,
            epoch,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            timeout_ticks,
            0,
        )
    }
}

#[inline]
pub fn sys_input_focus_create() -> Result<Handle, i64> {
    // SAFETY: no args.
    let r = unsafe { raw3(SYS_INPUT_FOCUS_CREATE, 0, 0, 0) };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

#[inline]
pub fn sys_input_focus_activate(h: Handle) -> i64 {
    // SAFETY: scalar arg only.
    unsafe { raw3(SYS_INPUT_FOCUS_ACTIVATE, h.0 as u64, 0, 0) }
}

#[inline]
pub fn sys_input_focus_mint_activator(h: Handle) -> Result<Handle, i64> {
    let r = unsafe { raw3(SYS_INPUT_FOCUS_MINT_ACTIVATOR, h.0 as u64, 0, 0) };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

#[inline]
pub fn sys_input_focus_test_inject(keycode: u8, modifiers: u8) -> i64 {
    unsafe {
        raw3(
            SYS_INPUT_FOCUS_TEST_INJECT,
            keycode as u64,
            modifiers as u64,
            0,
        )
    }
}

/// Desktop S1: `(width, height)` of the scanout output. EPERM unless the
/// caller holds the GpuSubmit capability; ENOSYS without `desktop-session`.
#[inline]
pub fn sys_fb_output_query() -> Result<(usize, usize), i64> {
    // SAFETY: no args.
    let r = unsafe { raw3(SYS_FB_OUTPUT_QUERY, 0, 0, 0) };
    if r < 0 {
        Err(r)
    } else {
        Ok(unpack_output_size(r as u64))
    }
}

/// Inverse of the kernel's `pack_output_size`: width in bits 63..32, height
/// in bits 31..0.
pub const fn unpack_output_size(packed: u64) -> (usize, usize) {
    ((packed >> 32) as usize, (packed & 0xFFFF_FFFF) as usize)
}

#[inline]
pub fn sys_input_focus_deactivate(h: Handle) -> i64 {
    // SAFETY: scalar arg only.
    unsafe { raw3(SYS_INPUT_FOCUS_DEACTIVATE, h.0 as u64, 0, 0) }
}

/// 0 iff this handle's focus object is the active input route.
#[inline]
pub fn sys_input_focus_poll(h: Handle) -> i64 {
    // SAFETY: scalar arg only.
    unsafe { raw3(SYS_INPUT_FOCUS_POLL, h.0 as u64, 0, 0) }
}

pub fn sys_input_focus_recv(h: Handle, out: &mut [u8; INPUT_EVENT_V1_LEN]) -> i64 {
    unsafe {
        raw3(
            SYS_INPUT_FOCUS_RECV,
            h.0 as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
        )
    }
}

/// DI-1a: claim the pointer route (`reclaim`: replace a live one, the WM
/// restart path). Returns its RECEIVE-only root.
#[inline]
pub fn sys_input_pointer_claim(reclaim: bool) -> Result<Handle, i64> {
    // SAFETY: scalar arg only.
    let r = unsafe { raw3(SYS_INPUT_POINTER_CLAIM, reclaim as u64, 0, 0) };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

/// DI-1a: forward pointer record `tag` to `target` (an ACTIVATE handle) at
/// surface-local `(x, y)`, or drop it with `target = None`. Exactly once.
#[inline]
pub fn sys_input_pointer_forward(
    route: Handle,
    tag: u16,
    target: Option<Handle>,
    x: i32,
    y: i32,
) -> i64 {
    let target = target.map_or(u32::MAX, |h| h.0);
    // SAFETY: scalar args only.
    unsafe {
        raw6(
            SYS_INPUT_POINTER_FORWARD,
            route.0 as u64,
            tag as u64,
            target as u64,
            x as i64 as u64,
            y as i64 as u64,
            0,
        )
    }
}

/// DI-1a: the pending SAK chord bits ([`SAK_TAB`] | [`SAK_SPACE`] |
/// [`SAK_ESCAPE`]), cleared; negative errno on failure.
#[inline]
pub fn sys_input_sak_take() -> i64 {
    // SAFETY: no args.
    unsafe { raw3(SYS_INPUT_SAK_TAKE, 0, 0, 0) }
}

/// OT-66 L1 test-only (see [`SYS_TEST_BLOCK_CUT`]): `op` 0 returns the
/// boot's cut step; 1 arms the kernel's block write/flush cut at it and
/// returns it; 2 disarms and returns the ops counted since arming.
#[inline]
pub fn sys_test_block_cut(op: u64) -> i64 {
    // SAFETY: one integer argument.
    unsafe { raw3(SYS_TEST_BLOCK_CUT, op, 0, 0) }
}

/// OT-22 test-only: the BSP's idle ticks since boot (see
/// [`SYS_TEST_IDLE_TICKS`]).
#[inline]
pub fn sys_test_idle_ticks() -> i64 {
    // SAFETY: no args.
    unsafe { raw3(SYS_TEST_IDLE_TICKS, 0, 0, 0) }
}

/// OT-22: park until an item is ready (returns the ready mask, bit i = item
/// i) or `deadline_ns` (absolute, `CLOCK_MONOTONIC`; `u64::MAX` = none)
/// passes (`ETIMEDOUT`). Level-triggered: then do the ordinary non-blocking
/// reads. `EAGAIN` means the kernel could not park now (poll once).
#[inline]
pub fn sys_wait_any(items: &[WaitItem], deadline_ns: u64) -> i64 {
    // SAFETY: `items` is a valid slice of 8-byte repr(C) records; the kernel
    // validates the range and bounds the count.
    unsafe {
        raw3(
            SYS_WAIT_ANY,
            items.as_ptr() as u64,
            items.len() as u64,
            deadline_ns,
        )
    }
}

/// `sys_send` without parking: EAGAIN unless a receiver is already waiting.
#[inline]
pub fn sys_try_send(h: Handle, buf: &[u8]) -> i64 {
    // SAFETY: buf is a valid slice; the kernel validates bounds.
    unsafe {
        raw3(
            SYS_TRY_SEND,
            h.0 as u64,
            buf.as_ptr() as u64,
            buf.len() as u64,
        )
    }
}

/// `sys_grant` without parking: EAGAIN unless a `sys_recv_grant` is waiting.
#[inline]
pub fn sys_try_grant(channel: Handle, source: Handle, offered: CapRights) -> i64 {
    // SAFETY: SYS_TRY_GRANT takes channel/source handle indices and packed rights.
    unsafe {
        raw3(
            SYS_TRY_GRANT,
            channel.0 as u64,
            source.0 as u64,
            offered.pack() as u64,
        )
    }
}

/// `sys_recv_grant` without parking: the new handle, or EAGAIN unless a
/// grantor is already waiting (as `Err`).
#[inline]
pub fn sys_try_recv_grant(channel: Handle, max_rights: CapRights) -> Result<Handle, i64> {
    // SAFETY: SYS_TRY_RECV_GRANT takes a channel handle index and packed rights.
    let r = unsafe {
        raw3(
            SYS_TRY_RECV_GRANT,
            channel.0 as u64,
            max_rights.pack() as u64,
            0,
        )
    };
    if r < 0 { Err(r) } else { Ok(Handle(r as u32)) }
}

/// `sys_recv` without parking: EAGAIN when no sender is waiting.
#[inline]
pub fn sys_try_recv(h: Handle, buf: &mut [u8]) -> i64 {
    // SAFETY: buf is a valid mutable slice; the kernel validates bounds.
    unsafe {
        raw3(
            SYS_TRY_RECV,
            h.0 as u64,
            buf.as_mut_ptr() as u64,
            buf.len() as u64,
        )
    }
}

pub fn decode_input_event(bytes: [u8; INPUT_EVENT_V1_LEN]) -> InputEventV1 {
    InputEventV1 {
        kind: bytes[0],
        flags: bytes[1],
        code: u16::from_le_bytes([bytes[2], bytes[3]]),
        x: i16::from_le_bytes([bytes[4], bytes[5]]),
        y: i16::from_le_bytes([bytes[6], bytes[7]]),
    }
}

/// Atrium buffer semantic-rights bits (mirror of the kernel's
/// `atrium::logic::buffer_rights`).
pub const ATRIUM_MAP_WRITE: u8 = 1 << 0;
pub const ATRIUM_MAP_READ: u8 = 1 << 1;
pub const ATRIUM_QUERY: u8 = 1 << 2;
pub const ATRIUM_PRESENT: u8 = 1 << 3;
pub const ATRIUM_COMMIT: u8 = 1 << 4;

/// Atrium input-focus semantic-rights bits (mirror of
/// `atrium::logic::focus_rights`).
pub const ATRIUM_FOCUS_ACTIVATE: u8 = 1 << 0;
pub const ATRIUM_FOCUS_RECEIVE: u8 = 1 << 1;

/// OT-66 L2: the lock chord's bit from [`sys_input_sak_take`] (Super+L, the
/// kernel's `sak-lock`), beside [`SAK_TAB`], [`SAK_SPACE`] and [`SAK_ESCAPE`].
/// At the end of the file so the lines above, and the panic locations of a
/// build that doesn't use it, stay as they were.
pub const SAK_LOCK: u8 = 8;
/// OT-66 L2: set in [`sys_input_sak_take`]'s bits when any key or pointer
/// event was produced since the last take (the kernel's `sak-lock`): the WM's
/// idle timer restarts on it. Not a chord.
pub const SAK_ACTIVITY: u8 = 0x10;

/// OT-66 L2b: a lock op of [`SYS_INPUT_SAK_TAKE`] (kernel `sak-lock-route`;
/// op 0 is the take itself). 1: register `h` (the WM's own activator) as the
/// lock focus; 2: lock now; 3: unlock, `h` the route from then on. A lock op
/// returns [`SAK_LOCK_OK`]; a kernel without them takes the latch instead and
/// returns its bits (`0..=0xff`).
pub fn sys_input_sak_op(op: u64, h: Handle) -> i64 {
    // SAFETY: scalar args.
    unsafe { raw3(SYS_INPUT_SAK_TAKE, op, u64::from(h.0), 0) }
}

/// A lock op's success ([`sys_input_sak_op`]).
pub const SAK_LOCK_OK: i64 = 0x100;
