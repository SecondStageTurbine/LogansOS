# Phase 4 THINK v0.2 — Network Stack Design Direction

**Date:** 2026-05-22 (filed late session, post Cerebro pressure-test)
**Recorder:** PM
**Status:** DRAFT — awaiting PL ratification of revised Q1–Q12
**Cadence position:** THINK v0.2 supersedes THINK v0.1 per Bible Article 24.2 (decisions are reversible; superseded artifacts retained as history). PLAN follows ratification.

**Supersedes:** `Plans/PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` (v0.1, retained as historical record per Article 24.2).

**Authority chain consumed:**

- All of v0.1's authority chain (Bible v1.7.0 Articles 1.2, 1.4, 3, 4.1, 9.10, 11, 12, 14, 18.1, 28; PM-NOTE design input + ADDENDUM; OBSERVE; OBSERVE consumption disposition; sequencing rung-by-rung note; storage driver prior-art reference).
- `Plans/PRESSURE-TEST-2026-05-22-phase-4-think-cerebro.md` — Cerebro's pressure-test (649 lines; 8 CONFIRMED, 5 REFINABLE, 1 CONSIDER-REVISITING, 0 REFUTED).
- `Plans/PM-DISPOSITION-2026-05-22-phase-4-think-pressure-test-consumption.md` — PM acceptance of pressure-test findings.
- PL ratification 2026-05-22 of Q-A (BUILD-1.1 sub-commit) and Q-B (BUILD-1f sub-commit) scope adds.

---

## 1. What v0.2 Changes Relative To v0.1

This is an update pass, not a fundamental restructure. No PM-NOTE confirmed position is reopened. No REFUTED verdicts. The five REFINABLE refinements are framing changes; the one CONSIDER-REVISITING (capability lifetime across address rotation) becomes a new BUILD-1 sub-commit (BUILD-1.1) with PL ratification. R11–R16 join the risk register. Q1–Q10 become Q1–Q12.

Concretely changed sections, mapped to pressure-test findings:

- **§3.1 (sockets-equivalent shape)** — Reframe "forward-accommodates UDP and TCP" as BUILD-3 hypothesis (per pressure-test §2.1, Cerebro Q1, new R16).
- **§3.3 (TLS placement)** — Acknowledge Linux net/tls CVE cluster; link to ADR-022 cell-swap as candidate revisit-trigger (per pressure-test §2.3, Cerebro Q3, new R12).
- **§3.4 (DNS resolver)** — Acknowledge encrypted-DNS trajectory tighter than typical Phase 5+ horizon; add BUILD-4d sub-commit for address-discovery-layer extensibility (per pressure-test §2.4, Cerebro Q4, new R14).
- **§3.10 (Privacy Extensions)** — Add capability-lifetime-under-address-rotation discussion; add BUILD-1.1 sub-commit (per pressure-test §2.7 §4.7b, Cerebro Q5, new R13). **PL ratified 2026-05-22.**
- **§4 (sub-BUILD sequencing)** — Acknowledge IPv6-before-IPv4 lacks documented precedent (per pressure-test §2.2, Cerebro Q2). Add BUILD-1.1 and BUILD-1f to BUILD-1 sub-commits. Add BUILD-4d to BUILD-4 sub-commits.
- **§5 R1 (smoltcp wrap/fork/scratch)** — Update framing with v0.13.0 evidence and Asterinas closure (per pressure-test §2.11, Cerebro Q8).
- **§5 R3 (real-hardware quirks)** — Extend with documented I219-V e1000e quirks (per pressure-test R15, Cerebro Q6 — absorbed without separate PL gate).
- **§5 R11–R16** — Add six new risks per pressure-test §3.
- **§9 Q1–Q12** — Revise from Q1–Q10 with two new ratifications surfaced by the pressure-test absorption.

All other sections in v0.1 are reproduced unchanged below for self-containment.

---

## 2. Scope of This Artifact (unchanged from v0.1)

This THINK pass:

- **Chooses** among the 14 design questions Cerebro's OBSERVE enumerated, scoped to what Phase 4 BUILD-1 ships.
- **Sequences** Phase 4 into BUILD-1 through BUILD-Nb sub-commits with explicit goal milestones.
- **Enumerates risks** premortem-style and assigns mitigations.
- **Identifies prerequisites** that must be true before BUILD-1 can start.
- **Runs the /simplify gate** per Bible Article 14 and documents what was cut.
- **Surfaces final PL ratifications** needed before PLAN opens.

This THINK pass does NOT open PLAN, file ADRs, touch kernel code, or re-litigate the six confirmed PM-NOTE positions.

---

## 3. The Goal Milestone (unchanged from v0.1)

Phase 4 ships when:

