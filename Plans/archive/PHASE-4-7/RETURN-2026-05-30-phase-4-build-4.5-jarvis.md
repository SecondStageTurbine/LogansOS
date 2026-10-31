# RETURN: Phase 4 BUILD-4.5 - IPv6 Neighbor Discovery (NDP)

**Date:** 2026-05-30
**From:** Jarvis
**To:** PM (Cerebro)
**Branch:** `codex/phase-4-build-4.5-ndp-jarvis`
**Status:** COMPLETE

## Sub-Commits

- `6b94e24` `build-4.5a: ipv6 ndp neighbor solicitation/advertisement + cache (rfc 4861)`
- BUILD-4.5b: pending final commit `build-4.5b: route ipv6 udp/tcp egress through ndp + sweep + bare-metal marker`

## ISC Results

- PASS ISC-4.5a-1 NS/NA build+parse round-trip + ICMPv6 checksum
- PASS ISC-4.5a-2 injected NA populates cache; lookup returns MAC; solicited-node multicast derivation correct
- PASS ISC-4.5a-3 cache ages out via tick
- PASS ISC-4.5a-4 no panic / defensive parsing
- PASS ISC-4.5b-1 IPv6 UDP/TCP egress resolves dst MAC from cache (no placeholder)
- PASS ISC-4.5b-2 miss -> NS + queue -> injected NA flushes
- PASS ISC-4.5b-3 scenario SMOKE + NEO_SMP=4; full sweep green
- PASS ISC-4.5b-4 IPv6 SLAAC/UDP/TCP regression green; no new panic in 4.5b diff

## Verification

```text
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-phase4-ndp-ipv6-neighbor
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-phase4-slaac-udp-ipv6-loopback
docker compose run --rm neo cargo check -p neoplutarch-kernel --features neo-test-phase4-tcp-connect-stream-close
```

Result: PASS. The warning set is pre-existing.

```text
docker compose run --rm neo cargo neo-test phase4_ndp_ipv6_neighbor
docker compose run --rm neo cargo neo-test phase4_slaac_udp_ipv6_loopback
docker compose run --rm neo cargo neo-test phase4_tcp_connect_stream_close
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test phase4_ndp_ipv6_neighbor
```

Result: PASS. Focused QEMU coverage validates the NDP miss/queue/flush path,
the SLAAC/UDP IPv6 regression, and the TCP IPv6 wire path.

```text
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Result: PASS, exit code 0. Output captured in
`/tmp/build45-full-sweep-rerun.log`. The full sweep includes
`phase4_slaac_udp_ipv6_loopback`, `phase4_ndp_ipv6_neighbor`,
`phase4_tcp_connect_stream_close`, and the final
`phase3_aarch64_virtio_barrier_lane` scenario PASS.

```text
git diff --check
git diff -U0 | rg -n "panic!|todo!|unimplemented!|unwrap\(|expect\("
rg -n "ipv6_destination_mac" kernel/src/net/tcp.rs
```

Result: PASS. `git diff --check` exited 0; panic/todo/unwrap/expect scan had no
matches; `ipv6_destination_mac` scan had no matches.

## Serial Evidence

Source: `target/neo-test/phase4_ndp_ipv6_neighbor.serial.log`

```text
NEO_TEST/1 event=BEGIN name=phase4_ndp_ipv6_neighbor phase=dispatch
[ kernel ] ndp: ns_out target=2001:0db8:...:0000:0002 dst_mac=33:33:ff:00:00:02
NEO_TRACE/1 name=phase4_ndp_ipv6_neighbor step=ns_out target_low=000002 dst_mac=33:33:ff:00:00:02
[ kernel ] ndp: na_in cache_populated target=2001:0db8:...:0000:0002 mac=52:54:00:12:34:57
[ kernel ] ndp: frame_egress resolved target=2001:0db8:...:0000:0002 dst_mac=52:54:00:12:34:57
NEO_TRACE/1 name=phase4_ndp_ipv6_neighbor step=na_in cache=populated mac=52:54:00:12:34:57
NEO_TRACE/1 name=phase4_ndp_ipv6_neighbor step=udp_frame_egress resolved_mac=52:54:00:12:34:57
[ kernel ] ndp: frame_egress resolved target=2001:0db8:...:0000:0002 dst_mac=52:54:00:12:34:57
NEO_TRACE/1 name=phase4_ndp_ipv6_neighbor step=tcp_frame_egress resolved_mac=52:54:00:12:34:57
NEO_TEST/1 event=PASS name=phase4_ndp_ipv6_neighbor ndp=ok ns_out=ok na_in=ok cache=ok udp_ipv6=ok tcp_ipv6=ok tx_submissions_delta=3 rx_completions_delta=3
```

## Code-Review Notes

- `kernel/src/net/ndp.rs` now owns minimal IPv6 neighbor cache state, NS/NA
  encode/decode/checksum logic, solicited-node multicast derivation, TTL aging,
  and an ARP-style pending frame queue.
- UDP IPv6 egress in `socket.rs` and TCP IPv6 egress in `tcp.rs` both route
  through `ndp::submit_or_queue_ipv6`; cache miss sends an NS and queues the
  Ethernet frame, and injected NA cache insertion flushes queued frames.
- The TCP IPv6 placeholder is gone: `ipv6_destination_mac()` no longer exists in
  `kernel/src/net/tcp.rs`.
- `phase4_ndp_ipv6_neighbor` proves NS out -> NA in -> cache populated -> queued
  UDP frame egress -> TCP IPv6 frame egress with the resolved MAC.
- `phase4_slaac_udp_ipv6_loopback` pre-seeds the synthetic peer cache entry so
  that regression remains focused on SLAAC/UDP egress instead of unresolved NDP
  queue state; the dedicated NDP scenario covers the miss/queue/flush path.
- `spawn_neo_init` services virtio-net completions before the final frame audit
  so completed test egress frames are reclaimed before leak accounting.
- smoltcp remains phy-only. No syscall or ABI changes were made.
- FM-4.5-1 real DAD and FM-4.5-2 Router Solicitation remain deferred.

## Blockers

None.
