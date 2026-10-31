// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0.5 — `std::sys::neoplutarch_rt`, the overlay's ONLY syscall seam.
//!
//! Overlaid into `library/std/src/sys/neoplutarch_rt.rs` (declared from
//! `sys/mod.rs` under `#[cfg(target_os = "neoplutarch")]`) by
//! `userland/neoplutarch-std-overlay/apply-overlay.sh`. Every raw syscall the
//! std overlay issues goes through the single `syscall4` trampoline below —
//! the stdio, alloc, exit and abort arms all call the named helpers here. That
//! keeps the A4 manifest gate (`userland/neo-std-pal/a4_gate.rs`) exact: the
//! `syscall` instruction may only appear inside a `syscall4`, its `rax` operand
//! may only be the trampoline's `num` parameter, and every `syscall4(...)` call
//! must name a manifest-allowed `SYS_*` constant whose value matches the kernel
//! table.
//!
//! `std` sits below libneo / `neo-std-pal`, so this cannot link them; it uses
//! the same ABI (RAX=num, RDI/RSI/RDX/R10=args, RCX/R11 clobbered). Syscalls:
//! the C0 set (`SYS_EXIT`, `SYS_WRITE`, `SYS_MAP_REGION`) and, since OT-61
//! C1.3, the C1 thread runtime std's threads, sync and `Instant` need: the
//! monotonic clock, futex wait/wake, thread create/exit/join/detach/get-info,
//! yield and `SYS_THREAD_SET_TLS`; since C2.0, `SYS_GETRANDOM` for std's
//! entropy; since C2.1b, the file calls std::fs needs (`SYS_OPEN`, `SYS_READ`,
//! `SYS_CLOSE`, `SYS_FSYNC`; writes reuse `SYS_WRITE` on the handle index),
//! `SYS_PROCESS_SELF` for `process::id` and `SYS_HANDLE_INFO` for
//! `std::os::neoplutarch`. Each is in the A4 manifest with its provenance. A1:
//! `SYS_MAP_REGION` maps R+W+NX; nothing here creates executable memory.
//!
//! Two child modules: [`tls`], the TLS key layer shared with libneo (a separate
//! file, `neoplutarch_rt/tls.rs`), and [`futex`], the `sys::futex` std's
//! futex-based `Mutex`, `Condvar`, `RwLock`, `Once` and thread parker use.

const SYS_EXIT: u64 = 0;
const SYS_WRITE: u64 = 1;
const SYS_MAP_REGION: u64 = 6;
const SYS_OPEN: u64 = 12;
const SYS_READ: u64 = 13;
const SYS_CLOSE: u64 = 14;
const SYS_FSYNC: u64 = 15;
const SYS_CLOCK_GETTIME: u64 = 0x43;
const SYS_FUTEX_WAIT: u64 = 0x44;
const SYS_FUTEX_WAKE: u64 = 0x45;
const SYS_THREAD_CREATE: u64 = 0x46;
const SYS_THREAD_EXIT: u64 = 0x47;
const SYS_THREAD_JOIN: u64 = 0x48;
const SYS_THREAD_DETACH: u64 = 0x49;
const SYS_SCHED_YIELD: u64 = 0x4A;
const SYS_THREAD_GET_INFO: u64 = 0x4D;
const SYS_THREAD_SET_TLS: u64 = 0x4F;
const SYS_PROCESS_SELF: u64 = 0x50;
const SYS_HANDLE_INFO: u64 = 0x51;
const SYS_GETRANDOM: u64 = 0x190;
// OT-61 rung 4 S3: the Atrium shared-pixel-buffer calls, for
// `std::os::neoplutarch::atrium` (a browser content process renders into its
// own buffer and hands the compositor a read-only, derived handle).
const SYS_DERIVE: u64 = 8;
const SYS_BUFFER_CREATE: u64 = 0x1A0;
const SYS_BUFFER_MAP: u64 = 0x1A1;
const SYS_BUFFER_QUERY: u64 = 0x1A2;
const SYS_BUFFER_COMMIT_DAMAGE: u64 = 0x1AD;
const SYS_BUFFER_UNMAP: u64 = 0x1B6;

/// `SYS_CLOCK_GETTIME`'s monotonic clock id.
const CLOCK_MONOTONIC: u64 = 1;

/// `SYS_FUTEX_WAIT`'s "no deadline".
pub const DEADLINE_INFINITE: u64 = u64::MAX;

