# Research: Phase 3 BUILD-3 Structural Challenge

| | |
|---|---|
| **Date** | 2026-05-14 |
| **Author** | Cerebro structural-challenge artifact |
| **Type** | Research artifact (informational; not normative). Structural challenge only. |
| **Dispatch** | `DISPATCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md` |
| **Authority** | `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md` |
| **No code, no normative-doc edits, no BUILD-3 implementation.** | |

---

## 1. Verdict: Narrow — not admit-in-full, not defer

**Narrow BUILD-3 to: kthread lifecycle + mark-sweep GC (physical-block-addressed, no CAS) + FLUSH negotiation + corrected `sys_fsync` semantics. Defer CAS/dedup explicitly to Phase 5.**

The admission question has a structural answer that pre-empts the measurement gate: **the bump allocator fills the disk permanently.** Orphaned blocks from every overwrite accumulate and are never reclaimed. This is not an optimization gap — it is a correctness gap for any filesystem use beyond bounded smoke tests. Mark-sweep GC is load-bearing infrastructure, not dedup. It must land before Phase 3 can be considered complete. Every other BUILD-3 candidate feature (write-back cache, FLUSH, CAS) can be separated from GC; GC cannot be separated from "NeoFS is usable for real workloads."

CAS/dedup is cleanly separable from GC and has no current demand signal. No Phase 3 workload generates measurable dedup. No Phase 5 planning has established CAS as a critical-path dependency. The measurement gate established by THINK v2 and Cerebro #1 remains unmet. CAS defers to Phase 5.

---

## 2. Recommended BUILD-3 Boundary

**BUILD-3 scope (admit now, no CAS):**

| Feature | Rationale for inclusion |
|---|---|
| kthread ownership/lifecycle primitive | Required by write-back GC worker; also needed by Phase 4 network stack |
| mark-sweep GC, physical-block-addressed | Disk longevity: orphans must be reclaimed before Phase 3 closes |
| FLUSH negotiation (`VIRTIO_BLK_F_FLUSH`) | Needed for correct `sys_fsync` durability; write-through alone is too slow |
| Write-back buffer cache | Performance; pairs with FLUSH; correct fsync implementation requires it |
| Corrected `sys_fsync` semantics | Currently documented no-op; must flush dirty blocks + FLUSH before returning |

**Deferred to Phase 5 (measurement-gated):**

| Feature | Defer condition |
|---|---|
| CAS hash-as-address index | Phase 5 ANSPEC-001 model-storage planning establishes critical-path dependency, OR measured dedup ratio > 5× on Phase 5 workloads |
| Dedup (DDT-style refcount) | Gate same as CAS index; operational cost known from ZFS prior art |
| Bloom filter front-end | Depends on CAS index |

**BUILD-3 split recommendation:**

BUILD-3a (prerequisite, no write-back kthread): FLUSH negotiation + corrected `sys_fsync` (synchronous dirty-block flush + FLUSH command). No buffer cache yet. Mark-sweep GC triggered on-demand (mount-time or explicit syscall). This immediately fixes disk longevity and fsync correctness without requiring kthread lifecycle work.

BUILD-3b (full BUILD-3): write-back buffer cache + periodic GC kthread worker, building on the kthread lifecycle primitive. This requires the per-mount kthread handle, stop/join semantics, and unmount-time teardown.

The split is optional — PM may choose to land them together if the kthread primitive is already addressed by the Jarvis prerequisite audit. The split is offered in case that audit reveals a kthread lifecycle gap that needs its own sub-commit.

---

## 3. kthread Lifecycle Recommendation

The PM decision correctly identifies the existing `sched::spawn("name", fn)` as insufficient: it returns no handle, has no cancellation, no join, and no per-mount scoping. The current virtio-blk completion worker is a global infinite loop — an anti-pattern for per-mount FS workers.

**Smallest safe per-mount kthread pattern:**

```rust
pub struct KthreadHandle {
    stop: Arc<AtomicBool>,
    done: Arc<IoRendezvous>,  // reuse the existing SC-3C wake primitive
}

impl KthreadHandle {
    pub fn spawn(name: &'static str, work: impl FnOnce(Arc<AtomicBool>) + Send + 'static) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::new(IoRendezvous::new());
        let stop_clone = stop.clone();
        let done_clone = done.clone();
        sched::spawn(name, move || {
            work(stop_clone);
            done_clone.signal();
        });
        Self { stop, done }
    }

    /// Signal stop and block until the worker acknowledges.
    pub fn stop_and_join(self) {
        self.stop.store(true, Ordering::Release);
        self.done.wait(); // uses SC-3C IPI cross-CPU wake — no new primitive
    }
}
```

