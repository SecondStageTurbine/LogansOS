# ADR-016: IRQ-safe heap allocator and scheduler heap-decoupling

| | |
|---|---|
| **Status** | Accepted (ratified 2026-05-08) |
| **Date** | 2026-05-08 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 8.5 (Critical-section discipline, ratified same day, v1.0.5) · Bible Article 8.3 (no global locks in hot paths — A2 honors structurally) · Bible Article 4.3 (deps as liabilities — A1 in-house) · Bible Article 7.5 (distinct allocation classes precedent) · Bible Articles 1.2 / 1.3 / 14 / 15.2 / 24 / 27 (governing posture and process) · `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` (bug analysis) · `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` (prior art across Rust no_std kernels) · `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` (McKenney framework) · `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` (active commitment this discharges via Phase 6) · `Plans/ALTIMIT-real-world-implementation.md` §A.2 (crash-immune-by-construction posture) · `kernel/src/mm/heap.rs` (locus of A1 implementation) · `kernel/src/sched/scheduler.rs` (locus of A2 implementation) |

---

## Context

WEDGE-INV-J1 (`Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md`) identified a heap-lock-re-entry deadlock in the scheduler's reaper path. The audit's per-call-site table showed the bug class generalizes: any heap allocation triggered while a per-CPU scheduler lock is held is structurally vulnerable to the same pattern, and additional latent sites exist beyond the proven mechanism. `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` framed the response as Family L (localized) + Family A (architectural).

Project lead's "Trust but Verify" ratification (this conversation, 2026-05-08) selected the strongest available posture: A2-direct (architectural decoupling) + A1 (IRQ-safe heap wrapper as defense-in-depth) + a Bible amendment (Article 8.5) encoding the discipline. Family L is skipped as throwaway work — A2 replaces the data structures L would have patched.

The decision aligns with:

