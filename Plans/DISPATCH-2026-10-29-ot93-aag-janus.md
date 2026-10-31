<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #149) → Janus: pre-merge cross-family AAG of OT-93 (#204), the kill-of-Ready-in-kernel fix

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. New SMP-core code (thread lifecycle, the kill path, the syscall return). The
cross-family lens is required **before** the merge (CLAUDE.md, the C1.1 precedent: you found the force-kill-vs-join UAF
that a five-lens Claude AAG had ruled safe).

**What it is.** PR #204, branch `claude/ot93-kill-ready-in-kernel`, head `b50c047f`. It's not merged; review the
branch: `git fetch origin && git diff origin/main...origin/claude/ot93-kill-ready-in-kernel`.
- **The defect (OT-93 / CP-95).** `kill_thread_by_pid` zombified a thread that was Ready inside a syscall. Its kernel
  stack was freed raw, so owned locals leaked. The concrete case: `ipc::Channel::send`'s rendezvous arm holds two
  `Arc<Pmap>` across `handoff_to_rendezvous_peer`.
- **The fix.** Zombify a Ready thread only when it resumes in ring 3.
  - `Thread::in_syscall` is set at entry and cleared at the return decision, under the scheduler lock.
  - A thread Ready in-syscall, or running, is spared. It reads `Process::force_killed` and exits at its syscall
    return, at entry, or at the timer or reschedule IPI from ring 3.
  - `terminate` IPIs the CPUs that run a survivor.
  - A spared thread is refused user-wait parks (`ESRCH`). Infallible kernel waits (device I/O, the NeoFS cache, tick
    sleep) still park.
  - The CP-65 and CP-66 markers are kept.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, and a concrete interleaving for any
fail): `Plans/RETURN-2026-10-29-ot93-aag-janus.md`.

1. **The `in_syscall` window.** Is it set before anything at syscall entry can be preempted, and cleared only after the
   return decision, with no path that leaves it set on a return to ring 3, or clear inside the kernel? Check the
   SYSCALL and IRETQ return paths and the fault/exception entries.
2. **Spared threads always die.** For each way a spared thread can be in the kernel, does it reach an exit point in
   bounded time?
   - Ready, running, or parked in an infallible kernel wait that a dying device never completes.
   - Is any kernel wait both not refused and unbounded?
3. **The `force_killed` ordering.** `terminate` publishes it before its scans. Can a thread check it, find it clear,
   then park or return to ring 3 after the scan missed it? Check across CPUs, and the IPI's ordering relative to the
   publish.
4. **The reaper.** Can the reaper free a stack, Pmap or `Process` while a spared thread on another CPU still runs on
   it? Compare the C1.1 force-kill-vs-join case.
5. **Join and wait.** Does a joiner or `sys_wait` caller ever see the process reaped before a spared thread has left the
   kernel? Or wait forever on a thread that's spared but never scheduled?
6. **The syscall return.** It now takes the scheduler lock once more. Lock order against every lock a syscall can hold
   at return: is IF off where CP-93 says (the final retire check)? `syscall_exit_irq_window` had to be changed; is that
   change test-only?
7. **The `threads_tls` failure** S4 saw once in six runs (check 8 step 6, FS.base). Could the new return path or the
   spared-thread path change when FS.base is written relative to the last switch-in (CP-83, #184)?
8. **The discriminator** `kill_ready_in_kernel`: is it deterministic, and does it fail with the old rule (the
   `-control` feature, a frame leak of +50)? Does it cover a kill landing between the handoff and the sender's resume?
9. **Pitfall rows** CP-30, -33, -65, -66, -85, -86, -93 and -95 in the PR body: is each "introduced, guarded or N/A"
   claim right?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (New SMP-core code: the required pre-merge cross-family lens.)
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot93-aag-janus
          git fetch origin && git checkout -B codex/ot93-aag-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot93-aag-janus.md
git diff origin/main...origin/claude/ot93-kill-ready-in-kernel
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
