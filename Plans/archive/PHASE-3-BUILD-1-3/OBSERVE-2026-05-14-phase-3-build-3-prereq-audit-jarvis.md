# OBSERVE: Phase 3 BUILD-3 prerequisite audit

Date: 2026-05-14
Lane: Jarvis read-only audit
Base observed: main at 096414c4da9f2294c35bcfc9e0c459a99c50078b
Dispatch: Plans/DISPATCH-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md

## 1. Verdict

BUILD-3 can be planned safely, but not as a single CAS/write-back/GC bundle.
The safe first implementation slice is a scheduler-owned kthread lifecycle
substrate. Current `sched::spawn` is sufficient for immortal global kernel
workers, but it is not sufficient for FS-owned write-back or GC workers that
must be stopped, joined or detached deliberately, and torn down before a mount
or device reference is dropped.

The next slice after kthread lifecycle should be the virtio FLUSH transition
and block-layer durable-write surface. Buffer-cache/write-back and `sys_fsync`
durability should follow only after FLUSH can be negotiated and exercised.
Mark-sweep reachability is feasible over the current NeoFS tree, but actual
reclamation needs a free-space format addition. CAS/dedup is not admitted by
current evidence: the tree has content hashes, but no measured dedup source or
ratio gate exists in the repo.

Recommended first BUILD-3 sub-commit: kthread ownership/lifecycle only. No
write-back cache, FLUSH negotiation, GC, CAS, NeoFS format revision, mmap,
exec loading, AHCI/NVMe, foreign FS, retype, or rootfs work should be included.

## 2. Current substrate map with file/function references

Scheduler and kthread-adjacent substrate:

- `kernel/src/sched/process.rs:41` defines `KERNEL_PID = ProcessId(0)`.
- `kernel/src/sched/thread.rs:149` implements `Thread::new(name, entry)`.
  It creates a kernel thread with `pid: KERNEL_PID`, no user address space,
  `ThreadState::Ready`, and a kernel stack.
- `kernel/src/sched/scheduler.rs:231` implements `sched::spawn(name, entry)`.
  It allocates a `Box<Thread>` with `Thread::new` and pushes it onto the
  current CPU ready queue. It returns `()`.
- `kernel/src/sched/scheduler.rs:551` implements `sched::exit() -> !`.
  It moves the current thread to the zombie queue and context-switches away.
- `kernel/src/sched/scheduler.rs:1039` implements `sched::reap()`. The BSP
  drains zombie queues and calls `Thread::free`; `process::on_thread_reap`
  ignores `KERNEL_PID`.
- `kernel/src/sched/scheduler.rs:74` defines `ThreadWakeToken`, the
  generation-checked scheduler token used by blocking I/O.
- `kernel/src/sched/scheduler.rs:96` defines the fixed parked-token table.
- `kernel/src/sched/scheduler.rs:700` implements
  `block_current_with_token_and_guard`, the atomic park-and-release primitive.
- `kernel/src/sched/scheduler.rs:804` implements `wake_thread_token`.
- `kernel/src/sched/mod.rs` exports the scheduler primitives, including
  `spawn`, `ThreadWakeToken`, and token wake/block helpers.

Storage worker call site:

- `kernel/src/drivers/block/virtio_blk.rs:765` implements
  `ensure_completion_worker()`.
- `kernel/src/drivers/block/virtio_blk.rs:770` calls
  `sched::spawn("virtio-blk-completion", virtio_blk_completion_worker)`.
- `kernel/src/drivers/block/virtio_blk.rs:774` implements the completion
  worker as an infinite poll/yield loop.

Block device and virtio-blk substrate:

- `kernel/src/drivers/block/mod.rs:25` defines `BlockOp::{Read, Write}` only.
- `kernel/src/drivers/block/mod.rs:60` defines `BlockDevice::submit`,
  `read_block`, and `write_block`; there is no flush operation or capability
  query.
- `kernel/src/drivers/block/virtio_blk.rs:31` defines
  `VIRTIO_BLK_F_FLUSH`.
- `kernel/src/drivers/block/virtio_blk.rs:32` defines
  `VIRTIO_BLK_F_CONFIG_WCE`.
- `kernel/src/drivers/block/virtio_blk.rs:64` and `:65` define request types
  for IN and OUT only.
- `kernel/src/drivers/block/virtio_blk.rs:199` records whether FLUSH was
  offered by the device.
