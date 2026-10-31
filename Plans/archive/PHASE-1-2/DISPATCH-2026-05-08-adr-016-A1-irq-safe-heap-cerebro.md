# Dispatch: ADR-016 A1 — IRQ-safe global heap allocator

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Implementation dispatch. Implements Decision 2 of `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` (the IRQ-safe heap wrapper, "A1 defense-in-depth"). This dispatch presupposes the Bible amendment 8.5 commit has landed (per `Plans/DISPATCH-2026-05-08-bible-amendment-8-5-commit-cerebro.md`); A1 is the conformance work for newly-binding Article 8.5.1.
**Substrate:** Project HEAD will be the bible-amendment commit (one commit ahead of `3b3b4db`). Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **Single focused commit.** Touches `kernel/src/mm/heap.rs` and (likely) `kernel/src/mm/mod.rs` for any module-export adjustments. No other files unless build necessities force otherwise.
2. **No stash interactions.** Stashes remain untouched.
3. **Build verification before commit.** `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings).
4. **Backup runs after commit lands.** `Plans/backup-to-synology.ps1`. Three-stash preservation handled by the 2026-05-08-refined backup mechanism.
5. **No functional verification on this commit.** A1 alone does not close the wedge — that's A2's job. A1 is defense-in-depth: the wedge can stay reproducible under A1-alone, and that's expected. Verification of the wedge fix happens at ADR-016 Phase 6 after A2 lands. Build verification (item 3) is sufficient for this commit.
6. **No INDEX.md update in this commit.** Saved for the next sweep.

---

## 1. Goal

Replace the global heap allocator's plain spinlock with an IRQ-safe wrapper that disables local interrupts during lock acquisition and restores prior interrupt state on release. Concretely, the new `static ALLOCATOR: ...` enforces Article 8.5.1's requirement structurally for the global heap.

The change preserves the existing heap allocator (`linked_list_allocator::Heap`); it adds an outer wrapper that handles the IRQ-safety dance. After this commit:

- Any `alloc::alloc::alloc(...)` / `dealloc(...)` call kernel-wide pays a single-digit-cycle local-IRQ-disable cost.
- An ISR that fires during a heap operation cannot deadlock against the interrupted heap-holder (because the heap-holder ran with interrupts disabled, so no ISR could fire mid-critical-section).
- An ISR that itself allocates against Article 8.5.2's prohibition (a violation, but possible if a future contributor errs) does not produce same-CPU heap-lock deadlock — the wrapper makes the violation non-fatal at the heap layer.

A1 does NOT close the wedge. The wedge is closed by A2 (separate dispatch, separate timeline) which decouples the scheduler from the heap entirely. A1 is defense-in-depth, structural enforcement of 8.5.1 for the heap layer specifically.

---

## 2. Required changes

### 2.1 New wrapper type around `linked_list_allocator::Heap`

Replace the existing `static ALLOCATOR: LockedHeap = LockedHeap::empty()` in `kernel/src/mm/heap.rs` with a new wrapper type — name suggestion: `IrqSafeHeap` — that holds an inner `linked_list_allocator::Heap` (the unlocked variant, NOT `LockedHeap`) under our own spinlock, and that performs IRQ-disable before lock acquisition.

The wrapper exposes:

- `pub const fn empty() -> Self` for static initialization at the `#[global_allocator]` site.
- `pub unsafe fn init(&self, start: *mut u8, size: usize)` matching the existing API used by `kernel/src/mm/mod.rs` (or wherever heap init is called from). The `unsafe` is on the caller, who guarantees the memory range is exclusive and valid for heap use.
- `unsafe impl GlobalAlloc` for the wrapper, with `alloc` and `dealloc` (and any other methods the existing trait impl provided — typically just those two).

Implementation sketch (you may refine):

