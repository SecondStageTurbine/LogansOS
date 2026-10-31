# PLAN: Phase 4 BUILD-3 — IPv4 + ARP + DHCPv4 + dual-stack source selection (RFC 6724)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — detailed spec for the BUILD-3 sub-commit chain
**Anchored on:** PM-DECISION-2026-05-30-phase-4-build-3-opening.md (Q1–Q5, §4.5)
**Implements:** THINK v0.2 §5 "IPv4 + DHCPv4 + dual-stack address selection"

---

## Context

BUILD-2 delivered an IPv6-only stack: hand-rolled SLAAC/RA control plane
(`ipv6.rs`, `icmpv6.rs`), UDP sockets with loopback delivery + egress capture
(`socket.rs`), smoltcp wired as a phy device only. BUILD-3 adds the IPv4
family to parity with IPv6 UDP, plus the two acquisition/resolution protocols
IPv4 needs (DHCPv4, ARP) and the cross-family source-address selection that
dual-stack requires (RFC 6724). It also closes the wire-TX gap so IPv4-to-host
datagrams actually leave the NIC.

**Goal milestone (THINK):** UDP over IPv4 to remote IPv4 hosts; dual-stack
source-address selection per RFC 6724; DHCPv4 (RFC 2131) for address
acquisition.

## ISCs (full text)

Carried verbatim from PM-DECISION §5. Pass/fail gates per sub-commit:

- **3a:** ISC-3a-1 IPv4 hdr round-trip+checksum · ISC-3a-2 ARP reply populates
  cache · ISC-3a-3 cache ages out · ISC-3a-4 no panic/defensive parsing.
- **3b:** ISC-3b-1 OFFER advances+extracts · ISC-3b-2 ACK→BOUND+lease ·
  ISC-3b-3 T1→RENEWING · ISC-3b-4 ACD stub deterministic, no panic.
- **3c:** ISC-3c-1 v4 dst→v4 src / v6 dst→v6 src · ISC-3c-2 prefer-IPv6
  default · ISC-3c-3 SLAAC scenario regression green · ISC-3c-4 explicit-bind
  bypass.
- **3d:** ISC-3d-1 IPv4 loopback round-trip · ISC-3d-2 wildcard→DHCP src in
  egress · ISC-3d-3 cross-family reject regression.
- **3e:** ISC-3e-1 correct frame egresses after injected ACK+ARP reply ·
  ISC-3e-2 ARP-miss queue+flush · ISC-3e-3 SMOKE + NEO_SMP=4.
- **3f:** ISC-3f-1 no new panic/unwrap · ISC-3f-2 sweep green or known
  perf-lane only · ISC-3f-3 bare-metal tracked.

## Non-scope

TCP; DHCPv6; smoltcp `iface::Interface` adoption; RFC 6724 Rules 3/4/6/7/8;
on-wire RFC 5227 conflict detection; real DHCP/ARP peer round-trip (all
bare-metal or later BUILDs). No new socket syscalls unless a test needs IPv4
status visibility — if so, append at 0x189 (0x188 reserved for connect).

## Implementation sketch

### 3a — `net/ipv4.rs` + `net/arp.rs`
- **`ipv4.rs`:** `Ipv4Header` build/parse; ones-complement header checksum
  (verify + emit); next-protocol constant for UDP (17); a `Ipv4AddressTable`
  skeleton (address, netmask, gateway, lease deadline) — populated in 3b/3c.
  Mirror the defensive byte-offset style of `icmpv6.rs::parse_*` (length-check
  before every slice).
- **`arp.rs`:** `ArpPacket` parse/build for Ethernet/IPv4 (htype=1, ptype=0x0800,
  opcode request/reply); `ArpCache` = `BTreeMap<[u8;4], (([u8;6]), expiry_tick)>`
  under a `spin::Mutex`, with `lookup`, `insert`, and tick-driven `age_out`;
  `arp::inject(frame)` test hook consuming a synthetic reply (precedent:
  `ipv6::inject_router_advertisement`).
- **Acceptance:** ISC-3a-1…3a-4. All host-testable.

### 3b — `net/dhcpv4.rs`
- DHCP message build/parse (BOOTP frame + magic cookie + RFC 2132 options:
  53 msg-type, 1 subnet, 3 router, 51 lease, 6 DNS, 50 requested-IP, 54
  server-id). State enum: `Init → Selecting(DISCOVER sent) → Requesting(OFFER
  seen) → Bound(ACK seen) → Renewing(T1) → Rebinding(T2)`.
- Kernel-owned `Lease { ip, mask, router, dns, t1, t2, expiry }`; tick-driven
  timer transitions. `inject_offer(frame)` / `inject_ack(frame)` hooks.
- Minimal RFC 5227 ACD: a `Probing`/`Announcing` sub-state with a stub timeout
  that, absent an injected conflict, transitions straight to Bound (mirror
  `DAD_TIMEOUT_TICKS`).
- **Acceptance:** ISC-3b-1…3b-4. All host-testable via injection + synthetic tick.

### 3c — `net/interface.rs` (the restructuring commit)
- New neutral aggregate: an interface view holding a reference/handle to the
  IPv6 `InterfaceAddressTable` and the IPv4 `Ipv4AddressTable`. Move the
  `INTERFACES`-level aggregation here; `ipv6.rs` and `ipv4.rs` remain
  per-family table owners.
