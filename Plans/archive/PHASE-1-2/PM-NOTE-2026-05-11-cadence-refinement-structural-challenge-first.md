# PM-NOTE: Cadence refinement — first Cerebro dispatch on new substrate = structural-challenge, not prior-art-anchoring

| | |
|---|---|
| **Date** | 2026-05-11 |
| **Author** | PM (NeoPlutarch project cadence) |
| **Type** | PM-tier cadence refinement (project-wide; informs all future Phase substrate work) |
| **Triggered by** | PL diagnostic post-Cerebro #1 Phase 3 consumption: pre-dispatch framing-gate operated as "is my question well-formed?" not "should I be asking a different question entirely?" |
| **Evidence** | Two-for-two: SC-4 D5 (process-table SMP) + Phase 3 Q5.a (NeoFS content-addressing) — both required Cerebro structural-reframe rather than prior-art anchoring of PM's existing lean |
| **Scope** | Cadence rule; not yet bible-amended. Candidate for Bible Article 14 amendment at Phase 4 close if pattern holds three-for-three |
| **Cross-refs** | `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` (SC-4 D5 reframe); `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` (Phase 3 Q5.a reframe); Bible Article 14 (Algorithm) |

---

## 1. The pattern, with PL's precision

PM operates at the **framing level**: what is the shape of the question? What are the right Q-items to enumerate? What's the BUILD scope split? Is this substrate or feature?

Cerebro operates at the **question level**: given a frame, what does production prior art say about the specific question?

When THINK v1 is written before structural-challenge has occurred, it embeds an un-challenged framing as substrate. The Q-items, BUILD scope, and design leans inside THINK v1 are then question-level descendants of that framing. **If the framing is wrong-shaped, every descendant is too.**

In two-for-two cadence (SC-4 D5; Phase 3 Q5.a), Cerebro's contribution was not "answering my question" but "reframing my question." That contribution can only happen if Cerebro is invited to operate at the framing level, not just the question level. The current dispatch shape ("here is Q5.a-e; anchor with prior art") biases Cerebro toward the question level. Cerebro is doing the framing-level work anyway, but only because the substrate gap is large enough to be visible from prior-art survey. **That's luck, not cadence.**

## 2. The failure mode of the previous framing-gate, named precisely

Post-SC-4 D5, PM added a pre-dispatch framing-gate. PM-stated as: "Before dispatching Cerebro, PM checks whether the question being asked is the right one or whether the framing itself is wrong-shaped."

In practice — and PL's diagnostic exposes this cleanly — the gate operated as **"Is my question well-formed?"** This is a within-frame coherence check. It catches typos, missing context, internally-inconsistent leans, ambiguous Q-item language. It does NOT catch structural mis-frames.

The sharpened gate: **"Should I be asking a different question entirely?"**

This is a distinct mental motion. Not "is this question correct?" but "is there a question I should be asking instead that this one is hiding?" The cognitive difference is the difference between proofreading and editing. The first asks whether the text is correct; the second asks whether the text should exist at all.

Worked example, Phase 3 Q5.a:
- Well-formed-question check (old gate): "Is 'content-addressed-at-block-layer with BLAKE3 4 KiB blocks' a coherent substrate for NeoFS?" → Yes, coherent. PASSES.
- Sharpened check (new gate): "Is there a question I should be asking instead — namely, 'should content-addressing be substrate or feature?'" → That's the question Cerebro answered. SURFACES.

## 3. The cadence rule

**First Cerebro dispatch on a new Phase substrate = explicit structural-challenge dispatch, not prior-art-anchoring dispatch.**

Dispatch question shape:
- "Here is PM's working frame for Phase N: [enumerated substrate leans, BUILD scope hypothesis, Q-items as exploration]."
- "**Challenge the shape of the question, not just answer questions within it.**"
- "Where does production prior art say the framing itself is wrong?"
- "What substrate-level reframe would production systems force on this design? Which of PM's substrate-vs-feature distinctions are inverted? Which BUILD-vs-deferred boundaries are wrongly placed?"

What this is NOT: a dispatch asking Cerebro to validate PM's specific design leans (D-items, Q-items) against prior art. That's a within-frame question-level dispatch — the right shape for Cerebro #2, #3, and onward, NOT for Cerebro #1.

## 4. Definition of "new substrate"

The rule applies when:
- Entering a new Phase (not a sub-step within an established phase).
- The substrate space is significantly less mature in PM head-state than the previous phase's was.
- PM does not have prior implementation experience in this substrate.

The rule does NOT apply when:
- Within an established phase doing sub-step or BUILD-level work.
- Question is narrow enough that "is my question well-formed?" is the right gate.
- PM has direct implementation experience in the substrate (so framing-level errors are unlikely).

Phase boundary examples:
- Phase 2 BUILD-3 SC-4 D5 was a sub-step within established Phase 2 substrate — but the Cerebro contribution was still structural-reframe. This suggests the rule may need to extend beyond "new Phase" to "new sub-substrate within a Phase" in some cases. Open question; resolves with more data points.
- Phase 3 Q5.a was new-substrate (Phase 3 is fresh; PM has no FS implementation experience in this kernel).
- Phase 4 (network stack) will be mid-maturity new-substrate; PM has Linux network stack mental models but no NeoPlutarch-specific implementation.
- Phase 5 (AI subsystem) will be LOW-maturity new-substrate; ANSPEC-001 is design substrate, not implementation experience.

