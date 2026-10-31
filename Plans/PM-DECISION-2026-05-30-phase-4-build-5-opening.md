# PM-DECISION: Phase 4 BUILD-5 opening — DNS + Happy Eyeballs + kTLS (Phase 4 ship)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — opens BUILD-5; §9 open points for PL ratification
**Authority chain:**
- CLOSE-REPORT-2026-05-30-phase-4-build-4.5.md — FM-4-1 discharged; on-link
  resolution complete for both families across UDP + TCP
- CLOSE-REPORT-2026-05-30-phase-4-build-3.md — FM-3-1 (RFC 6724 prefer-IPv6
  destination default) deferred *to BUILD-5*
- Phase 4 THINK v0.2 §4.3 (TLS placement), §4.4 (DNS resolver placement),
  §4.1 (sockets-equivalent ABI), §5 (sub-commit milestones)
- PM-NOTE-2026-05-28-cve-2026-31431-copy-fail (synchronous-per-packet kTLS)
- PM-NOTE-2026-05-28-cifswitch (key material opaque; per-record state kernel-private)
- PL-RATIFICATION-2026-05-28-phase-4-build-1: RDNSS lifted at BUILD-1 / consumed
  here; `ResolverConfig` populated-but-unused trajectory
- RFCs: 1035 (DNS), 6891 (EDNS(0)), 8106 (RDNSS), 8305 (Happy Eyeballs v2),
  6724 (address selection), 8446 (TLS 1.3)

---

## §1. Why this build exists

BUILD-5 **ships Phase 4**: a userspace program resolves a hostname, races a
dual-stack connection, completes a TLS 1.3 handshake, and performs an HTTPS GET
— end to end, on NeoPlutarch's own ABI. Every layer beneath it is now in place:
BUILD-1–2 (IPv6 core + SLAAC), BUILD-3 (IPv4 + DHCPv4 + dual-stack source
selection), BUILD-4 (hand-rolled TCP), BUILD-4.5 (NDP — on-link resolution for
both families). The transport and on-link layers are complete. What remains
before an HTTPS fetch is the **name → address → connection → encrypted record**
path. That is this build.

This is the largest build of the phase. It carries three subsystems that THINK
v0.2 originally sketched across BUILD-4/BUILD-5 and the close-report roadmap has
since consolidated under "BUILD-5 = DNS + Happy Eyeballs + kTLS (= Phase 4
ship)": a **DNS stub resolver**, **Happy Eyeballs (RFC 8305)** connection
racing, and **kTLS** (hybrid in-kernel record layer). It is scoped as a
multi-sub-commit build (§4) precisely because of that breadth.

## §2. Substrate

- **BUILD-4.5 close:** `49cd238`; landed on `main` at `23f75a3`. BUILD-5 branches
  from `origin/main`.
- **DNS server discovery is half-wired, never consumed.** DHCPv4 already
  extracts the option-6 DNS server into `Lease.dns` (`dhcpv4.rs`, option-6
  parse + lease field) but no resolver reads it. The RA path (`icmpv6.rs`
  `parse_router_advertisement`) parses prefix (3) and route (24) options but
  **silently drops RDNSS (option 25, RFC 8106)** at its `_ => {}` arm. There is
  **no `ResolverConfig` struct yet** — the BUILD-1 "populated-but-unused"
  trajectory was not actually built; BUILD-5 builds *and* consumes it.
- **Sockets ABI in place:** `0x180`–`0x187` (UDP create/bind/send/recv/close/
  stats/sendto/recvfrom) and `0x188`–`0x18B` (TCP connect/listen/accept/
  shutdown). `sys_net_connect_tcp` (`0x188`) currently takes a single
  `(addr, port)` tuple — BUILD-5 adds the name-based, candidate-list path
  alongside it.
- **No crypto in tree.** Only `blake3` (NeoFS content hashing, Phase 3) is
  present. No AEAD, no TLS, no DNS code. The kernel record layer needs a vetted
  no_std AEAD; userspace needs a TLS 1.3 handshake stack.
- **FM-3-1 lands here:** BUILD-3 left source selection family-directed and
  deferred the RFC 6724 prefer-IPv6 *destination* default to BUILD-5; it is the
  candidate-ordering rule Happy Eyeballs consumes.

## §3. Load-bearing decisions

### Q1 — DNS resolver implementation
**Hand-rolled, kernel-resident stub resolver** (RFC 1035), A + AAAA only,
mirroring the hand-roll control-plane posture. Build a `ResolverConfig`
populated from **both** sources — RDNSS (RFC 8106 option 25, lifted in
`icmpv6.rs` RA parsing) and the DHCPv4 `Lease.dns` field — and consume it.
Exposed internally and via `sys_net_resolve(name, family_hint) -> &[NetCandidate]`.
*Rejected:* a userspace DNS daemon — Happy Eyeballs' 50 ms/250 ms timing budget
needs DNS and connection racing co-located in the kernel (THINK §4.4 rationale).

