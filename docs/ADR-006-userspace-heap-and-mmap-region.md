# ADR-006: Userspace heap via `sys_map_region` + Pmap interior mutability

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-03 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 1.4 (POSIX disagreement) · Article 7.4 (allocator phases) · Article 12.3 (W^X) · Article 13 (Phase 1.5 scope) · ADR-004 (IPC channels — handle pattern this composes with) · ANSPEC-001 §8.3 (Phase 5 `tensor_map`, structurally identical) |

---

## Context

Phase 1.5 ships "userspace heap" per Bible Article 13. Phase 5+ (per ANSPEC-001 §8.3) introduces `tensor_map`: kernel allocates frames + maps into a process's pmap + returns a user VA + handle. The Phase 1.5b heap-growth primitive should compose with that — not be a one-off design Phase 5 has to replace.

Current state going into BUILD-4:

- ELF loader places binary segments at fixed VAs from the binary's program headers (typically starting at `0x400000`).
- User stack is mapped at `USER_STACK_TOP = 0x0000_7fff_ffff_f000` (16 KiB, growing down).
- `Process.pmap: Option<Arc<Pmap>>` since BUILD-3 (IPC needs `Arc<Pmap>` to translate sender buffers at rendezvous time). `Pmap::translate(&self)` is a read-only HHDM walk; `Pmap::map_user(&mut self, ...)` requires unique borrow, which is incompatible with `Arc<Pmap>` post-construction.
- No userspace heap, no `Box`/`Vec`/`String` in `neo-init` / `neo-worker`.

Four design axes drive BUILD-4. Each is decided independently below.

## Decision

**Q1 — kernel growth primitive: `sys_map_region(size) → user_va`** (mmap-style anonymous).

