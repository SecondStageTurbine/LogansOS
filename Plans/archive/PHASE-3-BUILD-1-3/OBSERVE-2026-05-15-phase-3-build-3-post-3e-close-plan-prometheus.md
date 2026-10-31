# OBSERVE - Phase 3 BUILD-3 Post-3E Close Plan

Date: 2026-05-15
Agent: Prometheus
Status: PLANNING ARTIFACT
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3e-final-acceptance.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-SMP-TIMEOUT-RESOLUTION-2026-05-15.md`

---

## 1. Is BUILD-3 Ready for a Close Report?

**Yes.** All planned sub-commits through 3E are accepted by PM. No further
sub-commits are expected or authorized within BUILD-3.

The original prereq audit (`OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md`)
identified seven slices: 3A through 3F. Of those:

| Slice | Planned | Delivered | PM Accepted |
|---|---|---|---|
| 3A | kthread lifecycle substrate | `e4ac074` — delivered, accepted | Yes (consumed by 3D/3E) |
| 3B | virtio-blk FLUSH negotiation | `a3023a0` — delivered, accepted | Yes (`64693de` verification accepted) |
| 3C | NeoFS free-space GC | `a7eeb21` — delivered, accepted | Yes (`67913b7` verification accepted) |
| 3D | write-back buffer cache | `b2a93f2` — delivered, accepted | Yes (`abd98ed`, `PM-DISPOSITION-2026-05-15-...-3d-consumption-and-3e-authorization`) |
| 3E | durable fsync | `8cd3549` — delivered, accepted | Yes (`PM-DISPOSITION-2026-05-15-...-3e-final-acceptance`) |
| 3F | CAS/dedup (measurement-gated) | Not started | N/A — measurement never admitted; PM has not authorized |

BUILD-3F remains deferred indefinitely. It is measurement-gated: no dedup
workload or ratio evidence exists in the repo to admit it. Closing BUILD-3
without 3F is consistent with the original plan, which conditioned 3F on a
measurement artifact that was never produced.

**Conclusion:** BUILD-3 is complete through 3E. A close report is appropriate
pending PM authorization. No sub-commits remain expected after 3E.

## 2. Artifacts and Commits a Close Report Should Cite

### 2A. Implementation Commits (in delivery order)

```
e4ac074 kernel/sched: add owned kthread lifecycle substrate          (3A)
a3023a0 kernel/storage: negotiate virtio-blk flush                   (3B)
a7eeb21 kernel/fs: add NeoFS free-space GC                           (3C)
b2a93f2 kernel/fs: add NeoFS write-back cache                        (3D)
8cd3549 kernel/fs: make NeoFS fsync durable                          (3E)
60319e3 test: tighten BUILD-3E fsync evidence                        (3E evidence hardening)
```

### 2B. Verification Artifacts

```
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3A-VERIFICATION-2026-05-14.md       (3A)
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3B-VERIFICATION-2026-05-14.md       (3B)
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3C-VERIFICATION-2026-05-14.md       (3C)
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3D-VERIFICATION-2026-05-14.md       (3D)
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md       (3E)
```

### 2C. PM Disposition Artifacts

```
Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3d-consumption-and-3e-authorization.md
Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3e-final-acceptance.md
```

### 2D. Resolution Artifacts

```
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-SMP-TIMEOUT-RESOLUTION-2026-05-15.md
```

### 2E. Supporting Planning Artifacts

```
Plans/OBSERVE-2026-05-14-phase-3-build-3-prereq-audit-jarvis.md       (original scope audit)
Plans/PREP-2026-05-15-phase-3-build-3-3e-evidence-matrix-prometheus.md (3E evidence matrix)
```

### 2F. Branch References

```
codex/phase-3-build-3-3a-kthread-lifecycle     HEAD: e4ac074
codex/phase-3-build-3-3b-virtio-flush          HEAD: 64693de
codex/phase-3-build-3-3c-neofs-gc-free-space   HEAD: 67913b7
codex/phase-3-build-3-3d-neofs-writeback-cache HEAD: abd98ed
codex/phase-3-build-3-3e-durable-fsync         HEAD: 747c3a6
```

## 3. Hard Gates for BUILD-3 Close

The 3E full SMP sweep is the last gate that was run. A BUILD-3 close report
should confirm these gates are still satisfied at close time:

### 3A. Final Full SMP Sweep (re-run at close)

```
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

This is the single gate that subsumes all sub-commit scenarios under maximum
concurrency pressure. If this passes cleanly, individual sub-commit regression
gates are implicitly covered.

**Note:** The 3E sweep has demonstrated transient QEMU timeout behavior
(7 attempt-level timeouts across two full runs, all retried to PASS). This is
classified as environmental harness pressure, not a correctness failure. The
close report should document any retry evidence from the final run rather than
claiming zero timeouts.

### 3B. Build/Check Gates (final state)

```
cargo k
cargo k --no-default-features
cargo check -p xtask
cargo check -p neofs-mkfs
```

Run on the current HEAD of `codex/phase-3-build-3-3e-durable-fsync`.

### 3C. aarch64 Runtime Barrier Lane

The `phase3_aarch64_virtio_barrier_lane` scenario must produce runtime QEMU
evidence (`compile_only=false`, `sentinel=runtime_qemu_aarch64`). This is
included in the full SMP sweep but worth calling out as a named gate because
the prereq audit listed "aarch64 barrier lane falling back to compile-only" as
a stop condition for all of BUILD-3.