### Q2 — DNS transport, EDNS, caching
**UDP/53 only**, EDNS(0) (RFC 6891) to advertise a reassembly-free UDP payload
size, and a **minimal TTL-aged answer cache** mirroring the ARP/NDP cache
discipline. *Deferred (forward-markers):* TCP/53 truncation fallback (FM-5-1),
DNSSEC, and DoT/DoH. The resolver is split so the **address-discovery layer is
separable from the query core** (THINK §4.4 v0.2 / BUILD-4d intent), preserving
the DoT/DoH extension point without touching the core later.

### Q3 — Happy Eyeballs + RFC 6724 destination default
**Implement HEv2 (RFC 8305):** issue AAAA and A in parallel; apply the ~50 ms
Resolution Delay preferring AAAA; sort candidates by the **RFC 6724 prefer-IPv6
destination rule** (this is the work that *closes FM-3-1*); attempt connections
staggered by a ~250 ms Connection Attempt Delay; first to complete wins, cancel
the losers. The internal API is **`&[NetCandidate]`-shaped**, HEv3-forward-
compatible (THINK §4.1). Full HEv3 protocol-priority is out of scope (FM-5-2).

### Q4 — kTLS architecture
**Hybrid split, per THINK §4.3 — non-negotiable shape:**
- **Userspace owns the TLS 1.3 handshake** (handshake state machine, key
  exchange, certificate verification) in `libneo`.
- **Kernel owns the symmetric record layer**, entered via
  `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)`. After enable, per-record
  state is **kernel-private** (CIFSwitch PM-NOTE) and key material is **opaque
  to the kernel except as cipher input** — the kernel never parses
  handshake-derived metadata.
- **The record-layer transform is synchronous, per-packet, in the network
  task's context** — *not* Linux's workqueue-completion-with-shared-mutable-
  state pattern. This eliminates the documented kTLS CVE class by construction
  (Copy Fail PM-NOTE R12 + §3.1) and is a **hard close-gate**, not a preference.
- **No userspace-modifiable memory under kernel crypto.** Plaintext/ciphertext
  buffers crossing the boundary are **copied, not shared** — no splice/vmsplice/
  sendfile-class zero-copy path (Copy Fail PM-NOTE §3.2/§3.3; out of scope
  absent an explicit `/simplify` re-litigation).
- **Rights:** enabling TLS requires WRITE on the `TcpSocket` cap; a READ-only
  holder cannot enable it (CIFSwitch PM-NOTE §3.3).

### Q5 — crypto provider & cipher suite (cardinal: do not roll your own crypto)
The control plane is hand-rolled; **the cryptography is not.** The kernel record
layer uses a **vetted pure-Rust no_std AEAD crate** — recommend RustCrypto
`chacha20poly1305` — providing **TLS_CHACHA20_POLY1305_SHA256** as the single
mandatory TLS 1.3 suite. ChaCha20-Poly1305 is chosen over AES-128-GCM to avoid
AES-NI runtime feature-detection and side-channel-prone table AES in-kernel;
it is constant-time pure Rust with no asm/std dependency. AES-128-GCM is a
**post-ship optional** (FM-5-3). Userspace handshake crypto is supplied by the
chosen TLS stack's own provider (Q6).

### Q6 — TLS scope, handshake stack, certificate validation
**TLS 1.3 only (RFC 8446), server-auth only** (no client certificates). The
userspace handshake stack is **`rustls` in `libneo`** (THINK §4.3), configured
to the single mandatory suite, with certificate-chain validation against a
**small bundled root store** (the demo server's trust anchor set — a full system
trust store is out of scope, FM-5-4). ALPN advertises **http/1.1**; the HTTP/1.1
GET itself lives in userspace (the demo program), not the kernel. *Risk noted:*
`rustls` in a no_std-adjacent `libneo` is the heaviest single integration in
this build; §4 sequences it behind the kernel record layer so the crypto
boundary is proven before the handshake stack lands. If `rustls` no_std
integration proves disproportionate, the fallback is a minimal hand-written
TLS 1.3 client handshake driving the *same* RustCrypto primitives — but that is
a §10 escalation, not a default.