- `net::tick()` fan-out → `ipv6::tick()` + `ipv4::tick()` (DHCP lease + ARP
  aging). The boot/timer caller switches to `net::tick()`.
- `select_source(dst: SocketAddr) -> Option<SocketAddr>`:
  - dst family selects the candidate table.
  - RFC 6724 Rule 1 (prefer same address), Rule 2 (matching scope), Rule 5
    (prefer the address whose family matches dst).
  - Skip deprecated / tentative / temporary-expired addresses (reuse the
    `AddressState` filtering already in `ipv6.rs`); this discharges R13.
  - Tie/dual-stack-reachable default: prefer IPv6 (Bible Art. 11).
  - `preferred_global_address` becomes one input to this function.
- **Acceptance:** ISC-3c-1…3c-4. ISC-3c-3 is the SLAAC regression gate.

### 3d — `send_to` hook + IPv4 loopback
- In `socket.rs::send_to`, when the socket's bound source is the
  unspecified/wildcard address, call `net::interface::select_source(dst)` to
  obtain the source; otherwise keep the bound source (preserve `socket.rs:168`).
- Record the selected source in `LAST_EGRESS` for non-loopback IPv4.
- IPv4 loopback (`127.0.0.1`) already routes via `BOUND_SOCKETS`; add the
  round-trip scenario coverage.
- **Acceptance:** ISC-3d-1…3d-3.

### 3e — real virtio-net wire TX
- Non-loopback `send_to`: assemble Ethernet (dst MAC from ARP cache, src MAC
  from `NetDevice::mac_address`) + IPv4 (checksum from 3a) + UDP; submit via
  `NetDevice::tx_submit` / the `SmolDevice` TX token.
- ARP cache miss: enqueue the pending datagram, emit an ARP request frame,
  flush on injected reply (RFC 826).
- DHCP DISCOVER/REQUEST routed through the same TX path.
- QEMU scenario `phase4_dhcpv4_ipv4_udp`: inject OFFER/ACK + ARP reply, assert
  a correctly-framed datagram egressed (egress capture / virtio TX counter).
- **Acceptance:** ISC-3e-1…3e-3. **QEMU-only.**

### 3f — sweep + bare-metal marker
- Full `cargo neo-test` sweep; quality pass (panic/unwrap scan over the four
  new modules); file the BUILD-3f bare-metal deferral (real lease/ARP/reply/
  RFC 5227 on wire), mirroring BUILD-2e.
- **Acceptance:** ISC-3f-1…3f-3. **QEMU-only.**

## Risks and mitigations

- **Scenario: IPv4 lands and the SLAAC path regresses via the tick fan-out.**
  Mitigation: ISC-3c-3 gates on `phase4_slaac_udp_ipv6_loopback` passing
  unchanged after `net::tick()` replaces the direct `ipv6::tick()` caller.
- **Scenario: source selection picks a deprecated/temporary-expired address
  (R13).** Mitigation: `select_source` filters on `AddressState` exactly as
  `preferred_global_address` does today; ISC-3c-1 asserts the preferred global.
- **Scenario: restructuring balloons beyond the 4 touch points (§4.5).**
  Mitigation: per-family tables stay; `InterfaceAddress` is not re-typed; if a
  fifth structural touch appears, Jarvis files a BLOCKER rather than expanding
  scope silently.
- **Scenario: wire TX can't be validated without a real peer.** Mitigation:
  egress floor (ISC-3e-1) asserts frame correctness, not round-trip; real
  reply is bare-metal 3f.

## Host-testable vs QEMU

- **Host (`cargo check` + unit tests), runnable in any environment:** all of
  3a, 3b, 3c, 3d. The injection hooks (`arp::inject`, `dhcpv4::inject_*`)
  follow the sanctioned `inject_router_advertisement` precedent — deterministic,
  no tap/radvd/dnsmasq.
- **QEMU harness required:** 3e, 3f (real virtio-net TX ring; SMOKE +
  NEO_SMP=4). Egress observed via `socket::debug_last_egress()` and/or virtio
  TX counters. *(Note: the web/session environment has no QEMU; 3e/3f run on a
  worker seat with the QEMU harness.)*
- **Bare metal (deferred, BUILD-3f tracked):** real DHCP lease, on-wire ARP,
  real UDP reply, RFC 5227 against a live claimant.

## Sequencing estimate

Sequential 3a→3f. Rough effort: 3a ~1 unit, 3b ~1.5 (option parsing), 3c ~1.5
(restructuring + RFC 6724), 3d ~0.5, 3e ~1.5 (wire TX + ARP queueing), 3f ~0.5.
3a–3d verifiable on the worker without QEMU; 3e–3f need the harness.

## Artifacts

- **Input:** PM-DECISION-2026-05-30-phase-4-build-3-opening.md; THINK v0.2;
  BUILD-2 code (`ipv6.rs`, `icmpv6.rs`, `socket.rs`, `device.rs`,
  `smoltcp_wrap.rs`, `virtio_net.rs`).
- **Output (Jarvis):** new `kernel/src/net/{ipv4,arp,dhcpv4,interface}.rs`;
  amendments to `ipv6.rs`, `socket.rs`, `mod.rs`, and the boot tick caller;
  scenario `phase4_dhcpv4_ipv4_udp`; RETURN-2026-…-phase-4-build-3-jarvis.md.
