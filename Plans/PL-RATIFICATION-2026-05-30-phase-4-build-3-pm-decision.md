# PL-RATIFICATION: Phase 4 BUILD-3 PM-DECISION — §10 open points ratified

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Status:** RATIFIED — unblocks the BUILD-3 Jarvis dispatch
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-3-opening.md (§10 open points)
- Plans/PLAN-2026-05-30-phase-4-build-3-ipv4-dhcpv4-dual-stack.md
- Plans/DISPATCH-2026-05-30-phase-4-build-3-ipv4-dhcpv4-jarvis.md

---

## Decision

All five §10 open points of the BUILD-3 PM-DECISION are **ratified as
recommended**. No exceptions.

| # | Point | Ratified |
|---|-------|----------|
| 1 | Q1 — ARP hand-rolled in `net/arp.rs` (reject smoltcp iface ARP) | ✓ |
| 2 | Q2 — DHCPv4 hand-rolled in `net/dhcpv4.rs` (reject smoltcp dhcpv4 socket) | ✓ |
| 3 | Q3 / §4.5 — RFC 6724 in a new neutral `net/interface.rs`; bounded restructuring of `ipv6.rs` (4 touch points); per-family tables preserved | ✓ |
| 4 | Q4 — minimal RFC 5227 ACD; on-wire conflict deferred to bare-metal 3f | ✓ |
| 5 | Q5 — real virtio-net wire TX in scope as 3e, gated to the egress floor | ✓ |

## Effect

- The "Authorized on PL ratification" items in PM-DECISION §9 are now
  authorized: the restructuring approach (Q3/§4.5) and the wire-TX scope
  (Q5/3e) as the BUILD-3 close gate.
- The Jarvis dispatch
  (`DISPATCH-2026-05-30-phase-4-build-3-ipv4-dhcpv4-jarvis.md`) is **live** —
  its only remaining gate was PM-DECISION + PLAN on the branch (satisfied) plus
  this §10 ratification. Jarvis is cleared to create
  `codex/phase-4-build-3-ipv4-dhcpv4-jarvis` from `d3f09bc` and begin 3a→3f.
- PM (Cerebro) remains in the PM seat: stands by for the Jarvis RETURN, then
  dispositions and merges per the BUILD-2 precedent.
