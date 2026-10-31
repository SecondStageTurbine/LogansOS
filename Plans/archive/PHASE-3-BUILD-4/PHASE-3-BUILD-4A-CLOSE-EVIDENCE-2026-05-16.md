# Phase 3 BUILD-4A Close Evidence

Date: 2026-05-16
Scope: Directory FileCaps + `SYS_READDIR = 16` over testfs and NeoFS
Substrate: `bb47d0a` main with BUILD-4A WIP applied

## Status

BUILD-4A final close gate passed.

Final gate command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Recorded wrapper status:

```text
start 2026-05-16T14:45:53.7387534-04:00
end 2026-05-16T15:32:13.7053239-04:00
exit 0
```

Evidence log: `target/verify-phase3-build4a-final-smp-no-meta.log`

## New Scenario Evidence

All BUILD-4A new scenarios emitted literal serial PASS frames:

```text
NEO_TEST/1 event=PASS name=phase3_vfs_readdir_testfs_root vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=phase3_vfs_readdir_requires_right vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=phase3_vfs_readdir_regular_is_enotdir vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=phase3_vfs_directory_read_is_eisdir vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=phase3_neofs_readdir_root neofs_readdir=root entries=3 order=mkfs cursor=3 eof=0 subdir_entries=0 subdir_eof=0 selected_superblock=0 generation=1 stale_generation_anchor=current_reachable_inode
NEO_TEST/1 event=PASS name=phase3_neofs_readdir_stale_generation neofs_readdir_stale=ok path=/neofs/readdir-stale-4a.txt errno=-116 captured_generation=1 current_generation=6 selected_superblock=0 mount_generation=1 anchor=current_reachable_inode_generation
NEO_TEST/1 event=PASS name=phase3_readdir_cursor_eof_stable vfs_user_scenario=ok exit_code=0
NEO_TEST/1 event=PASS name=phase3_readdir_buffer_too_small vfs_user_scenario=ok exit_code=0
```

The final `NEO_SMP=4 --no-meta` wrapper also recorded these xtask PASS lines:

```text
[xtask] neo-test: `phase3_vfs_readdir_testfs_root` PASS
[xtask] neo-test: `phase3_vfs_readdir_requires_right` PASS
[xtask] neo-test: `phase3_vfs_readdir_regular_is_enotdir` PASS
[xtask] neo-test: `phase3_vfs_directory_read_is_eisdir` PASS
[xtask] neo-test: `phase3_neofs_readdir_root` PASS
[xtask] neo-test: `phase3_neofs_readdir_stale_generation` PASS
[xtask] neo-test: `phase3_readdir_cursor_eof_stable` PASS
[xtask] neo-test: `phase3_readdir_buffer_too_small` PASS
```

`phase3_readdir_buffer_too_small` explicitly covers the `EINVAL` path for an undersized `NeoDirentV1` output buffer.

## Anti-Regression Evidence

Required anti-regressions passed in the final `NEO_SMP=4 --no-meta` gate:

```text
[xtask] neo-test: `phase3_vfs_testfs_open_read_close` PASS
[xtask] neo-test: `phase3_vfs_testfs_write_readback` PASS
[xtask] neo-test: `phase3_vfs_type_vs_rights_errors` PASS
[xtask] neo-test: `phase3_vfs_directory_read_is_eisdir` PASS
[xtask] neo-test: `phase3_block_vfs_smoke` PASS
[xtask] neo-test: `phase3_neofs_mount_readonly_root` PASS
[xtask] neo-test: `phase3_neofs_filecap_generation_stale` PASS
[xtask] neo-test: `phase3_neofs_fsync_dirty_persists_after_crash` PASS
```

`phase3_vfs_directory_read_is_eisdir` is intentionally listed in both sections: BUILD-4A adds directory handles, and existing `sys_read` on a directory must remain `EISDIR`.

## Code-Review Checkpoint

`sys_readdir_impl` resolves the FileCap with no premature rights gating:

```text
kernel/src/arch/x86_64/syscall.rs:750:     let file = match file_lookup(fd, FileRights::empty()) {
```

