# Phase 3 BUILD-1 Sub-Commit 1B0 Verification

Date: 2026-05-12
Lane: Jarvis / Codex
Branch: `codex/phase-3-build-1-jarvis`
Commit: containing commit. Use `git log -1 --oneline` for the final hash; a
self-referential hash cannot be embedded exactly in the commit it identifies.

## Summary

- Added scheduler-owned weak wake tokens with `ThreadWakeToken { cpu_id, slot, generation }` and `WakeTokenResult`.
- Added fixed per-CPU parked-token slots that hold scheduler-owned `Box<Thread>` values. No scheduler migration to `Arc<Thread>` was performed.
- Added `block_current_with_token_and_guard` for atomic guard install plus scheduler park/release.
- Added `wake_thread_token` with generation checks and `Woken` / `Gone` / `Stale` outcomes.
- Added token-parked process-exit composition through `zombify_parked_thread_by_pid`.
- Added the `phase3_sched_wake_token_round_trip`, `phase3_sched_wake_token_abandon`, and `phase3_sched_wake_token_stale_generation` scenarios.

## Files changed

- `kernel/src/sched/scheduler.rs`
- `kernel/src/sched/mod.rs`
- `kernel/src/sched/process.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/test_harness.rs`
- `kernel/Cargo.toml`
- `tools/xtask/src/main.rs`

## Verification Commands

| Command | Result | Notes |
|---|---|---|
| `cargo k` | PASS | Kernel/userspace build completed. Existing warnings remain; new token APIs are intentionally unused by the default kernel until storage 1B wiring. |
| `cargo k --no-default-features` | PASS | Kernel/userspace build completed. |
| `cargo neo-test phase3_sched_wake_token_round_trip` | BLOCKED | Scenario kernel builds, then host ISO assembly stops before QEMU: `Limine directory /home/neo/limine does not exist. Set $LIMINE_DIR or build the dev container.` |
| `cargo neo-test phase3_sched_wake_token_abandon` | BLOCKED | Scenario kernel builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test phase3_sched_wake_token_stale_generation` | BLOCKED | Scenario kernel builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test smp_cross_cpu_rendezvous_stress` | BLOCKED | Regression scenario kernel builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test q8c_blocked_kill_walk` | BLOCKED | Regression scenario kernel builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test q8c_close_peer_my_side_extract` | BLOCKED | Regression scenario kernel builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test cap_grant_basic` | BLOCKED | Cap scenario kernel/userspace builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test cap_grant_missing_meta` | BLOCKED | Cap scenario kernel/userspace builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test cap_revoke_local` | BLOCKED | Cap scenario kernel/userspace builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test cap_revoke_cross_process` | BLOCKED | Cap scenario kernel/userspace builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test cap_revoke_descendants_walk` | BLOCKED | Cap scenario kernel/userspace builds, then the same host Limine dependency stops QEMU. |
| `cargo neo-test all` | BLOCKED | First scenario builds, then the same host Limine dependency stops QEMU before runtime coverage. |
| `cargo fmt --check` | BLOCKED | `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`. |
| `git diff --check` | PASS | No whitespace errors; Git only reported expected LF-to-CRLF conversion warnings on touched files. |

## Scenario Intent

- `phase3_sched_wake_token_round_trip`: parks a kernel thread by token, wakes it through `wake_thread_token`, verifies the return value, and reaps the completed thread.
- `phase3_sched_wake_token_abandon`: parks a process-owned thread, terminates that process, verifies the token slot is drained, and verifies a late wake cannot resurrect the abandoned thread.
- `phase3_sched_wake_token_stale_generation`: reuses the same token slot, verifies the previous generation returns `Stale`, then wakes the current generation.

## Hard Gate Notes

- `Weak<Thread>` was not introduced.
- Scheduler thread ownership remains `Box<Thread>` in ready/current/zombie queues and in the token parked table.
- The token table is fixed-size per CPU and does not allocate while holding scheduler locks.
- Generation checking distinguishes stale handles from drained/gone parked threads.
- Runtime PASS is not claimed on this Windows host because QEMU did not launch.

## Serial Logs

- Expected new serial logs:
  - `target/neo-test/phase3_sched_wake_token_round_trip.serial.log`
  - `target/neo-test/phase3_sched_wake_token_abandon.serial.log`
  - `target/neo-test/phase3_sched_wake_token_stale_generation.serial.log`
- Not produced on this host because ISO assembly blocked before QEMU launch.

## Next Step

Original 1B virtio-blk read/write wiring should resume after 1B0 receives runtime execution on a host with the Limine ISO tooling available.
