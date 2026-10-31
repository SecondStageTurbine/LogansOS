<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Phase 4 - Network Stack

| | |
|---|---|
| **Status** | **CLOSED.** BUILD-1 through BUILD-5 complete and merged to `main` (`85d7a3d`). Phase 4 ships: end-to-end HTTPS GET over a raced dual-stack TLS 1.3 connection on NeoPlutarch's own ABI. |
| **Design start** | 2026-05-22 (THINK v0.2) |
| **Builds** | 2026-05-28 (BUILD-1a) through 2026-05-30 (BUILD-5) |
| **Closed** | 2026-05-30 |
| **Canonical design** | `Plans/PHASE-4-THINK-v0.2-2026-05-22-network-stack-design-direction.md`; opening `Plans/PM-DECISION-2026-05-28-phase-4-opening.md`; ADR-017 §D3 (connectivity) |
| **BUILD close reports** | `Plans/CLOSE-REPORT-2026-05-29-phase-4-build-1b.md`, `...-build-2.md`, `...-2026-05-30-phase-4-build-3.md`, `...-build-4.md`, `...-build-4.5.md`, `...-build-5.md` |

---

## Scope

Phase 4 builds NeoPlutarch's in-kernel network stack on the design ratified in
THINK v0.2. The phase posture: **IPv6 as default** with IPv4 supported (Bible
Article 11.2); a **native sockets-equivalent ABI** that is NeoPlutarch's own,
not a BSD-sockets shim; a **hand-rolled control plane** (smoltcp kept PHY-only)
with **no hand-rolled cryptographic primitives** (Article 11.3); ending in an
authenticated HTTPS fetch.

**In scope:** virtio-net device + smoltcp PHY wrap; IPv6 core (ICMPv6, ND,
SLAAC, RFC 8981 Privacy Extensions, RFC 8106 RDNSS); IPv4 + ARP + DHCPv4;
dual-stack source/destination selection (RFC 6724); hand-rolled UDP and TCP;
IPv6 NDP; DNS stub resolver; Happy Eyeballs (RFC 8305); hybrid kTLS.

**Anti-scope (deferred by design):** no AI subsystem (Phase 5+); no WiFi
(Phase 7.5 / ADR-017); no BSD-sockets compatibility shim in the native ABI
(personality-edge only, Article 5.7); no namespacing / multi-tenancy
(Phase 8+); no mDNS responder; no routing-table modification API; no
completion-based I/O (io_uring forward-marker); no pluggable congestion control
(CUBIC only; BBR deferred).

The phase ran as seven builds: BUILD-1 (1a substrate + 1b IPv6 core), BUILD-2
(SLAAC UDP), BUILD-3 (IPv4 + DHCPv4), BUILD-4 (TCP), BUILD-4.5 (NDP), and
BUILD-5 (DNS + Happy Eyeballs + kTLS, the ship). All 92 ISCs across the phase
passed.

## BUILD-1 - Virtio-net Substrate and IPv6 Core

BUILD-1 split into a device substrate (1a) and the IPv6 stack core (1b).

- `43fc2da` 1a: virtio-net PCI probe/bind (vendor `0x1AF4`, device `0x1000`),
  RX/TX ring management with a three-barrier publication-fence discipline
  (Release/SeqCst/Acquire, inherited from the Phase-3 BUILD-5b `2fdb7cb`
  bare-metal lesson), a minimal smoltcp PHY wrap shim, and an Ethernet-frame
  round-trip smoke test. No userspace syscall surface. (5 ISCs.)
- `c07a4fb` 1b: IPv6 address state machine — SLAAC stable + temporary addresses
  (RFC 4862 + RFC 8981 Privacy Extensions), ICMPv6 Echo, Neighbor Discovery
  parsing, and the RFC 8106 RDNSS option lifted into a kernel-resident
  `ResolverConfig` (populated-but-unused until BUILD-5). ISC revision `51f8e47`,
  merge `55bb011`. (5 ISCs.)

BUILD-1 introduced the UDP socket syscall floor `0x180`–`0x184`
(create/bind/send/recv/close). Design risks **R11** (IPv6 extension-header
attack surface) and **R13** (Privacy-Extensions + capability-bound socket
lifetime) were discharged by the 1f and 1.1 sub-builds respectively.

