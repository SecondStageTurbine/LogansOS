# PLAN: Phase 4 BUILD-5 — DNS + Happy Eyeballs + kTLS (Phase 4 ship)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Status:** active — ratified; per-sub-commit contract for the DISPATCH
**References:**
- PM-DECISION-2026-05-30-phase-4-build-5-opening.md (Q1–Q7, §4)
- PL-RATIFICATION-2026-05-30-phase-4-build-5-pm-decision.md
- THINK v0.2 §4.1/§4.3/§4.4; Copy Fail + CIFSwitch PM-NOTEs
- RFC 1035 / 6891 / 8106 / 8305 / 6724 / 8446 / 8439

---

## Substrate & integration points

- **Branch from** `origin/main` (`23f75a3`). BUILD-4.5 is landed.
- **Resolver inputs already on wire, unconsumed:**
  - DHCPv4 `Lease.dns` (`net/dhcpv4.rs`, option-6 parse + lease field).
  - RA RDNSS (option 25) — **not yet parsed**; `icmpv6.rs`
    `parse_router_advertisement` drops it at the `_ => {}` arm. 5a lifts it.
- **Sockets ABI:** UDP `0x180`–`0x187`, TCP `0x188`–`0x18B`
  (`arch/x86_64/syscall.rs`). 5a/5b/5c add new numbers `0x18C`+ (final
  assignment in 5a; keep contiguous).
- **TCP egress + NDP/ARP resolve/queue** are the layers HE connection attempts
  ride on. The record layer (5c) sits between the TCP send/recv path and the
  socket read/write syscalls.
- **No crypto in tree** beyond `blake3` (NeoFS). 5c adds the
  `chacha20poly1305` dependency to `kernel/Cargo.toml`; 5d adds `rustls`
  (single-suite config) to `userland/libneo/Cargo.toml`.

> **SPDX:** every new `.rs` starts `// SPDX-License-Identifier: MPL-2.0` as the
> first line; new `.toml` additions keep the existing header. CI enforces it.

---

## BUILD-5a — DNS resolver core + `ResolverConfig`

**New:** `kernel/src/net/dns.rs`. **Touch:** `icmpv6.rs` (RDNSS lift),
`net/mod.rs` (register + `tick`), `dhcpv4.rs` (expose lease DNS to config),
`arch/x86_64/syscall.rs` (+`sys_net_resolve`).

**Scope**
- `ResolverConfig { servers: heapless Vec<IpAddr, N>, … }` populated from RDNSS
  (RA option 25, RFC 8106 — lifetime + address list) **and** DHCPv4 `Lease.dns`.
  De-dup; RDNSS and DHCP coexist (dual-stack).
- RFC 1035 message codec: header, QD/AN sections, **A (1)** and **AAAA (28)**,
  name compression (read pointers defensively — bounded loop / visited-offset
  guard), EDNS(0) OPT record (RFC 6891) advertising a UDP payload size.
- Query over **UDP/53** using the existing UDP socket path; match by txn-id +
  question; minimal **TTL-aged answer cache** (`lookup`/`insert`/`age_out` via
  `tick`, mirroring `ArpCache`/`NeighborCache`).
- `sys_net_resolve(name_ptr, name_len, family_hint) -> &[NetCandidate]` —
  `NetCandidate { ip, family, … }`, the HEv3-forward-compatible shape.
- Test hooks: `dns::inject(response_bytes)`, `dns::tick`.

**Anti-scope:** no HE racing (5b), no TCP/53 fallback, no DNSSEC/DoT/DoH, no TLS.
Keep the **address-discovery layer** (which servers, how reached) in a separate
module/section from the **query core** (encode/decode/cache) so DoT/DoH can
extend the former later without touching the latter.

**ISCs**
- **ISC-5a-1:** A and AAAA query build + response parse round-trip; EDNS(0) OPT present and honored.
- **ISC-5a-2:** RDNSS (RA option 25) and DHCP option-6 each populate `ResolverConfig`; both present → both servers listed, de-duped.
- **ISC-5a-3:** injected response populates the cache; `sys_net_resolve` returns the candidate list; entries age out via `tick`.
- **ISC-5a-4:** malformed input — truncated message, compression-pointer loop, over-long name, count/length mismatch — rejected with no panic/unwrap/expect.

