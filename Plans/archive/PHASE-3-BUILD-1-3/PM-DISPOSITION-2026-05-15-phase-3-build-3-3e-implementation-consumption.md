# PM Disposition - Phase 3 BUILD-3E Implementation Consumption

Date: 2026-05-15
From: PM
To: Jarvis
Status: BUILD-3E IMPLEMENTATION ACCEPTED; 3E CLOSURE BLOCKED ON CLEAN SMP SWEEP

## 1. Accepted Commits

PM accepts the BUILD-3E durable NeoFS fsync implementation and follow-up
evidence-tightening commit as implementation-complete:

```text
8cd35490863514c9875a72704f8378d718eec908 kernel/fs: make NeoFS fsync durable
9ef359965b624d14b459e94439e0f78871222b29 docs: record Phase 3 BUILD-3E verification
60319e32d348082fa1e5f90ee98288cfcc5bcf42 test: tighten BUILD-3E fsync evidence
```

Accepted verification artifact, with caveat:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md
```

## 2. Accepted Technical Posture

PM accepts the 3E design posture:

- `sys_fsync(fd)` validates FileCap state before durability work.
- NeoFS Regular fsync uses the narrow `fsync_regular` insertion point.
- 3E uses mount-wide drain, not per-file dirty attribution.
- Dirty fsync drains the cache and observes virtio FLUSH command/completion.
- Clean fsync starts from a fully-clean mount and proves zero drain/flush deltas.
- Unfsync'd dirty-loss evidence holds write-back before the dirty write.
- Dirty FLUSH evidence asserts `device.supports_flush() == true`.
- The `pause_worker` / already-started `drain_pass` race is acceptable because
  foreground `drain_all()` loops to fully clean before fsync returns.
- Stale FileCap fsync returns ESTALE before durability mutation and remains
  anchored to current reachable inode generation.

## 3. Closure Caveat

BUILD-3E is not closed yet.

The full `NEO_SMP=4 cargo neo-test --no-meta` captured run exited `1`. The
verification artifact records that no terminal scenario failure was found, all
observed scenarios reached PASS after retries, and the captured run included
seven transient timeout attempts with seven retry PASS lines.

That is useful evidence, but it is not a clean hard-gate sweep. PM therefore
blocks BUILD-3E closure only on a clean full SMP sweep or a formal blocker that
identifies a persistent environment/harness failure.

## 4. Next Action

Jarvis is authorized for verification-only rerun under:

```text
Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-clean-smp-sweep-jarvis.md
```

No code changes, BUILD-3F, BUILD-4, CAS/dedup, AHCI/NVMe, foreign FS, rootfs
pivot, mmap/exec, or broad POSIX work is authorized by this disposition.

