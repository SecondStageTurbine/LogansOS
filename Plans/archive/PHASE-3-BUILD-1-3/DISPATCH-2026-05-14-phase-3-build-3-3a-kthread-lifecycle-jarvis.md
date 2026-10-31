# Dispatch - Phase 3 BUILD-3A Kthread Lifecycle

**Date:** 2026-05-14
**Agent:** Jarvis
**Status:** AUTHORIZED
**Authority:**
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-planning-consumption.md`
- `Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`
- `Plans/RESEARCH-2026-05-14-phase-3-build-3-structural-challenge-cerebro.md`

---

## 1. Mission

Implement BUILD-3 sub-commit 3A only: a scheduler-owned kthread ownership/lifecycle substrate suitable for future NeoFS write-back and GC workers.

No NeoFS write-back, no FLUSH, no GC, no free-space format change, no `sys_fsync` behavior change, and no CAS/dedup work are authorized in 3A.

Base:

```text
main at 096414c4da9f2294c35bcfc9e0c459a99c50078b
```

Recommended branch:

```text
codex/phase-3-build-3-3a-kthread-lifecycle
```

## 2. Required Outcome

Add a kernel-internal kthread lifecycle API that lets a subsystem own a worker without owning `Thread`.

The API shape is Jarvis-owned, but it must provide these semantics:

- stable kthread handle identity, preferably with generation protection;
- spawn of a kernel worker under `KERNEL_PID`;
- stop request visible to the worker;
- join or completion wait for owned workers;
- explicit detach semantics for immortal/global workers if supported;
- worker normal-exit completion reporting;
- failure/panic behavior documented or surfaced as a failed completion state;
- no `Arc<Thread>`, no leaked `Box<Thread>`, no raw `Thread` pointer in subsystem-owned state.

The substrate should be usable later by a NeoFS mount instance that owns write-back and GC workers, requests stop during unmount, wakes a parked worker if necessary, waits for clean completion, and only then drops mount-owned state.

## 3. Suggested Implementation Surface

Prefer a new scheduler module if it fits local patterns:

```text
kernel/src/sched/kthread.rs
```

Possible public concepts:

```text
KthreadHandle
KthreadId
KthreadStopToken
KthreadExitState or KthreadJoinResult
spawn_owned(...)
request_stop(...)
join(...)
detach(...)
```

This is not a mandated API. Preserve existing repo style and keep the smallest correct surface.

`sched::spawn(name, extern "C" fn() -> !)` may remain as the low-level/global primitive. Do not break the existing virtio-blk completion worker.

Only migrate the virtio-blk completion worker to detached kthread semantics if doing so is small and improves evidence. If migration creates risk, leave it alone and prove detached/owned behavior through targeted test workers.

## 4. Required Tests / Scenarios

Add focused runtime scenarios and xtask wiring. Names may vary if needed, but cover these behaviors:

1. `phase3_kthread_spawn_join_smoke`
   - spawn an owned kernel worker;
   - worker runs and exits normally;
   - owner joins and observes completion;
   - worker is reaped, not leaked.

2. `phase3_kthread_stop_join_smoke`
   - worker loops until stop requested;
   - owner requests stop;
   - worker observes stop at a clean checkpoint and exits;
   - join returns success.

3. `phase3_kthread_stop_wakes_parked_worker`
   - worker parks on a kthread-compatible wait path or local rendezvous;
   - stop request wakes it;
   - join completes without forced thread ownership.

4. `phase3_kthread_detached_worker_smoke`
   - detached/global worker semantics remain possible;
   - detached worker completion or continued life does not require a subsystem-owned join handle.

5. `phase3_kthread_no_thread_arc_lifetime`
   - prove through instrumentation, assertions, or code-level invariant that subsystem handles do not retain `Arc<Thread>` or scheduler-owned `Box<Thread>`.

If one of these is impossible or the local scheduler makes a different proof cleaner, document the substitution in the verification artifact.

## 5. Verification Requirements

Run and report:

```text
cargo k
cargo k --no-default-features
cargo check -p xtask
```

Run new 3A targeted scenarios in Docker/QEMU.

Because 3A touches scheduler lifetime, also run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Run targeted regression gates:

```text
cargo neo-test phase3_virtio_blk_abandoned_io_no_leak
cargo neo-test phase3_block_vfs_smoke
cargo neo-test phase3_virtio_blk_raw_rw_smoke
cargo neo-test phase3_aarch64_virtio_barrier_lane
```

If host QEMU is blocked by Limine, use the established Docker path and say so.

Run whitespace/style checks:

```text
git diff --check
git diff --cached --check
```

Scoped rustfmt for changed Rust files is expected. Do not claim global `cargo fmt --check` PASS unless it actually passes globally.

## 6. Verification Artifact

Create:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md
```

Include:

- commit SHA;
- changed files;
- API summary;
- explicit statement that no NeoFS write-back, FLUSH, GC, free-space format, fsync, or CAS work landed;
- runtime PASS frames for new kthread scenarios;
- regression results;
- any blocked host-only commands and Docker equivalents;
- global formatting caveat if applicable.

## 7. Stop Conditions

Stop and file a blocker instead of committing implementation if:

- the design requires `Arc<Thread>` or raw `Thread` ownership outside scheduler;
- kthread join cannot be implemented without broad scheduler/process lifetime changes;
- stop/join can kill a worker mid-critical section instead of using cooperative clean checkpoints;
- parked workers cannot be woken for stop without inventing an unsafe wake path;
- scheduler changes regress BUILD-1 abandoned-I/O or SC-3C wake-token behavior;
- the implementation starts FLUSH, write-back cache, GC, `sys_fsync` behavior changes, CAS, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broad POSIX work.

If a blocker is hit, create:

```text
Plans/BLOCKER-2026-05-14-phase-3-build-3-3a-kthread-lifecycle-jarvis.md
```

## 8. Commit Discipline

Commit only 3A implementation and its verification or blocker artifact.

Suggested commit message:

```text
kernel/sched: add owned kthread lifecycle substrate
```

Do not merge to `main`. Do not push. Do not start 3B.
