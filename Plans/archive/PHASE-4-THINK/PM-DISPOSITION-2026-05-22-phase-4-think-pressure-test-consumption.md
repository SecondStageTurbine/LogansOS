# PM-DISPOSITION: Phase 4 THINK Pressure-Test Consumption

**Date:** 2026-05-22
**Recorder:** PM
**Consumes:** `Plans/PRESSURE-TEST-2026-05-22-phase-4-think-cerebro.md`
**Closes-out:** `Plans/DISPATCH-2026-05-22-phase-4-think-pressure-test-cerebro.md`
**Status:** ACCEPTED — THINK v0.2 to follow per §3 below

---

## 1. Acceptance

PM accepts Cerebro's pressure-test artifact. The dispatch transitions from ACTIVE to CLOSED.

Acceptance criteria per dispatch §10:

| A | Item | Status |
|---|------|--------|
| A1 | Artifact at `Plans/PRESSURE-TEST-2026-05-XX-phase-4-think-cerebro.md` per §5 structure | ✅ |
| A2 | All 14 §4 lines have findings or stop-condition notes | ✅ — none skipped |
| A3 | No Bible amendment, no ADR draft, no ABI proposal | ✅ |
| A4 | Primary-source citation discipline at OBSERVE bar | ✅ — explicit "Cannot verify from primary source" notations where applicable |
| A5 | Length within §5 bounds, or stop condition surfaced | ✅ — 649 lines, within 600–1000 |
| A6 | No unverifiable citations | ✅ — verifiable URLs throughout; gaps marked |

Cerebro's adversarial-calibration note (TL;DR §1) is well-disciplined: the 0 REFUTED count is documented as "adversarial pass did not surface refuting evidence within budget," not as "no refuting evidence exists." This is the right epistemic posture for a pressure-test.

## 2. Net Verdict Across 14 Lines

Per the pressure-test §1 enumeration table:

- **8 CONFIRMED:** §4.5 (UDP-only BUILD-1), §4.6 (loopback short-circuit), §4.7a (Privacy Extensions default-on), §4.8 (notification-on-activity), §4.9 (CUBIC default), §4.10 (IPv6-only BUILD-1 test target), §4.12 (Phase 2 capability infrastructure), §4.13 (IETF corpus delta — no material change).
- **5 REFINABLE:** §4.1 (sockets-equivalent shape), §4.2 (sub-BUILD sequencing), §4.3 (kTLS placement), §4.4 (kernel-resident DNS), §4.11 (smoltcp wrap/fork/scratch).
- **1 CONSIDER-REVISITING:** §4.7b (capability-handle lifetime across temporary-address rotation).
- **0 REFUTED.**
- **6 added risks** R11–R16 with mitigations and interaction-with-R1–R10 table.

**No PM-NOTE confirmed position (a)–(f) is contradicted.** This is the critical check; all six remain CONFIRMED per Cerebro's own OBSERVE §8 and re-confirmed in this pass.

**No stop condition was hit.**

## 3. Disposition Plan

PM files **PHASE-4-THINK v0.2** absorbing the pressure-test findings. The v0.2 is not a fundamental restructure (no REFUTED verdicts); it is an update pass that:

### 3.1 Absorbs the five REFINABLE refinements

