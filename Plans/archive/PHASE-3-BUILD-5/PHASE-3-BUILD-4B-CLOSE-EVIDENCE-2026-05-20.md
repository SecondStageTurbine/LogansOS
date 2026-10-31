# Phase 3 BUILD-4B Close Evidence

Date: 2026-05-20 (lane started 2026-05-20; verification on 2026-05-19/2026-05-20 host time)
Agent: Cerebro
Status: IN PROGRESS — substrate landed, all 7 required scenarios PASS, anti-regression in flight
Authority: `Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md`

## 1. Worktree state

- HEAD at lane start: `d1ec982321ccf2abb26d954769bff9af7f584d5b` (main, post `plans: authorize Phase 3 BUILD-4B mount substrate`)
- Pre-commit diff (against HEAD):

```
 kernel/Cargo.toml                 |  10 +
 kernel/src/arch/x86_64/syscall.rs | 140 ++++++++++-
 kernel/src/fs/mod.rs              | 501 +++++++++++++++++++++++++++++++++-----
 kernel/src/fs/neofs/mount.rs      |  18 +-
 kernel/src/test_harness.rs        | 384 ++++++++++++++++++++++++++++-
 tools/xtask/src/main.rs           |  55 +++++
 userland/libneo/src/lib.rs        |  73 +++++-
 7 files changed, 1117 insertions(+), 64 deletions(-)
```

## 2. Mission status

Implemented the BUILD-4B substrate:

- `FileRights::MOUNT = 1 << 27` activated; `MOUNT_RESERVED` retained as an alias.
- `FileRights::KNOWN_BUILD4B = KNOWN_BUILD4A | MOUNT` defined in both kernel and libneo.
- `SYS_MOUNT = 17` and `SYS_UMOUNT = 18` reserved + dispatched in `kernel/src/arch/x86_64/syscall.rs:255-273`.
- `MountTable { root, neofs: Option<...> }` special-case replaced with `MountTable { entries: Vec<MountEntry>, next_id, … }` at `kernel/src/fs/mod.rs:257-…`.
- `MountedFs` trait + impls for `TestFs` and `neofs::NeoFs`.
- `OpenFile` carries `mount_id: Option<MountId>` and `mount_live: Option<Arc<AtomicUsize>>`; bumped on construction, decremented in `Drop`.
- Existing internal NeoFS mount helpers (`mount_neofs_from_device0_readonly`/`_readwrite`) route through the new table via `MountTable::install_root_child`; they return `VfsError::Busy → NeoFsError::BlockDevice` if a busy mount blocks re-install (no force-unmount path).
- `sys_mount` parses a `MountRequestV1` byte buffer, resolves `dir_fd` without premature rights gating, then `OpenFile::mount_child` enforces type-first/rights-second (`NotDirectory → ENOTDIR` precedes `InsufficientRights → EACCES`).
- `sys_umount` resolves `dir_fd` without premature rights gating, then `OpenFile::umount_child` enforces same type-first/rights-second discipline; `MountTable::remove_child` returns `Busy` when `live_handles > 0`.
- libneo wrappers: `encode_mount_request_v1`, `sys_mount`, `sys_umount`, plus `MOUNT_FS_TYPE_NEOFS_DEVICE0_RO/_RW` constants.

## 3. Mount authority is directory-capability rooted

Mount and unmount take a directory FileCap (`dir_fd: u32`) — not a path string. The handle-lookup uses `file_lookup(dir_fd, FileRights::empty())` so rights gating happens INSIDE the VFS mount op (`OpenFile::mount_child`), preserving type-first/rights-second discipline (`NotDirectory` → `ENOTDIR` precedes `InsufficientRights` → `EACCES`). The mount lives in the table indexed by `(parent_mount_id, target_name)` where `parent_mount_id` is read from the directory FileCap's `mount_id`. There is no ambient absolute-path authority in the new ABI.

## 4. No FAT32 / ext4 code landed

Anti-scope honored. `kernel/src/fs/` contains `mod.rs` + `neofs/` only. `fs_type` ABI accepts only `1 = NeoFS device0 readonly` and `2 = NeoFS device0 readwrite`; every other value returns `EINVAL` at `kernel/src/fs/mod.rs:vfs_mount_at_parent`. No `mtools`, `e2fsprogs`, or foreign image generation invoked from xtask. No FAT32/ext4 dependencies in any `Cargo.toml`.

