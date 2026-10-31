# Phase 3 BUILD-1 Sub-Commit 1B Verification

Date: 2026-05-12
Lane: Jarvis / Codex
Branch: `codex/phase-3-build-1-jarvis`
Base commit: `3b5d35d plans: ratify 1b0 runtime wake-token gate`
Commit: containing commit. Use `git log -1 --oneline` for the final hash; a
self-referential hash cannot be embedded exactly in the commit it identifies.
Dispatch: `DISPATCH-2026-05-12-phase-3-build-1-1b-resume-virtio-blk-jarvis.md`

## Verdict

PASS. Original BUILD-1 sub-commit 1B is implemented and runtime-verified.

No 1C VFS/syscall work and no 1D closure work was started.

## Summary

- Replaced the 1A probe-only virtio-blk path with a narrow modern virtio-blk read/write path.
- Kept the BUILD-1 feature policy strict: `VIRTIO_BLK_F_FLUSH` must be offered, while only `VIRTIO_F_VERSION_1` is negotiated.
- Kept `VIRTIO_BLK_F_FLUSH` and `VIRTIO_BLK_F_CONFIG_WCE` out of `driver_features`.
- Wired synchronous single-block reads and writes through a split virtqueue.
- Used `IoRendezvous.waiting_thread: Option<ThreadWakeToken>` and `block_current_with_token_and_guard`; no `Arc<Thread>` migration was introduced.
- Retained request header, data, and status buffers as `Arc<DmaBuffer>` inside `IoRendezvous` until completion handling is finished.
- Added neo-test live DMA-buffer accounting and an abandoned-I/O scenario that verifies late completion cannot resurrect the issuer.
- Split virtqueue ordering into descriptor-publish, notification-suppression, and used-ring acquire barriers.

## Files Changed

- `kernel/Cargo.toml`
- `kernel/src/drivers/block/dma.rs`
- `kernel/src/drivers/block/mod.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/drivers/pci.rs`
- `kernel/src/sched/scheduler.rs`
- `kernel/src/sched/thread.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`
- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md`

## Verification Commands

| Command | Result | Notes |
|---|---|---|
| `cargo k` | PASS | Kernel/userspace build completed with existing warnings only. |
| `cargo k --no-default-features` | PASS | Kernel/userspace build completed with existing warnings only. |
| `docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke` | PASS | Serial log: `target\neo-test\phase3_virtio_blk_raw_rw_smoke.serial.log`. |
| `docker compose run --rm neo cargo neo-test phase3_virtio_blk_flush_offered_gate` | PASS | Serial log: `target\neo-test\phase3_virtio_blk_flush_offered_gate.serial.log`. |
| `docker compose run --rm neo cargo neo-test phase3_virtio_blk_abandoned_io_no_leak` | PASS | Serial log: `target\neo-test\phase3_virtio_blk_abandoned_io_no_leak.serial.log`. |
| `$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke` | PASS | Serial log refreshed at `target\neo-test\phase3_virtio_blk_raw_rw_smoke.serial.log`. |
| `$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test all` | PASS after rerun | First attempt failed only `irqsafe_mutex_bench` timing threshold (`overhead_ns=11034 threshold_ns=6000`). Standalone `irqsafe_mutex_bench` rerun passed, then the full `all` rerun passed. Current bench serial log: `target\neo-test\irqsafe_mutex_bench.serial.log`. |
| `cargo fmt --check` | BLOCKED | `error: 'cargo-fmt.exe' is not installed for the toolchain 'nightly-2026-04-16-x86_64-pc-windows-msvc'.` |
| `git diff --check` | PASS | No whitespace errors; Git reported only expected LF-to-CRLF warnings on touched files. |

Current serial logs listed above are from the final clean targeted/full-run pass.

## Feature Negotiation Evidence

Probe/runtime feature-policy line:

```text
[ kernel ] virtio-blk: feature-policy bdf=00:03.0 offered=0x0000010130007e54 driver=0x0000000100000000 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false queue_size=8 capacity_sectors=32768 attach=rw
```

Flush gate PASS line:

```text
NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_offered_gate bdf=0x000300 offered_features=0x0000010130007e54 driver_features=0x0000000100000000 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false block_count=32768 block_size=512 attach=rw
```

The driver features value is exactly `0x0000000100000000`, which is `VIRTIO_F_VERSION_1` only. `F_FLUSH` is offered by the device and not negotiated. `CONFIG_WCE` is not negotiated.

## Raw Read/Write Evidence

```text
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 token_cpu=0 token_slot=0 wake=Woken
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 token_cpu=0 token_slot=0 wake=Woken
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

## Abandoned I/O No-Leak Evidence

```text
[ kernel ] virtio-blk: completion bdf=0x000300 status=0 token_cpu=0 token_slot=0 wake=Gone
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

`late_wake=2` is the neo-test encoding for `WakeTokenResult::Gone`. The completion does not resurrect the process-owned issuing thread, the in-flight DMA count drains to zero during completion, and the live DMA-buffer count returns to the pre-I/O baseline.

## Wake/Completion Path

`handle_interrupt()` drains the same completion path as the runtime worker:

```text
handle_interrupt() -> poll_primary_completions() -> poll_used() -> wake_thread_token()
```

The lazy completion worker is still used at runtime because block probing happens before scheduler initialization and this tree does not yet route storage interrupts through an IRQ vector. The IRQ-facing entry point exists and shares the completion drain; full hardware interrupt routing remains outside 1B.

## Descriptor-Ring Barrier Summary

- Descriptor publish barrier: `descriptor_publish_barrier()` records the event and executes `fence(Ordering::Release)` before publishing the avail index.
- Notification-suppression barrier: `notification_suppression_barrier()` records the event and executes `fence(Ordering::SeqCst)` before the queue notify write.
- Used-ring acquire barrier: `used_ring_acquire_barrier()` executes `fence(Ordering::Acquire)` before consuming used-ring status.
- Runtime evidence: `phase3_virtio_blk_raw_rw_smoke` reported `descriptor_publish_barriers=2` and `notification_suppression_barriers=2` for one write plus one read.

For x86_64 this is stronger than the minimum descriptor-publish ordering and preserves the store/load ordering needed before notification-suppression checks and notify. The code uses Rust atomic fences rather than x86-only intrinsics, but no aarch64 runtime lane exists in this repo today; `kernel/src/arch/mod.rs` still documents aarch64 as future work. BUILD-1 closure still needs a real aarch64 CI lane or a formal blocker artifact. This 1B sub-commit does not claim BUILD-1 closure.