- **§3.1 — sockets-equivalent shape (pressure-test §2.1 / Cerebro Q1).** Reframe the "forward-accommodates UDP and TCP without API-shape revisions" claim as a BUILD-3 design hypothesis to validate, not a precedent-supported certainty. The Hubris precedent (UDP-only, never staged to TCP) does not testify to forward-compatibility; the Redox precedent demonstrates additive scheme-method additions when UDP semantics needed extending. Add a speculative `sys_net_socket_tcp` sketch at BUILD-1 PLAN time per Cerebro's mitigation.
- **§4 — sub-BUILD sequencing (pressure-test §2.2 / Cerebro Q2).** Add a brief acknowledgment that IPv6-before-IPv4 lacks documented precedent in clean-room kernels, and that BUILD-1→BUILD-2 should be planned with the expectation of surprise rather than the easier expectation of an additive overlay. No sequence change.
- **§3.3 — TLS placement (pressure-test §2.3 / Cerebro Q3).** Acknowledge that "matches Linux kTLS" inherits Linux kTLS's documented failure-mode profile (7 CVEs across 2024–2025). Link to ADR-022 cell-swap as candidate revisit-trigger for BUILD-5's TLS state. No placement change.
- **§3.4 — DNS placement (pressure-test §2.4 / Cerebro Q4).** Acknowledge that the 2025–2026 encrypted-DNS direction (RFC 9461/9462, EDSR, U.S. EO, Firefox >85% DoH) carries a forward-marker dependency tighter than the typical Phase 5+ horizon. Plan a BUILD-4 sub-commit (BUILD-4d or similar) that hands the resolver's address-discovery responsibility to a configurable layer so DoT/DoH endpoints can later be supplied without revising the resolver core. The kernel-resident stub remains; the discovery layer becomes extensible.
- **§5 R1 — smoltcp wrap-vs-fork-vs-scratch (pressure-test §2.11 / Cerebro Q8).** Update R1 mitigation framing with: (a) v0.13.0 release evidence contests the "minimal maintenance" characterization originating in asterinas/asterinas#1821; (b) the Asterinas migration debate closed against Netstack3 strengthens smoltcp as the canonical Rust-native candidate; (c) Hubris is the only documented "wrap" analog with similar concurrency posture, weighted as supporting evidence for the wrap path. Final choice remains at PLAN.

### 3.2 Addresses the one CONSIDER-REVISITING

- **§3.10 — Privacy Extensions + capability lifetime (pressure-test §2.7 §4.7b / Cerebro Q5).** This is a localizable gap, not a structural fault. THINK §3.10 specifies Privacy Extensions defaults (TEMP_PREFERRED_LIFETIME = 1 day) but does not specify kernel behavior when a `UdpSocket` capability is bound to a temporary address that gets deprecated and removed.

  **PM proposes adding a new BUILD-1 sub-commit BUILD-1.1** defining the address-deprecation-event surface (a notification capability the network service posts when an address moves to deprecated state) and the socket-binding-retention semantics (a deprecated address is not removed while a `UdpSocket` capability bound to it is held — reference-counted retention, matching the documented Linux/macOS/Windows/iOS/Android pattern).

  **PL ratification needed** on the BUILD-1.1 scope add — see §5 Q-A below.

### 3.3 Absorbs R11–R16 into THINK §5 risk register

The six new risks integrate cleanly:

- **R11 (IPv6 extension-header receive-path).** Add to §5. Mitigation per Cerebro: BUILD-1 sub-commit BUILD-1f or similar documents extension-header processing posture (RFC 8200 + RFC 9673 SHOULD-process-MAY-skip; explicit rejection of Routing-type-0 deprecated per RFC 5095). **PL ratification needed** on the BUILD-1f scope add — see §5 Q-B below.
- **R12 (Linux net/tls async-crypto CVE class).** Add to §5. Mitigation per Cerebro: BUILD-5 design pass explicitly audits async-callback shape; consider ADR-022 cell-swap revisit-trigger candidacy for BUILD-5 TLS state.
- **R13 (capability + address rotation).** Add to §5. Mitigation = the BUILD-1.1 sub-commit per §3.2 above.
- **R14 (DoT/DoH 2025–2026 trajectory).** Add to §5. Mitigation = the BUILD-4 address-discovery-layer sub-commit per §3.1 §3.4 above.
- **R15 (Intel I219-V e1000e quirks).** Add to §5 (extends R3). Mitigation per Cerebro: PLAN for e1000e-landing BUILD enumerates documented quirks (DMA-buffer-overrun workaround, TSO-disable-at-probe, Tx-hang under stressful UDP).
- **R16 (Hubris UDP→TCP transition unevidenced).** Add to §5. Mitigation = the BUILD-3 speculative-sketch path per §3.1 above; R16 and the §3.1 refinement share the same mitigation.

