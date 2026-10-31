# [LANDED AND VERIFIED — supersession header below was overtaken by events]

> **Status update 2026-05-08 (later same day):** Cerebro had already begun implementing L when the supersession header below was written. By the time the supersession landed in this conversation, L was complete. Cerebro proceeded through landing and verification.
>
> **Landed:** commit `4c6abf4` (`sched: prevent heap-lock re-entry in reap zombie drain`) on `main`, ahead of the bible amendment commit `8ef8cac`.
>
> **Verified:** 200/200 cargo boots of `smp_per_cpu_scheduler_smoke` at NEO_SMP=4, plus 50/50 `cap_revoke_local` and 50/50 `cap_survive_exit` supplementary scenarios. Zero timeouts of any historical signature across 300 boots. Close report at `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md`.
>
> **Implication:** The proven WEDGE-INV-J1 mechanism (`POST_SMOKE_SMP_SCENARIO`) is closed at high statistical confidence. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` has a plausible discharge condition met, pending project lead's decision on whether L's coverage is sufficient (vs. ADR-016's A2 architectural follow-through).
>
> **The supersession header below (and §1-§7 of the original dispatch) is preserved for project history.** Read it as the planning artifact, not as current guidance — this dispatch has been executed.

---

# [SUPERSEDED — DO NOT IMPLEMENT]

> **Status:** SUPERSEDED 2026-05-08 by project lead's "Trust but Verify" decision.
> **Replaced by:** `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` (A2-direct + A1 defense-in-depth + Bible amendment 8.5).
> **Reason:** Family L was a localized fix to data structures (`VecDeque<Box<Thread>>` ready/zombie queues) that A2 replaces with bounded or pre-allocated storage. L's specific patches (capacity-preserving drain, drop-under-without_interrupts, push-back debug assertions) become moot once A2 lands — the things L is patching don't exist anymore in their current form. Per Bible Article 1.3 ("no toy code, no throwaway shortcuts"), L is skipped.
> **Disposition:** This dispatch is preserved verbatim below for project history. **Do not forward to Cerebro. Do not implement.** The wedge-fix-before-SC-3B commitment is now discharged by ADR-016's verification (Phase 6 of the implementation plan there), not by L.
> **What carries forward:** The verification methodology (200 cargo boots covering all four historical signatures) is reused in ADR-016's Phase 6. The H7 signature classifier reference and the scenario-coverage approach are not L-specific.

---

(Original dispatch text preserved verbatim below.)

---

# Dispatch: WEDGE-FIX-L — Family L localized fix for the heap-lock-re-entry wedge

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-08
**Type:** Fix dispatch + verification. Implements Family L (localized) of the L+A path approved by project lead, per `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` §3. Family A (architectural — IRQ-safe global heap allocator) is being drafted as `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` in parallel and is NOT in this dispatch's scope. This dispatch closes the proven mechanism; ADR-016 ratification (or not) is a separate normative decision that L's verification result helps inform.
**Substrate:** Project HEAD `3b3b4db` (`xtask: optional QEMU monitor for investigation use`). Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **Fix is a single focused commit** unless the change cleanly factors into a kernel-correctness commit + a debug-assertion commit; if so, two commits is acceptable. Do not exceed two.
2. **No stash interactions.** Stashes remain untouched.
3. **Build verification before commit** per project pattern: `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings).
4. **Backup runs after commit(s) land.** `Plans/backup-to-synology.ps1`. With three stashes, the 2026-05-08-refined backup mechanism handles preservation.
5. **Verification is part of this dispatch's deliverable**, not a separate dispatch. See §5.
6. **Honest verification reporting.** If the verification at the §5.1 sample size shows residual timeouts of any historical signature, surface them in the close report; do NOT extend the sample size in hopes of statistical zero. The result of L's verification feeds the ADR-016 ratification decision; an honest "L closed POST_SMOKE_SMP_SCENARIO but SMOKE_MID_PHASE recurred" is the kind of finding that justifies architectural follow-up.

---

## 1. Goal

Enforce the invariant: **no heap allocation OR deallocation may occur while `PerCpuScheduler.inner` is held, AND the BSP-scans-all reaper must not hold the global heap allocator lock during a window when timer interrupts are enabled and the scheduler can switch to a worker that performs heap-touching scheduler operations.**

Concretely, fix the two coupled mechanisms identified in `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` §"Mechanism-Family Ranking" item 1:

- **The `mem::take` zero-capacity left-behind.** `reap()` (in `kernel/src/sched/scheduler.rs:541-546`) does `mem::take(&mut s.zombies)` while holding `PerCpuScheduler.inner` under `without_interrupts`. The take leaves `s.zombies` as `VecDeque::default()` with zero capacity. Subsequent `s.zombies.push_back(prev)` calls (from `rotate(WhichQueue::Zombie)` in `kernel/src/sched/scheduler.rs:241-247`, reachable via `exit()`) trigger reallocations under scheduler-inner, which contend with the heap allocator lock.
- **The drained-buffer drop outside `without_interrupts`.** The drained `VecDeque<Box<Thread>>` from the take is processed outside the `without_interrupts` block (`kernel/src/sched/scheduler.rs:547-557`) and dropped at scope exit. The drop frees memory through the global heap allocator with interrupts ENABLED, leaving a window where a timer ISR can fire mid-drop, switch to a worker, and the worker's `exit()` path can attempt the heap operation that deadlocks against the interrupted reaper.