- `kernel/src/drivers/block/virtio_blk.rs:221` negotiates only
  `VIRTIO_F_VERSION_1`, so FLUSH is intentionally not negotiated today.
- `kernel/src/drivers/block/virtio_blk.rs:274` logs
  `F_FLUSH_negotiated=false F_CONFIG_WCE_negotiated=false`.
- `kernel/src/drivers/block/virtio_blk.rs:299` reports probe snapshot evidence
  with `flush_negotiated: false`.
- `kernel/src/drivers/block/virtio_blk.rs:412`,
  `:431`, and `:488` implement request submission and wait paths.
- `kernel/src/drivers/block/virtio_blk.rs:791` maps `BlockOp` to virtio
  request headers, currently read/write only.

NeoFS/VFS syscall substrate:

- `kernel/src/fs/mod.rs:92` defines the `Inode` trait with `read_at`,
  `write_at`, and generation/type helpers. There is no fsync or dirty-cache
  hook.
- `kernel/src/fs/mod.rs:124` validates stale file handles using the current
  reachable inode generation.
- `kernel/src/fs/mod.rs:135` performs type-first executable policy checks.
- `kernel/src/fs/mod.rs:161` and `:175` route file read/write through the
  inode trait after rights/type/generation validation.
- `kernel/src/arch/x86_64/syscall.rs:692` implements read syscall routing.
- `kernel/src/arch/x86_64/syscall.rs:715` implements file write syscall
  routing while preserving serial stdout compatibility.
- `kernel/src/arch/x86_64/syscall.rs:753` implements `sys_fsync_impl` as a
  BUILD-2 no-op that validates the handle and performs no durability work.
- `userland/libneo/src/lib.rs:45` defines `SYS_FSYNC = 15`; `:264` exposes
  the user wrapper.

NeoFS storage paths:

- `kernel/src/fs/neofs/mount.rs:181` selects the active superblock copy.
- `kernel/src/fs/neofs/mount.rs:236` validates a mount candidate.
- `kernel/src/fs/neofs/mount.rs:262` validates children reachable from root.
- `kernel/src/fs/neofs/inode.rs:28` defines `NeoFsState`; it has `device`,
  writable flag, metadata, root, and directory entries, but no free map,
  buffer cache, dirty list, worker handles, or CAS index.
- `kernel/src/fs/neofs/inode.rs:90` creates regular files.
- `kernel/src/fs/neofs/inode.rs:133` implements `commit_regular_write`.
  Current writes are synchronous CoW writes through content blocks, inode
  block, root entry update, root tree write, and superblock copies.
- `kernel/src/fs/neofs/inode.rs:315` reserves fresh inode blocks and
  generations.
- `kernel/src/fs/neofs/inode.rs:327` reserves fresh data/root blocks by
  advancing `next_free_block`.
- `kernel/src/fs/neofs/inode.rs:336` commits root entries.
- `kernel/src/fs/neofs/inode.rs:350` writes the root tree before publishing
  superblock copies.
- `kernel/src/fs/neofs/inode.rs:523` reads and verifies logical content.
- `kernel/src/fs/neofs/inode.rs:534` reads direct-block logical content.
- `kernel/src/fs/neofs/inode.rs:569` and `:578` are the narrow FS-block
  read/write helpers.
- `kernel/src/fs/neofs/inode.rs:636` implements logical-block write assembly.
- `kernel/src/fs/neofs/inode.rs:705` writes content blocks.
- `kernel/src/fs/neofs/inode.rs:722` allocates one new block from
  `superblock.next_free_block`.
- `kernel/src/fs/neofs/inode.rs:739` writes superblock copies.

Format and policy references:

- `kernel/src/fs/neofs/format.rs` defines `Superblock` with
  `next_free_block`, but no free-list, free-bitmap, or CAS-index root.
- `kernel/src/fs/neofs/format.rs` defines direct blocks, an unsupported
  `indirect_block`, content hash, and directory-entry count in `InodeRecord`.
- `docs/specs/neofs-format.md:88` through `:91` state that BUILD-2 has no
  free list/tree/bitmap and that orphaned CoW blocks remain until BUILD-3 GC.
- `docs/specs/neofs-format.md:122` through `:127` document direct blocks,
  the reserved indirect block, content hash, and directory-entry count.