### 3.4 Revises Q1–Q10

The original THINK §9 surfaced Q1–Q10 for PL ratification. Cerebro's findings revise the Q-list:

- **Q1–Q9 from original THINK remain substantively valid.** The pressure-test confirmed the underlying choices.
- **Q10 (open PLAN drafting now or wait) is unchanged.** Still PL's call.
- **New Q11–Q12 surface from Cerebro's findings:** see §5 below.

The revised Q-list lands in THINK v0.2 as Q1–Q12.

## 4. Authority For The v0.2 Filing

PM has authority to file THINK v0.2 absorbing the REFINABLE refinements and the R11–R16 additions because:

- All five REFINABLE verdicts are wording/framing refinements; no fundamental position change.
- R11–R16 are additive risk-register entries with mitigations; the underlying choices are not contradicted.
- The CONSIDER-REVISITING (Privacy Extensions + capability lifetime) is a localizable gap, not a structural fault.
- None of the findings contradict a PM-NOTE confirmed position (a)–(f).
- PM-NOTE-2026-05-10 (NeoPlutarch agent roster + PM operational posture clarification): PM owns operational + technical decisions; PL is dispatch courier + sign-off on legitimately PL-tier calls. The REFINABLE refinements and the additive risk register are operational; the two scope-add proposals (BUILD-1.1, BUILD-1f) are PL-tier because they expand BUILD-1's scope.

## 5. PL Decisions Surfaced

Two scope-add decisions need PL authorization before THINK v0.2 lands. Both are surfaced by Cerebro and reasoned-through by PM; PL has the final call.

### Q-A. BUILD-1.1 sub-commit — capability + address rotation

**Cerebro's surfacing (pressure-test §2.7, R13).** RFC 8981 Privacy Extensions enabled by default at BUILD-1 means TEMP_PREFERRED_LIFETIME = 1 day. A long-lived UDP socket bound to a temporary IPv6 address sees that address deprecated after a day, removed after another. THINK §3.10 does not specify kernel behavior across rotation. Documented production pattern (Linux, Windows, macOS, iOS, Android): deprecation events + IP-address reference counting.

**PM proposed mitigation.** Add BUILD-1.1 sub-commit defining:
1. **Address-deprecation-event surface** — a notification capability the network service posts when an address moves from preferred → deprecated state.
2. **Socket-binding-retention semantics** — a deprecated address is not removed while a `UdpSocket` capability bound to it is held (reference-counted retention).
3. **Test plan** — BUILD-1e bare-metal verification includes a >24h session to verify rotation does not silently break long-lived sockets.

**Scope impact.** Modest. Adds one notification primitive and a reference-counting machinery integration with the existing capability table. Both are clean extensions, not invasive changes. Estimated 1–2 commits worth of work for whichever worker takes BUILD-1.

**Recommendation: ACCEPT.** This is a real gap that would surface at BUILD-1e bare-metal verification anyway. Better to design it in than discover it as a 24h-test failure mode.

### Q-B. BUILD-1f sub-commit — IPv6 extension-header processing posture

**Cerebro's surfacing (pressure-test §2.14 R11).** RFC 9098 documents IPv6 extension-header receive-path as hazard category. CVE-2026-23442 (SRv6 NULL idev) and CVE-2026-43501 (RPL OOB) are recent Linux CVEs in this class. BUILD-1 ships IPv6 + ICMPv6 + ND + SLAAC; the Hop-by-Hop header is the most relevant at BUILD-1; RFC 9673 (Oct 2024) loosened mandatory Hop-by-Hop on hosts to SHOULD-process-MAY-skip.

