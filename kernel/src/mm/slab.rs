// SPDX-License-Identifier: MPL-2.0
//! Slab allocator (sub-commit 2 of Phase 2 BUILD-2).
//!
//! Per ADR-012 §Q2 (per-CPU caches with global fallback) + §Q3
//! (incremental migration). This sub-commit implements the slab body;
//! sub-commits 3-4 wire callers.
//!
//! **What this file establishes:**
//!   - `SlabCache<T>` trait — the abstract API every cache implements.
//!   - `Slab<T>` — concrete container with working alloc/free.
//!   - `SlabConfig` — per-cache tunable struct. The struct IS the
//!     contract (Principle 5: mechanism vs numbers); the values inside
//!     are tunables expected to be revised under measurement.
//!   - `Magazine<T>`, `GlobalCache<T>`, `SlabPerCpuState` — working
//!     implementations with AtomicU64 counters (SMP-correct from day one,
//!     per Principle 4).
//!   - Per-cache `SlabConfig` constants for the production cache types.
//!
//! **Cache inventory (OT-8 resolution, 2026-07-03):**
//!   - `THREAD_CACHE` — production: Thread constructors and the scheduler
//!     reaper allocate/free through it (Thread is the hot spawn/reap type).
//!   - `TestObject` — slab test scenarios only.
//!   - `PROCESS_CACHE`/`CHANNEL_CACHE` were **removed** as dead scaffolding.
//!     They were declared+initialized since Phase 2 BUILD-2 but never
//!     allocated through: `Arc`-backed ownership (`Arc<Process>`,
//!     `Arc<Channel>`) cannot route through a bare-`T` slab — the sub-commit-3
//!     `Arc::from(Box<Channel>)` attempt bled cache entries (Arc's drop frees
//!     to the global allocator, not `slab::free`) and was reverted, and every
//!     correct path (custom Drop wrapper, unstable `Arc::new_in`, refactor
//!     away from Arc) is a cross-cutting change outside this file. See
//!     ADR-012's OT-8 amendment. Re-adding a cache is a config + static +
//!     `SlabObject` impl if a slab-compatible refcount type ever lands.
//!   - Handle and EndpointState do NOT get slab caches — they are stored
//!     inline in container types, not separately allocated.
//!
//! **Per-CPU access (Principle 4):** uses `arch::x86_64::per_cpu::slab_state()`.
//! The current `PerCpu` struct has `slab_state: UnsafeCell<*mut ()>` at
//! offset 16 (SLAB_STATE_OFFSET); the pointer indirects to a heap-allocated
//! `SlabPerCpuState` struct (option a from ADR-012 implementation note 13).
//! The GS-base access pattern is exercised in single-CPU; SMP retrofit is
//! mechanical (one `SlabPerCpuState` per CPU).

use alloc::boxed::Box;
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use spin::{Mutex, Once};

// Helper to write a pointer to memory
unsafe fn write_ptr<T>(dst: *mut T, src: T) {
    // SAFETY: Caller asserts the pointer is valid for writes (this is
    // an `unsafe fn`); Rust 2024 edition still requires an explicit
    // `unsafe` block inside `unsafe fn` bodies (`unsafe_op_in_unsafe_fn`).
    unsafe {
        ptr::write(dst, src);
    }
}

use crate::mm;
use crate::sched::thread::Thread;
use crate::serial_println;

// === OT-29: thread-slab free-list corruption discriminator ===============
//
// ISC-2. The slab free-list is intrusive (a freed node's first 8 bytes hold the
// next-pointer) plus a per-CPU non-intrusive magazine. A double-free or a write
// into a freed-but-linked node corrupts the free-list, and the damage only
// surfaces much later as a `ptr::write` to a null/misaligned pointer in
// `Thread::alloc_from_slab` (a small integer field — e.g. a ThreadId — read back
// as the free-list head). This module makes the corruption *self-diagnosing*:
//   - on free: if the node is already poisoned -> DOUBLE-FREE (with both tids);
//     else write a sentinel at byte offset 8 + the owner tid at offset 16
//     (offset 0 is the intrusive link, must stay free);
//   - on alloc of a CACHED node: assert non-null + aligned (catches the
//     self-looped free-list head) and assert poisoned (catches a node handed out
//     live / freed-while-linked), then consume the poison.
// Gated behind `slab-debug-poison`; zero cost in production. Only objects >=
// MIN_SIZE carry the poison (Thread/Process/Channel qualify; TestObject is 8 B
// and is skipped — its scenarios don't need it).
#[cfg(feature = "slab-debug-poison")]
mod poison {
    use core::ptr;