`OpenFile::readdir_one()` remains the sole type-first, rights-second arbiter:

```text
kernel/src/fs/mod.rs:212:     pub fn readdir_one(&self, cursor: u64) -> Result<Option<VfsDirEntry>, VfsError> {
kernel/src/fs/mod.rs:213:         self.validate_current()?;
kernel/src/fs/mod.rs:214:         if self.inode.kind() != InodeKind::Directory {
kernel/src/fs/mod.rs:215:             return Err(VfsError::NotDirectory);
kernel/src/fs/mod.rs:217:         if !self.rights.contains(FileRights::READDIR) {
kernel/src/fs/mod.rs:218:             return Err(VfsError::InsufficientRights);
```

This preserves the required behavior: a regular file without `READDIR` returns `ENOTDIR`, not `EACCES`.

## NeoDirentV1 ABI Note

`NeoDirentV1.record_len` is exactly `16 + name_len`.

```text
kernel/src/arch/x86_64/syscall.rs:822:     let record_len = 16usize.checked_add(name.len()).ok_or(VfsError::Invalid)?;
kernel/src/arch/x86_64/syscall.rs:826:     out[0..4].copy_from_slice(&(record_len as u32).to_le_bytes());
kernel/src/arch/x86_64/syscall.rs:830:     out[8..16].copy_from_slice(&entry.next_cursor.to_le_bytes());
kernel/src/arch/x86_64/syscall.rs:831:     out[16..record_len].copy_from_slice(name);
```

`record_len` is not an extensibility field and not a skip-forward field in BUILD-4A. The encoded `next_cursor` equals the cursor value written back to userspace.

## Stale-Cap Fsync Isolation

Cerebro triage consumed by PM: `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`.

PM disposition: `phase3_neofs_fsync_filecap_stale_regression` is not a demonstrated BUILD-4A regression. It is not reproducible standalone on clean main or BUILD-4A WIP and is classified as likely full-suite/order/resource interaction or broader CI/SMP instability.

Isolation result for `phase3_neofs_fsync_filecap_stale_regression`:

| Substrate | SMP | Runs | Result |
|---|---:|---:|---|
| clean main | `NEO_SMP=4` | 20 | `20 PASS / 0 TIMEOUT / 0 FAIL / 0 OTHER` |
| BUILD-4A WIP | `NEO_SMP=4` | 20 | `20 PASS / 0 TIMEOUT / 0 FAIL / 0 OTHER` |
| clean main | `NEO_SMP=1` | 10 | `10 PASS / 0 TIMEOUT / 0 FAIL / 0 OTHER` |
| BUILD-4A WIP | `NEO_SMP=1` | 10 | `10 PASS / 0 TIMEOUT / 0 FAIL / 0 OTHER` |

Isolation logs: `target/isolation-build4a/{main-smp4,wip-smp4,main-smp1,wip-smp1}`.

Retry-pass evidence remains a residual substrate caveat:

```text
main-smp4 run-06: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
main-smp4 run-09: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
main-smp4 run-16: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
main-smp4 run-18: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
wip-smp4 run-02: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
wip-smp4 run-11: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
wip-smp1 run-01: phase3_neofs_fsync_filecap_stale_regression attempt 1/2 timed out; PASS after retry attempt 2/2
```

The final full-suite close gate also included retry-pass evidence:

```text
[xtask] neo-test: `phase3_build1_reboot_persistence_smoke` two-boot attempt 1/2 failed: QEMU timed out after 180s
[xtask] neo-test: `phase3_build1_reboot_persistence_smoke` two-boot PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_noop_write_through` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_noop_write_through` PASS after retry attempt 2/2
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` attempt 1/2 failed: QEMU timed out after 180s for
[xtask] neo-test: `phase3_neofs_fsync_filecap_stale_regression` PASS after retry attempt 2/2
```

## Scope Check

No `SYS_MOUNT`, `SYS_UMOUNT`, FAT32, ext4, mount-table refactor, symlink traversal, broad POSIX surface, BUILD-3F, or Fix 5 code landed in BUILD-4A.
