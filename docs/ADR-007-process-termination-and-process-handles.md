# ADR-007: Process termination via ProcessHandle capability

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-03 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 4.1 (monolithic IPC ethos) · Article 5.5 (spawn-only, no fork) · Article 12.1 (capability discipline, Phase 2) · Article 12.2 (no ambient authority) · Article 13 (Phase 1.5 scope) · ADR-004 (IPC channels — handle pattern this composes with) · ADR-005 (ring-transition prologue) · ADR-006 (interior mutability + Pmap::Drop chain this reuses) · ANSPEC-001 §3.7 (capability-handle pattern) |

---

## Context

Phase 1.5b is missing one primitive: **process termination from outside the process**. Today, `sys_exit` is suicide-only; nothing lets `neo-init` (or any future supervisor) say "stop neo-worker." That is the BUILD-5 deliverable and the gating piece before Phase 2's capability hardening can layer on top.

Three viable models were on the table:

1. **POSIX-style signals** — `sys_signal(signo, handler)`, `sys_kill(pid, signo)`, kernel-managed signal queue, return-to-userland handler dispatch.
2. **Capability-mediated kill** — `sys_spawn` returns a `ProcessHandle` capability alongside the pid; `sys_kill(handle)` works only if you hold it.
3. **Kill-as-IPC** — every spawned child gets a "control channel" issued to its parent; SIGTERM-equivalent is a message on it.

POSIX signals fight capability discipline (PID is an ambient name, not a capability) and drag in the entire SA_RESTART / SA_SIGINFO / signal-mask surface that Phase 2 would have to spend cycles re-locking-down. Kill-as-IPC is elegant for the cooperative case but degenerates into Model 2 once a forced-kill alternative is needed for runaway processes — at which point we have two mechanisms.

Phase 1.5b ships **Model 2: capability-mediated kill, forced-only, aggressive reclaim**. Cooperative shutdown is not lost — it can be layered on later via either Model 3 (control channel) or a SIGTERM-equivalent message on a regular handle, whenever a use case actually appears (probably Phase 4 service supervision).

## Decision

### Q1 — Mechanism: capability-mediated kill (ProcessHandle)

`sys_spawn` is widened: in addition to the new pid, it returns a **`ProcessHandle`** installed in the calling process's handle table. The handle's holder may call `sys_kill(handle)` to terminate the named process. There is no PID-keyed kill path; PIDs are not authority-bearing in NEO.

| Alternative | Rejected because |
|---|---|
| POSIX `sys_kill(pid, signo)` | PIDs are ambient names, not capabilities. Authorization becomes "kernel checks UID/owner," which we don't have and don't want. Drag-in cost (signal queues, masks, SA_RESTART, async-signal-safety) is enormous for a "stop this process" primitive. |
| Kill-as-IPC alone | Cooperative-only by construction; needs a forced-kill alternative for runaway processes. Two mechanisms is worse than one. Layer on top of Model 2 later if/when needed. |

**ABI shape.** `sys_spawn` was `i64 → pid`. It is now `i64 → packed (pid_low_32, handle_high_32)`, mirroring the `sys_chan_create` pattern. A negative return is still errno; the unpacking happens in `libneo::sys_spawn`. The kernel handle table gains a `Handle::Process { target: ProcessId }` variant alongside the existing `Handle::Channel`. `sys_kill(handle)` looks up the handle, verifies it is a `Process` variant, and terminates.

### Q2 — Cooperative vs forced: forced only

`sys_kill` reaps the target thread immediately. No graceful unwind, no userland involvement, no "killable point" concept. The kernel forcibly drops the target's `Process` Arc, which cascades through the same machinery `on_thread_reap` already uses for natural exit:

- `HandleTable.drain()` enumerates the target's handles. For each `Handle::Channel`, `close_peer` runs — exactly as it does on `sys_exit` — waking any peer blocked in `sys_recv`/`sys_send` with `-EPIPE`. `Handle::Process` variants need no peer-side cleanup (the handle is the whole capability; nothing is "linked" on the kernel side beyond the registry entry).
- `Pmap::Drop` walks the target's PML4 and frees every mapped frame.

| Alternative | Rejected because |
|---|---|
| Cooperative-only (request shutdown, child voluntarily `sys_exit`s) | An infinite-loop child is unkillable. Bad for testing; bad for any future supervisor model. |
| Both (SIGTERM cooperative + SIGKILL forced) | Doubles the surface for a marginal Phase 1.5b benefit. Layer the cooperative path on later when there's an actual use case. |

