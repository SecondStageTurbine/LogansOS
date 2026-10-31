# PM-NOTE: SC-3B Sub-step 5 stress-gate failure triage

**Date:** 2026-05-09
**Author:** PM
**Status:** Triage analysis. Not a dispatch (yet); becomes input to the project-lead decision on investigation shape and the eventual investigation dispatch.
**Triggered by:** Cerebro's report of stop-condition firing at run 36 of `smp_cross_cpu_rendezvous_stress` 200-loop NEO_SMP=4 closure gate. Substrate: HEAD `a61e318` (Sub-step 5 commit), backup `Z:\AltimitNeo\2026-05-09-144314`.

---

## 1. Stop discipline assessment

Cerebro followed BUILD dispatch §9.5 and §10 precisely:
- 200-run loop hit one timeout at run 36 (35/35 clean prior).
- Halted immediately; did not push through to retry-and-pray.
- Did not run NEO_SMP=2 supplementary loop (correct per stop rule).
- Did not start Sub-step 6 docs/close work (correct).
- Captured failure serial log at `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log`.
- Wrote verification log with classification.
- Did NOT amend the Sub-step 5 commit despite the literal `\n` artifact (correct per "no amend without explicit request").

**This is exactly the discipline the project's wedge-investigation arc proved out.** The 1/36 failure is *signal*, not flake. Treating it as flake and retrying would rebuild the bad pattern that necessitated the wedge fix. PM affirms the stop.

---

## 2. Failure signal extraction

### 2.1 What we have

- **Boot is clean.** All 4 CPUs come up. AP per-CPU schedulers all initialize. Default smoke (ping/pong/hog) runs and reaps cleanly.
- **Test BEGIN fires.** `NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch`.
- **Then silence.** No PASS, no FAIL, no BUG, no KERNEL PANIC, no further serial output. QEMU times out at 180s.

### 2.2 What this rules in/out

Rules out:
- **Boot or AP-bringup race.** Boot completed identically to the 35 prior successful runs.
- **Build defect.** `cargo k` is clean; same image as the 35 successes.
- **Catastrophic memory/page-table corruption.** Would surface as `KERNEL PANIC` or `BUG:` not silence.
- **Default scenarios broken.** All passed in pre-Sub-step-5 default suite check.
- **Wrong-CPU migration class.** No PASS frame existed to inspect `resumed_cpu`.

Rules in:
- **Hang inside the stress test scenario itself**, occurring after BEGIN was emitted but before any iteration could complete (or before the test could emit an iteration progress frame, if it emits any).
- **Lost wakeup**, **deadlock with waiter unpublished**, **deadlock with waiter published but not woken** — all three have the same external signature: silent hang with no PASS/FAIL.
- A **stress-amplified rare race**: 35 runs OK, run 36 hangs. The race window is small enough to clear most runs but large enough to occasionally trip.

### 2.3 The narrowing question

The stress scenario emits `phase=dispatch` in BEGIN. From the BUILD dispatch §6.2 sketch, "dispatch" likely means the test thread is dispatching iteration work (placing senders/receivers, kicking off the rendezvous loop). The test could be hanging:

- **(a) Before the first iteration runs.** Test setup hits a deadlock or hang.
- **(b) During iteration N for some N ≥ 0.** A sender/receiver pair fails to rendezvous; the orchestrator waits forever.
- **(c) After all iterations completed but PASS not emitted.** The terminal-frame emission path itself hangs.

We cannot distinguish (a)/(b)/(c) from the current artifact alone — the test does not appear to emit per-iteration progress frames, or buffering is hiding them. **Investigation must add instrumentation to distinguish.**

---

## 3. Hypothesis space

Ranked by PM's prior; not exhaustive. Investigation should refine.

### H1: Lost wakeup in Q4 wake-on-completer-CPU under contention

**Mechanism:** Q4 pushes the woken thread to the completer CPU's ready queue with cpu_id update. Under contention, two completers on different CPUs may simultaneously try to wake the same parked thread (e.g., the test creates a state where two senders both think they are the completer for the same blocked receiver). One push succeeds; the other is silently dropped or duplicates the thread, leaving an inconsistent state.

