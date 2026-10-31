# Phase 3 BUILD-4C Close Evidence

Date: 2026-05-20
Agent: Jarvis
Status: CLOSED - implementation commit `a627d670e81a277b1c5c80b3af22624225767ff2`
Authority: `Plans/DISPATCH-2026-05-20-phase-3-build-4C-fat32-readonly-jarvis.md`

## 1. Stop-condition resolution

PM clarified that `mtools` availability is environmental, not host-shell state. Canonical check passed inside the `neo` service runtime:

```text
docker compose run --rm neo bash -c 'which mformat mcopy mmd mdir mtype'
/usr/bin/mformat
/usr/bin/mcopy
/usr/bin/mmd
/usr/bin/mdir
/usr/bin/mtype
```

No toolchain stop condition remained.

## 2. Commit and diff stat

Implementation commit:

```text
a627d670e81a277b1c5c80b3af22624225767ff2 build-4C: add FAT32 read-only foreign filesystem driver
```

Diff stat:

```text
 kernel/Cargo.toml                      |  11 +
 kernel/src/arch/x86_64/syscall.rs      |   4 +-
 kernel/src/drivers/block/virtio_blk.rs |  18 +-
 kernel/src/fs/fat32.rs                 | 590 +++++++++++++++++++++++++++++++++
 kernel/src/fs/mod.rs                   | 114 ++++---
 kernel/src/test_harness.rs             | 464 +++++++++++++++++++++++---
 tools/xtask/src/main.rs                | 223 +++++++++++--
 userland/libneo/src/lib.rs             |   7 +-
 8 files changed, 1304 insertions(+), 127 deletions(-)
 create mode 100644 kernel/src/fs/fat32.rs
```

## 3. Implementation summary

`kernel/src/fs/fat32.rs` implements a read-only FAT32 `MountedFs` with VFAT LFN read support. It parses the BPB/EBPB, validates FAT32 layout, reads FAT cluster chains, walks directories, resolves short and long names case-insensitively, and presents FAT32 entries only as `Directory` or `Regular`.

FAT32 implementation did not require MountedFs trait changes.

The mount ABI now accepts `fs_type = 3` for FAT32 on `device1` read-only. `source = "device1"` is a fixed test-harness device selector, not a generic device-string parser.

`MountEntry` now carries `rights_mask: FileRights`. Effective open rights are:

```text
effective = inode.available_rights() & mount_entry.rights_mask
```

For FAT32, WRITE is always masked off. EXECUTE is masked off by default and admitted only when mount flag bit 0 is set.

Mount flag bit 0 is `MOUNT_FLAG_EXEC_ALLOWED`. Reserved nonzero bits return `EINVAL`. Default FAT32 mounts are noexec, so opening a FAT32 file with EXECUTE on a default mount is rejected with `EACCES`. With `MOUNT_FLAG_EXEC_ALLOWED`, EXECUTE may be held as an open right, but actual execution is still governed by the existing type-first executable policy.

The FAT32 write disposition is open-time rejection: WRITE is removed by the mount mask, so write-capable open returns `EACCES`.

`tools/xtask` generates the FAT32 disk deterministically with `mformat`, `mcopy`, `mmd`, `mdir`, and `mtype`; no binary fixture is committed.

The first FAT32 device1 run exposed that the virtio-blk completion worker was polling only the primary device. `kernel/src/drivers/block/virtio_blk.rs` now registers all virtio-blk devices and polls all completion queues. The new `VIRTIO_DEVICES` lock has IF discipline documented at its declaration site. No FAT32-internal lock was added.

## 4. Anti-scope confirmations

- no FAT32 write code landed; no ext4 code landed; no exFAT/FAT12/FAT16 code landed
- all FAT32 directory entries imported as Directory or Regular only; `.neom` imported as Regular, never Model
- no committed FAT32 binary image fixture landed
- no generic device-string parser landed
- no `MountedFs` or `Inode` trait change landed

## 5. Build gates

| Gate | Command | Result | Warning count |
|---|---|---:|---:|
| default build | `docker compose run --rm neo cargo k` | PASS | 52 |
| no-default build | `docker compose run --rm neo cargo k --no-default-features` | PASS | 52 |
| scoped diff check | `git diff --check -- <BUILD-4C source files>` | PASS | 0 errors |
| new file whitespace check | `kernel/src/fs/fat32.rs` trailing whitespace scan | PASS | 0 errors |

Feature-selected scenario builds still emit the repo's existing broad dead-code/test-helper warning profile. No warning was escalated to an error.

## 6. New FAT32 scenario PASS lines

```text
NEO_TEST/1 event=PASS name=phase3_fat32_mount_readonly mount=fat32 fs_type=3 source=device1 target=fat32 readonly=true
NEO_TEST/1 event=PASS name=phase3_fat32_readdir_root entries=5 hello=Regular readme=Regular subdir=Directory lfn=Regular neom=Regular
NEO_TEST/1 event=PASS name=phase3_fat32_open_read_regular_file path=/fat32/hello.txt bytes=18 content_match=true
NEO_TEST/1 event=PASS name=phase3_fat32_lfn_lookup lfn_lookup=case_preserving uppercase_lookup=case_insensitive
NEO_TEST/1 event=PASS name=phase3_fat32_subdir_walk path=/fat32/subdir/nested.txt content_match=true
NEO_TEST/1 event=PASS name=phase3_fat32_write_is_rejected write_rejected=open errno=EACCES readonly=true
NEO_TEST/1 event=PASS name=phase3_fat32_exec_blocked_by_default default_noexec=EACCES exec_allowed_flag=false
NEO_TEST/1 event=PASS name=phase3_fat32_exec_with_opt_in exec_open=ok exec_policy=ok exec_allowed_flag=true actual_exec=false
NEO_TEST/1 event=PASS name=phase3_fat32_type_import_is_regular neom_kind=Regular model_imports=0 symlink_imports=0 entries=5
```

## 7. Targeted anti-regressions

All dispatch-required anti-regressions passed:

```text
phase3_mount_neofs_device0_readonly PASS
phase3_mount_requires_mount_right PASS
phase3_umount_busy_is_ebusy PASS
phase3_neofs_readdir_root PASS
phase3_neofs_filecap_generation_stale PASS
phase3_neofs_fsync_filecap_stale_regression PASS
phase3_sched_wake_token_abandon PASS
phase3_vfs_directory_read_is_eisdir PASS
ipc_heap_kill_smoke PASS
cap_revoke_local PASS
```

## 8. Final close gate

Final gate:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: PASS, exit code 0.

Log path:

```text
target/neo-test/build4c-no-meta/combined.log
```

The full SMP no-meta run included all nine FAT32 scenarios, the BUILD-4B mount-table regressions, scheduler wake-token coverage, VFS directory-read coverage, capability revoke smoke, NeoFS persistence/writeback/GC coverage, and the aarch64 virtio barrier lane. All completed with exit code 0.
