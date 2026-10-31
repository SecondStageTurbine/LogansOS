# Phase 3 BUILD-4D Close Evidence - ext4 Read-Only Foreign Filesystem

Date: 2026-05-20
Owner: Jarvis
Implementation commit: aca3fb19256c9ccfde6f19e01aa957c141baec63

## Scope Delivered

BUILD-4D adds a read-only ext4 foreign filesystem driver for `device2` with `fs_type = 4`.

The implementation composes through the existing `MountedFs` trait. No `MountedFs` trait changes were required, so the BUILD-4B/BUILD-4C abstraction line held for a third filesystem with more complex on-disk structures than FAT32.

Implementation files:

```text
aca3fb1 build-4D: add ext4 read-only foreign filesystem driver
 kernel/Cargo.toml          |  11 +
 kernel/src/fs/ext4.rs      | 696 +++++++++++++++++++++++++++++++++++++++++++++
 kernel/src/fs/mod.rs       |  15 +
 kernel/src/test_harness.rs | 470 ++++++++++++++++++++++++++++--
 tools/xtask/src/main.rs    | 285 ++++++++++++++++++-
 userland/libneo/src/lib.rs |   1 +
 6 files changed, 1457 insertions(+), 21 deletions(-)
```

## Container Tool Verification

Canonical check result, with the command status echoed:

```text
$ docker compose run --rm neo bash -c 'which mke2fs e2cp debugfs dumpe2fs; status=$?; echo canonical_status=$status; which mke2fs debugfs dumpe2fs'
/usr/sbin/mke2fs
/usr/sbin/debugfs
/usr/sbin/dumpe2fs
canonical_status=1
/usr/sbin/mke2fs
/usr/sbin/debugfs
/usr/sbin/dumpe2fs
```

`mke2fs`, `debugfs`, and `dumpe2fs` are present inside the `neo` service container. `e2cp` is absent. Per the dispatch, this was not a stop condition because `debugfs` was allowed as the fallback population path. `xtask` now creates the ext4 fixture with `mke2fs`, writes content/xattrs with `debugfs`, validates the fragmented extent file with `debugfs filefrag`, validates the `.neom` xattr with `debugfs ea_list`, and runs `dumpe2fs -h`.

## Design Notes

- `kernel/src/fs/ext4.rs` is read-only and supports regular files and directories only.
- Extents are required. Inodes without `EXT4_EXTENTS_FL` are rejected as unsupported; no legacy block-mapping path was added.
- Extent trees are walked recursively with a depth limit. The BUILD-4D large-file scenario validated a fragmented fixture with 50 extents.
- Directory traversal is linear. No HTree index parser was added.
- Symlinks and special files are skipped from imported directory entries.
- xattrs are read/probed where present, but ignored for inode type import. The load-bearing `.neom` file imports as `Regular`, even with `user.neoplutarch.type=Model`.
- Mount flags are unchanged from BUILD-4C. `MOUNT_FLAG_EXEC_ALLOWED` remains bit 0; default foreign mounts are noexec.
- Effective rights still use inode rights AND mount rights. The ext4 mount mask denies WRITE always and denies EXECUTE unless `MOUNT_FLAG_EXEC_ALLOWED` is set.

Explicit anti-scope confirmations:

- No ext4 write code landed.
- No ext2/ext3 code landed.
- No legacy block-mapping code landed.
- No HTree index parsing landed.
- No new mount flag bits landed.
- No symlink traversal landed.
- No Article 10.3 type import from extension, filename, or xattr landed.

## Lock Hygiene

No new lock sites were introduced by BUILD-4D. `kernel/src/fs/ext4.rs` has no mutex, spinlock, IRQ-safe lock, or IF-discipline site. Existing VFS mount-table locking in `kernel/src/fs/mod.rs` was reused unchanged.

## Build Gates

```text
$ docker compose run --rm neo cargo fmt -- --check
exit=0

$ docker compose run --rm neo bash -lc "set -euo pipefail; cargo k > /tmp/build4d-cargo-k.log 2>&1; tail -n 8 /tmp/build4d-cargo-k.log"
warning: `neoplutarch-kernel` (bin "kernel") generated 52 warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.49s
exit=0

$ docker compose run --rm neo bash -lc "set -euo pipefail; cargo k --no-default-features > /tmp/build4d-cargo-k-nodefault.log 2>&1; tail -n 8 /tmp/build4d-cargo-k-nodefault.log"
warning: `neoplutarch-kernel` (bin "kernel") generated 52 warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.98s
exit=0

$ docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
exit=0
```

## Required BUILD-4D Scenarios

