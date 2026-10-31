# PL-RATIFICATION: Phase 4 BUILD-5 PM-DECISION — §9 ratified

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Status:** RATIFIED — unblocks the BUILD-5 PLAN + DISPATCH
**References:**
- Plans/PM-DECISION-2026-05-30-phase-4-build-5-opening.md (§9)

---

## Decision

All seven §9 load-bearing points **and** the 5a–5e sub-commit split are
**ratified as recommended**. No widening.

| # | Point | Ratified |
|---|-------|----------|
| 1 | Q1 — hand-rolled RFC 1035 stub (A/AAAA); `ResolverConfig` from RDNSS + DHCP DNS | ✓ |
| 2 | Q2 — UDP/53 + EDNS(0) + minimal TTL cache; TCP/53, DNSSEC, DoT/DoH deferred; address-discovery layer separable | ✓ |
| 3 | Q3 — HEv2 (RFC 8305) + RFC 6724 prefer-IPv6 dest default (closes FM-3-1); `&[NetCandidate]` API; HEv3 deferred | ✓ |
| 4 | Q4 — hybrid kTLS: userspace handshake / kernel synchronous-per-packet record layer; key opaque; copy-not-share; WRITE-gated enable | ✓ |
| 5 | Q5 — RustCrypto `chacha20poly1305`; TLS_CHACHA20_POLY1305_SHA256 single mandatory suite; **no hand-rolled crypto**; AES-GCM deferred (FM-5-3) | ✓ |
| 6 | Q6 — TLS 1.3 server-auth only; `rustls` in `libneo`; bundled root store; ALPN http/1.1; hand-written-handshake fallback = §10 escalation only | ✓ |
| 7 | Q7 — in-harness DNS + loopback TLS server for CI; real external dual-stack fetch → bare-metal; SLIRP-in-CI = separate decision | ✓ |
| 8 | Sub-commit split 5a–5e (§4) as the build contract | ✓ |

## Effect

- The **PLAN** (`PLAN-2026-05-30-phase-4-build-5-dns-he-ktls.md`) and **DISPATCH**
  (`DISPATCH-2026-05-30-phase-4-build-5-dns-he-ktls-jarvis.md`) are authorized
  and delivered in this package.
- Cardinal guardrail affirmed: **the control plane is hand-rolled; the
  cryptography is delegated to vetted primitives.** Any hand-rolled crypto
  primitive, zero-copy crypto path, client-cert/full-trust-store scope, AES-GCM
  in the ship, or real-outbound CI networking requires a fresh §10 decision.
- New forward-markers FM-5-1 (TCP/53), FM-5-2 (HEv3), FM-5-3 (AES-GCM),
  FM-5-4 (full trust store) are accepted as deferred.
- PM (Cerebro) remains in the PM seat for the RETURN → disposition → merge.
- BUILD-5 is the Phase 4 ship; its close discharges Phase 4.