> A NeoPlutarch userspace process, holding only a network-access capability handle, sends a TCP-framed HTTPS request to a real remote server on the dual-stack public Internet, receives the response, and verifies the TLS certificate against a kernel-managed trust anchor. The same process can also send and receive UDP datagrams on both IPv4 and IPv6. The Phase 3 substrate (block I/O, NeoFS) remains green; no regressions.

The Phase 4 ship moment is **BUILD-5 close** in the sub-BUILD numbering below.

---

## 4. The Chosen Position on Each Open Design Question

### 4.1 Sockets-equivalent API shape (Cerebro OBSERVE §6.1)

**Choice: layered-handle shape.** A process holds a `NetworkAccess` outer capability handed to it at spawn (or inherited). From `NetworkAccess` the process creates per-socket inner handles via a syscall family: `sys_net_socket_udp(NetworkAccess) -> Result<UdpSocket>`, `sys_net_socket_tcp(NetworkAccess) -> Result<TcpSocket>`, `sys_net_listen_tcp(NetworkAccess, bind_spec) -> Result<TcpListener>`, etc. Each inner handle is itself a capability that can be passed to other processes via the standard capability-passing mechanism (Bible Article 12).

**Rationale unchanged from v0.1:** endpoint-shaped too rigid; channel-shaped requires filesystem-namespace primary capability model NeoPlutarch doesn't match; connection-shaped insufficient for listening sockets; layered is the most flexible.

**HEv3 forward-compatibility unchanged:** connection-establishment syscalls take `&[NetCandidate]` candidate lists, not `(addr, port)` tuples.

**v0.2 refinement (per pressure-test §2.1, §2.5, §2.14 R16; PM-DISPOSITION §3.1).** The claim that "the layered-handle shape forward-accommodates both UDP and TCP without API-shape revisions" rests on a precedent that has not been exercised. Hubris (the cited UDP-only-then-TCP analog) remains UDP-only and has not staged toward TCP. Redox demonstrates the *additive* path (new scheme methods added when UDP semantics needed extending) rather than the *revisionary* path; that is supporting evidence but not equivalent evidence.

**This claim becomes a BUILD-3 design hypothesis to validate, not a precedent-supported certainty.** PM's mitigation per Cerebro: BUILD-1 PLAN sketches `sys_net_socket_tcp` and `sys_net_listen_tcp` speculatively (without committing them) to validate the layered shape composes with TCP's connection-state model, partial-write tracking, urgent-data handling. If the speculative sketch reveals an API-shape obstacle, it surfaces at BUILD-1 PLAN rather than BUILD-3 implementation.

### 4.2 In-kernel transport scope at BUILD-1 (unchanged from v0.1)

**Choice: UDP and ICMPv6 at BUILD-1. TCP at BUILD-3.** Verdict in pressure-test: CONFIRMED. The Redox precedent (additive scheme methods for UDP-specific semantics) supports the BUILD-1 commitment without contradicting the BUILD-3 sequencing choice.

### 4.3 TLS placement (Cerebro OBSERVE §6.3)

**Choice: hybrid kTLS-style. Deferred to BUILD-5.**

Userspace owns the TLS handshake (rustls in libneo or similar). Kernel takes ownership of the symmetric record layer via `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)`. Matches the Linux kTLS pattern, the only production-deployed "TLS in the kernel" reference in 2026.

**Rationale unchanged from v0.1:** pure-userspace prevents future `sendfile`-style optimizations the AI subsystem may want; pure-kernel-handshake violates Bible Article 11.3's no-rolling-own-crypto-primitives. Hybrid is the production answer.

**v0.2 refinement (per pressure-test §2.3, §2.14 R12; PM-DISPOSITION §3.1).** "Matches Linux kTLS" inherits Linux kTLS's documented failure-mode profile: seven distinct CVEs across 2024–2025 in `net/tls` (CVE-2024-26582/3/4/5 cluster: async-crypto state corruption; CVE-2025-38608: bpf+ktls plaintext truncation; CVE-2025-39946: `tls_rx_msg_size()` parser OOB; CVE-2025-40176: async decryption hold/wait). The recurring pattern is async-callback + crypto-state interactions across workqueue scheduling, backlog queue management, partial-read zero-copy, and callback-based decryption completion.

This is the failure-mode profile NeoPlutarch will absorb at BUILD-5. Two consequences:

1. **BUILD-5 design pass must explicitly audit the async-callback shape.** Where possible, structure the record layer as a synchronous per-packet transformation in the network task's context rather than Linux's "callback fires on workqueue completion with shared mutable state" pattern. The synchronous-per-packet path is more constrained but eliminates an entire class of the documented failure mode.

2. **BUILD-5 TLS state becomes a candidate ADR-022 cell-swap revisit-trigger.** ADR-022 forward-marks fault-recovery semantics for graceful-restart of kernel-resident components. TLS state with documented complex async-crypto failure modes is precisely the shape ADR-022's revisit-trigger criteria target. PM does not file the ADR-022 revisit here; the trigger fires at BUILD-5 PLAN time when the actual TLS state shape is committed.