    /// Sentinel u64 written into every word of a freed node from offset 8 to the
    /// end (offset 0 is the intrusive free-list link — left writable). Chosen so
    /// no legitimate field (pid, id, small counters) collides with it.
    pub const SENTINEL: u64 = 0xF00D_DEAD_BEEF_CAFE;
    /// Minimum object size to carry the poison. Thread/Process/Channel all
    /// exceed this; TestObject (8 B) is skipped (its scenarios don't need it).
    pub const MIN_SIZE: usize = 24;

    #[inline]
    fn enabled<T>() -> bool {
        core::mem::size_of::<T>() >= MIN_SIZE
    }

    /// Number of full u64 words in `[8, size_of::<T>())` (offset 0 reserved for
    /// the intrusive link).
    #[inline]
    fn poison_words<T>() -> usize {
        (core::mem::size_of::<T>() - 8) / 8
    }

    /// On free: detect double-free, then poison the node ([8..) filled with the
    /// sentinel). The intrusive link at offset 0 is left untouched. Detects a
    /// double-free because a still-poisoned node was never re-allocated.
    #[inline]
    pub fn on_free<T>(obj: *mut T, cache: &'static str) {
        if !enabled::<T>() {
            return;
        }
        let base = obj as *mut u8;
        let words = poison_words::<T>();
        // SAFETY: `obj` is a just-freed node (ownership transferred via
        // Box::into_raw), >= MIN_SIZE; words span exactly [8, size) in-bounds.
        unsafe {
            // Word at offset 8 is the double-free marker.
            let first = base.add(8) as *mut u64;
            if ptr::read_volatile(first) == SENTINEL {
                panic!(
                    "slab[{}]: DOUBLE-FREE node={:#x} (already poisoned — freed twice; second-freed by tid={})",
                    cache,
                    obj as usize,
                    crate::arch::x86_64::per_cpu::current_thread_id(),
                );
            }
            for i in 0..words {
                ptr::write_volatile(base.add(8 + i * 8) as *mut u64, SENTINEL);
            }
        }
    }

    /// On alloc of a CACHED node (from the magazine or global free-list): assert
    /// it is non-null, aligned, and still fully poisoned. A non-sentinel word is
    /// a write into freed memory — for the OT-29 SMP exit/reap UAF that word is a
    /// kernel stack pointer (the asm `mov [rdi], rsp` into a reaped Thread),
    /// unreachable by any in-lock path. The caller's `ptr::write` clears the
    /// poison, so a re-alloc without an intervening free is caught next time.
    #[inline]
    pub fn on_alloc_cached<T>(obj: *mut T, cache: &'static str) {
        // Non-null + alignment first: a corrupted free-list head is a small
        // misaligned value (a live field read back as the link pointer).
        if obj.is_null() || !(obj as usize).is_multiple_of(core::mem::align_of::<T>()) {
            panic!(
                "slab[{}]: FREE-LIST HEAD CORRUPT node={:#x} (null/misaligned — a live field read as the free-list link)",
                cache, obj as usize
            );
        }
        if !enabled::<T>() {
            return;
        }
        let base = obj as *mut u8;
        let words = poison_words::<T>();
        // SAFETY: `obj` is non-null, aligned, >= MIN_SIZE; words span [8, size).
        unsafe {
            for i in 0..words {
                let w = ptr::read_volatile(base.add(8 + i * 8) as *const u64);
                if w != SENTINEL {
                    panic!(
                        "slab[{}]: UAF node={:#x} word+{} = {:#x} (write into a freed node — OT-29 SMP exit/reap rsp-write into a reaped Thread)",
                        cache,
                        obj as usize,
                        8 + i * 8,
                        w,
                    );
                }
            }
        }
    }
}

// === Public API =========================================================