**Why ranked first:** Q4 is the newest mechanism. Under K=1000 iterations across 4 CPUs, contention windows are millions of times larger than under 50/50 supplementary loops. Concurrent rendezvous completion is exactly what the stress test is designed to exercise.

**Disconfirming evidence required:** instrumentation showing that for each rendezvous, exactly one Q4 wake fires per parked thread, and the parked thread leaves Channel state exactly when the wake fires.

### H2: Race in `block_current_into_with_guard` between installer publication and asm release

**Mechanism:** The installer publishes `prev_thread` into `EndpointState`. Then `mem::forget` happens. Then asm switches stack and releases Channel lock. After Channel release but before scheduler release, CPU M can extract prev_thread. CPU M then pushes prev_thread to its ready queue via Q4 wake. But: prev_thread.rsp was set by the asm BEFORE the lock release? Let me trace:

- `mem::replace(&mut s.current, next)` extracts old thread as `prev_box`. `prev_box.rsp` is whatever stale value was last in there.
- `prev_rsp_ptr = &mut prev_box.rsp` — captures pointer.
- `installer(state, prev_box)` — moves `prev_box` into `EndpointState`. **`prev_rsp_ptr` now points into the EndpointState!** This is a raw pointer aliasing the now-EndpointState-owned Box's interior.
- `mem::forget` guards.
- Asm: saves callee-regs to old stack, then `mov [rdi], rsp` writes current rsp to `*prev_rsp_ptr` — which is the `rsp` field inside the EndpointState's `prev_box`.

This *should* work. The pointer was captured before the move; the move doesn't invalidate it because Box's content is heap-pinned. But raw pointer aliasing through a Box move is the kind of thing that has UB-adjacent corner cases.

**Suspicion:** under stress, CPU M might extract prev_box from EndpointState *before* the asm has finished writing prev_rsp_ptr. CPU M then sees stale `rsp` and runs prev_thread with bad register state, leading to a hang (e.g., stack pointer invalid, eventually triple fault... but no PANIC seen, so probably not triple fault).

Actually, no — Channel lock is held during the asm's `mov [rdi], rsp` operation? Let me re-check:

Looking at the asm:
```
mov [rdi], rsp     # save old rsp to *prev_rsp_ptr
mov rsp, rsi       # switch to new stack
mov [rdx], 0       # release Channel lock
mov [rcx], 0       # release scheduler lock
```

So `[rdi]` write happens BEFORE Channel release. So when CPU M extracts prev_box, prev_box.rsp is already the saved rsp. That's consistent.

So H2 is probably wrong. But the raw-pointer-aliasing-through-Box-move pattern is worth auditing in code.

**Disconfirming evidence required:** code audit of `block_current_into_with_guard` showing the prev_rsp_ptr ordering relative to installer's move-of-prev_box and the asm's write-to-*prev_rsp_ptr.

### H3: Test scenario logic deadlock

**Mechanism:** The stress test creates pairs of senders and receivers. If pairing logic is wrong (e.g., sender on CPU 0 paired with receiver on CPU 0, both expecting the OTHER to be on a different CPU), they may both block on each other in a way that doesn't deadlock at low load (because some completion path has slack) but does at high load.

**Why ranked here:** the test was just written. Bugs in test code are common. Tests under stress reveal test bugs.

**Disconfirming evidence required:** read the test code, verify the pairing logic, confirm there's no test-internal state that can wedge across iterations.

### H4: Per-CPU ready queue starvation under stress

**Mechanism:** With Q4 wake-on-completer-CPU, all rendezvous completions push to the completer's queue. Under heavy contention, one CPU may end up the completer for most rendezvous and accumulate a long ready queue while other CPUs idle. This isn't strictly a hang, but if the orchestrator is on an idling CPU waiting for progress on a busy CPU, the orchestrator never sees progress.

