<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the CP-66 fix (your J3-P) → Jarvis (Codex)

**Type:** adversarial review only. **Deliverable:** a pushed RETURN file.

**Why you.** You found J3-P in B1c recheck 2: a graph `SYS_INFER_RUN` parked in admission is force-zombified, its `Arc`s leak, and `complete` admits the dead principal, wedging the accelerator. This fix touches the SMP / force-kill core. CLAUDE.md requires a different-family lens on it, and the author is Claude.

## What is under review

Branch `claude/admission-park-kill-fix`, based on `origin/main` `c074037e`; the PM names the head SHA in the relay. Read:

```
git fetch origin
git show origin/claude/admission-park-kill-fix:Plans/DISPATCH-2026-10-20-cp66-admission-kill-recheck-jarvis.md
git show origin/claude/admission-park-kill-fix:Plans/NOTE-2026-10-20-admission-park-kill-s2.md   # the shape
git diff origin/main origin/claude/admission-park-kill-fix -- kernel/src/ai/admission.rs kernel/src/sched/scheduler.rs kernel/src/sched/thread.rs kernel/src/sched/process.rs kernel/src/arch/x86_64/syscall.rs kernel/src/ai/exec.rs kernel/src/errno.rs
git show origin/claude/admission-park-kill-fix:docs/concurrency-pitfalls.md | grep -n "CP-66" # the entry
```

The fix in one paragraph: `Thread::admission_section` is set by `admission::run_scheduled` around the whole section. While it is set, `kill_thread_by_pid` and the token table's by-pid take skip the thread. `terminate` (both cfg branches) and `fault_kill_current_user` call `admission::cancel_pid(pid)` after publishing `exiting`. Under `QUEUES`, that removes the pid's pending requests and wakes each with `ECANCELED`. The woken caller unwinds (no work, no `complete`), and `sys_infer_run_syscall` exits it at the boundary (`process::current_torn_down`). A killed holder runs to completion, calls `complete`, then exits the same way. `submit_and_park` reads `exiting` under `QUEUES` and refuses to park. `complete` re-chooses when a wake finds no thread.

## Refute it

1. **Is every force-zombify route covered?** Look for a path that removes a thread that is inside the admission section without going through `kill_thread_by_pid` or `take_token_thread_by_pid`. Check the fault-kill reaper drains (`zombify_ipc_parked_siblings`, `zombify_wait_parked_siblings`), `wait_any`, and anything else that walks threads by pid.
2. **The no-park-after-cancel argument.** `exiting` is stored before `cancel_pid` takes `QUEUES`, and `submit_and_park` reads it under `QUEUES`. Is there an interleaving where a request parks unseen? Consider:
   - `terminate`'s process-exit-state CAS failing;
   - a natural exit racing a kill;
   - `process::get` returning `None` between the read and the lock.
3. **Marker windows.** The marker is set before `submit_and_park` and cleared after `complete`. Can the thread be Ready, or token-parked, outside that window while it still owns the request's `Arc`s? Can it be killed after the clear and before the boundary? The design says the syscall runs `IF=0` there; check that it really does on `default` and on `product`. The Running-unqueued syscall-return residual (CP-65 in B1c) is known; report it only if this change makes it worse.
4. **The cancel wake.** `cancel_pid` calls `wake_thread_token(token, ECANCELED)` under `QUEUES`. Check:
   - CP-31 re-entry capacity;
   - cross-CPU wake to a CPU that is not up;
   - a token already taken by `complete` (the pid's request was admitted a moment earlier: is it then counted as the running holder, and does it run correctly?).
5. **Can a live process be cancelled?** `blocked_return == ECANCELED` is the verdict. Could any other waker use `-125` on an admission token?
6. **The holder.** A marked holder of a killed process finishes `exec::run` after the process has left `PROCESSES`. Check the accounting `InflightGuard`, arena releases, and `current_process_for_syscall` failing on the output path. Any UAF? The `Thread` carries `pmap` and the `process` anchor.
7. **Terminate's accounting.** In the non-`browser-runtime` branch, `!kill_result && in_admission == 0` skips channel extraction when the pid has a thread in admission. A product process is single-threaded, so is that sound? With `browser-runtime`, does `debug_assert!(kill_result || found_blocked || in_admission != 0)` hide a real miss?
8. **The tests.**
   - Is `admission_park_kill` vacuous anywhere? The stand-in threads are kernel-built and bound to the process with `bind_process_for_test`, and run the dispatcher's arm and boundary. Do they exercise the same park and kill code as a ring-3 caller?
   - The Ready-holder leg uses a hold hook that yields inside the work closure. Is that equivalent to a preemption inside `dispatch_cpu_op`?
   - Does the control fail for the right reason?
9. **Composition with B1c (#74).** B1c adds `Thread::inference_lifecycle`, `position_by_pid_killable` and a wrapper around all four inference arms. Does this fix conflict with that design, or double-handle a thread? Propose the merged predicate.
10. Anything else.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp66-admission-kill-recheck-jarvis   (git fetch origin && git checkout -B codex/cp66-admission-kill-recheck-jarvis origin/claude/admission-park-kill-fix)
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test admission_park_kill admission_park_kill_threaded` should PASS.
- With `NEO_TEST_EXTRA_FEATURES=neo-test-admission-park-kill-control`, both should FAIL:
  - `admission_park_kill` with `reason=admission_stack_arcs_leaked`. Its log shows `running_after_complete=Some(<dead pid>)`, `served=false` and `model_arcs=4/1`.
  - `admission_park_kill_threaded` with `reason=admission_holder_never_held`. The wedge stops the holder leg: the device is still "running" the dead waiter.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions; grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then `cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='force kill zombify parked thread by pid admission queue'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-cp66-admission-kill-recheck-jarvis.md` to `codex/cp66-admission-kill-recheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the worktree.