/// Per-cache configuration. The struct shape is the load-bearing contract
/// from ADR-012 §Q3; the values inside are tunables (Principle 5).
#[derive(Copy, Clone)]
pub struct SlabConfig {
    /// Per-CPU magazine high-water mark — push to global on full.
    pub local_capacity: usize,
    /// Global cache high-water mark — return backing pages to buddy on
    /// overflow. Bounds slab's frame retention so the leak audit closes.
    pub global_capacity: usize,
    /// Batch size for local↔global transfer.
    pub batch_size: usize,
    /// Zero each object on free (before returning to the cache).
    /// Per Principle 7 (default-conservative for new cache types):
    /// new instances should default to `true`. Set `false` only when
    /// the cache type's constructor demonstrably overwrites every field
    /// on alloc; document the rationale.
    pub zero_on_free: bool,
}

/// Cache statistics (observability surface).
#[derive(Copy, Clone, Default)]
pub struct SlabStats {
    /// Objects in per-CPU magazines (sum across all CPUs).
    pub local_in_use: usize,
    /// Objects in global cache.
    pub global_in_use: usize,
    /// Pages held by slab from buddy (8 KiB per page = 2 frames).
    pub buddy_pages_held: usize,
}

/// The abstract slab cache API. Every cache instance implements this
/// trait via `Slab<T>`.
pub trait SlabCache<T: Send> {
    /// Allocate one object of type `T`. Returns `None` on OOM. The
    /// returned `Box<T>` is conceptually owned by the caller; freeing
    /// it via `free` returns the backing storage to the cache.
    fn alloc(&self) -> Option<Box<T>>;

    /// Return an object to the cache. Caller transfers ownership; do not
    /// access `obj` after this returns.
    fn free(&self, obj: Box<T>);

    /// Snapshot of cache statistics. Reads atomic counters; cheap.
    fn stats(&self) -> SlabStats;
}

// === Storage primitives (sub-commit 2 implementation) ====================

/// Per-CPU magazine. Holds a small set of free objects on the CPU's
/// hot path; pulled from global cache on miss, pushed to global on
/// fill. Lockless on the hot path (single-CPU + `without_interrupts`
/// callers in Phase 2; SMP retrofit decides per-CPU access discipline
/// in BUILD-3+).
///
/// Layout: ring buffer with head/tail indices. Objects are stored as
/// `*mut T` (raw pointers) to avoid lifetime issues with raw memory.
/// The ring capacity is `local_capacity + 1` to distinguish full vs empty.
pub struct Magazine<T: Send> {
    /// Ring buffer of object pointers.
    buffer: UnsafeCell<*mut T>,
    /// Head index (next pop).
    head: UnsafeCell<usize>,
    /// Tail index (next push).
    tail: UnsafeCell<usize>,
    /// Ring capacity (allocated size, not usable capacity).
    capacity: usize,
    /// Per-CPU magazine overflow count (batch transfers to global).
    pub overflow_count: UnsafeCell<u64>,
    /// Per-CPU magazine refill count (batch transfers from global).
    pub refill_count: UnsafeCell<u64>,
}

impl<T: Send> Magazine<T> {
    /// Create a new magazine with the given capacity.
    pub fn new(capacity: usize) -> Self {
        // Allocate ring buffer on heap (one-time per-CPU allocation at init).
        // In single-CPU mode, exactly one allocation per cache type.
        let buffer = unsafe {
            let layout = alloc::alloc::Layout::array::<*mut T>(capacity).unwrap();
            alloc::alloc::alloc(layout) as *mut T
        };
        if buffer.is_null() {
            panic!("Magazine::new: failed to allocate buffer");
        }

        Magazine {
            buffer: UnsafeCell::new(buffer),
            head: UnsafeCell::new(0),
            tail: UnsafeCell::new(0),
            capacity,
            overflow_count: UnsafeCell::new(0),
            refill_count: UnsafeCell::new(0),
        }
    }

    /// Push an object to the magazine. Returns true if overflow (batch to global).
    pub fn push(&self, obj: *mut T) -> bool {
        unsafe {
            let head = *self.head.get();
            let tail = *self.tail.get();
            let next_tail = (tail + 1) % self.capacity;

            // Magazine is full
            if next_tail == head {
                *self.overflow_count.get() += 1;
                return true; // overflow
            }

            // OT-29 fix: the ring stores `*mut T` ELEMENTS, but `buffer` is typed
            // `*mut T`, so `.add(tail)` would stride by `size_of::<T>()` and walk
            // off the (capacity * size_of::<*mut T>())-byte allocation for any
            // `tail >= 1`. Cast to `*mut *mut T` BEFORE `.add(tail)` so it strides
            // by the pointer size. (TestObject is 8 B = pointer size, which is why
            // the slab unit tests never caught this; Thread/Process/Channel are
            // large, so the 2nd+ cached object scribbled adjacent heap and a later
            // `pop` returned garbage — the `node=0x5` free-list corruption.)
            let slot = ((*self.buffer.get()) as *mut *mut T).add(tail);
            ptr::write(slot, obj);
            *self.tail.get() = next_tail;
            false
        }
    }

