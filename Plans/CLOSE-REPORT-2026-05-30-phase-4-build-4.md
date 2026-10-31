# CLOSE-REPORT: Phase 4 BUILD-4 — hand-rolled TCP + CUBIC + RFC 5961 + RFC 7323

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** ACCEPTED — ready for ff-only merge to main
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-4-opening.md (Q1–Q7, FM-1/2/3, ISCs)
- Plans/PLAN-2026-05-30-phase-4-build-4-tcp.md
- Plans/DISPATCH-2026-05-30-phase-4-build-4-tcp-jarvis.md
- Plans/PL-RATIFICATION-2026-05-30-phase-4-build-4-pm-decision.md
- Plans/RETURN-2026-05-30-phase-4-build-4-jarvis.md (Jarvis)
- Plans/BUILD-4g-BARE-METAL-DEFERRAL-2026-05-30.md (Jarvis)

---

## Summary

Phase 4 BUILD-4 (hand-rolled TCP + CUBIC + RFC 5961 + RFC 7323) is **accepted
on all 28 ISCs**. Reviewed by PM against `origin/codex/phase-4-build-4-tcp-jarvis`
@ `fb690c9` (seven commits `438cae2`…`fb690c9` atop main `ea241ac`), in two
passes: the 4a/4b foundation checkpoint (mid-build) and this full disposition.

**Goal milestone met:** TCP connection over IPv4 or IPv6; bidirectional
byte-stream; clean close; CUBIC congestion control; RFC 5961 challenge-ACK
mitigations; RFC 7323 window scaling negotiated. TCP is a type fork; the UDP
stack is untouched; smoltcp stays phy-only.

## Sub-commits

| Commit | Sub-commit |
|--------|-----------|
| `438cae2` | build-4a: tcp segment codec + rfc 9293 state machine skeleton |
| `1b40e89` | build-4b: tcp syscalls (0x188-0x18b) + cap derivation + rw rights |
| `13b5df8` | build-4c: tcp 3-way handshake (active + passive open) |
| `c4343e1` | build-4d: tcp data transfer + retransmit + rfc 7323 window scaling |
| `8a605d2` | build-4e: cubic congestion control |
| `8f9699e` | build-4f: rfc 5961 challenge-ack + clean close + time-wait accounting |
| `fb690c9` | build-4g: virtio-net tcp wire egress + sweep + bare-metal deferral |

## ISC results (28/28)

All 28 ISCs (4a-1 … 4g-4) reported PASS. 4a/4b re-verified by PM at the
foundation checkpoint; 4c–4f functional logic re-verified by PM source read;
4c/4g QEMU wire/SMP scenarios accepted on Jarvis's harness (PM environment has
no QEMU).

## PM independent verification (against dispatched constraints)

| Check | Result | Evidence |
|-------|--------|----------|
| **Q1/Q2 — hand-roll, type fork** | ✓ PASS | `tcp.rs` (1,499 lines) holds `TcpSocket`/`TcpListener`/`TcpState`; `socket.rs` not in the diff — UDP `Socket` untouched. |
| **No smoltcp iface** | ✓ PASS | `Cargo.toml:47` smoltcp features unchanged; the only `interface` ref in `tcp.rs` is `crate::net::interface` (our BUILD-3 source-selection), not smoltcp. |
| **Syscall ABI discipline** | ✓ PASS | TCP syscalls exactly `0x188–0x18B`; nothing beyond. |
| **FM-1 (cap derivation)** | ✓ PASS | `accept` mints `Handle::TcpSocket` via `install_cap(.., Some(parent_cap_id))` — existing Phase-2 derivation path, zero machinery change. No extension needed. |
| **FM-2 (READ/WRITE independence)** | ✓ PASS | `socket_rights` bits 4/5; `shutdown(WRITE)` clears WRITE, keeps READ; 4f adds the FIN-emission state side. |
| **FM-3 (TIME-WAIT decoupling)** | ✓ PASS | `insert_time_wait(tuple, owner)` retains state with owner attribution past cap drop; `time_wait_count()` + `tick()` time it out. |
| **Algorithms present** | ✓ PASS | `CubicState::{on_ack,on_loss}` + cwnd/ssthresh slow-start; window-scale negotiate (`parse_window_scale`); RTO retransmit queue (`retransmit_due`); challenge-ACK throttle (`challenge_ack_allowed`). |
| **Panic/unwrap scan over tcp.rs** | ✓ PASS | PM-run scan: no matches. |
| **Full NEO_SMP=4 sweep** | ✓ PASS | `neo-test all --no-meta` exit 0 at ATTEMPTS=1 (no retry masking). |
| **Q8C handle-match fix** | ✓ ACCEPTED | New `TcpSocket`/`TcpListener` variants added to the non-channel kill/close arms (BUILD-2 `Handle::Socket` precedent); focused Q8C scenario passes. In-scope. |