## BUILD-2 - IPv6 SLAAC UDP Transport

BUILD-2 closed with destination-aware IPv6 UDP over SLAAC-acquired addresses.
Implementation `6215d5a` (2a) → `b29b615` (2b) → `a448036` (2c) → `81e3e36`
(2d), merged `05cff8d`. (7/7 ISCs.)

BUILD-2 delivered the SLAAC address lifecycle (deprecation/removal), UDP
`sendto`/`recvfrom` with destination addressing, per-socket statistics, and the
RA-injection test path. It added syscalls `0x185` (`SYS_NET_STATS`), `0x186`
(`SYS_SOCKET_SENDTO`), and `0x187` (`SYS_SOCKET_RECVFROM`).

## BUILD-3 - IPv4 + DHCPv4 + Dual-Stack Selection

BUILD-3 closed with the IPv4 family and dual-stack source selection. The series
was `4a40e43` (3a IPv4 header + ARP, RFC 826) → `f60d239` (3b DHCPv4 state
machine + lease timers, RFC 2131/2132) → `3365eed` (3c dual-family interface +
RFC 6724 source selection) → `218421e` (3d send hook + IPv4 loopback) →
`19e466d` (3e virtio-net IPv4/UDP wire TX + ARP resolution) → `5d1edcd` (3f
sweep + bare-metal marker), ff-merged `d1ba0e8` → `5d1edcd`. (21/21 ISCs.)

BUILD-3 delivered hand-rolled IPv4 framing, an ARP cache (RFC 826) with aging,
a full DHCPv4 client state machine (Init → Selecting → Requesting → Bound →
Renewing → Rebinding) with T1/T2 lease timers, and family-directed RFC 6724
source selection (IPv4 dst → IPv4 source; IPv6 dst → IPv6 source). It raised
forward-markers FM-3-1 (RFC 6724 prefer-IPv6 *destination* default — deferred to
BUILD-5), FM-3-2 (zero UDP checksum, RFC 768-valid on IPv4), and FM-3-3
(`net::tick` harness-driven, not yet ISR-wired).

## BUILD-4 - Hand-Rolled TCP

BUILD-4 closed with a hand-rolled TCP, smoltcp still PHY-only and the UDP
`Socket` type left untouched (type fork). The series was `438cae2` (4a codec +
RFC 9293 skeleton) → `1b40e89` (4b syscalls `0x188`–`0x18B` + cap derivation +
READ/WRITE rights) → `13b5df8` (4c 3-way handshake) → `c4343e1` (4d data
transfer + retransmit + RFC 7323 window scaling) → `8a605d2` (4e CUBIC) →
`8f9699e` (4f RFC 5961 challenge-ACK + clean close + TIME-WAIT accounting) →
`fb690c9` (4g wire egress + sweep + bare-metal marker), ff-merged `ea241ac` →
`fb690c9`. (28/28 ISCs.)

BUILD-4 discharged the three speculative TCP-sketch forward-markers: **FM-1**
(cap derivation — `accept` mints `Handle::TcpSocket` via the existing Phase-2
`install_cap` lineage, zero machinery change), **FM-2** (independent READ/WRITE
rights bits; `shutdown(WRITE)` clears WRITE, keeps READ), and **FM-3**
(TIME-WAIT decoupled from the cap via owner-attributed state). It raised
**FM-4-1** (IPv6 next-hop resolution gap — IPv6 TCP/UDP egress framed correctly
but with a placeholder destination MAC, because IPv6 has no ARP and NDP did not
yet exist).

## BUILD-4.5 - IPv6 Neighbor Discovery

BUILD-4.5 was a focused ARP-parity build inserted to close FM-4-1 before the
ship. `6b94e24` (4.5a NS/NA + neighbor cache, RFC 4861) → `49cd238` (4.5b route
IPv6 UDP/TCP egress through NDP + sweep + bare-metal marker). (8/8 ISCs.)

It delivered hand-rolled NS/NA build/parse with ICMPv6 checksum, a minimal
neighbor cache with TTL aging, solicited-node multicast derivation, and a shared
`ndp::submit_or_queue_ipv6` resolve/queue path for both UDP and TCP — removing
the placeholder destination MAC. **FM-4-1 discharged.** Forward-markers FM-4.5-1
(real NS-based DAD) and FM-4.5-2 (Router Solicitation) deferred.

