# PL-RATIFICATION: Phase 4 BUILD-4.5 PM-DECISION — §10 ratified

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Status:** RATIFIED — unblocks the BUILD-4.5 Jarvis dispatch (still gated on BUILD-4 merge)
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md (§10)
- Plans/PLAN-2026-05-30-phase-4-build-4.5-ndp.md
- Plans/DISPATCH-2026-05-30-phase-4-build-4.5-ndp-jarvis.md

---

## Decision

All five §10 open points are **ratified as recommended**. No widening.

| # | Point | Ratified |
|---|-------|----------|
| 1 | Q1 — hand-roll `net/ndp.rs` mirroring `arp.rs`; smoltcp phy-only | ✓ |
| 2 | Q2 — minimal neighbor cache (TTL aging); full RFC 4861 NUD deferred | ✓ |
| 3 | Q3 — BUILD-2 minimal DAD unchanged; real NS-based DAD deferred (FM-4.5-1) | ✓ |
| 4 | Q4 — shared NDP resolve/queue for IPv6 UDP+TCP egress; solicited-node multicast NS on miss | ✓ |
| 5 | Q5 — no Router Solicitation; RA reception already works (FM-4.5-2) | ✓ |

## Effect

- The Jarvis dispatch (`DISPATCH-2026-05-30-phase-4-build-4.5-ndp-jarvis.md`)
  is **live**, with one remaining sequencing gate: **BUILD-4 must be merged to
  `origin/main`** before Jarvis branches `codex/phase-4-build-4.5-ndp-jarvis`
  (NDP integrates with BUILD-4's `tcp.rs` IPv6 egress).
- FM-4-1 (IPv6 NDP gap) is owned by this build; FM-4.5-1 (real DAD) and
  FM-4.5-2 (Router Solicitation) are new deferred forward-markers.
- PM (Cerebro) remains in the PM seat for the RETURN → disposition → merge.
