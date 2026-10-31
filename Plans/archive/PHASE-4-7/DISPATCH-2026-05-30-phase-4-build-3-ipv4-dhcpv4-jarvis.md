# DISPATCH: Phase 4 BUILD-3 — IPv4 + ARP + DHCPv4 + dual-stack (RFC 6724)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**To:** Jarvis
**Repo:** E:\AI Automation\AltimitNeo (GitHub: SecondStageTurbine/NeoPlutarch)
**Branch:** `codex/phase-4-build-3-ipv4-dhcpv4-jarvis` (create from origin/main at `d3f09bc`)
**Author:** SecondStageTurbine

---

## Your assignment

Implement Phase 4 BUILD-3: the IPv4 family at parity with IPv6 UDP, plus ARP
(RFC 826) and DHCPv4 (RFC 2131/2132) for IPv4 address acquisition/resolution,
plus cross-family source-address selection (RFC 6724), plus real virtio-net
wire transmission for non-loopback UDP.

**Goal milestone:** UDP over IPv4 to remote IPv4 hosts; dual-stack
source-address selection per RFC 6724; DHCPv4 for IPv4 address acquisition.

## Before you start

1. `git fetch origin main` and create the branch from `origin/main` at `d3f09bc`.
2. Read **PLAN-2026-05-30-phase-4-build-3-ipv4-dhcpv4-dual-stack.md** (full spec)
   and **PM-DECISION-2026-05-30-phase-4-build-3-opening.md** (Q1–Q5, §4.5).
3. Understand the BUILD-2 baseline by reading:
   - `kernel/src/net/ipv6.rs` — SLAAC state machine, `INTERFACES`, `tick()`,
     `preferred_global_address`, `AddressState` filtering.
   - `kernel/src/net/icmpv6.rs` — the byte-offset parser template you will mirror.
   - `kernel/src/net/socket.rs` — `send_to`, `is_loopback`, `BOUND_SOCKETS`,
     `LAST_EGRESS`, `SocketAddr` (`Inet4` already parses).
   - `kernel/src/net/device.rs` + `kernel/src/net/smoltcp_wrap.rs` — the
     `NetDevice` TX/RX trait + smoltcp phy adapter (TX path for 3e).
   - `kernel/src/drivers/net/virtio_net.rs` — TX submission + debug counters.
   - `kernel/src/arch/x86_64/syscall.rs:69–76` — socket syscall ledger.

**Hard constraints (from PM-DECISION §9):** no smoltcp `iface::Interface`
adoption; hand-rolled control plane only; per-family address tables preserved
(do NOT re-type `InterfaceAddress` into a tagged union); no new socket syscall
unless a test needs IPv4 status — if so, append at **0x189** (0x188 reserved).

## Sub-commits (6 total, sequential 3a → 3f)

### BUILD-3a: IPv4 plumbing + ARP
- **Create:** `kernel/src/net/ipv4.rs` (NEW), `kernel/src/net/arp.rs` (NEW).
- **Amend:** `kernel/src/net/mod.rs` (AMEND — `pub mod ipv4; pub mod arp;`).
- IPv4 header build/parse + ones-complement checksum; `Ipv4AddressTable`
  skeleton. ARP parse/build (RFC 826), aging `ArpCache`, `arp::inject` hook.
- **Closes:** ISC-3a-1…3a-4.
- **Commit:** `build-3a: ipv4 header + arp cache (rfc 826)`

### BUILD-3b: DHCPv4 state machine
- **Create:** `kernel/src/net/dhcpv4.rs` (NEW).
- **Amend:** `kernel/src/net/mod.rs` (AMEND).
- DISCOVER→OFFER→REQUEST→ACK + RFC 2132 options + lease T1/T2 timers +
  `inject_offer`/`inject_ack` + minimal RFC 5227 ACD stub.
- **Closes:** ISC-3b-1…3b-4.
- **Commit:** `build-3b: dhcpv4 state machine + lease timers (rfc 2131/2132)`

### BUILD-3c: interface unification + RFC 6724 (restructuring)
- **Create:** `kernel/src/net/interface.rs` (NEW).
- **Amend:** `kernel/src/net/ipv6.rs` (AMEND — aggregation moves out; `tick`
  becomes per-family), `kernel/src/net/mod.rs` (AMEND), boot tick caller
  (AMEND — switch to `net::tick()`).
- Neutral dual-family aggregate; `net::tick()` fan-out; `select_source(dst)`
  (RFC 6724 Rules 1/2/5 + prefer-IPv6 default, skipping deprecated/expired).
- **Closes:** ISC-3c-1…3c-4. **ISC-3c-3 is the SLAAC regression gate — do not
  break `phase4_slaac_udp_ipv6_loopback`.**
- **Commit:** `build-3c: dual-family interface + rfc 6724 source selection`

### BUILD-3d: send_to hook + IPv4 loopback
- **Amend:** `kernel/src/net/socket.rs` (AMEND — `send_to` calls
  `select_source` for wildcard binds; explicit binds unchanged).
- **Closes:** ISC-3d-1…3d-3.
- **Commit:** `build-3d: send_to source-selection hook + ipv4 loopback`

