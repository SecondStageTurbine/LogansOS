# Dispatch: SC-3B stress-gate failure investigation (parallel × 3)

**To:** Cerebro (coder 1), Prometheus (coder 3), and a third independent Claude session ("Claude-3" hereafter)
**From:** PM
**Date:** 2026-05-09
**Type:** Investigation-stage parallel dispatch. Per Bible Article 14, this is targeted investigation work following the BUILD-stage stop condition that fired during SC-3B Sub-step 5's verification gate. Three agents work independently and in parallel; PM synthesizes the three reports for project-lead ratification.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` (SC-3B Sub-step 5 commit). Latest backup: `Z:\AltimitNeo\2026-05-09-144314`.

Three preserved stashes (potentially load-bearing for this investigation):
- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

---

## 0. Standing instructions

1. **Independence required.** All three agents work in parallel without coordination. Do not read another agent's output even after submitting yours. The value of triple-parallel comes from independent perspectives.

2. **Per-agent output filenames:**
    - Cerebro → `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`
    - Prometheus → `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`
    - Claude-3 → `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`

3. **Execution authority differs.** Cerebro and Prometheus have access to the full source tree and can `git grep`, read files, propose code changes. Claude-3 is a fresh Claude session without prior project context; PL will provide files PL judges relevant. Claude-3's value is fresh analytical perspective, not source-grounded enumeration.

4. **No code changes; no commits; no kernel execution.** This dispatch is analysis-only. Phases B–E are *designed* in this dispatch but *executed* in a follow-up dispatch only by Cerebro (after PM synthesis + PL ratification).

5. **No stash interactions in this dispatch.** Stashes are reference material for the design of Phase B (instrumentation strategy). Do not apply them.

6. **No backup needed.** Working-tree analysis only.

7. **PM is on call.** Each agent may ping PM for clarification at any point during analysis.

---

## 1. Project briefing (for Claude-3 onboarding)

**This section is for Claude-3, which has no prior project context. Cerebro and Prometheus may skip to §2.**

### 1.1 What AltimitNeo is

AltimitNeo is a clean-slate AI-native operating system, written in Rust (`#![no_std]`), x86_64-first (aarch64 later), bootloaded by Limine. It is *not* a Linux derivative; it is a fresh kernel that treats AI/ML as a kernel-resident concern (tensors, models, inference contexts as kernel primitives — see ANSPEC-001).

The project follows a strict phase model with goal milestones. We are currently in **Phase 2 BUILD-3 sub-commit 3** ("SC-3B"). Earlier sub-commits in BUILD-3 introduced per-CPU GDT/IDT/TSS/IST, AP bring-up via Limine MP, and per-CPU schedulers. SC-3B retrofits the IPC handoff path to be SMP-safe under those per-CPU schedulers.

### 1.2 Governing artifacts

- `docs/bible.md` — the project's constitution. Articles 1.2 (correctness > coherence > performance), 1.3 (every commit is intended to ship), 4.3 (every external dependency is a liability), 8.3 (no global locks in hot paths), 8.5 (critical-section discipline), 12.1 (capability-based security from Phase 2), 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY cadence), 15.2 (SAFETY comments on unsafe blocks).
- `docs/ADR-013-phase-2-smp-retrofit.md` — the SMP retrofit ADR. §Q4 is wake-on-completer-CPU. §Q6 is lock-order asm release.
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — THINK-stage scope ratification.
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — ratified PLAN consolidating Cerebro and Prometheus's parallel analyses.
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — the BUILD dispatch, ratified by PL.
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's verification log (the one that recorded the stress-gate failure).
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM's triage analysis, primary input to this dispatch.

PL will provide these to Claude-3 as PL judges fit.

### 1.3 What SC-3B implemented

A 6-sub-step plan landed (Sub-steps 1-5 commits in the working branch):

- **Sub-step 1:** Introduced kernel-owned `SwitchMutex<T>` type. Migrated `Channel.state` and `PerCpuScheduler.inner` to it. Added `Thread.cpu_id` field. Added cross-CPU test placement hook.
- **Sub-step 2:** Added `context_switch_with_release2` asm helper in `kernel/src/arch/x86_64/context.rs`. Added `block_current_into_with_guard` Rust wrapper. Added `smp_block_release_smoke` scenario.
- **Sub-step 3:** Retrofitted `Channel::send` and `Channel::recv` to hold the channel `SwitchMutex` through `block_current_into_with_guard`. Implemented Q4 wake-on-completer-CPU. Added `smp_send_blocks_first_then_completes` and `smp_recv_blocks_first_then_completes`.
- **Sub-step 4:** Retrofitted `Channel::grant` and `Channel::recv_grant` symmetrically. Added `smp_grant_cross_cpu`.
- **Sub-step 5:** Added `smp_cross_cpu_migration_reap` and `smp_cross_cpu_rendezvous_stress` (1000-iteration mixed sender-first/receiver-first test). Wrote verification log. **Failed.**

