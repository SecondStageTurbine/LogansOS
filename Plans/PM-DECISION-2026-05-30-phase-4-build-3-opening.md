# PM-DECISION: Phase 4 BUILD-3 opening — IPv4 + ARP + DHCPv4 + dual-stack (RFC 6724)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — opens Phase 4 BUILD-3; §10 open points for PL ratification
**Authority chain:**
- Phase 4 THINK v0.2 (Plans/PHASE-4-THINK-v0.2-2026-05-22-...) — ratified 2026-05-22
- PM-DECISION-2026-05-28-phase-4-build-1-opening.md (BUILD structuring precedent)
- CLOSE-REPORT-2026-05-29-phase-4-build-2.md (BUILD-2 close; BUILD-3 declared NEXT)
- PL-RATIFICATION-2026-05-30-phase-4-build-2-send-destination-abi-close.md (ABI flag closed)
- Bible Article 11 (IPv6-default posture)

---

## §1. Substrate

BUILD-3 opens on a green substrate.

- **BUILD-2 close:** merge `05cff8d` (IPv6 SLAAC + UDP sendto/recvfrom + net stats).
- **origin/main HEAD:** `d3f09bc`.
- **Gate-clearing landings since BUILD-2 close (all on main):**
  - `c0a3b04` — `sys_wait` multi-waiter SMP race fixed (real intrinsic race,
    not a Phase-4 regression). 50/50 at NEO_SMP=4; no retry masking.
  - `a4d37f2` — perf-proxy drift root-caused as **verdict A** (stale,
    SMP-naive Phase-2 fixture, not a kernel regression) and SMP-gated.
  - `3833445`/`cfea7c8`/`f73ceba` — MPL-2.0 license reversal + SPDX CI lint.
- **Full NEO_SMP=4 sweep:** green (perf-proxy lane gated, syswait race fixed).
- **In flight:** nothing. No dispatch is open.

**Prerequisite checklist:**

| # | Prerequisite | State |
|---|--------------|-------|
| P1 | BUILD-2 functional ISCs (2-1…2-7) green | ✓ |
| P2 | Full NEO_SMP=4 sweep green (syswait + perf-proxy cleared) | ✓ |
| P3 | sendto/recvfrom ABI ratified (open item (a) closed) | ✓ (this session) |
| P4 | smoltcp 0.13 vendored, virtio-net driver live | ✓ |
| P5 | No active worker dispatch to collide with | ✓ |

The pre-Phase-5 perf-proxy *re-calibration study* (verdict-A follow-up) and
the BUILD-2e bare-metal RA deferral remain TRACKED; neither gates BUILD-3.

## §2. THINK v0.2 scope anchor

THINK v0.2 §5 reserves this scope under its "BUILD-2 — IPv4 + DHCPv4 +
dual-stack address selection" entry (the shipped build numbering advanced by
one when IPv6-SLAAC+UDP became its own build, so THINK's "BUILD-2" is the
shipped **BUILD-3**). Goal milestone, quoted:

> UDP over IPv4 to remote IPv4 hosts. Dual-stack source-address selection per
> RFC 6724. DHCPv4 (RFC 2131) for IPv4 address acquisition.

Supporting RFCs: RFC 826 (ARP), RFC 2131/2132 (DHCPv4 + options), RFC 6724
(default source-address selection), RFC 5227 (Address Conflict Detection).

THINK v0.2 line ~174 carries the load-bearing warning for this build:

> BUILD-1 → BUILD-2 should be planned with the expectation that some
> IPv6-first protocol code may need restructuring when IPv4 lands, not the
> easier expectation of additive overlay.

§4.5 discharges that warning explicitly.

## §3. Current substrate observations (code state)

Established by direct read of `origin/main`:

- **smoltcp is phy/parsing-only and IPv6-only today.** `kernel/Cargo.toml:47`
  enables `features = ["medium-ethernet", "proto-ipv6", "socket-raw"]` — no
  `proto-ipv4`, no `socket-dhcpv4`. No `smoltcp::iface::Interface` is ever
  instantiated; `SmolDevice` (smoltcp_wrap.rs) is a bare `phy::Device`. This
  is the ratified "hand-rolled control plane, smoltcp as phy/parsing only"
  posture, made concrete.
