# PM-DECISION: Phase 4 BUILD-4.5 opening — IPv6 Neighbor Discovery (NDP)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — opens BUILD-4.5; §10 open points for PL ratification
**Authority chain:**
- CLOSE-REPORT-2026-05-30-phase-4-build-4.md — FM-4-1 (IPv6 NDP gap) raised
- PM-DECISION-2026-05-30-phase-4-build-3-opening.md Q1 (hand-rolled ARP precedent)
- Phase 4 THINK v0.2 (IPv6-default; hand-rolled control plane posture)
- RFC 4861 (Neighbor Discovery for IPv6)

---

## §1. Why this build exists

BUILD-4 closed FM-4-1: IPv6 TCP (and UDP) wire egress frames correctly but
uses a **placeholder destination MAC**, because IPv6 has no ARP and its
equivalent — Neighbor Discovery (NDP, RFC 4861) — was never built. IPv4
resolves the next-hop MAC via the ARP cache (`arp.rs`); IPv6 cannot resolve a
real on-link neighbor. **Real IPv6 connectivity (TCP and UDP) depends on NDP.**

BUILD-5 (DNS + Happy Eyeballs + kTLS) ships the phase with an HTTPS fetch from
a real dual-stack server. On the IPv6 leg that requires resolving a real
next-hop, so NDP is a **BUILD-5 prerequisite**. PL elected (2026-05-30) to slot
this focused build in before BUILD-5. It is the IPv6 analogue of BUILD-3a's ARP
work — small and well-bounded.

## §2. Substrate

- **BUILD-4 close:** `fb690c9` (TCP). NDP branches from `origin/main` **after
  BUILD-4 is merged** (gating prereq — NDP integrates with `tcp.rs` IPv6 egress
  and the BUILD-3 IPv6 UDP egress).
- **Template:** `arp.rs` on main — `ArpCache` (lookup/insert/age_out, TTL),
  `PENDING` queue, `parse`/`build`, `inject` test hook, `submit_or_queue_ipv4`
  (miss → queue), `flush_pending` (resolve → fill dst MAC → submit), `tick`.
- **Framing:** `icmpv6.rs` knows ICMPv6 RA (type 134) + the option TLV walk.
  NDP adds NS (135) and NA (136) + the source/target link-layer-address option.
- **Integration point:** `tcp.rs:1240 ipv6_destination_mac()` (placeholder) and
  the IPv6 UDP egress; both to route through an NDP resolve/queue, exactly as
  `arp::lookup`/`submit_or_queue_ipv4` serve IPv4 (`tcp.rs:1150`).

## §3. Load-bearing decisions

### Q1 — implementation
**Hand-roll `net/ndp.rs`**, mirroring `arp.rs`; smoltcp stays phy-only. (No
alternative consistent with the ratified posture; smoltcp NDP would require its
`Interface`.)

### Q2 — neighbor cache depth
**Minimal:** address→MAC cache with TTL aging (reachable + expire), mirroring
`ArpCache` and the minimal-DAD precedent. The full RFC 4861 NUD state machine
(INCOMPLETE/REACHABLE/STALE/DELAY/PROBE + reachability confirmation) is
**deferred** — not needed for on-link next-hop resolution at this scope.
*Rejected:* full NUD now — scope creep beyond the ARP-parity goal.

### Q3 — DAD
**Keep BUILD-2's minimal DAD stub.** Do not upgrade to NS-based Duplicate
Address Detection in this build. (Now that NS exists, real DAD becomes a small
future add — forward-marker FM-4.5-1.)

### Q4 — integration shape
A shared `ndp::submit_or_queue_ipv6(dst_ip, frame, ...)` + `ndp::lookup` used by
**both** the IPv6 UDP and TCP egress paths, exactly as ARP serves both for
IPv4. On a cache miss: emit a Neighbor Solicitation to the **solicited-node
multicast** address (ff02::1:ffXX:XXXX, L2 33:33:ff:XX:XX:XX), queue the
datagram, flush on the matching Neighbor Advertisement.

### Q5 — Router Solicitation
**Out of scope.** RA *reception* already works (BUILD-2); proactive Router
Solicitation (RS) is not needed here. Forward-marker FM-4.5-2 if proactive RS
is wanted later.

## §4. Sub-commit split with ISCs