**Why ranked here:** Q4 is intentionally biased toward the completer CPU. That's the design. But "completer wakes on its own CPU" can lead to load skew under stress; the design assumed iterations would balance on average. Maybe they don't.

**Disconfirming evidence required:** instrumentation showing per-CPU ready-queue depth over time during the stress test.

### H5: SwitchMutex starvation under stress

**Mechanism:** Plain spinlock with no fairness; under heavy contention, one CPU may starve waiting for a mutex repeatedly stolen by other CPUs.

**Why ranked low:** SwitchMutex is structurally identical to spin::Mutex (which has worked in production for months). Stress doesn't change the lock primitive's behavior. Unless SwitchMutex's implementation has a bug not present in spin::Mutex.

**Disconfirming evidence required:** code audit of SwitchMutex against spin::Mutex; confirm Acquire/Release semantics match.

### H6: Asm helper missing memory barrier somewhere it's needed

**Mechanism:** Synthesis §1.4 ratified "no fence." Both PLAN agents agreed x86 TSO is sufficient for AtomicBool::store(false, Release). But: the asm sequence has `mov rsp, rsi` between the stack switch and the lock releases. Stack switches are not memory operations; they don't fence. If the compiler/CPU somehow reorders the lock-release stores against the stack-switch-derived state... no, that doesn't make sense, stack switches don't have memory dependencies that a CPU would reorder against.

**Why ranked very low:** x86 TSO + plain stores is well-understood. Linux uses this exact pattern.

---

## 4. Recommended investigation arc

This is a smaller investigation than the wedge arc — narrower scope, cleaner substrate (post-wedge-fix and post-A1), more recent commits to bisect.

### Phase A: Read the stress test source (~0.5 day)

Cerebro reads the stress test source he just wrote and confirms:
- What the dispatch phase does.
- Whether it emits per-iteration progress frames.
- Pairing logic for senders/receivers.
- Termination condition.

If the test has obvious self-deadlock potential (H3), fix it and re-run gate.

### Phase B: Add instrumentation (~0.5-1 day)

Add diagnostic emissions to the stress test:
- Per-iteration progress frame: `NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress iter=<n> sender_cpu=<m> receiver_cpu=<k>`
- Per-CPU ready-queue depth snapshots at iteration boundaries (or every N iterations).
- Thread.cpu_id snapshots at block and resume.

Optional: enable the existing wedge-investigation NMI watchdog stash (`stash@{0}`) to provide a forced kernel panic if the kernel hangs > 60s. This converts the silent hang into a panic with stack trace from each CPU. **Do NOT commit the watchdog; apply for investigation only.**

### Phase C: Reproduce with instrumentation (~0.5-2 days)

Re-run the 200-loop with instrumentation enabled. Failure may take 50-200 runs to manifest. Capture instrumented serial logs.

If failure manifests at iteration N:
- Inspect last PROGRESS frame: which iteration, which CPUs.
- Inspect ready-queue depths at that moment.
- Inspect Thread.cpu_id sequence for the affected threads.

If watchdog NMI fired: capture stack traces from all 4 CPUs. Identify which CPU is stuck where.

### Phase D: Form root-cause hypothesis (~0.5 day)

Using Phase C data, narrow to one of H1-H6 (or surface a new one). PM and Cerebro discuss; project lead ratifies hypothesis.

### Phase E: Fix and re-verify (~1-2 days)

Apply structural fix (not empirical retry). Run the full gate again per BUILD dispatch §9.

If the fix passes 200/200, Sub-step 6 (docs + close) opens. If not, return to Phase D.

### Total estimate

3-5 days. Investigation is bounded by the relatively narrow surface of SC-3B's new code (Q6 helper + Q4 wake + SwitchMutex + 4 IPC retrofit sites + stress test + test placement hook).

---

## 5. What NOT to do