**Key properties:**
- Reuses SC-3C IPI cross-CPU wake (`IoRendezvous`) — no new wakeup primitive
- Per-mount: each NeoFS mount creates its own `KthreadHandle`
- Stop signal is advisory: the worker loop checks `stop_flag` at natural quiescence points (between GC iterations, not mid-block-write)
- Unmount calls `handle.stop_and_join()` — blocks until worker drains

**Phase 4 forward-compat:** the `KthreadHandle` pattern generalizes to network-stack timer and receive-drain workers. Introducing it in BUILD-3 pays the Phase 4 bill early. This is intentional.

**Lifecycle rule (unmount):**
1. Mark the NeoFS instance as `UNMOUNTING` (atomic flag)
2. Reject any new file opens with `EBUSY`
3. Wait for all in-flight file operations to drain (existing IoRendezvous discipline)
4. Signal `KthreadHandle.stop`
5. `stop_and_join()` — worker runs to quiescence, flushes pending dirty pages, issues FLUSH
6. Release the mount table entry

Step 5 is the critical ordering invariant: the worker must not be killed mid-dirty-flush. The stop flag is advisory and the worker must honor it only when it reaches a clean checkpoint.

---

## 4. FLUSH/Cache/fsync Ordering Recommendation

### 4.1 Can FLUSH/cache land without CAS? Yes, cleanly.

FLUSH is a block-device command. Write-back cache is a page-level structure. Neither has any dependency on content-hash lookup, CAS index, or dedup table. The separation is clean.

### 4.2 Known pitfalls moving from write-through CoW to write-back

**Pitfall 1 — fsync must commit the full tree-path to root, not just the file's data.** In write-through, fsync is a no-op because all writes are already durable. In write-back, `sys_fsync(fd)` must durably commit: (a) the file's dirty data blocks, (b) the updated inode, (c) all ancestor directory inodes up to the root, (d) the new superblock, (e) then FLUSH. Committing only the data blocks without the superblock chain means the data is on disk but unreachable after a crash.

**Pitfall 2 — cache eviction ordering breaks CoW atomicity.** If the buffer cache evicts a dirty data block before the inode and superblock have been committed, the block is physically on disk but orphaned (no superblock references it). A crash at this point means the write is lost without the superblock's commit barrier. Fix: implement "write-ordering" tracking — data blocks can be evicted only after the superblock that records their inode has been committed. Mark dirty pages with a "superblock generation they belong to" and hold them until that generation is committed.

**Pitfall 3 — FLUSH placement.** FLUSH must be issued AFTER ALL three superblock copies have been written, not after each individual copy. If FLUSH fires after copy 0 but before copy 1, a crash leaves copy 1 in the device's volatile write cache, possibly at a different (newer) generation than copy 0. Mount would select copy 1 as the "highest valid generation" but it may be incomplete. Fix: write all superblock copies, then one FLUSH, then confirm completion before declaring the transaction committed.

**Pitfall 4 — `next_free_block` and tentative allocations.** In write-through, `next_free_block` advances only at superblock commit time (synchronous). In write-back, block allocation is tentative — the allocator advances `next_free_block` in memory, but if the superblock commit fails, those "allocated" blocks are free again. Fix: treat `next_free_block` as an in-memory cursor during dirty operation; only when the superblock commits does the new `next_free_block` become permanent. A crash recovers the old `next_free_block` from the valid superblock.

**Pitfall 5 — dirty metadata amplification.** In a CoW FS, overwriting a file dirtied the entire path: data → inode → all ancestor directory inodes → superblock. With many concurrent writes, dirty metadata can accumulate faster than the write-back flush can drain. Fix: batch superblock commits (one commit per N dirty files, or per time interval) and ensure the commit flushes all dirty metadata in the tree, not just the specific file's path.

### 4.3 Recommended write-ordering invariant for BUILD-3

For each superblock commit transaction:
```
1. Write all dirty data blocks (physical blocks for changed file content)
2. Write all dirty inode blocks (changed file inodes)
3. Write all dirty ancestor directory inode blocks
4. Write new superblock copies (all three, simultaneously)
5. FLUSH command → wait for completion
6. Mark the transaction committed; advance next_free_block permanently
```

Steps 1–3 may be interleaved but must complete before step 4. Step 5 must come after all of step 4. This is the Btrfs/ZFS write-ordering invariant confirmed by Cerebro #1 prior art.

---

## 5. GC Recommendation

### 5.1 Mark-sweep for physical-block-addressed CoW NeoFS

