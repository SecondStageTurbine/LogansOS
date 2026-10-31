# Dispatch - Phase 3 BUILD-3E Full SMP Timeout Investigation

Date: 2026-05-15
Agent: Jarvis
Status: AUTHORIZED INVESTIGATION
Authority:
- `Plans/PM-DISPOSITION-2026-05-15-phase-3-build-3-3e-clean-smp-blocker-consumption.md`
- `Plans/BLOCKER-2026-05-15-phase-3-build-3-3e-full-smp-sweep-jarvis.md`
- `Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-VERIFICATION-2026-05-15.md`

---

## 1. Mission

Investigate why `phase3_neofs_fsync_dirty_persists_after_crash` times out in the
full `NEO_SMP=4 --no-meta` sweep but passes when run alone.

This is an investigation lane, not feature work. Make the smallest diagnostic or
fix needed to satisfy the hard SMP gate. If a code fix is required, it must be
narrowly tied to the identified timeout cause and must not broaden BUILD-3E
scope.

Current accepted implementation HEAD:

```text
60319e32d348082fa1e5f90ee98288cfcc5bcf42
```

Current blocker documentation HEAD:

```text
1da2d9a
```

## 2. Working Hypotheses To Test

Do not assume the answer. Check at least these:

1. Full-suite disk/image reuse or stale raw image interaction for the two-boot
   fsync scenario.
2. QEMU/container resource pressure after many prior NeoFS boots.
3. xtask timeout/retry accounting specific to two-boot scenarios.
4. NeoFS write-back worker or kthread lifecycle state leaking across boots or
   scenarios through shared test artifacts.
5. Virtio completion worker/one-in-flight behavior under full-suite timing.
6. Fsync scenario requiring a longer per-boot timeout because full-suite cache
   and Docker IO pressure makes the first drain slower.
7. A real ordering/liveness bug masked by standalone timing.

## 3. Suggested Investigation Shape

Use a ladder rather than immediately rerunning the entire hour-long suite:

1. Run an ordered mini-suite ending with
   `phase3_neofs_fsync_dirty_persists_after_crash`, including the scenarios that
   immediately precede it in the full sweep.
2. Run the 3E fsync scenario after a cluster of 3C/3D NeoFS write-back and GC
   scenarios.
3. Inspect xtask raw-image naming, cleanup, retry, and timeout handling for
   two-boot scenarios.
4. Inspect the serial logs from the blocker for the last kernel line before
   timeout on both boot attempts.
5. If adding diagnostics is necessary, keep them scenario-gated under
   `neo-test` features and remove any noisy traces before final verification.

## 4. PASS Path

If a narrow fix is made:

- commit the implementation fix with a scoped message;
- rerun the targeted scenario under `NEO_SMP=4`;
- rerun the ordered mini-suite that reproduced or approximated the failure;
- rerun the full `NEO_SMP=4 --no-meta` sweep;
- update or add an investigation/verification addendum.

If no code fix is needed and a clean full sweep passes after environment or
harness clarification:

- file an addendum explaining the cause and the clean command evidence.

Suggested addendum path:

```text
Plans/PHASE-3-BUILD-3-SUB-COMMIT-3E-SMP-TIMEOUT-RESOLUTION-2026-05-15.md
```

## 5. FAIL Path

If the timeout remains unresolved, update the blocker or file an addendum with:

- exact commands and exit statuses;
- serial-log paths;
- last serial lines before timeout;
- whether the failure reproduces in an ordered mini-suite;
- whether the failure reproduces after 3C/3D NeoFS scenarios;
- whether the failure appears xtask timeout policy, Docker/QEMU environment, or
  kernel liveness.

## 6. Boundaries

No BUILD-3F, BUILD-4, CAS/dedup, AHCI/NVMe, foreign filesystem support, rootfs
pivot, mmap/exec, broad POSIX expansion, merge, or push is authorized.