- **The control plane is hand-rolled.** RA parsing is a byte-offset parser in
  `icmpv6.rs`; the SLAAC state machine, address table, and timers live in
  `ipv6.rs`. The test path injects synthetic RA frames straight into
  `ipv6::inject_router_advertisement()` — not through the virtio RX ring.
- **`INTERFACES` is IPv6-only** (`ipv6.rs:21`): `Mutex<Vec<InterfaceAddressTable>>`
  over `[u8;16]` IPv6 addresses + routes + RA-timeout state. No IPv4 analogue.
- **`send_to` does not transmit non-loopback** (`socket.rs:181–188`): it
  records `LAST_EGRESS` and returns `Ok(len)`. This egress-capture floor was
  sanctioned for BUILD-2; real wire TX was deferred.
- **`SocketAddr::Inet4` already exists** (`socket.rs:43–52`, parses 8-byte
  AF_INET sockaddr), `is_loopback` already handles `127.0.0.1` (`socket.rs:267`),
  and `BOUND_SOCKETS` is keyed `(family,port,addr)` — so IPv4 binds and IPv4
  **loopback delivery already work** through the generic path. The missing
  pieces are address acquisition (DHCP), L2 resolution (ARP), cross-family
  source selection, and real wire TX.
- **Source selection does not exist.** `send_to` takes `src` directly from the
  socket's `Bound(src)` state (`socket.rs:168–172`). A wildcard-bound socket
  has no selection logic; RFC 6724 has no home yet.
- **Syscall ledger:** socket syscalls occupy 0x180–0x187 (`syscall.rs:69–76`);
  0x188 reserved for `connect`. Next free for any BUILD-3 status syscall: 0x189.

## §4. Load-bearing PLAN-time decisions

### Q1 — ARP ownership
**Recommendation:** hand-rolled ARP cache + request/reply in a new
`net/arp.rs`, mirroring the `icmpv6.rs` byte-parser template.
**Rationale:** smoltcp's ARP is only reachable by adopting
`smoltcp::iface::Interface`, which hands smoltcp the entire L3 control plane —
contradicting the ratified posture and forking the architecture into two
incompatible control planes (hand-rolled IPv6 + smoltcp IPv4). RFC 826 is
small (one request/reply opcode pair, one aging cache).
**Residual risk:** low; ARP is well-bounded. Cross-ref THINK R1 (wrap-vs-fork).
**Rejected:** smoltcp `Interface` ARP — integration cost (neighbor cache,
route table, poll loop owned by smoltcp) dwarfs hand-rolling RFC 826.

### Q2 — DHCPv4 implementation
**Recommendation:** hand-rolled DISCOVER→OFFER→REQUEST→ACK state machine in a
new `net/dhcpv4.rs`, directly mirroring the SLAAC RA state machine in
`ipv6.rs`, with `inject_offer`/`inject_ack` test hooks mirroring
`inject_router_advertisement`.
**Rationale:** symmetry with the hand-rolled SLAAC machine; deterministic,
QEMU-free state-machine tests via the injection precedent. smoltcp's dhcpv4
socket again requires the iface.
**Residual risk:** low. DHCPv4 option parsing (RFC 2132) is the main surface.
**Rejected:** smoltcp `dhcpv4::Socket` — requires the iface; breaks symmetry.

### Q3 — RFC 6724 source-address selection placement
**Recommendation:** a new `net/interface.rs` owning a neutral per-interface
view that aggregates the IPv6 table (`ipv6::InterfaceAddressTable`) and a new
IPv4 table (`ipv4::Ipv4AddressTable`), exposing
`select_source(dst: SocketAddr) -> Option<SocketAddr>`. `send_to` calls it
only when the socket is bound to the unspecified/wildcard address; an
explicitly-bound socket keeps its bound source.
**Rationale:** cross-family arbitration cannot live inside an IPv6-only
`ipv6.rs`. A neutral aggregator keeps per-family tables intact while giving
RFC 6724 a correct home.
**Residual risk:** moderate — this is the restructuring commit (§4.5).
**Rejected A:** generalize `InterfaceAddressTable` to hold both families
inline — churns all SLAAC apply/refresh logic for no payoff.
**Rejected B:** put selection in `socket.rs` — inverts the layering (the
transport object reaching into two protocol tables).

