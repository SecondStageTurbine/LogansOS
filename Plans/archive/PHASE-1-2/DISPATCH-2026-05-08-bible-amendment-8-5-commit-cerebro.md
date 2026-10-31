# Dispatch: Bible amendment 8.5 + ADR-016 ratification (two-commit landing)

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Two docs-only commits in sequence: (Commit A) ratified Article 8.5 (Critical-section discipline) on `docs/bible.md`; (Commit B) ratified ADR-016 published as `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`. Per Bible Article 27, the project lead has approved the amendment in this conversation; per Bible Article 24, the project lead has approved ADR-016. This dispatch carries the exact text and metadata for both commits.
**Substrate:** Project HEAD `3b3b4db`. Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **Two focused commits in sequence.** Commit A touches only `docs/bible.md`. Commit B touches only `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`. No other files, no commingling.
2. **Ordering matters.** Commit A (bible amendment) lands first; Commit B (ADR-016 ratification) lands second. ADR-016 cites Bible Article 8.5 as ratified; the citation makes sense only after the bible commit lands.
3. **No stash interactions.** Stashes remain untouched.
4. **Build verification before each commit.** `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings). Docs-only changes should not affect the build, but verify per project pattern.
5. **Backup runs after BOTH commits land.** `Plans/backup-to-synology.ps1`. With three stashes, the 2026-05-08-refined backup mechanism handles preservation. One backup run captures both commits.
6. **No INDEX.md update in either commit.** INDEX is meaningfully behind (16 entries), but the governance commits stay focused. INDEX sweep happens in a subsequent commit (PM picks the timing).

---

## 1. COMMIT A — Bible amendment 8.5

### 1.1 Exact text to add to `docs/bible.md`

Insert the following new article 8.5 between the existing Article 8.4 (last line: "8.4 Threads are kernel-managed. There is no green-thread / M:N / fiber abstraction at the kernel level. Userspace may build coroutines on top, but the kernel sees OS threads.") and the existing Article 9 header line ("Article 9 — Drivers and Hardware Abstraction").

Insert verbatim, preserving paragraph breaks. The text follows existing bible prose-style conventions (sentence-cased sub-article titles followed by period and body; no markdown headers within an article; no bold markers on sub-sub-article titles; sub-articles flow as paragraph blocks).

```
8.5 Critical-section discipline. The kernel honors a written discipline for code that holds locks and that runs in interrupt context. The following rules apply kernel-wide.

8.5.1 Spinlock and interrupt discipline. Any spinlock that may be acquired from both interrupt context and process context must have IRQ-safe acquisition: the process-context acquirer disables local interrupts before taking the lock and restores the prior interrupt state on release. The kernel provides IRQ-safe lock primitives for this purpose. Plain spinlocks are reserved for locks that are exclusively process-context (no ISR ever takes them) or exclusively interrupt-context (no process ever takes them). The choice is documented at lock declaration via the type used.

8.5.2 No heap allocation in interrupt context. Interrupt handlers and their transitive callees may not allocate from or deallocate to the global heap. This includes timer handlers, IPI handlers, exception handlers, and any function reachable from them. ISR-context code paths use pre-allocated, fixed-size, or stack-bounded data structures. The global heap may not be made structurally safe to use from ISR context as an excuse to violate this rule; the rule stands regardless of allocator implementation.

8.5.3 Scheduler heap-independence. The scheduler's critical sections — any code path that holds a per-CPU scheduler lock — must not depend on the global heap allocator's availability. Scheduler-internal data structures use bounded or pre-allocated storage so that scheduling operations never trigger heap allocation. The scheduler's correctness is independent of whether the heap is currently locked, fragmented, exhausted, or otherwise unavailable.

8.5.4 No blocking while holding a spinlock. A spinlock may not be held across any operation that can block, sleep, yield, or wait on a condition. Operations that acquire other spinlocks are permitted only with documented lock-order discipline; operations that acquire mutexes or semaphores from spinlock-held context are forbidden. Code that needs to perform blocking operations must release all spinlocks first.

8.5.5 Enforcement. These rules are enforced by code review at PR time, by unsafe-block SAFETY comments per Article 15.2 where unsafe code is involved, and structurally by the type system where lock primitives encode the discipline. Future tooling (lockdep-equivalent runtime checks, static analysis) is anticipated but not required to ratify any individual rule.

