# DISPATCH: SC-3B Sub-step 6 — Docs closure (Prometheus, six-commit sequence)

**Date:** 2026-05-10
**Routed to:** Prometheus
**Type:** Multi-commit docs work — six logically grouped commits covering SC-3B post-closure carry-forwards
**Sequencing:** Post-`Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`. Independent of kernel-side rename dispatch (which is a separate small dispatch following this one).
**Coordination constraint:** Cerebro is between Phase 2 BUILD-3 commits; may start next sub-commit work at any time. `kernel/`, `boot/`, `tools/` are OFF-LIMITS to this dispatch.
**Total scope estimate:** ~2-3 hours of careful execution. Commit boundaries are explicit; verification before each commit.

---

## 1. Background and scope

`SC-3B` (Phase 2 BUILD-3 sub-commit 3) closed on 2026-05-10 via commit `e2b3f4f` (Sub-step 5c — IRQ-safe acquisition for `Channel.state`). The closure report at `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` declared §8 carry-forwards for documentation closure. This dispatch executes those carry-forwards.

**Six logical commits:**

| Commit | Subject | Files |
| --- | --- | --- |
| 1 | Three ADR proposals filed in Plans/ | 3 new files in Plans/ |
| 2 | ADR-013 amendments (SC-3B closure markers) | 1 modified file in docs/ |
| 3 | Bible v1.4.0 (Article 16.4 bench-surface expansion) | 1 modified file in docs/ |
| 4 | Redox DWRR PM-NOTE commit (file already on disk) | 1 new file in Plans/ (already written by PM) |
| 5 | PHASE-2.md update + INDEX.md sweep | 2 modified files |
| 6 | README.md rewrite (alignment with bible v1.3.0/v1.4.0) | 1 modified file at project root |

After all six commits land: `git push origin main`.

---

## 2. Coordination constraints

### 2.1 Negative-list (DO NOT TOUCH)

- ANY file under `kernel/`
- ANY file under `boot/`
- ANY file under `tools/`
- `Cargo.lock`, root or any `Cargo.toml`
- `ALTIMIT-real-world-implementation.md` (research artifact; preserve unchanged)
- ANY existing file in `Plans/` other than:
  - INDEX.md (commit 5)
  - The new files this dispatch creates (commits 1 and 4)

### 2.2 Cerebro coordination

