<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family check of the OT-22 rung-1 build (SYS_WAIT_ANY) → Jarvis (Codex)

**Type:** review only. **Deliverable:** a pushed RETURN file. **Why you:** this is scheduler/wake core (parks,
wakes, timer expiry, process teardown), so the AAG needs a different-family lens before merge. The design lens
is a separate dispatch (`Plans/DISPATCH-2026-10-19-ot22-multiplex-codex-crosscheck.md`); this one reviews the
code that implements it. The PR does not merge until the design RETURN has landed.

## What is under review

Branch `claude/ot22-multiplex` (from `origin/main` 6c2809bf); the PM names the head SHA in the relay. Diff:
`git diff origin/main...origin/claude/ot22-multiplex`. Design and as-built notes:
`docs/THINK-2026-10-19-ot22-readiness-multiplex.md` §3-§5 and §8.

1. **Kernel `wait-multiplex`.**
   - `kernel/src/sched/wait_any.rs`: the waiter table, register → re-check → park with a `fired` flag,
     `fire_channel`, `on_tick_expire`, `purge_pid`, `sleep_ticks`.
   - `kernel/src/arch/x86_64/syscall.rs`: `sys_wait_any_impl`, `wait_any_items`, `wait_any_scan`.
   - `kernel/src/ipc/mod.rs`: the fire in `send` (under `Channel.state`, before the sender parks) and in
     `close_peer`; `recv_ready` (a parked sender or `Closed`, exactly what `recv`/`try_recv` consume).
   - `kernel/src/atrium/focus.rs`: `wait_ready` (receivable and queued), `sak_pending`.
   - `kernel/src/arch/x86_64/timer.rs`: the tick expiry call.
   - `kernel/src/sched/process.rs`: purge before the kill in `terminate` and in the fault-kill path.
   - `kernel/src/sched/scheduler.rs`: `try_block_current_with_token_and_guard` now also built with
     `wait-multiplex`; per-CPU idle tid; `current_is_idle`; the neo-test idle-tick witness.
   - `kernel/src/main.rs`: the wait-for-init loop sleeps a tick per check (CP-59).
2. **Userland `desktop-park`.**
   - `userland/neo-init/src/desktop_session.rs`: `Scene::park` and its four call sites (session loop,
     `home::read_intent`, `lens::view`, `launch::recv_proof`).
   - `userland/neo-arthur/src/showcase_surface.rs` (`park`) and its two call sites in `main.rs`.
3. **ABI.** `kernel/src/sched/wait_any_logic.rs` and `userland/libneo/src/wait_abi.rs`, checked against the
   same hand-laid bytes in `tools/atrium-logic-tests`.

## Discriminators (claimed)

- `desktop_idle_parks`: the BSP idles ≥ 80% of a ≥ 200-tick quiet gap (local: 95% of 366).
- `desktop_idle_spins`, the control: the same kernel and script with the loops spinning, ≤ 5% (local: 0% of
  362).
- `wait_any_core`, four legs:
  - IPC: an IPC-only wait ends within 5 ticks of a real `Channel::send` (the control: ~300). Control
    `NEO_TEST_EXTRA_FEATURES=wait-multiplex-no-ipc-fire` must FAIL `reason=wait_any_ipc_wake_missing`
    (local: the wait ended 298 ticks late, `ipc_wakes=0`).
  - CP-14 race: 16/16 woken, with the fire landing inside the widened check-to-park window in most rounds.
  - Deadline: the wait ends at its tick, at most one tick late.
  - Kill while parked: the entry is purged, the thread is zombified and reaped.

## Refute it

1. **CP-14.** Can any interleaving lose a wake? Consider:
   - a send before registration, between registration and the re-check, between the re-check and the park,
     or during token install;
   - the tick racing a channel fire on the same entry;
   - a waiter on the BSP with the sender on an AP.
2. **Lock order.** Is there any path taking the wait table and then `Channel.state`, the scheduler lock and
   then the table, or `Channel.state` under the scheduler lock? Firing (and so `wake_thread_token`) happens
   with `Channel.state` held. Is that consistent with every existing state → scheduler edge
   (`block_current_into_with_guard`, `close_peer`, `extract_parked_if_owned`, `terminate`)?
3. **Counters.** `CHANNEL_WAITERS`, `TIMED_WAITERS` and `EARLIEST_DEADLINE_TICKS` across register, fire,
   expire, remove and purge. Can a timed waiter be skipped by the tick fast path forever? Can a counter
   underflow?
4. **Teardown (CP-30/33/36).** Is any `Arc` alive across the park? Look at the user-item copy, the
   `ipc_lookup` temporaries, the focus clone and `caller_holds_input_system`. What happens when a
   terminate races a fire that has already readied the waiter, or when a multi-threaded process has a
   sibling parked here?
5. **Readiness = what the reader consumes.** `recv_ready` counts a parked sender and `Closed` only. Does every
   parked loop's reader (`sys_try_recv`, `sys_input_focus_recv`) consume exactly what the wait reports, or
   can it report ready forever while the reader returns EAGAIN (a spin)? Can any parked loop hang (a source
   with no waker and no tick re-check)?
6. **Kill-switch.** With `wait-multiplex` off, or `desktop-park` off, is behaviour unchanged? Check the cfg
   routing in the kernel, neo-init, neo-arthur, libneo and xtask.
7. Anything else a Claude reviewer would structurally miss (the author and the AAG panel are Claude).

A finding counts only if it survives your own re-verification; uncertain items are notes. "No findings" must
say what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot22-rung1-crosscheck-jarvis   (git fetch origin && git checkout -B codex/ot22-rung1-crosscheck-jarvis origin/claude/ot22-multiplex)
```

```
git fetch origin
git show origin/claude/ot22-multiplex:Plans/DISPATCH-2026-10-19-ot22-rung1-codex-crosscheck.md
git diff origin/main...origin/claude/ot22-multiplex
```

Optional runs, at `NEO_SMP=4`:
- `cargo xtask neo-test wait_any_core`, `desktop_idle_parks` and `desktop_idle_spins` should PASS.
- `NEO_TEST_EXTRA_FEATURES=wait-multiplex-no-ipc-fire cargo xtask neo-test wait_any_core` should FAIL.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions;
grep still wins for exact symbols. Health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`,
expecting `{"status":"healthy",...}`. If it is down or degraded, the PL manages it: note that in your RETURN
and continue with direct reads.
Optional self-serve start:
`powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='park token wake channel send fire wait table'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-ot22-rung1-codex-crosscheck-jarvis.md` to
`codex/ot22-rung1-crosscheck-jarvis` and push it. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line and a repro;
- what you checked for each numbered item.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete the
worktree.