- **Bible Article 8.3** — "Global locks are forbidden in hot paths; we use per-CPU structures and lock-free or fine-grained locking." A2 honors 8.3 structurally; A1 alone would have left the global lock dependency in place.
- **Bible Article 1.2** priority order (Correctness > Coherence > Performance) — A2 gives strongest coherence (no scheduler↔heap coupling).
- **Bible Article 7.5** precedent — "Tensor memory is a distinct allocation class... not subject to the slab allocator." A2 extends the same precedent to scheduler memory.
- **Research finding** in `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — every comparable Rust no_std kernel (Theseus, Rust-for-Linux, Linux, Redox) has converged on encoded discipline plus structural enforcement; NeoPlutarch joins that convergence.
- **ALTIMIT posture** in `Plans/ALTIMIT-real-world-implementation.md` §A.2 — "crash-immune by construction, not by patching." A2 reduces coupling so a heap CVE doesn't reach scheduling; A1 provides defense-in-depth at the heap layer; the Bible amendment encodes the discipline as a project invariant.

Three layers, one coherent posture.

## Decision

**Adopt three coordinated changes, all ratified together:**

### Decision 1 (A2): Decouple scheduler from global heap

Replace the scheduler's `VecDeque<Box<Thread>>` ready and zombie queues with bounded or pre-allocated storage that does not depend on the general-purpose heap during scheduler operations. Specific structure (intrusive linked list via per-Thread next/prev fields, fixed-capacity ring buffer, or other) is determined by Cerebro in a follow-up A2 design dispatch; this ADR ratifies the architectural goal, not the implementation detail.

Constraints on the A2 design:

- Spawn must be able to fail gracefully when the per-CPU thread limit is reached (no panic).
- Per-CPU thread limit must be configurable at scheduler init, with a defensible default (provisionally: 64 threads/CPU; design dispatch refines).
- The Box-pointer-stability invariant in the existing scheduler module's doc-comment ("Storage: VecDeque<Box<Thread>>, NOT VecDeque<Thread>... the captured pointer survives the move") must be preserved or replaced by an equivalent invariant; thread `rsp` field addresses captured before context-switch must remain valid through enqueue/dequeue.
- Scheduler operations (`spawn`, `push_ready`, `yield_now`, `wake_thread`, `wake_thread_front`, `kill_thread_by_pid`, `exit`, `reap`) must contain no calls reachable to `alloc::alloc::alloc` or `alloc::alloc::dealloc` while holding `PerCpuScheduler.inner`.

### Decision 2 (A1): IRQ-safe global heap allocator wrapper

Wrap the global heap allocator with an IRQ-safe shim that disables local interrupts during acquire and restores prior interrupt state on release. Implementation: in-house wrapper (~30-50 lines) in `kernel/src/mm/heap.rs` (or a new `kernel/src/mm/irq_safe_heap.rs` adjacent file), built around the existing `linked_list_allocator::Heap` (the unlocked inner allocator) with our own `Mutex` + IRQ-safety dance.

Constraints on the A1 implementation:

- The wrapper preserves the existing `static ALLOCATOR: ...` API surface — every existing heap user continues to work unchanged.
- Local IRQ disable uses the same pattern as `crate::arch::x86_64::without_interrupts` (proven primitive), specifically `pushfq` / `cli` / `popfq` to save and restore the interrupt-flag state. No raw `cli` / `sti` without state preservation.
- All `unsafe` blocks carry SAFETY comments per Bible Article 15.2.
- No external crate dependency. Bible Article 4.3 favors in-house implementation for ~50 lines.
- A1 lands BEFORE A2 (defense-in-depth in place before the structural decoupling reaches main).

### Decision 3 (Bible amendment): Article 8.5 — Critical-section discipline

Co-ratify `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md`. This ADR's implementation work is the conformance work for that amendment's articles 8.5.1, 8.5.2, and 8.5.3. The amendment encodes the discipline as written project policy; the ADR provides the structural enforcement.

## Rationale

### Why A2 instead of A1-alone

A1 alone closes the heap-lock-re-entry deadlock class — the proven WEDGE-INV-J1 mechanism and any future analogous deadlock in any subsystem — but leaves scheduler↔heap coupling structurally in place. This means:

- Future heap allocator changes (a fragmentation fix, a new allocator strategy, a debugging mode) can perturb scheduling latency in non-obvious ways.
- A heap allocator CVE or design bug propagates to scheduling — the Pluto's Kiss survival surface is larger than necessary.
- Bible Article 8.3 ("Global locks are forbidden in hot paths") is satisfied by patch (the lock is still global, just IRQ-safe) rather than by structure.

A2 dissolves the coupling. Scheduling becomes correctness-independent of the heap. That's the stronger property.

### Why A1 anyway, on top of A2

A2 closes scheduler-mediated heap-lock-re-entry. It does NOT close the broader class of "any process-context code holds heap lock; ISR fires; ISR or its callees touch heap." Today, NeoPlutarch has no ISRs that touch the heap (per WEDGE-INV-J1 §3.4). Tomorrow's ISRs might:

- Phase 4 network drivers may need bottom-half allocation.
- Phase 7 tracing may allocate per-event records.
- Phase 8+ subsystems we haven't designed yet may add ISR paths that touch heap.

Bible Amendment 8.5.2 forbids ISR allocations, but rules can be violated. A1 makes violations non-fatal: even if a future ISR allocates against policy, the wrapper prevents same-CPU deadlock. That's the "verify" half of "Trust but Verify."

### Why the Bible amendment

Without 8.5, the discipline is implicit. Future contributors learn the rule by precedent or by code review. With 8.5, the rule is written down, ratified, and binding. This matches Bible Article 1.3's posture ("real operating systems engineering... every commit is intended to ship") — engineering disciplines are written, not implicit.

### Why skip Family L

L was a localized fix to the proven mechanism (capacity-preserving drain in `reap()`, drop-under-without_interrupts, debug assertions). A2 replaces the data structures L would have patched. L's work would be deleted by A2's implementation. Bible Article 1.3's "no toy code, no throwaway shortcuts" posture argues for skipping L entirely; the wedge stays open until A2 verifies, but A2 verification is the discharge of the wedge-fix-before-SC-3B commitment, not L.

The cost: the wedge-fix-before-SC-3B commitment stays open during A1+A2 implementation (estimate 1-3 weeks rather than hours). SC-3B does not begin until that commitment discharges. This is acceptable per project lead's "Trust but Verify" decision and the Bible's correctness-over-performance priority.

## Implementation plan

### Phase 1: Co-ratification (this turn)

- PM drafts Bible amendment (done): `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md`
- PM drafts revised ADR-016 (this document)
- PM marks the original L dispatch as superseded with a forward note

### Phase 2: PL ratifies

- PL reviews Bible amendment and this ADR
- PL ratifies (or modifies / rejects, with rationale)
- On ratification:
  - Bible amendment lands as a single commit on `main`. Bible version bumps from 1.0.4 to 1.0.5 (patch bump, matching Article 6.6 precedent).
  - This ADR's status updates to ACCEPTED, recorded as ADR-016-FINAL.

### Phase 3: A1 implementation (Cerebro, ~1-2 hours)

- PM drafts A1 implementation dispatch with strict scope: `kernel/src/mm/heap.rs` IRQ-safe wrapper, in-house, ~30-50 lines plus SAFETY comments.
- Cerebro implements; one focused commit. Subject: `mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)`.
- Build verification, backup, no functional verification needed at this stage (A1 alone doesn't close the wedge — that's A2's job).

### Phase 4: A2 design (Cerebro, ~1-3 days)

- PM drafts A2 design-request dispatch. Cerebro proposes A2 specifics: data structure choice (intrusive list vs. fixed-capacity ring buffer vs. other), per-CPU thread limit, API changes to `spawn` (fallible), migration plan for existing scheduler call sites.
- Cerebro produces design proposal as an analysis note in `Plans/`. No code yet.
- PM and PL review the design proposal. PL ratifies the design choice.

### Phase 5: A2 implementation (Cerebro, ~1-2 weeks)

- PM drafts A2 implementation dispatch incorporating the ratified design.
- Cerebro implements. Likely 3-6 commits (data structure types, scheduler internals migration, call site updates, doc updates, test coverage).
- Build verification after each commit; backup after each commit.

### Phase 6: Verification

- 200 cargo boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`. Classify any timeout against all four historical signatures.
- Supplementary scenario coverage as described in §5.2 of the (now-superseded) L dispatch — same methodology, just applied to the post-A2 substrate.
- Outcome 1 (clean 200/200, all four signatures absent): wedge-fix-before-SC-3B commitment discharges.
- Outcome 2 (residual signatures): widening investigation. A2 should structurally eliminate the entire class; if a signature survives, something else is going on.

