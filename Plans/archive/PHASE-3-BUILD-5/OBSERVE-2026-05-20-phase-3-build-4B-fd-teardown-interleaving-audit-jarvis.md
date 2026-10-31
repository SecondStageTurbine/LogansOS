# OBSERVE - Phase 3 BUILD-4B FD-Teardown Interleaving Audit

Date: 2026-05-20
Agent: Jarvis
Status: CLOSED
Verdict: PROCEED-AS-DISPATCHED

## Substrate

- Dispatch substrate named: `0682d24 test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts`
- Audited workspace HEAD: `d1ec982321ccf2abb26d954769bff9af7f584d5b`
- Workspace note: the tree already contains BUILD-4B mount-table/live-handle code in `kernel/src/fs/mod.rs`, `kernel/src/arch/x86_64/syscall.rs`, `kernel/src/fs/neofs/mount.rs`, and `userland/libneo/src/lib.rs`. This audit therefore evaluates the actual local implementation shape as well as the dispatched model.
- RAG health checked: `http://127.0.0.1:8404/health` returned healthy with `ollama=true`, `qdrant=true`.
- Required path note: `kernel/src/cap/` does not exist in this checkout. Capability/derivation/grant behavior is implemented in `kernel/src/sched/process.rs` and IPC grant rendezvous code in `kernel/src/ipc/mod.rs`.

## Source Sweeps

Direct reads and sweeps covered:

- `kernel/src/sched/process.rs`: `Handle::File`, handle table, derive/revoke/grant, natural exit, forced kill, channel peer cleanup.
- `kernel/src/fs/mod.rs`: `OpenFile`, live-handle increment/decrement, mount table remove/busy check, root open.
- `kernel/src/arch/x86_64/syscall.rs`: sys_open/read/write/readdir/mount/umount/close/fsync and fd lookup.
- `kernel/src/ipc/mod.rs`: grant blocked states and process-exit endpoint cleanup.
- `kernel/src/sched/kthread.rs`: owned kthread handle, stop, join, detach lifecycle.
- `kernel/src/sched/scheduler.rs`: kill, zombify, reaper behavior.
- `kernel/src/fs/neofs/{mount.rs,inode.rs,cache.rs}`: NeoFS state ownership, writeback worker, cache drain, state drop.
- `kernel/src/mm/tlb_shootdown.rs`: existing synchronous shootdown/ACK precedent.

Sweeps:

- `rg -n "OpenFile|Handle::File|enum Handle|struct OpenFile|impl Drop.*OpenFile|drop\\(|force|kill|reap|exit" ...`
- `rg -n "Cap|capability|cap_id|Capability|CapRights|derive|grant|Handle::File|OpenFile|Arc<OpenFile>|\\.clone\\(\\)" kernel/src userland tools`
- `rg -n "Arc<OpenFile>|OpenFile::|OpenFile|Handle::File|file_lookup\\(|install_excluding|install\\(" kernel/src userland tools -g '!target'`

## Q1 - Where Handle::File Drops During Force Kill

`Handle::File` is a handle-table variant carrying `file: Arc<OpenFile>` at `kernel/src/sched/process.rs:241`. The handle table drains by moving all live handles into a `Vec<Handle>` at `kernel/src/sched/process.rs:383`.

Forced kill is synchronous in the killer syscall path:

- `process::terminate` claims `EXIT_STATE_EXITING_FORCED` and marks `exiting=true` at `kernel/src/sched/process.rs:922` and `kernel/src/sched/process.rs:934`.
- It yanks the ready thread via `scheduler::kill_thread_by_pid` at `kernel/src/sched/process.rs:936`.
- It removes the `Process` from `PROCESSES` before handle drops at `kernel/src/sched/process.rs:943`.
- It drains the handle table at `kernel/src/sched/process.rs:952`.
- It closes channel peers at `kernel/src/sched/process.rs:977`.
- It explicitly drops the drained handle vector at `kernel/src/sched/process.rs:988`.

Therefore the process-owned `Handle::File` references drop before `terminate` returns. The killed `Thread`/`Pmap` may remain in the zombie queue until reap, as noted at `kernel/src/sched/process.rs:984`, but fd-table file references are released synchronously by `drop(drained)`.

Natural exit is similar but reaper-driven: `scheduler::reap` frees each zombie and calls `process::on_thread_reap(pid)` at `kernel/src/sched/scheduler.rs:1108`, and `on_thread_reap` drains/drops handles at `kernel/src/sched/process.rs:862` and `kernel/src/sched/process.rs:881`.

## Q2 - Non-FD Arc<OpenFile> Holders

Observed durable holders:

- FD table entries: `Handle::File { file: Arc<OpenFile>, ... }` at `kernel/src/sched/process.rs:241`.
- Syscall-local fd lookup clones: `file_lookup` returns an `Arc<OpenFile>` cloned from the handle table at `kernel/src/arch/x86_64/syscall.rs:1014` and `kernel/src/arch/x86_64/syscall.rs:1022`. This is important: if a process loses its fd during teardown while another syscall already holds a local clone, `OpenFile::Drop` does not run until that syscall-local clone is gone.
- Kernel tests create and pass `Arc<OpenFile>` in `kernel/src/test_harness.rs`, but production paths do not store hidden `OpenFile` Arcs outside handles/syscall locals.

Candidates checked:

- In-flight syscalls on auxiliary threads: current invariant remains one thread per process. `process::terminate` says Phase 1.5b has one thread per process and Phase 2+ needs scheduler-side enumeration before expansion at `kernel/src/sched/process.rs:955`. ADR-013 also ratifies single-thread-per-process for current scope.
- IPC fds in transit: IPC grant state stores `GrantPayload { grantor_pid, source_cap_id, offered }`, not a `Handle` or `Arc<OpenFile>`, at `kernel/src/ipc/mod.rs:102`. Grant completion looks the source up by cap id later at `kernel/src/sched/process.rs:608`. No file Arc is parked in channel state.
- Capability derivation/grant chains: `Handle::File::rights()` maps to empty `CapRights` at `kernel/src/sched/process.rs:257`, and `Handle::File::with_cap` preserves file rights while ignoring generic `CapRights` at `kernel/src/sched/process.rs:284`. Current generic derive/grant cannot produce a file cap with DERIVE/GRANT meta rights. File handles are not currently grantable through IPC.
- Scheduler bookkeeping: scheduler queues hold `Box<Thread>`, not `OpenFile`. Ready kill moves a thread to zombies at `kernel/src/sched/scheduler.rs:1024`; blocked kill zombifies a parked `Box<Thread>` at `kernel/src/sched/scheduler.rs:866` or `kernel/src/sched/scheduler.rs:889`.
- DmaBuffer transitives: NeoFS block I/O helpers take `Arc<DmaBuffer>` for device operations at `kernel/src/fs/neofs/inode.rs:1070`, separate from `OpenFile`.
- NeoFS writeback: the worker captures `Arc<NeoFsBlockCache>`, not `Arc<OpenFile>`, at `kernel/src/fs/neofs/cache.rs:346`. NeoFS inodes hold `Arc<NeoFsState>` at `kernel/src/fs/neofs/inode.rs:656`, but that is mount state, not an `OpenFile`.

No production path was found that can keep an `Arc<OpenFile>` alive without also being counted by the `OpenFile` object's own mount-live token.

## Q3 - Does Decrement-on-OpenFile-Drop Close the Race?

Yes for the current substrate, because the counter is attached to `OpenFile`, not to fd-table slots:

- `OpenFile::new_with_mount` increments the mount counter before returning the object at `kernel/src/fs/mod.rs:146` and `kernel/src/fs/mod.rs:153`.
- `OpenFile::Drop` decrements the counter at `kernel/src/fs/mod.rs:297`.
- `sys_umount` reaches `MountTable::remove_child`, which checks `live_handles > 0` under the mount-table lock before removal at `kernel/src/fs/mod.rs:465` and `kernel/src/fs/mod.rs:469`.
- fd lookup clones the same `Arc<OpenFile>` for syscall duration at `kernel/src/arch/x86_64/syscall.rs:1014`, so fd-table teardown does not decrement the mount counter to zero while a syscall-local `Arc<OpenFile>` remains alive.

This specifically closes the PM race shape if implemented as "count `OpenFile` object lifetime", as the current workspace does. A sequence number or epoch is not required for `OpenFile` lifetime visibility under today's single-thread-per-process and non-grantable-file-cap constraints.

Refinement note: do not implement the counter as fd-slot accounting. It must stay tied to `OpenFile` object construction/drop, otherwise syscall-local clones would reopen the T3-T6 window.

## Q4 - BUILD-3D Writeback Cache Hidden Work

Dirty NeoFS work can outlive the last user-visible FD as cache state, but it is not an unobservable UAF path in the audited code:

- Writes dirty cache blocks through `NeoFsBlockCache::write_block` / `write_bytes_at` at `kernel/src/fs/neofs/cache.rs:158` and `kernel/src/fs/neofs/cache.rs:175`.
- The background worker drains the cache at `kernel/src/fs/neofs/cache.rs:346`.
- On stop, the worker drains all remaining dirty state before returning success at `kernel/src/fs/neofs/cache.rs:350`.
- `NeoFsState::Drop` stops and joins the writeback worker at `kernel/src/fs/neofs/inode.rs:650`.
- `stop_writeback_worker` requests stop, wakes the worker, joins it, and reaps at `kernel/src/fs/neofs/cache.rs:364`.