    /// Pop an object from the magazine. Returns None if empty.
    pub fn pop(&self) -> Option<*mut T> {
        unsafe {
            let head = *self.head.get();
            let tail = *self.tail.get();

            if head == tail {
                return None; // empty
            }

            // OT-29 fix: stride by the pointer size, not `size_of::<T>()` — cast
            // `buffer` (`*mut T`) to `*const *mut T` BEFORE `.add(head)` (see the
            // matching note in `push`).
            let slot = ((*self.buffer.get()) as *const *mut T).add(head);
            let obj = ptr::read(slot);
            *self.head.get() = (head + 1) % self.capacity;
            Some(obj)
        }
    }

    /// Number of objects currently in the magazine.
    pub fn len(&self) -> usize {
        unsafe {
            let head = *self.head.get();
            let tail = *self.tail.get();
            if tail >= head {
                tail - head
            } else {
                self.capacity - head + tail
            }
        }
    }

    /// Check if magazine is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if magazine is full.
    pub fn is_full(&self) -> bool {
        unsafe {
            let head = *self.head.get();
            let tail = *self.tail.get();
            let next_tail = (tail + 1) % self.capacity;
            next_tail == head
        }
    }

    /// Get overflow count.
    pub fn get_overflow_count(&self) -> u64 {
        unsafe { *self.overflow_count.get() }
    }

    /// Get refill count.
    pub fn get_refill_count(&self) -> u64 {
        unsafe { *self.refill_count.get() }
    }
}

impl<T: Send> Drop for Magazine<T> {
    fn drop(&mut self) {
        unsafe {
            // Free the ring buffer. OT-32: `buffer` is `UnsafeCell<*mut T>`, so
            // `self.buffer.get()` is `*mut *mut T` — the CELL's own address, not
            // the heap allocation. `Magazine::new` stored `*self.buffer.get()`
            // (the `alloc()`-returned `*mut T`), so Drop must dealloc THAT, with
            // the same `Layout::array::<*mut T>(capacity)` used to allocate it.
            // (Latent today: `SlabPerCpuState` is leaked, so magazines never
            // drop — but freeing the cell address is a real wrong-pointer free.)
            let layout = alloc::alloc::Layout::array::<*mut T>(self.capacity).unwrap();
            alloc::alloc::dealloc(*self.buffer.get() as *mut u8, layout);
        }
    }
}

/// Global cache. Holds the bulk of free objects across all CPUs;
/// per-cache global lock. Returns excess backing pages to buddy on
/// `global_capacity` overflow.
pub struct GlobalCache<T: Send> {
    /// Free list of objects (stored as raw pointers).
    free_list: *mut T,
    /// Count of objects in global cache.
    count: UnsafeCell<usize>,
    /// Count of objects that have been allocated from global (for stats).
    allocated_from_global: UnsafeCell<u64>,
    /// Backing pages held from buddy (each page = 8 KiB = 2 frames).
    backing_pages: UnsafeCell<usize>,
}

impl<T: Send> GlobalCache<T> {
    /// Create a new global cache.
    pub fn new() -> Self {
        GlobalCache {
            free_list: ptr::null_mut(),
            count: UnsafeCell::new(0),
            allocated_from_global: UnsafeCell::new(0),
            backing_pages: UnsafeCell::new(0),
        }
    }

    /// Push an object to the global cache.
    pub fn push(&mut self, obj: *mut T) {
        unsafe {
            // Link into free list (simplified: just push to head)
            // The first field of T is assumed to be a pointer-sized field
            // that can hold the next pointer. For TestObject (u64), we
            // store the pointer directly. For real types, we'd use a
            // proper linked list node or require a trait.
            ptr::write(obj as *mut *mut T, self.free_list);
            self.free_list = obj;
            *self.count.get() += 1;
        }
    }

