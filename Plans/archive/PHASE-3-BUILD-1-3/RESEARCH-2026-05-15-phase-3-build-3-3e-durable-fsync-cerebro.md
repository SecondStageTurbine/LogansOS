# Cerebro Structural Challenge — BUILD-3E Durable NeoFS fsync

**Date:** 2026-05-15  
**Agent:** Cerebro (sidecar to Jarvis BUILD-3E dispatch)  
**Status:** CHALLENGE NOTE — for Jarvis and PM consumption during 3E  
**Authority:** `Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-structural-challenge-cerebro.md`

---

## Substrate Read Summary

All analysis below is grounded in direct reads of four 3D files:

| File | Key findings |
|---|---|
| `kernel/src/fs/neofs/cache.rs` | `NeoFsBlockCache`, `drain_all`, `drain_pass`, `pause_worker`, `CachedBlockRole` ordering (Data=0 → Superblock=4) |
| `kernel/src/fs/neofs/inode.rs` | `NeoFsState::fsync_regular` stub already present with mount-wide drain; `commit_regular_write` / `create_regular` pause-worker-then-entries-lock discipline |
| `kernel/src/fs/mod.rs` | `OpenFile::fsync` → `validate_current()` → `inode.fsync()`; generation captured at open time; `refresh_generation` after write |
| `kernel/src/arch/x86_64/syscall.rs` | `SYS_FSYNC = 15`; `sys_fsync_impl` → `file_lookup(fd, FileRights::empty())` → `file.fsync()`; `debug_sys_fsync_for_test` gated on `neo-test-phase3-neofs-3e` |

The `fsync_regular` stub in `inode.rs` already contains the correct 3E approach:

```rust
pub(super) fn fsync_regular(&self) -> Result<(), NeoFsError> {
    if !self.writable { return Ok(()); }
    // Conservative BUILD-3E fsync: NeoFS does not yet attribute dirty cache
    // state per file, so fsync serializes with namespace mutation and drains
    // the whole mount through the same ordered path as write-back/GC.
    let _entries = self.entries.lock();
    self.cache.drain_all()?;
    Ok(())
}
```

Jarvis's work is wiring the test scenarios and verifying this implementation against the 6 required scenarios. The structural challenge is about whether this posture is safe, honest, and complete.

---

## Q1 — Mount-wide drain vs per-file dirty attribution

**Verdict: Mount-wide drain is correct and sufficient. It must be stated as mount-wide.**

The 3D cache (`NeoFsBlockCache`) has no per-file dirty tracking. Dirty blocks carry a `CachedBlockRole` (Data / Inode / RootDirectory / FreeMap / Superblock) but no file identity. The cache cannot answer "which dirty blocks belong to file X."

NeoFS's CoW structure makes mount-wide drain honest: every write to any file produces a chain of dirty blocks (data → inode → root directory → superblock copies), and the superblock is the single root of trust. If ALL dirty blocks are flushed and the device-level FLUSH is issued, the entire on-disk state is consistent and durable — not just the specific file's state. There is no way to partially flush "just one file" without also making other dirty files durable, because they all share the same publication chain.

**What Jarvis must state in the verification artifact:**

> NeoFS `sys_fsync(fd)` in BUILD-3E drains all dirty cache state for the entire mount, not only blocks attributed to `fd`. This is correct for durability (all dirty data is durable after return) but is over-conservative relative to POSIX per-file semantics. Per-file dirty attribution would require tracking dirty blocks by inode identity — that is deferred to a future BUILD.

The verification artifact must NOT claim per-file precision. The honest contract is: after `sys_fsync(fd)` returns, all dirty NeoFS cache state at the call site is durable on the block device.

---

## Q2 — Narrowest correct ordering for sys_fsync(fd)

The ordering must exactly match the write-back order used by the background worker and GC drains. No deviation is acceptable.

**Step-by-step fsync ordering:**

