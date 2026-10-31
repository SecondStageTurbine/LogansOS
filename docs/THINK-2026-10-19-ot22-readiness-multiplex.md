<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-22: a bounded readiness multiplex to end the desktop's idle spin

**Status:** design, no kernel code. **Author:** S2 session (Claude). **Date:** 2026-10-19.
**Gate before code:** cross-family lens (Codex), `Plans/DISPATCH-2026-10-19-ot22-multiplex-codex-crosscheck.md`.
**Concurrency-core:** yes (parks, wakes, timer expiry), so the pitfall bank applies and the AAG needs a cross-family lens.

## For the PL (six lines)

1. Today the desktop never sleeps: the window manager, Home and Arthur all ask "anything for me?" in a tight loop, so an idle screen keeps a core busy.
2. The fix is one new kernel call: "wake me when any of these few things has something for me, or at this time, whichever comes first."
3. Messages between programs can wake a sleeper directly. Keyboard and mouse can't yet, because the kernel only reads them when someone asks; so a sleeper on input is woken once per timer tick (100 times a second) to look.
4. That turns 100% of a core into a few short checks per tick, with key echo delayed by at most one tick (10 ms).
5. Built in one small step behind a switch; with the switch off, the desktop polls exactly as today.
6. A QEMU test proves it: an idle desktop must spend most of its ticks in the idle thread, and the old build must not.

## 1. What spins today (measured against the code)

Every desktop loop is ring-3 code at IF=1, preempted only by the BSP's PIC tick (APs have no preemption point, timer.rs:335-341):

| Loop | Polls | Where |
|---|---|---|
| WM `arthur_session` (with `desktop-pointer`) | `sys_try_recv(surface)`, then `pointer_tick`: the pointer route (`sys_input_focus_recv`), the SAK latch (`sys_input_sak_take`), the WM's own focus while escaped; `spin_loop` | `desktop_session.rs` (loop around `try_recv_request`) |
| WM `home::read_intent` / `lens::view` | the WM's focus events; `pointer_idle` (route + SAK); `spin_loop` | `desktop_session.rs` |
| WM `launch::recv_proof` | `sys_try_recv(broker_end)` against a `sys_clock_gettime` deadline; `pointer_tick`; `sys_sched_yield` | `desktop_session.rs` |
| Arthur `run_desktop_session` | `poll_ascii` (`sys_input_focus_recv`), then the configure mailbox (a memory read); `spin_loop` | `neo-arthur/src/main.rs`, `showcase_surface.rs` |
| Lens worker | `sys_recv(MAIN)`: already blocks properly | `neo-worker/src/lens.rs` |

What each source is in the kernel:

| Source | Readiness today | Who produces it | Can it wake a waiter now? |
|---|---|---|---|
| IPC channel (rendezvous) | a peer parked `SendBlocked` on my side | the sender's `send` syscall, under `Channel.state` (`IrqSafeMutex`) | yes in principle: the send path already runs `wake_thread_on_current_cpu` for a parked receiver |
| InputFocus queue | `DELIVERY` queue non-empty for an active focus | **only a poller**: `sys_input_focus_recv` runs `hid_*::pump_routed_events()` first, which drains xHCI, decodes, stages and flushes. The xHCI MSI handler never decodes HID, and QEMU's qemu-xhci raises no MSI | no: there is no asynchronous producer |
| Pointer route (record ring) | same queue, same pump | same | no |
| SAK latch (`SAK_PENDING`, atomic OR) | non-zero | `hid_keyboard::drain()` inside the same pull pump | no |
| Configure mailbox (DI-1b) | seqlock serial advanced | WM memory store | no (a memory word, by design: CP-57) |
| Time | `TICKS` | BSP PIC IRQ0, 100 Hz | the futex already expires deadlines on the tick (`futex::on_tick_expire`, `browser-runtime` only) |

**The load-bearing fact:** input readiness has no producer except a poller. Any design that parks a thread waiting on input must either give input an asynchronous producer, or keep pulling it at a bounded rate. Pretending input is event-driven would park the desktop forever on the first idle key.