Cerebro may start next Phase 2 BUILD-3 sub-commit work at any time. Their work will touch kernel files. If at any point during this dispatch Prometheus observes uncommitted changes in `kernel/`, `boot/`, or `tools/` (other than from Cerebro's WIP visible in `git status`), STOP and request PM clarification.

Default behavior: stage only the docs/Plans/README files this dispatch specifies. Never use `git add -A` or `git add .` — always specify file paths explicitly.

### 2.3 Pre-flight check

```bash
cd /e/AI\ Automation/AltimitNeo
git rev-parse HEAD                          # expect cca6889 or successor (if Cerebro has committed since)
git log --oneline -5                        # confirm post-rename state
git status --porcelain | head -20           # survey current uncommitted state
test -f Plans/CLOSE-REPORT-2026-05-10-sc-3b.md && echo "OK: CLOSE-REPORT exists"
test -f Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md && echo "OK: Redox DWRR PM-NOTE on disk"
test -f docs/ADR-017-wifi-support-strategy.md && echo "OK: ADR-017 committed"
test -f docs/ADR-013-phase-2-smp-retrofit.md && echo "OK: ADR-013 exists"
```

If any verification fails, STOP and request PM clarification.

---

## 3. Commit 1 — Three ADR proposals filed in Plans/

### 3.1 Scope

Three new files to create in `Plans/`. These are PROPOSALS awaiting project-lead ratification. The actual ADR files in `docs/` are deferred to a post-ratification follow-up commit (not in this dispatch).

Files to create:
- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md`
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`
- `Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md`

### 3.2 File 1 content — Bible 8.5.4 clarification proposal

Create `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` with the following content verbatim:

```markdown
# ADR Proposal: Bible Article 8.5.4 clarification — preempt-while-holding for mixed-IRQ-context locks

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-018 (next available; verify at ratification time).
**Bible article affected on ratification:** Article 8.5.4 (amended to explicitly name the preempt-while-holding case).

---

## Rationale

SC-3B's investigation (closed 2026-05-10 via commit e2b3f4f) named a previously-unnamed failure mode: **Hpw — preempt-while-holding-lock + IRQ-off same-CPU waiter**, producing single-CPU self-deadlock.

The current Bible Article 8.5.4 ("No blocking while holding a spinlock") conceptually covers Hpw because preemption is a form of yielding. But it does not explicitly name the preempt-while-holding case, and the failure shape — same-CPU IRQ-off waiter spinning on a lock held by a preempted thread — was non-obvious enough that the SC-3B investigation required five passes (Phases A through D plus Janus second-pass) over 36 wall-clock hours to localize.

Explicit naming in the bible prevents future reviewers from missing the discipline. The proposed amendment also references the concrete fix pattern (`without_interrupts` wrapping for mixed-IRQ-context process-side acquirers) so the rule is actionable, not just declarative.

## Proposed amendment

### Current Article 8.5.4

> 8.5.4 No blocking while holding a spinlock. A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.

### Proposed new Article 8.5.4

> 8.5.4 No blocking, yielding, or being preempted while holding a spinlock.
>
> A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.
>
> Additionally — and this is the discipline that SC-3B's Hpw mechanism (commit e2b3f4f, 2026-05-10) was caused by violating: a spinlock acquired in preemptible context, where the same lock may also be acquired in IF=0 (interrupts-disabled) context on the same CPU, MUST be acquired with local interrupts disabled by the preemptible-context acquirer. The kernel-provided `without_interrupts(...)` wrapper is the canonical pattern; type-level enforcement is the subject of a separate proposal (see ADR-019 proposal at `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`).
>
> Failing this discipline produces the **Hpw failure mode**: a preemptible-context holder is preempted by an interrupt; the same CPU then enters an IF=0 acquirer that spins on the lock; the preempted holder cannot resume because the IF=0 spinner monopolizes the CPU; the system is in a single-CPU self-deadlock that an SMP scheduler cannot route around. See `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 for the canonical analysis and the discipline's first instance.

### Footer change on ratification

Bumps bible to v1.5.0 (or whichever version is current at ratification time). The footer's `Last amended:` line records the 8.5.4 expansion.

## Compatibility with existing bible articles

- **Article 8.5 (parent)** — preserved unchanged. 8.5.4 is the specific sub-clause being amended.
- **Article 8.5.1 (Spinlock and interrupt discipline)** — preserved unchanged. 8.5.1 already addresses interrupt-context-vs-process-context locks; the new 8.5.4 wording addresses preempt-while-holding which is a different failure shape (same-CPU self-deadlock rather than two-context mutual exclusion).
- **Article 8.5.5 (Enforcement)** — preserved unchanged. Adding "structural enforcement via type system" is the subject of the separate ADR-019 proposal.
- **Article 15.2 (SAFETY comments)** — adjacent. The new discipline implies that any spinlock acquisition in preemptible context where the lock may be ISR-acquired or IF=0-acquired requires a SAFETY-comment-style justification, or use of the IrqSafeMutex type from the ADR-019 proposal.

## Implementation discipline

This amendment is descriptive (it names the discipline) rather than prescriptive new code. SC-3B Sub-step 5c (commit e2b3f4f) already implemented the discipline locally for the five affected `Channel.state` call sites. The amendment generalizes the rule kernel-wide; future code review enforces it.

The companion ADR-019 proposal proposes structural (type-system) enforcement, eliminating the need for case-by-case review. Without ADR-019 ratification, the discipline depends on manual review.

## Open questions for ratification

- **Q1:** Should the amendment also name an audit trigger — e.g., "any new `spin::Mutex` declaration in a kernel module that has both ISR and process-context callers requires a SAFETY-style comment justifying why plain `spin::Mutex` is correct"? Or is type-system enforcement (ADR-019) the right answer instead of comment-based audit?
- **Q2:** Should `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §3 and §4 be cited inline in the bible (durable cross-reference) or only in the amendment commit message? Inline citation makes the bible self-contained but adds a Plans/ reference that may age.
- **Q3:** Should this amendment be combined with ADR-019 ratification into a single commit (since they're related), or land separately?

## Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — canonical SC-3B closure record
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — the Phase D capture that localized Hpw
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus's prior-art research that confirmed Hpw as a known bug class
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — verification log for the per-call-site fix
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` — companion proposal for type-system enforcement
- `docs/bible.md` Article 8.5 (current state) — the article being amended

## Project-lead decision required

Three options:
- **Ratify as drafted.** Bible amendment lands at next post-ratification commit; ADR-018 file lands in docs/.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; no bible amendment lands.

Decision goes in the amendment commit message + the ADR-018 status field on ratification.

---

**End of proposal.**
```

### 3.3 File 2 content — SwitchMutex IRQ-off by construction proposal

Create `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` with the following content verbatim:

```markdown
# ADR Proposal: SwitchMutex IRQ-off by construction (type-level enforcement)

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-019 (next available after ADR-018; verify at ratification time).
**Related:** ADR-018 proposal at `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` (companion declarative discipline; this proposal is the structural enforcement).
**Bible articles potentially affected:** Article 15 (Code Standards) may gain a clause about lock-type discipline; Article 8.5.5 (Enforcement) updated to reference type-system enforcement.

---

## Rationale

SC-3B Sub-step 5c (commit e2b3f4f, 2026-05-10) implemented IRQ-safe acquisition as a per-call-site discipline for `Channel.state`. The discipline works — gate scores went from intermittent MISSING_TERMINAL_FRAME to 200/200 PASS — but it is enforced only by code review.

Any future code path that acquires a mixed-IRQ-context lock from preemptible context without `without_interrupts(...)` re-introduces the Hpw bug class. The ADR-018-proposed bible amendment names the discipline; this proposal makes it un-bypassable in safe Rust.

The proposal: introduce (or rename to) an `IrqSafeMutex<T>` type that disables local interrupts on acquire and restores prior interrupt state on drop. Plain `spin::Mutex<T>` remains available for locks that are exclusively process-context or exclusively ISR-context. The type system distinguishes the two cases, and a mixed-IRQ-context lock that uses plain `spin::Mutex` is a type error (or a documented exception requiring SAFETY-comment-style justification).

## Decision context

### Current state

NeoPlutarch uses `SwitchMutex` for scheduler-protecting locks and `spin::Mutex` (from the `spin` crate) for general-purpose locks. Some `spin::Mutex` instances protect data that is touched in mixed-IRQ contexts (like `Channel.state` was before Sub-step 5c).

There is no current type-level distinction between:
- (a) Locks that are exclusively process-context (no ISR touches them; no IF=0 acquirer)
- (b) Locks that are exclusively ISR-context (no process-context acquirer)
- (c) Locks that are mixed-IRQ-context (both process-context and ISR or IF=0 process-context acquirers; the dangerous case)

Hpw applies only to category (c). Category (a) and (b) locks are safe with plain `spin::Mutex`.

### Proposed state

Three distinct lock types:

- **`spin::Mutex<T>`** (unchanged) — for locks that the discipline declares to be exclusively-process-context. Use requires a documented SAFETY-comment-style justification at the declaration site naming why no ISR or IF=0-process can acquire this lock.
- **`IrqLocalMutex<T>`** (new or renamed) — for locks that are exclusively ISR-context. Acquisition assumes IF is already off (per IRQ context). Same primitive as `spin::Mutex` in mechanics, distinct in type to make the invariant visible.
- **`IrqSafeMutex<T>`** (new or renamed `SwitchMutex`) — for locks that may be acquired from BOTH process and IF=0 contexts. Acquisition disables local interrupts and saves prior IF state in the guard; guard drop restores prior IF state. Cannot be misused: the type guarantees IRQ-off-on-acquire.

### Migration

Existing `SwitchMutex` callers are audited. Each is classified into one of the three categories. The bulk are expected to become `IrqSafeMutex` (the canonical mixed-context case). Any that turn out to be exclusively-process-context move to `spin::Mutex` with SAFETY-comment justification.

`Channel.state` and similar `spin::Mutex` instances that turn out to be category (c) are converted to `IrqSafeMutex`. This removes the per-call-site `without_interrupts(...)` wrapping that Sub-step 5c added — the discipline becomes structural.

## Concrete proposal

### API sketch

```rust
// kernel/src/sync/irqsafe_mutex.rs (new file or rename of existing switch_mutex.rs)

pub struct IrqSafeMutex<T> {
    inner: spin::Mutex<T>,
    // existing ownership-metadata fields (if ADR-020 ratifies, see Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md)
}

pub struct IrqSafeGuard<'a, T> {
    guard: spin::MutexGuard<'a, T>,
    prior_if_state: bool,
}

impl<T> IrqSafeMutex<T> {
    pub fn lock(&self) -> IrqSafeGuard<'_, T> {
        let prior_if_state = read_interrupt_flag();
        disable_local_interrupts();
        let guard = self.inner.lock();
        IrqSafeGuard { guard, prior_if_state }
    }
}

impl<T> Drop for IrqSafeGuard<'_, T> {
    fn drop(&mut self) {
        // guard.drop() releases inner spinlock first, then we restore IF
        if self.prior_if_state {
            enable_local_interrupts();
        }
    }
}
```

The above is sketch-level. Real implementation needs:
- Correct ordering of `enable_local_interrupts` and `inner` guard drop (the asm sequence matters; this is exactly the territory where SC-3A and SC-3B's release-on-switch concerns lived).
- IPI-vs-local-interrupt distinction if relevant.
- Re-entrancy semantics (does `IrqSafeMutex::lock()` work if interrupts are already disabled by an outer acquirer? Probably yes, and the prior-IF tracking handles it automatically).
- Interaction with the panic path (if a panic happens with the guard held, prior IF state should be restored).

### What this removes

Once `IrqSafeMutex` lands and `Channel.state` is migrated:
- The five `without_interrupts(...)` wrappers Sub-step 5c added become redundant. They can be removed in the migration commit.
- Future code touching `Channel.state` (or any other migrated lock) cannot reintroduce Hpw — the type system enforces IRQ-off-on-acquire.

### What this leaves open

- Locks that are exclusively-process-context still need discipline (SAFETY-comment-style justification at declaration) to prevent accidental ISR acquisition that would re-introduce the dual-context case.
- The discipline still applies kernel-wide; this proposal addresses the structural enforcement for the mixed-context case specifically.

## Implementation phases

If ratified, implementation could land in three sub-phases:

1. **Sub-phase A:** Introduce `IrqSafeMutex` type alongside `SwitchMutex` (no migration yet). Verify build, basic tests.
2. **Sub-phase B:** Migrate `SwitchMutex` callers that are mixed-IRQ-context to `IrqSafeMutex`. Audit each.
3. **Sub-phase C:** Migrate Sub-step 5c's `without_interrupts(...)`-wrapped `spin::Mutex` callers (Channel.state and similar) to `IrqSafeMutex`. Remove the wrappers.

Each sub-phase is a separate commit with its own gate (200/200 NEO_SMP=4 closure-gate equivalent). Total cost is moderate: maybe 1-2 weeks of focused work depending on the migration audit's scope.

## Open questions for ratification

- **Q1:** Naming. Is `IrqSafeMutex` the right name, or should `SwitchMutex` be renamed to it? `SwitchMutex` is established convention; renaming has churn. PM lean: keep `SwitchMutex` and create `IrqSafeMutex` as a sibling; consolidate later if reasonable.
- **Q2:** Should plain `spin::Mutex` be banned from kernel code (replaced everywhere by one of the three typed mutexes), or remain available with a justification requirement?
- **Q3:** Re-entrancy semantics — should `IrqSafeMutex` be re-entrant from the same thread? PM lean: NO, re-entrancy hides bugs. Throw the entire critical section into the inner spinlock; the outer caller is responsible for not nesting.
- **Q4:** Performance — IRQ-off-on-every-lock has a small cost (one read + two writes to RFLAGS, plus possible cache effects). Quantify before committing? PM lean: probably negligible in practice; verify with a micro-bench during sub-phase A.
- **Q5:** Sequencing — should this ratify alongside ADR-018, or land separately? PM lean: ratify together since they're conceptually linked.

## Cross-references

- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` — companion declarative discipline proposal
- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record establishing the Hpw mechanism
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — Sub-step 5c verification (the per-call-site fix that this proposal would make structural)
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — prior art on release-on-switch lock primitives (Asterinas, Redox `switch_finish_hook`)
- `docs/bible.md` Article 8.5 — critical-section discipline

## Project-lead decision required

Three options:
- **Ratify as drafted.** ADR-019 file lands in docs/; implementation work begins per the three-sub-phase plan.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; the per-call-site discipline remains the only enforcement mechanism.

Decision goes in the ratification commit message + the ADR-019 status field on ratification.

---

**End of proposal.**
```

### 3.4 File 3 content — Owner metadata as production-stable diagnostic proposal

Create `Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md` with the following content verbatim:

```markdown
# ADR Proposal: Owner metadata on lock primitives as production-stable diagnostic

**Date:** 2026-05-10
**Author:** PM (drafting)
**Status:** Proposal. NOT ratified. Per Bible Article 27, amendments require project-lead approval; this draft sits in Plans/ awaiting that decision.
**Tentative ADR number on ratification:** ADR-020 (next available after ADR-019; verify at ratification time).
**Related:** ADR-018 and ADR-019 proposals (companion). This proposal is independent: it could ratify with or without ADR-018/019.
**Bible articles potentially affected:** None directly. May add a small note to Article 15 (Code Standards) or Article 16 (Testing and Verification) referencing the diagnostic discipline.

---

## Rationale

SC-3B's Phase D investigation added ownership-metadata instrumentation to `SwitchMutex` to capture the holder identity at NMI fire time. The instrumentation (cpu_id, thread_id, acquisition_site, seqlock-protected reads) was the key diagnostic signal for naming the Hpw mechanism — without it, the investigation would likely have required additional passes.

The instrumentation was reverted at Sub-step 5c (commit e2b3f4f) per the discipline that gate-passing code does not retain instrumentation. But the diagnostic value was real, and the cost (12 bytes per lock plus cache-line considerations) is small enough that production-stable owner metadata is a defensible engineering trade-off.

This proposal: ship owner metadata as a feature-gated (or always-on lightweight) field on `SwitchMutex` and possibly other lock types, with seqlock-protected reads from interrupt context for NMI-handler use.

## Decision context

### Current state

`SwitchMutex` (and `spin::Mutex`) carry no ownership information. When a deadlock or hang occurs, the NMI handler can observe lock state (locked/unlocked) but cannot identify which thread/CPU holds the lock or where the lock was acquired. Diagnostic instrumentation must be added ad-hoc for each investigation.

### Proposed state

`SwitchMutex` (and possibly `IrqSafeMutex` if ADR-019 ratifies) carries:

```rust
struct LockOwner {
    cpu: u32,                  // CPU id of the holder; u32::MAX if unowned
    thread_id: u32,            // thread id of the holder; u32::MAX if unowned or unknown
    acquisition_site: u32,     // hash of (file, line) of the acquisition; 0 if unowned
    seq: AtomicU32,            // seqlock counter for consistent reads from interrupt context
}
```

Updated atomically with lock state via seqlock discipline:
- Writer (the thread acquiring or releasing the lock) increments `seq` to an odd value before updating cpu/thread_id/acquisition_site, then increments to an even value after.
- Reader (typically NMI handler) reads `seq`, then the fields, then `seq` again; if `seq` changed or is odd, retry or treat as "inconsistent."

Total cost per lock: 16 bytes (3 × u32 + 1 × AtomicU32) plus possible cache-line padding to avoid false sharing with the lock itself.

### Feature gate semantics

Two configurations:

- **`kernel-diagnostic` cfg enabled:** Owner metadata is always present, always updated, always readable.
- **`kernel-diagnostic` cfg disabled:** Owner metadata is absent (size 0; field is `()` or behind a `cfg`-gated wrapper); no cost.

Default for development builds: `kernel-diagnostic` enabled.
Default for release builds: `kernel-diagnostic` enabled (the small cost is worth the always-on diagnostic).
Default for size-optimized embedded builds (future): `kernel-diagnostic` disabled.

The feature gate is a build-time choice, not a runtime one. Once chosen at build, the metadata is either present or absent for the entire kernel image.

## Concrete proposal

### API sketch

```rust
// kernel/src/sync/lock_owner.rs (new file)

#[cfg(feature = "kernel-diagnostic")]
pub struct LockOwner {
    cpu: AtomicU32,
    thread_id: AtomicU32,
    acquisition_site: AtomicU32,
    seq: AtomicU32,
}

#[cfg(not(feature = "kernel-diagnostic"))]
pub struct LockOwner;  // ZST

impl LockOwner {
    pub fn acquire(&self, cpu: u32, thread_id: u32, site: u32) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::Release);  // odd
            self.cpu.store(cpu, Ordering::Relaxed);
            self.thread_id.store(thread_id, Ordering::Relaxed);
            self.acquisition_site.store(site, Ordering::Relaxed);
            self.seq.fetch_add(1, Ordering::Release);  // even
        }
    }

    pub fn release(&self) {
        #[cfg(feature = "kernel-diagnostic")]
        {
            self.seq.fetch_add(1, Ordering::Release);  // odd
            self.cpu.store(u32::MAX, Ordering::Relaxed);
            // ... clear other fields ...
            self.seq.fetch_add(1, Ordering::Release);  // even
        }
    }

    /// NMI-safe read. Returns Some(snapshot) if a consistent read was achieved,
    /// or None if the lock state was being updated during the read.
    pub fn read_consistent(&self) -> Option<LockOwnerSnapshot> {
        #[cfg(feature = "kernel-diagnostic")]
        {
            let seq1 = self.seq.load(Ordering::Acquire);
            if seq1 % 2 != 0 { return None; }
            let cpu = self.cpu.load(Ordering::Relaxed);
            let thread_id = self.thread_id.load(Ordering::Relaxed);
            let site = self.acquisition_site.load(Ordering::Relaxed);
            let seq2 = self.seq.load(Ordering::Acquire);
            if seq1 != seq2 { return None; }
            Some(LockOwnerSnapshot { cpu, thread_id, site })
        }
        #[cfg(not(feature = "kernel-diagnostic"))]
        { None }
    }
}
```

### Acquisition site encoding

`acquisition_site` is a u32 hash. Options:
- Hash of (file path, line number) — computed at compile time via a macro.
- Index into a static table of acquisition sites — also macro-generated.
- Symbol-table offset — requires DWARF or similar at NMI handler time, more complex.

PM lean: macro-generated hash of (file, line). Cheap to compute (compile-time constant), no runtime table lookup needed in NMI handler.

### Integration with SwitchMutex / IrqSafeMutex

Each `SwitchMutex` instance gains a `LockOwner` field. `lock()` calls `owner.acquire(current_cpu, current_thread_id, site_hash)`. Guard drop calls `owner.release()`. NMI handler reads `owner.read_consistent()`.

If ADR-019 ratifies, `IrqSafeMutex` also gains the field. The two share the `LockOwner` implementation.

## Cost-benefit summary

**Cost:**
- 16 bytes per lock instance plus possible cache-line padding (rounded up: probably 32-64 bytes effective per lock).
- Two atomic increments on every lock acquisition and release (small; not in the lock's critical-section cost).
- Macro for site-hash generation.

**Benefit:**
- Future similar bugs (Hpw, scheduler deadlocks, etc.) are diagnosable from the NMI handler in real time — no need for ad-hoc instrumentation passes.
- The 36-hour Phase A-through-D investigation arc that SC-3B required becomes ~6 hours when the diagnostic is already present.
- Tooling can be built on top: post-hang lockmap dumps, periodic lock-state telemetry, etc.

## Open questions for ratification

- **Q1:** Feature-gate or always-on? PM lean: always-on for development and release builds; cfg-gated only for size-optimized embedded.
- **Q2:** Apply to `spin::Mutex` too, or only `SwitchMutex`/`IrqSafeMutex`? PM lean: only the kernel-owned lock types; `spin::Mutex` is an external crate that we shouldn't fork.
- **Q3:** Should the diagnostic include a hold-time stat (e.g., timestamp at acquire, reported at NMI)? Useful for finding lock-holders that are taking too long. PM lean: defer to follow-up; this proposal lands the foundational metadata only.
- **Q4:** Seqlock vs RCU vs simple ordering for the consistent-read invariant? PM lean: seqlock — it's what Phase D's instrumentation used, and it's the canonical pattern for this case.
- **Q5:** Sequencing — independent of ADR-018/019, or bundled with one of them? PM lean: independent; this proposal can ratify on its own merits regardless of the other two.

## Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record; §8.1 declares this proposal as a carry-forward
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — Phase D capture; demonstrates the diagnostic value
- `Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` — companion proposal (declarative discipline)
- `Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` — companion proposal (structural enforcement)

## Project-lead decision required

Three options:
- **Ratify as drafted.** ADR-020 file lands in docs/; implementation begins.
- **Ratify with edits.** Project lead names changes; PM redrafts.
- **Reject.** Proposal stays in Plans/ as historical record; ad-hoc instrumentation remains the diagnostic mechanism.

Decision goes in the ratification commit message + the ADR-020 status field on ratification.

---

**End of proposal.**
```

