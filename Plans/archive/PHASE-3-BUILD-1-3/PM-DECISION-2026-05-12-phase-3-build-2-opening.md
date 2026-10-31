# PM DECISION - Phase 3 BUILD-2 opening

**Date:** 2026-05-12
**Status:** BUILD-2 PLAN/SPEC OPEN
**Consumes:** `Plans/PM-NOTE-2026-05-12-phase-3-build-1-integrated-on-main.md`
**Canonical design substrate:** `docs/ADR-021-phase-3-design-substrate.md`

---

## 1. Correction

The prior PM closeout phrase "no further agent action is required right now"
meant: no further BUILD-1 cleanup action is required.

It did not mean the project should idle. The next project action is to open
Phase 3 BUILD-2 under explicit PM scope.

---

## 2. Decision

Phase 3 BUILD-2 is opened as a PLAN/SPEC lane.

BUILD-2 goal from ADR-021:

> Kernel mounts a NeoFS image from virtio-blk and performs crash-consistent
> native file create/read/write.

The first BUILD-2 step is not free-form implementation. ADR-021 deliberately
does not ratify the exact NeoFS on-disk binary layout, so the next work must
settle the format and sub-commit split before kernel code starts.

---

## 3. Parallel Work Authorized

Three parallel artifacts are authorized:

1. **Cerebro:** NeoFS native v1 format structural challenge.
   - Dispatch:
     `Plans/DISPATCH-2026-05-12-phase-3-build-2-neofs-format-structural-challenge-cerebro.md`
   - Output:
     `Plans/RESEARCH-2026-05-12-phase-3-build-2-neofs-format-cerebro.md`

2. **Jarvis:** BUILD-2 observe pass and sub-commit implementation plan.
   - Dispatch:
     `Plans/DISPATCH-2026-05-12-phase-3-build-2-observe-and-subcommit-plan-jarvis.md`
   - Output:
     `Plans/OBSERVE-2026-05-12-phase-3-build-2-neofs-subcommit-plan-jarvis.md`

3. **Prometheus:** BUILD-2 evidence matrix and closure-gate shell.
   - Dispatch:
     `Plans/DISPATCH-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md`
   - Output:
     `Plans/PREP-2026-05-12-phase-3-build-2-evidence-matrix-prometheus.md`

These can run in parallel.

---

## 4. Implementation Boundary

No BUILD-2 kernel implementation is authorized yet.

The next implementation dispatch should be written only after PM consumes the
Cerebro format challenge, Jarvis observe/sub-commit plan, and Prometheus
evidence matrix.

Allowed now:

- Read-only codebase inspection.
- Research and structural challenge.
- BUILD-2 evidence-gate design.
- Proposed sub-commit sequencing.

Not allowed yet:

- Kernel NeoFS implementation.
- mkfs tool implementation.
- `docs/specs/neofs-format.md` as final normative spec.
- BUILD-3 CAS, dedup, buffer-cache write-back, kthreads, FLUSH negotiation, or
  background GC.
- BUILD-4 foreign filesystem work.
- BUILD-5 AHCI/NVMe work.

---

## 5. External Status

Phase 3 BUILD-1: CLOSED and integrated on local `main`.

Phase 3 BUILD-2: OPEN for PLAN/SPEC only.