### Q4 — Address Conflict Detection (RFC 5227)
**Recommendation:** minimal — a probe/announce *state* in the DHCP machine
with a stub timeout that transitions deterministically in the injection path,
mirroring BUILD-2's minimal DAD (`ipv6.rs:15` `DAD_TIMEOUT_TICKS = 1`).
On-wire conflict detection deferred to the bare-metal lane (3f), exactly as
bare-metal RA was deferred to BUILD-2e.
**Rejected:** full RFC 5227 probe/defend on the wire in QEMU CI — needs a
conflicting host, flaky, inconsistent with the minimal-DAD precedent.

### Q5 — Real wire transmission for non-loopback UDP
**Recommendation:** **in scope** as a dedicated sub-commit (3e), gated to the
egress floor. The "send_to only captures egress" gap is acceptable for IPv6
BUILD-2 but is the headline for IPv4-to-real-host: ARP + DHCP have no
observable point if nothing leaves the wire. Floor = "a correctly-framed
Ethernet+IPv4+UDP datagram, with ARP-resolved dst MAC and DHCP/selected src
IP, egressed the virtio-net TX ring." Receiving a real reply is bare-metal (3f).
**Rejected:** egress-capture-only for IPv4 too — would make ARP and DHCP
untestable end-to-end and BUILD-3 a paper deliverable.

## §4.5. Restructuring assessment (discharges THINK v0.2 line-174 warning)

**Verdict: a *bounded* restructuring of `ipv6.rs`, not additive overlay nor a
full rewrite.** The THINK warning is correct but the worst case does not
materialize. The coupling is confined to four touch points:

| Symbol / location | Change | Sub-commit |
|---|---|---|
| `ipv6.rs` `INTERFACES` (`:21`) | Aggregation moves to a neutral `net/interface.rs`; `ipv6.rs`/`ipv4.rs` stay per-family tables | 3c |
| `ipv6.rs` `preferred_global_address` (`:191`) | Becomes one input to cross-family `select_source(dst)`; caller moves | 3c |
| `ipv6.rs` `tick()` (`:175`) | Becomes a `net::tick()` fan-out → `ipv6::tick()` + `ipv4::tick()` (DHCP lease + ARP aging) | 3b/3c |
| `socket.rs` `send_to` (`:164`) | Adds a `select_source` hook for wildcard binds; adds the real-TX call in 3e | 3d/3e |

**Genuinely additive (no restructuring):** `SocketAddr::Inet4` parsing,
`BOUND_SOCKETS` keying, IPv4 loopback delivery (already family-generic); and
the net-new `arp.rs`/`ipv4.rs`/`dhcpv4.rs` modules. The `InterfaceAddress`
struct and all SLAAC apply/refresh logic are **not** churned.

## §5. Proposed sub-commit split with ISC criteria

### BUILD-3a — IPv4 packet plumbing + ARP module
**Scope:** `net/ipv4.rs` (IPv4 header build/parse + header checksum);
`net/arp.rs` (RFC 826 parse/build, kernel-owned aging cache keyed IPv4→MAC,
`arp::inject(frame)` test hook). Register in `mod.rs`. Confirm `is_loopback`
covers `127.0.0.0/8` policy intent.
**Anti-scope:** no DHCP, no source selection, no wire TX, no smoltcp iface.
- **ISC-3a-1:** IPv4 header build+parse round-trips with correct header
  checksum (host unit test).
- **ISC-3a-2:** ARP request parses to (sender IP/MAC, target IP); a crafted
  reply populates the cache; lookup returns the MAC (host unit test).