### BUILD-3e: real virtio-net wire TX
- **Amend:** `kernel/src/net/socket.rs` (AMEND — non-loopback TX),
  `kernel/src/net/arp.rs` (AMEND — pending-datagram queue on miss),
  `kernel/src/net/dhcpv4.rs` (AMEND — DISCOVER/REQUEST on wire),
  test harness (AMEND — scenario `phase4_dhcpv4_ipv4_udp`).
- Ethernet+IPv4+UDP assembly, ARP-resolved dst MAC, `NetDevice::tx_submit`.
- **Closes:** ISC-3e-1…3e-3. **QEMU.**
- **Commit:** `build-3e: virtio-net ipv4/udp wire tx + arp resolution`

### BUILD-3f: sweep + bare-metal marker
- Full sweep; panic/unwrap scan over the four new modules; file the BUILD-3f
  bare-metal deferral artifact.
- **Closes:** ISC-3f-1…3f-3. **QEMU.**
- **Commit:** `build-3f: build-3 sweep + bare-metal deferral marker`

## ISC checklist (unchecked on filing)

- [ ] ISC-3a-1 IPv4 header round-trip + checksum
- [ ] ISC-3a-2 ARP reply populates cache
- [ ] ISC-3a-3 ARP cache ages out
- [ ] ISC-3a-4 no panic / defensive parsing
- [ ] ISC-3b-1 OFFER advances + extracts lease fields
- [ ] ISC-3b-2 ACK → BOUND + lease deadline
- [ ] ISC-3b-3 T1 → RENEWING via tick
- [ ] ISC-3b-4 ACD stub deterministic, no panic
- [ ] ISC-3c-1 v4 dst → v4 src / v6 dst → v6 src
- [ ] ISC-3c-2 prefer-IPv6 default
- [ ] ISC-3c-3 SLAAC scenario regression green
- [ ] ISC-3c-4 explicit-bind bypass
- [ ] ISC-3d-1 IPv4 loopback round-trip
- [ ] ISC-3d-2 wildcard → DHCP src in egress
- [ ] ISC-3d-3 cross-family reject regression
- [ ] ISC-3e-1 correct frame egresses (injected ACK + ARP reply)
- [ ] ISC-3e-2 ARP-miss queue + flush
- [ ] ISC-3e-3 SMOKE + NEO_SMP=4
- [ ] ISC-3f-1 no new panic/unwrap
- [ ] ISC-3f-2 sweep green or known perf-lane only
- [ ] ISC-3f-3 bare-metal tracked

## Key design points (from PM-DECISION Q1–Q5)

- **Q1 ARP:** hand-rolled `net/arp.rs`, NOT smoltcp iface ARP.
- **Q2 DHCPv4:** hand-rolled `net/dhcpv4.rs`, mirror the SLAAC machine.
- **Q3 RFC 6724:** lives in `net/interface.rs`; `send_to` only calls it for
  wildcard binds; per-family tables preserved.
- **Q4 ACD:** minimal stub (mirror minimal DAD); on-wire conflict → bare metal.
- **Q5 wire TX:** in scope (3e), gated to the egress floor — frame correctness,
  not round-trip.

## Failure modes to avoid

1. **Breaking the SLAAC scenario** when `tick()` becomes `net::tick()` — run
   `phase4_slaac_udp_ipv6_loopback` after 3c (ISC-3c-3).
2. **Re-typing `InterfaceAddress`** into a tagged union — explicitly out of
   scope; per-family tables stay (§4.5).
3. **Parser panics** — length-check before every slice, mirror `icmpv6.rs`.
4. **Scope creep into TCP / DHCPv6 / full RFC 6724** — load-bearing rules only.
5. **Silent restructuring beyond the 4 touch points** — file a BLOCKER instead.
6. **New syscalls off the 0x189 append discipline.**

## Testing expectations

- **3a–3d (host, no QEMU needed):** `cargo check` clean; host unit tests for
  header/checksum, ARP cache, DHCP state machine, source selection, loopback.
  Use the injection hooks (`arp::inject`, `dhcpv4::inject_*`) — deterministic,
  no tap/radvd/dnsmasq.
- **3e–3f (QEMU harness):** `cargo neo-test phase4_dhcpv4_ipv4_udp` on SMOKE
  and `NEO_SMP=4`; full sweep. Egress via `socket::debug_last_egress()` /
  virtio TX counters.

## Return artifact

File `Plans/RETURN-2026-05-30-phase-4-build-3-jarvis.md` on completion with:
sub-commit hashes + messages; ISC results (PASS/FAIL/BLOCKED each); exact
`cargo check` / test / Docker invocations + output; code-review notes (ABI
decisions, pattern alignment, anything left for BUILD-4); any blocker links.

## Dispatch assumptions

BUILD-2 on main (`05cff8d`); syswait race fixed (`c0a3b04`); perf-proxy
SMP-gated (`a4d37f2`); full NEO_SMP=4 sweep green; sendto/recvfrom ABI ratified.

## Escalation path

Surface blockers as `Plans/BLOCKER-2026-MM-DD-<description>.md` and stop on the
affected sub-commit rather than expanding scope. Restructuring touching a fifth
structural point (beyond §4.5's four) is a blocker, not a judgment call.

---

**Fetch origin/main, create the feature branch, read the PLAN, and begin
BUILD-3a. 3a–3d are fully verifiable without QEMU; pause for the harness at 3e.**