- `docs/ADR-021-phase-3-design-substrate.md:474` through `:485` list BUILD-3
  candidates: CAS if admitted, mark-sweep, FLUSH, write-back cache, kthread
  primitive, and tests.
- `docs/ADR-021-phase-3-design-substrate.md:512` through `:516` explicitly
  call for a pre-BUILD-3 kthread audit and likely `kernel/src/sched/kthread.rs`
  lifecycle substrate.

## 3. Kthread gap analysis

Current status:

- `sched::spawn` creates a kernel thread but returns no identity or handle.
- There is no owner-visible stop flag, cancellation request, join, detach, or
  completion state.
- `sched::exit` can retire a kernel thread, and `sched::reap` can free the
  zombie `Thread`, but no subsystem can observe that completion directly.
- The virtio-blk completion worker is an immortal global worker. That is a
  reasonable use of the current primitive because it owns no per-mount
  teardown contract and intentionally runs forever.

Why this is insufficient for BUILD-3:

- A NeoFS write-back worker would be owned by a mount or filesystem instance.
  Unmount must request stop, wake the worker if it is parked, wait for dirty
  state to drain or be abandoned under a defined rule, and only then drop the
  mount/device/cache state.
- A GC worker has the same ownership problem, plus it may hold references into
  mount metadata and future free-space state. Detached unbounded execution
  after unmount would be a lifetime bug.
- Returning `Arc<Thread>` would repeat the risk avoided in BUILD-1 wake-token
  work. The scheduler should continue to own parked `Box<Thread>` storage.

Smallest recommended substrate:

- Add a scheduler-owned kthread control object, likely in
  `kernel/src/sched/kthread.rs`, without exposing `Thread` ownership.
- Return a `KthreadHandle` containing a stable kthread id, a stop-request
  control bit, and enough scheduler-owned completion identity for join.
- Provide `spawn_owned(name, owner, entry, ctx)` or an equivalent narrow API
  where the entry receives a stop token/context and returns through a normal
  kthread exit path.
- Provide `request_stop(handle)`, `join(handle)`, and `detach(handle)`.
- Preserve detached support for global device workers like virtio completion,
  but make detached status explicit.
- Require workers that may block to use an owned wake token or wait primitive
  that can be woken by `request_stop`.
- Extend reap/completion accounting so join observes the kthread id finishing;
  do not require user processes to own kernel threads.
- Define panic/exit behavior: panic should mark the kthread failed and wake
  joiners; normal exit should mark completed. The mount owner decides whether
  worker failure is fatal to future writes.

Unmount teardown rule for NeoFS BUILD-3:

- Filesystem state stores worker handles, not raw `Thread` pointers.
- Unmount requests stop for write-back and GC workers.
- Stop wakes any worker parked on cache/queue state.
- Write-back worker must either flush dirty entries or report a defined
  shutdown failure before the mount is dropped.
- Join must complete before mount-owned cache, device, or free-space state is
  destroyed.

Stop condition:

- If implementing this requires broad scheduler/process invariant changes or
  `Arc<Thread>` ownership, stop and file a scheduler substrate design blocker.

## 4. FLUSH/write-back/fsync transition map

Virtio FLUSH transition:

- Add a block-layer durable-write surface before touching NeoFS write-back.
  The narrow API could be `BlockDevice::flush()` plus a capability query, or
  `BlockOp::Flush` if the existing request path can represent a no-data
  command cleanly.
- Extend `kernel/src/drivers/block/mod.rs` so callers can discover whether a
  device supports flush and can request one without constructing virtio-specific
  headers.
- In `kernel/src/drivers/block/virtio_blk.rs`, negotiate
  `VIRTIO_BLK_F_FLUSH` when offered. Keep `VIRTIO_BLK_F_CONFIG_WCE` not
  negotiated unless a later dispatch explicitly changes write-cache policy.
- Add the virtio-blk FLUSH request type in the same request-header mapping
  area that currently handles IN/OUT.
- Issue FLUSH through the existing descriptor submit/completion path with no
  data buffer, or another shape matching the virtio-blk spec and the local
  descriptor builder.
- Preserve the existing descriptor-publish and notification-suppression memory
  barrier split. The aarch64 lane must remain real runtime evidence.

Current tests that must change when FLUSH is enabled:

- `kernel/src/test_harness.rs:4730` through `:4918` currently include the
  virtio-blk probe/raw-RW/FLUSH-offered gates.
