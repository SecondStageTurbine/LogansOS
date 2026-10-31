# PM-NOTE: WEDGE-INV-8 Case A localization — BSP spins on CPU0 PerCpuScheduler inner lock

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the WEDGE-INV-8 Case A verdict (Lock spin) and the mechanism's current state of localization: top-level form known (BSP spinning trying to acquire CPU0's `PerCpuScheduler.inner` spinlock during partial-reap of `smp-hog-{0..3}` workers), but lock-holder identity not yet known due to the existing NMI handler not preserving general-purpose registers and AP-side NMI delivery limitation.
**Authority:** PM. Records investigation results. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active in original form; this localization is progress toward it, not discharge of it.

---

## 1. The convergence

WEDGE-INV-8 ran 200 cargo-driven boots of `smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with the kernel-side NMI watchdog stash applied and a host-side wrapper that fires NMI on serial-silence detection. Of those 200 boots, 196 PASSed and 4 produced `TIMEOUT_IN_KERNEL_SMOKE` with the `POST_SMOKE_SMP_SCENARIO` signature. All 4 captured wedges produced clean state dumps via the NMI-trigger mechanism.

**All 4 captures converge on the same RIP after KASLR normalization**: `0xffffffff8007bc22`. `addr2line` resolves this to `core::core_arch::x86::sse2::_mm_pause`, immediately inside `spinning_top::spinlock::RawSpinlock::lock`. In every capture, the BSP is observed actively spinning in the lock-acquire pause loop, and CPU 0's `PerCpuScheduler.inner` cannot be acquired by the snapshot's nonblocking `try_lock`.

This is decisive convergence at the §6 Case A threshold and beyond. Across 4 independent reproductions, the wedge mechanism is identical. WEDGE-INV-2's 0/30 (instrumentation masks), WEDGE-INV-5/7's combined 0/120 (GDB stub masks), WEDGE-INV-6's 1/200 with a different signature (cargo-vs-direct timing shift) are now all retrospectively explainable: each was a different facet of "the BSP is spinning on CPU0's per-CPU scheduler inner lock and nobody's releasing." That answer is consistent with all measurements taken, even those that didn't reach mechanism.

## 2. What's known and what's not

### Known

- **The site**: BSP at `0xffffffff8007bc22` in `RawSpinlock::lock`'s `_mm_pause` loop, after KASLR slide normalization. Same address in all 4 captures.
- **The contended lock**: CPU 0's `PerCpuScheduler.inner`. The NMI snapshot's nonblocking `try_lock` fails on this lock in every capture, while succeeding on AP per-CPU scheduler inner locks.
- **The phase**: After `smp_per_cpu_scheduler_smoke`'s scenario dispatch begins. After at least one `smp-hog-N` reap completes. Specifically:
  - Run 24 wedged after `smp-hog-1` reaped (1 worker)
  - Run 41 wedged after `smp-hog-{0,1,2}` reaped (3 workers)
  - Run 105 wedged after `smp-hog-{0,1}` reaped (2 workers)
  - Run 129 wedged after `smp-hog-{0,1}` reaped (2 workers)
- **AP per-CPU schedulers are NOT spinning**. The NMI snapshot reads each AP's per-CPU scheduler state; the AP queues are empty (only their bootstrap `ap-N-main` threads as current). Whatever's happening, it's not "all CPUs spinning."
- **`zombie_count == 0` for all readable per-CPU schedulers at dump time.** The lock-free counter says no zombies. CPU 0's inner is locked and unreadable, so we can't confirm CPU 0's `zombie_count`.
- **Outstanding `smp-hog-*` workers are not visible in any AP queue.** They reaped, or they were never on AP queues, or they're stored somewhere the dump can't reach (likely behind the locked CPU 0 inner).

### Not known

- **The lock-holder.** Who holds CPU 0's `PerCpuScheduler.inner` while BSP spins on it. The NMI handler is `extern "x86-interrupt"` and does not preserve general-purpose registers; the lock pointer (likely passed via RDI per System V calling convention) is not captured in the dump. AP-side NMI delivery did not produce `[ NMI ]` markers despite QEMU monitor `cpu N; nmi` HMP commands being issued; whatever the cause, the AP states at wedge time are unobserved.
- **The lock-holder's RIP, state, and call path.** Whether the holder is mid-execution on an AP, halted in some state, in a fault path, or somewhere else.
- **Whether this is re-entrance, cross-CPU, or corruption.** Three plausible mechanism families are still live:
  - **Re-entrant deadlock.** BSP held the lock, called something, that something tried to re-acquire. Possible if some path between `lock()` and `unlock()` calls back into a function that itself locks. Spinlock re-entrance = spin-forever.
  - **Cross-CPU lock-while-blocked.** An AP took CPU 0's inner lock to do some cross-CPU operation, then halted/blocked/looped before unlocking. Per H1 findings, the `smp_per_cpu_scheduler_smoke` scenario uses `sched::spawn()` on `current_scheduler()` (BSP) — so workers spawn on CPU 0 — but their later operations may take CPU 0's lock from APs.
  - **Corruption / atomicity bug.** The lock's internal state is in a value that never satisfies `try_lock` (e.g., counter mismatch, atomic ordering violation). Less likely a priori but possible.

The captures don't yet distinguish these three.

## 3. Why this is a meaningful project-state change

This is the first time the wedge investigation has produced **specific, falsifiable, mechanism-level data**. Prior dispatches established the bug class (heisenbug under instrumentation, GDB stub masks, four signatures, partial-reap-class) but did not name a specific code location. WEDGE-INV-8 names a specific lock and a specific contention pattern.

The localization changes the disposition of several other project artifacts:

- **`Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`** (the lock-free `zombie_count == 0` short-circuit composition-safety analysis): the analysis remains correct under H1's narrow refutation, but the convergence-note's leading hypothesis (`Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10) is partially supported. The substrate-shift damping story stands; the bug is in the lock-acquire path, not in the short-circuit *itself*. The lock-free path may still be a "fastpath that hides slowpath bugs" per McKenney's framework (`Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` §6.4), but that's now a forward-looking concern, not a current-bug-cause concern.
- **`Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`** (the H1 narrow refutation): H1 was specifically about the lock-free `zombie_count` divergence. The Case A finding does not contradict H1's refutation. The bug we've localized is upstream of `zombie_count` divergence — it's about who can take CPU 0's inner lock, not what value `zombie_count` has when read.
- **`Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`** + **`Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md`** (the four-signature picture): now plausibly all explained by *one* lock-acquire-class bug whose surface manifestation depends on host-side timing (which determines exactly which thread reaches the contention point first, which in turn determines whether the wedge surfaces as `SMOKE_MID_PHASE`, `SMOKE_COMPLETION`, `POST_SMOKE_USER_PROCESS`, or `POST_SMOKE_SMP_SCENARIO`). One bug, four masks, none of them random.

