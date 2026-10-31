# OBSERVE: Phase 4 Network Stack Survey (Pre-THINK Substrate)

| | |
|---|---|
| **Date** | 2026-05-20 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-20-phase-4-network-stack-observe-cerebro.md`) |
| **Type** | OBSERVE-class research artifact per Bible Article 14. Informational only per Bible Article 18.1. Not normative. No Bible/ANSPEC/ADR amendments produced. |
| **Authority** | None. PM consumes; the artifact joins Phase 4 THINK's authority chain when Phase 4 THINK formally opens (post Phase 3 closure per `PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md`). |
| **Voice anchor** | `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` (citation density, primary-source discipline) + `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` (landscape framing, posture-alignment grading) |
| **Critical-posture invariant** | Indicative mood for what exists; interrogative mood for what Phase 4 THINK must answer; never imperative mood for what NeoPlutarch should do. |

---

## 1. Substrate State at Start of Research

**Bible version:** v1.6.0. Article 11 (Networking), Article 12 (Security Model), Article 9.10 (network driver phase placement), Article 3 (What We Are Not — no POSIX), Article 14 (the OBSERVE → THINK → PLAN → BUILD/VERIFY algorithm), Article 18.1 (informational artifacts), Article 1.2 (correctness > coherence > performance), Article 1.4 (parity is not a goal).

**Substrate commits:** Phase 3 active. Tip of `main` at `eba73ac` ("build-5a-NVMe: add NVMe block-device driver for QEMU NVMe emulation"); the preceding closure points are `396585a` (build-5a-AHCI close evidence) and `3da6563` (build-5a-AHCI driver). Phase 3 closure gate is BUILD-5b bare-metal validation (pending PL hardware acquisition). Phase 4 THINK does NOT open in this artifact's lifetime — this artifact is preparation, not THINK opening.

**Active lanes (per dispatch §5):**
- **Jarvis:** BUILD-5a-NVMe (Phase 3 implementation lane).
- **Cerebro:** Phase 4 network OBSERVE (this artifact; parallel research lane).
- **Prometheus:** idle.

The two active lanes share zero substrate. No coordination required.

**Authority chain consumed:** the dispatch (FULL), `PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`, `PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md`, Bible Articles enumerated above. Prior Cerebro RESEARCH artifacts consulted for register/depth match: the AI-native-OS field survey (`RESEARCH-2026-05-10`), the storage-driver prior-art artifact (`RESEARCH-2026-05-12`), the NeoFS prior-art artifact (`RESEARCH-2026-05-11`).

---

## 2. TL;DR

The Phase 4 network-stack landscape in mid-2026 has **one canonical Rust-native bare-metal candidate** (smoltcp, used in production by Hubris, by Redox, by the seL4/LionsOS ecosystem as a firewall component, and by Fuchsia's Asterinas-tracking ecosystem discussions), **one architecturally-closer-but-platform-coupled production analogue** (Fuchsia Netstack3 — Rust, capability-shaped via FIDL-over-Zircon-channels, single-threaded by design, but tightly bound to the Fuchsia component model), and **a clear set of capability-shaping precedents** ranging from Tock's typed-capability discipline (`NetworkCapabilityCreationCapability` for IP/UDP layers) through Hubris's compile-time socket-owner binding through Plan 9's `/net` filesystem-namespace approach.

**The PM-NOTE proposed Phase 4 positions are confirmed at the substantive level with two citation refinements and one minor framing refinement that PM may wish to fold in before Phase 4 THINK opens:**

1. **RFC 6434 was obsoleted by RFC 8504 (Jan 2019).** PM-NOTE §2.1 cites RFC 6434 for the IPsec MUST→SHOULD downgrade. RFC 6434 did make that downgrade in 2011, and RFC 8504 retained the SHOULD posture in 2019. PM-NOTE's framing is historically correct but cites the now-obsolete document. The current authoritative document for IPv6 host requirements is RFC 8504, which the PM-NOTE substrate-reading list should reflect.
2. **RFC 8415 was obsoleted by RFC 9915 (Jan 2026, four months ago).** The dispatch §2.1.B cites RFC 8415 for DHCPv6. RFC 9915 supersedes it; the substantive functional difference for an endpoint OS is the **removal of IA_TA support** (DHCPv6 temporary addresses) and the removal of the Server Unicast option — both as features that "have not been widely implemented." Phase 4 THINK's DHCPv6 reading list should be RFC 9915.
3. **Happy Eyeballs has an active v3 IETF draft.** `draft-ietf-happy-happyeyeballs-v3` (rev -03 as of March 2026) is in the new HAPPY WG queue. The substantive extension over RFC 8305 is **SVCB / HTTPS DNS resource records and QUIC-before-TCP protocol ordering**. RFC 8305 remains the deployed normative reference; v3 is the forward-marker. Phase 4 THINK should be aware that any v3-ready API surface will need to accommodate protocol-order sorting (QUIC priority) and SVCB hints, not just A/AAAA address races.

**On TCP/IPv6 substrate:** RFC 9293 (Aug 2022) is the current consolidated TCP specification. It updates RFC 5961 and RFC 1122/1011 without obsoleting them and remains algorithm-agnostic on congestion control. RFC 8200 (current IPv6 spec) was updated by RFC 9673 (Oct 2024) on Hop-by-Hop options processing — a routers-and-hosts SHOULD-process clarification, not a host-stack-rewrite event. RFC 8981 (Feb 2021) is the current temporary-address (Privacy Extensions) specification, MUST-implement on a modern IPv6 host per RFC 8504.

**On the Rust-kernel networking-stack reference set:** smoltcp is the dominant candidate. It is `no_std`, allocation-free at steady state, supports IPv4 and IPv6 with native dual-stack, has recent SACK work in master, supports CUBIC and Reno as optional congestion-control modules, and is used in production by Hubris (UDP/ICMP/ND only, no TCP) and by Redox (full TCP via the `smolnetd` daemon and scheme files). Notably the Asterinas project is actively debating whether to migrate from smoltcp to Netstack3 (open GitHub issue), which is a useful calibration signal that the Phase 4 THINK choice between "smoltcp-derived" (Bible Article 11.2 preferred candidate) and "original" is a real choice the production-Rust-kernel ecosystem is also debating in 2026, not a settled question.

**On capability shape:** the design space has clear poles. At one end, Fuchsia's protocol-capability routing via component manifests gates `fuchsia.posix.socket.Provider` access **before** runtime — a component without the capability lacks the FIDL endpoint in its namespace and cannot even invoke socket creation. At the other end, Tock's `NetworkCapabilityCreationCapability` is a Rust-typed token consumed at IP/UDP layer instantiation — capability-as-type, not capability-as-handle. Hubris splits the difference: each socket has a compile-time-bound owner task identified in `app.toml`; the capability is structural rather than dynamic. Plan 9's `/net` filesystem namespace is a fourth pole: the capability is namespace presence — if `/net/tcp/clone` is bound into the process namespace, it can dial; if not, it cannot. Phase 4 THINK will need to choose where on this design space NeoPlutarch's sockets-equivalent sits, and the trade-off axis is dynamic-vs-static-capability granted at process-spawn vs runtime.

**On TLS placement (Bible Article 11.3):** Linux's kTLS pattern is the production-deployed hybrid: the handshake stays in userspace (rustls is the Rust-ecosystem default), but record-layer encryption/decryption moves into the kernel and can be hardware-offloaded to NICs. This is the only "TLS in the kernel" pattern that has demonstrated production-scale operational legitimacy as of 2026. Phase 4 THINK's "TLS as a kernel concern" position composes naturally with this pattern — kernel does framing/crypto-primitives, userspace does the policy handshake — and avoids the "kernel rolls its own TLS" failure mode Article 11.3 already rejects.

The actionable bullet remains: NeoPlutarch's Bible Article 11 posture is well-aligned with current OS practice and current IETF state. The PM-NOTE proposed positions are substantively confirmed. The corrections this artifact surfaces are document-citation refinements (RFC 8504 supersedes 6434; RFC 9915 supersedes 8415; Happy Eyeballs v3 is in flight) and design-space surfacing for Phase 4 THINK, not direction changes.

---

## 3. §2.1.A Existing Kernel Network Stack Survey

### 3.1 smoltcp

**Primary sources:** `https://github.com/smoltcp-rs/smoltcp` (verified at research time); README and Cargo.toml read; recent release notes consulted.

**Architectural summary.** smoltcp is a `no_std` event-driven TCP/IP stack designed for bare-metal and real-time embedded systems. It compiles on stable Rust 1.91+ and is allocation-free at steady-state operation — the design explicitly avoids heap allocation in the data path, with optional `std` and `alloc` features for convenience that are not required. The execution model is poll-based: applications call `iface.poll()` to drive packet processing, rather than the stack owning timers or threads or interrupt handlers. Sockets are typed objects (TCP, UDP, ICMP, raw, DNS, DHCPv4) with pre-allocated buffer pairs. Supported protocols include IPv4 and IPv6 with full dual-stack routing, ARP (IPv4) and NDISC / RFC 4861 (IPv6), TCP with window scaling and out-of-order reassembly (4-32 gaps configurable), DNS query support, and DHCPv4 client; SACK was added in recent versions but historically deliberately omitted. Optional congestion-control modules implement CUBIC and Reno. PLPMTU discovery, timestamping, and urgent-pointer support are deliberately omitted. The media layer abstracts Ethernet II, IEEE 802.15.4, and raw IP-layer interfaces.

