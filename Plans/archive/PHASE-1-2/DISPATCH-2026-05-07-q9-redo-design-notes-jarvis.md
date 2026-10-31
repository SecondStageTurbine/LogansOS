# Dispatch: §Q9 redo design notes draft

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Type:** Doc-only research / design-notes draft. NOT a commit. NOT a normative ADR. Output is a `Plans/` artifact for future ADR-016 input when sub-commit 5+ schedules the §Q9 redo.
**Substrate:** Sub-commit 2 closure is in flight (Cerebro is running KASLR-Q1B higher-N diagnostic right now; do not interfere). Sub-commit 2's working tree is in `stash@{0}: kaslr-q1-higher-n-pre-diagnostic`. Do NOT apply or pop any stash. Use `git show stash@{0}:<path>` for read-only access to the stashed files.

---

## 0. Standing instructions

1. **Do NOT commit anything.** Output is a single `Plans/` file. Plans/ is gitignored except for INDEX.md.
2. **Do NOT modify any tracked file** under `kernel/`, `boot/`, `userland/`, `docs/`, `tools/`, `tests/`, or anywhere else outside `Plans/`.
3. **Do NOT touch any stash.** Cerebro's higher-N diagnostic depends on the stash list staying exactly as it is. Use `git show stash@{0}:<path>` for read-only inspection.
4. **Do NOT run `cargo neo-test`, `cargo k`, `docker compose`, or any other build/test command.** Cerebro has the docker box and the test-harness loop. Build/test traffic from a parallel coder will interfere.
5. **Pure synthesis output.** Read existing material; produce structured design notes.

---

## 1. The task

The §Q9 redo design notes capture, in a single durable artifact, what an eventual §Q9 replacement (likely landing in sub-commit 5+ or a later Phase 2 BUILD) must address. The notes synthesize three sources:

- **The committed ADR-013 §Q9 design** (in `docs/ADR-013-phase-2-smp-retrofit.md`, around line 898).
- **The stashed ADR-013 amendments** (in `stash@{0}:docs/ADR-013-phase-2-smp-retrofit.md`, look for "BUILD-3 amendment: sub-commit 2 Q9 deferred" and the followup "Q9 hypothesis (informational, not investigated)" amendment).
- **The current reaper mechanic** (in `kernel/src/sched/scheduler.rs` `reap()` and `kernel/src/sched/process.rs` `on_thread_reap`).

The output is a research artifact — informational, not normative. It becomes input to a future ADR-016 (or whatever number) when the §Q9 redo is scheduled.

---

## 2. Sources to read

### 2.1 Committed material (working tree HEAD)

- `docs/ADR-013-phase-2-smp-retrofit.md` — the original §Q9 design at section "## Q9: Reaper distribution" (~line 898). Read the whole §Q9 section.
- `kernel/src/sched/process.rs` — the `Process` struct and `on_thread_reap` function as committed at HEAD `9505619`. Note: `live_thread_count` is NOT in HEAD; the committed `Process` is single-thread-per-process by Phase 1.5b convention.
- `kernel/src/sched/scheduler.rs` — the `reap()` function as committed at HEAD `9505619`. Note: this is the BSP-only reaper. The per-CPU reaper extension was the §Q9 work that got deferred.

### 2.2 Stashed material (in `stash@{0}`)

Use `git show stash@{0}:<path>` for read-only access. Do not apply the stash.

- `git show stash@{0}:docs/ADR-013-phase-2-smp-retrofit.md` — contains the §Q9 deferral amendments. Specifically:
  - Section "**BUILD-3 amendment: sub-commit 2 Q9 deferred with bisection evidence**" — explains why §Q9 was removed from sub-commit 2.
  - Section "**BUILD-3 amendment: sub-commit 2 Q9 hypothesis (informational, not investigated)**" — names the leading hypothesis for the failure mechanism and lists the recommendations for a future redo.
- `git show stash@{0}:kernel/src/sched/process.rs` — contains the `live_thread_count: AtomicU32` field and the modified `on_thread_reap` that did `fetch_sub(1, AcqRel)`. Note: this code is the version that *failed*. Reading it is informational — the redo will not copy this code verbatim.
- `git show stash@{0}:kernel/src/sched/scheduler.rs` — contains the BSP-scans-all-CPUs `reap()` (the post-§Q9-deferral version). This is the *currently shipped* (in working tree) version, and the redo composes with it rather than replacing it.

### 2.3 PM decision / dispatch chain (in `Plans/`)

- `Plans/PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md` — records the Q9 deferral decision.
- `Plans/PM-DECISION-2026-05-06-Q5-bet-failed.md` — earlier §Q5 deletion mistake; provides project-history context for "deletions need verification."

---

## 3. Output file

Single output: `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`

Format: structured Markdown, sections per below. Length target: 600–1200 lines. Tighter is better; depth of analysis matters more than word count.

### 3.1 Required structure

