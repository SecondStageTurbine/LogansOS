# Phase 3 BUILD-3 Sub-Commit 3D Verification

Date: 2026-05-14
Agent: Jarvis
Branch: `codex/phase-3-build-3-3d-neofs-writeback-cache`
Implementation commit: `b2a93f29a41652daa5a139a111cf5b891f5f7450`

## Scope

3D implements a mount-owned NeoFS write-back buffer cache at the 4 KiB FS-block layer and drains dirty cache state through a NeoFS-owned 3A `KthreadHandle` worker.

No durable per-file fsync contract, CAS/dedup, AHCI/NVMe, foreign filesystem, rootfs pivot, mmap/exec, broad POSIX work, background GC worker, or BUILD-3E work landed.

## Changed Files

- `kernel/Cargo.toml`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/drivers/block/virtio_blk.rs`
- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/cache.rs`
- `kernel/src/fs/neofs/inode.rs`
- `kernel/src/fs/neofs/mod.rs`
- `kernel/src/fs/neofs/mount.rs`
- `kernel/src/test_harness.rs`
- `tools/xtask/src/main.rs`

## Architecture Summary

`kernel/src/fs/neofs/cache.rs` adds `NeoFsBlockCache`, owned by `NeoFsState`. The cache stores 4 KiB NeoFS blocks by physical FS block number, tracks clean/dirty state, records hit/miss counters, and tags dirty writes by role: data, inode, root directory, free map, and superblock.

Reads and same-boot lookup paths now consult cached blocks, while mount validation and GC live-set validation continue to use raw disk reads where the existing recovery model requires on-disk truth. Write paths mark cache entries dirty instead of synchronously writing every NeoFS block to disk.

## Write-Back Worker Lifecycle

Writable NeoFS mounts spawn a mount-owned `neofs-writeback` kthread using the 3A scheduler-owned `KthreadHandle` substrate. The worker drains bounded dirty batches and yields between passes. Mount teardown/test stop paths request stop, drain to a clean checkpoint, join the worker, and only then allow mount-owned cache state to be dropped.

Foreground mutating transactions use a cache pause guard so the worker does not interleave partial transaction drains with create/write/GC publication. `drain_all()` also pauses worker drains while the foreground drain completes.

## Dirty Ordering And FLUSH

Dirty write-back order is:

1. data blocks
2. inode blocks
3. root-directory blocks
4. free-map blocks
5. superblock copies

After dirty blocks drain, the cache issues one block-device FLUSH when dirty data reached the device and FLUSH is supported. This preserves the 3C CoW/superblock publication rule: write-back never publishes a superblock before the required data and metadata blocks have been submitted.

The full SMP sweep exposed a transient 3B flush evidence race: the virtio completion worker woke the issuer before dropping the completed flush rendezvous, so the issuer could briefly observe the two completed flush DMA buffers as live. `kernel/src/drivers/block/virtio_blk.rs` now copies completion metadata, drops the completed rendezvous/DMA ownership, and only then wakes the blocked thread. DMA buffers are still retained through device completion; they are released before issuer wake.

## Fsync Boundary

`sys_fsync` remains the BUILD-2/3C no-op. It validates the file handle and returns success, but does not drain the 3D cache, does not issue FLUSH, and does not mutate NeoFS generation.

PASS evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_noop_write_through neofs_fsync=noop path=/neofs/fsync-2d.txt bytes=62 fsync_rc=0 generation_before=3 generation_after=3 write_back_cache=true fsync_drained_cache=false dirty_blocks_before=10 dirty_blocks_after_fsync=10 dirty_blocks_after_drain=0 cache_flushes_before=0 cache_flushes_after_drain=1 flush_commands_before=0 flush_commands_after=0 no_flush_command=true no_intent_log=true selected_superblock=0 generation=1
```

## New 3D Runtime Evidence

Focused 3D scenario command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_cache_hit_miss_smoke phase3_neofs_writeback_dirty_drain phase3_neofs_writeback_worker_stop_join phase3_neofs_writeback_reboot_after_drain phase3_neofs_writeback_crash_before_drain phase3_neofs_gc_waits_for_clean_cache phase3_neofs_fsync_noop_write_through
```

Result: PASS.

