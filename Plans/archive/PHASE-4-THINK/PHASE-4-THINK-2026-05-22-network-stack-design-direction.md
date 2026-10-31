# Phase 4 THINK — Network Stack Design Direction

**Date:** 2026-05-22
**Recorder:** PM
**Status:** DRAFT — awaiting PL ratification
**Cadence position:** THINK (per Bible Article 14). OBSERVE closed 2026-05-20 (Cerebro). PLAN follows ratification.

**Authority chain consumed:**

- `docs/bible.md` v1.7.0 — Articles 1.2, 1.4, 3, 4.1, 9.10, 11, 12, 14, 18.1, 28
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md` — PL design input + PM commentary
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md` — citation refreshes (RFC 8504, RFC 9915, HEv3 draft)
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` — Cerebro research, 14 design questions enumerated
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md` — OBSERVE consumption
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md` — sequencing decision (Phase 4 opens post Phase 3 closure)
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` — `BlockDevice` trait pattern that `NetDevice` will mirror

**Sequencing note.** This THINK pass is authored 2026-05-22 with Phase 3 close pending (BUILD-5b bare-metal execution gated on Jarvis log-sink dispatch and PL hardware session). PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md preserved the convention that Phase 4 THINK opens after Phase 3 formal close. PL authorization on 2026-05-22 opens Phase 4 THINK in parallel with the pending Phase 3 closure, on the grounds that THINK is PM-side document work with no kernel-code substrate dependency and the wait would be idle PM time. Phase 4 PLAN does NOT open until Phase 3 formally closes; THINK is preparatory.

---

## 1. Scope of This Artifact

This THINK pass:

- **Chooses** among the 14 design questions Cerebro's OBSERVE enumerated, scoped to what Phase 4 BUILD-1 ships.
- **Sequences** Phase 4 into BUILD-1 through BUILD-Nb sub-commits with explicit goal milestones.
- **Enumerates risks** premortem-style and assigns mitigations.
- **Identifies prerequisites** that must be true before BUILD-1 can start.
- **Runs the /simplify gate** per Bible Article 14 and documents what was cut.
- **Surfaces final PL ratifications** needed before PLAN opens.

This THINK pass does NOT:

- Open PLAN. PLAN follows ratification.
- File ADRs. Individual design decisions that need durable normative force become ADRs at PLAN or BUILD-1 time.
- Touch kernel code, syscall ABIs, or `xtask`.
- Re-litigate the six confirmed PM-NOTE positions (dual-stack mandatory, IPv6-preferred + RFC 8305/6724, TLS primary + IPsec deferred, NAT64/DNS64 deferred, no BSD sockets, capability-protected sockets-equivalent). These are the Bible-grounded substrate the choices below rest on.

---

## 2. The Goal Milestone (Phase 4 Overall)

Phase 4 ships when:

> A NeoPlutarch userspace process, holding only a network-access capability handle, sends a TCP-framed HTTPS request to a real remote server on the dual-stack public Internet, receives the response, and verifies the TLS certificate against a kernel-managed trust anchor. The same process can also send and receive UDP datagrams on both IPv4 and IPv6. The Phase 3 substrate (block I/O, NeoFS) remains green; no regressions.

Sub-goal milestones map onto sub-BUILDs in §4. The Phase 4 ship moment is **BUILD-5 close** in the sub-BUILD numbering below.

**Why this milestone:** it exercises every load-bearing component of an endpoint OS's network story — capability gate, virtio-net (or equivalent) driver, dual-stack IP routing, ND/SLAAC + ARP/DHCPv4 address acquisition, UDP and TCP transport, Happy Eyeballs racing, DNS, TLS framing, application-driven send/receive. Anything less and the goal-milestone gap to "Bible Article 28 usable by real people" remains.

---

## 3. The Chosen Position on Each Open Design Question

Cerebro's OBSERVE §6 enumerated 14 design questions. Each is addressed here at BUILD-1 scope; BUILDs that deliver later capabilities are noted in §4.

### 3.1 Sockets-equivalent API shape (Cerebro §6.1)

**Choice: layered-handle shape.** Following the Fuchsia precedent but native to NeoPlutarch's capability table rather than channel-routed.

A process holds a `NetworkAccess` outer capability handed to it at spawn (or inherited). From `NetworkAccess` the process creates per-socket inner handles via a syscall family: `sys_net_socket_udp(NetworkAccess) -> Result<UdpSocket>`, `sys_net_socket_tcp(NetworkAccess) -> Result<TcpSocket>`, `sys_net_listen_tcp(NetworkAccess, bind_spec) -> Result<TcpListener>`, etc. Each inner handle is itself a capability that can be passed to other processes via the standard capability-passing mechanism (per Bible Article 12).

**Why this shape over the alternatives:**

