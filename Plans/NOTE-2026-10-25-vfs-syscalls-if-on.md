<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: the VFS file syscalls run their NeoFS body IF-on (CP-56, follow-up to #120)

**Seat:** S4. **Trigger:** the #120 lock audit (PR #120, issue comment 6396227883). The PM assigned it as
its own lane.

## The defect

- OT-74 made `sys_read` and `sys_write` run `file.read`/`file.write` inside `with_interrupts_enabled`.
  The NeoFS locks they take (`entries`, `root`, `record`, `inode_block`, `metadata`, all plain
  `SwitchMutex`es) therefore have IF=1 holders that the timer can preempt. One example is `sys_write` →
  `write_at` → `commit_regular_write` → `commit_child_update`, which takes `entries`.
- `sys_fsync`, `sys_open`, `sys_readdir` and `sys_ftruncate` still called into the VFS at the syscall's
  IF=0, and they take the same locks:
  - fsync: `fsync_regular` takes `entries`;
  - open: `resolve_path`/`create_regular` take `entries`;
  - readdir: `readdir_at` takes `entries`;
  - ftruncate: `commit_regular_truncate` → `commit_child_update` takes `entries`.
- If one of them lands on the CPU of a preempted holder, it spins with interrupts masked and the holder
  never runs again. That is CP-37 on `entries`, the CP-56 shape: the IF discipline was applied to
  read/write and missed on the other file syscalls.
- `sys_fsync` was also the IF=0 waiter in the #120 write-guard hang (CP-82).

## The change

- **The four bodies run IF-on.** `vfs_body_if_on` (`with_interrupts_enabled`) wraps `open_root_temporary`,
  `readdir_one`, `fsync` and `truncate` in `sys_open`, `sys_readdir`, `sys_fsync` and `sys_ftruncate`.
  The kernel-buffer readdir and the test open entry share these bodies.
- **What stays IF-off.** User copies (the SMAP window must not span a switch, CP-49) and handle-table
  work (`file_lookup`, `install_cap_excluding`, which `sys_close` takes IF-off) stay outside the wrap, as
  in `sys_read`/`sys_write`.
- **`MOUNTS` stays IF-off.** Mount, umount and model provenance (`FileProvenance::validate_current`)
  still take `MOUNTS` at IF=0. If open took it IF-on, it would become a preemptible holder of a lock an
  IF=0 path spins on, the inverse of the bug. So the two short `MOUNTS` sections in open
  (`resolve_path_to_inode`, `normalized_targets_root_child_mount`) run under `without_interrupts`. Both
  copy what they need and drop the lock before any filesystem call.
- **Kill-switch.** `neo-test-vfs-syscall-if-off-control` (test only) runs the four bodies at the caller's
  IF, as before.

Lock audit for the newly IF-on bodies (what an IF=1 holder could block):
- The NeoFS `SwitchMutex`es: these already had IF=1 holders (`sys_read`/`sys_write`, the writeback
  kthread), so nothing new.
- The cache's `IrqSafeMutex`: it masks IF itself.
- `MOUNTS`: kept IF-off, as above.
- FAT32/ext4/testfs inode state: reached the same way by `sys_read`, which is already IF-on.
- On virtio, the I/O gate already turned IF on inside these same bodies (CP-2). This change makes the
  whole body preemptible, not just the gate wait.

## Discriminator

`phase3_neofs_syscall_if_on`:
- For each call (fsync, open, readdir, ftruncate), a kthread spawned on the scenario's CPU takes
  `entries` and keeps it for 20 ticks while yielding with IF on, the state of a preempted `sys_write`.
- Once it holds the lock, the scenario thread makes the call inside `without_interrupts`, as a syscall
  arrives.
- The call must return its normal result and finish no earlier than the holder's release tick. Finishing
  earlier fails `call_did_not_contend_on_entries`, which means the leg did not exercise the lock.

Local results (`NEO_SMP=4`):

| run | result |
|---|---|
| fix | PASS in 13.4 s, first attempt. Ticks per leg (start / released / done): fsync 99/118/133, open 134/154/154 (fd 3), readdir 154/174/174 (25 bytes), ftruncate 174/194/200. |
| control `neo-test-vfs-syscall-if-off-control` | hangs on all 4 attempts (180 s each), every one right after `enqueued kthread 'entries-holder'`, at the first leg. |

**Falsification:** if the control ever passes, the leg does not reproduce the deadlock (for example, the
holder ran on another CPU), and the PASS proves nothing. If the fix hangs, a lock other than `entries`
is still taken IF=0 on the path.

## Still open (not in this lane)

- ~~**`sys_model_load`** reads a NeoFS-resident model (`read_model_file`, `neom_total_size_hint`) at IF=0.~~
  **Fixed by #123** (the disk open and `read_model_file` run inside `vfs_body_if_on`; relay #60 PASS).
  (An earlier version of this line also named `sys_tensor_write`. That was wrong: it reads no file.) A
  preloaded boot model never reaches these reads. Design: `Plans/NOTE-2026-10-25-model-load-if-on-design.md`.
- ~~**Model provenance** (`Model::validate_current` in model query, infer and every generate variant) reaches
  NeoFS `entries` at IF=0.~~ **Fixed by #127** (lane D: `model_still_current`, and the provenance `MOUNTS`
  check under `without_interrupts`; relay #65 items 1-5 PASS). Not in this NOTE's first version; Jarvis's
  relay #58 item 4 found it.
- ~~**The scenario could pass without contention** (the holder could release between `HELD` and the call).~~
  **Fixed by #129** (a `HELD -> CALLING` gate claimed IF-off right before each call, plus the
  `neo-test-vfs-syscall-if-on-late-call-control` control; relay #58 item 5).
- ~~**Umount drops a `NeoFsState` while holding `MOUNTS`.**~~ **Fixed in the follow-up lane (below).**

## Follow-up: filesystems are dropped after `MOUNTS` is released

**The defect.** `remove_child` removed the mount entry under `MOUNTS`. When that dropped the last
reference, `NeoFsState::drop` stopped the writeback kthread, which means draining (block I/O, so IF on in
the virtio gate) and then joining (a park), all with `MOUNTS` held. An IF=0 taker of `MOUNTS` on that CPU
(open's resolve, mount, provenance) would spin forever behind the parked holder.

**The audit found two more sites than this NOTE first listed:**
- `install_root_child` overwrote `entries[idx].fs`, dropping the replaced filesystem under the lock. This
  is the boot remount path.
- The neo-test `neofs_typed` shortcut was overwritten or cleared under the lock.

**Correction.** `add_child`'s `AlreadyExists` path does drop `fs` under the lock, but for NeoFS that is
never the last reference: `vfs_mount_at_parent` keeps `_neofs_typed`, declared before the guard, so it
drops after the guard. FAT32 and ext4 have no `Drop` that does I/O. The path is covered anyway.

**Change.** Every `MOUNTS` acquisition goes through `lock_mounts()`. `MountTable` methods park anything
they let go of (an unmounted, replaced or rejected filesystem, or an old typed shortcut) in a graveyard,
and `MountsGuard::drop` releases the lock before dropping it. This is structural: no per-call-site
convention to remember. Kill-switch: `neo-test-mounts-drop-under-lock-control` drops before releasing.

**Discriminator: `phase3_umount_drop_outside_mounts`.**
- A kthread spawned on the scenario's CPU unmounts a read-write NeoFS.
- A test hook holds the `NeoFsState` drop IF-on for 40 ticks, standing in for the stop-and-join park.
- While it is held, the scenario thread takes `MOUNTS` with interrupts off. It must get the lock while
  the drop is still paused.

Local results (`NEO_SMP=4`):
- **Fix:** PASS, 9.9 s. `umount_rc=0 mount_exists=false drop_still_paused=true`, and `MOUNTS` was taken in
  the same tick (47).
- **Control:** hangs on all 4 attempts, right after `umounter` is enqueued.
- **Regressions:** 14 scenarios pass on the first attempt. They cover all of `phase3_mount_*` and
  `phase3_umount_*`, `phase3_fat32_mount_readonly`, `phase3_neofs_syscall_if_on`, fsync, readdir,
  `neofs_truncate`, `arthur_memory_forgetting` and `block_write_guard`.