**What NeoPlutarch could learn / borrow.** Bible Article 11.2 names smoltcp as the derivation candidate, and the technical fit is direct: smoltcp's poll-based event model maps cleanly onto a kernel that wants to drive packet processing from its own scheduler rather than letting the stack own kernel threads; the no-allocation discipline matches NeoPlutarch's Phase 3 substrate ethos (the BUILD-1 virtio-blk driver already follows the same pattern per `RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` §4.3); the typed-socket model composes naturally with capability-shaped handles. The IPv6-default routing is native, not retrofit. CUBIC is available as a default congestion-control choice that matches current deployed Linux practice.

**What would not transfer.** smoltcp's BSD-sockets-shaped API surface (open/recv/send/close on socket handles) is exactly the surface Bible Article 11.2 says NeoPlutarch will not adopt — so the stack's internal protocol logic transfers, but its public API does not. smoltcp's "applications poll" model assumes a single-task-per-stack design that Phase 4 THINK will need to revise for a multi-process kernel: NeoPlutarch needs a way for many processes to receive packets without each polling the stack themselves. The deliberately omitted features (PLPMTU, timestamps) will eventually need attention for any "production endpoint OS" claim — Phase 4 THINK does not need to ship them at first BUILD, but should note them as forward markers.

**Production reference points.** Hubris uses smoltcp for its service-processor UDP/ICMP/IPv6 stack (no TCP). Redox uses smoltcp for its full Internet-facing TCP/UDP/IP stack via the `smolnetd` daemon and the `tcp:/`, `udp:/`, `ip:/`, `icmp:/` schemes. seL4's LionsOS ecosystem has used smoltcp as the packet-parsing core of an optional firewall component. Asterinas currently uses smoltcp but has an active GitHub issue (asterinas/asterinas#1821) debating migration to Netstack3 — the calibration value of this debate is that the smoltcp-vs-Netstack3 choice is a live question in the production-Rust-kernel community in 2026, not settled.

### 3.2 Fuchsia Netstack3

**Primary sources:** `https://fuchsia.dev/fuchsia-src/contribute/contributing-to-netstack/netstack3` (Fuchsia documentation); `https://fuchsia.dev/fuchsia-src/contribute/roadmap/2021/netstack3` (roadmap); APNIC coverage; podcast interview with Bruno Dal Bo Silva on Netstack3 internals.

**Architectural summary.** Netstack3 is the Rust rewrite of Fuchsia's network stack, replacing the Go-based Netstack2 (which was derived from gVisor). It is now the default protocol stack for Fuchsia. The architecture splits into "core" (~90% of LOC, platform-agnostic, builds and tests on developer machines without Fuchsia) and "bindings" (the Fuchsia-specific glue, ~10% of LOC). Netstack3 is **deliberately single-threaded** at the core level — it uses internal locks but does not assume preemption or interruption, which the Fuchsia team identifies as something that may need adaptation if ported to a different runtime. Other Fuchsia components communicate with Netstack3 via FIDL protocols over Zircon channels: a component requests a socket by sending a single message naming the desired socket type and a Zircon channel that Netstack3 then listens on for socket-specific requests. The other end of the channel is held by the client, which uses it to drive recv/send/close operations on its socket. Netstack3 spawns a `fuchsia_async::Task` per socket to dispatch requests.

**What NeoPlutarch could learn.** The capability-shape pattern is the most directly transferable lesson: a process does not call `socket()`; it asks the network service for a socket and receives an opaque handle (Zircon channel ↔ Fuchsia; NeoPlutarch capability handle ↔ kernel object). The handle is the capability, and the network service is the gatekeeper. The component-manifest declaration of `fuchsia.posix.socket.Provider` makes "does this component have any network access at all" a static design-time decision rather than a runtime check — components without the capability route literally cannot reach the FIDL endpoint. Phase 4 THINK should consider whether the equivalent invariant at NeoPlutarch's process-spawn time is desirable (a process either has the network capability handed to it at spawn or has no way to acquire one later). The 90/10 core-vs-bindings split is also instructive: the protocol logic should be testable on a host machine without booting a kernel.

**What would not transfer.** Netstack3's Fuchsia-tight binding is the cost of its capability discipline: it is structurally entangled with the component model, FIDL code generation, and Zircon channel semantics in ways that cannot be lifted out without rewriting the bindings layer. The single-threaded core design assumes Fuchsia's runtime; a NeoPlutarch port would need to either match that posture (single-task network service receiving IPC from clients) or factor differently. The POSIX-API-shaped FIDL protocols (`fuchsia.posix.socket.Provider`, `fuchsia.posix.socket.Datagram`) explicitly preserve BSD-sockets semantics over the FIDL channel — Phase 4 THINK has the option to learn the routing pattern without inheriting the BSD-sockets surface, since NeoPlutarch is not POSIX-compatible per Article 3.

### 3.3 Hubris (Oxide Computer)

**Primary sources:** `https://github.com/oxidecomputer/hubris` (repository); `task/net/` directory documentation; Oxide RFD 250 ("Management Network Topology and Proprioception"); Hubris reference docs.

**Architectural summary.** Hubris is Oxide's message-passing memory-protected kernel for deeply embedded systems — primarily the service processor on Oxide's rack systems. Hubris is **not dynamically loaded**: all tasks (network, thermal, update, etc.) are compiled together into a single distribution image via `cargo xtask dist`, with the structure specified in `app.toml`. Tasks communicate via synchronous message-passing IPC. The network task (`task/net`) implements IP and UDP **with a focus on IPv6** (no TCP support), plus ICMP echo and neighbor discovery, built on top of smoltcp. Other tasks interact with the network task via two IPC operations: `send_packet` (lend a read-only buffer with destination address/port; the stack copies into its socket buffers) and `recv_packet` (lend a writable buffer; receive payload plus source address/port metadata). **Each socket has a compile-time-bound owner task defined in `app.toml`**; only the owner can send/receive on it. The network task posts notifications to wake the owner task when its socket has activity. All resources — buffers, sockets, DMA descriptors — are statically allocated based on compile-time configuration; no runtime allocation.

**What NeoPlutarch could learn.** The compile-time socket-owner binding is a very strong capability-discipline pattern at the cost of zero dynamism: tasks cannot create sockets at runtime, and a task that does not declare a socket in `app.toml` literally cannot send or receive packets. The capability is structural, embedded in the build. For NeoPlutarch's userspace-process model this is too rigid (processes are dynamic), but the **principle** is transferable: capability handles for sockets at process spawn, not socket creation by ambient authority. The compile-time-static no-allocation discipline composes with Bible Article 15.4. The `lend-buffer` IPC pattern (caller owns the buffer; service operates on it for the duration of the call) maps naturally onto NeoPlutarch's existing capability-handle model for shared memory.

**What would not transfer.** Hubris's no-runtime-task-creation invariant is the wrong shape for an OS that must support user-spawned processes — Phase 4 THINK needs a more dynamic socket-allocation path. The deliberate omission of TCP is a fine choice for a service-processor RTOS; for NeoPlutarch as an endpoint OS targeting Bible Article 28's "usable by real people" test, TCP is non-optional. The synchronous-IPC-only design is tighter than what NeoPlutarch's existing per-process asynchronous capability model wants.

### 3.4 seL4-derived systems: LionsOS, CAmkES, smoltcp-in-component

**Primary sources:** `https://lionsos.org/` (LionsOS site); seL4 docs CAmkES manual; seL4 Foundation whitepaper; capDL documentation; an MSc thesis ("Memory-safe Network Services Through A Userspace Switch") on seL4 + smoltcp; CAmkES manifest repository.

**Architectural summary.** seL4 is a formally verified microkernel: all kernel objects are accessed exclusively through capabilities, and the kernel itself has been proven (binary-level, on ARMv7 with verification extending to other targets) to be free of certain bug classes. CAmkES is the component framework on top of seL4 — a static component-architecture description that compiles down to a capDL (Capability Distribution Language) specification, which captures exactly which capability each component holds at boot. LionsOS is the Trustworthy Systems group's modular OS-construction kit on top of seL4, using statically-configured components connected by lock-free queues with model-checked signaling. Network stacks on seL4 typically run as their own components: LionsOS, CAmkES network examples, and the upstream seL4 community have used **lwIP** and **PicoIP** historically; more recent work (cited in the MSc thesis and in firewall components) uses **smoltcp** specifically for packet parsing and validation in security-critical paths. Components do not share memory by default; they share through capability-routed queues.

**What NeoPlutarch could learn.** The capDL pattern — at-boot static capability distribution from a manifest — is the strongest formal expression of "capability-based security from Phase 2" (Bible Article 12.1). A process's network capability is granted at spawn time and is unforgeable thereafter. The lock-free queue + model-checked signaling pattern is a clean abstraction for kernel-mediated IPC between a network service and its clients. The composability story (firewall as a component; routing as a component) maps to Phase 4 THINK questions about whether routing/loopback/dhcp/dns are kernel-resident or whether they decompose.

**What would not transfer.** seL4 is a microkernel with very different design assumptions from Bible Article 4.1's monolithic-with-disciplined-boundaries posture. The cost of seL4's capability discipline (formally-verified IPC for every cross-component operation) does not translate to a monolithic kernel — NeoPlutarch's "capability handles inside one address space" is cheaper but provides weaker isolation. The static-at-boot capability distribution is a strong invariant but, like Hubris's static socket binding, requires dynamism extensions for a multi-user OS.

### 3.5 Theseus OS

**Primary sources:** `https://www.theseus-os.com/`; Theseus OSDI 2020 paper (Boos et al.); GitHub repository `theseus-os/Theseus`. Direct documentation of the Theseus net subsystem was not surfaced in the available web sources.