Sub-step 6 (docs + close) has not started.

### 1.4 The ratified Q6 ABI

Per `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` §1.1 and PL's ratification:

```rust
unsafe extern "C" fn context_switch_with_release2(
    prev_rsp_ptr: *mut u64,        // RDI
    next_rsp: u64,                 // RSI
    channel_lock: *const AtomicBool, // RDX (SwitchMutex byte) — released FIRST
    sched_lock: *const AtomicBool,   // RCX (SwitchMutex byte) — released SECOND
);
```

Asm sequence (x86 Intel syntax, conceptual):
```asm
push rbx; push rbp; push r12; push r13; push r14; push r15
mov [rdi], rsp        # save old rsp to *prev_rsp_ptr
mov rsp, rsi          # switch to new stack
mov byte ptr [rdx], 0 # release Channel.state (Channel-first per pre-thinking I1)
mov byte ptr [rcx], 0 # release PerCpuScheduler.inner
pop r15; pop r14; pop r13; pop r12; pop rbp; pop rbx
ret
```

No fence (x86 TSO sufficient). No RFLAGS save/restore (caller's `without_interrupts` is the contract). Lock release order is Channel-first per `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` Invariant I1.

### 1.5 Q4 wake-on-completer-CPU semantics

When CPU M's `Channel::send` finds a `RecvBlocked` waiter (parked from CPU N), CPU M:
1. Extracts the parked thread from `EndpointState`.
2. Pushes it to **CPU M's** ready queue (NOT CPU N's), with `Thread.cpu_id` updated to M.
3. Yields to it (consumer-first per ADR-011 BUILD-1 amendment).

Same pattern for `Channel::recv`, `Channel::grant`, `Channel::recv_grant`.

---

## 2. The failure

### 2.1 What happened

200-run loop of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. Runs 1-35 PASSed. Run 36 timed out at 180s. Cerebro followed BUILD dispatch §9.5 stop discipline: halted, classified, reported.

### 2.2 The serial log of the failed run

Boot is clean. All 4 CPUs (BSP + 3 APs) come up. Per-CPU schedulers initialize. Default smoke test (ping/pong/hog) runs and reaps cleanly. Then:

```
NEO_TEST/1 event=BEGIN name=smp_cross_cpu_rendezvous_stress phase=dispatch
```

Then nothing. No further serial output. QEMU times out at 180s.

No `BUG:`, no `KERNEL PANIC`, no `NEO_TEST/1 event=PASS`, no `NEO_TEST/1 event=FAIL`. Silent hang.

### 2.3 What 35 prior runs looked like