### Phase 7: Close

- PM writes close report. Wedge-fix-before-SC-3B commitment marked discharged. SC-3B work can begin.
- ADR-016 final status update: implementation commits' SHAs recorded.
- Bible amendment is already binding; no further action needed.

Estimated total: 2-4 weeks from co-ratification to wedge-discharged. The wedge stays open during this; SC-3B is gated.

## Consequences

### Engineering consequences

- Scheduler internals undergo meaningful surgery. Per-CPU bounded thread storage replaces unbounded `VecDeque<Box<Thread>>`. The `Box`-pointer-stability invariant must be preserved by the new structure (intrusive linked list, fixed-capacity ring buffer, or equivalent — all of which can preserve stable Thread addresses).
- Spawn becomes fallible. Callers must handle the per-CPU-full case. Likely policy: spawn over the limit returns an error; existing in-tree callers (test harness, init paths) get explicit handling rather than silent retry.
- The global heap is universally IRQ-safe. Every kernel allocation pays a small (single-digit-cycles) IRQ-disable cost. Overhead is below measurement noise for typical workloads.
- Scheduler critical sections become provably free of heap dependencies. Phase 7+ inference scheduling and Phase 8+ further work can build on this guarantee.

### Architectural consequences

- NeoPlutarch's posture matures: "scheduler is correctness-independent of the heap" is a real property, not an aspirational one.
- The Bible's Article 8.5 takes effect; future PRs are reviewed against it.
- The pattern "any subsystem that interacts with the heap and with interrupts uses the IRQ-safe API" is established. Future subsystems (network stack, filesystem, etc.) inherit the discipline.

