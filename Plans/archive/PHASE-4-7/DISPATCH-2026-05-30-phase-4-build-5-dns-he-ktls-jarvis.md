# DISPATCH: Phase 4 BUILD-5 — DNS + Happy Eyeballs + kTLS → Jarvis

**Date:** 2026-05-30
**From:** PM (Cerebro)
**To:** Jarvis
**Status:** live
**Authority:** PM-DECISION + PL-RATIFICATION + PLAN (2026-05-30, phase-4-build-5)

---

## Mission

Ship Phase 4. A userspace program resolves a hostname, races a dual-stack
connection (Happy Eyeballs), completes a TLS 1.3 handshake, and performs an
HTTPS GET — on NeoPlutarch's own ABI. Five sub-commits, 18 ISCs.

## Branch

Branch `codex/phase-4-build-5-dns-he-ktls-jarvis` from `origin/main`
(`23f75a3`, BUILD-4.5 landed). Read, in order:
`PM-DECISION-2026-05-30-phase-4-build-5-opening.md`,
`PLAN-2026-05-30-phase-4-build-5-dns-he-ktls.md`. The PLAN is the contract;
this dispatch is the working checklist.

## The one cardinal rule of this build

**Hand-roll the control plane; never hand-roll the cryptography.** DNS, Happy
Eyeballs, the RFC 8446 record *framing* — hand-rolled, mirroring prior builds.
The AEAD primitive is RustCrypto `chacha20poly1305`; the handshake is `rustls`.
If you find yourself writing a cipher, a hash, or a KX, **stop** — that is a
§10 escalation, not a sub-commit.

## Sub-commit checklist

### 5a — DNS resolver core + `ResolverConfig`
- [ ] `net/dns.rs`: RFC 1035 A/AAAA codec (defensive compression-pointer walk), EDNS(0) OPT, UDP/53 query, TTL answer cache (`lookup`/`insert`/`age_out`/`tick`), `inject` hook.
- [ ] Lift **RDNSS (RA option 25, RFC 8106)** in `icmpv6.rs` (the current `_ => {}` arm); build `ResolverConfig` from RDNSS **and** DHCPv4 `Lease.dns`.
- [ ] Keep address-discovery (servers) separable from query-core (codec/cache).
- [ ] `sys_net_resolve` → `&[NetCandidate]`; register `dns::tick` in `net::tick`.
- [ ] ISC-5a-1…4. **Host-testable.**

**Commit:** `build-5a: dns stub resolver (rfc 1035 a/aaaa) + resolver config (rdnss + dhcp)`

### 5b — Happy Eyeballs (RFC 8305) + RFC 6724 dest default
- [ ] RFC 6724 prefer-IPv6 destination sort over the candidate list — **closes FM-3-1**.
- [ ] RFC 8305 race: parallel A/AAAA, ~50 ms Resolution Delay (AAAA-first), ~250 ms staggered Connection Attempt Delay, first-wins, cancel + reclaim losers. Harness-tunable delays.
- [ ] `sys_net_dial(name, port)` → connected `TcpSocket`.
- [ ] Leave single-tuple `sys_net_connect_tcp` (`0x188`) untouched.
- [ ] ISC-5b-1…4. **Host-testable (with tunable delays); QEMU for the race.**

**Commit:** `build-5b: happy eyeballs (rfc 8305) + rfc 6724 prefer-ipv6 dest (closes fm-3-1)`

### 5c — kTLS record layer (ChaCha20-Poly1305, synchronous per-packet)
- [ ] Add `chacha20poly1305` (no_std, `default-features = false`) to `kernel/Cargo.toml`.
- [ ] `net/tls_record.rs`: TLS 1.3 record seal/open (per-record nonce seq⊕iv, AAD), kernel-private per-direction `{key, iv, seq}`.
- [ ] `sys_net_tcp_enable_tls`: **WRITE-right gated**; key material copied in then **zeroized**; never parsed as metadata; state not userspace-readable after enable.
- [ ] Transform is **synchronous, per-packet, in the net-task context** — no workqueue / shared-mutable / async callback. Buffers **copied, not shared**.
- [ ] Auth/format failure → connection error, **never panic**.
- [ ] ISC-5c-1…4. **Host-testable (KAT vectors + cap-rights).**

