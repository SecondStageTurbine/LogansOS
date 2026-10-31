<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: force-kill of an admission-parked graph inference — the fix shape (S2)

**Lane:** J3-P (Jarvis, B1c recheck 2), pre-existing on `origin/main` `c074037e`.
**Branch:** `claude/admission-park-kill-fix`.
**Pitfall:** CP-30 class. The new entry is **CP-66**: B1c (#74) reserves CP-63–65.

## 1. The bug on main, restated

A graph `SYS_INFER_RUN` (`sys_infer_run_impl` → `run_graph_infer` → `admission::run_scheduled`) owns, on its kernel stack:
- an `Arc<Model>`;
- a `Vec<Arc<Tensor>>`;
- the input-handle `Vec`;
- the output `Vec` once it has them.

It can then be found by a force-kill in two states:

| State | How it gets there | Who takes it |
|---|---|---|
| **Parked waiter** | a contended `submit_and_park` puts it in the scheduler token table | `zombify_parked_thread_by_pid`: terminate's two loops, and `fault_kill_current_user`'s sibling loop |
| **Ready holder** | the admitted holder runs `exec::run`; on `default`/`product` (`inference-preemptible`) each op runs with interrupts on, so a tick can leave it Ready inside the admission section | `kill_thread_by_pid` |

Either way the thread is zombified raw: its frames never drop, so the Arcs leak (CP-30). The admission state then goes wrong:

- **Killed waiter.** Its `PendingRequest` keeps a dead token. The holder's `complete` admits it: `running = dead pid`. The wake returns `Gone`/`Stale` and is ignored, and every later request defers behind a principal that will never call `complete`. **The accelerator is wedged for the system's lifetime.**
- **Killed holder.** `running` is never cleared: the same wedge.

The holder case is not in J3-P's text but has the same mechanism, so the fix covers both.

## 2. The fix: cooperative cancellation for the admission section

The rule (CP-66): **a thread inside the admission section is never zombified from a queue. It is cancelled, unwinds its own frames, and exits at the syscall boundary. The admission queue never holds, or admits, a principal that is being torn down.**

1. **Marker.** `Thread::admission_deferred_kill: bool`, under the owning scheduler lock, like `preempt_defer_ticks`. `run_scheduled` sets it on the current thread before `submit_and_park` and clears it after `complete`, or after a cancelled return. It covers every state in which the thread can be found by a kill scan: parked in the token table, or Ready after a preemption inside a dispatch. Outside the section the syscall runs `IF=0` and never parks, so the thread is not findable there.
2. **Kill scans skip marked threads.** `kill_thread_by_pid` uses a marker-aware ring search, and the token table's by-pid take skips marked threads. They stay where they are and are cancelled instead.
3. **`admission::cancel_pid(pid) -> usize`**, called by terminate (both cfg branches) and by `fault_kill_current_user` *before* their kill loops, after `exiting` is published. Under `QUEUES` it:
   - removes every `PendingRequest` of `pid` on every device;
   - wakes each one's token with the cancel verdict (`ECANCELED`), in the same `QUEUES → scheduler` order as `complete`.

   It returns how many of the pid's threads are in the section: those woken, plus the running holder. Terminate counts them as handled.
4. **No park after cancellation.** `submit_and_park` reads the caller's `exiting` (from an `Arc<Process>` dropped **before** the park, per CP-30) while holding `QUEUES`. If the process is exiting, it returns `Cancelled` without parking. `cancel_pid` takes `QUEUES` after `exiting` is stored, so a request is either already pending (and `cancel_pid` finds it) or sees `exiting`. There is no window in which a request parks unseen.
5. **Waiter unwinds.** On the cancel verdict, `submit_and_park` returns `Cancelled`. `run_scheduled` skips the work and `complete` (the request was never admitted), clears the marker and returns `Err(Cancelled)`. `run_graph_infer` maps that to the new `InferenceError::Cancelled` (`ECANCELED`). Every frame's locals drop as `sys_infer_run_impl` returns.
6. **Holder runs to completion.** Run-to-completion is the admission policy. A marked holder in a killed process is skipped by the scans, finishes its dispatch, calls `complete` (releasing the device to the next live request) and unwinds normally.
7. **Boundary exit.** The `SYS_INFER_RUN` dispatcher arm checks, after the arm returns, whether the caller's process is torn down: gone from `PROCESSES`, or `exiting`. If so it calls `scheduler::exit()` instead of returning to ring 3. The reap is the ordinary one: terminate has already removed the process, and a fault-killed process's last reap tears it down.
8. **`complete` never admits a dead principal (backstop).** If the chosen request's wake returns `Gone`/`Stale`, its principal cannot run. `complete` clears `running` and chooses again.

**Kill-switch / control.** `neo-test-admission-park-kill-control` compiles items 2, 3, 4, 7 and 8 out: the marker is not consulted, nothing is cancelled, and `complete` ignores the wake result. The control must show the leak (the model `Arc` count above baseline) and the wedge (a later graph inference never admitted).

**Cost.** Terminate returns while a cancelled waiter is still unwinding, or a marked holder is still finishing one dispatch. This is bounded by one inference and is the same ordering the zombified threads already had: reaped later.

## 3. The other marker-blind endpoint: `zombify_thread` (channel and process-wait)

PM asked whether these are reachable with stack-owned Arcs.

- **Channel park (`sys_send` / `sys_recv` / grant): yes, reachable, but a small leak.** The syscall frame keeps `channel: Arc<Channel>` (from `ipc_lookup`) across the park in `Channel::send`/`recv` (`syscall.rs:4611-4639`). Terminate extracts the waiter (`extract_parked_if_owned` → `zombify_thread`), and the frame's `Arc<Channel>` is never dropped: **one `Channel` allocation leaks per kill of a thread blocked in IPC.**
  - The caller's `Arc<Pmap>` is not stranded: it is moved into the endpoint state (`sender_pmap` / `receiver_pmap`) and dropped on extract.
  - There is no wedge: the peers are closed with `EPIPE`.
  - It is product-reachable (any terminate of a process blocked in IPC).
  - Fixing it needs a scheduler-owned anchor (the CP-36 pattern). The `Arc` cannot simply be dropped before the park, because the channel lock guard borrows it until the switch shim releases it. **Recommended as its own follow-up lane; not changed here.**
- **Process-wait park: not leaking.** Under `browser-runtime`, `wait_for_exit_abi` already rebinds the target to a raw pointer before parking (CP-30). In the default/product build the owned `Arc` stays, but nothing there force-zombifies a wait-parked thread.
  - **Pre-existing CP-36-class gap noted:** in the product build, terminate does not extract a wait-parked thread at all. It stays in the target's waiter queue and is later woken back into ring 3 of a terminated process. That is a containment gap, not a leak, and it is out of scope.
- **Found during the audit, same class:** `ai::reservation::guarded_clone` token-parks a user thread (through tensor lookup) while a mover holds the tensor's reservation, and its caller may own `Arc`s (for example `sys_infer_run_impl`'s model and inputs). `zombify_parked_thread_by_pid` takes it raw: a leak, but **no wedge** (the stale token's wake is ignored by design). Reachable only during a residency migration. Reported for a follow-up, not changed here.

## 4. Discriminators

Required by the PM.

**`admission_park_kill`** (the product configuration, no `browser-runtime`):
1. A harness principal holds the accelerator (`run_scheduled`).
2. Two stand-in processes P1 and P2 each run one real graph `SYS_INFER_RUN` (the dispatcher's own arm and boundary, on `VALID_TRANSFORMER_NEOM`). Both park in `submit_and_park`: `pending_len == 2`, and both are in the token table.
3. P1 is killed by `process::terminate`.
4. The holder completes.
5. A third process P3's graph inference is then **served**.
6. Asserted: P1 cancelled and exited at the boundary; its pending request was removed and not admitted; the model `Arc` count, the frames and the heap are back at baseline after the reap; and `pending_len == 0`.

**`admission_park_kill_threaded`** (`browser-runtime`):
- the same flow, plus the **fault/sibling** path: P2 has a sibling thread that runs `fault_kill_current_user`'s path while P2's request is parked;
- plus a **Ready-holder** leg: the holder is a real `SYS_INFER_RUN` thread of P4, killed by terminate while it is preempted inside a dispatch. It finishes, `complete`s and exits, and the next request is served.

**`admission_park_kill_control`** (the old behaviour): the same session as the first scenario. It must show the model `Arc` above baseline and P3 never served within its bound. It reports both and passes as "leak and wedge shown".

## 5. Pitfall bank

- **CP-66 (new):** an admission-parked or Ready-holding inference caller, force-zombified, leaks its frames and wedges the device queue. Guard: cooperative cancellation (§2).
- **CP-30:** gains a cross-reference.
- **CP-36:** gains the product-build wait-parked containment note.

The other concurrency classes this touches:

| Class | Effect | Guard |
|---|---|---|
| CP-14 (lock state at park) | unchanged | the park still installs under `QUEUES` |
| CP-31 (token re-entry capacity) | the cancel wake re-enters a thread that already holds a reservation | none needed: same as any token wake |
| CP-36 (lifetime) | the woken thread's `Process` lifetime | held by the `Thread` anchor and its `pmap` |
| Lock order | unchanged | `QUEUES → scheduler` in `cancel_pid`, the same as `complete` |

## 6. Coordination

- S1 is narrowing B1c so the graph route is unmarked; it touches neither `admission.rs` nor the `zombify_*` functions.
- If B1c lands first, its `Thread::inference_lifecycle` and this marker both mean "do not zombify". Merge them into one predicate in the scan, kept as `||`. The test helpers here use different names from B1c's (`abi_probe_process_for_test` / `attach_process_for_test`), so they do not collide.