- **ISC-3a-3:** ARP cache entries age out after the configured tick budget.
- **ISC-3a-4:** No unwrap/panic; defensive length checks before each parser
  advance (matches `icmpv6.rs` posture).

### BUILD-3b — DHCPv4 state machine (RFC 2131/2132)
**Scope:** `net/dhcpv4.rs` — DISCOVER→OFFER→REQUEST→ACK, option parsing
(subnet mask, router, lease time, DNS), kernel-owned lease + T1/T2 renewal
timers driven by tick, `inject_offer`/`inject_ack` hooks, minimal RFC 5227
ACD state (Q4).
**Anti-scope:** no address-table integration (3c), no wire TX, no real server.
- **ISC-3b-1:** synthetic OFFER advances DISCOVER→REQUESTING + extracts IP,
  mask, router, lease (host unit test).
- **ISC-3b-2:** synthetic ACK transitions to BOUND with the acquired
  `Ipv4AddressTable` entry + lease deadline.
- **ISC-3b-3:** lease T1 expiry transitions BOUND→RENEWING via tick.
- **ISC-3b-4:** ACD stub transitions deterministically to BOUND (no conflict);
  no unwrap/panic.

### BUILD-3c — interface unification + RFC 6724 source selection (restructuring)
**Scope:** `net/interface.rs` neutral dual-family aggregate; `net::tick()`
fan-out; `select_source(dst) -> Option<SocketAddr>` covering RFC 6724 Rules
1/2/5 + the prefer-IPv6 default (Bible Art. 11). `preferred_global_address`
becomes one input.
**Anti-scope:** no new transport behavior; no wire TX; RFC 6724 Rules
3/4/6/7/8 deferred (load-bearing rules only).
- **ISC-3c-1:** `select_source` picks the DHCP IPv4 source for an IPv4 dst and
  the SLAAC global for an IPv6 dst (host unit test, both tables populated).
- **ISC-3c-2:** dual-stack-reachable, no explicit bind → prefer-IPv6 default.
- **ISC-3c-3:** `net::tick()` drives both SLAAC refresh and DHCP lease aging;
  `phase4_slaac_udp_ipv6_loopback` still passes unchanged (regression gate).
- **ISC-3c-4:** explicitly-bound socket bypasses selection, keeps bound source
  (regression of `socket.rs:168`).

### BUILD-3d — send_to source-selection hook + IPv4 loopback validation
**Scope:** insert `select_source` in `socket.rs send_to` for wildcard binds;
preserve all explicit-bind paths; validate IPv4 loopback through the existing
generic `BOUND_SOCKETS` path; egress records selected source.
**Anti-scope:** no wire TX yet.
- **ISC-3d-1:** IPv4 UDP loopback send/recv round-trips between two sockets
  bound to `127.0.0.1:p1`/`:p2` (mirrors ISC-2-3).
- **ISC-3d-2:** wildcard-bound socket → non-loopback IPv4 dst produces a
  `LAST_EGRESS` with the DHCP-acquired source.
- **ISC-3d-3:** cross-family `send_to` (IPv4 socket, IPv6 dst) still rejects
  (regression of `socket.rs:173`).

### BUILD-3e — real virtio-net wire transmission (egress floor)
**Scope:** close the non-loopback transmit gap: assemble Ethernet+IPv4+UDP,
resolve dst MAC via ARP (issue request + queue datagram on cache miss, RFC
826), submit via `NetDevice::tx_submit` / `SmolDevice` TX token; route DHCP
DISCOVER/REQUEST through the same TX path; QEMU scenario
`phase4_dhcpv4_ipv4_udp` with injected OFFER/ACK + ARP reply.
**Anti-scope:** real server, real ARP peer, real reply — bare-metal (3f).
- **ISC-3e-1:** after injected DHCP ACK + injected ARP reply, a userspace
  `sendto` to a non-loopback IPv4 host egresses an Ethernet/IPv4/UDP frame
  with correct dst MAC, DHCP source IP, UDP ports (QEMU; egress capture or
  virtio TX-counter assertion).
- **ISC-3e-2:** ARP cache miss queues the datagram, emits an ARP request, and
  flushes the datagram on injected reply.