### 3.5 Stage and commit

```bash
# Verify the three new files exist and look right
ls -la Plans/ADR-PROPOSAL-2026-05-10-*.md
head -3 Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md
head -3 Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md
head -3 Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md

# Stage explicitly
git add Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md
git add Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md
git add Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md

# Verify negative-list compliance
git status --short | grep -E '^(M|A|D)' | grep -E '(kernel/|boot/|tools/)' && \
    echo "ERROR: kernel/boot/tools changes staged; STOP" && exit 1

git status --short
```

Commit message:

```
docs(plans): file three ADR proposals for SC-3B carry-forwards

Per Plans/CLOSE-REPORT-2026-05-10-sc-3b.md §8.1, SC-3B's closure
declared three follow-up ADR proposals for kernel-wide structural
prevention of the Hpw failure class:

- Bible 8.5.4 clarification (preempt-while-holding for mixed-IRQ-context locks)
- SwitchMutex IRQ-off by construction (type-system enforcement)
- Owner metadata as production-stable diagnostic

These proposals sit in Plans/ awaiting project-lead ratification per
Bible Article 27. On ratification, follow-up commits land the actual
ADR files in docs/ and the corresponding bible amendments.

Each proposal is self-contained with rationale, decision context,
open questions, and ratification options.
```

```bash
git commit -F <(cat <<'EOF'
docs(plans): file three ADR proposals for SC-3B carry-forwards

Per Plans/CLOSE-REPORT-2026-05-10-sc-3b.md §8.1, SC-3B's closure
declared three follow-up ADR proposals for kernel-wide structural
prevention of the Hpw failure class:

- Bible 8.5.4 clarification (preempt-while-holding for mixed-IRQ-context locks)
- SwitchMutex IRQ-off by construction (type-system enforcement)
- Owner metadata as production-stable diagnostic

These proposals sit in Plans/ awaiting project-lead ratification per
Bible Article 27. On ratification, follow-up commits land the actual
ADR files in docs/ and the corresponding bible amendments.

Each proposal is self-contained with rationale, decision context,
open questions, and ratification options.
EOF
)
git log -1 --stat
```

