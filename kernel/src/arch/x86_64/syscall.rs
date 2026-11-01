// SPDX-License-Identifier: MPL-2.0
//! SYSCALL entry path. Phase 1.4.
//!
//! Userspace issues syscalls via the SYSCALL instruction. CPU behavior:
//!   - CS = STAR[47:32], SS = STAR[47:32] + 8 (kernel selectors)
//!   - RIP = LSTAR
//!   - RCX = saved user RIP, R11 = saved user RFLAGS
//!   - RFLAGS &= ~SFMASK  (we mask IF, TF, AC, DF, NT; see `init`)
//!   - DOES NOT switch RSP — we do it ourselves via swapgs+per-CPU.
//!
//! Return path: **IRETQ** (not SYSRETQ). SYSRETQ on Intel skips a canonical
//! check on RCX → CVE-2012-0217. IRETQ costs ~30 extra cycles, which
//! Phase 1.4 doesn't care about. IRETQ restores neither RCX nor R11, so the
//! stub reloads them with what SYSRETQ would leave (user RIP, user RFLAGS)
//! instead of handing ring 3 kernel scratch (CP-43).
//!
//! Hardcoded selector values matching the GDT slot layout in `gdt.rs`:
//!   slot 4: user_data    → selector 0x23 (= 4<<3 | 3)
//!   slot 5: user_code_64 → selector 0x2B (= 5<<3 | 3)
//! `gdt.rs` runtime-asserts these values match the actual selectors.

use core::arch::global_asm;
use core::cmp::min;
#[cfg(any(
    feature = "neo-test-inference-preemptible-liveness",
    feature = "neo-test-arthur-real-brain-boot-qwen35"
))]
use core::sync::atomic::AtomicBool;
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;

use x86_64::PrivilegeLevel;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::Segment;
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, Msr, SFMask, Star};
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::CS;

use crate::ai::exec;
use crate::ai::model::{self, ModelQueryDesc};
use crate::ai::tensor::{self, TensorCreateDesc, TensorQueryDesc};
#[cfg(feature = "serial-stdin")]
use crate::drivers::console::serial::try_read_byte as try_read_serial_byte;
use crate::drivers::console::serial::write_bytes;
#[cfg(feature = "usb-hid")]
use crate::drivers::usb::hid_keyboard;
use crate::errno::{
    E2BIG, EACCES, EAGAIN, EBADF, EBUSY, EEXIST, EFAULT, EINVAL, EISDIR, ENOENT, ENOMEM, ENOSYS,
    ENOTCONN, ENOTDIR, EPERM, ESTALE,
};
use crate::fs::{self, FileRights, InodeKind, MountFlags, OpenFile, VfsDirEntry, VfsError};
use crate::net::dns::{self, DnsError};
use crate::net::happy_eyeballs::{self, DialError};
use crate::net::socket::{self, Socket, SocketError};
use crate::net::tcp::{self, TcpError, TcpListener, TcpSocket};
use crate::net::tls_record;
use crate::observability::KernelObservabilityV1;
use crate::sched::process::{
    self, CapRights, Handle, InheritedHandle, channel_rights, meta_rights, model_rights,
    process_rights, socket_rights,
};
use crate::serial_println;

use super::gdt::{
    kernel_code_selector, kernel_data_selector, user_code_64_selector, user_data_selector,
};
use super::per_cpu;

#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
mod qwen35_llama_cpp_golden {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/qwen35_llama_cpp_hello_golden.rs"
    ));
}

#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
static QWEN35_GENERATION_ORACLE_PASSED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
const QWEN35_HELLO_ORACLE: [u32; 32] = qwen35_llama_cpp_golden::LLAMA_CPP_QWEN35_HELLO_GREEDY_32;

#[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
pub(crate) fn qwen35_generation_oracle_passed() -> bool {
    QWEN35_GENERATION_ORACLE_PASSED.load(Ordering::Acquire)
}

// === Public syscall numbers (mirror in libneo) ===
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
/// OT-67 W1: a file's whole content replaced in one commit.
#[cfg(feature = "neofs-file-replace")]
pub const SYS_FILE_REPLACE: u64 = 24;
// OT-61 browser C1 host-runtime block (0x40..0x4F). C1.0 uses the LOW end
// (VM + clock); 0x44..0x4F is RESERVED for C1.1/C1.2 (futex/thread/TLS) so a
// later cross-check amending those shapes cannot renumber C1.0.
// The C1 surface exists only under `browser-runtime` (default OFF ⇒ the numbers
// return ENOSYS and these consts don't exist — the kill-switch keeps the default
// kernel bit-for-bit pre-C1). libneo defines the mirror consts unconditionally.
/// C1.0 (P5): map anonymous user memory with POSIX prot bits + optional guard.
#[cfg(feature = "browser-runtime")]
pub const SYS_VM_MAP: u64 = 0x40;
/// C1.0 (P5): unmap a user range (page-granular, fail-closed).
#[cfg(feature = "browser-runtime")]
pub const SYS_VM_UNMAP: u64 = 0x41;
/// C1.0 (P5): change permissions in place (W^X-enforced — no W+X).
#[cfg(feature = "browser-runtime")]
pub const SYS_VM_PROTECT: u64 = 0x42;
/// C1.0 (P4): monotonic/realtime clock in nanoseconds.
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
pub const SYS_CLOCK_GETTIME: u64 = 0x43;
// OT-61 C1.1 (P1+P2): futex + thread lifecycle (0x44..0x4D). 0x4E..0x4F stay
// RESERVED for C1.2 (SYS_THREAD_SET_TLS + spare) — do not renumber.
#[cfg(feature = "browser-runtime")]
pub const SYS_FUTEX_WAIT: u64 = 0x44;
#[cfg(feature = "browser-runtime")]
pub const SYS_FUTEX_WAKE: u64 = 0x45;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_CREATE: u64 = 0x46;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_EXIT: u64 = 0x47;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_JOIN: u64 = 0x48;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_DETACH: u64 = 0x49;
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
pub const SYS_SCHED_YIELD: u64 = 0x4A;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_SELF: u64 = 0x4B;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_SET_NAME: u64 = 0x4C;
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_GET_INFO: u64 = 0x4D;
/// C1.2 (P3): set the calling thread's FS.base (the `arch_prctl(ARCH_SET_FS)`
/// analogue). Takes the spare 0x4F, so the test-only probe below keeps 0x4E.
#[cfg(feature = "browser-runtime")]
pub const SYS_THREAD_SET_TLS: u64 = 0x4F;
/// OT-61 C2.1: the caller's own pid (the process pid on any of its threads).
/// A process holds no handle to itself, so `SYS_INSPECT` cannot answer this.
/// No arguments; grants nothing (a pid is not a capability).
#[cfg(any(feature = "browser-runtime", feature = "cap-rooted-open"))]
pub const SYS_PROCESS_SELF: u64 = 0x50;
/// OT-61 C2.1: `SYS_HANDLE_INFO(idx, out, len)`: the kind and rights of the
/// caller's own handle `idx` as a 16-byte record (`sched::handle_info_logic`).
/// Reads only the caller's table; grants nothing.
#[cfg(any(
    feature = "browser-runtime",
    feature = "cap-rooted-open",
    feature = "installer"
))]
pub const SYS_HANDLE_INFO: u64 = 0x51;
/// OT-61 C2.2: `SYS_SPAWN_BUDGETED(name, len, handles, n, budget_bytes)`:
/// `SYS_SPAWN` whose child gets its own memory account of `budget_bytes`
/// (rounded up to frames), nested under the caller's. Budgets only narrow,
/// so no capability is needed.
#[cfg(feature = "mem-quota")]
pub const SYS_SPAWN_BUDGETED: u64 = 0x52;
/// OT-61 C2.2: `SYS_MEM_INFO(out, 32)`: the caller's memory account as a
/// 32-byte record (`mm::mem_account_logic`). Reads only; grants nothing.
#[cfg(feature = "mem-quota")]
pub const SYS_MEM_INFO: u64 = 0x53;
/// OT-61 C2.2b: `SYS_SPAWN_BUDGETED2(name, len, handles, n, frame_bytes,
/// obj_bytes)`: `SYS_SPAWN_BUDGETED` with an object budget too (kernel-heap
/// bytes of declared weight, I-18). `frame_bytes` 0 means no frame budget of
/// its own; `obj_bytes` 0 is `EINVAL`. A new number because
/// `SYS_SPAWN_BUDGETED` never checked its sixth argument, so a non-zero value
/// there is silently ignored today.
#[cfg(feature = "obj-quota")]
pub const SYS_SPAWN_BUDGETED2: u64 = 0x54;
// C1.1 test-only: queue-length probe (borrows 0x4E from C1.2 reserved slot).
// Must NOT ship in production — test-only discriminator for the threads_futex barrier.
#[cfg(all(feature = "browser-runtime", feature = "neo-test"))]
pub const SYS_FUTEX_QUEUE_LEN: u64 = 0x4E;
/// CP-41 test-only probe: run kernel float work under the float state the
/// syscall entry established and report it. Exists only in the
/// `syscall_mxcsr_entry` scenario builds; never ships.
#[cfg(feature = "neo-test-syscall-mxcsr-entry")]
pub const SYS_TEST_FLOAT_PROBE: u64 = 0xF00;

/// POSIX-canonical protection bits for `SYS_VM_MAP`/`SYS_VM_PROTECT` — the
/// values `std::sys` and SpiderMonkey pass to `mmap`/`mprotect`.
#[cfg(feature = "browser-runtime")]
pub const PROT_READ: u64 = 1;
#[cfg(feature = "browser-runtime")]
pub const PROT_WRITE: u64 = 2;
#[cfg(feature = "browser-runtime")]
pub const PROT_EXEC: u64 = 4;
/// Clock ids for `SYS_CLOCK_GETTIME`.
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
pub const CLOCK_MONOTONIC: u64 = 1;
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
pub const CLOCK_REALTIME: u64 = 2;

/// AI subsystem syscall range reserved by Bible Article 6.1: 0x100..0x17F.
pub const SYS_TENSOR_CREATE: u64 = 0x100;
pub const SYS_TENSOR_QUERY: u64 = 0x101;
pub const SYS_TENSOR_SHARE: u64 = 0x102;
pub const SYS_TENSOR_DROP: u64 = 0x103;
/// Mutate a tensor's bytes (Phase 7 BUILD-3). The op boundary that enforces the
/// `tensor_rights::WRITE` capability bit: a read-only grantee is denied here. Scalar
/// ABI (handle, offset, user ptr, len) — no struct layout, so no byte-oracle.
pub const SYS_TENSOR_WRITE: u64 = 0x104;
pub const SYS_MODEL_LOAD: u64 = 0x110;
pub const SYS_MODEL_QUERY: u64 = 0x111;
pub const SYS_MODEL_DROP: u64 = 0x112;
/// Phase 5 inference sub-block: 0x120..0x12F.
pub const SYS_INFER_RUN: u64 = 0x120;
/// OT-37: autoregressive generation (forward + greedy sample + EOS-stop loop).
/// Composes the proven forward (qwen_inventory / graph exec) + the OT-30 greedy
/// sampler; the boot uses it so Arthur emits real tokens instead of immediate EOS.
pub const SYS_GENERATE: u64 = 0x121;
/// Phase 8 A1: grammar-guided generation. Same forward as `SYS_GENERATE`, but
/// the sampler masks logits to caller-provided valid candidate AST continuations.
pub const SYS_GENERATE_CONSTRAINED: u64 = 0x122;
/// Arthur A1 read/nav: one-step grammar-guided generation. Userland owns the
/// grammar/policy and supplies a per-token validity bitmask; the kernel only
/// applies it mechanically at the sampler boundary.
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
/// Phase 10.3 BUILD-10.3: userland randomness via SYS_GETRANDOM.
/// Fills a userland buffer from the kernel CSPRNG DRBG (10.1).
pub const SYS_GETRANDOM: u64 = 0x190;
/// Atrium G1 compositor range: 0x1A0..0x1AF (Rung 0 slice-1). Gated by the
/// default-off `atrium-compositor-g1` feature AND the runtime switch — off
/// means the dispatch arms are not compiled and every number is ENOSYS (the
/// kill-switch). Numbers are mirrored in libneo unconditionally.
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_CREATE: u64 = 0x1A0;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_MAP: u64 = 0x1A1;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_QUERY: u64 = 0x1A2;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_COMMIT: u64 = 0x1A3;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_FB_PRESENT: u64 = 0x1A4;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_COMMIT_DAMAGE: u64 = 0x1AD;
/// GPU P1 Slice 6: submit the sole trusted glass-blur manifest.  There is
/// deliberately no neighbouring `*_CREATE`: Queue/Buffer roots are privileged
/// kernel grants, and a successful submit alone mints a wait/query-only fence.
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_GPU_COMPUTE_SUBMIT_BLUR: u64 = 0x1AE;
/// Query/await one completed compute fence.  This number cannot submit or
/// present; scanout remains exclusively in `SYS_FB_PRESENT`.
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_GPU_COMPUTE_FENCE_WAIT: u64 = 0x1AF;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_PRESENT_FENCE_WAIT: u64 = 0x1A5;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_CREATE: u64 = 0x1A6;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_ACTIVATE: u64 = 0x1A7;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_DEACTIVATE: u64 = 0x1A8;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_POLL: u64 = 0x1A9;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_RECV: u64 = 0x1AA;
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_INPUT_FOCUS_MINT_ACTIVATOR: u64 = 0x1AB;
#[cfg(all(
    feature = "atrium-compositor-g1",
    any(
        feature = "neo-test-atrium-input-routing",
        feature = "neo-test-atrium-interactive",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "neo-test-product-integration"
    )
))]
pub const SYS_INPUT_FOCUS_TEST_INJECT: u64 = 0x1AC;
/// Desktop S1: report the scanout output size to the GpuSubmit holder (the
/// session compositor) so it can compose a full-screen scene for any GOP mode.
/// Compiled only with `desktop-session` (the kill-switch). Outside the
/// 0x1A0..0x1AF block, which is full.
#[cfg(feature = "desktop-session")]
pub const SYS_FB_OUTPUT_QUERY: u64 = 0x1B0;
/// DI-1a B-ref (THINK §8.3): claim the pointer route (flags bit 0 =
/// reclaim). Returns its RECEIVE-only root. Input-system authority only.
#[cfg(feature = "atrium-pointer-forward")]
pub const SYS_INPUT_POINTER_CLAIM: u64 = 0x1B1;
/// DI-1a B-ref: consume pointer record `tag` exactly once:
/// `(route, tag, target, x, y)`; `target == u32::MAX` drops it.
#[cfg(feature = "atrium-pointer-forward")]
pub const SYS_INPUT_POINTER_FORWARD: u64 = 0x1B2;
/// DI-1a: take (and clear) the pending SAK chord bits.
#[cfg(feature = "atrium-pointer-forward")]
pub const SYS_INPUT_SAK_TAKE: u64 = 0x1B3;
/// DI-1a (THINK §8.5): `SYS_RECV` that returns EAGAIN instead of parking
/// when no sender is waiting, so the single-threaded WM can poll its
/// surface channel and the pointer route in one loop.
#[cfg(feature = "atrium-pointer-forward")]
pub const SYS_TRY_RECV: u64 = 0x1B4;
/// OT-22 rung 1: `(items, n, deadline_monotonic_ns)` → ready mask (bit i =
/// item i) or ETIMEDOUT. Each item is 8 bytes: `u32 handle, u32 kind`
/// (`WAIT_KIND_*`). Parks until an item is ready or the deadline passes.
#[cfg(feature = "wait-multiplex")]
pub const SYS_WAIT_ANY: u64 = 0x1B5;
/// Chad #177 item 4: `(va)` → 0. Unmap the borrowed buffer mapping
/// `SYS_BUFFER_MAP` returned at `va`, releasing its pin and charge, so a
/// long-lived compositor can let a closed client's buffer go. EINVAL if no
/// buffer mapping starts at `va`.
#[cfg(feature = "atrium-compositor-g1")]
pub const SYS_BUFFER_UNMAP: u64 = 0x1B6;
/// OT-87 R4.4: the installer's disk list (`install_ui_logic`). Gated by the
/// `Installer` cap; ENOSYS without `installer`.
#[cfg(feature = "installer")]
pub const SYS_INSTALL_LIST: u64 = 0x1C0;
/// OT-87 R4.4: install to a listed disk (`install_ui_logic`). Gated by the
/// `Installer` cap; ENOSYS without `installer`.
#[cfg(feature = "installer")]
pub const SYS_INSTALL_RUN: u64 = 0x1C1;
#[cfg(feature = "wait-multiplex")]
pub(crate) use crate::sched::wait_any_logic::{WAIT_KIND_CHANNEL, WAIT_KIND_INPUT, WAIT_KIND_SAK};
/// CP-43 test-only: return with a kernel address in both `rcx` and `r11`, so
/// `syscall_return_clean_regs` can prove the entry stub reloads both. Ordinary
/// syscall routes rarely touch `r11` (LLVM allocates it last), which would
/// leave that half of the check vacuous. The registers are dirtied by
/// [`syscall_return_test_hook`] in the stub itself, so no compiler-generated
/// code runs between the dirtying and the reload. Exists only in that
/// scenario's builds; never ships.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
pub const SYS_TEST_DIRTY_SCRATCH: u64 = 0xF01;
/// CP-50 test-only: start a one-shot LAPIC timer interrupt `a0` bus cycles out
/// (`a1` = the probe's pass, for bookkeeping), then return with IF=1, as
/// `SYS_SPAWN` does after `spawn_user`. Sweeping `a0` walks the interrupt
/// across the syscall exit one instruction at a time, so
/// `syscall_exit_irq_window` can land one between the exit `swapgs` and
/// `iretq`. Exists only in that scenario's builds; never ships.
#[cfg(feature = "neo-test-syscall-exit-irq-window")]
pub const SYS_TEST_IRQ_WINDOW: u64 = 0xF02;
/// OT-22 test-only: the BSP's idle-thread ticks since boot, for the
/// desktop idle witness (`desktop_idle_parks` / `desktop_idle_spins`).
#[cfg(any(feature = "neo-test-desktop-idle", feature = "neo-test-login-cut"))]
pub const SYS_TEST_IDLE_TICKS: u64 = 0xF03;
/// OT-61 C1.2 test-only: `SYS_TEST_TLS(op, out)` for `threads_tls`.
/// - op 0 (probe): write `{cpu, recorded FS.base, live IA32_FS_BASE}` for the
///   calling thread to the 24-byte buffer at `out`, so the battery can check
///   that FS.base survives a context switch and a migration;
/// - op 1 (park): block the caller in a one-slot test mailbox;
/// - op 2 (unpark): wake the parked thread onto the caller's CPU with the IPC
///   handoff's migrating wake; returns 1, or 0 if nothing is parked yet.
///
/// Exists only in that scenario's builds; never ships.
#[cfg(feature = "neo-test-threads-tls")]
pub const SYS_TEST_TLS: u64 = 0xF04;
/// OT-61 C1.2 test-only: `SYS_THREAD_CREATE(entry, arg, stack_size, cpu)` with
/// the guard page on, whose new thread starts on CPU `cpu` instead of the
/// caller's. Nothing in userland otherwise puts a thread on another CPU, so this
/// is how `threads_tls` gets a waker there. Same capability gate. Exists only in
/// that scenario's builds; never ships.
#[cfg(any(
    feature = "neo-test-threads-tls",
    feature = "neo-test-threads-exit-group"
))]
pub const SYS_TEST_THREAD_CREATE_ON: u64 = 0xF05;
/// OT-61 C1.3b test-only: arm the calling thread so its next `SYS_FUTEX_WAIT`
/// waits, before taking the futex queue lock, until its process is being
/// retired (plus a few ticks, so the retire's purge has run). That parks it
/// after the purge: the late parker CP-86's futex refusal must catch.
/// `threads_exit_group` kernels only; never ships.
#[cfg(feature = "neo-test-threads-exit-group")]
pub const SYS_TEST_FUTEX_LATE_PARK: u64 = 0xF06;
/// OT-61 C1.3b test-only: `SYS_TEST_KERNEL_SPIN(ms)` spins in ring 0, with
/// interrupts on and no lock held, for about `ms` milliseconds (whole 10 ms
/// ticks, at most 2 s), then returns 0. A reschedule IPI that lands meanwhile
/// interrupts ring 0, so only the syscall-return check can retire the thread.
/// `threads_exit_group` kernels only; never ships.
#[cfg(feature = "neo-test-threads-exit-group")]
pub const SYS_TEST_KERNEL_SPIN: u64 = 0xF07;
/// OT-61 C1.3b review test-only: `SYS_TEST_TAIL_HOLD(flag)` arms the caller
/// and returns with interrupts ON, as `spawn_user` does. In its syscall
/// return, after the final retire check, the armed thread sets the user
/// `u32` at `flag` to 1 and holds (IF as the tail has it) until its process
/// is being retired. `threads_exit_group` kernels only; never ships.
#[cfg(feature = "neo-test-threads-exit-group")]
pub const SYS_TEST_TAIL_HOLD: u64 = 0xF09;
/// K1a-0 test-only: `SYS_TEST_CAP_GRAPH_RACE(op, h, kind)`. Op 0 arms a hook
/// (`process::cap_graph_race`): kinds 0-2 make the next revoke derive from,
/// grant from, or spawn-inherit the caller's handle `h` between its walk and
/// its removal; kind 3 makes the next process teardown revoke `h` between
/// its edges leaving and the re-parent. Op 1 returns the armed operation's
/// result. `cap_graph_revoke` kernels only; never ships.
#[cfg(feature = "neo-test-cap-graph-revoke")]
pub const SYS_TEST_CAP_GRAPH_RACE: u64 = 0xF08;

#[repr(C)]
#[derive(Copy, Clone)]
struct GenerateConstrainedDesc {
    max_new: u32,
    eos_token_id: u32,
    sequence_count: u32,
    _reserved0: u32,
    sequences_ptr: u64,
    tokens_ptr: u64,
    tokens_len: u32,
    _reserved1: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct GenerateMaskDesc {
    mask_ptr: u64,
    mask_bits: u32,
    mask_bytes: u32,
    flags: u32,
    _reserved0: u32,
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
const GENERATE_MASK_FLAG_DISABLED: u32 = 1;
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
const GENERATE_MASK_KNOWN_FLAGS: u32 = GENERATE_MASK_FLAG_DISABLED;

#[derive(Copy, Clone)]
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
struct GenerateConstrainedSequenceDesc {
    offset: u32,
    len: u32,
}

pub const PROCESS_SNAPSHOT_V1_VERSION: u32 = 1;
pub const PROCESS_SNAPSHOT_NAME_LEN: usize = 32;
#[cfg(feature = "kernel-sysstat")]
pub const SYSSTAT_V1_VERSION: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone)]
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
    /// — the direct I-14(a) handle-set witness (ADR-075 §2.4).
    pub network_handle_count: u32,
    pub name_len: u32,
    pub name: [u8; PROCESS_SNAPSHOT_NAME_LEN],
}

// I-12 byte-oracle for the SYS_INSPECT result ABI: a hand-computed layout
// asserted independently of the writer (kernel) and reader (libneo). Eight
// leading u32 fields (32 B) + name_len (4 B) + a 32-byte name = 68 B, no
// padding (every field is u32-aligned). Adding/reordering a field without
// updating BOTH this assert and the libneo mirror is a compile error.
const _: () = {
    assert!(core::mem::size_of::<ProcessSnapshotV1>() == 68);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, network_handle_count) == 28);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, name_len) == 32);
    assert!(core::mem::offset_of!(ProcessSnapshotV1, name) == 36);
};

#[repr(C)]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
pub struct NetStatsV1 {
    pub packets_in: u64,
    pub packets_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

// === Init ===

/// Wire up BSP SYSCALL/SYSRET MSRs and IA32_KERNEL_GSBASE.
///
/// Call exactly once during `arch::init_traps_bsp()`, AFTER `gdt::init_bsp()`
/// (selectors must exist).
pub fn init_bsp() {
    per_cpu::init_bsp();
    write_syscall_msrs();
    runtime_assert_gdt_selectors();
}

/// Wire up AP-local SYSCALL/SYSRET MSRs.
///
/// `per_cpu::init_ap()` has already wired this CPU's GS-base before this is
/// called from the AP entry path. Sub-commit 1a defines this for 1b.
pub fn init_ap() {
    write_syscall_msrs();
}

#[allow(dead_code)]
pub fn init() {
    init_bsp();
}

fn write_syscall_msrs() {
    // EFER.SCE = 1 enables the SYSCALL/SYSRET instructions.
    // SAFETY: writing model-specific registers during CPU-local trap init.
    unsafe {
        Efer::update(|f| f.insert(EferFlags::SYSTEM_CALL_EXTENSIONS));
    }

    // STAR — see Intel SDM Vol 2 SYSCALL/SYSRET. Even though we use IRETQ
    // for return, STAR.user_base must be valid (some debugger tools read it).
    Star::write(
        user_code_64_selector(),
        user_data_selector(),
        kernel_code_selector(),
        kernel_data_selector(),
    )
    .expect("STAR write failed: GDT slot ordering invariant broken");

    // LSTAR = entry RIP that SYSCALL transfers to.
    LStar::write(VirtAddr::new(syscall_entry as *const () as u64));

    // CSTAR = the entry for SYSCALL from compatibility mode (AMD; Intel #UDs).
    // Ring 3 has no 32-bit code segment to get there (gdt.rs, CP-48), but
    // left at 0 it would run ring 0 at RIP 0 on the user's RSP. Point it at a
    // sink that halts without touching the stack (SFMASK has cleared IF).
    // SAFETY: CPU-local trap init; the sink is kernel text.
    unsafe {
        Msr::new(IA32_CSTAR).write(compat_syscall_sink as *const () as u64);
    }

    // SYSENTER_CS = 0. On Intel SYSENTER is legal from 64-bit ring 3 and
    // enters ring 0 at SYSENTER_EIP/ESP unless SYSENTER_CS[15:2] is 0, which
    // makes it #GP(0); don't rely on firmware leaving the reset value (AMD
    // #UDs SYSENTER in long mode).
    // SAFETY: CPU-local trap init; every x86-64 CPU has the SYSENTER MSRs.
    unsafe {
        Msr::new(IA32_SYSENTER_CS).write(0);
    }

    // SFMASK = bits CLEARED in RFLAGS on SYSCALL entry:
    //   IF (9)  — interrupts off until kernel stack established
    //   TF (8)  — no single-step into kernel
    //   AC (18) — no alignment check during kernel exec
    //   DF (10) — SysV requires DF=0 on function entry
    //   NT (14) — ring 3 can set it with popfq, and IRETQ with NT=1 is #GP(0)
    //             in 64-bit mode, so the syscall's own iretq would fault and
    //             a blocked syscall would hand NT=1 to the next thread (CP-47)
    SFMask::write(
        RFlags::INTERRUPT_FLAG
            | RFlags::TRAP_FLAG
            | RFlags::ALIGNMENT_CHECK
            | RFlags::DIRECTION_FLAG
            | RFlags::NESTED_TASK,
    );

    serial_println!(
        "[ kernel ] syscall: SYSCALL/IRETQ enabled (LSTAR={:?}, SFMASK=IF|TF|AC|DF|NT)",
        VirtAddr::new(syscall_entry as *const () as u64)
    );
}

/// IA32_CSTAR: SYSCALL target RIP for compatibility mode.
const IA32_CSTAR: u32 = 0xC000_0083;
const IA32_SYSENTER_CS: u32 = 0x174;

// A compatibility-mode SYSCALL arrives in ring 0 with the user's RSP and GS
// (no stack switch) and IF=0 (SFMASK). The sink needs no stack: no
// instruction here reads or writes memory, so a user-chosen RSP is never used.
// NMI runs on its IST stack. Parking the CPU is fail-closed, not contained: it
// stays a TLB-shootdown target, so the next shootdown panics the kernel.
global_asm!(
    r#"
.global compat_syscall_sink
.type compat_syscall_sink, @function
compat_syscall_sink:
    cli
1:
    hlt
    jmp 1b
.size compat_syscall_sink, . - compat_syscall_sink
"#
);

unsafe extern "C" {
    fn compat_syscall_sink();
}

/// The address IA32_CSTAR is programmed with (CP-48).
#[cfg(feature = "neo-test-compat-mode-unreachable")]
pub fn compat_syscall_sink_addr() -> u64 {
    compat_syscall_sink as *const () as u64
}

fn runtime_assert_gdt_selectors() {
    // Runtime assertion that GDT layout matches the hardcoded selectors in
    // the asm. If gdt.rs is reordered without updating this file, fail loud.
    let u_data = user_data_selector().0;
    let u_code = user_code_64_selector().0;
    assert_eq!(
        u_data, 0x23,
        "user_data selector changed; update syscall.rs asm SS_USER constant"
    );
    assert_eq!(
        u_code, 0x2B,
        "user_code_64 selector changed; update syscall.rs asm CS_USER constant"
    );
}

// === Asm entry path ===
//
// Stack we BUILD on kernel stack (top→bottom in push order; lowest pushed
// last is at lowest addr):
//
//    +128: user SS  (constant 0x23)
//    +120: user RSP (saved from per-CPU)
//    +112: user RFLAGS (R11)
//    +104: user CS  (constant 0x2B)
//    +96 : user RIP (RCX)
//    +88 : saved RBX
//    +80 : saved RBP
//    +72 : saved R12
//    +64 : saved R13
//    +56 : saved R14
//    +48 : saved R15
//    +40 : saved RDI    ← arg 0 from user
//    +32 : saved RSI    ← arg 1
//    +24 : saved RDX    ← arg 2
//    +16 : saved R10    ← arg 3
//    +8  : saved R8     (Phase 1.4 unused)
//    +0  : saved R9     (Phase 1.4 unused)  ← RBX after `mov rbx, rsp` points here
//
// Then: align RSP, reserve the FPU save area (legacy FXSAVE or feature-on
// CPUID-sized XSAVE), save FPU state, load the default MXCSR.
// Call syscall_dispatch(num=rax, a0=[rbx+40], a1=[rbx+32], a2=[rbx+24], a3=[rbx+16]).
// On return: restore FPU state, mov rsp,rbx, pop GP regs, reload rcx/r11 from
// the IRETQ frame, cli, swapgs, iretq.

/// CP-43 discriminator hook, assembled into both `syscall_entry` variants only
/// in `syscall_return_clean_regs` builds; everywhere else it expands to nothing
/// and the stubs are unchanged. It sits after the GP pops, so after the last
/// compiler-generated instruction of the syscall. If `SYS_TEST_DIRTY_SCRATCH`
/// armed [`SYSCALL_TEST_DIRTY_SCRATCH`], it records this return's user RIP and
/// loads the armed kernel address into `rcx` and `r11`. The reload below is
/// then the only thing that can keep that address from ring 3, whatever the
/// compiler emits for the syscall body.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
macro_rules! syscall_return_test_hook {
    () => {
        r#"
    # CP-43 test hook (syscall_return_clean_regs builds only).
    cmp qword ptr [rip + SYSCALL_TEST_DIRTY_SCRATCH], 0
    je 2f
    mov rcx, [rsp]
    mov qword ptr [rip + SYSCALL_TEST_DIRTY_SCRATCH_RIP], rcx
    mov rcx, qword ptr [rip + SYSCALL_TEST_DIRTY_SCRATCH]
    mov r11, rcx
    mov qword ptr [rip + SYSCALL_TEST_DIRTY_SCRATCH], 0
2:
"#
    };
}

#[cfg(not(feature = "neo-test-syscall-return-clean-regs"))]
macro_rules! syscall_return_test_hook {
    () => {
        ""
    };
}

/// Armed by `SYS_TEST_DIRTY_SCRATCH` with a kernel address; consumed (and
/// zeroed) by [`syscall_return_test_hook`] on that syscall's return. The probe
/// is the only ring-3 code in the scenario, so no other return can take it.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
#[unsafe(no_mangle)]
static SYSCALL_TEST_DIRTY_SCRATCH: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// The user RIP of the return the hook dirtied; 0 until it fires.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
#[unsafe(no_mangle)]
static SYSCALL_TEST_DIRTY_SCRATCH_RIP: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// The user RIP whose return the CP-43 hook loaded kernel scratch into, or 0
/// if it never fired.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
pub fn syscall_test_dirty_scratch_rip() -> u64 {
    SYSCALL_TEST_DIRTY_SCRATCH_RIP.load(Ordering::Acquire)
}

#[cfg(not(feature = "xsave-fpu"))]
global_asm!(concat!(
    r#"
.global syscall_entry
.type syscall_entry, @function
syscall_entry:
    # Establish kernel GS so gs:[0..16] hits PER_CPU.
    swapgs
    mov gs:[8], rsp                 # save user RSP to per-CPU scratch
    mov rsp, gs:[0]                 # load kernel RSP

    # Build IRETQ frame on kernel stack, top-down.
    push 0x23                       # user SS
    push qword ptr gs:[8]           # user RSP (from per-CPU scratch)
    push r11                        # user RFLAGS
    push 0x2B                       # user CS
    push rcx                        # user RIP

    # Save user GP state (callee-saved + arg regs) so we can restore on iretq.
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    push rdi
    push rsi
    push rdx
    push r10
    push r8
    push r9

    # Anchor pre-align RSP via RBX. RBX's user value was just pushed; clobbering
    # the live RBX register is fine.
    mov rbx, rsp
    and rsp, 0xFFFFFFFFFFFFFFF0     # align RSP down to 16
    sub rsp, 512                    # FXSAVE area
    fxsave [rsp]
    # The Rust body is SSE code and must not run on ring 3's MXCSR (CP-41).
    # Load the default; fxrstor puts the user's back. No fninit: the kernel
    # has no x87 code, and fninit would add ~70 cycles to every syscall.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]

    # Marshal args: SysV expects RDI, RSI, RDX, RCX, R8, R9, then stack.
    mov rdi, rax                    # num
    mov rsi, [rbx + 40]             # a0  (saved user RDI)
    mov rdx, [rbx + 32]             # a1  (saved user RSI)
    mov rcx, [rbx + 24]             # a2  (saved user RDX)
    mov r8,  [rbx + 16]             # a3  (saved user R10)
    mov r9,  [rbx + 8]              # a4  (saved user R8)
    sub rsp, 16
    mov rax, [rbx + 0]              # a5  (saved user R9)
    mov [rsp], rax

    call syscall_dispatch
    add rsp, 16
    # RAX = return value. fxrstor doesn't touch RAX so it survives.

    fxrstor [rsp]
    mov rsp, rbx                    # undo align

    pop r9
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    # RSP now points at the IRETQ frame (RIP word at [rsp+0]).
"#,
    syscall_return_test_hook!(),
    r#"
    # SYSCALL destroyed user rcx/r11 and IRETQ restores neither, so they still
    # hold kernel scratch (CP-43). Give ring 3 what SYSRET would: rcx = return
    # RIP, r11 = RFLAGS. RAX is the return value; leave it.
    mov rcx, [rsp]                  # user RIP
    mov r11, [rsp + 16]             # user RFLAGS

    # Some syscalls return with IF=1 (`spawn_user` re-enables interrupts). An
    # interrupt taken after the swapgs would arrive at CPL 0 on the user GS,
    # so its stub would not swap and its per-CPU reads would fault (CP-50).
    # iretq restores the user's IF from the frame.
    cli
    # Labels only, no code: `syscall_exit_irq_window` locates the window.
.global syscall_exit_swapgs
syscall_exit_swapgs:
    swapgs                          # restore user GS
.global syscall_exit_iretq
syscall_exit_iretq:
    iretq
.size syscall_entry, . - syscall_entry
"#
));

#[cfg(feature = "xsave-fpu")]
global_asm!(concat!(
    r#"
.global syscall_entry
.type syscall_entry, @function
syscall_entry:
    # Establish kernel GS so gs:[0..16] hits PER_CPU.
    swapgs
    mov gs:[8], rsp                 # save user RSP to per-CPU scratch
    mov rsp, gs:[0]                 # load kernel RSP

    # Build IRETQ frame on kernel stack, top-down.
    push 0x23                       # user SS
    push qword ptr gs:[8]           # user RSP (from per-CPU scratch)
    push r11                        # user RFLAGS
    push 0x2B                       # user CS
    push rcx                        # user RIP

    # Save user GP state (callee-saved + arg regs) so we can restore on iretq.
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    push rdi
    push rsi
    push rdx
    push r10
    push r8
    push r9

    # Anchor pre-align RSP via RBX. RBX's user value was just pushed; clobbering
    # the live RBX register is fine.
    mov rbx, rsp
    mov r15, rax                    # preserve syscall number across FPU save
    and rsp, 0xFFFFFFFFFFFFFFC0     # XSAVE requires 64-byte alignment
    sub rsp, qword ptr [rip + FPU_SAVE_AREA_SIZE]
    mov rdi, rsp
    call fpu_save_area_asm
    # The Rust body is SSE code and must not run on ring 3's MXCSR (CP-41).
    # Load the default; the restore puts the user's back. No fninit: the kernel
    # has no x87 code, and fninit would add ~70 cycles to every syscall.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]

    # Marshal args: SysV expects RDI, RSI, RDX, RCX, R8, R9, then stack.
    mov rdi, r15                    # num
    mov rsi, [rbx + 40]             # a0  (saved user RDI)
    mov rdx, [rbx + 32]             # a1  (saved user RSI)
    mov rcx, [rbx + 24]             # a2  (saved user RDX)
    mov r8,  [rbx + 16]             # a3  (saved user R10)
    mov r9,  [rbx + 8]              # a4  (saved user R8)
    sub rsp, 16
    mov rax, [rbx + 0]              # a5  (saved user R9)
    mov [rsp], rax

    call syscall_dispatch
    add rsp, 16

    mov r15, rax                    # preserve return value across FPU restore
    mov rdi, rsp
    call fpu_restore_area_asm
    mov rax, r15
    mov rsp, rbx                    # undo align

    pop r9
    pop r8
    pop r10
    pop rdx
    pop rsi
    pop rdi
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    # RSP now points at the IRETQ frame (RIP word at [rsp+0]).
"#,
    syscall_return_test_hook!(),
    r#"
    # SYSCALL destroyed user rcx/r11 and IRETQ restores neither, so they still
    # hold kernel scratch (CP-43). Give ring 3 what SYSRET would: rcx = return
    # RIP, r11 = RFLAGS. RAX is the return value; leave it.
    mov rcx, [rsp]                  # user RIP
    mov r11, [rsp + 16]             # user RFLAGS

    # Some syscalls return with IF=1 (`spawn_user` re-enables interrupts). An
    # interrupt taken after the swapgs would arrive at CPL 0 on the user GS,
    # so its stub would not swap and its per-CPU reads would fault (CP-50).
    # iretq restores the user's IF from the frame.
    cli
    # Labels only, no code: `syscall_exit_irq_window` locates the window.
.global syscall_exit_swapgs
syscall_exit_swapgs:
    swapgs                          # restore user GS
.global syscall_exit_iretq
syscall_exit_iretq:
    iretq
.size syscall_entry, . - syscall_entry
"#
));

unsafe extern "C" {
    pub fn syscall_entry();
}

// === Rust dispatcher ===

#[cfg(feature = "neo-test-syscall-nt-masked")]
static SYSCALL_NT_LIVE: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

/// Syscalls dispatched with RFLAGS.NT set in the kernel (must be 0, CP-47).
#[cfg(feature = "neo-test-syscall-nt-masked")]
pub fn syscall_nt_live_for_test() -> u64 {
    SYSCALL_NT_LIVE.load(Ordering::Relaxed)
}

/// First entry into Rust from a syscall. Verifies CPL=3 on first call (per
/// ISC-34a) and dispatches to the implementation.
#[unsafe(no_mangle)]
extern "C" fn syscall_dispatch(
    num: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> i64 {
    debug_assert!(
        CS::get_reg().rpl() == PrivilegeLevel::Ring0,
        "syscall_dispatch entered without kernel CS"
    );
    // Clears the handoff preemption credit and marks the thread as inside a
    // syscall (OT-93); a thread of a force-killed process ends here.
    crate::sched::enter_syscall();
    crate::observability::record_syscall(num);
    #[cfg(feature = "freeze-watchdog")]
    let watchdog_pid = crate::watchdog::note_syscall(num);

    #[cfg(feature = "neo-test-syscall-nt-masked")]
    if x86_64::registers::rflags::read_raw() & RFlags::NESTED_TASK.bits() != 0 {
        SYSCALL_NT_LIVE.fetch_add(1, Ordering::Relaxed);
    }

    // OT-61 C1.3b (CP-86): a thread of a process being retired (a fault kill
    // or an exit group on another thread) ends here, at a ring-3 boundary
    // where it holds nothing, instead of running the syscall.
    #[cfg(feature = "browser-runtime")]
    crate::sched::process::retire_current_if_dying();

    let ret = match num {
        SYS_EXIT => sys_exit_impl(a0 as i32),
        SYS_WRITE => sys_write_impl(a0 as u32, a1 as *const u8, a2 as usize),
        SYS_SPAWN => sys_spawn_impl(
            a0 as *const u8,
            a1 as usize,
            a2 as *const u32,
            a3 as usize,
            #[cfg(feature = "mem-quota")]
            None,
            #[cfg(feature = "obj-quota")]
            None,
        ),
        SYS_CHAN_CREATE => sys_chan_create_impl(),
        SYS_SEND => sys_send_impl(a0 as u32, a1, a2 as usize),
        SYS_RECV => sys_recv_impl(a0 as u32, a1, a2 as usize),
        SYS_MAP_REGION => sys_map_region_impl(a0 as usize),
        // OT-61 browser C1.0 host-runtime primitives (VM + clock). Gated by
        // `browser-runtime` (default OFF ⇒ these numbers return ENOSYS and the
        // kernel is bit-for-bit pre-C1 — the Rule-1 kill-switch).
        #[cfg(feature = "browser-runtime")]
        SYS_VM_MAP => sys_vm_map_impl(a0 as usize, a1, a2 != 0),
        #[cfg(feature = "browser-runtime")]
        SYS_VM_UNMAP => sys_vm_unmap_impl(a0, a1 as usize),
        #[cfg(feature = "browser-runtime")]
        SYS_VM_PROTECT => sys_vm_protect_impl(a0, a1 as usize, a2),
        // Also the DI-1a pointer WM's: its polled LaunchCap proof has a
        // deadline (Janus DS-1b) and yields while it waits. DM-1: and the
        // desktop session's, whose transitions are paced to 60 Hz.
        #[cfg(any(
            feature = "browser-runtime",
            feature = "atrium-pointer-forward",
            feature = "desktop-session"
        ))]
        SYS_CLOCK_GETTIME => sys_clock_gettime_impl(a0),
        // OT-61 C1.1: futex + threads (same kill-switch: default OFF ⇒ ENOSYS).
        #[cfg(feature = "browser-runtime")]
        SYS_FUTEX_WAIT => sys_futex_wait_impl(a0, a1 as u32, a2),
        #[cfg(feature = "browser-runtime")]
        SYS_FUTEX_WAKE => sys_futex_wake_impl(a0, a1 as u32),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_CREATE => sys_thread_create_impl(a0, a1, a2 as usize, a3 != 0),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_EXIT => sys_thread_exit_impl(a0 as i32),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_JOIN => sys_thread_join_impl(a0, a1 as *mut i32),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_DETACH => sys_thread_detach_impl(a0),
        #[cfg(any(
            feature = "browser-runtime",
            feature = "atrium-pointer-forward",
            feature = "desktop-session"
        ))]
        SYS_SCHED_YIELD => sys_sched_yield_impl(),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_SELF => sys_thread_self_impl(),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_SET_NAME => sys_thread_set_name_impl(a0 as *const u8, a1 as usize),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_GET_INFO => sys_thread_get_info_impl(a0, a1 as *mut ThreadInfoV1, a2 as usize),
        #[cfg(feature = "browser-runtime")]
        SYS_THREAD_SET_TLS => sys_thread_set_tls_impl(a0),
        #[cfg(any(feature = "browser-runtime", feature = "cap-rooted-open"))]
        SYS_PROCESS_SELF => sys_process_self_impl(),
        #[cfg(any(
            feature = "browser-runtime",
            feature = "cap-rooted-open",
            feature = "installer"
        ))]
        SYS_HANDLE_INFO => sys_handle_info_impl(a0, a1, a2 as usize),
        #[cfg(feature = "mem-quota")]
        SYS_SPAWN_BUDGETED => sys_spawn_budgeted_impl(
            a0 as *const u8,
            a1 as usize,
            a2 as *const u32,
            a3 as usize,
            a4,
        ),
        #[cfg(feature = "obj-quota")]
        SYS_SPAWN_BUDGETED2 => sys_spawn_budgeted2_impl(
            a0 as *const u8,
            a1 as usize,
            a2 as *const u32,
            a3 as usize,
            a4,
            a5,
        ),
        #[cfg(feature = "mem-quota")]
        SYS_MEM_INFO => sys_mem_info_impl(a0, a1 as usize),
        #[cfg(all(feature = "browser-runtime", feature = "neo-test"))]
        SYS_FUTEX_QUEUE_LEN => sys_futex_queue_len_impl(a0),
        #[cfg(feature = "neo-test-syscall-mxcsr-entry")]
        SYS_TEST_FLOAT_PROBE => sys_test_float_probe_impl(a0 as *mut u8, a1 as usize),
        SYS_KILL => sys_kill_impl(a0 as u32),
        SYS_DERIVE => sys_derive_impl(a0 as u32, a1 as u8, a2 as u8),
        SYS_REVOKE => sys_revoke_impl(a0 as u32),
        SYS_GRANT => sys_grant_impl(a0 as u32, a1 as u32, a2 as u16),
        SYS_RECV_GRANT => sys_recv_grant_impl(a0 as u32, a1 as u16),
        SYS_OPEN => {
            let fd = sys_open_impl(a0 as *const u8, a1 as usize, a2 as u32, a3 as u32);
            // The startup timeline: Arthur's vocab read starts here. The open
            // succeeded, so the path was validated and copied once already.
            #[cfg(feature = "freeze-watchdog")]
            if fd >= 0
                && copy_user_bytes(a0 as *const u8, a1 as usize)
                    .is_ok_and(|path| path.windows(7).any(|w| w == b"/vocab/"))
            {
                crate::watchdog::note_vocab_open(watchdog_pid, fd as u64);
            }
            fd
        }
        SYS_READ => sys_read_impl(a0 as u32, a1 as *mut u8, a2 as usize),
        SYS_CLOSE => {
            #[cfg(feature = "freeze-watchdog")]
            crate::watchdog::note_close(watchdog_pid, u64::from(a0 as u32));
            sys_close_impl(a0 as u32)
        }
        SYS_FSYNC => sys_fsync_impl(a0 as u32),
        SYS_READDIR => sys_readdir_impl(a0 as u32, a1 as *mut u8, a2 as usize, a3 as *mut u64),
        SYS_MOUNT => sys_mount_impl(a0 as u32, a1 as *const u8, a2 as usize, a3 as u32),
        SYS_UMOUNT => sys_umount_impl(a0 as u32, a1 as *const u8, a2 as usize, a3 as u32),
        SYS_WAIT => sys_wait_impl(a0 as u32, a1),
        SYS_INSPECT => sys_inspect_impl(a0 as u32, a1 as *mut ProcessSnapshotV1, a2 as usize),
        SYS_SYSSTAT => sys_sysstat_impl(a0 as *mut SysStatV1, a1 as usize),
        SYS_OBSERVE => sys_observe_impl(a0 as *mut KernelObservabilityV1, a1 as usize),
        SYS_FTRUNCATE => sys_ftruncate_impl(a0 as u32, a1),
        #[cfg(feature = "neofs-file-replace")]
        SYS_FILE_REPLACE => sys_file_replace_impl(a0 as u32, a1 as *const u8, a2 as usize),
        SYS_TENSOR_CREATE => sys_tensor_create_impl(a0 as *const u8, a1 as usize),
        SYS_TENSOR_QUERY => sys_tensor_query_impl(a0 as u32, a1 as *mut u8, a2 as usize),
        SYS_TENSOR_SHARE => sys_tensor_share_impl(a0 as u32),
        SYS_TENSOR_DROP => sys_tensor_drop_impl(a0 as u32),
        SYS_TENSOR_WRITE => sys_tensor_write_impl(a0 as u32, a1, a2 as *const u8, a3 as usize),
        SYS_MODEL_LOAD => {
            let handle = sys_model_load_impl(a0 as *const u8, a1 as usize, a2 as u32);
            #[cfg(feature = "freeze-watchdog")]
            if handle >= 0 {
                crate::watchdog::note_model_handle();
            }
            handle
        }
        SYS_MODEL_QUERY => sys_model_query_impl(a0 as u32, a1 as *mut u8, a2 as usize),
        SYS_MODEL_DROP => sys_model_drop_impl(a0 as u32),
        // B1c's lifecycle marker covers only the inventory route, applied
        // inside once the route is known (B1c re-check #2, PM ruling); the
        // graph route's admission section is CP-66's, with its own boundary.
        SYS_INFER_RUN => sys_infer_run_syscall(
            a0 as u32,
            a1 as *const u32,
            a2 as u32,
            a3 as *mut u32,
            a4 as u32,
            a5 as u32,
        ),
        SYS_GENERATE => inference_syscall(|| {
            sys_generate_impl(
                a0 as u32,
                a1 as *const u32,
                a2 as u32,
                a3 as *mut u32,
                a4 as u32,
                a5,
            )
        }),
        SYS_GENERATE_CONSTRAINED => inference_syscall(|| {
            sys_generate_constrained_impl(
                a0 as u32,
                a1 as *const u32,
                a2 as u32,
                a3 as *mut u32,
                a4 as u32,
                a5 as *const GenerateConstrainedDesc,
            )
        }),
        SYS_GENERATE_MASKED => inference_syscall(|| {
            sys_generate_masked_impl(
                a0 as u32,
                a1 as *const u32,
                a2 as u32,
                a3 as *mut u32,
                a4 as *const GenerateMaskDesc,
            )
        }),
        // Paired with net_dial_cap_logic::authority_allows: cfg-off is the
        // ambient pre-feature path; cfg-on closes legacy egress and requires
        // matching authority at SYS_NET_DIAL/SYS_NET_LISTEN_TCP.
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_SOCKET_CREATE => sys_socket_create_impl(a0 as u32, a1 as u32),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_SOCKET_BIND => sys_socket_bind_impl(a0 as u32, a1 as *const u8, a2 as usize),
        SYS_SOCKET_SEND => sys_socket_send_impl(a0 as u32, a1 as *const u8, a2 as usize, a3 as u32),
        SYS_SOCKET_RECV => sys_socket_recv_impl(a0 as u32, a1 as *mut u8, a2 as usize, a3 as u32),
        SYS_SOCKET_CLOSE => sys_socket_close_impl(a0 as u32),
        SYS_NET_STATS => sys_net_stats_impl(a0 as u32, a1 as *mut NetStatsV1, a2 as usize),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_SOCKET_SENDTO => sys_socket_sendto_impl(
            a0 as u32,
            a1 as *const u8,
            a2 as usize,
            a3 as u32,
            a4 as *const u8,
            a5 as usize,
        ),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_SOCKET_RECVFROM => sys_socket_recvfrom_impl(
            a0 as u32,
            a1 as *mut u8,
            a2 as usize,
            a3 as u32,
            a4 as *mut u8,
            a5 as *mut usize,
        ),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_NET_CONNECT_TCP => sys_net_connect_tcp_impl(a0 as *const u8, a1 as usize, a2 as u16),
        #[cfg(feature = "net-dial-cap")]
        SYS_NET_LISTEN_TCP => {
            sys_net_listen_tcp_impl(a0 as *const u8, a1 as usize, a2 as usize, a3 as u32)
        }
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_NET_LISTEN_TCP => sys_net_listen_tcp_impl(a0 as *const u8, a1 as usize, a2 as usize),
        SYS_NET_ACCEPT_TCP => sys_net_accept_tcp_impl(a0 as u32),
        SYS_NET_SHUTDOWN_TCP => sys_net_shutdown_tcp_impl(a0 as u32, a1 as u32),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_NET_RESOLVE => sys_net_resolve_impl(
            a0 as *const u8,
            a1 as usize,
            a2 as u32,
            a3 as *mut dns::NetCandidate,
            a4 as usize,
        ),
        #[cfg(feature = "net-dial-cap")]
        SYS_SOCKET_CREATE | SYS_SOCKET_BIND | SYS_SOCKET_SENDTO | SYS_SOCKET_RECVFROM
        | SYS_NET_CONNECT_TCP | SYS_NET_RESOLVE => ENOSYS,
        #[cfg(feature = "net-dial-cap")]
        SYS_NET_DIAL => sys_net_dial_impl(a0 as *const u8, a1 as usize, a2 as u16, a3 as u32),
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_NET_DIAL => sys_net_dial_impl(a0 as *const u8, a1 as usize, a2 as u16),
        SYS_NET_TCP_ENABLE_TLS => {
            sys_net_tcp_enable_tls_impl(a0 as u32, a1 as *mut u8, a2 as usize)
        }
        #[cfg(feature = "net-dial-cap")]
        SYS_NET_DIAL_CAP_DERIVE => {
            sys_net_dial_cap_derive_impl(a0 as u32, a1 as *const u8, a2 as usize, a3 as u16)
        }
        #[cfg(not(feature = "net-dial-cap"))]
        SYS_NET_DIAL_CAP_DERIVE => ENOSYS,
        SYS_GETRANDOM => sys_getrandom_impl(a0 as *mut u8, a1 as usize),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_CREATE => sys_buffer_create_impl(a0 as *const u8, a1 as usize),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_MAP => sys_buffer_map_impl(a0 as u32, a1 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_UNMAP => sys_buffer_unmap_impl(a0),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_QUERY => sys_buffer_query_impl(a0 as u32, a1 as *mut u8, a2 as usize),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_COMMIT => sys_buffer_commit_impl(a0 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_FB_PRESENT => sys_fb_present_impl(a0 as u32, a1 as usize, a2 as usize),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_BUFFER_COMMIT_DAMAGE => {
            sys_buffer_commit_damage_impl(a0 as u32, a1 as *const u8, a2 as usize)
        }
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_PRESENT_FENCE_WAIT => {
            sys_present_fence_wait_impl(a0 as u32, a1, a2 as *mut u8, a3 as usize, a4)
        }
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_GPU_COMPUTE_SUBMIT_BLUR => {
            sys_gpu_compute_submit_blur_impl(a0 as u32, a1 as u32, a2 as u32)
        }
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_GPU_COMPUTE_FENCE_WAIT => sys_gpu_compute_fence_wait_impl(a0 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_CREATE => sys_input_focus_create_impl(),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_ACTIVATE => sys_input_focus_activate_impl(a0 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_DEACTIVATE => sys_input_focus_deactivate_impl(a0 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_POLL => sys_input_focus_poll_impl(a0 as u32),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_RECV => sys_input_focus_recv_impl(a0 as u32, a1 as *mut u8, a2 as usize),
        #[cfg(feature = "atrium-compositor-g1")]
        SYS_INPUT_FOCUS_MINT_ACTIVATOR => sys_input_focus_mint_activator_impl(a0 as u32),
        #[cfg(feature = "desktop-session")]
        SYS_FB_OUTPUT_QUERY => sys_fb_output_query_impl(),
        #[cfg(feature = "atrium-pointer-forward")]
        SYS_INPUT_POINTER_CLAIM => sys_input_pointer_claim_impl(a0),
        #[cfg(feature = "atrium-pointer-forward")]
        SYS_INPUT_POINTER_FORWARD => {
            sys_input_pointer_forward_impl(a0 as u32, a1, a2 as u32, a3 as i64, a4 as i64)
        }
        #[cfg(feature = "atrium-pointer-forward")]
        SYS_INPUT_SAK_TAKE => sys_input_sak_take_impl(a0, a1 as u32),
        #[cfg(feature = "atrium-pointer-forward")]
        SYS_TRY_RECV => sys_try_recv_impl(a0 as u32, a1, a2 as usize),
        #[cfg(feature = "wait-multiplex")]
        SYS_WAIT_ANY => sys_wait_any_impl(a0, a1 as usize, a2),
        #[cfg(feature = "installer")]
        SYS_INSTALL_LIST => sys_install_list_impl(a0, a1 as usize),
        #[cfg(feature = "installer")]
        SYS_INSTALL_RUN => sys_install_run_impl(a0, a1, a2),
        #[cfg(any(feature = "neo-test-desktop-idle", feature = "neo-test-login-cut"))]
        n @ (SYS_TEST_IDLE_TICKS | SYS_TEST_BLOCK_CUT) => test_idle_or_block_cut(n, a0),
        #[cfg(feature = "neo-test-threads-tls")]
        SYS_TEST_TLS => sys_test_tls_impl(a0, a1),
        #[cfg(any(
            feature = "neo-test-threads-tls",
            feature = "neo-test-threads-exit-group"
        ))]
        SYS_TEST_THREAD_CREATE_ON => sys_test_thread_create_on_impl(a0, a1, a2 as usize, a3),
        #[cfg(feature = "neo-test-threads-exit-group")]
        SYS_TEST_FUTEX_LATE_PARK => crate::sched::futex::arm_late_park_for_test(),
        #[cfg(feature = "neo-test-threads-exit-group")]
        SYS_TEST_KERNEL_SPIN => sys_test_kernel_spin_impl(a0),
        #[cfg(feature = "neo-test-threads-exit-group")]
        SYS_TEST_TAIL_HOLD => match validate_user_buf(a0, 4) {
            Ok(()) => crate::sched::process::arm_tail_hold_for_test(a0),
            Err(e) => e,
        },
        #[cfg(feature = "neo-test-cap-graph-revoke")]
        SYS_TEST_CAP_GRAPH_RACE => match a0 {
            0 => {
                process::cap_graph_race::arm(crate::sched::current_pid(), a1 as u32, a2 as u32);
                0
            }
            1 => process::cap_graph_race::result(),
            _ => EINVAL,
        },
        #[cfg(all(
            feature = "atrium-compositor-g1",
            any(
                feature = "neo-test-atrium-input-routing",
                feature = "neo-test-atrium-interactive",
                feature = "neo-test-dw1-desktop-wm",
                feature = "neo-test-dw2-scene-policy",
                feature = "neo-test-product-integration"
            )
        ))]
        SYS_INPUT_FOCUS_TEST_INJECT => sys_input_focus_test_inject_impl(a0 as u8, a1 as u8),
        #[cfg(feature = "neo-test-syscall-return-clean-regs")]
        SYS_TEST_DIRTY_SCRATCH => sys_test_dirty_scratch_impl(),
        #[cfg(feature = "neo-test-syscall-exit-irq-window")]
        SYS_TEST_IRQ_WINDOW => {
            crate::arch::x86_64::without_interrupts(crate::sched::leave_syscall);
            sys_test_irq_window_impl(a0, a1)
        }
        _ => ENOSYS,
    };
    // C1.3b (CP-86): and on the way back to ring 3, for a retire that began
    // while this thread was inside the syscall (parked, or running on
    // another CPU).
    //
    // With interrupts OFF from here to IRETQ (Jarvis, C1.3b review finding
    // 2; the classic "check work flags with IRQs off before returning to
    // user"). Some syscalls return with IF=1 (`spawn_user`'s quiesce resume).
    // A retire's reschedule IPI landing after this check but before the
    // epilogue's `cli` would interrupt ring 0, EOI, and skip the retire
    // (`idt.rs`, the handler retires only from ring 3): the thread then ran
    // its ring-3 loop forever. With interrupts off the IPI stays pending
    // through IRETQ and is taken in ring 3, where the handler retires it.
    // The epilogue disables them anyway, and IRETQ restores the user's IF.
    //
    // OT-93: the same discipline for a force-kill, in every build. The
    // syscall's frames have unwound; a thread the kill left Ready inside this
    // syscall, or running, exits here, and the clear of `in_syscall` and the
    // check are one section, so a later kill finds it out of the syscall.
    // `syscall_exit_irq_window` sweeps a timer across the instructions from
    // its arming to the asm epilogue, so its test syscall leaves the syscall
    // before it arms (the match arm) and skips this section. Test-only.
    #[cfg(feature = "neo-test-syscall-exit-irq-window")]
    let tail = num != SYS_TEST_IRQ_WINDOW;
    #[cfg(not(feature = "neo-test-syscall-exit-irq-window"))]
    let tail = true;
    if tail {
        crate::arch::x86_64::disable_interrupts();
        crate::sched::leave_syscall();
    }
    #[cfg(feature = "browser-runtime")]
    {
        crate::arch::x86_64::disable_interrupts();
        crate::sched::process::retire_current_if_dying();
        #[cfg(feature = "neo-test-threads-exit-group")]
        crate::sched::process::tail_hold_for_test();
    }
    ret
}

/// Validate that a user-supplied buffer `[addr, addr + len)` lies entirely
/// in the user half (`[0, USER_HALF_LIMIT)`). Returns `Ok(())` on success
/// or a negative errno (EFAULT for out-of-range/overflow, EINVAL for the
/// caller-policy "len must be > 0" check that callers run first).
///
/// This is the single point of truth for the user-half bounds discipline
/// every syscall taking a `*const u8 / *mut u8` must apply (per ADR-003
/// note about syscall-arg validation). The kernel does not yet check page
/// presence — an unmapped user page in the validated range causes a
/// kernel-mode #PF when the eventual access happens, currently fatal.
/// Phase 2 lock-down will add presence + permission checks at this layer.
/// B1c AAG J3/J4 (CP-65): an inference syscall's lifecycle. The thread is
/// marked a participant before the arm runs (before any allocation, model
/// clone or `Vec`), so force-kill never zombifies it, holder or waiter, while
/// its kernel stack owns resources. A torn-down caller cancels cooperatively
/// inside the arm. The exit decision is taken while still marked, after the
/// arm has returned (every local dropped, the stack released); the marker is
/// cleared only then, and a torn-down caller exits here.
pub(crate) fn inference_syscall(arm: impl FnOnce() -> i64) -> i64 {
    #[cfg(feature = "inference-stack")]
    {
        let pid = crate::sched::current_pid();
        let proc = process::get(pid);
        if let Some(proc) = &proc {
            proc.inference_participants.fetch_add(1, Ordering::AcqRel);
        }
        crate::sched::scheduler::set_current_inference_lifecycle(true);
        #[cfg(any(
            feature = "neo-test-real-lm-generate",
            feature = "neo-test-inference-kill"
        ))]
        INFERENCE_MARKED_ENTRIES.fetch_add(1, Ordering::AcqRel);
        let ret = arm();
        let torn_down = process::torn_down(pid);
        crate::sched::scheduler::set_current_inference_lifecycle(false);
        if let Some(proc) = &proc {
            proc.inference_participants.fetch_sub(1, Ordering::AcqRel);
        }
        // CP-30: no owned `Arc` across the diverging exit.
        drop(proc);
        if torn_down {
            #[cfg(feature = "neo-test-inference-kill")]
            INFERENCE_BOUNDARY_EXITS.fetch_add(1, Ordering::AcqRel);
            crate::sched::exit();
        }
        ret
    }
    #[cfg(not(feature = "inference-stack"))]
    {
        arm()
    }
}

/// Inference syscalls that took the lifecycle marker (the graph route of
/// `SYS_INFER_RUN` must not: re-check #2, PM ruling).
#[cfg(any(
    feature = "neo-test-real-lm-generate",
    feature = "neo-test-inference-kill"
))]
pub(crate) static INFERENCE_MARKED_ENTRIES: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// `inference_stack_kill`: participants that exited at the boundary.
#[cfg(feature = "neo-test-inference-kill")]
pub(crate) static INFERENCE_BOUNDARY_EXITS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

fn validate_user_buf(addr: u64, len: usize) -> Result<(), i64> {
    if addr >= USER_HALF_LIMIT {
        return Err(EFAULT);
    }
    let end = addr.checked_add(len as u64).ok_or(EFAULT)?;
    if end > USER_HALF_LIMIT {
        return Err(EFAULT);
    }
    Ok(())
}

fn sys_exit_impl(code: i32) -> i64 {
    let pid = current_process_for_syscall()
        .map(|proc| proc.pid)
        .unwrap_or_else(|_| crate::sched::current_pid());
    // OT-61 C1.3: with sibling threads, end the whole process (no return).
    #[cfg(feature = "browser-runtime")]
    crate::sched::process::exit_group_current(code);
    crate::sched::process::record_exit_code(pid, code);
    serial_println!("[ kernel ] proc: user thread exited with code {}", code);
    #[cfg(feature = "neo-test-tensor-create-query-drop")]
    crate::test_harness::finish_tensor_create_query_drop_from_user_exit(code);
    #[cfg(feature = "neo-test-model-load-cascade")]
    crate::test_harness::finish_model_load_cascade_from_user_exit(code);
    #[cfg(feature = "neo-test-infer-mlp-forward")]
    crate::test_harness::finish_infer_mlp_forward_from_user_exit(code);
    #[cfg(feature = "neo-test-infer-transformer-forward")]
    crate::test_harness::finish_infer_transformer_forward_from_user_exit(code);
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    crate::test_harness::finish_infer_transformer_forward_from_user_exit(code);
    #[cfg(feature = "neo-test-infer-embedding-forward")]
    crate::test_harness::finish_infer_embedding_forward_from_user_exit(code);
    #[cfg(not(any(
        feature = "neo-test-tensor-create-query-drop",
        feature = "neo-test-model-load-cascade",
        feature = "neo-test-infer-mlp-forward",
        feature = "neo-test-infer-transformer-forward",
        feature = "neo-test-inference-preemptible-liveness",
        feature = "neo-test-infer-embedding-forward"
    )))]
    crate::sched::exit();
    // sched::exit returns -> ! ; unreachable.
}

const USER_HALF_LIMIT: u64 = 0x0000_8000_0000_0000;
/// Maximum process name length accepted by sys_spawn. Phase 1.5 cap;
/// Phase 1.6 makes name handling more general.
const MAX_SPAWN_NAME_LEN: usize = 64;

/// Maximum number of handles a parent can pass to a child via sys_spawn.
/// Phase 1.5b cap; Phase 2 capability-grant generalises.
const MAX_SPAWN_HANDLES: usize = 16;

/// Keep user reads small so SMAP's AC window never covers slow serial I/O or
/// handle-table work.
const SYS_WRITE_CHUNK: usize = 256;
const MAX_OPEN_PATH_LEN: usize = 256;
const MAX_FILE_IO: usize = 4096;
const MAX_DIRENT_IO: usize = 4096;
#[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
const STDIN_FD: u32 = 0;
const STDOUT_FD: u32 = 1;
#[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
const RESERVED_FILE_FDS: &[u32] = &[STDIN_FD, STDOUT_FD];
#[cfg(not(any(feature = "serial-stdin", feature = "usb-hid")))]
const RESERVED_FILE_FDS: &[u32] = &[STDOUT_FD];
const TCP_MAX_BACKLOG: usize = 128;
const TCP_SHUTDOWN_READ: u32 = 1 << 0;
const TCP_SHUTDOWN_WRITE: u32 = 1 << 1;

fn unpack_rights(packed: u16) -> CapRights {
    CapRights::new((packed >> 8) as u8, (packed & 0xff) as u8)
}

/// Copy `len` bytes between kernel and user memory, one side each, through
/// the kernel's fault-tolerant copy (`arch::x86_64::user_copy`): `EFAULT`
/// when a user page faults (unmapped, not writable, or unmapped by another
/// thread mid-copy) instead of halting the kernel. The user side must be
/// `validate_user_buf`'d (in the user half); this makes it survivable when
/// it isn't mapped. Every syscall copy to or from user memory goes through
/// here (or `read_user`/`write_user`, which do).
fn user_copy(dst: *mut u8, src: *const u8, len: usize) -> Result<(), i64> {
    // SAFETY: the kernel side is the caller's buffer of `len` bytes; the user
    // side is in the user half, and a fault on it is recovered (EFAULT).
    if unsafe { crate::arch::x86_64::user_copy::try_copy(dst, src, len) } {
        Ok(())
    } else {
        Err(EFAULT)
    }
}

/// Check that every page of the user range `dst..dst+len` can be written,
/// without changing it: one byte of each page is read and written back
/// through the fault-tolerant copy. A syscall that consumes kernel state
/// (a joined thread, a received datagram or input event) runs this BEFORE
/// consuming, so a buffer that is unmapped or read-only is `EFAULT` with
/// nothing taken. A sibling thread that unmaps the buffer after this check
/// can still make the final copy fault (EFAULT then, after the consume): the
/// loss is that process's own, from racing its own syscall.
fn probe_user_writable(dst: *mut u8, len: usize) -> Result<(), i64> {
    if len == 0 {
        return Ok(());
    }
    let start = dst as u64;
    let last = start + (len as u64 - 1);
    let mut page = start;
    loop {
        let p = page as *mut u8;
        let mut byte = 0u8;
        user_copy(&mut byte, p, 1)?;
        user_copy(p, &byte, 1)?;
        let next = (page & !0xFFF) + 0x1000;
        if next > last {
            return Ok(());
        }
        page = next;
    }
}

/// Read a `T` from user memory (any bytes are a valid `T`: plain data).
fn read_user<T: Copy>(src: *const T) -> Result<T, i64> {
    let mut out = core::mem::MaybeUninit::<T>::uninit();
    user_copy(
        out.as_mut_ptr().cast(),
        src.cast(),
        core::mem::size_of::<T>(),
    )?;
    // SAFETY: the copy filled all of `out`; `T` is plain data.
    Ok(unsafe { out.assume_init() })
}

/// Write `value` to user memory.
fn write_user<T: Copy>(dst: *mut T, value: T) -> Result<(), i64> {
    user_copy(
        dst.cast(),
        (&value as *const T).cast(),
        core::mem::size_of::<T>(),
    )
}

/// Copy `len` bytes from user memory into a fresh kernel-heap `Vec`. Every
/// caller bounds `len` by its ABI before calling (`EINVAL`), and the
/// allocation itself is fallible: the kernel heap is one fixed region, and an
/// infallible allocation it can't meet panics the kernel, so a request too
/// large for it is `ENOMEM` to the caller instead.
fn copy_user_bytes(src: *const u8, len: usize) -> Result<alloc::vec::Vec<u8>, i64> {
    let mut out = alloc::vec::Vec::new();
    out.try_reserve_exact(len).map_err(|_| ENOMEM)?;
    // SAFETY: `try_reserve_exact` gave `len` bytes of capacity; the caller
    // validated `src..src+len` as user memory, and the copy fills them all.
    user_copy(out.as_mut_ptr(), src, len)?;
    unsafe { out.set_len(len) };
    Ok(out)
}

/// The largest tensor's storage: one buddy block of `2^MAX_ORDER` pages. A
/// tensor create or write longer than this is `EINVAL` before any copy.
const MAX_TENSOR_BYTES: usize = (1usize << crate::mm::frame::MAX_ORDER) * 4096;

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
/// B1c (F1): copy a fixed-size user descriptor into a stack array, so reading
/// the generate descriptors (whose fields the bounds policy needs) allocates
/// nothing.
fn copy_user_array<const N: usize>(src: *const u8) -> Result<[u8; N], i64> {
    validate_user_buf(src as u64, N)?;
    let mut out = [0u8; N];
    user_copy(out.as_mut_ptr(), src, N)?;
    Ok(out)
}

fn copy_user_u32s(src: *const u32, count: usize) -> Result<alloc::vec::Vec<u32>, i64> {
    let mut out = alloc::vec::Vec::<u32>::with_capacity(count);
    user_copy(out.as_mut_ptr().cast(), src.cast(), count * 4)?;
    // SAFETY: the copy filled `count` u32s of the capacity.
    unsafe { out.set_len(count) };
    Ok(out)
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn copy_generate_constrained_desc(
    src: *const GenerateConstrainedDesc,
) -> Result<GenerateConstrainedDesc, i64> {
    if src.is_null() {
        return Err(EFAULT);
    }
    let bytes =
        copy_user_array::<{ core::mem::size_of::<GenerateConstrainedDesc>() }>(src.cast::<u8>())?;
    Ok(GenerateConstrainedDesc {
        max_new: read_u32_le(&bytes, 0),
        eos_token_id: read_u32_le(&bytes, 4),
        sequence_count: read_u32_le(&bytes, 8),
        _reserved0: read_u32_le(&bytes, 12),
        sequences_ptr: read_u64_le(&bytes, 16),
        tokens_ptr: read_u64_le(&bytes, 24),
        tokens_len: read_u32_le(&bytes, 32),
        _reserved1: read_u32_le(&bytes, 36),
    })
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn copy_generate_mask_desc(src: *const GenerateMaskDesc) -> Result<GenerateMaskDesc, i64> {
    if src.is_null() {
        return Err(EFAULT);
    }
    let bytes = copy_user_array::<{ core::mem::size_of::<GenerateMaskDesc>() }>(src.cast::<u8>())?;
    Ok(GenerateMaskDesc {
        mask_ptr: read_u64_le(&bytes, 0),
        mask_bits: read_u32_le(&bytes, 8),
        mask_bytes: read_u32_le(&bytes, 12),
        flags: read_u32_le(&bytes, 16),
        _reserved0: read_u32_le(&bytes, 20),
    })
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn copy_generate_constrained_sequence_descs(
    ptr: *const GenerateConstrainedSequenceDesc,
    count: usize,
) -> Result<alloc::vec::Vec<GenerateConstrainedSequenceDesc>, i64> {
    if ptr.is_null() {
        return Err(EFAULT);
    }
    let byte_len = count
        .checked_mul(core::mem::size_of::<GenerateConstrainedSequenceDesc>())
        .ok_or(EINVAL)?;
    validate_user_buf(ptr as u64, byte_len)?;
    let bytes = copy_user_bytes(ptr.cast::<u8>(), byte_len)?;
    let mut out = alloc::vec::Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(core::mem::size_of::<GenerateConstrainedSequenceDesc>()) {
        out.push(GenerateConstrainedSequenceDesc {
            offset: read_u32_le(chunk, 0),
            len: read_u32_le(chunk, 4),
        });
    }
    Ok(out)
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn copy_kernel_bytes_to_user(dst: *mut u8, src: &[u8]) -> Result<(), i64> {
    user_copy(dst, src.as_ptr(), src.len())
}

fn zero_user_bytes(dst: *mut u8, len: usize) -> Result<(), i64> {
    static ZEROS: [u8; 256] = [0; 256];
    let mut done = 0;
    while done < len {
        let n = (len - done).min(ZEROS.len());
        user_copy(dst.wrapping_add(done), ZEROS.as_ptr(), n)?;
        done += n;
    }
    Ok(())
}

fn current_process_for_syscall() -> Result<alloc::sync::Arc<process::Process>, i64> {
    let pid = crate::sched::current_pid();
    if pid != process::KERNEL_PID {
        if let Some(proc) = process::get(pid) {
            return Ok(proc);
        }
    }

    let active_cr3 = per_cpu::current_per_cpu()
        .active_pmap_cr3
        .load(Ordering::Relaxed);
    if let Some(proc) = process::get_user_by_active_cr3(active_cr3) {
        return Ok(proc);
    }

    process::get(pid).ok_or(EINVAL)
}

fn copy_user_u64(src: *const u64) -> Result<u64, i64> {
    read_user(src)
}

fn copy_user_usize(src: *const usize) -> Result<usize, i64> {
    read_user(src)
}

fn copy_kernel_u64_to_user(dst: *mut u64, value: u64) -> Result<(), i64> {
    write_user(dst, value)
}

fn copy_kernel_usize_to_user(dst: *mut usize, value: usize) -> Result<(), i64> {
    write_user(dst, value)
}

fn copy_process_snapshot_to_user(
    dst: *mut ProcessSnapshotV1,
    value: ProcessSnapshotV1,
) -> Result<(), i64> {
    write_user(dst, value)
}

#[cfg(feature = "kernel-sysstat")]
fn copy_sysstat_to_user(dst: *mut SysStatV1, value: SysStatV1) -> Result<(), i64> {
    // SAFETY: sys_sysstat_impl validates the fixed-size user output buffer
    // before this write. SMAP remains closed except inside with_user_access.
    write_user(dst, value)
}

#[cfg(feature = "kernel-observability")]
fn copy_observability_to_user(
    dst: *mut KernelObservabilityV1,
    value: KernelObservabilityV1,
) -> Result<(), i64> {
    // SAFETY: sys_observe_impl validates the fixed-size user output buffer
    // before this write. SMAP remains closed except inside with_user_access.
    write_user(dst, value)
}

fn copy_net_stats_to_user(dst: *mut NetStatsV1, value: NetStatsV1) -> Result<(), i64> {
    write_user(dst, value)
}

fn copy_net_candidates_to_user(
    dst: *mut dns::NetCandidate,
    values: &[dns::NetCandidate],
) -> Result<(), i64> {
    user_copy(
        dst.cast(),
        values.as_ptr().cast(),
        core::mem::size_of_val(values),
    )
}

/// SYS_SPAWN dispatcher (Phase 1.5b). Validates name + handle-array
/// pointers per the user-half-bounds discipline, copies the handle index
/// list into kernel memory, looks up each handle in the calling process's
/// table, and calls `process::spawn_user_by_name`. The child receives
/// the handles at indices 0..n in the order given (ADR-004 §Q2).
///
/// **ADR-007 §Q1:** on success, also installs a
/// `Handle::Process { target: child_pid }` in the CALLER's handle table
/// — the ProcessHandle capability the parent uses with `sys_kill`. The
/// new pid (low 32) and the parent-side process-handle index (high 32)
/// are packed into the i64 return value, mirroring `sys_chan_create`'s
/// packing convention. Negative return = errno.
fn sys_spawn_impl(
    name_ptr: *const u8,
    name_len: usize,
    handles_ptr: *const u32,
    handles_count: usize,
    // OT-61 C2.2: `Some(frames)` from `SYS_SPAWN_BUDGETED`. A parameter of
    // this function (not a wrapper) so the default kernel is unchanged.
    #[cfg(feature = "mem-quota")] budget_frames: Option<u64>,
    // OT-61 C2.2b: `Some(bytes)` from `SYS_SPAWN_BUDGETED2`.
    #[cfg(feature = "obj-quota")] obj_budget: Option<u64>,
) -> i64 {
    if name_len == 0 || name_len > MAX_SPAWN_NAME_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(name_ptr as u64, name_len) {
        return e;
    }
    if handles_count > MAX_SPAWN_HANDLES {
        return EINVAL;
    }
    if handles_count > 0 {
        let bytes_needed = match handles_count.checked_mul(core::mem::size_of::<u32>()) {
            Some(v) => v,
            None => return EINVAL,
        };
        if let Err(e) = validate_user_buf(handles_ptr as u64, bytes_needed) {
            return e;
        }
    }

    let name_bytes = match copy_user_bytes(name_ptr, name_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let name = match core::str::from_utf8(&name_bytes) {
        Ok(s) => s,
        Err(_) => return EINVAL,
    };

    // Resolve handle indices in the CALLING process's table to actual
    // Handle clones. Done before spawn so a bad index aborts cleanly
    // without a half-built child.
    let cur_pid = crate::sched::current_pid();
    let caller = match crate::sched::process::get(cur_pid) {
        Some(p) => p,
        None => return EINVAL,
    };
    let mut inherited: alloc::vec::Vec<InheritedHandle> =
        alloc::vec::Vec::with_capacity(handles_count);
    if handles_count > 0 {
        let user_indices = match copy_user_u32s(handles_ptr, handles_count) {
            Ok(indices) => indices,
            Err(e) => return e,
        };
        if let Err(e) = crate::arch::x86_64::without_interrupts(|| {
            let table = caller.handles.lock();
            for idx in user_indices {
                match table.get(idx) {
                    // OT-87 R4.4: the installer authority is inherited only
                    // by the shell (the console installer); any other child
                    // is refused, so it can't spread past neo-init's choice.
                    #[cfg(feature = "installer")]
                    Some(Handle::Installer { .. }) if !installer_inheritable_by(name) => {
                        return Err(EPERM);
                    }
                    // OT-66 L1: the system store never leaves the session root.
                    #[cfg(feature = "system-store")]
                    Some(Handle::SystemStore { .. }) => return Err(EPERM),
                    // Dexter #187 item 3a: nor does the mount authority.
                    #[cfg(feature = "mount-authority")]
                    Some(Handle::MountAuthority { .. }) => return Err(EPERM),
                    #[cfg(not(feature = "cap-rooted-open"))]
                    Some(h @ Handle::File { rights, .. }) if is_spawn_inheritable_dir(rights) => {
                        // File capabilities intentionally still report no generic
                        // GRANT meta-rights, so sys_grant cannot delegate them.
                        // M1 admits only a spawned own-directory cap.
                        let parent = h.cap_id();
                        let child = h.clone().with_cap(process::alloc_cap_id(), h.rights());
                        inherited.push(InheritedHandle {
                            handle: child,
                            parent: Some(parent),
                        });
                    }
                    // OT-66 K1a: also a directory cap carrying GRANT (a
                    // derived RootDirCap, or a directory under one). Never
                    // a plain file.
                    #[cfg(feature = "cap-rooted-open")]
                    Some(h @ Handle::File { .. }) if process::is_spawn_inheritable_dir_cap(&h) => {
                        let parent = h.cap_id();
                        let child = h.clone().with_cap(process::alloc_cap_id(), h.rights());
                        inherited.push(InheritedHandle {
                            handle: child,
                            parent: Some(parent),
                        });
                    }
                    Some(Handle::File { .. }) => return Err(EPERM),
                    Some(h) if h.rights().has_meta(meta_rights::GRANT) => {
                        let parent = h.cap_id();
                        let child = h.clone().with_cap(process::alloc_cap_id(), h.rights());
                        inherited.push(InheritedHandle {
                            handle: child,
                            parent: Some(parent),
                        });
                    }
                    Some(_) => return Err(EPERM),
                    None => return Err(EBADF),
                }
            }
            Ok(())
        }) {
            return e;
        }
    }

    #[cfg(not(feature = "mem-quota"))]
    let spawned = process::spawn_user_by_name(name, &inherited);
    #[cfg(all(feature = "mem-quota", not(feature = "obj-quota")))]
    let spawned = match budget_frames {
        None => process::spawn_user_by_name(name, &inherited),
        Some(frames) => process::spawn_user_by_name_budgeted(name, &inherited, frames),
    };
    #[cfg(feature = "obj-quota")]
    let spawned = match (budget_frames, obj_budget) {
        (None, None) => process::spawn_user_by_name(name, &inherited),
        (Some(frames), None) => process::spawn_user_by_name_budgeted(name, &inherited, frames),
        (frames, Some(obj)) => process::spawn_user_by_name_budgeted2(
            name,
            &inherited,
            frames.unwrap_or(crate::mm::mem_account_logic::UNLIMITED),
            obj,
        ),
    };
    let child_pid = match spawned {
        Ok(pid) => pid,
        Err(process::SpawnError::SchedulerFull) => return EAGAIN,
        Err(process::SpawnError::ParentGone) => return EBADF,
        // OT-61 C2.2b: the parent's object budget cannot hold the spawn, or
        // the child's cannot hold its inherited handles.
        #[cfg(feature = "obj-quota")]
        Err(process::SpawnError::ObjQuota) => return EAGAIN,
        // OT-61 C2.2: the budget could not hold the child's PML4, image or
        // stack (charged before each was allocated, released by the dropped
        // Pmap).
        #[cfg(feature = "mem-quota")]
        Err(
            process::SpawnError::Pmap
            | process::SpawnError::Elf(crate::loader::elf::ElfError::OutOfFrames),
        ) if budget_frames.is_some() => return ENOMEM,
        Err(_) => return ENOENT,
    };
    #[cfg(feature = "freeze-watchdog")]
    if name == "arthur" {
        crate::watchdog::note_arthur_spawned();
    }

    // Install ProcessHandle in caller's table — ADR-007 §Q1 capability.
    // Done AFTER spawn succeeds so a spawn failure leaves the caller's
    // table untouched.
    let installed = process::install_cap(
        &caller,
        Handle::Process {
            target: child_pid,
            cap_id: process::alloc_cap_id(),
            rights: process::full_process_rights(),
        },
        None,
    );
    // OT-91: the caller's handle (if it went in) now carries the claim on the
    // child's exit code; the spawn's own claim goes. With no handle, nobody
    // can ever wait on the child, and its code is not kept.
    process::release_spawn_claim(child_pid);
    let process_handle_idx = match installed {
        Ok(idx) => idx,
        Err(_) => return EINVAL,
    };

    // Pack: low 32 = pid, high 32 = handle index. Same convention as
    // sys_chan_create. libneo::sys_spawn unpacks.
    (child_pid.0 as u64 | ((process_handle_idx as u64) << 32)) as i64
}

/// OT-61 C2.2: `SYS_SPAWN` with a memory budget. `budget_bytes == 0` is
/// `EINVAL`; a budget too small to load the module is `ENOMEM`.
#[cfg(feature = "mem-quota")]
fn sys_spawn_budgeted_impl(
    name_ptr: *const u8,
    name_len: usize,
    handles_ptr: *const u32,
    handles_count: usize,
    budget_bytes: u64,
) -> i64 {
    let Some(frames) = crate::mm::mem_account_logic::budget_frames(budget_bytes) else {
        return EINVAL;
    };
    sys_spawn_impl(
        name_ptr,
        name_len,
        handles_ptr,
        handles_count,
        Some(frames),
        #[cfg(feature = "obj-quota")]
        None,
    )
}

#[cfg(feature = "obj-quota")]
fn sys_spawn_budgeted2_impl(
    name_ptr: *const u8,
    name_len: usize,
    handles_ptr: *const u32,
    handles_count: usize,
    frame_bytes: u64,
    obj_bytes: u64,
) -> i64 {
    if obj_bytes == 0 {
        return EINVAL;
    }
    let frames = if frame_bytes == 0 {
        None
    } else {
        match crate::mm::mem_account_logic::budget_frames(frame_bytes) {
            Some(f) => Some(f),
            None => return EINVAL,
        }
    };
    sys_spawn_impl(
        name_ptr,
        name_len,
        handles_ptr,
        handles_count,
        frames,
        Some(obj_bytes),
    )
}

#[cfg(not(feature = "cap-rooted-open"))]
fn is_spawn_inheritable_dir(rights: FileRights) -> bool {
    rights.bits() == (FileRights::LOOKUP.bits() | FileRights::READDIR.bits())
}

// === IPC syscalls (Phase 1.5b) ==========================================

/// SYS_CHAN_CREATE — create a fresh anonymous channel and install both
/// endpoints in the calling process's handle table. Returns the two handle
/// indices packed into a single i64: low 32 bits = h_a, high 32 bits = h_b.
fn sys_chan_create_impl() -> i64 {
    let proc = match crate::sched::process::get(crate::sched::current_pid()) {
        Some(p) => p,
        None => return EINVAL,
    };
    let channel = crate::ipc::Channel::new();
    let h_a = match process::install_cap(
        &proc,
        Handle::Channel {
            channel: channel.clone(),
            side: crate::ipc::EndpointSide::A,
            cap_id: process::alloc_cap_id(),
            rights: process::full_channel_rights(),
        },
        None,
    ) {
        Ok(idx) => idx,
        #[cfg(feature = "obj-quota")]
        Err(process::InstallCapError::Quota) => return ENOMEM,
        Err(_) => return EINVAL,
    };
    let h_b = match process::install_cap(
        &proc,
        Handle::Channel {
            channel,
            side: crate::ipc::EndpointSide::B,
            cap_id: process::alloc_cap_id(),
            rights: process::full_channel_rights(),
        },
        None,
    ) {
        Ok(idx) => idx,
        // OT-61 C2.2b: the budget held one endpoint, not both; take the
        // first back out, so the refusal leaves nothing installed.
        #[cfg(feature = "obj-quota")]
        Err(process::InstallCapError::Quota) => {
            let _ = process::close_handle(&proc, h_a, |_| true);
            return ENOMEM;
        }
        Err(_) => return EINVAL,
    };
    (h_a as u64 | ((h_b as u64) << 32)) as i64
}

fn socket_errno(e: SocketError) -> i64 {
    match e {
        SocketError::Invalid => EINVAL,
        SocketError::Busy => EBUSY,
        SocketError::WouldBlock => EAGAIN,
        SocketError::NotConnected => ENOTCONN,
        SocketError::NoRoute => EAGAIN,
    }
}

fn tcp_errno(e: TcpError) -> i64 {
    match e {
        TcpError::Invalid => EINVAL,
        TcpError::Busy => EBUSY,
        TcpError::WouldBlock => EAGAIN,
        TcpError::BufferTooSmall | TcpError::Checksum => EINVAL,
    }
}

fn dns_errno(e: DnsError) -> i64 {
    match e {
        DnsError::Invalid | DnsError::BufferTooSmall | DnsError::Unsupported => EINVAL,
        DnsError::NotFound => ENOENT,
        DnsError::NoResolver | DnsError::WouldBlock => EAGAIN,
    }
}

fn dial_errno(e: DialError) -> i64 {
    match e {
        DialError::Invalid => EINVAL,
        DialError::Resolve => EAGAIN,
        DialError::NoCandidates => ENOENT,
        DialError::AllFailed => EAGAIN,
    }
}

fn tensor_errno(e: tensor::TensorError) -> i64 {
    match e {
        tensor::TensorError::InvalidShape
        | tensor::TensorError::InvalidDtype
        | tensor::TensorError::InvalidLength
        | tensor::TensorError::InvalidDevice => EINVAL,
        tensor::TensorError::PermissionDenied => EPERM,
        tensor::TensorError::OutOfMemory => ENOMEM,
    }
}

fn copy_tensor_create_desc_from_user(
    desc_ptr: *const u8,
    desc_len: usize,
) -> Result<TensorCreateDesc, i64> {
    let expected_len = core::mem::size_of::<TensorCreateDesc>();
    if desc_ptr.is_null() || desc_len != expected_len {
        return Err(EINVAL);
    }
    validate_user_buf(desc_ptr as u64, expected_len)?;
    let desc_bytes = copy_user_bytes(desc_ptr, expected_len)?;
    // SAFETY: desc_bytes has exactly size_of::<TensorCreateDesc>() bytes;
    // read_unaligned avoids imposing user ABI alignment on the descriptor.
    Ok(unsafe { core::ptr::read_unaligned(desc_bytes.as_ptr().cast::<TensorCreateDesc>()) })
}

fn copy_tensor_query_to_user(dst: *mut u8, value: TensorQueryDesc) -> Result<(), i64> {
    write_user(dst.cast::<TensorQueryDesc>(), value)
}

/// Resolve a tensor handle to its USE-gated identity — table read only, no
/// mint (BUILD-O1b): metadata paths stop here; byte-touching paths continue
/// through [`tensor_lookup`]'s mint.
fn tensor_handle_lookup(
    h: u32,
) -> Result<
    (
        crate::ai::arena::SlotKey,
        tensor::TensorMeta,
        process::CapId,
        CapRights,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        tensor_handle_lookup_in(&proc, h)
    })
}

/// The per-syscall tensor capability resolution, parameterized by the owning
/// process: the handle-table lookup + the USE-rights gate (meta + semantic),
/// bounded O(1). BUILD-LO extracted this from `tensor_handle_lookup` so the §4
/// latency oracle can measure the real cap-check work against a constructed
/// process without a userland round-trip. The shipped path is unchanged —
/// `tensor_handle_lookup` calls it with the current process under
/// `without_interrupts`, byte-identical to the pre-extraction body.
fn tensor_handle_lookup_in(
    proc: &alloc::sync::Arc<process::Process>,
    h: u32,
) -> Result<
    (
        crate::ai::arena::SlotKey,
        tensor::TensorMeta,
        process::CapId,
        CapRights,
    ),
    i64,
> {
    let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
    match handle {
        Handle::Tensor {
            slot,
            meta,
            cap_id,
            rights,
        } if rights.has_meta(meta_rights::USE)
            && rights.has_semantic(process::tensor_rights::USE) =>
        {
            Ok((slot, meta, cap_id, rights))
        }
        Handle::Tensor { .. } => Err(EPERM),
        _ => Err(EBADF),
    }
}

/// BUILD-LO latency-oracle probe seam (the §4 hot-path cost instrument):
/// the real cap-resolution work — `tensor_handle_lookup_in` under
/// `without_interrupts`, exactly as the shipped tensor syscalls run it —
/// against an explicitly-passed process, so the oracle measures the in-kernel
/// capability check with **no syscall entry** (budget verdict 1). Returns `0`
/// on success or the negative errno. Default-off; compiled only under the
/// oracle scenario feature.
#[cfg(feature = "neo-test-ai-latency-oracle")]
pub fn debug_cap_check(proc: &alloc::sync::Arc<process::Process>, h: u32) -> i64 {
    crate::arch::x86_64::without_interrupts(|| match tensor_handle_lookup_in(proc, h) {
        Ok(_) => 0,
        Err(e) => e,
    })
}

/// BUILD-LO latency-oracle probe seam: the in-kernel **metadata core** that
/// `sys_tensor_query_impl` runs — cap resolution + `arena::mint` +
/// `handle_count` + `query_desc` — excluding only the fixed-size user-buffer
/// copy and the syscall entry (the boundary verdict 1 excludes by design).
/// Drives budget verdict 2 (cost independent of tensor *size*). The cap lookup
/// runs under `without_interrupts`; the mint runs OUTSIDE it (the P4a accessor
/// may park) — the exact IRQ discipline of the real query path. Returns `0` on
/// success or the negative errno. Default-off.
#[cfg(feature = "neo-test-ai-latency-oracle")]
pub fn debug_query_meta(proc: &alloc::sync::Arc<process::Process>, h: u32) -> i64 {
    let slot = match crate::arch::x86_64::without_interrupts(|| tensor_handle_lookup_in(proc, h)) {
        Ok((slot, _meta, _cap, _rights)) => slot,
        Err(e) => return e,
    };
    let tensor = match crate::ai::arena::mint(slot) {
        Some(tensor) => tensor,
        None => return EBADF,
    };
    let refcount = u64::from(crate::ai::arena::handle_count(slot).unwrap_or(0));
    let _desc = tensor.query_desc(refcount);
    0
}

/// Resolve AND mint (BUILD-O1b): the returned `Arc` pins the tensor for
/// exactly this use (I-9); the caller drops it when the use completes. The
/// mint runs OUTSIDE `without_interrupts` — it can park across an in-flight
/// migration (the P4a accessor protocol; never IRQ-off). A stale slot (the
/// handle outlived its tensor — only reachable through a revoke race) fails
/// EBADF: I-13's production consumer, strictly safer than the pre-O1b race
/// (the revoke now wins; see ADR-043).
fn tensor_lookup(
    h: u32,
) -> Result<(alloc::sync::Arc<tensor::Tensor>, process::CapId, CapRights), i64> {
    let (slot, _meta, cap_id, rights) = tensor_handle_lookup(h)?;
    let tensor = crate::ai::arena::mint(slot).ok_or(EBADF)?;
    Ok((tensor, cap_id, rights))
}

fn sys_tensor_create_impl(desc_ptr: *const u8, desc_len: usize) -> i64 {
    let desc = match copy_tensor_create_desc_from_user(desc_ptr, desc_len) {
        Ok(desc) => desc,
        Err(e) => return e,
    };
    let data_len = if desc.data_len > usize::MAX as u64 {
        return EINVAL;
    } else {
        desc.data_len as usize
    };
    let dtype = match tensor::Dtype::from_abi(desc.dtype) {
        Ok(dtype) => dtype,
        Err(e) => return tensor_errno(e),
    };
    let shape = match tensor::Shape::new(desc.rank, desc.dims) {
        Ok(shape) => shape,
        Err(e) => return tensor_errno(e),
    };
    if let Err(e) = tensor::validate_data_len(shape, dtype, desc.data_len) {
        return tensor_errno(e);
    }
    // The shape alone is user-chosen, so its byte length is too: cap it at the
    // largest tensor storage can hold before copying it into the kernel heap.
    if data_len > MAX_TENSOR_BYTES {
        return EINVAL;
    }
    if desc.mutable > 1 {
        return EINVAL;
    }
    if desc.data_ptr == 0 {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(desc.data_ptr, data_len) {
        return e;
    }
    let data = match copy_user_bytes(desc.data_ptr as *const u8, data_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let tensor = match tensor::Tensor::from_desc_and_bytes(desc, &data) {
        Ok(tensor) => alloc::sync::Arc::new(tensor),
        Err(e) => return tensor_errno(e),
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    // BUILD-O1b: the arena owns the tensor at rest; the handle names it.
    // alloc's creator count transfers to the table (install retains, then
    // the creator releases) — on install failure the release frees the slot.
    let meta = tensor.meta();
    let slot = crate::ai::arena::alloc(tensor, crate::ai::arena::default_home());
    let handle = Handle::Tensor {
        slot,
        meta,
        cap_id: process::alloc_cap_id(),
        rights: process::full_tensor_rights(),
    };
    let installed = process::install_cap(&proc, handle, None);
    crate::ai::arena::release(slot);
    match installed {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

fn sys_tensor_query_impl(h: u32, out: *mut u8, out_len: usize) -> i64 {
    let expected_len = core::mem::size_of::<TensorQueryDesc>();
    if out.is_null() || out_len != expected_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, expected_len) {
        return e;
    }
    let (slot, _meta, _, _) = match tensor_handle_lookup(h) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let tensor = match crate::ai::arena::mint(slot) {
        Some(tensor) => tensor,
        None => return EBADF,
    };
    // The logical refcount IS the arena handle count (BUILD-O1b): with
    // install-retains/remove-releases it equals the live table entries —
    // exactly the pre-O1b Arc-derived figure at any quiescent point.
    let refcount = u64::from(crate::ai::arena::handle_count(slot).unwrap_or(0));
    let desc = tensor.query_desc(refcount);
    if let Err(e) = copy_tensor_query_to_user(out, desc) {
        return e;
    }
    0
}

fn sys_tensor_share_impl(h: u32) -> i64 {
    // Metadata-only: a share copies the slot key (N handles → 1 slot → 1
    // physical copy, §9.7); the table install carries the retain. No mint.
    let (slot, meta, parent_cap_id, rights) = match tensor_handle_lookup(h) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let handle = Handle::Tensor {
        slot,
        meta,
        cap_id: process::alloc_cap_id(),
        rights,
    };
    match process::install_cap(&proc, handle, Some(parent_cap_id)) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

fn sys_tensor_drop_impl(h: u32) -> i64 {
    let removed = match process::get(crate::sched::current_pid()) {
        Some(proc) => process::close_handle(&proc, h, |h| matches!(h, Handle::Tensor { .. })),
        None => Err(EINVAL),
    };
    match removed {
        Ok(Handle::Tensor { .. }) => 0,
        Ok(_) => EBADF,
        Err(e) => e,
    }
}

/// The op-boundary write enforcement (Phase 7 BUILD-3, ISC-2), parameterised by the
/// acting principal so both the `SYS_TENSOR_WRITE` syscall (current pid) and the
/// kernel sharing gate (an explicit reader pid) exercise the *same* check. Denials, in
/// order: (a) the capability lacks `tensor_rights::WRITE` — the **read-only-grantee**
/// case, the load-bearing cross-tenant denial, enforced even when the underlying
/// tensor object is itself mutable; (b) the tensor object is immutable/frozen (e.g.
/// model weights). On success copies `data` into the tensor's bytes at `offset`.
/// The syscall itself bounds the write against the tensor before it copies the
/// user's bytes (`sys_tensor_write_impl`); this whole-call form is for tests.
#[cfg(feature = "neo-test")]
pub(crate) fn tensor_write_checked(
    pid: process::ProcessId,
    h: u32,
    offset: u64,
    data: &[u8],
) -> i64 {
    match writable_tensor(pid, h) {
        Ok(tensor) => tensor_write_into(&tensor, offset, data),
        Err(e) => e,
    }
}

/// The tensor handle `h` of `pid` names, minted (the I-9 pin), if the handle
/// carries the rights to write it: `EBADF`, `EPERM` or `EINVAL` otherwise.
fn writable_tensor(
    pid: process::ProcessId,
    h: u32,
) -> Result<alloc::sync::Arc<tensor::Tensor>, i64> {
    let looked = crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(pid).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::Tensor { slot, rights, .. }
                if rights.has_meta(meta_rights::USE)
                    && rights.has_semantic(process::tensor_rights::USE) =>
            {
                Ok((slot, rights))
            }
            Handle::Tensor { .. } => Err(EPERM),
            _ => Err(EBADF),
        }
    });
    let (slot, rights) = looked?;
    // Mint outside without_interrupts: the write is the use (I-9 pin).
    let tensor = crate::ai::arena::mint(slot).ok_or(EBADF)?;
    // Capability-level read-only enforcement: a grantee without WRITE is denied even
    // if the tensor object itself is mutable — rights, not convention (ISC-2).
    if !rights.has_semantic(process::tensor_rights::WRITE) {
        return Err(EPERM);
    }
    Ok(tensor)
}

/// Write `data` at `offset` into a tensor [`writable_tensor`] returned.
fn tensor_write_into(tensor: &tensor::Tensor, offset: u64, data: &[u8]) -> i64 {
    // Object-level immutability (frozen weights) is independently denied; an
    // out-of-range write is EINVAL. OT-70: the tensor is a shared `Arc`, so the
    // write goes through `write_bytes` rather than a `&mut [u8]` view.
    let Ok(offset) = usize::try_from(offset) else {
        return EINVAL;
    };
    match tensor.write_bytes(offset, data) {
        Ok(()) => 0,
        Err(tensor::TensorError::PermissionDenied) => EPERM,
        Err(_) => EINVAL,
    }
}

fn sys_tensor_write_impl(h: u32, offset: u64, data_ptr: *const u8, data_len: usize) -> i64 {
    if data_len == 0 {
        return 0;
    }
    if data_ptr.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(data_ptr as u64, data_len) {
        return e;
    }
    if data_len > MAX_TENSOR_BYTES {
        return EINVAL;
    }
    // The handle, its rights, the tensor's mutability and the write's range
    // are all checked before any byte is copied into the kernel heap: the copy
    // can only be as long as the tensor it lands in. An immutable tensor is
    // EPERM ahead of a range error, as `write_bytes` orders them, and before
    // the copy (Jarvis #153: a 32 MiB write to a 64-byte immutable tensor
    // copied first and returned ENOMEM).
    let tensor = match writable_tensor(crate::sched::current_pid(), h) {
        Ok(tensor) => tensor,
        Err(e) => return e,
    };
    if !tensor.is_mutable() {
        return EPERM;
    }
    if usize::try_from(offset)
        .ok()
        .and_then(|o| o.checked_add(data_len))
        .is_none_or(|end| end > tensor.byte_len())
    {
        return EINVAL;
    }
    let data = match copy_user_bytes(data_ptr, data_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    tensor_write_into(&tensor, offset, &data)
}

fn copy_model_query_to_user(dst: *mut u8, value: ModelQueryDesc) -> Result<(), i64> {
    write_user(dst.cast::<ModelQueryDesc>(), value)
}

fn model_lookup(h: u32) -> Result<alloc::sync::Arc<model::Model>, i64> {
    let proc = current_process_for_syscall()?;
    crate::arch::x86_64::without_interrupts(|| model_from_handle(proc.handles.lock().get(h)))
}

/// The model-handle classification every generate/infer arm applies: no
/// entry (bad or closed index) is `EBADF`, a non-model handle is `EBADF`, a
/// model handle without `USE` is `EPERM`.
fn model_from_handle(handle: Option<Handle>) -> Result<alloc::sync::Arc<model::Model>, i64> {
    match handle.ok_or(EBADF)? {
        Handle::Model { model, rights, .. }
            if rights.has_meta(meta_rights::USE) && rights.has_semantic(model_rights::USE) =>
        {
            Ok(model)
        }
        Handle::Model { .. } => Err(EPERM),
        _ => Err(EBADF),
    }
}

/// B1c AAG J2: the syscall-level bounds probe. It calls the real
/// `SYS_GENERATE{,_CONSTRAINED,_MASKED}` and `SYS_INFER_RUN` arms as a user
/// process would: a registered process with one mapped user page, its CR3
/// active on this CPU. Run it on an AP (no timer there, so nothing switches
/// the CR3 back mid-probe). Each case reports its return value and the heap
/// allocation calls it made.
/// The probe's user process: one zeroed user page at `ABI_PROBE_VA`.
#[cfg(any(
    feature = "neo-test-real-lm-generate",
    feature = "neo-test-inference-kill"
))]
pub(crate) fn abi_probe_process_for_test(
    page: x86_64::structures::paging::PhysFrame,
) -> alloc::sync::Arc<process::Process> {
    use x86_64::structures::paging::PageTableFlags as F;
    let pmap = crate::mm::pmap::Pmap::new().expect("abi probe: pmap");
    pmap.map_user(
        x86_64::VirtAddr::new(ABI_PROBE_VA),
        page,
        F::PRESENT | F::WRITABLE | F::USER_ACCESSIBLE | F::NO_EXECUTE,
    )
    .expect("abi probe: map_user");
    process::register(process::Process::new_user("abi-probe", pmap))
}

#[cfg(any(
    feature = "neo-test-real-lm-generate",
    feature = "neo-test-inference-kill"
))]
pub(crate) const ABI_PROBE_VA: u64 = 0x0000_0010_0000_0000;

/// B1c AAG J3: one `SYS_GENERATE`, exactly as `syscall_dispatch` runs it (the
/// lifecycle wrapper around the real arm), for the kill scenario's callers.
#[cfg(feature = "neo-test-inference-kill")]
pub(crate) fn generate_syscall_for_test(
    model_h: u32,
    prompt: u64,
    prompt_len: u32,
    out: u64,
    out_cap: u32,
    max_new: u64,
) -> i64 {
    inference_syscall(|| {
        sys_generate_impl(
            model_h,
            prompt as *const u32,
            prompt_len,
            out as *mut u32,
            out_cap,
            max_new | (u64::from(u32::MAX) << 32),
        )
    })
}

/// B1c AAG J2: the syscall-level bounds probe. It calls the real
/// `SYS_GENERATE{,_CONSTRAINED,_MASKED}` and `SYS_INFER_RUN` arms as a user
/// process would: the calling thread belongs to `proc` (`current_pid`), and
/// `proc`'s CR3 is active on this CPU. Run it on an AP (no timer there, so
/// nothing switches the CR3 back mid-probe). Each case reports its return
/// value and the heap allocation calls it made.
#[cfg(feature = "neo-test-real-lm-generate")]
pub(crate) fn generate_abi_probe_for_test(
    model: alloc::sync::Arc<model::Model>,
    graph: alloc::sync::Arc<model::Model>,
    proc: alloc::sync::Arc<process::Process>,
    page: x86_64::structures::paging::PhysFrame,
) -> alloc::vec::Vec<(&'static str, i64, u64)> {
    use crate::ai::generate_logic::MAX_GENERATE_PROMPT_TOKENS as CAP;

    const VA: u64 = ABI_PROBE_VA;
    const PROMPT: u64 = VA;
    const OUT: u64 = VA + 0x40;
    const CDESC: u64 = VA + 0x100;
    const MDESC: u64 = VA + 0x140;
    const MASK: u64 = VA + 0x180;
    const INPUT: u64 = VA + 0x200;
    const OUTPUTS: u64 = VA + 0x208;
    // A kernel address: refused as `EFAULT` by any pointer check it reaches.
    const BAD: u64 = 0xffff_8000_0000_0000;

    let model_h = process::install_cap(
        &proc,
        Handle::Model {
            model,
            cap_id: process::alloc_cap_id(),
            rights: process::full_model_rights(),
        },
        None,
    )
    .expect("abi probe: model handle");
    let graph_h = process::install_cap(
        &proc,
        Handle::Model {
            model: graph,
            cap_id: process::alloc_cap_id(),
            rights: process::full_model_rights(),
        },
        None,
    )
    .expect("abi probe: graph handle");
    let tensor_h = |tokens: usize| -> u32 {
        let mut dims = [0u64; tensor::MAX_RANK];
        dims[0] = tokens as u64;
        let desc = TensorCreateDesc {
            rank: 1,
            dtype: tensor::Dtype::I32_ABI,
            mutable: 0,
            _pad: 0,
            dims,
            data_ptr: 0,
            data_len: (tokens * 4) as u64,
        };
        let bytes = alloc::vec![0u8; tokens * 4];
        let tensor = alloc::sync::Arc::new(
            tensor::Tensor::from_desc_and_bytes(desc, &bytes).expect("abi probe: tensor"),
        );
        let meta = tensor.meta();
        let slot = crate::ai::arena::alloc(tensor, crate::ai::arena::default_home());
        let idx = process::install_cap(
            &proc,
            Handle::Tensor {
                slot,
                meta,
                cap_id: process::alloc_cap_id(),
                rights: process::full_tensor_rights(),
            },
            None,
        )
        .expect("abi probe: tensor handle");
        crate::ai::arena::release(slot);
        idx as u32
    };
    let over_cap_h = tensor_h(CAP + 1);
    let ok_h = tensor_h(4);

    // The user page, written through the kernel's HHDM alias.
    let base = crate::mm::phys_to_virt(page.start_address()).as_mut_ptr::<u8>();
    let put = |off: u64, bytes: &[u8]| {
        // SAFETY: `page` is a 4 KiB frame this probe owns; every offset used
        // below plus its length stays inside it.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                base.add((off - VA) as usize),
                bytes.len(),
            )
        };
    };
    put(PROMPT, &[1u32, 5, 9, 2].map(u32::to_le_bytes).concat());
    // Constrained: 17 sequences (over the 16 cap), otherwise well formed.
    let mut cdesc = [0u8; 40];
    cdesc[0..4].copy_from_slice(&1u32.to_le_bytes());
    cdesc[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    cdesc[8..12].copy_from_slice(&17u32.to_le_bytes());
    cdesc[32..36].copy_from_slice(&1u32.to_le_bytes());
    put(CDESC, &cdesc);
    // Masked: mask_bits 0 without the disabled flag (a malformed mask).
    let mut mdesc = [0u8; 24];
    mdesc[0..8].copy_from_slice(&MASK.to_le_bytes());
    put(MDESC, &mdesc);
    put(INPUT, &over_cap_h.to_le_bytes());

    let kernel = crate::sched::process::kernel_cr3();
    let (frame, flags) = proc.pmap.as_ref().expect("abi probe: pmap").cr3();
    let activate = |frame, flags| {
        // SAFETY: both page tables carry the kernel half; this CPU has no
        // timer, so nothing switches CR3 under the probe.
        unsafe { x86_64::registers::control::Cr3::write(frame, flags) };
        per_cpu::current_per_cpu()
            .active_pmap_cr3
            .store(per_cpu::encode_cr3(frame, flags), Ordering::Relaxed);
    };
    activate(frame, flags);

    let mut results = alloc::vec::Vec::new();
    let mut case = |name: &'static str, call: &dyn Fn() -> i64| {
        crate::mm::heap::reset_instrumentation_counters();
        let ret = call();
        let allocs = crate::mm::heap::instrumentation_counters().alloc_calls;
        results.push((name, ret, allocs));
    };
    let p = |a: u64| a as *const u32;
    let o = |a: u64| a as *mut u32;
    let over = (CAP + 1) as u32;
    let plain = |max_new: u64| max_new | (u64::from(u32::MAX) << 32);

    // SYS_GENERATE: every bound is in a register, so all come first.
    case("plain_over_prompt_bad_ptrs", &|| {
        sys_generate_impl(model_h, p(BAD), over, o(BAD), 8, plain(1))
    });
    case("plain_over_new_bad_handle", &|| {
        sys_generate_impl(u32::MAX, p(PROMPT), 4, o(OUT), 8, plain(257))
    });
    case("plain_bad_prompt_ptr", &|| {
        sys_generate_impl(model_h, p(BAD), 4, o(OUT), 8, plain(1))
    });
    case("plain_bad_handle", &|| {
        sys_generate_impl(u32::MAX, p(PROMPT), 4, o(OUT), 8, plain(1))
    });
    case("plain_ok", &|| {
        sys_generate_impl(model_h, p(PROMPT), 4, o(OUT), 8, plain(2))
    });

    // SYS_GENERATE_CONSTRAINED: the prompt length (register) first, then the
    // descriptor read, then its bounds.
    let cd = |a: u64| a as *const GenerateConstrainedDesc;
    case("constrained_over_prompt_bad_desc", &|| {
        sys_generate_constrained_impl(model_h, p(BAD), over, o(BAD), 8, cd(BAD))
    });
    case("constrained_bad_desc", &|| {
        sys_generate_constrained_impl(model_h, p(PROMPT), 4, o(OUT), 8, cd(BAD))
    });
    case("constrained_over_sequences_bad_ptrs", &|| {
        sys_generate_constrained_impl(u32::MAX, p(BAD), 4, o(BAD), 8, cd(CDESC))
    });

    // SYS_GENERATE_MASKED: the same order; a malformed mask is EINVAL ahead
    // of a bad output pointer.
    let md = |a: u64| a as *const GenerateMaskDesc;
    case("masked_over_prompt_bad_desc", &|| {
        sys_generate_masked_impl(model_h, p(BAD), over, o(BAD), md(BAD))
    });
    case("masked_bad_desc", &|| {
        sys_generate_masked_impl(model_h, p(PROMPT), 4, o(OUT), md(BAD))
    });
    case("masked_bad_mask_null_out", &|| {
        sys_generate_masked_impl(model_h, p(PROMPT), 4, core::ptr::null_mut(), md(MDESC))
    });

    // SYS_INFER_RUN, inventory route: the token cap before any allocation.
    case("infer_over_cap", &|| {
        sys_infer_run_impl(model_h, p(INPUT), 1, o(OUTPUTS), 1, 0)
    });
    case("infer_bad_input_ptr", &|| {
        sys_infer_run_impl(model_h, p(BAD), 1, o(OUTPUTS), 1, 0)
    });
    case("infer_bad_model_handle", &|| {
        sys_infer_run_impl(u32::MAX, p(INPUT), 1, o(OUTPUTS), 1, 0)
    });
    put(INPUT, &u32::MAX.to_le_bytes());
    case("infer_bad_tensor_handle", &|| {
        sys_infer_run_impl(model_h, p(INPUT), 1, o(OUTPUTS), 1, 0)
    });
    put(INPUT, &ok_h.to_le_bytes());
    let marked = || INFERENCE_MARKED_ENTRIES.load(Ordering::Acquire);
    let marked0 = marked();
    // Re-check #2 (PM ruling): the inventory route takes the lifecycle
    // marker; the graph route does not (it is `main`'s, unmarked). Both
    // marker calls enter through `syscall_dispatch`'s real `SYS_INFER_RUN`
    // arm (Jarvis, re-check #3), so a wrapper added around the whole arm
    // would show up as a graph mark.
    case("infer_ok", &|| {
        syscall_dispatch(SYS_INFER_RUN, model_h as u64, INPUT, 1, OUTPUTS, 1, 0)
    });
    let inventory_marked = marked() - marked0;
    let marked1 = marked();
    let _ = syscall_dispatch(SYS_INFER_RUN, graph_h as u64, INPUT, 1, OUTPUTS, 1, 0);
    let graph_marked = marked() - marked1;
    results.push(("infer_inventory_marked", inventory_marked as i64, 0));
    results.push(("infer_graph_marked", graph_marked as i64, 0));

    activate(kernel.0, kernel.1);
    results
}

/// B1c (F1): the errnos `model_from_handle` gives for a missing (bad or
/// closed) index, a non-model handle, a model handle without the semantic
/// `USE`, one without the meta `USE`, and a full one.
#[cfg(feature = "neo-test-real-lm-generate")]
pub(crate) fn model_handle_errnos_for_test(
    model: alloc::sync::Arc<model::Model>,
) -> [Option<i64>; 5] {
    let errno = |handle: Option<Handle>| model_from_handle(handle).err();
    let full = process::full_model_rights();
    let with = |rights| Handle::Model {
        model: model.clone(),
        cap_id: process::alloc_cap_id(),
        rights,
    };
    let not_a_model = Handle::Process {
        target: process::KERNEL_PID,
        cap_id: process::alloc_cap_id(),
        rights: process::CapRights::new(full.meta, 0),
    };
    [
        errno(None),
        errno(Some(not_a_model)),
        errno(Some(with(process::CapRights::new(full.meta, 0)))),
        errno(Some(with(process::CapRights::new(
            full.meta & !meta_rights::USE,
            full.semantic,
        )))),
        errno(Some(with(full))),
    ]
}

fn neom_total_size_hint(file: &OpenFile) -> Result<usize, i64> {
    let mut header = [0u8; model::NEOM_HEADER_SIZE_V4];
    let n = file.read_at_offset(0, &mut header).map_err(vfs_errno)?;
    model::neom_total_size_hint(&header[..n]).map_err(|_| EINVAL)
}

pub(crate) fn read_model_file(file: &OpenFile) -> Result<alloc::vec::Vec<u8>, i64> {
    if let Ok(total_size) = neom_total_size_hint(file) {
        let mut bytes = alloc::vec![0u8; total_size];
        let mut offset = 0usize;
        while offset < total_size {
            let n = file
                .read_at_offset(offset, &mut bytes[offset..])
                .map_err(vfs_errno)?;
            if n == 0 {
                return Err(EINVAL);
            }
            offset = offset.checked_add(n).ok_or(EINVAL)?;
        }
        return Ok(bytes);
    }

    let mut bytes = alloc::vec::Vec::new();
    let mut chunk = alloc::vec![0u8; MAX_FILE_IO];
    loop {
        let n = file.read(&mut chunk).map_err(vfs_errno)?;
        if n == 0 {
            break;
        }
        let next_len = bytes.len().checked_add(n).ok_or(EINVAL)?;
        if next_len > model::NEOM_MAX_FILE_SIZE {
            return Err(EINVAL);
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(bytes)
}

fn sys_model_load_impl(path_ptr: *const u8, path_len: usize, flags: u32) -> i64 {
    if path_len == 0 || path_len > MAX_OPEN_PATH_LEN || path_ptr.is_null() {
        return EINVAL;
    }
    if flags != fs::OPEN_ROOT_TEMPORARY {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(path_ptr as u64, path_len) {
        return e;
    }
    let path_bytes = match copy_user_bytes(path_ptr, path_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let path = match core::str::from_utf8(&path_bytes) {
        Ok(path) => path,
        Err(_) => return EINVAL,
    };
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    model_load_path(&proc, path, flags)
}

/// `sys_model_load` after the user copy.
fn model_load_path(proc: &alloc::sync::Arc<process::Process>, path: &str, flags: u32) -> i64 {
    #[cfg(feature = "system-store")]
    if let Err(e) = system_store_check(proc, path) {
        return e;
    }
    // OT-66 K1a: a model load names a root-form path (a boot module, a
    // preloaded or NeoFS model, a file), so every source needs a RootDirCap
    // with READ.
    #[cfg(feature = "cap-rooted-open")]
    {
        let Ok(normalized) = fs::normalize_absolute_path(path) else {
            return EINVAL;
        };
        if let Err(e) = process::root_dir_cap_for(proc, &normalized, FileRights::READ) {
            return e;
        }
    }
    // B1b: `/boot/models/<file>` names a pinned boot module, not a file. The
    // model is built straight from the module (no staging Vec), after its
    // digest matched, and the module's pages are then reclaimed.
    #[cfg(feature = "model-boot-module")]
    if path.starts_with(crate::boot_model_logic::BOOT_MODEL_DIR) {
        serial_println!("model_open path={} source=boot-module", path);
        // Zenbook ask-freeze: a preloaded model is handed out, never loaded
        // here (a load in this syscall runs at IF=0 on a 32 KiB stack).
        #[cfg(feature = "boot-model-preload")]
        if let Some(result) = crate::boot_modules::preloaded(path) {
            return match result {
                Ok(model) => install_shared_model_handle(proc, model),
                Err(e) => e,
            };
        }
        return match crate::boot_modules::load_model(path, crate::sched::current_pid()) {
            Ok(model) => install_model_handle(proc, model),
            Err(e) => e,
        };
    }
    // OT-87 R2: a pinned model name on the installed NeoFS volume is held to
    // its pin. The boot preload built it already when it could; then this
    // hands out that model, as for a boot module.
    #[cfg(feature = "model-from-neofs")]
    let neofs_pin = crate::neofs_model::pin_for(path)
        .filter(|pin| matches!(pin, crate::boot_model_logic::NeofsPin::Model(_)));
    #[cfg(all(feature = "model-from-neofs", feature = "boot-model-preload"))]
    if neofs_pin.is_some()
        && let Some(result) = crate::boot_modules::preloaded_neofs(path)
    {
        serial_println!("model_open path={} source=neofs preloaded=true", path);
        return match result {
            Ok(model) => install_shared_model_handle(proc, model),
            Err(e) => e,
        };
    }
    // The NeoFS open and reads run IF-on, as in `sys_open`/`sys_read` (CP-56):
    // they take `entries`, which `sys_write` holds IF-on and preemptibly. The
    // build below stays where it was (a CP-74 question of its own, see
    // `Plans/NOTE-2026-10-25-model-load-if-on-design.md`).
    let file = match vfs_body_if_on(|| fs::open_root_temporary(path, flags, FileRights::READ)) {
        Ok(file) => file,
        Err(e) => return vfs_errno(e),
    };
    if !file.rights().contains(FileRights::READ) {
        return EACCES;
    }
    // The verified content hash against the pin, before any byte is read.
    #[cfg(feature = "model-from-neofs")]
    let neofs_checked = match neofs_pin {
        Some(pin) => match vfs_body_if_on(|| crate::neofs_model::check(&file, path, pin)) {
            Ok(checked) => Some(checked),
            Err(e) => return e,
        },
        None => None,
    };
    let provenance = file.provenance_snapshot();
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    {
        serial_println!("model_open path={}", path);
    }
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    let stage_b_load_start = super::timer::TICKS.load(Ordering::Acquire);
    let bytes = match vfs_body_if_on(|| read_model_file(&file)) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    // The bytes read are the generation that passed the pin.
    #[cfg(feature = "model-from-neofs")]
    if let Some(checked) = neofs_checked
        && let Err(e) = vfs_body_if_on(|| crate::neofs_model::recheck(&file, path, checked))
    {
        return e;
    }
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    {
        serial_println!("model_open path={} size={}", path, bytes.len());
    }
    drop(file);
    // Phase 7 BUILD-3 (ISC-3): load with content-hash dedup, owned by the loading
    // principal — a second principal loading identical immutable weights shares the
    // one physical copy instead of allocating a duplicate.
    let model = match model::Model::from_neom_bytes(
        &bytes,
        provenance,
        Some(crate::sched::current_pid()),
    ) {
        Ok(model) => model,
        Err(e) => return model::model_errno(e),
    };
    #[cfg(any(
        feature = "neo-test-arthur-real-brain-boot-15b",
        feature = "neo-test-arthur-real-brain-boot-3b",
        feature = "neo-test-arthur-real-brain-boot-qwen35"
    ))]
    log_stage_b_model_config(&model);
    #[cfg(all(
        feature = "neo-test-arthur-real-brain-boot",
        not(feature = "neo-test-arthur-real-brain-boot-15b"),
        not(feature = "neo-test-arthur-real-brain-boot-3b"),
        not(feature = "neo-test-arthur-real-brain-boot-qwen35")
    ))]
    {
        let elapsed_ms = super::timer::TICKS
            .load(Ordering::Acquire)
            .saturating_sub(stage_b_load_start)
            .saturating_mul(10);
        serial_println!(
            "model_load_resident bytes={} ms={}",
            bytes.len(),
            elapsed_ms
        );
    }
    #[cfg(any(
        feature = "neo-test-arthur-real-brain-boot-15b",
        feature = "neo-test-arthur-real-brain-boot-3b",
        feature = "neo-test-arthur-real-brain-boot-qwen35"
    ))]
    {
        let elapsed_ms = super::timer::TICKS
            .load(Ordering::Acquire)
            .saturating_sub(stage_b_load_start)
            .saturating_mul(10);
        serial_println!(
            "model_load_resident bytes={} heap_cap={} ms={}",
            bytes.len(),
            crate::mm::heap::HEAP_SIZE,
            elapsed_ms
        );
    }
    install_model_handle(proc, model)
}

/// Install a loaded model as a `Handle::Model` in `proc`, returning its index.
fn install_model_handle(proc: &alloc::sync::Arc<process::Process>, model: model::Model) -> i64 {
    install_shared_model_handle(proc, alloc::sync::Arc::new(model))
}

/// A handle to a model other handles may share (a preloaded boot model).
fn install_shared_model_handle(
    proc: &alloc::sync::Arc<process::Process>,
    model: alloc::sync::Arc<model::Model>,
) -> i64 {
    let handle = Handle::Model {
        model,
        cap_id: process::alloc_cap_id(),
        rights: process::full_model_rights(),
    };
    match process::install_cap(proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

// Model-agnostic: logs whatever dims the `.neom` header + tensor shapes derive
// (F2), so it serves both the 1.5B and 3B config-swap boots unchanged.
#[cfg(any(
    feature = "neo-test-arthur-real-brain-boot-15b",
    feature = "neo-test-arthur-real-brain-boot-3b",
    feature = "neo-test-arthur-real-brain-boot-qwen35"
))]
fn log_stage_b_model_config(model: &model::Model) {
    match model.model_config() {
        Some(config) => {
            let rope_theta = if config.rope_theta.to_bits() == 1_000_000.0f32.to_bits() {
                1_000_000u64
            } else if config.rope_theta.to_bits() == 10_000_000.0f32.to_bits() {
                10_000_000u64
            } else {
                0
            };
            serial_println!(
                "config layer_count={} head_dim={} rope_theta={} hidden={} intermediate={} q_heads={} kv_heads={} vocab={} context={} rms_norm_eps_bits=0x{:08x}",
                config.layer_count,
                config.head_dim,
                rope_theta,
                config.hidden_size,
                config.intermediate_size,
                config.q_heads,
                config.kv_heads,
                config.vocab,
                config.context,
                config.rms_norm_eps.to_bits()
            );
        }
        None => {
            serial_println!("config missing");
        }
    }
}

/// A model's provenance check (`Model::validate_current`). For a model loaded
/// from NeoFS it resolves the file's name again, taking NeoFS `entries`, which
/// `sys_write` holds IF-on and preemptibly, so it runs IF-on like the file
/// syscalls (CP-56). A boot-module model's provenance has no mount and no
/// generation, so its check returns at once and touches neither NeoFS nor
/// `MOUNTS`.
fn model_still_current(model: &model::Model) -> Result<(), model::ModelError> {
    vfs_body_if_on(|| model.validate_current())
}

/// `sys_model_query`'s body after the handle lookup.
fn model_query_desc(h: u32) -> Result<ModelQueryDesc, i64> {
    let model = model_lookup(h)?;
    model_still_current(&model).map_err(model::model_errno)?;
    Ok(model.query_desc())
}

/// `sys_model_query` into a kernel buffer: the same body.
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub fn debug_sys_model_query_for_test(h: u32) -> Result<ModelQueryDesc, i64> {
    model_query_desc(h)
}

fn sys_model_query_impl(h: u32, out: *mut u8, out_len: usize) -> i64 {
    let expected_len = core::mem::size_of::<ModelQueryDesc>();
    if out.is_null() || out_len != expected_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, expected_len) {
        return e;
    }
    match model_query_desc(h) {
        Ok(desc) => match copy_model_query_to_user(out, desc) {
            Ok(()) => 0,
            Err(e) => e,
        },
        Err(e) => e,
    }
}

fn sys_model_drop_impl(h: u32) -> i64 {
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let removed = process::close_handle(&proc, h, |h| matches!(h, Handle::Model { .. }));
    match removed {
        Ok(Handle::Model { .. }) => 0,
        Ok(_) => EBADF,
        Err(e) => e,
    }
}

fn infer_errno(err: exec::InferenceError) -> i64 {
    match err {
        exec::InferenceError::InvalidInput
        | exec::InferenceError::InvalidGraph
        | exec::InferenceError::Op(_)
        | exec::InferenceError::Accel(_) => EINVAL,
        exec::InferenceError::UnsupportedOp => ENOSYS,
        exec::InferenceError::Tensor(err) => tensor_errno(err),
        exec::InferenceError::Cancelled => crate::errno::ECANCELED,
    }
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_LIVENESS_DELTA: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_CONTEXT_SWITCH_DELTA: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_DISPATCH_PREEMPTIONS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_DISPATCH_ENABLED: AtomicBool = AtomicBool::new(true);
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_INFER_IDENTITY: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "neo-test-inference-preemptible-liveness")]
static INFERENCE_PREEMPTIBLE_GENERATE_IDENTITY: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub(crate) struct InferencePreemptibleDispatchProbe {
    tick_start: u64,
    switch_start: u64,
    if_enabled: bool,
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
struct InferencePreemptibleDispatchRestore {
    prev: bool,
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
impl Drop for InferencePreemptibleDispatchRestore {
    fn drop(&mut self) {
        INFERENCE_PREEMPTIBLE_DISPATCH_ENABLED.store(self.prev, Ordering::Release);
    }
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub fn debug_inference_preemptible_liveness_delta() -> u64 {
    INFERENCE_PREEMPTIBLE_LIVENESS_DELTA.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub fn debug_inference_preemptible_context_switch_delta() -> u64 {
    INFERENCE_PREEMPTIBLE_CONTEXT_SWITCH_DELTA.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub fn debug_inference_preemptible_infer_identity_ok() -> bool {
    INFERENCE_PREEMPTIBLE_INFER_IDENTITY.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub fn debug_inference_preemptible_generate_identity_ok() -> bool {
    INFERENCE_PREEMPTIBLE_GENERATE_IDENTITY.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub(crate) fn inference_preemptible_dispatch_enabled_for_test() -> bool {
    INFERENCE_PREEMPTIBLE_DISPATCH_ENABLED.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
fn without_inference_preemptible_dispatch<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let prev = INFERENCE_PREEMPTIBLE_DISPATCH_ENABLED.swap(false, Ordering::AcqRel);
    let _restore = InferencePreemptibleDispatchRestore { prev };
    f()
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
fn inference_preemptible_dispatch_preemptions() -> u64 {
    INFERENCE_PREEMPTIBLE_DISPATCH_PREEMPTIONS.load(Ordering::Acquire)
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub(crate) fn begin_inference_preemptible_dispatch_probe() -> InferencePreemptibleDispatchProbe {
    let if_enabled = super::interrupts_enabled();
    if !if_enabled {
        serial_println!(
            "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=compute_window_if_disabled scope=cpu_dispatch"
        );
    }
    InferencePreemptibleDispatchProbe {
        tick_start: super::timer::TICKS.load(Ordering::Acquire),
        switch_start: crate::sched::context_switch_count_for_test(),
        if_enabled,
    }
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
pub(crate) fn finish_inference_preemptible_dispatch_probe(
    probe: InferencePreemptibleDispatchProbe,
) -> bool {
    if !probe.if_enabled {
        return false;
    }

    let end = super::timer::TICKS.load(Ordering::Acquire);
    let switch_end = crate::sched::context_switch_count_for_test();
    let delta = end.saturating_sub(probe.tick_start);
    let switch_delta = switch_end.saturating_sub(probe.switch_start);
    if delta == 0 || switch_delta == 0 {
        return false;
    }

    INFERENCE_PREEMPTIBLE_LIVENESS_DELTA.store(delta, Ordering::Release);
    INFERENCE_PREEMPTIBLE_CONTEXT_SWITCH_DELTA.store(switch_delta, Ordering::Release);
    let preemptions = INFERENCE_PREEMPTIBLE_DISPATCH_PREEMPTIONS.fetch_add(1, Ordering::AcqRel) + 1;
    serial_println!(
        "NEO_TEST/1 event=INFO name=inference_preemptible_liveness tick_delta={} context_switch_delta={} scope=cpu_dispatch dispatch_preemptions={}",
        delta,
        switch_delta,
        preemptions
    );
    true
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
fn require_inference_preemptible_dispatch_preempted_since(
    start: u64,
    path: &str,
) -> Result<(), i64> {
    let end = inference_preemptible_dispatch_preemptions();
    if end == start {
        serial_println!(
            "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=no_context_switch_inside_compute path={} dispatch_preemptions_start={}",
            path,
            start
        );
        return Err(EAGAIN);
    }
    Ok(())
}

#[cfg(feature = "neo-test-inference-preemptible-liveness")]
fn tensor_outputs_match(
    left: &[alloc::sync::Arc<tensor::Tensor>],
    right: &[alloc::sync::Arc<tensor::Tensor>],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right.iter()).all(|(a, b)| {
            a.dtype() == b.dtype() && a.shape() == b.shape() && a.bytes() == b.bytes()
        })
}

/// `SYS_INFER_RUN`'s graph-model inputs: the handle array, then each tensor.
fn graph_infer_inputs(
    inputs_ptr: *const u32,
    input_len: usize,
) -> Result<alloc::vec::Vec<alloc::sync::Arc<tensor::Tensor>>, i64> {
    let input_handles = if input_len == 0 {
        alloc::vec::Vec::new()
    } else {
        copy_user_u32s(inputs_ptr, input_len)?
    };
    let mut inputs = alloc::vec::Vec::with_capacity(input_len);
    for handle in input_handles {
        let (tensor, _, _) = tensor_lookup(handle)?;
        inputs.push(tensor);
    }
    Ok(inputs)
}

/// `SYS_INFER_RUN` as the dispatcher runs it: the arm, then the CP-66
/// boundary. A caller cancelled in (or killed while inside) the accelerator
/// admission section returns here with every frame of the arm unwound — the
/// request's `Arc`s dropped — and, its process being torn down, exits instead
/// of returning to ring 3.
fn sys_infer_run_syscall(
    model_h: u32,
    inputs_ptr: *const u32,
    input_count: u32,
    outputs_ptr: *mut u32,
    output_cap: u32,
    flags: u32,
) -> i64 {
    let ret = sys_infer_run_impl(
        model_h,
        inputs_ptr,
        input_count,
        outputs_ptr,
        output_cap,
        flags,
    );
    if crate::ai::admission::CANCEL_ON && crate::sched::process::current_torn_down() {
        #[cfg(feature = "neo-test-admission-park-kill")]
        ADMISSION_BOUNDARY_EXITS.fetch_add(1, Ordering::AcqRel);
        crate::sched::exit();
    }
    ret
}

/// CP-66 discriminator: threads that exited at the `SYS_INFER_RUN` boundary.
#[cfg(feature = "neo-test-admission-park-kill")]
pub(crate) static ADMISSION_BOUNDARY_EXITS: core::sync::atomic::AtomicU64 =
    core::sync::atomic::AtomicU64::new(0);

/// CP-66 discriminator: one `SYS_INFER_RUN` exactly as the dispatcher runs it,
/// for the scenario's stand-in process threads.
#[cfg(feature = "neo-test-admission-park-kill")]
pub(crate) fn infer_run_syscall_for_test(
    model_h: u32,
    inputs: u64,
    input_count: u32,
    outputs: u64,
    output_cap: u32,
) -> i64 {
    sys_infer_run_syscall(
        model_h,
        inputs as *const u32,
        input_count,
        outputs as *mut u32,
        output_cap,
        0,
    )
}

/// CP-66 discriminator: a stand-in caller process with one user page mapped
/// at [`ADMISSION_KILL_VA`] (input handle at +0, output handles at +0x40).
#[cfg(any(
    feature = "neo-test-admission-park-kill",
    feature = "neo-test-ipc-park-kill",
    feature = "neo-test-kill-at-exit"
))]
pub(crate) fn admission_kill_process_for_test(
    page: x86_64::structures::paging::PhysFrame,
) -> alloc::sync::Arc<process::Process> {
    use x86_64::structures::paging::PageTableFlags as F;
    let pmap = crate::mm::pmap::Pmap::new().expect("admission kill: pmap");
    pmap.map_user(
        x86_64::VirtAddr::new(ADMISSION_KILL_VA),
        page,
        F::PRESENT | F::WRITABLE | F::USER_ACCESSIBLE | F::NO_EXECUTE,
    )
    .expect("admission kill: map_user");
    process::register(process::Process::new_user("admission-kill", pmap))
}

/// CP-67: a kernel-built stand-in thread's syscall, entered through the real
/// dispatcher (its entry work, then the arm) exactly as a ring-3 `syscall`
/// reaches it.
#[cfg(any(feature = "neo-test-ipc-park-kill", feature = "neo-test-kill-at-exit"))]
pub(crate) fn syscall_dispatch_for_test(num: u64, a0: u64, a1: u64, a2: u64) -> i64 {
    syscall_dispatch(num, a0, a1, a2, 0, 0, 0)
}

#[cfg(any(
    feature = "neo-test-admission-park-kill",
    feature = "neo-test-ipc-park-kill",
    feature = "neo-test-kill-at-exit"
))]
pub(crate) const ADMISSION_KILL_VA: u64 = 0x0000_0011_0000_0000;

/// CP-66 discriminator: while armed, an admitted graph caller yields inside
/// its work (before the forward) — a holder that is Ready mid-dispatch, as a
/// preemption leaves it — until disarmed.
#[cfg(feature = "neo-test-admission-park-kill")]
pub(crate) static ADMISSION_HOLD_ARMED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
#[cfg(feature = "neo-test-admission-park-kill")]
pub(crate) static ADMISSION_HOLDING: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "neo-test-admission-park-kill")]
fn admission_hold_for_test() {
    while ADMISSION_HOLD_ARMED.load(Ordering::Acquire) {
        ADMISSION_HOLDING.store(true, Ordering::Release);
        crate::sched::yield_now();
    }
}

fn sys_infer_run_impl(
    model_h: u32,
    inputs_ptr: *const u32,
    input_count: u32,
    outputs_ptr: *mut u32,
    output_cap: u32,
    flags: u32,
) -> i64 {
    if flags != 0 {
        return EINVAL;
    }
    let model = match model_lookup(model_h) {
        Ok(found) => found,
        Err(e) => return e,
    };
    if let Err(e) = model_still_current(&model) {
        return model::model_errno(e);
    }
    #[cfg(feature = "real-lm")]
    let stage_b_inventory = is_stage_b_inventory_model(&model);
    #[cfg(not(feature = "real-lm"))]
    let stage_b_inventory = false;

    // B1c (Jarvis re-check #2, PM ruling): only the inventory route, which
    // runs on the inference stack, carries the lifecycle marker; it is applied
    // here, as soon as the route is known and before any allocation on it.
    // The model `Arc` was taken just above with the thread Running at IF=0 and
    // no yield since, so it was never killable while unmarked. The graph route
    // is exactly `main`'s: no marker, no participant count.
    #[cfg(feature = "inference-stack")]
    if stage_b_inventory {
        return inference_syscall(move || {
            infer_run_routed(
                model,
                true,
                inputs_ptr,
                input_count,
                outputs_ptr,
                output_cap,
            )
        });
    }
    infer_run_routed(
        model,
        stage_b_inventory,
        inputs_ptr,
        input_count,
        outputs_ptr,
        output_cap,
    )
}

/// The inventory route's forward on the inference stack. `Err(ECANCELED)` if
/// the caller's process was torn down: while it waited for the stack, or after
/// it won the stack but before the forward (B1c re-check #2, C1).
#[cfg(feature = "real-lm")]
#[allow(clippy::type_complexity)]
fn inventory_infer(
    model: &model::Model,
    inputs: &[alloc::sync::Arc<tensor::Tensor>],
) -> Result<
    Result<alloc::vec::Vec<alloc::sync::Arc<tensor::Tensor>>, crate::ai::exec::InferenceError>,
    i64,
> {
    on_inference_stack(Err(crate::errno::ECANCELED), || {
        if inference_cancelled() {
            return Err(crate::errno::ECANCELED);
        }
        Ok(crate::ai::qwen_inventory::run(model, inputs))
    })
}

/// `SYS_INFER_RUN` after its route is classified (`stage_b_inventory`).
fn infer_run_routed(
    model: alloc::sync::Arc<model::Model>,
    stage_b_inventory: bool,
    inputs_ptr: *const u32,
    input_count: u32,
    outputs_ptr: *mut u32,
    output_cap: u32,
) -> i64 {
    let expected_input_count = if stage_b_inventory {
        1
    } else {
        model.input_count()
    };
    let expected_output_count = if stage_b_inventory {
        1
    } else {
        model.output_count()
    };
    if input_count != expected_input_count {
        return EINVAL;
    }
    if output_cap < expected_output_count {
        return E2BIG;
    }

    let input_len = match usize::try_from(input_count) {
        Ok(count) => count,
        Err(_) => return EINVAL,
    };
    let output_len = match usize::try_from(expected_output_count) {
        Ok(count) => count,
        Err(_) => return EINVAL,
    };
    let input_bytes = match input_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(len) => len,
        None => return EINVAL,
    };
    let output_bytes = match output_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(len) => len,
        None => return EINVAL,
    };
    if input_len != 0 {
        if inputs_ptr.is_null() {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(inputs_ptr as u64, input_bytes) {
            return e;
        }
    }
    if output_len != 0 {
        if outputs_ptr.is_null() {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(outputs_ptr as u64, output_bytes) {
            return e;
        }
    }

    // B1c (AAG J2): the inventory route admits its one token tensor before
    // any variable-size user copy or heap allocation. Its order: flags
    // (register); the model lookup, which classifies the route and allocates
    // nothing; the input count (register); the fixed 4-byte handle read into
    // a stack array; the tensor lookup (no allocation); the 2,048-token cap.
    // Only then is `inputs` built.
    #[cfg(feature = "real-lm")]
    let inputs = if stage_b_inventory {
        let handle = match copy_user_array::<4>(inputs_ptr.cast::<u8>()) {
            Ok(bytes) => u32::from_le_bytes(bytes),
            Err(e) => return e,
        };
        let (tensor, _, _) = match tensor_lookup(handle) {
            Ok(found) => found,
            Err(e) => return e,
        };
        if crate::ai::qwen_inventory::admit_input(&tensor).is_err() {
            return EINVAL;
        }
        alloc::vec![tensor]
    } else {
        match graph_infer_inputs(inputs_ptr, input_len) {
            Ok(inputs) => inputs,
            Err(e) => return e,
        }
    };
    #[cfg(not(feature = "real-lm"))]
    let inputs = match graph_infer_inputs(inputs_ptr, input_len) {
        Ok(inputs) => inputs,
        Err(e) => return e,
    };

    // Route the live inference through the §9.6 admission scheduler (BUILD-2L): the
    // caller is admitted immediately when the accelerator is free (single-tenant fast
    // path — no parking, no added latency) or **parks** (no busy-wait) until the
    // principal currently holding the device completes, then runs and releases the
    // device to the next pending request. The device/principal are the same ones the
    // accounting hooks (§9.5) attribute against, so scheduling and measurement agree.
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    let irq_off_outputs = without_inference_preemptible_dispatch(|| {
        crate::arch::x86_64::without_interrupts(|| {
            #[cfg(feature = "real-lm")]
            {
                if stage_b_inventory {
                    // B1c (AAG F-A): on the inference stack, never the 32 KiB thread stack.
                    inventory_infer(&model, &inputs)
                        .unwrap_or(Err(crate::ai::exec::InferenceError::InvalidInput))
                } else {
                    run_graph_infer(&model, &inputs)
                }
            }
            #[cfg(not(feature = "real-lm"))]
            {
                run_graph_infer(&model, &inputs)
            }
        })
    });
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    let irq_off_outputs = match irq_off_outputs {
        Ok(outputs) => outputs,
        Err(e) => {
            let errno = infer_errno(e);
            serial_println!(
                "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=irq_off_infer_failed errno={}",
                errno
            );
            return errno;
        }
    };

    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    let dispatch_preemptions_start = inference_preemptible_dispatch_preemptions();
    let outputs = {
        #[cfg(feature = "real-lm")]
        {
            if stage_b_inventory {
                // B1c (AAG F-A): on the inference stack, never the 32 KiB thread stack.
                // A cancelled caller returns `ECANCELED` (re-check #2, C1).
                match inventory_infer(&model, &inputs) {
                    Ok(outputs) => outputs,
                    Err(errno) => return errno,
                }
            } else {
                run_graph_infer(&model, &inputs)
            }
        }
        #[cfg(not(feature = "real-lm"))]
        {
            run_graph_infer(&model, &inputs)
        }
    };
    let outputs = match outputs {
        Ok(outputs) => outputs,
        Err(e) => {
            let errno = infer_errno(e);
            #[cfg(feature = "neo-test-arthur-real-brain-boot")]
            serial_println!("infer_run error err={:?} errno={}", e, errno);
            return errno;
        }
    };
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    if let Err(errno) = require_inference_preemptible_dispatch_preempted_since(
        dispatch_preemptions_start,
        "sys_infer_run",
    ) {
        return errno;
    }
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    if !tensor_outputs_match(&irq_off_outputs, &outputs) {
        serial_println!(
            "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=token_identity_mismatch"
        );
        return EINVAL;
    }
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    INFERENCE_PREEMPTIBLE_INFER_IDENTITY.store(true, Ordering::Release);
    #[cfg(feature = "neo-test-inference-preemptible-liveness")]
    serial_println!(
        "NEO_TEST/1 event=INFO name=inference_preemptible_liveness token_identity=sys_infer_run_eq"
    );
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    serial_println!("infer_run outputs_ready count={}", outputs.len());
    if outputs.len() != output_len {
        return EINVAL;
    }

    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let mut out_handles: alloc::vec::Vec<u32> = alloc::vec::Vec::with_capacity(outputs.len());
    for tensor in outputs {
        let meta = tensor.meta();
        let slot = crate::ai::arena::alloc(tensor, crate::ai::arena::default_home());
        let handle = Handle::Tensor {
            slot,
            meta,
            cap_id: process::alloc_cap_id(),
            rights: process::full_tensor_rights(),
        };
        let installed = process::install_cap(&proc, handle, None);
        // The alloc's creator count transfers to the table entry; on install
        // failure this release frees the slot (no leak on the error path).
        crate::ai::arena::release(slot);
        let idx = match installed {
            Ok(idx) => idx,
            Err(_) => return EINVAL,
        };
        let idx = match u32::try_from(idx) {
            Ok(idx) => idx,
            Err(_) => return EINVAL,
        };
        out_handles.push(idx);
    }

    if output_bytes != 0 {
        // SAFETY: out_handles is a contiguous Vec<u32>; copying its raw bytes
        // preserves the syscall ABI's little-endian handle array.
        let bytes =
            unsafe { core::slice::from_raw_parts(out_handles.as_ptr().cast::<u8>(), output_bytes) };
        if let Err(e) = copy_kernel_bytes_to_user(outputs_ptr.cast::<u8>(), bytes) {
            return e;
        }
    }
    #[cfg(feature = "neo-test-arthur-real-brain-boot")]
    serial_println!(
        "infer_run outputs_installed count={} first_handle={}",
        output_len,
        out_handles.first().copied().unwrap_or(0)
    );
    output_len as i64
}

/// OT-28 kill-switch — the KV-cache mode for inventory (real-LM) generation.
///
/// `Persistent` is the fast O(N) path that production ALWAYS uses: prefill the
/// prompt once, retain one `Qwen2KvCacheSet`, and advance a single token per
/// step (landed in `e3a55753`). `ReprefillBaseline` is the *disabled* mode — the
/// retain-vs-fresh baseline: it rebuilds the KV cache from the full growing
/// context on every step through the same `InventoryDecodeState` attention math.
/// A shared-math bug is therefore outside this differential and remains anchored
/// by the qwen35 llama.cpp golden. The two modes MUST produce bit-identical token
/// ids; the armed `sys_generate_kv_persist_equivalence` QEMU KAT proves that
/// property and its corruption control proves the KAT can go red (Rule-1).
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum KvGenMode {
    Persistent,
    #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
    PersistentCorruptedKat,
    // B1c (F3): the disabled O(N^2) baseline exists only for the equivalence
    // KAT; a product real-LM kernel carries the persistent path alone.
    #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
    ReprefillBaseline,
}

/// Prefill the persistent KV state: every prompt token advances the cache, and
/// only the last computes logits (Arthur latency NOTE, fix 3). The cache and the
/// last token's logits are bit-identical to advancing every token with logits;
/// `sys_generate_kv_persist_equivalence` and the host prefill KAT hold that.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
fn prefill_persistent(
    model: &model::Model,
    state: &mut crate::ai::qwen_inventory::InventoryDecodeState,
    prompt: &[u32],
) -> Result<alloc::vec::Vec<f32>, i64> {
    crate::ai::decode_logic::prefill_last_logits(prompt, |token, want_logits| {
        state.step(model, token, want_logits)
    })
    .map_err(infer_errno)
}

/// The per-step logits source for inventory generation. Both variants drive the
/// SAME forward math through the SAME `qwen_inventory` helpers — they differ ONLY
/// in whether the attention KV (and GDN recurrent) state is *reused* across steps
/// (`Persistent`) or *rebuilt from the growing context* each step (`Reprefill`).
/// The shared generation loop below is therefore byte-identical between modes
/// except for this source, which is exactly what the equivalence KAT pins.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
enum InventoryLogitsSource {
    Persistent(crate::ai::qwen_inventory::InventoryDecodeState),
    #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
    Reprefill {
        context: alloc::vec::Vec<u32>,
    },
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
impl InventoryLogitsSource {
    /// Prefill the prompt and return the logits that predict the first generated
    /// token. `Persistent` builds one decode state sized for the whole run and
    /// advances over the prompt; `Reprefill` seeds a context and evaluates it in
    /// full (a fresh decode state internally, discarded each call — the O(N^2)
    /// baseline).
    fn prefill(
        model: &model::Model,
        prompt: &[u32],
        max_context: usize,
        mode: KvGenMode,
    ) -> Result<(Self, alloc::vec::Vec<f32>), i64> {
        match mode {
            KvGenMode::Persistent => {
                let mut state =
                    crate::ai::qwen_inventory::InventoryDecodeState::new(model, max_context)
                        .map_err(infer_errno)?;
                let logits = prefill_persistent(model, &mut state, prompt)?;
                Ok((Self::Persistent(state), logits))
            }
            #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
            KvGenMode::PersistentCorruptedKat => {
                let mut state =
                    crate::ai::qwen_inventory::InventoryDecodeState::new(model, max_context)
                        .map_err(infer_errno)?;
                let logits = prefill_persistent(model, &mut state, prompt)?;
                if !state.corrupt_retained_key_for_kat().map_err(infer_errno)? {
                    return Err(EINVAL);
                }
                Ok((Self::Persistent(state), logits))
            }
            #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
            KvGenMode::ReprefillBaseline => {
                let context = prompt.to_vec();
                let logits =
                    crate::ai::qwen_inventory::next_logits(model, &context).map_err(infer_errno)?;
                Ok((Self::Reprefill { context }, logits))
            }
        }
    }

    /// Feed the just-sampled token and return the next-token logits. `Persistent`
    /// advances the retained cache by one position; `Reprefill` appends to the
    /// context and re-evaluates the whole prefix.
    fn feed(&mut self, model: &model::Model, token: u32) -> Result<alloc::vec::Vec<f32>, i64> {
        match self {
            Self::Persistent(state) => state.advance(model, token).map_err(infer_errno),
            #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
            Self::Reprefill { context } => {
                context.push(token);
                crate::ai::qwen_inventory::next_logits(model, context).map_err(infer_errno)
            }
        }
    }

    /// OT-16 execution stats are only produced by the `Persistent` decode state
    /// (the reprefill baseline discards its state each step).
    fn execution_stats(&self) -> Option<crate::ai::qwen_inventory::InventoryExecutionStats> {
        match self {
            Self::Persistent(state) => Some(state.execution_stats()),
            #[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
            Self::Reprefill { .. } => None,
        }
    }
}

/// The shared inventory generation core: prefill -> greedy argmax -> append ->
/// stop at EOS/max, parameterized by the KV-cache `mode`. `sys_generate_impl`
/// calls this with `Persistent` (production). The equivalence KAT calls it with
/// BOTH modes and asserts identical token ids — so the KAT proves *this exact
/// code path*, not a reimplementation. Returns the generated tokens, the
/// first-forward top-5 (for the OT-37 boot-vs-host diff instrument), and the
/// `Persistent`-only execution stats.
/// A9-B: the repetition penalty the generation core applies: the product's,
/// or the guard scenario's test value (its Python oracle is computed at it).
#[cfg(all(
    feature = "decode-repetition-guard",
    not(feature = "neo-test-real-lm-generate-guard")
))]
const DECODE_PENALTY: f32 = crate::ai::repetition_logic::DECODE_PENALTY;
#[cfg(feature = "neo-test-real-lm-generate-guard")]
const DECODE_PENALTY: f32 = 1.3;

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence"
))]
#[allow(clippy::type_complexity)]
fn generate_inventory_greedy(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
    eos: Option<u32>,
    mode: KvGenMode,
    rng: &mut crate::ai::generate::Pcg32,
) -> Result<
    (
        alloc::vec::Vec<u32>,
        Option<[(usize, f32); 5]>,
        Option<crate::ai::qwen_inventory::InventoryExecutionStats>,
    ),
    i64,
> {
    use crate::ai::generate::{SamplingConfig, sample_next_token};

    let max_context = prompt.len().checked_add(max_new).ok_or(EINVAL)?;
    if inference_cancelled() {
        return Err(crate::errno::ECANCELED);
    }
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::Prefill);
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::begin_generation();
    #[cfg(feature = "prefill-perf")]
    let perf_t0 = crate::ai::prefill_perf::tsc();
    let (mut source, mut logits) =
        InventoryLogitsSource::prefill(model, prompt, max_context, mode)?;
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::prefill_done(
        prompt.len(),
        crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0),
    );
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::Decode);
    #[cfg_attr(not(feature = "neo-test"), allow(unused_mut))]
    let mut first_top5 = None;
    let mut generated = alloc::vec::Vec::new();
    for step in 0..max_new {
        #[cfg(feature = "freeze-watchdog")]
        crate::watchdog::set_step(step as u64, max_new as u64);
        // B1c AAG J3: a torn-down caller stops between tokens.
        if inference_cancelled() {
            return Err(crate::errno::ECANCELED);
        }
        // B1c (F3): the top-5 instrument is computed only where it is logged.
        #[cfg(feature = "neo-test")]
        if step == 0 {
            first_top5 = Some(forward0_top5(&logits));
        }
        // A9-B: push recently generated tokens down before the pick.
        #[cfg(feature = "decode-repetition-guard")]
        crate::ai::repetition_logic::penalize(
            &mut logits,
            &generated,
            crate::ai::repetition_logic::REPETITION_WINDOW,
            DECODE_PENALTY,
        );
        let next = match sample_next_token(&logits, SamplingConfig::greedy(), rng) {
            Ok(next) => next,
            Err(_) => return Err(EINVAL),
        };
        generated.push(next);
        if eos == Some(next) {
            break;
        }
        // A9-B: a reply whose tail repeats an earlier run ends before it.
        #[cfg(feature = "decode-repetition-guard")]
        if let Some(keep) = crate::ai::repetition_logic::loop_end(
            &generated,
            crate::ai::repetition_logic::LOOP_NGRAM,
        ) {
            generated.truncate(keep);
            break;
        }
        if step + 1 < max_new {
            #[cfg(feature = "prefill-perf")]
            let perf_t0 = crate::ai::prefill_perf::tsc();
            logits = source.feed(model, next)?;
            #[cfg(feature = "prefill-perf")]
            crate::ai::prefill_perf::decode_token(
                crate::ai::prefill_perf::tsc().wrapping_sub(perf_t0),
            );
        }
    }
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::GenDone);
    #[cfg(feature = "prefill-perf")]
    crate::ai::prefill_perf::end_generation();
    Ok((generated, first_top5, source.execution_stats()))
}

/// Result of the OT-28 fast≡slow equivalence check: the token ids the two KV
/// modes produced, whether they are bit-identical, and how many distinct tokens
/// the sequence contains (a meaningfulness guard — a degenerate all-one-token
/// run would agree trivially even with a broken cache, so the KAT requires the
/// sequence to actually depend on the accumulated context).
#[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
pub(crate) struct KvPersistEquivReport {
    pub persistent_tokens: alloc::vec::Vec<u32>,
    pub reprefill_tokens: alloc::vec::Vec<u32>,
    pub identical: bool,
    pub distinct_token_count: usize,
}

/// Run the SAME real generation core (the one `sys_generate_impl` uses) both
/// ways — `Persistent` (fast, production) and `ReprefillBaseline` (the disabled
/// O(N^2) path) — on `model`+`prompt`, and report whether the token ids match.
/// Greedy sampling is pure argmax, so any divergence is a genuine KV-coherence
/// defect, not sampler noise. This is the armed, falsifiable regression behind
/// the OT-28 kill-switch.
#[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
pub(crate) fn kv_persist_equivalence_check(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
    eos: Option<u32>,
) -> Result<KvPersistEquivReport, i64> {
    kv_persist_equivalence_check_with_mode(model, prompt, max_new, eos, KvGenMode::Persistent)
}

/// Negative control for the equivalence KAT: corrupt one retained K position
/// after prefill, then prove the same comparison reports a token mismatch.
#[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
pub(crate) fn kv_persist_equivalence_negative_control(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
    eos: Option<u32>,
) -> Result<KvPersistEquivReport, i64> {
    kv_persist_equivalence_check_with_mode(
        model,
        prompt,
        max_new,
        eos,
        KvGenMode::PersistentCorruptedKat,
    )
}

#[cfg(feature = "neo-test-sys-generate-kv-persist-equivalence")]
fn kv_persist_equivalence_check_with_mode(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
    eos: Option<u32>,
    persistent_mode: KvGenMode,
) -> Result<KvPersistEquivReport, i64> {
    use crate::ai::generate::Pcg32;

    // Independent RNGs seeded identically. Greedy ignores the RNG, but keeping
    // them separate + equal makes the two runs differ ONLY in the KV mode.
    let mut rng_fast = Pcg32::seeded(0x853c_49e6_748f_ea9b, 0xda3e_39cb_94b9_5bdb);
    let mut rng_slow = Pcg32::seeded(0x853c_49e6_748f_ea9b, 0xda3e_39cb_94b9_5bdb);
    let (persistent_tokens, _, _) =
        generate_inventory_greedy(model, prompt, max_new, eos, persistent_mode, &mut rng_fast)?;
    let (reprefill_tokens, _, _) = generate_inventory_greedy(
        model,
        prompt,
        max_new,
        eos,
        KvGenMode::ReprefillBaseline,
        &mut rng_slow,
    )?;
    let identical = persistent_tokens == reprefill_tokens;
    let mut distinct: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
    for &token in &persistent_tokens {
        if !distinct.contains(&token) {
            distinct.push(token);
        }
    }
    Ok(KvPersistEquivReport {
        identical,
        distinct_token_count: distinct.len(),
        persistent_tokens,
        reprefill_tokens,
    })
}

/// OT-37 — autoregressive generation: forward -> greedy argmax -> append -> stop
/// at EOS/max. Reuses the PROVEN forward (qwen_inventory for the real inventory
/// brain; the graph `ai::generate` for the hermetic fixture) and the OT-30 greedy
/// sampler. The forward/matmul is untouched — this is the sampling-stop path the
/// boot was missing (it had stubbed generation to a hardcoded EOS). `packed`
/// carries `max_new_tokens` in the low 32 bits and `eos_token_id` (u32::MAX = no
/// EOS) in the high 32. Returns the count of generated tokens written to `out`.
/// B1c: the product real-LM generation core, shared by `SYS_GENERATE` and the
/// `real_lm_generate_boot` scenario. Bounded first (no copy or allocation for
/// an oversized request), inventory models only (fail closed), then the
/// OT-28 persistent-KV greedy loop.
#[cfg(feature = "real-lm")]
#[allow(clippy::type_complexity)]
pub(crate) fn real_lm_generate(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
    eos: Option<u32>,
) -> Result<
    (
        alloc::vec::Vec<u32>,
        Option<[(usize, f32); 5]>,
        Option<crate::ai::qwen_inventory::InventoryExecutionStats>,
    ),
    i64,
> {
    use crate::ai::generate::Pcg32;

    use crate::ai::generate_logic::{GenerateRequest, check_generate_request};
    if check_generate_request(GenerateRequest::Plain {
        prompt_len: prompt.len(),
        max_new,
    })
    .is_err()
    {
        return Err(EINVAL);
    }
    if prompt.iter().any(|&t| t > i32::MAX as u32) {
        return Err(EINVAL);
    }
    let mut rng = Pcg32::seeded(0x853c_49e6_748f_ea9b, 0xda3e_39cb_94b9_5bdb);
    // A9-D test kernel only (`desktop_busy_boot`): Arthur answers from the
    // tiny KAT inventory LM, whose vocabulary is a dozen ids, so its Qwen
    // chat prompt is replaced by the KAT oracle's prompt. The generation
    // itself (the product core and its fork-join) is unchanged: the
    // scenario needs a long real generation, not its words.
    // `max_new` is cut to 64, the length `generate_corunner_liveness` runs
    // it at (the KAT was built for short runs): about 56 s of generation per
    // answer under TCG.
    #[cfg(feature = "neo-test-desktop-busy-boot")]
    let prompt: &[u32] = &[1, 5, 9, 2];
    #[cfg(feature = "neo-test-desktop-busy-boot")]
    let max_new = max_new.min(64);
    #[cfg(feature = "neo-test-desktop-busy-boot")]
    crate::serial_println!("desktop_busy_boot generate prompt=kat max_new={}", max_new);

    // OT-37: sys_generate is the real-LM (inventory) generation path ONLY. The
    // hermetic CI fixture is a small embedding model, not an autoregressive LM
    // (its context window cannot hold the framed chat prompt), so the boot routes
    // the fixture through its own single-forward smoke path and never calls here.
    // A non-inventory model reaching this syscall is a wiring error: fail closed.
    if !is_stage_b_inventory_model(model) {
        return Err(EINVAL);
    }
    // OT-28: production ALWAYS uses the fast persistent-KV path. The shared
    // `generate_inventory_greedy` core does the prefill-once + retain-KV +
    // advance-one-token loop; the reprefill baseline is the kill-switch's
    // disabled mode, exercised only by the `sys_generate_kv_persist_equivalence`
    // KAT that proves the two produce bit-identical token ids. This is both the
    // normal autoregressive execution shape and the OT-16 proof that a GDN
    // layer's state from token N is consumed by token N+1.
    //
    // B1c (F5): the forward runs on the dedicated inference stack, so the
    // caller's 32 KiB kernel stack never carries it.
    // B1c AAG J3: a waiter whose process was torn down gives up here.
    let result = crate::mm::inference_stack::try_run(|| {
        generate_inventory_greedy(model, prompt, max_new, eos, KvGenMode::Persistent, &mut rng)
    })
    .unwrap_or(Err(crate::errno::ECANCELED));
    #[cfg(feature = "neo-test-desktop-busy-boot")]
    crate::serial_println!(
        "desktop_busy_boot generate tokens={} errno={}",
        result.as_ref().map_or(0, |r| r.0.len()),
        result.as_ref().err().copied().unwrap_or(0)
    );
    result
}

/// Control for `generate_corunner_liveness`: the same greedy generation at
/// IF=0 on the caller's stack (no inference stack, so nothing turns
/// interrupts on), the shape of the pre-B1c in-syscall forward.
#[cfg(feature = "neo-test-generate-corunner-liveness-control")]
pub(crate) fn real_lm_generate_if0_for_test(
    model: &model::Model,
    prompt: &[u32],
    max_new: usize,
) -> Result<alloc::vec::Vec<u32>, i64> {
    let mut rng = crate::ai::generate::Pcg32::seeded(0x853c_49e6_748f_ea9b, 0xda3e_39cb_94b9_5bdb);
    crate::arch::x86_64::without_interrupts(|| {
        generate_inventory_greedy(
            model,
            prompt,
            max_new,
            None,
            KvGenMode::Persistent,
            &mut rng,
        )
    })
    .map(|(tokens, _, _)| tokens)
}

#[cfg(feature = "real-lm")]
fn sys_generate_impl(
    model_h: u32,
    prompt_ptr: *const u32,
    prompt_len: u32,
    out_ptr: *mut u32,
    out_cap: u32,
    packed: u64,
) -> i64 {
    let max_new = (packed & 0xffff_ffff) as usize;
    let eos_raw = (packed >> 32) as u32;
    let eos = if eos_raw == u32::MAX {
        None
    } else {
        Some(eos_raw)
    };
    // B1c (F1): bounded before any copy or allocation (the KV cache is a `vec!`).
    if crate::ai::generate_logic::check_generate_request(
        crate::ai::generate_logic::GenerateRequest::Plain {
            prompt_len: prompt_len as usize,
            max_new,
        },
    )
    .is_err()
    {
        return EINVAL;
    }
    let prompt_len = prompt_len as usize;
    let out_cap = out_cap as usize;

    let prompt_bytes = match prompt_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(b) => b,
        None => return EINVAL,
    };
    if prompt_ptr.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(prompt_ptr as u64, prompt_bytes) {
        return e;
    }
    if out_cap != 0 {
        let out_bytes = match out_cap.checked_mul(core::mem::size_of::<u32>()) {
            Some(b) => b,
            None => return EINVAL,
        };
        if out_ptr.is_null() {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(out_ptr as u64, out_bytes) {
            return e;
        }
    }

    let model = match model_lookup(model_h) {
        Ok(m) => m,
        Err(e) => return e,
    };
    if let Err(e) = model_still_current(&model) {
        return model::model_errno(e);
    }

    let prompt = match copy_user_u32s(prompt_ptr, prompt_len) {
        Ok(tokens) => tokens,
        Err(e) => return e,
    };
    if prompt.iter().any(|&t| t > i32::MAX as u32) {
        return EINVAL;
    }

    let (generated, first_top5, execution_stats) =
        match real_lm_generate(model.as_ref(), &prompt, max_new, eos) {
            Ok(outcome) => outcome,
            Err(errno) => return errno,
        };
    #[cfg(not(feature = "neo-test-arthur-real-brain-boot-qwen35"))]
    let _ = &execution_stats;
    #[cfg(feature = "neo-test-arthur-real-brain-boot-qwen35")]
    {
        let Some(stats) = execution_stats else {
            return EINVAL;
        };
        if stats.attention_layers != 6
            || stats.gdn_layers != 18
            || stats.tokens_processed < 2
            || stats.gdn_min_steps != stats.tokens_processed
            || !stats.cross_token_state_carried
        {
            serial_println!(
                "qwen35_runtime_oracle=fail attention_layers={} gdn_layers={} tokens_processed={} gdn_min_steps={} cross_token_state_carried={}",
                stats.attention_layers,
                stats.gdn_layers,
                stats.tokens_processed,
                stats.gdn_min_steps,
                stats.cross_token_state_carried
            );
            return EINVAL;
        }
        if generated.as_slice() != QWEN35_HELLO_ORACLE {
            serial_println!(
                "qwen35_generation_oracle=fail expected_tokens={} actual_tokens={}",
                QWEN35_HELLO_ORACLE.len(),
                generated.len()
            );
            return EINVAL;
        }
        serial_println!(
            "qwen35_runtime_oracle=ok walker=executed attention_layers={} gdn_layers={} tokens_processed={} gdn_min_steps={} cross_token_state_carried={}",
            stats.attention_layers,
            stats.gdn_layers,
            stats.tokens_processed,
            stats.gdn_min_steps,
            stats.cross_token_state_carried
        );
        QWEN35_GENERATION_ORACLE_PASSED.store(true, Ordering::Release);
    }
    // Per-token breadcrumbs are test instrumentation; a product real-LM
    // kernel (B1c) does not print them.
    #[cfg(feature = "neo-test")]
    {
        if let Some(top) = first_top5 {
            log_forward0_top5(&top);
        }
        for (step, next) in generated.iter().copied().enumerate() {
            serial_println!(
                "sample step={} tok={} is_eos={}",
                step,
                next,
                eos == Some(next)
            );
        }
    }
    #[cfg(not(feature = "neo-test"))]
    let _ = first_top5;

    let n_written = generated.len().min(out_cap);
    if n_written != 0 {
        // SAFETY: `generated` is a contiguous Vec<u32>; copying its first
        // `n_written` little-endian words matches the syscall ABI's token array.
        let bytes = unsafe {
            core::slice::from_raw_parts(
                generated.as_ptr().cast::<u8>(),
                n_written * core::mem::size_of::<u32>(),
            )
        };
        if let Err(e) = copy_kernel_bytes_to_user(out_ptr.cast::<u8>(), bytes) {
            return e;
        }
    }
    n_written as i64
}

/// Phase 8 A1 — constrained autoregressive generation: forward math is identical
/// to `SYS_GENERATE`; only the sampler changes, masking the next-token choice to
/// continuations of the valid candidate AST set supplied by userland.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn sys_generate_constrained_impl(
    model_h: u32,
    prompt_ptr: *const u32,
    prompt_len: u32,
    out_ptr: *mut u32,
    out_cap: u32,
    desc_ptr: *const GenerateConstrainedDesc,
) -> i64 {
    // B1c (AAG J2): the register-carried bound before the descriptor copy.
    if crate::ai::generate_logic::check_prompt_len(prompt_len as usize).is_err() {
        return EINVAL;
    }
    let desc = match copy_generate_constrained_desc(desc_ptr) {
        Ok(desc) => desc,
        Err(errno) => return errno,
    };
    let max_new = desc.max_new as usize;
    let sequence_count = desc.sequence_count as usize;
    let tokens_len = desc.tokens_len as usize;
    let eos = if desc.eos_token_id == u32::MAX {
        None
    } else {
        Some(desc.eos_token_id)
    };
    // B1c (F1, AAG J2): the full policy right after the fixed-size descriptor
    // read, before any variable-size user copy, allocation, or model lookup.
    if crate::ai::generate_logic::check_generate_request(
        crate::ai::generate_logic::GenerateRequest::Constrained {
            prompt_len: prompt_len as usize,
            max_new,
            sequence_count,
            tokens_len,
        },
    )
    .is_err()
    {
        return EINVAL;
    }

    let prompt_len = prompt_len as usize;
    let out_cap = out_cap as usize;
    let prompt_bytes = match prompt_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(b) => b,
        None => return EINVAL,
    };
    if prompt_ptr.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(prompt_ptr as u64, prompt_bytes) {
        return e;
    }
    if out_cap != 0 {
        let out_bytes = match out_cap.checked_mul(core::mem::size_of::<u32>()) {
            Some(b) => b,
            None => return EINVAL,
        };
        if out_ptr.is_null() {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(out_ptr as u64, out_bytes) {
            return e;
        }
    }

    let sequence_descs = match copy_generate_constrained_sequence_descs(
        desc.sequences_ptr as *const GenerateConstrainedSequenceDesc,
        sequence_count,
    ) {
        Ok(descs) => descs,
        Err(errno) => return errno,
    };
    let token_bytes = match tokens_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(bytes) => bytes,
        None => return EINVAL,
    };
    if desc.tokens_ptr == 0 {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(desc.tokens_ptr, token_bytes) {
        return e;
    }
    let candidate_tokens = match copy_user_u32s(desc.tokens_ptr as *const u32, tokens_len) {
        Ok(tokens) => tokens,
        Err(e) => return e,
    };
    let mut candidate_sequences: alloc::vec::Vec<&[u32]> =
        alloc::vec::Vec::with_capacity(sequence_count);
    for sequence in &sequence_descs {
        let start = sequence.offset as usize;
        let len = sequence.len as usize;
        if len == 0 {
            return EINVAL;
        }
        let end = match start.checked_add(len) {
            Some(end) => end,
            None => return EINVAL,
        };
        let Some(tokens) = candidate_tokens.get(start..end) else {
            return EINVAL;
        };
        if tokens.iter().any(|&token| token > i32::MAX as u32) {
            return EINVAL;
        }
        candidate_sequences.push(tokens);
    }

    let model = match model_lookup(model_h) {
        Ok(m) => m,
        Err(e) => return e,
    };
    if let Err(e) = model_still_current(&model) {
        return model::model_errno(e);
    }
    let inventory_model = is_stage_b_inventory_model(&model);

    let mut context = match copy_user_u32s(prompt_ptr, prompt_len) {
        Ok(tokens) => tokens,
        Err(e) => return e,
    };
    if context.iter().any(|&token| token > i32::MAX as u32) {
        return EINVAL;
    }
    let ceiling = context.len().saturating_add(max_new).saturating_add(8);
    let generated = match on_inference_stack(Err(crate::errno::ECANCELED), || {
        constrained_loop(
            model.as_ref(),
            inventory_model,
            &mut context,
            max_new,
            eos,
            ceiling,
            &candidate_sequences,
        )
    }) {
        Ok(generated) => generated,
        Err(errno) => return errno,
    };

    let n_written = generated.len().min(out_cap);
    if n_written != 0 {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                generated.as_ptr().cast::<u8>(),
                n_written * core::mem::size_of::<u32>(),
            )
        };
        if let Err(e) = copy_kernel_bytes_to_user(out_ptr.cast::<u8>(), bytes) {
            return e;
        }
    }
    n_written as i64
}

/// The constrained generation loop (re-prefill per token; see the B1c NOTE
/// section 1 for its bounded O(n x p) cost), run on the inference stack.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn constrained_loop(
    model: &model::Model,
    inventory_model: bool,
    context: &mut alloc::vec::Vec<u32>,
    max_new: usize,
    eos: Option<u32>,
    ceiling: usize,
    candidate_sequences: &[&[u32]],
) -> Result<alloc::vec::Vec<u32>, i64> {
    use crate::ai::generate::constrained_argmax_token;

    let mut generated = alloc::vec::Vec::new();
    for step in 0..max_new {
        #[cfg(not(feature = "neo-test"))]
        let _ = step;
        // B1c AAG J3: a torn-down caller stops between tokens.
        if inference_cancelled() {
            return Err(crate::errno::ECANCELED);
        }
        let logits = if inventory_model {
            crate::ai::qwen_inventory::next_logits(model, context).map_err(infer_errno)?
        } else {
            crate::ai::generate::next_logits(model, context, 0).map_err(generate_errno)?
        };
        let next = constrained_argmax_token(&logits, &generated, candidate_sequences)
            .map_err(|_| EINVAL)?;
        #[cfg(feature = "neo-test")]
        serial_println!(
            "constrained_sample step={} tok={} is_eos={} model={}",
            step,
            next,
            eos == Some(next),
            if inventory_model {
                "inventory"
            } else {
                "graph"
            }
        );
        generated.push(next);
        context.push(next);
        if eos == Some(next)
            || candidate_sequences.contains(&generated.as_slice())
            || context.len() >= ceiling
        {
            break;
        }
    }
    Ok(generated)
}

/// B1c (F5): run a generate forward on the dedicated inference stack where
/// the kernel has one (`inference-stack`, implied by `real-lm`); the agentic
/// test builds without it keep running on the caller's stack as before.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
/// Run `f` on the inference stack. `cancelled` is the result if the caller's
/// process is torn down while it waits for the stack (B1c AAG J3).
fn on_inference_stack<R, F: FnOnce() -> R>(cancelled: R, f: F) -> R {
    #[cfg(feature = "inference-stack")]
    {
        crate::mm::inference_stack::try_run(f).unwrap_or(cancelled)
    }
    #[cfg(not(feature = "inference-stack"))]
    {
        let _ = cancelled;
        f()
    }
}

/// B1c AAG J3: the holder's between-token check (a torn-down caller stops
/// early; its locals drop and it exits at the syscall boundary).
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-sys-generate-kv-persist-equivalence",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
#[inline]
fn inference_cancelled() -> bool {
    #[cfg(feature = "inference-stack")]
    {
        crate::mm::inference_stack::cancel_requested()
    }
    #[cfg(not(feature = "inference-stack"))]
    {
        false
    }
}

#[cfg(not(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
)))]
fn sys_generate_constrained_impl(
    _model_h: u32,
    _prompt_ptr: *const u32,
    _prompt_len: u32,
    _out_ptr: *mut u32,
    _out_cap: u32,
    _desc_ptr: *const GenerateConstrainedDesc,
) -> i64 {
    EINVAL
}

/// Arthur A1 bitmask-constrained single-token generation. Userland computes the
/// compact grammar's token-validity bitmask; the kernel does not interpret
/// grammar or action policy. I-14 containment: this path takes no additional AI
/// governance lock beyond the existing generate forward, performs no new DMA,
/// and rejects malformed masks before sampling so hostile userland cannot drive
/// ring-0 OOB access.
#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn sys_generate_masked_impl(
    model_h: u32,
    prompt_ptr: *const u32,
    prompt_len: u32,
    out_ptr: *mut u32,
    desc_ptr: *const GenerateMaskDesc,
) -> i64 {
    use crate::ai::generate::masked_argmax_token;

    // B1c (AAG J2): the register-carried bound before the descriptor copy.
    if crate::ai::generate_logic::check_prompt_len(prompt_len as usize).is_err() {
        return EINVAL;
    }
    let desc = match copy_generate_mask_desc(desc_ptr) {
        Ok(desc) => desc,
        Err(errno) => return errno,
    };
    if desc._reserved0 != 0 || desc.flags & !GENERATE_MASK_KNOWN_FLAGS != 0 {
        return EINVAL;
    }
    let disabled = desc.flags & GENERATE_MASK_FLAG_DISABLED != 0;
    let mask_bits = desc.mask_bits as usize;
    let mask_bytes = desc.mask_bytes as usize;
    // B1c (F1, AAG J2): the full policy (prompt cap; exactly one new token;
    // the mask limits) right after the fixed-size descriptor read, before any
    // variable-size user copy, allocation, or model lookup. A malformed mask
    // is EINVAL ahead of a bad prompt or output pointer (EFAULT); B1c changed
    // that precedence from `main`'s, and the syscall-level test pins it.
    if crate::ai::generate_logic::check_generate_request(
        crate::ai::generate_logic::GenerateRequest::Masked {
            prompt_len: prompt_len as usize,
            disabled,
            mask_bits,
            mask_bytes,
        },
    )
    .is_err()
    {
        return EINVAL;
    }
    if out_ptr.is_null() {
        return EFAULT;
    }
    let prompt_len = prompt_len as usize;
    let prompt_bytes = match prompt_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(bytes) => bytes,
        None => return EINVAL,
    };
    if prompt_ptr.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(prompt_ptr as u64, prompt_bytes) {
        return e;
    }
    if let Err(e) = validate_user_buf(out_ptr as u64, core::mem::size_of::<u32>()) {
        return e;
    }

    let mut mask_storage = alloc::vec::Vec::new();
    if !disabled {
        if desc.mask_ptr == 0 {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(desc.mask_ptr, mask_bytes) {
            return e;
        }
        mask_storage = match copy_user_bytes(desc.mask_ptr as *const u8, mask_bytes) {
            Ok(bytes) => bytes,
            Err(e) => return e,
        };
    }

    let model = match model_lookup(model_h) {
        Ok(m) => m,
        Err(e) => return e,
    };
    if let Err(e) = model_still_current(&model) {
        return model::model_errno(e);
    }
    let inventory_model = is_stage_b_inventory_model(&model);

    let context = match copy_user_u32s(prompt_ptr, prompt_len) {
        Ok(tokens) => tokens,
        Err(e) => return e,
    };
    if context.iter().any(|&token| token > i32::MAX as u32) {
        return EINVAL;
    }
    let logits = match on_inference_stack(Err(crate::errno::ECANCELED), || {
        // B1c re-check #2 (C1): no forward for a torn-down caller.
        if inference_cancelled() {
            return Err(crate::errno::ECANCELED);
        }
        if inventory_model {
            crate::ai::qwen_inventory::next_logits(model.as_ref(), &context).map_err(infer_errno)
        } else {
            crate::ai::generate::next_logits(model.as_ref(), &context, 0).map_err(generate_errno)
        }
    }) {
        Ok(logits) => logits,
        Err(errno) => return errno,
    };
    let mask = if disabled {
        None
    } else {
        Some(mask_storage.as_slice())
    };
    let next = match masked_argmax_token(&logits, mask, mask_bits) {
        Ok(next) => next,
        Err(_) => return EINVAL,
    };
    let bytes = next.to_le_bytes();
    if let Err(e) = copy_kernel_bytes_to_user(out_ptr.cast::<u8>(), &bytes) {
        return e;
    }
    #[cfg(feature = "neo-test")]
    serial_println!(
        "masked_sample step=0 tok={} mask_bits={} disabled={} model={}",
        next,
        if disabled { logits.len() } else { mask_bits },
        disabled as u8,
        if inventory_model {
            "inventory"
        } else {
            "graph"
        }
    );
    1
}

#[cfg(not(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
)))]
fn sys_generate_masked_impl(
    _model_h: u32,
    _prompt_ptr: *const u32,
    _prompt_len: u32,
    _out_ptr: *mut u32,
    _desc_ptr: *const GenerateMaskDesc,
) -> i64 {
    EINVAL
}

#[cfg(all(
    not(feature = "real-lm"),
    feature = "neo-test-inference-preemptible-liveness"
))]
fn sys_generate_impl(
    model_h: u32,
    prompt_ptr: *const u32,
    prompt_len: u32,
    out_ptr: *mut u32,
    out_cap: u32,
    packed: u64,
) -> i64 {
    use crate::ai::generate::{GenerateConfig, Pcg32, SamplingConfig, generate};

    let max_new = (packed & 0xffff_ffff) as usize;
    let eos_raw = (packed >> 32) as u32;
    let eos = if eos_raw == u32::MAX {
        None
    } else {
        Some(eos_raw)
    };
    if max_new == 0 || prompt_len == 0 {
        return EINVAL;
    }
    let prompt_len = prompt_len as usize;
    let out_cap = out_cap as usize;
    let prompt_bytes = match prompt_len.checked_mul(core::mem::size_of::<u32>()) {
        Some(b) => b,
        None => return EINVAL,
    };
    if prompt_ptr.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(prompt_ptr as u64, prompt_bytes) {
        return e;
    }
    if out_cap != 0 {
        let out_bytes = match out_cap.checked_mul(core::mem::size_of::<u32>()) {
            Some(b) => b,
            None => return EINVAL,
        };
        if out_ptr.is_null() {
            return EFAULT;
        }
        if let Err(e) = validate_user_buf(out_ptr as u64, out_bytes) {
            return e;
        }
    }

    let model = match model_lookup(model_h) {
        Ok(m) => m,
        Err(e) => return e,
    };
    if let Err(e) = model_still_current(&model) {
        return model::model_errno(e);
    }
    let prompt = match copy_user_u32s(prompt_ptr, prompt_len) {
        Ok(tokens) => tokens,
        Err(e) => return e,
    };
    if prompt.iter().any(|&t| t > i32::MAX as u32) {
        return EINVAL;
    }
    let config = GenerateConfig {
        max_new_tokens: max_new,
        eos_token_id: eos,
        pad_token_id: 0,
    };

    let mut irq_off_rng = Pcg32::seeded(42, 54);
    let irq_off = match without_inference_preemptible_dispatch(|| {
        crate::arch::x86_64::without_interrupts(|| {
            generate(
                model.as_ref(),
                &prompt,
                config,
                SamplingConfig::greedy(),
                &mut irq_off_rng,
            )
        })
    }) {
        Ok(tokens) => tokens,
        Err(err) => {
            let errno = generate_errno(err);
            serial_println!(
                "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=irq_off_generate_failed errno={}",
                errno
            );
            return errno;
        }
    };

    let mut if1_rng = Pcg32::seeded(42, 54);
    let dispatch_preemptions_start = inference_preemptible_dispatch_preemptions();
    let generated = match generate(
        model.as_ref(),
        &prompt,
        config,
        SamplingConfig::greedy(),
        &mut if1_rng,
    )
    .map_err(generate_errno)
    {
        Ok(tokens) => tokens,
        Err(errno) => return errno,
    };
    if let Err(errno) = require_inference_preemptible_dispatch_preempted_since(
        dispatch_preemptions_start,
        "sys_generate_0x121",
    ) {
        return errno;
    }

    if generated != irq_off {
        serial_println!(
            "NEO_TEST/1 event=FAIL name=inference_preemptible_liveness reason=sys_generate_token_identity_mismatch"
        );
        return EINVAL;
    }
    INFERENCE_PREEMPTIBLE_GENERATE_IDENTITY.store(true, Ordering::Release);
    serial_println!(
        "NEO_TEST/1 event=INFO name=inference_preemptible_liveness token_identity=sys_generate_eq path=sys_generate_0x121 count={}",
        generated.len()
    );

    let n_written = generated.len().min(out_cap);
    if n_written != 0 {
        let bytes = unsafe {
            core::slice::from_raw_parts(
                generated.as_ptr().cast::<u8>(),
                n_written * core::mem::size_of::<u32>(),
            )
        };
        if let Err(errno) = copy_kernel_bytes_to_user(out_ptr.cast::<u8>(), bytes) {
            return errno;
        }
    }
    n_written as i64
}

#[cfg(any(
    feature = "neo-test-inference-preemptible-liveness",
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn generate_errno(err: crate::ai::generate::GenerateError) -> i64 {
    match err {
        crate::ai::generate::GenerateError::Inference(err) => infer_errno(err),
        crate::ai::generate::GenerateError::Tensor(err) => tensor_errno(err),
        crate::ai::generate::GenerateError::Logic(_)
        | crate::ai::generate::GenerateError::InvalidModelIo
        | crate::ai::generate::GenerateError::InvalidToken
        | crate::ai::generate::GenerateError::ContextFull
        | crate::ai::generate::GenerateError::OutputCount => EINVAL,
    }
}

#[cfg(all(
    not(feature = "real-lm"),
    not(feature = "neo-test-inference-preemptible-liveness")
))]
fn sys_generate_impl(
    _model_h: u32,
    _prompt_ptr: *const u32,
    _prompt_len: u32,
    _out_ptr: *mut u32,
    _out_cap: u32,
    _packed: u64,
) -> i64 {
    EINVAL
}

/// OT-37 ISC-1 instrument: the first-forward top-5 logits `(id, value)` so a
/// boot-vs-host diff localizes any future divergence at the forward layer. The
/// value is printed as its integer part (kernel serial has no float formatting).
/// Also used by the OT-28 shared generation core under the KV-persist KAT feature.
#[cfg(all(
    feature = "neo-test",
    any(
        feature = "real-lm",
        feature = "neo-test-sys-generate-kv-persist-equivalence"
    )
))]
fn forward0_top5(logits: &[f32]) -> [(usize, f32); 5] {
    let mut top: [(usize, f32); 5] = [(0, f32::NEG_INFINITY); 5];
    for (id, &value) in logits.iter().enumerate() {
        if value > top[4].1 {
            top[4] = (id, value);
            let mut j = 4;
            while j > 0 && top[j].1 > top[j - 1].1 {
                top.swap(j, j - 1);
                j -= 1;
            }
        }
    }
    top
}

#[cfg(all(feature = "real-lm", feature = "neo-test"))]
fn log_forward0_top5(top: &[(usize, f32); 5]) {
    serial_println!(
        "forward0 top5=[({},{}),({},{}),({},{}),({},{}),({},{})]",
        top[0].0,
        top[0].1 as i64,
        top[1].0,
        top[1].1 as i64,
        top[2].0,
        top[2].1 as i64,
        top[3].0,
        top[3].1 as i64,
        top[4].0,
        top[4].1 as i64
    );
}

#[cfg(any(
    feature = "real-lm",
    feature = "neo-test-arthur-agentic-actions",
    feature = "neo-test-arthur-reversibility",
    feature = "neo-test-arthur-multistep-agentic"
))]
fn is_stage_b_inventory_model(model: &model::Model) -> bool {
    model.input_count() == 0 && model.output_count() == 0 && model.nodes().is_empty()
}

fn run_graph_infer(
    model: &alloc::sync::Arc<model::Model>,
    inputs: &[alloc::sync::Arc<tensor::Tensor>],
) -> Result<alloc::vec::Vec<alloc::sync::Arc<tensor::Tensor>>, exec::InferenceError> {
    match crate::drivers::accel::default_driver() {
        Some(driver) => {
            let device = driver.descriptor().device;
            let principal = crate::sched::current_pid();
            // sys_inf_run carries no priority/reservation (BUILD-2L adds no ABI): every
            // live request shares one default priority, so contention resolves by
            // run-to-completion then FIFO. `run_scheduled` always completes (releasing
            // the device + waking the next parked caller) even if `exec::run` errors.
            match crate::ai::admission::run_scheduled(device, principal, 0, false, || {
                #[cfg(feature = "neo-test-admission-park-kill")]
                admission_hold_for_test();
                exec::run(model, inputs)
            }) {
                Ok(outputs) => outputs,
                // CP-66: withdrawn while parked (the process is being torn
                // down); the caller unwinds and exits at the boundary.
                Err(crate::ai::admission::Cancelled) => Err(exec::InferenceError::Cancelled),
            }
        }
        // No accelerator at all: nothing to schedule; `exec::run` returns NoDevice.
        None => exec::run(model, inputs),
    }
}

fn socket_lookup(h: u32, required_semantic: u8) -> Result<alloc::sync::Arc<Socket>, i64> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::Socket { socket, rights, .. }
                if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) =>
            {
                Ok(socket)
            }
            Handle::Socket { .. } => Err(EPERM),
            _ => Err(EBADF),
        }
    })
}

fn tcp_listener_lookup(
    h: u32,
    required_semantic: u8,
) -> Result<(alloc::sync::Arc<TcpListener>, process::CapId), i64> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::TcpListener {
                listener,
                cap_id,
                rights,
            } if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) => {
                Ok((listener, cap_id))
            }
            Handle::TcpListener { .. } => Err(EPERM),
            _ => Err(EBADF),
        }
    })
}

fn tcp_socket_lookup(h: u32, required_semantic: u8) -> Result<alloc::sync::Arc<TcpSocket>, i64> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::TcpSocket { socket, rights, .. }
                if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) =>
            {
                Ok(socket)
            }
            Handle::TcpSocket { .. } => Err(EPERM),
            _ => Err(EBADF),
        }
    })
}

fn sys_socket_create_impl(family: u32, socktype: u32) -> i64 {
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let socket = match Socket::new(proc.pid, family, socktype) {
        Ok(socket) => alloc::sync::Arc::new(socket),
        Err(e) => return socket_errno(e),
    };
    let handle = Handle::Socket {
        socket,
        cap_id: process::alloc_cap_id(),
        rights: process::full_socket_rights(),
    };
    match process::install_cap(&proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

fn sys_net_connect_tcp_impl(remote: *const u8, remote_len: usize, local_port: u16) -> i64 {
    let remote = match copy_user_socket_addr(remote, remote_len) {
        Ok(addr) => addr,
        Err(e) => return e,
    };
    if remote.port == 0 {
        return EINVAL;
    }
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let selected_port = if local_port == 0 {
        tcp::allocate_ephemeral_port()
    } else {
        local_port
    };
    let local = match tcp::select_local_source(remote, selected_port) {
        Ok(local) => local,
        Err(e) => return tcp_errno(e),
    };
    let tuple = match tcp::FourTuple::new(local, remote) {
        Ok(tuple) => tuple,
        Err(e) => return tcp_errno(e),
    };
    let socket = alloc::sync::Arc::new(TcpSocket::new(proc.pid, tuple, tcp::TcpState::SynSent));
    if let Err(e) = tcp::insert_connection(socket.clone()) {
        return tcp_errno(e);
    }
    if let Err(e) = socket.start_active_open() {
        tcp::remove_connection(tuple);
        return tcp_errno(e);
    }
    let handle = Handle::TcpSocket {
        socket,
        cap_id: process::alloc_cap_id(),
        rights: process::full_tcp_socket_rights(),
    };
    match process::install_cap(&proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => {
            tcp::remove_connection(tuple);
            EINVAL
        }
    }
}

#[cfg(not(feature = "net-dial-cap"))]
fn sys_net_listen_tcp_impl(local: *const u8, local_len: usize, backlog: usize) -> i64 {
    sys_net_listen_tcp_common(local, local_len, backlog)
}

#[cfg(feature = "net-dial-cap")]
fn sys_net_listen_tcp_impl(local: *const u8, local_len: usize, backlog: usize, cap_h: u32) -> i64 {
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let authorized = crate::arch::x86_64::without_interrupts(|| {
        matches!(
            proc.handles.lock().get(cap_h),
            Some(Handle::NetDial { host, port, rights, .. })
                if host.as_ref() == crate::sched::net_dial_cap_logic::ROOT_HOST_PATTERN
                    && port == crate::sched::net_dial_cap_logic::ROOT_PORT
                    && rights.has_meta(meta_rights::USE)
                    && rights.has_semantic(process::net_dial_rights::DIAL)
        )
    });
    if !authorized {
        return EACCES;
    }
    sys_net_listen_tcp_common(local, local_len, backlog)
}

fn sys_net_listen_tcp_common(local: *const u8, local_len: usize, backlog: usize) -> i64 {
    if backlog == 0 || backlog > TCP_MAX_BACKLOG {
        return EINVAL;
    }
    let local = match copy_user_socket_addr(local, local_len) {
        Ok(addr) => addr,
        Err(e) => return e,
    };
    if local.port == 0 {
        return EINVAL;
    }
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let listener = match TcpListener::new(proc.pid, local, backlog) {
        Ok(listener) => alloc::sync::Arc::new(listener),
        Err(e) => return tcp_errno(e),
    };
    if let Err(e) = tcp::insert_listener(listener.clone()) {
        return tcp_errno(e);
    }
    let handle = Handle::TcpListener {
        listener: listener.clone(),
        cap_id: process::alloc_cap_id(),
        rights: process::full_tcp_listener_rights(),
    };
    match process::install_cap(&proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => {
            tcp::remove_listener(local);
            EINVAL
        }
    }
}

fn sys_net_accept_tcp_impl(listener_h: u32) -> i64 {
    let (listener, parent_cap_id) = match tcp_listener_lookup(listener_h, socket_rights::READ) {
        Ok(listener) => listener,
        Err(e) => return e,
    };
    let socket = match listener.dequeue_accept() {
        Ok(socket) => socket,
        Err(e) => return tcp_errno(e),
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let handle = Handle::TcpSocket {
        socket: socket.clone(),
        cap_id: process::alloc_cap_id(),
        rights: process::full_tcp_socket_rights(),
    };
    match process::install_cap(&proc, handle, Some(parent_cap_id)) {
        Ok(idx) => idx as i64,
        Err(_) => {
            // Dequeued but never installed: nothing else will close it.
            crate::arch::x86_64::without_interrupts(|| tcp::close_connection(&socket));
            EINVAL
        }
    }
}

fn sys_net_shutdown_tcp_impl(h: u32, dir: u32) -> i64 {
    if dir == 0 || (dir & !(TCP_SHUTDOWN_READ | TCP_SHUTDOWN_WRITE)) != 0 {
        return EINVAL;
    }
    match crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let mut handles = proc.handles.lock();
        let handle = handles.get_mut(h).ok_or(EBADF)?;
        match handle {
            Handle::TcpSocket { socket, rights, .. } => {
                if !rights.has_meta(meta_rights::USE) {
                    return Err(EPERM);
                }
                if (dir & TCP_SHUTDOWN_READ) != 0 && !rights.has_semantic(socket_rights::READ) {
                    return Err(EPERM);
                }
                if (dir & TCP_SHUTDOWN_WRITE) != 0 && !rights.has_semantic(socket_rights::WRITE) {
                    return Err(EPERM);
                }
                if (dir & TCP_SHUTDOWN_READ) != 0 {
                    rights.semantic &= !socket_rights::READ;
                }
                if (dir & TCP_SHUTDOWN_WRITE) != 0 {
                    socket.shutdown_write().map_err(tcp_errno)?;
                    rights.semantic &= !socket_rights::WRITE;
                }
                Ok(0)
            }
            _ => Err(EBADF),
        }
    }) {
        Ok(value) => value,
        Err(e) => e,
    }
}

fn sys_net_resolve_impl(
    name_ptr: *const u8,
    name_len: usize,
    family_hint: u32,
    out_ptr: *mut dns::NetCandidate,
    out_len: usize,
) -> i64 {
    if name_len == 0 || name_len > dns::MAX_NAME_LEN || out_len == 0 {
        return EINVAL;
    }
    if out_len > dns::MAX_CANDIDATES {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(name_ptr as u64, name_len) {
        return e;
    }
    let out_bytes = match out_len.checked_mul(core::mem::size_of::<dns::NetCandidate>()) {
        Some(bytes) => bytes,
        None => return EINVAL,
    };
    if let Err(e) = validate_user_buf(out_ptr as u64, out_bytes) {
        return e;
    }
    let hint = match dns::FamilyHint::from_abi(family_hint) {
        Ok(hint) => hint,
        Err(e) => return dns_errno(e),
    };
    let name_bytes = match copy_user_bytes(name_ptr, name_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let name = match core::str::from_utf8(&name_bytes) {
        Ok(name) => name,
        Err(_) => return EINVAL,
    };
    let candidates = match dns::resolve(name, hint) {
        Ok(candidates) => candidates,
        Err(e) => return dns_errno(e),
    };
    let n = core::cmp::min(out_len, candidates.len());
    if let Err(e) = copy_net_candidates_to_user(out_ptr, &candidates[..n]) {
        return e;
    }
    n as i64
}

#[cfg(not(feature = "net-dial-cap"))]
fn sys_net_dial_impl(name_ptr: *const u8, name_len: usize, port: u16) -> i64 {
    if name_len == 0 || name_len > dns::MAX_NAME_LEN || port == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(name_ptr as u64, name_len) {
        return e;
    }
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let name_bytes = match copy_user_bytes(name_ptr, name_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let name = match core::str::from_utf8(&name_bytes) {
        Ok(name) => name,
        Err(_) => return EINVAL,
    };
    let socket = match happy_eyeballs::dial_name(proc.pid, name, port) {
        Ok(socket) => socket,
        Err(e) => return dial_errno(e),
    };
    tcp::pump_loopback_egress();
    let tuple = socket.tuple();
    let handle = Handle::TcpSocket {
        socket,
        cap_id: process::alloc_cap_id(),
        rights: process::full_tcp_socket_rights(),
    };
    match process::install_cap(&proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => {
            tcp::remove_connection(tuple);
            EINVAL
        }
    }
}

#[cfg(feature = "net-dial-cap")]
fn sys_net_dial_impl(name_ptr: *const u8, name_len: usize, port: u16, cap_h: u32) -> i64 {
    if name_len == 0 || name_len > dns::MAX_NAME_LEN || port == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(name_ptr as u64, name_len) {
        return e;
    }
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let name_bytes = match copy_user_bytes(name_ptr, name_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let name = match core::str::from_utf8(&name_bytes) {
        Ok(name) => name,
        Err(_) => return EINVAL,
    };

    // Snapshot the immutable scope under the established per-process handle
    // lock, then drop it before DNS/TCP work (CP-1: no new nested lock order).
    let scope = crate::arch::x86_64::without_interrupts(|| proc.handles.lock().get(cap_h));
    let (scope_host, scope_port) = match scope {
        Some(Handle::NetDial {
            host,
            port: scope_port,
            rights,
            ..
        }) if rights.has_meta(meta_rights::USE)
            && rights.has_semantic(process::net_dial_rights::DIAL) =>
        {
            (host, scope_port)
        }
        _ => {
            serial_println!(
                "[ kernel ] net_dial denied reason=missing_net_dial_cap pid={} host={} port={}",
                proc.pid.0,
                name,
                port
            );
            return EACCES;
        }
    };
    if !crate::sched::net_dial_cap_logic::scope_allows(&scope_host, scope_port, name, port) {
        serial_println!(
            "[ kernel ] net_dial denied reason=out_of_scope pid={} host={} port={} cap_host={} cap_port={}",
            proc.pid.0,
            name,
            port,
            scope_host,
            scope_port
        );
        return EACCES;
    }

    let socket = match happy_eyeballs::dial_name(proc.pid, name, port) {
        Ok(socket) => socket,
        Err(e) => return dial_errno(e),
    };
    tcp::pump_loopback_egress();
    let tuple = socket.tuple();
    let handle = Handle::TcpSocket {
        socket,
        cap_id: process::alloc_cap_id(),
        rights: process::full_tcp_socket_rights(),
    };
    // Revocation does not cascade to live sockets by design: the dial authority
    // authorizes connection creation, while the socket is installed as a new
    // lineage root. See OT-61 if cascade-on-revoke is later required.
    match process::install_cap(&proc, handle, None) {
        Ok(idx) => idx as i64,
        Err(_) => {
            tcp::remove_connection(tuple);
            EINVAL
        }
    }
}

#[cfg(feature = "net-dial-cap")]
fn sys_net_dial_cap_derive_impl(
    parent_h: u32,
    host_ptr: *const u8,
    host_len: usize,
    port: u16,
) -> i64 {
    if host_len == 0 || host_len > dns::MAX_NAME_LEN || port == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(host_ptr as u64, host_len) {
        return e;
    }
    let host_bytes = match copy_user_bytes(host_ptr, host_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let host = match core::str::from_utf8(&host_bytes) {
        Ok(host) => host,
        Err(_) => return EINVAL,
    };
    match process::derive_net_dial_cap(crate::sched::current_pid(), parent_h, host, port) {
        Ok(idx) => idx as i64,
        Err(e) => e,
    }
}

fn sys_net_tcp_enable_tls_impl(h: u32, key_ptr: *mut u8, key_len: usize) -> i64 {
    if key_len != tls_record::KEY_MATERIAL_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(key_ptr as u64, key_len) {
        return e;
    }
    let socket = match tcp_socket_lookup(h, socket_rights::WRITE) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let mut key_material = match copy_user_bytes(key_ptr as *const u8, key_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let result = socket.enable_tls(&mut key_material);
    tls_record::wipe_secret(&mut key_material);
    // The caller's copy of the key is its own memory: if a page of it can't
    // be written (EFAULT), it stays as it was, and the TLS result stands.
    let _ = zero_user_bytes(key_ptr, key_len);
    match result {
        Ok(()) => 0,
        Err(e) => tcp_errno(e),
    }
}

/// Phase 10.3 — SYS_GETRANDOM: fill a userland buffer with DRBG bytes.
///
/// Security-critical syscall: validates the user buffer rigorously before
/// any write. Buffer-validation truth table:
///   - buf=NULL (addr=0)       → EFAULT
///   - buf in kernel half      → EFAULT (validate_user_buf)
///   - buf+len overflows u64   → EFAULT (validate_user_buf)
///   - buf+len crosses USER_HALF_LIMIT → EFAULT (validate_user_buf)
///   - len == 0                → 0 (no-op, POSIX-compatible)
///   - len > MAX_GETRANDOM_LEN → EINVAL
///   - DRBG uninitialized      → ENOMEM (fail-closed, never zero/weak bytes)
///
/// # Safety
/// The userland-buffer write at the end is `unsafe` because it dereferences
/// a raw pointer into user space. The precondition is that `validate_user_buf`
/// has confirmed `[buf, buf + len)` lies entirely in the user half. SMAP
/// protects kernel memory; `with_user_access` temporarily lifts SMAP for the
/// copy window.
#[cfg(feature = "kernel-rng")]
fn sys_getrandom_impl(buf: *mut u8, len: usize) -> i64 {
    use crate::rng::MAX_GETRANDOM_LEN;

    // Zero-length is a no-op (POSIX getrandom-compatible).
    if len == 0 {
        return 0;
    }

    // Cap length to prevent excessive single-call allocations.
    if len > MAX_GETRANDOM_LEN {
        return EINVAL;
    }

    // Validate user buffer: must be in user half, no overflow.
    if buf.is_null() {
        return EFAULT;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }

    // Fill a kernel-side buffer first, then copy to user.
    // This ensures we never write partial/weak bytes to userland.
    let mut kernel_buf = alloc::vec![0u8; len];
    if let Err(e) = crate::rng::fill_random(&mut kernel_buf) {
        return e;
    }

    if let Err(e) = copy_kernel_bytes_to_user(buf, &kernel_buf) {
        return e;
    }

    len as i64
}

/// Fallback when `kernel-rng` feature is OFF — returns ENOSYS so the
/// kernel is bit-for-bit identical with the pre-10.3 build.
#[cfg(not(feature = "kernel-rng"))]
fn sys_getrandom_impl(_buf: *mut u8, _len: usize) -> i64 {
    ENOSYS
}

fn copy_user_socket_addr(addr: *const u8, addr_len: usize) -> Result<socket::SocketAddr, i64> {
    if addr_len == 0 || addr_len > 32 {
        return Err(EINVAL);
    }
    validate_user_buf(addr as u64, addr_len)?;
    let addr_bytes = copy_user_bytes(addr, addr_len)?;
    socket::SocketAddr::parse(&addr_bytes).map_err(socket_errno)
}

fn sys_socket_bind_impl(h: u32, addr: *const u8, addr_len: usize) -> i64 {
    if addr_len == 0 || addr_len > 32 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(addr as u64, addr_len) {
        return e;
    }
    let socket = match socket_lookup(h, socket_rights::BIND) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let addr_bytes = match copy_user_bytes(addr, addr_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let parsed = match socket::SocketAddr::parse(&addr_bytes) {
        Ok(addr) => addr,
        Err(e) => return socket_errno(e),
    };
    match socket.bind(parsed) {
        Ok(()) => 0,
        Err(e) => socket_errno(e),
    }
}

fn sys_socket_send_impl(h: u32, buf: *const u8, len: usize, flags: u32) -> i64 {
    if flags != 0 || len == 0 || len > socket::MAX_DATAGRAM {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    match tcp_socket_lookup(h, socket_rights::WRITE) {
        Ok(socket) => {
            let data = match copy_user_bytes(buf, len) {
                Ok(bytes) => bytes,
                Err(e) => return e,
            };
            return match socket.send_stream(&data) {
                Ok(n) => {
                    tcp::pump_loopback_egress();
                    n as i64
                }
                Err(e) => tcp_errno(e),
            };
        }
        Err(EPERM) => return EPERM,
        Err(_) => {}
    }
    // With NetDialCap enabled, the only writable network handle accepted here
    // is a TCP socket rooted by the scoped SYS_NET_DIAL path (or derived from
    // that socket).  Legacy datagram handles must not become an egress bypass
    // merely because one was inherited across a feature boundary.
    #[cfg(feature = "net-dial-cap")]
    return ENOSYS;

    #[cfg(not(feature = "net-dial-cap"))]
    let socket = match socket_lookup(h, socket_rights::SEND) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    #[cfg(not(feature = "net-dial-cap"))]
    let data = match copy_user_bytes(buf, len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    #[cfg(not(feature = "net-dial-cap"))]
    match socket.send(&data) {
        Ok(n) => n as i64,
        Err(e) => socket_errno(e),
    }
}

fn sys_socket_sendto_impl(
    h: u32,
    buf: *const u8,
    len: usize,
    flags: u32,
    addr: *const u8,
    addr_len: usize,
) -> i64 {
    if flags != 0 || len == 0 || len > socket::MAX_DATAGRAM || addr_len == 0 || addr_len > 32 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    if let Err(e) = validate_user_buf(addr as u64, addr_len) {
        return e;
    }
    let socket = match socket_lookup(h, socket_rights::SEND) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let data = match copy_user_bytes(buf, len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let addr_bytes = match copy_user_bytes(addr, addr_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let dst = match socket::SocketAddr::parse(&addr_bytes) {
        Ok(addr) => addr,
        Err(e) => return socket_errno(e),
    };
    match socket.send_to(dst, &data) {
        Ok(n) => n as i64,
        Err(e) => socket_errno(e),
    }
}

fn sys_socket_recv_impl(h: u32, buf: *mut u8, len: usize, flags: u32) -> i64 {
    if flags != 0 || len == 0 || len > socket::MAX_DATAGRAM {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    match tcp_socket_lookup(h, socket_rights::READ) {
        Ok(socket) => {
            let mut tmp = alloc::vec![0u8; len];
            for _ in 0..1024 {
                match socket.recv_stream(&mut tmp) {
                    Ok(n) => {
                        if let Err(e) = copy_kernel_bytes_to_user(buf, &tmp[..n]) {
                            return e;
                        }
                        return n as i64;
                    }
                    Err(TcpError::WouldBlock) => crate::sched::yield_now(),
                    Err(e) => return tcp_errno(e),
                }
            }
            return EAGAIN;
        }
        Err(EPERM) => return EPERM,
        Err(_) => {}
    }
    let socket = match socket_lookup(h, socket_rights::RECV) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let mut tmp = alloc::vec![0u8; len];
    for _ in 0..1024 {
        match socket.recv(&mut tmp) {
            Ok(n) => {
                if let Err(e) = copy_kernel_bytes_to_user(buf, &tmp[..n]) {
                    return e;
                }
                return n as i64;
            }
            Err(SocketError::WouldBlock) => crate::sched::yield_now(),
            Err(e) => return socket_errno(e),
        }
    }
    EAGAIN
}

fn sys_socket_recvfrom_impl(
    h: u32,
    buf: *mut u8,
    len: usize,
    flags: u32,
    addr_out: *mut u8,
    addr_len_io: *mut usize,
) -> i64 {
    if flags != 0 || len == 0 || len > socket::MAX_DATAGRAM {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    let wants_addr = !addr_out.is_null();
    let addr_capacity = if wants_addr {
        if addr_len_io.is_null() {
            return EINVAL;
        }
        if let Err(e) = validate_user_buf(addr_len_io as u64, core::mem::size_of::<usize>()) {
            return e;
        }
        let cap = match copy_user_usize(addr_len_io) {
            Ok(cap) => cap,
            Err(e) => return e,
        };
        if let Err(e) = validate_user_buf(addr_out as u64, cap) {
            return e;
        }
        cap
    } else {
        0
    };
    let socket = match socket_lookup(h, socket_rights::RECV) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let mut tmp = alloc::vec![0u8; len];
    for _ in 0..1024 {
        match socket.recv_from(&mut tmp) {
            Ok((n, src)) => {
                if wants_addr {
                    let mut encoded = [0u8; 20];
                    let encoded_len = encode_socket_addr(src, &mut encoded);
                    if addr_capacity < encoded_len {
                        return EINVAL;
                    }
                    if let Err(e) = copy_kernel_bytes_to_user(addr_out, &encoded[..encoded_len]) {
                        return e;
                    }
                    if let Err(e) = copy_kernel_usize_to_user(addr_len_io, encoded_len) {
                        return e;
                    }
                }
                if let Err(e) = copy_kernel_bytes_to_user(buf, &tmp[..n]) {
                    return e;
                }
                return n as i64;
            }
            Err(SocketError::WouldBlock) => crate::sched::yield_now(),
            Err(e) => return socket_errno(e),
        }
    }
    EAGAIN
}

fn encode_socket_addr(addr: socket::SocketAddr, out: &mut [u8; 20]) -> usize {
    let family = match addr.family {
        socket::AddressFamily::Inet4 => socket::AF_INET as u16,
        socket::AddressFamily::Inet6 => socket::AF_INET6 as u16,
    };
    out[0..2].copy_from_slice(&family.to_le_bytes());
    out[2..4].copy_from_slice(&addr.port.to_be_bytes());
    match addr.family {
        socket::AddressFamily::Inet4 => {
            out[4..8].copy_from_slice(&addr.addr[..4]);
            8
        }
        socket::AddressFamily::Inet6 => {
            out[4..20].copy_from_slice(&addr.addr);
            20
        }
    }
}

fn sys_socket_close_impl(h: u32) -> i64 {
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    let removed = match process::close_handle(&proc, h, |h| {
        matches!(
            h,
            Handle::Socket { .. } | Handle::TcpSocket { .. } | Handle::TcpListener { .. }
        )
    }) {
        Ok(removed) => removed,
        Err(e) => return e,
    };
    // An explicit close closes the object even if other handles name it. The
    // table entries are matched by identity, so an old handle's close never
    // takes out a newer socket on the same address.
    crate::arch::x86_64::without_interrupts(|| match removed {
        Handle::Socket { socket, .. } => socket.close(),
        Handle::TcpSocket { socket, .. } => tcp::close_connection(&socket),
        Handle::TcpListener { listener, .. } => tcp::close_listener(&listener),
        _ => {}
    });
    0
}

fn sys_net_stats_impl(h: u32, out: *mut NetStatsV1, out_len: usize) -> i64 {
    let stats_len = core::mem::size_of::<NetStatsV1>();
    if out.is_null() || out_len != stats_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, stats_len) {
        return e;
    }
    let socket = match socket_lookup(h, 0) {
        Ok(socket) => socket,
        Err(e) => return e,
    };
    let stats = socket.stats();
    match copy_net_stats_to_user(
        out,
        NetStatsV1 {
            packets_in: stats.packets_in,
            packets_out: stats.packets_out,
            bytes_in: stats.bytes_in,
            bytes_out: stats.bytes_out,
        },
    ) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// Resolve the calling process's `Arc<Pmap>`. Used by every syscall that
/// needs to mutate or translate through the caller's address space
/// (`sys_send`, `sys_recv`, `sys_map_region`). Returns `EINVAL` for the
/// kernel "process" (no pmap) or an unregistered pid (shouldn't happen).
fn pmap_for_caller() -> Result<alloc::sync::Arc<crate::mm::pmap::Pmap>, i64> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = crate::sched::process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        proc.pmap.as_ref().cloned().ok_or(EINVAL)
    })
}

/// Resolve the calling process's CHANNEL handle + pmap. Common preamble
/// for the IPC syscalls (`sys_send`, `sys_recv`). Returns
/// `(channel, side, pmap)` or a negative errno. Returns `EBADF` if the
/// handle index points to a non-Channel variant (e.g. a `ProcessHandle`
/// — the caller meant `sys_kill`, not `sys_send`).
fn ipc_lookup(
    h: u32,
    required_semantic: u8,
) -> Result<
    (
        alloc::sync::Arc<crate::ipc::Channel>,
        crate::ipc::EndpointSide,
        alloc::sync::Arc<crate::mm::pmap::Pmap>,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = crate::sched::process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        let pmap = proc.pmap.as_ref().cloned().ok_or(EINVAL)?;
        match handle {
            Handle::Channel {
                channel,
                side,
                rights,
                ..
            } if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) => {
                Ok((channel, side, pmap))
            }
            Handle::Channel { .. } => Err(EPERM),
            Handle::Process { .. }
            | Handle::File { .. }
            | Handle::Socket { .. }
            | Handle::TcpSocket { .. }
            | Handle::TcpListener { .. }
            | Handle::NetDial { .. }
            | Handle::Tensor { .. }
            | Handle::Model { .. } => Err(EBADF),
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { .. } => Err(EBADF),
            #[cfg(feature = "installer")]
            Handle::Installer { .. } => Err(EBADF),
            #[cfg(feature = "system-store")]
            Handle::SystemStore { .. } => Err(EBADF),
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { .. } => Err(EBADF),
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { .. }
            | Handle::InputFocus { .. }
            | Handle::GpuSubmit { .. }
            | Handle::InputSystem { .. }
            | Handle::GpuCompute { .. } => Err(EBADF),
        }
    })
}

fn channel_lookup(
    h: u32,
    required_semantic: u8,
) -> Result<
    (
        alloc::sync::Arc<crate::ipc::Channel>,
        crate::ipc::EndpointSide,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::Channel {
                channel,
                side,
                rights,
                ..
            } if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) => {
                Ok((channel, side))
            }
            Handle::Channel { .. } => Err(EPERM),
            Handle::Process { .. }
            | Handle::File { .. }
            | Handle::Socket { .. }
            | Handle::TcpSocket { .. }
            | Handle::TcpListener { .. }
            | Handle::NetDial { .. }
            | Handle::Tensor { .. }
            | Handle::Model { .. } => Err(EBADF),
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { .. } => Err(EBADF),
            #[cfg(feature = "installer")]
            Handle::Installer { .. } => Err(EBADF),
            #[cfg(feature = "system-store")]
            Handle::SystemStore { .. } => Err(EBADF),
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { .. } => Err(EBADF),
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { .. }
            | Handle::InputFocus { .. }
            | Handle::GpuSubmit { .. }
            | Handle::InputSystem { .. }
            | Handle::GpuCompute { .. } => Err(EBADF),
        }
    })
}

/// Resolve the calling process's PROCESS handle to its target pid.
/// Sibling of `ipc_lookup` for the kill path. Returns `EBADF` if the
/// handle index is unallocated or refers to a non-Process variant
/// (e.g. a channel handle — caller meant `sys_send`/`sys_recv`).
fn process_handle_lookup(
    h: u32,
    required_semantic: u8,
) -> Result<crate::sched::process::ProcessId, i64> {
    process_handle_lookup_pinned(h, required_semantic, false).map(|(pid, _)| pid)
}

/// As [`process_handle_lookup`]; with `pin`, also a claim on the target's
/// exit code, taken under the table lock while the authorizing handle is
/// still there (#171 F1), for a `sys_wait` or `SYS_INSPECT` to hold until
/// it has read the code.
fn process_handle_lookup_pinned(
    h: u32,
    required_semantic: u8,
    pin: bool,
) -> Result<
    (
        crate::sched::process::ProcessId,
        Option<crate::sched::process::ExitClaim>,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = crate::sched::process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handles = proc.handles.lock();
        let handle = handles.get(h).ok_or(EBADF)?;
        match handle {
            Handle::Process { target, rights, .. }
                if rights.has_meta(meta_rights::USE) && rights.has_semantic(required_semantic) =>
            {
                let claim = (pin && !cfg!(feature = "neo-test-wait-claim-pin-control"))
                    .then(|| crate::sched::process::ExitClaim::pin(target));
                Ok((target, claim))
            }
            Handle::Process { .. } => Err(EPERM),
            Handle::Channel { .. }
            | Handle::File { .. }
            | Handle::Socket { .. }
            | Handle::TcpSocket { .. }
            | Handle::TcpListener { .. }
            | Handle::NetDial { .. }
            | Handle::Tensor { .. }
            | Handle::Model { .. } => Err(EBADF),
            #[cfg(feature = "browser-runtime")]
            Handle::BrowserRuntime { .. } => Err(EBADF),
            #[cfg(feature = "installer")]
            Handle::Installer { .. } => Err(EBADF),
            #[cfg(feature = "system-store")]
            Handle::SystemStore { .. } => Err(EBADF),
            #[cfg(feature = "mount-authority")]
            Handle::MountAuthority { .. } => Err(EBADF),
            #[cfg(feature = "atrium-compositor-g1")]
            Handle::Buffer { .. }
            | Handle::InputFocus { .. }
            | Handle::GpuSubmit { .. }
            | Handle::InputSystem { .. }
            | Handle::GpuCompute { .. } => Err(EBADF),
        }
    })
}

/// SYS_SEND — send `len` bytes from `buf` (user VA) on the endpoint
/// identified by handle index `h`. Synchronous: blocks until a matching
/// recv arrives on the peer. Returns bytes sent, or negative errno.
fn sys_send_impl(h: u32, buf: u64, len: usize) -> i64 {
    if len == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf, len) {
        return e;
    }
    let (channel, side, pmap) = match ipc_lookup(h, channel_rights::SEND) {
        Ok(v) => v,
        Err(e) => return e,
    };
    // The channel lock is IRQ-safe by type, but the IPC handoff path also
    // touches scheduler queues after dropping that lock. CP-67: the channel is
    // owned by the thread, not this frame, while it may park.
    crate::sched::with_channel_anchor(channel, |channel| {
        crate::arch::x86_64::without_interrupts(|| channel.send(side, buf, len, pmap))
    })
}

/// SYS_RECV — receive up to `max_len` bytes into `buf` (user VA) on the
/// endpoint identified by handle index `h`. Synchronous: blocks until a
/// matching send arrives. Returns bytes received (≤ max_len), or
/// negative errno. Truncation: if the sender's payload exceeds max_len,
/// returns max_len; the caller compares to detect possible truncation
/// (ADR-004 §"Truncation policy").
fn sys_recv_impl(h: u32, buf: u64, max_len: usize) -> i64 {
    if max_len == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf, max_len) {
        return e;
    }
    let (channel, side, pmap) = match ipc_lookup(h, channel_rights::RECV) {
        Ok(v) => v,
        Err(e) => return e,
    };
    // The channel lock is IRQ-safe by type, but the IPC handoff path also
    // touches scheduler queues after dropping that lock. CP-67: the channel is
    // owned by the thread, not this frame, while it may park.
    crate::sched::with_channel_anchor(channel, |channel| {
        crate::arch::x86_64::without_interrupts(|| channel.recv(side, buf, max_len, pmap))
    })
}

/// `SYS_TRY_RECV`: `sys_recv_impl` without parking (EAGAIN when no sender
/// waits). Same lookup, same IRQ discipline.
#[cfg(feature = "atrium-pointer-forward")]
fn sys_try_recv_impl(h: u32, buf: u64, max_len: usize) -> i64 {
    if max_len == 0 {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf, max_len) {
        return e;
    }
    let (channel, side, pmap) = match ipc_lookup(h, channel_rights::RECV) {
        Ok(v) => v,
        Err(e) => return e,
    };
    crate::arch::x86_64::without_interrupts(|| channel.try_recv(side, buf, max_len, pmap))
}

/// One `SYS_WAIT_ANY` item.
#[cfg(feature = "wait-multiplex")]
#[derive(Clone, Copy)]
pub(crate) struct WaitItem {
    pub handle: u32,
    pub kind: u32,
}

/// What a wait's channel items named at one scan: each one's wait key and
/// the `CapId` of the handle that named it (Jarvis rung-1 F1: a numeric
/// handle can be closed and its slot reused, or revoked, between scans).
/// `CapId`s are kernel-unique and never reused, so equal identities mean the
/// same handle naming the same channel side.
#[cfg(feature = "wait-multiplex")]
#[derive(Clone, Copy, PartialEq, Eq)]
struct WaitIdentity {
    keys: [u64; crate::sched::wait_any::ITEMS_MAX],
    caps: [u64; crate::sched::wait_any::ITEMS_MAX],
    n: usize,
}

#[cfg(feature = "wait-multiplex")]
impl WaitIdentity {
    const EMPTY: Self = Self {
        keys: [0; crate::sched::wait_any::ITEMS_MAX],
        caps: [0; crate::sched::wait_any::ITEMS_MAX],
        n: 0,
    };
}

/// A channel item's channel, side and handle `CapId` (RECV + USE required).
#[cfg(feature = "wait-multiplex")]
fn wait_channel_lookup(
    h: u32,
) -> Result<
    (
        alloc::sync::Arc<crate::ipc::Channel>,
        crate::ipc::EndpointSide,
        process::CapId,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::Channel {
                channel,
                side,
                rights,
                cap_id,
            } if rights.has_meta(meta_rights::USE) && rights.has_semantic(channel_rights::RECV) => {
                Ok((channel, side, cap_id))
            }
            Handle::Channel { .. } => Err(EPERM),
            _ => Err(EBADF),
        }
    })
}

/// Readiness of every item now (bit i = item i), or an errno for a bad item.
/// Takes and drops every handle `Arc` itself: nothing owned survives into a
/// park (CP-30). Also returns what the channel items name (`ident`).
#[cfg(feature = "wait-multiplex")]
fn wait_any_scan(items: &[WaitItem], ident: &mut WaitIdentity) -> Result<u64, i64> {
    let mut mask = 0u64;
    *ident = WaitIdentity::EMPTY;
    for (i, item) in items.iter().enumerate() {
        let ready = match item.kind {
            WAIT_KIND_CHANNEL => {
                let (channel, side, cap_id) = wait_channel_lookup(item.handle)?;
                ident.keys[ident.n] = channel.wait_key(side);
                ident.caps[ident.n] = cap_id.0;
                ident.n += 1;
                channel.recv_ready(side)
            }
            #[cfg(feature = "atrium-input-routing")]
            WAIT_KIND_INPUT => {
                use crate::atrium::logic::focus_rights;
                let focus = crate::arch::x86_64::without_interrupts(|| {
                    let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
                    match proc.handles.lock().get(item.handle) {
                        Some(Handle::InputFocus { focus, rights, .. })
                            if rights.has_meta(process::meta_rights::USE)
                                && rights.has_semantic(focus_rights::RECEIVE)
                                && !rights.has_semantic(focus_rights::ACTIVATE) =>
                        {
                            Ok(focus.clone())
                        }
                        Some(Handle::InputFocus { .. }) => Err(EPERM),
                        _ => Err(EBADF),
                    }
                })?;
                focus.wait_ready()
            }
            #[cfg(feature = "atrium-pointer-forward")]
            WAIT_KIND_SAK => {
                caller_holds_input_system()?;
                crate::atrium::focus::sak_pending()
            }
            _ => return Err(EINVAL),
        };
        if ready {
            mask |= 1 << i;
        }
    }
    Ok(mask)
}

/// `SYS_WAIT_ANY` (THINK §3-§5). Level-triggered: returns which items are
/// ready now; the caller then does its ordinary non-blocking reads.
///
/// Input has no asynchronous producer, so a wait that names an input or SAK
/// item pumps the HID devices (as `SYS_INPUT_FOCUS_RECV` does) and, when
/// nothing is ready, parks only until the next tick, then pumps again. IPC
/// wakes are immediate (the send/grant/close path fires the waiter).
#[cfg(feature = "wait-multiplex")]
fn sys_wait_any_impl(items_ptr: u64, n: usize, deadline_ns: u64) -> i64 {
    use crate::sched::wait_any::ITEMS_MAX;
    if n > ITEMS_MAX {
        return EINVAL;
    }
    // Nothing to wait for and no deadline would park forever (AAG F2).
    if n == 0 && deadline_ns == u64::MAX {
        return EINVAL;
    }
    let mut raw = [0u8; ITEMS_MAX * 8];
    if n != 0 {
        if let Err(e) = validate_user_buf(items_ptr, n * 8) {
            return e;
        }
        // A fixed stack array, no heap (CP-30).
        if let Err(e) = user_copy(raw.as_mut_ptr(), items_ptr as *const u8, n * 8) {
            return e;
        }
    }
    let mut items = [WaitItem { handle: 0, kind: 0 }; ITEMS_MAX];
    for (item, bytes) in items.iter_mut().zip(raw.chunks_exact(8)).take(n) {
        let bytes: &[u8; 8] = bytes.try_into().expect("chunks of 8");
        (item.handle, item.kind) = crate::sched::wait_any_logic::decode_item(bytes);
    }
    wait_any_items(&items[..n], deadline_ns)
}

/// `SYS_WAIT_ANY`'s body over resolved items, for the calling thread's
/// process. Also the entry point of the kernel-side `wait_any_core` legs.
#[cfg(feature = "wait-multiplex")]
pub(crate) fn wait_any_items(items: &[WaitItem], deadline_ns: u64) -> i64 {
    use crate::sched::wait_any;
    use core::sync::atomic::Ordering;
    let pulls_input = items
        .iter()
        .any(|item| item.kind == WAIT_KIND_INPUT || item.kind == WAIT_KIND_SAK);
    let deadline_ticks = crate::sched::wait_any_logic::deadline_ticks(deadline_ns);
    let pid = crate::sched::current_pid();
    let mut first = WaitIdentity::EMPTY;
    let mut second = WaitIdentity::EMPTY;
    loop {
        if pulls_input {
            #[cfg(all(feature = "usb-hid", feature = "atrium-input-routing"))]
            hid_keyboard::pump_routed_events();
            #[cfg(all(feature = "ps2-keyboard", feature = "atrium-input-routing"))]
            crate::drivers::ps2::i8042::pump_routed_events();
            #[cfg(all(feature = "i2c-hid-touchpad", feature = "atrium-input-routing"))]
            crate::drivers::i2c_hid::touchpad::pump_routed_events();
            #[cfg(feature = "usb-hid-mouse")]
            crate::drivers::usb::hid_mouse::pump_routed_events();
        }
        let now = crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed);
        // Pulled sources are looked at again on the next tick.
        let park_until = if pulls_input {
            deadline_ticks.min(now + 1)
        } else {
            deadline_ticks
        };
        let mask = match wait_any_scan(items, &mut first) {
            Ok(v) => v,
            Err(e) => return e,
        };
        if mask != 0 {
            return mask as i64;
        }
        #[cfg(feature = "neo-test-wait-any-core")]
        wait_any::rebind_window_hook(pid);
        // Register, then check again (CP-14): a fire that lands after this
        // check finds the entry and sets `fired`, which the park sees.
        let Some(reg) = wait_any::register(pid, &first.keys[..first.n], park_until) else {
            return EAGAIN;
        };
        // AAG F1: never park a thread of an exiting process. `exiting` is set
        // before teardown purges the table, so either this sees it, or the
        // purge removes the entry and `park` declines (ticket mismatch), and
        // the next pass sees it.
        if process::get(pid).is_none_or(|p| p.is_exiting()) {
            wait_any::unregister(reg);
            return crate::errno::EPIPE;
        }
        let mask = match wait_any_scan(items, &mut second) {
            Ok(mask) => mask,
            Err(e) => {
                wait_any::unregister(reg);
                return e;
            }
        };
        if mask != 0 {
            wait_any::unregister(reg);
            return mask as i64;
        }
        // Jarvis rung-1 F1: an item's handle can be closed and its slot
        // reused for another channel, or revoked, after the first scan. A
        // removal fires the removed handle's key (`HandleTable` chokepoints),
        // but one that lands before the registration fires nobody. The second
        // scan reads every handle after the registration, so either it sees
        // the change here (start over: watch what the handles name now, or
        // return EBADF), or the removal comes later and fires this entry.
        if !cfg!(feature = "wait-multiplex-no-identity-check") && second != first {
            wait_any::unregister(reg);
            #[cfg(feature = "neo-test")]
            wait_any::STATS
                .identity_retries
                .fetch_add(1, Ordering::Relaxed);
            continue;
        }
        if now >= deadline_ticks {
            wait_any::unregister(reg);
            return crate::errno::ETIMEDOUT;
        }
        let why = crate::arch::x86_64::without_interrupts(|| wait_any::park(reg));
        if why == EAGAIN {
            return EAGAIN;
        }
        // An IPC wake re-scans at once; a tick wake re-pumps input, then
        // re-scans. Either way the deadline check above ends the loop.
    }
}

fn sys_write_impl(fd: u32, buf: *const u8, len: usize) -> i64 {
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }

    // Namespace rule for BUILD-1: fd 1 is permanently stdout.
    // sys_open never returns 1, and sys_write(1, ...) never resolves the
    // process handle table.
    if fd != STDOUT_FD {
        return sys_file_write_impl(fd, buf, len);
    }

    let mut copied = 0;
    while copied < len {
        let chunk_len = min(SYS_WRITE_CHUNK, len - copied);
        let mut chunk = [0u8; SYS_WRITE_CHUNK];
        // A fault ends the write: what was written stands (its count), and
        // nothing written is EFAULT.
        if user_copy(chunk.as_mut_ptr(), buf.wrapping_add(copied), chunk_len).is_err() {
            return if copied == 0 { EFAULT } else { copied as i64 };
        }
        write_bytes(&chunk[..chunk_len]);
        #[cfg(feature = "framebuffer-console")]
        crate::drivers::fb::console::write_bytes(&chunk[..chunk_len]);
        copied += chunk_len;
    }

    len as i64
}

fn sys_open_impl(path_ptr: *const u8, path_len: usize, flags: u32, rights: u32) -> i64 {
    if path_len == 0 || path_len > MAX_OPEN_PATH_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(path_ptr as u64, path_len) {
        return e;
    }
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };

    let path_bytes = match copy_user_bytes(path_ptr, path_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let path = match core::str::from_utf8(&path_bytes) {
        Ok(path) => path,
        Err(_) => return EINVAL,
    };
    open_path_and_install(&proc, path, flags, rights)
}

/// `sys_open` after the user copy: resolve (and maybe create) the path with
/// the VFS body IF-on, then install the handle IF-off as before.
fn open_path_and_install(
    proc: &alloc::sync::Arc<process::Process>,
    path: &str,
    flags: u32,
    rights: u32,
) -> i64 {
    #[cfg(feature = "system-store")]
    if let Err(e) = system_store_check(proc, path) {
        return e;
    }
    // Dexter #187 item 3a: a plain open no longer carries `MOUNT` (it has no
    // use without the mount authority, which `SYS_MOUNT`/`SYS_UMOUNT` check
    // first). Masked rather than refused, so an open asking for it still
    // opens the directory.
    #[cfg(feature = "mount-authority")]
    let rights = if may_mount(proc) {
        rights
    } else {
        rights & !FileRights::MOUNT.bits()
    };
    // OT-66 K1a/K1b: the root form resolves only through a directory cap the
    // caller holds whose scope contains the path, and asks for no more than
    // the cap's rights.
    #[cfg(feature = "cap-rooted-open")]
    let (root_cap, root_cap_rights) = {
        let normalized = match fs::normalize_absolute_path(path) {
            Ok(normalized) => normalized,
            Err(e) => return vfs_errno(e),
        };
        match process::root_dir_cap_for(proc, &normalized, FileRights::from_bits(rights)) {
            Ok(cap) => cap,
            Err(e) => return e,
        }
    };
    let file = match vfs_body_if_on(|| {
        fs::open_root_temporary(path, flags, FileRights::from_bits(rights))
    }) {
        Ok(file) => file,
        Err(e) => return vfs_errno(e),
    };
    // OT-87 R2: a pinned vocab name on the installed NeoFS volume opened to
    // read is served only when its verified content hash equals the pin. An
    // open for writing (the installer) is not checked.
    #[cfg(feature = "model-from-neofs")]
    if !FileRights::from_bits(rights).contains(FileRights::WRITE)
        && let Some(pin @ crate::boot_model_logic::NeofsPin::Vocab(_)) =
            crate::neofs_model::pin_for(path)
        && let Err(e) = vfs_body_if_on(|| crate::neofs_model::check(&file, path, pin))
    {
        return e;
    }

    // ISC-3: the opener roots the file's derivation tree. The backing `OpenFile`
    // keeps the user's semantic rights (file I/O semantics unchanged); only the
    // capability handle gains the owner meta-rights (DERIVE+GRANT) so it can be
    // derived/granted/inherited. With `file-caps` off, no meta bits are added —
    // the handle carries exactly the user's rights, as before this build.
    let handle_rights = crate::sched::file_cap_logic::root_file_handle_rights(
        rights,
        crate::sched::file_cap_logic::FILE_CAPS_ENABLED,
    );
    // OT-66 K1a: the opened file is the RootDirCap's child (so revoking the
    // cap reaches it), and its meta-rights are only those the cap holds.
    #[cfg(feature = "cap-rooted-open")]
    let handle_rights = handle_rights & root_cap_rights.bits();
    #[cfg(feature = "cap-rooted-open")]
    let parent = Some(root_cap);
    #[cfg(not(feature = "cap-rooted-open"))]
    let parent = None;
    let handle = Handle::File {
        file,
        cap_id: process::alloc_cap_id(),
        rights: FileRights::from_bits(handle_rights),
    };
    match process::install_cap_excluding(proc, handle, parent, RESERVED_FILE_FDS) {
        Ok(idx) => idx as i64,
        // The cap was revoked or closed while the open ran: no cap.
        #[cfg(feature = "cap-rooted-open")]
        Err(process::InstallCapError::ParentGone) => EPERM,
        Err(_) => EINVAL,
    }
}

/// Run the VFS/NeoFS body of a file syscall with interrupts on (CP-56 / CP-82):
/// the NeoFS locks (`entries`, `root`, `record`, `inode_block`, `metadata`)
/// are plain `SwitchMutex`es that `sys_read`/`sys_write` already hold IF-on
/// and preemptible (OT-74), so a body that took them at the syscall's IF=0
/// could spin forever on its own CPU behind a preempted holder. User copies and
/// handle-table work stay outside, IF-off, as in `sys_read`/`sys_write`.
/// Kill-switch: `neo-test-vfs-syscall-if-off-control` runs the body at the
/// caller's IF, as before (test only: its disabled state is the known hang).
#[inline(always)]
fn vfs_body_if_on<R>(f: impl FnOnce() -> R) -> R {
    #[cfg(not(feature = "neo-test-vfs-syscall-if-off-control"))]
    {
        crate::arch::x86_64::with_interrupts_enabled(f)
    }
    #[cfg(feature = "neo-test-vfs-syscall-if-off-control")]
    {
        f()
    }
}

fn sys_read_impl(fd: u32, buf: *mut u8, len: usize) -> i64 {
    if len == 0 {
        return 0;
    }
    if len > MAX_FILE_IO {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    #[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
    if fd == STDIN_FD {
        return sys_read_stdin(buf, len);
    }
    let (file, rights) = match file_lookup_with_rights(fd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let mut tmp = alloc::vec![0u8; len];
    // Originally added because NeoFS's `CacheInner` was a preemptible
    // `SwitchMutex` an IF-off reader could spin on forever (OT-59 #2 / CP-16 /
    // CP-2). That lock is IRQ-safe since OT-74 (2026-10-16), but the NeoFS
    // inode-level locks (`entries`/`root`/`record`, plain `SwitchMutex`es) can
    // still be held across a block-I/O park here, so the read keeps running
    // IF-on (into a kernel buffer); the user copy below stays IF-off.
    let n = match crate::arch::x86_64::with_interrupts_enabled(|| file.read_as(&mut tmp, rights)) {
        Ok(n) => n,
        Err(e) => return vfs_errno(e),
    };
    if let Err(e) = copy_kernel_bytes_to_user(buf, &tmp[..n]) {
        return e;
    }
    n as i64
}

#[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
fn sys_read_stdin(buf: *mut u8, len: usize) -> i64 {
    let mut tmp = [0u8; SYS_WRITE_CHUNK];
    let max_len = min(len, tmp.len());
    // SYSCALL entry masks IF; keep interrupts enabled while this blocking read
    // waits so HID/serial input can arrive, then restore IF before user copy.
    let first = crate::arch::x86_64::with_interrupts_enabled(|| {
        loop {
            if let Some(byte) = try_read_stdin_byte() {
                break byte;
            }
            crate::sched::yield_now();
            core::hint::spin_loop();
        }
    });
    tmp[0] = first;

    let mut copied = 1;
    while copied < max_len {
        let Some(byte) = try_read_stdin_byte() else {
            break;
        };
        tmp[copied] = byte;
        copied += 1;
    }

    if let Err(e) = copy_kernel_bytes_to_user(buf, &tmp[..copied]) {
        return e;
    }
    copied as i64
}

#[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
fn try_read_stdin_byte() -> Option<u8> {
    // Jarvis F8: an fd-0 reader advances routed pointer input too, as
    // SYS_INPUT_FOCUS_RECV does (device locks are held IF=0, CP-37).
    #[cfg(all(feature = "usb-hid-mouse", feature = "atrium-input-routing"))]
    if crate::atrium::input_routing_enabled() {
        crate::arch::x86_64::without_interrupts(crate::drivers::usb::hid_mouse::pump_routed_events);
    }
    #[cfg(feature = "usb-hid")]
    if let Some(byte) = hid_keyboard::try_read_byte() {
        return Some(byte);
    }
    #[cfg(feature = "serial-stdin")]
    if let Some(byte) = try_read_serial_byte() {
        return Some(byte);
    }
    None
}

fn sys_readdir_impl(fd: u32, out_ptr: *mut u8, out_len: usize, cursor_ptr: *mut u64) -> i64 {
    if out_len == 0 || out_len > MAX_DIRENT_IO {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out_ptr as u64, out_len) {
        return e;
    }
    if let Err(e) = validate_user_buf(cursor_ptr as u64, core::mem::size_of::<u64>()) {
        return e;
    }

    let cursor = match copy_user_u64(cursor_ptr) {
        Ok(cursor) => cursor,
        Err(e) => return e,
    };
    let (file, rights) = match file_lookup_with_rights(fd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let entry = match vfs_body_if_on(|| file.readdir_one(cursor, rights)) {
        Ok(Some(entry)) => entry,
        Ok(None) => return 0,
        Err(e) => return vfs_errno(e),
    };
    let mut record = alloc::vec![0u8; 16 + entry.name.len()];
    match encode_dirent_v1(&entry, &mut record) {
        Ok(n) if n <= out_len => {
            if let Err(e) = copy_kernel_bytes_to_user(out_ptr, &record[..n]) {
                return e;
            }
            if let Err(e) = copy_kernel_u64_to_user(cursor_ptr, entry.next_cursor) {
                return e;
            }
            n as i64
        }
        Ok(_) | Err(VfsError::Invalid) => EINVAL,
        Err(e) => vfs_errno(e),
    }
}

// === BUILD-4B mount/umount ============================================

/// Maximum `MountRequestV1` buffer length accepted by `sys_mount`. The header
/// is 32 bytes; the rest carries source + target strings. 1 KiB is well
/// beyond any plausible single-mount request and keeps SMAP windows tight.
const MAX_MOUNT_REQUEST_LEN: usize = 1024;
/// Maximum `name` length for `sys_umount`. Same envelope as
/// `MAX_OPEN_PATH_LEN` to keep the user-half copy bounded.
const MAX_UMOUNT_NAME_LEN: usize = MAX_OPEN_PATH_LEN;

#[derive(Debug)]
struct MountRequestV1 {
    fs_type: u32,
    source: alloc::string::String,
    target: alloc::string::String,
}

fn parse_mount_request_v1(buf: &[u8]) -> Result<MountRequestV1, i64> {
    if buf.len() < 32 {
        return Err(EINVAL);
    }
    let version = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if version != 1 {
        return Err(EINVAL);
    }
    let fs_type = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let source_off = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as usize;
    let source_len = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
    let target_off = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]) as usize;
    let target_len = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]) as usize;
    let reserved0 = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let reserved1 = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    if reserved0 != 0 || reserved1 != 0 {
        return Err(EINVAL);
    }
    let source_end = source_off.checked_add(source_len).ok_or(EINVAL)?;
    let target_end = target_off.checked_add(target_len).ok_or(EINVAL)?;
    if source_end > buf.len() || target_end > buf.len() {
        return Err(EINVAL);
    }
    if source_off < 32 || target_off < 32 {
        return Err(EINVAL);
    }
    let source = alloc::string::String::from(
        core::str::from_utf8(&buf[source_off..source_end]).map_err(|_| EINVAL)?,
    );
    let target = alloc::string::String::from(
        core::str::from_utf8(&buf[target_off..target_end]).map_err(|_| EINVAL)?,
    );
    Ok(MountRequestV1 {
        fs_type,
        source,
        target,
    })
}

fn do_mount(dir_fd: u32, req_bytes: &[u8], flags: u32) -> i64 {
    if flags & !fs::MOUNT_FLAG_EXEC_ALLOWED != 0 {
        return EINVAL;
    }
    #[cfg(feature = "mount-authority")]
    if let Err(e) = caller_holds_mount_authority() {
        return e;
    }
    let parsed = match parse_mount_request_v1(req_bytes) {
        Ok(p) => p,
        Err(e) => return e,
    };
    let (file, rights) = match file_lookup_with_rights(dir_fd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    match file.mount_child(
        &parsed.target,
        &parsed.source,
        parsed.fs_type,
        MountFlags(flags),
        rights,
    ) {
        Ok(()) => 0,
        Err(e) => vfs_errno(e),
    }
}

fn do_umount(dir_fd: u32, name_bytes: &[u8], flags: u32) -> i64 {
    if flags != 0 {
        return EINVAL;
    }
    #[cfg(feature = "mount-authority")]
    if let Err(e) = caller_holds_mount_authority() {
        return e;
    }
    let name = match core::str::from_utf8(name_bytes) {
        Ok(s) => s,
        Err(_) => return EINVAL,
    };
    let (file, rights) = match file_lookup_with_rights(dir_fd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    match file.umount_child(name, rights) {
        Ok(()) => 0,
        Err(e) => vfs_errno(e),
    }
}

fn sys_mount_impl(dir_fd: u32, req_ptr: *const u8, req_len: usize, flags: u32) -> i64 {
    if req_len == 0 || req_len > MAX_MOUNT_REQUEST_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(req_ptr as u64, req_len) {
        return e;
    }
    let req_bytes = match copy_user_bytes(req_ptr, req_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    do_mount(dir_fd, &req_bytes, flags)
}

fn sys_umount_impl(dir_fd: u32, name_ptr: *const u8, name_len: usize, flags: u32) -> i64 {
    if name_len == 0 || name_len > MAX_UMOUNT_NAME_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(name_ptr as u64, name_len) {
        return e;
    }
    let name_bytes = match copy_user_bytes(name_ptr, name_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    do_umount(dir_fd, &name_bytes, flags)
}

#[cfg(feature = "neo-test-phase3-mount-table")]
pub fn debug_sys_mount_for_test(dir_fd: u32, req_bytes: &[u8], flags: u32) -> i64 {
    do_mount(dir_fd, req_bytes, flags)
}

#[cfg(feature = "neo-test-phase3-mount-table")]
pub fn debug_sys_umount_for_test(dir_fd: u32, name: &str, flags: u32) -> i64 {
    do_umount(dir_fd, name.as_bytes(), flags)
}

fn sys_file_write_impl(fd: u32, buf: *const u8, len: usize) -> i64 {
    if len == 0 {
        return 0;
    }
    if len > MAX_FILE_IO {
        return EINVAL;
    }
    let file = match file_lookup(fd, FileRights::WRITE) {
        Ok(file) => file,
        Err(e) => return e,
    };
    let data = match copy_user_bytes(buf, len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    // See the IF-on note in `sys_read` (OT-59 #2 / CP-16, revised by OT-74): the
    // user copy above is done; the NeoFS write runs IF-on so a thread preempted
    // or parked while holding a NeoFS inode lock can be rescheduled.
    match crate::arch::x86_64::with_interrupts_enabled(|| file.write(&data)) {
        Ok(n) => n as i64,
        Err(e) => vfs_errno(e),
    }
}

fn sys_close_impl(fd: u32) -> i64 {
    #[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
    if fd == STDIN_FD {
        return EBADF;
    }
    if fd == STDOUT_FD {
        return EBADF;
    }
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let removed = process::close_handle(&proc, fd, |h| {
        matches!(h, Handle::File { .. } | Handle::Channel { .. })
    });
    match removed {
        Ok(Handle::File { .. }) => 0,
        Ok(Handle::Channel { channel, side, .. }) => {
            // Explicit endpoint drop has the same observable peer semantics
            // as process death: a blocked/next peer operation receives EPIPE.
            // Run after releasing the handle/derivation locks; close_peer may
            // wake a scheduler waiter.
            channel.close_peer(side, crate::sched::current_pid());
            0
        }
        Ok(_) => unreachable!(),
        Err(e) => e,
    }
}

fn sys_fsync_impl(fd: u32) -> i64 {
    // fsync needs no right: it moves no data across the handle (POSIX allows
    // it on a read-only descriptor).
    match file_lookup(fd, FileRights::empty()) {
        Ok(file) => match vfs_body_if_on(|| file.fsync()) {
            Ok(()) => 0,
            Err(e) => vfs_errno(e),
        },
        Err(e) => e,
    }
}

fn sys_ftruncate_impl(fd: u32, new_len: u64) -> i64 {
    let file = match file_lookup(fd, FileRights::WRITE) {
        Ok(file) => file,
        Err(e) => return e,
    };
    match vfs_body_if_on(|| file.truncate(new_len)) {
        Ok(()) => 0,
        Err(e) => vfs_errno(e),
    }
}

/// OT-67 W1: the largest replace (the editor's file cap). Checked before any
/// copy, so a caller can't size a kernel allocation past it.
#[cfg(feature = "neofs-file-replace")]
pub const MAX_REPLACE_BYTES: usize = 4 * 1024 * 1024;

/// SYS_FILE_REPLACE(fd, ptr, len): the file's content becomes exactly the
/// `len` bytes at `ptr`, in one commit (a cut leaves the old content or the
/// new). Needs WRITE. `EINVAL` past the cap, `ESTALE` when another handle
/// replaced the file first.
#[cfg(feature = "neofs-file-replace")]
fn sys_file_replace_impl(fd: u32, buf: *const u8, len: usize) -> i64 {
    if len > MAX_REPLACE_BYTES {
        return EINVAL;
    }
    let file = match file_lookup(fd, FileRights::WRITE) {
        Ok(file) => file,
        Err(e) => return e,
    };
    // #209 AAG: `copy_user_bytes` trusts its caller to have checked that the
    // whole range is user memory. Without this, a kernel address copied
    // kernel memory into the file.
    if let Err(e) = validate_user_buf(buf as u64, len) {
        return e;
    }
    let data = match copy_user_bytes_in_chunks(buf, len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    file_replace_body(&file, &data)
}

/// The bytes one chunk of a replace's user copy holds interrupts off for
/// (the syscall entry clears IF): 64 KiB, so a 4 MiB replace opens an
/// interrupt window 64 times instead of copying 1000 times a write's 4 KiB
/// with interrupts off.
#[cfg(feature = "neofs-file-replace")]
const REPLACE_COPY_CHUNK: usize = 64 * 1024;

/// [`copy_user_bytes`] for a validated range, in [`REPLACE_COPY_CHUNK`]s with
/// an interrupt window after each.
#[cfg(feature = "neofs-file-replace")]
fn copy_user_bytes_in_chunks(src: *const u8, len: usize) -> Result<alloc::vec::Vec<u8>, i64> {
    let mut out: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    out.try_reserve_exact(len).map_err(|_| ENOMEM)?;
    let mut done = 0;
    while done < len {
        let n = core::cmp::min(REPLACE_COPY_CHUNK, len - done);
        // `try_reserve_exact` gave `len` bytes of capacity and
        // `done + n <= len`. The length is set only once the bytes are written.
        user_copy(
            out.as_mut_ptr().wrapping_add(done),
            src.wrapping_add(done),
            n,
        )?;
        // SAFETY: the copy wrote bytes `done..done + n`.
        unsafe { out.set_len(done + n) };
        done += n;
        if done < len {
            crate::arch::x86_64::with_interrupts_enabled(core::hint::spin_loop);
        }
    }
    Ok(out)
}

/// The replace after the user copy: IF-on, as `sys_file_write_impl`'s write.
#[cfg(feature = "neofs-file-replace")]
fn file_replace_body(file: &crate::fs::OpenFile, data: &[u8]) -> i64 {
    match crate::arch::x86_64::with_interrupts_enabled(|| file.replace(data)) {
        Ok(()) => 0,
        Err(e) => vfs_errno(e),
    }
}

/// W1's scenario: the real syscall entry with a length past the cap and no
/// buffer at all. It must refuse before touching `buf`.
#[cfg(feature = "neo-test-neofs-file-replace")]
pub fn debug_sys_file_replace_over_cap_for_test(fd: u32) -> i64 {
    sys_file_replace_impl(fd, core::ptr::null(), MAX_REPLACE_BYTES + 1)
}

/// #209 AAG: the real syscall entry with a buffer that is not user memory.
/// `kernel_va` names kernel memory; the second call's range starts 16 bytes
/// below the user half's end and runs past it. Both must be `EFAULT` before
/// any copy.
#[cfg(feature = "neo-test-neofs-file-replace")]
pub fn debug_sys_file_replace_bad_buffers_for_test(fd: u32, kernel_va: *const u8) -> (i64, i64) {
    let kernel = sys_file_replace_impl(fd, kernel_va, 4096);
    let straddle = sys_file_replace_impl(fd, (USER_HALF_LIMIT - 16) as *const u8, 32);
    (kernel, straddle)
}

/// W1's scenario: the syscall's checks and body with a kernel buffer.
#[cfg(feature = "neo-test-neofs-file-replace")]
pub fn debug_sys_file_replace_for_test(fd: u32, data: &[u8]) -> i64 {
    if data.len() > MAX_REPLACE_BYTES {
        return EINVAL;
    }
    let file = match file_lookup(fd, FileRights::WRITE) {
        Ok(file) => file,
        Err(e) => return e,
    };
    file_replace_body(&file, data)
}

fn encode_dirent_v1(entry: &VfsDirEntry, out: &mut [u8]) -> Result<usize, VfsError> {
    let name = entry.name.as_bytes();
    let name_len = u16::try_from(name.len()).map_err(|_| VfsError::Invalid)?;
    let record_len = 16usize.checked_add(name.len()).ok_or(VfsError::Invalid)?;
    if out.len() < record_len {
        return Err(VfsError::Invalid);
    }
    out[0..4].copy_from_slice(&(record_len as u32).to_le_bytes());
    out[4..6].copy_from_slice(&name_len.to_le_bytes());
    out[6] = dirent_kind(entry.kind);
    out[7] = 0;
    out[8..16].copy_from_slice(&entry.next_cursor.to_le_bytes());
    out[16..record_len].copy_from_slice(name);
    Ok(record_len)
}

fn dirent_kind(kind: InodeKind) -> u8 {
    match kind {
        InodeKind::Directory => 1,
        InodeKind::Regular => 2,
        InodeKind::Symlink => 3,
        InodeKind::Model => 4,
    }
}

#[cfg(any(
    feature = "neo-test-phase3-neofs-fsync-noop-write-through",
    feature = "neo-test-phase3-neofs-3e"
))]
pub fn debug_sys_fsync_for_test(fd: u32) -> i64 {
    sys_fsync_impl(fd)
}

/// `sys_model_load` with a kernel-resident path: the same body after the user copy.
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub fn debug_sys_model_load_kernel_path_for_test(path: &str) -> i64 {
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    model_load_path(&proc, path, fs::OPEN_ROOT_TEMPORARY)
}

/// `sys_open` with a kernel-resident path: the same body after the user copy.
#[cfg(feature = "neo-test-phase3-neofs-syscall-if-on")]
pub fn debug_sys_open_kernel_path_for_test(path: &str, flags: u32, rights: u32) -> i64 {
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    open_path_and_install(&proc, path, flags, rights)
}

#[cfg(any(
    feature = "neo-test-neofs-truncate",
    feature = "neo-test-phase3-neofs-syscall-if-on"
))]
pub fn debug_sys_ftruncate_for_test(fd: u32, new_len: u64) -> i64 {
    sys_ftruncate_impl(fd, new_len)
}

#[cfg(feature = "neo-test")]
pub fn debug_sys_readdir_kernel_buf_for_test(fd: u32, out: &mut [u8], cursor: &mut u64) -> i64 {
    if out.is_empty() || out.len() > MAX_DIRENT_IO {
        return EINVAL;
    }
    let (file, rights) = match file_lookup_with_rights(fd) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let entry = match vfs_body_if_on(|| file.readdir_one(*cursor, rights)) {
        Ok(Some(entry)) => entry,
        Ok(None) => return 0,
        Err(e) => return vfs_errno(e),
    };
    match encode_dirent_v1(&entry, out) {
        Ok(n) => {
            *cursor = entry.next_cursor;
            n as i64
        }
        Err(e) => vfs_errno(e),
    }
}

#[cfg(feature = "neo-test-phase3-neofs-2e")]
pub fn debug_vfs_errno_for_test(e: VfsError) -> i64 {
    vfs_errno(e)
}

/// The OpenFile behind `fd` and the rights of the handle that names it. A
/// derive narrows only the handle and keeps the same OpenFile, so an operation
/// whose OpenFile method checks rights after the inode's type (read, readdir,
/// mount, umount) is handed both and checks their intersection, keeping its
/// type-first error order.
fn file_lookup_with_rights(fd: u32) -> Result<(alloc::sync::Arc<OpenFile>, FileRights), i64> {
    #[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
    if fd == STDIN_FD {
        return Err(EBADF);
    }
    if fd == STDOUT_FD {
        return Err(EBADF);
    }
    let proc = current_process_for_syscall()?;
    crate::arch::x86_64::without_interrupts(|| {
        let handle = proc.handles.lock().get(fd).ok_or(EBADF)?;
        match handle {
            Handle::File { file, rights, .. } => Ok((file, rights)),
            _ => Err(EBADF),
        }
    })
}

fn file_lookup(fd: u32, required: FileRights) -> Result<alloc::sync::Arc<OpenFile>, i64> {
    #[cfg(any(feature = "serial-stdin", feature = "usb-hid"))]
    if fd == STDIN_FD {
        return Err(EBADF);
    }
    if fd == STDOUT_FD {
        return Err(EBADF);
    }
    let proc = current_process_for_syscall()?;
    crate::arch::x86_64::without_interrupts(|| {
        let handle = proc.handles.lock().get(fd).ok_or(EBADF)?;
        match handle {
            Handle::File { file, rights, .. } if rights.contains(required) => Ok(file),
            Handle::File { .. } => Err(EACCES),
            _ => Err(EBADF),
        }
    })
}

pub(crate) fn vfs_errno(e: VfsError) -> i64 {
    match e {
        VfsError::NotFound => ENOENT,
        VfsError::Invalid => EINVAL,
        VfsError::Unsupported => ENOSYS,
        VfsError::TypeForbidden => EISDIR,
        VfsError::NotDirectory => ENOTDIR,
        VfsError::InsufficientRights => EACCES,
        VfsError::Stale => ESTALE,
        VfsError::AlreadyExists => EEXIST,
        VfsError::Busy => EBUSY,
        VfsError::Io => crate::errno::EIO,
        #[cfg(feature = "model-boot-module")]
        VfsError::Integrity => crate::errno::EBADMSG,
    }
}

/// Maximum size accepted by a single sys_map_region call. Phase 1.5b cap;
/// 16 MiB is more than any plausible Phase 1.5b userspace would request
/// in one shot. Phase 5+ tensor_map will need a larger cap or no cap.
const MAX_MAP_REGION_SIZE: usize = 16 * 1024 * 1024;

/// SYS_MAP_REGION — allocate `size` bytes of fresh user-half address
/// space, frame-allocate + map each page with USER_DATA_RW flags
/// (read+write+NX, per W^X), return the base user VA on success.
///
/// Per ADR-006 §Q1+Q2: kernel-chosen placement via the per-Pmap
/// monotonic mmap cursor (`USER_HEAP_BASE`-based). No `unmap` in
/// Phase 1.5b — regions persist for the process's lifetime; cleanup
/// happens at process exit via `Pmap::Drop`'s walker.
fn sys_map_region_impl(size: usize) -> i64 {
    if size == 0 || size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    match pmap.allocate_user_region(size, crate::mm::pmap::flags::USER_DATA_RW) {
        Ok(va) => {
            crate::arch::x86_64::flush_local_tlb();
            va as i64
        }
        Err(_) => ENOMEM,
    }
}

// ── OT-61 browser C1.0 host-runtime primitives (VM map/unmap/protect + clock) ──

/// Translate POSIX prot bits → page-table flags, **enforcing W^X**: a request
/// for both WRITE and EXEC is refused (`EINVAL`) — the A1 invariant. This is the
/// user-facing W^X gate (mirrored by `check_user_flags` at the Pmap boundary).
#[cfg(feature = "browser-runtime")]
fn prot_to_flags(prot: u64) -> Result<x86_64::structures::paging::PageTableFlags, i64> {
    let w = prot & PROT_WRITE != 0;
    let x = prot & PROT_EXEC != 0;
    if w && x {
        return Err(EINVAL); // W^X: never writable+executable.
    }
    if prot & !(PROT_READ | PROT_WRITE | PROT_EXEC) != 0 {
        return Err(EINVAL); // unknown prot bits.
    }
    Ok(if x {
        crate::mm::pmap::flags::USER_CODE // R+X
    } else if w {
        crate::mm::pmap::flags::USER_DATA_RW // R+W+NX
    } else {
        crate::mm::pmap::flags::USER_DATA_RO // R+NX
    })
}

/// SYS_VM_MAP(size, prot, guard) → base VA | errno. Maps `size` bytes of fresh
/// anonymous memory with `prot` (W^X-enforced); `guard!=0` leaves an unmapped
/// guard page below the region (a downward overflow faults, contained).
#[cfg(feature = "browser-runtime")]
fn sys_vm_map_impl(size: usize, prot: u64, guard: bool) -> i64 {
    if size == 0 || size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    let flags = match prot_to_flags(prot) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    match pmap.allocate_user_region_guarded(size, flags, guard) {
        Ok(va) => {
            crate::arch::x86_64::flush_local_tlb();
            va as i64
        }
        Err(crate::mm::pmap::PmapError::WriteAndExecute) => EINVAL,
        Err(_) => ENOMEM,
    }
}

/// SYS_VM_UNMAP(va, size) → 0 | errno. Fail-closed: an unmapped page in the
/// range is `EINVAL` (no silent partial unmap).
#[cfg(feature = "browser-runtime")]
fn sys_vm_unmap_impl(va: u64, size: usize) -> i64 {
    // Bound `size` (AAG must-fix 2): unbounded `size` → overflow + lock-held loop.
    if size == 0 || size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    // Validate the address BEFORE constructing VirtAddr (AAG must-fix 1):
    // `VirtAddr::new` panics on a non-canonical va and the kernel is panic=abort,
    // so a raw user u64 must go through `try_new` → EFAULT, never a panic.
    let va = match x86_64::VirtAddr::try_new(va) {
        Ok(v) => v,
        Err(_) => return EFAULT,
    };
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    match pmap.unmap_user_range(va, size) {
        Ok(()) => {
            crate::arch::x86_64::flush_local_tlb();
            0
        }
        Err(crate::mm::pmap::PmapError::NotMapped) => EINVAL,
        Err(crate::mm::pmap::PmapError::UserHalfOverlapsKernel) => EFAULT,
        Err(_) => EINVAL,
    }
}

/// SYS_VM_PROTECT(va, size, prot) → 0 | errno. Changes perms in place;
/// **W^X-enforced** (a request →W+X is `EINVAL`); an unmapped page is `EINVAL`.
#[cfg(feature = "browser-runtime")]
fn sys_vm_protect_impl(va: u64, size: usize, prot: u64) -> i64 {
    // Bound `size` (AAG must-fix 2) + validate `va` before VirtAddr (must-fix 1).
    if size == 0 || size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    let va = match x86_64::VirtAddr::try_new(va) {
        Ok(v) => v,
        Err(_) => return EFAULT,
    };
    let flags = match prot_to_flags(prot) {
        Ok(f) => f,
        Err(e) => return e, // W+X → EINVAL here.
    };
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    match pmap.protect_user(va, size, flags) {
        Ok(()) => {
            crate::arch::x86_64::flush_local_tlb();
            0
        }
        Err(crate::mm::pmap::PmapError::WriteAndExecute) => EINVAL,
        Err(crate::mm::pmap::PmapError::NotMapped) => EINVAL,
        Err(crate::mm::pmap::PmapError::UserHalfOverlapsKernel) => EFAULT,
        Err(_) => EINVAL,
    }
}

/// SYS_CLOCK_GETTIME(clock_id) → nanoseconds | errno. Monotonic (nondecreasing)
/// from the 100 Hz `TICKS` counter (10 ms resolution). Realtime is the same
/// monotonic value plus a boot epoch (C1 P4: one clock source): with
/// `rtc-epoch` the base the RTC seeded at boot (`rtc::realtime_base_ns`, the
/// RTC's local wall time, no offset — OT-85), else 0, boot-relative as C1.0.
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
fn sys_clock_gettime_impl(clock_id: u64) -> i64 {
    if clock_id != CLOCK_MONOTONIC && clock_id != CLOCK_REALTIME {
        return EINVAL;
    }
    let ticks = crate::arch::x86_64::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
    // 100 Hz ⇒ 10 ms/tick ⇒ 10_000_000 ns/tick. Saturating: monotonic never wraps.
    let monotonic = ticks.saturating_mul(10_000_000);
    #[cfg(feature = "rtc-epoch")]
    if clock_id == CLOCK_REALTIME {
        return monotonic.saturating_add(crate::arch::x86_64::rtc::realtime_base_ns()) as i64;
    }
    monotonic as i64
}

// ── OT-61 C1.1 host-runtime primitives (futex + threads) ──────────────────

/// User-visible thread-info record for `SYS_THREAD_GET_INFO` (v1, 40 bytes).
/// Downward-stack semantics: `usable_low..usable_low+usable_len` is the
/// mapped stack; the guard page (when present) sits immediately below at
/// `guard_low..guard_low+guard_len`.
#[cfg(feature = "browser-runtime")]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ThreadInfoV1 {
    pub tid: u64,
    pub usable_low: u64,
    pub usable_len: u64,
    pub guard_low: u64,
    pub guard_len: u64,
}

/// The `SYS_THREAD_CREATE` cap gate: the caller must hold a `BrowserRuntime`
/// handle with USE meta + THREAD_SPAWN semantic (the `NetDial` scan shape —
/// the frozen thread ABI carries no cap-handle argument). Fail-closed
/// `EPERM`; there is no self-mint path.
#[cfg(feature = "browser-runtime")]
fn browser_runtime_thread_cap_check() -> Result<(), i64> {
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return Err(EPERM),
    };
    let authorized = crate::arch::x86_64::without_interrupts(|| {
        proc.handles.lock().has_browser_runtime_thread_authority()
    });
    if authorized { Ok(()) } else { Err(EPERM) }
}

/// SYS_FUTEX_WAIT(uaddr, expected, deadline_monotonic_ns) → 0 | -ETIMEDOUT |
/// -EAGAIN. The deadline is an ABSOLUTE monotonic instant (the C1.0 clock's
/// timebase); `u64::MAX` = infinite, `0` = already-expired. The value
/// re-check happens under the futex queue guard (no lost wakeup).
#[cfg(feature = "browser-runtime")]
fn sys_futex_wait_impl(uaddr: u64, expected: u32, deadline_ns: u64) -> i64 {
    if !uaddr.is_multiple_of(4) {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(uaddr, 4) {
        return e;
    }
    // Translate to the physical queue key and DROP the Arc<Pmap> BEFORE the
    // potential block: a thread force-zombified while parked in the futex has
    // its stack freed raw (no Drop), so an owned Arc on the stack would leak.
    let paddr = {
        let pmap = match pmap_for_caller() {
            Ok(p) => p,
            Err(e) => return e,
        };
        let paddr = match pmap.translate(x86_64::VirtAddr::new(uaddr)) {
            Some(pa) => pa.as_u64(),
            None => return EFAULT,
        };
        // `user_copy_fault` test only: the word's page goes away after the
        // translation, as a sibling's unmap on another CPU could do, so the
        // value re-check under the queue guard faults (CP-99) and must come
        // back EFAULT instead of halting.
        #[cfg(feature = "neo-test-user-copy-fault")]
        if expected == USER_COPY_FAULT_FUTEX_UNMAP {
            let page = x86_64::VirtAddr::new(uaddr & !0xFFF);
            if pmap.unmap_user_range(page, 0x1000).is_err() {
                return EINVAL;
            }
        }
        paddr
        // pmap Arc dropped here.
    };
    crate::sched::futex::wait(paddr, uaddr, expected, deadline_ns)
}

/// `user_copy_fault` test only: a `SYS_FUTEX_WAIT` whose `expected` is this
/// unmaps the word's page between its translation and its value re-check.
#[cfg(feature = "neo-test-user-copy-fault")]
const USER_COPY_FAULT_FUTEX_UNMAP: u32 = 0xEFA0_17ED;

/// SYS_FUTEX_WAKE(uaddr, count) → number of threads woken (PRESERVED — the
/// rwlock's writer→reader fallback consumes it).
#[cfg(feature = "browser-runtime")]
fn sys_futex_wake_impl(uaddr: u64, count: u32) -> i64 {
    if !uaddr.is_multiple_of(4) {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(uaddr, 4) {
        return e;
    }
    let paddr = {
        let pmap = match pmap_for_caller() {
            Ok(p) => p,
            Err(e) => return e,
        };
        match pmap.translate(x86_64::VirtAddr::new(uaddr)) {
            Some(pa) => pa.as_u64(),
            None => return EFAULT,
        }
    };
    crate::sched::futex::wake(paddr, count)
}

/// SYS_FUTEX_QUEUE_LEN(uaddr) → queue length for key (test-only).
/// Translates uaddr to a physical key and returns the number of waiters
/// parked on that key. Used by threads_futex barrier to confirm all
/// waiters are enqueued before the waker starts.
#[cfg(all(feature = "browser-runtime", feature = "neo-test"))]
fn sys_futex_queue_len_impl(uaddr: u64) -> i64 {
    if !uaddr.is_multiple_of(4) {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(uaddr, 4) {
        return e;
    }
    let paddr = {
        let pmap = match pmap_for_caller() {
            Ok(p) => p,
            Err(e) => return e,
        };
        match pmap.translate(x86_64::VirtAddr::new(uaddr)) {
            Some(pa) => pa.as_u64(),
            None => return EFAULT,
        }
    };
    crate::sched::futex::queue_len_for_key(paddr) as i64
}

/// SYS_TEST_FLOAT_PROBE(out, len) → 0 | errno. CP-41 test-only: copies
/// `test_harness::syscall_float_probe()` (inherited MXCSR/FCW + result bits)
/// to `out`, which must be exactly `SYSCALL_FLOAT_PROBE_WORDS * 4` bytes.
#[cfg(feature = "neo-test-syscall-mxcsr-entry")]
fn sys_test_float_probe_impl(out: *mut u8, len: usize) -> i64 {
    use crate::test_harness::{SYSCALL_FLOAT_PROBE_WORDS, syscall_float_probe};
    let bytes = SYSCALL_FLOAT_PROBE_WORDS * 4;
    if out.is_null() || len != bytes {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, bytes) {
        return e;
    }
    let words = syscall_float_probe();
    match user_copy(out, words.as_ptr().cast::<u8>(), bytes) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// SYS_THREAD_CREATE(entry, arg, stack_size, guard) → tid | errno.
/// Cap-gated (`BrowserRuntime`/THREAD_SPAWN). The kernel allocates the
/// thread's user stack (R+W+NX, optional guard page below) from the caller's
/// `Pmap`; `arg` arrives in the entry's first SysV argument register.
#[cfg(feature = "browser-runtime")]
fn sys_thread_create_impl(entry: u64, arg: u64, stack_size: usize, guard: bool) -> i64 {
    if let Err(e) = browser_runtime_thread_cap_check() {
        return e;
    }
    if entry == 0 || entry >= USER_HALF_LIMIT {
        return EFAULT;
    }
    if stack_size == 0 || stack_size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    crate::sched::process::thread_create_for_current(entry, arg, stack_size, guard)
}

/// SYS_TEST_TLS(op, out); see [`SYS_TEST_TLS`].
#[cfg(feature = "neo-test-threads-tls")]
fn sys_test_tls_impl(op: u64, out: u64) -> i64 {
    match op {
        1 => return crate::sched::scheduler::test_tls_park_current(),
        2 => return i64::from(crate::sched::scheduler::test_tls_unpark_onto_current_cpu()),
        // C1.2 review: block for a tick and come back. On the BSP (the timer
        // CPU) the thread switches out to the idle thread and back in on the
        // same CPU, with no other user thread in between: the switch-in a
        // per-CPU FS.base cache would skip.
        3 => {
            crate::sched::tick_sleep::sleep_ticks(2);
            return 0;
        }
        0 => {}
        _ => return EINVAL,
    }
    if let Err(e) = validate_user_buf(out, 24) {
        return e;
    }
    let (cpu, fs_base, msr) = crate::arch::x86_64::without_interrupts(|| {
        let cpu = u64::from(crate::arch::x86_64::per_cpu::current_cpu_id());
        let (fs_base, msr) = crate::sched::scheduler::current_fs_base_and_msr();
        (cpu, fs_base, msr)
    });
    match write_user(out as *mut [u64; 3], [cpu, fs_base, msr]) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// SYS_TEST_KERNEL_SPIN(ms) → 0. See [`SYS_TEST_KERNEL_SPIN`].
#[cfg(feature = "neo-test-threads-exit-group")]
fn sys_test_kernel_spin_impl(ms: u64) -> i64 {
    use crate::arch::x86_64::timer::TICKS;
    let ticks = ms.min(2_000).div_ceil(10);
    // Syscalls run with IF=0; spin with interrupts on so the IPI (and TLB
    // shootdowns, CP-64) are taken here, in ring 0.
    let was_enabled = crate::arch::x86_64::interrupts_enabled();
    x86_64::instructions::interrupts::enable();
    let start = TICKS.load(Ordering::Relaxed);
    while TICKS.load(Ordering::Relaxed).saturating_sub(start) < ticks {
        core::hint::spin_loop();
    }
    if !was_enabled {
        x86_64::instructions::interrupts::disable();
    }
    0
}

/// SYS_TEST_THREAD_CREATE_ON(entry, arg, stack_size, cpu) → tid | errno.
#[cfg(any(
    feature = "neo-test-threads-tls",
    feature = "neo-test-threads-exit-group"
))]
fn sys_test_thread_create_on_impl(entry: u64, arg: u64, stack_size: usize, cpu: u64) -> i64 {
    if let Err(e) = browser_runtime_thread_cap_check() {
        return e;
    }
    if entry == 0 || entry >= USER_HALF_LIMIT {
        return EFAULT;
    }
    if stack_size == 0 || stack_size > MAX_MAP_REGION_SIZE {
        return EINVAL;
    }
    if cpu > u64::from(u32::MAX) || !crate::sched::scheduler::cpu_scheduler_ready(cpu as u32) {
        return EINVAL;
    }
    crate::sched::process::thread_create_for_current_on_cpu(
        entry, arg, stack_size, true, cpu as u32,
    )
}

/// SYS_PROCESS_SELF() → the caller's pid. See [`SYS_PROCESS_SELF`].
#[cfg(any(feature = "browser-runtime", feature = "cap-rooted-open"))]
fn sys_process_self_impl() -> i64 {
    match current_process_for_syscall() {
        Ok(proc) => i64::from(proc.pid.0),
        Err(e) => e,
    }
}

/// SYS_HANDLE_INFO(idx, out, len) → 0 | errno. `len` must be the record length
/// (`EINVAL`); an empty or out-of-range `idx` is `EBADF`. See
/// [`SYS_HANDLE_INFO`].
#[cfg(any(
    feature = "browser-runtime",
    feature = "cap-rooted-open",
    feature = "installer"
))]
fn sys_handle_info_impl(idx: u64, out: u64, len: usize) -> i64 {
    use crate::sched::handle_info_logic::{RECORD_LEN, encode};
    if len != RECORD_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out, RECORD_LEN) {
        return e;
    }
    let Ok(idx) = u32::try_from(idx) else {
        return EBADF;
    };
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let Some(handle) = proc.handles.lock().get(idx) else {
        return EBADF;
    };
    let record = encode(handle.handle_info());
    drop(handle);
    match user_copy(out as *mut u8, record.as_ptr(), RECORD_LEN) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// OT-61 C2.2: SYS_MEM_INFO(out, 32) → 0 | errno. The caller's memory
/// account: used and limit frames, whether it has its own budget, and the
/// frames in use system-wide.
#[cfg(feature = "mem-quota")]
fn sys_mem_info_impl(out: u64, len: usize) -> i64 {
    use crate::mm::mem_account_logic::{
        MEM_INFO_FLAG_OWN_BUDGET, MEM_INFO_V1_LEN, UNLIMITED, encode_mem_info,
    };
    #[cfg(feature = "obj-quota")]
    if len == crate::mm::mem_account_logic::MEM_INFO_V2_LEN {
        return sys_mem_info_v2_impl(out);
    }
    if len != MEM_INFO_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out, MEM_INFO_V1_LEN) {
        return e;
    }
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let account = proc
        .pmap
        .as_ref()
        .and_then(|p| p.account().cloned())
        .unwrap_or_else(process::root_mem_account);
    let flags = if account.limit() != UNLIMITED {
        MEM_INFO_FLAG_OWN_BUDGET
    } else {
        0
    };
    let record = encode_mem_info(
        flags,
        account.used(),
        account.limit(),
        crate::mm::frames_in_use(),
    );
    match user_copy(out as *mut u8, record.as_ptr(), MEM_INFO_V1_LEN) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// OT-61 C2.2b: `SYS_MEM_INFO` with a 48-byte buffer: the v2 record, which
/// adds the account's object counter (`mm::mem_account_logic`).
#[cfg(feature = "obj-quota")]
fn sys_mem_info_v2_impl(out: u64) -> i64 {
    use crate::mm::mem_account_logic::{
        MEM_INFO_FLAG_OWN_BUDGET, MEM_INFO_V2_LEN, UNLIMITED, encode_mem_info_v2,
    };
    if let Err(e) = validate_user_buf(out, MEM_INFO_V2_LEN) {
        return e;
    }
    let proc = match current_process_for_syscall() {
        Ok(proc) => proc,
        Err(e) => return e,
    };
    let account = proc
        .pmap
        .as_ref()
        .and_then(|p| p.account().cloned())
        .unwrap_or_else(process::root_mem_account);
    let flags = if account.limit() != UNLIMITED || account.obj_limit() != UNLIMITED {
        MEM_INFO_FLAG_OWN_BUDGET
    } else {
        0
    };
    let record = encode_mem_info_v2(
        flags,
        account.used(),
        account.limit(),
        crate::mm::frames_in_use(),
        account.obj_used(),
        account.obj_limit(),
    );
    match user_copy(out as *mut u8, record.as_ptr(), MEM_INFO_V2_LEN) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// SYS_THREAD_SET_TLS(base) → 0 | errno. Sets the calling thread's FS.base to
/// `base`, which the thread uses to locate its own TCB/key block. Requires the
/// C1 thread capability (`-EPERM`). `base` is 0 (clear) or a user-half address
/// below [`USER_HALF_LIMIT`] (`-EFAULT` otherwise, so the MSR only ever takes a
/// canonical value). It need not be mapped: the kernel never dereferences it,
/// and ring 3 faults on its own if it points nowhere, as with `arch_prctl`.
#[cfg(feature = "browser-runtime")]
fn sys_thread_set_tls_impl(base: u64) -> i64 {
    if let Err(e) = browser_runtime_thread_cap_check() {
        return e;
    }
    if base >= USER_HALF_LIMIT {
        return EFAULT;
    }
    crate::sched::scheduler::set_current_fs_base(base);
    0
}

/// SYS_THREAD_EXIT(code) → never returns. Wakes a parked joiner with the
/// exit status; the process is torn down only when its LAST thread reaps.
#[cfg(feature = "browser-runtime")]
fn sys_thread_exit_impl(code: i32) -> i64 {
    crate::sched::process::thread_exit_current(code)
}

/// SYS_THREAD_JOIN(tid, *mut exit_code) → 0 | errno. Single-joiner; blocks
/// until the target thread exits. A null out-pointer discards the status.
#[cfg(feature = "browser-runtime")]
fn sys_thread_join_impl(tid: u64, exit_out: *mut i32) -> i64 {
    use crate::sched::process::WAIT_EXITED_TAG;

    if !exit_out.is_null()
        && let Err(e) = validate_user_buf(exit_out as u64, core::mem::size_of::<i32>())
    {
        return e;
    }
    let ret = crate::sched::process::thread_join_for_current(tid);
    if ret < 0 {
        return ret;
    }
    if (ret & WAIT_EXITED_TAG) == 0 {
        // A woken join always carries the tag; anything else is a kernel
        // logic error surfaced as an errno, never a bogus success.
        return EINVAL;
    }
    let code = (ret & 0xFFFF_FFFF) as u32 as i32;
    if !exit_out.is_null() {
        if let Err(e) = write_user(exit_out, code) {
            return e;
        }
    }
    0
}

/// SYS_THREAD_DETACH(tid) → 0 | errno.
#[cfg(feature = "browser-runtime")]
fn sys_thread_detach_impl(tid: u64) -> i64 {
    crate::sched::process::thread_detach_for_current(tid)
}

/// SYS_SCHED_YIELD() → 0.
#[cfg(any(
    feature = "browser-runtime",
    feature = "atrium-pointer-forward",
    feature = "desktop-session"
))]
fn sys_sched_yield_impl() -> i64 {
    crate::sched::yield_now();
    0
}

/// SYS_THREAD_SELF() → tid.
#[cfg(feature = "browser-runtime")]
fn sys_thread_self_impl() -> i64 {
    crate::arch::x86_64::per_cpu::current_thread_id() as i64
}

/// SYS_THREAD_SET_NAME(ptr, len) → 0 | errno. Truncated to the registry cap.
#[cfg(feature = "browser-runtime")]
fn sys_thread_set_name_impl(ptr: *const u8, len: usize) -> i64 {
    if len > crate::sched::process::THREAD_NAME_CAP {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(ptr as u64, len) {
        return e;
    }
    let bytes = match copy_user_bytes(ptr, len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    crate::sched::process::thread_set_name_for_current(&bytes)
}

/// SYS_THREAD_GET_INFO(tid, out, out_len) → 0 | errno. `tid == 0` = self;
/// any same-process tid is queryable (valid through exit/reap). The P2
/// amendment: SpiderMonkey's stack-limit path (`JS_SetNativeStackQuota` ←
/// `pthread_getattr_np`/`pthread_attr_getstack`) binds here.
#[cfg(feature = "browser-runtime")]
fn sys_thread_get_info_impl(tid: u64, out: *mut ThreadInfoV1, out_len: usize) -> i64 {
    let info_len = core::mem::size_of::<ThreadInfoV1>();
    if out.is_null() || out_len != info_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, info_len) {
        return e;
    }
    let (tid, bounds) = match crate::sched::process::thread_get_info_for_current(tid) {
        Ok(pair) => pair,
        Err(e) => return e,
    };
    let info = ThreadInfoV1 {
        tid,
        usable_low: bounds.usable_low,
        usable_len: bounds.usable_len,
        guard_low: bounds.guard_low,
        guard_len: bounds.guard_len,
    };
    match write_user(out, info) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// SYS_KILL — terminate the process named by `Handle::Process { target }`
/// at handle index `h` in the calling process's table. Per ADR-007:
///
///   - Returns `-EBADF` if `h` is unallocated or refers to a non-Process
///     variant (e.g. a channel handle — caller meant `sys_send`/`sys_recv`).
///   - Returns `-EINVAL` if the resolved target is the calling process
///     (self-kill is illegal — Phase 4 capability-passing makes this
///     reachable; the explicit check is load-bearing per ADR-007 §
///     Implementation note 1).
///   - Returns `-EINVAL` if the target's thread is not in the scheduler
///     ready queue (Phase 1.5b limitation per ADR-007 §Q3 — Blocked-in-
///     IPC threads need Phase 2's blocked-thread walk to lift).
///   - Returns `0` on success. The target Pmap drops, frames reclaim,
///     channel peers wake with `-EPIPE`.
fn sys_kill_impl(h: u32) -> i64 {
    let target_pid = match process_handle_lookup(h, process_rights::KILL) {
        Ok(pid) => pid,
        Err(e) => return e,
    };
    // Self-kill guard — ADR-007 §Implementation note 1. Explicit check
    // because Phase 4 capability-passing IPC could deliver a process its
    // own ProcessHandle. "This can't happen" is not a kernel safety
    // argument; the check is.
    if target_pid == crate::sched::current_pid() {
        return EINVAL;
    }
    crate::arch::x86_64::without_interrupts(|| crate::sched::process::terminate(target_pid))
}

/// `exit_codes_bounded`'s step 5: a wait with this flag has its handle
/// revoked after the lookup authorized it and before it reads the code, the
/// window of #171 F1 (another CPU's revoke), placed deterministically.
#[cfg(feature = "neo-test-exit-codes-bounded")]
const WAIT_FLAG_TEST_REVOKE_AFTER_LOOKUP: u64 = 1 << 62;

fn sys_wait_impl(h: u32, flags: u64) -> i64 {
    #[cfg(feature = "neo-test-exit-codes-bounded")]
    let revoke_after_lookup = flags & WAIT_FLAG_TEST_REVOKE_AFTER_LOOKUP != 0;
    #[cfg(feature = "neo-test-exit-codes-bounded")]
    let flags = flags & !WAIT_FLAG_TEST_REVOKE_AFTER_LOOKUP;
    // The claim covers the window to the wait's read of a kept code (#171
    // F1); the wait releases it before it can park.
    let (target_pid, claim) = match process_handle_lookup_pinned(h, process_rights::WAIT, true) {
        Ok(found) => found,
        Err(e) => return e,
    };
    if target_pid == crate::sched::current_pid() {
        return EINVAL;
    }
    #[cfg(feature = "neo-test-exit-codes-bounded")]
    if revoke_after_lookup
        && crate::sched::process::revoke_handle(crate::sched::current_pid(), h).is_err()
    {
        return EINVAL;
    }
    crate::sched::process::wait_for_exit_abi(target_pid, flags, claim)
}

fn sys_inspect_impl(h: u32, out: *mut ProcessSnapshotV1, out_len: usize) -> i64 {
    let snapshot_len = core::mem::size_of::<ProcessSnapshotV1>();
    if out.is_null() {
        return EINVAL;
    }
    if out_len != snapshot_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, snapshot_len) {
        return e;
    }
    let (target_pid, _claim) = match process_handle_lookup_pinned(h, process_rights::INSPECT, true)
    {
        Ok(found) => found,
        Err(e) => return e,
    };
    let snapshot = match process::inspect_process(target_pid) {
        Ok(snapshot) => snapshot,
        Err(e) => return e,
    };

    let mut name = [0u8; PROCESS_SNAPSHOT_NAME_LEN];
    let name_bytes = snapshot.name.as_bytes();
    let name_len = min(name_bytes.len(), PROCESS_SNAPSHOT_NAME_LEN);
    name[..name_len].copy_from_slice(&name_bytes[..name_len]);
    match copy_process_snapshot_to_user(
        out,
        ProcessSnapshotV1 {
            version: PROCESS_SNAPSHOT_V1_VERSION,
            state: snapshot.state,
            pid: snapshot.pid.0,
            exit_code: snapshot.exit_code,
            thread_count: snapshot.thread_count,
            handle_count: snapshot.handle_count,
            channel_handle_count: snapshot.channel_handle_count,
            network_handle_count: snapshot.network_handle_count,
            name_len: name_len as u32,
            name,
        },
    ) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

#[cfg(feature = "kernel-sysstat")]
fn sys_sysstat_impl(out: *mut SysStatV1, out_len: usize) -> i64 {
    let stats_len = core::mem::size_of::<SysStatV1>();
    if out.is_null() {
        return EINVAL;
    }
    if out_len != stats_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, stats_len) {
        return e;
    }

    let frames_used = crate::mm::frames_in_use();
    let cores_online = min(super::smp::reported_cpu_count(), u32::MAX as usize) as u32;
    let uptime_ticks = super::timer::TICKS.load(Ordering::Relaxed);
    match copy_sysstat_to_user(
        out,
        SysStatV1 {
            version: SYSSTAT_V1_VERSION,
            _pad0: 0,
            process_count: process::user_process_count(),
            frames_used,
            memory_used_bytes: frames_used.saturating_mul(4096),
            cores_online,
            _pad1: 0,
            uptime_ticks,
        },
    ) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

#[cfg(not(feature = "kernel-sysstat"))]
fn sys_sysstat_impl(_out: *mut SysStatV1, _out_len: usize) -> i64 {
    ENOSYS
}

#[cfg(feature = "kernel-observability")]
fn sys_observe_impl(out: *mut KernelObservabilityV1, out_len: usize) -> i64 {
    let stats_len = core::mem::size_of::<KernelObservabilityV1>();
    if out.is_null() {
        return EINVAL;
    }
    if out_len != stats_len {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, stats_len) {
        return e;
    }

    if let Err(e) = copy_observability_to_user(out, crate::observability::snapshot()) {
        return e;
    }
    0
}

#[cfg(not(feature = "kernel-observability"))]
fn sys_observe_impl(_out: *mut KernelObservabilityV1, _out_len: usize) -> i64 {
    ENOSYS
}

#[cfg(feature = "neo-test-q8c-blocked-kill-foreign-parked")]
pub fn debug_sys_kill_for_test(h: u32) -> i64 {
    sys_kill_impl(h)
}

fn sys_derive_impl(h: u32, new_meta: u8, new_semantic: u8) -> i64 {
    match process::derive_handle(
        crate::sched::current_pid(),
        h,
        CapRights::new(new_meta, new_semantic),
    ) {
        Ok(idx) => idx as i64,
        Err(e) => e,
    }
}

fn sys_revoke_impl(h: u32) -> i64 {
    match process::revoke_handle(crate::sched::current_pid(), h) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

fn sys_grant_impl(channel_h: u32, src_h: u32, rights_subset: u16) -> i64 {
    let (channel, side) = match channel_lookup(channel_h, channel_rights::SEND) {
        Ok(v) => v,
        Err(e) => return e,
    };

    let donor_pid = crate::sched::current_pid();
    let source = match crate::arch::x86_64::without_interrupts(|| {
        let donor = process::get(donor_pid).ok_or(EINVAL)?;
        donor.handles.lock().get(src_h).ok_or(EBADF)
    }) {
        Ok(h) => h,
        Err(EBADF) => {
            channel.fail_recv_grant(side);
            return EBADF;
        }
        Err(e) => return e,
    };
    if !source.rights().has_meta(meta_rights::GRANT) {
        channel.fail_recv_grant(side);
        return EPERM;
    }
    // OT-87 R4.4: the installer authority moves only at spawn, to the shell;
    // never over a channel.
    #[cfg(feature = "installer")]
    if matches!(source, Handle::Installer { .. }) {
        channel.fail_recv_grant(side);
        return EPERM;
    }
    // OT-66 L1: nor does the system store, ever.
    #[cfg(feature = "system-store")]
    if matches!(source, Handle::SystemStore { .. }) {
        channel.fail_recv_grant(side);
        return EPERM;
    }
    // Dexter #187 item 3a: nor the mount authority.
    #[cfg(feature = "mount-authority")]
    if matches!(source, Handle::MountAuthority { .. }) {
        channel.fail_recv_grant(side);
        return EPERM;
    }

    let payload = crate::ipc::GrantPayload {
        grantor_pid: donor_pid,
        source_cap_id: source.cap_id(),
        offered: unpack_rights(rights_subset),
    };
    // CP-67: `source` is an owned clone of the handle (a model handle carries
    // an `Arc<Model>`). Nothing past here needs it, and a grantor killed while
    // parked would never drop it.
    #[cfg(not(feature = "neo-test-ipc-park-kill-control"))]
    drop(source);
    // The channel lock is IRQ-safe by type, but the IPC handoff path also
    // touches scheduler queues after dropping that lock. CP-67: the channel is
    // owned by the thread, not this frame, while it may park.
    crate::sched::with_channel_anchor(channel, |channel| {
        crate::arch::x86_64::without_interrupts(|| channel.grant(side, payload))
    })
}

fn sys_recv_grant_impl(channel_h: u32, max_rights: u16) -> i64 {
    let (channel, side) = match channel_lookup(channel_h, channel_rights::RECV) {
        Ok(v) => v,
        Err(e) => return e,
    };
    let recipient_pid = crate::sched::current_pid();
    // The channel lock is IRQ-safe by type, but the IPC handoff path also
    // touches scheduler queues after dropping that lock. CP-67: the channel is
    // owned by the thread, not this frame, while it may park.
    crate::sched::with_channel_anchor(channel, |channel| {
        crate::arch::x86_64::without_interrupts(|| {
            channel.recv_grant(side, recipient_pid, unpack_rights(max_rights))
        })
    })
}

#[cfg(feature = "neo-test-sys-getrandom")]
pub fn debug_sys_getrandom_for_test(buf: *mut u8, len: usize) -> i64 {
    sys_getrandom_impl(buf, len)
}

#[cfg(feature = "neo-test-neo-shell-builtins")]
pub fn debug_sys_sysstat_for_test(out: *mut SysStatV1, len: usize) -> i64 {
    sys_sysstat_impl(out, len)
}

#[cfg(feature = "neo-test-kernel-observability")]
pub fn debug_sys_observe_for_test(out: *mut KernelObservabilityV1, len: usize) -> i64 {
    sys_observe_impl(out, len)
}

// === Atrium G1 compositor slice-1 syscalls (Rung 0) ===
//
// Every impl below gates on `atrium::syscalls_enabled()` FIRST: with the
// runtime switch off, no object is ever minted and the caller sees ENOSYS —
// the second layer of the two-layer kill-switch (the first is the feature
// gate on the dispatch arms above). I-14 note: none of these paths touch an
// AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`) and
// none issues DMA — the compositing copy is a CPU write into the Limine
// framebuffer under the fb driver's own leaf lock.

/// Resolve a buffer handle to its object `Arc`, gating `USE` meta plus the
/// required semantic bits (EBADF wrong-variant / unallocated, EPERM rights)
/// — the tensor_handle_lookup pattern.
#[cfg(feature = "atrium-compositor-g1")]
fn buffer_handle_lookup(
    h: u32,
    need_semantic: u8,
) -> Result<alloc::sync::Arc<crate::atrium::buffer::SharedPixelBuffer>, i64> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::Buffer { buffer, rights, .. } => {
                if !rights.has_meta(process::meta_rights::USE)
                    || !rights.has_semantic(need_semantic)
                {
                    return Err(EPERM);
                }
                Ok(buffer)
            }
            _ => Err(EBADF),
        }
    })
}

/// Resolve an input-focus handle, gating `USE` meta + semantic bits.
#[cfg(feature = "atrium-compositor-g1")]
fn input_focus_handle_lookup(
    h: u32,
    need_semantic: u8,
) -> Result<alloc::sync::Arc<crate::atrium::focus::InputFocus>, i64> {
    input_focus_handle_lookup_with_cap(h, need_semantic).map(|(focus, _)| focus)
}

/// `input_focus_handle_lookup`, with the handle's `cap_id` (the mint records
/// it as the new sibling's parent).
#[cfg(feature = "atrium-compositor-g1")]
fn input_focus_handle_lookup_with_cap(
    h: u32,
    need_semantic: u8,
) -> Result<
    (
        alloc::sync::Arc<crate::atrium::focus::InputFocus>,
        process::CapId,
    ),
    i64,
> {
    crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        let handle = proc.handles.lock().get(h).ok_or(EBADF)?;
        match handle {
            Handle::InputFocus {
                focus,
                rights,
                cap_id,
            } => {
                if !rights.has_meta(process::meta_rights::USE)
                    || !rights.has_semantic(need_semantic)
                {
                    return Err(EPERM);
                }
                Ok((focus, cap_id))
            }
            _ => Err(EBADF),
        }
    })
}

/// SYS_BUFFER_CREATE — allocate a shared pixel buffer from a validated
/// `BufferDescV1` (24 bytes at `desc_ptr`). Returns the root (client)
/// handle: full buffer rights. The compositor role is minted from it via
/// the existing `SYS_DERIVE` attenuation (MAP_READ|QUERY|PRESENT, no
/// MAP_WRITE) — sibling derivations per the G1 rights model. `PRESENT` is
/// necessary object authority, but an armed scanout gate independently
/// requires the process-held `GpuSubmitCap`.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_create_impl(desc_ptr: *const u8, desc_len: usize) -> i64 {
    use crate::atrium::logic;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if desc_ptr.is_null() || desc_len != logic::BUFFER_DESC_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(desc_ptr as u64, desc_len) {
        return e;
    }
    let desc_bytes = match copy_user_bytes(desc_ptr, desc_len) {
        Ok(bytes) => bytes,
        Err(e) => return e,
    };
    let mut raw = [0u8; logic::BUFFER_DESC_V1_LEN];
    raw.copy_from_slice(&desc_bytes);
    let desc = logic::decode_desc(&raw);

    let buffer = match crate::atrium::buffer::SharedPixelBuffer::create(desc) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(p) => p,
        None => return EINVAL,
    };
    match process::install_cap(
        &proc,
        Handle::Buffer {
            buffer,
            cap_id: process::alloc_cap_id(),
            rights: process::full_buffer_rights(),
        },
        None,
    ) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

/// SYS_BUFFER_MAP — map the buffer's frames into the caller. `role` 0 =
/// client (RW+NX, needs MAP_WRITE); `role` 1 = compositor (hardware RO+NX,
/// needs MAP_READ). Both are borrowed leaves: the Pmap never frees them,
/// and the mapping pins the buffer `Arc` until the PTEs are gone.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_map_impl(h: u32, role: u32) -> i64 {
    use crate::atrium::logic::buffer_rights;
    use crate::mm::pmap::flags as mmflags;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let (need, flags) = match role {
        0 => (buffer_rights::MAP_WRITE, mmflags::USER_DATA_RW),
        1 => (buffer_rights::MAP_READ, mmflags::USER_DATA_RO),
        _ => return EINVAL,
    };
    let buffer = match buffer_handle_lookup(h, need) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    let pin: alloc::sync::Arc<dyn core::any::Any + Send + Sync> = buffer.clone();
    match pmap.map_borrowed_region(buffer.frames(), flags, pin) {
        Ok(va) => {
            crate::arch::x86_64::flush_local_tlb();
            // Scenario instrumentation: the compositor-role mapping is the
            // RO/NX write-fault probe target (ISC-5/6 discriminator).
            #[cfg(feature = "neo-test-atrium-slice1")]
            if role == 1 {
                crate::test_harness::atrium_arm_ro_write_fault(va);
            }
            // Slice-3: the compositor process's RO+NX import of the CLIENT's
            // buffer is the C3 write-fault target. Its final write faults;
            // the handler runs the cross-process composition pixel asserts.
            #[cfg(feature = "neo-test-atrium-slice3")]
            if role == 1 {
                crate::test_harness::atrium_slice3_arm_ro_write_fault(va);
            }
            va as i64
        }
        Err(_) => ENOMEM,
    }
}

/// SYS_BUFFER_UNMAP — remove the caller's borrowed buffer mapping at `va`
/// (as `SYS_BUFFER_MAP` returned it), releasing the pin and its charge
/// (Chad #177 item 4). Needs no handle: the mapping is the caller's own, and
/// removing access grants nothing.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_unmap_impl(va: u64) -> i64 {
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    // The control (`neo-test-desktop-home-cycle-control`) keeps the pin, as
    // before there was an unmap.
    if cfg!(feature = "neo-test-desktop-home-cycle-control") {
        return 0;
    }
    let pmap = match pmap_for_caller() {
        Ok(p) => p,
        Err(e) => return e,
    };
    match pmap.unmap_borrowed_region(va) {
        Ok(()) => 0,
        Err(_) => EINVAL,
    }
}

/// SYS_BUFFER_QUERY — copy the immutable descriptor (24-byte wire form)
/// to the caller. QUERY right.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_query_impl(h: u32, out: *mut u8, out_len: usize) -> i64 {
    use crate::atrium::logic;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if out.is_null() || out_len != logic::BUFFER_DESC_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, out_len) {
        return e;
    }
    let buffer = match buffer_handle_lookup(h, logic::buffer_rights::QUERY) {
        Ok(b) => b,
        Err(e) => return e,
    };
    let bytes = logic::encode_desc(&buffer.desc());
    match copy_kernel_bytes_to_user(out, &bytes) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

/// SYS_BUFFER_COMMIT — mark the client's pixel writes ready for the next
/// present (slice-1 full-repaint damage gate). COMMIT right.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_commit_impl(h: u32) -> i64 {
    use crate::atrium::logic::buffer_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    match buffer_handle_lookup(h, buffer_rights::COMMIT) {
        Ok(b) => {
            b.commit();
            0
        }
        Err(e) => e,
    }
}

/// SYS_BUFFER_COMMIT_DAMAGE — publish one bounded buffer-coordinate damage
/// rectangle with the same COMMIT authority as the legacy full repaint.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_buffer_commit_damage_impl(h: u32, rect: *const u8, rect_len: usize) -> i64 {
    use crate::atrium::logic::{self, buffer_rights};
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if rect.is_null() || rect_len != logic::DAMAGE_RECT_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(rect as u64, rect_len) {
        return e;
    }
    let mut bytes = [0u8; logic::DAMAGE_RECT_V1_LEN];
    if let Err(e) = user_copy(bytes.as_mut_ptr(), rect, bytes.len()) {
        return e;
    }
    let Some(damage) = logic::decode_damage(&bytes) else {
        return EINVAL;
    };
    match buffer_handle_lookup(h, buffer_rights::COMMIT) {
        Ok(buffer) => buffer.commit_damage(damage).map_or_else(|e| e, |()| 0),
        Err(e) => e,
    }
}

/// SYS_FB_PRESENT — composite the buffer to the framebuffer at
/// `(dst_x, dst_y)` through the fb driver's single-lock present batch and
/// complete its fence (Release-published AFTER the blit — I-16). PRESENT
/// object right plus, when scanout enforcement is armed, a process-held
/// `GpuSubmitCap`. Returns the present epoch (>= 1) for
/// SYS_PRESENT_FENCE_WAIT.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_fb_present_impl(h: u32, dst_x: usize, dst_y: usize) -> i64 {
    use crate::atrium::logic::buffer_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let buffer = match buffer_handle_lookup(h, buffer_rights::PRESENT) {
        Ok(b) => b,
        Err(e) => return e,
    };
    // Scarce scanout authority. Snapshot it under the handle-table lock, then
    // drop the lock before checking or presenting. This syscall-level check is
    // backend-independent: Software blits and HardwareFlip submits both reach
    // raw scanout. SharedPixelBuffer::present repeats the check for kernel
    // callers that bypass the syscall entry.
    let caller_holds_gpu_submit = match process::get(crate::sched::current_pid()) {
        Some(p) => p.handles.lock().holds_gpu_submit(),
        None => return EINVAL,
    };
    if !crate::atrium::present_backend::scanout_authorized(caller_holds_gpu_submit) {
        return EPERM;
    }
    match buffer.present(dst_x, dst_y, caller_holds_gpu_submit) {
        Ok(epoch) => epoch as i64,
        Err(e) => e,
    }
}

/// SYS_FB_OUTPUT_QUERY — the scanout output size, packed by
/// [`crate::atrium::logic::pack_output_size`]. Only a process holding the
/// scarce `GpuSubmitCap` (the session compositor) may learn the output
/// geometry: clients are laid out by the WM and never learn screen facts
/// (desktop-UX synthesis point 8). Read-only; touches no scanout state.
#[cfg(feature = "desktop-session")]
fn sys_fb_output_query_impl() -> i64 {
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let holds_gpu_submit = match process::get(crate::sched::current_pid()) {
        Some(p) => p.handles.lock().holds_gpu_submit(),
        None => return EINVAL,
    };
    if !holds_gpu_submit {
        return EPERM;
    }
    let (w, h) = crate::drivers::fb::dimensions();
    crate::atrium::logic::pack_output_size(w, h).unwrap_or(EINVAL)
}

/// GPU P1 Slice 6 — post-Slice-5 userland compute service.  The three input
/// handles are a READ buffer range, a WRITE buffer range, and the one
/// quota-bound manifest queue.  The caller cannot name a GPU address, kernel,
/// geometry, tint, or present target; all are fixed by the trusted G1 service.
///
/// On success the return value is a freshly installed `GpuCompute::Fence`
/// handle.  It is an explicit completion capability only — never a present
/// epoch, never a scanout authority.  A caller must separately pass the
/// existing `GpuSubmitCap` gate in `SYS_FB_PRESENT` to display any resulting
/// G1 surface.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_gpu_compute_submit_blur_impl(src_h: u32, dst_h: u32, queue_h: u32) -> i64 {
    use crate::atrium::blur::{self, BlurDims};
    use crate::drivers::accel::{self, gpu_compute};
    use crate::sched::process::{GpuComputeCap, gpu_compute_rights};

    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let Some(proc) = process::get(crate::sched::current_pid()) else {
        return EINVAL;
    };

    // Snapshot typed capabilities under the table lock, then drop it before
    // CPU/GPU work.  No governance lock is acquired; `gpu_compute` supplies
    // the CP-18 leaf serialization and bounded I-14(b) degrade.
    let (
        src,
        src_generation,
        src_offset,
        src_len,
        dst,
        dst_generation,
        dst_offset,
        dst_len,
        quota,
        queue_cap_id,
    ) = {
        let handles = proc.handles.lock();
        let Some(src_handle) = handles.get(src_h) else {
            return EBADF;
        };
        let Some(dst_handle) = handles.get(dst_h) else {
            return EBADF;
        };
        let Some(queue_handle) = handles.get(queue_h) else {
            return EBADF;
        };
        let Handle::GpuCompute {
            cap:
                GpuComputeCap::Buffer {
                    buffer: src,
                    generation: src_generation,
                    offset: src_offset,
                    len: src_len,
                },
            rights,
            ..
        } = src_handle
        else {
            return EBADF;
        };
        if !rights.has_meta(meta_rights::USE)
            || !rights.has_semantic(gpu_compute_rights::BUFFER_READ)
        {
            return EPERM;
        }
        let Handle::GpuCompute {
            cap:
                GpuComputeCap::Buffer {
                    buffer: dst,
                    generation: dst_generation,
                    offset: dst_offset,
                    len: dst_len,
                },
            rights,
            ..
        } = dst_handle
        else {
            return EBADF;
        };
        if !rights.has_meta(meta_rights::USE)
            || !rights.has_semantic(gpu_compute_rights::BUFFER_WRITE)
        {
            return EPERM;
        }
        let Handle::GpuCompute {
            cap: GpuComputeCap::Queue { byte_quota },
            cap_id,
            rights,
        } = queue_handle
        else {
            return EBADF;
        };
        if !rights.has_meta(meta_rights::USE)
            || !rights.has_semantic(gpu_compute_rights::QUEUE_SUBMIT)
        {
            return EPERM;
        }
        (
            src,
            src_generation,
            src_offset,
            src_len,
            dst,
            dst_generation,
            dst_offset,
            dst_len,
            byte_quota,
            cap_id,
        )
    };

    // The manifest has no subrange form.  Retain range metadata in the cap for
    // generation-safe future tiling, but fail closed unless this v1 whole-buffer
    // endpoint receives exactly its granted source/destination spans.
    if src_generation == 0
        || dst_generation == 0
        || src_offset != 0
        || dst_offset != 0
        || src_len != src.desc().bytes
        || dst_len != dst.desc().bytes
        || src_len > quota
        || dst_len > quota
        || alloc::sync::Arc::ptr_eq(&src, &dst)
        || src.desc() != dst.desc()
    {
        return EINVAL;
    }

    let desc = src.desc();
    let dims = BlurDims {
        width: desc.width as usize,
        height: desc.height as usize,
        stride: desc.stride as usize,
        radius: blur::BLUR_RADIUS_R8,
    };
    let src_pixels = match src.contiguous_pixels() {
        Ok(pixels) => pixels,
        Err(errno) => return errno,
    };
    let mut dst_pixels = match dst.contiguous_pixels_mut() {
        Ok(pixels) => pixels,
        Err(errno) => return errno,
    };
    let mut mid = alloc::vec![0u8; desc.bytes as usize];

    // Trusted policy chooses the only available compute substrate.  No user
    // handle or syscall argument can steer this selection.  CPU remains the
    // live floor when no GPU driver exists, when GPU is off, or when a submit
    // fails/overruns its bounded wait.
    let driver = accel::find_device_by_driver("accel_intel")
        .or_else(|| accel::find_device_by_driver("accel_sim"))
        .map(|(_, driver)| driver)
        .or_else(accel::default_driver);
    let token = if let Some(driver) = driver {
        match gpu_compute::blur_backdrop(
            &driver,
            src_pixels.as_slice(),
            &mut mid,
            dst_pixels.as_mut_slice(),
            dims,
            &blur::WEIGHTS_R8_Q16,
        ) {
            Ok(token) => token,
            Err(_) => return EINVAL,
        }
    } else {
        // Registry absence is not a user-visible GPU outage: retain the exact
        // CPU floor rather than failing a glass draw or creating ambient access.
        if blur::blur_pass_h(src_pixels.as_slice(), &mut mid, dims, &blur::WEIGHTS_R8_Q16).is_err()
            || blur::blur_pass_v(&mid, dst_pixels.as_mut_slice(), dims, &blur::WEIGHTS_R8_Q16)
                .is_err()
        {
            return EINVAL;
        }
        crate::drivers::accel::DispatchToken {
            device: crate::ai::tensor::Device::Cpu,
            sequence: 0,
        }
    };

    let fence = process::completed_gpu_compute_fence(token);
    match process::install_cap(&proc, fence, Some(queue_cap_id)) {
        Ok(handle) => handle as i64,
        Err(_) => EINVAL,
    }
}

/// Query/await a Slice-6 compute fence.  `gpu_compute::blur_backdrop` returns
/// only after its bounded wait, so v1's fence is already complete; keeping this
/// separate capability is nevertheless load-bearing — it proves completion can
/// be handed to a client without handing it queue or scanout authority.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_gpu_compute_fence_wait_impl(fence_h: u32) -> i64 {
    use crate::sched::process::{GpuComputeCap, gpu_compute_rights};

    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let Some(proc) = process::get(crate::sched::current_pid()) else {
        return EINVAL;
    };
    let handle = match proc.handles.lock().get(fence_h) {
        Some(handle) => handle,
        None => return EBADF,
    };
    let Handle::GpuCompute {
        cap: GpuComputeCap::Fence { token, generation },
        rights,
        ..
    } = handle
    else {
        return EBADF;
    };
    if generation == 0
        || !rights.has_meta(meta_rights::USE)
        || !(rights.has_semantic(gpu_compute_rights::FENCE_WAIT)
            || rights.has_semantic(gpu_compute_rights::FENCE_QUERY))
    {
        return EPERM;
    }
    // Tokens are allocated monotonically from a small device-local counter;
    // `i64` is the syscall ABI and zero is the documented CPU-floor sentinel.
    if token.sequence > i64::MAX as u64 {
        return EINVAL;
    }
    token.sequence as i64
}

/// SYS_PRESENT_FENCE_WAIT — bounded wait for present `epoch` on the
/// buffer's embedded fence; copies `PresentResultV1` out on success.
/// USE meta only — waiting is not a privilege. `timeout_ticks` bounds the
/// wait (0 = non-blocking probe, EAGAIN while pending; deadline reached,
/// ETIMEDOUT). The wait is a yield loop that explicitly re-enables interrupts
/// for its duration (SYSCALL entry masks IF; `fence_wait` wraps the loop in
/// `with_interrupts_enabled` — the CP-2 pattern), so the timer fires on this
/// CPU and the tick deadline is a live bound. It holds no lock across the
/// wait and cannot strand the CPU (CP-2/CP-3).
#[cfg(feature = "atrium-compositor-g1")]
fn sys_present_fence_wait_impl(
    h: u32,
    epoch: u64,
    out: *mut u8,
    out_len: usize,
    timeout_ticks: u64,
) -> i64 {
    use crate::atrium::buffer::PRESENT_RESULT_V1_LEN;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if out.is_null() || out_len != PRESENT_RESULT_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, out_len) {
        return e;
    }
    let buffer = match buffer_handle_lookup(h, 0) {
        Ok(b) => b,
        Err(e) => return e,
    };
    match buffer.fence_wait(epoch, timeout_ticks) {
        Ok(result) => {
            match write_user(out.cast::<crate::atrium::buffer::PresentResultV1>(), result) {
                Ok(()) => 0,
                Err(e) => e,
            }
        }
        Err(e) => e,
    }
}

/// SYS_INPUT_FOCUS_CREATE — mint a focus object; returns the root handle
/// The returned client root has RECEIVE but never ACTIVATE. A separate call
/// mints its disjoint ACTIVATE-only compositor sibling.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_create_impl() -> i64 {
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let proc = match process::get(crate::sched::current_pid()) {
        Some(p) => p,
        None => return EINVAL,
    };
    match process::install_cap(
        &proc,
        Handle::InputFocus {
            focus: alloc::sync::Arc::new(crate::atrium::focus::InputFocus::new()),
            cap_id: process::alloc_cap_id(),
            rights: process::client_input_focus_rights(),
        },
        None,
    ) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

/// Mint an ACTIVATE-only, non-DERIVE sibling for grant to the compositor.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_mint_activator_impl(h: u32) -> i64 {
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    let (focus, root_cap_id) = match input_focus_handle_lookup_with_cap(h, focus_rights::RECEIVE) {
        Ok(found) => found,
        Err(e) => return e,
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(proc) => proc,
        None => return EINVAL,
    };
    // Dexter #185 item 3 (Muse #167): the ACTIVATE sibling is recorded as a
    // child of the RECEIVE cap it was minted from, as `complete_grant` and
    // `sys_spawn` record theirs, so revoking that cap reaches it (and every
    // grant of it). With no parent it was a root of its own, and outlived
    // the revoke. `install_cap` re-checks the parent under `CAP_GRAPH`
    // (CP-87), so a revoke landing after the lookup refuses the mint.
    // The control (`neo-test-input-activate-revoke-control`) installs it
    // with no parent, as before.
    let parent = if cfg!(feature = "neo-test-input-activate-revoke-control") {
        None
    } else {
        Some(root_cap_id)
    };
    match process::install_cap(
        &proc,
        Handle::InputFocus {
            focus,
            cap_id: process::alloc_cap_id(),
            rights: process::compositor_input_focus_rights(),
        },
        parent,
    ) {
        Ok(idx) => idx as i64,
        Err(process::InstallCapError::ParentGone) => EBADF,
        Err(_) => EINVAL,
    }
}

/// Atomic route transfer through the compositor's ACTIVATE-only sibling.
/// Only the input-system holder (the WM) may move the keyboard route: any
/// process can create a focus and mint itself an ACTIVATE sibling (Arthur
/// mints one to grant to the WM), so the right on the handle alone would let
/// a client take every key (OT-66 L2 pre-merge AAG, F1).
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_activate_impl(h: u32) -> i64 {
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if let Err(e) = caller_holds_input_system() {
        return e;
    }
    match input_focus_handle_lookup(h, focus_rights::ACTIVATE) {
        Ok(f) => {
            // CP-54: the live client route set is capped.
            if f.activate() { 0 } else { EBUSY }
        }
        Err(e) => e,
    }
}

/// SYS_INPUT_FOCUS_DEACTIVATE — clear the route iff this object holds it
/// (compare-exchange: a stale root must not clobber the current holder).
/// ACTIVATE right. Returns 0 if cleared, EAGAIN if it was not the active
/// route.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_deactivate_impl(h: u32) -> i64 {
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    match input_focus_handle_lookup(h, focus_rights::ACTIVATE) {
        Ok(f) => {
            if f.deactivate() {
                0
            } else {
                EAGAIN
            }
        }
        Err(e) => e,
    }
}

/// SYS_INPUT_FOCUS_POLL — the routing gate (RECEIVE right, the client
/// leaf): 0 iff this exact object is the active route, EPERM otherwise.
/// Input security and input routing are the same comparison.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_poll_impl(h: u32) -> i64 {
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::syscalls_enabled() {
        return ENOSYS;
    }
    if cfg!(feature = "atrium-input-routing") {
        let leaf_only = crate::arch::x86_64::without_interrupts(|| {
            let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
            match proc.handles.lock().get(h) {
                Some(Handle::InputFocus { rights, .. })
                    if rights.has_semantic(focus_rights::ACTIVATE) =>
                {
                    Err(EPERM)
                }
                Some(Handle::InputFocus { .. }) => Ok(()),
                _ => Err(EBADF),
            }
        });
        if let Err(e) = leaf_only {
            return e;
        }
    }
    match input_focus_handle_lookup(h, focus_rights::RECEIVE) {
        Ok(f) => {
            if f.is_active() {
                0
            } else {
                EPERM
            }
        }
        Err(e) => e,
    }
}

/// Non-blocking RECEIVE-side dequeue. EAGAIN means the active leaf has no
/// event; EPERM means this leaf is not the active route. No park/wake edge is
/// introduced, deliberately avoiding CP-14/15/16 on the HID critical path.
#[cfg(feature = "atrium-compositor-g1")]
fn sys_input_focus_recv_impl(h: u32, out: *mut u8, out_len: usize) -> i64 {
    use crate::atrium::input_logic::{INPUT_EVENT_V1_LEN, encode_event};
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::input_routing_enabled() {
        return ENOSYS;
    }
    if out.is_null() || out_len != INPUT_EVENT_V1_LEN {
        return EINVAL;
    }
    if let Err(e) = validate_user_buf(out as u64, out_len) {
        return e;
    }
    #[cfg(all(feature = "usb-hid", feature = "atrium-input-routing"))]
    hid_keyboard::pump_routed_events();
    #[cfg(all(feature = "ps2-keyboard", feature = "atrium-input-routing"))]
    crate::drivers::ps2::i8042::pump_routed_events();
    #[cfg(all(feature = "i2c-hid-touchpad", feature = "atrium-input-routing"))]
    crate::drivers::i2c_hid::touchpad::pump_routed_events();
    #[cfg(feature = "usb-hid-mouse")]
    crate::drivers::usb::hid_mouse::pump_routed_events();
    let focus = match crate::arch::x86_64::without_interrupts(|| {
        let proc = process::get(crate::sched::current_pid()).ok_or(EINVAL)?;
        match proc.handles.lock().get(h) {
            Some(Handle::InputFocus { focus, rights, .. })
                if rights.has_meta(process::meta_rights::USE)
                    && rights.has_semantic(focus_rights::RECEIVE)
                    && !rights.has_semantic(focus_rights::ACTIVATE) =>
            {
                Ok(focus.clone())
            }
            Some(Handle::InputFocus { .. }) => Err(EPERM),
            _ => Err(EBADF),
        }
    }) {
        Ok(focus) => focus,
        Err(e) => return e,
    };
    if !focus.receivable() {
        return EPERM;
    }
    let Some(event) = focus.try_recv() else {
        return EAGAIN;
    };
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::note_key_received(&event);
    let bytes = encode_event(event);
    if let Err(e) = copy_kernel_bytes_to_user(out, &bytes) {
        return e;
    }
    0
}

/// DI-1a: the caller holds the exact, non-delegable input-system authority
/// (THINK §8.2). No enforcement toggle opens this (Jarvis F3).
/// OT-87 R4.4: the only module a spawn may hand the `Installer` cap to.
#[cfg(feature = "installer")]
fn installer_inheritable_by(name: &str) -> bool {
    name == "shell"
}

/// OT-66 L1 (Dexter #186 item 4): a root-form path under a mount's
/// `.system/` is the sign-in record's store, so it is `EPERM` unless `proc`
/// holds the `SystemStore` cap (the session root alone). Checked on the
/// normalized path (`//`, `.` and `..` folded) before anything resolves, so
/// a refusal says nothing about what exists there.
#[cfg(feature = "system-store")]
fn system_store_check(proc: &alloc::sync::Arc<process::Process>, path: &str) -> Result<(), i64> {
    if !fs::is_system_store_path(path) {
        return Ok(());
    }
    let holds =
        crate::arch::x86_64::without_interrupts(|| proc.handles.lock().holds_system_store());
    if holds { Ok(()) } else { Err(EPERM) }
}

/// Dexter #187 item 3a (J.c., option A): `EPERM` unless the caller holds the
/// `MountAuthority` cap (the session root alone). A directory handle's
/// `MOUNT` right is no longer enough to mount or unmount; checked before the
/// handle is looked up, so a refusal says nothing about it.
#[cfg(feature = "mount-authority")]
fn caller_holds_mount_authority() -> Result<(), i64> {
    let proc = current_process_for_syscall()?;
    if may_mount(&proc) { Ok(()) } else { Err(EPERM) }
}

/// Whether `proc` holds the mount authority. The revert-check
/// `neo-test-mount-authority-control` answers yes for every process, as
/// before the authority existed.
#[cfg(feature = "mount-authority")]
fn may_mount(proc: &process::Process) -> bool {
    cfg!(feature = "neo-test-mount-authority-control")
        || crate::arch::x86_64::without_interrupts(|| proc.handles.lock().holds_mount_authority())
}

/// OT-87 R4.4: `EPERM` unless the caller holds the `Installer` cap.
#[cfg(feature = "installer")]
fn caller_holds_installer() -> Result<(), i64> {
    let holds = crate::arch::x86_64::without_interrupts(|| {
        process::get(crate::sched::current_pid()).map(|p| p.handles.lock().holds_installer())
    });
    if holds == Some(true) {
        Ok(())
    } else {
        Err(EPERM)
    }
}

/// OT-87 R4.4: `SYS_INSTALL_LIST(buf, len) -> count`. Writes as many
/// `install_ui_logic::ROW_BYTES` records as fit in `len` and returns the
/// number of disks.
#[cfg(feature = "installer")]
fn sys_install_list_impl(out: u64, len: usize) -> i64 {
    use crate::install_ui_logic::ROW_BYTES;
    if let Err(e) = caller_holds_installer() {
        return e;
    }
    if len > 0 {
        if out == 0 {
            return EINVAL;
        }
        if let Err(e) = validate_user_buf(out, len) {
            return e;
        }
    }
    let rows = crate::installer_ui::list();
    let fit = rows.len().min(len / ROW_BYTES);
    for (i, row) in rows.iter().take(fit).enumerate() {
        // Record `i` ends within the validated `out..out + len`.
        if let Err(e) = user_copy(
            (out as *mut u8).wrapping_add(i * ROW_BYTES),
            row.as_ptr(),
            ROW_BYTES,
        ) {
            return e;
        }
    }
    rows.len() as i64
}

/// OT-87 R4.4: `SYS_INSTALL_RUN(slot, generation, start) -> status`.
/// Synchronous: returns when the install has finished or stopped.
#[cfg(feature = "installer")]
fn sys_install_run_impl(slot: u64, generation: u64, start: u64) -> i64 {
    if let Err(e) = caller_holds_installer() {
        return e;
    }
    crate::installer_ui::run(slot, generation, start)
}

#[cfg(feature = "atrium-compositor-g1")]
fn caller_holds_input_system() -> Result<(), i64> {
    let holds = crate::arch::x86_64::without_interrupts(|| {
        process::get(crate::sched::current_pid()).map(|p| p.handles.lock().holds_input_system())
    })
    .ok_or(EINVAL)?;
    if holds { Ok(()) } else { Err(EPERM) }
}

/// SYS_INPUT_POINTER_CLAIM(flags) — bit 0 = reclaim (replace a live route,
/// the WM-restart path). Every claim starts a new session generation.
#[cfg(feature = "atrium-pointer-forward")]
fn sys_input_pointer_claim_impl(flags: u64) -> i64 {
    if !crate::atrium::input_routing_enabled() {
        return ENOSYS;
    }
    if flags & !1 != 0 {
        return EINVAL;
    }
    if let Err(e) = caller_holds_input_system() {
        return e;
    }
    let focus = match crate::atrium::focus::claim_pointer_route(flags & 1 != 0) {
        Ok(focus) => focus,
        Err(crate::atrium::focus::ClaimError::Exists) => return EEXIST,
    };
    let proc = match process::get(crate::sched::current_pid()) {
        Some(p) => p,
        None => return EINVAL,
    };
    match process::install_cap(
        &proc,
        Handle::InputFocus {
            focus,
            cap_id: process::alloc_cap_id(),
            rights: process::pointer_route_rights(),
        },
        None,
    ) {
        Ok(idx) => idx as i64,
        Err(_) => EINVAL,
    }
}

/// SYS_INPUT_POINTER_FORWARD(route, tag, target, x, y) — forward pointer
/// record `tag` to `target` (an ACTIVATE handle) at surface-local `x, y`, or
/// drop it (`target == u32::MAX`). Exactly once; buttons and phase come from
/// the record. Coordinates outside i16 are EINVAL (never clamped). Handles
/// are resolved first and the handle-table lock released before the
/// delivery lock is taken (Jarvis checklist 3).
#[cfg(feature = "atrium-pointer-forward")]
fn sys_input_pointer_forward_impl(route: u32, tag: u64, target: u32, x: i64, y: i64) -> i64 {
    use crate::atrium::focus::ForwardError;
    use crate::atrium::logic::focus_rights;
    if !crate::atrium::input_routing_enabled() {
        return ENOSYS;
    }
    if let Err(e) = caller_holds_input_system() {
        return e;
    }
    let Ok(tag) = u16::try_from(tag) else {
        return EINVAL;
    };
    let Some((x, y)) = crate::atrium::pointer_route_logic::forward_coords(x, y) else {
        return EINVAL;
    };
    let route = match input_focus_handle_lookup(route, focus_rights::RECEIVE) {
        Ok(route) => route,
        Err(e) => return e,
    };
    let target = if target == u32::MAX {
        None
    } else {
        match input_focus_handle_lookup(target, focus_rights::ACTIVATE) {
            Ok(focus) => Some(focus),
            Err(e) => return e,
        }
    };
    match crate::atrium::focus::forward_record(&route, tag, target.as_deref(), x, y) {
        Ok(()) => 0,
        Err(ForwardError::NotRoute) => EPERM,
        Err(ForwardError::Stale) => ESTALE,
        Err(ForwardError::Already) => crate::errno::EALREADY,
        Err(ForwardError::NoTarget) => crate::errno::ESRCH,
    }
}

/// SYS_INPUT_SAK_TAKE(op, h) — op 0: the pending SAK chord bits
/// (`sak_logic::sak_bit`), cleared. `sak-lock-route` adds the lock ops
/// ([`sak_op`]). Input-system authority only.
#[cfg(feature = "atrium-pointer-forward")]
fn sys_input_sak_take_impl(op: u64, h: u32) -> i64 {
    if !crate::atrium::input_routing_enabled() {
        return ENOSYS;
    }
    if let Err(e) = caller_holds_input_system() {
        return e;
    }
    sak_op(op, h)
}

#[cfg(all(
    feature = "atrium-compositor-g1",
    any(
        feature = "neo-test-atrium-input-routing",
        feature = "neo-test-atrium-interactive",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "neo-test-product-integration"
    )
))]
fn sys_input_focus_test_inject_impl(keycode: u8, modifiers: u8) -> i64 {
    if crate::atrium::focus::route_event(crate::atrium::input_logic::InputEventV1::key(
        keycode, modifiers, true,
    )) {
        0
    } else {
        EAGAIN
    }
}

/// SYS_TEST_DIRTY_SCRATCH() → 0. CP-43 test-only: arms
/// [`syscall_return_test_hook`] with a kernel text address, which the hook
/// loads into `rcx` and `r11` on this syscall's way out, after every
/// compiler-generated instruction, as any kernel code is free to leave them.
#[cfg(feature = "neo-test-syscall-return-clean-regs")]
fn sys_test_dirty_scratch_impl() -> i64 {
    SYSCALL_TEST_DIRTY_SCRATCH.store(
        sys_test_dirty_scratch_impl as *const () as u64,
        Ordering::Release,
    );
    0
}

/// SYS_TEST_IRQ_WINDOW(count, pass) → 0. CP-50 test-only: arms the scenario's
/// one-shot timer, then enables interrupts and returns with them on, the way
/// `spawn_user` does (`LocalShootdownQuiesce::resume`).
#[cfg(feature = "neo-test-syscall-exit-irq-window")]
fn sys_test_irq_window_impl(count: u64, pass: u64) -> i64 {
    if count == 0 || count > u32::MAX as u64 || pass > u32::MAX as u64 {
        return EINVAL;
    }
    crate::test_harness::arm_syscall_exit_irq(count as u32, pass as u32);
    super::enable_interrupts();
    0
}

/// Kill-switch equality scenario probes: with the runtime switch off these
/// must return ENOSYS without minting anything. Null args are fine — the
/// enabled-gate fires before any validation.
#[cfg(all(
    feature = "atrium-compositor-g1",
    feature = "neo-test-atrium-killswitch"
))]
pub fn debug_sys_atrium_buffer_create_for_test() -> i64 {
    sys_buffer_create_impl(core::ptr::null(), crate::atrium::logic::BUFFER_DESC_V1_LEN)
}

#[cfg(all(
    feature = "atrium-compositor-g1",
    feature = "neo-test-atrium-killswitch"
))]
pub fn debug_sys_atrium_focus_create_for_test() -> i64 {
    sys_input_focus_create_impl()
}

/// OT-66 L1 test-only (`neo-test-login-cut`): `SYS_TEST_BLOCK_CUT(op)`: 0,
/// the boot's cut step; 1, arm the block write/flush cut at it; 2, disarm and
/// return the ops counted since arming (`drivers::block::login_cut`). Exists
/// only in that scenario's builds (and, so the two can share a dispatch arm,
/// `neo-test-desktop-idle`'s, where it is ENOSYS).
#[cfg(any(feature = "neo-test-desktop-idle", feature = "neo-test-login-cut"))]
pub const SYS_TEST_BLOCK_CUT: u64 = 0xF0A;

/// The test syscalls `SYS_TEST_IDLE_TICKS` and `SYS_TEST_BLOCK_CUT`. They
/// share one dispatch arm, and this constant and function live at the end of
/// the file, so a kernel without either feature has the same lines as before
/// (and so the same panic locations: the kill-switch is bit-for-bit).
#[cfg(any(feature = "neo-test-desktop-idle", feature = "neo-test-login-cut"))]
fn test_idle_or_block_cut(num: u64, a0: u64) -> i64 {
    match num {
        #[cfg(feature = "neo-test-desktop-idle")]
        SYS_TEST_IDLE_TICKS => crate::sched::scheduler::debug_idle_ticks(0) as i64,
        #[cfg(feature = "neo-test-login-cut")]
        SYS_TEST_BLOCK_CUT => crate::drivers::block::login_cut::syscall(a0),
        _ => {
            let _ = a0;
            ENOSYS
        }
    }
}

/// Without `sak-lock-route` every op is the take (the op and handle are
/// ignored, as before L2b).
#[cfg(all(feature = "atrium-pointer-forward", not(feature = "sak-lock-route")))]
#[inline(always)]
fn sak_op(_op: u64, _h: u32) -> i64 {
    crate::atrium::focus::take_sak() as i64
}

/// OT-66 L2b: `SYS_INPUT_SAK_TAKE`'s lock ops, input-system authority only
/// (checked by the caller), on the WM's own focus (an ACTIVATE handle):
/// - 0: take the latch (as without the feature);
/// - 1 (`h`): register `h` as the lock focus, the route while locked;
/// - 2: lock now (the idle lock; Super+L locks in the latch itself);
/// - 3 (`h`): unlock, with `h` the route from now on.
///
/// The lock ops return [`SAK_LOCK_OK`], outside the take's range, so the
/// WM tells a kernel without them (whose take ignores the op) apart, and keeps
/// the bits that take returned. ESRCH: no lock focus registered. EBUSY: the
/// focus can't be a route (the client route cap).
#[cfg(feature = "sak-lock-route")]
fn sak_op(op: u64, h: u32) -> i64 {
    use crate::atrium::focus;
    use crate::atrium::logic::focus_rights;
    let lookup = || input_focus_handle_lookup(h, focus_rights::ACTIVATE);
    match op {
        0 => focus::take_sak() as i64,
        1 => match lookup() {
            Ok(f) if focus::register_lock_route(&f) => SAK_LOCK_OK,
            Ok(_) => EBUSY,
            Err(e) => e,
        },
        2 if focus::engage_lock_route() => SAK_LOCK_OK,
        2 => crate::errno::ESRCH,
        3 => match lookup() {
            Ok(f) if focus::release_lock_route(&f) => SAK_LOCK_OK,
            Ok(_) => EBUSY,
            Err(e) => e,
        },
        _ => EINVAL,
    }
}

/// A lock op's success (`sak_op`): above every latch value (a `u8`).
#[cfg(feature = "sak-lock-route")]
const SAK_LOCK_OK: i64 = 0x100;
