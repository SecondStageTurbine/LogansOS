<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: OT-61 C1.1 — two cross-CPU concurrency bugs (one a UAF) → Cerebro

**Date:** 2026-07-26
**From:** PM (Claude) — cross-model AAG: 5 Claude lenses CLEAN, then a **Codex** cross-check (Janus) **BROKE two cross-CPU seams**
**Candidate:** `a061bd6f` (branch `codex/ot61-c1.1-aag-fix-forward-cerebro`)
**Status:** **BOUNCED — NOT merged, no 4-shard token stands.** The C1.1 code is otherwise excellent (futex compare-then-park atomicity, CP-30, kill-switch byte-identical, non-vacuous SMP-4 tests all verified clean). But the two cross-CPU seams the Claude memory-safety lens flagged "couldn't discharge from the diff" both **break under true SMP** — Janus (Codex) built the concrete interleavings. **This is why the cross-model check exists: the author and the whole PM panel are Claude, and they shared the exact blind spot.** Both are real; fix before merge.

**Key fact Janus established:** C1.1 admits new threads to the *creator's* CPU but does **not pin them** — **IPC rendezvous deliberately migrates a blocked thread to the completing CPU** (`ipc/mod.rs:558-562`, `scheduler.rs:1723-1726`). So same-process threads DO split across CPUs, and both seams are reachable.

## Finding 1 — per-CPU `live_count` accounting leak (BREAKS)
The last-process teardown is safe (it uses `ThreadRegistry.live` under the per-process lock, not the per-CPU `live_count` — so no early `Pmap` drop *through this counter*; the Claude lens was right about that). **But the migration leak is real:**
1. CPU A admits T, increments A's `live_count` (`scheduler.rs:508-520,546-557`).
2. T blocks in IPC; its reservation stays on A.
3. CPU B completes the rendezvous → IPC wakes T **onto B** (`ipc/mod.rs:558-562`).
4. The enqueue stamps T as CPU B but does **no A→B `live_count` transfer** (`scheduler.rs:616-630,1709-1720`).
5. T exits, reaped from B → decrements **B's** count (`scheduler.rs:2117-2130`).
6. **A keeps a phantom reservation forever; B saturates at zero or consumes another thread's reservation.**

Result: `ready.len() <= live_count` no longer holds on B, and A's count never returns to zero → future admission/wake pressure can **reject work early OR panic on the "supposedly impossible" full-ring wake** — i.e. blocker-A's panic is **still reachable** via this drift, not actually fixed under cross-CPU migration.
- **Fix:** make migration a **reservation transfer under ordered source/dest scheduler locks + a destination-capacity check** (if B is full, wake on A). Forced zombification must likewise preserve/transfer the reservation owner.

## Finding 2 — force-kill vs. join wake: concrete UAF (BREAKS)
The token generation check serializes ownership of the parked `Box<Thread>`, but does **not** protect the raw `Process` registry / done-word **after a valid wake**. My Claude lens 2 claimed "force-kill zombifies the parked joiner, never resumes it, so the raw pointers are never dereferenced post-free" — **that is wrong**: a concurrent *valid* wake resumes the joiner into a Ready→Running window the kill scan can't see. Interleaving (Janus):
1. Joiner J drops its `Arc<Process>`, retains **raw** pointers to `Process::threads` + the target's boxed done-word (`process.rs:2497-2513,2519-2544`).
2. J parks (generation-tagged scheduler slot + futex entry).
3. Target on CPU B exits, stores `done_word=1`, starts the join wake (`process.rs:2458-2472`).
4. `futex::wake` removes J's token under the futex lock, **then drops that lock before invoking the scheduler** (`futex.rs:221-254`).
5. CPU A concurrently runs `terminate(P)`; `purge_pid(P)` **no longer sees J's removed futex entry** (`process.rs:2893-2905`).
6. The valid wake makes J **Ready** (`scheduler.rs:1729-1746`).
7. J becomes **current before `terminate` scans**. `kill_thread_by_pid` sees only Ready threads; the token scan sees only parked threads — **current J is invisible to both** (`scheduler.rs:2064-2089`).
8. `terminate` removes P from `PROCESSES` and drops the last `Arc<Process>` (`process.rs:2916-2919,2981-2986`).
9. **J resumes and dereferences the freed done-word (`process.rs:2554`) or the freed registry lock (`process.rs:2569`) → UAF.**

