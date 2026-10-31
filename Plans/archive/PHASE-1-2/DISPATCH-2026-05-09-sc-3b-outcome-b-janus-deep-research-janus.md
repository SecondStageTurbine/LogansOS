# Dispatch: SC-3B silent-timeout — deep-internet prior-art research

**To:** Janus (deepseek-v4-flash:cloud)
**From:** PM
**Date:** 2026-05-09
**Type:** Single-recipient deep-research dispatch. Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md`, this is a recurrence-triggered Janus deep-internet research call. Output is a research artifact, not a recommendation. PM consumes the artifact and produces follow-up dispatches based on findings. This dispatch runs in parallel with `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-nmi-capture-cerebro.md` (Cerebro NMI capture on the same failure pattern); the two are independent.
**Substrate context:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Janus does not modify substrate; this is research only. Janus does not need access to the kernel source for this dispatch; the question is "what does the external systems community know about patterns matching ours."

---

## 0. Trigger

**Hard recurrence per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1.**

Specifically, four conditions satisfied:

1. **Same failure signature across multiple observation windows:**
   - Original BUILD §9 gate: run 36/200 silent-timeout (NEO_SMP=4 stress).
   - Fix 6.1 attempt: run 6/200 silent-timeout.
   - Closure-gate retry (clean substrate): run 10/200 silent-timeout.
   - Three independent reproductions of the same missing-terminal-frame signature.

2. **Triple-parallel investigation produced no convergent root cause:** Cerebro / Prometheus / Claude-3 ranked Hn (deterministic Q4 migration accumulation) as top candidate at ~50% confidence. Fix 6.1 (per-pair direction alternation, addressing Hn's surface) didn't close the gate. Hn's confidence dropped post-Fix-6.1 to 35%; remaining mass distributes across H1-variant (lost wakeup, 25%), H4 (per-CPU starvation, 15%), Hh (host-load-dependent timing, 20%, PM-promoted). No single hypothesis dominates.

3. **Phase B instrumentation pass produced null capture:** 50/50 PASS with NMI watchdog armed; 50/50 PASS clean counterfactual. The bug didn't reproduce in that observation window.

4. **Hypothesis ranking includes "host-load-dependent" / "QEMU-specific" element (Hh):** the synthesis's mechanism for Hn relied on QEMU SMP scheduling timing being slow enough to push the orchestrator's finite polls toward 180s. PM promoted Hh based on the failure's appearance/disappearance pattern correlating with host load conditions.

The project's internal substrate (Bible, ADRs, ANSPEC-001, three-way investigation, Phase B findings, two prior-art research notes from the wedge investigation) does not contain a reference to a similar pattern outside this project. External prior art is the next move.

---

## 1. The question

**What does the systems community know about silent kernel hangs during cross-CPU IPC stress under QEMU SMP, where the failure manifests at low-single-digit-percent rate with no panic, no terminal frame, and reproduces unevenly across observation windows in ways consistent with host-load-dependent QEMU scheduling timing?**

Concretely, decompose into:

### 1.1 Failure-pattern queries

- Has anyone observed silent kernel hangs in QEMU SMP emulation specifically tied to host CPU contention or QEMU thread scheduling?
- Are there documented cases where kernel cross-CPU rendezvous primitives manifest hangs only under emulation timing pressure but not on bare metal?
- What's the Linux / FreeBSD / seL4 / Theseus / Asterinas community experience with intermittent silent-hang patterns in IPC stress tests?

### 1.2 Mechanism queries

- Wake-on-completer-CPU (push-the-blocked-thread-to-the-CPU-that-completes-the-rendezvous) is our Q4 design. Has this pattern been used elsewhere? What pathological cases have been reported?
- Per-pair direction-locking in stress tests (where sender/receiver direction is constant per pair across iterations) — has anyone documented the migration-accumulation bias this produces under wake-on-completer-CPU semantics?
- Asymmetric yield-vs-no-yield paths in IPC completion (we have `continue_current_after_receive` no-yield and `handoff_to_rendezvous_peer` yields-immediately as siblings depending on which side completed first) — is this asymmetry a known footgun?

### 1.3 Diagnostic-pattern queries

- NMI watchdog patterns specifically tuned for capturing silent hangs in SMP kernels under QEMU. Threshold tuning, false-positive rates, instrumentation perturbation effects.
- Approaches other projects use to discriminate "real bug" from "QEMU emulation timing artifact" in stress test failures.
- Practices around closing sub-commits when a low-rate intermittent bug is observed but unsamplable in some observation windows.

### 1.4 Architectural-pattern queries

- Linux's wake_q / scheduler IPI patterns for cross-CPU wake. Known issues, regression history.
- seL4's IPC fastpath SMP design and its handling of cross-CPU rendezvous.
- Theseus OS's intralingual approach to cross-CPU primitives.
- Fuchsia / Zircon's channel handoff under SMP. Any documented bugs of this shape.
- Asterinas (USENIX ATC '25) and any reported issues in their SMP IPC.

---

## 2. Sources to prioritize

In rough priority order (most likely to highest hit rate):

1. **Linux kernel mailing list (LKML) / lore.kernel.org.** Search for "silent hang IPC SMP," "QEMU SMP intermittent," "wake_q migration scheduler," "cross-CPU rendezvous timeout." Recent threads (2023-2026) most relevant given evolving SMP scheduler.

2. **OSDev wiki and forum (forum.osdev.org).** Hobby kernel community has direct experience with this exact class of bug. Search for "QEMU SMP hang," "rendezvous deadlock," "cross-CPU wake," "silent kernel timeout."

3. **r/osdev and r/rust subreddits.** Recent posts (last 2-3 years). Hobby kernel projects in Rust frequently hit similar walls and discuss publicly. Search threads, link to specific posts where applicable.

4. **GitHub issue trackers:**
   - rust-osdev organization repos (x86_64 crate, bootloader crate, blog_os).
   - Theseus OS (theseus-os/Theseus).
   - Asterinas (asterinas/asterinas).
   - Redox (redox-os/kernel).
   - seL4 (seL4/seL4) — though formal-methods-anchored, may have practical SMP IPC issues.
   - QEMU itself (qemu/qemu) — for known SMP emulation timing issues that affect guest kernels.

5. **Academic papers.** Search arXiv (cs.OS), USENIX (OSDI, ATC, NSDI), ACM (SOSP, EuroSys, ASPLOS) for:
   - Cross-CPU IPC primitive design
   - Wake-on-completer-CPU patterns
   - Stress test methodology for SMP kernels
   - QEMU emulation fidelity studies

6. **Vendor specifications.** Intel SDM Vol. 3A Chapters 8-10 (Multiple-Processor Management, APIC, Inter-Processor Interrupts) and AMD APM equivalent. Less likely to directly address our pattern but may surface architectural details that inform Hn / H1-variant analysis.

7. **Linux kernel source as cross-reference.** `kernel/sched/core.c`, `kernel/sched/wake_q.c`, IPC code paths in `ipc/`. Read-only reference for design ideas (do NOT propose copying code; this is research, not implementation).

---

## 3. Constraints on output

- **Cite exact sources.** URLs for forum posts, mailing-list archive links (lore.kernel.org has stable URLs), commit hashes for kernel source references, arXiv IDs / DOIs for papers, full thread URLs for Reddit / OSDev forum.
- **Distinguish primary sources from secondary aggregator content.** A primary-source LKML thread carries more weight than a Reddit post citing that thread.
- **Note where prior art conflicts or where consensus is absent.** If two sources disagree on whether a pattern is a real bug or a known-and-accepted limitation, surface both.
- **Do NOT propose fixes.** Janus reports findings; PM and Cerebro decide what to do with them.
- **Do NOT introduce code.** Research artifact only. No diff suggestions, no patch proposals.
- **Be honest about hallucination risk.** If Janus is uncertain whether a cited paper / thread / commit actually exists at the given location, flag the uncertainty explicitly. PM will spot-check citations.
- **Be honest about consensus overstatement.** If a finding is from a single source, do not phrase it as "the community knows X." Use "one report from <source> indicates X."

---

## 4. Output structure

Produce `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` with this structure:

```
# Research: SC-3B silent-timeout prior art