PASS frame example from a successful run:
```
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

Single-run sanity check before the 200-loop also PASSed identically.

### 2.4 What was tested cleanly before the stress gate

- `cargo k` PASS (18 warnings).
- Single `cargo neo-tty` boot at NEO_SMP=4: clean boot, leak audit clean.
- Single-run of all 6 SC-3B scenarios at NEO_SMP=4: all PASS.
- Full default `cargo neo-test` suite at NEO_SMP=4: 33 PASS frames + 1 expected meta-test FAIL.
- 50/50 supplementary loops for 5 scenarios (`smp_block_release_smoke`, `smp_send_blocks_first_then_completes`, `smp_recv_blocks_first_then_completes`, `smp_grant_cross_cpu`, `smp_cross_cpu_migration_reap`): all 50/50 PASS, zero failures.

The gate failed only at the final 200-run primary stress closure step. The bug is contention-amplified — it doesn't manifest at 50 runs but does at scale.

---

## 3. PM's hypothesis space (your starting point)

PM's triage at `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` §3 ranks six hypotheses. Each agent should engage with these *and* propose any additional hypotheses PM missed.

### H1: Lost wakeup in Q4 wake-on-completer-CPU under contention
Two completers on different CPUs simultaneously try to wake the same parked thread. One push succeeds; the other is silently dropped or duplicates. Stress amplifies.

### H2: Race in `block_current_into_with_guard` between installer publication and asm release
The raw pointer `prev_rsp_ptr` aliases the now-EndpointState-owned Box's interior. Box move semantics + raw pointer through asm boundary may have a UB-adjacent corner case under stress.

### H3: Stress test scenario logic deadlock
Pairing logic, iteration logic, or state cleanup between iterations has a bug that doesn't trigger at low load but does at high load.

### H4: Per-CPU ready-queue starvation under stress
Q4's "completer wakes on its own CPU" can lead to load skew. If the orchestrator is on an idling CPU waiting for progress on a busy CPU, no progress is observed.

### H5: SwitchMutex starvation under stress
Plain spinlock with no fairness; one CPU starves waiting for a lock repeatedly stolen by other CPUs.

### H6: Asm helper missing memory barrier somewhere needed
Synthesis ratified "no fence." Both PLAN agents agreed x86 TSO is sufficient. Possibly wrong; possibly some compiler/CPU reordering not anticipated.

### Possible Hn (open invitation)

PM may have missed something. Each agent should propose any hypothesis not in H1–H6.

---

## 4. Phase A — Source reading and hypothesis enumeration (all three agents)

Each agent reads the relevant source and produces:

- **Source-reading inventory:** what files were read (file paths only).
- **Inventory of relevant code paths:** the asm helper, the Rust wrapper, the IPC retrofit at 4 sites, the stress test scenario itself, the Q4 wake helper, SwitchMutex implementation.
- **Hypothesis enumeration:** ranked list with rationale per hypothesis. Engage with H1–H6; add Hn as needed.
- **For each hypothesis:** what disconfirming evidence would rule it out, what confirming evidence would prove it.

**Cerebro and Prometheus** can `git grep` and read source directly.

**Claude-3** receives source files from PL. Suggested files PL may want to provide:
- `kernel/src/arch/x86_64/context.rs` (asm helper)
- `kernel/src/sched/scheduler.rs` (`block_current_into_with_guard`, Q4 wake helpers)
- `kernel/src/ipc/mod.rs` (4 IPC retrofit sites)
- `kernel/src/sync/switch_mutex.rs` (or wherever SwitchMutex lives)
- The stress test source (location TBD by Cerebro/Prometheus reading).
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` Invariants I1–I6.

If Claude-3 lacks a file, mark it explicitly in the report ("file X not provided; analysis is constrained to inferences from related material").

---

## 5. Phase B — Instrumentation strategy (all three agents propose; only Cerebro implements later)

Each agent designs an instrumentation strategy that would distinguish among the leading hypotheses.

Required output:

- **Per-iteration progress frames:** what should the stress test emit per iteration to enable post-mortem reconstruction.
- **Per-CPU snapshots:** ready-queue depth, current thread, idle/active state at iteration boundaries.
- **Thread.cpu_id tracking:** before block, after asm switch, on wake.
- **Lock contention stats:** SwitchMutex acquisition spin-count or wait-time, per-mutex.
- **Channel state snapshots:** before block, after install, after release.
- **NMI watchdog usage:** does the agent recommend applying `stash@{0}` (NMI watchdog from wedge investigation) for this investigation? If yes, what NMI threshold?

Each strategy should:
- Be feasible: no instrumentation that itself breaks the test (e.g., serial output that affects timing fundamentally).
- Be hypothesis-resolving: each piece of instrumentation should disconfirm or confirm at least one of H1–H6 + any Hn.
- Be ordered: which to add first, what to do if first round of data is inconclusive.

---

## 6. Phase C — Reproduce predictions (all three agents)

Each agent predicts:

- **Reproduction probability:** with the proposed instrumentation, what's the probability the failure reproduces in the next 50 runs? In 200 runs?
- **Per-hypothesis predicted signature:** if H1 is correct, what would the instrumented data show? Same for H2, H3, H4, H5, H6, and any Hn.
- **Heisenbug risk:** could instrumentation hide the bug? If yes, what's the mitigation? (E.g., conditional emission only after iteration N to avoid timing perturbation early.)

This is forecasting, not execution. Cerebro will execute Phase C after PM synthesis + PL ratification.

---

## 7. Phase D — Root-cause hypothesis ranking (all three agents)

Each agent provides:

- **Most likely hypothesis with rationale.** Why this one given the symptoms (BEGIN-then-silence, 35/35 then 1 failure, no panics, no BUGs).
- **Top-3 ranked.**
- **Confidence assessment.** "60% confident" vs. "30% confident" — agents should be calibrated about uncertainty.
- **What would change your mind.** What evidence (from instrumented re-run, source audit, or other) would shift the ranking?

---

## 8. Phase E — Fix design (all three agents propose; only Cerebro implements later)