1. **validate_current** — `OpenFile::validate_current()` compares captured generation with `current_child_generation(name)`. Reject stale FileCaps before doing any durability work.
2. **acquire entries lock** — `self.entries.lock()`. This serializes with ALL concurrent CoW writes (`commit_regular_write`, `create_regular`) which also acquire the entries lock. No new dirty blocks can be created while this lock is held.
3. **pause_worker** — `self.cache.drain_all()` internally calls `pause_worker()`, which increments `foreground_writers`. This prevents NEW worker cycles from starting (`drain_worker_cycle` checks `foreground_writers != 0` at entry and returns early).
4. **drain dirty blocks in role order** — `drain_pass` selects dirty blocks by ascending `writeback_order()`:
   - Data blocks (order 0) first
   - Inode blocks (order 1)
   - RootDirectory blocks (order 2)
   - FreeMap blocks (order 3)
   - Superblock blocks (order 4) last
5. **Issue VIRTIO_BLK_T_FLUSH** — after all dirty blocks are written, if `flush_required` is set (set whenever any Superblock block is marked dirty), issue `device.flush()` and wait for completion.
6. **Clear flush_required** — inside lock, re-verify all dirty blocks gone, then clear `flush_required = false` and increment `flushes` counter.
7. **Loop until fully clean** — `drain_all` loops until `!flush_required && !any_dirty`. This compensates for any concurrent worker cycles (see Q3).
8. **Release entries lock** — via `_entries` drop at function exit.
9. **Return Ok(())** — fsync syscall returns 0 to userspace.

**Why this ordering is the minimum correct set:**

- Steps 1-3 must precede step 4 to prevent new dirty blocks from being added during drain.
- Step 4 ordering must match CoW publication order or the on-disk state could be partially committed (data without its inode, inode without its superblock).
- Step 5 must follow step 4 being fully complete (no remaining dirty blocks) so the FLUSH covers ALL data written in this fsync.
- Step 7 must be a loop, not a one-shot, because of the worker race (Q3).

---

## Q3 — Fsync race with background writeback worker

**Race window:** `pause_worker()` increments `foreground_writers` but does NOT atomically stop a `drain_pass` that has already started. A worker cycle that passed the `foreground_writers == 0` check before `pause_worker()` executes will continue to drain blocks concurrently with `drain_all`.

**Code path:**

```
// Worker kthread loop:
fn drain_worker_cycle(&self) -> ... {
    {
        let inner = self.inner.lock();
        if inner.writeback_hold || inner.foreground_writers != 0 {
            return Ok(DrainReport::default());  // only checked at entry
        }
    }
    drain_pass(device, inner_lock, Some(WORKER_DRAIN_BLOCKS_PER_CYCLE))
    // ^^^ can run concurrently with drain_all after pause_worker increments foreground_writers
}
```

