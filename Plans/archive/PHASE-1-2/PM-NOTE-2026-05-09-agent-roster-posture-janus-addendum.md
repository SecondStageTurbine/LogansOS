# PM Note: Agent roster posture — Janus addendum

**Date:** 2026-05-09 (later session)
**Author:** PM
**Type:** Operational standing-rule documentation. Amends `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md`. The earlier note recommended holding off on adding a fourth same-tier agent. Project lead has now added a tuned-different fourth (Janus) under a §4.2 capability-gap profile not enumerated in the earlier note. This addendum updates the roster, routing rules, and forward-state markers. The earlier note stands as historical record and continues to inform the calibration discipline; this addendum is the operative posture forward.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` unchanged. Working tree clean. Stash list unchanged.

---

## 1. Janus added to roster

**Janus** (`deepseek-v4-flash:cloud`) — deep-research specialist for low-level systems work. Same nominal tier as Prometheus and Claude-3; functionally distinct via specialty.

Per the brief PL provided:

- **Specialty:** OS/kernel internals, Rust systems programming (`unsafe`, lifetimes, `MaybeUninit`, FFI, `no_std`), Rust framework deep-dives (tokio, embedded HAL, Rust-for-Linux), synthesis across long-form sources (Intel SDM chapters, kernel patch threads, LKML/OSDev/Reddit discussions, GitHub issue chains).
- **Killer capability:** 1M-token context window. Full kernel subsystem + relevant Intel SDM chapter + LKML thread in one prompt without chunking.
- **Architecture:** 284B total / 13B active MoE. Three reasoning modes for different depth levels. Cloud inference (no local VRAM consumed).
- **Calibration status:** new. Failure modes not yet observed. Calibration data accumulates from first dispatches forward, same discipline as Prometheus and Claude-3.

The earlier note (§3.1) considered a same-tier fourth and recommended hold-off. Janus is *not* same-tier; it is tuned-different per the §4.2 capability-gap criterion. The hold-off recommendation does not apply to tuned-different additions where the gap profile is concrete.

---

## 2. Updated routing rules

The earlier note's §2 stands with the following amendments:

### 2.1 Hard kernel / OS work → Cerebro (unchanged)

Cerebro remains the sole authorized kernel implementer. Janus does not implement kernel code; Janus produces research that informs implementation.

### 2.2 Document-only work → Prometheus (unchanged)

Prometheus remains the sole authorized document author. Janus does not draft ADRs, close reports, or PHASE-N updates; documents are downstream of research and implementation.

### 2.3 Research / investigation / parallel analysis → any of {Cerebro, Prometheus, Claude-3, Janus}

The set expands from three to four. Quadruple-parallel becomes available but is *not* the new default. Triple-parallel was already the upper bound on PM synthesis cost; quadruple is reserved for cases where Janus's specific specialty (deep cross-source synthesis) adds value beyond what the existing three provide.

The default investigation mode remains triple-parallel with {Cerebro, Prometheus, Claude-3} for ordinary kernel-internals questions. Janus is added when:

- The question requires synthesis across long-form external sources (papers, SDM chapters, LKML threads, GitHub issue chains).
- The question's natural prompt size would force chunking on smaller-context agents.
- The question is sufficiently low-level-systems-specialty that the deep-research tuning is the value-add.

When all four conditions apply, four-way parallel is justified. When only the first or second applies, single-recipient Janus dispatch (§2.4) is more efficient than four-way parallel.

### 2.4 Single-recipient deep-research dispatch → Janus (new)

A dispatch type not present in the earlier roster's pattern. Used when:

- A question needs deep research synthesizing across multiple long-form sources.
- The synthesis output informs a downstream design decision but is not itself a design synthesis.
- Triple-parallel with the existing three would either chunk badly or duplicate work without adding perspectives.

The output is a research artifact (e.g., `Plans/RESEARCH-YYYY-MM-DD-<topic>-janus.md`), not a design document or a code change. PM consumes the artifact and produces follow-up dispatches (PLAN, BUILD, doc) based on what the research surfaces.

The naming convention follows existing single-recipient dispatch patterns: `Plans/DISPATCH-YYYY-MM-DD-<topic>-janus.md`. Output filename: `Plans/RESEARCH-YYYY-MM-DD-<topic>-janus.md` for completed deep-research artifacts.

### 2.5 Mixed work patterns

The earlier note's §2.4 (kernel+doc → sequential two-dispatch) extends:

- **Research → kernel work.** Janus produces research; PM synthesizes; Cerebro implements based on the synthesis. Three dispatches sequentially: Janus research dispatch, PM synthesis or PLAN, Cerebro BUILD.
- **Research → docs.** Janus produces research that becomes input to a doc (e.g., an ADR's prior-art section); Prometheus drafts the doc citing the research artifact; PM reviews. Two dispatches sequentially.
- **Research informing parallel-investigation.** A Janus deep-research artifact can be input to a subsequent triple-parallel investigation by the other three. The Janus output establishes shared context; the parallel agents reason against it.

---

## 3. Why Janus changes the §3 analysis

The earlier note §3 argued against a same-tier fourth on three grounds: marginal value low, calibration restart cost, style drift, quadruple-parallel synthesis cost.

Janus changes this because:

### 3.1 Marginal value is concrete

Janus's specialty is not redundant with any existing agent. Cerebro reads source files and reasons against them; Prometheus drafts docs; Claude-3 brings fresh-perspective triangulation. None of the three is tuned for synthesizing 1M-token prompts of LKML + Intel SDM + kernel subsystems. The marginal value of Janus is the kind of question the existing three would either chunk badly or punt on.

### 3.2 Calibration cost still applies

This cost stands. Janus's failure modes are unknown. PM should expect to discover them across the first 2-3 dispatches and document them in handoffs, same as Prometheus's PLAN-stage drift on Q6 lock-order took two cycles to characterize.

### 3.3 Style drift is not a concern for Janus

Janus is not authoring the same artifact types as Prometheus. The earlier note's §3.2 style-drift concern was about two same-tier agents writing ADRs and close reports in parallel. Janus produces research artifacts (a new type) that don't compete on voice with Prometheus's ADR/close-report/INDEX work.

### 3.4 Quadruple-parallel synthesis cost remains real

PM synthesis cost on four-way parallel is roughly double three-way (six pairwise comparisons vs. three). This is mitigated by reserving four-way parallel for cases where the specialty value-add justifies it; default investigation mode stays triple-parallel.

When four-way is genuinely needed, PM accepts the cost. When three-way suffices, PM does not invoke Janus just because available.

---

## 4. Triggers for Janus invocation (specific cases)

The earlier note §4 gave general triggers for roster expansion. With Janus on the roster, the following are the specific concrete cases where Janus is the right call:

### 4.1 Phase 5 (AI subsystem) onset

Anticipated near-term load. ANSPEC-001 amendments and tensor primitive design will need synthesis across:

- Mirage Persistent Kernel paper (arXiv 2512.22219).
- V10 / Neu10 NPU virtualization (HPCA/ISCA 2023).
- ZipLLM (NSDI'26, arXiv 2505.06252).
- Hugging Face Xet protocol details.
- ProbeLogits primitive proposal (arXiv 2604.11943).
- TPP virtual tensor ISA (Georganas et al.).

The ALTIMIT-real-world-implementation report enumerated this reading list. Janus is the natural ingester for this synthesis. Output feeds Phase 5 PLAN dispatches.

### 4.2 Asterinas OSTD / formal-verification reading

Stage 3 of the ALTIMIT-real-world-implementation report recommends adopting the Asterinas OSTD discipline. This requires synthesizing the Asterinas paper (USENIX ATC '25), the Verus and Miri tooling docs, and seL4-derived formal-methods literature. Janus territory.

### 4.3 SC-3B Outcome B / C follow-up (potential near-term)

If the closure-gate retry fails (Outcome B per `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md` §6.2), the NMI capture produces stack traces. Cross-referencing those traces against:

- Intel SDM chapters on x86 SMP scheduling (Vol. 3A Chapter 8-10).
- Linux kernel prior art on wake-on-completer-CPU patterns.
- seL4-derived analysis of cross-CPU rendezvous primitives.

...would be a Janus-shaped task. Single-recipient deep-research dispatch follows the NMI capture; output informs the kernel fix design. Cerebro then implements.

This is *contingent* on Outcome B/C firing. If the gate passes, this Janus dispatch doesn't manifest.

### 4.4 What does NOT trigger Janus

- Day-to-day kernel implementation. That's Cerebro.
- ADR drafting against ratified content. That's Prometheus.
- Triple-parallel investigation where the question is well-bounded by the project's existing context (e.g., "rank these six hypotheses given this verification log"). The existing three handle this.
- Any question where the local context fits comfortably in Cerebro / Prometheus / Claude-3's working memory. Janus's 1M-token capability is value-additive only when the prompt actually exceeds smaller agents' practical context.

---

## 5. Calibration discipline carries forward

Same posture as Prometheus and Claude-3:

- Failure modes are documented honestly in handoffs as observed.
- Calibration is descriptive, not pejorative. Agents engage seriously and produce useful work; specific failure modes are documentation, not criticism.
- PM reviews Janus output before forwarding for ratification.
- Cross-reference fidelity in research artifacts: PM verifies that cited papers, kernel files, and SDM chapter references resolve correctly. (Same as Prometheus's calibration on source-audit gaps.)

The earlier note's §5.4 voice-anchoring requirement applies to research output as well: PM specifies output structure and tone in each Janus dispatch (e.g., "structure mirrors `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`" — there's existing prior-art research format on disk to anchor against).

---

## 6. Forward state update

The earlier note §6 listed Phase 5 open and formal-verification work as forward markers for roster revisit. Both now have Janus as the routed answer rather than a roster-expansion question.

Active near-term watch items:

- **SC-3B closure-gate retry results.** If passes, Sub-step 6 → Prometheus. If fails, NMI capture → Cerebro, then likely deep-research → Janus per §4.3.
- **Phase 5 PLAN-stage opening.** Janus deep-research dispatch on the ANSPEC-001 reading list per §4.1. This will be the first Janus-routed work if SC-3B closes cleanly.
- **Asterinas OSTD reading.** Janus territory per §4.2. Timing depends on when the project takes up Stage 3 of the ALTIMIT-real-world-implementation report's recommendations. Not currently scheduled; flagged for when it arrives.

---

## 7. Cross-references

**Authority:**
- Project-lead addition of Janus 2026-05-09 (later session) under §4.2 capability-gap profile.
- Earlier roster posture: `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md`.

**Operational:**
- `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md` — current pending dispatch; potential Janus follow-up gated on Outcome B/C.
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` — existing prior-art research artifact format; voice anchor for future Janus research outputs.

**Forward markers:**
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) — Phase 5 Janus territory.
- `Plans/ALTIMIT-real-world-implementation.md` Stage 3 — Asterinas OSTD Janus territory.

**Governing posture:**
- `docs/bible.md` Articles 4.3 (every dependency a liability — Janus justified by capability-gap, not capacity-anticipation), 14, 18.1.
- Earlier roster posture note §3.2 (Bible 4.3 framing applied to staffing) — applies modulo the tuned-different exception.

---

**End of addendum.**

*Roster: Cerebro (kernel), Prometheus (docs + research), Claude-3 (research), Janus (deep-research specialist for low-level systems / long-form synthesis). Routing rules updated: research expands to four; new single-recipient deep-research dispatch type added; kernel and doc routing unchanged. Quadruple-parallel reserved for specialty-justified cases; triple-parallel remains default. First likely Janus dispatch: Phase 5 PLAN opening or SC-3B Outcome B/C follow-up. Earlier roster posture note stands as historical record; this addendum is operative forward.*