### 3D. Scope Enforcement

The close report must include an explicit statement that no CAS/dedup, AHCI/NVMe,
foreign filesystem, rootfs pivot, mmap/exec, broad POSIX, BUILD-4, or
unauthorized work landed on any BUILD-3 branch. Each sub-commit verification
artifact already contains this statement; the close report consolidates it.

## 4. Unresolved Caveats to Carry Into the Close Report

These caveats are known, accepted limitations that must appear in the close
report rather than being silently dropped:

### C1. Mount-wide fsync drain (not per-file)

`fsync_regular()` calls `cache.drain_all()`, which drains ALL dirty blocks on
the mount, not just blocks belonging to the fsync'd file. The cache tracks
blocks by `(block_number, role)` with no per-file attribution. This is
documented in the 3E verification artifact as `drain_scope=mount_wide` and is
intentionally conservative for BUILD-3E.

**Forward marker:** Per-file dirty attribution would narrow fsync cost for
multi-file workloads. Not a correctness issue; a performance optimization for
a future BUILD.

### C2. Quiescent GC (full lock, no incremental)

The 3C mark-sweep GC holds the entry table and metadata locks for the entire
mark/sweep/publish cycle. There is no incremental or concurrent GC. This was
accepted during 3C and does not need to be resolved before BUILD-3 close.

**Forward marker:** Incremental GC if pause becomes visible under larger
workloads (FM-Phase3-5 from the prereq audit).

### C3. No background GC worker

GC runs in the foreground only. There is no background GC kthread. The write-back
worker from 3D exists, but a dedicated GC worker was not implemented within
BUILD-3.

**Forward marker:** Background GC worker would reduce foreground pause time.

### C4. VIRTIO_BLK_F_CONFIG_WCE not negotiated

The virtio-blk driver explicitly does NOT negotiate `VIRTIO_BLK_F_CONFIG_WCE`
(write-back caching at the device level). This was a deliberate 3B decision:
rely on explicit FLUSH commands rather than device-level write-cache semantics.

**Forward marker:** Device write-cache negotiation is a future optimization,
not a correctness gap.

### C5. Transient QEMU timeout behavior under full SMP sweep

The full `NEO_SMP=4 --no-meta` sweep has exhibited 7 attempt-level QEMU
timeouts across two runs, all with successful retry PASS. Classification:
full-suite/QEMU timeout pressure, not a deterministic correctness failure.

**Harness weakness:** xtask uses fixed per-scenario serial paths; failed
retry-attempt serial logs can be overwritten by later retries. If this class
of issue recurs, preserve per-attempt serial logs before deeper kernel work.

### C6. Kernel panic remains fatal (no unwind)

The 3A kthread substrate cannot recover from kernel panics inside worker
threads. This is a known architectural constraint acknowledged during 3A and
carried through all BUILD-3 sub-commits.

**Forward marker:** Not a BUILD-3 concern. Would require a separate unwind
or panic-recovery BUILD.

### C7. Single-thread-per-process invariant

The scheduler enforces one thread per process (D13 from SC-4). Multi-threaded
user processes are Phase 2+ future work. This constrains how fsync and write-back
interact with user-space concurrency but is not a gap.

### C8. Legacy BUILD-2 image compatibility

The 3C free-map format addition preserves mountability of legacy BUILD-2 images
(all-zero superblock extension) through the bump-only allocation path. This is
a forward-compatibility feature, not a caveat, but should be stated in the close
report as part of the NeoFS format evolution record.

## 5. Post-Close Work to Dispatch Separately

If PM accepts a BUILD-3 close report, these items should be dispatched as
separate planning tasks, not folded into the close:

### P1. Docs/index/merge dispatch

- Update `docs/phases/PHASE-3.md` status from "BUILD-3 active" to "BUILD-3 closed"
  with close commit SHAs and verification artifact references.
- Update `Plans/INDEX.md` (if maintained) with BUILD-3 close entry.
- Merge all five `codex/phase-3-build-3-*` branches into the integration target
  designated by PM. **Do not merge to `main` without explicit PM authorization.**
- Push only after PM authorizes.

### P2. BUILD-4 planning dispatch

Phase 3 BUILD-4 scope has not been defined. A prereq audit for BUILD-4 would
need to assess what Phase 3 goals remain after BUILD-3 close (e.g., CAS/dedup
measurement, broader POSIX, mmap/exec, rootfs pivot) and propose sub-commits.

### P3. BUILD-3F reconsideration dispatch

If PM wants to revisit CAS/dedup, a measurement artifact must be produced first:
run representative workloads, measure duplicate block ratios, and determine
whether content-addressing admits savings worth the implementation cost. This
is a separate planning lane, not part of BUILD-3 close.

### P4. Harness improvement dispatch

The xtask serial-log overwrite weakness (C5) should be addressed before heavy
SMP test runs in future BUILDs. A narrow dispatch to preserve per-attempt serial
logs would improve timeout investigation quality.

## 6. Boundaries

This document does not:
- Produce a close report
- Authorize merge, push, or BUILD-4
- Edit kernel code
- Authorize BUILD-3F, CAS/dedup, AHCI/NVMe, foreign filesystem support, rootfs
  pivot, mmap/exec, or broad POSIX expansion
