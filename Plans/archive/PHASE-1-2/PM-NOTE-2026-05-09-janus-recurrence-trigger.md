# PM Note: Janus deep-internet research trigger — recurring-error pattern

**Date:** 2026-05-09 (later session, second addendum)
**Author:** PM
**Type:** Operational standing-rule documentation. Second addendum to `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` and `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`. Adds a specific Janus invocation trigger not enumerated in either prior note: recurrence-pattern-based deep-internet research. The two prior notes stand as historical record; this addendum is operative forward in conjunction with them.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` unchanged. Working tree clean. Stash list unchanged.

---

## 1. The trigger

Project lead added 2026-05-09 (later session): **when the same error or failure pattern recurs across attempted fixes, dispatch Janus to deep-research external sources for prior art on the same problem.**

External sources include but are not limited to:

- **Rust ecosystem:** r/rust, r/embedded, r/osdev subreddits; rust-lang internals forum; users.rust-lang.org; rust-lang GitHub issue tracker.
- **OS development:** OSDev wiki and forum; LKML archives (lore.kernel.org); seL4 mailing list; Theseus OS GitHub issues; Asterinas GitHub issues.
- **Academic:** arXiv (cs.OS, cs.PL); USENIX papers (OSDI, ATC, NSDI); ACM digital library (SOSP, EuroSys, ASPLOS); IEEE (HPCA, ISCA, MICRO).
- **Vendor / specification:** Intel SDM chapters; AMD APM chapters; ARM ARM (Architecture Reference Manual); UEFI specifications; ACPI specifications; relevant RFCs.
- **Implementation prior art:** Linux kernel source for analogous subsystems (where licensing permits read-only reference for design ideas, not code copying); seL4 reference manual; Fuchsia / Zircon documentation; FreeBSD source as cross-reference.

The trigger is the *pattern of recurrence*, not the topic. Janus is invoked when internal reasoning has hit the same wall multiple times and external prior art may have a solution we can't derive from our own substrate.

---

## 2. What constitutes "recurring error"

PM judgment call, but the heuristic baseline is:

### 2.1 Hard recurrence (Janus dispatch warranted)

Two or more of the following:

- Same failure signature observed across two or more independent fix attempts (e.g., SC-3B's silent-timeout signature at run 36/200, then run 6/200 of Fix 6.1).
- Triple-parallel investigation produced no convergent root cause despite full source access.
- Phase B / instrumentation pass produced null capture results, refuting nothing and confirming nothing.
- A hypothesis ranking includes a "host-load-dependent" or "timing-fragile" or "QEMU-specific" element that suggests the bug is real but unsamplable in our environment.
- The project's internal substrate (Bible, ADRs, ANSPEC-001, prior research notes) does not contain a reference to a similar pattern.

When two or more of these conditions hold, the recurrence is hard. Janus deep-internet research is justified.

### 2.2 Soft recurrence (consider, don't reflexively dispatch)

One of the above plus operational pressure to close. Soft recurrence might or might not warrant Janus depending on:

- Whether closure is gate-blocked or just slow.
- Whether the existing three agents have exhausted their analytical traction.
- Whether the cost of one Janus dispatch (cloud quota, PM synthesis time on the resulting research artifact) is justified relative to "wait one more iteration."

Soft recurrence is a judgment call. PM proposes; project lead can confirm or override.

### 2.3 What does NOT trigger Janus deep-internet research

- A single failure with a clear signature, before any fix attempt. The first response is internal investigation (triple-parallel, source audit, Phase A ranking). External prior art comes after we've established what we're looking for.
- A new feature design where prior art is naturally part of the spec work. That's a Phase milestone trigger (per the prior addendum §4.1, §4.2), not a recurrence trigger.
- General curiosity about how others solved a similar problem. Janus's cloud quota is a real cost; speculative searches don't justify it.
- A fix that worked in our environment but we want to verify against external standards. That's PM review against the Bible / ANSPEC, not external research.

---

## 3. Dispatch shape for recurrence-triggered Janus research

A specific dispatch template emerges. Distinct from the "Phase 5 reading list ingestion" pattern in the prior addendum §4.1.

### 3.1 Dispatch structure

```
# Dispatch: Janus deep-internet research — <recurring-error-topic>