### BUILD-4.5a — `net/ndp.rs` (NS/NA + neighbor cache)
**Scope:** NS (135) / NA (136) parse+build with ICMPv6 pseudo-header checksum;
`NeighborCache` (ipv6→MAC, TTL, lookup/insert/age_out) mirroring `ArpCache`;
solicited-node multicast address + multicast MAC derivation; `ndp::inject` test
hook; `ndp::tick`. Register in `mod.rs`.
**Anti-scope:** no egress integration (4.5b), no full NUD, no DAD change, no RS.
- **ISC-4.5a-1:** NS/NA build+parse round-trip; ICMPv6 checksum verifies.
- **ISC-4.5a-2:** an injected NA populates the cache; `lookup` returns the MAC;
  solicited-node multicast (addr + 33:33-prefixed MAC) derivation correct.
- **ISC-4.5a-3:** cache entries age out via `tick`.
- **ISC-4.5a-4:** no unwrap/panic; defensive length checks.

### BUILD-4.5b — IPv6 egress integration + wire + sweep
**Scope:** replace `tcp.rs ipv6_destination_mac` placeholder and the IPv6 UDP
egress with `ndp::lookup` + `submit_or_queue_ipv6` (miss → NS + queue → flush on
NA); wire `ndp::tick` into `net::tick`; QEMU scenario
`phase4_ndp_ipv6_neighbor`; full sweep; bare-metal marker (real on-link neighbor).
- **ISC-4.5b-1:** IPv6 UDP/TCP egress to an on-link neighbor resolves the dst
  MAC from the cache — no placeholder (host/QEMU).
- **ISC-4.5b-2:** cache miss emits an NS to the solicited-node multicast and
  queues the datagram; an injected NA flushes it (mirror ARP ISC-3e-2).
- **ISC-4.5b-3:** scenario passes SMOKE + NEO_SMP=4; full `neo-test all` green.
- **ISC-4.5b-4:** existing IPv6 SLAAC/UDP/TCP scenarios still green
  (regression); no new unwrap/panic.

## §5. /simplify review

Hand-rolled, mirror `arp.rs` (Q1). Minimal cache, no NUD (Q2). DAD unchanged
(Q3). One shared resolve/queue for UDP+TCP (Q4). No RS (Q5). Two sub-commits.

## §6. Risk cross-reference

| Item | Owner | Comment |
|---|---|---|
| FM-4-1 (IPv6 NDP gap) | 4.5a/4.5b | This build closes it for on-link neighbors. |
| R1 (smoltcp posture) | 4.5a | Reaffirmed: hand-roll, phy-only. |
| FM-4.5-1 (real DAD), FM-4.5-2 (RS) | — | New forward-markers; deferred. |

## §7. Authorization boundary

- **Authorized now:** PLAN + DISPATCH drafting; Jarvis begins 4.5a (host-testable)
  once BUILD-4 is on main and PLAN + this PM-DECISION are on the branch.
- **Authorized on PL ratification (§10):** the minimal-cache depth (Q2) and the
  DAD/RS deferrals (Q3/Q5) as the close gate.
- **Not authorized:** smoltcp iface; full NUD; DAD rework; new syscalls (NDP is
  internal — no ABI surface).

## §8. Required next artifacts

1. PLAN — `Plans/PLAN-2026-05-30-phase-4-build-4.5-ndp.md`.
2. DISPATCH — `Plans/DISPATCH-2026-05-30-phase-4-build-4.5-ndp-jarvis.md`.
3. INDEX.md amendment.

## §9. Open points for ratification (§10)

1. **Q1** hand-rolled `net/ndp.rs` mirroring `arp.rs` (phy-only).
2. **Q2** minimal neighbor cache (TTL aging), full RFC 4861 NUD deferred.
3. **Q3** BUILD-2 minimal DAD unchanged (real NS-based DAD deferred, FM-4.5-1).
4. **Q4** shared `ndp` resolve/queue for both IPv6 UDP and TCP egress;
   solicited-node multicast NS on miss.
5. **Q5** no Router Solicitation (RA reception already works; FM-4.5-2).

---

**BUILD-4.5 opens as the IPv6 ARP-parity build, closing FM-4-1 ahead of BUILD-5.
PLAN and DISPATCH follow in this package.**
