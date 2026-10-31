<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-067: Fix the thread-slab magazine stride bug (OT-29)

**Status:** Accepted
**Date:** 2026-06-22
**Dispatch:** `Plans/DISPATCH-2026-06-22-sched-thread-slab-spawn-reap-spawn-ub.md`
**Scope:** A two-line bug fix in the slab per-CPU magazine, plus a permanent SMP
regression and a debug-gated slab free-list discriminator. **Restores** the existing
thread-slab free-list-integrity invariant; no new invariant, no kill-switch, no
allocator rewrite.

> **Naming note.** The dispatch (`…-spawn-reap-spawn-ub`) and OT-29 were framed around an
> SMP exit-vs-reap use-after-free hypothesis. The instrument refuted that; the real defect
> is the magazine indexing bug documented below, hence this ADR's `…-magazine-stride` slug.
> The ADR number is **067** because Phase 9's ADR-066 (forward mixed-quant) landed first.

## The bug

At `NEO_SMP=4`, a **heavy** first process (Arthur: ChaCha20 + multiple file writes) that
EXITs, is REAPED, and is followed by more spawns panics in `Thread::alloc_from_slab`
(`kernel/src/sched/thread.rs:135`) with a `ptr::write` to a null/misaligned pointer. The
lighter M1 `arthur_memory` process does not trip it. Surfaced by M2's falsifiable scenario
(QEMU run 27926197328, shard 4); M2 shipped a single-spawn workaround, leaving the bug live.

**Root cause — a stride bug in `Magazine` indexing (`kernel/src/mm/slab.rs`).** The per-CPU
magazine is a ring of `*mut T` element pointers. Its backing store is allocated as
`Layout::array::<*mut T>(capacity)` (an array of *pointers*) but the field is typed
`buffer: UnsafeCell<*mut T>`. `push`/`pop` then indexed it as

```rust
ptr::write((*self.buffer.get()).add(tail) as *mut *mut T, obj);   // push
let obj = ptr::read((*self.buffer.get()).add(head) as *const *mut T); // pop
```

`(*self.buffer.get())` is `*mut T`, so `.add(tail)` strides by **`size_of::<T>()`**, not by
the pointer size. The allocation is only `capacity * size_of::<*mut T>()` bytes, so for any
`tail >= 1` the write lands **out of bounds**, scribbling adjacent heap; a later `pop` at
`head >= 1` reads that garbage and returns a bogus "object pointer" (observed: `0x5`).
`Slab::alloc` wraps it `Box::from_raw`, and `alloc_from_slab`'s `ptr::write` faults — or,
when the garbage is a small value, it lands in the intrusive global free-list as a corrupt
head (`slab[thread]: FREE-LIST HEAD CORRUPT node=0x5`).

**Why it hid for so long, and why "heavy/SMP" was a red herring.** The slab unit tests
exercise the magazine only with `TestObject` (a `u64`, 8 bytes). For an 8-byte `T`,
`size_of::<T>()` *equals* the pointer size, so the wrong stride is accidentally correct and
the tests pass. The three production caches (`Thread`/`Process`/`Channel`) are all large, so
the bug is live for them — but only manifests once **≥ 2** large objects are cached in the
magazine at once (`tail`/`head` advance past 0). Light, single-spawn flows keep the magazine
at 0–1 entries and never trip it; heavy/many-spawn flows (the M2 Arthur load; this
regression's 16-round loop) reach ≥ 2 cached threads reliably. The correlation with SMP in
the original sighting was incidental (M2 ran at `NEO_SMP=4`); the defect is not a race.

This refuted the dispatch's lead hypothesis (an SMP exit/reap use-after-free) and a
matching independent analysis (Forge). A trial fix that re-ordered `exit()`'s lock release
was applied and **the regression failed identically** (`node=0x5`, fix-independent) — which
is exactly how the misdiagnosis was caught and reverted. The lesson: the cheap discriminator
beat the sophisticated theory.

## The fix

Cast `buffer` (`*mut T`) to `*mut *mut T` / `*const *mut T` **before** `.add(index)` in
`Magazine::push` and `Magazine::pop`, so the stride is the pointer size. Two lines; the
allocation, the field, `len`/`is_full`, and `Drop` are unchanged. No allocator rewrite, no
scheduler change, no new `unsafe`.

## The discriminator (ISC-2) + the regression (ISC-1/ISC-4)

- **`slab-debug-poison`** (`kernel/src/mm/slab.rs`, default-off, zero production cost):
  on free, `[8, size)` of a node is sentinel-filled (offset 0 is the intrusive link); on a
  cached alloc the node is asserted non-null, aligned, and still fully sentinel. This turns
  the downstream `ptr::write` fault into a one-hop-upstream verdict — here, `FREE-LIST HEAD
  CORRUPT node=0x5`, which (with the fix-independence of the exit() trial) pinned the defect
  to the slab, not the scheduler. Kept as paid-down debt (the regression exercises it).
- **`sched_spawn_reap_spawn_heavy`** (a permanent neo-test scenario, armed forever): a
  `neo-init` driver loops heavy-Arthur spawn → exit → reap → spawn ×16 at `NEO_SMP=4` with
  the poison instrument compiled in. **Before the fix:** `FREE-LIST HEAD CORRUPT node=0x5`,
  4/4 attempts. **After the fix:** `event=PASS`. Conclusive on both pass and fail.

## Consequences

- The thread-slab magazine is correct for all object sizes; heavy spawn→reap→spawn is sound
  at SMP — unblocking Stage-B (a real heavy Arthur) on bare metal.
- No new cross-cutting invariant; no kill-switch (a bug fix, not a policy layer).
- **Flagged follow-ons (not in this fix's scope):**
  1. The slab's own unit tests use only an 8-byte `TestObject`, which masked this stride
     bug. A host-tier magazine test with a large (`> 8 B`) element type is a cheap
     regression that would have caught it directly.
  2. The dispatch's exit-vs-reap window (`exit()` drops the scheduler lock before the asm
     `rsp`-write, while the BSP reaper frees outside the lock) is a *theoretical* SMP hazard
     the IPC block paths already guard via `context_switch_with_release2`. It was **not** the
     cause here and is unconfirmed; left as a candidate follow-on rather than a speculative
     change.