### 4.4 DNS resolver placement (Cerebro OBSERVE §6.4)

**Choice: kernel-resident stub resolver at BUILD-4, coupled to Happy Eyeballs.**

A stub resolver lives in the kernel network service alongside the transport layer. RFC 1035 query format, RFC 6891 EDNS(0), RFC 8106 RDNSS from RAs. Reachable via `sys_net_resolve(name, family_hint) -> Result<&[NetCandidate]>` and called internally by connection-establishment syscalls.

**Rationale unchanged from v0.1:** Happy Eyeballs requires tight coupling between DNS and connection racing; userspace daemon adds round-trips that work against HE's 50ms/250ms timing budgets. Kernel-resident keeps it in one place.

**v0.2 refinement (per pressure-test §2.4, §2.14 R14; PM-DISPOSITION §3.1).** The 2025–2026 direction of travel in encrypted-DNS deployment is tighter than THINK v0.1's "DoT/DoH explicitly deferred" framing implied:

- RFC 9461 (SVCB for DNS, Nov 2023) and RFC 9462 (Discovery of Designated Resolvers, Nov 2023) standardize client-side discovery of encrypted resolvers.
- `draft-ietf-add-encrypted-dns-server-redirection` (EDSR) defines redirection semantics for encrypted resolvers.
- January 2025 U.S. Executive Order mandates DNS encryption for federal systems.
- Mozilla reports Firefox DoH adoption above 85% among U.S. users by 2025–2026.
- Industrial and enterprise networks routinely block port 53 to non-approved servers; consumer-carrier port-53 blocking is less documented at scale but exists.

THINK v0.1's "stub resolver only, port 53 only" is functionally sufficient for residential dual-stack contexts at BUILD-4 ship and Phase 4 ship (BUILD-5 HTTPS to a real server). The refinement is to design BUILD-4 such that DoT/DoH support is a configurable layer addable later without revising the resolver core.

**BUILD-4d sub-commit added (see §5):** the resolver's address-discovery responsibility (where to send queries) is exposed via a configurable layer separate from the query-encode/response-parse core. At BUILD-4 ship, the layer reads RDNSS from RAs and DHCPv4 (per Phase 4 BUILD-2 timing). Later BUILDs (Phase 5+) can extend the discovery layer with RFC 9461 SVCB, RFC 9462 designated-resolver, and EDSR redirection semantics without modifying the query core. The kernel-resident posture remains; the discovery layer becomes extensible.

### 4.5 Network-driver trait surface (unchanged from v0.1)

**Choice: `NetDevice` trait parallels `BlockDevice`.** Maximum reuse of Phase 3 substrate. BUILD-1 ships virtio-net only; e1000e and USB Ethernet are later BUILDs.

### 4.6 Namespacing / multi-tenancy (unchanged from v0.1)

**Choice: flat at Phase 4. Namespacing deferred to Phase 8+.** Verdict in pressure-test: not separately addressed; covered under §4.7 framework. /simplify cut surfaced in §8 below.

### 4.7 Loopback (unchanged from v0.1)

**Choice: short-circuit at IP layer.** Verdict in pressure-test: CONFIRMED. Forward markers: when firewall/filter machinery lands (Phase 8+), short-circuit needs an explicit decision about filter rules on loopback; when tcpdump-equivalent lands, short-circuit needs a tap point; MTU semantics on loopback may legitimately differ from real interfaces (document as intentional in BUILD-1d).

### 4.8 Routing table (unchanged from v0.1)

**BUILD-1 choice: kernel-managed, SLAAC-derived, no manual modification API.**

### 4.9 Address autoconfig (unchanged from v0.1)

**BUILD-1 choice: SLAAC only, kernel-resident.** DHCPv4 in BUILD-2. DHCPv6 in BUILD-3+ only if concrete need.

### 4.10 IPv6 Privacy Extensions (Cerebro OBSERVE §6.10)

**Choice: enabled by default at BUILD-1 per RFC 8981.** Default lifetimes: TEMP_VALID_LIFETIME 2 days; TEMP_PREFERRED_LIFETIME 1 day; TEMP_IDGEN_RETRIES 3; MAX_DESYNC_FACTOR 0.4 × preferred.

**Verdict in pressure-test §4.7a: CONFIRMED** on the network-policy axis (privacy-default aligned with current BCP and modern OS practice).

**v0.2 addition (per pressure-test §2.7 §4.7b, Cerebro Q5, new R13; PM-DISPOSITION §3.2; PL ratified Q-A 2026-05-22).** Privacy-default at BUILD-1 means TEMP_PREFERRED_LIFETIME = 1 day. A long-lived `UdpSocket` capability handle bound to a temporary IPv6 address sees that address deprecated after a day and removed after another day. THINK v0.1 was silent on kernel behavior across rotation. The documented production pattern across Linux, Windows, macOS, iOS, and Android uses deprecation events + IP-address reference counting; NeoPlutarch's layered-handle capability model needs an equivalent surface.

