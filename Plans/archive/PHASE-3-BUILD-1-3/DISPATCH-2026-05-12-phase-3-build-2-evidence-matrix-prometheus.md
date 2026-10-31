# DISPATCH - Phase 3 BUILD-2 evidence matrix

**Date:** 2026-05-12
**To:** Prometheus
**From:** PM
**Status:** READY
**Mode:** closure-prep / evidence design only

---

## 1. Mission

Prepare the BUILD-2 evidence matrix before implementation starts.

This is not a close report. It is the checklist that will keep BUILD-2 honest
once Jarvis begins implementation.

---

## 2. Inputs

Read:

- `docs/ADR-021-phase-3-design-substrate.md`
- `docs/phases/PHASE-3.md`
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-2-opening.md`
- `Plans/RESEARCH-2026-05-12-phase-3-vfs-typed-inode-capability-cerebro.md`

If available by the time you work, also read:

- `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md`
- `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`

Do not wait for those optional inputs if PM asks you to run immediately; leave
explicit slots for them.

---

## 3. Required Matrix

Define evidence slots for:

- mkfs image creation and deterministic image hash.
- NeoFS superblock validation.
- root directory mount from virtio-blk.
- native file create/read/write.
- same-boot file readback.
- two-boot persistence readback.
- crash/recovery simulation for incomplete transaction.
- redundant superblock selection.
- checksum/corruption rejection.
- per-file `sys_fsync` intent-log semantics.
- inode generation stale-handle rejection.
- FileCap rights enforcement with `FileRights(u32)`.
- `TypeForbidden` vs `InsufficientRights` distinction.
- typed inode discriminant enforcement.
- `Model` executable-mapping refusal or stubbed policy hook.
- anti-scope assertions proving no CAS, dedup, buffer-cache write-back,
  kthreads, AHCI/NVMe, foreign FS, or rootfs pivot was introduced.

---

## 4. Deliverable

Create:

`Plans/PREP-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md`

Required sections:

1. BUILD-2 closure claim template.
2. Sub-commit evidence slots.
3. Runtime scenario matrix.
4. Regression gates inherited from BUILD-1.
5. Crash/recovery gates.
6. Typed-inode/FileCap gates.
7. Anti-scope gates.
8. Stop conditions.

No normative docs edits. No close report.