```

(The blank line at the end of the block above belongs in the inserted text — it provides paragraph separation between 8.5.5 and the "Article 9 — Drivers and Hardware Abstraction" header that follows.)

### 1.2 Footer changes to `docs/bible.md`

Replace the existing footer block:

```
Document: AltimitNeo Bible
Version: 1.0.4
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-06 (Article 6.6 added, inference-governance-hooks clause)
Authority: Project Lead

"We build real operating systems engineering. Every commit is intended to ship."
```

With:

```
Document: AltimitNeo Bible
Version: 1.0.5
Status: Ratified
Ratified: 2026-05-02 (commit aa2c949)
Last amended: 2026-05-08 (Article 8.5 added, critical-section discipline)
Authority: Project Lead

"We build real operating systems engineering. Every commit is intended to ship."
```

Two lines change: `Version: 1.0.4` → `Version: 1.0.5`, and the `Last amended:` line picks up the new date and amendment description. The closing quotation is unchanged.

### 1.3 Commit A message

Subject:

```
docs: bible.md — Article 8.5 critical-section discipline (v1.0.5)
```

Body:

```
Adds Article 8.5 (Critical-section discipline) to the AltimitNeo Bible
under Article 8 (Concurrency and Scheduling). The new article codifies
five sub-articles encoding the kernel-wide discipline for code that
holds locks and that runs in interrupt context:

- 8.5.1 Spinlock and interrupt discipline (IRQ-safe lock acquisition
  required when both contexts may take a lock)
- 8.5.2 No heap allocation in interrupt context (ISRs and their
  transitive callees may not touch the global heap)
- 8.5.3 Scheduler heap-independence (scheduler critical sections must
  not depend on the global heap allocator)
- 8.5.4 No blocking while holding a spinlock (restated existing
  practice for new contributors)
- 8.5.5 Enforcement (code review, SAFETY comments, type system; future
  tooling anticipated)

Triggered by WEDGE-INV-J1's identification of a heap-lock-re-entry
deadlock in the scheduler's reaper path (Plans/WEDGE-INV-J1-AUDIT-
2026-05-08.md). The amendment encodes the discipline as project policy;
the structural enforcement work is governed by ADR-016 (A1 IRQ-safe
heap wrapper + A2 scheduler heap-decoupling), ratified separately and
landing in Commit B of this dispatch.

Bible version bumps 1.0.4 -> 1.0.5 (patch bump, matching Article 6.6
addition precedent: 1.0.3 -> 1.0.4).

Per Bible Article 27, this amendment was approved by the project lead
in conversation on 2026-05-08. Rationale preserved in
Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-
discipline.md.

No code changes. Docs-only.
```

### 1.4 Commit A pre-commit checklist

Before `git add docs/bible.md && git commit`:

- [ ] `docs/bible.md` viewed in editor; new Article 8.5 reads correctly between 8.4 and Article 9.
- [ ] `docs/bible.md` viewed in editor; footer reads `Version: 1.0.5` and `Last amended: 2026-05-08 (Article 8.5 added, critical-section discipline)`.
- [ ] `git diff docs/bible.md` shows ONLY the additions in §1.1 and the two-line replacement in §1.2. No unintended changes.
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline.
- [ ] `git status` shows only `docs/bible.md` modified.

---

## 2. COMMIT B — ADR-016 ratification

### 2.1 New file: `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`

Create the canonical ratified ADR by copying the body of `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` (everything from the `## Context` section through the closing `*Three coordinated changes...*` italic line) and prepending a new ADR-015-style header per §2.2 below.

Drop these working-tree drafting artifacts from the Plans/ DRAFT during the copy:

