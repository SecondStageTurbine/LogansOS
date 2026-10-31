# BUILD-3f Bare-Metal Deferral Marker

**Date:** 2026-05-30
**Build:** Phase 4 BUILD-3 - IPv4 + ARP + DHCPv4 + dual-stack source selection
**Status:** TRACKED, not blocking BUILD-3 close

## Deferred Surface

BUILD-3 validates the IPv4 egress floor in QEMU: DHCPv4 message framing leaves
virtio-net, synthetic ACK acquisition installs the lease, ARP miss queues the
IPv4/UDP datagram, injected ARP reply flushes the pending frame, and the final
Ethernet+IPv4+UDP frame is observed with the DHCP-selected source address.

The following real-network behaviors remain for the bare-metal lane:

- Real DHCPv4 lease acquisition from a live server.
- Real ARP request/reply exchange with a live IPv4 peer.
- Real IPv4 UDP reply reception after remote egress.
- RFC 5227 on-wire conflict/probe/defend behavior beyond the deterministic ACD stub.

## Rationale

This matches the BUILD-3 opening decision: QEMU gates frame correctness and
virtio-net TX, while real server/peer behavior is hardware-network dependent
and should not make CI flaky.

## Tracking

- Closes ISC-3f-3 as TRACKED.
- Carry into the next bare-metal network verification pass.