**PM proposed mitigation.** Add BUILD-1f sub-commit documenting:
1. **Hop-by-Hop posture** — SHOULD-process per RFC 8200 + RFC 9673; deferred-with-fast-path-skip when constrained.
2. **Routing-type-0 rejection** — explicit refusal per RFC 5095 (deprecated).
3. **Defensive parser posture** — validate length fields before parser advance; reject malformed option chains; log diagnostically (using BUILD-5b block-device log sink that just shipped).
4. **Other extension-header types** — explicit refusal at BUILD-1 (SRv6, RPL, etc. not in scope).

**Scope impact.** Small. Most of the work is "audit the BUILD-1b ICMPv6+ND parser path for the defensive posture" plus explicit rejection-list for non-supported extension-header types. Estimated <1 commit.

**Recommendation: ACCEPT.** Cheap to do right, expensive to retrofit if extension-header CVE class surfaces during BUILD-1 testing or later. The defensive parser posture is required correctness discipline regardless; BUILD-1f makes it explicit.

### Q-Defer. Cerebro's Q6 (I219-V quirks in R3 enumeration)

**Cerebro's Q6.** Extend THINK §5 R3 with documented e1000e quirks (DMA-buffer-overrun workaround, TSO-disable-at-probe, Tx-hang under stressful UDP).

**PM disposition.** ACCEPTED as a §3.3 mitigation in THINK v0.2; not a separate PL decision. R3 already names the I219-V; adding the documented quirk list is enumeration, not scope add. The e1000e driver itself is forward-marker'd to BUILD-2b or BUILD-6; the quirks bind PLAN-time decisions for whichever BUILD lands e1000e.

## 6. Operational Posture

Per the 2026-05-10 PM-NOTE on PL operational posture: PM (Cerebro) is the expert and owns operational + technical decisions. PL is dispatch courier + sign-off on legitimately PL-tier calls.

The five REFINABLE absorptions (framing/wording changes) and the six R11–R16 additions are operational; PM files THINK v0.2 with them absorbed.

The two scope-add proposals (Q-A BUILD-1.1, Q-B BUILD-1f) are PL-tier because they expand BUILD-1's scope. PM surfaces them per §5 above; PL nod required before THINK v0.2 lands with them.

The third decision (Q-Defer) is not PL-tier; PM absorbs.

## 7. Forward Sequencing

**This disposition does NOT:**
- Close PHASE-4-THINK. THINK v0.2 supersedes v0.1 when filed.
- Open Phase 4 PLAN. PLAN remains gated on Phase 3 formal close + PL ratification of revised Q-list per THINK v0.2.
- Modify Bible articles. No PM-NOTE confirmed position contradicted; no amendment triggered.
- File any ADR. R12 surfaced a candidate ADR-022 revisit-trigger for BUILD-5 TLS state, but the trigger fires at BUILD-5 PLAN time, not now.

**Active lanes after this disposition:**
- **Cerebro:** idle. Pressure-test closed.
- **Codex:** idle. Log-sink dispatch closed.
- **Prometheus:** idle.
- **PM:** drafting THINK v0.2 contingent on PL Q-A/Q-B decisions.
- **PL:** holds (a) BUILD-5b hardware-session scheduling, (b) Q-A/Q-B scope-add decisions, (c) Q10 ratification-gate (open PLAN now or wait for Phase 3 close).

## 8. INDEX And Bookkeeping

INDEX.md updates in the same commit:

- Cerebro dispatch row: status `active` → `closed`. Link to this disposition.
- New rows for the pressure-test artifact and this disposition under "Recent Phase 4 Substrate."
- PHASE-4-THINK row description annotated: "pressure-tested 2026-05-22; v0.2 pending Q-A/Q-B PL decisions."

---

**End of disposition.**

*Cerebro's pressure-test consumed. 8 CONFIRMED, 5 REFINABLE, 1 CONSIDER-REVISITING, 0 REFUTED. PM absorbs the framing refinements + R11–R16 into THINK v0.2 on its own authority; surfaces two BUILD-1 scope adds (Q-A capability + address rotation; Q-B IPv6 extension-header posture) for PL ratification. PLAN remains gated on Phase 3 close + Q-list ratification.*
