# BUILD-4g Bare-Metal Deferral: TCP Real Peer Verification

**Date:** 2026-05-30
**Owner:** Jarvis
**Status:** TRACKED - not blocking BUILD-4 close

## Deferred Scope

BUILD-4g validates virtio-net TCP frame egress for IPv4 and IPv6 in QEMU, plus
deterministic injected-peer TCP handshake, stream, retransmit, CUBIC, RFC 5961,
RFC 7323, and close/TIME-WAIT behavior.

The following remain bare-metal/live-network validation items:

- Real remote-peer TCP handshake over IPv4 and IPv6.
- Real bidirectional TCP stream transfer across a physical NIC path.
- Real RTT/RTO behavior under host/network delay.
- Real loss recovery and CUBIC response under observable packet loss.
- Real RFC 5961 challenge-ACK behavior against live out-of-window RST/ACK input.
- Real TIME-WAIT retention and process attribution across live process teardown.
- IPv6 neighbor discovery integration for non-multicast IPv6 TCP peer egress.

## QEMU Coverage Landed

- IPv4 TCP virtio-net egress builds Ethernet + IPv4 + TCP frames and resolves
  destination MACs through the ARP cache/queue path.
- IPv6 TCP virtio-net egress builds Ethernet + IPv6 + TCP frames with the current
  placeholder L2 destination behavior until neighbor discovery lands.
- `phase4_tcp_connect_stream_close` asserts both IPv4 and IPv6 wire egress
  captures and TX submission movement.

## Close Disposition

Bare-metal live-peer verification is explicitly deferred to the hardware/network
validation lane and does not block BUILD-4 closure.
