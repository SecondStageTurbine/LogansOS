# PLAN: Phase 4 BUILD-4.5 — IPv6 Neighbor Discovery (NDP)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — spec for the BUILD-4.5 sub-commit chain
**Anchored on:** PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md (Q1–Q5)
**Closes:** FM-4-1 (BUILD-4 IPv6 placeholder destination MAC)

---

## Context

IPv4 resolves the next-hop MAC via `arp.rs`; IPv6 has no equivalent yet, so
BUILD-4's IPv6 wire egress used a placeholder destination MAC (FM-4-1).
BUILD-4.5 adds hand-rolled NDP (RFC 4861 Neighbor Solicitation/Advertisement +
a neighbor cache) — the IPv6 analogue of ARP — and routes IPv6 UDP/TCP egress
through it. smoltcp stays phy-only.

**Goal:** an IPv6 UDP/TCP datagram to an on-link neighbor egresses with a real,
NDP-resolved destination MAC; cache miss solicits, queues, and flushes on
advertisement.

## ISCs

Carried verbatim from PM-DECISION §4 (4.5a-1…4.5b-4). Pass/fail gates per
sub-commit; see the DISPATCH checklist.

## Non-scope

Full RFC 4861 NUD state machine; NS-based DAD (BUILD-2 stub stays); Router
Solicitation; Redirect; secure ND (RFC 3971); any syscall/ABI change (NDP is
internal to the egress path).

## Implementation sketch

### 4.5a — `net/ndp.rs`
Mirror `arp.rs` closely:
- **Codec:** parse/build Neighbor Solicitation (ICMPv6 type 135, target
  address + optional source link-layer-addr option type 1) and Neighbor
  Advertisement (type 136, target address + target link-layer-addr option type
  2, R/S/O flags). ICMPv6 checksum over the IPv6 pseudo-header (reuse the
  `tcp.rs`/`ipv4.rs` pseudo-header checksum approach; NDP is ICMPv6 so the
  checksum covers the IPv6 pseudo-header + ICMPv6 body).
- **`NeighborCache`:** `BTreeMap<[u8;16], NeighborEntry{ mac, expires_at }>`
  with `lookup(addr, now)` / `insert(addr, mac, now)` / `age_out(now)` and a
  TTL constant — direct analogue of `ArpCache`.
- **Solicited-node multicast:** address `ff02::1:ffXX:XXXX` from the low 24
  bits of the target; L2 multicast MAC `33:33:ff:XX:XX:XX`. Helper +
  unit-tested derivation.
- **Test hook:** `ndp::inject(frame)` consuming a synthetic NA (the RA/ARP
  injection precedent). `ndp::tick()` ages the cache.
- **Acceptance:** ISC-4.5a-1…4.5a-4. Host-testable.

### 4.5b — IPv6 egress integration
- **`ndp::lookup(addr) -> Option<[u8;6]>`** and
  **`ndp::submit_or_queue_ipv6(dst_ip, frame, interface_id)`** mirroring
  `arp::lookup` / `arp::submit_or_queue_ipv4`: on hit fill the dst MAC and
  submit; on miss emit an NS to the solicited-node multicast, push the frame to
  a `PENDING` queue (with a limit), and `flush_pending` on the matching NA.
- **Replace** `tcp.rs ipv6_destination_mac()` (the placeholder) and the IPv6
  UDP egress path with the NDP resolve/queue. Both protocols share it, exactly
  as they share ARP for IPv4.
- **Wire `ndp::tick()` into `net::tick()`** (mod.rs fan-out) alongside
  `arp::tick`.
- **Scenario** `phase4_ndp_ipv6_neighbor`: inject an NA, assert an IPv6 UDP/TCP
  frame egresses with the resolved MAC; assert miss→NS+queue→NA→flush.
- **Acceptance:** ISC-4.5b-1…4.5b-4. **QEMU** for the wire scenario.

## Risks and mitigations

- **Regression to IPv6 SLAAC/UDP/TCP egress.** Mitigation: ISC-4.5b-4 gates the
  existing IPv6 scenarios green after the egress path swaps to NDP resolve.
- **Loopback IPv6 must not solicit.** Mitigation: loopback (::1) and
  link-local-to-self short-circuit before NDP, mirroring `is_loopback` handling.
- **Checksum scope (ICMPv6 pseudo-header).** Mitigation: 4.5a unit test asserts
  NS/NA checksum round-trip.

## Host-testable vs QEMU

- **Host:** all of 4.5a (codec, cache, multicast derivation via `ndp::inject` +
  synthetic tick) and the miss/queue/flush logic of 4.5b.
- **QEMU:** the `phase4_ndp_ipv6_neighbor` wire scenario + SMOKE/NEO_SMP=4 +
  full sweep. *(PM/web env has no QEMU; runs on a worker seat.)*
- **Bare metal (deferred):** real on-link IPv6 neighbor solicit/advertise
  against a live host (folds into the BUILD-4g bare-metal lane).

## Sequencing

Sequential 4.5a → 4.5b. Small build (ARP-parity scope): 4.5a ~1 unit, 4.5b ~1.
4.5a host-verifiable on the worker; 4.5b wire needs the harness.

## Artifacts

- **Input:** PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md; `arp.rs`
  (template); `icmpv6.rs` (framing); `tcp.rs` IPv6 egress (`ipv6_destination_mac`
  at the placeholder); BUILD-3 IPv6 UDP egress; `net::tick`.
- **Output (Jarvis):** new `kernel/src/net/ndp.rs`; `tcp.rs`/IPv6-UDP egress
  swap to NDP resolve; `net::tick` fan-out; scenario `phase4_ndp_ipv6_neighbor`;
  RETURN-2026-…-phase-4-build-4.5-jarvis.md.
