<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-22 readiness multiplex design crosscheck — Jarvis

**Seat:** Jarvis (Codex-family)  
**Reviewed:** `docs/THINK-2026-10-19-ot22-readiness-multiplex.md` on `claude/ot22-multiplex-think`  
**Verdict:** **REDESIGN**

Keep Option A's bounded, level-triggered `sys_wait_any` ABI and tick-pulled HID strategy, but do not build the proposed `Arc<WaitCell>` lifecycle as written. The waiter storage/ownership, duplicate-registration policy, capacity fallback, and teardown indexing must be specified first. Finding 1 is a CP-30 blocker: a force-zombified blocked syscall does not unwind its kernel stack, so an owned `Arc<WaitCell>` (or any resolved object `Arc`) held across the park leaks.

The local RAG health endpoint was healthy (`ollama=true`, `qdrant=true`), but the index reported `last_indexed=2026-10-17` and returned no results for the three OT-22 conceptual queries. I therefore used direct reads of the named source files for the conclusions below.

## Findings

### F1 — BLOCKER — the Arc-backed wait cell violates CP-30 unless it is owned outside the blocked stack

The design declares a per-waiter, Arc-backed cell and registers `Arc<WaitCell>` values on channels (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:76-84`). A waiter must retain some way to identify/deregister that cell after `block_current_with_token_and_guard` returns; the natural implementation leaves an owning `Arc` in the blocked syscall frame. `sys_wait_any` will also naturally resolve channel/focus handles to owned object `Arc`s before it registers and checks them.

That is forbidden here. A force-zombified blocked thread's stack is freed as raw memory with no unwinding and no `Drop`; CP-30 explicitly requires that a user-blocking syscall hold no owned `Arc`, `Box`, or load-bearing guard across `block_current_*` (`docs/concurrency-pitfalls.md:1096-1121`). The futex precedent calls this out and parks with only Copy values plus a static registry (`kernel/src/sched/futex.rs:143-151`). Purging the registration clones does not drop the owning `Arc` abandoned in the dead syscall frame.

**Required redesign:** place each cell in a registry whose owner is not the blocked frame (for example, a bounded pid/tid+generation keyed table or scheduler/thread-owned storage), and let channel/timer registrations carry only a Copy key or a non-owning generation-checked reference. Resolve handles and register while awake, then drop every resolved `Arc` before the park. Add a force-kill/frame-baseline test proving that killing a `wait_any` waiter returns frames/cells to baseline.

### F2 — HIGH — a single endpoint slot can strand a waiter because same-side handles are clonable

The design assumes “at most one waiter per side, as today's rendezvous” and proposes one registration slot (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:80`). That is not an enforceable capability invariant. `sys_spawn` clones any GRANT-capable handle into the child while the caller retains its handle (`kernel/src/arch/x86_64/syscall.rs:1519-1553`), and full channel handles carry GRANT (`kernel/src/sched/process.rs:224-232`). With `browser-runtime`, one process can also have multiple threads.

Interleaving:

1. Thread/process A registers cell A in side B's receive-wait slot, checks no sender, and heads toward park.
2. A sibling or spawned grantee holding the same side registers cell B.
3. If registration overwrites, A's token can park with no object pointing to cell A.
4. A sender publishes `SendBlocked` and fires only cell B; A sleeps indefinitely.

The existing rendezvous state is also a single slot (`kernel/src/ipc/mod.rs:79-103`), but that does not make silent overwrite safe for a new API.

**Required change:** registration must atomically reject an occupied endpoint slot (`EBUSY`/`EAGAIN`) or use a bounded multi-waiter structure. For N-item setup, define rollback of every earlier registration if a later item collides. Key every slot by cell generation so deregistration from an old call cannot clear a newer registration. Add a two-waiter same-side negative/race test.

### F3 — HIGH — the stated token-full behavior is incompatible with the named park primitive

Step 3 calls `block_current_with_token_and_guard` (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:82`), while the CP-31 table promises a full token table returns `EAGAIN` (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:104`). The named primitive instead panics on a full table (`kernel/src/sched/scheduler.rs:1618-1620`). The fallible primitive is currently compiled only with `browser-runtime` (`kernel/src/sched/scheduler.rs:1668-1689`), whereas the proposed `wait-multiplex` feature is separate.

This wait also differs from the futex precedent: futex installs its queue entry inside the successful token-install closure, so token exhaustion leaves no published waiter (`kernel/src/sched/futex.rs:174-203`). OT-22 has already registered up to eight channel slots and a timer entry before attempting to reserve a token.

**Required change:** make the fallible guarded-token primitive available to `wait-multiplex`, use it, drop the returned cell guard before acquiring any registration/timer lock, roll back every registration on failure, and specify/test each userland caller's `EAGAIN` retry behavior. “The caller spins this iteration” is not an ABI contract and does not establish that deadlines and existing loop error handling remain correct.

### F4 — HIGH — `purge_pid` is not implementable from the specified cell shape, and teardown ordering is load-bearing

The design says process death purges cells from channels and the timed list (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:103`), but the specified `CellState` has only `fired`, `token`, and `why` (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:76-77`). There is no pid/tid/generation by which a global timed table or a shared endpoint slot can identify the dying process's entry. The futex table stores `pid` in every waiter and `purge_pid` removes by it (`kernel/src/sched/futex.rs:62-77`, `kernel/src/sched/futex.rs:398-422`).