For the top-2 hypotheses by likelihood, each agent designs a structural fix:

- **Code shape of the fix:** what files change, what semantics change, what new invariant is established or restored.
- **Verification:** how to verify the fix actually addresses the hypothesis (not just makes the test pass through luck).
- **Anti-regression:** what existing scenarios could a buggy fix break.
- **/simplify gate:** is there a smaller fix that addresses the same root cause? Is there a larger fix that addresses related issues at once?

Per Bible 1.3, the fix must be a real ship — not a band-aid.

---

## 9. Output document structure

Each agent's output file follows this skeleton:

```
# SC-3B stress-gate failure investigation ([agent name])

**Date:** 2026-05-09
**Author:** [Cerebro | Prometheus | Claude-3]
**Status:** Investigation analysis. Awaiting PM synthesis with parallel agents' analyses.

---

## 1. Substrate verification

[Each agent confirms what they could read and what was constrained.]

## 2. Phase A — Source reading and hypothesis enumeration

### 2.1 Files read
[List]

### 2.2 Relevant code paths
[Brief inventory with key invariants]

### 2.3 Hypothesis enumeration
For each of H1–H6 + any Hn:
  - Rationale
  - Confirming evidence
  - Disconfirming evidence

## 3. Phase B — Instrumentation strategy

[Designed instrumentation]

## 4. Phase C — Reproduction predictions

[Per-hypothesis predicted signatures, heisenbug risk]

## 5. Phase D — Root-cause hypothesis ranking

[Ranked, calibrated]

## 6. Phase E — Fix design (top-2 hypotheses)

[Per-hypothesis fix sketch]

## 7. Risks and unknowns

[What you couldn't analyze, what you'd want PM to address]

## 8. Cross-references

[Files cited]

---

**End of investigation.**
```

The structure is a guideline. If an agent finds a better organization, propose it.

---

## 10. Specific anti-scope

Do not, in this dispatch:

- Write code or propose specific file diffs (Phase E is *fix design*, not *fix implementation*).
- Apply the wedge investigation stashes (Phase B is *instrumentation strategy*, not *applying the stash*).
- Run `cargo neo-test` or any kernel boot.
- Amend the Sub-step 5 commit.
- Propose lowering the 200/200 closure standard.
- Propose reverting Sub-steps 1-4.
- Coordinate with the other two agents.

---

## 11. Post-completion actions

After producing your investigation document:

1. Notify PM that your investigation is ready for review.
2. PM waits for **all three** agents' investigations before producing the synthesis.
3. Do not read the other agents' files even after submitting yours.
4. PM produces `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` comparing the three analyses across all five phases.
5. PM brings synthesis to project lead for ratification.
6. Project lead ratifies (or modifies) the consolidated investigation plan.
7. PM drafts a follow-up dispatch for Cerebro to execute Phases B (instrument), C (reproduce), and E (fix + re-verify).

No commits, no backup needed for this dispatch.

---

## 12. Cross-references

Mandatory:
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md` — PM's triage; primary input.
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — Cerebro's verification log.
- `target/verify/sc3b-gate-06-stress-neo-smp4/run-036.failure.serial.log` — the failure artifact.
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` — the BUILD dispatch including stress test specification (§6.2).
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — the ratified PLAN.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6.

Recommended:
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — Invariants I1–I6.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` — Cerebro's PLAN.
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` — Prometheus's PLAN.
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — THINK ratification.
- `docs/bible.md` Articles 1.2, 1.3, 4.3, 8.3, 8.5, 12.1, 14, 15.2.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — why this work matters for AI-native primitives.
- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — wedge-fix β outcome (precedent for handling commitment-met-but-need-investigation patterns).
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — wedge-fix discharge, statistical framing precedent.

For Claude-3 specifically, PL will provide source files PL judges relevant. Suggested:
- `kernel/src/arch/x86_64/context.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/ipc/mod.rs`
- `kernel/src/sync/switch_mutex.rs` (or wherever SwitchMutex lives — Cerebro decided file location during Sub-step 1).
- The stress test source.

---

**End of dispatch.**

*Three working-tree artifacts (one per agent): `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`, `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`, `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`. Five phases, five proposals each. Independent parallel analysis. PM synthesizes after all three return. Synthesis is input to project-lead ratification, then a follow-up execution dispatch to Cerebro for Phases B-C-E.*

*PM is on call for clarification during any agent's analysis but will not relay between agents. Investigation duration estimate: 1-2 days for analysis (fastest agent waits for slowest); execution by Cerebro 2-4 days after synthesis ratifies.*
