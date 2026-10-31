# Dispatch: SC-3B source audit — `place_thread_on_cpu` + AP main + reschedule-IPI infrastructure

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** Investigation-stage source-read dispatch. Per Bible Article 14, this is the gate between Outcome B's NMI capture (closed by `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`) and the next fix-design dispatch. Source-read only: no kernel modifications, no commits, no test runs. Goal is to confirm or refute Hp (missing reschedule-IPI on cross-CPU enqueue + non-yielding AP main) at 75% confidence per `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3. Cheap, high-information-density.
**Substrate:** HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Working tree clean. Stash list unchanged from Outcome-B exit state.

---

## 0. Standing instructions

1. **Source-read only.** No `kernel/src/` modifications. No commits. No `cargo k`, `cargo neo-test`, `cargo neo-tty`. Only `git grep`, `view`, file reading.
2. **No stash interactions.** All four stashes remain untouched.
3. **No backup needed at end** unless source files are modified (they should not be). If somehow a working-tree change happens, halt and ping PM.
4. **PM is on call** for clarification on what to look for or how to interpret findings.
5. **The output is a findings document, not code.** Goal is mechanism clarification, not implementation.

---

## 1. Goal

Read three areas of kernel source and report what's there. Specifically:

1. `place_thread_on_cpu` implementation — does it send a reschedule IPI to the target CPU after enqueue?
2. AP-N-main implementation — what does the AP bootstrap thread do after init? Does it yield or halt?
3. Reschedule-IPI infrastructure — does the kernel have any cross-CPU reschedule-IPI mechanism? If yes, who calls it?

Plus a quick-look question: what does `yield_now` do? Does it actually pop the next thread from the ready queue and context-switch?

The findings determine the next dispatch:
- If Hp confirmed (`place_thread_on_cpu` doesn't IPI + AP main doesn't yield aggressively + no other wake mechanism): fix-design dispatch for the kernel fix follows.
- If Hp refuted (resched IPI exists, AP main yields properly): second-pass instrumentation dispatch follows to gather more state.
- If partial (one piece present, another missing): PM ranks-with-data and decides per-case.

---

## 2. The source-read scope

### 2.1 `place_thread_on_cpu`

Likely location: `kernel/src/sched/scheduler.rs` (test-feature-gated per BUILD dispatch §2.4).

Read the function. Report:

1. **Full function source** (or the relevant block — function should be small).
2. **Lock acquisition pattern.** Which lock(s)? Order?
3. **Enqueue mechanism.** `ready.push_back`? `push_front`? Any other ordering logic?
4. **Cpu_id update.** Where is `Thread.cpu_id` set?
5. **IPI or wake-signal.** After enqueue, does the function:
   - Send any IPI to the target CPU?
   - Call any wake-signal function (e.g., `wake_target_cpu`, `notify_cpu`, `send_resched`, etc.)?
   - Do nothing post-enqueue?
6. **Cross-reference:** is there any helper this calls that might IPI?

### 2.2 `wake_thread_on_current_cpu` and related Q4 helpers

Per the SC-3B BUILD dispatch §4.1, this helper exists and is called from `Channel::send` / `Channel::recv` completions. It's "current CPU" by definition, so reschedule-IPI shouldn't be needed. But verify:

1. **Full function source.**
2. **Does it ever push to a non-current CPU's queue?** (Should not, by name.)
3. **Does it cause a yield/reschedule on the current CPU?** (Probably not — Q4 design is "wake on completer's CPU and let the completer continue running per consumer-first wake.")
4. **Are there any sibling helpers (`wake_thread`, `wake_thread_front`) that DO push cross-CPU?** If so, what do they do post-push?

### 2.3 AP-N-main / AP bootstrap thread

Likely location: `kernel/src/init/` or `kernel/src/sched/scheduler.rs` near AP startup. May be named `ap_main`, `ap_entry`, `ap_idle`, `ap_loop`, or similar. The NMI snapshot showed thread names `ap-1-main`, `ap-2-main`, `ap-3-main`, so search for the function that creates threads with that name pattern.