Either fix alone is insufficient: zero-capacity remediation without irq-protected drop still leaves the timer-interrupt-during-drop window; irq-protected drop without zero-capacity remediation still leaves any other path (ready-queue growth in `spawn`/`yield_now`/`wake_thread`/etc.) vulnerable to the same heap-lock-re-entry pattern under capacity pressure. Both must land together.

---

## 2. Required changes

### 2.1 reap() — preserve `s.zombies` capacity

The live `s.zombies` queue must never fall below `ZOMBIE_INITIAL_CAPACITY` (currently 4) capacity through the reap operation. Implementation choice is yours; the contract is "after `reap()` returns, `s.zombies.capacity() >= ZOMBIE_INITIAL_CAPACITY`."

Sketch of one viable approach (you may choose differently):

- Pre-allocate a replacement `VecDeque::with_capacity(ZOMBIE_INITIAL_CAPACITY)` outside the `without_interrupts` block (heap allocation happens with IF=1, no scheduler lock — safe).
- Inside `without_interrupts`, use `mem::replace(&mut s.zombies, replacement)` to swap the live and drained queues. The drained one carries the OLD allocation; the live one now has the FRESH allocation.
- Process the drained queue outside the lock as today.

Other viable approaches: a persistent reusable buffer (BSP-only, since `reap()` is BSP-only); an `unsafe` no-alloc swap; a different API shape entirely. Pick the cleanest one given the codebase. Explain the choice in the commit message.

### 2.2 reap() — drop the drained buffer under `without_interrupts`

After per-zombie processing completes, the drained `VecDeque`'s buffer must be freed with interrupts disabled. The simplest implementation is an explicit `without_interrupts(|| drop(drained))` at the end of `reap()`. If your approach to (2.1) avoids dropping the drained buffer at all (e.g., persistent reusable buffer), this requirement is satisfied vacuously; document that in the commit message.

### 2.3 Debug assertions on scheduler-held push paths

Add `debug_assert!` (preferred over runtime assert — fails loud in test builds, costs nothing in release) at every site where a `VecDeque` push happens while `PerCpuScheduler.inner` is held. Per the audit's per-call-site table, the sites are:

- `rotate` ready push (used by `yield_now`, etc.) — `kernel/src/sched/scheduler.rs` rotate body
- `rotate` zombie push — same function
- `spawn` ready push
- `push_ready`
- `wake_thread` (push back)
- `wake_thread_front` (push front)
- `kill_thread_by_pid` zombie push

Assertion form, schematic:

```rust
debug_assert!(
    s.ready.capacity() > s.ready.len(),
    "scheduler-held ready push would allocate (cap={}, len={})",
    s.ready.capacity(), s.ready.len()
);
s.ready.push_back(...);
```

Adapt to each site's variable names. The assertion makes any future regression in capacity discipline fail loudly during tests; in release builds it's a no-op.

### 2.4 No other changes

Do NOT refactor scheduler internals beyond what's necessary for (2.1) and (2.2). Do NOT add new abstractions. Do NOT touch the heap allocator, IRQ-safety wrappers, or any architectural surface — those are ADR-016's territory and explicitly out of scope here. Keep the diff minimal.

If during implementation you discover a constraint that makes the fix design impractical (e.g., a Rust ownership issue, a type-system limitation), surface it; do not power through with a workaround that increases complexity.

---

## 3. Commit hygiene

Commit message must include:

- A one-line subject of the form `sched: prevent heap-lock re-entry in reap zombie drain` or similar.
- A body explaining what the change does and why, referencing `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` for the mechanism.
- Explicit mention of which approach was chosen for (2.1) and why.
- Note about `debug_assert!` placement and the rationale (fail-loud regression catching).

If you split into two commits (kernel-correctness + debug-assertions), the second commit's message references the first.

After commit(s) land, run the backup script per §0.4. Verify the backup preserves all three stashes (the 2026-05-08-refined mechanism handles this automatically; just confirm the log shows `Stashes to preserve: 3`).

---

## 4. Pre-verification sanity check

Before the §5 verification loop, run a single boot of `cargo neo-test smp_per_cpu_scheduler_smoke` at `NEO_SMP=4` with no instrumentation stash applied. Expectation: PASS. If this single boot fails, something in the fix went wrong and the verification loop would be wasted; debug and re-apply before §5.

---

## 5. Verification

The fix is verified by demonstrating that none of the four historical wedge signatures appear at sample size sufficient for ~95% confidence the rate is zero.

### 5.1 Primary verification loop

