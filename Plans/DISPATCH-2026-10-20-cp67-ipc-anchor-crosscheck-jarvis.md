<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of CP-67 (the IPC park anchor) → Jarvis (Codex)

**Type:** adversarial review only. **Deliverable:** a pushed RETURN file.

**Why you.** CP-67 changes what a force-killed thread owns while it is parked in a channel: thread
lifecycle, reap and the kill paths, which is the SMP / force-kill core. CLAUDE.md requires a
different-family lens on it, and the author is Claude. You reviewed CP-66, whose audit found this leak.
A Codex seat other than Jarvis (Janus, Chad) is equally valid if Jarvis is busy.

## What is under review

Branch `claude/cp66-followups`, based on `origin/main` `8d3b146d`; the PM names the head SHA in the relay.
Read:

```
git fetch origin
git show origin/claude/cp66-followups:Plans/DISPATCH-2026-10-20-cp67-ipc-anchor-crosscheck-jarvis.md
git diff origin/main origin/claude/cp66-followups -- kernel/src/sched/scheduler.rs kernel/src/sched/thread.rs kernel/src/sched/mod.rs kernel/src/arch/x86_64/syscall.rs kernel/src/ipc/mod.rs
git show origin/claude/cp66-followups:docs/concurrency-pitfalls.md | grep -n "CP-67"   # the entry
```

The fix in one paragraph. Before the change, `sys_send`, `sys_recv`, `sys_grant` and `sys_recv_grant` each
held an owned `Arc<Channel>` in the syscall frame across the endpoint park. Terminate and fault-kill
extract the parked waiter and zombify it, and the reaper frees the stack raw, so that count leaked.
Now:
- `sched::with_channel_anchor(channel, op)` moves the `Arc` into `Thread::ipc_anchor` under the
  current CPU's scheduler lock, and lends `op` a `&Arc` that points into the boxed `Thread`.
- After `op` returns, the thread takes the anchor back and drops it outside the lock.
- `Thread::free` drops it for a reaped thread.
- `sys_grant` now drops its cloned source `Handle` before it can park.

## Refute it

1. **The borrowed pointer.** `op` holds `&Arc<Channel>` into the running thread's `Thread` across a park,
   possibly a migration, and a resume on another CPU. Can that storage move or be freed while the frame
   still uses it? Check the slab `THREAD_CACHE`, `mem::replace(&mut s.current, next)`, and every place a
   `Box<Thread>` is moved: does any of them copy the `Thread` value instead of the box?
2. **Take-back on the right thread.** After resume, `current_scheduler().inner.lock().current` must be this
   thread. Is there any path where `op` returns while `current` is a different thread, for example a
   deferred handoff (`continue_current_after_receive`, `defer_current_preempt_for_handoff`) or a
   same-CPU wake?
3. **Every exit path from a park.** Rendezvous wake, `EPIPE` on peer close, `EFAULT` copy failure, a
   foreign-process waiter woken on close, extract-and-zombify (terminate, fault-kill sibling drains), and
   a thread woken to Ready then killed by `kill_thread_by_pid` before it runs. Each must drop the anchor
   exactly once. Is any path missed, or does any drop it twice?
4. **The last count.** The anchor can be a channel's last `Arc`, so `Channel` can drop in two places: at
   the take-back (outside the scheduler lock), or in `Thread::free` at reap. Can either run with a lock
   held that `Channel`'s drop takes (its `IrqSafeMutex`, the heap)? Can a `Channel` drop while another
   thread is still parked in it? The design claims not, because every parked waiter holds an anchor.
5. **The cycle.** While a thread is parked, `Channel → state → Box<Thread> → Arc<Channel>` is a cycle.
   The design claims it lasts exactly as long as the old frame's count did. Is there a state where the
   old code would have freed the channel and the new code does not? Consider every handle closed while
   the thread is parked, and the peer process exiting.
6. **Nesting and reentrancy.** `debug_assert!(ipc_anchor.is_none())`. Can a thread enter a second
   anchored call while one is live: a nested syscall, an interrupt path, or a kernel thread calling
   `Channel::send` directly (test code does)? Kernel callers that bypass the helper keep their own `Arc`.
   Are any of them killable?
7. **`sys_grant`'s early drop.** Nothing after `drop(source)` may need the source handle's lifetime.
   Check `complete_grant`'s use of `source_cap_id` when the grantor's handle is revoked while it is parked
   (the grant already re-resolves by `CapId`).
8. **The tests.**
   - `ipc_park_kill`'s stand-ins are kernel-built threads bound to a process (`bind_process_for_test`). They
     enter the real `syscall_dispatch`. Is the park and kill they exercise the same as a ring-3 caller's?
   - Does the control fail for the right reason in every leg?
   - Is the rendezvous leg enough to prove the take-back path drops exactly once?
9. **Composition.** Does `ipc_anchor` interact with CP-66's `admission_section`, B1c's
   `inference_lifecycle`, or `kill_protected`? It should not: a thread is never in IPC and in those
   sections at once.
10. **The other changes on the branch.** These are smaller, but cover them if you can:
    - clippy fixes under `neo-test-inference-kill`, `neo-test-real-lm-generate` and
      `neo-test-inference-preemptible-liveness`;
    - B1c's marked-entry probe now enters through `syscall_dispatch`;
    - five new feature-gated CI arms;
    - the `guarded_clone` NOTE's reachability claim, that no production code starts the mover.
11. Anything else.

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings"
must say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/cp67-ipc-anchor-crosscheck-jarvis   (git fetch origin && git checkout -B codex/cp67-ipc-anchor-crosscheck-jarvis origin/claude/cp66-followups)
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test ipc_park_kill` should PASS. Each kill leg logs `channel_count parked=3 after=1
  model_count=1/1`.
- With `NEO_TEST_EXTRA_FEATURES=neo-test-ipc-park-kill-control` it should FAIL with
  `reason=ipc_park_kill_arc_leaked leg=recv`. Every leg logs `after=2`, and the grant and recv_grant legs
  `model_count=2/1` (the grant leg leaks it; the count carries over).

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='extract parked thread from channel endpoint on process terminate'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-20-cp67-ipc-anchor-crosscheck-jarvis.md` to
`codex/cp67-ipc-anchor-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
