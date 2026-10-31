# PM-NOTE ADDENDUM - Phase 4 Network Design Input: Citation Refreshes Per Cerebro OBSERVE

Date: 2026-05-20
From: PM
Status: ADDITIVE TO `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
Subject: Three citation refreshes surfaced by Cerebro's Phase 4 OBSERVE pass. Substantive positions unchanged; only citations and one design-space refinement are updated.

Authority:

- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md` (original)
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` §9
  (research source of refreshes)
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md`
  §4.1 (this addendum authorized at OBSERVE consumption)

## 1. Disposition

The original PM-NOTE is preserved unchanged as historical record of PM's
2026-05-20 framing. This addendum is additive. When Phase 4 THINK opens, the
authority chain includes both artifacts; the addendum's refreshes supersede
the original PM-NOTE's specific citations where they differ.

The original PM-NOTE's six substantive positions (dual-stack mandatory,
IPv6-preferred with Happy Eyeballs + RFC 6724, TLS as security primitive
with IPsec deferred, NAT64/DNS64 deferred, no BSD sockets, capability-
protected sockets-equivalent) all stand confirmed by Cerebro's research per
the OBSERVE artifact §8. No substantive position requires retraction.

## 2. Citation Refreshes

### 2.1 IPv6 Node Requirements: RFC 6434 → RFC 8504

The original PM-NOTE §2.1 cited RFC 6434 (Dec 2011) for the IPsec
SHOULD-not-MUST posture. Cerebro's research confirms RFC 6434 has been
superseded by **RFC 8504** (Jan 2019), which is the current IPv6 Node
Requirements RFC.

The substantive position — that IPsec is SHOULD-not-MUST in IPv6 node
requirements and that NeoPlutarch should not commit to mandatory IPsec —
is unchanged. RFC 8504 preserves the SHOULD posture from RFC 6434.

**Phase 4 THINK substrate-reading list should reference RFC 8504, not RFC
6434.**

### 2.2 DHCPv6: RFC 8415 → RFC 9915

The original PM-NOTE §3 listed RFC 8415 in the DHCPv6 reference space.
Cerebro's research confirms **RFC 9915** (Jan 2026) is the current DHCPv6
spec. Notable changes from RFC 8415:

- IA_TA (Identity Association for Temporary Addresses via DHCP) has been
  removed. Temporary address generation belongs to SLAAC privacy
  extensions (RFC 8981), not DHCP. NeoPlutarch's Phase 4 DHCPv6 client (if
  implemented) targets RFC 9915 directly without IA_TA.
- Server Unicast option removed. Phase 4 DHCPv6 client speaks multicast
  only, simplifying the implementation surface.

The substantive position — that DHCPv6 is a Phase 4 candidate alongside
SLAAC, with placement (kernel-resident vs userspace) deferred to THINK —
is unchanged. Phase 4 THINK should target RFC 9915.

### 2.3 Happy Eyeballs: RFC 8305 → draft-ietf-happy-happyeyeballs-v3-03

The original PM-NOTE §2.2 cited RFC 8305 (Happy Eyeballs v2) as the
load-bearing connection-racing mechanism for dual-stack endpoints.
Cerebro's research surfaced an in-flight successor: **draft-ietf-happy-
happyeyeballs-v3-03** (March 2026), which extends RFC 8305 with:

- **SVCB / HTTPS resource record support.** SVCB (RFC 9460) and HTTPS
  records carry protocol-priority and alpn hints, allowing endpoints to
  negotiate protocol preference (HTTP/3 over QUIC vs HTTP/2 over TCP)
  before connection establishment. Happy Eyeballs v3 incorporates this
  into the connection-racing algorithm.
- **QUIC-before-TCP attempts.** When SVCB hints indicate QUIC
  availability, v3 races QUIC before falling back to TCP, in addition to
  the IPv6-before-IPv4 race that v2 specified.

**This is more than a citation refresh — it is a design-space refinement.**
The sockets-equivalent API NeoPlutarch designs in Phase 4 should
accommodate a **protocol-priority axis**, not just an A/AAAA address-family
axis. Specifically:

- Connection-establishment objects need to carry not just `(family, addr,
  port)` but also `(protocol, alpn, transport)` to support the v3 racing
  algorithm.
- The capability handle for a connection-in-progress may transition
  protocol/transport (QUIC vs TCP) before establishment completes; the
  capability shape needs to reflect this.

PM does NOT pre-resolve the design question of how the sockets-equivalent
API exposes protocol priority. That is Phase 4 THINK's call. But the
design-space implication is recorded here so THINK can plan for it.

Caveat: Happy Eyeballs v3 is a working draft, not a published RFC. The
draft may shift between now and when Phase 4 THINK formally opens.
Phase 4 THINK should re-check the document's status at THINK opening and
target either the published RFC (if available) or the most recent
draft revision.

## 3. What This Addendum Does NOT Do

- Does NOT modify the original PM-NOTE in place. The original stands as
  historical record.
- Does NOT change any of the six substantive positions (a–f) the original
  PM-NOTE proposed. All six remain proposed-and-confirmed.
- Does NOT open Phase 4 THINK. THINK still waits on Phase 3 closure per
  the rung-by-rung sequencing decision.
- Does NOT propose any Bible amendment.
- Does NOT propose syscall ABI or implementation choices.
- Does NOT bind Phase 4 THINK to any specific protocol-priority API
  shape. The Happy Eyeballs v3 finding is a constraint on the design
  space, not a chosen solution.

## 4. Phase 4 THINK Authority Chain After This Addendum

When Phase 4 THINK formally opens (post Phase 3 closure), the canonical
authority chain is:

- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
  (original PL design input + PM commentary)
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md`
  (this artifact — citation refreshes + Happy Eyeballs v3 design-space
  refinement)
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md`
  (Cerebro's research)
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md`
  (sequencing decision)
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md`
  (OBSERVE consumption)
- Bible Article 11, 9.10, 12, 3
- Phase 3 close artifact (created at Phase 3 closure)

## 5. Lane Status (Unchanged)

- Jarvis: BUILD-5a-NVMe active.
- Cerebro: idle (Phase 4 OBSERVE closed).
- Prometheus: idle.
