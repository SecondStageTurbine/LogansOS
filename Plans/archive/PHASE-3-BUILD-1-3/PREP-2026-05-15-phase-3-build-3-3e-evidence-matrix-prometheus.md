# PREP - Phase 3 BUILD-3E Evidence Matrix

Date: 2026-05-15
Agent: Prometheus
Status: PLANNING ARTIFACT
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md`
- `Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-durable-fsync-jarvis.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md`

---

## 1. Purpose

Evidence matrix and close-readiness checklist for BUILD-3E: durable NeoFS
`sys_fsync`. This artifact is consumed by Jarvis during implementation and by PM
when judging the 3E verification artifact.

This document does not implement, verify, or close BUILD-3E.

## 2. Implementation Context

The current 3E implementation uses a **conservative mount-wide drain**:
`fsync_regular()` acquires the namespace entries lock (serializing with mutation)
and calls `cache.drain_all()`, which pauses the background worker and drains all
dirty blocks in CoW order (data → inode → root-dir → free-map → superblock),
then issues one FLUSH when `flush_required` is set and all blocks are clean.

Per-file dirty attribution does not yet exist. The cache tracks dirty state by
block number and role, not by owning file. This is documented in the code and
must be stated explicitly in the verification artifact.

## 3. Evidence Matrix

### E1 — syscall/VFS contract for sys_fsync(fd)

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_noop_write_through` (contract-changed evidence) |
| **Expected PASS-frame fields** | `event=PASS`, `neofs_fsync=durable_3e_contract_changed`, `fsync_rc=0`, `generation_before==generation_after`, `fsync_drained_cache=true`, `dirty_blocks_after_fsync=0`, `drained_blocks_delta>0`, `cache_flushes_delta>=1`, `flush_commands_delta>=1`, `flush_completions_delta>=1` |
| **Failure mode** | fsync returns without draining cache; fsync mutates NeoFS generation unexpectedly; fsync returns non-zero for valid fd |
| **Gate** | **Hard close gate** — proves the 3E contract changed from the 3D no-op and that fsync actually drains and flushes |

### E2 — stale FileCap rejection before durability work

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_filecap_stale_regression` |
| **Expected PASS-frame fields** | `event=PASS`, `stale_fsync=ESTALE`, `stale_errno=-116`, `stale_before_durability=true`, `dirty_blocks_before_stale==dirty_blocks_after_stale`, `drained_blocks` unchanged, `flush_commands_after_stale_delta=0`, `refreshed_generation!=old_cap_generation`, `dirty_blocks_after_refreshed_fsync=0` |
| **Failure mode** | stale FileCap fsync succeeds; stale fsync drains cache or issues FLUSH; generation does not advance after mutation invalidating old handle |
| **Gate** | **Hard close gate** — stale handles must never reach durability code |

### E3 — dirty fsync drains required cache state

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_issues_flush` |
| **Expected PASS-frame fields** | `event=PASS`, `neofs_fsync=issues_flush`, `dirty_blocks_before>0`, `dirty_blocks_after=0`, `drained_blocks_delta>0`, `cache_flushes_delta>=1`, `flush_commands_delta>=1`, `flush_completions_delta>=1`, `drain_scope=mount_wide` |
| **Failure mode** | fsync returns while dirty blocks remain; fsync does not advance drain counters; cache state is left dirty after success return |
| **Gate** | **Hard close gate** — core durability guarantee |

### E4 — dirty fsync issues and observes real VIRTIO_BLK_T_FLUSH

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_issues_flush` (same scenario, FLUSH counters) |
| **Expected PASS-frame fields** | `flush_commands_delta>=1`, `flush_completions_delta>=1` |
| **Failure mode** | dirty data reaches device but no FLUSH command is issued; FLUSH command issued but completion not observed before return |
| **Gate** | **Hard close gate** — FLUSH is the durability anchor for write-back caches |

### E5 — clean fsync does not issue spurious FLUSH

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_clean_no_extra_flush` |
| **Expected PASS-frame fields** | `event=PASS`, `neofs_fsync=clean_no_extra_flush`, `dirty_blocks_before=0`, `dirty_blocks_after=0`, `drained_blocks_delta=0`, `cache_flushes_delta=0`, `flush_commands_delta=0`, `flush_completions_delta=0` |
| **Failure mode** | clean fsync issues FLUSH command; clean fsync advances drain/flush counters when nothing was dirty |
| **Gate** | **Hard close gate** — clean fsync must be cheap and deterministic |

