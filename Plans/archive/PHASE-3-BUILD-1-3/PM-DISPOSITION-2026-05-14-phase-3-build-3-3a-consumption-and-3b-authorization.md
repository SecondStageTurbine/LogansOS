# PM Disposition - Phase 3 BUILD-3A Consumption and 3B Authorization

**Date:** 2026-05-14
**Status:** 3A ACCEPTED; 3B authorized by separate dispatch
**Consumes:**
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md`
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-3-planning-consumption.md`
- `Plans/DISPATCH-2026-05-14-phase-3-build-3-3a-kthread-lifecycle-jarvis.md`

---

## 1. Decision

PM accepts BUILD-3 sub-commit 3A.

3A commit:

```text
e4ac0746dbaf6f4b6f26e16a895fe0f40c5c8ba5 kernel/sched: add owned kthread lifecycle substrate
```

Branch:

```text
codex/phase-3-build-3-3a-kthread-lifecycle
```

BUILD-3 may proceed to 3B only:

```text
Plans/DISPATCH-2026-05-14-phase-3-build-3-3b-virtio-flush-jarvis.md
```

## 2. 3A Acceptance Basis

Jarvis reports and local PM checks confirm:

- scheduler-owned kthread substrate landed in `kernel/src/sched/kthread.rs`;
- handle generation identity, stop, join, detach, and completion semantics are present;
- subsystem handles do not own `Arc<Thread>`, `Box<Thread>`, or raw `Thread` pointers;
- five targeted kthread scenarios passed under Docker/QEMU;
- targeted storage/block/aarch64 regression lane passed;
- full `NEO_SMP=4 cargo neo-test --no-meta` passed;
- no BUILD-3B+ scope landed.

Changed files in the 3A commit:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md
kernel/Cargo.toml
kernel/src/sched/kthread.rs
kernel/src/sched/mod.rs
kernel/src/sched/scheduler.rs
kernel/src/test_harness.rs
tools/xtask/src/main.rs
```

## 3. Artifact Caveat

The 3A verification artifact retained this placeholder:

```text
Commit SHA: reported after commit; this artifact is committed with the implementation.
```

PM records the actual commit SHA in this disposition. This is not a 3A correctness blocker because the implementation commit, branch, and verification report were independently confirmed. Future verification artifacts must embed the exact commit SHA and must not leave a placeholder in the committed artifact.

## 4. 3B Authorization

3B is the block-layer and virtio-blk FLUSH transition only.

3B may:

- add a block-layer flush capability/API;
- negotiate `VIRTIO_BLK_F_FLUSH` when offered;
- issue and complete real `VIRTIO_BLK_T_FLUSH` commands under QEMU;
- update runtime evidence from `F_FLUSH_negotiated=false` to `F_FLUSH_negotiated=true`;
- keep `VIRTIO_BLK_F_CONFIG_WCE` not negotiated.

3B must not change NeoFS write-back, GC, free-space format, `sys_fsync` semantics, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec, or broader POSIX work.

## 5. Carry-Forward Gates

The 3A kthread substrate is now available for later NeoFS write-back and GC workers. It does not by itself authorize 3D write-back or 3C GC.

The BUILD-2 `sys_fsync` no-op remains in force until a later PM dispatch changes the syscall contract.

CAS/dedup remains deferred to Phase 5 unless PM explicitly re-admits it with measurement evidence or Phase 5 critical-path language.

## 6. Closure

BUILD-3A is accepted. BUILD-3B may begin under the dedicated Jarvis dispatch. No merge to `main` and no push are authorized by this disposition.