- The "Co-ratified with" frontmatter row (it's a drafting note; the Related row in §2.2 covers cross-references).
- The "Supersedes" frontmatter row's mention of the earlier-same-day A1-only draft (working-tree drafting churn; not relevant to canonical history).
- The closing-paragraph claim "this turn's drafts" — change "this turn's drafts to wedge-discharged" to "co-ratification to wedge-discharged" or similar (drafting verbiage shouldn't survive into the canonical doc).
- Any other DRAFT-isms (e.g., "DRAFT — PM-authored proposal, awaiting project lead ratification" anywhere in the body — should be `Accepted (ratified 2026-05-08)` if mentioned at all).

Body content (Context, Decision, Rationale, Implementation plan, Consequences, Alternatives considered, References, closing italic line) is otherwise identical to the Plans/ DRAFT.

### 2.2 Header (frontmatter table) for `docs/ADR-016-...md`

Use this exact header, matching the ADR-015 style in `docs/ADR-015-kaslr-tolerant-heap-placement.md`:

```
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
```

H1 is `# ADR-016: IRQ-safe heap allocator and scheduler heap-decoupling` (matching ADR-015's `# ADR-015: KASLR-tolerant kernel heap placement` style). The horizontal rule (`---`) separates header from body.

### 2.3 Commit B message

Subject:

```
docs: ADR-016 IRQ-safe heap allocator and scheduler heap-decoupling (ratified)
```

Body:

```
Publishes the ratified ADR-016 to docs/. ADR-016 captures three
coordinated decisions:

1. A2: Decouple scheduler from global heap. Replace VecDeque<Box<Thread>>
   ready/zombie queues with bounded or pre-allocated storage so scheduler
   critical sections do not depend on the global heap allocator.

2. A1: IRQ-safe global heap allocator wrapper. Defense-in-depth structural
   enforcement of Bible 8.5.1 for the global heap layer.

3. Co-ratification of Bible amendment 8.5 (landed in the immediately
   preceding commit).

ADR-016 is the architectural response to the heap-lock-re-entry
deadlock identified in WEDGE-INV-J1
(Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md). Implementation lands in
stages over the next 2-4 weeks: A1 first (small wrapper, ~30-50 lines),
then A2 design proposal (PL ratification), then A2 implementation,
then verification. The wedge-fix-before-SC-3B commitment in
Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md discharges on Phase 6
verification (200 cargo boots covering all four historical signatures).

Drafting history preserved in
Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md
(working-tree drafting record) and
Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md (superseded
A1-only earlier draft, retained for project history).

Per Bible Article 24, this ADR was approved by the project lead in
conversation on 2026-05-08 ("Trust but Verify. - Skip L").

No code changes. Docs-only.
```

### 2.4 Commit B pre-commit checklist

Before `git add docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md && git commit`:

- [ ] `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` viewed in editor; H1, frontmatter table, and body all read correctly.
- [ ] No "DRAFT" markers anywhere in the body; status reads `Accepted (ratified 2026-05-08)` in the frontmatter.
- [ ] No remaining "Co-ratified with" or "earlier same-day draft" frontmatter rows.
- [ ] Spot-check Related row for completeness (Bible articles, prior-art research notes, locus files).
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline.
- [ ] `git status` shows only `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` as a NEW file (untracked, then staged).

---

## 3. Post-commit actions

After BOTH commits land:

1. `Plans/backup-to-synology.ps1` runs once. Verify the log shows `Stashes to preserve: 3` and the new HEAD SHA recorded (HEAD will be Commit B's SHA, with Commit A as HEAD~1).
2. Notify PM (project status/log/handoff per project convention) that the two governance commits landed. PM picks up by drafting the A1 implementation dispatch (already drafted: `Plans/DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md`).

---

## 4. Cross-references

- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` — the ratified amendment text and rationale (source for Commit A's §1.1).
- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` — the ratified ADR draft (source for Commit B's body content).
- `Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` — project-state record of the ratification.
- `Plans/DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md` — the next dispatch; presupposes both commits in this dispatch have landed.
- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the bug analysis that motivated both governance changes.
- `docs/ADR-015-kaslr-tolerant-heap-placement.md` — the style template for Commit B's header.
- Bible Articles 24 (Decision-Making) and 27 (Amendments) — the authority and process governing both commits.

---

**End of dispatch.**

*Two commits, both docs-only, governance-focused, co-ratified. Commit A: bible.md gets Article 8.5 and v1.0.5 footer. Commit B: docs/ADR-016 file created with ADR-015-style header and body from the Plans/ DRAFT. One backup run after both land. PM is on call for any drafting question Cerebro surfaces during the landing.*