```rust
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::Heap;
use spinning_top::Spinlock;
use crate::arch::x86_64::without_interrupts;

pub struct IrqSafeHeap {
    inner: Spinlock<Heap>,
}

impl IrqSafeHeap {
    pub const fn empty() -> Self {
        Self { inner: Spinlock::new(Heap::empty()) }
    }

    /// SAFETY: caller guarantees the [start, start+size) range is exclusive,
    /// valid for read/write, and remains so for the kernel's lifetime. Init
    /// runs with interrupts disabled so concurrent ISR access cannot observe
    /// a partially-initialized heap.
    pub unsafe fn init(&self, start: *mut u8, size: usize) {
        without_interrupts(|| {
            // SAFETY: the closure holds the lock for the duration of init;
            // caller's contract on start/size satisfies linked_list_allocator's
            // init preconditions.
            self.inner.lock().init(start as usize, size);
        });
    }
}

unsafe impl GlobalAlloc for IrqSafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        without_interrupts(|| {
            self.inner
                .lock()
                .allocate_first_fit(layout)
                .ok()
                .map_or(core::ptr::null_mut(), |p| p.as_ptr())
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: caller of dealloc upholds GlobalAlloc's contract that ptr was
        // obtained from a prior alloc on this allocator with the same layout.
        without_interrupts(|| {
            self.inner
                .lock()
                .deallocate(NonNull::new_unchecked(ptr), layout)
        })
    }
}

#[global_allocator]
static ALLOCATOR: IrqSafeHeap = IrqSafeHeap::empty();
```

Key points:

- `without_interrupts` is the existing project primitive in `kernel/src/arch/x86_64/...` — it does the `pushfq` / `cli` / `popfq` save-and-restore dance. Reuse it; do NOT introduce raw `cli` / `sti`.
- The lock is acquired INSIDE the `without_interrupts` closure. Acquisition order matters: IF=0 first, then take lock. The wrapper must not yield, allocate, or call out between `without_interrupts` entry and the lock acquisition.
- `Spinlock::new(Heap::empty())` is `const fn` — verify on the version of `spinning_top` we're on (`spinning_top` 0.3.x is `const fn`; older versions are not). If our pinned version doesn't support const construction, fall back to a `OnceCell`-style lazy init, but the cleaner path is the const ctor.
- `Heap` (not `LockedHeap`) is the inner allocator. `linked_list_allocator` exposes both; we want the unlocked one because we provide our own lock.

If `spinning_top::Spinlock::new` is not `const fn` in our pinned version, alternative approaches in priority order:
1. Switch the inner lock to `spin::Mutex` (Cargo.toml shows `spin` is already an existing dependency — same lock semantics, has `const fn new` since 0.9).
2. Use a `Once` / `OnceCell` lazy initializer. Less clean.
3. Bump `spinning_top` if a newer version has `const fn new` — only if (1) and (2) are unworkable.

Document the choice in the commit message body.

### 2.2 Update heap init call site

The existing heap init in `kernel/src/mm/mod.rs` (or wherever `ALLOCATOR.lock().init(...)` is currently called) must be updated to the new API. Current call shape (likely):

```rust
unsafe { ALLOCATOR.lock().init(start, size); }
```

New call shape:

```rust
// SAFETY: heap region [start, start+size) is exclusive and valid for the kernel's lifetime.
unsafe { ALLOCATOR.init(start, size); }
```

The `IrqSafeHeap::init` method internally handles interrupt-disable and lock acquisition, so the caller no longer needs `.lock()`.

### 2.3 SAFETY comments per Article 15.2

Every `unsafe` block introduced or modified in this commit must carry a `// SAFETY:` comment explaining the invariant being upheld. The sketch above includes them. Audit the final code; CI fails on missing SAFETY comments per project pattern.

### 2.4 Module visibility / exports

If `IrqSafeHeap` is the new global allocator type, it likely doesn't need to be `pub` outside the `mm::heap` module — only the `static ALLOCATOR` matters externally, and that's `#[global_allocator]`-attributed. Keep visibility minimal. If existing code references `LockedHeap` as a type elsewhere (search with `git grep LockedHeap`), update those references accordingly — but expect zero or one such references (heap is usually self-contained).

### 2.5 No other changes

Do NOT:

- Replace `linked_list_allocator` itself. Out of scope.
- Add new heap features (statistics, debug tracing, alternative strategies). Out of scope.
- Modify scheduler internals. That's A2 territory, separate dispatch.
- Touch any other `kernel/src/...` subsystem.