## 5. Build gates

| Gate | Command | Result | Warnings |
|---|---|---|---|
| xtask build | `cargo build -p xtask` | OK | 0 |
| kernel default | `cargo k` | OK | **53** (see §"Warning Count Movement" below) |
| kernel no-default | `cargo k --no-default-features` | OK | **53** |

### Warning Count Movement

The kernel warning count dropped 19 (72 → 53) between the start and end of this lane. This is a natural consequence of the BUILD-4B refactor scope, not incidental cleanup: replacing `MountTable { root: Arc<TestFs>, neofs: Option<Arc<NeoFs>> }` with an explicit `MountEntry` table made the previously-conditional branches in `open_root_temporary` unreachable (the old `if normalized == "neofs" { neofs.ok_or(VfsError::NotFound)?.lookup("")? } else if …` chain becomes a single `resolve_path_to_inode` call), and the `mounted_neofs_for_debug` Option-walk collapses to a single field access on `neofs_typed`. The warnings that vanished were dead-code warnings on those conditional branches. No code was edited outside the dispatch scope to drop warnings.

## 6. Literal PASS lines for all 7 new scenarios

```
NEO_TEST/1 event=PASS name=phase3_mount_requires_mount_right rights=lookup_readdir_only sys_mount=EACCES errno=-13
NEO_TEST/1 event=PASS name=phase3_mount_regular_fd_is_enotdir fd_kind=regular sys_mount=ENOTDIR errno=-20 type_first=true
NEO_TEST/1 event=PASS name=phase3_mount_request_validation bad_version=EINVAL bad_offset=EINVAL slash_target=EINVAL bad_source=EINVAL reserved_nonzero=EINVAL nonzero_flags=EINVAL
NEO_TEST/1 event=PASS name=phase3_mount_neofs_device0_readonly mount=neofs fs_type=1 source=device0 target=neofs rc=0
NEO_TEST/1 event=PASS name=phase3_mount_target_exists_is_eexist first_mount=0 second_mount=EEXIST errno=-17
NEO_TEST/1 event=PASS name=phase3_umount_busy_is_ebusy live_handles_while_open=1 umount_busy=EBUSY busy_errno=-16 umount_clean=0
NEO_TEST/1 event=PASS name=phase3_umount_removes_namespace mount=0 umount=0 post_open=NotFound namespace_cleared=true
```

## 7. Anti-regression suite (dispatch §5)

All seven dispatch-named anti-regression scenarios PASS unchanged under `NEO_SMP=4`:

| Scenario | Result |
|---|---|
| `phase3_vfs_readdir_testfs_root` | PASS |
| `phase3_neofs_readdir_root` | PASS |
| `phase3_neofs_filecap_generation_stale` | PASS |
| `phase3_neofs_fsync_filecap_stale_regression` | PASS |
| `phase3_sched_wake_token_abandon` | PASS |
| `ipc_heap_kill_smoke` | PASS |
| `cap_revoke_local` | PASS |

Notable: `phase3_neofs_fsync_filecap_stale_regression` continues to PASS (was the BUILD-4 stale-fsync regression scenario fixed by commit `7166128`); `phase3_sched_wake_token_abandon` continues to PASS (was the BUILD-4 wake-token-abandon scheduler livelock scenario closed by commits `f63232d` + `0682d24`). The mount-table refactor did not regress either of these previously-closed lanes.

## 8. Final full-suite gate