**Architectural summary (network-relevant slice).** Theseus is the single-address-space intralingual OS from the Rice/Yale collaboration (OSDI 2020). Its distinguishing architectural commitment is **cell-swap fault recovery**: live components can be replaced in-place by swapping cells (linked code units) and migrating their state. ADR-022 already names this as a forward marker for Phase 4+ graceful-recovery scenarios. Theseus's network stack documentation is not extensively surfaced in the public materials; the repository structure (kernel/, libs/, applications/, tools/) does not advertise a dedicated `net/` subdirectory at the top level, and the project's primary engineering surface for networking appears more limited than its memory-management / cell-swap surface. The OSDI 2020 paper focuses on the fault-recovery mechanism and the intralingual design, not on a network stack.

**What NeoPlutarch could learn.** The forward marker for Phase 4+ is the cell-swap pattern itself — if Phase 4 BUILDs land a network stack whose components (routing table, congestion-control module, DHCP client) might benefit from in-place upgrade rather than reboot, ADR-022's revisit-trigger fires.

**What would not transfer.** Phase 4 cannot inherit Theseus's network stack itself — there is not a sufficient surfaced surface for this; whatever network code exists in Theseus is research-tier and not a production reference. The single-address-space model is incompatible with NeoPlutarch's multi-process design from Phase 1.5.

**Status note.** This survey did not find a clear Theseus net subsystem in publicly fetched content. If Phase 4 THINK has a specific Theseus mechanism in mind (cell-swap on a routing table, for example), targeted source-level inspection would resolve it; this OBSERVE artifact deliberately does not speculate.

### 3.6 Linux (comparison only — not a copy target per Bible Article 1.4)

Linux's network stack is the largest deployed reference codebase in this space. Architecturally relevant features for the comparison axis:

- **Socket API.** BSD sockets in their canonical form (`socket(2)`, `bind(2)`, `connect(2)`, `accept(2)`, `recv(2)`, `send(2)`, plus `setsockopt(2)`/`getsockopt(2)`). Bible Article 11.2 explicitly says NeoPlutarch will not adopt this.
- **Protocol implementation.** Full TCP/UDP/ICMP/IP across IPv4 and IPv6 with rich extensions: SACK, ECN, TFO, MPTCP, KCM, kTLS. Endpoint OS-grade.
- **Congestion control.** Pluggable. Default since 2.6.19 is CUBIC. BBRv1 has been available since kernel 4.9; BBRv2 and BBRv3 are not yet mainline as of mid-2026 (BBRv3 is "showing great results" per Phoronix coverage and has been earmarked for upstream, but is not the default). PRR, Reno, Vegas, Westwood, and many more are available.
- **kTLS.** Linux's kernel TLS feature lets the userspace handshake (rustls or OpenSSL) hand off the symmetric record-layer to the kernel after negotiation, enabling `sendfile(2)` over TLS connections and offload to TLS-capable NICs. This is the production pattern for "TLS in the kernel" in 2026.
- **Address autoconfig.** SLAAC (RFC 4862), DHCPv4 (RFC 2131), DHCPv6 (RFC 8415, now superseded by RFC 9915); RA processing for default routes; RFC 8981 Privacy Extensions enabled by default for global addresses on most distros.
- **Netfilter, eBPF.** The hookable in-kernel programmable surface that Bible Article 12.4 explicitly rejects for the Phase 1-3 era.

**What NeoPlutarch borrows.** The protocol-correctness reference (RFC compliance), the kTLS factoring as a TLS-placement option to consider, the congestion-control default (CUBIC) as a known-good starting point, the Privacy Extensions enabled-by-default policy. The implementation lives elsewhere — Linux's network stack is GPL and is in C and the goal is "rightness" per Article 1.4, not parity.

**What NeoPlutarch rejects.** BSD sockets (Article 11.2). The eBPF programmable in-kernel hook surface (Article 12.4). The ambient-authority model where any process can call `socket(2)` (Article 12 — capability discipline). The "many congestion controls compiled in" complexity for Phase 4 first cut (Phase 4 THINK may pick a single default).

---

## 4. §2.1.B RFC Survey with Current Status

This section maps each RFC the dispatch named to its current document status, what it normatively requires of an endpoint OS, and where Bible posture already composes. Primary sources fetched at research time from `https://www.rfc-editor.org/rfc/<id>` and `https://datatracker.ietf.org/doc/<id>/`.

### 4.1 RFC 6724 — Default Address Selection for IPv6 (Sept 2012)

**Status.** PROPOSED STANDARD; errata exist. Obsoletes RFC 3484. Current authority for IPv6 source-address and destination-address selection.

**Normative requirement on an endpoint OS.** IPv6 implementations MUST implement both source and destination address selection algorithms. The selection rules MUST NOT override application-supplied source addresses. SHOULD provide a configurable policy table.

**Algorithm shape.** Source selection applies 8 pair-wise comparison rules (same address as destination, appropriate scope, non-deprecated, home address, outgoing interface, advertised prefix, matching policy label, temporary-vs-public, longest matching prefix). Destination selection applies 10 rules across both IPv4 and IPv6 candidates (avoid unreachable, matching scope, non-deprecated source, home address, matching labels, higher precedence, native non-tunneled, smaller scope, longest prefix match, original order). Default policy table assigns native IPv6 precedence 40, IPv4-mapped precedence 35 — this is what makes "IPv6 preferred" deterministic across implementations.

**Bible composition.** Article 11.2 ("IPv6 as default") is satisfied normatively only if the stack implements RFC 6724 with the default policy table. PM-NOTE position (b) is well-supported.

### 4.2 RFC 8305 — Happy Eyeballs Version 2 (Dec 2017)

**Status.** PROPOSED STANDARD. Current normative reference for dual-stack connection establishment.

**Algorithm shape.** AAAA query first, then A; if A arrives before AAAA, wait the Resolution Delay (50ms default) before proceeding. Sort candidate addresses via RFC 6724. Interleave IPv6 and IPv4 attempts; stagger each attempt by Connection Attempt Delay (250ms default, 100ms minimum, 2s maximum, 10ms hard floor). First connection to succeed wins; all others are cancelled. Composes directly with RFC 6724 — that's the sort key.

**OS vs application division.** The specification expects **OS-level implementation** of the Happy Eyeballs engine: applications hand the stack a hostname and receive a connected socket; the stack manages the racing internally. This factoring matters for Phase 4 THINK because it puts DNS resolution (or at least DNS-result-driven connection racing) inside the network service's responsibility envelope rather than the application's.

**Forward marker — Happy Eyeballs v3.** `draft-ietf-happy-happyeyeballs-v3` (rev -03, March 2026, in the HAPPY WG) extends RFC 8305 with **SVCB / HTTPS DNS resource record handling** and **protocol-order sorting (QUIC before TCP)**. The SVCB query is issued before AAAA and A so its address hints and protocol information are available for the sort. The client SHOULD prioritize QUIC over TCP within an address family. APNIC Labs has analyzed implementation state of HEv2 across major OSes in October 2025; v3 is forward, not deployed. Phase 4 THINK should be aware that any sockets-equivalent API exposed in Phase 4 BUILDs will eventually need to accommodate a protocol-priority axis (TCP vs QUIC) and SVCB hints, not just A/AAAA addresses.

**Bible composition.** PM-NOTE position (b) is well-supported by RFC 8305. The v3 draft does not invalidate the position; it extends the connection-establishment algorithm without changing the address-selection foundation.

### 4.3 RFC 6434 → RFC 8504 — IPv6 Node Requirements

**Status.** RFC 6434 (Dec 2011, Informational) is **OBSOLETED by RFC 8504** (Jan 2019, Best Current Practice / Internet Standard track). PM-NOTE §2.1 cites RFC 6434 for the IPsec MUST→SHOULD downgrade — historically correct for the 2011 timeframe, but the current authoritative document is RFC 8504, which **retained the SHOULD posture** for IPsec rather than downgrading further.

**RFC 8504 normative posture for an IPv6 host (MUST/SHOULD/MAY summary):**

| Requirement | Level |
|---|---|
| SLAAC (RFC 4862) | MUST |
| DHCPv6 client (RFC 3315, now RFC 8415 / RFC 9915) | SHOULD |
| MLDv2 (RFC 3810) | MUST |
| Neighbor Discovery (RFC 4861) with Router/Prefix Discovery and NUD | MUST |
| Privacy Extensions (RFC 4941, now RFC 8981) | SHOULD, with user enable/disable |
| Path MTU Discovery (RFC 8201) | SHOULD |
| ICMPv6 (RFC 4443) | MUST |
| Default Address Selection (RFC 6724) | MUST |
| DNS stub resolver with AAAA, ip6.arpa, EDNS(0); RDNSS (RFC 8106) | RDNSS support MUST |
| IPsec (RFC 4301) | SHOULD (retained from RFC 6434) |

**Bible composition.** PM-NOTE position (c) — TLS-at-application-layer as the security primitive, IPsec deferred — is consistent with RFC 8504's SHOULD-not-MUST IPsec posture and with deployed-Internet 2026 reality. The PM-NOTE's substantive position is right; the suggested citation refinement is to cite RFC 8504 (current) rather than only RFC 6434 (obsolete).

**Working-group status.** The IETF 6man WG is the maintenance home for IPv6 host requirements. As of mid-2026, 6man's chartered scope is "maintenance, upkeep, and advancement" rather than major changes; this OBSERVE pass did not find an active draft superseding RFC 8504. Phase 4 THINK should treat RFC 8504 as the authoritative target.

