# Dispatch: SC-3B PLAN-stage checkpoint analysis

**To:** Cerebro (coder 1) and Prometheus (coder 3) — parallel analysis, independent work, no coordination between agents
**From:** PM
**Date:** 2026-05-09
**Type:** PLAN-stage analysis dispatch. Per Bible Article 14's four-stage cadence (OBSERVE → THINK → PLAN → BUILD/VERIFY), this is the PLAN stage for SC-3B. Project lead has ratified the THINK-stage scope (Option ii: Q4 + Q6 + IPC retrofit + cross-CPU scenarios) per `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`. This dispatch asks both agents (Cerebro and Prometheus) to produce parallel PLAN-stage analysis documents addressing the eight open implementation questions enumerated in that note's §6. PM synthesizes after both return.
**Substrate:** Project HEAD `79b35f4` (`docs: Plans/INDEX.md — wedge-fix discharge + β outcome artifacts`). Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

---

## 0. Standing instructions

1. **No code changes.** This dispatch produces ONE working-tree artifact PER AGENT: `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` (Cerebro) or `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` (Prometheus). No kernel source changes; no commits; no build verification needed.
2. **Independence required.** Cerebro and Prometheus do this analysis in parallel and independently. Each agent must not read the other's output file (if it already exists in `Plans/`) and must not coordinate before submitting. The value of parallel analysis comes from independent perspectives; PM synthesizes after both return. If you finish first and the other's file is on disk, do not open it — ping PM if you're unsure.
3. **No stash interactions.** Stashes remain untouched.
4. **No backup needed.** Working-tree-only artifacts in Plans/, captured by the next regular backup.
5. **Source-reading is in scope.** Each agent should `git grep`, read scheduler/IPC source, read ADR-013, read the SC-3A pre-thinking note, and any other context needed to ground the analysis. The deliverable is implementation-grounded analysis, not abstract design.

---

## 1. Goal

Each agent produces their own PLAN-stage analysis document at a per-agent filename:

- Cerebro → `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md`
- Prometheus → `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md`

Each document proposes concrete answers to the eight open implementation questions for SC-3B. The two documents are written independently and submitted to PM. PM then produces a synthesis note (`Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`) comparing the two proposed answers per question, surfacing convergences and divergences. Project lead ratifies based on the synthesis.

The PLAN docs are **proposals with rationale**, not specifications. Project lead may accept, modify, or reject any proposal. Each document should make ratification efficient by giving each question a recommended answer plus the alternatives considered and why they were rejected.

---

## 2. The eight open questions

Each question below is from `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` §6, with elaboration on what's needed in the PLAN doc.

### 2.1 Q6 ABI shape

The `context_switch_with_release` asm helper has three candidate shapes per `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` §5.1:

- **(a) Const-generic array:** `unsafe fn context_switch_with_release<const N: usize>(prev_rsp_ptr: *mut u64, next_rsp: u64, locks: &[*const AtomicBool; N]) -> i64`
- **(b) Fixed-shape struct:** `ReleaseSet { locks: [*const AtomicBool; 4], count: u8 }` passed by value
- **(c) Per-call-site specialization:** `context_switch_with_release2(prev_rsp_ptr, next_rsp, lock1, lock2)` etc.

PLAN should:

- Recommend one shape.
- Explain the trade: (c) is simplest if the only caller is the IPC path; (a) is most general; (b) is middle ground.
- For SC-3B specifically, all `block_current_into` callers hold exactly two locks (Channel mutex + scheduler lock). Confirm this by enumerating callers in the kernel source.
- Consider whether SC-5's blocked-thread-kill walk (Q8.c) might need a different shape later, and whether SC-3B's choice forecloses or accommodates that.
- Address how unsafe the asm-Rust boundary is per Bible 15.2 (SAFETY comments) — the chosen shape must be tractable to comment.

### 2.2 Memory ordering

The asm performs lock releases as plain stores. Open question: is `mfence` (or `lfence`/`sfence`) needed between the two release stores, or does x86's natural store ordering suffice for `spin::Mutex`-equivalent Release semantics?