Each call returns a fresh kernel-allocated user-half region. The userspace allocator (libneo's `LibneoHeap`) consumes one or more regions. No `munmap` in Phase 1.5b — regions persist for process lifetime; `Pmap::Drop`'s walker frees all user-half frames at process exit (already implemented in Phase 1.5a's BUILD-1).

| Alternative | Rejected because |
|---|---|
| POSIX `brk` / `sbrk` | 1970s artifact — single contiguous data segment, no separation between allocator pages and other allocations. Doesn't compose with Phase 5 `tensor_map` which needs the same primitive shape (`(size) → user_va + handle`). Would have to be deleted-and-replaced. Bible 1.4 license to disagree, applied. |
| Fixed heap region at spawn, no growth | Hardcoded ceiling. Doesn't lay `tensor_map` groundwork. Userspace allocator has to know its size at link time. |

**`sys_map_region` is the same shape Phase 5 `tensor_map` will take** — kernel allocates frames, maps into pmap, returns VA. Phase 5 extends with a tensor handle and tensor-storage flags; the underlying primitive is unchanged.

**Q2 — heap region placement: kernel-chosen via per-Pmap cursor.**

Each `Pmap` carries `next_mmap_va: u64` (under interior-mutability lock — see Q4). Cursor starts at `USER_HEAP_BASE = 0x0000_4000_0000_0000` — well clear of binary segments (typically `0x400000`) and the user stack (top at `0x0000_7fff_ffff_f000`, growing down). Each `sys_map_region(size)` rounds size up to a page multiple, returns the current cursor as the region's base VA, and advances the cursor by the rounded size.

| Alternative | Rejected because |
|---|---|
| Heap right after binary's data segment | Couples heap layout to ELF layout; brittle when binaries get bigger; ELF loader has to communicate the data-segment-end VA to the heap subsystem. |
| Fixed VA per process | Inflexible; pre-commits address-space layout in a way that hurts when multiple region kinds (heap, tensor, future shm) coexist. |

The `0x0000_4000_0000_0000`–`0x0000_7000_0000_0000` window gives ~64 TiB of growth before the cursor approaches the user stack region. Bounds-check rejects requests that would cross `USER_HEAP_LIMIT = 0x0000_7000_0000_0000`.

**Q3 — userspace allocator: `linked_list_allocator::Heap` (unlocked variant).**

Userspace is single-threaded for Phase 1.5b, so the locked `LockedHeap` would only add overhead. libneo wraps the `Heap` in a `spin::Mutex` itself (for the GlobalAlloc trait's `&self` requirement) — that mutex is the per-process userspace allocator lock. It is not the kernel's heap lock; the kernel's `LockedHeap` is a separate, unrelated lock.

| Alternative | Rejected because |
|---|---|
| `buddy_system_allocator` | Better amortized allocation behavior, but ~3× more code. Phase 1.5b workloads are small (control-plane messages, small `Vec`s); linked-list is fine. Phase 2 can swap if measurement says it's needed. |
| Custom small-allocation allocator | No measured perf problem yet. Premature. |

**Q4 — Pmap mutability post-`Arc::new`: interior mutability inside `Pmap`.**

`Process.pmap: Option<Arc<Pmap>>` (BUILD-3 form) precludes `&mut Pmap`. `sys_map_region` needs to mutate the pmap (allocate frames, install mappings, advance the cursor) AFTER it's wrapped in `Arc`. The chosen shape:

```rust
pub struct Pmap {
    pml4_frame: PhysFrame<Size4KiB>,
    inner: Mutex<PmapState>,
}

struct PmapState {
    next_mmap_va: u64,
}

impl Pmap {
    pub fn translate(&self, va: VirtAddr) -> Option<PhysAddr> {
        // Read-only walk through HHDM-mapped page tables.
        // CRITICAL: must NOT touch self.inner. The IPC hot path
        // (cross_pmap_copy) calls translate per page; growing a lock
        // through this borrow chain would defeat the whole point of
        // interior mutability inside Pmap.
    }

    pub fn map_user(&self, va, frame, flags) -> Result<(), PmapError> {
        // ... checks ...
        let _g = self.inner.lock();
        self.map_user_inner(va, frame, flags)
    }

    pub fn allocate_user_region(&self, size, flags) -> Result<u64, PmapError> {
        let mut g = self.inner.lock();
        let va = g.next_mmap_va;
        // ... advance cursor, allocate frames, map each via map_user_inner ...
    }
}
```

| Alternative | Rejected because |
|---|---|
| `Arc<Mutex<Pmap>>` everywhere | Disruptive — every IPC `translate` call site grows a `pmap.lock()`; self-IPC (process talking to itself) deadlocks on recursive lock. |
| `Arc<RwLock<Pmap>>` | Better than Mutex for the read/write asymmetry, but still touches every translate site, and recursive-reader semantics in spin's RwLock are subtle enough to rule out for a hot path. |

**The `translate(&self)` / `map_user(&self)` split is the load-bearing distinction.** `translate` is pure read of HHDM-mapped page tables — no allocation, no mutation, no lock needed. `map_user` and `allocate_user_region` mutate page tables; they take the inner lock. The IPC hot path uses only `translate` and never sees the mutex.

## Consequences

### Positive

- **`sys_map_region` is forward-compatible with Phase 5 `tensor_map`.** Same primitive shape; Phase 5 extends with tensor handles + flags, doesn't replace.
- **Userspace gets `Box` / `Vec` / `String`.** libneo's GlobalAlloc impl unlocks the standard `alloc::*` ergonomics for every userspace program. IPC payloads can now be built dynamically rather than statically sized.
- **IPC hot path unchanged.** `Pmap::translate(&self)` is the IPC hot path's single Pmap call site; the interior-mut refactor leaves it lock-free.
- **`Pmap::map_user` becomes `&self`-callable from any context.** `sys_map_region` from a user syscall, the ELF loader during spawn, future `tensor_map` — all use the same API.

### Negative

- **One more spin::Mutex per process.** Held briefly during `map_user` and `allocate_user_region`. Single-CPU + IRQs-off path during BUILD-4 syscalls; Phase 2 SMP is the SMP retrofit point.
- **`linked_list_allocator::Heap::extend` requires adjacent regions.** Phase 1.5b only has one region-allocating syscall (`sys_map_region`), and it always uses the per-Pmap cursor monotonically, so successive calls produce adjacent regions. Phase 5+ may introduce other region-allocating primitives (tensor_map, shm) — those will need their own cursor or a real region tracker, and the heap extension contract will need re-examining. Documented in libneo's `LibneoHeap` doc comment.

### Neutral

- **`Pmap::new` no longer needs `mut`.** All existing `&mut Pmap` callers (`elf::load_into`, `elf::allocate_user_stack`, `process::spawn_user`) update to `&Pmap`. Mechanical rename; no semantic change.
- **No `munmap` in Phase 1.5b.** Process exit handles all cleanup via `Pmap::Drop` (BUILD-1's two-pass walker). A standalone `sys_unmap_region` could land later if a real caller needs it.
- **`#[global_allocator]` declared in libneo, inherited by every linker-using binary.** Forecloses per-binary allocator choice, which is fine for Phase 1.5b's two binaries (`neo-init`, `neo-worker`) that both want the same `LibneoHeap`. When Phase 5's inference subsystem introduces a tensor-aware allocator, libneo should grow a `default-features = ["global-alloc"]` Cargo feature so consumers can opt out without forking. Tracked here, not implemented in this commit (no consumer needing the opt-out yet).

## Implementation

### `kernel/src/mm/pmap.rs`

- Add `inner: Mutex<PmapState>` field, `PmapState { next_mmap_va: u64 }` struct.
- `Pmap::new()` initializes cursor to `USER_HEAP_BASE`.
- `translate(&self) -> Option<PhysAddr>` unchanged signature, **explicitly does not touch `self.inner`**.
- `map_user(&self, …) -> Result<…>` (was `&mut self`): acquires `inner.lock()`, calls `map_user_inner` (the page-table walk).
- `map_user_inner(&self, …)` private: assumes lock held; performs the walk.
- `allocate_user_region(&self, size, flags) -> Result<u64, PmapError>` new: acquires lock, takes cursor, advances cursor, allocates frames, maps each, returns base VA.

### `kernel/src/loader/elf.rs`

- `load_into(&self, pmap: &Pmap)` (was `&mut Pmap`).
- `allocate_user_stack(pmap: &Pmap, ...)` (was `&mut Pmap`).

### `kernel/src/sched/process.rs`

- `spawn_user`: drop `mut` from `let mut pmap = Pmap::new()?;`.

### `kernel/src/arch/x86_64/syscall.rs`

- `SYS_MAP_REGION = 6`.
- `sys_map_region_impl(size: usize) -> i64`: validates size, looks up caller's pmap, calls `allocate_user_region`, returns user VA or negative errno.

### `userland/libneo`

- Add `linked_list_allocator` and `spin` to `Cargo.toml` dependencies.
- `pub fn sys_map_region(size: usize) -> Result<*mut u8, i64>` wrapper.
- `pub struct LibneoHeap` implements `GlobalAlloc`. Internal `spin::Mutex<linked_list_allocator::Heap>`. Lazy bootstrap (first alloc requests an initial region of `BOOTSTRAP_HEAP_SIZE = 256 KiB`); on alloc failure within the existing region, requests another region of `max(layout.size(), GROWTH_CHUNK)` rounded up, calls `Heap::extend`. Failed `sys_map_region` returns null.
- `#[global_allocator] static HEAP: LibneoHeap = LibneoHeap::new();`

### Userspace demo

- `neo-init` builds an `alloc::vec::Vec<u8>` payload of meaningful size (e.g., a heap-allocated greeting message), sends it via the existing IPC channel.
- `neo-worker` receives into a heap-allocated `Vec<u8>`, prints the payload.

## Verification (filled in as the BUILD-4 commit lands)

- [x] `Pmap::translate` signature unchanged (`&self`); body verified to NOT reference `self.inner`.
- [x] `sys_map_region(4096)` returns a writable user-half VA; userspace can write/read; data persists.
- [x] `LibneoHeap` lazy bootstrap on first `Box::new`; subsequent allocations reuse the region.
- [x] Heap extension via second `sys_map_region` call works (provoke by allocating > BOOTSTRAP_HEAP_SIZE).
- [x] IPC payload built from heap-allocated `Vec<u8>` round-trips through the channel.
- [x] Phase 1.3 ping/pong/hog smoke still passes (anti-regression).
- [x] Phase 1.5a Pmap::Drop walker still freed for both processes after exit.
- [x] Boot completes through framebuffer + halt with no faults.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
