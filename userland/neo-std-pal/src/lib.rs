// SPDX-License-Identifier: MPL-2.0
//! OT-61 browser C0 — the LogansOS **platform abstraction layer (PAL)** for a
//! hosted Rust `std` target (`x86_64-unknown-neoplutarch`).
//!
//! **What this is (and is NOT).** This is the *real* platform-primitive tier —
//! the `sys`-layer implementation — authored in the shape the rust-src
//! `library/std/src/sys/*` overlay consumes. It is **not** a "std-lite crate
//! that shadows a few std names" (the direction Chad/Janus/the cross-family
//! check REFUTEd): it issues **raw libneo syscalls** for the primitives that
//! already close (write/heap/exit) and makes **every not-yet-supported surface
//! fail DETERMINISTICALLY and LOUDLY, carrying its symbol name** — never a
//! silent wrong answer, never UB. C0 does not build the C1 primitives
//! (threads/TLS/waits/VM-protect/clocks); it makes their absence *enumerable*.
//!
//! **C0 wiring (closed now, on the existing syscall surface):**
//! - `write_fd` → `SYS_WRITE` (stdout/stderr → the process's serial/console);
//! - [`SyscallHeap`] → `SYS_MAP_REGION` (the monotonic R+W+**NX** user heap —
//!   A1: **no executable memory**, stays W^X);
//! - `exit` → `SYS_EXIT`.
//!
//! **A1 (reserved, NOT built).** LogansOS user memory is R+W+NX; there is no
//! runtime-executable-code primitive. This PAL introduces **none** — the heap
//! is NX, and no stub grants W^X. JIT is a future kill-switchable invariant.
//!
//! **A3 (co-design).** Each unsupported surface names BOTH consumers it must
//! serve when C1 builds it, so C1 freezes ONE ABI, not a std-only one:
//! see the per-stub `// A3:` notes (Rust `std::sys` ⟷ SpiderMonkey pthread).
//!
//! **A4 (manifest).** Every raw syscall this crate issues is a host-surface
//! entry in `tools/ot61-pal-manifest.toml`; `xtask pal-manifest-check` fails
//! the build if this crate references a `SYS_*` not in the manifest (the
//! anti-compat-island guard). C0 issues exactly: SYS_WRITE, SYS_MAP_REGION,
//! SYS_EXIT.

#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

use linked_list_allocator::Heap;

// ── raw syscall ABI (mirror of libneo; the PAL cannot depend on libneo — it
// sits *below* it in the layering, exactly as std's PAL sits below user crates)
// RAX = number, RDI/RSI/RDX/R10/R8/R9 = args 0..5, return in RAX. ──────────────

/// The three syscall numbers C0 is allowed to issue. Kept in sync with the A4
/// manifest (`tools/ot61-pal-manifest.toml`) and `kernel/src/arch/x86_64/syscall.rs`.
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_MAP_REGION: u64 = 6;

/// The per-call `sys_map_region` ceiling (mirror of `MAX_MAP_REGION_SIZE`); a
/// heap growth larger than this is split into contiguous maps (the monotonic
/// cursor abuts them — the libneo heap discipline).
pub const MAX_MAP_REGION_SIZE: usize = 16 * 1024 * 1024;
/// Page size the kernel maps in.
pub const PAGE_SIZE: usize = 4096;

/// # Safety
/// `num`/`args` must form a valid syscall per the LogansOS ABI.
// Never inlined: xtask's ELF check requires every `syscall` instruction to sit
// inside this symbol (or the std overlay's twin), so it must stay one symbol.
#[inline(never)]
unsafe fn syscall4(num: u64, a0: u64, a1: u64, a2: u64, a3: u64) -> i64 {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") num,
            in("rdi") a0,
            in("rsi") a1,
            in("rdx") a2,
            in("r10") a3,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
            options(nostack),
        );
    }
    ret
}

// ── C0 wired primitives ───────────────────────────────────────────────────────

