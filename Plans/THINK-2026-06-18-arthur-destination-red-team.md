<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: red-team session — the Arthur-as-destination probes

**Date:** 2026-06-18
**From:** PM (Claude)
**Context:** A PL-driven strategic red-team session. Unlike the 2026-06-17 adversarial-verification
log (`THINK-2026-06-17-i9-i13-adversarial-verification.md`), which attacked a *technical* invariant
(the I-9/I-13 refcount+generation core) with code-grounded probes, this session attacked the
*strategy and architecture* — and used the project's own recent "Arthur is the destination" turn
against ADR-001, ADR-042, and the kernel/userland line. This log records the probes, the verdicts,
where the findings went, and the two reusable lessons — so the reasoning survives the chat that
produced it. The OT-15…OT-18 entries cite "PL red-team session 2026-06-18" as their source; **this
is that source.**

The standing rule of the exercise (the discipline this whole project runs on): **no defending with
vapor.** A claim that something is built when it is only designed is a loss, not a save. Every probe
below ends either *refuted from the constitution* or *conceded and banked* — never hand-waved.

## 1. The probes and verdicts

| # | Probe (PL) | Target | Verdict |
|---|------------|--------|---------|
| E1 | **Ecosystem vacuum** — clean-slate ABI ⇒ an isolation tax; "perfect but useless research experiment." | Article 1 | **Refuted + conceded.** Native-ABI debt ≠ permanent incompatibility (personality layer, Article 5.6/5.7 + ADR-023). But the bridge is **unbuilt**, and the adoption leap is **KC-3 (terminal)**. |
| E2 | **Non-deterministic kernel** — "Correctness #1" vs probabilistic/black-box ML. | Article 2 | **Refuted + 2 concessions.** Kernel *governs* intelligence, doesn't *run* it (Article 2.5); compute is deterministic (the determinism KATs, result-invariance @SMP=4); sampling is seeded userland policy. Concede: cross-*device* equivalence is **tolerance-bounded** not bit-exact (§9.4); "correct execution" ≠ "correct answer" (the safety net is the capability perimeter, not a proof of the model). |
| B1 | **Bootstrap deadlock** — where does the toolchain live; are you a complex Linux container? | ADR-023 / Article 1 | **Refuted + conceded.** Build-toolchain (host-side, cross-compiled `x86_64-neoplutarch.json`) ≠ runtime ABI; boots bare-metal (not a container). Concede: **not self-hosting** — a real, distant, unbuilt milestone. |
| B2 | **Semantic collapse** — an "AI-native" kernel reroutes a guest's `read()` ⇒ a "hallucinating substrate." | Article 2 / 5.7 | **Refuted in the premise.** No inference in the resource path (2.5/6.6/23.3); placement is deterministic + result-invariant — the feared mechanism is architecturally forbidden. Concede: POSIX-emulation **fidelity** is a hard, unbuilt per-syscall burden (the WSL1/Wine cost). |
| B3 | **Resource exhaustion / "personality leak" DoS** — does "isolated by construction" cover scheduler latency? | Article 5.6 | **Partly defended, real gap conceded.** "Isolated by construction" is *ABI* isolation, not *temporal*. The AI surface is the *best*-defended (§9.5/§9.6 admission control, I-14(b) silicon-proven). Concede: **general CPU-time QoS under a hostile hog is unbuilt** → **OT-15**. |
| A1 | **Layer leak** — is the product's intelligence in the most expensive place to change? | ADR-001 | **Mostly foreclosed, one real leak.** The fast-changing product layer is already userland (ADR-001 items 3–5; Arthur Slices 1–4). Leak: `forward_logic.rs` is **Qwen2-hardcoded** in ring 0 → should be `.neom`-graph-data-driven (§19.2) → **OT-16**. |
| A2 | **Is "kernel-owned models" the right primitive?** — delete it, keep authority + device-memory + capabilities; does the thesis survive? | ADR-001 / Article 2 | **Conceded — and the architecture already agrees.** Phase-7 O1/O2 made the primitive **tensor + capability + residency**; `Model = Vec<SlotKey>` is a thin aggregate. "Kernel-owned model" is legacy framing → thesis restatement (see §6). |
| A3 | **Slot arena optimized for today's scarce resource (model memory) vs tomorrow's (persistent context)?** | ADR-042 | **Half each + real gap.** Arena is correctly tensor-only; durable personal context was never going to live there — it belongs in **NeoFS + a capability/privacy model**, which is **undesigned** and privacy-critical → **OT-17**. |
| A4 | **Vision ("think alongside you") vs Bible (governance/authority)** — companion vs compliance officer. | Article 29.3 / VISION | **Tension real, direction exists, ergonomics unbuilt.** Grants should be coarse/durable/front-loaded (silent cap check); but the blast-radius-vs-friction tradeoff is genuinely unresolved → **OT-18**. |
| A5 | **If Arthur succeeds, does LogansOS need to be a whole OS?** — platform vs OS; right destination, wrong vehicle? | the whole project | **= KC-3 / Bet D.** The trust property (29.3) requires substrate control — an Arthur *app* on a conventional OS *is* the ambient-authority agent the project calls unsafe. But the *marginal* gain over a hardened iOS/macOS sandbox may not justify the 100–1000× adoption cost. **The terminal, unhedged bet; not resolved.** |

## 2. Findings → where they went