Expected: one commit adding three files to `Plans/`, nothing in kernel/boot/tools/.

---

## 4. Commit 2 — ADR-013 §Q4/§Q6/§Q7 amendments

### 4.1 Scope

Modify `docs/ADR-013-phase-2-smp-retrofit.md` to add closure markers documenting that SC-3B (the Q4/Q6 IPC retrofit) closed on 2026-05-10.

### 4.2 Spec

Read `docs/ADR-013-phase-2-smp-retrofit.md` to locate §Q4, §Q6, §Q7. For each section that is now closed:

- **§Q4 closure note:** Add a paragraph at the end stating "**Closed 2026-05-10** via SC-3B (Phase 2 BUILD-3 sub-commit 3). Closure commit: `e2b3f4f`. Sub-step 5c — IRQ-safe acquisition for `Channel.state`. Investigation arc and structural lessons recorded in `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`."
- **§Q6 closure note:** Same wording adapted to Q6.
- **§Q7 closure note:** This section was the "structural lessons" forward-look. Add a paragraph stating: "**SC-3B yielded three structural lessons proposed for kernel-wide application**, per `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` §8.1: (1) Bible Article 8.5.4 clarification explicitly naming the preempt-while-holding case (ADR-018 proposal); (2) `SwitchMutex` IRQ-off by construction via type-system enforcement (ADR-019 proposal); (3) lock owner metadata as production-stable diagnostic (ADR-020 proposal). These proposals are in `Plans/ADR-PROPOSAL-2026-05-10-*.md` awaiting ratification."

