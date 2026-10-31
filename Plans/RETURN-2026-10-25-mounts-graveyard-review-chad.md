<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #59): MOUNTS graveyard post-merge review (#122)

**Overall verdict: PASS. All six checklist items pass.**

Reviewed SHA: `b12df62ae5dd056c602a7791b683d4b4bdfff9e2`.
Branch: `codex/mounts-graveyard-review-chad`.
Scope: review only; no source edits, build, or new test execution.

Read the dispatch, CP-37/CP-56, the VFS IF-on NOTE, requested diff from `27a1ad30`,
the full MountTable/guard implementation and its callers, NeoFS ownership/teardown,
writeback-worker stop/join, and scenario/scheduler paths. RAG health was healthy
(ollama=true, qdrant=true). A filtered semantic discovery query returned no hits;
used direct reads and exact-symbol searches against the reviewed worktree.

## Checklist

| # | Verdict | Enforcing evidence at the reviewed SHA |
|---|---|---|
| 1. One way in | **PASS** | Whole-tree Rust searches for MOUNTS, MountTable and lock_mounts find one raw acquisition: `kernel/src/fs/mod.rs:642`, inside `lock_mounts` (`:640`). All other acquisition sites use the wrapper (`:213`, `:696`, `:707`, `:736`, `:765`, `:817`, `:844`, `:933`, `:1035`, `:1049`, `:1060`, `:1332`, `:1349`). MOUNTS is private at `:628`; no bypass acquisition/alias was found. The checklist's no-direct-lock condition is satisfied outside the wrapper's necessary raw acquisition. |
| 2. Displaced entries | **PASS** | `set_neofs_typed` moves the previous shortcut to graveyard.typed (`kernel/src/fs/mod.rs:507` through `:509`). `install_root_child` defers the rejected new filesystem on Busy (`:561`), and moves the replaced old filesystem with mem::replace and push (`:565`, `:566`). `add_child` defers the rejected filesystem on AlreadyExists (`:595`). `remove_child` moves entry.fs out of the removed entry into the graveyard (`:619`, `:620`); the entry's remaining fields contain no filesystem/state Arc. Other methods only inspect entries, clone references, update scalar/atomic fields, or move new filesystems into new entries (`:574`, `:600`). No other entry/typed overwrite, clear, retain, truncate, pop, or removal was found. |
| 3. Unlock, then drop | **PASS** | `MountsGuard::drop` takes its owned guard at `kernel/src/fs/mod.rs:661`, replaces the entire table graveyard with Default at `:664`, drops the lock guard at `:667`, then drops the detached graveyard at `:668`. The underlying guard actually unlocks with a Release store at `kernel/src/sync/switch_mutex.rs:302`. Thus a later acquisition sees an empty graveyard, even if an earlier detached graveyard is still being destroyed. Ordinary returns, temporary guards, explicit drop, and `?` all use this same Drop. The only inverse order is the explicitly gated control at `fs/mod.rs:670` through `:673`. No table-owned NeoFsState teardown runs under the holder's MOUNTS guard on normal paths. |
| 4. add_child claim | **PASS** | `vfs_mount_at_parent` binds `_neofs_typed` at `kernel/src/fs/mod.rs:987`, constructing Some(typed) unconditionally for both NeoFS mount types at `:1002` through `:1004`, including non-neo-test builds. The mount guard is declared later at `:1035`. On `add_child(...)?` failure (`:1036`), that guard drops before the earlier typed local, so the rejected trait-object Arc cannot be the last NeoFS reference while MOUNTS is held. The typed local is moved only on the later success branch (`:1040`). The new code covers rejection structurally anyway by moving fs to the graveyard (`:595`). The existing EEXIST scenario also exercises two readonly NeoFS mounts of the same target (`kernel/src/test_harness.rs:31927`, `:31932`, errno assertion `:31933`); it does not separately prove writeback teardown for a rejected read-write mount. |
| 5. Scenario / control | **PASS** | `kernel/src/test_harness.rs:32024` mounts read-write NeoFS (type 2), arms a 40-tick drop pause (`:32034`), and spawns the umounter on its CPU (`:32035`). `kthread::spawn_owned` enqueues through `kernel/src/sched/kthread.rs:275`; `scheduler::push_ready_kthread` uses the current CPU queue (`kernel/src/sched/scheduler.rs:697`, `:702`, `:720`, `:726`). The NeoFsState Drop hook runs before stop/join (`kernel/src/fs/neofs/inode.rs:1069`); it publishes the pause and yields IF-on (`:1100` through `:1106`). The parent waits for that pause (`test_harness.rs:32044`), then takes MOUNTS inside without_interrupts (`:32051`, `:32052`; actual acquisition `fs/mod.rs:1060`), samples that the drop remains paused (`:32053`), and rejects errors, a surviving mount, or a completed pause (`:32066`, `:32069`, `:32072`). The control reverses destructor/unlock order (`fs/mod.rs:672`), causing the same-CPU IF=0 lock taker to spin behind the paused holder. The positive scenario is a nonignored gate entry (`tools/xtask/src/main.rs:3950` through `:3953`); the control appears only in the explanatory opt-in comment (`:3948`) and Cargo feature (`kernel/Cargo.toml:1264`), not as a gate entry or a normal scenario dependency. |
| 6. No new lock order | **PASS** | Mount construction/probing finishes before acquiring MOUNTS (`kernel/src/fs/mod.rs:991` through `:1035`, and `:813`/`:817`, `:840`/`:844`). Path resolution clones its filesystem under MOUNTS but calls lookup/create only after that guard's scope ends (`:735` through `:750`). Deferred teardown starts after unlock (`:667`, `:668`), then `NeoFsState::drop` calls stop_writeback_worker (`kernel/src/fs/neofs/inode.rs:1070`). That helper takes the handle out under a short worker lock (`kernel/src/fs/neofs/cache.rs:443`) before requesting stop, waking the cache waiter, and joining (`:451` through `:453`). The cache waiter is taken under the cache lock before wake (`:366`, `:367`). The writeback worker captures only the cache (`:421` through `:435`) and drains/parks without consulting MOUNTS; no reverse mount-table acquisition was found in cache/state teardown. No MOUNTS-to-NeoFS-lock or MOUNTS-to-join nesting remains in this path, so moving destruction out removes that ordering rather than creating another one. |