## 5. THINK v1 status under the new cadence

THINK v1 on new substrate is **explicitly pre-structural-challenge exploration.** Frame:
- Q-items are leans, not substrate.
- BUILD scope is a working hypothesis, not a commitment.
- D-decisions are PM's first-pass enumeration of the design space.
- Cerebro #1 is invited to reshape any of it at the framing level.

THINK v1 documents should carry an explicit header tag: "Pre-structural-challenge exploration; expected to be substantially revised post-Cerebro-#1."

THINK v2 on new substrate is **post-structural-challenge synthesis.** Q-items anchored against Cerebro #1's structural-challenge return. BUILD scope committed (within reason; subsequent Cerebro dispatches at question-level may still surface refinements).

Subsequent THINK iterations (v3+) integrate Cerebro #2 + #3 + ... at the question level. These are refinement passes, not reframes.

## 6. Retroactive application to current Phase 3 work

Cerebro #1 just returned. The dispatch was framed as prior-art-anchoring ("NeoFS prior art; anchors Q5.a-e"), NOT structural-challenge. Despite the framing, Cerebro did the structural-challenge work anyway — the substrate gap was visible from prior-art survey because three production CAS systems' post-mortems pointed unambiguously to single-tier-first. **The new cadence rule would have made this deliberate rather than incidental.**

Cerebro #2 (storage drivers) and Cerebro #3 (VFS) are **within the structurally-challenged Phase 3 frame** Cerebro #1 established. They are question-level dispatches now. Framings per `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §6 stand without modification.

## 7. Prospective application — Phase 4 and Phase 5

**Phase 4 (network stack).** Mid-maturity new-substrate. First Cerebro dispatch = structural-challenge. Dispatch frame: "PM's lean is in-kernel stack, smoltcp-derived or original, IPv6-default, not-BSD-sockets API. Bible Article 11.2 establishes posture. Challenge: where does production prior art say this framing is wrong? Should the syscall surface be designed differently? Should the kernel-vs-userspace boundary sit elsewhere? Should we expect more reframe than PM is currently positioned to imagine?"

**Phase 5 (AI subsystem).** LOW-maturity new-substrate; the highest-stakes phase of the project. ANSPEC-001 exists but is design substrate, not implementation experience. ALTIMIT artifact is research, not implementation. First Cerebro dispatch = **the largest structural-challenge Cerebro dispatch of the project to date.** Time budget: 12-20 hours minimum. Explicit invitation to tear up ANSPEC-001 substrate at the framing level if production AI-OS prior art (or its conspicuous absence) warrants. This is the dispatch where getting the framing wrong is project-defining.

The Phase 5 structural-challenge dispatch should explicitly invite Cerebro to challenge:
- Whether kernel-resident tensor/model/inference-context primitives are correctly factored (vs split differently).
- Whether the syscall numbering and surface (0x100..0x1FF) anticipates the right operation set.
- Whether the inference scheduler should be a sibling to the main scheduler or integrated.
- Whether the Bible 23 anti-scope (no system AI persona, no embedded models) is sufficient or under-specified for the engineering implications.
- Whether the accelerator driver class abstraction matches what real accelerator hardware exposes circa 2026-2027.

## 8. Forward marker — Bible Article 14 amendment candidacy

If the structural-challenge-first cadence pays off at Phase 4 (i.e., either confirms PM framing cleanly OR reframes substantively without operational pain), **promote to Bible Article 14 amendment at Phase 4 close.** Three-for-three would be evidence the cadence rule generalizes.

Per Bible Article 27, amendment requires rationale + version bump + PM approval (PM authority per corrected role model; PL is courier).

If Phase 4 structural-challenge dispatch produces minimal reframe (PM head-state was correct because PM has Linux network stack experience), do NOT amend the bible — the rule is project cadence, not normative discipline, and may not apply uniformly when PM has direct substrate experience. In that case: leave as PM-NOTE-tier cadence guidance.

Either way, file a closure PM-NOTE at Phase 4 substrate-entry capturing the outcome.

## 9. Closure

The pre-dispatch framing-gate is renamed and sharpened: "Should I be asking a different question entirely?" not "Is my question well-formed?" The first Cerebro dispatch on new substrate is explicitly structural-challenge. THINK v1 on new substrate is explicitly pre-challenge exploration with a header tag.

Phase 3 Cerebro #1's outcome was a lucky version of this. Phase 4 first-Cerebro and Phase 5 first-Cerebro will be the deliberate version. Bible Article 14 amendment candidacy resolves at Phase 4 close.

---

**End of PM-NOTE.**

*First Cerebro dispatch on new substrate = structural-challenge, not prior-art-anchoring. Pre-dispatch framing-gate sharpened from "is my question well-formed?" to "should I be asking a different question entirely?" THINK v1 = pre-challenge exploration. Cerebro #2/#3 on Phase 3 stay at question-level. Phase 4/5 first-Cerebro dispatches apply the rule deliberately. Bible Article 14 amendment candidate if pattern holds three-for-three at Phase 4 close.*