**BUILD-1.1 sub-commit added (see §5)** defining:

1. **Address-deprecation-event capability** — a notification capability the network service posts when an address moves from preferred → deprecated state. Processes holding `UdpSocket` capabilities on the affected address receive the notification through the existing Phase 1-2 notification primitive (the same primitive used for socket-activity notifications per §4.14).

2. **Socket-binding-retention semantics** — a deprecated address is not removed from the kernel routing/source-selection state while a `UdpSocket` capability bound to it is held. Implementation: per-address reference count that includes capability-bound sockets in the count; address-removal-from-stack is gated on count reaching zero. This matches the documented Linux/macOS/Windows/iOS/Android pattern, integrated with NeoPlutarch's capability table rather than POSIX file-descriptor accounting.

3. **Test plan extension** — BUILD-1e bare-metal verification includes a >24h continuous session to verify Privacy Extensions rotation does not silently break long-lived sockets. The block-device log sink (now shipped) captures rotation events for post-run analysis.

### 4.11 mDNS / link-local discovery (unchanged from v0.1)

**Choice: deferred to Phase 8+.**

### 4.12 TCP congestion control default (unchanged from v0.1)

**Choice: CUBIC only at BUILD-3 ship. Pluggable architecture deferred.** Verdict in pressure-test: CONFIRMED — BBRv3 still not mainline as of May 2026; CUBIC remains broadly deployed; forward-marker framing sound.

### 4.13 Resource limits and accountability (unchanged from v0.1)

**BUILD-1 choice: per-process socket limit 64; per-process counters (packets-in/out, bytes-in/out) exposed via `sys_net_stats(socket_handle)`.**

### 4.14 Async/sync I/O semantics (unchanged from v0.1)

**Choice: notification-on-activity (Hubris precedent).** Verdict in pressure-test: CONFIRMED. io_uring forward-marker reinforced (API still actively evolving in 2025–2026).

---

## 5. Phase 4 Sub-BUILD Sequencing

**v0.2 framing note (per pressure-test §2.2, Cerebro Q2; PM-DISPOSITION §3.1).** The PM choice of IPv6-before-IPv4 has no documented precedent in clean-room kernels surveyed by Cerebro. smoltcp's own history added IPv6 *after* IPv4; Redox inherited both simultaneously by adopting smoltcp wholesale; Hubris is UDP-over-IPv6 only in a controlled service-processor context; Asterinas inherited smoltcp's full coverage on adoption; LionsOS ships lwIP out-of-kernel; Theseus's network surface is unverified. The choice is normatively sound (dual-stack mandatory per PM-NOTE (a); IPv6 default per Bible Article 11) but the comparison set does not testify to it. BUILD-1 → BUILD-2 should be planned with the expectation that some IPv6-first protocol code may need restructuring when IPv4 lands, not the easier expectation of additive overlay.

Sub-BUILD list below absorbs the v0.2 changes (BUILD-1.1, BUILD-1f, BUILD-4d). Pre-v0.2 sub-commits unchanged.

### BUILD-1 — IPv6 + UDP + virtio-net + capability shape

**Goal milestone:** A NeoPlutarch userspace process, holding a `NetworkAccess` capability, opens a `UdpSocket`, binds to a SLAAC-acquired IPv6 global address, sends a UDP datagram to a remote IPv6 host, receives the response. The reply path uses notification-on-activity to wake the process. Loopback short-circuit works for process-to-process UDP on `::1`. Privacy Extensions enabled; address rotation does not silently break long-lived sockets. The Phase 3 substrate is green.

**Anti-scope (deferred to later BUILDs):** TCP, IPv4, DNS, Happy Eyeballs, TLS, DHCPv4/v6, e1000e, USB Ethernet, routing modification, mDNS, namespacing.

**Sub-commits:**