### E6 — fsync'd data survives crash/reboot without mount teardown

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_dirty_persists_after_crash` (write stage + readback stage) |
| **Expected PASS-frame fields** | Write: `stage=write`, `neofs_fsync_persist=prepared`, `dirty_blocks_before_fsync>0`, `dirty_blocks_after_fsync=0`, `drained_blocks_delta>0`, `cache_flushes_delta>=1`, `flush_commands_delta>=1`, `drain_scope=mount_wide`, `image=fresh`. Readback: `stage=readback`, `neofs_fsync_persist=ok`, `image=reused` |
| **Failure mode** | fsync'd data is lost after crash; file not found on reboot; data content mismatch on readback |
| **Gate** | **Hard close gate** — end-to-end durability proof |

### E7 — unfsync'd dirty data may be lost but must not corrupt old committed state

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_unfsynced_dirty_may_be_lost` (write stage + readback stage) |
| **Expected PASS-frame fields** | Write: `stage=write`, `neofs_unfsynced_dirty=prepared`, `marker_dirty_blocks_after_fsync=0`, `dirty_blocks_before_exit>0`, `writeback_hold=true`, `no_target_fsync=true`, `image=fresh`. Readback: `stage=readback`, `neofs_unfsynced_dirty=lost_without_corruption`, `dirty_target_absent=true`, `image=reused` |
| **Failure mode** | unfsync'd dirty file persists (should be lost); committed marker is corrupt/unreadable; old state is torn by partial writes |
| **Gate** | **Hard close gate** — crash safety boundary: loss is acceptable, corruption is not |

