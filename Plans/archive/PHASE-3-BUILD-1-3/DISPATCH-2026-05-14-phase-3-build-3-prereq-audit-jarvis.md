# Dispatch - Phase 3 BUILD-3 Prerequisite Audit

**Date:** 2026-05-14
**Agent:** Jarvis
**Status:** DISPATCHED
**Authority:** `Plans/PM-DECISION-2026-05-14-phase-3-build-3-opening.md`

---

## 1. Mission

Perform a read-only BUILD-3 prerequisite audit and file an observe artifact. Do not edit kernel, userland, tool, docs/spec, ADR, Bible, test, or implementation files.

Create:

```text
Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md
```

## 2. Gate

BUILD-3 implementation is not authorized. This dispatch answers whether BUILD-3 can be planned safely and what the first implementation slice should be.

## 3. Questions To Answer

1. **Kthread substrate.** Confirm the current kernel-thread primitive shape. Include `sched::spawn`, `Thread::new`, `KERNEL_PID`, `reap`, `exit`, and any storage-driver worker call sites. Decide whether it is enough for FS-owned write-back/GC workers.
2. **Worker ownership gap.** If insufficient, propose the smallest kthread handle/lifecycle substrate needed for BUILD-3. Address stop flag, handle identity, join/reap or detached semantics, unmount teardown, panic/exit behavior, and avoiding `Arc<Thread>` lifetime mistakes.
3. **Virtio FLUSH transition.** Audit current virtio-blk feature handling. What exact files/functions would change to negotiate `VIRTIO_BLK_F_FLUSH` and issue flush commands? What tests must prove `CONFIG_WCE` and FLUSH semantics?
4. **Buffer-cache/write-back entry point.** Locate current NeoFS read/write/CoW paths and identify the smallest insertion points for cache lookup, dirty tracking, and write-back without changing the on-disk format prematurely.
5. **`sys_fsync` contract transition.** BUILD-2 fsync is a no-op over write-through I/O. Identify the minimal code/docs/tests that must change when BUILD-3 makes fsync durable over write-back cache.
6. **Mark-sweep GC feasibility.** Using current NeoFS root/inode/direct-block layout, sketch the reachable-block walk and orphan reclamation path. Flag any format gap.
7. **CAS/dedup admission.** State whether the codebase currently has a meaningful measurement source for dedup value. If not, recommend a no-code measurement plan or a deferral path.
8. **Sub-commit proposal.** Propose a conservative BUILD-3 split. Include anti-scope and runtime gates for each slice.

## 4. Expected Output Shape

Use sections:

1. Verdict
2. Current substrate map with file/function references
3. Kthread gap analysis
4. FLUSH/write-back/fsync transition map
5. GC and CAS/dedup admission analysis
6. Recommended BUILD-3 sub-commit sequence
7. Stop conditions
8. Verification

## 5. Verification

Run only artifact checks:

```text
git diff --check -- Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md
```

Report whether the observe artifact is untracked or committed. Do not commit unless separately authorized.

## 6. Anti-Scope

No code. No BUILD-3 implementation. No normative docs. No ADR/Bible/spec edits. No tests. No branch merge. No push.