**Why this is safe (and why drain_all's loop is the fix):**

1. `drain_pass` uses compare-and-clear: it only marks a block clean if the block's bytes exactly match the snapshot taken before the I/O. If two drainers both write the same block, the second write is idempotent (same bytes) and the clear is still correct.
2. Two concurrent FLUSHes are wasteful but not incorrect. The second FLUSH covers everything the first did, plus any blocks written between them. The `flush_required` double-check inside lock (`if inner.flush_required && !any_dirty`) prevents double-counting in the stats.
3. **`drain_all`'s loop is the deterministic fix**: it does not return until `!flush_required && !any_dirty`. Even if the worker races to drain some blocks or even issue FLUSH, `drain_all` loops until it confirms all dirty blocks are gone. The worker cannot cause `drain_all` to return early with dirty data remaining.
4. No new dirty blocks can be created during `drain_all` because `fsync_regular` holds the entries lock, and all CoW write paths (`commit_regular_write`, `create_regular`) take the entries lock before writing dirty blocks.

**Required discipline for Jarvis:**

Do NOT add a "pause-then-check-once-and-return" pattern. The loop in `drain_all` is load-bearing. Any optimization that removes the loop (e.g., checking clean state before calling `drain_all` and skipping it) must re-verify atomicity with the worker race.

The scenario `phase3_neofs_fsync_coordinates_with_writeback_gc` must exercise this race: dirty state, background worker running, fsync called concurrently. If it passes (fsync returns, all dirty gone, FLUSH observed), the race is correctly handled.

---

## Q4 — Fsync race with GC

**GC protocol (from `debug_run_quiescent_gc`):**

```rust
let _entries = self.entries.lock();      // serialize with CoW writes and fsync
self.cache.drain_all()?;                  // drain before mark/sweep
let _writeback_pause = self.cache.pause_worker();
let mut metadata = self.metadata.lock();
self.run_gc_locked(&mut metadata)        // collect_live_blocks reads from DEVICE
```

GC and fsync are serialized by the entries lock. They cannot run concurrently. The one that acquires the entries lock first completes its full drain-then-operation sequence; the other then acquires and starts fresh on a clean cache.

**The specific concern about dirty unpublished blocks:**

`collect_live_blocks` reads from the DEVICE (via `read_inode_from_block` and `read_fs_block`), not from the cache. If dirty blocks in the cache have not yet been written to the device, GC would see the OLD on-disk state and could mark new (dirty, cache-only) blocks as free for reuse.

**Why this cannot happen in practice:**

- GC calls `self.cache.drain_all()` before `run_gc_locked`
- After `drain_all`, all dirty blocks are on the device and `flush_required` is false
- The entries lock is held throughout, so no new dirty blocks can appear between drain and mark/sweep
- `collect_live_blocks` therefore reads a consistent, fully-published on-disk state

**No additional GC-specific locking is required.** The existing entries lock + drain_all sequence is sufficient. Jarvis must ensure GC is not called without first draining, which the existing code already enforces.

**Scenario `phase3_neofs_fsync_coordinates_with_writeback_gc`** should verify that after a mixed fsync+GC session, the free map is consistent with the live set (no live blocks marked free, no dead blocks missed).

---

## Q5 — Stale FileCaps after CoW generation changes

**Verdict: The stale check is correct. It must compare against the current reachable inode generation, not the old inode block.**

**The check chain:**

```
OpenFile::fsync()
  → validate_current()
      captured = *self.generation.lock()      // captured at open() time
      current  = self.inode.current_generation()?

NeoInode::current_generation()
  → self.state.current_child_generation(name)
      → entry = self.lookup_entry(name)       // current directory entries (in-memory)
      → (record, _) = self.read_cached_inode(entry.child_block)
      → Ok(record.generation)                // generation of the CURRENT reachable inode
```

After a CoW write:
- `commit_regular_write` creates a new inode block with `new_record.generation = generation` (next global generation)
- Updates `entries[entry_index].child_block = new_inode_block` (directory entry now points to new inode)
- Old inode block is abandoned (not zeroed, not freed until GC)

When a stale FileCap calls `validate_current`:
- `captured` = old generation (from when the file was opened)
- `current_child_generation(name)` reads the DIRECTORY ENTRY which now points to `new_inode_block` → reads `new_record.generation`
- `captured != current` → `Err(VfsError::Stale)` → syscall returns ESTALE (-116)

**The check is directory-anchored, not old-block-anchored.** Even if the old inode block is still in the cache (not GC'd), `current_generation()` does NOT read from it — it looks up the file by name in the directory, finds the current `child_block`, and reads THAT inode's generation. This is correct.

**Critical invariant for Jarvis:** do not change `NeoInode::current_generation()` to read from `self.inode_block` (the NeoInode struct's cached field). That field is updated after writes by the same process but is stale for OTHER processes that opened before the write. The directory-entry-anchored lookup is the correct anchor for the staleness check.

**Note on `refresh_generation`:** `OpenFile::write()` calls `refresh_generation()` after a successful write, updating the captured generation to the new value. So a process that wrote to a file and then calls fsync with the same handle will NOT see ESTALE — its captured generation was updated. Only holders of pre-write handles see ESTALE. This matches POSIX fsync semantics.

---

## Q6 — Clean-file fsync: conditions for no spurious FLUSH

**The clean-file no-FLUSH condition is mount-scope, not file-scope.**

`drain_all`'s fast-exit path:
1. `drain_pass` with no dirty blocks exits immediately (no blocks found in `filter(dirty)`)
2. `should_flush = flush_required && !any_dirty`
3. If `flush_required = false` (no superblock was ever written as dirty during this mount's lifetime without being cleared) → `should_flush = false` → no FLUSH issued
4. `drain_all` checks `!flush_required && !any_dirty` → `true` → returns immediately

**Under what conditions is clean-file fsync a true no-op?**

Only if the WHOLE MOUNT is clean: `!flush_required && !any_dirty_blocks`. Not just the specific file's blocks.

With mount-wide dirty attribution, if another file has dirty blocks at the time of the clean-file fsync:
- `drain_pass` will drain those blocks
- If any were Superblock-role, `flush_required` was set
- After draining, FLUSH will be issued
- `flushes` counter increments

This means the clean-file fsync scenario (`phase3_neofs_fsync_clean_no_extra_flush`) MUST be tested on a mount with no other dirty files. The test must:
1. Create a file, write it, then WAIT until the background worker has drained and the FLUSH counter has been captured (baseline)
2. Then read-only open a second file (or re-open the same clean file after all dirty state has been resolved)
3. Call fsync on the clean file
4. Assert dirty count remains 0 AND FLUSH counter has not incremented from baseline

**What Jarvis must NOT do:** run the clean-file fsync immediately after a write to another file (dirty mount). That would produce a "spurious" FLUSH that is actually correct behavior (another file's dirty blocks get flushed). The scenario would pass for wrong reasons.

**Acceptable simplification for 3E:** clean-file fsync on a dirty mount drains all dirty blocks. This is conservative-correct, not spurious from a durability standpoint. The verification artifact must state this.

---

## Q7 — Crash/reboot scenarios sufficient to distinguish durability states

Three distinct reboot scenarios are required, each proving a different contract:

### Scenario A: fsync'd data survives crash

**Setup:**
1. Write a NeoFS regular file (creates dirty blocks in cache)
2. Call `sys_fsync(fd)` → `drain_all` completes → FLUSH issued and completed
3. QEMU hard halt (kill process or `reboot 0x1` ACPI reset, NOT graceful shutdown)
4. Reboot with same disk image

**Observations required:**
- Mount succeeds (superblock valid, generation present)
- File found in directory entries with correct `child_block`
- `read_verified_logical_content` returns correct data (content hash matches)
- `parse_inode` succeeds with expected `file_size`, `block_count`, `direct_blocks`

**Evidence this satisfies:** after FLUSH returns, the device has committed all data to persistent storage. QEMU virtio-blk with `cache=none` or `writethrough` behavior means `VIRTIO_BLK_T_FLUSH` completion guarantees the data is durable.

### Scenario B: Unfsync'd dirty data may be lost

**Setup:**
1. Create file A, write, fsync it (committed baseline)
2. `debug_set_writeback_hold(true)` — prevents background worker from draining
3. Create file B, write (creates dirty blocks in cache, NOT on device — hold prevents drain)
4. Do NOT call fsync on file B
5. QEMU hard halt
6. Reboot

**Observations required:**
- Mount succeeds (old superblock is valid — file B's dirty blocks were never published)
- File A found with correct content (it was fsync'd before the hold)
- File B is either: (a) absent from directory (if superblock was never updated) OR (b) present but with old content (zero-size, from before the dirty write) — NOT torn/corrupt
- No `ContentHashMismatch` on mount, no invalid superblock detection

**Critical test constraint:** `debug_set_writeback_hold(true)` MUST be called before the file B write, and the halt must happen before the hold is lifted. If the background worker drains file B's blocks before halt, file B survives — which is correct behavior but doesn't exercise the "may be lost" contract. The test must hold the worker to create a controlled non-durable state.

### Scenario C: Old committed state is not torn by crash

This is implicitly covered by Scenario B's "mount succeeds" assertion. But Jarvis should add explicit assertions:
- `selected_superblock_slot` in the mount report indicates which superblock copy was selected on recovery
- At least one superblock copy is valid (rotation to the other copy on crash before second write means one copy always valid)
- The `generation` in the selected superblock matches the last fsync'd state, not a partial future state

**Why 3 scenarios, not 6:** Scenarios B and C are proven by the same QEMU run. The 6 Jarvis dispatch scenarios also cover coordination (Scenario 5), FLUSH evidence (Scenario 3), clean-file (Scenario 4), and stale FileCap regression (Scenario 6). Those are covered by the in-process test harness; the crash scenarios (A and B) require QEMU reboot.

---

## Q8 — 3D virtio completion-ordering fix and 3E FLUSH burden

**What the 3D fix established:** virtio-blk FLUSH (`VIRTIO_BLK_T_FLUSH`) completion is observed before `device.flush()` returns `Ok(())`. The caller (now `drain_pass` in the 3D cache) can trust that `device.flush()` is synchronous: it returns only after the virtio ring reports completion.

**New evidence burden for 3E:**

3E must verify the full FLUSH chain, not just that `device.flush()` was called:

1. **`VIRTIO_BLK_T_FLUSH` command counter advances** — the virtio driver's stats (if exposed) must show a new FLUSH command was submitted to the virtio ring during a dirty fsync.
2. **`CacheStats.flushes` advances after dirty fsync** — `cache.debug_stats().flushes` must increment. This confirms `flush_device_with_retry` returned `Ok(())` AND the completion check inside lock succeeded.
3. **`CacheStats.flush_required == false` after fsync** — confirms the FLUSH was accounted for and cleared.
4. **`CacheStats.dirty_blocks == 0` after fsync** — confirms all dirty blocks were written before the FLUSH.

**The 3D ordering fix means** that if `CacheStats.flushes` increments, the FLUSH completion was observed. Jarvis does NOT need to probe the virtio ring directly — the cache counter is sufficient evidence that the completion-ordering fix is working correctly.

**One wrinkle:** `device.supports_flush()` is checked twice in `drain_pass` (before calling `flush_device_with_retry` and inside the counter-update lock). If both checks agree (which they always will — the device capability doesn't change at runtime), the counter increments correctly. Jarvis should assert `supports_flush() == true` in the test setup to confirm the QEMU virtio-blk device was configured with VIRTIO_BLK_F_FLUSH capability.

---

## Recommended Implementation Posture

**Implement as-is.** The `fsync_regular` stub is correct. Jarvis must:

1. Verify the stub is reachable from `sys_fsync_impl` via `file.fsync()` → `NeoInode::fsync()` → `state.fsync_regular()` — confirm this chain is complete and no arm returns early incorrectly.
2. Wire the 6 QEMU scenarios with the assertions specified in this note.
3. Add `debug_sys_fsync_for_test` to the test harness path under the `neo-test-phase3-neofs-3e` feature gate.
4. In the verification artifact: explicitly state mount-wide drain semantics, not per-file.

---

## Must-Fix Hazards Before 3E Can Close

| # | Hazard | Location | Fix required |
|---|---|---|---|
| H1 | **Stale verification claim** | Verification artifact | State "mount-wide drain, not per-file" explicitly. Failing to do so is a false durability claim. |
| H2 | **Clean-file test has pre-dirty mount** | Scenario 4 test setup | Ensure mount is clean (background worker has drained everything, FLUSH counter captured) before testing clean-file fsync. Otherwise the scenario proves nothing. |
| H3 | **Writeback hold required for Scenario B** | Scenario 2 test setup | Must call `debug_set_writeback_hold(true)` before the unsynced write, and crash before the hold is lifted. Otherwise the worker drains and the scenario is vacuous. |
| H4 | **supports_flush() assertion** | Scenario 3/5 | Assert `device.supports_flush() == true` in QEMU test setup. If FLUSH is not negotiated, `CacheStats.flushes` won't increment and the FLUSH evidence is absent. |

---

## Acceptable Simplifications for 3E

- **Mount-wide dirty drain** — correct, honest, performant enough for 3E scale
- **Conservative entries lock serialization** — serializes with all CoW writes, not just writes to the fsynced file; safe, simple
- **No intent log** — crash recovery relies on CoW superblock rotation, not journal; correct for NeoFS's current design
- **Duplicate FLUSH from concurrent worker + fsync** — wasteful but not incorrect; `drain_all`'s loop handles this
- **`validate_current` reads from cache, not device** — acceptable because entries lock prevents any CoW write from dirtying new blocks during the check

---

## Unacceptable Shortcuts

- **Claiming per-file durability** without per-file dirty attribution in the cache — this is a false claim
- **Removing the `drain_all` loop** (replacing with a single-pass check) — breaks the worker race compensation
- **Skipping `validate_current()` before durability work** — stale FileCaps must be rejected before fsync drains the mount
- **Returning success before `device.flush()` completes** — `flush_device_with_retry` must synchronously wait for virtio completion
- **Calling `drain_all` without holding the entries lock** — concurrent CoW writes could add new dirty blocks between drain completing and return, invalidating the durability guarantee
- **Using `debug_set_writeback_hold` as a substitute for `pause_worker`** in the fsync path — writeback hold is a test-only control; production fsync uses `pause_worker` via `drain_all`

---

## Required Scenario/Evidence Matrix

| Scenario | What to exercise | What to assert |
|---|---|---|
| `phase3_neofs_fsync_dirty_persists_after_crash` | Write → fsync → QEMU hard halt → reboot | File present, content hash correct, mount succeeds |
| `phase3_neofs_unfsynced_dirty_may_be_lost` | Writeback held → write → no fsync → halt → reboot | Marker file intact; second file absent or old-state; no corrupt/torn state |
| `phase3_neofs_fsync_issues_flush` | Dirty write → fsync | `dirty_blocks == 0`, `flush_required == false`, `flushes > pre_fsync_flushes`, `supports_flush() == true` |
| `phase3_neofs_fsync_clean_no_extra_flush` | Mount fully clean → open existing file → fsync | `dirty_blocks == 0` before AND after; `flushes` counter unchanged; `drained_blocks == 0` |
| `phase3_neofs_fsync_coordinates_with_writeback_gc` | Dirty state + background worker running → concurrent fsync | fsync returns; `dirty_blocks == 0`; GC free map consistent with live set |
| `phase3_neofs_fsync_filecap_stale_regression` | Open file → write (new generation) → fsync old handle | Old handle returns ESTALE before durability work; refreshed handle fsyncs successfully |

**Regression gates:** run the full Jarvis §6 list. Pay special attention to `phase3_neofs_filecap_generation_stale` (must still pass — stale check behavior must not be weakened by 3E).

---

## Mount-Wide Drain vs Per-File Drain: Explicit Recommendation

**Use mount-wide drain for 3E. Per-file drain is Phase 3F+ work.**

Mount-wide drain is correct because NeoFS CoW makes all files share the same superblock publication chain. Per-file durability cannot be stronger than mount-wide durability without a separate stable-storage anchor per file (e.g., per-file journal entries or CAS content addressing).

Per-file drain would require the cache to track dirty blocks by inode identity. This is a significant cache redesign. The return is that `fsync(fd_A)` no longer drains `fd_B`'s dirty blocks. For 3E scale (single-mount, small write workloads), the over-drain is not a problem.

Deferred: per-file dirty attribution, per-file fsync barriers, intent log.

---

## Blocker Assessment

**No blocker.** The 3D substrate is sufficient for an honest BUILD-3E close.

The `fsync_regular` implementation in `inode.rs` (already present in the 3D changes) uses the correct approach: entries lock for namespace serialization, `drain_all` for cache drain and FLUSH. The `validate_current` stale check in `OpenFile::fsync` is correct and directory-anchored. The syscall dispatch at `SYS_FSYNC = 15` is wired.

Jarvis's remaining work is: test scenario wiring, verification artifact, and confirming the six scenario assertions pass. No new kernel primitives are required.

If Jarvis discovers during scenario wiring that `device.supports_flush()` returns false in the test environment (QEMU not configured with VIRTIO_BLK_F_FLUSH), that would be a test-infrastructure issue, not a kernel blocker. The kernel code correctly handles non-flush-capable devices (still drains dirty blocks, just doesn't increment the FLUSH counter).

---

*Cerebro challenge complete. This note is a sidecar — it does not block Jarvis from proceeding.*