`Stale`/`Gone` cannot help — J's token is current-generation and legitimately returns `Woken`; the failure is *after* the token check, in Ready→Running.
**Same window, containment hole:** the fault-kill path avoids *this* early drop (last-reap retains the process) — but the identical Ready→Running window lets a **sibling escape the claimed whole-process fault-kill** (I-14 containment): there is **no running-sibling kill check**.
- **Fix:** (a) do NOT deregister/drop `Process` during forced termination until **every** thread reaches reap; (b) add a **process-kill flag checked at wake AND at syscall/interrupt return, plus IPIs to retire currently-running siblings**; (c) give each `Thread` a **scheduler-owned `Arc<Process>`** (not an Arc on the suspendable kernel stack) so join's raw registry/done-word pointers stay valid until that thread is reaped; (d) generation is for token ownership, not lifetime — also close the theoretical `u32`-wrap ABA at `scheduler.rs:1086`.

## Re-submit
Fix both (or, if you believe Janus's interleaving cannot form, refute it with the concrete code reason the window closes — cross-model divergence is resolved by evidence, not seniority). Update the pitfall bank (these are new cross-CPU classes — CP-3x). Rebase-fresh, re-run the **4-shard `workflow_dispatch` gate on the exact fixed SHA**, and the PM re-runs the memory-safety + the Janus cross-CPU lens on the fix. The scenarios should gain a **cross-CPU migration + concurrent-terminate** battery (the current SMP-4 batteries did not deterministically hit either seam — that's why the gate was green-clean while the bug is real).

## Relay — what to tell Cerebro
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-07-26-c1.1-crosscpu-uaf-cerebro.md` + Janus's cross-check (`Plans/DISPATCH-2026-07-26-c1.1-crossmodel-smp-seams-cross-check-janus.md`, and his RETURN if he pushes it). Same branch; rebase-fresh (main moved).
- **One-line:** **C1.1 BOUNCED — a Codex cross-check (Janus) broke two cross-CPU seams the all-Claude AAG missed; the code is otherwise excellent.** IPC rendezvous migrates a blocked thread to the completing CPU (threads aren't pinned), so both are reachable. **(1)** migration wakes a thread onto CPU B but does NOT transfer its `live_count` reservation from A → A keeps a phantom reservation, B saturates/steals → `ready.len()<=live_count` breaks and blocker-A's full-ring panic is still reachable; fix = reservation transfer under ordered src/dst scheduler locks + dest-capacity check. **(2)** concrete UAF: a valid join-wake makes the parked joiner J Ready→Running *before* a concurrent `terminate(P)` scans; J is invisible to both the Ready-only kill scan and the parked-only token scan, `terminate` drops the last `Arc<Process>`, J resumes and derefs the freed done-word/registry (`process.rs:2554/2569`); generation checks ownership not lifetime; fix = don't drop Process until all threads reap + a kill-flag checked at wake & syscall/interrupt-return + retire-running-siblings IPI + give each Thread a scheduler-owned `Arc<Process>` (off the suspendable stack). Same window also lets a sibling escape the whole-process fault-kill (no running-sibling check). Add a cross-CPU-migration + concurrent-terminate battery; rebase, re-gate the exact SHA, PM re-audits (incl. the Codex cross-CPU lens).
- **RAG** (PowerShell): health-check first; e.g. `scheduler live_count admit reap migrate cpu_id reservation`, `ipc rendezvous wake completing cpu`, `terminate purge_pid kill_thread_by_pid PROCESSES drop`, `thread_join done_word raw pointer registry`. If down, note it + grep/Read.
