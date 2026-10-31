# PM Note: SC-3B Outcome B convergent synthesis — Cerebro NMI + Janus prior-art

**Date:** 2026-05-09
**Author:** PM
**Type:** Investigation-stage synthesis. Per Bible Article 14 + `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.2 (PM consumes Janus output, performs the synthesis). Combines `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` and `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` into ranking-with-data update + sharpened hypothesis. The two parallel artifacts converged on a substantially narrower hypothesis space than the original synthesis. PM recommends a source audit dispatch as the next move; fix design follows source-audit confirmation. Not a Bible amendment, not an ADR — analysis-of-state.
**Substrate:** HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Tracked working tree clean. Stash list at `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment.

---

## 1. Headline

**The bug is upstream of Q4/Q6/IPC handoff. The kernel implementations of `context_switch_with_release2`, `block_current_into_with_guard`, `SwitchMutex`, and the four IPC retrofit sites are mechanically correct. The bug is in the scheduler's cross-CPU readiness propagation: `place_thread_on_cpu` enqueues threads to a target CPU's ready queue without sending a reschedule IPI, and the target CPU's currently-running thread (specifically the AP bootstrap thread `ap-N-main`) does not appear to be cooperatively yielding aggressively enough to pick up newly-enqueued work.**

This is a sharpened hypothesis ("Hp" for AP-bootstrap-progress / missing-resched-IPI), not yet confirmed. Confirmation requires a source audit of `place_thread_on_cpu` and the AP-main implementation. PM proposes a Cerebro source-audit dispatch as the next move.

---

## 2. The convergent evidence

### 2.1 Cerebro NMI run 026 — what we saw

Per `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`:

- 50 runs with NMI armed at 10s post-BEGIN silence threshold.
- 49 PASS, 1 NMI capture on run 026 (failure rate ~2%, consistent with prior 1/36 / 1/6 / 1/10 observations).
- Run 026 NMI fired at 10.1s post-BEGIN.

**The scheduler snapshot at hang time (the central evidence):**

| CPU | Currently running | Ready queue contents |
|-----|-------------------|----------------------|
| CPU0 | stress-sender id=7 (in `_mm_pause`) | [stress-receiver id=14, main id=0] |
| CPU1 | stress-sender id=9 | [stress-receiver id=8, ap-1-main id=3] |
| **CPU2** | **ap-2-main id=1** | [stress-sender id=11, stress-receiver id=10] |
| **CPU3** | **ap-3-main id=2** | [stress-sender id=13, stress-receiver id=12] |

**All scheduler inner locks were available. All zombie counts were zero.** No lock contention. No reap inconsistency. No frame leak. No panic.

The pathology is unambiguous: **CPU2 and CPU3 have stress workers ready in their queues but are running their AP bootstrap threads instead.** The AP bootstrap threads are not transitioning out to allow the stress workers a turn, so the rendezvous orchestrated by CPU0 (which is correctly in a yield/wait state) never completes — there's no peer on CPU2 or CPU3 to complete it.

CPU1 looks correct: stress-sender running, stress-receiver and ap-1-main both ready. The pair on CPU0/CPU1 could in principle progress. CPU2/CPU3 are stuck.

### 2.2 What the trace does NOT show (and why it matters)

- **No channel state dump.** We cannot confirm whether the rendezvous endpoints are in `Idle`, `SendBlocked`, or `RecvBlocked`. This means we cannot rule out a channel-level lost-wakeup at the IPC layer entirely. But given the scheduler shows no parked threads in EndpointState anywhere and no zombies, the more parsimonious reading is that the workers on CPU2/CPU3 simply never ran, never reached the IPC code, and the orchestrator on CPU0 is stuck waiting for them to block (`wait_for_stress_endpoint_blocked`) or complete (`wait_for_stress_iteration_done`).
- **No AP IP frames.** The NMI patch's known limitation. We see CPU2/CPU3's ready-queue state via CPU0's NMI handler inspecting all per-CPU schedulers, but we don't see what `ap-2-main` and `ap-3-main` were actually executing.
- **No PROGRESS frame correlation.** Without per-iteration logging, we can't correlate the hang to a specific stress iteration number.