If §Q4, §Q6, or §Q7 do not exist verbatim (the section headers may differ), find the closest semantic equivalent and add the closure markers there. Document any deviation in the commit message.

If ADR-013 has a "Status" field at the top (most ADRs do), do NOT change its overall status — the ADR may still have unclosed sub-items. The amendment is sub-section closure notes, not whole-ADR closure.

### 4.3 Stage and commit

```bash
# Read to confirm structure before editing
grep -n -E "^##|^###" docs/ADR-013-phase-2-smp-retrofit.md | head -40

# After editing the three sections as specified...
git diff docs/ADR-013-phase-2-smp-retrofit.md   # review before staging
git add docs/ADR-013-phase-2-smp-retrofit.md
git status --short
git commit -m "docs(adr): ADR-013 — close §Q4 and §Q6 (SC-3B); record §Q7 structural lessons

SC-3B (Phase 2 BUILD-3 sub-commit 3) closed 2026-05-10 via commit
e2b3f4f. ADR-013's Q4 and Q6 sections (the IPC retrofit work) are
now closed. Q7 (structural lessons forward-look) receives the
three ADR proposals that SC-3B's investigation yielded.

See Plans/CLOSE-REPORT-2026-05-10-sc-3b.md for the full closure
record and Plans/ADR-PROPOSAL-2026-05-10-*.md for the proposed
follow-up ADRs."
```

---

## 5. Commit 3 — Bible v1.4.0 (Article 16.4 bench-surface expansion)

### 5.1 Scope

Amend `docs/bible.md` Article 16.4 to expand the enumerated performance-regression benches to include scheduling fairness. Bump bible version footer to v1.4.0.

### 5.2 Amendment text

Current Article 16.4:

> 16.4 Performance regressions are tracked from Phase 2 onward. Boot time, syscall latency, context-switch cost, and (eventually) inference throughput are measured per-commit on a reference VM.

Proposed replacement:

> 16.4 Performance regressions are tracked from Phase 2 onward. Boot time, syscall latency, context-switch cost, scheduling fairness (within-class weighted-share regressions and across-class priority preemption latency), and (eventually) inference throughput are measured per-commit on a reference VM. The scheduling-fairness bench is implementation-defined; the canonical reference is Redox's `schedrs` methodology adapted to NeoPlutarch's class structure. See `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` for the prior-art triage that motivated this enumeration expansion.

### 5.3 Footer update

Current footer:

```
Footer
Document: AltimitNeo Bible
Version: 1.3.0
[...]
Last amended: 2026-05-10 (Article 25.1 amended — project rename from AltimitNeo to NeoPlutarch; Article 25.2 clarified — neo- subsystem prefixes preserved as established convention. Trademark-driven rename per Janus pre-flight at Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md.)
```

Replace with:

```
Footer
Document: NeoPlutarch Bible
Version: 1.4.0
[...]
Last amended: 2026-05-10 (Article 16.4 amended — bench-surface expanded to include scheduling fairness with Redox `schedrs` as canonical reference. Prior-art triage at Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md. Earlier same-day amendments: Article 25.1 project rename, Article 25.2 prefix clarification, Article 9 expansion + Article 28 Product Accessibility, Article 9.10 WiFi Phase 7.5 commitment, Article 17 boot/firmware/, Article 26 vendor firmware clarification.)
```

Note: the `Document:` line should change from "AltimitNeo Bible" to "NeoPlutarch Bible" if it currently says AltimitNeo (which it should per the rename). Verify before editing.

### 5.4 Stage and commit

```bash
# Read the relevant section first
grep -n "16.4" docs/bible.md | head -3
tail -20 docs/bible.md   # check footer

# After editing...
git diff docs/bible.md
git add docs/bible.md
git commit -m "docs(bible): v1.4.0 — Article 16.4 amendment, scheduling-fairness bench

Adds scheduling-fairness to the enumerated performance-regression
benches in Article 16.4, with Redox schedrs as the canonical
reference adapted to NeoPlutarch's class structure (Article 8.1).

Triggered by the Redox DWRR scheduler announcement (April 2026)
research triage at Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md.

Bumps bible v1.3.0 → v1.4.0."
```

---

