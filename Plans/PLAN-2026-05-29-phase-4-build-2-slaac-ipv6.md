# PLAN: Phase 4 BUILD-2 — IPv6 address autoconfig (SLAAC) + UDP transport stack

**Date:** 2026-05-29
**Authority:** Phase 4 THINK (2026-05-22, archived), Phase 4 goal milestone (§2)
**From:** PM (Cerebro)
**To:** Jarvis (codex/phase-4-build-2-jarvis)

---

## Context

BUILD-1b delivered the socket syscall surface (0x180–0x184: create, bind, send, recv, close) plus userland wrappers. The smoltcp integration (BUILD-1a) provides the lower-layer IP/UDP/ICMP handling. BUILD-2 delivers the missing piece between syscalls and a working UDP endpoint: **automatic IPv6 address acquisition via SLAAC** and **full UDP send/receive with kernel-managed packet dispatch.**

**Goal milestone:** A QEMU-hosted userspace process acquires a global IPv6 address automatically (via Router Advertisements + SLAAC), opens a UDP socket, binds to the acquired address, and exchanges datagrams with a simulated remote host on the same network.

---

## ISCs (Acceptance Criteria)

**ISC-2-1:** SLAAC state machine compiles (RFC 4862 + Privacy Extensions per RFC 8981)
- IPv6 address state enum (tentative, preferred, deprecated, invalid)
- Duplicate Address Detection (DAD) coroutine + timeout handling
- Privacy Extension temporary address generation + lifecycle
- Interface address table with kernel-owned list

**ISC-2-2:** Router Advertisement parsing and processing works (RFC 4861)
- RA reception on IPv6 link-local (ff02::1 multicast listen)
- PIO (Prefix Information Option) extraction
- RIO (Route Information Option) extraction + route installation (if applicable at BUILD-2 scope)
- RA timer reset on each RA reception (30 sec default, configurable)

**ISC-2-3:** UDP datagram loopback send/recv works end-to-end
- Process A: `sys_socket_create(UDP) -> handle_A`
- Process A: `sys_socket_bind(handle_A, [::1]:5555)`
- Process B: `sys_socket_create(UDP) -> handle_B`
- Process B: `sys_socket_bind(handle_B, [::1]:5556)`
- Process A: `sys_socket_send(handle_A, packet, [::1]:5556)`
- Process B: `sys_socket_recv(handle_B) -> packet` (received packet matches sent)
- Both: verify loopback short-circuit is being used (kernel log + statistics confirm no driver involvement)

**ISC-2-4:** UDP send/recv on SLAAC-acquired address works (QEMU only, not bare-metal yet)
- Router (QEMU simulated or a real router on the VLAN) advertises a prefix, e.g., `2001:db8::/64`
- NeoPlutarch process receives RA, derives global unicast address (e.g., `2001:db8::pid:tid:randomid`)
- Process binds to the global address, sends UDP to a "remote" host on the same prefix (e.g., a netcat listener on the host-side network)
- Process receives UDP response from remote
- Packet inspection confirms correct source IPv6 address, correct dest, correct UDP ports

**ISC-2-5:** QEMU smoke test scenario (phase4_slaac_udp_ipv6_loopback) passes on both NEO_TEST=SMOKE and NEO_SMP=4 variants

**ISC-2-6:** Kernel UDP accounting is visible
- `sys_net_stats(handle)` returns packet counts (in/out, bytes in/out) for the socket
- Per-process accounting (`sys_proc_stats(..., NET)` if such syscall exists) is updated

**ISC-2-7:** No unwrap/panic in new IPv6 or UDP-refinement code

---

## Non-scope