### Q7 — ship demo & verification strategy
The deterministic CI scenario must not depend on real outbound internet (prior
builds used injected/synthetic harness peers, no tap/SLIRP/radvd). **Recommend:**
the QEMU `phase4_https_fetch` scenario runs the full path — `sys_net_resolve`
against an **in-harness DNS responder**, Happy Eyeballs racing, TLS 1.3
handshake + record exchange + HTTP/1.1 GET against an **in-harness/loopback TLS
server** — asserting the decrypted plaintext response body. The **real external
dual-stack HTTPS fetch is the bare-metal deliverable**, deferred to the
bare-metal validation marker (consistent with every prior build's CI-vs-metal
split). *Escalation point (§10):* if PL wants real outbound in CI, that means
introducing QEMU user-mode networking (SLIRP) — a test-infra change worth a
separate decision.

## §4. Sub-commit split with ISCs (pending §9 ratification)

> Five sub-commits. ISC detail is finalized in the PLAN after ratification; the
> shape below is the proposed contract.

### BUILD-5a — DNS resolver core + `ResolverConfig`
**Scope:** `ResolverConfig` populated from RDNSS (lift option 25 in
`icmpv6.rs`) + DHCPv4 `Lease.dns`; RFC 1035 A/AAAA query/response codec with
EDNS(0); minimal TTL answer cache; `sys_net_resolve`; `inject`/`tick` test hooks.
**Anti-scope:** no HE, no TCP/53, no DNSSEC/DoT/DoH, no TLS.
- **ISC-5a-1:** A + AAAA query build / response parse round-trip; EDNS(0) OPT present.
- **ISC-5a-2:** RDNSS (RA) and DHCP option-6 both populate `ResolverConfig`.
- **ISC-5a-3:** injected response populates cache; `sys_net_resolve` returns candidates; TTL ages out via `tick`.
- **ISC-5a-4:** malformed/compressed-pointer-loop responses rejected without panic/unwrap.

### BUILD-5b — Happy Eyeballs + RFC 6724 destination default (closes FM-3-1)
**Scope:** RFC 8305 racing (parallel A/AAAA, Resolution Delay, staggered
Connection Attempt Delay, first-wins/cancel); RFC 6724 prefer-IPv6 destination
candidate sort; `&[NetCandidate]` internal API; name-based dial path.
**Anti-scope:** no TLS; no HEv3 priority.
- **ISC-5b-1:** mixed AAAA/A candidate set sorts IPv6-first per RFC 6724.
- **ISC-5b-2:** IPv6-reachable peer → IPv6 wins; IPv6 black-holed → A path wins within budget; loser cancelled.
- **ISC-5b-3:** existing single-tuple `connect_tcp` (`0x188`) path unchanged (regression).
- **ISC-5b-4:** no panic on empty/all-failing candidate lists.

### BUILD-5c — kTLS record layer (ChaCha20-Poly1305, synchronous per-packet)
**Scope:** `sys_net_tcp_enable_tls(TcpSocket, KeyMaterial)`; synchronous
per-packet AEAD seal/open in the network task context; kernel-private record
state; copy-not-share boundary buffers; WRITE-rights gate.
**Anti-scope:** no handshake (5d); no zero-copy; no AES-GCM.
- **ISC-5c-1:** known-answer AEAD seal/open vectors (RFC 8439) pass in-kernel.
- **ISC-5c-2:** enable_tls requires WRITE; READ-only cap rejected; post-enable record state not userspace-readable.
- **ISC-5c-3:** record transform is synchronous in the net-task path (no workqueue/shared-mutable handoff) — verified by construction + review.
- **ISC-5c-4:** no unwrap/panic; truncated/forged-tag records rejected (auth failure ≠ crash).

### BUILD-5d — userspace TLS 1.3 handshake + enable_tls wiring + HTTP/1.1
**Scope:** `rustls` (single suite) in `libneo`; handshake over a connected
`TcpSocket`; derive + hand `KeyMaterial` to the kernel via `enable_tls`;
bundled root store; minimal HTTP/1.1 GET helper.
**Anti-scope:** no client certs; no full trust store; no h2.
- **ISC-5d-1:** TLS 1.3 handshake to the in-harness server completes; cert chain validates against the bundled root.
- **ISC-5d-2:** post-handshake, application bytes flow through the kernel record layer; a GET returns the expected body.
- **ISC-5d-3:** bad-cert / wrong-name / downgrade attempts fail closed.

### BUILD-5e — HTTPS ship scenario + full sweep + bare-metal marker
**Scope:** `phase4_https_fetch` QEMU scenario (resolve → HE → TLS → GET, dual
candidate set); full `neo-test all` sweep; bare-metal deferral marker (real
external dual-stack server).
- **ISC-5e-1:** scenario passes SMOKE + NEO_SMP=4; asserts decrypted body.
- **ISC-5e-2:** full `neo-test all` green at ATTEMPTS=1; all prior phase-4 scenarios regress green.
- **ISC-5e-3:** bare-metal marker filed (real-server fetch, SLIRP/outbound, AES-GCM, TCP/53, DoT/DoH).

