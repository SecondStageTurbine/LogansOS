# PLAN: Phase 4 BUILD-4 — TCP (hand-rolled) + CUBIC + RFC 5961 + RFC 7323

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — detailed spec for the BUILD-4 sub-commit chain
**Anchored on:** PM-DECISION-2026-05-30-phase-4-build-4-opening.md (Q1–Q7, FMs)
**Implements:** THINK v0.2 §5 TCP milestone (full ratified scope)

---

## Context

BUILD-1/2/3 delivered a hand-rolled UDP stack over IPv4+IPv6 with a phy-only
smoltcp. BUILD-4 adds TCP as the next hand-rolled layer: a new `net/tcp.rs`
implementing RFC 9293 connection state, reliable byte-stream transfer, CUBIC
congestion control, RFC 5961 challenge-ACK mitigations, and RFC 7323 window
scaling. smoltcp stays a phy/parsing device. The UDP socket path is left
untouched; TCP is a **type fork** (`TcpSocket`/`TcpListener`), not an extension
of the datagram `Socket`.

**Goal milestone:** TCP connection to a remote host over IPv4 or IPv6;
bidirectional send/receive; clean close; CUBIC; RFC 5961; RFC 7323 negotiated.

## ISCs (full text)

Carried verbatim from PM-DECISION §5 (28 ISCs across 4a–4g). Pass/fail gates
per sub-commit; see the DISPATCH checklist for the enumerated list.

## Non-scope

TLS, DNS, Happy Eyeballs (BUILD-5); pluggable congestion control; ECN; RACK-TLP;
SACK beyond reassembly needs; smoltcp `iface::Interface`; new socket syscalls
beyond 0x188–0x18B.

## Implementation sketch

### 4a — `net/tcp.rs` foundation
- **Types (Q2 fork):** `TcpSocket` (connection endpoint: state, send/recv
  buffers, seq/ack vars, timers) and `TcpListener` (accept queue, backlog).
  Distinct from the UDP `Socket`. Registered in `mod.rs`.
- **Segment codec:** build/parse TCP header (ports, seq, ack, data-offset,
  flags, window, checksum, urgent, options); **pseudo-header checksum** over
  IPv4 and IPv6 (reuse `ipv4.rs` checksum helper; add the IPv6 pseudo-header).
- **State enum:** RFC 9293 (CLOSED, LISTEN, SYN-SENT, SYN-RECEIVED,
  ESTABLISHED, FIN-WAIT-1/2, CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT) + a
  transition table mirroring the `dhcpv4.rs` FSM style.
- **Connection table:** `BTreeMap<FourTuple, Arc<TcpSocket>>` keyed by
  (local ip/port, remote ip/port), under a `spin::Mutex`.
- **Acceptance:** ISC-4a-1…4a-4. Host-testable.

### 4b — syscalls + capability integration
- **Syscalls:** `sys_net_connect_tcp` 0x188, `sys_net_listen_tcp` 0x189,
  `sys_net_accept_tcp` 0x18A, `sys_net_shutdown_tcp` 0x18B. `Handle::Socket`
  gains a Tcp kind (or a sibling `Handle::TcpSocket`/`Handle::TcpListener`).
- **FM-1 (cap derivation):** `accept` mints a derived `TcpSocket` cap from the
  `TcpListener` cap. **Gating check:** confirm the Phase-2 derivation walk
  (`sys_derive`) supports inner→inner socket-cap minting; if not, extend the
  cap subsystem (small) — and if that extension is non-trivial, file a BLOCKER.
- **FM-2 (rights):** add `READ` (bit 4) and `WRITE` (bit 5) to `socket_rights`;
  `shutdown(WRITE)` revokes WRITE, keeps READ. UDP keeps SEND/RECV (bits 0/1).
- **Acceptance:** ISC-4b-1…4b-4. Host-testable.

### 4c — 3-way handshake
- Active open: `connect` → SYN, SYN-SENT, on SYN-ACK send ACK → ESTABLISHED.
- Passive open: `listen` → LISTEN; on SYN send SYN-ACK, SYN-RECEIVED; on ACK →
  ESTABLISHED, enqueue to the accept backlog; `accept` dequeues.
- ISS selection; RST on SYN to a closed port.
- **Acceptance:** ISC-4c-1…4c-4 (loopback + injected-peer; SMOKE + NEO_SMP=4).