### 4.4 RFC 9293 — TCP (Aug 2022)

**Status.** INTERNET STANDARDS TRACK. Consolidates and obsoletes RFC 793, 879, 2873, 6093, 6429, 6528, and 6691. **Updates** (does not obsolete) RFC 1011, RFC 1122, and RFC 5961. RFC 7323 (TCP timestamps and window scaling) is referenced as commonly used but remains optional and is neither mandated nor obsoleted.

**Endpoint-OS normative summary.** MUST-numbered requirements include unsigned-number window handling (MUST-1), checksum generation and verification (MUST-2/3), End-of-Option / NOP / MSS support (MUST-4), receive any option (MUST-5), ignore unimplemented options (MUST-6), reset and log illegal option lengths (MUST-7), clock-driven ISN selection (MUST-8), simultaneous-open support (MUST-10), track passive-vs-active SYN-RECEIVED (MUST-11), per-connection Nagle enable/disable (MUST-17), RFC 6298 RTO computation (MUST-18). Congestion control is mandatory but **algorithm-agnostic** — the spec does not prescribe CUBIC or Reno; it requires the implementation support some congestion-control mechanism. PMTUD and PLPMTUD recommended.

**Bible composition.** Phase 4 THINK will need to choose a default congestion-control algorithm. The smoltcp ecosystem ships CUBIC and Reno; Linux's default is CUBIC; BBRv3 is forward-marker not deployed-default as of mid-2026. RFC 9293 does not constrain this choice; Phase 4 THINK is free to choose, including "single algorithm at first BUILD" rather than pluggable.

### 4.5 RFC 8200 — Internet Protocol, Version 6 Specification (July 2017)

**Status.** INTERNET STANDARD (STD 86). Obsoletes RFC 2460. **Updated by RFC 9673** (Oct 2024) on Hop-by-Hop options processing.

**Endpoint-OS normative summary.** 128-bit addresses; traffic class and flow label handling; extension headers (Hop-by-Hop, Routing, Fragment, Destination Options); fragmentation by source nodes only with reassembly at destination; minimum link MTU 1280 octets; upper-layer-checksum recalculation accounting for 128-bit pseudo-header. Stricter fragment-overlap and header-placement rules than IPv4 era.

**RFC 9673 update.** Loosens mandatory Hop-by-Hop processing on routers and hosts: hosts SHOULD process Hop-by-Hop in received packets but MAY skip if constrained; in either case the rest of the packet is processed normally. Effect for Phase 4 THINK: no host-stack-rewrite, but the Hop-by-Hop handling code path is permitted to fast-path-skip in resource-constrained contexts.

**Bible composition.** Article 11.2's "IPv6 as default" assumes RFC 8200 compliance. PM-NOTE position (a) "dual-stack mandatory" is satisfied by implementing both RFC 8200 (IPv6) and RFC 791/RFC 1122 (IPv4) endpoints, with the deterministic-preference layer (RFC 6724) selecting between them.

### 4.6 RFC 8504 — IPv6 Node Requirements (Jan 2019)

Treated in 4.3 above (succeeds RFC 6434).

### 4.7 RFC 8085 — UDP Usage Guidelines (March 2017)

**Status.** BEST CURRENT PRACTICE (BCP 145). Obsoletes RFC 5405. The normative guidance document for UDP application behavior; provides reference for OS stack UDP support.

**Endpoint-OS normative summary.** Applications using UDP bear primary responsibility for congestion control — the OS stack does not provide this for UDP. Bulk transfers SHOULD implement TFRC or TCP-friendly windowing; low-volume apps SHOULD limit to ~1 datagram per RTT. Avoid IP fragmentation; implement PMTUD or use 576 (v4) / 1280 (v6) conservative defaults. **IPv4 checksums recommended but optional; IPv6 checksums mandatory by default**, with zero-checksum mode permitted only under RFC 6935/6936 strict conditions. Multicast UDP applications have additional guidelines.

**Bible composition.** The "IPv6 checksums mandatory" requirement is a direct stack-implementation constraint Phase 4 THINK should bake in. The "applications own UDP congestion control" division is a cleaner API contract than TCP's "stack owns CC" — this is a useful design contrast point.

### 4.8 RFC 5961 — TCP Robustness Against Blind In-Window Attacks (Aug 2010)

**Status.** PROPOSED STANDARD. **Updated by RFC 9293** (not obsoleted — remains normatively relevant alongside RFC 9293). The challenge-ACK mechanism is the key contribution.

**Endpoint-OS normative summary.** RST acceptance SHOULD require exact SEG.SEQ == RCV.NXT match; in-window non-matching RSTs trigger a challenge ACK before discard. All in-state SYN segments SHOULD elicit a challenge ACK regardless of sequence number. Data-injection ACK validation MAY check (SND.UNA - MAX.SND.WND) ≤ SEG.ACK ≤ SND.NXT. Challenge ACK rate SHOULD be throttled (suggested 10 per 5s window, configurable).

**Bible composition.** Article 1.2 (correctness > coherence > performance) favors implementing RFC 5961 mitigations from Phase 4 BUILD-1 rather than retrofit. The throttling rate is a tuning parameter Phase 4 THINK can pick at PLAN time.

### 4.9 RFC 7323 — TCP Extensions for High Performance (Sept 2014)

**Status.** PROPOSED STANDARD. Obsoletes RFC 1323.

**Endpoint-OS optional extensions.** Window Scale (WS) — TCP windows beyond 16-bit limit, scale factor up to 2^14 enabling 1 GiB windows. Timestamps (TS) for RTTM and PAWS. Once negotiated in SYN/SYN-ACK, timestamps MUST appear in every non-RST segment. PAWS processing MUST take precedence over the standard acceptability check.

**Deployment caveat.** Some middleboxes strip TCP options; the spec acknowledges this practical hazard. The deployed Internet generally supports WS and TS; modern stacks negotiate them by default.

**Bible composition.** Phase 4 THINK choice: ship RFC 7323 from BUILD-1 (matches modern endpoint expectations and large-BDP-path performance), or defer to a later BUILD (smaller initial code surface). smoltcp supports window scaling; the implementation cost is moderate.

### 4.10 RFC 4861 — IPv6 Neighbor Discovery (Sept 2007)

**Status.** INTERNET STANDARD (with updates by RFCs 5942, 6980, 7048, 7527, 7559, 8028, 8319, 8425, 9131, 9685, 9762, 9926). Obsoletes RFC 2461. MUST-implement per RFC 8504.

**Endpoint-OS normative summary.** Five ICMPv6 message types: Router Solicitation, Router Advertisement, Neighbor Solicitation, Neighbor Advertisement, Redirect. Required functions: address resolution for on-link destinations, router discovery and default-router selection, Neighbor Unreachability Detection, Duplicate Address Detection (in conjunction with SLAAC per RFC 4862), Redirect message handling.

**Bible composition.** Phase 4 THINK will need this; smoltcp implements it. No design tension with current Bible posture.

### 4.11 RFC 4862 — IPv6 Stateless Address Autoconfiguration (Sept 2007)

**Status.** DRAFT STANDARD. **Updated by RFC 7527 and RFC 9762.** Obsoletes RFC 2462. MUST-implement per RFC 8504.

**Endpoint-OS normative summary.** Link-local address formation via fe80::/10 + interface identifier (no router required). DAD MUST be performed on all unicast addresses (link-local and global) via Neighbor Solicitation. Global address formation via Router Advertisement prefix + interface identifier. Multiple addresses can be assigned simultaneously to support graceful renumbering. Preferred and deprecated address states.

**Bible composition.** Phase 4 THINK question — surfaced for §6: should SLAAC processing live in the kernel network service or in a userspace service? Linux puts the kernel in charge of address assignment but routes RA processing through a userspace daemon (NetworkManager / systemd-networkd) for higher-level policy; Fuchsia's Netstack3 handles SLAAC in-process. Hubris is too constrained to be a precedent (compile-time addressing). Surfacing only; not choosing.

### 4.12 RFC 2131 — DHCP for IPv4 (March 1997)

**Status.** DRAFT STANDARD. Updated by RFCs 3396, 4361, 5494, 6842. Obsoleted RFC 1541.

**Endpoint-OS normative summary.** Client state machine: INIT, SELECTING, REQUESTING, BOUND, RENEWING, REBINDING, INIT-REBOOT. Four core message types (DISCOVER, OFFER, REQUEST, ACK) plus NAK, DECLINE, RELEASE, INFORM. Randomized exponential backoff for retransmissions. ARP conflict detection before address use. Consistent client identifier across all messages. Lease expiration / renewal timer compliance.

**Bible composition.** Phase 4 THINK design-space question: kernel-resident DHCPv4 client, or userspace daemon over a raw-socket capability? smoltcp's `Dhcpv4Socket` puts the client in-stack; Linux historically separates dhcpcd / systemd-networkd into userspace. Surfacing only.

### 4.13 RFC 8415 → RFC 9915 — DHCPv6

**Status.** RFC 8415 (Nov 2018) is **OBSOLETED by RFC 9915** (Jan 2026). This is recent — four months before Phase 4 OBSERVE is conducted — and the dispatch §2.1.B cites RFC 8415 explicitly. RFC 9915 is the current normative reference.

