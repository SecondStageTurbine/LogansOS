<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #174) → Janus: pre-merge cross-family AAG of OT-98 (#219), a kill racing a natural exit

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. SMP-core code (the kill path and thread lifecycle); the cross-family lens is
required before the merge. It rewrites the same function as #204, which you review in relay #149, so do this one after
#149.

**What it is.** PR #219, branch `claude/ot98-kill-vs-exit`, head `82a7cffe`. It's not merged; review the branch:
`git fetch origin && git diff origin/main...origin/claude/ot98-kill-vs-exit`.
- **The defect (OT-98 / CP-98).** `terminate` (`kernel/src/sched/process.rs`) `debug_assert!`s that it found the
  target Ready or parked. The shipped product kernel keeps debug assertions on, so any process holding a
  ProcessHandle, any spawner included, could panic the kernel. Two states trip it:
  - the target has exited but isn't reaped yet;
  - the target is parked in `sys_wait`.
- **The fix.**
  - `record_exit_code` (sys_exit and the fault kill) claims EXITING_NATURAL at the commit point, so a kill that loses
    gets ESRCH and the reap delivers the code. Under browser-runtime the last `SYS_THREAD_EXIT` claims too.
  - `terminate` extracts a target parked in `sys_wait`, in every build.
  - The waiter drops its own `Arc<Process>` under the target's waiters lock before parking (CP-30).
  - The scan's `Arc`s are dropped with interrupts on (CP-91).
  - It conflicts with #204 only on the assertion line, which resolves to `found_blocked || survivors > 0`.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, and a concrete interleaving for any
fail): `Plans/RETURN-2026-10-29-ot98-aag-janus.md`.

1. **One winner.** Natural exit, the fault kill and `sys_kill`, in every interleaving across CPUs, including two
   concurrent kills and, under browser-runtime, a kill against the last `SYS_THREAD_EXIT`. Does exactly one win? Does
   each loser change nothing and return the right errno? Does a waiter get the exit code when exit won?
2. **The `sys_wait` extraction.** Can `terminate` extract a parked waiter while that waiter's target is exiting and
   waking it (a double wake, a lost wake, a wake into freed memory)? Check the lock order of the target's waiters
   lock, `PROCESSES`, the cap graph and the scheduler lock against every other path that takes them.
3. **The `Arc` dropped under the waiters lock.** Is the PR's "never the last `Arc`" argument sound when the target
   exits, is killed, or is reaped concurrently? If it were the last, what `Drop` would run under that lock?
4. **Remaining states.** Can the target be in any other kernel wait when `terminate` runs (futex, tick sleep, the
   NeoFS cache, admission, inference) where the assertion still fails? Check the default build and browser-runtime,
   with and without #204.
5. **With #204.** Trial-merge #204 onto this branch: with the assertion resolved as above, do the two changes compose?
   Does #204's spared-thread path and this PR's exit claim leave one winner?
6. **The discriminator** `kill_at_exit` and its control `neo-test-kill-at-exit-control`: deterministic? Does it reach
   both states? Does the control really restore main's behaviour (it must panic)? Are the `Weak` checks sound
   evidence that both processes and pmaps were freed?
7. **Pitfall rows** in the PR body (CP-30, -65, -66, -85, -91, -98): is each "introduced, guarded or N/A" claim right?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (SMP-core kill path: the required pre-merge cross-family lens.)
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot98-aag-janus
          git fetch origin && git checkout -B codex/ot98-aag-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot98-aag-janus.md
git diff origin/main...origin/claude/ot98-kill-vs-exit
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