## §5. /simplify review

Hand-rolled DNS + HE consistent with prior builds (Q1/Q3). Crypto delegated to a
vetted crate, never hand-rolled (Q5). One mandatory suite, TLS 1.3 only,
server-auth only (Q6) — the minimum that reaches HTTPS. Synchronous-per-packet
record layer is the simpler *and* safer construction (Q4). Five sub-commits, each
independently reviewable; 5c lands and proves the crypto boundary before 5d
brings the heavy handshake stack.

## §6. Risk cross-reference

| Item | Owner | Comment |
|---|---|---|
| FM-3-1 (RFC 6724 prefer-IPv6 dest) | 5b | Closed by the HE candidate sort. |
| kTLS CVE class (R12 / Copy Fail) | 5c | Synchronous-per-packet + copy-not-share by construction. |
| Key material / record-state exposure (CIFSwitch) | 5c | Opaque key, kernel-private state, WRITE-gated enable. |
| `rustls` no_std integration weight | 5d | Sequenced behind 5c; hand-written-handshake fallback is a §10 escalation. |
| Real-outbound in CI | 5e | Deferred to bare-metal; SLIRP is a separate §10 decision. |
| New forward-markers FM-5-1…5-4 | — | TCP/53, HEv3, AES-GCM, full trust store — all deferred. |

## §7. Authorization boundary

- **Authorized now:** this PM-DECISION + INDEX entry. **PLAN + DISPATCH are
  *held* until §9 is ratified** — the sub-commit ISCs depend on the crypto
  provider (Q5), TLS scope (Q6), and verification strategy (Q7), which are
  load-bearing enough that drafting ISCs ahead of ratification would bake in
  unratified choices.
- **Authorized on PL ratification (§9):** the resolver/HE/kTLS scope, the
  ChaCha20-Poly1305 + `rustls` + bundled-root posture, and the in-harness CI /
  bare-metal-real-server verification split as the close gate.
- **Not authorized:** hand-rolled crypto primitives; zero-copy crypto path;
  client-cert / full-trust-store scope; AES-GCM in the ship; real outbound CI
  networking (SLIRP) without a separate decision.

## §8. Required next artifacts (post-ratification)

1. PLAN — `Plans/PLAN-2026-05-30-phase-4-build-5-dns-he-ktls.md` (per-sub-commit
   ISC detail; resolver/HE/record-layer/handshake/scenario specs).
2. DISPATCH — `Plans/DISPATCH-2026-05-30-phase-4-build-5-…-jarvis.md`.
3. INDEX.md amendment (already opens the BUILD-5 section here).

## §9. Open points for ratification

1. **Q1** hand-rolled kernel-resident RFC 1035 stub (A/AAAA), `ResolverConfig`
   from RDNSS + DHCP DNS.
2. **Q2** UDP/53 + EDNS(0) + minimal TTL cache; TCP/53, DNSSEC, DoT/DoH deferred;
   address-discovery layer kept separable.
3. **Q3** HEv2 (RFC 8305) + RFC 6724 prefer-IPv6 destination default (closes
   FM-3-1); `&[NetCandidate]` API; HEv3 deferred.
4. **Q4** hybrid kTLS: userspace handshake / kernel synchronous-per-packet
   record layer; key opaque; copy-not-share; WRITE-gated enable.
5. **Q5** vetted no_std AEAD (RustCrypto `chacha20poly1305`),
   TLS_CHACHA20_POLY1305_SHA256 as the single mandatory suite; **no hand-rolled
   crypto**; AES-GCM deferred (FM-5-3).
6. **Q6** TLS 1.3 only, server-auth only, `rustls` in `libneo`, bundled root
   store, ALPN http/1.1; hand-written-handshake fallback only as a §10
   escalation.
7. **Q7** in-harness DNS + loopback/in-harness TLS server for the deterministic
   `phase4_https_fetch` CI scenario; real external dual-stack fetch deferred to
   bare-metal; SLIRP-in-CI is a separate decision if wanted.
8. **Sub-commit split** 5a–5e (§4) as the build contract.

---

**BUILD-5 opens as the Phase 4 ship: name → candidate race → TLS 1.3 record
layer → HTTPS GET. The control plane is hand-rolled; the cryptography is
delegated to vetted primitives and constrained to a synchronous-per-packet,
copy-not-share boundary. PLAN and DISPATCH follow on §9 ratification.**
