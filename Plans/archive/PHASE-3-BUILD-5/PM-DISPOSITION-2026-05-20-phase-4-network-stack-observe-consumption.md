# PM Disposition - Phase 4 Network Stack OBSERVE Artifact Consumption

Date: 2026-05-20
From: PM
Status: ACCEPTED / OBSERVE CLOSED / ARTIFACT ENTERS PHASE 4 THINK AUTHORITY CHAIN
Consumed artifact:

```text
Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md
```

Authority: `Plans/DISPATCH-2026-05-20-phase-4-network-stack-observe-cerebro.md`

## 1. Disposition

PM accepts Cerebro's Phase 4 network stack OBSERVE artifact. All dispatch
acceptance gates are met. The artifact is informational per Bible Article
18.1 and enters Phase 4 THINK's authority chain pending Phase 4 THINK's
formal opening (post Phase 3 closure per the rung-by-rung sequencing
decision).

## 2. Gate Verification

PM walked the artifact against the dispatch contract:

| Dispatch requirement (§2.1) | Status |
|---|---|
| §A — Existing kernel network stacks (6 subjects: smoltcp, Fuchsia Netstack3, Hubris, seL4-derived, Theseus, Linux as comparison-only) | ✓ §3 covers all six |
| §B — RFC survey with current status per document | ✓ §4 covers 14 RFCs |
| §C — Capability-shaped socket API design space | ✓ §5 covers 6 patterns |
| §D — Design questions Phase 4 THINK will need to resolve | ✓ §6 enumerates 14 questions |
| §E — Where Bible/ADR posture pre-resolves design space | ✓ §7 enumerates 6 items |
| §F — Confirm or refute PM-NOTE proposed positions a–f | ✓ §8 addresses all six explicitly |
| Suggested corrections to PM-NOTE | ✓ §9 surfaces three citation refreshes |
| Design-questions-for-THINK enumeration | ✓ §10 captures THINK queue |
| References list | ✓ §11 |
| Verbatim closure per dispatch §6 | ✓ §12 blockquoted at line 628, lowercase |

Dispatch anti-scope (§2.2) was honored:

- No design decisions made.
- No implementation proposals.
- No Bible amendment proposals.
- No ADR or ANSPEC drafting.
- No prescriptive language; indicative/interrogative mood preserved.
- No Phase 5+ AI subsystem bleed.

Dispatch stop conditions (§3): none triggered.

- §3.5 flagged Theseus networking detail as thin, but this is consistent
  with ADR-022's positioning of Theseus as forward-marker reference rather
  than load-bearing precedent. PM accepts the thin coverage; deeper Theseus
  networking research is not necessary for Phase 4 THINK.

Length: 634 lines (well under the §3 stop-condition ceiling of 3000 lines).
This is comprehensive-but-not-exhaustive, the right OBSERVE shape.

## 3. Substantive Findings

### 3.1 PM-NOTE positions a–f all substantively confirmed

Cerebro's research confirms all six positions PM proposed in the Phase 4
design-input PM-NOTE:

a. Dual-stack mandatory — confirmed against current routing landscape;
b. IPv6-preferred with Happy Eyeballs + RFC 6724 — confirmed as current
   practice across Chrome, Firefox, Windows, macOS, Linux;
c. TLS as security primitive (IPsec deferred to Phase 8+) — confirmed
   against current IETF posture; IPsec usage in consumer/server
   deployments remains rare;
d. NAT64/DNS64 deferred to Phase 8+ — confirmed as operator-side
   infrastructure concern, not endpoint-OS concern;
e. No BSD sockets — confirmed as compatible with all surveyed
   capability-shaped designs (Fuchsia FIDL, Hubris IPC, Tock tokens,
   Plan 9 namespace, Capsicum);
f. Capability-protected sockets-equivalent — confirmed as the
   prevailing approach in modern Rust OS designs.

No PM-NOTE position requires retraction. The substrate is sound.

### 3.2 Three citation refreshes surfaced

Cerebro's research surfaced three IETF updates that the PM-NOTE's
citations should reflect:

1. **RFC 6434 (Dec 2011) → RFC 8504 (Jan 2019).** RFC 8504 is the
   current IPv6 Node Requirements RFC, superseding 6434. The
   SHOULD-not-MUST posture on IPsec is preserved; the citation is just
   stale.