```markdown
# §Q9 redo design notes

**Date:** 2026-05-07
**Status:** DRAFT research artifact — not normative. Future input to ADR-016 or successor.
**Author:** Jarvis
**Substrate:** ADR-013 §Q9 was deferred at sub-commit 2 closure (see PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md and the stashed ADR-013 amendments). These notes capture what a future §Q9 redo must address.

---

## 1. Why §Q9 was deferred

[Summarize the bisection narrative from the stashed amendment: 100x gate failed at run 81 in `reap()` after `pong` reap, EXP-H proved sub-commit-2-minus-Q9 was clean, Q9 was deferred. Quote the key lines from the amendment but paraphrase the surrounding narrative.]

## 2. The leading hypothesis (informational)

[Explain the `live_thread_count` race the amendment named: `fetch_sub(1, AcqRel)` returning a count != 1 on a one-thread-per-process kernel means initialization ordering is wrong. Reproduce the amendment's recommendations verbatim or near-verbatim since they're load-bearing for the redo.]

## 3. The shape of the redo

[This is the core analytical section. For each of the amendment's recommendations, write 100–200 words of design analysis:

### 3.1 Initialize counter before threads become runnable
- What does "before threads become runnable" mean precisely in NEO's spawn flow?
- Where in `Process::new` / `Thread::new` should the counter increment happen?
- What's the failure mode if the counter is initialized after the thread is enqueued?

### 3.2 Verify exactly-once decrement semantics under SMP
- What does "exactly-once" decrement mean given that reapers run on multiple CPUs?
- Could the same Thread be reaped twice under a buggy zombie-routing path?
- What invariants must hold on the `zombies` queue's contents?

### 3.3 Avoid AP-side reaper participation until threads can route to APs
- Sub-commit 2 ships with BSP-scans-all-CPUs reap. Threads can be on APs (via Q4 wake-on-completer-CPU); APs don't reap. This is fine for now because the BSP's scan covers all queues.
- When does AP-side reaping become necessary? (Hypothesis: when reap latency from BSP-only scans becomes a measurable bottleneck, or when a workload pattern emerges where AP-side reaping is structurally necessary.)
- What's the simplest mechanism that re-enables AP-side reaping safely?

### 3.4 Use SeqCst until weaker ordering is proven
- Why SeqCst over AcqRel for this case?
- What memory-ordering analysis does the redo need to do before downgrading?

### 3.5 Add regression test for multi-thread-per-process load
- What's the test shape? (Hypothesis: spawn process with 4 threads, exit them concurrently on `-smp 4`, verify exactly one deregistration.)
- Why was the original Q9 test inadequate to catch the race?
]

## 4. Open questions for ADR-016

[List 4–8 open design questions the future ADR will need to resolve. Each question is a sentence or two, no answers — those are for the ADR conversation when scheduled.]

## 5. Composability concerns

[Apply the safety-non-compositionality lens from `docs/research/ALTIMIT-real-world-implementation.md` §G if helpful — the §Q9 failure was a compositional one (counter + reaper + registry produced a deadlock that no individual component had). One paragraph max.]

## 6. Cross-references

[List the load-bearing files and ADR sections.]

---

**End of notes.**

*Draft research artifact. Becomes ADR-016 input when sub-commit 5+ schedules the §Q9 redo.*
```

### 3.2 What the notes are NOT

- Not a fix proposal. The §Q9 redo is genuinely deferred; these notes preserve context, they don't implement.
- Not a code review. Don't propose specific Rust code in the redo — the future ADR conversation does that.
- Not a critique of the original §Q9 design. The original §Q9 was a reasonable design that ran into a real bug; the notes treat the failure as data, not as evidence the original was bad.
- Not a Bible amendment proposal. §Q9 is a kernel mechanism question, not a Bible-level question.

---

## 4. Constraints

- **Length:** 600–1200 lines. Don't pad. If 800 is enough, ship at 800.
- **Tone:** Match the existing ADR-013 / Plans/ artifact tone. Direct, named tradeoffs, named alternatives, no marketing.
- **Citations:** When referencing the stashed ADR-013 amendments, quote sparingly (1–2 short quotes max from the same source per project standard) and paraphrase the rest. The amendment text is canon and should not be reproduced wholesale.
- **No external research.** No web searches. Everything you need is in the working tree, the stashes, and the Plans/ directory.

---

## 5. Time budget

- ~30 min: read all sources in §2 (committed ADR-013 §Q9, stashed amendments, current reaper code, stashed reaper code, PM decisions).
- ~90 min: outline + first draft of the §3.1 structure analysis (the analytical core).
- ~30 min: §1, §2, §4, §5, §6 fill-in.
- ~15 min: edit pass for length/tone/citation discipline.

**Total: ~2.5–3h.** If you hit 4h and aren't done, surface — the notes don't need to be perfect, they need to capture context durably.

---

## 6. Close

When done, the file `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` exists and meets §3.1's structure. Ping PM with a one-line completion: "Q9 redo design notes drafted: <path>, <line count> lines, <main observations from the analysis pass>." PM reviews when convenient (likely after Cerebro's higher-N close report lands and sub-commit 2 closes).

**Do not** add an entry to `Plans/INDEX.md` for this artifact. INDEX.md is stale and gets a Jarvis sweep after sub-commit 2 closes; this artifact will be entered then alongside the rest of the day's work.

---

## 7. Why this matters

The deep-research §Q9 analysis from yesterday's PM-side thinking lives only in conversation history. Without a `Plans/` capture, it risks being lost across session boundaries. The redo is genuinely needed at Phase 2's full SMP scale (multi-thread-per-process workloads under `-smp 4+`), and when it's scheduled, the future PM and coder will benefit from a structured starting point rather than re-deriving the analysis from the bisection log.

This is also the kind of doc-work that's perfectly safe to do in parallel with Cerebro's loop: it touches no tracked files, doesn't compete for the docker test-runner, and produces a durable artifact. Exactly the right shape for "Jarvis idle until sub-commit 2 closes" interpreted maximally permissively (idle from *committing*, not idle from *thinking*).

---

**End of dispatch.**
