# Dispatch: Phase 4 THINK Pressure-Test Research Pass

**Date:** 2026-05-22
**Recorder:** PM
**Dispatched to:** Cerebro (Qwen3.6-27B, deep internet research lane)
**Authority:** PL (verbal, 2026-05-22)
**Status:** ACTIVE — open for execution
**Cadence position:** Research pass that informs PM's THINK consumption. Per Bible Article 18.1: informational output, not normative.

**Authority chain to consume:**

- `Plans/PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` — the artifact under pressure-test
- `Plans/OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` — Cerebro's own prior research substrate
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload.md` — original PM-NOTE
- `Plans/PM-NOTE-2026-05-20-phase-4-network-design-input-pl-preload-ADDENDUM.md` — citation refreshes
- `Plans/PM-DISPOSITION-2026-05-20-phase-4-network-stack-observe-consumption.md` — OBSERVE consumption
- `docs/bible.md` v1.7.0 — Articles 1.2, 1.4, 3, 4.1, 9.10, 11, 12, 14, 18.1, 28
- `docs/ADR-022-fault-recovery-direction-forward-marker.md` — cell-swap forward marker
- `docs/ADR-023-linux-personality-layer.md` — personality subsystem layer

---

## 1. Why This Dispatch Exists

PM filed `PHASE-4-THINK-2026-05-22-network-stack-design-direction.md` earlier today after consuming the 2026-05-20 OBSERVE pass and the PM-NOTE substrate. THINK chose positions on the 14 design questions OBSERVE enumerated, sequenced Phase 4 into five sub-BUILDs, enumerated 10 risks with mitigations, and identified 5 prerequisites. It is internally consistent at the level of a single PM sitting.

It has not been pressure-tested.

A PM-written THINK on a multi-month phase that touches kernel-level code, IETF protocol corpus, and capability architecture deserves an adversarial read before PLAN drafting begins. PL authorized this pressure-test pass on 2026-05-22 with the framing: "do some deep thinking and research for Phase 4 THINK."

This dispatch operationalizes that authorization. Cerebro is the right lane because the work is primary-source-heavy literature and codebase analysis, the depth required is high, and Cerebro has substrate familiarity from the 2026-05-20 OBSERVE.

---

## 2. Scope: What This Pass IS

This is a **pressure-test research pass**. For each substantive choice THINK made, the goal is to surface:

- **The strongest counterargument** that an experienced kernel/network engineer would raise.
- **Evidence from current (2025–2026) literature, production deployments, ecosystem moves, bug trackers, or postmortems** that supports or undermines the choice.
- **Risks PM may have missed** that current practice or recent incidents have surfaced.
- **Validation or refutation of the sub-BUILD sequencing** by comparison against other Rust kernel projects' first-network-stack histories.
- **IETF/RFC developments between 2026-05-20 and now** that bear on Phase 4.

The pass produces an informational artifact PM consumes via PM-DISPOSITION. If pressure-test findings warrant changes to THINK, PM files a THINK v0.2 or amendment. If THINK survives the pressure-test substantially intact, that strengthens PL ratification of Q1–Q10.

---

## 3. Scope: What This Pass IS NOT

- **Not a re-litigation of the six confirmed PM-NOTE positions** (dual-stack mandatory, IPv6-preferred + Happy Eyeballs + RFC 6724, TLS primary + IPsec deferred, NAT64/DNS64 deferred, no BSD sockets, capability-protected sockets-equivalent). These were confirmed by Cerebro's own OBSERVE §8 two days ago. The pressure-test is on **THINK's specific implementation choices that rest on those positions**, not on the positions themselves.

- **Not a new survey.** OBSERVE-2026-05-20 already enumerated the landscape: smoltcp, Fuchsia Netstack3, Hubris, seL4/LionsOS, Tock, Plan 9, Capsicum, Linux/kTLS, Asterinas debate. Do not re-survey unless a specific choice's pressure-test requires deeper inspection of a system already surveyed.

- **Not kernel code or syscall ABI work.** THINK explicitly reserves those for PLAN. Do not propose ABIs.

- **Not Bible amendments or ADR drafts.** Output is informational per Bible Article 18.1.

- **Not an open-ended Phase 4 research lane.** Bounded scope per §4 below.

- **Not a Phase 5+ scope exploration.** Forward markers noted in THINK §4 ("post-Phase-4 forward markers") are out of scope; this pass focuses on what BUILD-1 through BUILD-5 actually ship.

---

## 4. Specific Lines Of Inquiry

The following 14 lines are ranked roughly by load-bearingness for the Phase 4 design — the items whose wrongness would cost the most to fix in flight. Spend research budget proportionally. Lines 1–6 are highest priority; 7–14 are secondary; if any prove a dead end, skip and surface in §6.

### 4.1 Sockets-equivalent API shape — layered handle vs alternatives

THINK §3.1 chose the layered-handle shape (Fuchsia-precedent, native to NeoPlutarch's handle table). The alternatives (endpoint, channel, connection-shaped, Tock typed-capability) were dismissed with specific reasoning.

**Investigate:**

- Has any production OS published a postmortem or design retrospective on choosing one of these shapes and later regretting it? Particularly look at Fuchsia (since they chose the channel-routed-to-capability pattern that's closest to what THINK chose), Redox (smoltcp + scheme-based), and Hubris (compile-time-bound) for any "we'd do it differently now" signal.
- Does the Tock typed-capability pattern (`NetworkCapabilityCreationCapability`) have a history of friction with dynamic socket allocation that NeoPlutarch should be aware of?
- The Fuchsia FIDL-channel pattern routes capabilities through a separate service component. THINK chose to fold the capability check into kernel syscall entry rather than route through a service component. Does the Asterinas migration debate (smoltcp vs Netstack3) shed any light on this choice — does Asterinas keep capability checks in-kernel or route through a userspace network service?
- Surface any 2025–2026 academic paper or industry talk on capability-shaped network APIs that THINK should be aware of.

### 4.2 Sub-BUILD sequencing: UDP-first / TCP-third / TLS-fifth

THINK §4 sequenced Phase 4 as BUILD-1 (IPv6+UDP+virtio-net) → BUILD-2 (IPv4+DHCPv4) → BUILD-3 (TCP+CUBIC+5961+7323) → BUILD-4 (DNS+HE) → BUILD-5 (kTLS-hybrid TLS = Phase 4 ship).

**Investigate:**

- What was each of the following projects' actual sequencing for their first network stack?
  - Redox OS (smoltcp-derived). Did they ship UDP before TCP, or together? Any sequencing-regret signal?
  - Asterinas. Same question.
  - Theseus OS. Same question.
  - LionsOS. Same question.
  - Hubris (already known to ship UDP+ICMPv6 only, no TCP).
  - Plan 9 (historical — interesting only if a Bell Labs source on first-stack sequencing is reachable).
- Has anyone shipped "UDP-only first BUILD" and then found that adding TCP later required revising the API shape because TCP's connection-state model didn't compose with the UDP-only API? If yes, what specifically had to change?
- THINK §3.1 claims the layered-handle shape forward-accommodates both UDP and TCP. Does any production system contradict this?
- Has anyone shipped "IPv6 before IPv4" sequencing? Most stacks add IPv6 to a working IPv4 stack; THINK does the inverse. Any postmortem evidence on the inverse ordering?
- THINK places TLS at BUILD-5 (= Phase 4 ship). Is that the right ordering, or should TLS land earlier so the rest of Phase 4 has a TLS-capable transport to test against? Consider what AI subsystem (Phase 5) needs from the network stack at Phase 4 close.

### 4.3 kTLS-hybrid placement and current state of rustls-ktls

THINK §3.3 chose hybrid kTLS-style (userspace handshake via rustls in libneo, kernel record layer via `sys_net_tcp_enable_tls`). The Linux kTLS pattern is the production reference.

**Investigate:**

- What is the **current production state of rustls-ktls** as of mid-2026? When was the last release? Any known stability issues? Any ecosystem signal it's the wrong abstraction?
- What does the **kernel-side complexity of kTLS** actually look like in Linux? Lines of code, attack surface, known CVEs in the kTLS code path? This calibrates how much code NeoPlutarch will need.
- Are there 2025–2026 papers or talks proposing a different TLS-in-kernel pattern that's better than the Linux kTLS model?
- Does Fuchsia Netstack3 have a TLS story yet? If so, what shape?
- Asterinas — does it use kTLS or pure-userspace TLS? Any opinion documented?
- THINK explicitly does not propose kernel-resident TLS handshake (Bible Article 11.3 forbids rolling own crypto). Confirm this remains current best practice — no recent shift toward kernel TLS handshake.

### 4.4 Kernel-resident DNS resolver

THINK §3.4 chose kernel-resident stub resolver at BUILD-4, coupled to Happy Eyeballs. Forward markers (DoT, DoH, DNSSEC) explicitly deferred.

**Investigate:**

- Are there production OSes that put DNS in the kernel and then **moved it out** because the surface area grew unmanageably? Linux NetworkManager / systemd-resolved sit in userspace; was kernel-side DNS ever tried and rejected?
- Has Fuchsia Netstack3 published its DNS placement choice? If kernel-side, what's the surface area?
- What's the **2026 state of DNS-over-HTTPS / DNS-over-TLS deployment**? Are there carrier networks that block port 53? If so, "stub resolver only, port 53 only" may be insufficient for the Phase 4 ship milestone (HTTPS to real server).
- RFC 9461 (DoT) and RFC 8484 (DoH) — are there active drafts mandating DoT/DoH from endpoint OSes?
- Has any new 2025–2026 IETF work on DNS placement architecture appeared?

### 4.5 BUILD-1 transport scope — UDP only, no TCP

THINK §3.2 explicitly defers TCP to BUILD-3 to keep BUILD-1 small.

**Investigate:**

- The Hubris precedent ships UDP+ICMPv6 only, never adding TCP. Is there a Hubris design-retrospective document on whether UDP-only proved limiting? RFD 250 was already cited; look for follow-ups.
- The opposite case: Asterinas reportedly migrated from smoltcp to Netstack3. Was the migration motivated by smoltcp's TCP behavior under load? If yes, smoltcp may have hidden TCP-design landmines NeoPlutarch will hit in BUILD-3.
- The smoltcp release log between 2026-05-20 and now: are there recent TCP-related fixes or feature additions worth tracking?
- Has any Rust kernel project published "we shipped UDP first and here's what we learned"?

### 4.6 Loopback short-circuit at IP layer

THINK §3.7 chose short-circuit (kernel-internal queue, no driver/MAC/routing) over routed-loopback (virtual interface, normal stack traversal).

**Investigate:**

- Does any production kernel use short-circuit loopback and have documented bugs from the choice? Common pitfalls: forgetting to apply firewall rules to loopback traffic (NeoPlutarch has no firewall yet, so probably moot); breaking packet-tap diagnostics (NeoPlutarch has no tcpdump-equivalent yet, also moot); subtle MTU semantics where loopback delivers packets larger than the configured interface MTU.
- Has any Linux maintainer published a discussion of "why Linux's loopback is routed-through-the-stack" that articulates the arguments for the routed approach?
- Any postmortem evidence that short-circuit loopback caused a category of bug that took a long time to find?

### 4.7 SLAAC + Privacy Extensions interaction with capability-handle lifetime

THINK enables RFC 8981 Privacy Extensions by default. Temporary addresses rotate (TEMP_PREFERRED_LIFETIME = 1 day default). A long-lived UDP socket holding a capability handle may have been bound to a temporary address that is now deprecated or removed.

**Investigate:**

- How does Linux handle this case? When a temporary address is removed, what happens to sockets bound to it?
- How does Fuchsia Netstack3 handle it?
- Is there an RFC 8981 §3.6 or §3.7 normative requirement on existing-socket behavior across address rotation?
- Is there a published design pattern for "long-lived socket + rotating source address"?

### 4.8 Notification-on-activity vs io_uring scaling

THINK §3.14 chose notification-on-activity (Hubris precedent). io_uring-style completion-based I/O is named as a forward marker.

**Investigate:**

- Is there 2025–2026 data on **at what fan-out scale** notification-on-activity becomes a bottleneck? Phase 4 BUILD-1 has per-process limit 64 sockets; if notification-on-activity scales fine to 1000+ sockets per process there's no near-term concern.
- Has Linux's io_uring evolved in 2025–2026 in ways that suggest a different completion-ring shape would be better for NeoPlutarch's eventual high-throughput need?
- Has Fuchsia Netstack3 shipped a completion-ring API, or is it still channel-based?
- Wider question: is there a 2025–2026 trend in Rust kernel projects to skip the readiness/notification middle ground and go straight to completion-based?

### 4.9 CUBIC vs BBRv3 default congestion control

THINK §3.12 chose CUBIC at BUILD-3 ship.

**Investigate:**

- Has **BBRv3 been merged into mainline Linux** as of mid-2026? Cerebro's OBSERVE §3.6 said "not yet mainline; planned for upstream"; check current status.
- If BBRv3 is mainline, has it become the new default in any major distribution? If yes, NeoPlutarch should consider matching that default rather than choosing the older CUBIC.
- Are there 2025–2026 published comparisons of CUBIC vs BBRv3 in real production deployments (beyond Google's own data)? Notable for whether the published "30% throughput improvement on high-BDP shallow-buffer paths" generalizes.
- smoltcp's CUBIC implementation — any recent issues, known bugs, or feature gaps versus Linux's implementation?
- Is there a sound argument for shipping BBRv3 as the BUILD-3 default to be ahead of Linux rather than behind?

### 4.10 IPv6-only BUILD-1 test target reliability

THINK §3 ships IPv6-only at BUILD-1 with BUILD-1e bare-metal verification gated on IPv6 network functionality (PL pre-check).

**Investigate:**

- 2026 state of IPv6 deployment in US residential broadband. Specifically, what is the IPv6 coverage rate among major US ISPs (Comcast, Spectrum, AT&T, Verizon Fios) as of mid-2026? PL is in Melbourne, FL (per user-context location); residential ISPs there are likely Spectrum or AT&T fiber. Check those specifically.
- Are there known IPv6 quirks on the most-common consumer CPE (router) firmware (typical eero, Netgear Orbi, Spectrum-issued gateways)?
- What public IPv6 test targets are recommended in 2026 for "is my IPv6 working" verification — ipv6.google.com and ipv6.cloudflare.com are perennial; any others worth knowing about?

### 4.11 smoltcp wrap vs fork vs from-scratch (THINK R1 mitigation)

THINK §5 R1 says the wrap-vs-fork-vs-from-scratch choice is deferred to PLAN. This pressure-test should not pre-empt that decision but should surface evidence that informs it.

**Investigate:**

- What is the **current smoltcp release status and maintenance health**? Look at: time since last release, number of open issues, time-to-merge for recent PRs, active maintainers list. A project showing maintenance friction is a poor wrap target.
- The Asterinas migration debate (smoltcp → Netstack3) — has it resolved, and what was the deciding factor? This is the closest analog to NeoPlutarch's choice.
- Are there recent forks of smoltcp by other projects (kernel or embedded) worth knowing about? Sometimes a fork captures upstream-rejected improvements.
- Has any project published a "we forked smoltcp and here's what we learned" retrospective?

### 4.12 Phase 2 capability infrastructure verification (THINK P2 prerequisite)

THINK §6 P2 is a prerequisite check on Phase 2 capability infrastructure: capability handing at spawn, IPC capability-passing, per-handle rights bitmaps.

**Investigate (lighter touch — this is mostly a kernel-code-review task for PM, not a research task):**

- Surface any **publicly-discussed pattern for "capability-at-spawn"** that NeoPlutarch should match. seL4 capDL is one model; Fuchsia component-manifest is another. Any others worth knowing about?
- Has any project published a retrospective on capability-handling-via-IPC scaling that NeoPlutarch should be aware of? Particularly Fuchsia's experience with FIDL-over-channel capability passing — there's likely a postmortem or design doc somewhere.
- Capsicum's per-handle rights bitmap — is there a 2025–2026 paper on its operational experience that would inform Phase 4's design of `NetworkAccess` rights?

### 4.13 IETF/RFC developments since 2026-05-20

Cerebro's OBSERVE §11 listed the RFC corpus current as of 2026-05-20. Two days have passed; not much will have changed, but check for:

- **draft-ietf-happy-happyeyeballs-v3** — revision since -03 (March 2026)? RFC publication? WG status change?
- Any new 6man WG drafts published in the last week?
- Any new v6ops WG drafts published in the last week?
- Any RFC publication events (RFC editor queue clearance) for documents NeoPlutarch should know about?
- BBR-related drafts (informational drafts for BBRv3) — any new revision?

### 4.14 Risks PM may have missed

THINK §5 enumerates 10 risks (R1–R10) with mitigations. Pressure-test by:

- Identifying **risks present in production network stacks that THINK did not enumerate**. Categories worth checking: (a) corner cases in protocol implementation (e.g., TCP receive-window edge conditions, IPv6 extension header chaining); (b) interaction risks with adjacent subsystems (interrupt handlers, scheduling, memory allocator); (c) real-hardware quirks beyond Intel I219-V (e.g., DMA coherency on AMD systems, MSI-X routing failures); (d) IETF-protocol-mandate risks (something Phase 4 BUILD-N forgets that an RFC requires); (e) cross-version compatibility risks (e.g., NeoPlutarch BUILD-1 talking to old IPv6-broken peers).
- Surface known production-incident classes from 2025–2026 in network-stack code that NeoPlutarch should specifically pre-empt.

---

## 5. Output Format

A research artifact at `Plans/PRESSURE-TEST-2026-05-XX-phase-4-think-cerebro.md`, structured roughly:

- Header (date, author, type per Bible Article 18.1, authority chain consumed)
- §1 TL;DR — top 3 findings, each in one paragraph. Calibrate by impact.
- §2 Per-line findings (§4.1 through §4.14 above). For each line:
  - Brief restatement of THINK's choice.
  - Strongest counterargument surfaced.
  - Evidence (primary sources, citation density per the OBSERVE voice anchor).
  - Pressure-test verdict: CONFIRMED (no change needed), REFINABLE (citation refresh or minor framing), CONSIDER-REVISITING (substantive evidence that THINK should reconsider), or REFUTED (THINK's choice has documented production failure modes).
- §3 Risks added beyond R1–R10. Each with mitigation suggestion.
- §4 Sub-BUILD sequencing validation. Summarize what each comparable project did; identify whether THINK's sequencing matches or diverges from precedent.
- §5 IETF/RFC corpus delta from 2026-05-20 to today.
- §6 Stop conditions hit (if any). PM-intervention points (if any).
- §7 Open questions for PM consumption.
- §8 References — primary sources, codebases, papers, IETF documents.

Length expectation: 600–1000 lines. Substantial but bounded. Cerebro's OBSERVE was ~1000 lines; this pressure-test is narrower in scope but deeper per item, so similar overall.

---

## 6. Anti-Goals

- Do not re-survey systems already mapped in OBSERVE. Cite the OBSERVE finding and only go deeper where the pressure-test specifically requires.
- Do not produce more than one CONSIDER-REVISITING verdict per §4 line without strong primary-source backing. Pressure-test is a sharpened critical posture, not a re-design pass.
- Do not propose ABIs, syscall numbers, or kernel-code patterns. That is PLAN's job.
- Do not invent IETF drafts, RFC numbers, or commit hashes. If a citation cannot be verified from primary sources, do not include it. The OBSERVE corpus showed strong citation discipline; maintain it.
- Do not exceed §5's length expectation by more than 50%. If a single line of inquiry would require dramatically more space, surface it as a stop condition and ask PM whether to continue.
- Do not file Bible amendments or ADR drafts. Informational per Article 18.1.

---

## 7. Stop Conditions

Halt and request PM intervention if:

- A pressure-test finding contradicts a **PM-NOTE confirmed position** (a–f). This would be a more serious finding than just "THINK should reconsider X" and warrants PM review before continued research.
- Primary sources are inaccessible (paywall, removed pages, dead links) for a substantively important line. Surface what was checked and request alternative.
- A pressure-test verdict would require fundamentally restructuring THINK (e.g., the entire layered-handle shape is refuted). Surface the finding and stop; PM decides whether to continue or halt the pass.
- The IETF corpus check surfaces an event that materially changes Phase 4's design (e.g., HEv3 promoted to RFC; a new Internet-Standard-track protocol mandating endpoint behavior). Surface and stop for PM consideration.
- The research budget is roughly two-thirds consumed and §4.7–§4.14 remain. Surface what is incomplete; PM decides whether to extend or accept partial.

---

## 8. Closure Path

When the artifact is filed:

1. Cerebro emits the pressure-test artifact to `Plans/PRESSURE-TEST-2026-05-XX-phase-4-think-cerebro.md`.
2. Cerebro files no Bible amendment, no ADR. Informational per Article 18.1.
3. PM consumes via `PM-DISPOSITION-2026-05-XX-phase-4-think-pressure-test-consumption.md`. If findings warrant a THINK v0.2, PM files that under `Plans/PHASE-4-THINK-v0.2-2026-05-XX-network-stack-design-direction.md` referencing both the original THINK and the pressure-test. If THINK survives substantially intact, PM-DISPOSITION acknowledges and PL ratification of Q1–Q10 (or a revised Q-list) proceeds.
4. Phase 4 PLAN drafting remains gated on Phase 3 formal close and PL ratification. The pressure-test pass does not gate or open PLAN.

---

## 9. Operational Notes

- **Voice anchor for the artifact:** Cerebro's own `OBSERVE-2026-05-20-phase-4-network-stack-survey-cerebro.md` and `RESEARCH-2026-05-12-phase-3-storage-driver-prior-art-cerebro.md`. Citation density, primary-source discipline, interrogative mood for what should be investigated, indicative for what exists, no imperative for what NeoPlutarch should do.
- **Time budget:** This pass is research-heavy and bounded. Estimated half a day to a full day of focused work, similar to the 2026-05-20 OBSERVE.
- **Critical-posture invariant:** Pressure-test verdicts must rest on cited primary sources. "I think CUBIC is the wrong default" without 2025–2026 evidence is below the bar.
- **PM seat coordination:** PM (this seat) authored the THINK artifact under pressure-test. Cerebro may surface findings that conclude PM was wrong on specific choices; that is the entire point of the pass. Phrase findings analytically, not adversarially.
- **Sub-step boundary:** This pass is one of three parallel lanes today. The other two: Jarvis on `DISPATCH-2026-05-22-block-device-log-sink-and-phase-indicator-jarvis.md` (kernel code); PM on Phase 3 formal closure once Jarvis ships and PL executes BUILD-5b. The three lanes share zero substrate. No coordination required from Cerebro side.

---

## 10. Acceptance Criteria

The dispatch is complete when:

- **(A1)** A pressure-test artifact lands at `Plans/PRESSURE-TEST-2026-05-XX-phase-4-think-cerebro.md` per §5 structure.
- **(A2)** Each of the 14 lines of inquiry per §4 has either a finding section or an explicit "skipped, see §6 stop condition" note.
- **(A3)** No Bible amendment, no ADR draft, no ABI proposal in the output.
- **(A4)** Primary-source citation discipline maintained at the OBSERVE bar.
- **(A5)** Length within §5's bounds, or stop condition surfaced.
- **(A6)** No unverifiable citations (per anti-goal §6).

---

**End of dispatch.**

*Phase 4 THINK pressure-test research pass authorized 2026-05-22 by PL. Cerebro lane. Informational per Bible Article 18.1. PM consumes; PL ratification of Q1–Q10 (or revised) follows.*