### Q3 — Cleanup discipline: aggressive reclaim through existing Drop chain

`sys_kill` reuses BUILD-3's proven cleanup path. Specifically:

1. Yank the target's thread from the scheduler ready queue and push it into the zombie queue (so the existing reaper drops the `Box<Thread>` on its next pass).
2. Remove the `Process` Arc from `PROCESSES` **first** — before any handle drops or `close_peer` runs.
3. Drain the target's handles and call `close_peer` on each `Channel` variant (same pattern as `on_thread_reap`).
4. Drop the `Process` Arc, triggering `Pmap::Drop` and the per-process frame walker.

**Phase 1.5b limitation: target must be in `Ready` state.** The Phase 1.5b minimum scenario is "init kills worker after the IPC round-trip"; worker enters a CPU-burn loop after sending pong, so it lives in the ready queue at kill time. Killing a thread parked inside an `EndpointState` (blocked in `sys_send`/`sys_recv`) requires walking every channel handle the target holds and yanking the parked `Box<Thread>` out of the matching state variant. That is a defensible restriction for now: `sys_kill` returns `-EINVAL` if the target's thread isn't in the ready queue. Phase 2 SMP work (or a smaller follow-on if needed sooner) will lift the restriction by adding a `kill_blocked_thread` walk.

## Non-decisions (deferred)

