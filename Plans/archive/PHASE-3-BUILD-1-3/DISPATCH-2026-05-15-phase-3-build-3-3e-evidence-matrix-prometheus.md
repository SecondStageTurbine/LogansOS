# Dispatch - Phase 3 BUILD-3E Evidence Matrix

Date: 2026-05-15
Agent: Prometheus
Status: AUTHORIZED SIDEcar
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md`
- `Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-durable-fsync-jarvis.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md`

---

## 1. Mission

Prepare the BUILD-3E evidence matrix and close-readiness checklist for durable
NeoFS `sys_fsync`.

This is not implementation and not closure. Do not edit kernel code. Produce a
planning/evidence artifact that Jarvis can use while implementing 3E and PM can
use when judging the 3E verification artifact.

## 2. Required Output

Create:

```text
Plans/PREP-2026-05-15-phase-3-build-3-3e-evidence-matrix-prometheus.md
```

## 3. Evidence Matrix Requirements

The matrix must cover:

- syscall/VFS contract for `sys_fsync(fd)`;
- stale FileCap rejection before durability work;
- dirty fsync drains required cache state;
- dirty fsync issues and observes real `VIRTIO_BLK_T_FLUSH`;
- clean fsync does not issue an unexplained/spurious FLUSH;
- fsync'd data survives crash/reboot without relying on mount teardown;
- unfsync'd dirty data may be lost but must not corrupt old committed state;
- fsync coordination with background write-back;
- fsync coordination with GC;
- 3D write-back scenarios remain passing;
- 3B virtio FLUSH/abandoned-I/O scenarios remain passing;
- aarch64 virtio barrier lane remains runtime evidence, not compile-only;
- full `NEO_SMP=4 --no-meta` sweep remains required before 3E acceptance.

For each row, include:

- scenario name or proposed scenario name;
- expected PASS-frame fields;
- failure mode it detects;
- whether it is a hard close gate or supporting evidence.

## 4. Close-Readiness Checklist

Include a concise checklist PM can use against
`Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`.

The checklist must explicitly reject closure if:

- fsync returns before dirty cache drain and FLUSH completion;
- fsync weakens CoW/superblock publication ordering;
- fsync races GC or background write-back into torn state;
- stale FileCaps can fsync;
- clean fsync causes unexplained FLUSH traffic;
- crash after fsync loses fsync'd data;
- crash before fsync corrupts old committed state;
- out-of-scope CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec,
  broad POSIX, BUILD-4, or unauthorized work lands.

## 5. Boundaries

Do not produce a close report. Do not authorize BUILD-3F/BUILD-4. Do not edit
kernel code.