2. **RFC 8415 → RFC 9915 (Jan 2026).** Current DHCPv6 spec. RFC 9915
   removed IA_TA (temporary addresses via DHCP) and Server Unicast
   options. Neither was load-bearing for the PM-NOTE's positions, but
   Phase 4 THINK should target 9915.
3. **draft-ietf-happy-happyeyeballs-v3-03 (March 2026).** Extends RFC
   8305 with SVCB/HTTPS record support and QUIC-before-TCP attempts.
   **This is the most consequential finding** — not just a citation
   refresh but a design-space refinement: the sockets-equivalent API
   design needs to accommodate a protocol-priority axis, not just
   A/AAAA pairs. Phase 4 THINK should plan for this.

PM addresses these in a separate addendum to the PM-NOTE (see §4).

### 3.3 Capability-shape design poles identified

Cerebro's §5 surveyed six capability-shaped socket designs and
identified clear poles for Phase 4 THINK to choose between:

- **Fuchsia FIDL channels** — sockets backed by typed IPC channels;
  protocol-aware capability handles.
- **Hubris compile-time binding** — networking capabilities resolved
  at task-graph compile time; no dynamic socket creation.
- **Tock typed tokens** — capability tokens carry protocol type info;
  embedded-leaning.
- **Plan 9 namespace** — sockets as files in a hierarchical
  namespace; per-connection file objects.
- **Capsicum per-fd rights** — file-descriptor-shaped capabilities
  with attenuable rights bits.

These are genuinely different design poles. Phase 4 THINK will need
to choose (or synthesize). PM observes this is the most consequential
single decision Phase 4 THINK will make; it shapes the entire
syscall-surface for networking.

### 3.4 14 design questions captured for THINK

Cerebro's §10 enumerated 14 design questions for Phase 4 THINK to
resolve. PM does not pre-resolve any of them in this disposition;
that's THINK's job. The list will be the canonical THINK question
queue when THINK formally opens.

## 4. Forward Action

### 4.1 PM-NOTE addendum (citation refreshes)

PM files a small addendum to the Phase 4 design-input PM-NOTE
recording the three citation refreshes and the Happy Eyeballs v3
design-space refinement:

```text
Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md
```

The original PM-NOTE is preserved unchanged as historical record. The
addendum is additive and explicitly references the OBSERVE artifact
as its source of authority for the refreshes.

### 4.2 Cerebro lane closure

Cerebro's Phase 4 OBSERVE lane is closed. Cerebro is idle and
available for whatever PL signals next. The OBSERVE artifact lives in
the project's authority chain awaiting Phase 4 THINK's formal
opening.

### 4.3 Phase 4 THINK opening trigger (preserved)

The trigger for Phase 4 THINK formal opening remains unchanged:
Phase 3 closure (BUILD-5a-NVMe + BUILD-5b + Phase 3 close artifact).
BUILD-5b is hardware-gated on PL's T480s 20L8 acquisition path.

When Phase 4 THINK opens, the authority chain will include:

- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md`
  (PL design input + PM commentary)
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md`
  (citation refreshes from this OBSERVE pass)
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md`
  (this consumed artifact)
- `Plans/PM-NOTE-2026-05-20-post-phase-3-sequencing-rung-by-rung.md`
  (sequencing decision)
- Bible Article 11, 9.10, 12, 3 (current network posture)
- Phase 3 close artifact (substrate state, to be created at Phase 3
  closure)

## 5. Lane Status

After this disposition:

- **Jarvis:** BUILD-5a-NVMe active.
- **Cerebro:** Phase 4 OBSERVE closed (this disposition), idle. Available
  for further research lanes or for eventual Phase 4 THINK dispatch.
- **Prometheus:** idle.

The day's productive lanes (Jarvis storage driver + Cerebro Phase 4
research) yielded two parallel substantive closures with zero shared
substrate. The parallel-lane experiment worked as designed.

## 6. PM Posture

PM consumes the OBSERVE artifact and acknowledges its quality. Cerebro's
research depth matched the prior ALTIMIT research artifact's quality
bar. The artifact will be a load-bearing reference when Phase 4 THINK
opens.

PM stands ready for:

- Jarvis's BUILD-5a-NVMe return (next major adjudication moment).
- PL's BUILD-5b hardware-acquisition signal.
- PL's signal on any further pre-Phase-4-THINK research lanes (e.g. a
  similar OBSERVE pass on Phase 5 AI subsystem work, given that Phase
  5 also has substantial existing substrate via ANSPEC-001/002).
- Any other PL direction.
