<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE-REPORT: Phase 4 BUILD-4.5 — IPv6 Neighbor Discovery (NDP)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Disposition:** **ACCEPTED**
**Branch:** `codex/phase-4-build-4.5-ndp-jarvis`
**Sub-commits:** `6b94e24` (4.5a), `49cd238` (4.5b)
**References:**
- Plans/RETURN-2026-05-30-phase-4-build-4.5-jarvis.md
- Plans/PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md
- Plans/PLAN-2026-05-30-phase-4-build-4.5-ndp.md
- Plans/PL-RATIFICATION-2026-05-30-phase-4-build-4.5-pm-decision.md

---

## Disposition

BUILD-4.5 is **ACCEPTED** on all 8 ISCs. This build closes **FM-4-1** (the
IPv6 NDP neighbor-resolution gap left by BUILD-4), giving IPv6 UDP and TCP
egress real on-link MAC resolution ahead of BUILD-5.

| ISC | Result | Verified |
|-----|--------|----------|
| ISC-4.5a-1 | PASS | NS/NA build+parse round-trip + ICMPv6 checksum |
| ISC-4.5a-2 | PASS | injected NA populates cache; lookup returns MAC; solicited-node multicast derivation correct |
| ISC-4.5a-3 | PASS | cache ages out via tick |
| ISC-4.5a-4 | PASS | no panic / defensive parsing |
| ISC-4.5b-1 | PASS | IPv6 UDP/TCP egress resolves dst MAC from cache (no placeholder) |
| ISC-4.5b-2 | PASS | miss → NS + queue → injected NA flushes |
| ISC-4.5b-3 | PASS | scenario SMOKE + NEO_SMP=4; full sweep green |
| ISC-4.5b-4 | PASS | IPv6 SLAAC/UDP/TCP regression green; no new panic in 4.5b diff |

## PM Verification (independent)

- **Placeholder removed (Q4 honored):** `git grep ipv6_destination_mac` over
  `kernel/src/net/` returns no matches at `49cd238`. Both UDP (`socket.rs`) and
  TCP (`tcp.rs`) IPv6 egress now route through the shared
  `ndp::submit_or_queue_ipv6` resolve/queue path — the ratified Q4 design.
- **Serial proof:** the `phase4_ndp_ipv6_neighbor` QEMU log shows the full
  chain — `ns_out` (solicited-node multicast `33:33:ff:00:00:02`) →
  `na_in cache_populated` → `frame_egress` with the resolved unicast MAC
  `52:54:00:12:34:57` for both UDP and TCP, terminating in
  `event=PASS … udp_ipv6=ok tcp_ipv6=ok tx_submissions_delta=3 rx_completions_delta=3`.
- **Sweep:** `neo-test all --no-meta` at `NEO_SMP=4 ATTEMPTS=1` PASS, exit 0,
  including `phase4_slaac_udp_ipv6_loopback`, `phase4_ndp_ipv6_neighbor`,
  `phase4_tcp_connect_stream_close`, and the aarch64 barrier-lane scenario.
- **Hygiene:** `git diff --check` clean; panic/todo/unwrap/expect scan over the
  diff clean; targeted rustfmt clean.
- **Diffstat:** ndp.rs (+202), socket.rs (+87 IPv6 egress), tcp.rs (−placeholder
  / +ndp call), test_harness + xtask wiring for the new scenario. No syscall or
  ABI changes; smoltcp remains phy-only.

## Constraints honored

- **Q1** hand-rolled `net/ndp.rs` mirroring `arp.rs`; smoltcp phy-only. ✓
- **Q2** minimal neighbor cache with TTL aging; no full RFC 4861 NUD. ✓
- **Q3** BUILD-2 minimal DAD left unchanged. ✓
- **Q4** shared NDP resolve/queue for IPv6 UDP **and** TCP; solicited-node
  multicast NS on miss; queued frame flushed on NA. ✓
- **Q5** no Router Solicitation; RA reception untouched. ✓

One disclosed design choice: `phase4_slaac_udp_ipv6_loopback` now pre-seeds the
synthetic peer cache entry so that regression stays focused on SLAAC/UDP egress;
the dedicated `phase4_ndp_ipv6_neighbor` scenario owns the miss/queue/flush
path. Reasonable separation of concerns — accepted.

## Forward-markers (carried)

- **FM-4.5-1** — real NS-based Duplicate Address Detection (BUILD-2 minimal DAD
  retained for now). Deferred.
- **FM-4.5-2** — Router Solicitation (RA reception already works). Deferred.
- Full RFC 4861 NUD / Redirect / SEND scope and live-NIC IPv6 NDP captured in
  `BUILD-4.5b-BARE-METAL-DEFERRAL-2026-05-30.md`. Not blocking.

FM-4-1 is **discharged** by this build.

## Next

BUILD-4.5 lands with the BUILD-4 stack in a single fast-forward to `main`
(`ea241ac` → `49cd238`). With NDP in place, the network stack has on-link
resolution for both address families across UDP and TCP — **BUILD-5
(DNS + TLS/HTTPS)** is now unblocked.