- **Endpoint-shaped** (Hubris compile-time binding): too rigid for a multi-process OS where socket count is dynamic per process. Rejected.
- **Channel-shaped** (Plan 9 `/net` namespace): would require committing to a filesystem-namespace primary capability model, which NeoPlutarch's existing capability-handle architecture does not match. Rejected.
- **Connection-shaped only:** insufficient — must support listening sockets that are not yet connected. Rejected.
- **Layered (chosen):** flexible enough to add new socket types (raw, ICMP, ICMPv6) without API-shape revisions; composes naturally with the existing handle table; per-handle rights bitmaps can be added later (Capsicum precedent) without breaking the shape.

**Forward-compatibility with HEv3 protocol-priority:** the connection-establishment syscall (`sys_net_connect_tcp` and a future `sys_net_dial_https`) takes a candidate-list parameter, not an `(addr, port)` tuple. The candidate-list type is `&[NetCandidate]` where `NetCandidate` carries `(protocol, transport, address, port, alpn_hint)`. HEv2 fills it with `[(any, tcp, v6, port), (any, tcp, v4, port)]`. HEv3 future-extends by adding QUIC and SVCB-derived ALPN hints to the candidate list; the syscall surface does not change.

### 3.2 In-kernel transport scope at BUILD-1 (Cerebro §6.2)

**Choice: UDP and ICMPv6 at BUILD-1. TCP at BUILD-3.**

BUILD-1 has the smallest possible end-to-end stack: virtio-net driver, IPv6 (RFC 8200), ICMPv6 (RFC 4443), ND (RFC 4861), SLAAC (RFC 4862), UDP (RFC 768), capability gate, layered-handle API. No TCP, no IPv4, no Happy Eyeballs, no DNS, no TLS yet. This is enough to prove the architecture end-to-end and small enough to ship without sub-BUILD explosion.

**Why UDP first:** RFC 9293 (TCP) is large surface area — sequence numbers, window scaling, RTT estimation, retransmission, congestion control, RFC 5961 blind-injection mitigations, RFC 7323 extensions. Adding it to BUILD-1 doubles or triples the scope. UDP exercises the same packet pipeline (driver → IP → transport → capability-protected user delivery) at a fraction of the protocol surface, validating the architecture before committing to TCP correctness on it.

### 3.3 TLS placement (Cerebro §6.3)

**Choice: hybrid kTLS-style. Deferred to BUILD-5.**

Userspace owns the TLS handshake (rustls in libneo or similar — userspace concern, not Bible-scope). Once the handshake completes, the kernel takes ownership of the symmetric record layer via a `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)` syscall. This matches the Linux kTLS pattern that the Cerebro OBSERVE §3.6 identified as the only production-deployed "TLS in the kernel" pattern in 2026.

**Why hybrid over pure-userspace or pure-kernel:**

- Pure userspace works fine for correctness but: (a) prevents future `sendfile`-style optimizations the AI subsystem may want for model-download paths, (b) gives the kernel no visibility into TLS-protected traffic which conflicts with Bible Article 11.3's "kernel concern at least to the extent that the inference subsystem may need authenticated model downloads."
- Pure kernel would mean kernel-resident TLS handshake — directly contradicts Bible Article 11.3 ("we do not roll our own crypto primitives") because handshake logic involves significant policy (certificate path validation, hostname matching, OCSP / CT log handling) we should not host in kernel space.
- Hybrid is the production answer and matches Article 11.3 exactly: kernel does framing and symmetric crypto (the parts where the rules are clear and the primitives are audited); userspace does the policy handshake.

**Deferred to BUILD-5** because BUILD-1 through BUILD-4 do not need TLS to validate their respective milestones. TLS is on the goal-milestone critical path but not on the BUILD-1 critical path.

### 3.4 DNS resolver placement (Cerebro §6.4)

**Choice: kernel-resident. Deferred to BUILD-4 (with Happy Eyeballs).**

A stub resolver lives in the kernel network service alongside the transport layer. It implements RFC 1035 query format, RFC 6891 EDNS(0) for larger responses, RFC 8106 RDNSS for getting nameservers from RAs. The resolver is reachable via `sys_net_resolve(name, family_hint) -> Result<&[NetCandidate]>` and is also called internally by the connection-establishment syscalls when a hostname is supplied.

**Why kernel-resident:** Happy Eyeballs (RFC 8305) requires tight coupling between DNS resolution and connection racing — the AAAA-vs-A query timing, the 50ms Resolution Delay, the Connection Attempt Delay staggering, the sort via RFC 6724 all happen between "I have a hostname" and "I have a connected socket." Pushing DNS into a userspace daemon means every connection involves at least one userspace round-trip per query before the connection logic can proceed. Kernel-resident keeps it in one place.

**Implementation note:** smoltcp ships a `DnsSocket` — the pattern is well-precedented in the ecosystem.

### 3.5 Network-driver trait surface (Cerebro §6.5)

**Choice: `NetDevice` trait parallels `BlockDevice`. Maximum reuse of Phase 3 substrate.**

The `NetDevice` trait in `kernel/src/drivers/net/mod.rs` mirrors `BlockDevice`'s shape: PCI enumeration via the existing infrastructure, MSI/MSI-X setup reusing the storage-side pattern, DMA-buffer-via-`Arc<DmaBuffer>` for RX/TX ring entries, interrupt routing through the established Phase 3 path. The negotiated-feature-bit discipline (virtio-net feature bits) at probe time follows the virtio-blk precedent.

