# Dispatch: §Q9 redo design notes — PM revisions pass

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Parent:** `Plans/DISPATCH-2026-05-07-q9-redo-design-notes-jarvis.md` (the original dispatch). `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` is your draft. This dispatch is a revisions pass.
**Type:** Doc-only revisions to the existing draft. NOT a commit. NOT new content beyond the four edits below.

---

## 0. Framing — the draft is good

The draft is solid. The five main observations you summarized are correct. The §3.1 spawn-flow analysis is concrete and accurate. The §4 open questions are mostly well-shaped. §5's composability framing is the right cross-pollination from the research artifact.

This dispatch is a sharpening pass, not a critique. Four edits, each scoped to a paragraph or two. ~30–45 min total.

---

## 1. The four edits

### Edit 1 — Sharpen §3.2's atomic-safety reassurance

Current §3.2 contains this passage (paraphrased):

> "This is actually safe because the atomic decrement ensures exactly one CPU sees the transition to 0."

That's true *for the atomic operation in isolation*, but it's more reassuring than warranted about what the original §Q9 failure mode actually was. The leading hypothesis from the ADR-013 amendment is that the count was *already wrong* (0 or >1 instead of the expected 1) by the time the decrement ran — i.e., the bug was in initialization-order or counting-correctness *upstream of the decrement*, not in the decrement's atomicity.

**Edit:** Add a paragraph after the "actually safe because the atomic..." sentence noting that this safety only holds *given the count is correct on entry*. The leading hypothesis isn't "two CPUs both saw 0" — it's "the count was already corrupt before the decrement, so the decrement's outcome was nonsensical regardless of its atomicity." Connect this back to §3.1's "initialize before threads become runnable" recommendation: §3.1's correctness depends on §3.2's correctness, but neither is sufficient alone.

Rough length: 4-6 sentences added. Do not delete the existing passage; the local atomic-correctness statement is still true and worth keeping.

### Edit 2 — Reframe §3.4's reordering example

Current §3.4 constructs a reordering hazard between "CPU0 decrements... meanwhile CPU1's `spawn_user()` has just incremented `live_thread_count` from 0 to 1 but hasn't yet made the new thread visible in `PROCESSES`."

That construction is structurally off. `spawn_user()` runs under `without_interrupts()` and acquires the `PROCESSES` write lock for `register()`. The `PROCESSES` lock is the synchronization point for "is this process visible." So a CPU racing on `live_thread_count` after a different CPU has spawned the Process must first acquire the same lock, which serializes the access. The `spawn_user`-vs-reaper race as described can't happen.

The *correct* framing is multi-thread-per-process: a Process already exists, has thread T1 alive, T1 (or some other thread of the same process) calls a future `sys_thread_create` to spawn T2, T1 then exits. T2's increment must happen-before T1's reaper sees the count. That's the actual reordering hazard `SeqCst` defends against.

**Edit:** Replace the spawn-vs-reap example with a same-process-thread-create-vs-reap example. Note that:
- Phase 1.5b doesn't have `sys_thread_create`; this hazard becomes real only when multi-thread-per-process spawn lands (post-Phase 1.5).
- The hazard is one of the reasons §Q9 *can't* be fixed in isolation from the multi-thread-per-process roadmap — the test that would catch it requires a primitive that doesn't exist yet.
- The §Q9 redo should be co-scheduled with `sys_thread_create` design, not landed standalone before multi-thread support exists.

Conclusion of §3.4 (recommending `SeqCst`) is unchanged — the recommendation is still right. Only the worked example needs updating.

Rough length: replace ~6-8 sentences in the middle of §3.4 with the corrected example. ~6-8 sentences out, ~8-10 in.

### Edit 3 — §4 Q5: demote from open question to settled

Current §4 Q5: "Does a thread spawned on CPU0, migrated to CPU1 via rendezvous, and exited on CPU1 — get reaped by CPU1, or does the original CPU0's reaper retain ownership?"