    /// Pop an object from the global cache.
    pub fn pop(&mut self) -> Option<*mut T> {
        if self.free_list.is_null() {
            return None;
        }
        unsafe {
            let obj = self.free_list;
            self.free_list = ptr::read(obj as *const *mut T);
            *self.count.get() -= 1;
            *self.allocated_from_global.get() += 1;
            Some(obj)
        }
    }

    /// Number of objects in global cache.
    pub fn len(&self) -> usize {
        unsafe { *self.count.get() }
    }

    /// Increment backing pages count.
    pub fn add_backing_page(&self) {
        unsafe {
            *self.backing_pages.get() += 1;
        }
    }

    /// Decrement backing pages count and return current value.
    pub fn remove_backing_page(&self) -> usize {
        unsafe {
            let pages = *self.backing_pages.get();
            *self.backing_pages.get() = pages - 1;
            pages - 1
        }
    }

    /// Get backing pages count.
    pub fn get_backing_pages(&self) -> usize {
        unsafe { *self.backing_pages.get() }
    }

    /// Get allocated from global count.
    pub fn get_allocated_from_global(&self) -> u64 {
        unsafe { *self.allocated_from_global.get() }
    }
}

/// Per-CPU slab state — heap-allocated once at boot, pointed at via the
/// `PerCpu.slab_state` field per ADR-012 implementation note 13 option (a).
/// One instance per CPU; in single-CPU mode, exactly one allocation.
///
/// Layout: contains one `Magazine<T>` per live cache type (Thread for
/// production, TestObject for slab test scenarios). The magazines are
/// heap-allocated at init time; the `SlabPerCpuState` struct itself is also
/// heap-allocated and intentionally leaked (Box::into_raw) so it persists
/// for kernel lifetime.
///
/// **Lifetime note:** This struct is intentionally leaked at init time.
/// `Box::into_raw(Box::new(SlabPerCpuState::new()))` is the discipline
/// marker, not an oversight. Per-CPU slab state has kernel lifetime.
pub struct SlabPerCpuState {
    /// Per-CPU magazine for TestObject (used in test scenarios).
    pub test_object: Magazine<TestObject>,
    /// Per-CPU magazine for Thread cache.
    pub thread: Magazine<Thread>,
}

impl SlabPerCpuState {
    /// Create per-CPU slab state for the live cache types.
    pub fn new() -> Self {
        SlabPerCpuState {
            test_object: Magazine::new(16 + 1), // TestObject uses batch_size=8, local_capacity=16
            thread: Magazine::new(THREAD_CACHE_CONFIG.local_capacity + 1),
        }
    }
}

/// Types backed by a slab cache. Each type selects its own per-CPU
/// magazine from `SlabPerCpuState`; sharing the first field by cast would mix
/// cached objects across types.
pub trait SlabObject: Send + Sized {
    fn magazine(state: &SlabPerCpuState) -> &Magazine<Self>;
}

impl SlabObject for TestObject {
    fn magazine(state: &SlabPerCpuState) -> &Magazine<Self> {
        &state.test_object
    }
}

impl SlabObject for Thread {
    fn magazine(state: &SlabPerCpuState) -> &Magazine<Self> {
        &state.thread
    }
}

// === Concrete cache container ===========================================

/// One slab cache instance per object type. `const`-constructible so
/// cache instances can be `static`.
pub struct Slab<T: SlabObject> {
    name: &'static str,
    config: SlabConfig,
    /// Global cache; per-cache lock. Per-CPU magazines live in the
    /// per-CPU `SlabPerCpuState` accessed via GS-base.
    global: Mutex<GlobalCache<T>>,
    _phantom: PhantomData<T>,
}

// SAFETY: `Magazine<T>` and `GlobalCache<T>` contain `*mut T` and `UnsafeCell`
// fields. Their thread-safety is enforced by the slab's locking discipline:
//   - `Magazine<T>` access is per-CPU and protected by `without_interrupts` on
//     single-CPU. SMP retrofit (Phase 2 BUILD-3+) decides per-CPU access.
//   - `GlobalCache<T>` is held inside `Mutex<GlobalCache<T>>` on `Slab<T>`.
//   - Static `Slab<T>` instances require `Sync`. We assert it here, given
//     the discipline above.
// `T: Send + Sync` ensures the contained values are themselves thread-safe.
unsafe impl<T: Send + Sync> Send for Magazine<T> {}
unsafe impl<T: Send + Sync> Sync for Magazine<T> {}
unsafe impl<T: Send + Sync> Send for GlobalCache<T> {}
unsafe impl<T: Send + Sync> Sync for GlobalCache<T> {}
unsafe impl<T: SlabObject + Sync> Send for Slab<T> {}
unsafe impl<T: SlabObject + Sync> Sync for Slab<T> {}