- The current `phase3_virtio_blk_flush_offered_gate` expects FLUSH offered but
  not negotiated, and expects CONFIG_WCE not negotiated.
- BUILD-3 FLUSH tests should invert only the FLUSH part: offered and negotiated
  must both be true; CONFIG_WCE should remain not negotiated unless separately
  authorized.
- Add evidence that an actual FLUSH command completes successfully, not just a
  feature-bit negotiation log.
- Keep a negative path for CONFIG_WCE so write-back-cache semantics are not
  silently changed.

Buffer-cache insertion point:

- The smallest cache seam is NeoFS FS-block granularity, not raw 512-byte block
  sectors. The likely insertion points are `read_fs_block` and `write_fs_block`
  in `kernel/src/fs/neofs/inode.rs`.
- A cache at this level can preserve the existing on-disk BUILD-2 format while
  caching 4 KiB NeoFS blocks and delegating sector splitting to the current
  block helpers.
- `commit_regular_write` should remain the ordering authority at first. It
  already knows the CoW sequence: content blocks, inode block, root tree, then
  superblock copies.
- Dirty tracking should identify data, inode, root-directory, root-inode, and
  superblock blocks separately. Superblock publication must not be reported
  durable before all prerequisite dirty blocks have completed write-back and
  the block device FLUSH has completed.
- A write-back worker should consume dirty entries from mount-owned cache
  state and use the kthread handle substrate above. It should not introduce a
  format change by itself.

`sys_fsync` transition:

- Current BUILD-2 behavior is intentionally a no-op:
  `kernel/src/arch/x86_64/syscall.rs:753` validates the file handle and returns
  success without issuing storage commands.
- `kernel/src/test_harness.rs:5919` verifies this as
  `phase3_neofs_fsync_noop_write_through`, including no generation mutation and
  no flush command.
- BUILD-3 should change `fsync` only after the cache and block FLUSH layers
  exist. The narrow contract should be: for a regular NeoFS file, force dirty
  data reachable from that file, force required metadata up through the active
  superblock publication if the file's generation changed, issue block-device
  FLUSH, and return storage error on failure.
- Docs to update when authorized: `docs/phases/PHASE-3.md`,
  `docs/specs/neofs-format.md` if ordering semantics become normative there,
  and syscall/libneo notes wherever BUILD-2 no-op behavior is documented.
- Tests to replace/add: the BUILD-2 no-op fsync scenario should become a
  durable fsync scenario, with a crash/reboot split before fsync and after
  fsync. The post-fsync path must show the reopened file contents and metadata
  are present after reboot.

Stop condition:

- If FLUSH negotiation invalidates BUILD-1/BUILD-2 virtio evidence or cannot
  be exercised as a real QEMU runtime command, stop and file a FLUSH blocker.

## 5. GC and CAS/dedup admission analysis

Mark-sweep feasibility:

- Current reachable walk is feasible and small. Start from the selected
  superblock's root inode block. Mark the root inode block. Mark each direct
  block used by the root directory. Parse root directory entries. For each
  reachable child, mark the child inode block and the child's direct content
  blocks.
- Current NeoFS only supports root-level names through the existing path
  normalization and mount validation. Directory creation is not present, and
  indirect blocks are reserved/unsupported, so the live graph is not deep.
- The walker should still be written as a format-aware visitor so later
  directory nesting or indirect blocks can be added without rewriting GC.

Reclamation gap:

- Actual orphan reclamation is not possible with the current persisted format.
  The superblock has `next_free_block`, but no persisted free bitmap, free
  extent tree, reusable allocation cursor, or free-space generation.
- BUILD-2 CoW intentionally leaks obsolete blocks until BUILD-3. A mark-only
  scan can report garbage but cannot safely reallocate it after reboot.
- Adding reclamation will require a NeoFS format revision or a compatible
  format extension. Current parser behavior treats reserved superblock bytes as
  zero-only, so using reserved fields is still a parser/spec change.
- The smallest useful GC format change is likely a free bitmap or extent map
  root plus generation/checksum coverage. This should be its own BUILD-3
  sub-commit after write-back/fsync durability is settled.

CAS/dedup admission:

- Current meaningful evidence is insufficient. NeoFS records content hashes,
  and ADR-021 discusses hash-addressed content, but the repo does not contain
  a representative measured workload, duplicate-block counter, or dedup-ratio
  gate that would justify CAS as the next implementation slice.
