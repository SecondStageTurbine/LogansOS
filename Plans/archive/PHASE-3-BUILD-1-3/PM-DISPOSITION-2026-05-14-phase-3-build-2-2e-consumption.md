# PM DISPOSITION - Phase 3 BUILD-2 2E consumption

**Date:** 2026-05-14
**Status:** 2E ACCEPTED; BUILD-2 CLOSE BLOCKED ON FULL SMP SWEEP
**Consumes:**
- Commit `f6532cb7dabb5f94adc1134b5cfd2fdd6b2ee6be` (`kernel/fs: enforce NeoFS typed FileCaps`)
- `Plans/PHASE-3-BUILD-2-SUB-COMMIT-2E-VERIFICATION-2026-05-13.md`
- `Plans/PM-NOTE-2026-05-13-phase-3-build-2-2e-sidecar-consumption.md`
- `Plans/PREP-2026-05-13-phase-3-build-2-close-evidence-prometheus.md`

---

## 1. Decision

BUILD-2 sub-commit 2E is accepted as an implementation sub-commit.

BUILD-2 is **not closed** yet.

The remaining close blocker is a clean full non-explicit SMP sweep:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Jarvis is authorized to run that closure gate and file an evidence addendum.
No new feature work, no BUILD-3 work, no close report, and no merge/push are
authorized by this disposition.

---

## 2. 2E Evidence Accepted

PM accepts the 2E evidence:

- Type-first executable policy helper is present.
- `Model` returns `TypeForbidden` even without execute rights.
- `Regular` succeeds only when the FileCap carries the execute bit.
- Type-first / rights-second four-corner matrix passed.
- `FileRights` remains a 32-bit word; high reserved bits remain representable.
- FileCap generation capture and stale rejection use current reachable NeoFS
  root-entry generation.
- `ESTALE = -116` is wired through kernel/libneo.
- Reserved/future type evidence uses deterministic rejection, which PM accepts.
- Six 2E QEMU scenarios pass, including targeted `NEO_SMP=4`.
- Required 2D/2C/2B/BUILD-1 targeted regression lane passes.
- Build/tooling gates pass.
- No real mmap/exec, disk exec, retype, GC, CAS, FileCap grant/derive/revoke,
  close report, BUILD-3 work, or push was done.

`phase3_aarch64_virtio_barrier_lane` was not rerun, and that is acceptable for
2E because the 2E commit did not touch virtio, barrier, or block-driver files.

---

## 3. Close Blocker

The full non-explicit SMP sweep was attempted but did not complete. It failed
before the NeoFS 2E scenarios at:

```text
phase3_virtio_blk_abandoned_io_no_leak
reason=virtio_blk_abandoned_io_dma_live_leak before=3 after=6
```

Jarvis isolated the failure with targeted reruns:

- base `a2f1e49` targeted `phase3_virtio_blk_abandoned_io_no_leak` under
  `NEO_SMP=4`: PASS;
- 2E branch targeted `phase3_virtio_blk_abandoned_io_no_leak` under
  `NEO_SMP=4`: PASS.

This isolation is useful but not sufficient for BUILD-2 closure because the
close stop condition requires a clean full non-explicit SMP sweep or explicit
PM waiver. No waiver is granted here.

---

## 4. Authorized Next Action

Jarvis may perform the closure-gate rerun only:

`Plans/DISPATCH-2026-05-14-phase-3-build-2-close-smp-sweep-jarvis.md`

If the full sweep passes, Jarvis should file:

`Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md`

That addendum may be committed as a Plans-only evidence commit. It must not
claim BUILD-2 closure by itself.

If the full sweep fails again, Jarvis should file a blocker/failure artifact
with the failing scenario, serial log excerpt, targeted rerun result, and
whether the failure reproduces on base `a2f1e49` and on the 2E branch.

---

## 5. Stop Conditions

Stop and file a blocker if:

- the full sweep repeatedly fails;
- the failing scenario cannot be isolated;
- the failure appears caused by 2E;
- fixing the failure would require code changes beyond a trivial test harness
  plumbing issue;
- the work pressures BUILD-3 mechanisms, broad POSIX work, or new NeoFS scope.

---

## 6. External Status

Phase 3 BUILD-2 state after this disposition:

- 2A: accepted.
- 2B: accepted.
- 2C: accepted.
- 2D: accepted.
- 2E: accepted as implementation.
- BUILD-2 closure: blocked pending clean full non-explicit SMP sweep.
- BUILD-3: not started.
