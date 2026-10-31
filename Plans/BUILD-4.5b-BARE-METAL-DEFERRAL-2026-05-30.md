# BUILD-4.5b Bare-Metal Deferral - IPv6 Neighbor Discovery

**Date:** 2026-05-30
**Owner:** Jarvis
**Status:** TRACKED - not blocking BUILD-4.5 close
**Branch:** `codex/phase-4-build-4.5-ndp-jarvis`

## Deferred Scope

BUILD-4.5 lands QEMU-verified hand-rolled IPv6 NDP for on-link neighbor
resolution, but does not claim live-network bare-metal coverage for:

- real host/NIC IPv6 Neighbor Solicitation and Neighbor Advertisement exchange
  against an external peer;
- FM-4.5-1 real Duplicate Address Detection using NS/NA;
- FM-4.5-2 Router Solicitation and router-discovered prefixes;
- full RFC 4861 Neighbor Unreachability Detection, Redirect, or SEND.

## Landed Coverage

- `net/ndp.rs` builds/parses NS and NA, validates ICMPv6 pseudo-header
  checksums, derives solicited-node multicast IPv6/MAC destinations, maintains a
  TTL-aged neighbor cache, and exposes an injected-packet test hook.
- IPv6 UDP and TCP egress share `ndp::submit_or_queue_ipv6`.
- Cache miss sends solicited-node multicast NS and queues the Ethernet frame.
- Injected NA populates the cache and flushes the queued UDP frame.
- TCP IPv6 egress uses the same resolved neighbor MAC path; the previous
  placeholder destination MAC helper has been removed.
- QEMU serial evidence captures NS out -> NA in -> cache populated -> resolved
  UDP/TCP frame egress.

## Follow-Up Trigger

Close this marker when a later build validates live IPv6 neighbor discovery on a
real host/NIC path and, if selected by PM/PL, promotes real DAD and Router
Solicitation from deferred scope into implementation scope.