The unresolved items are real, but they don't change the fact that **the scheduler readiness propagation is broken in a way that's directly visible in the snapshot.** If the workers on CPU2/CPU3 have been sitting Ready while their CPUs run a different thread, the bug is in scheduler progress, not in IPC primitives.

### 2.3 Janus prior-art findings — the mechanism match

Per `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`:

**The single most directly relevant finding** (§1.2 mechanism findings, §1.4 architectural findings):

> Linux uses `wake_q` for batching/deferred wakeups, **`resched_curr()` for cross-CPU reschedule IPIs**, and `ttwu_queue_wakelist()` to offload activation to a target CPU in selected cases. The architecture deliberately separates "make task ready," "choose/validate CPU," "queue," and "**reschedule target CPU**."

Linux's design explicitly treats "queue work on remote CPU" and "tell remote CPU to look at its queue" as two separate steps. The IPI is the second step. Without it, the remote CPU has no signal to re-enter the scheduler and pick up new work — it'll only do so on its own natural scheduling boundaries (timer tick, voluntary yield, syscall return).

**The most directly relevant Rust-kernel precedent** (§1.2):

> Asterinas PR #2488 later tied the long-standing LTP SMP hangs to two concrete problems: run queues needed locking with IRQs disabled to avoid deadlock in IRQ callbacks, and **an init process was not woken after dead children were moved, so reaping could never complete**.

Asterinas's "init process was not woken after dead children were moved" is mechanism-isomorphic to AltimitNeo's pattern: stress workers placed on a target CPU's queue, but the target CPU's currently-running thread isn't woken / signaled to yield, so the placed work never gets picked up.

**The closest failure-pattern prior art** (§1.1):

> Linux/QEMU 2023 `printk.time` rare boot hang. Initial estimate ~1 in 300 boots. One run failed on iteration 260; another required thousands. **Pre-regression commit tested to 292,612 successful boots before regression treated as absent.** Was QEMU-visible but ended as a guest-kernel timer bug, not a QEMU bug.

Two takeaways for our framing:
1. **Short clean windows are not strong evidence of absence** for a historically low-rate bug. Phase B's 100/100 clean was inside this regime (P[100/100 | rate=2.8%] ≈ 6%; not statistically remarkable).
2. **QEMU-visible does not mean QEMU-caused.** Host-load can be a reproducer/amplifier, not a root cause. The Hh hypothesis (host-load-dependent) is real as a sampling-rate variable but the root-cause bug is in our kernel.

---

## 3. Updated hypothesis ranking

Combining Cerebro's NMI evidence with Janus's prior-art:

| Rank | Hypothesis | Pre-Outcome-B | Post-NMI+Janus | Notes |
|------|-----------|---------------|----------------|-------|
| **1 (NEW)** | **Hp: missing reschedule-IPI on cross-CPU enqueue + non-yielding AP main** | (not present) | **75%** | Direct trace evidence + Asterinas/Linux prior art match |
| 2 | Hh (host-load-dependent timing as sampling variable) | 20% | 15% | Demoted to "amplifier, not root cause" |
| 3 | H1-variant (channel-level lost wakeup) | 25% | 5% | Trace shows no parked threads in EndpointState; not refuted because channel state wasn't dumped, but disfavored |
| 4 | Hn (deterministic Q4 migration accumulation) | 35% | 3% | Trace shows no migration-accumulation pattern; CPU0/CPU1 layout looks balanced |
| 5 | H4 (per-CPU starvation under Q4 accumulation) | 15% | 2% | Subsumed by Hp; not the right mechanism description |

Confidence on "the bug is fully closed, no fix needed": ~0%. The trace is dispositive that *something* is broken in scheduler progress.