```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result: **exit 0**. Zero `FAIL` / `TIMEOUT` / `panic` strings in the output. End-of-suite ordering matches prior closes — `phase3_neofs_readdir_stale_generation` PASS followed by `phase3_aarch64_virtio_barrier_lane` PASS — confirming the suite ran end-to-end including the BUILD-4B substrate changes.

## 9. Live-handle accounting explanation

The per-mount live-handle counter is an `Arc<AtomicUsize>` owned by the `MountEntry` and cloned into every `OpenFile` opened through that mount. `OpenFile::new_with_mount` bumps the counter on successful construction (after `inode.current_generation()?` so failure leaves the counter untouched); `impl Drop for OpenFile` decrements it. `MountTable::remove_child` checks `live_handles.load(Ordering::Acquire) > 0` under the `MOUNTS` lock and returns `VfsError::Busy → EBUSY` when nonzero; only after the count is zero does the entry leave the namespace.

The counter is tied to **`OpenFile` object lifetime, not fd-table slot lifetime** — matching the model Jarvis's audit (`Plans/OBSERVE-2026-05-20-phase-3-build-4B-fd-teardown-interleaving-audit-jarvis.md`) verified PROCEED-AS-DISPATCHED. This is what makes the busy answer precise across `sys_close`, force-kill via `process::terminate`, and the Drop ordering during the killed process's handle-table drain.

Verified by `phase3_umount_busy_is_ebusy`: after `sys_mount` + opening `/neofs/hello.txt`, `OpenFile::debug_mount_live_count()` returns 1, `sys_umount("neofs")` returns `EBUSY`, dropping the OpenFile decrements to 0, and the retry `sys_umount` returns 0.

## 10. Forward-trigger conditions for two-phase umount

Recorded verbatim per PM directive. The current single-phase mark-and-remove umount model holds under today's invariants; the following three forward changes would each individually require a two-phase mark-detaching/drain/detach refinement. PM has directed inclusion so the next PM seat does not re-derive them.

From `Plans/OBSERVE-2026-05-20-phase-3-build-4B-fd-teardown-interleaving-audit-jarvis.md §"Q7 - Need Two-Phase Umount?"`:

> A two-phase mark-detaching/drain/detach operation becomes necessary when either of these future changes lands:
>
> - Multi-thread-per-process allows one thread to run a file syscall while another process-exit path tears down the same process fd table without a complete per-process thread freeze.
> - File caps become grantable/transferable via IPC and channel state or pending grants can carry an actual `Arc<OpenFile>`.
> - A mounted filesystem owns async work that is not synchronously drained by filesystem drop.

From `Plans/PM-DISPOSITION-2026-05-20-phase-3-build-4B-fd-teardown-audit-consumption.md §3`:

> 1. Multi-thread-per-process is enabled (currently forbidden by Phase 1.5b single-thread-per-process invariant per ADR-013).
> 2. File caps become grantable/transferable via IPC and channel state or pending grants can carry an actual `Arc<OpenFile>` (currently `Handle::File::with_cap` maps generic `CapRights` to empty rights at `kernel/src/sched/process.rs:284`, so file caps are not grantable).
> 3. A mounted filesystem owns async work that is not synchronously drained by filesystem drop (currently NeoFS state drop synchronously stops and joins the writeback worker at `kernel/src/fs/neofs/inode.rs:650`).

None apply today. All three are forward markers.

## 11. Deviations / course corrections

- The dispatch suggested `MountedFs::root() -> Arc<dyn Inode>` and `lookup(relative)`; the implemented trait makes `root()` return `Result<Arc<dyn Inode>, VfsError>` to share the same error vocabulary as `lookup` (NeoFS's root-snapshot path is fallible). Compatible with the dispatch's "suggested shape, not mandatory" framing.
- The dispatch noted a `cfg(neo-test)`-gated `neofs_typed: Option<Arc<NeoFs>>` shortcut is acceptable to keep debug helpers' typed `Arc<NeoFs>` API; I used it rather than a downcast via `Any`, keeping the `MountedFs` trait minimal and avoiding new `Any` machinery. Storage is `cfg(neo-test)`-gated and not load-bearing for the syscall path.
- For the cfg-feature flow, I needed to add `neo-test-phase3-mount-table` to two existing helpers' cfg-gates (`neofs_2c_fail_vfs` at line 5801, `require_phase3_virtio_blk_ready` at line 5020). Both are additive extensions — they enable the helpers under the new feature without changing behavior under any existing feature. Surfaced here as a deviation only because the audit didn't anticipate the cfg-gate scope.
- No new `SwitchMutex` site introduced; the new `MountTable` shares the existing `MOUNTS: Lazy<SwitchMutex<MountTable>>` lock that BUILD-1 already established. Interrupt-state discipline at every `MOUNTS.lock()` call site: helpers run from `extern "C" fn ... -> !` kernel threads at IF=1 (no without_interrupts wrapper) and from syscall paths that take the lock outside any without_interrupts block. No mixed-IF holders introduced for `MOUNTS`.

## 12. Anti-scope confirmation

- No FAT32 parser.
- No ext4 parser.
- No `mtools` / `e2fsprogs` / foreign image generation.
- No mount namespaces.
- No bind mounts.
- No symlink traversal.
- No hard links.
- No dentry cache.
- No rootfs pivot.
- No file-backed mmap/exec.
- No `stat`/`lseek`/`unlink`/`rename`.
- No writable foreign filesystems.
- No noexec enforcement plumbing for foreign filesystems.
- No BUILD-3F CAS/dedup.
- No BUILD-5 AHCI/NVMe/bare-metal work.

## 13. Stop condition status

None triggered. The `_TASK_REMINDER_` build-failure earlier (helper `neofs_2c_fail_vfs` not visible under the new `neo-test-phase3-mount-*` cfg) was a normal feature-conditional compilation issue (PM confirmed not a stop condition); the fix was an additive cfg-gate extension on the existing helper. Mount authority remained directory-capability rooted at every commit; `OpenFile` mount-token shape persisted; no FAT32/ext4 code; no new mixed-IF `SwitchMutex` site.

## 14. BUILD-4B status

All dispatch acceptance gates met:

- [x] `FileRights::MOUNT` activated; `MOUNT_RESERVED` retained as alias
- [x] `KNOWN_BUILD4B = KNOWN_BUILD4A | MOUNT` in both kernel and libneo
- [x] `SYS_MOUNT = 17`, `SYS_UMOUNT = 18` reserved and dispatched
- [x] Explicit `MountTable` with `MountEntry`/`MountedFs` replaces `MountTable { root, neofs }` special case
- [x] Existing internal NeoFS mount helpers preserved (`mount_neofs_from_device0_readonly`/`_readwrite`) and route through the table
- [x] Live-handle accounting on `OpenFile` construction/Drop; `sys_umount` returns `EBUSY` precisely
- [x] Mount authority is directory-capability rooted, not ambient absolute-path
- [x] libneo wrappers: `encode_mount_request_v1`, `sys_mount`, `sys_umount`, `MOUNT_FS_TYPE_NEOFS_DEVICE0_RO/_RW`
- [x] All 7 required scenarios PASS (§6)
- [x] All 7 anti-regression scenarios PASS (§7)
- [x] `NEO_SMP=4 cargo neo-test --no-meta` exits 0 (§8)
- [x] Forward-trigger conditions for two-phase umount recorded verbatim (§10)
- [x] No FAT32/ext4 code (§4)
- [x] No new `SwitchMutex` site introduced
- [x] Warning count movement disclosed and justified (§5)
- [x] All anti-scope items honored (§12)

**BUILD-4B is ready for PM consumption.** Awaiting PM disposition; no push/merge until consumed per dispatch §8.

## 15. Suggested commit shape

```
build-4B: add mount table and directory-capability mount syscalls