## BUILD-5 - DNS + Happy Eyeballs + kTLS (Phase 4 Ship)

BUILD-5 ships the phase. `aab4923` (5a DNS stub resolver + `ResolverConfig`) →
`c26f1f3` (5b Happy Eyeballs + RFC 6724 prefer-IPv6 destination) → `439f853`
(5c kTLS record layer) → `2c12357` (5d rustls handshake + HTTP/1.1) → `85d7a3d`
(5e ship scenario + sweep). (18/18 ISCs.)

BUILD-5 delivered a kernel-resident RFC 1035 stub resolver (A/AAAA, EDNS(0), TTL
cache) consuming RDNSS and DHCPv4 DNS into `ResolverConfig`; Happy Eyeballs
(RFC 8305) dual-stack connection racing with RFC 6724 destination ordering
(**discharging FM-3-1**); and the hybrid kTLS split — userspace `rustls` 1.3
handshake (`rustls-rustcrypto` provider restricted to a single suite,
`TLS13_CHACHA20_POLY1305_SHA256`, TLS 1.3 only) over a kernel record layer
(`sys_net_tcp_enable_tls`) running the vetted `chacha20poly1305` AEAD
**synchronously per packet**, with key material WRITE-gated, copied-not-shared,
and zeroized. The native ABI completes at `0x18C`–`0x18E` (`SYS_NET_RESOLVE`,
`SYS_NET_DIAL`, `SYS_NET_TCP_ENABLE_TLS`). The `phase4_https_fetch` scenario
proves the full chain in CI.

## Course Corrections

The **BUILD-2 send-destination ABI blocker** (`BLOCKER-2026-05-29-...-send-destination-abi`)
surfaced that BUILD-1b's merged socket floor (`0x180`–`0x184`) had no
destination-address argument, blocking BUILD-2's UDP `sendto`. Resolved by PM
ratification authorizing `0x186`/`0x187` (`SENDTO`/`RECVFROM`) plus the
RA-injection test path, rather than retrofitting `connect` semantics onto UDP.

The **BUILD-2d SMP=4 perf-proxy blocker** (`BLOCKER-2026-05-29-...-perf-proxy`)
was a `phase2_perf_proxy_baseline` perf-counter drift under `NEO_SMP=4`
(pre-existing Phase-2 scheduler behavior, not a network-path fault). Resolved by
a separate-lane carve-out: the perf-proxy benchmark was excluded from the
functional close gate and its root cause escalated as a pre-Phase-5
prerequisite. The underlying SMP-gate and syswait-race fixes (`a4d37f2`,
`c0a3b04`) held from BUILD-3 onward, so no further carve-out was needed —
BUILD-3, BUILD-4, BUILD-4.5, and BUILD-5 each closed on a fully green
`neo-test all` at `NEO_SMP=4 ATTEMPTS=1`.

## Verification

Each build closed against per-ISC verification recorded in its close report.
The standard close gate per build was: feature-gated `cargo check`; `git diff
--check`; a panic/unwrap/expect/todo scan over the diff; targeted `rustfmt
--check`; the build's dedicated QEMU scenario at SMOKE + `NEO_SMP=4`; and the
full `cargo neo-test all --no-meta` sweep at `NEO_SMP=4 ATTEMPTS=1`, exit `0`.
PM independent verification was by source audit for host-testable sub-commits
(the PM seat has no QEMU) and by accepting the Jarvis QEMU sweep for wire/SMP
scenarios.

The phase ship gate (`phase4_https_fetch`) asserts the decrypted HTTP body after
`resolve → Happy Eyeballs → TLS 1.3 handshake → kTLS record exchange`, with the
serial log showing `event=PASS … https_fetch=ok dns=ok happy_eyeballs=ok
tls13=rustls ktls=ok http_body=ok`.

---

## Phase 4 Formal Close — 2026-05-30

Phase 4 is **CLOSED** at BUILD-5 (`85d7a3d`).

### Builds completed

