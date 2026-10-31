# DISPATCH — Q9 deferral documentation (Prometheus)

**Date filed:** 2026-05-11
**From:** PM
**To:** Prometheus (Qwen3.6-27B)
**Ferry:** PL ferries to Prometheus session.
**Status:** Doc-only single commit. Closes the last remaining Phase 2 BUILD-3 code-adjacent item per `Plans/Q9-EXPLORATION-2026-05-11.md` §6.1.

---

## 1. Mandate

Single doc-only commit landing three changes:

1. **PHASE-2.md forward marker addition.** New marker FM-Q9 in the existing "Forward reference: Phase 2+ capability + process-exit optimizations" subsection (landed via SC-4 closure integration at commit `3367a4f`).
2. **ADR-013 §Q9 amendment.** New amendment subsection recording the BUILD-3 final disposition (Q9 deferred permanently in current shape; D8 obsoletes `live_thread_count` under D13).
3. **INDEX.md entry** for `Plans/Q9-EXPLORATION-2026-05-11.md`.

## 2. Inputs

- `Plans/Q9-EXPLORATION-2026-05-11.md` — canonical PM-tier disposition.
- `docs/phases/PHASE-2.md` — target file for change 1. Existing "Forward reference: Phase 2+ capability + process-exit optimizations" subsection is at lines ~581+.
- `docs/ADR-013-phase-2-smp-retrofit.md` — target file for change 2. Existing amendments cluster at lines ~1099+ (sub-commit 1a) through ~1199 (sub-commit 2 Q9 hypothesis). Append new amendment after the last existing amendment.
- `Plans/INDEX.md` — target file for change 3.

## 3. Specific content

### 3.1 PHASE-2.md FM-Q9 forward marker

Format parallel to FM-D14 (COMPLETED_EXIT_CODES growth bound — also single-thread-per-process gated):

```markdown
- **FM-Q9 — Per-CPU reaper distribution with multi-thread support.** ADR-013 §Q9 specified per-CPU reapers + `Process.live_thread_count: AtomicU32` gate. Sub-commit 2 attempted implementation; 100x `ipc_heap_kill_smoke` failed at run 81/100 with a kernel hang in `reap()`; 8-experiment bisection localized to Q9-specific machinery; deferred. The interim shape (BSP scans all per-CPU schedulers' zombie queues; APs do not reap) is provably correct under SC-4 D13's single-thread-per-process invariant — the thread reap IS the process exit, and SC-4 D8's `Process.exit_state: AtomicU8` CAS already provides Process-exit serialization. `live_thread_count` is obsolete under D13. Q9 lifts only when Phase 2+ multi-threaded user processes enter scope, at which point design alternatives in `Plans/Q9-EXPLORATION-2026-05-11.md` §3 (a/c/d) become live options. See ADR-013 BUILD-3 amendment "sub-commit 2 Q9 deferred with bisection evidence" (lines ~1170-1187) for the original deferral; ADR-013 BUILD-3 amendment "Q9 deferred permanently under D13 invariant" (newly added by this commit) for the final disposition.
```

### 3.2 ADR-013 §Q9 amendment

Append a new amendment subsection after the existing "BUILD-3 amendment: sub-commit 2 Q9 hypothesis (informational, not investigated)" subsection:

```markdown
## BUILD-3 amendment: Q9 deferred permanently under D13 invariant

Sub-commit 2 deferred Q9 with bisection evidence and an unverified hypothesis. SC-4 (BUILD-3 sub-commits 5+6, closed 2026-05-11 at commit `57f3cb1`) added `Process.exit_state: AtomicU8` CAS-claim discipline (D8) and ratified the single-thread-per-process invariant (D13).

Under D13, the thread reap IS the process exit: there is exactly one thread per user process, and that thread's `on_thread_reap` path uniquely triggers Process deregistration. D8's CAS already serializes natural-exit-vs-forced-kill at the Process level. `live_thread_count` was Q9's mechanism for multi-thread "is this the last thread?" serialization — under D13 there is no such ambiguity, and the CAS handles single-thread serialization.

Q9's interim shape from this amendment cluster (BSP scans all per-CPU schedulers' zombie queues; APs do not reap; `live_thread_count` absent) is therefore the Phase 2 BUILD-3 final disposition. The deferral becomes permanent under D13.

If/when Phase 2+ multi-threaded user processes enter scope (currently a Phase 2+ forward marker per SC-4 D13), the Q9 design space reopens. Recommended starting point: `Plans/Q9-EXPLORATION-2026-05-11.md` §3 alternative (c) — per-CPU reapers without `live_thread_count`, using D8's CAS as the unified Process-exit serialization primitive. The `live_thread_count` original design (alternative a) carries the sub-commit 2 hypothesis-unverified race surface; alternative (c) avoids it by composing with D8 instead of duplicating its serialization function.

This amendment closes Q9 for Phase 2 BUILD-3. No code changes; the interim shape from "sub-commit 2 Q9 deferred with bisection evidence" stands as the final disposition.
```

### 3.3 INDEX.md entry

Format matches existing entries. Add to the SC-4 artifacts section or a new "Q9+ artifacts (2026-05-11)" section per Prometheus's reading of INDEX.md current organization.

Entry contents: `Plans/Q9-EXPLORATION-2026-05-11.md` — Q9+ exploration; PM-tier decision to defer Q9 permanently under D13 invariant; bare-metal verification scheduled non-blocking; rustfmt tooling gap pending PL operational call.

## 4. Commit message

Title:

```
docs: defer ADR-013 Q9 permanently under SC-4 D13 invariant; add FM-Q9 forward marker
```

Body:

```
Closes the last code-adjacent Phase 2 BUILD-3 item per PM disposition in
Plans/Q9-EXPLORATION-2026-05-11.md.

PHASE-2.md: new FM-Q9 forward marker in "Forward reference: Phase 2+
capability + process-exit optimizations" subsection.

ADR-013 §Q9: new amendment "Q9 deferred permanently under D13 invariant"
recording the BUILD-3 final disposition. SC-4 D8's exit_state CAS
obsoletes the live_thread_count mechanism under D13's single-thread-
per-process invariant. Phase 2+ multi-thread will re-open Q9 design
space; recommended starting alternative is Plans/Q9-EXPLORATION-2026-
05-11.md §3 (c).

INDEX.md: entry for the Q9+ exploration artifact.

See:
- Plans/Q9-EXPLORATION-2026-05-11.md
- Plans/CLOSE-REPORT-2026-05-11-sc-4.md §8 (Q9+ pointer)
- docs/ADR-013-phase-2-smp-retrofit.md §Q9 + existing amendments
```

## 5. Verification gates

Doc-only; no `cargo k` gates. Required checks:

1. Diff review: three files changed (`docs/phases/PHASE-2.md`, `docs/ADR-013-phase-2-smp-retrofit.md`, `Plans/INDEX.md`).
2. Cross-reference integrity: every cited artifact exists.
3. FM-Q9 placement matches FM-D14 format.
4. ADR-013 amendment matches existing amendment-subsection style (parallel to "sub-commit 2 Q9 deferred with bisection evidence").
5. No `kernel/src/*` touches.

## 6. Push

Branch: `prometheus/q9-deferral-documentation-2026-05-11`. Clean fast-forward to `origin/main`.

## 7. Verification report

File `Plans/Q9-DEFERRAL-DOCUMENTATION-VERIFICATION-2026-05-11.md`. Format matches prior Prometheus verification reports.

## 8. Closure protocol

Standard: SHA + report filename + diff summary back to PL via Prometheus. PM consumes; no follow-up PM-NOTE required unless something unexpected surfaced.

---

**End of dispatch.**

*Single doc-only commit. ~50-80 line touches across 3 files. Closes Q9 deferral as permanent under D13 invariant.*
