# DISPATCH: Phase 4 BUILD-4 — TCP (hand-rolled) + CUBIC + RFC 5961 + RFC 7323

**Date:** 2026-05-30
**From:** PM (Cerebro)
**To:** Jarvis
**Repo:** E:\AI Automation\AltimitNeo (GitHub: SecondStageTurbine/NeoPlutarch)
**Branch:** `codex/phase-4-build-4-tcp-jarvis` (create from `origin/main` at its latest — must include the BUILD-3-closed index update)
**Author:** SecondStageTurbine

---

## Your assignment

Implement Phase 4 BUILD-4: hand-rolled TCP in a new `net/tcp.rs` — RFC 9293
connection state machine, reliable byte-stream transfer, CUBIC congestion
control, RFC 5961 challenge-ACK mitigations, RFC 7323 window scaling — over
both IPv4 and IPv6, with connect/listen/accept/shutdown syscalls and
capability integration. smoltcp stays phy-only; do NOT adopt its Interface.

**Goal milestone:** TCP connection to a remote host over IPv4 or IPv6;
bidirectional send/receive; clean close; CUBIC; RFC 5961; RFC 7323 negotiated.

## Before you start

1. `git fetch origin main`; create the branch from `origin/main` (latest —
   confirm it has the BUILD-3 close + the index-closed update).
2. Read **PLAN-2026-05-30-phase-4-build-4-tcp.md** and
   **PM-DECISION-2026-05-30-phase-4-build-4-opening.md** (Q1–Q7, FM-1/2/3).
3. Read **PM-NOTE-2026-05-28-phase-4-build-1-speculative-tcp-shape-sketch.md**
   — the FM-1/FM-2/FM-3 source.
4. Understand the substrate by reading: `kernel/src/net/socket.rs` (UDP shape —
   do NOT modify its struct), `kernel/src/net/ipv4.rs` (header/checksum +
   `transmit_*` egress), `kernel/src/net/interface.rs` (`select_source`),
   `kernel/src/net/dhcpv4.rs` (FSM style to mirror), `kernel/src/net/mod.rs`
   (`net::tick()` fan-out), `kernel/src/arch/x86_64/syscall.rs:69–76` (ledger),
   and the `socket_rights` bitmap + `sys_derive` in `kernel/src/sched/process.rs`.

**Hard constraints (PM-DECISION §9):** no smoltcp `iface::Interface`; TCP is a
**type fork** (`TcpSocket`/`TcpListener`), do NOT add connection state to the
UDP `Socket` struct; CUBIC only (non-pluggable); new syscalls only at
0x188–0x18B.

## Sub-commits (7 total, sequential 4a → 4g)

### BUILD-4a: TCP types + segment codec + state machine skeleton
- **Create:** `kernel/src/net/tcp.rs` (NEW). **Amend:** `mod.rs`.
- `TcpSocket`/`TcpListener` types; TCP segment build/parse + pseudo-header
  checksum (IPv4 + IPv6); RFC 9293 state enum + transition table; 4-tuple
  connection table.
- **Closes:** ISC-4a-1…4a-4. **Commit:** `build-4a: tcp segment codec + rfc 9293 state machine skeleton`

### BUILD-4b: syscalls + capability integration (FM-1/FM-2)
- **Amend:** `syscall.rs` (0x188–0x18B + handlers), `process.rs` (READ/WRITE
  rights bits; cap derivation for accept), handle table (Tcp kind).
- FM-1: `accept` mints a derived `TcpSocket` cap — **confirm Phase-2 derivation
  first**; extend if needed; BLOCKER if the extension is non-trivial.
  FM-2: separate READ/WRITE bits; `shutdown(WRITE)` semantics.
- **Closes:** ISC-4b-1…4b-4. **Commit:** `build-4b: tcp syscalls (0x188-0x18b) + cap derivation + rw rights`

### BUILD-4c: 3-way handshake
- **Amend:** `tcp.rs`. Active + passive open; accept queue; ISS; RST on closed port.
- **Closes:** ISC-4c-1…4c-4. **Commit:** `build-4c: tcp 3-way handshake (active + passive open)`

### BUILD-4d: data transfer + retransmission + RFC 7323
- **Amend:** `tcp.rs`. Sliding window, seq/ack, partial-write, reassembly,
  RTO + backoff, RFC 7323 window scaling.
- **Closes:** ISC-4d-1…4d-4. **Commit:** `build-4d: tcp data transfer + retransmit + rfc 7323 window scaling`

### BUILD-4e: CUBIC congestion control
- **Amend:** `tcp.rs`. cwnd/ssthresh, slow-start, CUBIC avoidance, fast
  retransmit/recovery.
- **Closes:** ISC-4e-1…4e-3. **Commit:** `build-4e: cubic congestion control`

### BUILD-4f: RFC 5961 + clean close + FM-3
- **Amend:** `tcp.rs`. Challenge-ACK + throttle; FIN/CLOSE/TIME-WAIT; half-close;
  FM-3 TIME-WAIT cap-lifecycle decoupling + process accounting.
- **Closes:** ISC-4f-1…4f-4. **Commit:** `build-4f: rfc 5961 challenge-ack + clean close + time-wait accounting`

