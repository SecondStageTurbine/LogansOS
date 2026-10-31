# RETURN: Phase 4 BUILD-3 - IPv4 + ARP + DHCPv4 + dual-stack

**Date:** 2026-05-30
**From:** Jarvis
**To:** PM (Cerebro)
**Branch:** `codex/phase-4-build-3-ipv4-dhcpv4-jarvis`
**Status:** COMPLETE

## Sub-Commits

- `4a40e43` `build-3a: ipv4 header + arp cache (rfc 826)`
- `f60d239` `build-3b: dhcpv4 state machine + lease timers (rfc 2131/2132)`
- `3365eed` `build-3c: dual-family interface + rfc 6724 source selection`
- `218421e` `build-3d: send_to source-selection hook + ipv4 loopback`
- `19e466d` `build-3e: virtio-net ipv4/udp wire tx + arp resolution`
- BUILD-3f: pending final commit `build-3f: build-3 sweep + bare-metal deferral marker`

## ISC Results

- PASS ISC-3a-1 IPv4 header round-trip + checksum
- PASS ISC-3a-2 ARP reply populates cache
- PASS ISC-3a-3 ARP cache ages out
- PASS ISC-3a-4 no panic / defensive parsing
- PASS ISC-3b-1 OFFER advances + extracts lease fields
- PASS ISC-3b-2 ACK -> BOUND + lease deadline
- PASS ISC-3b-3 T1 -> RENEWING via tick
- PASS ISC-3b-4 ACD stub deterministic, no panic
- PASS ISC-3c-1 v4 dst -> v4 src / v6 dst -> v6 src
- PASS ISC-3c-2 prefer-IPv6 default
- PASS ISC-3c-3 SLAAC scenario regression green
- PASS ISC-3c-4 explicit-bind bypass
- PASS ISC-3d-1 IPv4 loopback round-trip
- PASS ISC-3d-2 wildcard -> DHCP src in egress
- PASS ISC-3d-3 cross-family reject regression
- PASS ISC-3e-1 correct frame egresses (injected ACK + ARP reply)
- PASS ISC-3e-2 ARP-miss queue + flush
- PASS ISC-3e-3 SMOKE + NEO_SMP=4
- PASS ISC-3f-1 no new panic/unwrap
- PASS ISC-3f-2 sweep green
- PASS ISC-3f-3 bare-metal tracked

## Verification

```text
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-phase4-dhcpv4-ipv4-udp
```

Result: PASS. Final rerun completed with 164 existing warnings.

```text
docker compose run --rm neo cargo neo-test phase4_dhcpv4_ipv4_udp
```

Result: PASS. QEMU harness validated synthetic DHCP ACK lease install, ARP miss
queueing, injected ARP reply flush, and observed Ethernet+IPv4+UDP peer egress.

```text
rg -n "unwrap\(|expect\(|panic!|todo!|unimplemented!|unreachable!" kernel/src/net/ipv4.rs kernel/src/net/arp.rs kernel/src/net/dhcpv4.rs kernel/src/net/interface.rs
```

Result: PASS. No matches; `rg` exited 1 as expected for an empty match set.

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_dhcpv4_ipv4_udp --no-meta
```

Result: PASS.

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Result: PASS, exit code 0. Existing warning counts varied across test builds
(205-220 observed), with no BUILD-3 failures.

## Code-Review Notes

- No new socket/syscall ABI was added; the existing `send_to` surface was reused.
- ARP and DHCPv4 are hand-rolled under `kernel/src/net/arp.rs` and
  `kernel/src/net/dhcpv4.rs`; no smoltcp `iface::Interface` adoption.
- Per-family address tables remain separate. The dual-family layer in
  `kernel/src/net/interface.rs` aggregates selection without re-typing the
  IPv6 `InterfaceAddress` model into a tagged union.
- RFC 6724 implementation is scoped to the load-bearing BUILD-3 rules: exact
  family match, appropriate scope, outgoing interface preference, and IPv6
  default preference where both families are otherwise viable.
- IPv4 UDP egress currently emits a zero UDP checksum, which is valid for IPv4
  UDP and keeps BUILD-3 scoped to the requested egress floor.
- Bare-metal live-network behavior is tracked in
  `Plans/BUILD-3f-BARE-METAL-DEFERRAL-2026-05-30.md`: real DHCPv4 server lease,
  real ARP peer exchange, real IPv4 UDP reply reception, and full on-wire
  RFC 5227 conflict/probe/defend behavior.

## Blockers

None.