## 2. Options

### A. A bounded wait-on-N syscall with a deadline (poll / WaitForMultipleObjects shape)

`sys_wait_any(set: &[WaitItem; n ≤ 8], deadline_ticks) -> ready_mask | ETIMEDOUT`. Each item names a handle and an interest (channel: a sender is waiting; focus/route: an event is queued; SAK: pending; nothing else at rung 0). Level-triggered: the call returns which items are ready *now*; the caller then does the existing non-blocking reads.

- **IPC:** a real event. `send` finding no receiver parks as today, and fires any wait cell registered on the receiving side (§3).
- **Input:** the call runs the same pull pump the recv syscall runs (syscall context, IF=0, unchanged), checks readiness, and if nothing is ready parks with an effective deadline of `min(deadline, next tick)` while any input item is in the set. A tick wakes it to pump again. Input stays pulled, but at 100 Hz from a parked thread instead of millions of times a second from a spinning one.
- **Fit:** every desktop loop is "a few sources plus maybe a deadline": the WM's session loop is {surface channel, route, SAK, WM focus}; `recv_proof` is {broker_end, route, SAK, deadline}; Arthur is {its focus} plus the mailbox (tick-quantized).
- **Cost:** one syscall, a per-waiter wait cell, a registration on each named channel, and a timed-waiter list on the tick. Bounded: n ≤ 8, registrations removed on return.

### B. A per-process readiness ring (kqueue / io_uring shape)

Objects post readiness records into a per-process ring; the process parks on the ring.

- **Pros:** scales to many objects; edge events carry data; no per-call registration.
- **Cons for us:** persistent registrations whose lifetimes must follow every handle (close, grant, revoke, process death: the CP-30/32/33 teardown classes), ring overflow and edge/level semantics, and a second delivery path beside `DELIVERY` (the CP-55 epoch discipline would need to hold on both). It also still needs an input producer. Overkill for desktop loops of 2-4 sources.

### C. Per-object deadline receives (recv_timeout, focus_recv_wait)

Give each object its own blocking receive with a deadline.

- **Pros:** smallest per object; reuses each object's lock for CP-14.
- **Cons:** the WM waits on four sources at once. With per-object waits it must rotate with short deadlines, which is quantized polling with extra latency on every other source, or a thread per source (the WM is single-threaded on purpose: threads need `browser-runtime`). It doesn't end the spin for the cases that matter.

**Recommendation: A.** It is the smallest design that parks *every* desktop loop, keeps the HID path exactly as it is, and leaves B open for later (A's wait cell is the primitive a ring would post into).

## 3. Wake and park design (option A)

### The wait cell (CP-14)

The waiter cannot hold N object locks across its context switch. It parks on **one** lock it owns:

```
WaitCell { lock: IrqSafeMutex<CellState> }       // per waiting thread, Arc'd
CellState { fired: bool, token: Option<ThreadWakeToken>, why: u8 /* IPC | TICK | DEADLINE | DEATH */ }
```

1. Under each named channel's lock, register `Arc<WaitCell>` for my side (a fixed slot per endpoint, not a list: at most one waiter per side, as today's rendezvous). Release that lock.
2. Run the input pump (syscall context, IF=0, unchanged). Then check every item's readiness. If anything is ready, deregister and return the mask: no park.
3. Take the cell lock. If `fired`, release, deregister, re-check, return. Otherwise park with `block_current_with_token_and_guard(cell_guard, |s, token| s.token = Some(token))`: the token is installed while holding the cell lock, and the lock is released on the new stack (`switch_context_with_release2`). **There is no instant where the thread is committed-to-sleep, lock-free and still on-CPU.**
4. A waker (send path, tick expiry, peer death) takes the cell lock, sets `fired`, takes the token if present, **drops the cell lock**, then calls `wake_thread_token` (CP-16: never wake under a foreign lock). A wake that lands between steps 1 and 3 just sets `fired`; step 3 sees it. `wake_thread_token` is take-once: a second waker gets `Stale`, which is harmless.
5. On return (any reason), deregister from every channel under its lock. A registration outlives nothing: the `Arc` keeps the cell valid for a waker that read it before deregistration.