/// stdout/stderr descriptors. LogansOS has ONE console descriptor: fd 1 is
/// permanently the console, and the kernel resolves every other fd through the
/// process's file-handle table (`kernel/src/arch/x86_64/syscall.rs`,
/// `sys_write_impl`). So stderr is fd 1 too: a write to fd 2 fails `EBADF`, or
/// lands in whatever file the process holds as handle 2. (OT-61 C0.5 finding:
/// C0 had `STDERR_FD = 2`, so on-target every loud-stub message would fail.)
pub const STDOUT_FD: u64 = 1;
pub const STDERR_FD: u64 = 1;

/// Write `buf` to `fd` (SYS_WRITE). Returns bytes written or a negative errno.
/// std's `sys::stdio::Stdout/Stderr::write` overlay calls this.
pub fn write_fd(fd: u64, buf: &[u8]) -> i64 {
    // SAFETY: SYS_WRITE(fd, ptr, len) — the kernel validates buf+len.
    unsafe { syscall4(SYS_WRITE, fd, buf.as_ptr() as u64, buf.len() as u64, 0) }
}

/// Convenience: write a byte string to stderr (used by the loud-stub path).
pub fn write_stderr(msg: &[u8]) {
    let _ = write_fd(STDERR_FD, msg);
}

/// Terminate the process cleanly with `code` (SYS_EXIT). std's
/// `sys::process::exit` / `abort_internal` overlay calls this. Never returns.
pub fn exit(code: i32) -> ! {
    // SAFETY: SYS_EXIT(code) does not return (kernel drops the process).
    unsafe {
        syscall4(SYS_EXIT, code as u64, 0, 0, 0);
    }
    // Fail closed if the kernel ever returned: spin (never UB).
    loop {
        core::hint::spin_loop();
    }
}

/// Map `size` bytes of fresh anonymous R+W+**NX** user memory (SYS_MAP_REGION):
/// the kernel picks the address (monotonic cursor), zeroes it, and the mapping
/// is **non-executable** (A1 — no W^X hole). Returns the base or a negative
/// errno. std's `sys::alloc` overlay and [`SyscallHeap`] draw from this.
pub fn map_region(size: usize) -> Result<*mut u8, i64> {
    // SAFETY: SYS_MAP_REGION(size) returns a user VA or a negative errno.
    let r = unsafe { syscall4(SYS_MAP_REGION, size as u64, 0, 0, 0) };
    if r < 0 { Err(r) } else { Ok(r as *mut u8) }
}

