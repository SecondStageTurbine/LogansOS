# Phase 3 BUILD-3 Sub-Commit 3C Verification - NeoFS Free-Space GC

**Date:** 2026-05-14
**Agent:** Jarvis
**Branch:** `codex/phase-3-build-3-3c-neofs-gc-free-space`
**Base:** `64693dede3c56bf8177f9adc94d81ef077044ee7`
**Implementation commit:** `a7eeb2135161778479961b323562580a6d42bfaa`
**Status:** PASS

## Scope

Implemented BUILD-3C only: NeoFS physical-block free-space metadata plus quiescent mark-sweep GC.

No write-back cache, durable `sys_fsync` contract change, background GC worker, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work landed.

## Changed Files

- `docs/specs/neofs-format.md`
- `kernel/Cargo.toml`
- `kernel/src/fs/mod.rs`
- `kernel/src/fs/neofs/format.rs`
- `kernel/src/fs/neofs/inode.rs`
- `kernel/src/fs/neofs/mod.rs`
- `kernel/src/fs/neofs/mount.rs`
- `kernel/src/test_harness.rs`
- `tools/neofs-mkfs/src/format.rs`
- `tools/neofs-mkfs/src/main.rs`
- `tools/xtask/src/main.rs`

## Free-Space Format

The superblock keeps BUILD-2 compatibility by treating an all-zero extension at bytes `88..480` as legacy bump-only state. BUILD-3C images use bytes `88..136` for:

- `free_map_start_block: u64`
- `free_map_block_count: u64`
- `free_map_hash: [u8; 32]`

The free map is a raw bitmap with one bit per physical filesystem block. Bit `1` means free/reusable; bit `0` means allocated, metadata, live, reserved, or virgin beyond `next_free_block`. The map length is `ceil(total_blocks / 32768)` filesystem blocks and is BLAKE3-covered by the selected superblock.

Mount validates the selected candidate's free-map hash and rejects any candidate whose map marks reachable metadata or live file blocks free. Legacy BUILD-2 all-zero-extension images remain mountable through the bump-only path.

## Mkfs and Tooling

`neofs-mkfs` now initializes deterministic BUILD-3C free-map metadata for normal images and reports `free_map_block` / `free_map_present`. It also adds `--variant corrupt-free-map` for the recovery-selection scenario.

`xtask neo-test` wires the six new 3C scenarios, two reboot stages, and the corrupt-map image variant.

## GC Algorithm

The GC path is explicit and quiescent. It locks the NeoFS entry table and metadata, then:

1. marks superblock-copy blocks, the selected free-map block range, the selected root inode, directory data blocks, child inode blocks, and reachable direct data blocks;
2. sweeps unreachable allocated blocks below `next_free_block` into a new bitmap;
3. allocates a new physical block/range for the free-map publication;
4. writes the new free-map contents;
5. writes all superblock copies with the new free-map reference and hash;
6. issues one block-device FLUSH if the mounted device supports FLUSH;
7. only then treats the in-memory transaction as committed.

Allocation on free-map images now consumes free bits below the high-water cursor first, then falls back to bump allocation. `next_free_block` remains the high-water compatibility cursor.

## Scenario Evidence

Command:

```text
docker compose run --rm neo sh -lc 'cargo neo-test phase3_neofs_gc_mark_live_set --no-meta && cargo neo-test phase3_neofs_gc_reclaims_orphan_after_overwrite --no-meta && cargo neo-test phase3_neofs_gc_reuses_reclaimed_block --no-meta && cargo neo-test phase3_neofs_gc_reboot_persistence --no-meta && cargo neo-test phase3_neofs_gc_crash_before_publish --no-meta && cargo neo-test phase3_neofs_gc_corrupt_free_map_rejected --no-meta'
```

Result: PASS.

PASS frames:

- `target/neo-test/phase3_neofs_gc_mark_live_set.serial.log`: `live_blocks=11 reclaimed_blocks=16381 free_blocks_before=0 free_blocks_after=16381 free_map_blocks=1 map_start_block=1`
- `target/neo-test/phase3_neofs_gc_reclaims_orphan_after_overwrite.serial.log`: `old_data_block=16396 new_data_block=16400 reclaimed_blocks=16390 free_blocks_before=0 free_blocks_after=16390`
- `target/neo-test/phase3_neofs_gc_reuses_reclaimed_block.serial.log`: `reclaimed_blocks=16390 free_blocks_after_gc=16390 reused_data_block=6 high_water_before_reuse=16404`
- `target/neo-test/phase3_neofs_gc_reboot_persistence.boot1.serial.log`: `stage=write reclaimed_blocks=16390 image=fresh`
- `target/neo-test/phase3_neofs_gc_reboot_persistence.boot2.serial.log`: `stage=readback free_blocks_before=16390 free_blocks_after=16381 image=reused`
- `target/neo-test/phase3_neofs_gc_crash_before_publish.boot1.serial.log`: `injection=free_map_before_superblock expected_recovery=published_root_old_map`
- `target/neo-test/phase3_neofs_gc_crash_before_publish.boot2.serial.log`: `unpublished_gc_ignored=true free_blocks=0 rejected_superblock_mask=0x00`
- `target/neo-test/phase3_neofs_gc_corrupt_free_map_rejected.serial.log`: `corrupt_candidate_rejected=true selected_superblock=0 rejected_superblock_mask=0x04 valid_superblocks=2 rejected_superblocks=1`

The reuse scenario proves reclaimed physical blocks can be allocated again without aliasing live file content: the later file reused block `6`, below `high_water_before_reuse=16404`, after GC made reclaimed space available.

## Fsync Evidence

`sys_fsync` remains the BUILD-2 documented no-op. The `phase3_neofs_fsync_noop_write_through` scenario passed with:

```text
generation_before=3 generation_after=3 flush_commands_before=2 flush_commands_after=2 no_flush_command=true no_intent_log=true
```

Serial log: `target/neo-test/phase3_neofs_fsync_noop_write_through.serial.log`.

## Regression Gates

PASS:

- `git diff --check`
- `cargo k`
- `cargo k --no-default-features`
- `cargo check -p xtask`
- `cargo check -p neofs-mkfs`
- `cargo run -p neofs-mkfs -- sanity --output-dir target\neofs-mkfs-sanity`
- targeted Docker/QEMU regression lane:
  - `phase3_virtio_blk_flush_command_smoke`
  - `phase3_virtio_blk_raw_rw_smoke`
  - `phase3_block_vfs_smoke`
  - `phase3_neofs_create_small_file`
  - `phase3_neofs_write_readback_same_boot`
  - `phase3_neofs_reboot_persistence_small_file`
  - `phase3_neofs_cow_write_after_write`
  - `phase3_neofs_superblock_rotation_recovery`
  - `phase3_neofs_filecap_generation_stale`
  - `phase3_aarch64_virtio_barrier_lane`
- full SMP sweep: `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta`

Full SMP sweep result: PASS, exit code 0.

## Staged Diff Gate

`git diff --cached --check` must be rerun after staging this artifact for the verification commit.