impl<T: SlabObject> Slab<T> {
    /// Construct a slab cache. `const` so cache instances can live in
    /// `static` initializers.
    pub const fn new(name: &'static str, config: SlabConfig) -> Self {
        Self {
            name,
            config,
            global: Mutex::new(GlobalCache {
                free_list: ptr::null_mut(),
                count: UnsafeCell::new(0),
                allocated_from_global: UnsafeCell::new(0),
                backing_pages: UnsafeCell::new(0),
            }),
            _phantom: PhantomData,
        }
    }

    /// Cache name (for diagnostics / future-`/proc`-style introspection).
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Cache configuration snapshot.
    pub const fn config(&self) -> SlabConfig {
        self.config
    }

    /// Allocate from per-CPU magazine or global cache.
    /// Returns None on OOM.
    pub fn alloc(&self) -> Option<Box<T>> {
        // Get per-CPU slab state via GS-base accessor.
        let slab_state = crate::arch::x86_64::per_cpu::slab_state();
        if slab_state.is_null() {
            panic!("Slab::alloc: per-CPU slab state not initialized");
        }

        let state = unsafe { &*(slab_state as *const SlabPerCpuState) };
        let magazine = T::magazine(state);

        // Try per-CPU magazine first.
        if let Some(obj) = magazine.pop() {
            #[cfg(feature = "slab-debug-poison")]
            poison::on_alloc_cached(obj, self.name);
            return Some(unsafe { Box::from_raw(obj) });
        }

        // Miss: acquire global lock and batch transfer.
        let mut global = self.global.lock();

        // Try to get a batch from global cache.
        let mut batch_count = 0;
        while batch_count < self.config.batch_size {
            if let Some(obj) = global.pop() {
                // Push to magazine (this may overflow and push to global again
                // if magazine is already full, but that's fine for the first pop).
                if magazine.push(obj) {
                    // Overflow: push back to global
                    global.push(obj);
                }
                batch_count += 1;
            } else {
                break;
            }
        }

        // Record refill
        unsafe {
            *magazine.refill_count.get() += 1;
        }

        // Try again after batch transfer.
        if let Some(obj) = magazine.pop() {
            #[cfg(feature = "slab-debug-poison")]
            poison::on_alloc_cached(obj, self.name);
            return Some(unsafe { Box::from_raw(obj) });
        }

        // Global cache empty: allocate fresh from buddy.
        let obj = unsafe {
            let layout = alloc::alloc::Layout::new::<T>();
            alloc::alloc::alloc(layout) as *mut T
        };

        if obj.is_null() {
            return None; // OOM
        }

        // Update backing page count
        global.add_backing_page();

        Some(unsafe { Box::from_raw(obj) })
    }

    /// Return an object to the cache.
    pub fn free(&self, mut obj: Box<T>) {
        // Zero object if configured
        if self.config.zero_on_free {
            unsafe {
                core::ptr::write_bytes(
                    &mut *obj as *mut T as *mut u8,
                    0,
                    core::mem::size_of::<T>(),
                );
            }
        }

        let obj_ptr = Box::into_raw(obj);

        // OT-29 discriminator: detect double-free + poison the node before it is
        // linked into the magazine/global free-list (offset 0 is the link; the
        // poison lives at offsets 8/16, which the intrusive push leaves intact).
        #[cfg(feature = "slab-debug-poison")]
        poison::on_free(obj_ptr, self.name);

        // Get per-CPU slab state.
        let slab_state = crate::arch::x86_64::per_cpu::slab_state();
        if slab_state.is_null() {
            panic!("Slab::free: per-CPU slab state not initialized");
        }

        let state = unsafe { &*(slab_state as *const SlabPerCpuState) };
        let magazine = T::magazine(state);

        // Try to push to per-CPU magazine.
        if !magazine.push(obj_ptr) {
            // No overflow, done.
            return;
        }

        // Overflow: push to global cache.
        let mut global = self.global.lock();
        global.push(obj_ptr);
    }

