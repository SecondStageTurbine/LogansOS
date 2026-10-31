# RETURN: Phase 4 BUILD-4 - hand-rolled TCP + CUBIC + RFC 5961 + RFC 7323

**Date:** 2026-05-30
**From:** Jarvis
**To:** PM (Cerebro)
**Branch:** `codex/phase-4-build-4-tcp-jarvis`
**Status:** COMPLETE

## Sub-Commits

- `438cae2` `build-4a: tcp segment codec + rfc 9293 state machine skeleton`
- `1b40e89` `build-4b: tcp syscalls (0x188-0x18b) + cap derivation + rw rights`
- `13b5df8` `build-4c: tcp 3-way handshake (active + passive open)`
- `c4343e1` `build-4d: tcp data transfer + retransmit + rfc 7323 window scaling`
- `8a605d2` `build-4e: cubic congestion control`
- `8f9699e` `build-4f: rfc 5961 challenge-ack + clean close + time-wait accounting`
- BUILD-4g: pending final commit `build-4g: virtio-net tcp wire egress + sweep + bare-metal deferral`

## ISC Results

- PASS ISC-4a-1 segment round-trip + pseudo-header checksum (v4+v6)
- PASS ISC-4a-2 state enum + transition table + 4-tuple table
- PASS ISC-4a-3 UDP path untouched
- PASS ISC-4a-4 no panic / defensive parsing
- PASS ISC-4b-1 syscalls 0x188-0x18B dispatch + arg-validate
- PASS ISC-4b-2 FM-1 accept mints derived TcpSocket cap
- PASS ISC-4b-3 FM-2 shutdown(WRITE) revokes WRITE, keeps READ
- PASS ISC-4b-4 no new panic/unwrap
- PASS ISC-4c-1 loopback handshake -> ESTABLISHED
- PASS ISC-4c-2 listener accepts injected SYN -> accept yields connected socket
- PASS ISC-4c-3 SYN to closed port -> RST
- PASS ISC-4c-4 handshake SMOKE + NEO_SMP=4
- PASS ISC-4d-1 bidirectional ordered byte stream
- PASS ISC-4d-2 partial-write returns bytes-accepted <= window
- PASS ISC-4d-3 retransmit after RTO + out-of-order reassembly
- PASS ISC-4d-4 RFC 7323 window scale negotiated + honored
- PASS ISC-4e-1 cwnd grows in slow-start to ssthresh
- PASS ISC-4e-2 congestion event -> CUBIC reduction + recovery
- PASS ISC-4e-3 no regression to 4d
- PASS ISC-4f-1 four-way close + TIME-WAIT retention via tick
- PASS ISC-4f-2 out-of-window RST -> challenge-ACK + throttle
- PASS ISC-4f-3 half-close shutdown(WRITE)
- PASS ISC-4f-4 FM-3 TIME-WAIT persists past cap drop, process-attributed
- PASS ISC-4g-1 correct TCP segments egress virtio-net (v4 + v6)
- PASS ISC-4g-2 scenario SMOKE + NEO_SMP=4
- PASS ISC-4g-3 sweep green; no new panic in TCP diff
- PASS ISC-4g-4 bare-metal tracked

## Verification

```text
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-phase4-tcp-connect-stream-close
```

Result: PASS. Final rerun completed with 211 existing warnings.

```text
docker compose run --rm neo cargo neo-test phase4_tcp_connect_stream_close
```

Result: PASS. QEMU harness validated injected TCP handshake/stream/close plus
IPv4 and IPv6 virtio-net TCP wire egress captures. QEMU emitted the existing
`hub 4 is not connected to host network` warning.

```text
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-q8c-blocked-kill-foreign-parked
docker compose run --rm neo cargo neo-test q8c_blocked_kill_foreign_parked
```

Result: PASS. The first full sweep exposed a test-harness exhaustive-match gap
for the new TCP handle variants in Q8C; the match now treats `TcpSocket` and
`TcpListener` as non-channel handles and the focused Q8C scenario passes.

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Result: PASS, exit code 0. Full output captured in `/tmp/build4-full-sweep.log`;
failure-marker scan was empty.

```text
git diff --check
git diff -U0 | rg -n "panic!|todo!|unimplemented!|unwrap\(|expect\("
```

Result: PASS. `git diff --check` exited 0; panic/todo/unwrap/expect scan had no
matches.

## FM-1 Finding

Phase-2 cap derivation was sufficient. TCP `accept` uses the existing derived
cap path through `install_cap(..., Some(parent_cap_id))`, preserving lineage
without extending the cap subsystem. No FM-1 blocker was required.

## Code-Review Notes

- TCP is a type fork in `kernel/src/net/tcp.rs`; the UDP `Socket` struct remains
  untouched.
- smoltcp remains phy-only; no `iface::Interface` was adopted.
- `shutdown(WRITE)` now has both rights-side and state-machine-side behavior:
  WRITE is revoked and FIN emission drives the TCP close state.
- TIME-WAIT is retained in kernel TCP state with owner attribution after cap
  lifecycle drop.
- IPv4 wire egress uses the ARP cache/queue path; IPv6 wire egress emits an
  Ethernet + IPv6 + TCP frame with placeholder destination MAC behavior pending
  neighbor discovery and bare-metal live-peer validation.
- The 4g egress drain preserves loopback segments for injected host tests while
  still flushing later non-loopback segments to virtio-net.
- Bare-metal live-peer verification is tracked in
  `Plans/BUILD-4g-BARE-METAL-DEFERRAL-2026-05-30.md`.

## Blockers

None.