- **No SIGCHLD-equivalent.** Parent learns child exited by some other means (Phase 2 `sys_wait` over a ProcessHandle; or the handle's existence is enough for now).
- **No graceful unwind on kill.** `sys_recv`/`sys_send` from the killed process simply do not return — the thread is reaped before it could ever resume. The peer's wake-with-EPIPE path is unaffected; that path runs from `close_peer`, which operates on `Channel` state and never looks up the killed process by PID.
- **Self-kill is illegal.** A process holding a `ProcessHandle` to itself (which it should not, but capabilities can be passed in Phase 4 IPC) calling `sys_kill` on it returns `-EINVAL`. The check is explicit in `sys_kill_impl`, not "this can't happen."

## Implementation notes (load-bearing — read before changing)

1. **Self-kill guard returns `-EINVAL` explicitly.** After resolving the handle to a `target_pid`, `sys_kill` compares against `current_pid()` and returns `-EINVAL` immediately if equal. Do not rely on "no one would ever do this" — Phase 4 capability-passing makes this reachable.

2. **Killed-recv non-path.** A common misreading: "what does the killed process's `sys_recv` return?" The answer is **nothing — the thread is gone**. Only the *peer* (still-alive process) blocked on the killed process's channels wakes with `-EPIPE`, and that wake comes from the existing `close_peer` path that BUILD-3 already uses for natural exit. There is no special "killed-recv" code path.

3. **PROCESSES-removed-before-HandleTable-drop ordering.** `terminate(pid)` removes the `Process` from the `PROCESSES` BTreeMap first, then drains and closes the handle table, then drops the Arc. A `debug_assert!` immediately after the removal confirms the registry no longer has the pid before any handle starts being dropped. This is the same Arc-lifetime-ordering lesson the BUILD-3 phantom-win simplify finding surfaced (see the load-bearing comment in `on_thread_reap`): if a handle drop's `close_peer` ever grew a registry lookup, finding the killed pid mid-drop would be a use-after-deregister bug. The assert encodes the invariant statically.

4. **Reaper double-cleanup is benign.** `terminate` does the channel-close work directly, then pushes the target thread to the zombie queue. The reaper later drops the zombie's `Box<Thread>` and calls `on_thread_reap(pid)`. By then `PROCESSES.get(&pid)` returns `None`, so `on_thread_reap` returns early — no double `close_peer`, no panic. The dual-entry-point pattern is intentional: `sys_kill` cannot wait for the reaper because the reaper runs on the bootstrap thread, which may not get scheduled before `sys_kill` returns to userland.

5. **Aggressive reclaim is correct because the chain is correct.** BUILD-3 proved the Drop chain ordering for natural exit (HandleTable → Channels → Pmap → frame reclaim). `sys_kill` just enters the same chain from a different doorway. No new ordering invariants are introduced; the existing ones are reused.

6. **Stale ProcessHandle returns ESRCH, not EINVAL.** `terminate(pid)` checks `PROCESSES.get(&pid)` FIRST and returns `-ESRCH` if the target is gone (typically a double-kill on the same handle). EINVAL is reserved for "target exists but isn't in a killable state" (Phase 1.5b Blocked-in-IPC limitation). The two-errno split matters because Phase 2 callers will assume POSIX-shaped semantics — `ESRCH = -3` (mirrored in libneo at the standard POSIX number even though we are not POSIX-compatible) — and writing the libneo wrapper today against EINVAL would require a churning rewrite later. **Phase 4 PID-recycling concern:** Phase 1.5b's `NEXT_PID` is monotonic (never reused), so a stale `ProcessHandle` cannot accidentally point at a freshly-spawned process. Phase 2+ may add PID recycling, at which point a stale handle could resolve to a *different* live process and `sys_kill` would terminate the wrong target. The fix at that point is handle revocation (mark Process variants as "stale" when the target dies, return ESRCH on lookup) — Phase 2 capability-permission-bits work has the right hooks. Documented here so it doesn't get lost.

## Cross-references

- **ADR-004 §"Process-exit cleanup"** — defines the `close_peer` semantics that `sys_kill` reuses verbatim.
- **ADR-005 §"Ring-transition prologue"** — the killed thread, if it had been mid-syscall, would have already gone through this prologue; killing it from the ready queue means we never see it mid-prologue.
- **ADR-006 §"Pmap::Drop"** — the frame-reclaim walker that `sys_kill` triggers via the Process Arc drop.
- **Bible Article 12.1 (Phase 2 capability discipline)** — `Handle::Process` is a capability in the Phase 2 sense; this ADR locks in the shape so Phase 2 only needs to extend with permission bits and revocation, not redesign.

## Test scenario (Phase 1.5b BUILD-6 e2e smoke)

The single boot exercises happy-path AND both runtime-constructable unhappy paths. (The third unhappy path — self-kill via passed handle — is tagged `PHASE-4-FOLLOWUP` in `kernel/src/sched/process.rs` because cap-passing-over-IPC is required to construct it; the self-kill guard at `sys_kill_impl` is correct today, just not runtime-reachable.)

1. `neo-init` creates two channels (chan_main, chan_block), spawns `neo-worker` with both endpoints, captures the returned `ProcessHandle`.
2. **Happy IPC**: `neo-init` sends a heap-built ping on chan_main; worker recvs and sends a heap-built pong; `neo-init` recvs the pong.
3. **Synchronization**: `neo-init` sends "go-block" on chan_block. Worker consumes go-block and immediately enters `sys_recv` on chan_main; there must be no worker-side diagnostic syscall in that handoff window, or a timer tick can make init observe worker as Ready instead of RecvBlocked.
4. **Unhappy #1 — kill on Blocked → EINVAL**: `neo-init` calls `sys_kill(worker_handle)`. Expected: -EINVAL (worker not in ready queue; Phase 1.5b limitation per §Q3).
5. **Wake worker**: `neo-init` sends "unblock" on chan_main. Worker returns from sys_recv, enters spin_loop.
6. **Happy kill**: `neo-init` calls `sys_kill(worker_handle)`. Expected: 0. Kernel logs `proc: terminated 'worker' via sys_kill`; Pmap drop walker reclaims frames.
7. **Unhappy #3 — double-kill → ESRCH**: `neo-init` calls `sys_kill(worker_handle)` AGAIN on the now-stale handle. Expected: -ESRCH (target gone from PROCESSES; Implementation note 6).
8. `neo-init` calls `sys_exit(0)`. Final reap of init runs. Boot trace ends with both processes deregistered.
9. **Leak detector**: kernel asserts `frames_in_use_post_reap == frames_in_use_baseline` via `mm::frame_audit("final-post-reap")` + `debug_assert!`. Per-event `frame_audit` snapshots at spawn / kill / natural-exit make in-flight accounting auditable.

## Future Phase 2 hardening

- Convert `Handle::Process` to a true capability with permission bits (kill/wait/inspect).
- Add `sys_wait(handle)` that blocks until the target exits, returning its exit code.
- Lift the Ready-state restriction by walking blocked-thread parking sites when `sys_kill` finds the target not Ready.
- Phase 4 capability-grant IPC will let processes pass `ProcessHandle` to each other; the self-kill guard becomes load-bearing at that point.