- **ISC-3e-3:** scenario passes on SMOKE and NEO_SMP=4 (mirrors ISC-2-5).

### BUILD-3f — sweep + bare-metal deferral marker
**Scope:** full `cargo neo-test` sweep; code-quality pass; create the BUILD-3f
bare-metal tracking item (real DHCP lease, real ARP, real reply, RFC 5227
on-wire conflict) — deferred as BUILD-2e was.
**Anti-scope:** no new functionality.
- **ISC-3f-1:** no new unwrap/panic across `ipv4.rs`/`arp.rs`/`dhcpv4.rs`/
  `interface.rs` (mirrors ISC-2-7).
- **ISC-3f-2:** sweep green or blocked only on the known perf-proxy lane
  (separate-lane precedent).
- **ISC-3f-3:** bare-metal BUILD-3f tracked, not blocking close.

## §6. /simplify inner-loop review

- **No smoltcp iface adoption.** Hand-rolled control plane stays (Q1/Q2).
- **RFC 6724 load-bearing rules only** (1/2/5 + prefer-IPv6); Rules 3/4/6/7/8
  deferred until a concrete need.
- **Minimal ACD**, mirroring minimal DAD; on-wire conflict to bare metal.
- **Wire TX gated to the egress floor**; real round-trip to bare metal.
- **Sequential sub-commits** 3a→3f, not parallel. No TCP scaffolding (Phase 4
  BUILD-4). No DHCPv6 (no concrete need; THINK §"DHCPv6 in BUILD-3+ only if
  concrete need").

## §7. Risk-register cross-reference

| THINK risk | Owner | Comment |
|---|---|---|
| R1 (smoltcp wrap-vs-fork) | 3a/3b | Q1/Q2 reaffirm phy-only posture; no fork. |
| IPv6-first restructuring (line 174) | 3c | Discharged §4.5: bounded, 4 touch points. |
| R13 (privacy-ext lifetime vs socket cap) | 3c/3d | Source selection must skip deprecated/temporary-expired addresses; reuse `ipv6.rs` state filtering. |
| R12/R14 (resolver/DNS) | — | Out of scope; BUILD-4/5. |

## §8. Required next artifacts (this package)

1. PLAN — `Plans/PLAN-2026-05-30-phase-4-build-3-ipv4-dhcpv4-dual-stack.md`
   (detailed per-sub-commit spec).
2. DISPATCH — `Plans/DISPATCH-2026-05-30-phase-4-build-3-ipv4-dhcpv4-jarvis.md`
   (implementation instructions to Jarvis).
3. INDEX.md amendment.

## §9. Authorization boundary

- **Authorized now:** PLAN drafting; DISPATCH drafting; Jarvis begins 3a→3d
  (the host-testable, additive + bounded-restructuring commits) on the feature
  branch once PLAN + this PM-DECISION are on the branch.
- **Authorized on PL ratification (§10):** the restructuring approach (Q3/§4.5)
  and the wire-TX scope (Q5/3e) as the BUILD-3 close gate.
- **Not authorized:** smoltcp iface adoption; TCP; DHCPv6; RFC 6724 full rule
  set; any new socket syscall outside the 0x189+ append discipline without a
  separate ABI note.

## §10. Open points for ratification

Silent ratification (PL takes no exception within the normal window) confirms
all five. Explicit exception on any point reworks that point only.

1. **Q1** ARP hand-rolled in `net/arp.rs` (reject smoltcp iface ARP).
2. **Q2** DHCPv4 hand-rolled in `net/dhcpv4.rs` (reject smoltcp dhcpv4 socket).
3. **Q3 / §4.5** RFC 6724 in a new neutral `net/interface.rs`; bounded
   restructuring of `ipv6.rs` (4 touch points), per-family tables preserved.
4. **Q4** minimal RFC 5227 ACD; on-wire conflict deferred to bare-metal 3f.
5. **Q5** real virtio-net wire TX in scope as 3e, gated to the egress floor.

---

**BUILD-3 opens on a green substrate. PLAN and DISPATCH follow in this package.**
