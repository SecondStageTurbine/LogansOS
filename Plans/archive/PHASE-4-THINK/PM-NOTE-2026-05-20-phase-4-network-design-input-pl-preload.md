# PM-NOTE - Phase 4 Network Design Input (PL Preload)

Date: 2026-05-20
From: PM (capturing PL design input)
Status: PRESERVED FOR PHASE 4 THINK / NOT YET NORMATIVE
Subject: PL pre-loaded design guidance for the Phase 4 network stack THINK substrate. This note exists so the input is preserved between now and Phase 4 THINK opening (after BUILD-5b closes), and is not lost across PM seats or working sessions.

Authority:

- `docs/bible.md` v1.6.0 Article 11 (Networking)
- PL input 2026-05-20: dual-stack as hard requirement, IPv6-first with v4 graceful fallback
- PL signal 2026-05-20: "rung by rung" — Phase 4 THINK opens before Phase 5 THINK

## 1. PL Design Input Summary

PL surfaced the following design guidance for the Phase 4 network stack:

a. **Dual-stack as a hard requirement.** Both IPv4 and IPv6 first-class.
   Neither relegated to "fallback library status." Real-world routing
   landscape will not be pure-IPv6 within the project's design horizon;
   an IPv6-only OS fails Bible Article 28's "usable by real people"
   test.

b. **IPv6-first with graceful IPv4 fallback.** Connections prefer IPv6
   when both AAAA and A records resolve, with deterministic fallback
   when v6 fails. Aligns with existing Bible Article 11.2.

c. **Transition mechanisms** for IPv4↔IPv6 communication on partially-
   upgraded networks.

## 2. PM Adjustments And Cautions

PM commentary on PL's input, captured here so Phase 4 THINK starts with
both the input and the engineering refinements:

### 2.1 Mandatory IPsec is no longer current

PL's input echoed a common misconception: that "IPv6 features built-in
security features like mandatory IPsec." This was true in the IPv6
specs of the late 1990s. **RFC 6434 (December 2011) explicitly
downgraded IPsec from MUST to SHOULD in IPv6 node requirements.**

In practice, IPsec is rarely used on consumer-grade IPv6 deployments
in 2026. Security comes from TLS at the application layer, just like
IPv4.

NeoPlutarch should NOT commit to mandatory IPsec on the basis of the
older IPv6 framing. The TLS-at-application-layer story Bible Article
11.3 already covers ("TLS is a kernel concern at least to the extent
that the inference subsystem may need authenticated model downloads")
is the correct posture. IPsec can be a Phase 8+ optional capability if
a concrete driver emerges.

### 2.2 Happy Eyeballs (RFC 8305), not NAT64, is the load-bearing dual-stack mechanism for an endpoint OS

PL's input mentioned NAT64/DNS64 as a transition helper. Worth
distinguishing the use cases:

- **Happy Eyeballs (RFC 8305).** When a hostname resolves to both A and
  AAAA records, try both in parallel with a small head-start for IPv6,
  take whichever connects first. This is the dominant client-side
  dual-stack pattern and is what makes IPv6 deployment painless on
  Chrome, Firefox, modern macOS, etc. **This is the load-bearing
  mechanism for NeoPlutarch as an endpoint OS.**
- **NAT64/DNS64.** Requires a NAT64 gateway on the network (operator-
  side infrastructure). Mostly useful for IPv6-only clients reaching
  IPv4-only servers. Niche case for a workstation/laptop OS; matters
  more for mobile carriers and certain enterprise deployments. Phase
  8+ when a concrete deployment needs it.
- **464XLAT / DS-Lite.** Relevant for IPv6-only network deployments
  needing IPv4-application support. Also Phase 8+.

For Phase 4 scope: **Happy Eyeballs is the load-bearing mechanism.**
NAT64/DNS64 deferred to Phase 8+.

### 2.3 RFC 6724 address selection

Closely related to Happy Eyeballs: RFC 6724 specifies the deterministic
algorithm for selecting source and destination addresses when multiple
candidates exist. This is what makes "IPv6-preferred" actually work
deterministically across implementations.

PM directs that RFC 6724 be in the Phase 4 THINK's substrate-reading
list when THINK opens.

## 3. PL's Original Linked Sources

PL provided the following sources in the design input:

- apalrd.net IPv6 post (personal blog; reasonable but IPv6-purist-skewed)
- spiceworks IPv6 community discussion (older, repeats outdated
  IPsec-mandatory line)
- serverfault question on new-service IPv4-vs-IPv6 decision
- AWS IPv4/IPv6 comparison
- biztechmagazine 2017 article (older content; some claims need
  re-verification against current IETF posture)
- ir.com IPv4-IPv6 transition guide
- IETF IPv6 Working Group
- Google IPv6 Statistics

PM directs that the IETF IPv6 Working Group and Google IPv6 Statistics
sources be treated as canonical reference. Industry blogs and 2017-era
articles should be cross-checked against current RFCs (notably RFC
6724, RFC 8305, RFC 6434) before being treated as normative.

## 4. What Phase 4 THINK Should Eventually Commit To

PM proposed positions (subject to Cerebro research and PL ratification
when Phase 4 THINK opens):

a. **Dual-stack mandatory.** Both IPv4 and IPv6 first-class.
b. **IPv6-preferred with Happy Eyeballs (RFC 8305) for connection
   establishment.** RFC 6724 for address selection.
c. **TLS as the security primitive.** IPsec deferred to Phase 8+
   opt-in capability.
d. **NAT64/DNS64 transition support deferred** to Phase 8+ until a
   concrete deployment requires it.
e. **No BSD sockets.** Bible Article 11.2 already names this; new API
   design is part of Phase 4 THINK's job.
f. **In-kernel TCP/UDP/ICMP.** Bible Article 11.2 names "smoltcp-derived
   or original."
g. **Capability-based network access.** Sockets-equivalent objects are
   capability-protected like every other kernel object per Bible
   Article 12.

These are PM's reading of the existing Bible posture plus the PL design
input plus current IETF state. Cerebro's THINK research will refine or
revise.

## 5. Disposition

This note is informational only. It binds nothing. It preserves PL's
design input and PM's commentary for the Phase 4 THINK substrate.

When Phase 4 THINK opens (after BUILD-5b closes per the rung-by-rung
sequencing PL signaled), this note becomes part of THINK's authority
chain. The PM seat at that time may agree or disagree with §2 and §4
positions; the input itself is the durable contribution.

## 6. Lane Status

This note does NOT open Phase 4 THINK now. Phase 4 THINK waits for
BUILD-5b (bare-metal validation) closure, which itself waits for PL
hardware acquisition.

Lanes after this note: unchanged.

- **Jarvis:** BUILD-5a-NVMe active.
- **Cerebro:** idle.
- **Prometheus:** idle.