**Commit:** `build-5c: ktls record layer — chacha20-poly1305 synchronous per-packet (rfc 8446)`

### 5d — userspace TLS 1.3 handshake + wiring + HTTP/1.1
- [ ] Add `rustls` (TLS 1.3 only, single suite, no_std-compatible provider for ChaCha20-Poly1305 / SHA-256 / KX) to `userland/libneo/Cargo.toml`.
- [ ] `libneo/src/tls.rs`: client handshake over a dialed `TcpSocket`, **server-auth only**, validate against a **small bundled root store**; hand traffic secrets to `sys_net_tcp_enable_tls`.
- [ ] ALPN http/1.1; minimal HTTP/1.1 GET helper over the encrypted socket.
- [ ] ISC-5d-1…3. **QEMU (against the 5e in-harness server).**

**Commit:** `build-5d: userspace rustls tls 1.3 handshake + enable_tls wiring + http/1.1`

### 5e — HTTPS ship scenario + sweep + bare-metal marker
- [ ] `phase4_https_fetch`: in-harness DNS responder + loopback TLS server; resolve → HE → TLS → GET → assert decrypted body.
- [ ] Full `neo-test all` at `NEO_SMP=4 ATTEMPTS=1`; all prior phase-4 scenarios green.
- [ ] `Plans/BUILD-5e-BARE-METAL-DEFERRAL-2026-05-30.md`: real external dual-stack fetch, SLIRP/outbound, AES-GCM (FM-5-3), TCP/53 (FM-5-1), HEv3 (FM-5-2), DoT/DoH, full trust store (FM-5-4).
- [ ] ISC-5e-1…3. **QEMU.**

**Commit:** `build-5e: https-fetch ship scenario + full sweep + bare-metal marker`

## Sequencing note

5a → 5b → 5c are independent of the handshake and **host-testable**; land and
verify them first. 5c proves the crypto boundary **before** 5d brings the heavy
`rustls` integration — do not reorder. **Pause for PM after 5c** (the crypto
boundary checkpoint, like the 4.5a pause) so we confirm the record layer before
the handshake stack lands.

## Per-sub-commit hygiene (every commit)

- SPDX header on every new file (`.rs` → `// SPDX-License-Identifier: MPL-2.0` first line).
- `git diff --check` clean.
- `git diff -U0 | rg -n "panic!|todo!|unimplemented!|unwrap\(|expect\("` → no new matches.
- `rustfmt --check` on touched files.
- Feature-gated `cargo check -p neoplutarch-kernel` for the relevant scenario feature.

## Failure modes to avoid

1. **Rolling your own crypto.** The cardinal rule. AEAD = `chacha20poly1305`; handshake = `rustls`. No exceptions without a §10 escalation.
2. **Async/workqueue record layer.** The transform is synchronous-per-packet in the net-task context. This is the entire point of the kTLS posture (Copy Fail PM-NOTE). Do not introduce a completion callback or shared-mutable record state.
3. **Sharing userspace buffers under crypto.** Copy in, zeroize key material, no splice/zero-copy. (Copy Fail §3.2/§3.3.)
4. **Ungated `enable_tls`.** WRITE right is mandatory; READ-only must be rejected.
5. **Scope creep:** no AES-GCM, no TCP/53 fallback, no client certs, no full trust store, no HTTP/2, no DNSSEC/DoT/DoH, no HEv3, no key update. File forward-markers; do not build.
6. **Real outbound in CI.** The deterministic scenario is in-harness DNS + loopback TLS. Real external fetch is the bare-metal marker. SLIRP-in-CI needs a separate decision.
7. **Wall-clock-bound HE tests.** Expose tunable delays so the harness doesn't sit on 250 ms timers.

## RETURN

When all five land + QEMU-verified, send the RETURN with: per-ISC PASS table,
the `phase4_https_fetch` serial excerpt (resolve → HE pick → handshake →
decrypted body), confirmation the diff contains **no hand-rolled crypto
primitive** and the record layer is synchronous-per-packet, FM-3-1 discharge
confirmation, and the FM-5-1…5-4 forward-markers. PM dispositions, writes the
BUILD-5 close report, and lands the Phase 4 ship.
