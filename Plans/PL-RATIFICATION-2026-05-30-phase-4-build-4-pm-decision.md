# PL-RATIFICATION: Phase 4 BUILD-4 PM-DECISION — strategy, scope, and §10 ratified

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Status:** RATIFIED — unblocks the BUILD-4 Jarvis dispatch
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-4-opening.md (Q1–Q7, §10)
- Plans/PLAN-2026-05-30-phase-4-build-4-tcp.md
- Plans/DISPATCH-2026-05-30-phase-4-build-4-tcp-jarvis.md
- Plans/PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md (FM-1/2/3)

---

## Decision

The BUILD-4 implementation strategy, scope, and all six §10 open points are
**ratified as recommended**. No exceptions.

### Pre-decided this session (TCP opening questions)

| Item | Ratified |
|------|----------|
| **Q1 — implementation strategy:** hand-roll TCP in `net/tcp.rs`; smoltcp stays phy-only (reject `iface::Interface` adoption per R1) | ✓ |
| **Scope:** full ratified THINK milestone — TCP + CUBIC + RFC 5961 + RFC 7323 in-build (sequenced 4a–4g), not deferred | ✓ |

### §10 open points

| # | Point | Ratified |
|---|-------|----------|
| 1 | Q2 — TCP socket **type fork** (`TcpSocket`/`TcpListener`); UDP `Socket` untouched | ✓ |
| 2 | Q3 / FM-1 — `accept` mints a derived inner cap via `sys_derive`; extend the cap subsystem if Phase-2 derivation can't (4b gating check) | ✓ |
| 3 | Q4 / FM-2 — separate READ/WRITE rights bits for half-close `shutdown(WRITE)` | ✓ |
| 4 | Q5 / FM-3 — TIME-WAIT state kernel-retained past cap drop, process-attributed | ✓ |
| 5 | Q6 — CUBIC only, non-pluggable | ✓ |
| 6 | Q7 — RFC 5961 challenge-ACK + injected-peer QEMU gate; real-peer round-trip deferred to bare-metal 4g | ✓ |

## Effect

- The "Authorized on PL ratification" items in PM-DECISION §9 are now
  authorized: the type-fork shape, the FM-1 cap-derivation approach, the
  FM-2/FM-3 cap decisions, and CUBIC-only.
- R1 (smoltcp wrap-vs-fork) is **resolved** for TCP: hand-roll, phy-only.
- R16 residual is discharged by BUILD-4 implementation (the speculative sketch
  discharged the design).
- The Jarvis dispatch
  (`DISPATCH-2026-05-30-phase-4-build-4-tcp-jarvis.md`) is **live** — gated
  only on PM-DECISION + PLAN on the branch (satisfied) and this ratification.
  Jarvis is cleared to create `codex/phase-4-build-4-tcp-jarvis` from the
  latest `origin/main` and begin 4a → 4g.
- PM (Cerebro) remains in the PM seat: stands by for the Jarvis RETURN, then
  dispositions and authorizes the merge.