- IPv4, ARP, DHCPv4 (Phase 4 BUILD-3)
- TCP, congestion control (Phase 4 BUILD-4)
- DNS, Happy Eyeballs (Phase 4 BUILD-5+)
- TLS, certificate validation (Phase 4 BUILD-5+)
- e1000e or other real-hardware drivers (forward-marker'd)
- Routing table modification API
- Policy routing, source address selection (full RFC 6724; BUILD-2 implements only the SLAAC side)
- mDNS, link-local discovery

---

## Implementation sketch (sub-commits)

### BUILD-2a — SLAAC state machine + RA processing

**What:** IPv6 address lifecycle + RA parser

**Files:**
- `kernel/src/net/ipv6.rs` (new): Address table, SLAAC state machine, DAD coroutine, Privacy Extensions timer
- `kernel/src/net/icmpv6.rs` (new or extend): RA handler, PIO/RIO option extraction
- `kernel/src/sched/process.rs` (amend): per-process IPv6 address list + refcount

**Deliverables:**
- Address state machine: tentative → (DAD succeeds) → preferred → (lifetime expires) → deprecated → invalid
- RA listener on ff02::1 (solicited and unsolicited)
- PIO prefix extraction: apply RFC 4862 prefix processing
- Privacy Extension IID: use RFC 8981 temp address generation (truncated hash of base IID + random value, daily rotation)
- Timers: preferred lifetime, valid lifetime, RA recv timeout (30 sec)

**Acceptance:** ISC-2-1, ISC-2-2

### BUILD-2b — UDP send/recv refinement + loopback short-circuit validation

**What:** Extend the socket API to fully wire up send/recv paths, validate loopback interception

**Files:**
- `kernel/src/net/socket.rs` (amend): UDP send path (move from BUILD-1b "bind-only" to "send/recv")
- `kernel/src/net/udp.rs` (new or extend): UDP RX queue per socket, notification on arrival, send-to-RX-queue for loopback
- `kernel/src/net/device.rs` (amend): loopback short-circuit trigger on dest `::1`

**Deliverables:**
- UDP send syscall fully wired (currently a stub returning ENOTSUP or ENOTIMPL): packet assembly, length checks, socket-state validation
- UDP RX queue: bounded (e.g., 16 packets per socket), notification wake on arrival
- Loopback intercept: if dest IP is `::1`, short-circuit before driver, deliver directly to destination socket's RX queue
- Statistics bump: per-socket packet/byte counters

**Acceptance:** ISC-2-3, ISC-2-6

### BUILD-2c — QEMU SLAAC integration test + accounting

**What:** Wiring SLAAC into QEMU test harness, adding sys_net_stats syscall

**Files:**
- `userland/libneo/src/lib.rs` (amend): add `sys_net_stats(handle)` syscall wrapper
- `kernel/src/arch/x86_64/syscall.rs` (amend): syscall 0x185 (`sys_net_stats`)
- `userland/neo-init/src/main.rs` (amend): phase4_slaac_udp_ipv6_loopback test scenario
- `tools/xtask/src/main.rs` (amend): wire test scenario into harness

**Deliverables:**
- `sys_net_stats` syscall: returns `{packets_in, packets_out, bytes_in, bytes_out}` for a socket handle
- Test scenario: spawn process, let it acquire SLAAC address, bind UDP socket, loopback send/recv to verify round-trip
- QEMU network setup (harness): configure virtio-net with a subnet, QEMU DHCPv6 server or simulated RA generator

**Acceptance:** ISC-2-4, ISC-2-5, ISC-2-6

### BUILD-2d — Full sweep + code quality

**What:** NEO_SMP=4 cargo neo-test all; code review

**Acceptance:** ISC-2-7, general code standards

---

## Risks and mitigations

### R1. SLAAC + DAD interactions with the UDP delivery path

Scenario: A socket is bound to an address. The address becomes tentative (in DAD). A UDP packet arrives destined for that address. What happens?

**Mitigation:** Decision at PLAN: (a) reject incoming UDP on tentative addresses (safest, per RFC 4862); (b) accept but mark as "tentative source" for application to decide. Choose (a) for BUILD-2 simplicity. Later (Phase 5+), add a socket option if an application needs tentative-address behavior.

### R2. Privacy Extension timer interactions with long-lived sockets

Scenario: A socket is bound to a Privacy Extension temporary address. The temp address's preferred lifetime expires. The socket still has in-flight or buffered packets. What happens?

**Mitigation:** Decision at PLAN: (a) socket close on temp-address expiration (aggressive but simple); (b) socket marked as "stale-address" but continues operating (requires RFC 8605 understanding). Choose (a) for BUILD-2. Later versions (Phase 5+) can soften this if apps complain.

### R3. QEMU RA simulation might not behave like real routers

Some QEMU network backends do not forward RA or have buggy RIO handling.

**Mitigation:** Use QEMU's built-in IPv6 support (tap interface with kernel IPv6 forwarding on the host). Alternatively, spin up dnsmasq or radvd on the host to serve RAs. Both paths are low-complexity; defer the exact choice to PLAN.

---

## Sequencing and timeline

- **PLAN approval + Jarvis dispatch:** 1 day
- **BUILD-2a (SLAAC + RA):** 3–4 days (state machine is well-specified by RFC; main complexity is Privacy Extensions RFC 8981 edge cases)
- **BUILD-2b (UDP send/recv):** 2 days (wiring syscall to the stack)
- **BUILD-2c (integration test):** 1 day (harness work is routine)
- **BUILD-2d (sweep + QA):** 2 days
- **Estimate total:** ~9 days (7 working days accounting for weekends)

---

## Artifacts

**Inputs:**
- Phase 4 THINK 2026-05-22 (authority)
- Phase 4 BUILD-1a/1b code on origin/main

**Outputs:**
- PLAN (this document, on main after PL ratification)
- Feature branch: `codex/phase-4-build-2-jarvis`
- RETURN report on close
- PL-RATIFICATION on close

---

**Next step: PL ratification, then dispatch to Jarvis.**