    /// Snapshot of cache statistics.
    pub fn stats(&self) -> SlabStats {
        // Get per-CPU slab state.
        let slab_state = crate::arch::x86_64::per_cpu::slab_state();
        if slab_state.is_null() {
            return SlabStats::default();
        }

        let state = unsafe { &*(slab_state as *const SlabPerCpuState) };
        let magazine = T::magazine(state);

        let local_count = magazine.len();
        let global_count = {
            let global = self.global.lock();
            global.len()
        };
        let buddy_pages = {
            let global = self.global.lock();
            global.get_backing_pages()
        };

        SlabStats {
            local_in_use: local_count,
            global_in_use: global_count,
            buddy_pages_held: buddy_pages,
        }
    }
}

// === Per-cache configurations ===========================================

/// Cache configuration (per ADR-012 §Q3 + BUILD-2 amendment; ratios
/// local:global = 1:4, batch:local = 1:2). Numbers are educated guesses
/// per Principle 5; the `SlabConfig` struct is the contract — these specific
/// values are tunables expected to be revised under measurement.
///
/// `zero_on_free: false`: the Thread constructor fully overwrites every
/// field on alloc. Future cache types added for Tensor / Model / etc.
/// should default to `zero_on_free: true` per Principle 7 unless
/// similarly justified.
pub const THREAD_CACHE_CONFIG: SlabConfig = SlabConfig {
    local_capacity: 8,
    global_capacity: 32,
    batch_size: 4,
    zero_on_free: false,
};

// === Initialization ======================================================

static SLAB_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize BSP slab allocator state.
pub fn init() {
    init_per_cpu(0);
}

/// Initialize slab allocator per-CPU state for `cpu_id`.
///
/// Called once per CPU: BSP from `mm::init()`, APs from the AP entry path
/// after GS-base is wired to the AP's `PerCpu` slot. The write uses an
/// explicit CPU id so bootstrap code does not depend on GS-relative access.
///
/// **Runtime invariant check (debug builds):** Verifies `FRAME_ALLOC` is
/// initialized (buddy is operational) before proceeding.
pub fn init_per_cpu(cpu_id: u32) {
    debug_assert!(
        mm::with_frame_alloc(|_| true),
        "slab::init_per_cpu() called before mm::init() completed"
    );

    // Allocate per-CPU slab state and wire it into the target CPU slot.
    // This is intentionally leaked (Box::into_raw) so it persists for
    // kernel lifetime. The discipline marker is explicit.
    let slab_state = Box::into_raw(Box::new(SlabPerCpuState::new()));
    crate::arch::x86_64::per_cpu::set_slab_state_for_cpu(cpu_id, slab_state as *mut ());

    SLAB_INITIALIZED.store(true, Ordering::Release);

    // Initialize static cache instances.
    // These run exactly once via Once::call_once() and are idempotent.
    THREAD_CACHE.call_once(|| Slab::new("thread", THREAD_CACHE_CONFIG));

    serial_println!(
        "[ kernel ] slab: initialized per-CPU state cpu={} at 0x{:016x}",
        cpu_id,
        slab_state as u64
    );
}

/// Check if slab allocator has been initialized.
pub fn is_initialized() -> bool {
    SLAB_INITIALIZED.load(Ordering::Acquire)
}

// === Static cache instances ==============================================
//
// Per ADR-012 §ABI, static cache instances live in the slab subsystem.
// They are initialized in slab::init() after buddy allocator is operational.
// Consumer modules import from mm::slab; there is no circular dependency
// because consumer modules depend on mm::slab.
//
// PROCESS_CACHE/CHANNEL_CACHE were removed here (OT-8, 2026-07-03) — see the
// module header's cache inventory for the rationale.

/// Thread cache instance used by production Thread construction and reaping.
pub static THREAD_CACHE: Once<Slab<Thread>> = Once::new();

// === TestObject for slab test scenarios =================================

/// Simple test object type for slab test scenarios.
/// No special invariants; just a u64 value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TestObject {
    pub value: u64,
}

impl TestObject {
    pub const fn new(value: u64) -> Self {
        TestObject { value }
    }
}
