# Dispatch: WEDGE-INV-J1 — code audit of CPU 0 `PerCpuScheduler.inner` lock acquire/release sites

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Static code audit. **No commits, no test runs, no kernel changes, no builds beyond what's needed for symbol resolution.** Substrate is read-only at HEAD `3b3b4db`. Output is a single analysis note in `Plans/`. Cerebro's recommendation in `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md` is the primary input; `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` provides the highest-prior hypothesis to test first.
**Substrate:** Project HEAD `3b3b4db` (`xtask: optional QEMU monitor for investigation use`). Working tree should be clean on tracked files (the only intentional working-tree changes are the untracked `Plans/` artifacts and `tools/wedge-investigation/` scratch). Three preserved stashes:

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this audit; we're reading code on `main`.

---

## 0. Standing instructions

1. **Substrate is fully read-only.** No commits. No `git stash apply`. No kernel modifications. No test runs. Build only as needed for `addr2line` / `objdump` symbol resolution; that build does not produce a commit.
2. **Stashes are not popped, dropped, or applied.** They remain reference state.
3. **No backup is required for this dispatch** — no tracked changes land.
4. **Output is one analysis note**, prose-with-code-citations, in `Plans/`. No code in working tree changes; no scratch tooling required.
5. **Honest uncertainty is acceptable.** If the audit narrows to a small set of candidate mechanisms without conclusive identification, the right output is "candidates X, Y, Z, in priority order, with reasoning for each." A "we don't know yet, here's what to look at next" outcome is a valid §6 verdict.

---

## 1. Goal

Identify the bug — or narrow it to a small set of high-confidence candidates — by static analysis of every code path that can hold or attempt to acquire CPU 0's `PerCpuScheduler.inner` spinlock during the partial-reap-of-`smp-hog-{0..3}` phase.

WEDGE-INV-8 named the lock and the contention pattern. The lock-holder is unknown, and three plausible mechanism families remain live: re-entrant deadlock, cross-CPU lock-while-blocked, or atomicity/corruption bug. Audit either picks one, narrows to two, or names specific call sites worth probing in WEDGE-INV-J2's targeted capture.

**A focused research note (`Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`) identifies a specific high-prior sub-case of the re-entrant family: interrupt-context same-CPU lock re-entrance.** `spinning_top` provides plain `Spinlock` with no IRQ-safe variant; multiple Rust no_std kernels (Theseus, Rust-for-Linux, Redox) have hit this pattern and built standardized solutions. If `PerCpuScheduler.inner` is acquired in process context AND any same-CPU interrupt handler (timer ISR, IPI, exception) takes the same lock, the deadlock is structural. The audit's first job is to verify or refute this hypothesis explicitly — see §3.4 below, which is now required-priority not best-effort.

The end state is a written analysis note that a future Cerebro (or PM) can read and either (a) implement a fix from, or (b) build the next observation dispatch on top of.

---

## 2. Background

WEDGE-INV-8 captured 4 wedges; all converge on BSP at static RIP `0xffffffff8007bc22` in `spinning_top::spinlock::RawSpinlock::lock`'s `_mm_pause` loop. The contended lock is CPU 0's `PerCpuScheduler.inner`. AP per-CPU scheduler inner locks were readable (not contended in any AP capture). `zombie_count == 0` in all readable per-CPU schedulers; CPU 0's `zombie_count` is unreadable behind the locked inner.

Phase: post-`smp_per_cpu_scheduler_smoke`-scenario-dispatch, after at least one `smp-hog-N` reap completes. Worker counts vary across captures (1, 3, 2, 2 reaped before wedge); the lock-spin site is identical regardless.

Per `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md`: the scenario's `sched::spawn()` enqueues on `current_scheduler()` (BSP), so the workers spawn on CPU 0's per-CPU scheduler. APs may run them after migration; their data structures originate on CPU 0.

Three plausible mechanism families:

- **Re-entrant deadlock**: BSP holds CPU 0's inner, calls something that re-acquires it. Spinlock re-entrance is spin-forever. **Sub-case promoted to highest-prior by the research note: interrupt-context same-CPU re-entrance, where a timer ISR or IPI handler arrives while CPU 0's inner is held by process-context code on the same CPU and the handler's path takes the same lock. The pattern is well-documented; `spinning_top` provides no built-in protection; the deadlock is structural if the conditions intersect.**
- **Cross-CPU lock-while-blocked**: An AP took CPU 0's inner (cross-CPU), then halted/blocked/looped/faulted before unlocking.
- **Atomicity/corruption**: The lock's internal state is in a value that no `try_lock` can satisfy. Less likely a priori but possible.

The audit either picks one or excludes one or more.

---

## 3. Audit scope

### 3.1 Required: every direct call site of CPU 0's `PerCpuScheduler.inner.lock()` and `.try_lock()`

Or whatever the actual API is for acquiring the inner spinlock — adapt to the codebase. Enumerate every site that takes this lock specifically (CPU 0's, by index 0, OR generic `current_per_cpu_scheduler().inner.lock()` resolved on CPU 0, OR BSP-scans-all reaper iterating into `per_cpu_schedulers[0]`). Both BSP and AP code paths.

For each call site:

1. **Function and file:line**.
2. **Calling context**: BSP only? AP only? Either? Interrupt context? Process context?
3. **Lock-hold duration**: what code runs between `lock()` and the matching `unlock()` / drop-of-guard?
4. **Are interrupts disabled while the lock is held?** Look for `interrupts::disable()` / `cli` / `without_interrupts(...)` / equivalent guard around the acquire. If not, and §3.4 finds an interrupt handler that takes this lock, the bug is identified.
5. **Can the held code path block, sleep, halt, or yield?** If so, naming the path is the candidate-mechanism.
6. **Can the held code path call into another function that takes the same lock?** Re-entrance check. If the lock is not re-entrant (spinlocks usually aren't), any re-entry is the bug.
7. **Can the held code path take ANOTHER lock?** Lock-order risk. If yes, list the other locks; classical AB-BA deadlock requires two cross-locked sites.
8. **Is the unlock path unconditional?** Look for early-return paths, panic paths, fault paths that bypass unlock. Spinlocks held across unwinding are particularly suspicious.

### 3.2 Required: every BSP-scans-all-CPUs reaper path

Per `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` and `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`, the reaper scans all per-CPU schedulers from BSP. List the iteration:

- Where does the reaper start?
- Order of CPU traversal (CPU 0 first? last?)
- Per-CPU lock acquisition pattern (lock CPU 0's inner, then unlock; lock CPU 1's; etc.; OR hold all while iterating?)
- The lock-free `if zombie_count == 0` short-circuit's exact location and what it gates
- What does the reaper do with each zombie it finds? Specifically: does it ever take ANOTHER lock while holding a per-CPU inner?
- **Are interrupts disabled while the reaper holds a per-CPU inner?**

The reaper is the most likely location for the bug given the partial-reap context. Audit it with the most attention.

### 3.3 Required: scenario worker spawn / migration / exit paths

`smp-hog-{0..3}` workers go through:

- spawn (on CPU 0's scheduler per H1 findings)
- run (possibly migrated to APs)
- exit (becomes zombie on... where?)
- reap (by BSP-scans-all reaper)

For each transition, note any code that takes CPU 0's inner. Look specifically at the exit path: when an AP-running thread exits, where does its zombie state get registered? If the exit path runs on the AP and registers the zombie on CPU 0's queue, the AP is taking CPU 0's inner — that's a cross-CPU lock acquisition path and a candidate for mechanism family 2.

### 3.4 Required: schedule-tick, IPI, and exception handler paths (PROMOTED to highest priority)

**This is the focused research note's primary check.** Per `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`, the most likely single mechanism is interrupt-context same-CPU lock re-entrance. The audit must answer:

1. **Does the timer-tick ISR (or whatever drives scheduler ticks on AltimitNeo) take `PerCpuScheduler.inner` either directly or transitively?** Walk from the ISR entry point through all calls. If any callee invokes `current_scheduler().inner.lock()` or equivalent, this is the smoking gun.
2. **Do APIC IPI handlers (TLB shootdown, reschedule IPI, NMI, any other) take `PerCpuScheduler.inner`?** Same walk-from-handler-entry approach.
3. **Do exception handlers (page fault, GP fault, etc.) take `PerCpuScheduler.inner`?** Less likely, but enumerate.
4. **For each interrupt-context acquirer found in (1)–(3): is the matching process-context acquirer (from §3.1) wrapping its lock in `interrupts::disable()` / equivalent?** If process-context says "no interrupts disabled" AND interrupt-context says "I take this lock," the deadlock is structural.

Concrete fail-fast criterion: if step (1) shows the timer ISR ends up calling `inner.lock()`, AND any §3.1 process-context call site does NOT disable interrupts before acquire, **stop the audit and report Verdict A (Bug identified) with the interrupt-context-re-entrance mechanism named.** No need to continue auditing; the bug is identified.

Concrete refutation criterion: if step (1)–(3) show NO interrupt handler takes the lock (directly or transitively), interrupt-context re-entrance is refuted. Move on to mechanism family 2 (cross-CPU lock-while-blocked) and 3 (atomicity).

### 3.5 Out of scope

Do NOT audit the entire codebase. Stay focused on CPU 0's `PerCpuScheduler.inner`. Do NOT speculatively re-audit code that doesn't take that lock. Do NOT chase the lock-free `zombie_count == 0` short-circuit in itself; the SC-3A pre-thinking already analyzed it. The bug we're after is in lock-acquire/release pairing, not in the short-circuit's correctness in isolation.

If during the audit you find a different bug class (e.g., a use-after-free in some unrelated path, an obvious panic-via-uninitialized-memory), surface it in the analysis note as a separate observation; do not fold it into the wedge investigation.

---

## 4. Methodology

Suggested approach (Cerebro adapts based on what the codebase looks like):

1. **Locate the lock type.** Confirm CPU 0's per-CPU scheduler's inner is a `spinning_top::Spinlock<...>` or `RawSpinlock`-wrapped equivalent. Capture the exact type and its acquisition API. **Specifically confirm whether it's a plain `spinning_top::Spinlock` or a wrapper with built-in IRQ-safety** — the research note's hypothesis is structurally refuted if AltimitNeo already uses an IRQ-safe wrapper.
2. **Use `git grep` or `rg` to enumerate call sites.** Patterns: `.inner.lock(`, `.inner.try_lock(`, `current_scheduler` followed by `.inner`, `per_cpu_schedulers[0]` or `per_cpu_schedulers.get(0)` patterns, `bsp_scheduler` references.
3. **Do §3.4 first.** The interrupt-context check is fail-fast — if it fires, the audit is done. If it refutes, move on.
4. **For each call site, walk the held region.** From `lock()` to the matching unlock/drop, follow each call. If the call takes additional locks, note them. If the call may yield/block/halt, note that.
5. **Build a per-call-site table** in the analysis note: site, context, hold duration summary, IRQ-disabled status, blocking-risk analysis, re-entry-risk analysis, lock-order analysis.
6. **Propose ranked candidates** at the end of the analysis. For each candidate, name the file:line, the family it belongs to (interrupt-context-re-entrance / other-re-entrance / cross-CPU / atomicity), the evidence supporting it, and what would falsify it.
7. **Optionally**, use `addr2line` to confirm the static RIP `0xffffffff8007bc22` resolves to the lock-acquire path you'd predict from the audit. If it does, that's confirming evidence; if not, the audit may have missed a path.

Time budget guidance: ~30-60 minutes of focused reading should be enough to enumerate the call sites and rank candidates. Don't try to be exhaustive; aim for "small set of high-confidence candidates."

---

## 5. Output

A single analysis note: `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md`. Suggested structure:

- **Summary**: top-line finding(s).
- **Lock type confirmation**: what wraps `PerCpuScheduler.inner`; is it IRQ-safe out of the box?
- **§3.4 result**: does any interrupt handler take this lock? Are process-context acquirers IRQ-disabled? Verdict on interrupt-context-re-entrance hypothesis.
- **Per-call-site table**: as described in §4.5.
- **Reaper analysis**: walkthrough of the BSP-scans-all-CPUs reaper with attention to per-CPU lock interaction.
- **Worker exit-path analysis**: how `smp-hog-N` thread exit registers a zombie and on which CPU's queue.
- **Mechanism-family ranking**: interrupt-context-re-entrance / other-re-entrance / cross-CPU / atomicity, with weight of evidence for each.
- **Candidate sites in priority order**: file:line, mechanism family, evidence, falsification criterion.
- **If a fix is identifiable from audit alone**: state it explicitly with the proposed change (do NOT commit; the fix dispatch is separate). The research note's §5 lists three fix-family options for interrupt-context cases.
- **If audit doesn't conclusively identify the bug**: name the candidates that the next observation dispatch (J2) should target with the lock-pointer comparison.

Length target: ~150-300 lines. Comprehensive enough to be useful, focused enough to be readable.

---

## 6. Decision verdicts

Pick one for the analysis note's headline:

### Verdict A — Bug identified
Audit conclusively names the bug. Fix dispatch follows; J2 not needed. Write the analysis note with the fix explicit; PM follows up with a fix dispatch for Cerebro to implement.

### Verdict B — Strong candidate, needs confirmation
Audit narrows to one high-confidence candidate but cannot exclude others. Worth running J2 with the lock-pointer comparison targeted at the candidate's lock object. PM follows up with WEDGE-INV-J2 dispatch focused on the candidate.

### Verdict C — Multiple candidates, no clear winner
Audit produces N candidate sites without ability to rank decisively. J2 runs with broader RDI comparison against all candidates. PM follows up with WEDGE-INV-J2 dispatch with the candidate set.

### Verdict D — No candidate found
Audit doesn't identify a plausible mechanism. The bug is somewhere not enumerated; either the audit missed a path (re-do with widened scope) or the bug is in atomicity/corruption (which audit can't easily find). PM and project lead reassess.

---

## 7. Stop conditions

Stop and ping for direction in any of these cases:

- **The audit reveals a kernel-correctness gap unrelated to the wedge** (e.g., obvious data race elsewhere, panic-on-init bug, etc.). Surface in a separate observation in the analysis note; don't fold into this dispatch's deliverable.
- **The codebase doesn't match expectations** — e.g., `PerCpuScheduler` doesn't exist by that name, or the inner spinlock isn't `spinning_top`-derived. Adapt naming and ping if the deviation is significant; don't assume.
- **Time budget materially exceeds 90 minutes** — if audit is taking longer than expected, the audit has gone deeper than intended. Stop, write what you have so far, ping for direction.
- **§3.4 fail-fast fires** — if the interrupt-context-re-entrance hypothesis confirms early, write Verdict A and stop; no need to enumerate every other call site.

---

## 8. Cross-references

- `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md` — the captures + the recommendation that drove this audit.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` — the project-state framing for what's known/unknown.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — McKenney framework, structural precedents.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — **the focused research that elevates §3.4 to highest priority. Read this before starting the audit.** Includes the §5 fix-family options if Verdict A fires on interrupt-context-re-entrance.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — the lock-free short-circuit composition-safety analysis. The audit's reaper section is structurally on top of this analysis but answers different questions.
- `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` — §Q9 redo design notes; informs the BSP-scans-all reaper context.
- `Plans/H1-EXPERIMENT-FINDINGS-2026-05-08.md` — the BSP-spawn finding for `smp_per_cpu_scheduler_smoke`.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — the timing-window-disruption hypothesis; partially supported by the localization.

---

**End of dispatch.**

*Static analysis. No instrumentation, no observation, no test runs. Cerebro reads code, writes analysis note, names candidates. PM and project lead pick the next move from the §6 verdict. The §3.4 fail-fast is the fastest possible path to Verdict A; if the research note's hypothesis holds, the bug is named within the first 15-30 minutes.*