PLAN should:

- Read the actual `spin::Mutex` implementation Cerebro uses in `kernel/src/mm/heap.rs` (now post-A1) and elsewhere. Identify the unlock primitive (likely `AtomicBool::store(false, Release)` or equivalent).
- Explain whether x86's TSO ordering makes the explicit fence unnecessary (typically yes — x86 stores are ordered in program order, and the release memory ordering on x86 compiles to a plain MOV for AtomicBool::store).
- If a fence IS needed for some reason (e.g., compiler reordering the asm's stores against subsequent code), specify which fence and why.
- Reference: Linux's `spin_unlock` on x86 is a plain MOV + a compiler barrier, no CPU fence.

### 2.3 Panic-safety of the `mem::forget`-then-asm window

Per pre-thinking note §5.3: the window between `mem::forget(channel_guard)` / `mem::forget(sched_guard)` and `context_switch_with_release(...)` must contain no panicable operations. If a panic fires there, the locks are forgotten but never released — system locks up.

PLAN should:

- Audit the proposed `block_current_into` body for panicable operations between forget and asm-call.
- Recommend a code structure that minimizes the window (probably: prepare release set, forget guards, asm-call, all in a single `unsafe` block with no intervening operations).
- Note any helper code that might compile-to a panic path (e.g., array indexing, integer overflow in debug builds, unwrap on Option). Eliminate or move outside the window.
- Consider whether `#[track_caller]` or `core::intrinsics::abort` could be used as a defense-in-depth backstop; the PLAN may recommend or reject this.

### 2.4 Cross-process bookkeeping ordering

Per pre-thinking note §5.4: `cross_process_prep` (CR3 update) and `cross_thread_kstack` (TSS.rsp0 / per_cpu.kernel_rsp update) currently run inside the SCHED-locked block. Under Q6, these must complete BEFORE the asm release because the new thread's stack is its own kernel stack — TSS.rsp0 must already point to it before the stack switch.

PLAN should:

- Confirm the current location of `cross_process_prep` and `cross_thread_kstack` calls in `kernel/src/sched/scheduler.rs`.
- Specify the Q6-correct ordering: lock both, do bookkeeping, prepare release set, forget guards, asm-call.
- Identify any state that must be captured into the asm-call's input (e.g., `next_rsp` is read from `sched.current.rsp` after `mem::replace`; ensure that's done before forget).
- Note: the per_cpu.kernel_rsp write semantics matter — it's read by the SYSCALL entry path, which can fire from ring-3 code immediately after the new thread starts running. The new value must be visible by then.

### 2.5 Block-target enumeration

Open: which functions in the kernel call `block_current_into`?

PLAN should:

- `git grep block_current_into kernel/src/` and enumerate all call sites.
- Group call sites by call shape: which release the same set of locks, which release different sets.
- Confirm or refute the assumption that all callers hold exactly Channel mutex + scheduler lock.
- If any callers don't fit the assumption (e.g., a caller that holds only the scheduler lock, or holds three locks), surface it — this affects Q6 ABI choice.

### 2.6 Test scenario set

Open: which cross-CPU IPC patterns to test at NEO_SMP=4?

PLAN should propose a scenario list. Candidates:

- **`smp_send_blocks_first_then_completes`** — sender on CPU A blocks on a full channel, receiver on CPU B drains and unblocks; verify sender wakes and runs on CPU B (completer-CPU per Q4).
- **`smp_recv_blocks_first_then_completes`** — receiver on CPU A blocks on empty channel, sender on CPU B sends; verify receiver wakes and runs on CPU B.
- **`smp_grant_cross_cpu`** — grantor on CPU A blocks on `sys_grant`, recipient on CPU B `sys_recv_grant`s; verify capability arrives with correct rights and recipient runs on CPU B.
- **`smp_kill_during_rendezvous`** — kill the parker thread while it's parked in EndpointState; verify Q8.c-precursor behavior or document that this is deferred to Q8.c.
- **`smp_multi_grant_chain`** — A grants to B, B grants derived to C, all cross-CPU; verify chain integrity.
- **`smp_concurrent_rendezvous_storm`** — N senders + N receivers across all CPUs at once, verify no deadlock and all messages delivered.
- **Anti-regression coverage:** all existing single-CPU IPC scenarios pass at NEO_SMP=4.