- BUILD-3 should not start with CAS. The no-code admission path is to measure
  representative NeoFS images or Phase 5 model artifacts by fixed-size content
  hash, then report duplicate bytes, unique bytes, expected index memory, and
  write amplification impact.
- Synthetic mkfs images can sanity-check tooling, but should not be the
  admission gate.
- If measurement does not show meaningful duplication or a Phase 5 critical
  path need, defer CAS and keep content hashes as verification metadata.

Stop condition:

- If GC requires a broad NeoFS migration instead of a narrow free-space format
  addition, stop and file a format-design blocker.
- If CAS has no measured dedup value, keep it out of BUILD-3 implementation
  until PM admits it explicitly.

## 6. Recommended BUILD-3 sub-commit sequence

1. BUILD-3A: kthread lifecycle substrate.
   - Add scheduler-owned kthread handles, stop request, join/detach, completion
     accounting, and panic/exit reporting.
   - Migrate only the virtio completion worker if useful to prove detached
     global-worker semantics; otherwise keep migration out of scope.
   - Runtime gate: worker starts, stop wakes a parked worker, join observes
     completion, detached worker remains valid, no `Arc<Thread>` migration.

2. BUILD-3B: block FLUSH transition.
   - Add block flush capability/API.
   - Negotiate `VIRTIO_BLK_F_FLUSH` when offered.
   - Keep `CONFIG_WCE` not negotiated unless separately authorized.
   - Runtime gate: real QEMU virtio FLUSH command completion, negative
     CONFIG_WCE evidence, x86 and aarch64 descriptor-barrier coverage.

3. BUILD-3C: in-memory NeoFS buffer cache plus write-back skeleton.
   - Insert FS-block cache at `read_fs_block`/`write_fs_block`.
   - Track dirty blocks by role.
   - Add write-back worker using kthread handles.
   - Keep on-disk format unchanged.
   - Runtime gate: read/write persistence remains equivalent to BUILD-2 under
     clean shutdown, and dirty cache drains on unmount.

4. BUILD-3D: durable `sys_fsync`.
   - Convert `SYS_FSYNC = 15` from BUILD-2 no-op to file durability boundary.
   - Force relevant dirty file data/metadata, publish required superblock
     state, issue block FLUSH, and return errors.
   - Runtime gate: crash/reboot before fsync may lose unflushed writes under
     documented rules; crash/reboot after fsync preserves them.

5. BUILD-3E: mark-sweep reachability and free-space format addition.
   - First implement read-only live-set scanner.
   - Then add narrow free-space persistence if PM authorizes a format revision.
   - Runtime gate: deterministic orphan creation and later reuse without
     corrupting active root selection.

6. BUILD-3F: CAS/dedup only if admitted by measurement.
   - Add measurement artifact first.
   - If admitted, add content index and crash-safe insertion order.
   - Runtime gate: measured duplicate workload uses fewer physical content
     blocks without losing file-generation stale-handle behavior.

Anti-scope for all BUILD-3 slices:

- No BUILD-4+ work.
- No AHCI/NVMe.
- No foreign filesystem.
- No rootfs boot dependency.
- No mmap/exec loading.
- No retype or broad typed-inode expansion.
- No FileCap grant/derive/revoke model.
- No broad POSIX expansion.
- No intent log unless a later PM dispatch moves it back into scope.

## 7. Stop conditions

- Stop if kthread lifecycle requires exposing `Arc<Thread>` or broad
  scheduler/process ownership changes.
- Stop if worker teardown cannot be expressed as stop/wake/join before mount
  state is dropped.
- Stop if FLUSH cannot be negotiated and completed in real QEMU runtime.
- Stop if the aarch64 barrier lane falls back to compile-only evidence.
- Stop if write-back cache makes `sys_fsync` durable before the syscall
  contract and tests are updated.
- Stop if a dirty superblock can be published before prerequisite data and
  metadata write-back are complete.
- Stop if GC needs a broad format migration instead of a narrow free-space
  addition.
- Stop if CAS admission has no measured dedup value or Phase 5 critical path.

## 8. Verification

Read-only audit only. No source code, tests, normative docs, branch merges, or
pushes were performed for this observe lane.

Required artifact check:

```text
git diff --check -- Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md
```

Result: PASS after artifact creation; command exited 0 with no output.

Artifact status: untracked unless PM separately authorizes staging/commit.