- **Do not retry the gate without classification.** This is the wedge-investigation lesson, hard-earned.
- **Do not amend the Sub-step 5 commit.** Cerebro's discipline here is correct; the literal `\n` artifact is a cosmetic issue for a future cleanup commit, not a re-history-write.
- **Do not revert Sub-steps 1-4.** That work is good; the bug is somewhere in Sub-step 5 logic (stress test, Q4 wake under contention) or in interaction between earlier sub-steps and the stress load. Bisect, don't carpet-bomb.
- **Do not lower the gate threshold.** 200/200 was the explicit closure standard ratified at PLAN. Lowering it would cement a flake-pattern decision under pressure.
- **Do not start Sub-step 6.** Per dispatch §10, stop conditions hold until investigation completes.
- **Do not run NEO_SMP=2 supplementary loop yet.** It might pass (lower contention reduces failure rate) and create false confidence. Wait until NEO_SMP=4 closes.

---

## 6. Project lead decision points

PL needs to decide:

1. **Investigation scope:** Phase A only, A+B, or full A→E? PM recommends full A→E. Phase A alone is unlikely to reveal H1/H2 (those need instrumentation).
2. **Investigation owner:** Cerebro is the natural choice (has fresh implementation context). Prometheus could parallel-investigate but the calibration data from the PLAN stage suggests Cerebro is more reliable for this work.
3. **Watchdog stash usage:** apply `stash@{0}` (NMI watchdog) for investigation? PM recommends yes — it converts hangs into actionable panics.
4. **Bible 14 /simplify gate on the investigation:** any compression? PM thinks A→E is the minimum to find root cause; compressing further risks insufficient signal.
5. **Communication shape:** does PM dispatch the investigation to Cerebro as one document covering Phases A-E, or in two stages (A-B then C-E based on Phase A findings)? PM defaults to one-document; PL may prefer staged.

---

## 7. Hopes and concerns

**Hopes:**
- The bug is in stress test logic (H3) — easiest fix, no kernel changes.
- Or the bug is a small race in Q4 (H1) catchable with a 5-line change.

**Concerns:**
- The bug is in Q6's asm-Rust boundary (H2). That's the kind of thing that can require architectural rework, blowing scope.
- The bug is in SwitchMutex itself (H5). That'd be embarrassing but fixable.
- The bug doesn't reproduce with instrumentation enabled (Heisenbug). Then we need creative tactics — possibly enable instrumentation only after iteration N where N is set high enough to trigger but low enough to capture.

PM is reasonably optimistic this resolves in 3-5 days. The substrate is clean (post-wedge-fix); the changes are bounded; the stress test is contained; the failure signature is amenable to instrumentation.

---

## 8. Forward queue if PL ratifies investigation

1. **PM drafts investigation dispatch** for Cerebro covering Phases A-E.
2. **Cerebro executes** the investigation, ping PM at hypothesis-confirmation point (between Phases C and D).
3. **PM and Cerebro discuss** root cause; PM brings to PL for ratification of fix shape.
4. **PL ratifies fix.**
5. **Cerebro implements fix** (probably 1 commit on top of Sub-step 5).
6. **Re-run gate** per dispatch §9.
7. **If gate passes:** Sub-step 6 opens (docs + close). If not: return to step 2.

The investigation may also produce learning artifacts: an ADR-013 amendment with the discovered race + fix, or a Bible amendment if the issue reveals a design discipline gap.

---

## 9. Cross-references

- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's verification log; primary input.
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` — failure artifact.
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — BUILD dispatch (§9.5 stop condition fired correctly; §10 stop conditions all honored).
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — ratified PLAN (200/200 closure standard from §1.9).
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — wedge-fix β outcome (precedent for handling commitment-met-but-need-investigation patterns).
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — wedge-fix discharge, statistical framing precedent.
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` — Sub-step 5 commit. Not to be amended; investigation produces follow-up commits.

---

**End of triage.**

*Stop discipline correct. Failure is signal, not flake. Recommend Phases A-E investigation by Cerebro, ~3-5 days, structural fix on top of Sub-step 5. PM standing by to draft investigation dispatch on PL ratification.*