## Findings carried forward (non-blocking forward-markers)

1. **FM-4-1 (IPv6 next-hop resolution / NDP gap):** IPv6 TCP wire egress emits a
   correctly-framed Ethernet+IPv6+TCP segment but with a **placeholder
   destination MAC** — IPv6 has no ARP, and Neighbor Discovery (NDP, RFC 4861
   neighbor solicitation) was never implemented (BUILD-2 did RA parsing only).
   IPv4 resolves the next hop via the ARP cache; IPv6 cannot resolve a real
   on-link neighbor yet. The injected-peer QEMU gate asserts egress framing, so
   this does not block BUILD-4. **Real IPv6 peers require NDP neighbor
   resolution** — tracked for bare-metal (4g) and as a stack-level gap to
   schedule (candidate BUILD-5 prerequisite or a small dedicated build).
2. **FM-4-2 (egress drain test accommodation):** the 4g egress path preserves
   loopback segments for injected host tests while flushing non-loopback
   segments to virtio-net. Test-observability shim; benign.

Neither gates BUILD-4. FM-4-1 is the consequential one and is recorded as a
forward dependency for real IPv6 connectivity.

## Separate-lane note

Full `neo-test all` NEO_SMP=4 sweep is green at ATTEMPTS=1 — no perf-proxy
carve-out, no separate lane. The BUILD-2 perf-proxy SMP-gate and the syswait
race fix continue to hold through BUILD-4.

## Phase 4 progress

| BUILD | Content | Status |
|-------|---------|--------|
| BUILD-1 | virtio-net + UDP socket syscalls + loopback | ✓ CLOSED |
| BUILD-2 | IPv6 SLAAC + sendto/recvfrom + net stats | ✓ CLOSED |
| BUILD-3 | IPv4 + ARP + DHCPv4 + dual-stack (RFC 6724) | ✓ CLOSED |
| BUILD-4 | TCP + CUBIC + RFC 5961 + RFC 7323 | ✓ **CLOSED (this report)** |
| BUILD-4g | bare-metal live-peer TCP + IPv6 NDP (deferred) | 📋 TRACKED |
| BUILD-5 | DNS + Happy Eyeballs + kTLS (= Phase 4 ship) | 📋 NEXT |

## Open items carried forward

1. **FM-4-1 — IPv6 NDP neighbor resolution** — required for real IPv6
   connectivity (TCP and UDP both); schedule before/within BUILD-5 or as a
   dedicated build. **The most consequential carried item.**
2. **BUILD-4g bare-metal** — real remote-peer handshake/data/close, real RTT
   (tracked, `BUILD-4g-BARE-METAL-DEFERRAL-2026-05-30.md`).
3. **FM-3-2 (zero UDP checksum), FM-3-3 (net::tick ISR-wiring)** — carried from
   BUILD-3; unchanged.
4. **Perf-proxy baseline re-calibration study** — pre-Phase-5 prerequisite.
5. **ANSPEC-002 §17/§18 canonical integration** — pending.

## Merge

ff-only merge of `codex/phase-4-build-4-tcp-jarvis` (`fb690c9`) to main
authorized. Performed from the PM/main seat (this PM session is a remote seat
without main-push rights).

---

**BUILD-4 is CLOSED. The transport layer (UDP + TCP) is complete. Next: BUILD-5
(DNS + Happy Eyeballs + kTLS) — the Phase 4 ship. FM-4-1 (IPv6 NDP) is a
forward dependency to sequence.**