PLAN should select 4-8 scenarios with rationale, and identify any scenario that requires Q8.c support to test (those are deferred to a future sub-commit).

### 2.7 Verification methodology

Open: cargo-driven verification loop shape.

PLAN should propose:

- Boot count per scenario (mirror L's 200/200 for the primary, 50/50 for supplementary).
- NEO_SMP value(s) to test (likely just 4 for primary; consider NEO_SMP=8 or NEO_SMP=2 as supplementary if hardware permits).
- Pass criteria per scenario (terminal frame format, exit code, any structural invariants like leak audit clean).
- Anti-regression set (which existing scenarios must pass at NEO_SMP=4 to confirm no regression).
- Statistical confidence target (mirror the discharge artifact's 0.45% framing — what evidence threshold do we need to be comfortable shipping?).
- Failure modes to watch for (deadlock, livelock, lost message, capability rights drift, killed-during-rendezvous corner cases).

### 2.8 Composition with A1 and 8.5.3 advancement

Two related sub-questions:

**A1 composition:** A1 is in main; the global heap is now IRQ-safe. Q6's asm release operates on `spin::Mutex` instances, NOT on the heap allocator's lock — but `cross_process_prep` and `cross_thread_kstack` may indirectly allocate (e.g., if process registry growth happens; though in practice neither allocates). PLAN should:

- Verify that `cross_process_prep` and `cross_thread_kstack` do not allocate or touch the heap during the SCHED-locked window.
- Verify that the asm helper's saved/restored interrupt state is consistent with the without_interrupts entry to `block_current_into`.
- Note any A1 / Q6 interaction that warrants attention during BUILD.

**Bible 8.5.3 advancement:** Article 8.5.3 says "scheduling operations never trigger heap allocation." A2 is the formal conformance milestone (Phase 2 BUILD-4-or-later). Does Q6 incidentally advance 8.5.3? PLAN should:

- Note whether Q6's cleaner lock-order discipline reduces 8.5.3 violation surface, even if not eliminating it.
- Confirm Q6 doesn't introduce NEW 8.5.3 violations (e.g., by adding heap-using code to the SCHED-locked window).
- Document the partial advancement in the PLAN for the BUILD's commit message later.

---

## 3. Output document structure

Each agent's output file (`Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` for Cerebro or `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md` for Prometheus) should follow this structure:

```
# SC-3B PLAN-stage checkpoint analysis ([agent name])

**Date:** 2026-05-09
**Author:** [Cerebro | Prometheus]
**Status:** PLAN-stage analysis. Awaiting project-lead ratification (post-synthesis with the parallel agent's analysis).
**Authority:** Project lead. PM dispatched; [agent] proposes; project lead ratifies after PM synthesis.

---

## 1. Substrate verification

[Quick confirmation: HEAD is 79b35f4, source tree matches, ADR-013 §Q4+§Q6 are still the relevant substrate, etc.]

## 2. Open questions and proposed answers

### 2.1 Q6 ABI shape — recommendation: [a/b/c]
[Rationale, alternatives considered, decision criteria.]

### 2.2 Memory ordering — recommendation: [no fence / specific fence]
[Rationale, reference to spin::Mutex impl, x86 TSO discussion.]

### 2.3 Panic-safety of the forget-then-asm window — recommendation: [code structure]
[Audit of panicable operations, defensive structure proposal.]

### 2.4 Cross-process bookkeeping ordering — recommendation: [ordering specification]
[Current locations, Q6-correct ordering, state-capture requirements.]

### 2.5 Block-target enumeration — confirmed call sites:
[List of all block_current_into callers with their lock-sets.]

### 2.6 Test scenario set — recommendation: [N scenarios named]
[Each scenario: name, what it tests, prerequisites, deferred-to-Q8.c flags.]

### 2.7 Verification methodology — recommendation: [loop shape]
[Boot counts, NEO_SMP values, pass criteria, anti-regression set, statistical target.]

### 2.8 Composition with A1 and 8.5.3 advancement — recommendation: [composition is clean / specific concerns]
[A1 composition, 8.5.3 partial advancement.]

## 3. Implementation sketch

[Optional: pseudocode for the new block_current_into body, asm helper signature, IPC retrofit sites. Helps PL visualize what BUILD will produce.]

## 4. Risks and unknowns

[Anything Cerebro flagged during analysis that doesn't fit cleanly into the eight questions but should be on PL's radar.]

## 5. Cross-references

[Links to all source files, ADRs, and Plans/ artifacts referenced.]

---

**End of PLAN.**
```

The structure is a guideline, not a straitjacket. If an agent finds a better organization during analysis, propose it — PM is happy to defer to whatever shape makes the document most usable for synthesis and ratification. **The two agents may organize their documents differently from each other; that is expected and fine.**

---

## 4. Source files each agent should consult

Mandatory:

- `kernel/src/sched/scheduler.rs` — current `block_current_into`, `reap`, lock-acquisition patterns.
- `kernel/src/sched/process.rs` — process registry locks and how they compose.
- `kernel/src/ipc/` (or wherever IPC syscalls live) — `sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant`.
- `kernel/src/arch/x86_64/per_cpu.rs` — current per-CPU access primitives.
- `kernel/src/arch/x86_64/syscall.rs` — SYSCALL entry, kernel_rsp handling.
- `docs/ADR-013-phase-2-smp-retrofit.md` — Q4, Q6 substrate.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — prior PM analysis of Q6 composition.
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — the THINK note this PLAN follows from.

Recommended:

- `Plans/PM-NOTE-2026-05-08-beta-outcome.md` — context for A1 composition.
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — context for what's already in main.
- `docs/bible.md` Articles 1.2, 1.3, 8.3, 8.5, 12.1, 14, 15.2 — governing posture.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §4.6, §5, §10.2 — AI-native primitives that reuse this work; useful for understanding why the choices matter beyond Phase 2.

---

## 5. Pre-completion checklist

Before declaring the PLAN complete:

- [ ] All eight open questions from §2 above have a recommended answer plus rationale.
- [ ] Block-target enumeration (§2.5) is complete and definitive — every `block_current_into` caller is listed.
- [ ] Test scenarios (§2.6) are concrete enough that the BUILD dispatch can name them by file path / scenario name.
- [ ] Verification methodology (§2.7) gives PM enough material to draft the BUILD dispatch's verification §.
- [ ] Implementation sketch (§3 of output structure) exists at least in stub form; doesn't need to be production-ready code.
- [ ] Risks and unknowns (§4 of output structure) flag anything PL should know that didn't fit elsewhere.

---

## 6. Post-completion actions

After producing your PLAN document:

1. Notify PM that your PLAN analysis is ready for review. PM will wait for **both agents'** analyses before producing the synthesis.
2. Do not read the other agent's output file even after submitting yours. Independence is preserved through ratification.
3. PM produces `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` comparing the two analyses question-by-question.
4. PM brings synthesis to project lead for ratification.
5. Project lead ratifies (or modifies / rejects).
6. PM drafts the SC-3B BUILD dispatch using ratified answers.

No commits, no backup needed for this dispatch.

---

## 7. Cross-references

- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md` — the THINK note this PLAN flows from.
- `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` — prior PM analysis on Q6 composition; primary input.
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — substrate.
- `docs/bible.md` Article 14 — the four-stage cadence this dispatch operates in.
- Bible Article 8.5.4 — the discipline Q6 structurally enforces for `block_current_into`.

---

**End of dispatch.**

*Two working-tree artifacts (one per agent): `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md` and `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md`. Eight open questions, eight proposed answers each, with rationale. Independent parallel analysis. PM synthesizes after both return. Synthesis is input to project-lead ratification. PM is on call for clarification during either agent's analysis but will not relay between agents.*
