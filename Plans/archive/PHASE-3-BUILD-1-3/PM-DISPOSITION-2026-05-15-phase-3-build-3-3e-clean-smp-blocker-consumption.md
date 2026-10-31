# PM Disposition - Phase 3 BUILD-3E Clean SMP Blocker Consumption

Date: 2026-05-15
From: PM
To: Jarvis
Status: BLOCKER ACCEPTED; INVESTIGATION AUTHORIZED

## 1. Consumed Blocker

PM accepts the BUILD-3E full SMP sweep blocker filed at:

```text
Plans/BLOCKER-2026-05-15-phase-3-build-3-3e-full-smp-sweep-jarvis.md
```

Blocker documentation commits:

```text
6b53e49 docs: file BUILD-3E full SMP sweep blocker
1da2d9a docs: fix BUILD-3E blocker formatting
```

## 2. Accepted Facts

Verification-only full sweep command:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Result:

- Exit: `1`
- Time window: `2026-05-15T14:11:49-04:00` to `2026-05-15T15:04:25-04:00`
- Terminal scenario: `phase3_neofs_fsync_dirty_persists_after_crash`
- Failure: timeout on both two-boot attempts

Targeted rerun:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
```

Result:

- Exit: `0`
- Scenario passed both boots

PM classifies this as a full-suite/order/resource interaction until proven
otherwise. The standalone 3E dirty fsync crash/reboot scenario is not currently
rejected as functionally broken.

## 3. Current BUILD-3E State

BUILD-3E implementation remains accepted at:

```text
60319e32d348082fa1e5f90ee98288cfcc5bcf42
```

BUILD-3E closure remains blocked. The hard SMP gate is not satisfied.

## 4. Next Action

Jarvis is authorized for a narrow investigation lane:

```text
Plans/DISPATCH-2026-05-15-phase-3-build-3-3e-full-smp-timeout-investigation-jarvis.md
```

No BUILD-3F, BUILD-4, CAS/dedup, AHCI/NVMe, foreign FS, rootfs pivot, mmap/exec,
broad POSIX, merge, or push is authorized.

