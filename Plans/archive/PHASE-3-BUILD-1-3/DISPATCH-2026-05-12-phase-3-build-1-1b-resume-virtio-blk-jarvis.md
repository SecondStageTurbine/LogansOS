# Dispatch: Phase 3 BUILD-1 1B Resume - Virtio-blk Read/Write

Date: 2026-05-12
Recipient: Jarvis
Branch: `codex/phase-3-build-1-jarvis`
Base state: `3b5d35d plans: ratify 1b0 runtime wake-token gate`
Priority: Resume original BUILD-1 sub-commit 1B only

## PM Decision

1B0 is runtime-ratified.

Evidence:

- `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B0-RUNTIME-ADDENDUM-2026-05-12.md`
- Targeted 1B0 runtime gate PASS under Docker/Limine with `NEO_SMP=4`.
- Full `docker compose run --rm neo cargo neo-test all` PASS under the same lane.
- Artifact committed at `3b5d35d`.

PM now authorizes original BUILD-1 sub-commit 1B to resume.

Do not proceed into 1C or 1D until 1B lands, verifies, and reports back.

## Important Correction To Original 1B Dispatch

The original BUILD-1 dispatch contained the pre-blocker placeholder:

```rust
waiting_thread: Option<Weak<Thread>>
```

That is now superseded by the 1B0 scheduler-owned wake-token substrate. Use:

```rust
struct IoRendezvous {
    done: AtomicBool,
    waiting_thread: Option<ThreadWakeToken>,
    dma_buffers: Vec<Arc<DmaBuffer>>,
    result: IoResult,
}
```

`ThreadWakeToken` is the local weak-wake equivalent for BUILD-1. Do not migrate scheduler ownership to `Arc<Thread>`.

## 1B Scope

Resume the original 1B virtio-blk write-through read/write path:

- modern virtio-blk queue setup;
- synchronous `read_block` and `write_block` over async device completion;
- IRQ completion that calls `wake_thread_token`;
- issuing thread blocks through `block_current_with_token_and_guard`;
- DMA buffers remain `Arc<DmaBuffer>` and stay alive until completion no longer needs them;
- F_FLUSH offered gate is mandatory;
- `VIRTIO_BLK_F_FLUSH` and `VIRTIO_BLK_F_CONFIG_WCE` are not negotiated in BUILD-1;
- descriptor-ring barriers distinguish descriptor-publish ordering from post-avail notification-suppression ordering.

Keep the implementation narrow. No VFS work in this resume dispatch.

## Required Scenarios

Add or complete:

- `phase3_virtio_blk_raw_rw_smoke`
- `phase3_virtio_blk_flush_offered_gate`
- `phase3_virtio_blk_abandoned_io_no_leak`

The abandoned-I/O scenario must prove late completion cannot resurrect the issuing thread and that `Arc<DmaBuffer>` ownership drains after completion/abandon handling.

## Verification Gate

Run at minimum:

```powershell
cargo k
cargo k --no-default-features
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
docker compose run --rm neo cargo neo-test phase3_virtio_blk_flush_offered_gate
docker compose run --rm neo cargo neo-test phase3_virtio_blk_abandoned_io_no_leak
$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
$env:NEO_SMP='4'; docker compose run --rm neo cargo neo-test all
```

If `cargo fmt --check` remains blocked by the missing pinned nightly rustfmt, record that exactly as prior artifacts do.

## Reporting

On PASS:

- Create `Plans/PHASE-3-BUILD-1-SUB-COMMIT-1B-VERIFICATION-2026-05-12.md`.
- Include commit hash, files changed, commands/results, serial-log paths, and feature-negotiation log lines.
- Include the exact F_FLUSH offered/not-negotiated evidence.
- Include the abandoned-I/O no-leak result.
- Include the descriptor-ring barrier summary for x86 and the status of the aarch64 exposure problem.
- Commit the code and verification artifact as the 1B sub-commit.

On FAIL:

- Stop.
- File `Plans/FAILURE-2026-05-12-phase-3-build-1-1b-jarvis.md`.
- Include failing scenario, serial log, suspected source area, and whether the failure is storage logic, wake-token integration, virtqueue ordering, or environment.

## Stop Conditions

Stop immediately if:

- F_FLUSH is not offered and cannot be made available in the BUILD-1 QEMU/device configuration;
- the driver attaches when F_FLUSH is absent;
- BUILD-1 negotiates F_FLUSH or CONFIG_WCE;
- I/O completion requires `Arc<Thread>` or any owning thread reference;
- `block_current_with_token_and_guard` cannot be used without a lost-wakeup window;
- DMA buffers can be freed before the device completion path is finished;
- descriptor-ring ordering collapses into one vague fence;
- existing `docker compose run --rm neo cargo neo-test all` regresses.

## PM Posture

The scheduler lifetime blocker is closed. The next risk is storage correctness: virtio feature policy, virtqueue ordering, IRQ completion, and abandoned-I/O cleanup. Keep 1B focused there.