- **OT-15** — scheduler QoS / per-principal CPU-time DoS-resistance (from B3).
- **OT-16** — model-architecture coupling in the ring-0 forward pass; `.neom`-data-driven graph (from A1).
- **OT-17** — Arthur's persistent-memory substrate (NeoFS + capability/privacy); **undesigned, privacy-critical** (from A3).
- **OT-18** — grant ergonomics; the companion-vs-compliance-officer / blast-radius tradeoff (from A4).
- **E1/A5** — already the ledger's **KC-3 (terminal)** + **Bet D**; no new entry, re-confirmed.
- **E2/B2 concessions** — tolerance-bounded cross-device equivalence (§9.4) and "correct execution ≠ correct answer" are existing, documented limits; restated, not new.

The three new product-gaps (OT-16/17/18) **converge on one frontier:** the post-Slice-4 Arthur
agent-maturation work, when Arthur grows *memory*, *authority*, and *a second model*. OT-17 and OT-18
are the same privacy boundary (memory and authority co-design). None bites today — one trusted
principal, one model, ephemeral context — which is exactly why they are *banked* rather than *urgent*.

## 3. Lesson 1 — the "invented mechanism vs real gap" lens

Every strong attack this session had the same shape: a **dramatic, constitution-foreclosed
mechanism** wrapped around a **genuine unbuilt gap**.

| Attack | The invented (foreclosed) mechanism | The real (unbuilt) gap |
|--------|-------------------------------------|------------------------|
| B1 | a *build deadlock* / "complex Linux container" | self-hosting |
| B2 | a *hallucinating substrate* rerouting syscalls | POSIX-translation fidelity |
| B3 | a *DoS of the AI primitives* | general CPU-time QoS |
| A1 | *product intelligence in ring 0* (broadly) | the Qwen2-hardcoded forward graph (narrowly) |

The lens is reusable, for red-teaming *and* for honest self-assessment: **when an attack lands,
separate the costume from the gap.** Defend the costume from the architecture (these are category
errors the constitution explicitly forecloses); bank the gap (these are the things the OPEN-THREADS
register exists to hold so they don't rot into "glossed over"). An attack that produces neither a
refutation nor a banked gap was answered with vapor — a failure of the exercise, not a success.

## 4. Lesson 2 — the road → traveler phase-shift

The probes clustered, not by accident, on a real transition. The **road** (the kernel, Phases 0–7.5)
was a **correctness-shaped** problem set: every hard question had ground truth and a falsifier — the
byte-oracle, the invariant ledger, cached==uncached, the fence refusal. The project's core discipline
(byte-oracles, verify-before-merge, the invariant ledger) is *machinery for problems that have an
oracle.*

OT-16/17/18 are the first problems where **correctness is necessary but no longer the hard part.**
There is no byte-oracle for "does this grant feel like a companion or a compliance officer," no KAT
for "what should Arthur remember, and what's the contract when it forgets." These are
**judgment-shaped and value-shaped**, and the discipline that built the road does not directly
transfer — *you cannot out-rigor your way to good grant ergonomics.*

Two things make this a transition, not a crisis:

1. **The road is what makes the traveler safe to attempt.** OT-18's blast-radius concern is *bounded*
   only because the capability system is real and silicon-proven; OT-17's privacy contract is
   *enforceable* only because the kernel governs authority by construction. A companion-vs-cop
   question on an ambient-authority OS is unanswerable — you cannot tune the friction of a companion
   you cannot contain. The two months of road turned the traveler problems from *impossible* into
   *merely hard.*
2. **The traveler's falsifier is N=1 real use.** The road was falsified by oracles; the traveler can
   only be falsified by the author actually living with Arthur, because "feels like a companion" has
   exactly one test instrument and it is a human using it. This is *why* the build lane (Slices, 9.x,
   the input path) is not a detour from OT-17/18 — it is the apparatus that makes them testable at
   all.

**The caution that keeps this from becoming a rationalization:** "the interesting problems are about
the traveler now" is true, but the road still has load-bearing unfinished sections — OT-15 (CPU-QoS),
POSIX fidelity, self-hosting. They don't bite today (one trusted principal) but they are exactly the
sections that fail the day a personality or an untrusted process arrives. The traveler earns the
*attention*; the road's named debts cannot be allowed to rot because they're less exciting. The
honest statement is **not** "the road is done" — it is "the road is correct enough to carry a
traveler."

## 5. The A2 thesis-framing decision — RESOLVED (PL sign-off 2026-06-18)

**Decision: option (a)** — recorded as **S5** in Strategic Honesty Ledger §2 (Structural Claims);
the Bible/ADR-001 normative text is intentionally left unchanged (the code already embodies it, only
the words lag). The original framing of the decision is preserved below for the record.

### Original framing (for the record)

Probe A2 surfaced a **thesis-framing** update, which is a PL call because it restates a foundational
premise. The accurate thesis is no longer "models are kernel objects" (ADR-001 / Article 2 wording) —
Phase-7 (O1/O2) already made it false in code. The accurate statement is:

> *The kernel governs AI compute **and the memory it touches** as capability-secured resources. The
> primitive is **tensor + capability + residency**; a **model** is a thin aggregate over governed
> tensors.*

Options: **(a)** add this as a one-line clarifying note to Strategic Honesty Ledger §2 (Structural
Claims), leaving the Bible/ADR-001 normative text untouched; or **(b)** decide the premise itself
should be reworded (a Bible/ADR amendment). PM recommendation: **(a)** — the code already embodies it;
the words just lag, and a ledger note closes the gap without a normative change. Pending PL sign-off.
