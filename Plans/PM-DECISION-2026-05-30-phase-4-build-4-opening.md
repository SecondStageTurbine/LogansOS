# PM-DECISION: Phase 4 BUILD-4 opening — TCP + CUBIC + RFC 5961 + RFC 7323

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — opens Phase 4 BUILD-4; §10 open points for PL ratification
**Authority chain:**
- Phase 4 THINK v0.2 §5 "BUILD-3 — TCP + congestion control + RFC 5961" (ratified 2026-05-22)
- PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md (R16 partial discharge; FM-1/2/3)
- CLOSE-REPORT-2026-05-30-phase-4-build-3.md (BUILD-3 closed; substrate green)
- PL decision 2026-05-30 (this session): hand-roll TCP; full ratified scope
- Bible Article 11 (IPv6-default) / Article 11.2 (smoltcp posture)

---

## §1. Substrate

BUILD-4 opens on a green, fully-merged substrate.

- **BUILD-3 close + merge:** main `ef6be5b` (IPv4 + ARP + DHCPv4 + dual-stack).
- **Net control plane on main:** hand-rolled IPv6 SLAAC (`ipv6.rs`), ICMPv6
  (`icmpv6.rs`), IPv4 (`ipv4.rs`), ARP (`arp.rs`), DHCPv4 (`dhcpv4.rs`),
  UDP sockets (`socket.rs`), dual-family `interface.rs`, `net::tick()` fan-out.
- **smoltcp:** phy/parsing-only, IPv6-only features
  (`kernel/Cargo.toml:47` = `medium-ethernet, proto-ipv6, socket-raw`); no
  `iface::Interface` instantiated; `SmolDevice` exercised only in a smoke test.
- **Full NEO_SMP=4 sweep:** green at ATTEMPTS=1 through BUILD-3.

**Prerequisite checklist:**

| # | Prerequisite | State |
|---|--------------|-------|
| P1 | BUILD-3 functional ISCs green + merged | ✓ |
| P2 | Full NEO_SMP=4 sweep green | ✓ |
| P3 | R16 layered-handle shape validated for TCP (speculative sketch) | ✓ (partial; this PLAN discharges residual) |
| P4 | Socket syscall ledger has room (0x188+) | ✓ |
| P5 | No active worker dispatch | ✓ |

## §2. THINK v0.2 scope anchor

THINK v0.2 §5 "BUILD-3 — TCP + congestion control + RFC 5961 mitigations" is
the shipped **BUILD-4** (build numbering advanced by one at the IPv6-SLAAC
split). Goal milestone, quoted:

> TCP connection to remote host on either IPv4 or IPv6; bidirectional
> send/receive; clean close. CUBIC congestion control. RFC 5961 challenge-ACK
> mitigations. RFC 7323 window scaling negotiated.

PL ratified the **full scope** this session: CUBIC + RFC 5961 + RFC 7323 are
in-build (sequenced as sub-commits 4e/4f/4d), not deferred. Anti-scope: TLS
(BUILD-5), DNS/Happy Eyeballs (BUILD-5), pluggable congestion control
(deferred per THINK), ECN/RACK-TLP, SACK beyond what window scaling needs.

## §3. Substrate observations (TCP readiness)

Established by design review of `origin/main`:

- **The Socket type is datagram-shaped.** `socket.rs` `SocketState` is
  `Unbound/Bound/Closed` (no connection states); `SocketInner.rx_queue` is a
  `VecDeque<Datagram>` (no stream buffering/partial-write); `send()` is a stub;
  constructor rejects `socktype != SOCK_DGRAM`. TCP cannot overlay this — it
  needs its own connection-state machine, stream buffers, and partial I/O.
- **Layered-handle shape composes with TCP** (speculative sketch, R16): outer
  `NetworkAccess` cap + inner `TcpSocket`/`TcpListener` caps parallel the UDP
  structure; no outer-cap change; shared close/wait primitives. **No
  API-shape obstacle found.** Three forward markers (FM-1/2/3) handed to this
  PLAN.
- **smoltcp `TcpSocket` requires `iface::Interface`** — adopting it hands
  smoltcp the IP/ARP/routing control plane the project hand-rolled across
  BUILD-1/2/3 (~1,076 lines). PL declined this path. smoltcp stays phy-only.