- **BUILD-1a — substrate.** `NetDevice` trait, virtio-net probe and bringup, RX/TX ring management. PCI + MSI-X + DMA-buffer infrastructure reused from Phase 3.
- **BUILD-1b — IPv6 stack core.** IP layer, ICMPv6, ND, SLAAC, Privacy Extensions per RFC 8981. smoltcp-derived or smoltcp-wrapped (decided at PLAN).
- **BUILD-1c — UDP + capability gate.** UDP send/receive path. `NetworkAccess` outer capability with `UdpSocket` inner capability. Syscall surface: `sys_net_socket_udp`, `sys_net_send_udp`, `sys_net_recv_udp`, `sys_net_close`, `sys_net_wait`.
- **BUILD-1d — loopback short-circuit + accountability.** `::1` interception, per-process counters, `sys_net_stats`. Document MTU semantics on loopback as intentional difference from real interfaces.
- **BUILD-1.1 — capability + address rotation** *(v0.2 addition; PL ratified Q-A 2026-05-22).* Address-deprecation-event notification capability; reference-counted address retention while capability-bound sockets exist; integration with the notification primitive used in BUILD-1c. Discharges new R13.
- **BUILD-1f — IPv6 extension-header processing posture** *(v0.2 addition; PL ratified Q-B 2026-05-22).* Hop-by-Hop SHOULD-process per RFC 8200 + RFC 9673 with deferred-with-fast-path-skip; explicit refusal of Routing-type-0 per RFC 5095 (deprecated); defensive parser posture (validate length fields before parser advance; reject malformed option chains; log diagnostically via BUILD-5b block-device log sink); explicit refusal of non-supported extension-header types at BUILD-1 (SRv6, RPL, etc.). Discharges new R11.
- **BUILD-1e — bare-metal verification.** Real hardware target (virtio-net under VMware on the OptiPlex 7080 Micro). Mirrors Phase 3 BUILD-5b in role. Test plan extension per BUILD-1.1: >24h continuous session validating Privacy Extensions rotation does not break long-lived sockets.

### BUILD-2 — IPv4 + DHCPv4 + dual-stack address selection (unchanged from v0.1)

**Goal milestone:** UDP over IPv4 to remote IPv4 hosts. Dual-stack source-address selection per RFC 6724. DHCPv4 (RFC 2131) for IPv4 address acquisition.

### BUILD-3 — TCP + congestion control + RFC 5961 mitigations (unchanged from v0.1)

**Goal milestone:** TCP connection to remote host on either IPv4 or IPv6; bidirectional send/receive; clean close. CUBIC congestion control. RFC 5961 challenge-ACK mitigations. RFC 7323 window scaling negotiated.

**v0.2 addition to BUILD-3 PLAN-time work** *(per pressure-test §2.1, new R16):* validate the layered-handle shape's forward-compatibility with TCP's connection-state model. PM-DISPOSITION §3.1 directs BUILD-1 PLAN to sketch `sys_net_socket_tcp` speculatively; if the sketch surfaces an API-shape obstacle, BUILD-3 PLAN reconsiders.

### BUILD-4 — DNS + Happy Eyeballs

**Goal milestone:** `sys_net_dial(name)` with hostname; kernel performs Happy Eyeballs (RFC 8305) over AAAA and A query results; returns connected TCP socket. API surface is HEv3-compatible at the candidate-list level (HEv2 logic implemented at BUILD-4 ship).

**Sub-commits:**

- BUILD-4a — DNS stub resolver core (RFC 1035 + EDNS(0)).
- BUILD-4b — Happy Eyeballs v2 racing engine.
- BUILD-4c — RDNSS from RAs (RFC 8106) + DHCPv4-option-6 nameservers.
- **BUILD-4d — address-discovery-layer extensibility** *(v0.2 addition per pressure-test §2.4, Cerebro Q4, new R14).* The resolver's address-discovery layer is exposed as a configurable boundary separate from the query-encode/response-parse core. At BUILD-4 ship, the layer reads RDNSS + DHCPv4-option-6. Later BUILDs (Phase 5+) can extend the layer with RFC 9461 SVCB resolver discovery, RFC 9462 designated-resolver, and EDSR redirection semantics without modifying the query core. Discharges new R14.

### BUILD-5 — kTLS-hybrid TLS = Phase 4 ship (unchanged from v0.1)

**Goal milestone:** Userspace handshake via rustls in libneo; `sys_net_tcp_enable_tls` hands record-layer ownership to kernel; HTTPS to a real server; certificate verified against kernel trust anchor.

**v0.2 addition to BUILD-5 PLAN-time work** *(per pressure-test §2.3, new R12):* explicit async-callback-shape audit per §4.3 above; consider ADR-022 cell-swap revisit-trigger for TLS state.

### Post-Phase-4 forward markers (unchanged from v0.1)

e1000e driver; USB Ethernet (CDC-ECM/NCM/RNDIS); HEv3 protocol-priority extension; namespacing / per-process routing-view; pluggable congestion control; BBR family CCs; IPsec opt-in; NAT64/DNS64/464XLAT; mDNS responder; DoT/DoH at the address-discovery layer extension (per BUILD-4d).

---

## 6. Risk Register

Risks R1–R10 unchanged from v0.1 (with R3 extended per v0.2). R11–R16 added per pressure-test §3.

### R1. smoltcp polling model fit with NeoPlutarch's interrupt-driven scheduler

*Mechanism, mitigation paths (wrap, fork, from-scratch) unchanged from v0.1.*