BUILD-1 ships exactly one `NetDevice` implementation: `virtio_net`. This is the smallest validation surface and parallel to Phase 3 BUILD-1's virtio-blk-only landing.

**What this preempts:** Phase 4 BUILD-2+ can add more `NetDevice` impls (e1000e, USB Ethernet) without revisiting the trait surface. The trait itself does not change between BUILDs.

### 3.6 Namespacing / multi-tenancy (Cerebro §6.6)

**Choice: flat at Phase 4. Namespacing deferred to Phase 8+.**

Every process with a `NetworkAccess` capability sees the same routing table, the same interfaces, the same addresses. The capability handle controls access (can-create-sockets, can-bind-low-ports, can-modify-routing); it does not partition the network namespace itself.

**Why flat:** Linux network namespaces are operationally complex and intersect heavily with container infrastructure (cgroups, mount namespaces) that NeoPlutarch does not have. Capability-typed access (per Cerebro §6.6 third pole) is interesting but does not pay for itself at Phase 4 scope — the workloads that benefit from it (multi-tenant servers, sandbox processes) do not exist yet in the NeoPlutarch userspace.

**/simplify cut.** Surfaced in §7.

### 3.7 Loopback (Cerebro §6.7)

**Choice: short-circuit. Implemented at BUILD-1.**

When a packet is sent to `::1` or `127.0.0.1`, the kernel short-circuits at the IP layer: the packet is delivered to the destination socket's RX queue without going through the driver, MAC, or routing logic. The semantics observable from userspace are identical to traversing a real interface (same packet content arrives, same metadata except interface ID), but the path is internal.

**Why short-circuit over routed-loopback:**

- Cleaner: no virtual interface to manage, no MAC address to assign, no RA processing to fake out on the loopback.
- Faster: local IPC over UDP becomes a buffer copy plus a notification post, not a packet building + parsing round-trip.
- Simpler test path: BUILD-1 verification uses loopback for "process A sends UDP to process B" without needing a second machine.

The cost is that loopback is not a uniform code path with real interfaces, so a class of bugs that only manifest on real interfaces are not exposed by loopback testing. This is acceptable; bare-metal verification (BUILD-Nb pattern) covers that gap.

### 3.8 Routing table (Cerebro §6.8)

**Choice for BUILD-1: kernel-managed, SLAAC-derived, no manual modification API.**

The routing table is constructed automatically from Router Advertisements processed at SLAAC time. Link-local routes are derived from interface bringup. The loopback short-circuit (3.7) intercepts before the routing table. No syscall exists yet for userspace to add/remove routes; if one becomes necessary at a later BUILD, the routing-table-modification capability is a separate right on `NetworkAccess` (per Capsicum-style rights bitmap, per §3.1).

**/simplify cut for BUILD-1:** static routes, route metrics, policy routing. Surface only if a concrete BUILD needs them.

### 3.9 Address autoconfig (Cerebro §6.9)

**Choice for BUILD-1: SLAAC only, kernel-resident.**

BUILD-1's address-acquisition path: RA listening, prefix processing per RFC 4862, IID generation per RFC 8981 (Privacy Extensions, see 3.10), DAD per RFC 4862, route installation per the RA's PIO and RIO.

**Deferred BUILDs:**