## Exit-path and ownership checks

- **Unmount success:** remove_child moves the trait-object Arc into graveyard.fs. The neo-test
  shortcut is moved into graveyard.typed by set_neofs_typed(None) (`fs/mod.rs:1050` through
  `:1053`). Both sets of references leave the table together; their final releases occur
  after unlock. If other inode/reference owners remain, their later final release is
  independent of the mount guard.
- **Rejected/replacement mount:** Busy rejects into the graveyard without changing the old
  entry; replacement moves the old Arc without decrementing it in place. `?` in the boot
  helpers (`fs/mod.rs:820`, `:847`) or vfs_mount_at_parent (`:1036`) drops the later guard
  before earlier typed locals. Success replaces any old typed shortcut through the setter.
- **Fresh guard:** mem::take installs a new empty Graveyard while still locked. A concurrent
  or reentrant later guard cannot inherit references whose destructors are already running.
  Default creates empty vectors, and no filesystem method is invoked while draining the
  table itself. Allocation failure/kernel panic aborts rather than unwinding into another
  normal exit (`Cargo.toml:86`, `:89`); no allocation-failure recovery claim is made.

The wrapper preserves caller IF rather than masking it itself. This change structurally
moves table-owned filesystem destruction outside MOUNTS; it does not convert the existing
MOUNTS lock into an IrqSafeMutex. Existing resolve-path IF-off sections remain in place
(`fs/mod.rs:735`, `:764`). The separate model-load/ARENA issues identified in the NOTE are
outside this dispatch and were not treated as changes introduced by #122.

## Existing CI and control evidence

[Run 40194763872](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40194763872)
reports all 23 jobs successful on `e744701c3ac365982e84f9c35bbc0bfb4aec83df`. I fetched
that object and verified no diff against the reviewed squash in `kernel`, `tools`, or `.github`.

[QEMU shard 2/4](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40194763872/job/121173983966)
records:

```text
2026-10-25T13:15:08.8297995Z [xtask] neo-test: `phase3_umount_drop_outside_mounts` PASS
```

The control hang was verified by tracing its compiled order and the same-CPU schedule:
CPU0's umounter retains MOUNTS while its Drop publishes DROP_PAUSED and yields IF-on;
CPU0's parent observes the pause, masks IF, and spins acquiring MOUNTS. The umounter
cannot resume to clear the pause/unlock, so the outer harness must time out. The NOTE
reports four prior control timeouts; I did not independently execute a control or inspect
those local run files. Only the positive CI log above was directly verified here.

## Workspace handoff

Only this RETURN is committed. Preexisting untracked graft cache files were preserved in
the named stash `chad preexisting graft cache before relay 59 mounts graveyard review`.
No source fix, revert, build, or test run was performed. Final handoff requires empty
output from `git status --porcelain` and `git log '@{u}..'` after push.