**What RFC 9915 changes vs RFC 8415.** (i) **IA_TA support is removed** — DHCPv6 temporary addresses are no longer specified. Privacy-Extensions temporary addresses (RFC 8981) remain the answer for IPv6 privacy. (ii) **Server Unicast option is removed** — clients always send to multicast ff02::1:2 and listen on UDP port 546. (iii) Verified errata from RFC 8415 era are incorporated. (iv) DNS domain-name encoding explicitly forbids RFC 1035 compression. (v) UDP port-usage rules are clarified for clients, servers, and relay agents.

**Endpoint-OS normative summary (RFC 9915).** Stateful (IA_NA, IA_PD) and stateless modes. IA_TA support removed. Solicit/Advertise/Request/Reply/Renew/Rebind/Release/Decline message types. Multicast-only client transmissions. Rate limiting per §14.1. T1/T2 timer handling.

**Bible composition.** Phase 4 THINK's DHCPv6 reading list should be RFC 9915, not RFC 8415. PM-NOTE substrate-reading list should be updated to match.

### 4.14 RFC 8981 — IPv6 Temporary Addresses (Privacy Extensions) (Feb 2021)

**Status.** INTERNET STANDARDS TRACK. Obsoletes RFC 4941 (which obsoleted RFC 3041). Current normative reference for IPv6 privacy.

**Endpoint-OS normative summary.** MUST generate randomized temporary IIDs for each SLAAC prefix; two algorithms specified (simple PRNG randomization with reserved-IID and existing-address checks, or PRF-based `RID = F(Prefix, Net_Iface, Network_ID, Time, DAD_Counter, secret_key)` using BLAKE3 or HMAC-SHA-256). MUST perform DAD. MUST regenerate before deprecation. MUST remove temporary addresses on link change. MUST provide user enable/disable. Default lifetimes: TEMP_VALID_LIFETIME 2 days; TEMP_PREFERRED_LIFETIME 1 day; TEMP_IDGEN_RETRIES 3; MAX_DESYNC_FACTOR 0.4 × preferred.

**Bible composition.** Phase 4 THINK question — surfaced for §6: enable Privacy Extensions by default? Linux (most distros) yes. macOS yes. Windows yes. The privacy-vs-debuggability tension is well-trodden; the BCP-aligned answer is "default on, with user override." Surfacing only.

---

## 5. §2.1.C Capability-Shaped Socket API Design Space

This section enumerates the design-space poles. Phase 4 THINK will need to choose a position; this OBSERVE pass maps the territory.

### 5.1 Fuchsia fdio + FIDL over Zircon channels

**The shape.** A process wanting a socket sends a single FIDL message on the `fuchsia.posix.socket.Provider` channel naming the desired socket type (datagram, stream, raw) and a Zircon channel endpoint that the network stack will accept ownership of. The network stack (Netstack3) becomes the server end of the socket-specific channel; the process holds the client end and uses it for recv/send/close operations. **The capability is the Zircon channel.** Process A cannot forge it; process B cannot peek into it. The kernel-mediated channel is the unforgeable token.

**Initial capability acquisition.** At component-spawn time, the component manifest (`.cml` file) declares `use: [{ protocol: "fuchsia.posix.socket.Provider" }]`. Capability routing through the parent component validates that a route to this protocol exists; if not, the manifest fails to instantiate and the component never starts. **A component without the capability route literally lacks the FIDL endpoint in its namespace** — there is no `socket()` call to make, and no runtime check is needed because the capability is structural.

**Syscall surface shape.** Zero new syscalls — FIDL channels reuse Zircon's existing channel primitives (`zx_channel_create`, `zx_channel_write`, `zx_channel_read`). The "socket API" is generated FIDL bindings; for C-compatible programs `fdio` exposes a POSIX-shaped surface on top of the channels, but the FIDL channel is the underlying capability.