## 6. Commit 4 — Redox DWRR PM-NOTE (file already on disk)

### 6.1 Scope

The file `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` was written by PM directly (outside Prometheus dispatch flow). Verify it exists and is consistent with the bible v1.4.0 Article 16.4 amendment from commit 3, then add and commit.

### 6.2 Verification

```bash
test -f Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md && echo "OK: file exists"
head -15 Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md  # confirm header
wc -l Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md     # expect 100-140 lines
grep "schedrs" Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md  # confirm key term
```

If file is missing or malformed, STOP and request PM clarification. Do NOT recreate from scratch — the inline content given to PM by Janus + drafted by PM is the canonical version.

### 6.3 Stage and commit

```bash
git add Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md
git status --short
git commit -m "docs(plans): Redox DWRR scheduler prior-art triage PM-NOTE

External research triage of the April 2026 Redox OS DWRR scheduler
announcement (RSoC 2026, Akshit Gaur). Identifies the interleaved-
DWRR variant as candidate prior art for within-class fairness in
NeoPlutarch's Compute and Background scheduler classes (Bible Article 8.1).

The Article 16.4 bench-surface expansion (bible v1.4.0, committed
in this docs sub-step series) is the load-bearing carry-forward
from this triage.

Multi-source corroboration: Redox blog, Phoronix, OSnews, Madshrimps."
```

---

## 7. Commit 5 — PHASE-2.md update + INDEX.md sweep

### 7.1 PHASE-2.md update

Read `docs/phases/PHASE-2.md`. Find the section tracking BUILD-3 sub-commits. Add a closure entry for SC-3B / sub-step 5c with:

- Date: 2026-05-10
- Closure commit: `e2b3f4f`
- Verification log: `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`
- Closure report: `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`
- Gate scores: 200/200 NEO_SMP=4, 25/25 NEO_SMP=2
- Mechanism named: Hpw — preempt-while-holding + IRQ-off same-CPU waiter
- Follow-up ADR proposals: ADR-018/019/020 (in Plans/)

If PHASE-2.md doesn't yet exist, create it with at minimum these sections:
- Phase 2 scope (per Bible Article 13)
- BUILD-1 status
- BUILD-2 status
- BUILD-3 sub-commit ledger:
  - Sub-commit 1 (1a, 1b)
  - Sub-commit 2
  - Sub-commit 3 (SC-3B) — closed 2026-05-10
- Next: BUILD-3 sub-commit 4 (or BUILD-4 if BUILD-3 is complete)

The exact format should match other PHASE-N.md files if they exist in `docs/phases/`. Read those first to match conventions.

### 7.2 INDEX.md sweep

Read `Plans/INDEX.md`. It is the master index of Plans/ artifacts. Sweep to incorporate all artifacts created between 2026-05-09 and 2026-05-10. Specifically, ensure the following are listed under appropriate sections:

**SC-3B investigation artifacts (2026-05-09 to 2026-05-10):**
- All `DISPATCH-2026-05-09-sc-3b-*.md` and `DISPATCH-2026-05-10-sc-3b-*.md` files
- `SC-3B-INVESTIGATION-2026-05-09-cerebro.md`, `-prometheus.md`, `-claude.md`
- `SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`
- `SC-3B-PHASE-B-FINDINGS-2026-05-09.md`
- `SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`
- `SC-3B-PHASE-C-FINDINGS-2026-05-10.md`
- `SC-3B-PHASE-D-FINDINGS-2026-05-10.md`
- `SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md`
- `SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`
- `SC-3B-VERIFICATION-2026-05-09.md`, `-fix-6-1.md`
- `SC-3B-SOURCE-AUDIT-2026-05-09.md`
- `SC-3B-PLAN-CHECKPOINT-2026-05-09-*.md`, `-SYNTHESIS-2026-05-09.md`
- `RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`
- `RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`
- `CLOSE-REPORT-2026-05-10-sc-3b.md`
- `HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`
- `HANDOFF-2026-05-09-sc-3b-sub-step-5b-in-flight.md`

**PM notes and decisions:**
- All `PM-NOTE-2026-05-09-*.md` and `PM-NOTE-2026-05-10-*.md` files (calibration, agent roster, Janus recurrence, Janus first-pass spot-check, Redox DWRR prior-art)

**Bible amendment artifacts:**
- `AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md`
- `ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md`
- The three new ADR proposals from commit 1 of this dispatch

**Rename and TM pre-flight:**
- `DISPATCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`
- `RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`
- `DISPATCH-2026-05-10-rename-three-commit-prometheus.md`

Group these under whatever section headers INDEX.md uses (e.g., "SC-3B investigation," "Bible amendments," "Rename and TM pre-flight," "PM notes," "Research artifacts"). If INDEX.md doesn't have these headers yet, add them.

If `Plans/INDEX.md` is significantly out of date (more than the 2026-05-09/05-10 backlog), document the deviation in the commit message but only sweep the SC-3B-era and rename-era backlog — comprehensive historical sweep is out of scope for this dispatch.

### 7.3 Stage and commit

```bash
git add docs/phases/PHASE-2.md Plans/INDEX.md
git status --short
git commit -m "docs(phases,index): mark SC-3B closure; INDEX sweep for 2026-05-09/10 backlog

PHASE-2.md: BUILD-3 sub-commit 3 (SC-3B) marked closed 2026-05-10.
Cross-references to CLOSE-REPORT, verification log, and the three
ADR proposals in Plans/.

INDEX.md: sweep to incorporate SC-3B investigation artifacts (~30
files), PM notes (~10 files), bible amendment proposals, rename
and TM pre-flight artifacts. Comprehensive historical sweep is
out of scope; this commit catches up the recent backlog."
```

---

## 8. Commit 6 — README.md rewrite

### 8.1 Scope

Replace `README.md` at project root entirely. The current README has substantive content inconsistencies with the bible (microkernel vs monolithic, stale phase roadmap, ALTFS vs NeoFS, .neo-model vs .neom, cloud-routed inference in roadmap, docs/design.md reference instead of docs/bible.md, on-the-nose .hack referencing).

The rewrite aligns README with bible v1.4.0 state, removes the inconsistencies, and preserves the homage acknowledgment as referential speech (TM-defensible).

### 8.2 Full replacement content

Write `README.md` with the following content verbatim:

```markdown
# NeoPlutarch

AI-native operating system in Rust. Clean-slate, monolithic kernel, capability-secured, single-node. Targets x86_64 (and later aarch64). Currently at Phase 2 (SMP + capability system + buddy/slab allocators + advanced TLB management).

This is not a Linux derivative. It is not a Unix clone. It borrows ideas where ideas are good, but it owes no compatibility debt to POSIX, to the Linux syscall ABI, to ELF dynamic linking conventions, or to the assumption that a CPU is the privileged compute substrate.

The project was originally named "AltimitNeo" through Phase 2 BUILD-3 sub-commit 2; a 2026-05-10 trademark-driven rename adopted "NeoPlutarch" to remove derivative-name exposure to Bandai Namco's `.hack` franchise IP. The bible preamble preserves the design-inspiration acknowledgment ("descended in spirit from Altimit OS") as referential speech. See `docs/bible.md` and `ALTIMIT-real-world-implementation.md` for the deeper context.

## Status

Phase 2 in progress. The phase ledger is in `docs/bible.md` Article 13. Most recent activity:

- 2026-05-10: SC-3B (Phase 2 BUILD-3 sub-commit 3, Q4/Q6 IPC retrofit) closed via commit `e2b3f4f`. See `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`.
- 2026-05-10: Bible v1.4.0 ratified. Project renamed AltimitNeo → NeoPlutarch.

## What this project is

A clean-slate **monolithic** kernel in Rust with AI/ML treated as first-class kernel primitives. Specifically:

- The kernel exposes a tensor object as a kernel-managed primitive alongside processes, threads, and memory mappings (Bible Article 2.2).
- Accelerators (GPUs, NPUs, TPUs when present) are kernel-managed devices with first-class drivers, not userspace-hijacked PCIe BARs (Article 2.2).
- The CUDA/ROCm/Metal model where userspace processes own the accelerator is explicitly rejected (Article 2.3).
- The "AI is a service over a socket" model (LM Studio, Ollama, llama.cpp daemons) is explicitly rejected. Inference is a syscall family, not an IPC dance (Article 2.4).
- The OS has no chatbot persona; no telemetry, ever; no embedded specific model in the kernel (Article 23).

See `docs/bible.md` for the foundational charter.

## What this project is not

- Not a Linux derivative, fork, or rebadge.
- Not a research microkernel demonstration. This is a usable system, not a paper.
- Not an "AI assistant OS" in the consumer-product sense.
- Not a unikernel or single-application appliance.
- Not a cloud-only OS. Bare metal first; virtualization is a guest scenario, not a host one.
- Not a POSIX-compatible system. A POSIX compatibility shim is planned for porting userland tools, but the native ABI is its own.

See `docs/bible.md` Article 3 for the canonical anti-scope.

## Quick start (Docker — recommended)

You'll need Docker Desktop (Windows/Mac) or Docker Engine + Compose (Linux). Nothing else — Rust nightly, QEMU, and the Limine bootloader all live inside the image.

One-time setup:

    docker compose build

Takes 5–10 minutes the first time.

Run kernel tests:

    docker compose run --rm neo cargo neo-test all

Boot in QEMU with VNC framebuffer:

    docker compose up neo-vnc

Connect a VNC client to `localhost:5901`.

Drop into a dev shell:

    docker compose run --rm neo

From the dev shell:

    cargo k             # build the kernel only
    cargo neo-tty       # build + run, serial console only
    cargo neo-vnc       # build + run with VNC :5901
    cargo xtask help    # see all xtask commands

## Roadmap

Per `docs/bible.md` Article 13:

- Phase 0 — Boot, framebuffer, serial. **Complete.**
- Phase 1.1 — GDT, IDT, basic interrupts. **Complete.**
- Phase 1.2 — Memory management, paging, frame allocator. **Complete.**
- Phase 1.3 — Cooperative + preemptive scheduling, kernel threads. **Complete.**
- Phase 1.4 — Userspace: SYSCALL/SYSRET, per-process page tables, ELF loader, first ring-3 process. **Complete.**
- Phase 1.5 — Multiple userspace processes, basic IPC, userspace heap, process termination, and pre-Phase-2 security consolidation. **Complete.**
- Phase 2 — SMP, capability system, buddy+slab allocators, advanced TLB management. **In progress.**
- Phase 3 — NeoFS (content-addressed), block layer, real disk I/O.
- Phase 4 — Network stack, IPv6, sockets-equivalent. Includes USB stack landing and USB Ethernet adapter support per ADR-017 §D3.
- Phase 5 — AI subsystem foundation: tensor primitive, model object, CPU inference backend.
- Phase 6 — Accelerator drivers (virtio-gpu compute, then real GPU/NPU).
- Phase 7 — Inference scheduler, multi-tenant model serving.
- Phase 7.5 — WiFi support: Intel iwlwifi family, WPA2/WPA3 client mode, hardware-default regulatory, bundled redistributable firmware. See ADR-017.
- Phase 8+ — Userland (window system, shell, package management, native applications).

Phases 5–7 are the AI-native phases that justify the project's existence.

## Repo layout

| Path | Purpose |
| --- | --- |
| `kernel/` | The kernel crate (`no_std`, Rust nightly, x86_64). |
| `userland/libneo/` | Userspace runtime library (`no_std`). |
| `userland/neo-init/` | PID 1 (placeholder; becomes the supervisor in later phases). |
| `tools/xtask/` | Build orchestrator. `cargo xtask help` lists commands. |
| `boot/` | Limine bootloader config. From Phase 7.5+, also bundled vendor firmware. |
| `docs/bible.md` | The project's foundational charter. **Read this first.** |
| `docs/ADR-NNN-*.md` | Architecture Decision Records. |
| `docs/specs/` | Technical specifications (ANSPEC-NNN, etc.). |
| `docs/phases/` | Per-phase plans and verification logs. |
| `Plans/` | Working artifacts: dispatches, PM-NOTEs, investigation findings. |
| `ALTIMIT-real-world-implementation.md` | Research artifact analyzing `.hack` canon as design inspiration. |

## Why?

Every existing OS treats AI as a feature bolted on top: an app, a service, a sidebar. NeoPlutarch treats intelligence as a kernel-blessed primitive — like memory, like CPU time, like a file handle. Apps make a syscall. The OS handles scheduling and protection. The actual matmul happens in driver code, accelerator firmware, or a dedicated kernel inference module — but the kernel knows it is happening and to whom.

Local-first by structure. No telemetry, ever (Bible Article 23.2).

## Status caveats

Phase 2 BUILD-3 sub-commit 3 (SC-3B / Sub-step 5c) closed 2026-05-10. The kernel package metadata (`kernel/Cargo.toml`) and any kernel boot title strings may still reference "AltimitNeo" until the kernel-side rename follow-up commits land. The docs rename (this commit) lands docs-only references; kernel-side completion is tracked separately.

## Troubleshooting

`docker compose build` fails on the rustup step:
Usually a transient network issue with `sh.rustup.rs`. Re-run `docker compose build` — Docker layer caching means you won't redo earlier steps.

VNC connects but the screen is black:
The kernel might have panicked before reaching the framebuffer init. Check the QEMU console output (the terminal where you ran `docker compose up neo-vnc`) for kernel diagnostics. The serial log will say what happened.

`cargo neo-vnc` says `failed to launch qemu-system-x86_64`:
You're running it on the host instead of inside the container, and QEMU isn't on your PATH. Either install QEMU or use `docker compose up neo-vnc` instead.

Build is incredibly slow on Windows:
File I/O across the Docker Desktop boundary is slower than native. Most of the time you'll only feel this on the first build of a fresh checkout; cargo's incremental cache makes subsequent builds fast.

## License

`MIT OR Apache-2.0` for project-authored code and documentation. Bundled vendor firmware (Phase 7.5+) retains its own vendor license. See `docs/bible.md` Article 26.

## Origin and homage

The project was originally named "AltimitNeo." A 2026-05-10 trademark pre-flight (`Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`) led to the rename. The design-inspiration acknowledgment "descended in spirit from Altimit OS" is preserved in the bible preamble as factual referential speech. The deep-research artifact analyzing `.hack` canon as inspiration is `ALTIMIT-real-world-implementation.md`.
```