The order is also essential: purge every wait registration first, then extract/zombify the scheduler token, then close/drain handles. Otherwise a concurrent send/tick/`close_peer` can take the still-published token and re-ready a thread after forced exit has begun. The current threaded terminate path demonstrates the intended pattern by purging futex entries before walking token tables (`kernel/src/sched/process.rs:2942-2958`).

**Required change:** add stable owner identity plus generation to the registry entry and all registration keys; specify purge-before-zombify-before-close ordering; make old wakers prove `Stale` without touching a reused cell; and make the kill leg assert channel slots, timer slots, token slots, and frame counts all return to baseline.

### F5 — MEDIUM — the proposed lockdep discriminator does not exist for these locks

No existing scheduler→`Channel.state` path was found. Existing IPC block paths take channel→scheduler (`kernel/src/ipc/mod.rs:192-203`, `kernel/src/sched/scheduler.rs:1558-1603`); completion and close paths drop `Channel.state` before waking (`kernel/src/ipc/mod.rs:152-165`, `kernel/src/ipc/mod.rs:539-552`). The proposed collect/drop/fire discipline therefore has no demonstrated inversion.

However, the design claims ordered lockdep levels will cover `Channel.state`, `WaitCell`, and scheduler (`docs/THINK-2026-10-19-ot22-readiness-multiplex.md:86`, `:100`). `IrqSafeMutex::new` currently creates an unordered `SwitchMutex` (`kernel/src/sync/irqsafe_mutex.rs:26-31`), and the documented ordered band explicitly leaves the scheduler lock unordered (`kernel/src/sync/mod.rs:14-20`). That discriminator cannot be delivered merely by assigning constants.

**Required change:** either extend `IrqSafeMutex` and scheduler instrumentation so the claimed chain is actually checked, or narrow the claim and add explicit nested-lock witnesses. The timed-list rule must also say: collect cells under the list lock, drop it, then fire; never take `WaitCell` while holding the timer-list lock.

## Dispatch checklist

1. **Lost wakeup / two-CPU ownership:** the register→check→cell-guard→token-install sequence is CP-14-sound if all publications use the same cell guard and wakers drop foreign locks before firing. Send-before-park sets `fired`; send-after-token takes the token once. Generation-checked scheduler tokens prevent two wakers from owning the parked thread. This conclusion is conditional on fixing F1-F4 and on rechecking/looping after a spurious fire rather than returning a false ready bit.
2. **Lock order:** no present scheduler→channel inversion found; present IPC is channel→scheduler. The planned waker order is sound, but the promised lockdep coverage is not presently available (F5), and timer-list→cell must use collect/drop/fire.
3. **Tick rescue:** yes, a broad desktop test could pass with IPC fire broken. The dedicated channel-only leg plus `wait-multiplex-no-ipc-fire` negative control is sufficient, provided a channel-only/no-deadline wait is not entered in the tick list and the scenario watchdog supplies the negative timeout. Checking `why == IPC` and “same tick” is useful corroboration.
4. **Token capacity:** the present infallible primitive panics; F3 must be fixed. After that, `EAGAIN` can be memory-safe, but it is not proven safe for every caller until all registrations roll back and each loop's retry/deadline semantics are tested. Sustained token exhaustion also defeats the idle-utilization goal even if it preserves correctness.
5. **BSP residency:** true for production user threads today. User spawn and thread-create enqueue on the current CPU (`kernel/src/sched/scheduler.rs:511-534`, `kernel/src/sched/process.rs:2206-2210`), normal user creation originates on the BSP, AP bootstrap receives no routed schedulable work (`kernel/src/sched/scheduler.rs:348-379`), and arbitrary placement is test-only (`kernel/src/sched/scheduler.rs:2008-2049`). If this changes, the EOI-only remote IPI can leave a ready waiter behind a busy AP until it re-enters the scheduler, so the 10 ms bound does not survive.
6. **HID path:** the rung adds no HID work in IRQ context. The pump remains syscall-context work; `flush` uses a bounded `DELIVERY.try_lock` with IF masked (`kernel/src/atrium/focus.rs:280-330`). The tick may only scan a bounded waiter table and collect/fire cells after dropping that table lock. No new `DELIVERY`, `PRIMARY`, or device-lock acquisition belongs in the timer handler.
7. **Alternatives:** B remains materially larger because persistent registrations, ring overflow, and teardown apply to every process/object. C does not express the WM's 2-4 simultaneous interests without rotating deadlines or extra threads. Option A remains the right external shape after the internal lifecycle redesign.
8. **Cross-family structural check:** CP-30 is the main missed structure: Rust ownership in a parked syscall frame is not reclaimed when that thread is force-zombified. The same review also exposed the non-unique endpoint-side assumption, the infallible/fallible park contradiction, and the absent purge key/lockdep mechanism.

## Conditions to advance rung 1

- Replace stack-owned `Arc<WaitCell>`/resolved object ownership across park with registry-owned, generation-keyed state and Copy/non-owning waiter keys.
- Define collision and rollback semantics for all N registrations.
- Use a fallible token park under `wait-multiplex` and prove the full-table path leaves no registration behind.
- Store pid/tid/generation and specify purge-before-zombify-before-close.
- Add the CP-30 frame-baseline kill leg, same-side double-registration leg, token-full rollback leg, wake-before-park leg, and the existing no-IPC-fire negative control.