- **Syscall ledger:** socket syscalls 0x180–0x187 (`syscall.rs:69–76`); 0x188
  is the next free slot. TCP adds connect/listen/accept/shutdown at 0x188–0x18B.
- **Capability rights:** `socket_rights` (process.rs) uses bits 0–3
  (SEND/RECV/BIND/CLOSE); bits 4–7 free. FM-2 (READ/WRITE independence for
  half-close) fits without an ABI break. `sys_derive` (0x08) exists for cap
  derivation; FM-1 must confirm it mints an inner `TcpSocket` cap from a
  `TcpListener` cap.

## §4. Load-bearing PLAN-time decisions

### Q1 — Implementation strategy (PL-decided)
**Hand-roll TCP** in a new `net/tcp.rs`, reusing the `ipv4.rs` header/checksum,
`socket.rs` queue, `net::tick()` timer, and `transmit_ipv4_udp` egress
patterns. smoltcp stays phy-only. *Rejected:* smoltcp `TcpSocket` (forces
`Interface` adoption, duplicates/replaces shipped IP/ARP/DHCP) and the hybrid
(smoltcp 0.13 has no clean TCP-without-Interface path). Recorded per PL.

### Q2 — TCP socket type shape
**Recommendation: type fork.** Introduce distinct `TcpSocket` and
`TcpListener` types with their own state machines and buffers; do **not**
extend the UDP `Socket` struct with connection states. `Handle::Socket`
carries a Tcp variant (or a sibling handle kind). Keeps the UDP path
unperturbed (BUILD-1/2/3 regression-safe) and matches the Hubris layered
shape.
**Residual risk:** moderate — two socket kinds in the handle table; mitigated
by sharing the cap/rights plumbing.
**Rejected:** extend `Socket` with an optional `TcpState` — churns every UDP
path with state guards, regression risk to shipped code.

### Q3 — FM-1: capability derivation for `accept`
**Recommendation:** `sys_net_accept_tcp(listener) -> tcp_socket` mints a new
inner `TcpSocket` cap derived from the `TcpListener` cap, via the existing
`sys_derive` machinery. **4b must first confirm** the Phase-2 derivation walk
supports minting an inner socket cap from another inner cap; if it does not,
extend the cap subsystem (preferred, small) rather than reshape `accept`.
**Residual risk:** the Phase-2 confirmation is a gating check inside 4b; if it
fails and the extension is non-trivial, 4b files a BLOCKER.
**Rejected:** root-minted accept cap not attributed to the listener — breaks
the derivation lineage and per-process accounting.

### Q4 — FM-2: READ/WRITE rights independence (half-close)
**Recommendation:** allocate separate READ and WRITE bits in `socket_rights`
(bits 4/5, leaving 0–3 as the UDP SEND/RECV/BIND/CLOSE semantics) so
`sys_net_shutdown_tcp(WRITE)` can revoke WRITE while READ is held
(FIN-WAIT / half-close). UDP keeps using SEND+RECV together.
**Rejected:** overload SEND/RECV for half-close — conflates UDP and TCP
rights semantics.

### Q5 — FM-3: TIME-WAIT cap-lifecycle decoupling
**Recommendation:** TCP TIME-WAIT state lingers 2·MSL **after** the cap is
dropped; the kernel retains it independently and attributes it to the
originating process for resource accounting. The per-process socket counter
counts kernel-side structures, not 1:1 cap correspondence.
**Residual risk:** accounting edge cases; covered by 4f ISCs.

### Q6 — Congestion control
**Recommendation:** **CUBIC only, non-pluggable** (THINK-ratified). Slow-start
→ CUBIC congestion avoidance, fast retransmit/recovery. Reno math as the
fallback inside the CUBIC curve, not a selectable algorithm. Pluggable CC
deferred (THINK).

### Q7 — RFC 5961 + wire-test posture
**Recommendation:** RFC 5961 challenge-ACK for blind RST / SYN / data
injection + ACK throttling (4f). The QEMU gate drives the state machine by
**injecting peer segments** (the sanctioned RA/DHCP injection precedent) and
asserts correct TCP egress; full handshake/data/close run over loopback
in-kernel. Real remote-peer round-trip is **bare-metal (4g deferral)**, like
BUILD-2e/BUILD-3f.

## §5. Proposed sub-commit split with ISC criteria