## 0. Trigger
[Brief recurrence pattern: what failure signature, how many attempts,
what's been tried, why external prior art now]

## 1. The question
[Single specific question, phrased as a research query, e.g.:
"What does the systems community know about silent kernel hangs
during cross-CPU IPC stress under QEMU SMP, where the failure
manifests at sub-1% rate with no panic and no terminal frame?"]

## 2. Sources to prioritize
[Specific sources Janus should hit based on the topic]

## 3. Constraints on output
- Cite exact sources with URLs / DOIs / arXiv IDs / commit hashes.
- Distinguish primary sources from secondary / aggregator content.
- Note where prior art conflicts or where consensus is absent.
- Do NOT propose fixes. Janus reports; PM and Cerebro decide.
- Do NOT introduce code. Research artifact only.

## 4. Output structure
Plans/RESEARCH-YYYY-MM-DD-<topic>-janus.md with:
- TL;DR
- Sources surveyed
- Findings by hypothesis
- Conflicts / contested claims
- Recommended next steps for our project
- References

## 5. Anti-scope
[What Janus should NOT do for this dispatch]

## 6. Cross-references
```

### 3.2 What Janus produces

A research artifact, not a recommendation. Format mirrors `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` and `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` already on disk — these are the existing voice anchors for prior-art research output.

The artifact is consumed by PM. PM produces follow-up dispatches:

- If Janus surfaces a known fix pattern → PM produces a PLAN-stage dispatch for the appropriate agent to implement (Cerebro for kernel, Prometheus for ADR amendment if a design decision changes).
- If Janus surfaces a known *non*-fix (i.e., the community has tried X and X doesn't work) → PM produces a ranking-with-data update rather than a fix dispatch. The "what doesn't work" is itself useful intelligence.
- If Janus finds nothing relevant → PM documents the null result and the project proceeds with internal-reasoning-only fix design. Bible 1.3 still applies; lack of external prior art doesn't lower the ship-quality bar.

### 3.3 PM review of Janus deep-internet output

Calibration discipline applies. Specific review steps:

- **Verify cited URLs resolve.** Cloud-LLM hallucination of citations is a known failure mode. PM spot-checks a sample of Janus's citations.
- **Verify cited papers are real.** arXiv IDs, DOIs, USENIX paper titles. Same hallucination concern.
- **Verify claims are not paraphrased beyond the source's actual content.** Janus may compress aggressively at 1M-token scale; PM verifies that key technical claims trace back to the source as cited.
- **Distinguish strong consensus from a single source.** Janus may report "the community knows X" when X is one paper or one Reddit thread; PM downgrades single-source claims accordingly.

The first 2-3 Janus dispatches accumulate calibration data on hallucination rate, citation fidelity, and consensus-overstatement patterns. Documented in handoffs as observed.

---

## 4. Concrete near-term applications

### 4.1 SC-3B as retroactive case study (informational, not actionable)

The SC-3B silent-timeout pattern that recurred at run 36/200 (original) and run 6/200 (Fix 6.1) was a textbook recurrence trigger. We didn't have Janus then. If the closure-gate retry fails (Outcome B/C), the trigger fires forward and the Janus dispatch follows the NMI capture per the prior addendum §4.3.

If the closure-gate retry passes 200/200 + 25/25, the recurrence trigger has been overtaken by events — the bug is unsamplable, no further fix is in scope, and the Janus dispatch is not needed for this arc. The retroactive lesson is documented in the close report's forward-concerns section.

### 4.2 Future likely triggers

Phase 2 has remaining sub-commits (BUILD-3 Q7 TLB management, Q8 capability table SMP composition; BUILD-4+ ADR-016 A2 scheduler heap-decoupling). Each carries its own failure-mode risk. If any of these hits a hard recurrence pattern, Janus deep-internet research is the routing answer.

Phase 5 carries higher recurrence risk because the AI subsystem is genuinely unprecedented in OS work — there's no Linux equivalent to copy from. Tensor primitives, model-object lifecycle, accelerator scheduling at the kernel level: each is exploratory enough that hitting walls twice on the same problem is plausible. Janus deep-internet research becomes a regular feature of Phase 5 work.

---

## 5. Cost discipline

Per the original Janus brief from PL: "Any task where the cloud quota matters more than the answer quality" is anti-scope. Cloud quota is a real budget; PM treats it as such.

Heuristics for cost discipline on recurrence-triggered Janus research:

- **One Janus dispatch per recurrence event.** Not iteratively refining the query through multiple Janus calls; produce one well-formed deep-research dispatch and consume the output.
- **Expect quality output to take Janus's full reasoning depth.** The deep-research output is high-effort by design; multiple shallow Janus calls is worse cost-and-quality than one deep call.
- **Internal reasoning before external reasoning.** Janus comes after the project's internal investigation has done its work. If triple-parallel investigation hasn't been run, that's the first move; Janus is layered on top, not substituted for.

This is consistent with Bible 4.3 applied to inference cost: every cloud call is a liability; justify and minimize.

---

## 6. Forward state

The prior addendum §6 listed Phase 5 PLAN opening and Asterinas OSTD reading as Janus's near-term territory. This second addendum adds:

- **SC-3B Outcome B/C contingency.** If the closure-gate retry fails, recurrence-triggered Janus research follows the NMI capture pass.
- **Phase 2 remaining sub-commits.** Recurrence trigger watch active.
- **Phase 5 recurring-error preparedness.** AI-native primitive design carries elevated recurrence risk; Janus deep-internet research becomes a regular feature.

---

## 7. Cross-references

**Authority:**
- Project-lead extension of Janus role 2026-05-09 (later session) to include recurrence-triggered deep-internet research.
- Original Janus brief (in `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` §1).

**Operational precedent — existing prior-art research format:**
- `Plans/RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md`.
- `Plans/RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md`.

These are pre-Janus research artifacts the project produced internally during the wedge investigation. Voice anchor for future Janus output.

**Relationship to existing notes:**
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` — original lean-roster note.
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` — first addendum adding Janus and §4 triggers (Phase 5, Asterinas, SC-3B Outcome B/C).
- This note — second addendum adding the recurrence trigger.

**Forward markers:**
- `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md` — current pending dispatch; Outcome B/C activates this trigger.
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) — Phase 5 territory; elevated recurrence risk.

**Governing posture:**
- `docs/bible.md` Articles 1.3 (every commit ships — applies to research dispatches as commitments to honest output), 4.3 (cloud quota as dependency), 14, 18.1.

---

**End of second addendum.**

*Janus role expanded to recurrence-triggered deep-internet research. Trigger: same failure pattern across two or more fix attempts, with internal investigation exhausted. Output: research artifact (not a recommendation). PM consumes, produces follow-up dispatches per findings. Calibration discipline applies (hallucinated citations are the named failure mode). Cost discipline applies (one well-formed dispatch per recurrence event, not iterative). Near-term: contingent on SC-3B Outcome B/C; structural feature of Phase 5 work.*