- DHCPv4 in BUILD-2 (kernel-resident client following smoltcp's `Dhcpv4Socket` precedent).
- DHCPv6 in BUILD-3+ — only if a concrete deployment needs it. RFC 9915 is the spec target (per ADDENDUM); IA_TA is not implemented; multicast-only client transmissions.

**Why SLAAC-only at BUILD-1:** the test path is QEMU + a virtual network where SLAAC just works. Real-hardware verification (BUILD-1b) will be on a real LAN where SLAAC also just works (most home routers advertise RAs by default). DHCPv4 is critical for IPv4 deployment but the order-of-operations is "IPv6 first" per the goal posture, so DHCPv4 lands when IPv4 lands (BUILD-2).

### 3.10 IPv6 Privacy Extensions (Cerebro §6.10)

**Choice: enabled by default at BUILD-1 per RFC 8981.** Aligned with current BCP and modern OS practice.

Default lifetimes per RFC 8981: TEMP_VALID_LIFETIME 2 days; TEMP_PREFERRED_LIFETIME 1 day; TEMP_IDGEN_RETRIES 3; MAX_DESYNC_FACTOR 0.4 × preferred. User override capability deferred (no userspace control surface exists for it yet; can land in BUILD-4+).

### 3.11 mDNS / link-local discovery (Cerebro §6.11)

**Choice: deferred to Phase 8+.**

mDNS (RFC 6762) is userspace in every reference OS (Linux Avahi, macOS Bonjour, Fuchsia component). Phase 4 lands the underlying UDP multicast plumbing; a userspace mDNS responder is a separate Phase 8+ concern.

**/simplify cut.** Surfaced in §7.

### 3.12 TCP congestion control default (Cerebro §6.12)

**Choice: CUBIC only at BUILD-3 ship. Pluggable architecture deferred.**

When TCP lands in BUILD-3, the congestion control implementation is CUBIC (RFC 9438; smoltcp ships it). The CC module is structurally separable inside the TCP implementation but is not exposed as a userspace-configurable choice yet.

**Why CUBIC over the alternatives:**

- Reno / NewReno: ancient default, well-understood, but objectively underperforms CUBIC on the high-BDP paths common in 2026. Wrong default.
- BBRv1: shows good throughput but has known fairness issues with CUBIC traffic on shared links. Pre-mature for a default.
- BBRv3: experimental; not yet mainline Linux as of mid-2026. Forward marker, not default.
- Pluggable from BUILD-3 ship: extra complexity that no immediate workload needs.

CUBIC at BUILD-3, pluggable architecture if any concrete need surfaces in a later BUILD or Phase.

### 3.13 Resource limits and accountability (Cerebro §6.13)

**Choice for BUILD-1: per-process socket limit 64. Per-process counters for packets-in/packets-out/bytes-in/bytes-out, exposed via `sys_net_stats(socket_handle)`.**

The limit number (64) is a defensible default for a workstation OS — comfortably above any normal workload, low enough that a buggy process cannot exhaust kernel resources. Adjustable in later BUILDs if needed.

The per-process counters compose with Bible Article 22.3's existing inference-accountability precedent: the OS knows what each process did.

### 3.14 Async/sync I/O semantics (Cerebro §6.14)

**Choice: notification-on-activity. Composed with the existing capability handle.**

A process holding a `UdpSocket` or `TcpSocket` capability receives a kernel notification (using existing Phase 1-2 notification primitives) when the socket has activity: RX bytes available, connection state change, send-side capacity available. The process can either block waiting for the notification (`sys_net_wait(socket_handle)`) or poll its state (`sys_net_poll(socket_handle) -> SocketReady`).

**Why notification over alternatives:**

- Pure blocking (`recv` blocks until data): simple but expensive for processes with multiple sockets — N blocking threads per N sockets, doesn't scale.
- Edge-triggered readiness (epoll/kqueue): more state to manage in userspace, edge-vs-level confusion is a well-known source of bugs.
- Completion-based (io_uring, IOCP): the most performant choice for high-fan-out servers but adds substantial kernel-side ring infrastructure. Phase 4 BUILDs do not yet need it. Forward marker for Phase 8+ if performance demand justifies.
- Notification-on-activity (Hubris precedent): integrates with NeoPlutarch's existing notification primitives, scales to many sockets per process, simple userspace pattern. Right shape for Phase 4.

The kernel may add a completion-ring path in a later BUILD or Phase without disturbing this. The notification primitive is the BUILD-1 commitment.

---

## 4. Phase 4 Sub-BUILD Sequencing

Phase 4 ships in five sub-BUILDs plus bare-metal verifications, paralleling the Phase 3 BUILD-N / BUILD-Nb pattern. Each has an explicit goal milestone.

### BUILD-1 — IPv6 + UDP + virtio-net + capability shape

**Goal milestone:** A NeoPlutarch userspace process, holding a `NetworkAccess` capability, opens a `UdpSocket`, binds to a SLAAC-acquired IPv6 global address, sends a UDP datagram to a remote IPv6 host, receives the response. The reply path uses notification-on-activity to wake the process. Loopback short-circuit works for process-to-process UDP on `::1`. The Phase 3 substrate is green.

**Anti-scope (deferred to later BUILDs):** TCP, IPv4, DNS, Happy Eyeballs, TLS, DHCPv4/v6, e1000e, USB Ethernet, routing modification, mDNS, namespacing.

**Sub-commits:**

- **BUILD-1a — substrate.** `NetDevice` trait, virtio-net probe and bringup, RX/TX ring management. PCI + MSI-X + DMA-buffer infrastructure reused from Phase 3.
- **BUILD-1b — IPv6 stack core.** IP layer, ICMPv6, ND, SLAAC, Privacy Extensions per RFC 8981. smoltcp-derived or smoltcp-wrapped (decided at PLAN).
- **BUILD-1c — UDP + capability gate.** UDP send/receive path. `NetworkAccess` outer capability with `UdpSocket` inner capability. syscall surface: `sys_net_socket_udp`, `sys_net_send_udp`, `sys_net_recv_udp`, `sys_net_close`, `sys_net_wait`.
- **BUILD-1d — loopback short-circuit + accountability.** `::1` interception, per-process counters, `sys_net_stats`.
- **BUILD-1e — bare-metal verification.** Real hardware target (the OptiPlex 7080 Micro's Ethernet is an Intel I219-V; driver scope is identical to virtio-net at the trait level but the chip differs — see risk R3). Mirrors Phase 3 BUILD-5b in role.

### BUILD-2 — IPv4 + DHCPv4 + dual-stack address selection

**Goal milestone:** The same process can now also send UDP datagrams over IPv4 to remote IPv4 hosts. Dual-stack source-address selection follows RFC 6724. Address acquisition on IPv4 networks works via DHCPv4 (RFC 2131).

**Anti-scope (deferred):** TCP, DNS, Happy Eyeballs, TLS.

**Sub-commits roughly: IP v4 + ARP, DHCPv4, RFC 6724 selection, dual-stack test, bare-metal verification.

### BUILD-3 — TCP + congestion control + RFC 5961 mitigations

**Goal milestone:** The process can establish a TCP connection to a remote host on either IPv4 or IPv6, send and receive bidirectionally, close cleanly. CUBIC congestion control is in effect. RFC 5961 challenge-ACK mitigations are implemented. RFC 7323 window scaling negotiates.

**Anti-scope (deferred):** DNS, Happy Eyeballs, TLS.

### BUILD-4 — DNS + Happy Eyeballs

**Goal milestone:** The process calls `sys_net_dial(name)` with a hostname; the kernel performs Happy Eyeballs (RFC 8305) over the AAAA and A query results, returns a connected TCP socket. The API surface is HEv3-compatible at the candidate-list level even though only HEv2 logic is implemented.

**Anti-scope (deferred):** TLS, HEv3 SVCB/QUIC support.

### BUILD-5 — kTLS-hybrid TLS

**Goal milestone (= Phase 4 ship milestone, see §2):** The process opens a TCP connection via `sys_net_dial`, performs the TLS handshake in userspace (rustls in libneo), calls `sys_net_tcp_enable_tls` to hand record-layer ownership to the kernel, exchanges HTTPS traffic with a real server, verifies the cert against a kernel-managed trust anchor.

**Anti-scope (Phase 8+):** HEv3 protocol-priority (QUIC/SVCB), e1000e bare-metal, USB Ethernet, namespacing, mDNS, IPsec.

### Post-Phase-4 forward markers (Phase 5–8)

- e1000e driver (Bible Article 9.10 names it; BUILD-2b or BUILD-6 placement decided at PLAN).
- USB Ethernet drivers (CDC-ECM, CDC-NCM, RNDIS — gated by USB stack landing per ADR-017).
- HEv3 protocol-priority extension (QUIC racing, SVCB hints).
- Namespacing / per-process routing-view (Phase 8+).
- Pluggable congestion control (CC choice exposed to userspace).
- BBR family CCs.
- IPsec opt-in (Phase 8+, only if concrete need surfaces).
- NAT64/DNS64/464XLAT (Phase 8+, only if concrete need surfaces).
- mDNS responder (Phase 8+, userspace).

---

## 5. Risk Register

Premortem-style. Each risk has a mitigation. Risks are listed in rough order of "what would derail BUILD-1 specifically" first, then broader.

### R1. smoltcp's polling model doesn't fit NeoPlutarch's interrupt-driven kernel scheduler.

smoltcp assumes one task calls `iface.poll()` to drive the stack. NeoPlutarch wants packet processing driven by RX interrupts on one path and TX requests from userspace on the other.

**Mitigation.** At PLAN time, decide between (a) wrap smoltcp's `Interface` in a kernel-owned scheduling shim (a single kernel task that wakes on either RX-IRQ or TX-pending and calls `poll()`); (b) fork smoltcp and refactor the poll loop into event-driven entry points; (c) write the IP/UDP/ICMPv6 layer from scratch using smoltcp only as a reference. Option (a) is the cheapest BUILD-1 path; (b) and (c) are more work but yield a cleaner kernel-native architecture. Decision deferred to PLAN.

### R2. The `NetworkAccess` capability requires Phase 2 capability infrastructure that may not be ready in the shape we need.

The chosen layered-handle shape (§3.1) needs: capability-handle creation at spawn (handed by the parent or by init), capability-handle passing via the existing IPC capability-passing path, per-capability rights bitmap support. Phase 2's capability work landed (per Bible phase ledger), but the specific support for "give this handle to spawned process" may need verification.

**Mitigation.** **Prerequisite check before PLAN:** read the Phase 2 capability code (`kernel/src/cap/` or wherever it lives) and confirm it supports (a) handing a capability to a child at spawn, (b) passing a capability via IPC, (c) per-handle rights bitmaps. If any of these is missing, Phase 4 BUILD-1 must extend Phase 2 capability first; that extension is a sub-commit BUILD-1.0.

### R3. virtio-net on QEMU behaves differently from the Intel I219-V on the OptiPlex 7080 Micro.

The OptiPlex 7080 Micro's onboard Ethernet is an Intel I219-V. virtio-net's QEMU emulation is well-behaved and standards-compliant. The Intel I219-V is a real PCH-integrated NIC with its own quirks (Intel e1000e driver class — TX/RX descriptor rings, MSI-X with multiple vectors, manageability engine concerns, real DMA timing).

**Mitigation.** BUILD-1 ships virtio-net (QEMU-target only). BUILD-1e is bare-metal verification on virtio-net under VMware (the same VMware path Phase 3 BUILD-5a used) — confirms the virtio-net code path against real-ish hardware. **Real-real-hardware verification (Intel I219-V) requires the e1000e driver, which is forward-marker'd to BUILD-2b or BUILD-6.** Phase 4 ship milestone does not require I219-V; it requires "a NIC, on real hardware, doing the protocol stack work." VMware-under-real-hardware satisfies that.

### R4. ND/SLAAC behavior against PL's home router may surface unexpected quirks.

Some routers do not send unsolicited RAs and require explicit RS solicitation; some have buggy RA flag handling. The first time BUILD-1e runs against PL's home network is when we find out.

**Mitigation.** Implement RS solicitation explicitly at BUILD-1b ship. Log RA processing for diagnostic visibility (using the BUILD-5b block-device log sink dispatch artifact — which conveniently lands alongside or before Phase 4 BUILD-1 starts). If RA processing surfaces issues, file as a sub-commit fix.

### R5. Loopback short-circuit needs careful synchronization across the capability-handle ownership contract.

When process A sends a UDP datagram to `::1:1234` and process B is bound to that port, the kernel must locate B's `UdpSocket` capability, enqueue the packet onto B's RX queue, and post B's notification — all without violating the "only the capability holder can read this socket" invariant.

**Mitigation.** Loopback short-circuit at the IP layer constructs a packet (same wire format as if it traversed an interface) and then calls into the standard "deliver to bound socket" path. The "find bound socket" path already needs to verify capability ownership for the inbound-from-interface case; loopback uses the same machinery. No new ownership-bypass paths.

### R6. IPv6-only BUILD-1 has limited test coverage against IPv4-only servers.

A BUILD-1 process cannot connect to `*.gov.ipv4-only.example.com`. Most home networks are dual-stack but ICMP / NDP / IPv6-multicast can be filtered on poorly-configured CPE.

**Mitigation.** BUILD-1's success criteria reference a known-good IPv6 test target (Google's `ipv6.google.com`, Cloudflare's `ipv6.cloudflare.com`, or a self-hosted target on PL's network). If PL's home network is not IPv6-functional, this surfaces as a BUILD-1e blocker that pre-empts BUILD-1 ship until PL's network is fixed or the test target is moved to a known-good network. **PL pre-check needed:** confirm IPv6 functionality on PL's network (most modern ISPs provide it; PL likely has it already given Article-28 sensibility of his usual setups). Surfaced in §9 Q1.

### R7. smoltcp's API may not expose all the hooks we need for capability-handle interception.

smoltcp's socket API is BSD-shaped; we want to layer NeoPlutarch capability semantics on top. Some hooks (per-socket owner, per-socket capability check) may not exist in upstream smoltcp.

**Mitigation.** If R1 mitigation chooses option (a) (smoltcp wrap), the capability check happens at the syscall boundary, not inside smoltcp. smoltcp owns the protocol logic and the in-stack socket buffers; NeoPlutarch's capability layer owns the kernel↔userspace boundary. No upstream smoltcp changes required. If options (b) or (c) are chosen, the capability-handle hooks become part of the fork or the from-scratch implementation. Risk localized.

### R8. Bible v1.7.0 Article 5.6 (personality subsystems) may imply Linux-compat network namespace requirements.

ADR-023 and Bible Article 5.6 added the Linux personality concept. A Linux personality may eventually expose `socket(2)` and friends. This does not change Phase 4 native-API design but may affect API-shape decisions if the personality story needs to wrap.

**Mitigation.** The chosen layered-handle shape (§3.1) is compatible with a future Linux-personality wrapper: a personality library can implement `socket(2)` by calling `sys_net_socket_udp/tcp` and tracking the returned NeoPlutarch capability handle in a personality-private file-descriptor table. The personality emits BSD-shaped semantics over the NeoPlutarch-native primitives. No Phase 4 native-API change needed to accommodate this.

### R9. The "kernel-resident DNS resolver" choice (§3.4) may bloat the kernel substrate.

DNS is the kind of subsystem that grows: validating-resolver flags, DNSSEC, DoT, DoH, EDNS Client Subnet, …. Once it's in the kernel, every feature is kernel scope creep.

**Mitigation.** BUILD-4 DNS scope is bounded: stub resolver only (no recursion, no DNSSEC validation), RFC 1035 + EDNS(0) + RDNSS from RAs. Forward markers (DoT, DoH, DNSSEC validation) are explicitly Phase 5+ and gated on concrete need. If the DNS subsystem grows past stub-resolver scope, a Phase-4-late or Phase-5 design pass evaluates moving validation out into a userspace daemon with kernel as transport-only.

### R10. The Phase 4 BUILD numbering may collide with Phase 3 BUILD numbering if both are open simultaneously.

This THINK pass opens 2026-05-22 with Phase 3 close still pending. Phase 4 PLAN is gated on Phase 3 close. But if a confusion arises ("BUILD-1" means which phase's BUILD-1?), bookkeeping suffers.

**Mitigation.** Phase 4 BUILDs are always named `Phase-4-BUILD-N` (or `P4-BUILD-N` in shorthand) in artifact filenames and git commit messages. Never bare `BUILD-N`. PLAN-phase artifacts will land in `Plans/PLAN-2026-XX-XX-phase-4-build-1-*.md` and downstream artifacts mirror.

---

## 6. Prerequisites For Phase 4 BUILD-1 To Start

Each must be true before PLAN's plan-to-implementation transition.

**P1.** Phase 3 is formally closed via PM-DISPOSITION. (BUILD-5b execution and evidence is the gating step; the Jarvis log-sink dispatch is the unblocker.)

**P2.** Phase 2 capability infrastructure verified to support (per R2): capability handing to spawned process, capability passing via IPC, per-handle rights bitmaps. If any is missing, BUILD-1.0 lands the extension first.

**P3.** PL has verified IPv6 functionality on his network (per R6). Or alternative test target identified.

**P4.** The choice between smoltcp-wrap, smoltcp-fork, and from-scratch (per R1) is made at PLAN time. THINK does not pre-empt this.

**P5.** No active Phase 3 unresolved blockers (e.g., a BUILD-5b failure that requires kernel-code fix; that fix would land first before Phase 4 PLAN opens).

---

## 7. /simplify Pass

Per Bible Article 14, the /simplify gate before BUILD removes anything that can be cut without breaking the goal milestone. Phase 4 THINK's /simplify pass is run on the BUILD-1 scope and on the Phase-4-overall sequencing.

**Cut from BUILD-1:**

- IPv4. Doubles the protocol-layer surface; defers cleanly to BUILD-2.
- TCP. Doubles the transport surface; defers cleanly to BUILD-3.
- DNS. Coupled to Happy Eyeballs; defers to BUILD-4.
- Happy Eyeballs. Defers to BUILD-4 with DNS.
- TLS. Defers to BUILD-5.
- DHCPv4. Defers to BUILD-2.
- DHCPv6. Defers to BUILD-3+, only if concrete need.
- e1000e driver. Defers to BUILD-2b or BUILD-6.
- USB Ethernet. Defers to USB-stack-landing.
- Namespacing. Defers to Phase 8+.
- mDNS. Defers to Phase 8+.
- IPsec. Defers to Phase 8+, only if concrete need.
- Pluggable congestion control. Defers; CUBIC-only at BUILD-3.
- Completion-based I/O (io_uring-style). Defers; notification-on-activity at BUILD-1.

**Cut from Phase 4 entirely:**

- HEv3 protocol-priority full implementation (QUIC, SVCB). API surface accommodates it (§3.1); implementation defers to Phase 5+.
- BBR-family congestion controls. Forward markers.
- NAT64 / DNS64 / 464XLAT.
- User-configurable IPv6 Privacy Extensions override. Defaults-on, no API.
- Routing-modification API. Add when concrete need surfaces.
- Static route configuration. Add when concrete need surfaces.

**Kept after /simplify:**

- IPv6 + ICMPv6 + ND + SLAAC + Privacy Extensions (RFC 8981) at BUILD-1.
- UDP at BUILD-1.
- virtio-net at BUILD-1.
- Layered-handle capability shape at BUILD-1.
- Loopback short-circuit at BUILD-1.
- Per-process accountability counters at BUILD-1.
- Notification-on-activity I/O at BUILD-1.
- `NetDevice` trait paralleling `BlockDevice` at BUILD-1.

The /simplify cuts roughly halve the BUILD-1 scope from "the obvious thing to ship for a Phase 4" and double the number of sub-BUILDs over the phase. The trade-off is "smaller, more verifiable steps" against "fewer ship moments" — Bible Article 1.2 (correctness > coherence > performance) favors smaller verifiable steps.

---

## 8. What This THINK Does NOT Decide (Reserved for PLAN)

Items punted to PLAN-time decision because they are implementation-detail questions whose right answer depends on PLAN's deeper inspection of the kernel substrate.

- smoltcp wrap vs fork vs from-scratch (R1).
- Exact syscall numbers and the byte-level ABI for `sys_net_*`.
- DMA-buffer-pool sizing for virtio-net RX/TX rings.
- Whether `NetworkAccess` rights bitmap is `u32` or `u64`.
- Specific kernel-task scheduling priority for the network task.
- Test-harness scenario names (paralleling Phase 3's `phase3_ahci_*` / `phase3_nvme_*` naming).
- Whether IPv6 RDNSS option processing (RFC 8106) is required at BUILD-1 or BUILD-4 (DNS).

---

## 9. Open Questions For PL

Before PLAN opens, PL ratifications wanted on these substantive choices (THINK has made the call; PL can override):

- **Q1.** Phase 4 BUILD-1 scope is **IPv6 + UDP only** (no IPv4, no TCP). Acceptable, or do you want both IP families in BUILD-1?
- **Q2.** Sockets-equivalent API shape is **layered handle** (NetworkAccess outer, per-socket inner). Acceptable, or do you want a different shape (endpoint, channel, connection)?
- **Q3.** TLS is **hybrid kTLS-style at BUILD-5**. Acceptable, or pure-userspace TLS (no kernel involvement)?
- **Q4.** DNS resolver is **kernel-resident, BUILD-4**. Acceptable, or userspace daemon?
- **Q5.** TCP congestion control default is **CUBIC at BUILD-3 ship**, pluggable architecture deferred. Acceptable?
- **Q6.** Loopback is **short-circuit at IP layer**, not routed through the full stack. Acceptable?
- **Q7.** Async I/O is **notification-on-activity** (Hubris-style), not completion-based (io_uring-style). Acceptable?
- **Q8.** Sub-BUILD sequencing is **BUILD-1 (IPv6+UDP) → BUILD-2 (IPv4) → BUILD-3 (TCP) → BUILD-4 (DNS+HE) → BUILD-5 (TLS) → Phase 4 ship**. Acceptable, or do you want a different sequence?
- **Q9.** Real-real-hardware (Intel I219-V) verification is **deferred to BUILD-2b or BUILD-6** with e1000e driver landing. virtio-net-under-VMware is the bare-metal target for BUILD-1e. Acceptable?
- **Q10.** PL ratification gate: does PL want to ratify THINK now (open PLAN drafting), or wait for Phase 3 formal close before opening PLAN?

---

## 10. References

### Bible articles (v1.7.0)

- Article 1.2 (correctness > coherence > performance)
- Article 1.4 (parity is not a goal; rightness is)
- Article 3 (no POSIX; native ABI is our own)
- Article 4.1 (monolithic kernel with disciplined module boundaries)
- Article 9.10 (network driver phase placement: virtio-net + e1000e Phase 4; USB Ethernet bundled; RTL8169/ixgbe Phase 5+; WiFi Phase 7.5)
- Article 11 (Networking — full text)
- Article 12 (Security model — capability-based)
- Article 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY cadence; /simplify gate)
- Article 18.1 (informational artifacts)
- Article 28 (usable by real people test)

### RFCs (per OBSERVE refresh)

- RFC 768 (UDP, 1980) — base.
- RFC 791 (IPv4) / RFC 1122 (IPv4 host requirements) — background.
- RFC 1035 (DNS, 1987) — base for BUILD-4.
- RFC 2131 (DHCPv4, 1997) — BUILD-2.
- RFC 4443 (ICMPv6, 2006) — BUILD-1.
- RFC 4861 (IPv6 ND, 2007) — BUILD-1.
- RFC 4862 (SLAAC, 2007) — BUILD-1.
- RFC 5961 (TCP blind-injection mitigations, 2010) — BUILD-3.
- RFC 6724 (IPv6 Default Address Selection, 2012) — BUILD-2 (dual-stack).
- RFC 6891 (EDNS(0), 2013) — BUILD-4.
- RFC 7323 (TCP High-Performance Extensions, 2014) — BUILD-3.
- RFC 8085 (UDP Usage Guidelines, BCP 145, 2017) — BUILD-1.
- RFC 8106 (RDNSS, 2017) — BUILD-1 or BUILD-4.
- RFC 8200 (IPv6, STD 86, 2017) — BUILD-1.
- RFC 8305 (Happy Eyeballs v2, 2017) — BUILD-4.
- RFC 8504 (IPv6 Node Requirements, 2019) — substrate (supersedes RFC 6434).
- RFC 8981 (IPv6 Privacy Extensions, 2021) — BUILD-1.
- RFC 9293 (TCP, 2022) — BUILD-3.
- RFC 9438 (CUBIC, 2023) — BUILD-3.
- RFC 9673 (IPv6 Hop-by-Hop Options Processing, 2024) — BUILD-1 awareness.
- RFC 9915 (DHCPv6, 2026) — deferred BUILD; substrate awareness.
- `draft-ietf-happy-happyeyeballs-v3-03` (HEv3, March 2026) — Phase 5+ forward marker.

### OS / codebases (per OBSERVE)

- smoltcp — canonical Rust-native bare-metal candidate per Bible Article 11.2.
- Fuchsia Netstack3 — capability-shape precedent for the layered-handle choice (§3.1).
- Hubris — notification-on-activity precedent (§3.14).
- Linux kTLS — hybrid-TLS production reference (§3.3).
- Plan 9 — alternative-design pole, rejected for incompatibility with NeoPlutarch's capability model.
- Capsicum — per-handle rights bitmap precedent for future Phase 4+ socket-rights work.

### Cross-references in NeoPlutarch artifacts

- `docs/bible.md` v1.7.0
- `docs/ADR-017-*.md` (USB stack and USB Ethernet sequencing)
- `docs/ADR-022-fault-recovery-direction-forward-marker.md` (cell-swap forward marker for network-stack components)
- `docs/ADR-023-linux-personality-layer.md` (Linux personality; informs R8 mitigation)
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md`
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md`
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md`
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md`
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` (BlockDevice substrate that NetDevice mirrors)

---

**End of THINK artifact.**

*Phase 4 design direction chosen. 14 OBSERVE design questions addressed at BUILD-1 scope. Five-BUILD sequencing established (IPv6+UDP → IPv4 → TCP → DNS+HE → TLS). 10 risks enumerated with mitigations. 5 prerequisites identified. /simplify pass cut 14 items from BUILD-1; further deferrals listed. 10 PL ratifications surfaced. PLAN does not open until Phase 3 formally closes and PL ratifies §9.*