### BUILD-4a — TCP types + segment codec + state machine skeleton
**Scope:** `net/tcp.rs` (NEW): `TcpSocket`/`TcpListener` types (Q2 fork); TCP
segment build/parse with pseudo-header checksum (IPv4 + IPv6); RFC 9293 state
enum; connection table keyed by the 4-tuple (src ip/port, dst ip/port). No
data transfer, no syscalls yet.
**Anti-scope:** no handshake logic, no CC, no syscalls, no wire TX.
- **ISC-4a-1:** TCP segment build+parse round-trips; pseudo-header checksum
  verifies for both IPv4 and IPv6 (host unit test).
- **ISC-4a-2:** RFC 9293 state enum + transition table compile; connection
  table insert/lookup/remove by 4-tuple (host unit test).
- **ISC-4a-3:** UDP path untouched — `socket.rs` UDP ISCs still green
  (regression).
- **ISC-4a-4:** no unwrap/panic; defensive length checks (matches net posture).

### BUILD-4b — TCP socket syscalls + capability integration (FM-1/FM-2)
**Scope:** syscalls `sys_net_connect_tcp` (0x188), `sys_net_listen_tcp`
(0x189), `sys_net_accept_tcp` (0x18A), `sys_net_shutdown_tcp` (0x18B);
`Handle::Socket` Tcp variant; FM-1 cap derivation for accept (confirm Phase-2
derivation walk; extend if needed); FM-2 separate READ/WRITE rights bits.
**Anti-scope:** no actual segment exchange yet (4c); stubs return EAGAIN/ENOTCONN.
- **ISC-4b-1:** the four syscalls dispatch and validate args; ledger appends at
  0x188–0x18B (no collision).
- **ISC-4b-2:** FM-1 — `accept` mints a derived `TcpSocket` cap from the
  `TcpListener` cap; derivation lineage holds (host test). If Phase-2 cannot
  derive, the cap-subsystem extension lands here.
- **ISC-4b-3:** FM-2 — `shutdown(WRITE)` revokes WRITE while READ persists;
  rights bitmap separates READ/WRITE (host test).
- **ISC-4b-4:** no new unwrap/panic.

### BUILD-4c — connection establishment (3-way handshake)
**Scope:** active open (SYN-SENT → ESTABLISHED) and passive open (LISTEN →
SYN-RECEIVED → ESTABLISHED) + accept queue; ISS selection; over loopback /
injected-peer.
- **ISC-4c-1:** loopback connect completes the 3-way handshake to ESTABLISHED.
- **ISC-4c-2:** a listener accepts an injected SYN, reaches ESTABLISHED, and
  `accept` yields a connected `TcpSocket`.
- **ISC-4c-3:** SYN to a closed port emits RST (host/QEMU).
- **ISC-4c-4:** handshake passes SMOKE and NEO_SMP=4.

### BUILD-4d — data transfer + retransmission + RFC 7323 window scaling
**Scope:** stream send/recv with sliding window, seq/ack tracking,
partial-write returns, out-of-order reassembly, RTO + exponential backoff
retransmit queue, RFC 7323 window-scale option negotiation.
- **ISC-4d-1:** bidirectional byte stream over loopback; ordered delivery.
- **ISC-4d-2:** partial write returns bytes-accepted ≤ window; caller retries
  remainder.
- **ISC-4d-3:** dropped segment is retransmitted after RTO; reassembly handles
  out-of-order arrival.
- **ISC-4d-4:** window-scale option negotiated in the handshake; scaled window
  honored.

### BUILD-4e — CUBIC congestion control
**Scope:** cwnd, slow-start, CUBIC congestion-avoidance curve, fast
retransmit/recovery (Q6).
- **ISC-4e-1:** cwnd grows in slow-start until ssthresh.
- **ISC-4e-2:** a congestion event applies CUBIC window reduction + recovery.
- **ISC-4e-3:** no regression to 4d data-transfer ISCs.

### BUILD-4f — RFC 5961 mitigations + clean close (FM-3)
**Scope:** RFC 5961 challenge-ACK (blind RST/SYN/data injection) + ACK
throttling; FIN-WAIT-1/2, CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT; half-close
`shutdown(WRITE)`; FM-3 TIME-WAIT cap-lifecycle decoupling + process accounting.
- **ISC-4f-1:** four-way close completes; TIME-WAIT entered and 2·MSL timed via
  `net::tick`.