200 cargo-driven boots of `cargo neo-test smp_per_cpu_scheduler_smoke` at `NEO_SMP=4`. No stash applied. Standard cargo path (matches the WEDGE-INV-3+4 baseline that produced the 2.67% rate).

Classify any timeout per the canonical signatures from `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1, plus explicit checks for:

- `POST_SMOKE_SMP_SCENARIO` (the proven mechanism's signature; pre-fix rate 2.67%)
- `SMOKE_MID_PHASE`
- `SMOKE_COMPLETION` (the literal classifier output, not just the elapsed-time bucket)
- `POST_SMOKE_USER_PROCESS`

Expected outcome: 200/200 PASS, 0 timeouts of any classifier signature.

Statistical context: at the historical 2.67% pre-fix rate, P[0 timeouts in 200 boots | rate unchanged] ≈ 0.45%. A clean 200/200 is therefore strong evidence the proven mechanism is closed. P[0 timeouts in 200 | rate is exactly zero] = 1.0 trivially; the test is one-sided.

### 5.2 Signature-coverage broadening (as scenario design permits)

The audit's unifying-bug interpretation suggests the other three historical signatures may map to `spawn` / `yield_now` / `wake_thread` paths exercised by other scenarios. The 200-boot loop on `smp_per_cpu_scheduler_smoke` exercises some but not all of those paths. To strengthen confidence in the unifying interpretation, also run:

- 50 boots of any scenario that historically produced `SMOKE_MID_PHASE`, if such a scenario can be identified from the project's signature-history records.
- 50 boots of any scenario that historically produced `POST_SMOKE_USER_PROCESS`.

If those scenarios cannot be cleanly named from existing artifacts, document the gap in the close report and proceed with the 200-boot primary loop alone. Do NOT speculatively run unrelated scenarios; we want signal, not noise.

### 5.3 Stop conditions during verification

- **Any timeout with a historical signature appears.** Stop. Do NOT extend the loop in hopes of statistical zero. Capture the failed-run serial log, classify, write a partial close report, ping for direction. The result is meaningful: it suggests L is necessary but not sufficient, and ADR-016 ratification accelerates from "secondary" to "urgent."
- **A novel signature appears** (one not in the historical four). Stop, capture, classify. This would be a new bug class and warrants its own investigation.
- **Performance regression** observable in mean boot time vs. WEDGE-INV-7's baseline (~30s/boot at steady state). If the fix has unexpectedly slowed the kernel, surface it; debug assertions in release builds should be free, but if release isn't built right or assertions are firing, investigate.

### 5.4 Verification artifacts

Produce:

- `target/verify/WEDGE-FIX-L-results.jsonl` — per-boot classification.
- `target/verify/WEDGE-FIX-L-runner.log` — full output.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — close report. Structure: §summary, §per-boot results, §classification table, §statistical interpretation, §recommendation (close commitment, or escalate to ADR-016 urgent).

---

## 6. Disposition matrix

After verification, pick one for the close report's headline:

### Outcome 1 — Clean discharge
200/200 PASS, no timeouts of any signature (and any §5.2 supplementary scenarios also pass cleanly). The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` is ready to be marked discharged. ADR-016 stays as architectural follow-up; PL ratifies on its own timeline. SC-3B work can begin per the commitment's release condition.

### Outcome 2 — POST_SMOKE_SMP_SCENARIO closed, residual elsewhere
The proven mechanism is closed but one or more of the other three historical signatures appears. The unifying-bug interpretation holds (these are also heap-lock-re-entry class) but L doesn't cover them. ADR-016 ratification becomes urgent; the wedge-fix-before-SC-3B commitment is partially discharged but not complete. PL decides whether to expand L (additional capacity-preservation in ready-queue paths) or accelerate A.

### Outcome 3 — POST_SMOKE_SMP_SCENARIO not closed
The fix didn't close the proven mechanism. Something in the implementation went wrong. Triage. This would be a surprise given the audit's specificity, but if it happens, we go back to mechanism analysis with the failed-fix data as a new input.

### Outcome 4 — Novel signature
A signature not in the historical four appears. New investigation; this dispatch's deliverable is the verification data, not the fix itself.

---

## 7. Cross-references

- `Plans/WEDGE-INV-J1-AUDIT-2026-05-08.md` — the audit naming the bug; canonical source.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` — the project-state framing for the L+A approach.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment this dispatch discharges.
- `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` — the parallel architectural follow-up (Family A); not in scope here.
- `Plans/WEDGE-INV-8-FINDINGS-2026-05-08.md` — the captures whose RIP is now identified.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — canonical signature classifier; reference for §5.1 above.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — the directional research on why this class of bug is recurring in Rust no_std kernels.

---

**End of dispatch.**

*Family L closes the proven mechanism. Family A is the architectural follow-up. Verification result feeds the ADR-016 ratification decision. The wedge-fix-before-SC-3B commitment discharges on Outcome 1; on Outcome 2 it discharges partially with ADR-016 picking up the rest. Either way, the path forward is clear.*