NeoFS v1 is physical-block-addressed. Every inode has `direct_blocks[8]` containing physical block addresses. The superblock has `root_inode_block`. This makes mark-sweep trivial:

**Mark phase (read-only, crash-safe):**
1. Read the valid superblock → get `root_inode_block`
2. Read the root inode (Directory kind) → mark block as live
3. For each `direct_blocks[i]` in the root inode → mark directory data blocks as live
4. For each directory entry in data blocks → get `child_block` → mark child inode block as live → recurse
5. For each file inode (Regular/Model kind) → mark its `direct_blocks[i]` as live
6. Result: a bitmap of live blocks from 0 to `total_blocks - 1`

**Sweep phase (one atomic superblock commit):**
1. Scan the bitmap: blocks below `next_free_block` not marked live are reclaimable
2. Compact the free-block accounting: write a new free-block bitmap or update `next_free_block` to the first unallocated block
3. Write a new superblock with the post-GC free-space state and an incremented generation
4. FLUSH

**What must be atomic:** the transition from old free-space state to new. This is guaranteed by the superblock commit — if GC crashes before the new superblock commits, the old superblock remains valid and the next boot can re-run GC from scratch (the mark phase is read-only and idempotent).

**What can be rebuild-only (no persistent GC state):** the live-set bitmap is computed fresh every GC run. No persistent GC metadata. The sweep result (new free-space accounting) is the only output, written via the normal superblock commit.

**Free-space accounting upgrade:** the post-GC allocator should upgrade from the BUILD-2 bump pointer to a simple free-block bitmap. The bitmap replaces `next_free_block` in the superblock or lives in a dedicated block range. After GC, allocations use `first_free_bit` scan in the bitmap (O(blocks/64) worst case, acceptable for BUILD-3 scale). BUILD-4+ can optimize to a B-tree-backed free list.

### 5.2 GC must not run concurrently with write operations

The mark phase reads the superblock and follows all reachable inodes. A concurrent write that creates a new inode referencing newly-allocated blocks could race with the mark phase: if the new inode is written after the mark phase passes that region, its data blocks would be swept as garbage.

Fix for BUILD-3: **GC runs at unmount time or at mount time (before any write operations begin).** This avoids concurrent-access complexity. If an on-demand GC syscall is added (e.g., `sys_gc()` for testing), it must acquire a write quiescence — no new file operations during the mark phase. The simplest implementation: `sys_gc()` freezes the mount (rejects new writes), runs GC, commits the new superblock, then resumes. For BUILD-3 test scenarios, this is sufficient.

### 5.3 GC + write-back interaction

