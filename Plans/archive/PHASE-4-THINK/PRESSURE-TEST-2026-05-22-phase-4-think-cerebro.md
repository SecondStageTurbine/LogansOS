# PRESSURE-TEST: Phase 4 THINK Adversarial Research Pass

| | |
|---|---|
| **Date** | 2026-05-22 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-22-phase-4-think-pressure-test-cerebro.md`) |
| **Type** | Pressure-test research artifact per Bible Article 14. Informational only per Bible Article 18.1. Not normative. No Bible/ANSPEC/ADR amendments produced. |
| **Authority** | None. PM consumes via PM-DISPOSITION; if findings warrant, PM files THINK v0.2 referencing this artifact. PL ratification of Q1–Q10 (or revised) follows. |
| **Voice anchor** | `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` and `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`. Citation density; primary-source URLs verified at fetch; indicative for what exists, interrogative for what is uncertain, no imperative toward NeoPlutarch. |
| **Critical-posture invariant** | Pressure-test verdicts rest on cited primary sources. Where a citation could not be verified within the research budget, "Cannot verify from primary source" is marked explicitly with a mitigation note. |
| **Scope discipline** | Adversarial against PM's substantive choices in PHASE-4-THINK-2026-05-22 §3.1–§3.14. Does not re-litigate PM-NOTE positions (a)–(f); does not propose ABIs; does not file Bible amendments or ADR drafts. |

---

## 1. TL;DR

**Three findings carry weight; the rest of THINK survives pressure-test substantially intact.**

The first load-bearing finding is that the Asterinas smoltcp→Netstack3 migration debate **resolved against migration**: issue asterinas/asterinas#1821 closed as "not planned," with Asterinas remaining on smoltcp. This updates Cerebro's 2026-05-20 OBSERVE §3.1 framing, which presented the debate as live. The cited reasons in the closure thread — Netstack3's single-threaded design with no documented multithreading timeline, "upstream maintenance status uncertain given Fuchsia project's current trajectory," and bindings-layer workload — sharpen the calibration NeoPlutarch will need to do at PLAN time for THINK R1 (smoltcp wrap vs fork vs from-scratch). The choice remains open; the production-Rust-kernel ecosystem has narrowed its options not by reaching consensus on Netstack3 but by walking away from migration. smoltcp's own v0.13.0 release on March 20, 2025 — SLAAC, TCP retransmit and zero-window fixes, Rust Edition 2024 bump — contests the "minimal maintenance" characterization that originated in the same Asterinas thread. The maintenance signal is contested by primary release evidence; the choice of smoltcp wrap vs fork rests on engineering questions specific to NeoPlutarch's kernel-task scheduling model, not on smoltcp's external maintenance trajectory alone.

The second load-bearing finding is that **Hubris is still UDP-only**, has not added TCP, and therefore cannot be cited as evidence that THINK §3.1's layered-handle shape forward-accommodates TCP without revision. The Hubris `task/net` README states explicitly: "we currently only support IP and UDP, with a focus on IPv6 … not, at the moment, intended as a fully general network stack." The PM choice at §3.1 ("forward-accommodate UDP and TCP without API-shape revisions") rests on a precedent that has not been exercised. Independent of whether the layered-handle shape actually composes well with TCP — which remains an engineering question THINK can and should address at PLAN — the Hubris analog does not testify to it. The closest analog that *has* added TCP atop a capability-shaped API is Fuchsia Netstack3 via FIDL channels; Netstack3's published `TENETS_AND_DESIGN_DECISIONS.md` and `IMPROVEMENTS.md` do not include an API-shape regret at the FIDL-routing layer, but their documented `DeviceId` field-layout regret is at a lower IPC boundary and not directly transferable.

The third load-bearing finding is that **THINK's "kTLS hybrid is the production-deployed reference" framing is true but understated**: Linux's `net/tls` shipped at least seven distinct CVEs across 2024–2025 with a recurring failure mode in async-crypto state management (double-free, UAF, race conditions in async callbacks; ciphertext-length recalculation after plaintext truncation; out-of-bounds via `tls_rx_msg_size()` parser path). CVE-2025-39946, CVE-2025-38608, CVE-2025-40176, and the CVE-2024-26582/26583/26584/26585 cluster constitute documented evidence that the kernel TLS code path is complex and continues to surface correctness bugs in production-tier review. PM choice at §3.3 (kTLS-hybrid at BUILD-5) is structurally aligned with current practice; the pressure-test refinement is to acknowledge that "matches Linux kTLS" inherits Linux kTLS's documented failure-mode profile, and that "BUILD-5 ship" allocates the kernel-side complexity into the smallest possible sub-BUILD with the latest possible verification window.

**Net verdict across 14 lines (literal enumeration, dispatch §4 order):**

| Dispatch §4 line | Title (abbreviated) | Verdict |
|---|---|---|
| §4.1 | Sockets-equivalent API shape — layered handle vs alternatives | REFINABLE |
| §4.2 | Sub-BUILD sequencing — UDP-first / TCP-third / TLS-fifth | REFINABLE |
| §4.3 | kTLS-hybrid placement and rustls-ktls state | REFINABLE |
| §4.4 | Kernel-resident DNS resolver | REFINABLE |
| §4.5 | BUILD-1 transport scope — UDP only, no TCP | CONFIRMED |
| §4.6 | Loopback short-circuit at IP layer | CONFIRMED |
| §4.7a | RFC 8981 Privacy Extensions default-on (network policy) | CONFIRMED |
| §4.7b | Capability-handle lifetime across address rotation (OS primitive) | CONSIDER-REVISITING |
| §4.8 | Notification-on-activity vs io_uring scaling | CONFIRMED |
| §4.9 | CUBIC vs BBRv3 default congestion control | CONFIRMED |
| §4.10 | IPv6-only BUILD-1 test target reliability | CONFIRMED |
| §4.11 | smoltcp wrap vs fork vs from-scratch | REFINABLE |
| §4.12 | Phase 2 capability infrastructure verification | CONFIRMED |
| §4.13 | IETF/RFC developments since 2026-05-20 | NO MATERIAL CHANGE |
| §4.14 | Risks PM may have missed | ADDITIONAL RISKS SURFACED |

**Totals:** 8 CONFIRMED, 5 REFINABLE, 1 CONSIDER-REVISITING (§4.7b — capability lifecycle), 0 REFUTED. Six additional risks surfaced beyond R1–R10 with mitigation suggestions (see §3). IETF delta 2026-05-20 → 2026-05-22: no material change. No stop condition hit. PM-NOTE positions (a)–(f) remain confirmed.

**Adversarial calibration note (active reading of 0 REFUTED).** The 0 REFUTED count is not passive non-finding. The attack angles tested and rejected within the research budget include: (i) attempting to find a Fuchsia Netstack3 retrospective at the FIDL-channel-via-component-manifest layer that would refute §4.1's layered-handle choice — none surfaced; the only documented regret is at the `DeviceId` field layout, a lower IPC layer; (ii) testing whether the Asterinas smoltcp→Netstack3 migration debate had resolved against smoltcp (which would have weakened §4.11) — it resolved against Netstack3, strengthening smoltcp's position; (iii) checking whether BBRv3 had landed mainline (which would have refuted §4.9's CUBIC default) — confirmed not mainline; (iv) checking whether HEv3 had advanced to RFC publication (which would have triggered §6 stop condition on §4.13 IETF delta) — still at draft-03 with no movement; (v) searching for production OS short-circuit-loopback bug postmortems (which would have refuted §4.6) — none surfaced. The 5 REFINABLE verdicts and the 1 CONSIDER-REVISITING represent where the pressure landed; the 8 CONFIRMED represent positions whose adversarial reading did not surface 2025–2026 evidence sufficient to revise the verdict bucket. The dispatch §6 anti-goal caps CONSIDER-REVISITING per line at one without strong primary-source backing; this artifact respects that cap. PM should weight CONFIRMED dispositions as "adversarial pass did not surface refuting evidence within budget" rather than "no refuting evidence exists."

---

## 2. Per-Line Findings

The 14 lines of inquiry from dispatch §4 are addressed in order. Each subsection contains: a brief restatement of PM's choice, the strongest counterargument primary sources surface, the evidence, and a pressure-test verdict.

### 2.1 Sockets-equivalent API shape — layered handle vs alternatives

**PM choice (THINK §3.1).** Layered handle: outer `NetworkAccess` capability handed at spawn (or inherited via IPC capability-passing per Bible Article 12); inner per-socket handles created via `sys_net_socket_udp(NetworkAccess) -> Result<UdpSocket>`, `sys_net_socket_tcp(NetworkAccess) -> Result<TcpSocket>`, `sys_net_listen_tcp(NetworkAccess, bind_spec) -> Result<TcpListener>`. Forward-compatibility with HEv3 protocol-priority by taking `&[NetCandidate]` rather than `(addr, port)`. Endpoint, channel, connection-shaped, and Tock typed-capability poles dismissed.

**Strongest counterargument.** Fuchsia is the closest analog (Rust, capability-shaped, multi-process), and Fuchsia's `TENETS_AND_DESIGN_DECISIONS.md` for Netstack3 emphasizes trait-based API boundaries with context traits that "explicitly spell out their dependencies," interior-mutability rather than exclusive-state assumptions, and atomic event-driven state exposure. The document does not retrospect on the FIDL-channel-via-component-manifest shape itself; it presents the shape as a current design tenet. The published Netstack3 `IMPROVEMENTS.md` names one structural regret — the `DeviceId` field layout forcing protocol-specific HashMap lookups, where they would prefer "top 32 bits to identify the type of device, bottom 32 bits to identify the device itself" for unified storage — but this regret lives at the kernel-internal device-identifier layer, not at the FIDL routing layer. The absence of a published FIDL-shape postmortem is itself a data point: the largest deployed Rust capability-shaped network API in production has, after multiple years, not surfaced an API-shape regret at the layer the PM choice most closely tracks.

The Tock typed-capability friction with dynamic socket allocation is real at the kernel-allocation level: Tock's kernel "does not have a shared heap and so does not support dynamic memory allocation within the kernel; all kernel memory is statically allocated. When device memory is statically allocated, kernel software for managing shared abstractions like radio interfaces must make a static decision about the number of requests it will support." The grant mechanism is the workaround. This friction is not directly transferable to NeoPlutarch's monolithic-kernel-with-heap architecture, but it does document a category of pain when capability-as-type meets dynamic resource provision.

The Hubris compile-time-bound owner-task model has not been exercised against the dynamic-allocation case that NeoPlutarch needs, because Hubris is a statically-configured service-processor OS with no dynamic process spawn — the PM's framing of "rejected for incompatibility with NeoPlutarch's multi-process model" is sound, but the *positive* claim that the layered-handle shape resolves the static/dynamic tension cleanly is not testified to by Hubris.

**Evidence.**

- Fuchsia Netstack3 design tenets: [Fuchsia source `TENETS_AND_DESIGN_DECISIONS.md`](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/TENETS_AND_DESIGN_DECISIONS.md).
- Fuchsia Netstack3 improvements document: [Fuchsia source `IMPROVEMENTS.md`](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/IMPROVEMENTS.md).
- Tock kernel static-allocation discipline: [Tock OS Wikipedia, citing primary docs](https://en.wikipedia.org/wiki/Tock_(operating_system)). Cannot verify a Tock-specific GitHub issue or mailing-list thread naming the typed-capability + dynamic-allocation tension at the *network* layer from primary sources; the grant-mechanism workaround is documented at the general-resource-allocation layer.
- Hubris UDP-only state: [Hubris `task/net` source tree](https://github.com/oxidecomputer/hubris/tree/master/task/net), explicit README quote "we currently only support IP and UDP."
- Cannot verify a published Netstack3 retrospective at the FIDL-routing-shape layer from primary sources. The netstack.fm episode with Bruno Dal Bo Silva (Staff Engineer, Fuchsia) on 2025-10-07 contains chapters titled "comparing netstack3 with smoltcp" and "unsafe code usage" that may discuss API-shape considerations, but the episode is audio-only and content cannot be confirmed without transcription. [netstack.fm episode index](https://podcasts.apple.com/us/podcast/fuchsias-netstack3-with-bruno-dal-bo-silva/id1833932472?i=1000730511978).

**Pressure-test verdict: REFINABLE.** PM choice is structurally aligned with documented practice. The framing refinement is to acknowledge that the layered-handle shape's forward-compatibility with TCP is an engineering claim that no existing analog (Fuchsia at FIDL layer, Hubris UDP-only, Tock static-allocation, Plan 9 namespace) directly testifies to from the static→dynamic axis the PM choice navigates. THINK §3.1's forward-accommodate-UDP-and-TCP claim is best framed as a hypothesis that BUILD-3 will validate, not a precedent-supported certainty.

### 2.2 Sub-BUILD sequencing — UDP-first / TCP-third / TLS-fifth

**PM choice (THINK §4).** Phase 4 ships in five sub-BUILDs: BUILD-1 (IPv6+UDP+virtio-net+capability shape) → BUILD-2 (IPv4+DHCPv4) → BUILD-3 (TCP+CUBIC+RFC5961+RFC7323) → BUILD-4 (DNS+Happy Eyeballs) → BUILD-5 (kTLS-hybrid TLS = Phase 4 ship). IPv6-before-IPv4 is the inverse of the historical "IPv4 first, IPv6 retrofit" pattern. TLS lands last in Phase 4.

**Strongest counterargument.** Among the surveyed clean-room kernel projects, no primary-source documents the inverse-ordering (IPv6 before IPv4) for a general-purpose dual-stack endpoint OS. smoltcp itself — the candidate stack NeoPlutarch's Phase 4 builds on — added IPv6 in PR #72 *after* IPv4 was already working; maintainers explicitly requested "a simple, clean version of IPv6 basics first before considering [extension headers and multicast]." Redox OS integrated networking by adopting smoltcp wholesale via PR #845, inheriting whatever protocol coverage smoltcp had at that moment rather than implementing protocols in any order themselves. Hubris is UDP-over-IPv6 with ICMPv6 only — in a controlled L2-isolated service-processor network, not a general-purpose Internet-facing stack. LionsOS ships lwIP as an out-of-kernel component over sDDF; the kernel itself (seL4) provides no stack and no protocol-ordering signal. Asterinas 0.16.0 and 0.17.0 release notes document socket-API expansion (UNIX, netlink, SOCK_SEQPACKET, sendmmsg, SO_BROADCAST) but do not disclose IPv4-vs-IPv6 ordering for the underlying transport. Theseus's network surface remains unverified from primary source.

The framing where this finding matters is not "the PM is wrong to do IPv6-first" — the dual-stack-mandatory PM-NOTE position (a) is confirmed, IPv6 is the modern default per RFC 8504, and there is no normative obstacle to either ordering. The framing where this matters is that the comparison set offers no published precedent for what to expect during BUILD-1→BUILD-2 IPv4 retrofit: whether the IPv6-first protocol code naturally leaves room for IPv4's ARP, source-routing-table behavior, and IPv4-specific RFC 6724 destination-selection rules, or whether a more invasive refactor is needed when IPv4 lands. THINK §4 already names this implicitly in §6 P5 ("no active Phase 3 unresolved blockers"), but the absence of a precedent suggests the BUILD-1→BUILD-2 transition is a place where PM should expect surprise.

**Evidence.**

- smoltcp IPv6-after-IPv4 history: [smoltcp PR #72](https://github.com/smoltcp-rs/smoltcp/pull/72) — IPv6 added *after* IPv4; maintainer-requested "simple, clean version of IPv6 basics first."
- Redox adoption of smoltcp wholesale: [redox-os/redox PR #845](https://github.com/redox-os/redox/pull/845) "use smoltcp as networking stack."
- Hubris UDP-over-IPv6 + ICMPv6 only, controlled L2: [Hubris README](https://github.com/oxidecomputer/hubris) and [RFD 250](https://rfd.shared.oxide.computer/rfd/0250).
- LionsOS out-of-kernel stack (no in-kernel sequencing precedent): [LionsOS paper arXiv 2501.06234](https://arxiv.org/pdf/2501.06234).
- Asterinas 0.16.0 and 0.17.0 release notes (no IPv4/IPv6 ordering disclosed): [asterinas.github.io announcement 0.17.0](https://asterinas.github.io/2025/12/19/announcing-asterinas-0.17.0.html).
- Cannot verify Theseus network-stack ordering from primary source within research budget. [theseus-os/Theseus](https://github.com/theseus-os/Theseus).

**Pressure-test verdict: REFINABLE.** PM choice is normatively sound and not refuted by any precedent. The pressure-test refinement is that no documented analog testifies to the IPv6-first path, and BUILD-1→BUILD-2 should be planned with the expectation that some IPv6-shaped code may need restructuring when IPv4 lands, rather than the easier expectation of an additive overlay. Forward-marker: if PLAN's smoltcp wrap choice (THINK R1) inherits smoltcp's IPv6-after-IPv4 internal architecture, the BUILD-1 commits will already have IPv4 plumbing latent; if PLAN chooses fork or from-scratch, BUILD-1→BUILD-2 may surface integration friction that no precedent has documented.

### 2.3 kTLS-hybrid placement and current state of rustls-ktls

**PM choice (THINK §3.3).** Hybrid kTLS-style at BUILD-5: userspace owns the TLS handshake (rustls in libneo or equivalent); kernel takes ownership of the symmetric record layer via `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)`. Matches the Linux kTLS pattern as the only "TLS in the kernel" production reference. Composes with Bible Article 11.3.

**Strongest counterargument.** The "production-deployed reference" framing is structurally true and tonally understated. Linux `net/tls` has shipped seven distinct CVEs across 2024–2025 with a recurring failure-mode profile:

| CVE | Code path | Failure-mode class |
|---|---|---|
| CVE-2025-39946 | `tls_rx_msg_size()` parser | Out-of-bounds; 1-of-3 callsites failed to abort parser on invalid header. Fixed in 6.12.49. |
| CVE-2025-38608 | bpf + ktls plaintext-to-ciphertext | Ciphertext length not recalculated after plaintext truncation; transmits uninitialized buffer. |
| CVE-2025-40176 | Kernel TLS async decryption | Hold/wait path in async-decrypt-completion. |
| CVE-2024-26582 | net/tls async crypto | Double-free in async-crypto completion callback. |
| CVE-2024-26583 | net/tls async crypto | UAF in async-crypto completion path. |
| CVE-2024-26584 | net/tls async crypto | Race condition between completion and retry. |
| CVE-2024-26585 | net/tls async crypto | Backlog-queue interaction with partial-read zero-copy. |

Independent post-mortem analysis of the 2024 cluster characterizes the root pattern as "intricate interactions between asynchronous workqueue scheduling, backlog queue management in the crypto subsystem, partial read handling with zero-copy mechanisms, and callback-based decryption completion." This is the reference shape PM chose. Choosing it inherits the reference's complexity profile in addition to its production legitimacy.

The rustls-ktls crate naming: there is no published crate named `rustls-ktls`. The canonical crates under the rustls organization are `ktls` (latest `v6.0.2`, April 7, 2025), `ktls-core`, `ktls-sys` (`v1.0.2`, September 26, 2024), and `ktls-stream`. The `ktls` repository has 90 stars and 15 forks; the `ktls` crate has ~28K monthly downloads, trending up. Three of its dependencies are flagged "obsolete" or "outdated" on lib.rs (nix, socket2 — obsolete; thiserror, rcgen — outdated). The recommendation in the docs is "use the latest Linux kernel, at least 6.6 LTS, for better support of kTLS."

The 2025–2026 academic literature does not surface a paper proposing a different kernel-TLS pattern beyond the Linux kTLS model. NVIDIA documents hardware-offload extensions (ConnectX/BlueField) that build on the Linux kTLS interface rather than replacing it.

Fuchsia Netstack3's TLS story is not documented in the surfaced Netstack3 docs directory; the Netstack3 documentation index covers control flow, dual-stack sockets, packet parsing, POSIX compatibility, and benchmarks, but no TLS module is listed. Whether Fuchsia's TLS story is "via Fuchsia-component-userspace" or "absent at the Netstack3 layer" cannot be confirmed from the surfaced material. Asterinas does not document a TLS pattern in release notes 0.16.0 or 0.17.0 or in the roadmap.

**Evidence.**

- CVE-2025-39946 analysis: [faith2dxy.xyz kCTF analysis 2025-10-02](https://faith2dxy.xyz/2025-10-02/kCTF-TLS-nday-analysis/).
- CVE-2025-38608: [SUSE security advisory CVE-2025-38608](https://www.suse.com/security/cve/CVE-2025-38608.html).
- CVE-2025-40176: [OpenCVE linux_kernel index](https://app.opencve.io/cve/?product=linux_kernel&vendor=linux).
- CVE-2024-26582/3/4/5 analysis: [u1f383.github.io 2025-01-21 Linux kernel TLS part 2](https://u1f383.github.io/linux/2025/01/21/linux-kernel-tls-part-2.html).
- ktls crate releases: [rustls/ktls releases](https://github.com/rustls/ktls/releases).
- ktls crate download/dependency signal: [lib.rs/crates/ktls](https://lib.rs/crates/ktls), [crates.io/crates/ktls-core](https://crates.io/crates/ktls-core).
- Linux kTLS documentation: [docs.kernel.org/networking/tls.html](https://docs.kernel.org/networking/tls.html), [docs.kernel.org/networking/tls-offload.html](https://docs.kernel.org/networking/tls-offload.html).
- NVIDIA kTLS hardware-offload: [docs.nvidia.com kTLS Offloads](https://docs.nvidia.com/doca/sdk/ktls-offloads/index.html).
- Netstack3 docs index (no TLS listed): [Fuchsia Netstack3 docs](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/). Cannot verify Netstack3 TLS placement directly.
- Cannot verify exact LoC count of Linux `net/tls/` from primary source within this research budget; would require direct `wc -l` on kernel.org `net/tls/` tree.
- Cannot verify Asterinas TLS pattern from primary source; not in release notes or roadmap.

**Pressure-test verdict: REFINABLE.** PM choice is structurally sound and matches current production practice. The pressure-test refinement is to acknowledge in THINK that "matches Linux kTLS" inherits the documented failure-mode profile, and that BUILD-5's late placement minimizes the exposure window. The framing "the hybrid is the production answer" is correct; the framing where PM may want refinement is to make explicit that the kernel-side complexity NeoPlutarch will absorb at BUILD-5 is the same complexity that has produced seven CVEs in the Linux reference across 24 months — and that ADR-022's cell-swap forward marker (THINK §10 references) is exactly the failure-recovery shape this complexity class historically benefits from.

### 2.4 Kernel-resident DNS resolver

**PM choice (THINK §3.4).** Kernel-resident stub resolver at BUILD-4, coupled to Happy Eyeballs. RFC 1035 query format, RFC 6891 EDNS(0), RFC 8106 RDNSS from RAs. Reachable via `sys_net_resolve(name, family_hint) -> Result<&[NetCandidate]>`. Forward markers (DoT, DoH, DNSSEC) explicitly deferred.

**Strongest counterargument.** Linux has historically not put a *full* DNS resolver in the kernel. The narrow exception is `CONFIG_DNS_RESOLVER` — a keyring-based mechanism since 2.6.36, used by CIFS and AFS, which upcalls to `/sbin/request-key` in userspace rather than resolving in-kernel. No surfaced LKML thread documents an explicit "full in-kernel resolver proposed and rejected" debate; the observable pattern is that production resolver logic has always lived in userspace (systemd-resolved, NetworkManager, nscd, dnsmasq) without a competing kernel proposal entering tree. Fuchsia's legacy Go netstack (Netstack2) carried a `dns` package as part of the netstack itself; Netstack3's documentation index does not list DNS as a Netstack3 component — a possible shift to a userspace component, though this cannot be confirmed from surfaced material.

The 2025–2026 IETF and OS deployment direction is encrypted-DNS-by-default at the client OS:

- **RFC 9461** (Service Binding Mapping for DNS Servers, November 2023) standardizes SVCB records so clients can discover encrypted-transport endpoints.
- **RFC 9462** (Discovery of Designated Resolvers, November 2023) defines client-side discovery of encrypted resolvers.
- **draft-ietf-add-encrypted-dns-server-redirection** (EDSR) — latest revision 2025-11-04 (now marked Expired); the mechanism forbids clients from following a redirection and then sending unencrypted traffic to the new resolver.
- **draft-ietf-add-requirements** — requirements for discovering designated resolvers.
- Mozilla reports DoH adoption above 85% among U.S. Firefox users by 2025–2026.
- A January 2025 U.S. Executive Order mandates DNS encryption for federal systems.
- Operator-side blocking: industrial networks routinely block port 53 (UDP/TCP) to non-approved servers, port 853 (DoT), and ports 784/8853 (DoQ); DoH on 443 cannot be blocked without breaking HTTPS broadly. Consumer-carrier (residential ISP) port-53 blocking is less documented in surfaced sources; the surfaced material covers enterprise/industrial blocking and OS-level DoH deployment, not consumer-carrier behavior at scale.

No surfaced IETF draft mandates DoT/DoH from endpoint OSes as a normative requirement; the work centers on *discovery* (RFC 9461/9462) and *redirection* (EDSR) of encrypted resolvers when a client chooses to use them.

**Evidence.**

- Linux `CONFIG_DNS_RESOLVER` keyring-based shim with `/sbin/request-key` upcall: [docs.kernel.org/networking/dns_resolver.html](https://docs.kernel.org/networking/dns_resolver.html), [LWN article 2010 archive](https://lwn.net/Articles/399546/).
- Fuchsia Netstack2 Go `dns` package: [pkg.go.dev .../netstack/dns](https://pkg.go.dev/go.fuchsia.dev/fuchsia/src/connectivity/network/netstack/dns).
- Fuchsia Netstack3 docs index (no DNS module listed): [Fuchsia Netstack3 docs](https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/).
- RFC 9461: [datatracker rfc9461](https://datatracker.ietf.org/doc/html/rfc9461).
- RFC 9462: [rfc-editor rfc9462](https://www.rfc-editor.org/rfc/rfc9462.html).
- EDSR draft: [datatracker draft-ietf-add-encrypted-dns-server-redirection](https://datatracker.ietf.org/doc/draft-ietf-add-encrypted-dns-server-redirection/).
- ADD requirements: [draft-ietf-add-requirements-00](https://www.ietf.org/archive/id/draft-ietf-add-requirements-00.html).
- Firefox DoH adoption signal: [captaindns.com 2025 analysis](https://www.captaindns.com/en/blog/doh3-doq-dot-2025-latest).
- January 2025 U.S. EO + enterprise DoT acceleration: [mr-alias.com DNS privacy guide](https://mr-alias.com/articles/dns-privacy-guide.html).
- Industrial port-53/853/784/8853 blocking patterns: [industrialmonitordirect.com encrypted-DNS blocking](https://industrialmonitordirect.com/blogs/knowledgebase/blocking-encrypted-dns-in-industrial-networks-doh-dot-and-quic-bypass-prevention).
- Cannot verify Netstack3 DNS placement (kernel vs userspace component) from primary source within research budget.
- Cannot verify consumer-carrier (residential ISP) port-53 blocking statistics for 2025–2026 from primary source; surfaced material covers enterprise/industrial blocking only.

**Pressure-test verdict: REFINABLE.** PM choice for BUILD-4 (stub resolver, port 53 only, RFC 1035 + EDNS(0) + RDNSS) is functionally sufficient for the BUILD-4 Happy-Eyeballs ship milestone and for the Phase 4 ship milestone (BUILD-5 HTTPS to a real server) in residential dual-stack contexts. The pressure-test refinement is that the 2025–2026 direction of travel (RFC 9461/9462, EDSR, U.S. EO on federal DNS encryption, Firefox >85% DoH) suggests "DoT/DoH as forward marker" is more load-bearing than THINK §3.4's current framing implies. Independent question, not pressure-tested here: whether Phase 5+ AI-subsystem authenticated model downloads (Bible Article 11.3 scope) need DoT/DoH on the path between client and resolver, separate from TLS on the path between client and origin. THINK §3.4 leaves this for Phase 5+; the pressure-test does not contradict the deferral but flags that "stub resolver only" carries a forward-marker dependency that is tighter than the typical Phase 5+ horizon.

### 2.5 BUILD-1 transport scope — UDP only, no TCP

**PM choice (THINK §3.2).** UDP and ICMPv6 at BUILD-1. TCP at BUILD-3. Rationale: RFC 9293 (TCP) is large surface area (sequence numbers, window scaling, RTT estimation, retransmission, congestion control, RFC 5961 blind-injection mitigations, RFC 7323 extensions); UDP exercises the same packet pipeline at a fraction of the protocol surface, validating the architecture before committing to TCP correctness on it.

**Strongest counterargument.** The Hubris precedent — UDP+ICMPv6 only, no TCP — is the closest analog and has *not* been pressure-tested by adding TCP later. Hubris remains UDP-only as of this investigation; RFD 250 does not name a TCP roadmap. The opposite analog, Asterinas, did not stage UDP-first; Asterinas inherited smoltcp's full TCP+UDP coverage on adoption. Redox's smolnetd history shows TCP and UDP were *staged* — "in an early stage of development, the previous separate daemons (ipd, udpd, and tcpd) were removed and everything was contained in ethernetd, though TCP sockets were not yet implemented" — and new scheme methods `sockrecv`/`socksend` were added because UDP didn't compose with pure read/write semantics. The Redox UDP-first staging is the closest documented analog to PM's choice, and it surfaces the API-shape question Redox encountered when UDP needed semantics that pure read/write couldn't carry. Redox's response was to add new scheme methods rather than revise existing ones — supportive of the "additive, not revisionary" framing PM relies on.

The smoltcp release log between 2026-05-20 and 2026-05-22 contains no TCP-related fixes or feature additions. Cerebro's OBSERVE-2026-05-20 §3.1 noted that smoltcp recently added SACK; no post-OBSERVE SACK or TCP-related events occurred in the 2-day window. The v0.13.0 release (March 20, 2025) included TCP zero-window-probe and retransmit fixes — substantive but already absorbed into the OBSERVE substrate.

**Evidence.**

- Hubris UDP-only state: [Hubris `task/net`](https://github.com/oxidecomputer/hubris/tree/master/task/net), explicit README "we currently only support IP and UDP, with a focus on IPv6."
- Asterinas inheriting smoltcp full coverage: [Asterinas USENIX ATC 2025 paper arXiv 2506.03876](https://arxiv.org/html/2506.03876v1), "Asterinas uses the smoltcp library, which lacks congestion control."
- Redox UDP-first staging with `sockrecv`/`socksend` additive scheme methods: [redox-os/redox PR #845](https://github.com/redox-os/redox/pull/845), [smoltcp issue #50 Redox integration](https://github.com/smoltcp-rs/smoltcp/issues/50).
- smoltcp v0.13.0 release notes (March 20, 2025): [smoltcp releases](https://github.com/smoltcp-rs/smoltcp/releases).
- Cannot verify, in surfaced sources, a documented case where a project shipped UDP-only and then was forced to revise the public API shape to accommodate TCP. Absence of evidence is not evidence of absence; the closest precedent (Redox) demonstrates the *additive* path (new scheme methods) without revising existing ones.

**Pressure-test verdict: CONFIRMED.** PM choice is well-supported by the Redox precedent (additive scheme methods for UDP-specific semantics) and not contradicted by any surfaced analog. The unevidenced claim "the layered-handle shape forward-accommodates TCP without revision" (THINK §3.1) becomes a BUILD-3 hypothesis-to-validate rather than a precedent-supported certainty (see §2.1 above); the BUILD-1 commitment to UDP-only is sound regardless.

### 2.6 Loopback short-circuit

**PM choice (THINK §3.7).** Short-circuit at the IP layer: when a packet is sent to `::1` or `127.0.0.1`, the kernel short-circuits at the IP layer; the packet is delivered to the destination socket's RX queue without going through driver, MAC, or routing logic. The semantics from userspace are identical to traversing a real interface (same packet content, same metadata except interface ID), but the path is internal.

**Strongest counterargument.** Linux's loopback is *not* short-circuited at the IP layer; `drivers/net/loopback.c` is a virtual netdev driver whose packets travel through the standard `netif_rx` input path. The practical consequence is that loopback traffic *does* traverse the INPUT chain — the canonical iptables rule `iptables -A INPUT -i lo -j ACCEPT` is required for many setups and is documented in the netfilter HOWTO. Linux's choice is not "go all the way to MAC and back" but rather "virtual driver, in the input path, with hooks intact." NeoPlutarch's short-circuit-at-IP-layer is more aggressive than Linux's; it bypasses everything from the link layer up to the IP delivery hook.

The smoltcp ecosystem's history with loopback is informative. smoltcp historically *lacked loopback entirely* — connecting to 127.0.0.1 triggered ARP flooding (smoltcp issue #50, in the Redox integration thread). Redox's `LoopbackDevice` is a workaround in the link layer rather than a short-circuit at the IP layer. The smoltcp situation gives the PM choice a small clearance: NeoPlutarch is doing something smoltcp's upstream API did not provide for, but the choice (short-circuit) avoids the ARP-flooding pitfall that smoltcp's absence-of-loopback caused.

No CVE or errata directly attributes a bug class to short-circuit loopback semantics in the surfaced material. FreeBSD's `set skip on lo0` is intentional bypass (configuration semantics), not a documented short-circuit pitfall. Common pitfalls named in the dispatch — firewall rules bypassing loopback (NeoPlutarch has no firewall yet), packet-tap diagnostics broken (NeoPlutarch has no tcpdump-equivalent yet), MTU-larger-than-interface semantics — are all real categories of risk but not documented as load-bearing bugs in production OS history within this investigation.

Fuchsia Netstack3's loopback implementation is not specified in the surfaced docs; would require source-level inspection of `src/connectivity/network/netstack3/core/`. Plan 9 loopback semantics were not located within research budget.

**Evidence.**

- Linux loopback routed through `netif_rx`: [netfilter HOWTO §7](https://www.netfilter.org/documentation/HOWTO/packet-filtering-HOWTO-7.html), [DigitalOcean iptables essentials](https://www.digitalocean.com/community/tutorials/iptables-essentials-common-firewall-rules-and-commands).
- smoltcp historical absence of loopback / ARP flood: [smoltcp issue #50](https://github.com/smoltcp-rs/smoltcp/issues/50).
- Joyride Linux-stack-redesign paper treats routed-loopback as baseline assumption: [arXiv 2509.25015](https://arxiv.org/pdf/2509.25015).
- Cannot verify a canonical LKML "why loopback is routed" thread from primary source.
- Cannot verify Fuchsia Netstack3 loopback delivery model from primary source within budget.
- Cannot verify Plan 9 loopback semantics from primary source within budget.

**Pressure-test verdict: CONFIRMED.** PM choice is sound for BUILD-1's scope and benefits from the smoltcp-ecosystem-context (the smoltcp ARP-flood pitfall is avoided by short-circuiting before link layer). The forward markers PM should be aware of: (a) when firewall or filter-table machinery lands (Phase 8+ per THINK §6), short-circuit loopback will need an explicit decision about whether filter rules apply on the loopback path; (b) when a tcpdump-equivalent lands, the short-circuit path will need a tap point or the diagnostic surface degrades on loopback specifically; (c) MTU semantics on loopback may legitimately differ from real interfaces; PM should document that as intentional in BUILD-1d. None of these are immediate risks but all become real at later Phase milestones.

### 2.7 SLAAC + Privacy Extensions interaction with capability-handle lifetime

**PM choice (THINK §3.10).** RFC 8981 Privacy Extensions enabled by default at BUILD-1. Default lifetimes per RFC 8981: TEMP_VALID_LIFETIME 2 days; TEMP_PREFERRED_LIFETIME 1 day; TEMP_IDGEN_RETRIES 3; MAX_DESYNC_FACTOR 0.4 × preferred. User override capability deferred (no userspace control surface yet; can land in BUILD-4+).

**Strongest counterargument.** RFC 8981 enables temporary addresses that rotate (TEMP_PREFERRED_LIFETIME = 1 day default). A long-lived UDP socket holding a `UdpSocket` capability handle may have been bound to a temporary address that, after a day, is deprecated and after another day is removed. THINK §3.10 specifies the lifetimes but does not specify the kernel-side behavior when a capability-bound socket's source address is rotated. The documented production pattern across Linux, Windows, macOS, iOS, Android — surfaced via RFC 8981 implementation discussions — is two-part: (1) applications can receive a management event when an address moves to deprecated state, so listeners can act on it by closing the socket bound to that source; (2) the OS uses IP-address reference counting, so a deprecated address is not removed while a socket still references it. Both of these patterns require API surface that THINK §3.10 does not enumerate and that does not appear elsewhere in THINK §3's syscall list (no `sys_net_subscribe_address_events` or equivalent; no statement about reference-counted address retention while a `UdpSocket` capability is open).

The interaction with the capability-handle model is the specific NeoPlutarch concern. In Linux, the deprecation/reference-counting machinery operates over file descriptors and the network namespace. In NeoPlutarch's layered-handle model, the equivalent question is: when a temporary address is deprecated and the OS would normally remove it, does the kernel network service consult the capability table to find all `UdpSocket` handles bound to that address? If yes, the reference-counting pattern composes with capabilities cleanly. If no — if address rotation is independent of the capability table — then either capability-bound sockets break silently when their source rotates, or the kernel must add a new machinery layer.

This is not a refutation of the privacy-by-default choice; it is a missed risk in the BUILD-1 design that the kernel must answer before BUILD-1 ships, because Privacy Extensions are enabled by default and the rotation timing (1 day preferred) will fire during normal BUILD-1 testing if test sessions span midnight UTC.

**Evidence.**

- RFC 8981 §3.6 ("Configuration Variables") and §3.7 ("Deployment Considerations") on lifetime defaults and "user override" requirement: [rfc-editor rfc8981](https://www.rfc-editor.org/rfc/rfc8981.html).
- Modern OS default-enabled state (Windows, macOS, iOS, Android): documented in multiple sources; representative summary at [oneuptime.com 2026-03-20 SLAAC privacy](https://oneuptime.com/blog/post/2026-03-20-slaac-privacy-extensions/view), corroborated by the Zephyr RTOS implementation discussion at [zephyrproject-rtos/zephyr PR #71996](https://github.com/zephyrproject-rtos/zephyr/pull/71996).
- Reference-counting + deprecation-event pattern: documented in implementation surveys; representative at [makikiweb.com IPv6 SLAAC RFC 8981](http://www.makikiweb.com/ipv6/slaac_and_rfc8981.html).
- Cannot verify Fuchsia Netstack3 or smoltcp-specific implementation of the deprecation-event surface from primary source within research budget.

**Pressure-test verdict (split — two distinct concerns):**

- **§4.7a — RFC 8981 Privacy Extensions default-on (network-policy choice): CONFIRMED.** Aligned with current BCP and modern OS practice (Windows, macOS, iOS, Android default-on; Linux default varies). RFC 8504 SHOULD posture. PM choice in THINK §3.10 (privacy enabled by default with BCP-aligned lifetimes) is sound on the policy axis.
- **§4.7b — Capability-handle lifetime across temporary-address rotation (OS primitive correctness): CONSIDER-REVISITING.** Distinct concern class from §4.7a; this is not a network policy choice but a missing piece of the OS capability model. THINK §3.10 does not specify the kernel-side behavior when a `UdpSocket` capability is bound to a temporary address that gets deprecated and removed under default 1-day TEMP_PREFERRED_LIFETIME. The documented production pattern (deprecation event + IP-address reference counting) requires API surface that does not appear in THINK §3's syscall list. This is a localizable gap, not a structural fault — but it is a primitive-correctness concern that warrants its own disposition rather than deferral at network-policy priority.

PM may wish to surface §4.7b as an explicit BUILD-1 design question — either (a) extend THINK §3.10 with a statement on capability-bound-socket behavior across address rotation, or (b) add a new BUILD-1 sub-commit (BUILD-1.1 or similar) that defines the address-deprecation-event surface (a notification capability the network service posts when an address moves to deprecated state) and the socket-binding-retention semantics (a deprecated address is not removed while a `UdpSocket` capability bound to it is held). Whichever path, the test plan for BUILD-1e should include a >24h session to verify Privacy Extensions rotation does not silently break long-lived sockets.

### 2.8 Notification-on-activity vs io_uring scaling

**PM choice (THINK §3.14).** Notification-on-activity: process holding `UdpSocket`/`TcpSocket` receives kernel notification (existing Phase 1-2 notification primitives) on RX-bytes-available, connection-state-change, send-side-capacity-available. Block via `sys_net_wait` or poll via `sys_net_poll`. io_uring-style completion-based I/O named as forward marker.

**Strongest counterargument.** Linux io_uring's 2025–2026 evolution (kernel 6.13 added hybrid IO polling as a variant of strict IOPOLL with initial sleep delay; ring resizing for grow-as-needed; fixed wait regions, partial buffer clones, mapped regions, sync messages between rings) indicates the design space for completion-based I/O is still actively settling. The PM's deferral to Phase 8+ is consistent with the maturity signal: io_uring is not stable enough at the API layer that a fresh implementation should commit to a specific shape in 2026. The Hubris notification-on-activity precedent (PM cites this) operates at very modest scale; THINK §3.13's per-process limit of 64 sockets is well within the scale at which notification-on-activity has no documented bottleneck.

No surfaced 2025–2026 data places notification-on-activity at a specific fan-out scale where it bottlenecks. The general inference, supported by industry deployment of io_uring for high-throughput cases (10K+ sockets per process, common in modern proxy and database workloads), is that completion-based I/O matters at 4–5 orders of magnitude beyond Phase 4 BUILD-1's commitment. The Fuchsia Netstack3 docs do not surface a completion-ring API; Netstack3's interaction model remains FIDL-channel-based per the published design tenets.

**Evidence.**

- io_uring 6.13 hybrid polling and ring resizing: [Phoronix Linux 6.13 io_uring](https://www.phoronix.com/news/Linux-6.13-IO_uring).
- Modern io_uring architecture (shared ring buffers, kernel/userspace shared memory): [kernel.dk io_uring PDF](https://kernel.dk/io_uring.pdf), [Wikipedia io_uring](https://en.wikipedia.org/wiki/Io_uring), [man7 io_uring(7)](https://man7.org/linux/man-pages/man7/io_uring.7.html).
- Hubris notification-on-activity precedent: [Hubris README](https://github.com/oxidecomputer/hubris).
- Cannot verify a specific 2025–2026 measurement of the fan-out scale at which notification-on-activity bottlenecks from primary source.

**Pressure-test verdict: CONFIRMED.** PM choice is well-supported by the Hubris precedent and the per-process-64-socket BUILD-1 scope. The forward-marker deferral to Phase 8+ is reinforced by io_uring's ongoing API evolution in 2025–2026 — committing now to a specific completion-ring shape would be premature.

### 2.9 CUBIC vs BBRv3 default congestion control

**PM choice (THINK §3.12).** CUBIC at BUILD-3 ship. Pluggable architecture deferred. Reno/NewReno rejected (underperforms on high-BDP), BBRv1 rejected (fairness issues with CUBIC on shared links), BBRv3 named as forward marker (experimental; not yet mainline).

**Strongest counterargument.** BBRv3 remains *not in mainline Linux* as of May 2026. Confirmed by multiple primary sources: the Google `bbr-dev` mailing list states explicitly that "net-next and mainline Linux trees still have BBR 1.x, not BBRv3" (March 2025); a Hacker News discussion in August 2025 affirms the same; Phoronix coverage describes BBRv3 as "will be upstreamed" without documenting a merge event. Google's `google/bbr` repository v3 branch had its most recent significant push around March 18, 2025 (README and iproute2 patches), with no mainline-merge commits since. The IETF BBR draft progression: `draft-ietf-ccwg-bbr-05` published 2 March 2026, intended status **Experimental** (not Standards Track), replacing the earlier `draft-cardwell-ccwg-bbr`. The CCWG document state is WG-document but pre-publication.

Independent 2025 peer-reviewed evaluations: IFIP Networking 2025 ("Insights into BBRv3's Performance and Behavior by Experimental Evaluation"), arXiv 2505.07741 ("BBR's Sharing Behavior with CUBIC and Reno"), arXiv 2510.22461 ("Should BBR be the default TCP Congestion Control Protocol?"), and a 2024 ScienceDirect study on wired broadband. Consistent findings: BBRv3 demonstrates real throughput improvements on high-BDP shallow-buffer paths but exhibits **unfairness to loss-based flows on deep-buffer, variable-RTT paths without FQ-CoDel/CAKE queue discipline**. CUBIC fairness degrades on deep-buffer paths without similar queue discipline — but the documented unfairness pattern differs by direction.

smoltcp's CUBIC implementation: no open CUBIC-correctness bugs in tracker. The single CUBIC-named issue (#949 "Performance drop with increasing buffer size," open since July 2024) is about buffer-size scaling, not algorithm correctness.

**Evidence.**

- BBRv3 not mainline as of May 2026: [Google bbr-dev list March 2025 thread](https://groups.google.com/g/bbr-dev/c/4vwZGw0nxdQ), [Hacker News discussion August 2025](https://news.ycombinator.com/item?id=45061454), [Phoronix Google BBRv3 Linux coverage](https://www.phoronix.com/news/Google-BBRv3-Linux), [google/bbr v3 branch](https://github.com/google/bbr/tree/v3).
- IETF draft-ietf-ccwg-bbr-05 Experimental status: [datatracker draft-ietf-ccwg-bbr](https://datatracker.ietf.org/doc/draft-ietf-ccwg-bbr/).
- IFIP Networking 2025 BBRv3 evaluation: [networking.ifip.org 2025 paper](https://networking.ifip.org/2025/images/Net25_papers/1571125683.pdf).
- arXiv 2505.07741 BBR sharing behavior: [arXiv 2505.07741](https://arxiv.org/html/2505.07741v1).
- arXiv 2510.22461 default-TCP-CC analysis: [arXiv 2510.22461](https://arxiv.org/html/2510.22461v1).
- smoltcp CUBIC tracker: [smoltcp issue #949](https://github.com/smoltcp-rs/smoltcp/issues/949).

**Pressure-test verdict: CONFIRMED.** PM choice is well-supported. CUBIC remains the broadly-deployed default; BBRv3 is not mainline and remains Experimental at IETF; the forward-marker framing is sound. Independent 2025 evaluations corroborate the trade-off PM names. smoltcp's CUBIC has no documented correctness issues. The BBRv3-as-deliberately-ahead-of-Linux argument (a "Phase 4 ships ahead of Linux defaults" framing) is unsupported by current production deployment data — most BBRv3 deployments require either Google's out-of-tree patches or community-maintained kernel forks.

### 2.10 IPv6-only BUILD-1 test target reliability

**PM choice (THINK §3).** BUILD-1 ships IPv6-only; BUILD-1e bare-metal verification is gated on IPv6 network functionality (PL pre-check, see THINK Risk R6).

**Strongest counterargument.** US IPv6 deployment as of mid-2026 is around 50% per Google's measurement; Cloudflare Radar shows 40.1% IPv6 in HTTP requests; APNIC Labs shows 43.13% IPv6-capable networks. Country variation is dramatic (France 73%, India 72%, Italy 17%). Within the US specifically, the residential dual-stack deployment pattern is the typical case. Spectrum's support pages document native IPv6 to residential customers; AT&T fiber and Verizon Fios have been rolling out IPv6 on residential fiber. Cannot verify specific deployment percentages for Spectrum, AT&T, or Verizon by region for 2025–2026 from primary source within research budget — the surfaced material confirms IPv6 availability but does not quantify region-specific coverage. PL's location (Melbourne, FL) is plausibly Spectrum or AT&T fiber — both ISPs document native IPv6 to residential customers, but regional rollout state is not documented to the granularity that confirms specific Melbourne FL coverage.

The IPv6-only BUILD-1 test target: `ipv6.google.com` and `ipv6.cloudflare.com` are perennial. No new public IPv6-only test targets surfaced in 2025–2026 worth knowing about beyond these two.

Real-hardware IPv6 quirks on consumer CPE: some routers do not send unsolicited RAs and require explicit RS solicitation; some have buggy RA flag handling. THINK R4 already names this risk and proposes mitigation (RS solicitation explicit at BUILD-1b ship, RA processing logged for diagnostic visibility via the BUILD-5b block-device log-sink). The PM mitigation is sound.

**Evidence.**

- US IPv6 ~50% per Google: [Google IPv6 statistics](https://www.google.com/intl/en/ipv6/statistics.html), [The Register on 50% milestone](https://www.theregister.com/2026/04/17/ipv6_50_percent_google/).
- Cloudflare Radar 40.1%: [Cloudflare Radar IPv6 report](https://radar.cloudflare.com/reports/ipv6).
- Spectrum native IPv6 to residential: [Spectrum IPv6 support](https://www.spectrum.net/support/internet/ipv6).
- Consumer ISP IPv6 enablement guide: [tools.forwardingplane.net ISP enable guide](https://tools.forwardingplane.net/guides/isp-enable/).
- Cannot verify specific Spectrum / AT&T Fiber / Verizon Fios IPv6 deployment percentages by region for 2025–2026; primary-source surface includes ISP documentation pages but not aggregated regional coverage data.

**Pressure-test verdict: CONFIRMED.** PM choice and THINK R4/R6 mitigations are sound. The PL pre-check (R6) for IPv6 functionality on PL's home network is the right shape; the surfaced material does not provide region-specific confirmation in advance, but Spectrum's native-IPv6 documentation and the general US 50% deployment context make the pre-check a low-risk gate rather than a high-risk blocker.

### 2.11 smoltcp wrap vs fork vs from-scratch (THINK R1 mitigation)

**PM choice (THINK §5 R1).** Decision deferred to PLAN. Three options enumerated: (a) wrap smoltcp's `Interface` in a kernel-owned scheduling shim; (b) fork smoltcp and refactor poll loop into event-driven entry points; (c) write IP/UDP/ICMPv6 layer from scratch using smoltcp only as reference.

**Strongest counterargument / supporting evidence.** smoltcp's current state and the Asterinas migration outcome update the calibration:

- smoltcp v0.13.0 (March 20, 2025) shipped substantive content: IPv6 SLAAC, TCP improvements (zero window probes, retransmit fixes, RFC compliance), raw socket enhancements, Rust Edition 2024. Prior releases v0.12.0 (November 28, 2024) and v0.11.0 (December 23, 2023). Repository state at last check: 77 open issues, 25 open PRs, 1,997 commits on main, 4.5k stars. The "minimal maintenance" characterization that originated in asterinas/asterinas#1821 (February 2025) is contested by the v0.13.0 release published one month later.
- The Asterinas smoltcp→Netstack3 migration debate **closed as "not planned"** (issue #1821). Asterinas remains on smoltcp. Documented reasons in the closure thread: significant bindings-layer workload; Netstack3 designed for single-threaded scenarios with unclear multithreading timeline; "upstream maintenance status uncertain given Fuchsia project's current trajectory."
- The Asterinas USENIX ATC 2025 paper confirms current state: "Asterinas uses the smoltcp library, which lacks congestion control, enabling TCP to operate at full speed and resulting in faster performance compared to Linux."
- Hubris's `task/net` is documented as "a small netstack based on smoltcp" — phrasing consistent with a *wrap*, not a fork or vendored modification. smoltcp is a Cargo dependency in Hubris.
- A Galois Inc. fork exists (`GaloisInc/rs_smoltcp`, marked obsolete, redirected to `GaloisInc/smoltcp` firewall branch). Cannot verify a published retrospective from a smoltcp forker.
- `netstack-smoltcp` (cavivie and automesh-network forks) wrap smoltcp for VPN-style userland use cases. No published retrospective located.

**Evidence.**

- smoltcp v0.13.0 release notes: [smoltcp releases](https://github.com/smoltcp-rs/smoltcp/releases).
- Asterinas issue #1821 closure: [asterinas/asterinas#1821](https://github.com/asterinas/asterinas/issues/1821).
- Asterinas USENIX ATC 2025 paper: [arXiv 2506.03876](https://arxiv.org/html/2506.03876v1), [ACM ATC 2025 listing](https://dl.acm.org/doi/10.5555/3768039.3768058).
- Hubris `task/net` as smoltcp-dependent wrap: [Hubris task/net](https://github.com/oxidecomputer/hubris/tree/master/task/net).
- Galois firewall smoltcp fork: [GaloisInc/rs_smoltcp](https://github.com/GaloisInc/rs_smoltcp).
- Cannot verify smoltcp time-to-merge metrics or last-commit date from primary source via WebFetch within research budget.
- Cannot verify a published retrospective from any smoltcp forker.

**Pressure-test verdict: REFINABLE.** The choice remains open at PLAN time per PM. The pressure-test refinements: (1) the "minimal maintenance" characterization is contested by the v0.13.0 release evidence; (2) Asterinas's migration debate resolving against Netstack3 *strengthens* smoltcp as the canonical Rust-native bare-metal candidate Bible Article 11.2 names — there is no live alternative gaining mindshare; (3) the Hubris wrap pattern (smoltcp as Cargo dependency, kernel-owned scheduling shim around it) is the only documented analog of the "wrap" path among comparison projects with similar concurrency posture. PLAN's choice should weigh the Hubris-wrap precedent (modest scope but proven) against the engineering question of whether NeoPlutarch's kernel-task scheduling model exposes more or fewer of smoltcp's poll-loop assumptions than Hubris's does.

### 2.12 Phase 2 capability infrastructure verification (THINK P2 prerequisite)

**PM choice (THINK §6 P2).** Prerequisite check before PLAN: read Phase 2 capability code and confirm (a) capability handing to spawned process, (b) capability passing via IPC, (c) per-handle rights bitmaps. If any missing, BUILD-1.0 lands the extension first.

**Strongest counterargument / supporting evidence.** The capability-at-spawn pattern is well-established in production OS designs:

- **seL4 capDL** is the canonical formal expression: at-boot static capability distribution from a manifest, with the entire capability distribution fixed at boot. capDL specifies which capability each component holds at boot; new components cannot acquire capabilities except by being granted them via existing capability invocation paths. The seL4 documentation site enumerates the capDL workflow, the language specification, and the python capdl library that inspects ELF files and creates paging structures.
- **Fuchsia component manifest** (`.cml` file) declares `use: [{ protocol: "fuchsia.posix.socket.Provider" }]` at component definition. Capability routing through the parent component validates that a route to this protocol exists; if not, the manifest fails to instantiate and the component never starts. The component-without-the-capability literally lacks the FIDL endpoint in its namespace.
- **Capsicum** (FreeBSD) per-fd rights bitmap: capabilities are file descriptors with restricted rights (`cap_rights_limit(2)`); operations on existing capabilities are gated by per-fd rights bitmaps. The canonical Capsicum paper (Watson et al., 2010) is the reference; no 2025–2026 retrospective paper surfaced in research.

The pattern PM names — capability handing at spawn, IPC capability-passing, per-handle rights bitmaps — is precisely the union of these production patterns. The risk in the THINK P2 prerequisite check is that the Phase 2 implementation may not yet support all three; the mitigation (BUILD-1.0 extension) is the right shape.

**Evidence.**

- seL4 capDL documentation: [seL4 capDL docs](https://docs.sel4.systems/projects/capdl/), [capDL language specification](https://docs.sel4.systems/projects/capdl/lang-spec.html), [seL4/capdl GitHub](https://github.com/seL4/capdl).
- seL4 capabilities tutorial: [seL4 capabilities](https://docs.sel4.systems/Tutorials/capabilities.html).
- Fuchsia component manifest (CML) documentation: [Fuchsia cml reference](https://fuchsia.dev/reference/cml), [Fuchsia capabilities](https://fuchsia.dev/fuchsia-src/concepts/components/v2/capabilities).
- Capsicum FreeBSD: [Capsicum 2010 paper PDF](https://papers.freebsd.org/2010/rwatson-capsicum.files/rwatson-capsicum-paper.pdf), [Capsicum FreeBSD docs](https://www.cl.cam.ac.uk/research/security/capsicum/freebsd.html).
- Cannot verify a 2025–2026 retrospective paper on Capsicum operational experience from primary source within research budget; the canonical reference remains the 2010 paper.

**Pressure-test verdict: CONFIRMED.** PM choice is well-grounded. The three documented patterns (seL4 capDL, Fuchsia component manifest, Capsicum per-fd rights bitmap) are union-of-features matching THINK P2's requirements. No new 2025–2026 pattern surfaced that NeoPlutarch should reach for. The BUILD-1.0 extension mitigation is appropriate.

### 2.13 IETF/RFC developments since 2026-05-20

**PM choice / Cerebro substrate.** Cerebro OBSERVE §11 listed the RFC corpus current as of 2026-05-20. Two days have passed. The pressure-test surveys the delta.

**Findings.**

- **draft-ietf-happy-happyeyeballs-v3:** Still at revision `-03`, dated 2 March 2026, intended status Standards Track, expires 3 September 2026. No new revision, no RFC publication, no WG state change.
- **6man WG:** Most recent activity surfaced is `draft-ietf-6man-sidlist-clarification-03` updated 2026-05-19 (one day before the OBSERVE baseline). No new drafts published in the 2026-05-20 → 2026-05-22 window.
- **v6ops WG:** No new drafts in the two-day window; latest dated drafts are 2026-05-16 and earlier.
- **HAPPY WG:** No documents updated; only listed draft is the v3-03 from March.
- **CCWG (replaces iccrg in the BBR-track work):** `draft-ietf-ccwg-bbr-05` published 2 March 2026, intended status Experimental. No new revision in the two-day window.
- **RFC editor queue:** Cannot verify state within research budget; the surfaced material did not document an event for documents Phase 4 cares about.

**Evidence.**

- HEv3 -03 status: [datatracker draft-ietf-happy-happyeyeballs-v3](https://datatracker.ietf.org/doc/draft-ietf-happy-happyeyeballs-v3/).
- 6man recent activity: [6man documents](https://datatracker.ietf.org/wg/6man/documents/).
- v6ops recent activity: [v6ops documents](https://datatracker.ietf.org/wg/v6ops/documents/).
- CCWG BBR draft state: [datatracker draft-ietf-ccwg-bbr](https://datatracker.ietf.org/doc/draft-ietf-ccwg-bbr/).
- RFC editor queue: [queue.rfc-editor.org](https://queue.rfc-editor.org/) (not re-fetched within research budget).

**Pressure-test verdict: NO MATERIAL CHANGE.** The IETF/RFC corpus current as of 2026-05-20 is current as of 2026-05-22. No event in the two-day window materially changes Phase 4's design substrate.

### 2.14 Risks PM may have missed

**PM choice (THINK §5).** Enumerates risks R1–R10 with mitigations.

**Strongest counterargument.** Six additional risks surface in primary sources that THINK R1–R10 do not enumerate. Each is summarized here and developed with mitigation suggestion in §3 below.

- **R11. IPv6 extension header receive-path attack surface.** RFC 9098 documents the operational implications of IPv6 packets with extension headers; the receive path is a documented hazard category. CVE-2026-23442 (SRv6 NULL idev dereference combining extension-header parsing + HMAC validation + routing) and CVE-2026-43501 (IPv6 RPL out-of-bounds write in source-routing feature) are recent CVEs in Linux's IPv6 extension-header processing. NeoPlutarch BUILD-1 does not ship SRv6 or RPL, but the general category (extension-header processing on the receive path) intersects BUILD-1's ICMPv6 + ND + SLAAC code path and could intersect future BUILDs.
- **R12. Linux net/tls async-crypto-state CVE cluster.** Seven distinct CVEs across 2024–2025 in Linux net/tls (see §2.3). This is the production reference shape PM cites for BUILD-5; the failure-mode profile is documented.
- **R13. SLAAC + Privacy Extensions + long-lived-socket capability lifetime.** See §2.7. Address rotation under default 1-day TEMP_PREFERRED_LIFETIME interacts with capability-bound socket retention; THINK §3.10 does not specify behavior.
- **R14. DoT/DoH 2025–2026 deployment trajectory.** See §2.4. The "port 53 only" framing carries a forward-marker dependency tighter than typical Phase 5+ horizons.
- **R15. Intel I219-V e1000e known driver quirks.** Linux's `e1000e` driver has documented DMA-buffer-overrun handling for I219 (commit `b10effb`) and a documented TSO regression that caused I219-LM to run at ~60% of advertised speed from Linux 5.8 (mid-2020) through Linux 6.3 fix. I219-LM and I219-V devices can fall into unrecovered Tx hang under stressful UDP traffic and multiple Ethernet-cable reconnections, recovered only by system reboot. Phase 4's I219-V real-real-hardware verification (THINK R3) inherits these quirks via the e1000e driver shape; the bug class is documented in production reference but not in THINK R3.
- **R16. Hubris UDP→TCP transition unevidenced (precedent gap).** See §2.1 and §2.5. PM cites Hubris as precedent for the layered-handle shape's forward-compatibility with TCP; Hubris has not added TCP. The claim "forward-accommodates TCP without API-shape revisions" is a BUILD-3 hypothesis, not a precedent-supported certainty.

**Evidence.** See per-risk evidence under §3 below.

**Pressure-test verdict: ADDITIONAL RISKS SURFACED.** Six new risks; see §3.

---

## 3. Risks Added Beyond R1–R10

Each item: risk name, mechanism, mitigation suggestion, **interaction with R1–R10** (composition-failure analysis), and trigger condition. Informational; PM authority for whether to absorb into THINK or defer.

### Interaction summary table

| New | Trigger | Amplifies / interacts with | Owner |
|---|---|---|---|
| R11 (IPv6 ext-header receive path) | Malformed Hop-by-Hop or extension header arrives at BUILD-1 IPv6 input | R3 (real-hardware quirks may surface ext-header corner cases under load); R7 (smoltcp ext-header coverage may need audit) | BUILD-1 PLAN |
| R12 (kTLS async-crypto CVE class) | BUILD-5 record-layer state corruption under concurrent crypto callbacks | R1 (smoltcp wrap-vs-fork choice constrains how kTLS interacts with scheduler); ADR-022 cell-swap revisit-trigger candidate | BUILD-5 PLAN |
| R13 (capability + address rotation) | Long-lived socket bound to temporary IPv6 address ages past TEMP_PREFERRED_LIFETIME (1 day default) | R5 (loopback short-circuit ownership invariant could face the same rotation-vs-binding question on link-local); R2 (Phase 2 capability infrastructure must support address-event subscription as a capability) | BUILD-1 PLAN (sub-commit BUILD-1.1) |
| R14 (DoT/DoH 2025–2026 trajectory) | BUILD-4 stub resolver fails silently against a port-53-blocking carrier path before BUILD-5 ships | R6 (PL pre-check should include "port 53 reaches a public resolver" alongside "IPv6 functional") | BUILD-4 PLAN |
| R15 (I219-V e1000e quirks) | Real-hardware verification on I219-V surfaces DMA-overrun or TSO regression | R3 (already names I219-V; this adds the documented quirk list) | BUILD-2b or BUILD-6 PLAN |
| R16 (Hubris UDP→TCP unevidenced) | BUILD-3 TCP API surface reveals the layered-handle shape requires revision | R1 (smoltcp wrap-vs-fork choice may absorb or amplify the API-shape revision cost) | BUILD-1 PLAN (speculative sketch); BUILD-3 PLAN (validation) |



### R11. IPv6 extension header receive-path attack surface

**Mechanism.** IPv6 extension headers (Hop-by-Hop, Routing, Fragment, Destination Options, plus protocol-specific extensions like SRv6 and RPL) are processed on the receive path. The receive path is a documented hazard category — RFC 9098 surveys operational implications, and recent Linux CVEs (CVE-2026-23442 SRv6 NULL idev dereference; CVE-2026-43501 IPv6 RPL out-of-bounds write) document the failure-mode class in production code.

**Phase 4 exposure.** BUILD-1 ships IPv6 + ICMPv6 + ND + SLAAC. The Hop-by-Hop extension header is the most relevant at BUILD-1; RFC 9673 (October 2024) loosened mandatory Hop-by-Hop processing on hosts to SHOULD-process-MAY-skip, which gives BUILD-1 a fast-path option. The remaining extension-header surface (Routing, Fragment, Destination Options) appears in later BUILDs as TCP and DNS land.

**Mitigation suggestion.** Add an explicit BUILD-1 sub-commit (BUILD-1f or similar) that documents the extension-header processing posture: SHOULD-process Hop-by-Hop per RFC 8200 + RFC 9673, deferred-with-fast-path-skip; explicit handling of Routing-type-0 (deprecated per RFC 5095) and other extension-header types that NeoPlutarch will refuse to process at BUILD-1. Audit the BUILD-1 ICMPv6 + ND receive path for the same defensive posture: validate length fields before parser advance; reject malformed option chains; log diagnostically.

**Evidence.** [RFC 9098 Operational Implications of IPv6 Extension Headers](https://www.rfc-editor.org/rfc/rfc9098.html); [CVE-2026-23442 SRv6 analysis](https://windowsforum.com/threads/cve-2026-23442-srv6-kernel-bug-null-idev-dereference-in-ipv6-routing-paths.410111/); [CVE-2026-43501 RPL OOB write analysis](https://windowsnews.ai/article/cve-2026-43501-linux-ipv6-rpl-out-of-bounds-write-patch-the-right-kernels.419270).

### R12. Linux net/tls async-crypto-state CVE cluster

**Mechanism.** Linux `net/tls` has shipped seven distinct CVEs across 2024–2025 with a recurring failure-mode profile in async-crypto state management (see §2.3 table). The root pattern: intricate interactions between asynchronous workqueue scheduling, backlog queue management in the crypto subsystem, partial read handling with zero-copy mechanisms, and callback-based decryption completion.

**Phase 4 exposure.** BUILD-5 lands the kTLS-hybrid record layer. The kernel-side complexity NeoPlutarch absorbs at BUILD-5 is the same complexity that has produced seven CVEs in the Linux reference across 24 months.

**Mitigation suggestion.** BUILD-5 design pass should explicitly audit the async-callback shape: avoid Linux's pattern of "callback fires on workqueue completion with shared mutable state" if BUILD-5 can structure the record layer as a synchronous per-packet transformation in the network task's context. The ADR-022 cell-swap forward marker (THINK §10 references) is the failure-recovery shape this complexity class historically benefits from; consider whether BUILD-5's TLS state is a candidate for ADR-022 revisit-trigger at design time.

**Evidence.** [u1f383 Linux kernel TLS analysis 2025-01-21](https://u1f383.github.io/linux/2025/01/21/linux-kernel-tls-part-2.html); [CVE-2025-39946 analysis](https://faith2dxy.xyz/2025-10-02/kCTF-TLS-nday-analysis/); [SUSE CVE-2025-38608 advisory](https://www.suse.com/security/cve/CVE-2025-38608.html).

### R13. SLAAC + Privacy Extensions + capability-bound socket lifetime

**Mechanism.** RFC 8981 Privacy Extensions enabled by default (THINK §3.10) generates temporary addresses with TEMP_PREFERRED_LIFETIME = 1 day default. A long-lived `UdpSocket` capability handle bound to a temporary address sees that address deprecated after a day and removed after another day. THINK §3.10 does not specify the kernel behavior across rotation.

**Phase 4 exposure.** BUILD-1 (UDP + capability gate) and later BUILDs are exposed. The documented production pattern across Linux, Windows, macOS, iOS, and Android uses deprecation events + IP-address reference counting; NeoPlutarch's layered-handle capability model has no documented equivalent surface in THINK.

**Mitigation suggestion.** Either (a) extend THINK §3.10 with a statement on capability-bound-socket behavior across address rotation, or (b) add a new BUILD-1 sub-commit (BUILD-1.1 or similar) that defines the address-deprecation-event surface (a notification capability that the network service posts when an address moves to deprecated state) and the socket-binding-retention semantics (a deprecated address is not removed while a `UdpSocket` capability bound to it is held). Test plan for BUILD-1e should include a >24h session to verify Privacy Extensions rotation does not silently break long-lived sockets.

**Evidence.** [RFC 8981 §3.6, §3.7](https://www.rfc-editor.org/rfc/rfc8981.html); [oneuptime SLAAC privacy 2026-03-20](https://oneuptime.com/blog/post/2026-03-20-slaac-privacy-extensions/view); [makikiweb IPv6 SLAAC RFC 8981](http://www.makikiweb.com/ipv6/slaac_and_rfc8981.html).

### R14. DoT/DoH 2025–2026 deployment trajectory

**Mechanism.** The 2025–2026 direction of travel is encrypted-DNS-by-default at the client OS: RFC 9461 (SVCB-for-DNS) and RFC 9462 (Designated-Resolver discovery) standardize the client-side machinery; EDSR (active IETF draft) defines redirection semantics; a January 2025 U.S. Executive Order mandates DNS encryption for federal systems; Mozilla reports DoH adoption above 85% among U.S. Firefox users. THINK §3.4's "stub resolver, port 53 only, forward markers (DoT, DoH, DNSSEC) explicitly deferred" framing is functionally sufficient for BUILD-4 milestones but tonally out of step with the documented client-OS trajectory.

**Phase 4 exposure.** BUILD-4 (DNS + Happy Eyeballs) ships port-53-only resolution. BUILD-5 (HTTPS to a real server) needs DNS resolution that may pass through ISP networks; if a target ISP blocks port 53 for non-approved servers (documented in industrial/enterprise contexts, less documented for residential), the resolution may fail silently.

**Mitigation suggestion.** Acknowledge the forward-marker dependency in THINK §3.4 as tighter than the typical Phase 5+ horizon. Plan a BUILD-4 sub-commit that hands the resolver address-discovery responsibility to a configurable layer (so that DoT or DoH endpoints can later be supplied without revising the resolver core). The kernel-resident stub resolver remains; the discovery layer becomes extensible.

**Evidence.** [RFC 9461](https://datatracker.ietf.org/doc/html/rfc9461); [RFC 9462](https://www.rfc-editor.org/rfc/rfc9462.html); [EDSR draft](https://datatracker.ietf.org/doc/draft-ietf-add-encrypted-dns-server-redirection/); [captaindns 2025 DoH/DoQ/DoT analysis](https://www.captaindns.com/en/blog/doh3-doq-dot-2025-latest); [mr-alias DNS privacy guide](https://mr-alias.com/articles/dns-privacy-guide.html).

### R15. Intel I219-V e1000e known driver quirks

**Mechanism.** Linux's `e1000e` driver has shipped documented workarounds for the I219 chip family: commit `b10effb` ("e1000e: fix buffer overrun while the I219 is processing DMA transactions") slows DMA access by reducing outstanding requests to avoid a buffer overrun; the workaround impacts TCP performance, and disabling TSO eliminates the performance loss without noticeable CPU impact. A separate TSO-related regression from Linux 5.8 (mid-2020) through Linux 6.3 caused I219-LM to run at ~60% of advertised speed. I219-LM and I219-V devices can fall into unrecovered Tx hang under stressful UDP traffic and multiple Ethernet-cable reconnections, recovered only by system reboot.

**Phase 4 exposure.** THINK R3 names "real-real-hardware verification (Intel I219-V) requires the e1000e driver, which is forward-marker'd to BUILD-2b or BUILD-6." When the e1000e driver lands, it inherits the chip's quirks; the workarounds Linux documents (DMA outstanding-request slowdown, TSO-disable at probe time, manageability-engine considerations) are the substrate any NeoPlutarch e1000e implementation will need to address.

**Mitigation suggestion.** When PLAN opens for the BUILD that lands e1000e, the design pass should explicitly enumerate the I219 quirks (DMA-buffer-overrun workaround, TSO-disable-at-probe, Tx-hang failure mode under stressful UDP) and document the chosen mitigation per Bible Article 9.10's correctness-before-coherence-before-performance ordering. The "stressful UDP under cable reconnect" failure mode is particularly relevant given BUILD-1's UDP-first scope: if BUILD-1e bare-metal verification on virtio-net-under-VMware reveals any analog hardware-quirk, the same posture transfers to I219-V.

**Evidence.** [Linux commit b10effb e1000e I219 DMA buffer-overrun fix](https://github.com/torvalds/linux/commit/b10effb92e272051dd1ec0d7be56bf9ca85ab927); [Phoronix I219-LM 60% speed Linux 6.3 fix](https://www.phoronix.com/news/Intel-i219-LM-Linux-60p-Fix); [Linux Mint I219-V forum thread (Tx hang documentation)](https://forums.linuxmint.com/viewtopic.php?t=411759); [Proxmox I219-V e1000e driver thread](https://forum.proxmox.com/threads/problem-with-intel-i219-v-ethernet-adapter-and-e1000e-driver.182266/).

### R16. Hubris UDP→TCP transition unevidenced (precedent gap)

**Mechanism.** THINK §3.1 claims the layered-handle shape forward-accommodates UDP and TCP without API-shape revisions, citing Hubris as precedent. Hubris is UDP-over-IPv6 + ICMPv6 only; Hubris has not added TCP. The transition has not been exercised. The claim is a BUILD-3 hypothesis, not a precedent-supported certainty.

**Phase 4 exposure.** BUILD-3 lands TCP on the layered-handle shape committed at BUILD-1. If the shape requires revision when TCP arrives — for example, if `sys_net_socket_tcp` returns a handle with semantics that `sys_net_socket_udp` did not anticipate (connection-state queries, partial-write tracking, urgent-data handling) — the revision affects BUILD-1's already-shipped API.

**Mitigation suggestion.** Reframe THINK §3.1 to acknowledge that "forward-accommodates UDP and TCP" is a BUILD-3 design hypothesis to validate rather than a precedent-supported certainty. At BUILD-1 design time, sketch the BUILD-3 `sys_net_socket_tcp` and `sys_net_listen_tcp` syscall surfaces speculatively (without committing them) to validate that the layered-handle shape composes with TCP's connection-state model. If the speculative sketch reveals an API-shape obstacle, surface it at BUILD-1 PLAN time rather than discovering it at BUILD-3.

**Evidence.** See §2.1 and §2.5 above. [Hubris task/net README](https://github.com/oxidecomputer/hubris/tree/master/task/net).

---

## 4. Sub-BUILD Sequencing Validation Across Projects

| Project | First-stack scope | Sequencing | Capability shape | Note |
|---|---|---|---|---|
| **Hubris** | UDP + ICMPv6 over IPv6 only | UDP-only; never staged toward TCP | Compile-time-bound owner-task per `app.toml` | Closest analog to BUILD-1 transport scope; closed L2 management network, not general-purpose |
| **Redox / smolnetd** | UDP+TCP+ICMP+IP via smoltcp | Staged: ethernetd-with-no-TCP first; TCP added later; `sockrecv`/`socksend` scheme methods added additively for UDP semantics | Scheme-rooted paths (`/scheme/tcp/127.0.0.1/3000`) | Documented additive API path when UDP semantics needed extending |
| **Asterinas** | Full smoltcp coverage on adoption | Not staged; inherited at integration | smoltcp BSD-style socket layer above; Asterinas adds netlink, UNIX, SOCK_SEQPACKET in 0.16–0.17 release window | Migration debate to Netstack3 closed as "not planned" (Feb 2025) |
| **LionsOS** | lwIP component over sDDF | Out-of-kernel; modular component split | seL4 capability discipline at component boundary | No in-kernel protocol-ordering signal |
| **Fuchsia Netstack3** | TCP + UDP + ARP + ICMP + IPv6 + DHCP in Rust rewrite | Replacement of Go-based Netstack2; full coverage on landing | FIDL channels routed via component manifest | Single-threaded core; bindings split 90/10 |
| **smoltcp upstream** | IPv4-first; IPv6 added later (PR #72) | Sequenced: IPv4 first, IPv6 after | BSD-style socket type per protocol | The PM choice to do IPv6-first contradicts smoltcp's own historical ordering |
| **Theseus OS** | Not documented in surfaced material | Unknown | Single-address-space cell architecture | Cannot verify networking from primary source |
| **NeoPlutarch Phase 4 (PM choice)** | IPv6 + UDP + ICMPv6 + virtio-net at BUILD-1; staged toward IPv4 (B-2), TCP (B-3), DNS+HE (B-4), TLS (B-5) | IPv6-first; UDP-first; TLS-last | Layered handle: NetworkAccess outer + per-socket inner via Cargo-package syscall family | Novel sequencing without documented precedent for IPv6-before-IPv4 in clean-room kernels |

**Summary.** The PM Phase 4 sequencing matches Hubris on BUILD-1 transport scope (UDP + ICMPv6 + IPv6) and matches Redox on the additive-API path. The PM sequencing *diverges* from every documented analog on the IPv6-before-IPv4 ordering and the late-TLS-placement: no surveyed project shipped IPv6 before IPv4 in a clean-room implementation; Redox inherited both simultaneously via smoltcp adoption. The PM choice is normatively sound and not refuted, but the comparison set does not testify to it; BUILD-1→BUILD-2 should be planned with the expectation of surprise.

---

## 5. IETF / RFC Corpus Delta from 2026-05-20 to 2026-05-22

No material change. Detail:

- `draft-ietf-happy-happyeyeballs-v3-03` (2 March 2026): unchanged. Standards Track intended; expires 3 September 2026.
- 6man WG: `draft-ietf-6man-sidlist-clarification-03` (2026-05-19): pre-baseline. No new drafts in 2-day window.
- v6ops WG: no new drafts in 2-day window.
- HAPPY WG: no new documents.
- CCWG: `draft-ietf-ccwg-bbr-05` (2 March 2026): Experimental, unchanged.
- RFC editor queue: not re-checked within research budget. No event surfaced affecting Phase 4 corpus.

The Cerebro OBSERVE §11 corpus current as of 2026-05-20 is current as of 2026-05-22.

---

## 6. Stop Conditions and Scope Acknowledgments

### Pre-Phase-2 deviation scope

The pre-Phase-2 audit scope (deviations #10, #7, #8 per the pre-Phase-2 close memory) is out of scope for this pressure-test pass per dispatch §3 ("Not Phase 5+ scope exploration"; "Not a new survey"). THINK P2 (§6 of the THINK artifact) is the locus for Phase 2 capability infrastructure validation; this pressure-test confirmed P2's three required capabilities (capability handing at spawn, IPC capability-passing, per-handle rights bitmaps) are well-grounded in production patterns (§2.12 of this artifact), but did not audit whether Phase 4 THINK silently re-opens any pre-Phase-2 deviation. If PM has reason to suspect such re-opening, that is a PM-DISPOSITION-time question to surface separately — not in scope for the Cerebro pressure-test pass.

### Stop conditions

No stop condition was hit during this research pass.

- No pressure-test finding contradicts a PM-NOTE confirmed position (a)–(f). All six remain CONFIRMED per Cerebro OBSERVE §8.
- Primary sources were accessible for every load-bearing claim; where a source could not be verified within research budget, this is marked explicitly ("Cannot verify from primary source") with a mitigation note.
- No verdict requires fundamentally restructuring THINK. Of the 14 lines, 8 are CONFIRMED, 5 are REFINABLE, 1 is CONSIDER-REVISITING. The CONSIDER-REVISITING (§4.7 SLAAC + capability lifetime) is a localizable gap in THINK §3.10, not a structural fault.
- The IETF corpus check surfaced no event materially changing Phase 4's design.
- The research budget was consumed within the bounded half-day envelope; §4.1–§4.14 are all addressed; no incomplete-lane surface conditions.

---

## 7. Open Questions for PM Consumption

These are surfaced for PM-DISPOSITION consideration. Cerebro does not direct PM action; PM has authority for what to fold into THINK v0.2 (if any), what to defer to a later Phase, and what to dismiss.

**Q1 (from §2.1, §2.5, §2.14 R16).** Should THINK §3.1 reframe "forward-accommodates UDP and TCP without API-shape revisions" as a BUILD-3 design hypothesis to validate rather than a precedent-supported certainty? The Hubris precedent (UDP-only, never staged to TCP) does not testify to forward-compatibility; the Redox precedent demonstrates additive-but-sometimes-needed scheme method additions. Sketching `sys_net_socket_tcp` speculatively at BUILD-1 PLAN time would mitigate.

**Q2 (from §2.2, §2.14 sequencing finding).** Should THINK §4 acknowledge that IPv6-before-IPv4 lacks documented precedent in clean-room kernels, and that BUILD-1→BUILD-2 should be planned with the expectation of surprise rather than the easier expectation of an additive overlay?

**Q3 (from §2.3, §2.14 R12).** Should THINK §3.3 acknowledge that "matches Linux kTLS" inherits Linux kTLS's documented failure-mode profile (seven CVEs across 2024–2025, recurring async-crypto-state-management pattern), and that ADR-022's cell-swap forward marker is a candidate revisit-trigger for BUILD-5's TLS state?

**Q4 (from §2.4, §2.14 R14).** Should THINK §3.4 acknowledge that the 2025–2026 encrypted-DNS direction (RFC 9461/9462, EDSR, U.S. EO, Firefox >85% DoH) carries a forward-marker dependency tighter than the typical Phase 5+ horizon, and plan a BUILD-4 sub-commit that hands the resolver's address-discovery responsibility to a configurable layer?

**Q5 (from §2.7, §2.14 R13).** Should THINK §3.10 specify the kernel-side behavior when a capability-bound socket's source address is rotated under Privacy Extensions defaults? Either extend §3.10 with a statement, or add a new BUILD-1 sub-commit (BUILD-1.1) that defines the address-deprecation-event surface and the socket-binding-retention semantics.

**Q6 (from §2.11, §2.14 R15).** Should THINK §5 R3 (Intel I219-V) be extended with the documented e1000e quirks (DMA-buffer-overrun workaround, TSO-disable-at-probe, Tx-hang under stressful UDP) so that PLAN inherits the production-reference failure-mode profile? Phase 4's I219-V verification is forward-marker'd, but the quirks are known now.

**Q7 (from §2.14 R11).** Should THINK §3 include an explicit BUILD-1 sub-commit on IPv6 extension-header processing posture (RFC 8200 + RFC 9673 SHOULD-process-MAY-skip; explicit rejection of Routing-type-0 deprecated per RFC 5095)?

**Q8 (from §2.11).** PLAN's smoltcp-wrap-vs-fork-vs-from-scratch choice (THINK R1) is informed by: (a) the v0.13.0 release evidence contesting "minimal maintenance," (b) Asterinas closing the migration debate against Netstack3, (c) Hubris being the only documented "wrap" analog with similar concurrency posture. Cerebro does not advise; the choice remains PLAN's.

---

## 8. References

### IETF documents

- RFC 5095 — Deprecation of Type 0 Routing Headers in IPv6
- RFC 8200 — Internet Protocol, Version 6 Specification (STD 86)
- RFC 8981 — Temporary Address Extensions for SLAAC in IPv6 (Privacy Extensions): https://www.rfc-editor.org/rfc/rfc8981.html
- RFC 9098 — Operational Implications of IPv6 Packets with Extension Headers: https://www.rfc-editor.org/rfc/rfc9098.html
- RFC 9293 — Transmission Control Protocol (TCP)
- RFC 9461 — Service Binding Mapping for DNS Servers: https://datatracker.ietf.org/doc/html/rfc9461
- RFC 9462 — Discovery of Designated Resolvers: https://www.rfc-editor.org/rfc/rfc9462.html
- RFC 9673 — IPv6 Hop-by-Hop Options Processing
- `draft-ietf-happy-happyeyeballs-v3-03`: https://datatracker.ietf.org/doc/draft-ietf-happy-happyeyeballs-v3/
- `draft-ietf-ccwg-bbr-05` (BBRv3 informational): https://datatracker.ietf.org/doc/draft-ietf-ccwg-bbr/
- `draft-ietf-add-encrypted-dns-server-redirection`: https://datatracker.ietf.org/doc/draft-ietf-add-encrypted-dns-server-redirection/
- `draft-ietf-add-requirements-00`: https://www.ietf.org/archive/id/draft-ietf-add-requirements-00.html

### IETF working groups

- 6man (IPv6 Maintenance): https://datatracker.ietf.org/wg/6man/documents/
- v6ops (IPv6 Operations): https://datatracker.ietf.org/wg/v6ops/documents/
- HAPPY (Happy Eyeballs): https://datatracker.ietf.org/wg/happy/
- ADD (Adaptive DNS Discovery): https://datatracker.ietf.org/wg/add/
- CCWG (Congestion Control Working Group)

### OS / kernel codebases

- smoltcp upstream: https://github.com/smoltcp-rs/smoltcp
- smoltcp v0.13.0 release notes: https://github.com/smoltcp-rs/smoltcp/releases
- smoltcp IPv6-after-IPv4 PR #72: https://github.com/smoltcp-rs/smoltcp/pull/72
- smoltcp issue #50 (Redox integration, loopback ARP-flood): https://github.com/smoltcp-rs/smoltcp/issues/50
- smoltcp CUBIC issue #949: https://github.com/smoltcp-rs/smoltcp/issues/949
- Hubris `task/net`: https://github.com/oxidecomputer/hubris/tree/master/task/net
- Hubris README: https://github.com/oxidecomputer/hubris
- Oxide RFD 250: https://rfd.shared.oxide.computer/rfd/0250
- Redox `redox-os/redox` PR #845 (smoltcp adoption): https://github.com/redox-os/redox/pull/845
- Fuchsia Netstack3 source tree: https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/
- Fuchsia Netstack3 design tenets: https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/TENETS_AND_DESIGN_DECISIONS.md
- Fuchsia Netstack3 improvements: https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/IMPROVEMENTS.md
- Fuchsia Netstack3 docs index: https://fuchsia.googlesource.com/fuchsia/+/refs/heads/main/src/connectivity/network/netstack3/docs/
- Fuchsia component manifest (CML) reference: https://fuchsia.dev/reference/cml
- Fuchsia capabilities concept: https://fuchsia.dev/fuchsia-src/concepts/components/v2/capabilities
- Asterinas issue #1821 (smoltcp→Netstack3 migration debate, closed not planned): https://github.com/asterinas/asterinas/issues/1821
- Asterinas USENIX ATC 2025 paper (arXiv): https://arxiv.org/html/2506.03876v1
- Asterinas USENIX ATC 2025 (ACM listing): https://dl.acm.org/doi/10.5555/3768039.3768058
- Asterinas 0.17.0 announcement: https://asterinas.github.io/2025/12/19/announcing-asterinas-0.17.0.html
- LionsOS paper (arXiv): https://arxiv.org/pdf/2501.06234
- seL4 capDL documentation: https://docs.sel4.systems/projects/capdl/
- seL4 capDL language spec: https://docs.sel4.systems/projects/capdl/lang-spec.html
- seL4/capdl GitHub: https://github.com/seL4/capdl
- seL4 capabilities tutorial: https://docs.sel4.systems/Tutorials/capabilities.html
- Capsicum paper (2010): https://papers.freebsd.org/2010/rwatson-capsicum.files/rwatson-capsicum-paper.pdf
- Capsicum FreeBSD docs: https://www.cl.cam.ac.uk/research/security/capsicum/freebsd.html
- Linux netfilter HOWTO §7 (loopback INPUT chain): https://www.netfilter.org/documentation/HOWTO/packet-filtering-HOWTO-7.html
- Linux `CONFIG_DNS_RESOLVER` docs: https://docs.kernel.org/networking/dns_resolver.html
- Linux kTLS documentation: https://docs.kernel.org/networking/tls.html
- Linux kTLS offload documentation: https://docs.kernel.org/networking/tls-offload.html
- Linux commit b10effb (e1000e I219 DMA buffer-overrun fix): https://github.com/torvalds/linux/commit/b10effb92e272051dd1ec0d7be56bf9ca85ab927
- Phoronix I219-LM 60% Linux 6.3 fix: https://www.phoronix.com/news/Intel-i219-LM-Linux-60p-Fix
- Phoronix Linux 6.13 io_uring: https://www.phoronix.com/news/Linux-6.13-IO_uring
- io_uring whitepaper (kernel.dk): https://kernel.dk/io_uring.pdf
- Google `google/bbr` v3 branch: https://github.com/google/bbr/tree/v3
- Google bbr-dev list thread (BBRv3 not mainline): https://groups.google.com/g/bbr-dev/c/4vwZGw0nxdQ
- Hacker News BBRv3 mainline status thread: https://news.ycombinator.com/item?id=45061454
- ktls crate (rustls org) releases: https://github.com/rustls/ktls/releases
- ktls crate lib.rs metadata: https://lib.rs/crates/ktls
- ktls-core crates.io: https://crates.io/crates/ktls-core
- NVIDIA kTLS hardware-offload docs: https://docs.nvidia.com/doca/sdk/ktls-offloads/index.html
- ANLAB-KAIST/usnet_sockets (smoltcp-userspace wrap example): https://github.com/ANLAB-KAIST/usnet_sockets

### CVE references

- CVE-2026-23442 (SRv6 NULL idev dereference): https://windowsforum.com/threads/cve-2026-23442-srv6-kernel-bug-null-idev-dereference-in-ipv6-routing-paths.410111/
- CVE-2026-43501 (IPv6 RPL out-of-bounds write): https://windowsnews.ai/article/cve-2026-43501-linux-ipv6-rpl-out-of-bounds-write-patch-the-right-kernels.419270
- CVE-2025-39946 (tls_rx_msg_size parser OOB): https://faith2dxy.xyz/2025-10-02/kCTF-TLS-nday-analysis/
- CVE-2025-38608 (bpf+ktls plaintext truncation): https://www.suse.com/security/cve/CVE-2025-38608.html
- CVE-2024-26582/3/4/5 cluster analysis: https://u1f383.github.io/linux/2025/01/21/linux-kernel-tls-part-2.html

### Industry / signal points

- Google IPv6 statistics: https://www.google.com/intl/en/ipv6/statistics.html
- The Register IPv6 50% milestone (April 17, 2026): https://www.theregister.com/2026/04/17/ipv6_50_percent_google/
- Cloudflare Radar IPv6 report (40.1%): https://radar.cloudflare.com/reports/ipv6
- Spectrum native IPv6 to residential: https://www.spectrum.net/support/internet/ipv6
- Mozilla Firefox DoH >85% U.S. adoption analysis: https://www.captaindns.com/en/blog/doh3-doq-dot-2025-latest
- DNS encryption U.S. EO January 2025 + enterprise DoT acceleration: https://mr-alias.com/articles/dns-privacy-guide.html
- Industrial network port-53/853/784/8853 blocking patterns: https://industrialmonitordirect.com/blogs/knowledgebase/blocking-encrypted-dns-in-industrial-networks-doh-dot-and-quic-bypass-prevention
- Joyride Linux network-stack-redesign paper (treats routed-loopback as baseline): https://arxiv.org/pdf/2509.25015
- IFIP Networking 2025 BBRv3 evaluation: https://networking.ifip.org/2025/images/Net25_papers/1571125683.pdf
- arXiv 2505.07741 BBR sharing behavior: https://arxiv.org/html/2505.07741v1
- arXiv 2510.22461 default TCP CC analysis: https://arxiv.org/html/2510.22461v1
- netstack.fm episode with Bruno Dal Bo Silva (Fuchsia Netstack3 Staff Engineer, October 7, 2025): https://podcasts.apple.com/us/podcast/fuchsias-netstack3-with-bruno-dal-bo-silva/id1833932472?i=1000730511978
- Zephyr RTOS RFC 8981 implementation PR #71996: https://github.com/zephyrproject-rtos/zephyr/pull/71996
- Phoronix Google BBRv3 Linux upstream signal: https://www.phoronix.com/news/Google-BBRv3-Linux

### Cross-references in NeoPlutarch artifacts

- `Plans/PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` — the artifact under pressure-test
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` — Cerebro's OBSERVE substrate
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md`
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md`
- `Plans/RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md` (voice anchor)
- `docs/bible.md` v1.7.0 — Articles 1.2, 1.4, 3, 4.1, 9.10, 11, 12, 14, 18.1, 28
- `docs/ADR-022-fault-recovery-direction-forward-marker.md`
- `docs/ADR-023-linux-personality-layer.md`

---

## 9. Closure

Per dispatch §8: this artifact is informational per Bible Article 18.1; it does not bind the project. PM consumes via PM-DISPOSITION. If findings warrant THINK v0.2, PM files that referencing both the original THINK and this pressure-test. If THINK survives substantially intact, PM-DISPOSITION acknowledges and PL ratification of Q1–Q10 (or a revised Q-list) proceeds. Phase 4 PLAN drafting remains gated on Phase 3 formal close and PL ratification; this pressure-test does not gate or open PLAN.

---

**End of pressure-test artifact.**

*Phase 4 THINK pressure-tested. 14 lines addressed. 8 CONFIRMED, 5 REFINABLE, 1 CONSIDER-REVISITING, 0 REFUTED. Six additional risks (R11–R16) surfaced with mitigation suggestions. IETF corpus delta from 2026-05-20 to 2026-05-22: no material change. PM-NOTE positions (a)–(f) remain confirmed. No stop conditions hit. PM consumption follows.*