BUILD-1 (1a `43fc2da`, 1b `c07a4fb`), BUILD-2 (`05cff8d`), BUILD-3 (`5d1edcd`),
BUILD-4 (`fb690c9`), BUILD-4.5 (`49cd238`), BUILD-5 (`85d7a3d`). **92/92 ISCs
PASS.** Native network ABI `0x180`–`0x18E` (15 syscalls).

### Forward-markers discharged within the phase

- **R11** (ext-header attack surface) → BUILD-1f; **R13** (Privacy-Extensions
  lifetime) → BUILD-1.1.
- **FM-1 / FM-2 / FM-3** (TCP cap derivation / READ-WRITE independence /
  TIME-WAIT decoupling) → BUILD-4.
- **FM-4-1** (IPv6 NDP gap) → BUILD-4.5.
- **FM-3-1** (RFC 6724 prefer-IPv6 destination) → BUILD-5.

### Forward-markers carried beyond the phase

- **FM-3-2** zero UDP checksum (IPv4); **FM-3-3** `net::tick` ISR-wiring.
- **FM-4.5-1** real NS-based DAD; **FM-4.5-2** Router Solicitation.
- **FM-5-1** TCP/53 truncation fallback; **FM-5-2** HEv3 protocol-priority;
  **FM-5-3** AES-128-GCM suite; **FM-5-4** full/system trust store.

### Bare-metal deferrals (hardware-network validation lane)

`BUILD-3f-BARE-METAL-DEFERRAL` (live DHCPv4/ARP/IPv4 UDP, RFC 5227 on-wire ACD),
`BUILD-4g-BARE-METAL-DEFERRAL` (live remote-peer TCP handshake/stream/RTT/loss/
RFC 5961/TIME-WAIT), `BUILD-4.5b-BARE-METAL-DEFERRAL` (live NS/NA, real DAD, RS,
full RFC 4861 NUD/Redirect/SEND). The BUILD-5 real external dual-stack HTTPS
fetch over real outbound (QEMU SLIRP) is the phase-level bare-metal deliverable.
QEMU gates frame correctness and wire egress; real server/peer behavior is
hardware-network dependent and is held to the metal lane so CI stays
deterministic.

### Deviations

- **BUILD-4.5 was unplanned.** It was inserted between BUILD-4 and the ship to
  discharge FM-4-1 (IPv6 NDP), which BUILD-4's placeholder destination MAC had
  exposed. Small and well-bounded (the IPv6 analogue of BUILD-3a's ARP work).
- **Two blockers** (UDP destination ABI; SMP=4 perf-proxy) were resolved by ABI
  authorization and a benchmark-lane carve-out respectively; neither required
  network-code rework.
- **USB stack / USB Ethernet (ADR-017 §D3)** was named in the opening scope as
  the bare-metal connectivity path but was **not delivered** across BUILD-1–5;
  the stack shipped over virtio-net. Dispositioned by PL decision 2026-05-30
  (`Plans/PL-DECISION-2026-05-30-usb-ethernet-descope.md`, Article 24.1):
  **re-deferred to the Phase 7.5 connectivity lane** alongside WiFi. Phase 4
  closes on the virtio-net stack; the Article 13 ledger (Bible v1.9.0) records
  the completion and the re-deferral. This is a re-deferral, not a cancellation —
  USB Ethernet remains a committed Phase 7.5 deliverable.

### Outcome

The Phase 4 network stack — virtio-net device, IPv6 (SLAAC + Privacy Extensions
+ ICMPv6 + NDP), IPv4 (ARP + DHCPv4), dual-stack RFC 6724 selection, hand-rolled
UDP and TCP (CUBIC + RFC 7323 + RFC 5961), a DNS stub resolver, Happy Eyeballs,
and hybrid kTLS — is complete and verified in CI. NeoPlutarch resolves a
hostname, races a dual-stack connection, completes a TLS 1.3 handshake, and
performs an HTTPS GET, end to end, on its own native ABI, with no hand-rolled
cryptography. **Phase 4 ships.** USB stack + USB Ethernet were re-deferred to
the Phase 7.5 connectivity lane (PL decision 2026-05-30); the Article 13 ledger
records Phase 4 Complete at Bible v1.9.0. Phase 5 (AI subsystem foundation) is
next per the ledger.