**Lock order:** `Channel.state` → (drop) → `WaitCell` → (drop) → scheduler. The send path collects the registered cell under `Channel.state`, drops it, then fires the cell. No lock is held across another except the cell lock across the park, which only the scheduler's release-on-new-stack ends. This order must be added to the lockdep levels (CP-1, CP-16's discriminator).

### The tick (CP-15/21/22)

- Timed waiters live in a bounded list (as the futex's `FUTEX_QUEUE`, 64 slots), keyed by `deadline_ticks`, with an `EARLIEST` atomic so an idle tick costs one load.
- `timer_rust_handler` (BSP, IF=0) fires expired cells: set `fired`, take the token, wake. **No allocation, no HID work, no `DELIVERY` lock in IRQ context.** The tick only wakes; the woken thread pumps input in its own syscall.
- User threads run on the BSP today (`place_thread_on_cpu` is test-only), so a tick wake lands on the CPU that takes the tick. If user threads ever run on APs, the wake crosses CPUs via `wake_thread_token`'s reschedule IPI. The target's idle loop re-runs `rotate` after `hlt` (CP-22/CP-28 guarded), but a *busy* AP is CP-15/21's latency case. That is not a deadlock, but this design's latency bound assumes BSP-resident waiters; the rung asserts it.

### Wake-before-park, lost wakeup, cross-CPU

| Pitfall | This design | Guard / discriminator |
|---|---|---|
| CP-14 lost wakeup | fire sets `fired` under the cell lock; park checks `fired` under the same lock and releases it on the new stack | park/fire balance counters; the `wait_any_wake_before_park` host model and the QEMU race leg (§5) |
| CP-15/21 stranded wake (EOI-only IPI) | waiters are BSP-resident; tick and send wakes are same-CPU | the rung asserts waiter CPU == BSP; an AP placement is out of scope until preemptible APs land |
| CP-16 wake under a foreign lock | collect under `Channel.state`, drop, then fire; the tick fires with no other lock held | ordered lockdep levels for `Channel.state`, `WaitCell`, scheduler |
| CP-20 publish-then-wake ordering | readiness is published under the object lock before the cell is fired; the cell lock is the release/acquire edge | review invariant + the race leg |
| CP-22/28 idle `sti; hlt` | unchanged: the idle thread is untouched | existing guards |
| CP-30/33/36 kill a parked waiter | process death purges its cells from channels and the timed list (as `futex::purge_pid`); wakers tolerate `Gone`/`Stale` | a kill-while-parked leg |
| CP-31 token table capacity | `TOKEN_PARKED_SLOTS = 16` per CPU; three desktop processes each park at most one token | a full table returns `EAGAIN` without parking: the caller spins this iteration, degraded but never wrong |
| CP-37 IF discipline | every new lock (`WaitCell`, the timed list) is taken only with IF=0: syscall entry is IF=0, and the tick handler is IF=0 | a test-only IF-on acquisition witness, as CP-56's `mixed_if` leg |
| CP-55 deferred input bound at capture | input is still staged and flushed by the existing pump under the `ACTIVATION_EPOCH` discipline; the multiplex only decides *when* the pump runs | `hid_contention` handoff leg unchanged and re-run |
| CP-56 IF at every lock on the path | the wait syscall reuses `pump_routed_events` exactly as `sys_input_focus_recv` does | `hid_contention` mixed_if leg re-run |
| CP-57 server-initiated send | unchanged: the WM still never sends unsolicited; the mailbox stays a memory word, checked at the waiter's tick wake | — |

### The HID path never blocks

Nothing on the HID path waits or allocates. The pump still runs only in syscall context at IF=0, exactly as today: in the wait syscall instead of only the recv syscall. The tick handler gains only a bounded list walk and token wakes (the futex's `on_tick_expire` shape). The MSI handler is unchanged.

## 4. Readiness per source (rung 0)

| Item | Ready when | Waker |
|---|---|---|
| Channel, recv interest | the peer is `SendBlocked`/`GrantBlocked` on my side, or the channel is `Closed` | `send` / `grant` / `close_peer` fire my side's cell |
| Focus or route | its queue is non-empty after the pump, or it is not receivable (EPERM is an answer) | the tick (input is pulled) |
| SAK | `SAK_PENDING != 0` after the pump | the tick |
| Deadline | `TICKS >= deadline` | the tick |

Arthur's configure mailbox is not a kernel object: Arthur adds a short deadline (one tick) to its wait while it has a mailbox, which is exactly the rate it needs.

**Latency:** IPC wakes immediately. Input and SAK are seen within one tick plus the pump: at most 10 ms at 100 Hz, against today's sub-millisecond spin. A one-tick key echo delay is the honest cost of rung 0. Rung 2 (§6) removes it where an asynchronous input producer exists.

## 5. The smallest rung, its discriminator, falsification and kill-switch

### Rung 0 (the only rung this THINK asks to build)

- **Kernel:** `SYS_WAIT_ANY` (feature `wait-multiplex`): the wait cell, channel registration slots, the timed list on the tick, and process-death purge. Items are limited to channel recv, focus/route, SAK and a deadline.
- **Userland:** the WM's session loop, `home::read_intent`/`lens::view`, `launch::recv_proof` and Arthur's `run_desktop_session` replace `spin_loop` with `sys_wait_any`, behind `desktop-park` in neo-init and neo-arthur, which requires the kernel feature.
- **Kill-switch:** with `wait-multiplex` off the syscall is absent, and with `desktop-park` off the loops are byte-for-byte today's spin. Host KAT: the loop bodies compiled without `desktop-park` are unchanged.

### Discriminator: an idle desktop parks

- **Witness.** Today's `SchedulerStatsV1.idle_ticks` counts a tick as idle only when `current_thread_id == u64::MAX`, but the idle thread has a real tid, so idle time lands in `run_ticks` and the counter is ~0 regardless. Rung 0 fixes the accounting (a tick is idle when the CPU's current thread is its idle thread) and adds a neo-test witness: the BSP's idle ticks over a window.
- **Scenario `desktop_idle_parks`:** boot to Home, launch Arthur (intent), let the session sit untouched for a window of W ticks (e.g. 300), then type a key and require its echo.
  - Pass: BSP idle ticks ≥ 80% of W; wake causes logged with `IPC + TICK + DEADLINE` counts; the key echoes within 2 ticks of its HMP send.
  - Then the ordinary end (Esc, Home, readback) must hold, so parking didn't break the session.

### Falsification

- **Control:** the same scenario built with `desktop-park` off must show BSP idle ticks ≤ 5% of W. The control proves the witness measures parking, not accounting noise. The scenario runs both, and the pair is the evidence.
- **The IPC wake is real:** in a wait with only a channel item (no input, no deadline), Arthur's commit must wake the WM with cause `IPC` before any tick could have (`why == IPC`, and the wait returns in the same tick it parked). A build with the send-path fire compiled out (`wait-multiplex-no-ipc-fire`, test-only) must fail that leg by timing out. Without this leg, the tick would quietly rescue a broken IPC wake, and the test would pass while the design was wrong.
- **Race leg (CP-14):** a test-only delay between registration and park while another thread sends; the wait must return `IPC` every time over K iterations, and the park/fire counters must balance.
- **Kill-while-parked (CP-30/33):** kill a process parked in `wait_any`; its cells must be purged (timed list and channel slots empty) and the waker must see `Gone`.

### Cost

Kernel: one syscall plus three small structures (cell, channel slot, timed list), the accounting fix, purge hooks. Userland: four loops. Gate: kernel-touching (4-shard), plus the new scenario. The AAG needs a cross-family lens (concurrency core).

## 6. Later rungs (not asked for now)

- **Rung 1:** more interests: send-ready, futex keys and buffer fences in the same wait, so `fence_wait`'s yield-spin can park too.
- **Rung 2: an asynchronous input producer.** On hardware with xHCI MSI, a deferred worker drains and flushes on the interrupt and fires focus cells. That removes the one-tick input latency. It touches the HID path, so it needs its own design against CP-37/51/55/56. QEMU without MSI keeps rung 0's tick pull.
- **Rung 3:** a readiness ring (option B) if a process ever waits on many objects.

## 7. Open questions for the Codex lens

1. Is a fixed one-waiter slot per channel side enough? Today's rendezvous allows one receiver per side; can two threads of one process wait on the same side?
2. Is the cell-lock-across-park plus release-on-new-stack exactly the CP-14 invariant with `block_current_with_token_and_guard`, or does the token table's capacity path (`try_block_current_with_token_and_guard`, CP-31) need its own fallback ordering?
3. Does any path take `Channel.state` while holding the scheduler lock (which would invert the proposed order)?
4. Is BSP residency of user threads guaranteed, or only observed? The latency claim depends on it.
5. Is a 100 Hz pull at idle acceptable on the bare-metal target's power budget, or does rung 2 need to come first there?

## 8. As built (rung 1, PR on `claude/ot22-multiplex`)

The PM's "rung 1" is this THINK's §5 rung 0. What the build changed from the design, and why:

- **One waiter table, not per-channel slots.** The futex's shape (`kernel/src/sched/wait_any.rs`): a
  16-slot `IrqSafeMutex` table of entries holding plain channel keys (channel address | side), the
  deadline, the park token and a `fired` flag. The waiter registers, re-checks every item under each
  object's own lock, then parks under the table guard only if not fired. A send fires the table under
  `Channel.state`, *before* the sender parks; the order is `Channel.state` → table → scheduler, and the
  table guard is never held while taking a channel lock. No `Arc` lives in the table or across the park
  (CP-30). Answers §7 Q1: there is no per-side slot to overflow; any number of waiters (≤ 16) may watch a
  side.
- **A channel is ready exactly when a recv would consume something**: a parked sender, or `Closed`. A parked
  grantor is not (no recv consumes it, so reporting it would spin the caller), and `grant` does not fire.
  Receiving grants in a wait is a later item kind.
- **A focus that is not receivable is not ready.** Every client treats EPERM as "not focused yet" and keeps
  waiting, so "EPERM is an answer" would have turned an unfocused Arthur back into a spinner. Focus moves
  have no waker; an input wait never parks past the next tick, which re-checks it.
- **The kernel's wait-for-init loop sleeps a tick per check** (CP-59). It yield-spun on the BSP, which pinned
  idle at 0 regardless of the desktop. The fix is gated on `wait-multiplex` like everything else.
- **The idle witness is the WM's**, not a kernel window: the first quiet gap of ≥ 200 ticks between two of
  Arthur's requests, with the BSP's idle ticks across it (a test-only syscall). The scripted gap is 3 s
  between Arthur's first answer and the Esc that yields to Home.
- **The race leg widens the window between the readiness check and the park** (a test-only spin at the top
  of `park`), with the sender on CPU 1 firing once the waiter is registered. Over 16 rounds every wait ends
  on the send and the fire landed inside the window in most of them.
- **No new lockdep level.** `IrqSafeMutex` has no lock-order hook (`Channel.state` itself is unordered).
  The order above is enforced by construction and stated in the module docs; giving both locks levels is a
  follow-up if the bank wants it machine-checked.
- **Not done here** (§6 rungs): send-ready, futex keys and fences as items; an asynchronous input producer;
  the product desktop turning `desktop-park` on (a PM/PL call, after the Codex lens).

## 8. Addendum (2026-10-19): the Codex design findings, mapped to the build

Jarvis's design cross-check (`Plans/RETURN-2026-10-19-ot22-multiplex-codex-crosscheck-jarvis.md`, verdict
REDESIGN) arrived after rung 1 was built. The build had already left §3's `Arc<WaitCell>` shape. This
section maps each finding to what #58 does and corrects §3's lockdep claim (F5). It also records the rung-1
review's one finding (Jarvis rung-1 F1), which is fixed on #58.

