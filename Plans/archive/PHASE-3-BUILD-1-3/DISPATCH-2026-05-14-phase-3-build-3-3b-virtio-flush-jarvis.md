# Dispatch - Phase 3 BUILD-3B Virtio-blk FLUSH

**Date:** 2026-05-14
**Agent:** Jarvis
**Status:** AUTHORIZED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-3a-consumption-and-3b-authorization.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md`
- `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
- `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`

---

## 1. Mission

Implement BUILD-3 sub-commit 3B only: add a block-layer flush surface and negotiate/issue virtio-blk FLUSH commands.

Base:

```text
e4ac0746dbaf6f4b6f26e16a895fe0f40c5c8ba5
```

Recommended branch:

```text
codex/phase-3-build-3-3b-virtio-flush
```

## 2. Required Outcome

3B changes the storage stack from "F_FLUSH offered but not negotiated" to "F_FLUSH offered and negotiated, with real FLUSH command completion evidence."

Required semantics:

- add a kernel-internal block flush API/capability query;
- negotiate `VIRTIO_BLK_F_FLUSH` when the device offers it;
- continue to refuse attach if BUILD-3's required FLUSH capability is not offered;
- keep `VIRTIO_BLK_F_CONFIG_WCE` not negotiated;
- implement `VIRTIO_BLK_T_FLUSH` request issue and completion through the existing virtqueue path;
- preserve descriptor-publish and notification-suppression barrier discipline;
- retain DMA/header/status lifetime until completion for FLUSH requests;
- update probe and runtime evidence logs so `F_FLUSH_negotiated=true` is explicit.

## 3. Scope Boundaries

Authorized files are whatever 3B genuinely needs in:

- block-layer driver interfaces;
- virtio-blk driver;
- kernel test harness feature/scenario plumbing;
- xtask scenario wiring;
- verification artifact.

3B must not implement:

- NeoFS write-back cache;
- mark-sweep GC;
- free-space bitmap/format changes;
- `sys_fsync` behavior changes;
- durable fsync over write-back;
- CAS/dedup;
- AHCI or NVMe;
- FAT32/ext4 or foreign FS;
- rootfs pivot;
- file-backed mmap/exec;
- broader POSIX syscalls.

BUILD-2 `sys_fsync` remains a no-op over the current synchronous path. 3B may add a direct block-device flush test, but `sys_fsync` must not start calling it yet.

## 4. Suggested Implementation Notes

Use local patterns over this sketch, but the likely changes are:

- extend `kernel/src/drivers/block/mod.rs` with flush support, such as `BlockDevice::flush()` and `supports_flush()` or an equivalent capability surface;
- add `BlockOp::Flush` only if it composes cleanly with no data buffers;
- add `VIRTIO_BLK_T_FLUSH = 4` in `kernel/src/drivers/block/virtio_blk.rs`;
- teach the pending-I/O/request builder to handle header + status descriptors without a data descriptor, if that is the spec-correct local shape;
- keep request completion accounting and wake-token behavior shared with read/write;
- keep `CONFIG_WCE` policy separate and false.

Do not paper over a no-data descriptor bug by attaching a fake data block. If the existing descriptor builder cannot represent FLUSH cleanly, refactor narrowly enough that read/write behavior and abandoned-I/O DMA retention stay covered.

## 5. Required Runtime Scenarios

Add or update scenarios so the final serial evidence proves:

1. FLUSH negotiation gate
   - `F_FLUSH_offered=true`
   - `F_FLUSH_negotiated=true`
   - `F_CONFIG_WCE_negotiated=false`
   - attach succeeds only with offered FLUSH

2. FLUSH command smoke
   - a real `VIRTIO_BLK_T_FLUSH` command is issued;
   - completion is observed through the virtio used-ring path;
   - completion wakes the blocked caller through the existing storage rendezvous;
   - no data buffer is corrupted or fabricated;
   - descriptor-publish and notification-suppression barrier counters remain correct.

3. Read/write regression under negotiated FLUSH
   - raw read/write still passes;
   - existing VFS/block smoke still passes;
   - logs now correctly show `F_FLUSH_negotiated=true` and `F_CONFIG_WCE_negotiated=false`.

4. Fsync non-transition guard
   - the BUILD-2 `phase3_neofs_fsync_noop_write_through` behavior remains a no-op;
   - if that scenario's log text must change because the driver now supports FLUSH, make the assertion precise: fsync itself did not issue FLUSH and did not mutate generation.

Suggested scenario names:

```text
phase3_virtio_blk_flush_negotiated_gate
phase3_virtio_blk_flush_command_smoke
```

Renaming the existing `phase3_virtio_blk_flush_offered_gate` is acceptable if xtask and artifacts make the transition obvious.

## 6. Verification Requirements

Run and report:

```text
cargo k
cargo k --no-default-features
cargo check -p xtask
```

Run new 3B targeted Docker/QEMU scenarios.

Run targeted regression gates:

```text
cargo neo-test phase3_virtio_blk_raw_rw_smoke
cargo neo-test phase3_virtio_blk_abandoned_io_no_leak
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_neofs_fsync_noop_write_through
cargo neo-test phase3_aarch64_virtio_barrier_lane
```

Because 3B touches the virtio request path, run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Use the established Docker path if host QEMU/Limine is blocked.

Run:

```text
git diff --check
git diff --cached --check
```

Scoped rustfmt for changed Rust files is expected. Do not claim global `cargo fmt --check` PASS unless it actually passes globally.

## 7. Verification Artifact

Create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md
```

Include:

- exact commit SHA;
- changed files;
- block flush API summary;
- virtio feature policy summary;
- PASS frames for FLUSH negotiation and command completion;
- evidence that `CONFIG_WCE` remains not negotiated;
- evidence that `sys_fsync` behavior did not change;
- targeted regression and full SMP sweep results;
- aarch64 barrier lane result;
- explicit statement that no write-back, GC, fsync transition, CAS, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work landed.

Do not leave a placeholder for the commit SHA in the committed verification artifact.

## 8. Stop Conditions

Stop and file a blocker instead of committing if:

- QEMU does not offer `VIRTIO_BLK_F_FLUSH`;
- FLUSH cannot be negotiated without also negotiating `CONFIG_WCE`;
- a real FLUSH command cannot be issued and completed through the existing virtqueue path;
- FLUSH request lifetime requires fake data buffers, leaked DMA buffers, or `Arc<Thread>` ownership;
- descriptor publish or notification suppression barriers regress;
- the aarch64 barrier lane falls back to compile-only evidence;
- `sys_fsync` behavior changes before the durable fsync contract dispatch;
- the implementation starts write-back cache, GC, free-space format changes, CAS, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work.

If blocked, create:

```text
Plans/BLOCKER-2026-05-14-phase-3-build-3-3b-virtio-flush-jarvis.md
```

## 9. Commit Discipline

Commit only 3B implementation and its verification or blocker artifact.

Suggested commit message:

```text
kernel/storage: negotiate virtio-blk flush
```

Do not merge to `main`. Do not push. Do not start 3C.
