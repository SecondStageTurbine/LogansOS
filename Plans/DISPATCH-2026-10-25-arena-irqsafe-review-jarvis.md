<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #62): post-merge review of the IRQ-safe ARENA lock (#125, lane C) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This changes the AI-governance `ARENA` table lock from a plain spin lock to an
`IrqSafeMutex`. That's SMP/interrupt core (CP-37/CP-56), and the code is Claude-authored, so it gets a Codex-family
review. It follows on from your relay #58 (VFS IF-on), whose items 4–6 named this class. **Do this after relay #60.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `723a9e1f` (main; the squash of PR #125, gated as `989ebebf`, run `40213725380`,
22/22).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/arena-irqsafe-review-jarvis
          git fetch origin && git checkout -B codex/arena-irqsafe-review-jarvis 723a9e1f713a32121dabe5c8ff00f4dfad5745ed

git show origin/main:Plans/DISPATCH-2026-10-25-arena-irqsafe-review-jarvis.md   # this file
git show origin/main:Plans/NOTE-2026-10-25-model-load-if-on-design.md
git diff 0ffc4e22 723a9e1f -- kernel tools docs/concurrency-pitfalls.md
```

Read `docs/concurrency-pitfalls.md` CP-37 and CP-56 first, then `kernel/src/sync/` for `IrqSafeMutex` and
`SwitchMutex`.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='ARENA arena_lock IrqSafeMutex lock_order ARENA reservation guarded_clone'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete sequence (CPU, IF state, which lock is
held).

1. **One way in.** Every acquisition of the `ARENA` in `kernel/src/ai/arena.rs` goes through `arena_lock()`. (The
   unrelated `ARENA` in `kernel/src/drivers/accel/accel_sim.rs` is out of scope; say so if you check it.)
2. **Nested locks.** List every lock taken while `ARENA` is held (the comment says a slot acquire may nest under it,
   12 < 15). For each, check that no path holds that lock with interrupts on and can be preempted on the same CPU.
   If one can, an IF=0 `ARENA` holder spinning on it deadlocks, which is CP-37 one level down.
3. **No park, no long work under the lock.** No `ARENA` section parks, sleeps, joins, or calls into the
   reservation daemon. `free` and `release` drain the occupant after dropping the lock. Note any section that
   allocates (for example `alloc`'s push or `live_keys`'s collect) and whether the allocator's own lock is IRQ-safe or
   ever held IF-on.
4. **Callers at both IF states.** The forward on the inference stack (IF=1, preemptible) and the tensor syscalls and
   handle table (IF=0) both reach `ARENA` (`mint`/`lookup`, `alloc`, `retain`, `release`, `handle_count`). Confirm each
   is now safe at either IF, and that nothing relied on the old lock leaving interrupts on.
5. **I-14(a) test.** `touchpad.rs`'s run-time check holds `ARENA` and then has the ISR body run to completion. With
   `ARENA` now IF=0, check how the ISR body still runs in that test (called directly, or a self-IPI after the
   section), and that the test still proves what its comment claims.
6. **The scenario.** `arena_if_off` takes `ARENA` IF-on from a kernel thread, from the inference stack's closure,
   and from an IF=0 section, and asserts the IF-on witness reads 0. Under the control
   `neo-test-arena-mixed-if-control`, it must FAIL `arena_held_with_interrupts_on`. Check that the witness is counted
   inside the section (after the lock is taken), so it can't pass by accident. Check that the control isn't in the
   gate list.
7. **Kill-switch.** With the control feature, the lock type is exactly the old `SwitchMutex::new_ordered(...,
   lock_order::ARENA)`, so the rest of `arena.rs` is unchanged.

## Close condition

- Commit `Plans/RETURN-2026-10-25-arena-irqsafe-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