| Finding | Design (§3) | Build (#58) | Evidence | Left open |
|---|---|---|---|---|
| **F1 BLOCKER**: a stack-owned `Arc<WaitCell>` leaks on force-zombify (CP-30) | Per-waiter `Arc<WaitCell>` registered on channels | **No cell object.** A static bounded table (`WAIT_TABLE`, 16 slots) of plain entries: `ticket, pid, keys[8], deadline, token, fired`. The parked frame holds only a `Copy` `Registration { slot, ticket }`. `wait_any_scan` takes and drops every resolved `Arc` (channel, focus, process) before registration and park. | `wait_any_core` kill leg: `entries_left=0 zombies=0` after killing a parked waiter | No frame-count baseline is asserted, only table and zombie counts. By construction nothing allocated is on the parked stack. |
| **F2 HIGH**: one slot per endpoint side can be overwritten | One registration slot per channel side | **No per-channel slots.** Waiters are matched by key in the table. Any number (≤ 16, ≤ 4 per pid) can watch the same side, and a fire wakes every match. Nothing is overwritten, so there is no N-item rollback to define. | — | No leg runs two waiters on one side. |
| **F3 HIGH**: the infallible park panics on a full token table | `block_current_with_token_and_guard`, "EAGAIN" claimed | `park` uses `try_block_current_with_token_and_guard`, compiled for `wait-multiplex`. On `Err` the guard comes back untouched, the entry is removed, and the syscall returns EAGAIN. Userland treats EAGAIN as "this iteration polls" and loops back to its non-blocking reads, with deadlines unchanged. | The `try_*` contract is shared with the futex | No token-full rollback leg. |
| **F4 HIGH**: no purge key; teardown order | `CellState` had no owner identity | Every entry stores `pid`; `purge_pid` removes by it. The order is: `terminate` sets `exiting`, purges, kills and zombifies, purges again, then closes handles. Fault-kill purges before zombify and the reaper drains again. A thread of an exiting process never parks (EPIPE). Wakers hold only generation-checked tokens (`Stale` after reuse). | Kill leg | — |
| **F5 MEDIUM**: the lockdep discriminator does not exist | "ordered lockdep levels for `Channel.state`, `WaitCell`, scheduler" | **Claim corrected.** `IrqSafeMutex::new` is unordered and the scheduler lock is outside the ordered band, so no lockdep witness checks this chain. The order holds by construction, and these are its only nesting edges:<br>• `Channel.state` → `WAIT_TABLE` (`fire_channel` under the channel lock);<br>• handle table → `WAIT_TABLE` (`fire_handle_gone` from the removal chokepoints);<br>• `WAIT_TABLE` → scheduler (the park installs its token under the table guard, released on the new stack).<br>Every fire and the tick drop `WAIT_TABLE` before `wake_thread_token`. No path takes a channel or handle-table lock under `WAIT_TABLE` or the scheduler lock (verified in Jarvis's rung-1 review, item 2, and for the new edge here). | Review only | A mechanical witness needs ordered levels on `Channel.state`, `WAIT_TABLE` and the scheduler lock. |

**Rung-1 F1 HIGH (a handle changes what it names), fixed on #58.**

- **The bug.** An item is a numeric handle, and the registration watched the channel the handle named at the
  first scan. A close-and-reuse (`install` takes the lowest free slot), or a revoke (`remove_cap_ids` clears
  slots and closes nothing), changed what the handle names without firing that key. A channel-only wait with
  no deadline then slept forever.
- **The fix, part 1.** The scan records each channel item's key and handle `CapId` (kernel-unique, never
  reused). The second scan, which runs after registration, must see the same identity, or the wait starts
  over: it re-watches, or returns EBADF.
- **The fix, part 2.** The `HandleTable` removal chokepoints fire the removed channel handle's key
  (`WHY_HANDLE`), so a change after the second scan wakes the waiter.
- **Legs and controls.** `wait_any_core` gains a rebind leg and a revoke leg, each with its own compiled-out
  control. The pitfall is banked as **CP-61**.