### 8.3 Stage and commit

```bash
git diff README.md   # confirm full replacement, not just edit
git add README.md
git status --short
git commit -m "docs(readme): rewrite README to align with bible v1.4.0

The pre-rewrite README had several substantive inconsistencies with
bible v1.4.0 state, which the AltimitNeo→NeoPlutarch token rename
in Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md
did not correct:

- 'A from-scratch microkernel' / 'Pure microkernel' contradicted
  Article 4.1 (monolithic kernel with disciplined module boundaries).
- Phase 0 status framing was years out of date relative to Article 13
  (Phase 1.5 complete; Phase 2 in progress).
- Phase roadmap (1=memory, 2=drivers+ALTFS, 2.5=neo-inference, ...)
  bore no resemblance to Article 13's actual phase ledger.
- 'ALTFS' was a derivative-name reference (canonical filesystem from
  .hack); bible specifies NeoFS in Article 10.2.
- '.neo-model' was an invented file extension; bible specifies .neom
  in Article 25.2.
- 'neo-inference' subsystem doesn't exist in the bible; Article 6
  specifies the AI subsystem with three layers.
- Phase 4 'cloud-routed inference (Anthropic, OpenAI)' was in tension
  with Article 23.2 ('Inference happens on the user's hardware').
- 'docs/design.md' was referenced as canonical 'architecture spec';
  bible is canonical per Article 27.
- 'Inspired by the ALTIMIT OS of the .hack universe' was on-the-nose
  enough to weaken the rename's TM-defensibility framing.

This rewrite replaces README with content aligned to bible v1.4.0,
preserves the homage acknowledgment as factual referential speech
in the preamble (TM-defensible), and references docs/bible.md as
the canonical foundational document."
```

---

## 9. Push to origin

After all six commits land:

```bash
git log --oneline -10                    # sanity-check commit sequence
git push origin main                     # push the six new commits
gh repo view SecondStageTurbine/NeoPlutarch  # verify visibility
```

Expected: GitHub shows the six new commits in addition to the previous four (e2b3f4f, 04f4a95, 57439a6, cca6889). README on GitHub now renders the new content.

---

## 10. Post-push verification

- [ ] `git log --oneline | head -15` shows the expected commit sequence
- [ ] `git grep 'microkernel' README.md` returns 0 hits (or only the "not a research microkernel demonstration" anti-scope line)
- [ ] `git grep 'AltimitNeo' docs/` returns expected count (mostly historical references in CLOSE-REPORT and ADR-013 amendments, NOT in bible.md or README.md)
- [ ] `git grep 'ALTFS\|\\.neo-model\\|neo-inference' README.md docs/` returns 0 hits except where flagged as historical/incorrect
- [ ] `docs/bible.md` version footer reads v1.4.0
- [ ] `gh repo view SecondStageTurbine/NeoPlutarch` shows README aligned with bible state

If any verification fails, document the failure and stop pushing if not already pushed. Do not amend pushed commits without PM direction.

---

## 11. Failure handling

Same protocol as `Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md` §7:

- STOP immediately on any failure.
- Do NOT force-push to overwrite remote state.
- Capture failing command, error output, `git log -3 --oneline`, `git status --short`.
- File a brief note at `Plans/PROMETHEUS-FAILURE-2026-05-10-sub-step-6-dispatch.md` with the captured state.
- Return to PM for triage.

---

## 12. Out of scope

- **Kernel-side rename:** separate follow-up dispatch. `kernel/Cargo.toml` package name, kernel source AltimitNeo references, boot/limine.cfg boot title, tools/xtask references. Not touched by this dispatch.
- **ADR-011 supersession:** deferred until ADR-018 ratification. The supersession marker on ADR-011 would point to "ADR-018" which doesn't exist yet (only the proposal does). Post-ratification follow-up commit.
- **Actual ADR landing:** ADR-018/019/020 files in docs/ land post-ratification, not in this dispatch.
- **docs/design.md audit:** if `docs/design.md` exists and predates the bible, it may be stale-cruft. Triaging it (update to match bible, delete, or preserve as historical) is a separate follow-up. Not touched by this dispatch.
- **Kernel-bible-alignment audit:** code references to ALTFS, .neo-model, neo-inference, microkernel that may exist in kernel/ comments or test code. Cerebro-routed audit, separate dispatch.

---

## 13. Cross-references

- `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — primary input; §8 carry-forwards drive this dispatch
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — verification log referenced by the proposals
- `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` — referenced by commits 3 and 4
- `Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md` — prior dispatch establishing negative-list discipline; this dispatch follows the same pattern
- `docs/bible.md` v1.3.0 — input state; v1.4.0 is output state after commit 3
- `docs/ADR-013-phase-2-smp-retrofit.md` — modified in commit 2
- `docs/ADR-017-wifi-support-strategy.md` — recent ADR; referenced from README
- `docs/phases/PHASE-2.md` — modified in commit 5
- `Plans/INDEX.md` — modified in commit 5

---

## 14. Estimated execution time

- Pre-flight + commit 1 (three ADR proposals — content already drafted, mostly file creation): ~30 minutes
- Commit 2 (ADR-013 amendments — small edits): ~15 minutes
- Commit 3 (bible v1.4.0 amendment — Article 16.4 + footer): ~15 minutes
- Commit 4 (Redox PM-NOTE — verify + add): ~5 minutes
- Commit 5 (PHASE-2.md + INDEX.md — substantial sweep): ~45 minutes
- Commit 6 (README.md rewrite — full replacement): ~20 minutes
- Push + post-push verification: ~10 minutes

**Total target: ~2.5 hours.**

If execution exceeds 4 hours, STOP and report progress.

---

**End of dispatch.**