This is settled in the current architecture. The current code routes zombies to whatever CPU called `exit()`. §Q4's wake-on-completer-CPU migrates the thread; if the migrated thread later calls `exit()`, it dies on its current CPU. The reaper follows the thread's *final location*, not its original location. This invariant is implicit in `rotate(ThreadState::Zombie, WhichQueue::Zombie)` placing the Box in `s.zombies` (the *current* CPU's scheduler's zombie queue).

**Edit:** Remove Q5 from the open-questions list. Add a one-paragraph note in §3.3 (or wherever fits best — probably near the AP-side reaper discussion) stating: "The reaper follows the thread's final location, not its origin. A thread spawned on CPU0, migrated to CPU1 via §Q4 wake-on-completer-CPU, and exited on CPU1 produces a zombie in CPU1's queue and is reaped by CPU1's reaper. This is implicit in the current `rotate(ThreadState::Zombie, WhichQueue::Zombie)` semantics; the redo preserves it."

Renumber the remaining Q4 (formerly Q4) and onward as Q4, Q5, Q6, Q7. (8 questions become 7.)

### Edit 4 — Cross-link §4 Q7 with `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`

Current §4 Q7: "What is the interaction between Q9's reaper distribution and sub-commit 3a's ManuallyDrop + asm release?"

This question is well-shaped, and PM has already drafted parallel design notes that partially answer it: see `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` §3.4 ("the cross-CPU rendezvous wake during SC 3a") and §3.5 ("BSP reaper observing zombies during CPU N's block"). The pre-thinking concludes that SC 3a's asm release composes correctly with sub-commit 2's BSP-scans-all-CPUs reaper because (a) the reaper holds no lock while waiting for CPU N's scheduler lock, (b) the asm release order Channel-first-then-scheduler ensures cross-CPU wakers proceed promptly, and (c) `sched.zombies` (what the reaper reads) is independent of `sched.current` (which is what `block_current_into` mutates).

**Edit:** Update §4 Q7 to reference the SC 3a pre-thinking artifact. Phrase it as "Partially answered in `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` §3.4–§3.5 for the *current* (BSP-only reap) architecture. Open question for the §Q9 redo: how does the analysis change when AP-side reaping re-enables (per §3.3 above)? In particular, does an AP-side reaper holding its own scheduler lock interact differently with cross-CPU wake-on-completer-CPU rendezvous than the BSP reaper does?"

This sharpens the question (it's now asking about something *not* yet analyzed) and connects the two artifacts so future ADR-016 readers find both.

Also add a corresponding cross-reference to `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` in §6 of the design notes (the cross-references section). One-liner.

---

## 2. What you do NOT do

- Do not rewrite or restructure sections beyond the four edits above.
- Do not add new analysis threads. The four edits are sharpening, not expansion.
- Do not change the file's overall shape, tone, or structure.
- Do not delete any of the four open questions other than the renumber-after-Q5-removal.
- Do not modify §1, §2, §3.1, §3.3, §3.5, or §5 except for Edit 3's added paragraph in §3.3.
- Do not commit anything (same constraint as the original dispatch).
- Do not touch any stash.
- Do not run any build/test command.

---

## 3. Time budget

- 5 min: re-read the four edit specs above and locate the affected paragraphs in your draft.
- 10 min: Edit 1 (§3.2 atomic-safety paragraph addition).
- 15 min: Edit 2 (§3.4 reordering-example replacement; this is the most substantive edit).
- 5 min: Edit 3 (§4 Q5 removal + §3.3 paragraph addition + question renumbering).
- 5 min: Edit 4 (§4 Q7 update + §6 cross-reference).
- 5 min: re-read whole file to verify no break in flow.

**Total: ~45 min.** If you hit 1h and aren't done, surface — I'll pick up the rest myself.

---

## 4. Close

When done, ping PM with a one-line completion: "Q9 redo notes revised: <line count>, <any deviations from the four edits>." PM does a final read.

---

## 5. Why this matters

The §Q9 redo design notes will, when sub-commit 5+ schedules the §Q9 work, be the starting substrate for ADR-016. Polishing it now while context is fresh saves the future ADR-016 author from re-deriving the analysis under closure pressure. The four edits push the artifact from "good draft" to "publication-grade research artifact" with bounded effort.

---

**End of dispatch.**