/// LogansOS errnos this module tests for (negated, as the kernel returns them).
pub const EAGAIN: i64 = -11;
pub const ETIMEDOUT: i64 = -110;

pub(crate) mod tls;

/// The console descriptor. LogansOS fd 1 is permanently the console; the kernel
/// resolves every other fd through the process's file-handle table
/// (`sys_write_impl`). There is no separate stderr descriptor, so stdout AND
/// stderr both write here; fd 2 would fail `EBADF` (which std silently treats
/// as success on a standard stream) or hit a file the process holds as handle 2.
pub const CONSOLE_FD: u64 = 1;

/// Exit code of a deterministic std abort (`abort_internal`: every panic under
/// `panic=abort`, every `rtabort!`, `handle_alloc_error`). 128 + SIGABRT(6),
/// the conventional "aborted" code — distinct from a clean exit and from the
/// kernel's fault-kill code (139).
pub const ABORT_EXIT_CODE: i32 = 134;

// Never inlined: xtask's ELF check requires every `syscall` instruction to sit
// inside this symbol (or the PAL's twin), so it must stay one symbol.
#[inline(never)]
unsafe fn syscall4(num: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    // SAFETY: the caller upholds the per-syscall argument contract; the kernel
    // validates every user pointer + length.
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") num,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
            options(nostack, preserves_flags),
        );
    }
    ret
}

/// `SYS_WRITE(fd, ptr, len)`: bytes written, or a negative LogansOS errno.
pub fn write(fd: u64, buf: &[u8]) -> i64 {
    // SAFETY: `buf` is a live slice; the kernel validates ptr+len.
    unsafe { syscall4(SYS_WRITE, fd, buf.as_ptr() as u64, buf.len() as u64, 0) }
}

/// `SYS_MAP_REGION(size)`: a fresh zeroed R+W+NX user span, or null on error.
pub fn map_region(size: usize) -> *mut u8 {
    // SAFETY: SYS_MAP_REGION takes only a size; it returns a VA or -errno.
    let r = unsafe { syscall4(SYS_MAP_REGION, size as u64, 0, 0, 0) };
    if r < 0 { core::ptr::null_mut() } else { r as *mut u8 }
}

/// `SYS_EXIT(code)`: terminate the process. Never returns.
pub fn exit(code: i32) -> ! {
    // SAFETY: SYS_EXIT does not return; the kernel reaps the process.
    unsafe {
        syscall4(SYS_EXIT, code as u32 as u64, 0, 0, 0);
    }
    // Unreachable unless the kernel returned from SYS_EXIT, which it does not.
    // Spin rather than `ud2`: a user #UD halts the machine (idt.rs).
    loop {
        core::hint::spin_loop();
    }
}

/// The neoplutarch `abort_internal`: a loud line on stderr, then a clean
/// process exit with [`ABORT_EXIT_CODE`]. Replaces `core::intrinsics::abort()`
/// (`ud2`), whose user-mode #UD the kernel answers with `halt_forever()` — a
/// whole-machine hang, not a process abort.
pub fn abort() -> ! {
    let _ = write(CONSOLE_FD, b"[ std ] neoplutarch abort_internal -> SYS_EXIT(134)\n");
    exit(ABORT_EXIT_CODE)
}

/// `SYS_CLOCK_GETTIME(CLOCK_MONOTONIC)`: nanoseconds since boot, nondecreasing
/// (10 ms resolution), or a negative errno (`ENOSYS` on a kernel without the C1
/// runtime).
pub fn monotonic_ns() -> Result<u64, i64> {
    // SAFETY: SYS_CLOCK_GETTIME takes a clock id; it returns ns or -errno.
    let r = unsafe { syscall4(SYS_CLOCK_GETTIME, CLOCK_MONOTONIC, 0, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r as u64) }
}

/// `SYS_FUTEX_WAIT(word, expected, deadline)`: block while `*word == expected`
/// until woken or the ABSOLUTE monotonic `deadline_ns` ([`DEADLINE_INFINITE`]
/// for none). 0 when woken (spurious wakes allowed), else a negative errno:
/// [`ETIMEDOUT`], or `EAGAIN` when the word no longer held `expected`.
pub fn futex_wait_until(word: *const u32, expected: u32, deadline_ns: u64) -> i64 {
    // SAFETY: the kernel validates the 4-aligned user address.
    unsafe { syscall4(SYS_FUTEX_WAIT, word as u64, expected as u64, deadline_ns, 0) }
}