If during implementation you discover a project-specific constraint that makes the sketch impractical (e.g., `Spinlock::new` const issue, an unexpected heap initialization shape, a missing trait), surface it; do not power through with a workaround that increases complexity.

---

## 3. Commit hygiene

Subject:

```
mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)
```

Body must include:

- One-paragraph summary of what changed (`LockedHeap` → `IrqSafeHeap` wrapper).
- Reference to ADR-016 and Bible Article 8.5.1 as governing.
- Note that this is A1 (defense-in-depth); the wedge fix proper is A2 (separate work).
- Documentation of the inner-lock choice: `spinning_top::Spinlock` vs `spin::Mutex` per §2.1, with the reason (const-fn support, dependency hygiene).
- Note about overhead: every kernel allocation now pays one local-IRQ-disable cycle (~single-digit cycles on modern x86_64); below measurement noise for typical kernel workloads.
- Reference to `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` for the bug class this defense-in-depth addresses.

After commit lands, `Plans/backup-to-synology.ps1` runs. Verify the log shows `Stashes to preserve: 3` and the new HEAD SHA recorded. The backup must capture this commit + the prior bible-amendment commit + the historical commits.

---

## 4. Pre-commit checklist

- [ ] `kernel/src/mm/heap.rs` viewed in editor; new `IrqSafeHeap` type reads correctly; no references to old `LockedHeap` remain.
- [ ] Heap init call site (likely `kernel/src/mm/mod.rs`) updated to new API.
- [ ] `git grep -i LockedHeap` returns no matches in source code (it may match in vendored crate sources under `target/` or in lockfile entries — those are fine).
- [ ] All new/modified `unsafe` blocks have `// SAFETY:` comments.
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline. No new warnings.
- [ ] `git diff --stat` shows changes confined to `kernel/src/mm/` and possibly `Cargo.toml`/`Cargo.lock` if a dependency note changed (see §2.1 fallbacks).
- [ ] `git status` clean except for staged changes.

---

## 5. No verification loop on this commit

A1 is defense-in-depth. The wedge mechanism (`POST_SMOKE_SMP_SCENARIO`) is enabled by scheduler-mediated heap lock re-entry; A1 makes that deadlock impossible at the heap layer, BUT the wedge can also reach the heap from the scheduler at lower probability through other paths that A2 eliminates. Running 200 boots after A1 alone is expected to show some residual rate; that is not a verification failure; that's the expected substrate for A2 to address.

Verification of the wedge fix as a whole happens after A2 lands, per ADR-016 Phase 6. This commit's deliverable is correct A1 implementation + clean build. Nothing more.

If you observe wedge-class behavior IMPROVING dramatically under A1 alone (e.g., 0/50 instead of historical ~2.67%), that's noteworthy — surface it in the commit body or in a follow-up note. It would suggest the proven mechanism's vulnerability window was narrower than the audit's per-call-site enumeration suggested. But do not run 200 boots to chase the question; A2 is coming and will resolve the bigger picture.

---

## 6. Cross-references

- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` — Decision 2 (A1) is what this dispatch implements.
- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` — Article 8.5.1 is the policy this implementation satisfies for the heap layer.
- `Plans/DISPATCH-2026-05-08-bible-amendment-8-5-commit-cerebro.md` — the prior commit that this commit's substrate depends on.
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the audit identifying the heap-lock-re-entry class.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — prior art on the wrapper pattern (Theseus `irq_safety`, Rust-for-Linux `lock_irqdisable`, Linux `spin_lock_irqsave`).
- `Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` — project-state record of the ratification.
- Bible Article 4.3 — every external dependency is a liability; in-house implementation preferred. This dispatch implements in-house (no `irq_safety` crate adoption).
- Bible Article 15.2 — `unsafe` blocks require SAFETY comments.

---

**End of dispatch.**

*One commit. `kernel/src/mm/heap.rs` (and minor call-site update). ~30-50 lines plus SAFETY comments. Defense-in-depth structural enforcement of Article 8.5.1 for the global heap. The wedge does NOT close on this commit; that's A2's job. PM is on call for any constraint that surfaces during implementation.*