If BUILD-3b adds a write-back buffer cache, GC must be checkpoint-aware: it must only run when the buffer cache is clean (all dirty pages committed). A dirty-buffer-at-GC-time scenario would mark dirty-but-not-yet-committed blocks as garbage (they're not yet reachable from the superblock). Fix: GC triggers a full flush-and-commit of the buffer cache before starting the mark phase.

---

## 6. CAS/Dedup Measurement Gate

**The crisp admit/defer rule:**

CAS is admitted to Phase 3 (or Phase 5) when the following condition is met:

> **Phase 5 ANSPEC-001 model-storage planning formally establishes that Phase 5's model dedup requirement cannot be satisfied by physical-block-addressed NeoFS plus a higher-level content-hash lookup (e.g., a userspace model registry),** OR a measured Phase 3/5 workload shows dedup ratio > 5× with DDT memory cost within the system's RAM budget.

Neither condition is currently met. No Phase 5 model-storage planning has been filed. No Phase 3 workloads exist. **CAS defers to Phase 5.**

**Why not now:**
- No demand signal. The BUILD-2 workload is a handful of test files; there is no model-dedup workload to measure.
- The operational cost is known and severe (ZFS dedup post-mortem: 1-3 GiB RAM/TB, CPU cost on every read+write). This cost must be justified by a specific workload benefit before implementation.
- Content-hash fields are already reserved in every inode. The format is forward-compatible. Deferral costs nothing on-disk.
- Phase 5 may decide model dedup belongs at a higher layer (ANSPEC-001 model registry, not kernel FS dedup). If so, CAS never needs to be a Phase 3 artifact.

**If CAS is eventually admitted:** the THINK v2 "Venti-style bloom filter plus on-disk hash table" remains the right first shape. It is the lowest-RAM, lowest-implementation-complexity CAS index with proven production lineage (Plan 9 Venti). The smaller alternative — a linear scan at create time — is only viable for a handful of model files; it does not generalize. Admit Venti-style or don't admit CAS.

---

## 7. Top Risks for Jarvis

### Risk 1 — GC races write operations (highest priority)
If GC runs concurrently with a write that allocates new blocks not yet reachable from the superblock, the sweep phase will reclaim those in-flight blocks. **Mitigation:** BUILD-3 GC runs only at write quiescence (mount-time, unmount-time, or after a write-quiescence fence). Do not implement concurrent GC in BUILD-3.

### Risk 2 — kthread stop during mid-dirty-flush
If the mount teardown calls `stop_and_join()` while the GC worker is in the middle of writing dirty pages, the filesystem may be left in a partially-flushed state. **Mitigation:** the stop signal is advisory; the worker checks it only at clean checkpoints (after completing a full flush-and-commit cycle). The unmount path waits for the worker's full clean exit via the `done` rendezvous, not a kill.

### Risk 3 — fsync commits only file data, not the full tree-path
If `sys_fsync` implementation flushes data blocks but not the ancestor inode chain and superblock, a crash after fsync leaves the data on disk but unreachable. The test would show data readable in the same boot but lost after reboot. **Mitigation:** the fsync implementation must commit the full tree-path from the fsynced file to the root, then issue FLUSH.

### Risk 4 — FLUSH issued before all superblock copies are written
A FLUSH after writing only the first superblock copy could leave the second and third copies in the device's volatile cache. A crash at that moment produces asymmetric superblock state. **Mitigation:** write all three superblock copies, then one FLUSH. The BUILD-3 verification must include a "FLUSH once after all copies" assertion in the commit path.

### Risk 5 — write-back cache evicts dirty pages before superblock commits
If the buffer cache evicts a dirty page (writes it to disk) before the superblock that references it is committed, the page is physically on disk but logically orphaned. **Mitigation:** implement write-ordering: dirty pages carry a "required commit generation" field; they cannot be written to disk before the superblock of that generation is committed. This is the key correctness invariant for write-back in a CoW FS.

---

## 8. Dispatch Language PM Can Give Jarvis

**If PM chooses BUILD-3 as NARROWED (recommended):**

```
Phase 3 BUILD-3 is authorized. Scope:

Required (no CAS):
- kthread ownership/lifecycle primitive (`KthreadHandle` with stop/join; reuses SC-3C IoRendezvous)
- mark-sweep GC, physical-block-addressed (no CAS index, no content-hash lookup)
- write-back buffer cache
- virtio-blk FLUSH negotiation (`VIRTIO_BLK_F_FLUSH` and `VIRTIO_BLK_T_FLUSH`)
- corrected `sys_fsync` semantics: flush dirty blocks → commit ancestor inode chain → commit superblock copies → FLUSH → return

Deferred to Phase 5:
- CAS hash-as-address index
- dedup / DDT-style refcount
- bloom filter

GC constraint: BUILD-3 GC runs at write quiescence only (mount-time, unmount-time, or sys_gc() with quiescence fence). No concurrent GC with active writes.

fsync constraint: sys_fsync must commit the full tree-path from the fsynced file to the root before FLUSH. Not just the file's data blocks.

FLUSH constraint: write all three superblock copies, then one FLUSH, then declare the transaction committed.

Do not start CAS, AHCI/NVMe, foreign FS, rootfs pivot, retype, mmap/exec, or broad POSIX work.
```

**If PM chooses to split BUILD-3:**

```
Phase 3 BUILD-3a is authorized. Scope:

- mark-sweep GC, physical-block-addressed, at quiescence (no kthread, no concurrent GC)
- virtio-blk FLUSH negotiation
- corrected sys_fsync (synchronous dirty-block flush + FLUSH; no buffer cache)

BUILD-3b (kthread + write-back cache) follows after BUILD-3a closes.
CAS/dedup remains deferred to Phase 5.
```

---

## 9. Time-Budget Actual

~35 minutes. No external research agents. Synthesis from ADR-021, THINK v2, BUILD-2 close report, PM decision, and prior Cerebro research artifacts (#1, #2, #3, NeoFS format).

---

**End of structural challenge artifact.**

*Verdict: NARROW. Admit kthread lifecycle + mark-sweep GC (physical-block-addressed, no CAS) + FLUSH + write-back + corrected sys_fsync. Defer CAS to Phase 5. The admit/defer rule is: CAS admitted only when Phase 5 model-storage planning establishes it as critical path OR measured dedup ratio > 5×. Neither condition is currently met. Mark-sweep GC is not optional — the bump allocator fills disks permanently; GC is a correctness requirement for usable NeoFS. FLUSH and write-back are independent of CAS and should be included in BUILD-3. kthread stop must be advisory (checkpoint-based), not forced mid-dirty-flush. FLUSH must come after all three superblock copies, not after the first.*
