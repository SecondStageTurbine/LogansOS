# DISPATCH - Phase 3 BUILD-1 1B0 scheduler weak wake-token substrate (Jarvis)

**Date filed:** 2026-05-12
**From:** PM (NeoPlutarch Phase 3 cadence)
**To:** Jarvis (Codex-backed kernel implementation lane)
**Ferry:** PL ferries to Jarvis session.
**Status:** Focused unblock dispatch after Jarvis correctly stopped at the 1B hard gate. Continue on branch `codex/phase-3-build-1-jarvis`.

---

## 1. Mandate

Implement the scheduler-owned weak wake-token substrate needed before Phase 3 BUILD-1 1B virtio-blk I/O can proceed.

This dispatch supersedes the literal `Option<Weak<Thread>>` wording in the original BUILD-1 dispatch. ADR-021 has been refined: the requirement is weak, non-owning wake authority. In the current Box-owned scheduler, the concrete type is a scheduler-owned parked-thread table plus a generation-checked wake token.

Do not migrate the scheduler to `Arc<Thread>` in this dispatch.

---

## 2. Required inputs

Read before coding:

1. `Plans/PM-DECISION-2026-05-12-phase-3-build-1-1b-scheduler-lifetime-blocker.md`
2. `Plans/FAILURE-2026-05-12-phase-3-build-1-jarvis.md`
3. `docs/ADR-021-phase-3-design-substrate.md` Q2 and BUILD-1 acceptance gates
4. `kernel/src/sched/scheduler.rs`
5. `kernel/src/sched/thread.rs`
6. `kernel/src/ipc/mod.rs`
7. `kernel/src/sync/switch_mutex.rs`
8. `kernel/src/test_harness.rs`
9. `kernel/Cargo.toml`
10. `tools/xtask/src/main.rs`

---

## 3. Design target

Add a scheduler token with these semantics:

```rust
pub struct ThreadWakeToken {
    cpu_id: u32,
    slot: u32,
    generation: u32,
}
```

Names and exact field widths are Jarvis's discretion. Semantics are not.

The token:

- is non-owning;
- is safe to store inside storage `IoRendezvous`;
- does not keep a `Thread` alive;
- can be resolved by storage completion to wake the parked thread;
- fails cleanly if process exit has already abandoned/zombified the thread;
- uses a generation counter to reject stale slot reuse.

The scheduler:

- owns parked `Box<Thread>` values in a scheduler-owned table or per-CPU parked table;
- provides an atomic park/register/release operation for storage rendezvous;
- provides a wake-by-token operation that extracts the parked thread, writes the blocked return value, enqueues it, and sends an SC-3C cross-CPU wake/reschedule IPI when needed;
- provides an abandon/zombify path that process exit can use, or integrates with the existing blocked-thread kill walk so storage-blocked threads are not immortal.

---

## 4. API shape

Recommended API sketch:

```rust
pub fn block_current_with_token_and_guard<T, F>(
    guard: SwitchMutexGuard<'_, T>,
    install: F,
) -> i64
where
    F: FnOnce(&mut T, ThreadWakeToken);

pub fn wake_thread_token(token: ThreadWakeToken, return_value: i64) -> WakeTokenResult;

pub enum WakeTokenResult {
    Woken,
    Gone,
    Stale,
}
```

Requirements:

- `block_current_with_token_and_guard` must be atomic with release of the caller guard, matching the existing `block_current_into_with_guard` lost-wakeup discipline.
- The install closure receives the token while both the caller state and scheduler state still make the park operation atomic.
- The function returns through the resumed thread's blocked-return path, just like existing IPC blocking syscalls.
- `wake_thread_token` may be called from IRQ completion context under the existing IRQ-off discipline. If it wakes a thread owned by another CPU, it must compose with the SC-3C cross-CPU wake/reschedule path.
- Do not require storage code to hold an owning `Box<Thread>`.

If this API shape cannot be implemented without reopening a scheduler invariant, STOP and file a failure artifact.

---

## 5. Process-exit composition

The abandoned-I/O gate is load-bearing.

Jarvis must ensure a thread blocked on storage I/O can be removed from the scheduler-owned parked table during process exit or kill. After that removal:

- the thread is zombified/reaped through normal scheduler cleanup;
- a later storage completion sees `Gone` or `Stale`;
- DMA buffers are released by the storage rendezvous, not by scheduler cleanup;
- no `Thread` lifetime is extended by the rendezvous token.

If the current process-exit walk only sees IPC endpoint-owned blocked threads, extend it with a storage/scheduler-blocked path or add a scheduler API process exit can call by `ProcessId`.

---

## 6. Verification gates

All must pass before committing 1B0:

- `cargo k`
- `cargo k --no-default-features`
- `cargo neo-test all`, unless still host-blocked by Limine; if blocked, record the exact same environment blocker and run the targeted build gates.
- New scenario `phase3_sched_wake_token_round_trip`:
  - a kernel test thread blocks through the new token API;
  - another thread wakes it by token;
  - blocked return value is observed exactly once.
- New scenario `phase3_sched_wake_token_abandon`:
  - a thread blocks through the new token API;
  - process/kill cleanup removes or zombifies it;
  - later wake by token returns `Gone` or `Stale`;
  - no zombie/reap leak remains.
- New scenario `phase3_sched_wake_token_stale_generation`:
  - force slot reuse;
  - old token must not wake the new occupant.
- Existing regression scenarios:
  - `smp_cross_cpu_rendezvous_stress`
  - `q8c_blocked_kill_walk`
  - `q8c_close_peer_my_side_extract`
  - cap grant/revoke scenarios if `cargo neo-test all` remains environment-blocked.
- `cargo fmt --check` if rustfmt exists; otherwise record BLOCKED as before.

If QEMU remains blocked by missing Limine tooling, do not fake runtime PASS. The verification artifact should say build PASS / runtime BLOCKED and preserve exact blocker text.

---

## 7. Verification artifact

Create:

```text
Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-VERIFICATION-2026-05-12.md
```

Include:

- commit hash;
- files changed;
- API names actually chosen;
- exact token semantics;
- process-exit/abandon path;
- test results;
- whether QEMU runtime verification was run or host-blocked.

If blocked:

```text
Plans/FAILURE-2026-05-12-phase-3-build-1-1b0-jarvis.md
```

---

## 8. After 1B0

If 1B0 passes, resume the original BUILD-1 sub-commit 1B virtio-blk read/write implementation with this revised rendezvous shape:

```rust
struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<ThreadWakeToken>,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}
```

The original hard gates still apply:

- `VIRTIO_BLK_F_FLUSH` offered and not negotiated;
- `VIRTIO_BLK_F_CONFIG_WCE` not negotiated;
- DMA buffers Arc-owned until completion;
- abandoned-I/O test proves thread lifetime is not extended;
- descriptor-ring barrier audit;
- aarch64 barrier lane before BUILD-1 closure.

---

**End of dispatch.**
