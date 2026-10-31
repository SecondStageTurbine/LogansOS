# PM Note: Agent roster posture

**Date:** 2026-05-09
**Author:** PM
**Type:** Operational standing-rule documentation. Not a Bible amendment, not an ADR, not a dispatch. Captures the project-lead-ratified routing rules for coder-agent assignment, the rationale for current roster size, and the triggers that would justify expansion. New PM sessions inherit this posture rather than relitigating it. Per Bible Article 18.1, this is documentation, not normative governance — routing changes from PL stand without ADR overhead, but the rationale is preserved here.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f`. Working tree clean. Stashes preserved per Phase B exit state.

---

## 1. Current roster

Three active coder-agents:

- **Cerebro** (coder 1) — primary kernel implementer. Has direct source access, can `git grep`, can build, can run kernel tests in QEMU. Most reliable for kernel work. Established calibration: high source-audit fidelity, careful stop-discipline (honored both BUILD §9.5 and Fix 6.1 stop conditions cleanly), good independent reasoning in investigation work (reached Hn at 50% in pre-synthesis pass with full source access).
- **Prometheus** (coder 2) — document author and parallel research participant. Has source access. Established calibration: clean structural output, occasional source-audit gaps on serialization context (PLAN-stage Q6 lock-order inversion; investigation-stage H7 ruled out by SwitchMutex audit Prometheus didn't engage with). Useful as parallel/redundant analyst; PM's primary for document work.
- **Claude-3** (coder 3) — fresh-session research participant. No persistent project context; PL provides files per dispatch. Established calibration: strong analytical perspective when given source files; correctly reached Hn at 55% in SC-3B investigation with constrained file access. Useful for fresh-perspective triangulation.

**Retired:**

- **Jarvis** — formerly handled INDEX hygiene and docs-only work. Retired 2026-05-09. Document-only work consolidates to Prometheus (including INDEX hygiene formerly Jarvis's domain). Historical references to Jarvis in dispatches and notes prior to 2026-05-09 stand as record.

---

## 2. Routing rules (PL-ratified 2026-05-09)

The routing rules are simple and exhaustive:

### 2.1 Hard kernel / OS work → Cerebro

Includes anything that:
- Modifies `kernel/src/` or any source under the kernel build.
- Runs `cargo k`, `cargo neo-test`, `cargo neo-tty`, or any kernel build/boot command.
- Touches the substrate at the commit level (commits, amends, rebases on kernel-tracked code).
- Applies stashes to the kernel working tree (e.g., NMI watchdog instrumentation).
- Executes kernel verification gates in QEMU.

Cerebro is the sole authorized agent for this category. No parallel kernel implementation; per the L precedent, parallelism on kernel work creates merge surface that's worse than the tie-breaking value.

### 2.2 Document-only work → Prometheus

Includes:
- ADR drafts and amendments (e.g., ADR-013 §Q4/§Q6 implementation sub-sections after SC-3B closes).
- Bible amendment drafts (e.g., the 8.5 critical-section discipline draft was a Prometheus-class artifact).
- PHASE-N.md updates.
- Close reports (`Plans/CLOSE-REPORT-*.md`).
- INDEX sweep maintenance (formerly Jarvis's; now Prometheus).
- Working-tree-only artifacts in `Plans/` that are author-grade prose, not implementation.

Prometheus is the sole authorized agent for this category by default. PM remains the reviewer and gatekeeper before dispatches forward; calibration data on Prometheus's source-audit gaps means doc artifacts that cross-reference source need PM verification of the references. PM voice anchoring (matching L discharge tone, SC-3B PLAN synthesis structure, etc.) is enforced via explicit style direction in each Prometheus dispatch.

### 2.3 Research / investigation / parallel analysis → any of {Cerebro, Prometheus, Claude-3}

Used for:
- Multi-agent parallel investigations (e.g., the SC-3B three-way investigation).
- THINK-stage or PLAN-stage parallel analysis where independent perspectives are valuable.
- Hypothesis enumeration, source-audit reconnaissance, design synthesis input.

Any subset of the three, including all three, may be dispatched in parallel. The convention is `-parallel.md` suffix on the dispatch filename and per-agent output filenames following `{base}-{agent}.md`. Agents do not coordinate or read each other's outputs before submitting; PM synthesizes after all return.

The triple-parallel cost on SC-3B was substantial but produced two corroborations of Hn (Cerebro + Claude-3) plus calibration data (Prometheus's H7). The pattern is established. PM continues to synthesize.

### 2.4 Mixed kernel + doc work

When an artifact spans both kernel and docs (e.g., a kernel commit that needs a corresponding ADR amendment), the dispatch goes to Cerebro for the kernel commit and a follow-up dispatch goes to Prometheus for the ADR. **Two dispatches, sequential, not parallel.** Docs follow ratified implementation rather than the other way around.

This pattern matches the established Sub-step 6 cadence: Cerebro implements, gate verifies, Prometheus documents. Sub-commit boundaries hold the work together.

---

## 3. Why the roster is three, not four

Project lead surfaced (2026-05-09) the question of adding a fourth same-tier agent ("Janus" at Prometheus's level). PM analysis recommended **hold off**. The recommendation was ratified.

### 3.1 Marginal value of a fourth same-tier agent

If Janus is functionally identical to Prometheus rather than tuned-different (e.g., adversarial-review specialist, formal-methods-leaning, structural-defect specialist), the marginal value reduces to:

- **Prometheus unavailable for a session** — real value, but rare.
- **Two parallel document streams in flight** — not currently the load. Phase 2 BUILD-3 work is sequential; doc workload is one stream at a time.

### 3.2 Marginal cost of a fourth

- **Calibration restart.** Each agent's failure modes need observation across several dispatches before PM can route confidently. Prometheus's PLAN-stage drift on Q6 lock-order took two investigation cycles to characterize. Janus would take a similar cycle.
- **Style drift.** Two same-tier agents writing the same artifact types diverge in voice over time even when given identical instructions. The house style currently coherent because one document author (Prometheus) plus PM review is enforcing it. Two authors in the same lane requires either explicit style anchoring per dispatch or post-hoc voice reconciliation in PM review.
- **Quadruple-parallel synthesis cost.** Four reports compared pairwise is six comparisons vs. three for triple. PM synthesis time roughly doubles. The SC-3B investigation synthesis was already ~700 lines for three agents.

### 3.3 Bible 4.3 framing

Bible Article 4.3 ("every external dependency is a liability") applies analogously to project staffing: every agent is a calibration cost and a synthesis cost. Three is working. Add a fourth when there's a load to absorb, not before.

The lean-roster posture is consistent with the project's broader engineering discipline: minimal moving parts, justified additions, clear standards. It mirrors the kernel's own dependency posture (every external crate justified in writing in `Cargo.toml` comments per Bible 4.3 mechanics).

---

## 4. Triggers that would justify roster expansion

PM revisits the three-vs-four decision if any of the following materializes:

### 4.1 Saturation triggers

- **Prometheus unavailable for a multi-day stretch** while doc work is queued. (Operational, not strategic.)
- **Two or more document streams need to land in the same dispatch cycle.** Concrete example: Phase 5 opens with ANSPEC-001 amendments + new ADRs for tensor primitives + PHASE-5.md milestone updates landing in the same week. Sequential serialization through Prometheus would block Phase 5 progress; parallel doc-tier helps.
- **A research investigation produces three genuinely-divergent top hypotheses** with similar confidence and PM cannot synthesize without a tie-breaker. Hasn't happened yet (SC-3B investigation had two corroborations on Hn; not three-way divergent).

### 4.2 Capability-gap triggers

If the project encounters a task type that Cerebro / Prometheus / Claude-3 can't cover, the right move is a tuned-different fourth, not a same-tier fourth. Examples of plausible gaps:

- **Adversarial review** — a fourth tuned to actively seek refutations of PM and other agents' analyses. Different from current roster which generally engages constructively.
- **Formal-methods specialist** — tuned for verification-style reasoning (Asterinas OSTD-style, seL4-derived). Becomes valuable if/when AltimitNeo formal verification work opens (Phase 5+ per the ALTIMIT-real-world-implementation report's Stage 3 recommendations).
- **Performance / benchmarking specialist** — tuned for measurement, regression analysis, micro-architecture reasoning. Becomes valuable when Phase 2's performance work begins per Bible 16.4.

PM does not currently recommend adding any of these. The triggers above are forward markers for if and when the project's work mix shifts.

### 4.3 What does NOT trigger expansion

- **PM workload feels heavy.** Synthesis cost is the PM's job; adding agents shifts work from each agent to PM, not the reverse.
- **Aesthetic preference for "more parallel."** Triple-parallel was used once (SC-3B investigation) because the failure was high-stakes and ambiguous. Most dispatches are single-agent.
- **Pressure from project-lead to expand capacity for capacity's sake.** Capacity should follow load, not anticipate it speculatively.

---

## 5. Operational notes

### 5.1 Naming conventions for parallel dispatches

When dispatching to multiple agents in parallel:

- Dispatch filename: `Plans/DISPATCH-YYYY-MM-DD-<topic>-parallel.md`. The `-parallel.md` suffix signals multi-recipient; absence of suffix signals single-recipient.
- Per-agent output filenames: `Plans/<base>-<agent>.md` (e.g., `SC-3B-INVESTIGATION-2026-05-09-cerebro.md`).
- PM synthesis filename: `Plans/<base>-SYNTHESIS-YYYY-MM-DD.md` (e.g., `SC-3B-PLAN-SYNTHESIS-2026-05-09.md`).

Single-recipient dispatches use `-cerebro.md` or `-prometheus.md` suffix per the established pattern.

### 5.2 Calibration data accumulation

Calibration on each agent accumulates across dispatches and is referenced honestly in handoffs. Current calibration captured in:

- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §3.4 (Prometheus calibration).
- This note's §1 (all three).
- Future close reports for arcs spanning multiple dispatches per agent.

Calibration is descriptive, not pejorative. Agents engage seriously and produce useful work; specific failure modes are documentation, not criticism.

### 5.3 Cerebro's privileged position

Cerebro is the sole authorized kernel implementer and runs all kernel verification gates. This concentration is deliberate — kernel work is sequential, and parallel kernel implementation creates merge-surface costs. The L precedent demonstrated this: a single Cerebro implementation of A1 plus a single Cerebro implementation of L (the localized fix) closed the wedge investigation cleanly. Parallel implementation would have produced two patches needing reconciliation.

This also means Cerebro is a single point of dependency for kernel work. If Cerebro is unavailable, kernel work pauses. PM accepts this. The alternative — training a second kernel implementer — has higher cost than the rare unavailability cost.

### 5.4 Document-only work has voice anchoring requirement

When dispatching document work to Prometheus, PM includes explicit voice anchoring:

- Reference the prior artifact whose tone should match (e.g., "match the L discharge framing," "structure mirrors `Plans/SC-3B-VERIFICATION-2026-05-09.md`").
- Specify any honest-framing requirements (e.g., the SC-3B close report must preserve "observed but unreproducible" language; not soften to "fixed").
- Specify cross-reference fidelity requirements (e.g., "verify all `kernel/src/` paths cited resolve to existing files at HEAD").

PM reviews Prometheus output before forwarding for ratification. The review is a calibration-cost line item.

---

## 6. Forward state

Roster posture stands until a §4 trigger materializes. PM revisits proactively if:

- Phase 5 opens (anticipated multi-stream doc workload).
- Performance work begins per Bible 16.4 and Phase 2 closes (anticipated capability gap).
- Formal verification work opens per ALTIMIT-real-world-implementation report Stage 3 recommendations (anticipated capability gap).

In the absence of trigger materialization, no roster change. New PM sessions inherit this posture from this note.

---

## 7. Cross-references

**Authority:**
- Project-lead delegation 2026-05-09 of routing rule ratification to PM (within established four-anchor constraint: Bible + real-world OS + Pluto's-Kiss-survivor + AI-at-kernel-level).
- Project-lead ratification 2026-05-09 of three-agent roster (PM recommended hold-off on Janus addition).

**Operational precedent:**
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md` — three-way parallel research dispatch convention.
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md` — PM synthesis structure.
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §3.4 — Prometheus calibration record.

**Governing posture:**
- `docs/bible.md` Articles 1.3 (every commit intended to ship — applies to dispatches as commitments), 4.3 (every dependency a liability — applied here to staffing), 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY cadence — agent assignment maps to cadence stages), 18.1 (documentation discipline).

**Forward markers (Phase 5+):**
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) — anticipated Phase 5 doc workload.
- `Plans/ALTIMIT-real-world-implementation.md` (research artifact) Stage 3 — anticipated formal-verification capability gap.

---

**End of note.**

*Roster: Cerebro (kernel), Prometheus (docs + research), Claude-3 (research). Jarvis retired. Janus held off pending §4 triggers. Bible 4.3 applied to staffing: lean roster, justified additions. New PM sessions inherit this posture without relitigating.*