Read the function. Report:

1. **Full function source** (or at least the loop body if the function is long).
2. **Post-init behavior.** After the AP is initialized, what does its main thread do?
3. **Yield discipline.** Does it call `yield_now()` in a tight loop? Periodically? Never?
4. **Halt / wait-for-event.** Does it call `hlt` or similar to wait for interrupt?
5. **Wake mechanism.** If it halts, what wakes it? Timer interrupt? IPI? Anything?
6. **Lifetime.** Does the AP main thread ever exit, or does it loop forever?

### 2.4 Reschedule-IPI infrastructure

Likely location: `kernel/src/arch/x86_64/apic.rs` or `kernel/src/arch/x86_64/ipi.rs` or similar. Search for existing IPI mechanisms.

Report:

1. **Is there any IPI-send function in the kernel?** (E.g., `send_ipi`, `send_resched`, `apic_send_ipi`.)
2. **What vectors are reserved for IPIs?** Specifically, is there a "reschedule" or "wake" vector?
3. **Who calls existing IPI senders?** Search `git grep` for callers.
4. **Is there any timer interrupt that drives per-CPU scheduling?** What's its frequency? Does it tick on all CPUs?
5. **Cross-reference:** any TODO / FIXME comments mentioning IPI, reschedule, cross-CPU wake?

### 2.5 `yield_now`

Likely location: `kernel/src/sched/scheduler.rs` or `kernel/src/sched/mod.rs`.

Report:

1. **Full function source.** Should be small.
2. **Does it cause a context switch?** Specifically, does it pop the next thread from the ready queue and context-switch to it?
3. **What if the ready queue is empty?** Does it return immediately, or halt, or pause?
4. **What's its assembly footprint?** Does the implementation involve `_mm_pause`, `hlt`, or just function calls?

This question matters because the NMI snapshot showed CPU0's stress-sender id=7 currently in `_mm_pause`, with main id=0 ready behind it. If `yield_now` pops the ready queue and context-switches, then main 0 should be running on CPU0, not stress-sender 7. The snapshot might be a brief moment between context switches, but verify the design.

---

## 3. Reporting

Produce `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md`. Structure:

```
# SC-3B source audit findings

## Substrate
[HEAD, no modifications]

## §2.1 place_thread_on_cpu
[Full function source + analysis per the questions]

## §2.2 wake_thread_on_current_cpu and Q4 helpers
[Same shape]

## §2.3 AP-N-main
[Same shape]

## §2.4 Reschedule-IPI infrastructure
[Same shape, including cross-references]

## §2.5 yield_now
[Same shape]

## Hp confirmation status
[Per-question, is Hp supported, refuted, or partial?]

## Recommendation
[For each of: Hp confirmed / Hp refuted / Hp partial — what's the next move?]

## Cross-references
```

The report is reading-and-reporting, not opinionating. Do not propose fixes. Do not write design documents. PM consumes the findings and produces the next dispatch.

---

## 4. Stop conditions

Halt and ping PM if any of these fire:

- Working-tree modification of any kernel source file (this dispatch should not produce any).
- A function the dispatch references doesn't exist (e.g., `place_thread_on_cpu` not found). Halt and surface — PM may have the wrong location and we re-scope.
- The AP main is implemented as separate per-AP entry points (`ap_1_main`, `ap_2_main`) rather than a shared function. Surface this — it changes the audit.
- A finding contradicts something in the SC-3B BUILD dispatch §2-§4 specification (e.g., `Thread.cpu_id` doesn't exist when BUILD §2.3 said it does). Surface this — it implies the BUILD doesn't match what shipped.

---

## 5. Estimated wall time

- Source navigation + reading: ~30-45 minutes for an experienced read of a known codebase.
- Findings document: ~15-30 minutes.
- Total: ~45-75 minutes.

This is a cheap dispatch by design. The synthesis at `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 puts Hp at 75%; this dispatch resolves the remaining 25% with low effort.

---

## 6. Outcomes

### 6.1 Outcome A — Hp confirmed

Source audit shows: `place_thread_on_cpu` does not IPI; AP-N-main does not yield aggressively (or halts without resched-IPI wake); no reschedule-IPI infrastructure exists.

PM produces a kernel-fix design dispatch. Likely shape:
- Add reschedule-IPI infrastructure (vector + handler + send function).
- Modify `place_thread_on_cpu` to send the IPI after enqueue.
- Audit other cross-CPU enqueue sites (probably just `place_thread_on_cpu` exists; verify).
- Optionally: improve AP-main yield discipline as defense-in-depth.

Reference: Linux `resched_curr()` / `smp_send_reschedule()` pattern (Janus §1.4) and Asterinas PR #2488 for analogous Rust-kernel work.

### 6.2 Outcome B — Hp refuted

Source audit shows: resched IPI exists and is called from `place_thread_on_cpu`; AP-N-main yields properly. The bug is elsewhere.

PM produces a second-pass instrumentation dispatch:
- PROGRESS frames in stress orchestrator.
- cpu_id tracking before/after rendezvous.
- Channel endpoint state dump in NMI path.

Run targeted loop, re-rank.

### 6.3 Outcome C — Partial

Some of §2.1-§2.4 supports Hp, others don't. PM ranks-with-data and decides per-case. Most likely sub-cases:

- C1: IPI exists but `place_thread_on_cpu` doesn't call it. Fix is one line + a test. Likely.
- C2: IPI doesn't exist but AP-main yields aggressively. Bug is more subtle; needs second-pass instrumentation.
- C3: IPI exists, `place_thread_on_cpu` calls it, but AP-main isn't responding to it (e.g., interrupt-disabled in the wrong place). Mechanism is interesting; second-pass instrumentation likely.

PM decides on receipt of the findings.

### 6.4 Outcome D — Source not as expected

Some referenced function doesn't exist or has a different shape than the SC-3B BUILD dispatch implied. Halt and ping PM — we may need to re-anchor the investigation against actual current substrate.

---

## 7. Cross-references

**Authority for this dispatch:**
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §4 + §5 — synthesis recommendation; source audit before fix design.
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 — Hp at 75%; this dispatch resolves the remaining 25%.

**Inputs to the audit:**
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` — NMI scheduler snapshot showing CPU2/CPU3 ap-N-main running while stress workers ready.
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` §1.2, §1.4 — Linux resched_curr / wake_q / ttwu_queue_wakelist patterns; Asterinas PR #2488 missing-wake precedent.

**Source files Cerebro is reading (read-only):**
- `kernel/src/sched/scheduler.rs` — likely location of `place_thread_on_cpu`, `wake_thread_on_current_cpu`, `yield_now`.
- `kernel/src/init/` — likely location of AP-main.
- `kernel/src/arch/x86_64/apic.rs` (or similar) — likely location of IPI infrastructure.

**SC-3B specification context:**
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` §2.3 (Thread.cpu_id), §2.4 (place_thread_on_cpu), §4.1 (wake_thread_on_current_cpu) — the spec the implementation should match.
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — design ratifications.

**Substrate:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`.
- Stash list `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment.

**Governing posture:**
- `docs/bible.md` Articles 1.2, 14, 15.2.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4 (consumer-first wake; Q4 confirmed mechanically correct by NMI trace).

---

**End of dispatch.**

*Read-only source audit. Three areas: place_thread_on_cpu, AP-N-main, reschedule-IPI infrastructure. Plus quick-look on yield_now. Goal: confirm or refute Hp at 75%. Estimated 45-75 minutes. Output: Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md. PM consumes, produces next dispatch (kernel fix design if Hp confirmed; second-pass instrumentation if refuted).*