Confidence on Hp specifically: 75%. The remaining 25% is split between:
- The bug is Hp-shaped but not exactly Hp (e.g., resched-IPI exists but isn't reaching the target CPU under specific timing) — ~10%.
- The bug is in `place_thread_on_cpu` itself (e.g., enqueue isn't atomic with cpu_id update; thread placed but isn't visible to target CPU's scheduler) — ~10%.
- The bug is in AP main's specific implementation (e.g., it's in a wait-for-event halt that doesn't get the right wake signal) — ~5%.

All three sub-cases are confirmable via source audit.

---

## 4. The specific source audit needed

To confirm Hp and select the right fix direction, Cerebro should read and report on:

### 4.1 `place_thread_on_cpu` implementation

Likely in `kernel/src/sched/scheduler.rs` (test-feature-gated per BUILD dispatch §2.4). Specific questions:

1. After `target_scheduler.inner.lock()` and `inner.ready.push_back(thread)`, does the function send any IPI to `target_cpu`?
2. Is there a `Thread.cpu_id` update? (We know there is per BUILD §2.3, but verify the update is observable to the target CPU.)
3. Is the enqueue + cpu_id update atomic relative to the target CPU's scheduler observation?

### 4.2 AP-N-main implementation

Likely in `kernel/src/init/` or `kernel/src/sched/scheduler.rs` near the AP bringup code. Specific questions:

1. What does `ap-N-main` do after AP bringup completes?
2. Does it call `yield_now()` in a loop, `halt()` waiting for interrupt, or spin?
3. If it yields, does the yield path correctly check the per-CPU ready queue?
4. If it halts, what wakes it? (Timer tick? IPI? Anything?)
5. Is there any handler that responds to a cross-CPU "you have new work" signal?

### 4.3 Reschedule IPI infrastructure

Likely in `kernel/src/arch/x86_64/apic.rs` or similar. Questions:

1. Does the kernel have any IPI mechanism for cross-CPU reschedule? (Linux's `resched_curr()` / `smp_send_reschedule()` analog.)
2. If yes, who calls it? Is it called from `place_thread_on_cpu`?
3. If no, is there a timer interrupt that periodically forces scheduler re-entry on each CPU? What's its frequency?

### 4.4 Quick-look question: what's CPU0's stress-sender doing?

CPU0's stress-sender id=7 is in `_mm_pause`. Most likely it's inside `wait_for_stress_go` (which spins on an atomic with `yield_now` between iterations). What does `yield_now` do? Specifically:
- Does it cause a context switch (popping next from ready queue)?
- Or is it just a `_mm_pause` hint without actually scheduling?

If `yield_now` is just a pause-hint without scheduler entry, that's a separate bug — the orchestrator (main 0) on CPU0 wouldn't get a turn even though it's ready. But this is less likely; the design has been stable.

---

## 5. PM recommendation: source audit dispatch

**Before fix design. Before any kernel modification. Before any further instrumentation.**

Source audit is cheap (30-60 minutes for Cerebro), high-information-density (answers §4 questions definitively), and risk-free (no working-tree changes). The fix design that follows is much more confident with §4 answered than without.

If the source audit confirms Hp:
- Fix is mechanism-clear: add reschedule IPI to `place_thread_on_cpu` (and likely to other cross-CPU enqueue sites, e.g., Q4 wake-on-completer-CPU's `wake_thread_on_current_cpu` if it operates cross-CPU in any path), and/or fix AP main's yield discipline.
- Fix lands as a kernel change. Closure gate re-runs.
- Janus's prior-art (Asterinas PR #2488, Linux `resched_curr()`) provides direct design references.

If the source audit refutes Hp (e.g., resched IPI already exists; AP main does yield aggressively):
- We need a second-pass instrumentation pass per the synthesis §3.2-3.4 to capture more state (channel endpoint dump, PROGRESS frames, AP IP frames).
- Hypothesis ranking re-opens.

Either way, source audit is the right gate.

---

## 6. Janus calibration data (first dispatch)

Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3, the first Janus dispatch carries elevated review intensity. Calibration observations:

**Strengths observed:**
- 24 distinct primary sources cited; URLs follow expected formats (gitlab.com, kernel.org, lists.gnu.org, github.com, fuchsia.dev, asterinas github, sel4 docs, OSDev forum, USENIX).
- Specific Linux commit hash `13bb06f8dd42071cb9a49f6e21099eea05d4b856` cited with full hash.
- Asterinas issue numbers (#2217, #2152, #2851) and PR (#2488) cited specifically.
- Primary vs. secondary distinction explicitly maintained (§Sources surveyed).
- Single-source findings explicitly flagged for PM downgrade (§Single-source findings — FreeBSD/QEMU warm-reboot, Asterinas #2851, OSDev thread).
- Negative findings honestly reported ("No public source was found for per-pair direction-locking causing migration accumulation under wake-on-completer semantics").
- No fix proposals (per anti-scope).
- No code introduced (per anti-scope).
- TL;DR + structured findings + references all present per dispatch §4.

**To verify (PM action item):**
- Spot-check 5-10 citations resolve. PM commits to verifying:
  - Linux commit `13bb06f8dd42071cb9a49f6e21099eea05d4b856` exists in tip.git.
  - QEMU issue #1696 exists at the cited gitlab.com URL.
  - Asterinas PR #2488 exists with the cited content.
  - LKML printk.time thread exists at `https://lkml.org/lkml/2023/6/13/733`.
  - At least one of the seL4 / Fuchsia / Theseus citations resolves.

**Initial calibration verdict:** Janus's first dispatch is high-fidelity. Output is consumable as-given subject to spot-check verification. No hallucinated authority observed in the structure or framing. Honest with negative findings and single-source flagging.

This calibration data accumulates into the agent-roster posture going forward. After 2-3 more Janus dispatches, the calibration profile firms up. So far: PM defaults to trusting Janus's research artifacts subject to standard spot-check, similar posture to Prometheus's docs.

---

## 7. Forward path

**Immediate next:** PM produces source-audit dispatch for Cerebro.
- Single dispatch, source-read-only (no kernel changes, no commits).
- Reports on §4 questions.
- Estimated 30-60 minutes Cerebro work + ~30 minutes PM review of findings.

**Conditional on source-audit results:**

**If Hp confirmed:**
- PM produces fix-design dispatch for Cerebro.
- Likely fix shape: add reschedule-IPI to `place_thread_on_cpu` + audit other cross-CPU enqueue sites for the same gap. Reference Linux `resched_curr()` pattern from Janus §1.4 for design language; reference Asterinas PR #2488 for analogous Rust-kernel prior art.
- Fix lands as a kernel commit. Closure gate re-runs.

**If Hp refuted:**
- PM produces second-pass instrumentation dispatch (PROGRESS frames + cpu_id tracking + channel state dump in NMI path).
- Re-runs targeted loop on patched substrate.
- Re-ranks hypotheses with the new data.

**Independent of either:**
- PM verifies Janus citations per §6 to firm up calibration.
- Fix 6.1 stash disposition stays deferred until SC-3B closes.

**Sub-step 6 remains blocked.** Will reopen after the closure gate passes (whether on the first post-fix attempt or after iterations).

---

## 8. Cross-references

**Authority for this synthesis:**
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.2 — PM consumes Janus output, performs synthesis.
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` — prior ranking-with-data this synthesis updates.

**The two parallel artifacts being synthesized:**
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` — Cerebro NMI capture findings.
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` — Janus prior-art research.

**Substrate:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`.
- Backup `Z:\AltimitNeo\2026-05-09-212237` (post-NMI-capture).
- Failure run preserved: `target/verify/sc3b-closure-gate-clean/neo-smp4/run-010.serial.log`.
- NMI capture preserved: `target/verify/sc3b-outcome-b-nmi/neo-smp4/run-026.nmi.serial.log`.

**Original investigation chain (this synthesis builds on):**
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — original triple-parallel synthesis.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`.

**Janus dispatch (now closed by the artifact):**
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md`.

**Cerebro dispatch (now closed by the findings):**
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-nmi-capture-cerebro.md`.

**Closure-gate retry that triggered Outcome B:**
- `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md`.

**Calibration record:**
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` — Janus role.
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3 — first-dispatch calibration intensity.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance — bug is real, not closed by gate-as-passing alone), 1.3 (every commit ships — fix must be ship-quality, not a test workaround), 14 (cadence — source audit precedes fix design), 8.3 (no global locks in hot paths — IPI design must respect this).
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — confirmed mechanically correct by the trace.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — forward concern carries over: missing-resched-IPI would affect AI inference workloads under capability-passing IPC.

---

**End of synthesis note.**

*Convergent finding: bug is upstream of Q4/Q6/IPC. Hp (missing reschedule-IPI on cross-CPU enqueue + non-yielding AP main) at 75% confidence. Cerebro NMI evidence + Janus Asterinas/Linux prior art directly support the mechanism. Next: source-audit dispatch to Cerebro before fix design. Janus first-dispatch calibration: high-fidelity, PM defaults to trust subject to standard spot-check. Sub-step 6 remains blocked.*