### Governance consequences

- Bible version 1.0.5 (sub-article addition matching Article 6.6 precedent's patch-bump convention). The amendment process under Article 27 follows established precedent — written rationale, version bump, project-lead approval. ADR-016 is the largest-scope ADR to date in the project (touches scheduler, heap, and Bible policy). Sets a benchmark for cross-cutting ADRs.

### Risk consequences

- A2 is meaningful surgery. Risk of introducing new bugs during the migration. Mitigation: incremental commits, build verification after each, full verification loop in Phase 6.
- A2 commits to a per-CPU thread bound. If the bound is set too low, future scenarios may hit it; if too high, memory waste. Mitigation: design dispatch surfaces the choice; PL ratifies; bound is configurable.
- A1 wrapper introduces `unsafe` code in `kernel/src/mm/heap.rs`. Risk of bug in the IRQ-state-save/restore dance. Mitigation: reuse the proven `without_interrupts` pattern; tight SAFETY comments; small surface (~30-50 lines).

## Alternatives considered

### Alternative 1: A1 alone (originally recommended)

Rejected per project lead. Leaves scheduler↔heap coupling, doesn't honor Article 8.3 structurally. Faster to land but architecturally weaker.

### Alternative 2: A2 alone

Rejected per project lead. Closes scheduler-mediated heap-lock-re-entry but leaves the broader class open for future ISRs. Bible amendment 8.5.2 disciplines this but without structural backstop. "Trust but Verify" requires both layers.

### Alternative 3: Family L only (initial framing)

Rejected per project lead. Patches the proven mechanism but leaves multiple latent sites in `spawn`/`yield_now`/`wake_thread`/etc. and provides no architectural improvement. Throwaway work given A2 supersedes the patches.

### Alternative 4: Defer everything to Phase 2

Rejected by PM-NOTE-2026-05-07 wedge-fix commitment. The commitment is to fix the wedge before SC-3B; SC-3B is current work. Deferring to Phase 2 would re-open a previously-discharged commitment.

### Alternative 5: Replace `linked_list_allocator` entirely

Out of scope. NeoPlutarch's heap allocator choice is governed by separate ADRs; this ADR works within the existing choice. If a future ADR replaces `linked_list_allocator`, the IRQ-safe wrapper from this ADR remains applicable to the replacement.

## References

- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` — co-ratified bible amendment encoding the discipline.
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the audit identifying the bug class.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` — the L+A framing this ADR diverges from (skips L).
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — prior art across Rust no_std kernels showing the convergence on encoded discipline + structural enforcement.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — McKenney framework; "don't randomly hack concurrent code" applies and is partially encoded by 8.5.
- `Plans/ALTIMIT-real-world-implementation.md` §A.2 — ALTIMIT's "crash-immune by construction" posture; this ADR + Bible amendment 8.5 advances it.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment this ADR's implementation discharges.
- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` — superseded earlier-same-day draft (recommended A1-only); marked superseded by note.
- Bible Articles 1.2, 1.3, 4.3, 7.5, 8.3, 14, 15.2, 24, 27 — governing this ADR's posture and process.
- Theseus `irq_safety` crate — `github.com/theseus-os/irq_safety`. Reference implementation if Cerebro needs a comparison point during A1 implementation.
- Rust-for-Linux `kernel::sync::SpinLock::lock_irqdisable` — `rust-for-linux.github.io/docs/kernel/sync/struct.SpinLock.html`. Conceptually equivalent.
- Linux kernel locking documentation — `docs.kernel.org/locking/locktypes.html`. The convention this ADR adopts.

---

*Three coordinated changes: A2 (scheduler decoupling), A1 (IRQ-safe heap wrapper), Bible amendment 8.5 (encoded discipline). Co-ratified together; implementation lands in stages; wedge-fix-before-SC-3B commitment discharges on Phase 6 verification. Trust but Verify.*

---

## Amendment 2026-05-08 — A2 reframed to Phase 2 conformance work

| | |
|---|---|
| **Status** | Accepted (post-L empirical verification) |
| **Date** | 2026-05-08 |
| **Authority** | Project Lead |
| **Affects** | Decision 1 (A2 — scheduler heap-decoupling) phasing only. Decisions 2 and 3 unchanged. |

### Background

This ADR was originally ratified earlier on 2026-05-08 with three coordinated decisions: A2 (scheduler heap-decoupling) as pre-SC-3B work, A1 (IRQ-safe heap wrapper) as defense-in-depth, and co-ratification of Bible amendment 8.5. The ADR's original "Skip L" framing was issued before Cerebro had reported on the L localized fix dispatch. Cerebro had already begun L by the time the supersession landed in conversation; L was completed and committed at `4c6abf4` (`sched: prevent heap-lock re-entry in reap zombie drain`).

L verified 300/300: 200/200 across `smp_per_cpu_scheduler_smoke` plus 50/50 each across `cap_revoke_local` and `cap_survive_exit` at NEO_SMP=4. Zero historical-signature timeouts. Verification report at `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md`.

A1 implemented in `kernel/src/mm/heap.rs` and committed at `aac0c11` (`mm: ADR-016 IRQ-safe global heap allocator (A1 defense-in-depth)`). The wrapper uses `spin::Mutex` (already a direct dep) around `linked_list_allocator::Heap`, with `without_interrupts(...)` around init/alloc/dealloc. The global heap is now universally IRQ-safe. Bible 8.5.1 conforms structurally for the heap layer.

### What this amendment changes

A2 (scheduler heap-decoupling) is reframed from pre-SC-3B work to **Phase 2 conformance work**, scheduled alongside the buddy+slab allocator subsystem rework already on Phase 2's plate. The technical content of the A2 design (bounded or pre-allocated scheduler storage replacing `VecDeque<Box<Thread>>`) stands as the Phase 2 target; only the timeline shifts.

### Why

- L's 300/300 verification is empirically conclusive for the proven WEDGE-INV-J1 mechanism (`POST_SMOKE_SMP_SCENARIO`). The bug is closed.
- A1 closes the broader heap-lock-re-entry class structurally at the heap layer. Same-CPU deadlock from any process/ISR contention on the global heap is impossible by construction.
- A2 was originally framed as bug fix; with the bug fixed, A2's value is now Bible 8.5.3 structural conformance plus architectural cleanliness, not bug closure. Both naturally align with Phase 2's allocator subsystem work.
- Bible has well-established phased-conformance precedent: Article 7.5 (tensor allocation class, Phase 5+), Article 8.1 (inference scheduler class, Phase 5+) describe goal states reached over phases. Article 8.5.3 conforms at Phase 2; the bible itself does not require immediate conformance.
- Doing A2 now would spend ~2-4 weeks on architectural conformance with no improvement to bug-closure (300/300 doesn't get more conclusive with more verification). Phase 2 is the right phase boundary for the work.

### What does NOT change

- **Decision 2 (A1 IRQ-safe heap wrapper):** implemented at `aac0c11` and in `main`. Status unchanged.
- **Decision 3 (Bible amendment 8.5 co-ratification):** Bible v1.0.5 in `main`. Status unchanged.
- **The architectural goal of Decision 1 (A2):** unchanged. Scheduler ready/zombie queues will use bounded or pre-allocated storage; spawn becomes fallible at the per-CPU bound; the heap-independence invariant becomes structural rather than empirical. Only the *when* shifts.
- **Bible 8.5.3 binding status:** unchanged. Article 8.5.3 stands as project policy. A2 is the conformance milestone; conformance lands at Phase 2.

### Updated phasing of the original Implementation plan

The original "Implementation plan" section's Phases 1-3 stand as written (and have already executed). Phases 4-7 are reframed:

- **Original Phase 4 (A2 design dispatch):** moved to Phase 2 kickoff. PM drafts at Phase 2 launch, not now.
- **Original Phase 5 (A2 implementation):** moved to Phase 2. Implementation lands as part of Phase 2's allocator subsystem rework.
- **Original Phase 6 (verification):** moved to Phase 2 closeout. Verification re-runs against the post-A2 substrate at Phase 2 close.
- **Original Phase 7 (close):** moved to Phase 2 closeout.

### Wedge-fix-before-SC-3B commitment discharge

The commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` (test-runner wedge fixed before sub-commit 3b ships) is **discharged 2026-05-08** by L (`4c6abf4`) with A1 (`aac0c11`) as defense-in-depth. Discharge record at `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`. SC-3B work is unblocked.

### Cross-references

- Bible Article 8.5 (Critical-section discipline, ratified 2026-05-08, v1.0.5) — A2 implements 8.5.3 conformance at Phase 2.
- Bible Articles 7.5, 8.1 — phased-conformance precedents.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — formal discharge record.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — project-state record of the β execution.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — the 300/300 verification supporting the discharge.
- Phase 2 plan (forthcoming `docs/phases/PHASE-2.md` or equivalent) — receives the A2 work at Phase 2 kickoff.

---

*Amendment closes. Three decisions of original ADR-016 stand; Decision 1 (A2) phasing shifts to Phase 2; Decisions 2 and 3 are in main; wedge discharged on L+A1.*

---

## Amendment 2026-05-25 - A2 implementation landed

| | |
|---|---|
| **Status** | Implemented |
| **Date** | 2026-05-25 |
| **Authority** | Phase 2 BUILD subphase 2 dispatch |
| **Affects** | Decision 1 (A2 - scheduler heap-decoupling) implementation status. Decisions 2 and 3 unchanged. |
| **Implementation commit** | `5aa5baaff6029769323a0ced3e4e088ccbbf9d43` |

### Implementation choice

A2 is implemented as a local fixed-capacity scheduler ring in `kernel/src/sched/scheduler.rs`:

- `ThreadRing<const N: usize>` stores `[Option<Box<Thread>>; N]` plus head/length indices.
- `SCHEDULER_RING_CAPACITY` is 64, used for both per-CPU ready and zombie rings.
- The ring supports push-back, push-front, pop-front, PID removal, and zombie draining without heap growth.
- The structure stores `Box<Thread>`, not inline `Thread` values, so scheduler movement changes only Box pointer ownership; the Thread allocation and captured `rsp` field address remain stable.

### Heap-decoupling result

`PerCpuSchedulerInner.ready` and `.zombies` no longer use `VecDeque`. Scheduler ring operations do not allocate. The reaper pops one zombie while holding `PerCpuScheduler.inner`, then drops/frees the `Thread` after releasing the scheduler lock. User spawn capacity pressure returns `-EAGAIN` through `sys_spawn`; internal wake/exit paths treat ring exhaustion as an invariant violation rather than an ABI-visible condition.

### Verification

The subphase 2 close report records:

- `cargo k` PASS.
- `sched_ready_queue_capacity_pressure` PASS at `NEO_SMP=4`.
- SMP regression scenarios PASS at `NEO_SMP=4`.
- Subphase 1 wait/inspect and full `cap_*` regression gates PASS at `NEO_SMP=4`.
- Current successful serial logs scanned clean for `event=FAIL`, `KERNEL PANIC`, `BUG:`, and frame-leak markers.

---

*Amendment closes. ADR-016 A2 is implemented for Phase 2 BUILD subphase 2; scheduler ready/zombie storage is heap-decoupled by fixed-capacity per-CPU rings of `Box<Thread>`.*
