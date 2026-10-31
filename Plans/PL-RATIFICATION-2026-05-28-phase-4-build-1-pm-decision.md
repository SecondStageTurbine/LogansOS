# PL Ratification — Phase 4 BUILD-1 PM-DECISION (Opening)

**Date:** 2026-05-28
**Recorder:** PM (Cerebro)
**Ratifies:** `Plans/PM-DECISION-2026-05-28-phase-4-build-1-opening.md`
**Companion:** `Plans/PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md` (R16 mitigation)
**Status:** RATIFIED in full (silent ratification of all four §10 open points)

## 1. Disposition

PL ratified the Phase 4 BUILD-1 opening PM-DECISION in chat 2026-05-28: *"Ratified, commit and push."* Per PM-DECISION §10's silent-ratification rule, all four PM recommendations are accepted:

| # | Point | Ratified position |
|---|-------|-------------------|
| 1 | Q1 smoltcp posture (P4 prerequisite) | **Hubris-style wrap.** Cargo dependency on smoltcp v0.13.0, kernel-owned scheduling shim. No fork. |
| 2 | Q2 RDNSS option processing placement | **Lift at BUILD-1 (in BUILD-1b SLAAC code) / consume at BUILD-4** (stub resolver). Populated-but-unused `ResolverConfig` at BUILD-1 ship. |
| 3 | Q3 Speculative TCP sketch depth | **Syscall signatures + connection-state-machine composition outline.** Filed as `PM-NOTE-...-speculative-tcp-shape-sketch.md`. Outcome: layered shape composes with TCP; three forward markers (FM-1, FM-2, FM-3) handed to BUILD-3 PLAN. No BUILD-1c API revision required. |
| 4 | §5 sub-commit ISC sets + BUILD-1b/1f dispatch fusion | **Accepted as written.** BUILD-1b and BUILD-1f land in one dispatch because extension-header processing physically lives in the IPv6 receive path. All seven sub-commits' ISC criteria stand. |

## 2. What This Ratification Unblocks

- **BUILD-1a implementation dispatch to Jarvis.** Authorized now; gated only on the pre-implementation research return from Cerebro.
- **Pre-implementation research dispatch to Cerebro.** Authorized now; the research output is the immediate substrate Jarvis consumes.
- **Documentation / prior-art dispatch to Prometheus.** Authorized now; complements Cerebro's deep-dive.
- **Cargo dependency on `smoltcp`** at the version specified in the Cerebro research return.
- **Creation of `kernel/src/net/` and `kernel/src/drivers/net/`** module trees by Jarvis on BUILD-1a implementation.

## 3. What This Ratification Does NOT Do

- Does NOT authorize BUILD-1b/1c/1d/1.1/1f/1e implementation work. Each sub-commit opens after its predecessor closes per the PM-DECISION §6 sequential discipline.
- Does NOT authorize BUILD-2 (IPv4) or beyond.
- Does NOT pre-commit the `NetworkAccess` rights bitmap layout (Q4, decided at BUILD-1c dispatch — with FM-2 reservation of separate READ / WRITE bits).
- Does NOT close THINK v0.2; THINK remains the canonical Phase 4 design artifact until Phase 4 ship at BUILD-5.

## 4. Forward Action

Three dispatch artifacts and one PL push action follow this ratification in the same commit:

- `DISPATCH-2026-05-28-phase-4-build-1a-pre-implementation-research-cerebro.md` — Cerebro pre-implementation research.
- `DISPATCH-2026-05-28-phase-4-build-1a-rust-virtio-net-prior-art-prometheus.md` — Prometheus ecosystem prior-art survey.
- `DISPATCH-2026-05-28-phase-4-build-1a-netdevice-virtio-net-smoltcp-wrap-jarvis.md` — Jarvis BUILD-1a implementation, gated on Cerebro research return.

PL push action: this commit + the preceding `29b8036` plan commit to `origin/main`.

---

**Ratified. BUILD-1 PLAN locked. Three workers dispatched. Implementation gated as documented in each dispatch.**