PASS frames from the final SMP sweep serial logs:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_cache_hit_miss_smoke neofs_cache=hit_miss path=/neofs/hello.txt misses_delta_first=2 hits_delta_second=6 cached_blocks=2 dirty_blocks=0 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_dirty_drain neofs_writeback=dirty_drain path=/neofs/wb-drain.txt dirty_blocks_before=10 dirty_blocks_after=0 drained_blocks_delta=10 cache_flushes_delta=1 data_dirty_marks=1 inode_dirty_marks=2 root_dirty_marks=4 superblock_dirty_marks=6 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_worker_stop_join neofs_writeback=worker_stop_join path=/neofs/wb-stop.txt dirty_blocks_before_stop=10 dirty_blocks_after_stop=0 stopped=true joined=true exit_success=true cache_flushes=1 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_reboot_after_drain stage=write neofs_writeback_reboot=prepared path=/neofs/wb-persist.txt bytes=57 dirty_blocks_after_drain=0 cache_flushes=1 selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_reboot_after_drain stage=readback neofs_writeback_reboot=ok path=/neofs/wb-persist.txt bytes=57 selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_crash_before_drain stage=write neofs_writeback_crash=prepared marker=/neofs/wb-crash-marker.txt target=/neofs/wb-crash-dirty.txt marker_dirty_blocks_after_drain=0 dirty_blocks_before_exit=10 writeback_hold=true selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_writeback_crash_before_drain stage=readback neofs_writeback_crash=ok marker=/neofs/wb-crash-marker.txt target=/neofs/wb-crash-dirty.txt dirty_target_absent=true selected_superblock=0 generation=3 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_gc_waits_for_clean_cache neofs_gc_clean_cache=ok path=/neofs/wb-gc.txt dirty_blocks_before_gc=14 forced_drain_blocks_delta=14 reclaimed_blocks=16390 dirty_blocks_after_gc=4 dirty_blocks_after_release=0 cache_flushes=2 selected_superblock=0 generation=1
```

GC evidence: `phase3_neofs_gc_waits_for_clean_cache` proves dirty cache state exists before GC, GC forces a drain before mark/sweep, and dirty state is released only after write-back hold is released. Existing 3C GC reboot, crash-before-publish, corrupt free-map rejection, reclaimed-block reuse, and orphan reclamation scenarios also passed after the 3D cache insertion.

## Regression Evidence

Build/check gates:

```text
rustfmt --edition 2021 kernel/src/fs/neofs/cache.rs kernel/src/fs/neofs/inode.rs kernel/src/fs/neofs/mod.rs kernel/src/fs/neofs/mount.rs kernel/src/fs/mod.rs kernel/src/arch/x86_64/syscall.rs kernel/src/drivers/block/virtio_blk.rs kernel/src/test_harness.rs tools/xtask/src/main.rs
git diff --check
cargo check -p xtask
cargo check -p neofs-mkfs
cargo k
cargo k --no-default-features
```

Result: PASS. Existing warning profile only.

Targeted regression command:

```text
docker compose run --rm neo cargo neo-test phase3_virtio_blk_flush_command_smoke phase3_virtio_blk_abandoned_io_no_leak phase3_block_vfs_smoke phase3_neofs_gc_reuses_reclaimed_block phase3_neofs_gc_reboot_persistence phase3_neofs_gc_corrupt_free_map_rejected phase3_neofs_filecap_generation_stale phase3_aarch64_virtio_barrier_lane
```

Result: PASS.

Selected PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_virtio_blk_flush_command_smoke bdf=0x000300 request_type=4 VIRTIO_BLK_T_FLUSH=true completions_delta=1 flush_commands_delta=1 flush_completions_delta=1 wake_result=1 retained_dma_buffers=2 has_data_buffer=false readback=ok descriptor_publish_barriers_delta=1 notification_suppression_barriers_delta=1 live_before=4 live_after=4 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
NEO_TEST/1 event=PASS name=phase3_virtio_blk_abandoned_io_no_leak bdf=0x000300 retained_dma=3 live_before=3 live_after=3 late_wake=2 completions_delta=1 F_FLUSH_offered=true F_FLUSH_negotiated=true F_CONFIG_WCE_negotiated=false
NEO_TEST/1 event=PASS name=phase3_neofs_gc_reboot_persistence stage=write neofs_gc_reboot=prepared path=/neofs/gc-persist.txt reclaimed_blocks=16390 dirty_blocks_after_drain=0 cache_flushes=2 selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_gc_reboot_persistence stage=readback neofs_gc_reboot=ok path=/neofs/gc-persist.txt reuse_path=/neofs/gc-persist-reuse.txt free_blocks_before=16390 free_blocks_after=16381 dirty_blocks_after_drain=0 cache_flushes=1 selected_superblock=0 generation=5 image=reused
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_aarch64_virtio_barrier_lane runtime=aarch64 qemu_machine=virt transport=virtio_mmio event_idx_negotiated=true feature_low=0x30006e54 feature_high=0x00000101 sector=8 block_size=512 write_ok=true read_ok=true data_integrity=ok publish_descriptors=2 before_notification_check=2 notification_decisions=2 notifications_sent=2 compile_only=false sentinel=runtime_qemu_aarch64
```

Full SMP sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: PASS. The local command wrapper reached its 3600s timeout while the Docker `--rm` container continued running; `docker wait altimitneo-neo-run-8526985658e5` returned `0`. The final fresh serial timestamp was `phase3_aarch64_virtio_barrier_lane.serial.log` at 2026-05-15 02:48:22 with the runtime aarch64 PASS frame above.

Final staged hygiene before implementation commit:

```text
git diff --cached --check
```

Result: PASS.