---

## BUILD-5b — Happy Eyeballs (RFC 8305) + RFC 6724 destination default

**New:** `kernel/src/net/happy_eyeballs.rs` (or `dial.rs`). **Touch:**
`interface.rs`/`ipv6.rs` (RFC 6724 dest sort), `tcp.rs` (multi-attempt connect
plumbing), `syscall.rs` (+`sys_net_dial` name-based connect).

**Scope**
- RFC 6724 **prefer-IPv6 destination** candidate ordering (Rule 10 + family
  preference) over the `&[NetCandidate]` list — **this closes FM-3-1**.
- RFC 8305 racing: resolve A + AAAA in parallel; **~50 ms Resolution Delay**
  preferring AAAA before starting v4 attempts; **staggered Connection Attempt
  Delay (~250 ms)** across candidates; first TCP connection to complete wins,
  remaining in-flight attempts cancelled and their sockets torn down.
- `sys_net_dial(name, port, …) -> TcpSocket`: resolve → sort → race → return the
  connected socket. Built on the existing TCP connect path + NDP/ARP resolve.
- Tunable delays exposed for the harness (so tests don't wall-clock 250 ms).

**Anti-scope:** no TLS; no HEv3 protocol-priority; the single-tuple
`sys_net_connect_tcp` (`0x188`) stays as-is.

**ISCs**
- **ISC-5b-1:** a mixed AAAA/A candidate set is ordered IPv6-first per RFC 6724.
- **ISC-5b-2:** IPv6 reachable → IPv6 connection wins; IPv6 black-holed → the A candidate wins within the attempt budget; the losing attempt is cancelled and its socket reclaimed (leak check).
- **ISC-5b-3:** existing `sys_net_connect_tcp` single-tuple path unchanged (regression); existing TCP scenarios green.
- **ISC-5b-4:** empty candidate list and all-attempts-fail return a clean error, no panic; no socket leak on the all-fail path.

---

## BUILD-5c — kTLS record layer (ChaCha20-Poly1305, synchronous per-packet)

**New:** `kernel/src/net/tls_record.rs`. **Touch:** `kernel/Cargo.toml`
(+`chacha20poly1305`, no_std), `socket.rs`/`tcp.rs` (record hook in the
send/recv path), `syscall.rs` (+`sys_net_tcp_enable_tls`).

**Scope**
- Dependency: RustCrypto `chacha20poly1305` (no_std, `default-features = false`).
  **No hand-rolled crypto.**
- `sys_net_tcp_enable_tls(tcp_socket, key_material_ptr, key_material_len)`:
  validates **WRITE** right on the socket cap; installs per-direction
  `{key, iv, seq}` as **kernel-private** state on the socket; key material is
  consumed as cipher input only (never parsed/retained as metadata) and the
  source buffer is **copied in, then zeroized**.
- TLS 1.3 record framing (RFC 8446 §5): on send, seal application data into
  `TLSCiphertext` records with the per-record nonce (seq ⊕ iv) and AAD; on recv,
  open and verify the tag; **auth failure tears the connection, never panics**.
- The seal/open transform runs **synchronously, per packet, in the network
  task's execution context** — no workqueue, no shared-mutable handoff, no async
  completion callback. **Buffers are copied across the userspace boundary, not
  shared** (no splice/vmsplice/zero-copy).

**Anti-scope:** no handshake (5d) — keys arrive pre-derived from userspace; no
AES-GCM; no zero-copy; no key update / renegotiation (post-ship).

**ISCs**
- **ISC-5c-1:** RFC 8439 known-answer vectors for ChaCha20-Poly1305 seal+open pass in-kernel; a TLS 1.3 record round-trips (seal then open → original plaintext).
- **ISC-5c-2:** `enable_tls` requires WRITE — a READ-only cap holder is rejected; after enable, record-layer key/seq state is not readable from userspace.
- **ISC-5c-3:** the record transform is synchronous in the net-task path (no workqueue / shared-mutable / async-callback) — asserted by construction and called out for review; key buffer zeroized after copy-in.
- **ISC-5c-4:** forged tag, truncated record, and over-length record are rejected (auth/format failure → connection error, never crash); no unwrap/panic.

---

## BUILD-5d — userspace TLS 1.3 handshake + `enable_tls` wiring + HTTP/1.1

**New:** `userland/libneo/src/tls.rs` (+ a tiny HTTP/1.1 GET helper). **Touch:**
`userland/libneo/Cargo.toml` (+`rustls`, single-suite, no_std-compatible
config + a no_std-friendly provider for ChaCha20-Poly1305 / SHA-256 / the
TLS 1.3 KX), the ship demo program.

**Scope**
- Drive a `rustls` client handshake over a connected `TcpSocket`
  (from `sys_net_dial`), configured to **TLS 1.3 only**, the **single suite**,
  **server-auth only**, validating the chain against a **small bundled root
  store** (the demo server's trust anchor — *not* a full system store).
- On handshake completion, extract the negotiated traffic secrets / key material
  and hand them to the kernel via `sys_net_tcp_enable_tls`; thereafter
  application bytes flow as plaintext through the normal send/recv syscalls and
  the kernel record layer does the crypto.
- ALPN advertises **http/1.1**; a minimal HTTP/1.1 `GET` helper issues the
  request and reads the response over the now-encrypted socket.

**Anti-scope:** no client certs; no full/system trust store (FM-5-4); no HTTP/2;
no TLS 1.2.

**ISCs**
- **ISC-5d-1:** TLS 1.3 handshake to the in-harness server completes; the certificate chain validates against the bundled root; the negotiated suite is TLS_CHACHA20_POLY1305_SHA256.
- **ISC-5d-2:** post-handshake, an HTTP/1.1 GET returns the expected response body decrypted through the kernel record layer.
- **ISC-5d-3:** bad certificate, wrong server name, and a TLS-1.2-downgrade attempt each fail **closed** (no plaintext exchanged).

---

## BUILD-5e — HTTPS ship scenario + full sweep + bare-metal marker

**New:** `phase4_https_fetch` scenario in the test harness + the in-harness DNS
responder and loopback TLS server fixtures; `Plans/BUILD-5e-BARE-METAL-DEFERRAL-…`.
**Touch:** `tools/xtask` scenario registration; `kernel/Cargo.toml` feature flag.

**Scope**
- End-to-end deterministic scenario: program calls `sys_net_dial("name")` →
  resolve via the **in-harness DNS responder** → HE race over the returned
  dual candidate set → TLS 1.3 handshake against the **loopback/in-harness TLS
  server** → HTTP/1.1 GET → assert the **decrypted response body**.
- Full `neo-test all` sweep at `NEO_SMP=4 ATTEMPTS=1`; all prior phase-4
  scenarios regress green.
- Bare-metal deferral marker: real external **dual-stack** HTTPS fetch,
  QEMU SLIRP / real outbound, AES-GCM (FM-5-3), TCP/53 (FM-5-1), DoT/DoH,
  full trust store (FM-5-4).

**ISCs**
- **ISC-5e-1:** `phase4_https_fetch` passes SMOKE + NEO_SMP=4; the asserted plaintext body matches the fixture server's response exactly.
- **ISC-5e-2:** full `neo-test all` green at ATTEMPTS=1; no regression across BUILD-1–4.5 scenarios; panic/unwrap/expect scan and `git diff --check` clean.
- **ISC-5e-3:** bare-metal deferral marker filed with the deferred-scope list.

---

## Close gate

All 18 ISCs PASS; full `neo-test all` NEO_SMP=4 green at ATTEMPTS=1;
no hand-rolled crypto primitive in the diff; record layer synchronous-per-packet
and copy-not-share by construction; FM-3-1 discharged; FM-5-1…5-4 filed.
BUILD-5 close **discharges Phase 4.**