- **ISC-4f-2:** an out-of-window RST triggers a challenge-ACK, not a teardown;
  ACK throttle bounds challenge rate.
- **ISC-4f-3:** half-close — `shutdown(WRITE)` sends FIN while RECV still
  drains; FM-2 right enforced.
- **ISC-4f-4:** FM-3 — TIME-WAIT state persists after the cap drops, attributed
  to the originating process; counter accounts kernel-side structures.

### BUILD-4g — wire integration + sweep + bare-metal deferral
**Scope:** real virtio-net TCP egress over IPv4 and IPv6; QEMU scenario
`phase4_tcp_connect_stream_close` driven by injected peer segments; full sweep;
bare-metal deferral marker (real remote peer handshake/data/close, real RTT).
- **ISC-4g-1:** injected-peer handshake + data + close produces correct TCP
  segments on the virtio-net TX ring (IPv4 and IPv6).
- **ISC-4g-2:** scenario passes SMOKE and NEO_SMP=4.
- **ISC-4g-3:** full `neo-test all` green or known perf-lane only; no new
  unwrap/panic across `tcp.rs`.
- **ISC-4g-4:** bare-metal BUILD-4g tracked, not blocking close.

## §6. /simplify inner-loop review

- Hand-rolled, smoltcp phy-only (Q1). CUBIC only, non-pluggable (Q6).
- RFC 5961 = challenge-ACK + throttle; not the full anti-spoofing suite.
- RFC 7323 = window scaling; timestamps only if window scaling needs them.
- No SACK beyond reassembly needs; no ECN/RACK-TLP; no TLS/DNS (BUILD-5).
- Sequential 4a→4g. Type fork, not Socket-struct churn.

## §7. Risk-register cross-reference

| THINK risk | Owner | Comment |
|---|---|---|
| R1 (smoltcp wrap-vs-fork) | 4a | **Resolved by PL:** hand-roll; smoltcp phy-only. |
| R16 (layered shape vs TCP) | 4b/4c | Speculative sketch discharged the design; implementation discharges the residual. |
| FM-1 (cap derivation for accept) | 4b | Confirm Phase-2 derivation; extend if needed. |
| FM-2 (READ/WRITE independence) | 4b | Separate rights bits. |
| FM-3 (TIME-WAIT cap-lifecycle) | 4f | Kernel retains state; process-attributed. |
| R13 (addr lifetime vs socket cap) | 4f | TCP long-lived caps vs temp-addr deprecation; reuse `interface.rs` state filtering. |

## §8. Required next artifacts (this package)

1. PLAN — `Plans/PLAN-2026-05-30-phase-4-build-4-tcp.md`.
2. DISPATCH — `Plans/DISPATCH-2026-05-30-phase-4-build-4-tcp-jarvis.md`.
3. INDEX.md amendment.

## §9. Authorization boundary

- **Authorized now:** PLAN + DISPATCH drafting; Jarvis begins 4a→4b
  (foundation + syscalls/caps, host-testable) once PLAN + this PM-DECISION are
  on the branch.
- **Authorized on PL ratification (§10):** the type-fork shape (Q2), the FM-1
  cap-derivation approach (Q3), the FM-2/FM-3 cap decisions, and CUBIC-only.
- **Not authorized:** smoltcp iface adoption; pluggable CC; TLS/DNS; new socket
  syscalls beyond the 0x188–0x18B append without a separate ABI note.

## §10. Open points for ratification

1. **Q2** TCP socket **type fork** (`TcpSocket`/`TcpListener`), not Socket-struct extension.
2. **Q3 / FM-1** `accept` mints a derived inner cap via `sys_derive`; extend the cap subsystem if Phase-2 derivation can't (4b gating check).
3. **Q4 / FM-2** separate READ/WRITE rights bits for half-close.
4. **Q5 / FM-3** TIME-WAIT state kernel-retained past cap drop, process-attributed.
5. **Q6** CUBIC only, non-pluggable.
6. **Q7** RFC 5961 challenge-ACK + injected-peer QEMU gate; real-peer round-trip deferred to bare-metal 4g.

---

**BUILD-4 opens hand-rolled, full ratified scope. PLAN and DISPATCH follow in this package.**