### E8 — fsync coordination with background write-back

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_coordinates_with_writeback_gc` |
| **Expected PASS-frame fields** | `event=PASS`, `neofs_fsync_gc=coordinated`, `dirty_blocks_before_fsync>0`, `dirty_blocks_after_fsync=0`, `dirty_blocks_after_gc>0`, `dirty_blocks_after_gc_fsync=0`, `dirty_blocks_after_release=0`, `drained_blocks_delta>0`, `gc_fsync_drained_blocks_delta>0`, `reclaimed_blocks>0`, `writeback_hold_during_fsync=true`, `drain_scope=mount_wide` |
| **Failure mode** | fsync races background worker into torn state; fsync returns while worker is mid-drain; GC sees dirty cache after fsync should have cleaned it |
| **Gate** | **Hard close gate** — write-back coordination prevents torn publication |

### E9 — fsync coordination with GC

| Field | Value |
|---|---|
| **Scenario** | `phase3_neofs_fsync_coordinates_with_writeback_gc` (GC sub-assertion) |
| **Expected PASS-frame fields** | `reclaimed_blocks>0`, `dirty_blocks_after_gc>0` (GC publishes metadata, leaving dirty state), `dirty_blocks_after_gc_fsync=0` (second fsync cleans GC metadata) |
| **Failure mode** | GC runs mark/sweep over dirty unpublished blocks; fsync and GC interleave into torn free-map or superblock state |
| **Gate** | **Hard close gate** — GC must see clean cache before mark/sweep; fsync must handle GC's metadata dirtiness |

### E10 — 3D write-back scenarios remain passing

| Field | Value |
|---|---|
| **Scenario** | Regression: `phase3_neofs_writeback_dirty_drain`, `phase3_neofs_writeback_worker_stop_join` |
| **Expected PASS-frame fields** | Same PASS frames as 3D verification artifact — no regression in dirty drain, worker lifecycle, or stop/join semantics |
| **Failure mode** | 3E fsync breaks background write-back; worker cannot stop and drain; mount teardown fails after 3E changes |
| **Gate** | **Hard close gate** — 3D substrate must remain intact |

### E11 — 3B virtio FLUSH/abandoned-I/O scenarios remain passing

| Field | Value |
|---|---|
| **Scenario** | Regression: `phase3_virtio_blk_flush_command_smoke`, `phase3_virtio_blk_abandoned_io_no_leak` |
| **Expected PASS-frame fields** | `VIRTIO_BLK_T_FLUSH=true`, `completions_delta=1`, `flush_commands_delta=1`, `retained_dma_buffers` stable, `live_before==live_after` for abandoned I/O |
| **Failure mode** | 3E fsync breaks FLUSH command path; DMA buffer lifetime is violated; abandoned I/O leaks under fsync load |
| **Gate** | **Hard close gate** — virtio FLUSH substrate must remain intact |

### E12 — aarch64 virtio barrier lane remains runtime evidence

| Field | Value |
|---|---|
| **Scenario** | Regression: `phase3_aarch64_virtio_barrier_lane` |
| **Expected PASS-frame fields** | `runtime=aarch64`, `qemu_machine=virt`, `transport=virtio_mmio`, `event_idx_negotiated=true`, `write_ok=true`, `read_ok=true`, `data_integrity=ok`, `compile_only=false`, `sentinel=runtime_qemu_aarch64` |
| **Failure mode** | aarch64 lane degrades to compile-only; virtio_mmio transport fails; data integrity check fails on aarch64 |
| **Gate** | **Hard close gate** — aarch64 must produce runtime QEMU evidence, not compile-only proof |

### E13 — full NEO_SMP=4 --no-meta sweep

| Field | Value |
|---|---|
| **Scenario** | `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta` |
| **Expected PASS-frame fields** | All scenarios exit 0; `docker wait` returns 0; final serial log contains aarch64 runtime barrier PASS |
| **Failure mode** | SMP-exposed race in fsync/drain/flush ordering; any scenario fails under concurrency stress; container exits non-zero |
| **Gate** | **Hard close gate** — required before 3E acceptance. SMP sweep exposes races that single-core tests miss |

## 4. Evidence Summary Table

| ID | Scenario | Gate | Detects |
|---|---|---|---|
| E1 | `phase3_neofs_fsync_noop_write_through` | Hard | Contract changed from no-op to durable drain+flush |
| E2 | `phase3_neofs_fsync_filecap_stale_regression` | Hard | Stale FileCap reaches durability code |
| E3 | `phase3_neofs_fsync_issues_flush` | Hard | Dirty fsync returns without draining cache |
| E4 | `phase3_neofs_fsync_issues_flush` (FLUSH counters) | Hard | FLUSH not issued/observed for dirty fsync |
| E5 | `phase3_neofs_fsync_clean_no_extra_flush` | Hard | Clean fsync issues spurious FLUSH |
| E6 | `phase3_neofs_fsync_dirty_persists_after_crash` | Hard | Fsync'd data lost after crash/reboot |
| E7 | `phase3_neofs_unfsynced_dirty_may_be_lost` | Hard | Unfsync'd dirty data corrupts old committed state |
| E8 | `phase3_neofs_fsync_coordinates_with_writeback_gc` | Hard | Fsync races write-back into torn publication |
| E9 | `phase3_neofs_fsync_coordinates_with_writeback_gc` (GC) | Hard | GC marks/sweeps dirty unpublished blocks |
| E10 | 3D write-back regression suite | Hard | 3D substrate broken by 3E changes |
| E11 | 3B virtio FLUSH regression suite | Hard | Virtio FLUSH/DMA substrate broken by 3E |
| E12 | `phase3_aarch64_virtio_barrier_lane` | Hard | aarch64 runtime evidence lost |
| E13 | Full SMP sweep | Hard | SMP-exposed races in fsync/drain/flush |

All 13 rows are hard close gates. There are no supporting-only rows because every
scenario tests a safety-critical property of the durability contract.

## 5. Close-Readiness Checklist

PM uses this checklist against
`Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`.

### 5A. Fsync Contract

- [ ] fsync validates `fd` as existing file handle capability (EBADF/EINVAL on bad fd)
- [ ] fsync rejects stale NeoFS FileCaps with ESTALE (-116) before any durability work
- [ ] fsync on non-Regular files returns TypeForbidden
- [ ] fsync on read-only mounts returns success without draining
- [ ] fsync drains dirty cache state for dirty NeoFS Regular files
- [ ] fsync issues and observes VIRTIO_BLK_T_FLUSH when drain reaches device
- [ ] fsync waits for all write-back work to complete before returning success
- [ ] fsync preserves VIRTIO_BLK_F_FLUSH negotiation, does not negotiate F_CONFIG_WCE
- [ ] fsync preserves 3C/3D CoW publication ordering: data → inode/root → free-map → superblock → FLUSH
- [ ] Clean-file fsync is cheap: no drain, no FLUSH, no counter advancement

### 5B. Durability Guarantees

- [ ] Crash after fsync: fsync'd data and required metadata survive reboot
- [ ] Crash before fsync: unfsync'd dirty data may be lost but old committed state is not corrupted
- [ ] fsync returns only after dirty cache drain AND FLUSH completion
- [ ] fsync does not weaken CoW/superblock publication ordering

### 5C. Coordination

- [ ] fsync serializes with background write-back worker via pause guard (foreground_writers)
- [ ] fsync cannot race GC into torn mark/sweep state
- [ ] GC still sees clean cache before mark/sweep after fsync drains
- [ ] Stale FileCaps cannot fsync or mutate durability state

### 5D. Regression Gates

- [ ] `phase3_virtio_blk_flush_command_smoke` — PASS
- [ ] `phase3_virtio_blk_abandoned_io_no_leak` — PASS
- [ ] `phase3_block_vfs_smoke` — PASS
- [ ] `phase3_neofs_writeback_dirty_drain` — PASS
- [ ] `phase3_neofs_writeback_worker_stop_join` — PASS
- [ ] `phase3_neofs_gc_waits_for_clean_cache` — PASS
- [ ] `phase3_neofs_gc_reboot_persistence` — PASS
- [ ] `phase3_neofs_gc_corrupt_free_map_rejected` — PASS
- [ ] `phase3_neofs_filecap_generation_stale` — PASS
- [ ] `phase3_aarch64_virtio_barrier_lane` — PASS (runtime, not compile-only)

### 5E. Build/Check Gates

- [ ] `rustfmt --edition 2021` on changed files — PASS
- [ ] `git diff --check` — PASS
- [ ] `cargo check -p xtask` — PASS
- [ ] `cargo check -p neofs-mkfs` — PASS
- [ ] `cargo k` — PASS
- [ ] `cargo k --no-default-features` — PASS

### 5F. SMP Sweep

- [ ] `NEO_SMP=4 --no-meta` full sweep — container exit 0, all scenarios PASS

### 5G. Scope Enforcement

REJECT closure if any of these land:

- [ ] No CAS/dedup code
- [ ] No AHCI/NVMe drivers
- [ ] No foreign filesystem support
- [ ] No rootfs pivot
- [ ] No mmap/exec implementation
- [ ] No broad POSIX semantics beyond what 3E requires
- [ ] No intent log (unless separately authorized by PM)
- [ ] No background GC expansion beyond existing 3C/3D behavior
- [ ] No BUILD-4 or BUILD-3F+ work
- [ ] No unauthorized NeoFS native work outside durable fsync

## 6. Known Design Decisions for Verification Artifact

The verification artifact must state these explicitly:

1. **Mount-wide drain scope**: `fsync_regular()` calls `cache.drain_all()`, which drains
   ALL dirty blocks on the mount, not just blocks belonging to the fsync'd file. This is
   conservative and correct because NeoFS does not yet attribute dirty cache state per file.
   The verification artifact must label `drain_scope=mount_wide` in all relevant PASS frames.

2. **Namespace serialization**: fsync acquires `self.entries.lock()` before draining, which
   serializes with namespace mutation (create/open-rw). This prevents the worker or GC from
   interleaving partial transaction drains during fsync.

3. **FLUSH conditional**: FLUSH is only issued when `flush_required` is true (set by any
   superblock dirty mark) AND all dirty blocks have been drained. Clean mounts with no
   dirty state produce no FLUSH traffic.

4. **Read-only mount fast path**: `fsync_regular()` returns success immediately on
   read-only mounts without touching the cache, since there can be no dirty state.

5. **No per-file precision**: The cache tracks blocks by `(block_number, role)`, not by
   owning inode or file. Future builds may narrow drain scope, but 3E is mount-wide.

## 7. Boundaries

This document does not:
- Produce a close report
- Authorize BUILD-3F or BUILD-4
- Edit kernel code
- Replace the verification artifact that Jarvis must produce after implementation
