# DISPATCH — AI-native OS field survey (Cerebro)

**Date filed:** 2026-05-10 (later session)
**From:** PM
**To:** Cerebro (Claude, deep-research lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3)
**Ferry:** PL ferries this dispatch from PM session to a fresh Cerebro session.
**Status:** Second Cerebro deep-research dispatch under the new agent roster. Lower-stakes than the SC-3C D15 dispatch (this is situational awareness, not D-decision ratification input); useful as a second calibration data point for Cerebro deep-research voice profile.

---

## 1. Mandate

Conduct a field survey of projects publicly framed as "AI-native operating systems," "agent OSes," or "kernel for AI agents" as of mid-2026. Verify the project list pasted into the project conversation on 2026-05-10 (which contained citation problems by PM's preliminary audit); audit each named project against primary sources; survey the broader landscape for projects the user-pasted list did not include but that NeoPlutarch should know about; synthesize against NeoPlutarch's Bible Article 23 posture.

The deliverable is a research artifact at `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md` that PM consumes for situational awareness. Unlike the SC-3C D15 dispatch, this is not gating any project decisions — it informs NeoPlutarch's mental research-reference set and reinforces or challenges existing Bible-23 stance.

## 2. Context for Cerebro (read these first, in order)

**Authoritative inputs:**

1. **`ALTIMIT-real-world-implementation.md`** (project root) — the project's existing landscape framing. §C treats AIOS, MLOS, Heimdall, ProbeLogits, TPP, Mirage Persistent Kernel; §D treats multi-tenant accelerator scheduling; §E treats content-addressed model deduplication; §F treats the crash-immunity question; §G treats capability-based security for AI objects; §J treats ambient AI vs. system persona. This is the rigor target for landscape framing — Cerebro's output should be commensurable in scope and detail.

2. **`Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md`** — PM's first-pass audit of the user-pasted project list and broader landscape survey, executed before this dispatch existed and acknowledged as preliminary. Treat as a *starting reference*, not authority. PM's audit produced a Sense-A/B/C/D taxonomy (kernel-level AI primitives / agent runtime branded as OS / AI-built OS / agent-isolation OS) that PM finds useful; Cerebro is free to confirm, refine, or replace this taxonomy.

3. **`docs/bible.md` v1.5.0 Article 23** — no system AI assistant (23.1), no telemetry (23.2), no embedded model in kernel (23.3). The survey should grade each surveyed project against these three clauses since the project's existing framing argues Article 23 is the most ALTIMIT-faithful clause and that pressure to soften it will increase. Cerebro's findings inform whether that pressure is real and what shape it takes.

**Voice anchor for output style and rigor:**

4. **`Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`** — Janus voice anchor. Same format expectations as the SC-3C D15 dispatch.

**Calibration discipline anchor:**

5. **`Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`** — citation-rigor discipline. PM's preliminary audit found one likely-fabricated project (OctantCore), one HN-id misattribution (Pneuma cited at 47388478, actually at 47557165), and multiple mischaracterizations of real projects. The user-pasted content failed citation spot-check at a high error rate. Cerebro should independently verify every claim — not transcribe PM's preliminary findings.

## 3. Research scope

**Required coverage — audit each of the seven user-pasted projects:**

- **OpenFang** (`github.com/RightNow-AI/openfang`) — verify the 137k LOC Rust claim, the 14-crate structure, the ~32MB binary footprint, and whether the "Agent OS" framing is kernel-level or userspace.
- **Anima OS** (cited as arXiv 2604.11943) — PM's preliminary audit concluded Anima OS and ProbeLogits are the same project (paper + implementation pair). Cerebro should independently verify this conclusion.
- **OctantCore** (cited at HN id 47388478) — PM's preliminary audit could not find any project by this name; the HN id points to a different, real, tragic personal story ("Built a 1.3M-line agent-native OS in Rust while homeless"). Cerebro should independently search for OctantCore and either confirm non-existence or surface evidence PM missed.
- **AIOS** (`github.com/agiresearch/AIOS`) — verify the userspace-vs-kernel question; PM's preliminary read says userspace Python on Linux despite "LLM-as-kernel" branding.
- **Pneuma** — PM's preliminary audit found the actual HN post at id 47557165 (Show HN, April 7, 2026), not the cited 47388478. Cerebro should verify Pneuma's actual architecture (WASM + Wasmtime + WGPU) and confirm userspace classification.
- **VibeOS** (`github.com/kaansenol5/VibeOS`) — verify the C-language claim (PM's preliminary audit says C, not Rust as user-pasted content's framing implied), aarch64 target, and AI-built-vs-AI-aware distinction.
- **ZenithOS** (`github.com/rajkrish0608/ZenithOS`) — verify the dual-project structure (kernel + daemon) and the size/maturity profile.

**Required coverage — survey broader landscape for projects the user-pasted list did NOT include:**

- Asterinas (Linux-ABI Rust framekernel, ATC25) — closest legitimate architectural peer per SC-3C TLB research.
- Theseus OS (USENIX OSDI 2020, cell-swapping fault recovery).
- Redox OS (long-running Rust microkernel).
- seL4-based agent-isolation prototypes (referenced in `awesome-agent-runtime-security`).
- Microsoft Hyperlight + HyperAgent (micro-VM agent isolation).
- Microsoft Agent Governance Toolkit (April 2026 release; explicit OS-pattern-as-metaphor).
- Synthesis-OS (`github.com/GastonGelhorn/synthesis-os` — userspace macOS layer with Rust syscall-pattern scheduler).
- oneclaw (`github.com/nclamvn/oneclaw` — edge/IoT agent kernel).
- Inner Warden (`github.com/InnerWarden/innerwarden` — eBPF EDR for agents).

**Out of scope:**

- Proprietary in-house AI-stack-on-OS efforts (Apple Intelligence kernel extensions, Google Android 16 AI runtime) — surface as forward markers if relevant; do not deep-dive.
- Agent-framework comparisons that don't claim OS-level framing (LangChain, AutoGen, CrewAI as comparator only, not as primary objects of survey).
- Deep-dive into any single project's internal implementation (this is a landscape survey, not a single-project deep-dive).

## 4. Required output structure

Output file: **`Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md`**.

Note: this filename does NOT conflict with `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md`; the two are distinct artifacts with distinct provenance.

Sections (in order):

1. **Header** — date, author (Cerebro), type, authority (none, situational awareness), voice anchor (`ALTIMIT-real-world-implementation.md` plus Janus artifact).
2. **TL;DR** — synthesis of headline findings: how many real kernel-level AI-native OSes exist; how the field actually shapes up; what NeoPlutarch's position in it looks like.
3. **Sources surveyed** — primary / secondary / future-marker structure per Janus discipline.
4. **The category and what it means** — definition work. The phrase "AI-native OS" is used in 2026 to mean at least four distinct things (PM's preliminary audit proposed a Sense-A/B/C/D taxonomy; Cerebro may refine, replace, or confirm). Whatever taxonomy Cerebro lands on, project counts per category should be provided where Cerebro has signal.
5. **Audit: user-pasted project list** — per-project verification of each of the seven projects. For each: (a) verified-real-vs-cannot-verify status, (b) accurate description from primary source, (c) Bible Article 23 alignment assessment, (d) architectural relevance to NeoPlutarch (none / field-awareness / watch-this-peer / direct reference).
6. **Projects worth knowing that the user-pasted list did NOT include** — per-project treatment of the broader landscape items.
7. **What the field actually looks like in 2026 Q2** — synthesis section. Population estimates per category. Bible-23 alignment patterns. Closest legitimate peer for NeoPlutarch identified.
8. **Synthesis: implications for NeoPlutarch** — does the survey support or challenge Bible Article 23? Does it identify any new watch-this-project peers? Does it surface any reference-set updates? No project artifact amendments are expected; this section is about mental research-reference set updates.
9. **Calibration notes** — explicit acknowledgment of verified vs. unverified vs. single-source claims; future-dated arXiv IDs flagged; any project counts marked as estimates rather than precise enumeration; time-budget actual.
10. **Cross-references** — inputs consumed, outputs produced (none — this is situational awareness).
11. **Closure paragraph** — synthesis statement: how does the field-survey position NeoPlutarch?

## 5. Citation rigor expectations

Same Janus discipline as the SC-3C D15 dispatch. Specific expectations for this dispatch:

- Every project name must be verified against a primary source (GitHub repo, arXiv paper, official site). If a project cannot be verified, say so explicitly.
- HN post IDs and Reddit URLs cited in PM's preliminary audit should be independently verified by Cerebro, not transcribed.
- The arXiv 2604.11943 ID needs independent verification of the Anima-OS-and-ProbeLogits-are-the-same-project claim. PM's preliminary audit reversed an earlier (in-conversation) PM read on this; Cerebro's independent verification is calibration data on both PM passes.
- Project size claims (LOC, stars, age, contributor count) should be quoted with the source URL where verifiable; treated as approximate where Cerebro has only secondary sources.
- Architectural classification claims (kernel-level vs. userspace, AI-native vs. AI-built, Sense-A/B/C/D) should rest on primary-source evidence (README, paper, documentation), not on marketing copy.

## 6. Time budget

PM's read on appropriate budget for this dispatch: **2–3 hours** wall-clock. Seven user-pasted projects to audit + ~9 broader-landscape items to survey + taxonomy and synthesis work. Comparable in scope to the SC-3C D15 dispatch.

Stop-discipline guidance: if Cerebro encounters a substantial new sub-field (e.g., the LLM-OS-kernel literature in 2026 H1, or the agent-isolation-via-Hyperlight ecosystem), surface it in §6 (Projects worth knowing) and §8 (Implications for NeoPlutarch) without deep-diving. The survey is field-awareness, not project-evaluation.

## 7. Closure protocol

When the research artifact is complete:

1. Cerebro reports back to PL with: (a) confirmation the artifact is filed at the specified path, (b) one-line summary of the field shape (how many real Sense-A-equivalent projects exist, what closest peer NeoPlutarch has, whether Bible Article 23 is reinforced or challenged), (c) any divergences from PM's preliminary audit, (d) time-budget actual.
2. PL ferries the report and artifact reference back to PM.
3. PM consumes the artifact. **No follow-up PM-NOTE is required** unless Cerebro's findings substantially change NeoPlutarch's strategic posture — in which case PM produces a follow-up note for PL review. Otherwise, the artifact lands as situational-awareness reference.
4. PM-preliminary artifact remains in place as historical record of the role-confusion error and PM's first-pass read.

## 8. Cross-references

- `ALTIMIT-real-world-implementation.md` (project root) — voice anchor and landscape framing reference.
- `Plans/PM-PRELIMINARY-RESEARCH-2026-05-10-ai-native-os-field-survey.md` — PM-preliminary starting reference (input, not authority).
- `docs/bible.md` Article 23 — alignment grading target.
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — voice anchor.
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.3 — Cerebro deep-research lane.
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — citation discipline.
- `Plans/PM-NOTE-2026-05-10-pm-role-confusion-recovery.md` — calibration record.

---

**End of dispatch.**

*PM dispatches Cerebro for proper field survey. PM-preliminary artifact provided as starting reference; disagreement welcome. Output filename `Plans/RESEARCH-2026-05-10-ai-native-os-field-survey-cerebro.md`. Voice anchor: Janus + `ALTIMIT-real-world-implementation.md`. Time budget 2–3 hours. Closure: situational-awareness reference; no follow-up PM-NOTE unless findings substantially change NeoPlutarch's posture.*