/// `SYS_FUTEX_WAKE(word, count)`: the number of waiters woken, or -errno.
pub fn futex_wake_n(word: *const u32, count: u32) -> i64 {
    // SAFETY: the kernel validates the 4-aligned user address.
    unsafe { syscall4(SYS_FUTEX_WAKE, word as u64, count as u64, 0, 0) }
}

/// `SYS_THREAD_CREATE(entry, arg, stack_size, guard=1)`: a new thread running
/// `entry(arg)` on a kernel-allocated stack with an unmapped guard page below
/// it. The tid, or -errno (`EPERM` without the C1 capability).
pub fn thread_create(entry: extern "C" fn(usize) -> !, arg: usize, stack_size: usize) -> i64 {
    // SAFETY: the kernel validates `entry` (a user address) and the size.
    unsafe { syscall4(SYS_THREAD_CREATE, entry as usize as u64, arg as u64, stack_size as u64, 1) }
}

/// `SYS_THREAD_EXIT(code)`: end the calling thread. Never returns.
pub fn thread_exit(code: i32) -> ! {
    // SAFETY: SYS_THREAD_EXIT does not return.
    unsafe {
        syscall4(SYS_THREAD_EXIT, code as u32 as u64, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}

/// `SYS_THREAD_JOIN(tid, &code)`: block until `tid` exits; its exit code, or
/// -errno.
pub fn thread_join(tid: u64) -> Result<i32, i64> {
    let mut code: i32 = 0;
    // SAFETY: `code` is a live local the kernel writes 4 bytes into.
    let r = unsafe { syscall4(SYS_THREAD_JOIN, tid, &raw mut code as u64, 0, 0) };
    if r < 0 { Err(r) } else { Ok(code) }
}

/// `SYS_THREAD_DETACH(tid)`: 0 or -errno.
pub fn thread_detach(tid: u64) -> i64 {
    // SAFETY: SYS_THREAD_DETACH takes a tid only.
    unsafe { syscall4(SYS_THREAD_DETACH, tid, 0, 0, 0) }
}

/// `SYS_SCHED_YIELD()`.
pub fn sched_yield() {
    // SAFETY: SYS_SCHED_YIELD takes no arguments.
    let _ = unsafe { syscall4(SYS_SCHED_YIELD, 0, 0, 0, 0) };
}

/// The calling thread's stack `(usable_low, usable_len)`, from
/// `SYS_THREAD_GET_INFO(0)` (the kernel's 40-byte `ThreadInfoV1`: tid,
/// usable_low, usable_len, guard_low, guard_len).
pub fn stack_bounds() -> Option<(u64, u64)> {
    let mut info = [0u64; 5];
    // SAFETY: `info` is a live 40-byte local; tid 0 is the caller.
    let r = unsafe {
        syscall4(SYS_THREAD_GET_INFO, 0, info.as_mut_ptr() as u64, crate::mem::size_of::<[u64; 5]>() as u64, 0)
    };
    (r == 0 && info[1] != 0).then_some((info[1], info[2]))
}

/// C2.0: `SYS_GETRANDOM(buf, len)`: fills `buf` from the kernel CSPRNG and
/// returns its length, or -errno (`ENOSYS` without `kernel-rng`, `EINVAL` past
/// the kernel's 64 KiB per-call ceiling). Never a partial fill.
pub fn getrandom(buf: &mut [u8]) -> i64 {
    // SAFETY: `buf` is a live mutable slice; the kernel validates ptr+len.
    unsafe { syscall4(SYS_GETRANDOM, buf.as_mut_ptr() as u64, buf.len() as u64, 0, 0) }
}

/// C2.1b: `SYS_OPEN`'s root form (`flags` 0 opens, 1 creates a regular file).
pub const OPEN_ROOT: u32 = 0;
pub const OPEN_CREATE_REGULAR: u32 = 1;
/// `FileRights` bits (kernel `fs::FileRights`).
pub const FILE_READ: u32 = 1 << 0;
pub const FILE_WRITE: u32 = 1 << 1;
pub const FILE_LOOKUP: u32 = 1 << 2;
pub const FILE_READDIR: u32 = 1 << 3;

/// C2.1b: `SYS_OPEN(path, flags, rights)`: a handle index, or -errno. On a
/// `cap-rooted-open` kernel the root form needs a RootDirCap (EPERM without
/// one, EACCES when `rights` exceed it).
pub fn open(path: &[u8], flags: u32, rights: u32) -> i64 {
    // SAFETY: `path` is a live slice; the kernel validates ptr+len.
    unsafe { syscall4(SYS_OPEN, path.as_ptr() as u64, path.len() as u64, flags as u64, rights as u64) }
}

/// C2.1b: `SYS_READ(handle, buf, len)`: bytes read (0 at end), or -errno.
pub fn read(handle: u32, buf: &mut [u8]) -> i64 {
    // SAFETY: `buf` is a live mutable slice; the kernel validates ptr+len.
    unsafe { syscall4(SYS_READ, handle as u64, buf.as_mut_ptr() as u64, buf.len() as u64, 0) }
}

/// C2.1b: `SYS_CLOSE(handle)`: 0, or -errno.
pub fn close(handle: u32) -> i64 {
    // SAFETY: SYS_CLOSE takes only a handle index.
    unsafe { syscall4(SYS_CLOSE, handle as u64, 0, 0, 0) }
}

/// C2.1b: `SYS_FSYNC(handle)`: 0, or -errno.
pub fn fsync(handle: u32) -> i64 {
    // SAFETY: SYS_FSYNC takes only a handle index.
    unsafe { syscall4(SYS_FSYNC, handle as u64, 0, 0, 0) }
}

/// C2.1b: `SYS_PROCESS_SELF()`: the caller's pid, or -errno (`ENOSYS` on a
/// kernel with neither `browser-runtime` nor `cap-rooted-open`).
pub fn process_self() -> i64 {
    // SAFETY: SYS_PROCESS_SELF takes no arguments.
    unsafe { syscall4(SYS_PROCESS_SELF, 0, 0, 0, 0) }
}

/// C2.1b: `SYS_HANDLE_INFO(idx, out, 16)`: fills `out` with the v1 record of
/// the caller's handle `idx`; 0, or -errno (`EBADF` for an empty slot).
pub fn handle_info(idx: u32, out: &mut [u8; 16]) -> i64 {
    // SAFETY: `out` is a live 16-byte local; the kernel validates ptr+len.
    unsafe { syscall4(SYS_HANDLE_INFO, idx as u64, out.as_mut_ptr() as u64, 16, 0) }
}

/// S3: `SYS_BUFFER_CREATE(desc, 24)`: a new shared pixel buffer from a
/// `BufferDescV1`; the root handle (full buffer rights) or -errno (`ENOSYS`
/// on a kernel without the Atrium runtime). Needs no capability: the buffer is
/// the caller's own memory, charged to its account.
pub fn buffer_create(desc: &[u8; 24]) -> i64 {
    // SAFETY: `desc` is a live 24-byte value; the kernel copies it in.
    unsafe { syscall4(SYS_BUFFER_CREATE, desc.as_ptr() as u64, 24, 0, 0) }
}

/// S3: `SYS_BUFFER_MAP(handle, role)`: map the buffer; role 0 is RW+NX
/// (needs MAP_WRITE), role 1 is RO+NX (needs MAP_READ). The base address, or
/// -errno.
pub fn buffer_map(handle: u32, role: u32) -> i64 {
    // SAFETY: two scalars; the kernel picks the address.
    unsafe { syscall4(SYS_BUFFER_MAP, handle as u64, role as u64, 0, 0) }
}

/// S3: `SYS_BUFFER_UNMAP(base)`: remove a mapping made by [`buffer_map`].
pub fn buffer_unmap(base: u64) -> i64 {
    // SAFETY: the kernel only removes a buffer mapping that starts at `base`.
    unsafe { syscall4(SYS_BUFFER_UNMAP, base, 0, 0, 0) }
}

/// S3: `SYS_BUFFER_QUERY(handle, out, 24)`: the buffer's `BufferDescV1`.
pub fn buffer_query(handle: u32, out: &mut [u8; 24]) -> i64 {
    // SAFETY: `out` is a live 24-byte local; the kernel validates ptr+len.
    unsafe { syscall4(SYS_BUFFER_QUERY, handle as u64, out.as_mut_ptr() as u64, 24, 0) }
}

/// S3: `SYS_BUFFER_COMMIT_DAMAGE(handle, rect, 8)`: publish one damage
/// rectangle (`DamageRectV1`) for the compositor.
pub fn buffer_commit_damage(handle: u32, rect: &[u8; 8]) -> i64 {
    // SAFETY: `rect` is a live 8-byte value; the kernel copies it in.
    unsafe { syscall4(SYS_BUFFER_COMMIT_DAMAGE, handle as u64, rect.as_ptr() as u64, 8, 0) }
}

/// S3: `SYS_DERIVE(handle, meta, semantic)`: a new handle to the same object
/// with at most the given rights (attenuation only; the kernel refuses
/// anything wider than `handle`'s). The new index, or -errno.
pub fn derive(handle: u32, meta: u8, semantic: u8) -> i64 {
    // SAFETY: three scalars.
    unsafe { syscall4(SYS_DERIVE, handle as u64, meta as u64, semantic as u64, 0) }
}

/// C2.1b: the handle-table slots [`handle_info`] scans (the kernel's tables
/// are small; a process past this many handles is not a sealed renderer).
pub const HANDLE_SCAN: u32 = 256;
/// `SYS_HANDLE_INFO` kind code of a file handle, and its ROOT_DIR flag.
pub const HANDLE_KIND_FILE: u32 = 3;
pub const HANDLE_FLAG_ROOT_DIR: u8 = 1 << 0;

/// C2.1b: whether this process holds a RootDirCap (a file handle on "/").
/// `false` when the kernel has no `SYS_HANDLE_INFO`, so std::fs fails closed
/// there too.
pub fn holds_root_dir_cap() -> bool {
    let mut rec = [0u8; 16];
    (0..HANDLE_SCAN).any(|idx| {
        handle_info(idx, &mut rec) == 0
            && u32::from_le_bytes([rec[4], rec[5], rec[6], rec[7]]) == HANDLE_KIND_FILE
            && rec[14] & HANDLE_FLAG_ROOT_DIR != 0
    })
}

/// [`tls`]'s `SYS_THREAD_SET_TLS(base)`: 0 or -errno.
fn set_tls(base: u64) -> i64 {
    // SAFETY: the kernel only records `base`; it never dereferences it.
    unsafe { syscall4(SYS_THREAD_SET_TLS, base, 0, 0, 0) }
}

/// [`tls`]'s allocator: std's global allocator.
///
/// # Safety
/// `layout` has a non-zero size.
unsafe fn alloc_zeroed(layout: crate::alloc::Layout) -> *mut u8 {
    // SAFETY: the caller's contract.
    unsafe { crate::alloc::alloc_zeroed(layout) }
}

/// # Safety
/// `ptr` came from [`alloc_zeroed`] with this `layout`.
unsafe fn dealloc(ptr: *mut u8, layout: crate::alloc::Layout) {
    // SAFETY: the caller's contract.
    unsafe { crate::alloc::dealloc(ptr, layout) }
}

/// `std::sys::futex` for neoplutarch (declared from `sys/mod.rs`), the shape
/// std's futex-based `Mutex`, `Condvar`, `RwLock`, `Once` and thread parker
/// take (the hermit/Linux `futex.rs` contract).
pub(crate) mod futex {
    use crate::sync::atomic::Atomic;
    use crate::time::Duration;

    /// An atomic for use as a futex: the kernel's futex word is 32 bits.
    pub type Futex = Atomic<Primitive>;
    pub type Primitive = u32;
    /// std's small futex: the same 32-bit word.
    pub type SmallFutex = Atomic<SmallPrimitive>;
    pub type SmallPrimitive = u32;

    /// Wait while `futex` holds `expected`, for at most `timeout`. `false`
    /// only on timeout; a wake, a spurious wake or a changed value return
    /// `true` (the callers re-check their state). The kernel takes an absolute
    /// monotonic deadline, so the relative timeout is added to the clock; a sum
    /// past `u64` nanoseconds (584 years) waits without a deadline.
    pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
        let deadline = match timeout {
            None => super::DEADLINE_INFINITE,
            // No clock: report the wait as timed out at once rather than
            // turning a timed wait into an unbounded one.
            Some(t) => match super::monotonic_ns() {
                Ok(now) => u64::try_from(t.as_nanos())
                    .ok()
                    .and_then(|ns| now.checked_add(ns))
                    .unwrap_or(super::DEADLINE_INFINITE),
                Err(_) => return false,
            },
        };
        super::futex_wait_until(futex.as_ptr(), expected, deadline) != super::ETIMEDOUT
    }

    /// Wake one waiter. `true` if one was woken.
    #[inline]
    pub fn futex_wake(futex: &Atomic<u32>) -> bool {
        super::futex_wake_n(futex.as_ptr(), 1) > 0
    }

    /// Wake every waiter.
    #[inline]
    pub fn futex_wake_all(futex: &Atomic<u32>) {
        super::futex_wake_n(futex.as_ptr(), u32::MAX);
    }
}