## TL;DR
[3-5 sentences. The most relevant finding(s) for our case.]

## Sources surveyed
[List of sources actually consulted with URLs / IDs.
Distinguish "primary source consulted directly" from "secondary reference."]

## Findings by query

### 1.1 Failure-pattern findings
[Per-query findings with citations.]

### 1.2 Mechanism findings

### 1.3 Diagnostic-pattern findings

### 1.4 Architectural-pattern findings

## Conflicts and contested claims
[Where sources disagree.]

## Single-source findings (low confidence)
[Findings that come from one source only. Flagged for PM downgrade.]

## Recommended next steps for our project
[NOT fix proposals. Things like "verify our wake-on-completer-CPU
matches the Linux wake_q pattern in <commit hash>" or
"the seL4 reference manual's section on IPC fastpath SMP
discusses an analogous trade-off; PM may want to read it before
the next fix design."]

## References
[Full citation list with URLs / DOIs / IDs. Sufficient for PM to
verify each citation resolves correctly.]
```

Voice anchor: `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` and `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` — existing prior-art research artifacts in the project's voice. Match their tone and structure.

---

## 5. Anti-scope

Janus does NOT, in this dispatch:

- Read the project's kernel source. The question is external; internal substrate is already analyzed by Cerebro / Prometheus / Claude-3.
- Propose specific fixes for our codebase. PM and Cerebro decide.
- Claim more confidence than the cited sources support.
- Synthesize a "unified theory of the bug" — that's PM's synthesis job after Janus's research and Cerebro's NMI capture both return.
- Make recommendations about Bible amendments, ADR amendments, or routing rules. Out of scope.
- Re-analyze our own three-way investigation reports. They're internal context; Janus's value is external.
- Cite from the project's own artifacts (`Plans/`, `docs/`) as if they were external prior art.

---

## 6. Cost discipline

Per the Janus brief and `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §5:

- **One well-formed dispatch, one research artifact.** Not iterative refinement through multiple Janus calls.
- **Use the 1M-token context budget for source-rich synthesis, not query iteration.** Drop relevant LKML thread + Linux source file + Intel SDM chapter + OSDev forum thread into one prompt where Janus is doing the cross-source synthesis. Do not chunk speculatively.
- **Time budget:** the parallel Cerebro NMI dispatch estimates 1-2 hours; Janus running in the same wall-clock window is the design intent. If Janus's deep research takes substantially longer (say >4 hours), surface it to PM rather than running silently.

---

## 7. PM review on receipt

PM reviews Janus output before integrating into ranking-with-data:

- Spot-check 5-10 citations: do URLs resolve, are arXiv IDs real, do commit hashes exist in the named repos, do mailing-list archive links resolve.
- Verify "the community" claims trace to actual multi-source consensus, not single-source extrapolation.
- Cross-reference Janus's findings against Cerebro's NMI capture findings (when both return).
- Identify any findings that suggest specific kernel reading (e.g., "look at Linux commit ABC123 for an analogous pattern") — these become input to Cerebro's next dispatch if applicable.

The first Janus dispatch on this project carries elevated review intensity per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3. Calibration data accumulates: hallucination rate, citation fidelity, consensus overstatement patterns are documented in this dispatch's findings review and carried forward.

---

## 8. Outcomes and forward path

Three plausible Janus output profiles:

### 8.1 Outcome A — Strong external prior-art match

Janus finds a documented case of substantially the same pattern, with a community-recognized fix or workaround. PM cross-references with Cerebro's NMI capture (if it landed by then), produces a fix design dispatch for Cerebro. The kernel work follows.

### 8.2 Outcome B — Partial / suggestive prior-art

Janus finds related patterns (e.g., Linux's wake_q has had similar bugs but in a different shape) without a direct match. PM treats these as design-input rather than fix-source. Combined with Cerebro's NMI traces, may re-rank hypotheses or surface new ones (Hn-prime, H1-variant-prime, etc.).

### 8.3 Outcome C — No relevant prior art found

Janus reports honestly that the community has not documented this pattern. This is itself useful: it means the bug is genuinely novel-to-public-knowledge. PM proceeds with Cerebro's NMI traces alone as primary evidence; fix design is project-internal.

In all three outcomes, the Janus output is the artifact and PM does the synthesis.

---

## 9. Cross-references

**Authority for this dispatch:**
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1 — hard recurrence trigger.
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` §2.4 — single-recipient deep-research dispatch type.
- Project lead's Janus brief (in agent-roster-janus-addendum §1).

**The recurrence pattern this dispatch addresses:**
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — original gate run 36 failure.
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` — Fix 6.1 attempt run 6 failure.
- Closure-gate retry verification (Cerebro to land; run 10/200 failure preserved at `target/verify/sc3b-closure-gate-clean/neo-smp4/run-010.serial.log`).
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` — Phase B null capture.

**Internal investigation context (informational; Janus does not re-analyze):**
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` — three-way investigation synthesis.
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md` — ranking-with-data update post-Phase-B.

**Voice anchors:**
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`.

**Parallel running dispatch:**
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-nmi-capture-cerebro.md` — Cerebro's NMI capture on the same recurrence pattern.

**Governing posture:**
- `docs/bible.md` Articles 4.3 (cloud quota dependency), 14, 18.1.
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` (full operative posture for this dispatch type).

---

**End of dispatch.**

*Janus deep-internet research on the SC-3B silent-timeout recurrence pattern. Three independent reproductions; triple-parallel investigation no convergent root cause; Phase B null capture; Hh hypothesis suggests host-load-dependent QEMU timing. External prior art is the question. One well-formed dispatch, one research artifact. Mirrors existing prior-art research format. Runs in parallel with Cerebro NMI capture; PM synthesizes both. Calibration data collection on first Janus dispatch carries elevated PM review intensity. Estimated 1-4 hours.*