**What this protects.** Process isolation (one process cannot interfere with another's socket because the channels are unforgeable). Per-protocol capability granularity (a component with `fuchsia.posix.socket.Provider` access can create new sockets; one without cannot). Network reach is structural at the component-tree level.

### 5.2 Hubris task IPC with compile-time socket binding

**The shape.** Each socket has a compile-time-bound owner task identified in `app.toml`. The owner is the only task that can send/receive on the socket. Other tasks cannot reference the socket at all. The IPC operations `send_packet` and `recv_packet` are synchronous lend-buffer messages to the network task.

**Initial capability acquisition.** At `cargo xtask dist` time. Static. No runtime allocation. A task without a socket declaration cannot acquire one later.

**Syscall surface shape.** Hubris IPC primitives (`sys_send`, `sys_recv`) with task-and-operation-coded message envelopes. The capability is task identity verified by the Hubris kernel during message dispatch.

**What this protects.** Same isolation guarantees as Fuchsia, but more rigid (no dynamic sockets). Cost of rigidity: zero runtime allocation, zero ambient authority, full statically-verifiable IPC topology. Suits a service-processor RTOS. Wrong shape for a multi-user OS without significant adaptation.

### 5.3 Capsicum (FreeBSD)

**The shape.** Capsicum capability mode (`cap_enter(2)`) puts a process into a sandbox where access to global OS namespaces (filesystem path namespace, PID namespace) is restricted. Capabilities are file descriptors with restricted rights bitmaps (`cap_rights_limit(2)`). The FreeBSD manual page on `capsicum(4)` documents the framework's general posture but the surfaced sources do not enumerate the specific socket-related restrictions in detail — the consensus in surrounding documentation is that `socket(2)` itself can be restricted in cap mode, and operations on existing sockets are gated by per-fd capability rights.

**Initial capability acquisition.** A process opens resources (files, sockets) **before** entering capability mode, then `cap_enter()`. After entering, the process has only the file descriptors it brought with it (each with its restricted rights bitmap). New ambient operations are denied. Sockets passed via UNIX-domain SCM_RIGHTS can extend a process's capability set even after `cap_enter()`.

**Syscall surface shape.** Mostly the existing BSD-sockets surface, but every syscall is gated against the capability rights bitmap of the fd it operates on. New syscall `cap_rights_limit` and `cap_enter`.

**What this protects.** Lateral movement: a compromised process in cap mode cannot open new network sockets or escape to the filesystem namespace. Bible Article 11.2's "no BSD sockets" position rules out direct adoption; the **interesting transferable pattern** is per-handle rights bitmaps — a NeoPlutarch capability handle could carry per-operation rights (send-only, recv-only, no-bind, etc.) as a natural extension to the existing capability model.

### 5.4 seL4 IPC-as-syscall

**The shape.** In seL4 every cross-component operation is an IPC call gated by an endpoint capability. A process holds capabilities to endpoints; to invoke a service, it sends an IPC message via a capability invocation. Network services in CAmkES-built seL4 systems run as their own components with endpoint capabilities routed at boot via capDL.

**Initial capability acquisition.** capDL at boot time. The entire capability distribution is fixed at boot; new components cannot acquire capabilities except by being granted them via existing capability invocation paths.

**Syscall surface shape.** A single `seL4_Call` / `seL4_Send` / `seL4_Recv` family of IPC primitives. Everything else is built on top.

**What this protects.** Formally-verified IPC semantics. The capability cannot be forged, leaked except by explicit grant, or escaped by the kernel itself (proven). The cost is microkernel IPC overhead and a static-at-boot capability distribution that requires careful manifest design.

### 5.5 Plan 9 /net filesystem namespace

**The shape.** No special socket syscalls. Network devices, interfaces, protocols, and services are all files under `/net`. A process dials a remote service by writing to `/net/cs` (connection server) a symbolic address like `tcp!plan9.bell-labs.com!http`; cs returns a clone path; writing to the clone path connects; reading the corresponding data file does I/O. Listening is symmetric: open `/net/tcp/clone`, write `announce 80`, accept incoming connections by reading from the resulting connection directory.

**Initial capability acquisition.** A process's namespace mounts determine what is reachable. If `/net` is bound into the namespace, the process has network access; if a more restricted `/net` (e.g. with only `/net/cs` and a single connection directory bound) is present, the process has constrained network access. Per-Plan-9 ethos: capability is namespace presence.

**Syscall surface shape.** Zero new syscalls. `open`, `read`, `write`, `close` are sufficient. The protocol-specific operations are encoded as text written to control files (the "9P" protocol convention).

**What this protects.** Composability with the rest of the system (everything-is-a-file); ease of network-namespace virtualization (just bind a different `/net` tree); zero new syscall surface to verify. Bible Article 11.2's "not BSD sockets" leaves Plan 9 as a fair design pole to consider, although Bible Article 3 (no POSIX) and Article 4.1 (monolithic kernel) suggest NeoPlutarch is unlikely to adopt the full 9P-over-mounts approach.

### 5.6 Tock typed capabilities (Rust-native bonus reference)

Not in the dispatch's named list but surfaced during research as the most directly Rust-shaped capability model: Tock declares typed capability tokens (`NetworkCapabilityCreationCapability`) that are required to instantiate IP and UDP layer objects. The capability is a phantom type held by a kernel module; modules without the capability cannot construct the protected object. **The capability is in the type system**, not in a handle table.

**What this protects.** Compile-time guarantees that only blessed kernel modules can construct network-layer instances. For NeoPlutarch's internal kernel structure (Article 4.1 monolithic with disciplined module boundaries), this pattern composes with Rust's type system at zero runtime cost. The transferable principle: capability-as-type for kernel-internal capability discipline, capability-as-handle for the kernel↔userspace boundary.

---

## 6. §2.1.D Design Questions Phase 4 THINK Will Need to Resolve

The following enumeration is non-exhaustive. Each question surfaces the design space; this artifact does not choose.

### 6.1 Sockets-equivalent API shape

Phase 4 THINK will need to decide what a process's "network handle" actually points at. The poles are:
- **Endpoint-shaped** (Fuchsia, Hubris, Linux): the handle represents one side of a logical connection or listening point.
- **Channel-shaped** (Plan 9 connection-directory model): the handle is a transport for control + data over a structured namespace.
- **Connection-shaped**: the handle is a specific connection, with the listen object separate.
- **Layered**: an outer "network capability" handle from which inner per-socket handles can be created (Fuchsia's `fuchsia.posix.socket.Provider` is this shape).

Trade-off axis: the dynamic-vs-static-capability granted at process-spawn vs runtime, and whether the API surface is one syscall family (`net_open`/`net_close`/`net_io`) or many specialized syscalls.

### 6.2 In-kernel TCP/UDP/ICMP scope

Phase 4 THINK will need to decide what's in the kernel network service at BUILD-1 vs deferred:
- Hubris precedent: UDP + ICMPv6 ND only, no TCP — RTOS-appropriate.
- Redox / Asterinas / Fuchsia precedent: full TCP + UDP + ICMP from the start.
- A minimal-first approach (UDP first, TCP second) buys early validation of the capability shape with less RFC-9293 surface area; a full-stack-first approach prevents API-shape surprises when TCP is added.

Trade-off axis: BUILD-1 surface area vs API-shape risk of incremental retrofit.

### 6.3 TLS scope

Phase 4 THINK will need to decide where TLS lives:
- **Pure userspace** (rustls / libneo): kernel is uninvolved; TLS connections look like ordinary TCP to the kernel.
- **Hybrid kTLS-style**: userspace handles the handshake, kernel takes the record layer after negotiation (Linux pattern; rustls/ktls crate available).
- **Kernel-resident**: the kernel knows what a TLS connection is; rustls (or a minimal subset) runs in kernel space.

Bible Article 11.3 explicitly says TLS is a kernel concern at least to the extent that the inference subsystem needs authenticated model downloads. The hybrid kTLS pattern is the production-deployed answer in 2026 and avoids the Article-11.3-forbidden "roll our own primitives" failure mode while giving the kernel framing-and-symmetric-crypto custody.

Trade-off axis: how much TLS state the kernel owns vs how much application policy reaches into kernel space; correctness boundary (Article 1.2).

### 6.4 DNS resolver placement

- **Kernel-resident** (smoltcp's `DnsSocket`, Plan 9's `/net/cs`): DNS resolution is a network service responsibility.
- **Userspace daemon over a UDP-socket capability** (Linux, BSD): the OS provides UDP; userspace provides resolver state.

Happy Eyeballs (RFC 8305) and especially HEv3 (draft) push DNS racing into close coupling with the connection logic. If Phase 4 supports HE natively, that coupling argues for kernel-resident DNS or for a tightly-coupled kernel↔resolver interface. Whether the resolver is "in" the kernel or "owned by" the network service is a Phase 4 THINK design question.

### 6.5 Hardware driver layer composition with the Phase 3 BlockDevice substrate

Bible Article 9.10 names virtio-net and e1000e as Phase 4 first-class drivers; the Phase 3 substrate already shipped a virtio-blk and AHCI driver pattern. Phase 4 THINK will need to decide:
- Does the network driver trait surface (`NetDevice`) parallel `BlockDevice` in structure? The PCI enumeration, MSI/MSI-X setup, DMA-buffer-via-`Arc<DmaBuffer>` pattern, and interrupt-routing precedent are directly reusable per `RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`.
- Does virtio-net's negotiated-feature-bit discipline (assert F_FLUSH-equivalent invariants at probe time) compose with virtio-blk's already-shipped pattern?
- USB Ethernet (Article 9.10 names CDC-ECM, CDC-NCM, RNDIS as Phase 4 deliverables) is bundled with the USB-stack landing — the USB-stack-landing prerequisite is a Phase 4 sub-ordering question.

Trade-off axis: code reuse vs subsystem boundary cleanliness.

### 6.6 Namespacing / multi-tenancy / per-process network capability scoping

Phase 4 THINK will need to decide what a process's "view" of the network is:
- **Flat**: every process sees the same routing table, interfaces, addresses. Capability gates create/use of sockets, not visibility.
- **Namespaced** (Linux network namespaces, Fuchsia per-component routing): different processes can see different routing/interfaces/addresses. Capability gates which namespace the process inherits.
- **Capability-typed access**: a process can only send/receive on the specific destination addresses or ports its capability permits.

The capability-typed-access pole is unusual in production deployments but composes naturally with Bible Article 12.1's "every kernel object is referenced by a capability handle" — a socket capability with attached rights bitmap (cf. Capsicum) or destination-mask is a legitimate design pole.

### 6.7 Loopback

Phase 4 THINK will need to decide loopback semantics:
- **Standard 127.0.0.1 + ::1 routing-table loopback**: same code path as a real interface, just routed to a virtual device.
- **Short-circuit loopback**: kernel-internal IPC bypasses the stack entirely for `::1` traffic, exposing the same socket surface but with no protocol overhead.

Trade-off axis: code-path uniformity vs performance for local IPC.

### 6.8 Routing table representation and update model

- **Per-process or system-global routing table**: composability with §6.6 namespacing.
- **Static (boot-time) or dynamic (RA-driven, DHCP-driven, manually-managed)** updates.
- **Capability for routing-table modification**: who can add a default route? Only a process holding a specific network-admin capability.

### 6.9 Address autoconfiguration: DHCPv4, SLAAC, DHCPv6

Three independent decisions:
- **DHCPv4**: kernel-resident client (smoltcp `Dhcpv4Socket` pattern) vs userspace daemon over a raw-socket capability.
- **SLAAC**: kernel-resident processing of Router Advertisements (Fuchsia / smoltcp pattern) vs userspace daemon receiving RAs and writing kernel addresses.
- **DHCPv6**: per RFC 9915 (note: IA_TA support removed; ff02::1:2 multicast only). Kernel-resident or userspace?

The Bible has no current commitment here. SLAAC alone is sufficient for many home/office IPv6 deployments; DHCPv6 is needed for enterprise and ISP-managed networks. Phase 4 THINK can pick a minimum (SLAAC-only first BUILD; DHCPv6 later) or a comprehensive baseline.

### 6.10 IPv6 Privacy Extensions (RFC 8981)

Enable by default per current BCP and modern OS practice? Bible has no commitment; the privacy-vs-debuggability axis is well-trodden; the BCP-aligned default is "on, with user override."

### 6.11 mDNS / link-local discovery

Phase 4 or deferred? mDNS (RFC 6762) is widely deployed for local service discovery (Apple's Bonjour, Linux's Avahi). It's a userspace concern in Linux; Fuchsia has it as a component. Surfacing only.

### 6.12 TCP congestion control default

- **CUBIC** (Linux default since 2.6.19; smoltcp supports as an optional module).
- **Reno / NewReno** (RFC 5681; smoltcp supports as an optional module).
- **BBRv1** (Linux available since 4.9; not the default).
- **BBRv2 / BBRv3** (not yet mainline Linux as of mid-2026; BBRv3 has shown 30% throughput improvement over CUBIC on high-BDP shallow-buffer paths per published research; planned for Linux upstream).

RFC 9293 is algorithm-agnostic. Phase 4 THINK has free choice. CUBIC is the conservative, broadly-deployed default; BBRv3 is the forward-looking choice if Phase 4 wants to ship something deliberately ahead of Linux defaults; "pluggable from BUILD-1" is a cost increase relative to "single algorithm at BUILD-1, pluggable later."

### 6.13 Socket / connection-pool resource limits and accountability

Each process holds bounded sockets / connections; each connection consumes kernel state (buffers, retransmit queues, congestion state). Phase 4 THINK will need to decide:
- **Per-process limits**: capability handle limits, fd limits, RLIMIT-style.
- **System-wide limits**: total socket count, total connection count.
- **Accountability surface**: per-socket / per-process bandwidth, packets-out, packets-in counters. Composes with Bible Article 22.3 (inference accountability — the precedent is set).

### 6.14 Asynchronous-vs-synchronous I/O semantics

Phase 4 THINK will need to decide:
- **Blocking calls only** (simple, expensive for high-fan-in servers).
- **Edge-triggered or level-triggered readiness notifications** (Linux epoll, FreeBSD kqueue).
- **Completion-based** (Windows IOCP, Linux io_uring, Hubris notification-on-socket-activity).

Hubris's notification-on-activity pattern composes naturally with capability handles. io_uring's completion-queue pattern composes with the kernel↔userspace shared-ring pattern Phase 3 substrate experience makes familiar. The choice has API-shape implications that propagate to every application.

---

## 7. §2.1.E Pre-Resolved Design-Space Enumeration

This section makes explicit where existing Bible and ADR posture already pre-resolves design space, so Phase 4 THINK does not re-litigate decided questions.

### 7.1 Bible Article 11.2 — in-kernel stack, IPv4+IPv6 dual, IPv6-default, not BSD sockets

**Decided.** The stack is in the kernel (Article 11.1 + 11.2). The stack supports IPv4 and IPv6. IPv6 is the default. The public API is not BSD sockets.

**Left for THINK.** What "not BSD sockets" looks like in API shape (§6.1). Which RFC features are first-cut and which are later (§6.2 — TCP/UDP/ICMP scope at BUILD-1). The dual-stack-mandatory PM-NOTE position (a) extends this with a stronger commitment: not just supports-both but mandatory-both, with neither downgraded to optional-library status.

### 7.2 Bible Article 11.3 — TLS as kernel concern at least for authenticated model downloads; minimal audited crypto

**Decided.** TLS is in the kernel's responsibility envelope to support authenticated model downloads. The crypto is audited and minimal. NeoPlutarch does not roll its own crypto primitives.

**Left for THINK.** TLS placement detail (§6.3) — pure-kernel-resident vs hybrid kTLS-style vs pure userspace. The PM-NOTE position (c) extends this with a commitment: TLS-at-application-layer is the security primitive, IPsec is deferred to Phase 8+ opt-in. This is RFC-8504-aligned (IPsec SHOULD-not-MUST) and current-deployment-aligned.

### 7.3 Bible Article 9.10 — virtio-net + e1000e Phase 4 first-class; USB Ethernet bundled; RTL8169 / ixgbe Phase 5+; WiFi Phase 7.5

**Decided.** The Phase 4 hardware driver list is enumerated. virtio-net first (QEMU validation path); e1000e second (real-hardware coverage). USB Ethernet (CDC-ECM, CDC-NCM, RNDIS) bundled with the Phase 4 USB-stack landing per ADR-017 §D3. Realtek RTL8169-class and Intel ixgbe-class deferred to Phase 5+. WiFi deferred to Phase 7.5.

**Left for THINK.** The `NetDevice` trait surface (§6.5). The sub-sequencing of virtio-net and e1000e (which is BUILD-1 of Phase 4? which is later?). The USB-stack-landing prerequisite ordering (USB stack must land before USB Ethernet; how do those sub-BUILDs interleave?).

### 7.4 Bible Article 12 — capability-based security; sockets-equivalent objects are capability-protected

**Decided.** Every kernel object is referenced by a capability handle. Holding the handle is permission; there is no ambient authority. No root user. Privilege is per-capability. Sockets-equivalent objects are no exception.

**Left for THINK.** The shape of the socket capability (§6.1, §5). The initial-capability-acquisition path: does a process get a network capability at spawn (Fuchsia component-manifest precedent), or must it request one from a privileged service (capability-handing-over precedent), or both? The relationship between socket-capability rights bitmaps (Capsicum precedent) and broader capability-rights metadata. PM-NOTE position (f) — capability-protected sockets-equivalent — is satisfied by any of these shapes.

### 7.5 Bible Article 3 — no POSIX compatibility; no BSD-sockets API

**Decided.** The native ABI is not POSIX. A POSIX shim **may** exist for porting userland tools, but the native interface is NeoPlutarch's own.

**Left for THINK.** Whether a libneo-level POSIX-shim wraps the native sockets-equivalent into a BSD-socket-shaped API for portability (Fuchsia's fdio precedent shows this is achievable on top of a non-POSIX-native capability model). Phase 8+ concern; not Phase 4 BUILD scope.

### 7.6 ADR-022 — Theseus cell-swap forward marker for Phase 4+ graceful-recovery triggers

**Decided.** Any Phase 4+ failure mode where graceful recovery would significantly beat panic is an ADR-022 revisit-trigger. The network stack has several candidate failure modes: device-driver wedge, congestion-control state divergence, routing-table corruption, transient SLAAC-derived address conflict.

**Left for THINK.** Whether any of these candidate failure modes warrants a Phase-4-class cell-swap recovery design (vs. simpler kernel-resident "reset this component" approaches). The Theseus precedent is forward-marker, not requirement.

---

## 8. §2.1.F PM-NOTE Position Confirmation / Refutation

The PM-NOTE proposed six positions. Each is graded as **CONFIRMED** (current RFCs and current OS practice support), **REFINABLE** (substantively right but with citation or framing updates worth folding in before Phase 4 THINK opens), or **REFUTED** (current state has moved past the PM-NOTE framing).

### Position (a) — Dual-stack mandatory

**CONFIRMED.** Current Internet deployment makes a single-stack endpoint OS untenable for "usable by real people" (Bible Article 28). IPv6 hit 50.1% of Google's traffic on March 28, 2026 — historic for the first crossover, but the other 49.9% remains IPv4. Cloudflare Radar shows 40.1% IPv6 in HTTP requests; APNIC Labs shows 43.13% IPv6-capable networks. Country variation is dramatic: France 73%, India 72%, Saudi Arabia 65%; vs. Italy 17%, Spain 10%, Egypt 4%. An IPv6-only endpoint OS is broken in most of the world; an IPv4-only endpoint OS misses growing infrastructure. RFC 8504 (IPv6 Node Requirements) and RFC 1122 (IPv4 host requirements) are both normative; both must be satisfied. Bible Article 11.2 already specifies dual-stack; the PM-NOTE elevates this to "mandatory-both, not optional-library."

### Position (b) — IPv6-preferred with Happy Eyeballs (RFC 8305); RFC 6724 for address selection

**CONFIRMED, with forward-marker.** RFC 8305 is the current normative reference for dual-stack connection establishment. RFC 6724 is the deterministic address-selection foundation 8305 builds on. The combination is what makes "IPv6-preferred" actually work consistently across implementations. **Forward marker:** `draft-ietf-happy-happyeyeballs-v3` (rev -03, March 2026, active in the HAPPY WG) extends RFC 8305 with SVCB / HTTPS DNS resource record handling and protocol-order sorting (QUIC before TCP). Phase 4 THINK should design the sockets-equivalent and the connection-establishment API in a way that does not preclude future v3 adoption; specifically, the API should not be tightly coupled to the A/AAAA address pair, but rather to a candidate-address-and-protocol-priority list. This is design-shape advice for Phase 4 THINK, not a refutation of the PM-NOTE position.

### Position (c) — TLS as security primitive; IPsec deferred to Phase 8+ opt-in

**CONFIRMED, with citation refinement.** RFC 8504 (Jan 2019) — the current IPv6 Node Requirements document — retained IPsec at SHOULD-not-MUST, exactly as RFC 6434 (Dec 2011) originally specified. PM-NOTE §2.1 cites RFC 6434 for the IPsec downgrade. The historical attribution is correct; the current normative authority is RFC 8504. **Citation suggestion for PM-NOTE update:** include RFC 8504 alongside (or instead of) RFC 6434 in the substrate-reading list. Substantively: TLS is the deployed-Internet-2026 security primitive; consumer-grade IPv6 deployments rarely use IPsec; the Linux/kTLS hybrid is the production "TLS in the kernel" pattern (see §6.3); Bible Article 11.3 already commits to this framing. Phase 4 THINK has design choices about TLS placement detail (§6.3) but the policy commitment is sound.

### Position (d) — NAT64/DNS64 transition support deferred to Phase 8+

**CONFIRMED.** NAT64/DNS64 (RFC 6146 / RFC 6147) is operator-side transition infrastructure for IPv6-only networks reaching IPv4-only servers. The endpoint OS's role is limited: in IPv6-only client deployments (mobile carriers, certain enterprise networks) the host benefits from NAT64-prefix discovery (RFC 7050 / RFC 8781) and from 464XLAT CLAT-side translation (RFC 6877). For a workstation/laptop OS — NeoPlutarch's target — these are niche scenarios. The v6ops WG has active drafts on IPv6-mostly networks and on multi-domain IPv6-only underlay frameworks (Nov/Dec 2025 milestones), but the host-OS work for Phase 4 is bounded by Happy Eyeballs's local-synthesis pattern (RFC 8305 §7 — synthesize an IPv6 address from an A record via DNS64 only as last resort) which can be supported without full NAT64/DNS64/CLAT machinery. **Phase 8+ deferral is sound.**

### Position (e) — No BSD sockets

**CONFIRMED.** Bible Article 11.2 already commits this. Article 3 (no POSIX compatibility) makes it a constitutional position. Section 5 of this artifact maps the design space for what replaces BSD sockets — Fuchsia FIDL-channels-as-capability, Plan 9 namespace-as-capability, Tock types-as-capability, Hubris compile-time-binding-as-capability, Capsicum rights-bitmap-on-handle. The PM-NOTE position is constitutional consensus; Phase 4 THINK chooses among the shapes the design space offers.

### Position (f) — Capability-protected sockets-equivalent

**CONFIRMED.** Bible Article 12 already commits this. Section 5 maps the capability-shape design space. The PM-NOTE position is constitutional consensus; Phase 4 THINK chooses the specific instantiation.

---

## 9. Suggested PM-NOTE Corrections

These corrections do not change the substance of any PM-NOTE position. They update document citations to current RFCs and surface one forward-marker. Surfaced here per dispatch §4 ("explicit list of suggested-corrections to the PM-NOTE if research found any"). PM has the option to fold these into a PM-NOTE-v0.2 before Phase 4 THINK opens, or to leave the PM-NOTE as a preserved-input artifact and incorporate the corrections directly into Phase 4 THINK's substrate-reading list. PM's call.

1. **Cite RFC 8504 (Jan 2019) in addition to (or in place of) RFC 6434 (Dec 2011) for the IPsec MUST→SHOULD posture.** PM-NOTE §2.1's historical framing of "RFC 6434 (December 2011) explicitly downgraded IPsec from MUST to SHOULD" is correct, but RFC 6434 is now obsolete (superseded by RFC 8504). RFC 8504 retained the SHOULD posture and is the current authoritative document for IPv6 host requirements. The substrate-reading list in PM-NOTE §2.3 names RFC 6724, RFC 8305, and RFC 6434 — RFC 8504 should be added.

2. **Cite RFC 9915 (Jan 2026) for DHCPv6, not RFC 8415 (Nov 2018).** Dispatch §2.1.B names RFC 8415 in the survey list. RFC 9915 obsoleted RFC 8415 four months before this OBSERVE pass. The substantive functional difference: IA_TA support (DHCPv6 temporary addresses) and the Server Unicast option are removed. Phase 4 THINK's DHCPv6 reading should target RFC 9915. (This correction applies to the dispatch's survey list; PM-NOTE itself does not name RFC 8415 explicitly.)

3. **Surface `draft-ietf-happy-happyeyeballs-v3` as the forward-marker for Happy Eyeballs.** PM-NOTE position (b) names RFC 8305. RFC 8305 remains the deployed normative reference, but HEv3 is in active IETF process (rev -03 as of March 2026; HAPPY WG; SVCB/HTTPS records and QUIC-before-TCP priority). Phase 4 THINK should design APIs that accommodate the v3 protocol-priority axis (not just A/AAAA addresses).

None of these corrections invalidates a PM-NOTE position. They are citation refinements and a forward-marker.

---

## 10. Design Questions Surfaced for Phase 4 THINK

Re-enumerated from §6 for the PM-consumption summary. Phase 4 THINK is the authority for resolution.

1. Sockets-equivalent API shape (channel / endpoint / connection / layered).
2. In-kernel TCP/UDP/ICMP scope at BUILD-1 vs deferred BUILDs.
3. TLS placement (pure userspace / hybrid kTLS-style / kernel-resident).
4. DNS resolver placement (kernel-resident / userspace daemon).
5. Network-driver trait surface composition with the Phase 3 BlockDevice substrate.
6. Namespacing / multi-tenancy / per-process network capability scoping.
7. Loopback semantics (standard routing vs short-circuit).
8. Routing table representation, update model, modification-capability gate.
9. Address autoconfig placement: DHCPv4 (kernel vs userspace), SLAAC (kernel vs userspace), DHCPv6 (per RFC 9915, kernel vs userspace).
10. IPv6 Privacy Extensions (RFC 8981) default-on policy.
11. mDNS / link-local discovery scope (Phase 4 or deferred).
12. TCP congestion control default (CUBIC / Reno / BBRv3 / pluggable).
13. Socket / connection resource limits and accountability surface (per-socket counters).
14. Asynchronous-vs-synchronous I/O semantics (blocking / readiness / completion).

Phase 4 THINK does not need to resolve all 14 at PLAN entry; some (e.g. mDNS, Privacy Extensions, congestion-control default) can be left for later BUILDs once the core capability-shape and protocol-scope decisions are made.

---

## 11. References

### RFCs cited

- RFC 791 (IPv4) / RFC 1122 (host requirements) — background, not directly fetched
- RFC 2131 (DHCPv4, March 1997) — `https://www.rfc-editor.org/rfc/rfc2131`
- RFC 4861 (IPv6 ND, Sept 2007) — `https://www.rfc-editor.org/rfc/rfc4861`
- RFC 4862 (SLAAC, Sept 2007) — `https://www.rfc-editor.org/rfc/rfc4862`
- RFC 5961 (TCP blind-injection mitigations, Aug 2010) — `https://www.rfc-editor.org/rfc/rfc5961`
- RFC 6434 (IPv6 Node Requirements, Dec 2011) — `https://datatracker.ietf.org/doc/rfc6434/` — **obsoleted by RFC 8504**
- RFC 6724 (Default Address Selection for IPv6, Sept 2012) — `https://www.rfc-editor.org/rfc/rfc6724`
- RFC 7323 (TCP High-Performance Extensions, Sept 2014) — `https://www.rfc-editor.org/rfc/rfc7323`
- RFC 8085 (UDP Usage Guidelines, BCP 145, March 2017) — `https://www.rfc-editor.org/rfc/rfc8085`
- RFC 8200 (IPv6, STD 86, July 2017) — `https://www.rfc-editor.org/rfc/rfc8200` — **updated by RFC 9673**
- RFC 8305 (Happy Eyeballs v2, Dec 2017) — `https://www.rfc-editor.org/rfc/rfc8305`
- RFC 8415 (DHCPv6, Nov 2018) — **obsoleted by RFC 9915**
- RFC 8504 (IPv6 Node Requirements, Jan 2019) — `https://www.rfc-editor.org/rfc/rfc8504`
- RFC 8981 (IPv6 Privacy Extensions, Feb 2021) — `https://www.rfc-editor.org/rfc/rfc8981`
- RFC 9293 (TCP, Aug 2022) — `https://www.rfc-editor.org/rfc/rfc9293`
- RFC 9673 (IPv6 Hop-by-Hop Options Processing, Oct 2024) — `https://www.rfc-editor.org/rfc/rfc9673`
- RFC 9915 (DHCPv6, Jan 2026) — `https://www.rfc-editor.org/rfc/rfc9915`
- `draft-ietf-happy-happyeyeballs-v3-03` (Happy Eyeballs v3, March 2026; active IETF draft) — `https://datatracker.ietf.org/doc/draft-ietf-happy-happyeyeballs-v3/`

### IETF working groups

- 6man (IPv6 Maintenance) — `https://datatracker.ietf.org/wg/6man/about/`
- v6ops (IPv6 Operations) — `https://datatracker.ietf.org/wg/v6ops/about/`
- HAPPY (Happy Eyeballs) — emerging WG, drafts under `https://datatracker.ietf.org/wg/happy/`

### OS / codebases reviewed

- smoltcp — `https://github.com/smoltcp-rs/smoltcp`
- Fuchsia Netstack3 — `https://fuchsia.dev/fuchsia-src/contribute/contributing-to-netstack/netstack3`
- Fuchsia capability routing — `https://fuchsia.dev/fuchsia-src/concepts/components/v2/capabilities`
- Fuchsia component manifests — `https://fuchsia.dev/reference/cml`
- Oxide Hubris — `https://github.com/oxidecomputer/hubris`, `task/net/` and `app.toml`-driven configuration
- Hubris reference docs — `https://hubris.oxide.computer/reference/`
- Oxide RFD 250 (Management Network Topology) — `https://rfd.shared.oxide.computer/rfd/0250`
- Tock typed capabilities — `https://docs.tockos.org/kernel/capabilities/`
- LionsOS — `https://lionsos.org/`
- seL4 / CAmkES — `https://docs.sel4.systems/projects/camkes/`
- seL4 capDL — `https://docs.sel4.systems/projects/capdl/`
- seL4 Foundation whitepaper — `https://sel4.systems/About/seL4-whitepaper.pdf`
- Theseus OS — `https://www.theseus-os.com/`; OSDI 2020 paper (Boos et al.)
- Redox OS netstack — `https://github.com/redox-os/netstack` (`smolnetd` daemon and scheme-rooted paths)
- Plan 9 networking — `https://9p.io/sys/doc/net/net.html` ("The Organization of Networks in Plan 9")
- FreeBSD Capsicum — `https://man.freebsd.org/cgi/man.cgi?capsicum(4)`
- Linux kTLS — `https://docs.kernel.org/networking/tls.html`; `https://github.com/rustls/ktls`
- Asterinas netstack debate — `https://github.com/asterinas/asterinas/issues/1821` (smoltcp vs Netstack3)

### Industry signal points

- Google IPv6 statistics — `https://www.google.com/intl/en/ipv6/statistics.html` (50.1% milestone March 28, 2026)
- APNIC Blog: "Google hits 50% IPv6" (April 28, 2026)
- Internet Society Pulse: "18 Years Later, IPv6 Reaches Majority" (April 2026)
- APNIC Blog: "Analysing the state of Happy Eyeballs implementations" (October 2025)
- Phoronix coverage of BBRv3 upstream trajectory

### Cross-references in NeoPlutarch artifacts

- `docs/bible.md` v1.6.0 — Articles 1.2, 1.4, 3, 4.1, 9.10, 10, 11, 12, 14, 18.1, 28
- `docs/ADR-017-*.md` — USB stack and USB Ethernet sequencing per Phase 4
- `docs/ADR-022-*.md` — Theseus cell-swap forward marker
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md` — substrate this OBSERVE confirms/refines
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md` — sequencing context
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` — Phase 3 substrate that the network-driver trait composition will inherit from (§6.5)
- `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` — voice / register anchor
- `Plans/RESEARCH-2026-05-11-phase-3-neofs-prior-art-cerebro.md` — depth anchor

---

## 12. Closure (per dispatch §4)

> this artifact is OBSERVE-class per Bible Article 14; it maps terrain and does not choose routes. It is informational per Bible Article 18.1; it does not bind the project. Phase 4 THINK is the natural authority for resolving the design questions enumerated herein.

---

**End of research artifact.**

*Phase 4 substrate confirmed. PM-NOTE proposed positions (a)-(f) all confirmed at the substantive level. Three citation refinements surfaced: RFC 8504 supersedes RFC 6434 (IPv6 host requirements; IPsec retained at SHOULD); RFC 9915 supersedes RFC 8415 (DHCPv6; IA_TA removed); draft-ietf-happy-happyeyeballs-v3 is the HEv3 forward-marker adding SVCB/HTTPS records and QUIC-before-TCP. No Bible amendments triggered. No ADR drafted. No ANSPEC drafted. 14 design questions enumerated for Phase 4 THINK to resolve. smoltcp confirmed as the canonical Rust-native bare-metal candidate per Bible Article 11.2; Fuchsia Netstack3 confirmed as the closest capability-shaped Rust production analogue; Tock + Hubris + Plan 9 + Capsicum + seL4 capability-shape poles surfaced for Phase 4 THINK's API-shape decision. No stop conditions hit; no PM intervention required between now and PM consumption.*