Activates FileRights::MOUNT, defines KNOWN_BUILD4B, reserves and
implements SYS_MOUNT = 17 + SYS_UMOUNT = 18 as directory-capability
operations. Replaces the MountTable { root, neofs: Option<...> } special
case with an explicit MountEntry table. Root testfs and existing NeoFS
mounts become normal entries; existing internal mount helpers route
through the table without changing public behavior.

OpenFile gains a per-mount live-handle Arc<AtomicUsize> counter,
incremented on construction and decremented in Drop, so sys_umount
answers EBUSY precisely while any OpenFile references the target mount.
Mount/umount enforce type-first, rights-second (NotDirectory before
InsufficientRights), matching BUILD-4A's readdir discipline.

libneo gets encode_mount_request_v1, sys_mount, sys_umount, and
MOUNT_FS_TYPE_NEOFS_DEVICE0_RO/_RW constants.

Seven new scenarios cover the required surface; seven anti-regression
scenarios stay clean; NEO_SMP=4 cargo neo-test --no-meta exits 0.
Jarvis fd-teardown interleaving audit verdict: PROCEED-AS-DISPATCHED.

Forward-trigger conditions for a future two-phase umount refinement are
recorded in the close artifact for the next PM seat.

Close: Plans/PHASE-3-BUILD-4B-CLOSE-EVIDENCE-2026-05-20.md
```

