# PM-NOTE: Cerebro dispatch framing — two-level diagnostic

| | |
|---|---|
| **Date** | 2026-05-12 |
| **Author** | PM (NeoPlutarch project cadence) |
| **Type** | Project-cadence artifact (not phase-specific; applies to all future Cerebro dispatches) |
| **Trigger** | Pattern observed twice: SC-4 D5 (process-table SMP) and Phase 3 NeoFS Cerebro #1. The PM-NOTE `PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §2 named the symptom correctly but the mechanism incompletely. This artifact states the mechanism precisely. |

---

## 1. The symptom

Cerebro returned a reframing of PM's question, not an answer to it. In both cases, this was the decisive contribution.

- SC-4 D5: PM asked "what lock type for PROCESSES?" Cerebro returned "should PROCESSES be on the hot path at all?"
- Phase 3 #1: PM asked "validate these five BUILD-1 design decisions." Cerebro returned "defer content-addressing to BUILD-2 entirely — the structural wins don't require it."

The pre-dispatch framing-gate ("have I framed the right question?") did not prevent either recurrence.

---

## 2. The mechanism — two-level framing

PM's design work operates at two distinct levels, which are easy to conflate:

**Level 1 — PM framing:** what question should be asked? What is the design space? What are the decision axes?

**Level 2 — question-level:** within that framing, what is the right answer?

The failure mode: PM writes THINK v1 operating entirely at Level 1. Cerebro is dispatched to answer a Level 2 question. Cerebro discovers the Level 1 framing is wrong and answers a Level 1 question instead. PM receives the return, discovers it's a Level 1 correction, writes THINK v2, and re-dispatches (or re-consumes with material scope changes).

This failure mode is not catchable by the prior framing-gate formulation: "is my question well-formed?" is itself a Level 2 operation. To ask whether a question is well-formed, you already have a question — you're testing it, not finding it. The gate stays at Level 1 and cannot catch Level 1 errors from Level 1 inspection.

The gate that would catch it: "should I be asking a different question entirely?" This is a Level 0 operation — stepping outside the current question frame. It requires temporarily suspending the framing and asking whether the problem being solved is the problem that exists.

These are not a wording difference. They are different cognitive acts:

- "Is my question well-formed?" → reads the current question; checks internal consistency; returns yes/no
- "Should I be asking a different question entirely?" → reads the domain; checks whether the current question correctly represents the domain's actual demand; returns a possibly-different question

The second gate requires domain knowledge to operate — specifically, knowledge of what production systems have found out the hard way, which is exactly what Cerebro provides.

---

## 3. The operational implication

This means the pre-dispatch framing-gate cannot be a PM-internal gate alone on new-phase or new-substrate work. The gate requires domain knowledge PM does not yet have. Cerebro is that domain knowledge.

**The corrected cadence for new-phase substrate:**

1. PM writes a **substrate characterization** (what do we know, what are the open questions, what are the design axes) — this is THINK v1 in its current form.
2. PM dispatches **Cerebro with a Level-0 mandate**: "What does the prior art say the right questions are? What are the questions PM hasn't yet asked? What production systems would refuse PM's current framing and why?"
3. PM receives Cerebro's Level-0 output. If reframing is present, PM accepts or rejects it with reasoning. PM writes THINK v2 incorporating the settled framing.
4. PM dispatches Cerebro with **Level-2 mandates** (the current SC-3C D15 / SC-4 D5 / Phase 3 #1 shape): validate specific decisions within the now-settled framing.

Under the current cadence, steps 2 and 4 are both Level-2 dispatches. The Level-0 dispatch is missing. Cerebro is finding Level-0 problems while doing Level-2 work — which produces correct output but forces a THINK v2 cycle that would have been unnecessary if the Level-0 question had been asked first.

**The explicit framing for a Level-0 dispatch:**

> "PM's current frame for [substrate/phase/subsystem] is [X]. What does the prior art say is wrong with this frame? What are the questions PM hasn't thought to ask? What production systems built [something like X] and what did they discover?"

This is explicitly an invitation to reframing, not validation. It should appear before THINK v1 is treated as authoritative, not after.

---

## 4. What this doesn't change

- Level-2 Cerebro dispatches (SC-3C D15, SC-4 D5, Phase 3 #1) remain the right shape for validating settled decisions. They are not wrong; they arrived at the wrong point in the sequence.
- The SC-4 D5 and Phase 3 #1 reframes were still highly valuable even arriving post-THINK-v1 — the cost was one re-work cycle, not a fundamental failure. The two-for-two pattern is useful signal, not a crisis.
- This artifact does not change Phase 3's current forward path. THINK v2, ADR-021, and Cerebro #2/#3 proceed as specified in the Phase 3 consumption PM-NOTE. The corrected cadence applies starting from **Phase 4 substrate work**.

---

## 5. The Phase 4 commitment

Phase 4's primary substrate is the network stack. Before Phase 4 THINK v1 is written, PM should dispatch a **Level-0 Cerebro survey** with the mandate: "What do TCP/IP stack prior-art systems (Plan 9, seL4 network server, Theseus, Redox) find to be the wrong questions for a microkernel network stack?" The Phase 4 THINK v1 framing should incorporate that return.

The equivalent for any subsequent new-phase substrate: before writing THINK v1 authoritatively, execute the Level-0 survey as the first document in the phase folder, not the last.

---

## 6. Cross-references

**Triggered by:**
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §2 — the incomplete mechanism this artifact corrects
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` — the first instance of the pattern

**Referenced by:**
- `Plans/PM-NOTE-2026-05-11-phase-3-cerebro-1-consumption-and-q5-reframed.md` §2 (amended to point here)
- Phase 4 THINK v1 (when written) — should consume this artifact's §5 commitment before authoring

**Stored in memory:**
- `C:\Users\<user>\.claude\projects\E--AI-Automation-AltimitNeo\memory\feedback_cerebro_first_move_cadence.md`

---

**End of PM-NOTE.**

*The two-level framing diagnostic: PM writes THINK v1 at Level 1 (question framing); Cerebro is dispatched for Level 2 (question answering); Cerebro discovers Level 1 is wrong. The failure mode is not catchable by "is my question well-formed?" — that is itself a Level 2 operation. The gate that catches it is "should I be asking a different question entirely?" — a Level 0 operation requiring domain knowledge PM doesn't yet have. Corrected cadence: Level-0 Cerebro survey before THINK v1 is authoritative, starting from Phase 4.*