```text
NEO_TEST/1 event=PASS name=phase3_ext4_mount_readonly mount=ext4 fs_type=4 source=device2 target=ext4 readonly=true
NEO_TEST/1 event=PASS name=phase3_ext4_readdir_root entries=7 hello=Regular readme=Regular subdir=Directory large=Regular neom=Regular symlink_skipped=true
NEO_TEST/1 event=PASS name=phase3_ext4_open_read_regular_file path=/ext4/hello.txt bytes=17 content_match=true
NEO_TEST/1 event=PASS name=phase3_ext4_subdir_walk path=/ext4/subdir/nested.txt content_match=true
NEO_TEST/1 event=PASS name=phase3_ext4_extent_chain_read path=/ext4/large.bin bytes=1048576 extents=50 pattern_match=true
NEO_TEST/1 event=PASS name=phase3_ext4_write_is_rejected write_rejected=open errno=EACCES readonly=true
NEO_TEST/1 event=PASS name=phase3_ext4_exec_blocked_by_default default_noexec=EACCES exec_allowed_flag=false
NEO_TEST/1 event=PASS name=phase3_ext4_exec_with_opt_in exec_open=ok exec_policy=ok exec_allowed_flag=true actual_exec=false
NEO_TEST/1 event=PASS name=phase3_ext4_type_import_is_regular neom_kind=Regular xattr_type_ignored=true model_imports=0 symlink_imports=0 entries=7
```

## Targeted Anti-Regression Scenarios

```text
NEO_TEST/1 event=PASS name=phase3_mount_neofs_device0_readonly mount=neofs fs_type=1 source=device0 target=neofs rc=0
NEO_TEST/1 event=PASS name=phase3_mount_requires_mount_right rights=lookup_readdir_only sys_mount=EACCES errno=-13
NEO_TEST/1 event=PASS name=phase3_umount_busy_is_ebusy live_handles_while_open=1 umount_busy=EBUSY busy_errno=-16 umount_clean=0
NEO_TEST/1 event=PASS name=phase3_fat32_mount_readonly mount=fat32 fs_type=3 source=device1 target=fat32 readonly=true
NEO_TEST/1 event=PASS name=phase3_fat32_readdir_root entries=5 hello=Regular readme=Regular subdir=Directory lfn=Regular neom=Regular
NEO_TEST/1 event=PASS name=phase3_fat32_open_read_regular_file path=/fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_fat32_lfn_lookup lfn_lookup=case_preserving uppercase_lookup=case_insensitive
NEO_TEST/1 event=PASS name=phase3_fat32_write_is_rejected write_rejected=open errno=EACCES readonly=true
NEO_TEST/1 event=PASS name=phase3_fat32_exec_blocked_by_default default_noexec=EACCES exec_allowed_flag=false
NEO_TEST/1 event=PASS name=phase3_fat32_type_import_is_regular neom_kind=Regular model_imports=0 symlink_imports=0 entries=5
NEO_TEST/1 event=PASS name=phase3_neofs_readdir_root neofs_readdir=root entries=3 order=mkfs cursor=3 eof=0 subdir_entries=0 subdir_eof=0 selected_superblock=0 generation=1 stale_generation_anchor=current_reachable_inode
NEO_TEST/1 event=PASS name=phase3_neofs_filecap_generation_stale path=/neofs/stale-2e.txt stale_cap=ESTALE stale_errno=-116 old_cap_generation=7 refreshed_generation=9 current_inode_generation=11 handle_a_continued=true reachable_generation_checked=true selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
NEO_TEST/1 event=PASS name=phase3_sched_wake_token_abandon token_cpu=0 token_slot=0 generation=1 late_wake=Gone abandon=zombified_and_reaped
NEO_TEST/1 event=PASS name=phase3_vfs_directory_read_is_eisdir vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=ipc_heap_kill_smoke leak_audit=clean exit_code=0
NEO_TEST/1 event=PASS name=cap_revoke_local cap_scenario=ok exit_code=0
```

## Deviations And Disposition

`e2cp` was not present in the canonical container check. The dispatch explicitly allowed `debugfs` as the fallback if `e2cp` was missing, so the fixture generator uses `debugfs -w -f` for file population and xattr setup. No Dockerfile amendment is needed because all load-bearing tools (`mke2fs`, `debugfs`, `dumpe2fs`) are present.

An early ext4 large-file read path reread the whole file per VFS read call and timed out in QEMU. The final implementation uses range reads over extent overlaps, and `phase3_ext4_extent_chain_read` now passes with a 1 MiB, 50-extent fixture.