`sys_umount` currently removes the mount entry under `MOUNTS` via `vfs_umount_at_parent` at `kernel/src/fs/mod.rs:606`; `MountTable::remove_child` performs the busy check and removes the entry at `kernel/src/fs/mod.rs:465`. Dropping the removed `Arc<dyn MountedFs>` triggers NeoFS state teardown, and NeoFS state teardown drains writeback before the filesystem object is destroyed.

So writeback is a hidden mount-state reference/work item, but current object ownership makes it part of filesystem drop/drain rather than an `OpenFile` refcount obligation. No queued writeback can be newly created after the last `OpenFile` drops, because the only production writers are through `OpenFile::write` at `kernel/src/fs/mod.rs:235` into `NeoInode::write_at` at `kernel/src/fs/neofs/inode.rs:760`.

Recommended wording for Cerebro: `sys_umount` may detach after `live_handles == 0` if filesystem drop is synchronous and drains writeback before returning. If future filesystems detach mount table entries before their own worker drain completes, the busy predicate must include that work or the detach must be two-phase.

## Q5 - Force-Kill Sync or Reaper-Posted?

Force-kill is synchronous for process-owned handles:

- `terminate` runs in the killer's context and drains/drops the target's handles before returning 0 at `kernel/src/sched/process.rs:909` through `kernel/src/sched/process.rs:998`.
- `scheduler::kill_thread_by_pid` synchronously removes a ready victim from a ready queue and moves it to zombies at `kernel/src/sched/scheduler.rs:1024`.
- If the victim is blocked, `terminate` extracts it from channel state or token-parked state and zombifies it before closing peers at `kernel/src/sched/process.rs:953` and `kernel/src/sched/process.rs:968`.

The reaper remains responsible for the killed `Thread` object's final free and Pmap tail reference, but not for fd-table `Handle::File` drops.

## Q6 - Existing Drain/Epoch Precedent

There is no general RCU/epoch reclamation primitive to reuse for mount lifetime today.

Useful precedent exists, but it is narrower:

- TLB shootdown is synchronous publish-IPI-self-invalidate-wait-for-ACK at `kernel/src/mm/tlb_shootdown.rs:157` and `kernel/src/mm/tlb_shootdown.rs:227`.
- Scheduler wake-token parking is generation-checked and returns `Woken/Gone/Stale`; it is suitable for non-owning wake authority, not mount-object reclamation.
- Kthread lifecycle has subsystem-owned stop/join handles at `kernel/src/sched/kthread.rs:80`, `kernel/src/sched/kthread.rs:321`, and `kernel/src/sched/kthread.rs:329`.
- NeoFS already uses that kthread lifecycle to stop and drain writeback on state drop at `kernel/src/fs/neofs/inode.rs:650`.

BUILD-4B should reuse the kthread stop/join/drain pattern for filesystem workers. It does not need to invent a kernel-wide epoch scheme for the current OpenFile/live-handle model.

## Q7 - Need Two-Phase Umount?

Not for the current dispatched model, if these constraints remain true:

- The mount-table busy check and entry removal stay under `MOUNTS` lock.
- `live_handles` counts `OpenFile` object lifetime, not fd slots.
- Filesystem object drop synchronously drains filesystem-owned hidden work before `sys_umount` returns.
- File caps remain non-grantable through generic IPC until a future dispatch explicitly extends file-cap grant semantics.

Current code satisfies the first three:

- `vfs_umount_at_parent` locks `MOUNTS` and calls `remove_child` at `kernel/src/fs/mod.rs:606`.
- `remove_child` checks `live_handles` and removes under that lock at `kernel/src/fs/mod.rs:465`.
- `OpenFile` increments/decrements per object at `kernel/src/fs/mod.rs:153` and `kernel/src/fs/mod.rs:297`.
- NeoFS drop stops and drains writeback at `kernel/src/fs/neofs/inode.rs:650`.

A two-phase mark-detaching/drain/detach operation becomes necessary when either of these future changes lands:

- Multi-thread-per-process allows one thread to run a file syscall while another process-exit path tears down the same process fd table without a complete per-process thread freeze.
- File caps become grantable/transferable via IPC and channel state or pending grants can carry an actual `Arc<OpenFile>`.
- A mounted filesystem owns async work that is not synchronously drained by filesystem drop.

## Verdict

PROCEED-AS-DISPATCHED.

The live-handle token model closes the audited force-kill/fd-teardown interleaving if Cerebro keeps the count tied to `OpenFile` object lifetime. The current workspace implementation already follows that shape. No sequence-number, epoch, or two-phase umount refinement is required for current Phase 3 BUILD-4B constraints.

## Incidental Findings

- The dispatch listed `kernel/src/cap/`, but capability code is in `kernel/src/sched/process.rs`; there is no separate cap module in this tree.
- The audited workspace is ahead of the dispatch substrate and already contains BUILD-4B code. PM should treat this artifact as auditing the live local implementation, not only the pre-implementation plan.
- Mixed-IF lock sites were visible around mount table and NeoFS cache paths, but no new mixed-IF issue was chased in this audit scope.