This last point would be the strongest possible interpretation. It is not yet proved — three signatures are post-substrate-shift quiet, only `POST_SMOKE_SMP_SCENARIO` actively reproduces, and it's possible the other three are different bugs that ADR-015 happened to fix. WEDGE-INV-9's lock-pointer + AP-state capture, plus an audit of CPU 0 inner lock acquire/release sites, would either confirm or refute the unifying interpretation.

## 4. Disposition of the wedge-fix-before-SC-3B commitment

**Active in original form.** Localization is not discharge. The commitment requires a *fix*, not a *finding*. Per `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` and project lead's standing directive recorded in `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md`: "we cannot ship it with errors."

The remaining work to discharge:

1. Identify the lock-holder and its call path (lock-pointer dump + AP capture, OR audit, OR both).
2. Determine the mechanism family (re-entrance / cross-CPU lock-while-blocked / corruption / something else).
3. Design and commit a fix.
4. Verify the fix at sufficient sample size (likely 200+ cargo-driven boots) that the rate is statistically zero, AND verify the other three historical signatures (`SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) are also gone if the unifying interpretation holds — or if the unifying interpretation refutes, treat each remaining signature as its own bug to localize and fix.
5. Mark the commitment met, with a close report.

The path to discharge is now clearer than at any prior point in the investigation, but is not yet short.

## 5. Stash topology

WEDGE-INV-8 introduced a new stash. Current state:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog` (NEW — the NMI watchdog kernel-side handler from WEDGE-INV-8)
- `stash@{1}: On main: wedge-investigation-2-instrumentation` (was `stash@{0}`)
- `stash@{2}: On main: h1-experiment-instrumentation` (was `stash@{1}`)

All three are preserved in the latest backup at `Z:\AltimitNeo\2026-05-08-212442\AltimitNeo.git` per the 2026-05-08-refined backup mechanism's `refs/stashes/{0,1,2}` pinning.

The stash topology now means: if a future chat boundary rebuilds the workspace from backup and re-applies stashes, `git stash apply refs/stashes/0` recovers WEDGE-INV-8's NMI watchdog, NOT WEDGE-INV-2's instrumentation. The numbering is stable per backup but unstable across stash creations. Each backup's `RESTORE.md` documents the at-creation-time mapping; consult it on recovery.

## 6. What does NOT change with this note

- The Bible. No amendments.
- Any ADR.
- The wedge-fix-before-SC-3B commitment (active, see §4).
- The "Open Project Commitments" section of `Plans/INDEX.md`.
- ADR-015 (KASLR-tolerant heap placement). The mechanism we've localized is a lock-acquire bug, not a memory-layout bug. ADR-015's substrate change appears to have damped this bug from PATCH1's ~22% (after H7 correction: ~14%) down to 2.67% by changing timing windows; that does not make ADR-015 the fix, and the localized bug remains.

## 7. Cross-references

- `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md` — the findings being summarized; canonical source for the Case A verdict and the captured RIP/lock data.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md` — the dispatch that produced the captures.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md` — the project-state context including PL's "find the mechanism" directive that drove WEDGE-INV-7→8.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md` — the four-signature picture; this note's §3 partially explains it via the unifying-bug interpretation (not yet proved).
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` — the convergence-note framing; §10 timing-window-disruption story partially supported by this localization.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — McKenney framework; §6.4 (de-emphasize fastpaths) is forward-looking now that mechanism is named.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — the lock-free short-circuit analysis; not contradicted but its applicability re-framed in §3 above.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — H1's narrow refutation stands; the localized bug is not what H1 was testing.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment.

---

**End of note.**

*The mechanism is partially named. BSP spins on CPU 0's `PerCpuScheduler.inner` during partial-reap of `smp-hog-*` workers. The lock-holder is unknown; the next dispatch identifies it via either targeted code audit, register-saving NMI capture, or both. Discharge of the commitment requires a fix, not a finding.*
