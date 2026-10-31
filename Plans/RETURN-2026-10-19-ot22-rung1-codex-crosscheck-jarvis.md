<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-22 rung-1 (`SYS_WAIT_ANY`) crosscheck — Jarvis

**Seat:** Jarvis (Codex-family)  
**Reviewed head:** `290e7f508804c3967b10921a862ee7dea0c15292`  
**Verdict:** **BOUNCE**

The implementation successfully replaces the design's unsafe stack-owned `Arc<WaitCell>` with a bounded table of Copy keys/tokens, supports multiple waiters per channel side, uses the fallible token park, and supplies pid-keyed purge. One independent liveness bug remains: numeric handle identity can change or disappear without firing the channel key on which an IPC-only wait parked. This can strand an infinite-deadline waiter.

The local RAG service was healthy but its index was stale (`last_indexed=2026-10-17`) and returned no OT-22 results, so this review used direct reads of the candidate source.

## Finding

### F1 — HIGH — channel handle revoke/rebind can strand an IPC-only waiter forever

`wait_any_items` resolves each numeric handle to a channel pointer/side, stores those raw channel keys, registers them, then resolves the numeric handles a second time (`kernel/src/arch/x86_64/syscall.rs:4630-4770`). The second scan's newly resolved keys are written into the local `keys` array but are not compared with the keys already stored in the registration (`kernel/src/arch/x86_64/syscall.rs:4750-4770`). The table watches channel identity, not the capability/handle-slot identity (`kernel/src/sched/wait_any.rs:58-68`, `:131-159`).

Two reachable failures follow.

**Close/reuse interleaving (kernel built with both `browser-runtime` and `wait-multiplex`):**

1. Waiter W's first scan resolves handle `h` to channel A, finds no sender, and records key A locally.
2. Before W calls `register`, sibling S closes `h`. `close_peer` fires key A, but W is not registered yet, so the fire is legitimately empty (`kernel/src/arch/x86_64/syscall.rs:5122-5154`).
3. S creates/receives another channel handle. `HandleTable::install` deliberately reuses the lowest empty slot, so the new channel B is again named `h` (`kernel/src/sched/process.rs:1282-1292`).
4. W registers key A. Its second scan resolves `h` to B and finds B not ready, but it does not detect that the registered key and current handle identity differ.
5. W parks on key A with no deadline/input tick. A sender on B fires key B. W is never targeted and sleeps forever.

**Cross-process revoke interleaving (does not require a sibling thread):**

1. W parks indefinitely on a derived channel handle.
2. The ancestor revokes that derivation. `revoke_handle` removes matching capability entries from every affected handle table (`kernel/src/sched/process.rs:1753-1811`), and `remove_cap_ids` simply clears the slots (`kernel/src/sched/process.rs:1390-1400`). It does not close the channel or fire `WAIT_TABLE`.
3. With no channel send/close and no deadline/input item, W is never woken to rescan and return `EBADF`.

This is not a harmless stale-key wake: it is the opposite direction, a missing wake after the item named by the ABI stops naming the registered object. Input/SAK waits escape through their next-tick recheck; channel-only waits intentionally do not.

**Required fix:** make handle/capability invalidation part of the watched condition. One sound shape is a stable cap-id/handle-generation snapshot per item, an under-lock equality check between registration and the second scan, and a handle-table mutation hook that fires affected waiters on close/revoke/replacement. A bounded periodic recheck for every wait can also preserve liveness, but then the IPC negative control must continue to prove the return cause is a real IPC fire rather than tick rescue. Add both regressions above; the close/reuse test must place W and S in one process and require a send on B to wake W, while the revoke test must require the parked call to return an invalidation error without any channel traffic.

## Dispatch checklist

1. **CP-14 / send timing:** for a stable handle identity, the send wake is sound. Send-before-registration is caught by the first/second `recv_ready`; send after registration sets `fired`; send during token install serializes on `WAIT_TABLE`; the sender publishes `SendBlocked` while retaining `Channel.state`, so a re-scan cannot pass it before publication. Tick-vs-channel fire is take-once under the table lock. BSP waiter/AP sender uses the generation-checked token and the BSP's tick/idle loop for dispatch. F1 is the uncovered identity-change interleaving adjacent to this proof.
2. **Lock order:** all observed nesting follows `Channel.state → WAIT_TABLE → scheduler`. `wait_any_scan` drops each channel lock before `park` takes the table; tick/purge take only the table and drop it before waking; teardown purges before scheduler zombify and closes channels later. No `WAIT_TABLE → Channel.state`, scheduler→table, or scheduler→channel edge was found. Waking while `Channel.state` is held lengthens that IF-off section and differs from the original design, but no reverse edge currently makes it a deadlock.
3. **Counters:** `CHANNEL_WAITERS` and `TIMED_WAITERS` increment once per installed entry and decrement only after a ticket match or a `purge_pid` take. Fire/expire leave counts intact until removal, while `recompute_earliest` excludes fired entries. Register, fire, expire, remove, and purge therefore have no found underflow/double-remove path; a newly registered timed waiter can miss at most the currently racing tick, not all future ticks.
4. **Teardown / CP-30/33/36:** the user item array, pid, registration, keys, deadline, and token are Copy/static-table state. `ipc_lookup`'s channel/Pmap clones, the focus clone, `caller_holds_input_system`'s process clone, and the exiting-process check all drop before `park`. `terminate` marks exiting, purges before killing token waiters, and purges again after the race-closing zombify pass; fault kill follows the same purge-before-zombify shape. A fire that already took a token either wakes before kill (then `kill_thread_by_pid` removes it) or finds `Stale` after zombify. No owned resource across park was found.
5. **Readiness parity:** `recv_ready` is exactly `SendBlocked | Closed`, matching `recv`/`try_recv`; grants neither report ready nor fire. Focus readiness requires receivable+queued and input/SAK always get a next-tick pump/recheck. The four desktop loop mappings otherwise consume what they wait for. F1 is the sole found source-without-waker: handle invalidation/rebinding.
6. **Kill switches:** kernel `wait_any` modules, syscall route, tick expiry, main-loop sleep, and corrected observability accounting are cfg-gated by `wait-multiplex`; user calls are cfg-gated by `desktop-park`; xtask selects parked features only for the parked scenario. `IDLE_TID` publication exists unconditionally, but all behavior-changing consumers are gated, so no feature-off behavior drift was found.
7. **Cross-family structural check:** the implementation handled the design review's Arc lifetime, multi-waiter, token-full, pid purge, and unordered-lock realities. The missed structure is that a wait item names a mutable numeric handle slot while the registration names only the object that happened to occupy it during the first scan.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo test -p atrium-logic-tests --locked` — PASS, 112 tests.
- `cargo check -p neoplutarch-kernel --features browser-runtime,wait-multiplex` — PASS; the close/reuse configuration is a supported build combination.
- The relay reported the exact-head QEMU gate green. I did not repeat the three long QEMU scenarios; their existing legs do not exercise handle revoke/rebind.