### 4d — data transfer + retransmission + RFC 7323
- Sliding-window send/recv; seq/ack bookkeeping; partial-write returns
  bytes-accepted; out-of-order reassembly buffer; RTO estimation
  (SRTT/RTTVAR) + exponential backoff retransmit queue; RFC 7323 window-scale
  option negotiated in the handshake and applied to the advertised window.
- **Acceptance:** ISC-4d-1…4d-4.

### 4e — CUBIC
- cwnd + ssthresh; slow-start; CUBIC congestion-avoidance window function;
  fast retransmit (3 dup-ACKs) + fast recovery. Reno-equivalent math inside
  the CUBIC region as the floor.
- **Acceptance:** ISC-4e-1…4e-3 (no regression to 4d).

### 4f — RFC 5961 + clean close + FM-3
- RFC 5961: challenge-ACK for in-window-but-not-exact RST, for SYN in an
  established connection, and for blind data injection; ACK throttle (rate cap).
- Close: FIN-WAIT-1/2, CLOSE-WAIT, CLOSING, LAST-ACK, TIME-WAIT (2·MSL via
  `net::tick`); half-close `shutdown(WRITE)`.
- **FM-3:** TIME-WAIT state retained past cap drop, attributed to the
  originating process; per-process socket counter counts kernel structures.
- **Acceptance:** ISC-4f-1…4f-4.

### 4g — wire integration + sweep + bare-metal marker
- Real virtio-net TCP egress (IPv4 + IPv6) via the `transmit_*` path; QEMU
  scenario `phase4_tcp_connect_stream_close` driven by injected peer segments;
  full sweep; file the bare-metal deferral (real remote peer handshake/data/
  close, real RTT, real loss).
- **Acceptance:** ISC-4g-1…4g-4. **QEMU.**

## Risks and mitigations

- **TCP correctness surface (seq/ack, retransmit, close races).** Mitigation:
  exhaustive host unit tests per state transition (4a–4f are host-testable via
  injected segments, the sanctioned precedent); QEMU only for wire framing (4g).
- **FM-1 Phase-2 derivation may not support inner→inner.** Mitigation: 4b
  confirms first; small cap-subsystem extension if needed; BLOCKER if not small.
- **Restructuring leak into the UDP path.** Mitigation: type fork (Q2); ISC-4a-3
  gates UDP regression; `Socket` struct is not modified.
- **CUBIC tuning instability under QEMU timing.** Mitigation: hand-rolled gives
  the β / window-function levers; 4e ISCs assert qualitative behavior, not
  absolute throughput.
- **TIME-WAIT accounting leak (FM-3).** Mitigation: ISC-4f-4 asserts process
  attribution + counter semantics.

## Host-testable vs QEMU

- **Host (`cargo check` + unit tests), runnable anywhere:** 4a, 4b, and the
  functional core of 4c–4f via injected peer segments (handshake, data,
  retransmit, CUBIC events, RFC 5961, close, TIME-WAIT) and the cap/rights
  tests. This is the bulk of BUILD-4.
- **QEMU harness:** 4c/4g SMOKE + NEO_SMP=4 scenarios; real virtio-net TX
  framing. *(This PM/web environment has no QEMU; runs on a worker seat.)*
- **Bare metal (deferred, 4g tracked):** real remote-peer handshake/data/close,
  real RTT/loss, RFC 5961 against a real spoofer.

## Sequencing estimate

Sequential 4a→4g. Rough effort: 4a ~1.5 (codec + FSM + table), 4b ~1.5
(syscalls + cap derivation/rights), 4c ~1.5 (handshake), 4d ~2 (data +
retransmit + window scaling — the heaviest), 4e ~1.5 (CUBIC), 4f ~1.5 (5961 +
close + FM-3), 4g ~1 (wire + sweep). Larger than BUILD-3; 4a–4f host-verifiable
on the worker, 4c/4g need the harness.

## Artifacts

- **Input:** PM-DECISION-2026-05-30-phase-4-build-4-opening.md; THINK v0.2;
  PM-NOTE speculative TCP sketch (FM-1/2/3); BUILD-1/2/3 net code on main
  (`socket.rs`, `ipv4.rs`, `interface.rs`, `net::tick`, `transmit_ipv4_udp`).
- **Output (Jarvis):** new `kernel/src/net/tcp.rs`; syscall ledger
  0x188–0x18B; `socket_rights` READ/WRITE bits; `Handle` Tcp kinds; scenario
  `phase4_tcp_connect_stream_close`; RETURN-2026-…-phase-4-build-4-jarvis.md.