**v0.2 framing update (per pressure-test §2.11, Cerebro Q8; PM-DISPOSITION §3.1).** Calibration evidence updates:
- smoltcp v0.13.0 (March 20, 2025) shipped substantive content (IPv6 SLAAC, TCP improvements, Rust Edition 2024). The "minimal maintenance" characterization originating in asterinas/asterinas#1821 (February 2025) is contested by the v0.13.0 release one month later.
- The Asterinas smoltcp→Netstack3 migration debate closed as "not planned" (#1821). Asterinas stays on smoltcp. The production-Rust-kernel ecosystem has narrowed its options not by reaching consensus on Netstack3 but by walking away from migration. smoltcp's position as the canonical Rust-native bare-metal candidate (Bible Article 11.2) is strengthened.
- Hubris's `task/net` is documented as "a small netstack based on smoltcp" — the only documented "wrap" analog with similar concurrency posture; smoltcp is a Cargo dependency in Hubris, kernel-owned scheduling shim around it.

PLAN's choice should weigh the Hubris-wrap precedent (modest scope but proven) against the engineering question of whether NeoPlutarch's kernel-task scheduling model exposes more or fewer of smoltcp's poll-loop assumptions than Hubris's does. Decision remains at PLAN time.

### R2. Phase 2 capability infrastructure verification (unchanged from v0.1)

### R3. Real-hardware quirks (v0.2 extension per pressure-test §2.14 R15, Cerebro Q6)

*Original mechanism (Intel I219-V differs from QEMU virtio-net) unchanged from v0.1. v0.2 adds documented e1000e quirks:*

- **DMA-buffer-overrun workaround** (Linux commit `b10effb`): slows DMA access by reducing outstanding requests to avoid buffer overrun on I219; impacts TCP performance unless TSO is disabled.
- **TSO regression Linux 5.8 → 6.3** caused I219-LM to run at ~60% of advertised speed; fixed in 6.3.
- **Unrecovered Tx hang** under stressful UDP traffic + multiple Ethernet-cable reconnections on I219-LM/V; recovered only by system reboot.

When PLAN opens for the BUILD that lands e1000e (BUILD-2b or BUILD-6), the design pass enumerates these quirks and documents chosen mitigation per Bible Article 9.10's correctness > coherence > performance ordering. The stressful-UDP failure mode is particularly relevant given BUILD-1's UDP-first scope.

### R4–R10 (unchanged from v0.1)

### R11. IPv6 extension-header receive-path attack surface (new in v0.2)

**Mechanism.** Per RFC 9098, IPv6 extension-header receive path is a documented hazard category. Recent CVEs: CVE-2026-23442 (SRv6 NULL idev dereference combining extension-header parsing + HMAC validation + routing) and CVE-2026-43501 (IPv6 RPL out-of-bounds write).

**Phase 4 exposure.** BUILD-1 ships IPv6 + ICMPv6 + ND + SLAAC. Hop-by-Hop is the most relevant extension header at BUILD-1; RFC 9673 (Oct 2024) loosened mandatory Hop-by-Hop on hosts to SHOULD-process-MAY-skip. Other extension-header surface appears in later BUILDs.

**Mitigation.** BUILD-1f sub-commit per §5. Discharged at BUILD-1 ship.

### R12. Linux net/tls async-crypto-state CVE cluster (new in v0.2)

**Mechanism.** Linux `net/tls` shipped seven distinct CVEs across 2024–2025 with recurring async-crypto state-management failure mode (CVE-2024-26582/3/4/5 cluster; CVE-2025-38608; CVE-2025-39946; CVE-2025-40176). Root pattern: async-workqueue scheduling + backlog queue + zero-copy partial-read + callback-based decryption-completion interactions.

**Phase 4 exposure.** BUILD-5 lands the kTLS-hybrid record layer. NeoPlutarch absorbs the kernel-side complexity that produced the CVE cluster.

**Mitigation.** Per §4.3 v0.2 addition: BUILD-5 design pass explicitly audits async-callback shape; considers synchronous-per-packet structure instead of Linux's "callback fires on workqueue completion with shared mutable state"; ADR-022 cell-swap becomes a candidate revisit-trigger for BUILD-5 TLS state at BUILD-5 PLAN time.

### R13. SLAAC + Privacy Extensions + capability-bound socket lifetime (new in v0.2)

**Mechanism.** Privacy Extensions default-on means TEMP_PREFERRED_LIFETIME = 1 day. Long-lived `UdpSocket` capability bound to a temporary address sees deprecation+removal under default rotation. THINK v0.1 silent on kernel behavior.

**Phase 4 exposure.** BUILD-1 and later BUILDs.

**Mitigation.** BUILD-1.1 sub-commit per §5. PL ratified Q-A 2026-05-22. Discharged at BUILD-1 ship.

### R14. DoT/DoH 2025–2026 deployment trajectory (new in v0.2)

**Mechanism.** Encrypted-DNS deployment is accelerating (RFC 9461, RFC 9462, EDSR, U.S. EO January 2025, Firefox >85% DoH). Port-53 blocking exists in industrial/enterprise contexts. THINK v0.1's "port 53 only, forward markers deferred" is functionally sufficient for residential dual-stack contexts but tonally out of step with the documented trajectory.

**Phase 4 exposure.** BUILD-4 DNS; BUILD-5 HTTPS-to-real-server.

**Mitigation.** BUILD-4d sub-commit per §5. The kernel-resident stub remains; the address-discovery layer becomes extensible so DoT/DoH endpoints can later be supplied without revising the resolver core.

### R15. Intel I219-V e1000e known driver quirks (new in v0.2)

Absorbed into R3 extension above rather than as separate risk. Listed here for cross-reference completeness.

### R16. Hubris UDP→TCP transition unevidenced (new in v0.2)

**Mechanism.** THINK v0.1 §3.1 claimed Hubris precedent for layered-handle shape's forward-compatibility with TCP. Hubris has not added TCP. The claim is a BUILD-3 hypothesis to validate, not a precedent-supported certainty.

**Phase 4 exposure.** BUILD-3 TCP API surface may surface API-shape obstacle that affects BUILD-1's already-shipped surface.

**Mitigation.** Per §4.1 v0.2 refinement: BUILD-1 PLAN sketches `sys_net_socket_tcp` and `sys_net_listen_tcp` speculatively. If the sketch reveals an API-shape obstacle, surface at BUILD-1 PLAN rather than discover at BUILD-3.

### Interaction-with-R1–R10 table (per pressure-test §3)

| New | Trigger | Amplifies / interacts with | Owner |
|---|---|---|---|
| R11 | Malformed Hop-by-Hop or extension header at BUILD-1 IPv6 input | R3 (real-hardware quirks may surface ext-header corner cases); R7 (smoltcp ext-header coverage audit) | BUILD-1 PLAN |
| R12 | BUILD-5 record-layer state corruption under concurrent crypto callbacks | R1 (smoltcp wrap-vs-fork choice constrains kTLS-scheduler interaction); ADR-022 cell-swap revisit candidate | BUILD-5 PLAN |
| R13 | Long-lived socket bound to temp address ages past TEMP_PREFERRED_LIFETIME | R5 (loopback short-circuit ownership invariant could face same question on link-local); R2 (Phase 2 capability infrastructure must support address-event subscription as a capability) | BUILD-1 PLAN (BUILD-1.1) |
| R14 | BUILD-4 stub resolver fails silently against port-53-blocking carrier before BUILD-5 ships | R6 (PL pre-check should include "port 53 reaches a public resolver") | BUILD-4 PLAN |
| R15 | I219-V e1000e DMA-overrun or TSO regression under real-hardware verification | R3 (absorbed) | BUILD-2b or BUILD-6 PLAN |
| R16 | BUILD-3 TCP API surface reveals layered-handle shape requires revision | R1 (smoltcp wrap-vs-fork choice may absorb or amplify API-shape revision cost) | BUILD-1 PLAN (speculative sketch); BUILD-3 PLAN (validation) |

---

## 7. Prerequisites For Phase 4 BUILD-1 To Start (unchanged from v0.1)

P1 Phase 3 formally closed. P2 Phase 2 capability infrastructure verified. P3 PL IPv6 network functional. P4 smoltcp wrap/fork/scratch decided at PLAN. P5 no Phase 3 unresolved blockers.

---

## 8. /simplify Pass

**Cut from BUILD-1 (unchanged from v0.1):** IPv4, TCP, DNS, Happy Eyeballs, TLS, DHCPv4, DHCPv6, e1000e, USB Ethernet, namespacing, mDNS, IPsec, pluggable CC, completion-based I/O.

**v0.2 sub-commit additions to BUILD-1 (not cuts):** BUILD-1.1 (capability + address rotation) and BUILD-1f (IPv6 extension-header processing). Both PL-ratified 2026-05-22. Both pre-empt failure modes that would surface in BUILD-1e verification or later production.

**v0.2 sub-commit additions to BUILD-4:** BUILD-4d (address-discovery layer extensibility). Pre-empts DoT/DoH retrofit at Phase 5+.

**Net BUILD-1 scope change v0.1 → v0.2:** approximately +2-3 commits worth of work. BUILD-1.1 is modest; BUILD-1f is small. The /simplify discipline is preserved: nothing added that doesn't pre-empt a documented failure mode in the comparison set.

**Cut from Phase 4 entirely (unchanged):** HEv3 full implementation, BBR-family, NAT64/DNS64/464XLAT, user-configurable Privacy Extensions, routing-modification API, static route configuration.

---

## 9. What This THINK Does NOT Decide (Reserved for PLAN; unchanged from v0.1)

smoltcp wrap vs fork vs from-scratch; exact syscall numbers and byte-level ABI; DMA-buffer-pool sizing; `NetworkAccess` rights bitmap representation; specific kernel-task scheduling priority; test-harness scenario names; whether RFC 8106 RDNSS option processing is at BUILD-1 or BUILD-4.

---

## 10. Open Questions For PL (revised Q1–Q12)

Q1–Q10 from v0.1 with substance preserved; Q11 and Q12 added from pressure-test findings.

- **Q1.** Phase 4 BUILD-1 scope is **IPv6 + UDP only** (no IPv4, no TCP). Acceptable, or both IP families in BUILD-1?
- **Q2.** Sockets-equivalent API shape is **layered handle**. Acceptable, or different shape?
- **Q3.** TLS is **hybrid kTLS-style at BUILD-5**. Acceptable, or pure-userspace TLS?
- **Q4.** DNS resolver is **kernel-resident, BUILD-4**, with **BUILD-4d address-discovery-layer extensibility** for future DoT/DoH. Acceptable, or userspace daemon?
- **Q5.** TCP congestion control default is **CUBIC at BUILD-3 ship**, pluggable deferred. Acceptable?
- **Q6.** Loopback is **short-circuit at IP layer**. Acceptable?
- **Q7.** Async I/O is **notification-on-activity**. Acceptable?
- **Q8.** Sub-BUILD sequencing is **BUILD-1 (IPv6+UDP) → BUILD-2 (IPv4) → BUILD-3 (TCP) → BUILD-4 (DNS+HE) → BUILD-5 (TLS)**. Acceptable?
- **Q9.** Real-real-hardware (Intel I219-V) verification deferred to **BUILD-2b or BUILD-6** with e1000e driver. virtio-net-under-VMware is BUILD-1e bare-metal target. Acceptable?
- **Q10.** PL ratification gate: open PLAN drafting **now**, or wait for Phase 3 formal close?
- **Q11** *(new in v0.2 per Cerebro Q1, R16).* THINK §4.1 v0.2 reframes "layered-handle shape forward-accommodates UDP and TCP without revision" as a **BUILD-3 design hypothesis to validate** rather than precedent-supported certainty. BUILD-1 PLAN sketches `sys_net_socket_tcp` speculatively. Acceptable, or do you want stronger commitment (or weaker)?
- **Q12** *(new in v0.2 per Cerebro Q3, R12; PM-DISPOSITION §3.1).* THINK §4.3 v0.2 acknowledges Linux net/tls failure-mode profile and identifies BUILD-5 TLS state as **candidate ADR-022 cell-swap revisit-trigger** at BUILD-5 PLAN time. Acceptable, or do you want ADR-022 revisit-trigger formalized earlier (e.g., at Phase 4 PLAN drafting time)?

**Pre-resolved by PL in 2026-05-22 ratifications (do not need re-ratification):**

- **Q-A.** BUILD-1.1 sub-commit (capability + address rotation). RATIFIED.
- **Q-B.** BUILD-1f sub-commit (IPv6 extension-header processing posture). RATIFIED.

---

## 11. References

All references in v0.1 §10 are preserved. v0.2 adds:

### New RFCs and IETF documents

- RFC 5095 — Deprecation of Type 0 Routing Headers in IPv6 (for BUILD-1f).
- RFC 9098 — Operational Implications of IPv6 Packets with Extension Headers (R11 substrate).
- RFC 9461 — Service Binding Mapping for DNS Servers (BUILD-4d forward marker).
- RFC 9462 — Discovery of Designated Resolvers (BUILD-4d forward marker).
- `draft-ietf-add-encrypted-dns-server-redirection` (EDSR) — encrypted-DNS redirection semantics.

### New CVE references

- CVE-2024-26582, 26583, 26584, 26585 cluster — Linux net/tls async-crypto state corruption.
- CVE-2025-38608 — Linux bpf+ktls plaintext truncation.
- CVE-2025-39946 — Linux `tls_rx_msg_size()` parser OOB.
- CVE-2025-40176 — Linux net/tls async decryption hold/wait.
- CVE-2026-23442 — Linux SRv6 NULL idev dereference.
- CVE-2026-43501 — Linux IPv6 RPL out-of-bounds write.

### New cross-references in NeoPlutarch artifacts

- `Plans/PRESSURE-TEST-2026-05-22-phase-4-think-cerebro.md` — Cerebro's pressure-test substrate for v0.2.
- `Plans/PM-DISPOSITION-2026-05-22-phase-4-think-pressure-test-consumption.md` — PM acceptance authorizing v0.2 filing.

---

**End of THINK v0.2.**

*Phase 4 design direction refined. 5 REFINABLE refinements absorbed. 1 CONSIDER-REVISITING resolved via BUILD-1.1 sub-commit (PL ratified Q-A 2026-05-22). R11–R16 added with mitigations. BUILD-1 gains BUILD-1.1 and BUILD-1f sub-commits (PL ratified Q-A and Q-B). BUILD-4 gains BUILD-4d address-discovery-layer extensibility sub-commit. Q1–Q10 revised to Q1–Q12. PLAN remains gated on Phase 3 formal closure and PL ratification of Q1–Q12 (or revised list).*