#[inline]
const fn page_round_up(n: usize) -> usize {
    (n + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

/// The C0 global allocator: linked-list heaps backed by `SYS_MAP_REGION`,
/// grown in ≤16 MiB maps (the libneo heap discipline, re-derived here because
/// the PAL sits below libneo). **Never unmaps** (C0 has no `sys_unmap`;
/// regions persist to exit, bounded by the per-process quota, C2.2). NX.
///
/// **Several regions (OT-61 C2.3a).** A new map abuts the heap's top only if
/// nothing else was mapped since. Thread stacks (with their guard pages) come
/// from the same per-process cursor, so once a process has spawned a thread
/// the next heap map can land past a stack. Growing one heap in place then
/// wrote allocator metadata into whatever followed its top: a guard page
/// (a ring-3 #PF, found by `servo_host_smoke`) or, worse, a live stack. So a
/// map that abuts the last region extends it, and any other starts a new one.
/// This is libneo's `LibneoHeap` discipline (its DI-1b fix, for buffer maps),
/// which this copy had not followed; growth is geometric, as there, so the
/// region count grows with the log of the heap size.
pub struct SyscallHeap {
    inner: spin_once::Mutex<Regions>,
}

/// The most disjoint regions a process's heap may span. Each new one is at
/// least 64 KiB; past the limit an allocation fails (null) instead of
/// corrupting anything.
const MAX_HEAP_REGIONS: usize = 64;

struct Regions {
    heaps: [Heap; MAX_HEAP_REGIONS],
    n: usize,
}

impl Regions {
    fn live(&mut self) -> &mut [Heap] {
        &mut self.heaps[..self.n]
    }

    /// Bytes the regions own.
    fn owned(&self) -> usize {
        self.heaps[..self.n].iter().map(|h| h.size()).sum()
    }

    fn allocate(&mut self, layout: Layout) -> Option<*mut u8> {
        // Newest first: the region just mapped for this request is last.
        self.live()
            .iter_mut()
            .rev()
            .find_map(|h| h.allocate_first_fit(layout).ok())
            .map(|p| p.as_ptr())
    }

    /// Add a fresh map of `len` bytes at `region`: extend the last heap when
    /// the map starts at its top, else start a new heap. False when full.
    fn add(&mut self, region: *mut u8, len: usize) -> bool {
        if let Some(last) = self.live().last_mut()
            && last.top() == region
        {
            // SAFETY: `region..region+len` is fresh, process-owned, and
            // starts exactly at this heap's top.
            unsafe { last.extend(len) };
            return true;
        }
        if self.n == MAX_HEAP_REGIONS {
            return false;
        }
        // SAFETY: a fresh, process-owned range of length `len`.
        unsafe { self.heaps[self.n].init(region, len) };
        self.n += 1;
        true
    }
}

/// Bytes to map for `layout` when no region can hold it: its size, plus its
/// alignment when that is more than a page (a fresh map is only
/// page-aligned), and at least 64 KiB.
const fn grow_need(layout: Layout) -> usize {
    let pad = if layout.align() > PAGE_SIZE {
        layout.align()
    } else {
        0
    };
    let need = layout.size().saturating_add(pad);
    if need < 64 * 1024 { 64 * 1024 } else { need }
}

impl SyscallHeap {
    pub const fn new() -> Self {
        Self {
            inner: spin_once::Mutex::new(Regions {
                heaps: [const { Heap::empty() }; MAX_HEAP_REGIONS],
                n: 0,
            }),
        }
    }
}

impl Default for SyscallHeap {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: the heap serializes with its own lock; regions are exclusively owned
// by this process and NX; the allocator obeys the GlobalAlloc contract.
unsafe impl GlobalAlloc for SyscallHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut r = self.inner.lock();
        if let Some(p) = r.allocate(layout) {
            return p;
        }
        // Grow: request the layout size + slack, split into ≤16 MiB maps.
        // Consecutive maps usually abut and extend one heap; one that doesn't
        // (another thread mapped in between) starts a new region, so an
        // allocation larger than one map can then fail (null), never corrupt.
        // A new map is only page-aligned, so a block with a larger alignment
        // (SpiderMonkey's 1 MiB GC chunks, through posix_memalign) needs up
        // to `align` more to find an aligned start inside it (OT-61 C3c-1:
        // without this, a fresh 1 MiB map can't hold a 1 MiB-aligned 1 MiB
        // block, and the allocation fails).
        let need = grow_need(layout);
        // Geometric once a region exists: each growth may cost a region slot.
        let chunk = if r.n == 0 {
            need
        } else {
            need.max((r.owned() / 4).min(MAX_MAP_REGION_SIZE))
        };
        let total = page_round_up(chunk).saturating_add(64 * 1024);
        let mut remaining = total;
        while remaining > 0 {
            let this = remaining.min(MAX_MAP_REGION_SIZE);
            let region = match map_region(this) {
                Ok(p) => p,
                Err(_) => return core::ptr::null_mut(),
            };
            if !r.add(region, this) {
                return core::ptr::null_mut();
            }
            remaining -= this;
        }
        r.allocate(layout).unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if let Some(p) = core::ptr::NonNull::new(ptr) {
            let mut r = self.inner.lock();
            // The region that owns `ptr`: GlobalAlloc guarantees (ptr, layout)
            // came from `alloc`, which only hands out memory inside a region.
            if let Some(h) = r
                .live()
                .iter_mut()
                .find(|h| h.bottom() <= ptr && ptr < h.top())
            {
                // SAFETY: `ptr` was allocated from `h` with `layout`.
                unsafe { h.deallocate(p, layout) };
            }
        }
    }
}

// ── the deterministic loud-failure primitive (the "loud absence" property) ────

/// Fire when an unsupported PAL entry is reached: write a NAMED diagnostic to
/// stderr and exit with the reserved unsupported-PAL code. Deterministic,
/// enumerable, never a silent hang/UB. This is what every C1+ stub calls; a
/// green `std-pal-smoke` requires that the *supported* path never trips it and
/// that a deliberately-unsupported call trips it *by name*.
pub const UNSUPPORTED_PAL_EXIT_CODE: i32 = 90;

/// Guard so the first unsupported hit reports even if called re-entrantly.
static REPORTED: AtomicBool = AtomicBool::new(false);

pub fn unsupported_pal(symbol: &str) -> ! {
    if !REPORTED.swap(true, Ordering::SeqCst) {
        write_stderr(b"[ neo-std-pal ] UNSUPPORTED PAL ENTRY: ");
        write_stderr(symbol.as_bytes());
        write_stderr(b" (C1+ surface not built in C0)\n");
    }
    exit(UNSUPPORTED_PAL_EXIT_CODE)
}

/// Deterministic loud stub for a not-yet-supported PAL entry. Usage:
/// `pal_unsupported!("sys::thread::Thread::new")`.
#[macro_export]
macro_rules! pal_unsupported {
    ($sym:literal) => {
        $crate::unsupported_pal($sym)
    };
}

// ── the reserved C1+ surfaces — DETERMINISTIC stubs, A3-annotated ─────────────
// C0 declares these so their absence is enumerable + named; C1/C2/C3 replace
// each with a real primitive that serves BOTH consumers named in the A3 note.

/// C1 — user thread lifecycle. A3: std `sys::thread::Thread::{new,join}` ⟷
/// SpiderMonkey `js/src/threading/posix/PosixThread.cpp` (create/join/detach/
/// name). C1 freezes one create/join/detach/stack-bounds ABI serving both.
pub fn thread_new_unsupported() -> ! {
    pal_unsupported!("sys::thread::Thread::new [C1]")
}
/// C1 — TLS keys + destructors. A3: std `sys::thread_local::key::{LazyKey,get,
/// set}` ⟷ SpiderMonkey/Mozilla `Mutex.cpp` TLS + GC-root TLS. One key ABI.
pub fn tls_key_unsupported() -> ! {
    pal_unsupported!("sys::thread_local::key [C1]")
}
/// C1 — blocking wait/wake (mutex/condvar, timed). A3: std `sys::sync` ⟷
/// SpiderMonkey `mozglue/misc/ConditionVariable_posix.cpp` monotonic-deadline
/// waits. One wait-on-address + monotonic-timeout primitive.
pub fn wait_unsupported() -> ! {
    pal_unsupported!("sys::sync::{Mutex,Condvar} [C1]")
}
/// C1 — VM unmap/protect/reserve + guard pages (the real region tracker). A3:
/// std `sys::alloc`/VM ⟷ SpiderMonkey GC `js/src/gc/Memory.cpp` reserve/commit/
/// decommit/protect. C0 has only monotonic map (no unmap/protect) — this is the
/// deep MM rung, and A2's per-process quota lands with it.
pub fn vm_protect_unsupported() -> ! {
    pal_unsupported!("sys::vm::{unmap,protect,reserve} [C1; A2 quota rides here]")
}
/// C1 — monotonic + realtime clocks. A3: std `sys::time::{Instant,SystemTime}`
/// ⟷ SpiderMonkey `mozglue/misc/TimeStamp_posix.cpp`. One monotonic + one
/// realtime clock with a defined resolution/monotonicity contract.
pub fn clock_unsupported() -> ! {
    pal_unsupported!("sys::time [C1]")
}
/// C2 — sealed file/resource handles (no ambient fs). A3: std `sys::fs` behind
/// installed preopens ⟷ Servo net filemanager. Fails closed until a broker/
/// preopen is installed.
pub fn fs_unsupported() -> ! {
    pal_unsupported!("sys::fs [C2 — sealed handles only]")
}
/// C2 — network via the broker (renderer gets NO dial cap). A3: std `sys::net`
/// denied ⟷ Servo Hyper/Tokio transport over a broker channel. Ambient
/// `std::net` MUST fail here.
pub fn net_unsupported() -> ! {
    pal_unsupported!("sys::net [C2 — broker only, renderer zero-dial]")
}
/// C2 — entropy. A3: std `sys::random::fill_bytes` ⟷ SpiderMonkey RNG seed.
/// Wired at C2 to a sealed entropy handle; loud until then.
pub fn random_unsupported() -> ! {
    pal_unsupported!("sys::random::fill_bytes [C2]")
}
/// process factory (Servo multiprocess). A3: std `sys::process::Command` ⟷
/// Servo `sandboxing.rs`. Replaced by the embedder fixed-module `sys_spawn`
/// manifest — NOT ambient `Command`. Loud until the embedder seam lands.
pub fn process_spawn_unsupported() -> ! {
    pal_unsupported!("sys::process::Command [embedder seam]")
}

// ── a minimal internal spin mutex (the PAL is below spin/libneo) ──────────────
mod spin_once {
    use core::cell::UnsafeCell;
    use core::ops::{Deref, DerefMut};
    use core::sync::atomic::{AtomicBool, Ordering};

    pub struct Mutex<T> {
        locked: AtomicBool,
        val: UnsafeCell<T>,
    }
    // SAFETY: single-threaded userspace in C0; the lock is a correctness
    // placeholder that becomes real at C1 (wait/wake). No cross-thread sharing
    // exists to race until then.
    unsafe impl<T: Send> Sync for Mutex<T> {}
    unsafe impl<T: Send> Send for Mutex<T> {}

    impl<T> Mutex<T> {
        pub const fn new(val: T) -> Self {
            Self {
                locked: AtomicBool::new(false),
                val: UnsafeCell::new(val),
            }
        }
        pub fn lock(&self) -> Guard<'_, T> {
            while self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_err()
            {
                core::hint::spin_loop();
            }
            Guard { m: self }
        }
    }
    pub struct Guard<'a, T> {
        m: &'a Mutex<T>,
    }
    impl<T> Deref for Guard<'_, T> {
        type Target = T;
        fn deref(&self) -> &T {
            // SAFETY: the lock is held.
            unsafe { &*self.m.val.get() }
        }
    }
    impl<T> DerefMut for Guard<'_, T> {
        fn deref_mut(&mut self) -> &mut T {
            // SAFETY: the lock is held exclusively.
            unsafe { &mut *self.m.val.get() }
        }
    }
    impl<T> Drop for Guard<'_, T> {
        fn drop(&mut self) {
            self.m.locked.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod grow_need_tests {
    use super::{PAGE_SIZE, grow_need};
    use core::alloc::Layout;

    const MIB: usize = 1 << 20;

    #[test]
    fn small_requests_map_at_least_64k() {
        assert_eq!(
            grow_need(Layout::from_size_align(100, 8).unwrap()),
            64 * 1024
        );
    }

    #[test]
    fn page_alignment_needs_no_padding() {
        // A fresh map is page-aligned already.
        let l = Layout::from_size_align(MIB, PAGE_SIZE).unwrap();
        assert_eq!(grow_need(l), MIB);
    }

    #[test]
    fn larger_alignment_reserves_room_to_align() {
        // SpiderMonkey's GC chunk: 1 MiB at 1 MiB alignment needs 2 MiB of
        // page-aligned space to be sure of an aligned start.
        let l = Layout::from_size_align(MIB, MIB).unwrap();
        assert_eq!(grow_need(l), 2 * MIB);
        let l = Layout::from_size_align(256 * 1024, MIB).unwrap();
        assert_eq!(grow_need(l), 256 * 1024 + MIB);
    }
}
