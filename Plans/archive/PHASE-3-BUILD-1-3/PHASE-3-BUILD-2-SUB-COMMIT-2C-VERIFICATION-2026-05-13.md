# Phase 3 BUILD-2 Sub-Commit 2C Verification

Date: 2026-05-13
Owner: Jarvis
Branch: `codex/phase-3-build-2-2c-neofs-create-write`
Base: `309f28e` (`kernel/fs: mount NeoFS images read-only`)
Dispatch: PM chat dispatch for `Plans/DISPATCH-2026-05-13-phase-3-build-2-2c-neofs-create-write-jarvis.md`; that file was not present in the local working tree at artifact creation.

## Verdict

`PASS`

BUILD-2 sub-commit 2C implements NeoFS create/write/readback for small `Regular` files on the existing 2A/2B format and the BUILD-2 bump allocator model.

No 2D crash/recovery hardening, GC/free-space manager, truncate, unlink, rename, `Model` creation, `Directory` creation, or BUILD-3+ work is included.

## Implementation Summary

- Extended the existing `sys_open(path, flags, rights)` ABI with `OPEN_CREATE_REGULAR = 1 << 0`; argument order and syscall number are unchanged.
- Added `libneo::OPEN_CREATE_REGULAR` while preserving `OPEN_ROOT_TEMPORARY = 0`.
- Mounted NeoFS read-write for 2C scenarios while retaining the 2B read-only mount behavior.
- Added on-disk NeoFS writers for superblocks, inodes, and directory entries using the 2A layout.
- Added a mount-owned NeoFS state object with a monotonic bump allocator backed by `next_free_block` and `next_generation`.
- Implemented narrow `/neofs/<name>` regular-file creation only; nested paths, `.`, `..`, overlong names, existing names, directory creation, and model creation are rejected.
- Implemented regular-file writes up to the eight-direct-block limit and persisted inode content hashes, root directory content hash, root inode hash, and superblock copies.
- Kept `FileRights` as `u32`; newly created regular files carry read/write availability, and read-only mounts mask regular files back to read-only.

## Targeted Runtime Scenarios

Runtime verification was run in the Docker dev container with real QEMU x86_64.

### Create Small Regular File

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_create_small_file
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_create_small_file neofs_create=ok create_flag=OPEN_CREATE_REGULAR path=/neofs/create-2c.txt bytes=40 close_reopen_read=ok selected_superblock=0 generation=1 no_directory_create=true no_model_create=true
```

### Eight Direct Blocks Same-Boot Readback

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_write_readback_same_boot
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_write_readback_same_boot neofs_write_readback=ok path=/neofs/direct8.bin bytes=32768 direct_blocks=8 close_reopen_read=ok selected_superblock=0 generation=1 runtime_mutation=true
```

### Bump Allocator Reuse Guard

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_allocator_reuse_guard
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_allocator_reuse_guard neofs_allocator_reuse_guard=ok first_path=/neofs/alloc-a.bin second_path=/neofs/alloc-b.bin alias_check=distinct_content bump_allocator=true selected_superblock=0 generation=1
```

### Two-Boot Persistence

Command:

```text
docker compose run --rm neo cargo neo-test phase3_neofs_reboot_persistence_small_file
```

Serial evidence:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_reboot_persistence_small_file stage=write neofs_reboot_persistence=ok path=/neofs/persist-2c.txt bytes=50 selected_superblock=0 generation=1 image=fresh
NEO_TEST/1 event=PASS name=phase3_neofs_reboot_persistence_small_file stage=readback neofs_reboot_persistence=ok path=/neofs/persist-2c.txt bytes=50 selected_superblock=0 generation=4 image=reused
```

## Regression Evidence

The 2B mount scenarios still pass with 2C present:

```text
NEO_TEST/1 event=PASS name=phase3_neofs_mount_readonly_root neofs_mount=ok selected_superblock=0 generation=1 valid_superblocks=3 rejected_superblocks=0 root_kind=Directory hello_read=ok model_kind=Model readonly_write_denied=InsufficientRights runtime_mutation=false
NEO_TEST/1 event=PASS name=phase3_neofs_mount_selects_highest_generation neofs_mount=ok selected_superblock=2 generation=3 highest_generation_selected=true valid_superblocks=3 rejected_superblocks=0 runtime_mutation=false
NEO_TEST/1 event=PASS name=phase3_neofs_mount_rejects_bad_superblock neofs_mount=ok selected_superblock=1 generation=1 corrupt_primary_rejected=true rejected_superblock_mask=0x01 valid_superblocks=2 rejected_superblocks=1 runtime_mutation=false
```

The BUILD-1 storage/VFS regressions still emit serial PASS with flush/cache policy intact:

```text
NEO_STDOUT_COMPAT/1 namespace=serial fd=1 source=vfs-block-smoke
NEO_TEST/1 event=PASS name=phase3_block_vfs_smoke bdf=0x000300 block_vfs_same_boot=true raw_rw=ok vfs_syscalls=ok stdout_namespace=fd1_serial lba=3 block_size=512 completions_delta=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
NEO_TEST/1 event=PASS name=phase3_virtio_blk_raw_rw_smoke bdf=0x000300 block_size=512 lba=1 completions=2 descriptor_publish_barriers=2 notification_suppression_barriers=2 F_FLUSH_offered=true F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false
```

Note: the host wrapper for `phase3_neofs_mount_selects_highest_generation` and `phase3_block_vfs_smoke` reached the outer command timeout after QEMU had already emitted the PASS line. No test container remained running afterward.

## Build And Tooling Evidence

Commands completed successfully unless noted:

```text
cargo check -p xtask
cargo check -p neofs-mkfs
cargo k
cargo k --no-default-features
rustfmt --check kernel\src\fs\mod.rs kernel\src\fs\neofs\mod.rs kernel\src\fs\neofs\format.rs kernel\src\fs\neofs\inode.rs kernel\src\fs\neofs\mount.rs tools\xtask\src\main.rs
docker compose run --rm neo cargo neo-test phase3_neofs_create_small_file
docker compose run --rm neo cargo neo-test phase3_neofs_write_readback_same_boot
docker compose run --rm neo cargo neo-test phase3_neofs_allocator_reuse_guard
docker compose run --rm neo cargo neo-test phase3_neofs_reboot_persistence_small_file
docker compose run --rm neo cargo neo-test phase3_neofs_mount_readonly_root
docker compose run --rm neo cargo neo-test phase3_neofs_mount_rejects_bad_superblock
docker compose run --rm neo cargo neo-test phase3_virtio_blk_raw_rw_smoke
```

`cargo fmt --check` over the full workspace was not used as a closure gate because the repository has broad pre-existing rustfmt drift in unrelated files. The scoped check above covers the NeoFS/VFS/xtask files changed for 2C that do not already carry shared-file drift.

## Scope Guard

- No NeoFS runtime mutation outside regular file create/write under `/neofs/<name>`.
- No runtime mkfs.
- No free-space reuse beyond monotonic bump allocation.
- No crash/recovery or copy-on-write hardening beyond synchronous block writes and persisted checksums.
- No truncate, unlink, rename, symlink, hard link, readdir syscall, mmap/exec, mount namespace, `Directory` creation, or `Model` creation.
- No BUILD-2 2D/2E and no BUILD-3+ work.
