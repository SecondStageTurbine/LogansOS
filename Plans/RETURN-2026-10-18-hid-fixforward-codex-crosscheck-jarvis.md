<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: #48 HID fix-forward cross-check — Jarvis

**Verdict: BOUNCE.** Both findings from my earlier #44 RETURN are **CLOSED**, and Janus's #38 queue
fallback finding is also closed. The new epoch boundary prevents a staged batch from crossing a focus
handoff, and every `PRIMARY`/HID-device acquisition in the reviewed paths is forced to IF=0. However,
the broader CP-37 invariant is still false: the last `InputFocus` reference can be dropped with IF=1,
and its destructor takes the spinning `DELIVERY` lock without masking interrupts.

Reviewed `origin/claude/hid-fixforward` at `75a7eb9b`. RAG health was healthy; semantic discovery was
followed by direct reads of the focus router, both HID drivers, syscall and process-teardown paths,
queue policy, tests, and QEMU discriminator. `cargo test -p atrium-logic-tests` passed all 96 tests.

## Earlier findings

1. **#44 F1 — CLOSED.** `Stage` carries `ACTIVATION_EPOCH`; every successful activation,
   deactivation, and active-focus destruction bumps it while serialized by `DELIVERY`. `stage_event`
   discards an older batch before appending, and `flush` rechecks the epoch under `DELIVERY` before
   selecting the active route. This covers a racing epoch read, a report batch split by a handoff,
   A → B → A, and deactivate/reactivate of the same focus. Pointer-route records intentionally bypass
   the keyboard epoch; kind-2 mouse delivery without a live pointer route does not.
2. **#44 F2 — CLOSED.** Keyboard and mouse bind stores, `primary()` lookups, fd-0 reads, routed pumps,
   pointer reads, and debug injection acquire `PRIMARY` and the device lock inside
   `without_interrupts`. The mixed-IF witness is present at each of those lock sites; its control
   restores the old keyboard-`PRIMARY` acquisition shape.
3. **#38 queue fallback — CLOSED.** Key overflow finds and removes the oldest key. On a key-only
   stream that index is zero, exactly matching legacy `pop_front`; the capacity-3 release-at-head KAT
   passes.

## Finding

### 1. HIGH / blocker — `InputFocus::drop` can still acquire `DELIVERY` with IF=1

**Files:** `kernel/src/atrium/focus.rs:528-543`; `kernel/src/sched/process.rs:2356-2376,
2978-2988`.

`InputFocus::drop` calls `DELIVERY.lock()` directly. Process teardown drains the handle table inside
`without_interrupts`, but deliberately drops the returned handle vector later, after that closure has
restored the caller's interrupt state. If the vector owns the final `Arc<InputFocus>`, the destructor
therefore takes `DELIVERY` with IF=1. The same is possible for any ordinary last-handle close whose
final `Arc` destruction is not enclosed by an IF=0 guard.

Repro/interleaving:

1. On CPU N, process teardown reaches `drop(drained)` with interrupts enabled; final focus destruction
   acquires `DELIVERY`.
2. The timer preempts that thread before the destructor releases the lock.
3. Another runnable thread on CPU N enters an Atrium syscall (SYSCALL masks IF) and calls
   `activate`, `try_recv`, pointer claim, or pointer forward; it spins on the same `DELIVERY` lock.
4. With IF=0, the timer cannot resume the preempted owner, so CPU N deadlocks.

Wrap the destructor's whole `DELIVERY` critical section in `without_interrupts` (and preferably make
the same rule self-contained at every blocking `DELIVERY.lock()` entry point). Extend the mixed-IF
witness to `DELIVERY` and exercise final-focus destruction; the current witness counts only
`PRIMARY` and HID-device acquisitions, so it cannot detect this path.

## Dispatch checks

1. **Lock sites / witness:** all claimed `PRIMARY` and device-lock sites are IF=0. `deactivate` and
   `flush` are self-contained IF=0. Finding 1 is the remaining reachable IF=1 `DELIVERY` acquisition;
   the existing witness does not cover it.
2. **Epoch disclosure:** the epoch is sampled at append and checked under the handoff lock at flush.
   Old batches cannot reach E+1, including ABA and same-focus reactivation. Pointer-route records are
   deliberately session/ring-bound rather than activation-bound.
3. **Stale state:** the kernel does not synthesize releases; the ADR now explicitly makes focus loss
   release all keys client-side. A release discarded across a handoff is therefore safe only under
   that contract. I found no contradictory stateful key decoder in the shipped Home/Arthur clients.
4. **Handoff discriminator:** the other CPU holds `DELIVERY`; report-count progress and a rise in
   `FLUSH_CONTENDED` prove the key was staged. B is activated only after the holder exits and before
   this CPU pumps again. The exact `b_events=0 a_events=0 handoff_dropped=2` assertion is non-vacuous.
5. **Mixed-IF discriminator:** it drives both fd-0 entry points with IF=1 and counts the two primary
   plus two device acquisitions per iteration. The negative control faithfully restores main's
   keyboard-`PRIMARY` shape, but it is not a complete witness for `DELIVERY` (Finding 1).
6. **Queue compatibility:** on key-only queues the first non-pointer is index zero, so removal is
   bit-identical to legacy `pop_front`. Mixed queues preserve the sole unsuperseded pointer transition.
7. **Other:** no additional disclosure, duplication, unbounded stage work, or queue-policy regression
   survived re-verification.