### BUILD-4g: wire integration + sweep + bare-metal marker
- **Amend:** `tcp.rs`, `socket.rs`/`ipv4.rs` egress as needed, test harness
  (scenario `phase4_tcp_connect_stream_close`).
- Real virtio-net TCP egress (IPv4 + IPv6); full sweep; bare-metal deferral file.
- **Closes:** ISC-4g-1…4g-4. **QEMU.** **Commit:** `build-4g: virtio-net tcp wire egress + sweep + bare-metal deferral`

## ISC checklist (unchecked on filing)

- [ ] ISC-4a-1 segment round-trip + pseudo-header checksum (v4+v6)
- [ ] ISC-4a-2 state enum + transition table + 4-tuple table
- [ ] ISC-4a-3 UDP path untouched (regression)
- [ ] ISC-4a-4 no panic / defensive parsing
- [ ] ISC-4b-1 syscalls 0x188–0x18B dispatch + arg-validate
- [ ] ISC-4b-2 FM-1 accept mints derived TcpSocket cap (lineage holds)
- [ ] ISC-4b-3 FM-2 shutdown(WRITE) revokes WRITE, keeps READ
- [ ] ISC-4b-4 no new panic/unwrap
- [ ] ISC-4c-1 loopback handshake → ESTABLISHED
- [ ] ISC-4c-2 listener accepts injected SYN → accept yields connected socket
- [ ] ISC-4c-3 SYN to closed port → RST
- [ ] ISC-4c-4 handshake SMOKE + NEO_SMP=4
- [ ] ISC-4d-1 bidirectional ordered byte stream
- [ ] ISC-4d-2 partial-write returns bytes-accepted ≤ window
- [ ] ISC-4d-3 retransmit after RTO + out-of-order reassembly
- [ ] ISC-4d-4 RFC 7323 window scale negotiated + honored
- [ ] ISC-4e-1 cwnd grows in slow-start to ssthresh
- [ ] ISC-4e-2 congestion event → CUBIC reduction + recovery
- [ ] ISC-4e-3 no regression to 4d
- [ ] ISC-4f-1 four-way close + TIME-WAIT 2·MSL via tick
- [ ] ISC-4f-2 out-of-window RST → challenge-ACK + throttle
- [ ] ISC-4f-3 half-close shutdown(WRITE)
- [ ] ISC-4f-4 FM-3 TIME-WAIT persists past cap drop, process-attributed
- [ ] ISC-4g-1 correct TCP segments egress virtio-net (v4 + v6)
- [ ] ISC-4g-2 scenario SMOKE + NEO_SMP=4
- [ ] ISC-4g-3 sweep green or known perf-lane; no new panic in tcp.rs
- [ ] ISC-4g-4 bare-metal tracked

## Key design points (PM-DECISION Q1–Q7)

- **Q1/Q2:** hand-rolled `net/tcp.rs`; type fork (`TcpSocket`/`TcpListener`),
  UDP `Socket` untouched.
- **Q3/FM-1:** `accept` derives an inner cap; confirm Phase-2 first.
- **Q4/FM-2:** separate READ/WRITE rights bits.
- **Q5/FM-3:** TIME-WAIT kernel-retained past cap drop, process-attributed.
- **Q6:** CUBIC only.
- **Q7:** drive 4c–4f state machine via injected peer segments (RA/DHCP
  injection precedent); real remote peer is bare-metal (4g).

## Failure modes to avoid

1. **Touching the UDP `Socket` struct** — TCP is a fork; UDP ISCs must stay green.
2. **Pulling in smoltcp `Interface`** — phy-only stays.
3. **Parser/segment panics** — length-check before every slice; pseudo-header
   checksum must cover v4 and v6 correctly.
4. **Scope creep** — no TLS/DNS, no pluggable CC, no SACK/ECN/RACK-TLP.
5. **Syscalls off the 0x188–0x18B append.**
6. **FM-1 silently reshaping accept** if Phase-2 derivation falls short — file a
   BLOCKER instead.

## Testing expectations

- **4a–4f (host, no QEMU):** `cargo check` clean; host unit tests for segment
  codec, FSM transitions, handshake, data/retransmit, CUBIC events, RFC 5961,
  close/TIME-WAIT, and cap derivation/rights. Drive the state machine with
  injected peer segments (deterministic, no real network).
- **4c/4g (QEMU harness):** `phase4_tcp_connect_stream_close` SMOKE +
  `NEO_SMP=4`; full sweep.

## Return artifact

File `Plans/RETURN-2026-05-30-phase-4-build-4-jarvis.md`: sub-commit hashes +
messages; ISC results (PASS/FAIL/BLOCKED each); exact cargo/test/Docker
invocations + output; FM-1 finding (did Phase-2 derivation suffice or need
extension?); code-review notes; blocker links.

## Dispatch assumptions

BUILD-3 on main (`ef6be5b`) + index-closed; full NEO_SMP=4 sweep green;
speculative TCP sketch (FM-1/2/3) on main; socket ledger free at 0x188+.

## Escalation path

`Plans/BLOCKER-2026-MM-DD-<description>.md` and stop on the affected sub-commit.
A non-trivial FM-1 cap-subsystem extension is a BLOCKER, not a silent reshape.

---

**Fetch origin/main, create the feature branch, read the PLAN, and begin
BUILD-4a. 4a–4f are host-verifiable; pause for the harness at 4c/4g.**
