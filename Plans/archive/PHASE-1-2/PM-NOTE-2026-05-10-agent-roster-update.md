# PM Note: Agent roster update — Cerebro/Jarvis/Prometheus, Janus retired, Claude-3 absorbed

**Date:** 2026-05-10 (later session, post-Sub-step-6 close)
**Author:** PM
**Type:** Operational standing-rule documentation. Supersedes both `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` and `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`. Project lead reconfigured the agent roster on 2026-05-10 (evening) following the Sub-step 6 close + slogan addendum landing. The prior two notes stand as historical record of the previous posture; this note is operative forward. Per Bible Article 18.1, this is documentation, not normative governance — routing changes from PL stand without ADR overhead, but the rationale is preserved here.
**Substrate:** Project HEAD `3a2cb70260cc69ef82ad1964c9d34dea300253b3`. Local `main` matches `origin/main`. Working tree clean.

---

## 1. New roster

Three active agents, each with a single specified model backing and a single primary lane:

| Role | Model | Primary lane |
|---|---|---|
| **Jarvis** | Codex | Heavy coding — kernel source, kernel build/boot, kernel verification gates |
| **Cerebro** | Claude | Lighter coding + deep internet research |
| **Prometheus** | Qwen3.6-27B | Documenting, amending, document cleanup; trial scope for small coding |

**Retired:**

- **Janus** (`deepseek-v4-flash:cloud`) — deep-research specialist for low-level systems work. Retired 2026-05-10. The deep-research function consolidates onto Cerebro under the new roster. Three Janus dispatches landed during the agent's lifetime (SC-3B silent-timeout prior art, SC-3B release-on-switch prior art, NeoPlutarch TM pre-flight). Calibration data captured in those artifacts and in `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` becomes historical-only; not transferable to Cerebro since model differs.

- **Claude-3** — fresh-session research participant / ad-hoc parallel-Claude seat under the prior roster. Absorbed into Cerebro under interpretation ratified by PL 2026-05-10 (evening). Going forward, all Claude-class work — fresh-context or not, light coding or deep research — routes to Cerebro. The `-claude-3.md` dispatch suffix convention is retired; existing files (`Plans/DISPATCH-2026-05-10-readme-slogan-addendum-claude-3.md`, `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`) stand as historical artifacts.

**Naming continuity note:**

The name "Jarvis" was previously used in this project (through 2026-05-09) for an INDEX-hygiene / docs-only agent that was retired 2026-05-09 and absorbed into Prometheus. The new Jarvis (Codex, kernel implementer) is unrelated to the old Jarvis (docs / INDEX). Historical references to "Jarvis" in dispatches and notes dated 2026-05-06 through 2026-05-09 refer to the old agent and the old role. Dispatches dated 2026-05-10 forward referring to Jarvis refer to the new agent (Codex) and the new role (heavy kernel coding).

---

## 2. Routing rules

### 2.1 Hard kernel / OS work → Jarvis

Includes anything that:

- Modifies `kernel/src/` or any source under the kernel build.
- Runs `cargo k`, `cargo neo-test`, `cargo neo-tty`, or any kernel build/boot command.
- Touches the substrate at the commit level on kernel-tracked code (commits, amends, rebases).
- Applies stashes to the kernel working tree.
- Executes kernel verification gates in QEMU.

Jarvis is the sole authorized agent for this category. The single-implementer concentration that previously applied to Cerebro carries forward unchanged in shape — kernel work remains sequential, no parallel kernel implementation, per the L precedent. The agent identity changes; the discipline does not.

### 2.2 Document-only work → Prometheus

Includes:

- ADR drafts and amendments.
- Bible amendment drafts.
- PHASE-N.md updates.
- Close reports (`Plans/CLOSE-REPORT-*.md`).
- INDEX sweep maintenance.
- Working-tree-only artifacts in `Plans/` that are author-grade prose.

Unchanged in shape from the prior roster. The agent identity is the same (Prometheus); the backing model changes from Claude Code-class to Qwen3.6-27B. Style anchoring discipline and PM review-before-forward discipline carry forward unchanged.

### 2.3 Light coding / deep research / research-informing-implementation → Cerebro

This is the synthesized lane combining what was previously Cerebro's lighter-side work + Claude-3's fresh-perspective research + Janus's long-form synthesis. Used for:

- **Light coding** — small dispatches that touch `tools/`, scripts, configuration, CI, or non-kernel build/test infrastructure. Anything that wouldn't trigger Jarvis's full kernel verification protocol.
- **`git grep` audits, source reconnaissance** — read-only investigation across kernel and userland source. The kernel-bible-alignment audit pattern lives here.
- **Deep research dispatches** — synthesizing across multiple sources (papers, prior art, kernel discussions, SDM chapters where applicable). Output is a research artifact (`Plans/RESEARCH-YYYY-MM-DD-<topic>-cerebro.md`); PM consumes and produces follow-up dispatches.
- **Research-informing-implementation patterns** — Cerebro researches; PM synthesizes / PLANs; Jarvis implements based on the synthesis. Three dispatches sequentially.
- **Triple-parallel investigation participation** — when warranted, Cerebro participates alongside Jarvis and Prometheus. Cost / benefit threshold for triple-parallel unchanged from prior posture: reserved for high-stakes ambiguous investigations.

### 2.4 Small-coding trial → Prometheus

New scope under this roster. PL has authorized Prometheus to attempt small coding work as a trial. "Small" means:

- Tooling scripts (`tools/`, build helpers, doc generators).
- Configuration file edits where the syntax is unambiguous.
- Test scaffolding that doesn't touch kernel source.

Explicit exclusions: anything in `kernel/src/`, `boot/`, or that requires running kernel verification gates. Those remain Jarvis's lane regardless of how small the change appears.

Trial scope carries calibration risk — Prometheus's Qwen3.6-27B backing is unproven for code generation in this project's context. PM treats the first 1-2 small-coding outputs as calibration data, not load-bearing implementation. If the calibration profile is acceptable, scope expands incrementally. If it isn't, small coding consolidates back to Jarvis for everything code-shaped.

### 2.5 Mixed work patterns

The prior posture's sequential-doc-after-implementation pattern carries forward with role substitutions:

- **Research → kernel work.** Cerebro researches; PM PLANs; Jarvis implements. Three dispatches sequentially.
- **Research → docs.** Cerebro researches; Prometheus drafts the doc citing the research artifact; PM reviews. Two dispatches sequentially.
- **Kernel + doc work.** Jarvis implements; Prometheus documents. Two dispatches sequentially. Docs follow ratified implementation, not the reverse.

---

## 3. Calibration discipline under the new roster

Three different starting points for the three agents:

### 3.1 Jarvis (Codex) — fresh calibration

New agent on this project. Failure modes unknown. Codex's general code-generation profile differs from Claude Code's; specific quirks for kernel work (`unsafe`, `no_std`, embedded HAL patterns, lifetime-heavy code) are unproven. PM treats the first 2-3 Jarvis kernel dispatches as calibration data, documents observed failure modes in handoffs as they appear, and reviews output with elevated intensity until a calibration profile firms up. Same discipline as the prior posture applied to Prometheus's PLAN-stage drift on Q6 lock-order.

The kernel-implementer concentration is structural (one implementer at a time, per L precedent), but it's a high-stakes seat. PM does not skip the calibration cycle for Jarvis even though the seat is critical.

### 3.2 Cerebro (Claude) — partial-transfer calibration

The model is Claude, which this project has worked with extensively. The role is new — Cerebro pivots from kernel implementer to lighter-coding + deep-research. Calibration data from kernel-implementer Cerebro (high source-audit fidelity, careful stop-discipline, good independent reasoning in investigation work) **partially transfers** to the new role:

- **Source-audit fidelity carries forward** for `git grep` audits and source reconnaissance work.
- **Stop-discipline carries forward** for any work with explicit gates (e.g., research dispatches with structured output requirements).
- **Independent reasoning carries forward** for investigation participation.

What doesn't transfer:

- **Deep-research-synthesis profile is unproven.** Cerebro hasn't done the long-form prior-art-synthesis work that Janus did. The first 1-2 Cerebro research dispatches are calibration data for this new lane.
- **Voice anchoring for research artifacts.** The prior research-artifact format was anchored on Janus's output style. Cerebro's natural voice differs; PM specifies output structure explicitly in research dispatches until a Cerebro-research voice profile establishes.

### 3.3 Prometheus (Qwen3.6-27B) — fresh calibration

Same role-name as the prior Prometheus, different backing model. Calibration data accumulated under the prior Claude Code-class Prometheus (clean structural output, source-audit gaps on serialization context, voice-anchored-via-explicit-direction) **does not transfer** to Qwen3.6-27B. PM treats the first 2-3 new-Prometheus dispatches as fresh calibration even though the role is unchanged.

Particular watch items:

- **Source-audit fidelity** when citing kernel files. Prior Prometheus had observed gaps here; Qwen3.6-27B's behavior unknown.
- **Voice match to existing artifacts.** Voice anchoring may need stronger direction than under the prior backing.
- **Cross-reference accuracy.** PM verifies cited paths and dates resolve correctly at HEAD for the first several dispatches.
- **Small-coding output quality** in the trial scope. Treated as data, not load-bearing, until a profile firms up.

---

## 4. Why this roster, why now

PL did not provide explicit rationale in the brief; the following is PM inference for the record:

- **Specialization sharper.** Each agent has a clearer single lane than the prior four-agent roster. Hard coding (Jarvis), light coding + research (Cerebro), docs + amendment + cleanup (Prometheus). Fewer overlap zones.
- **Codex for kernel work.** Codex is OpenAI's code-focused model class; routing the highest-stakes coding work to a code-specialty model is consistent with Bible 1.2 (correctness > coherence > performance — correctness benefits from model specialization).
- **Claude pivots to its strongest comparative lane.** Claude's analytical and synthesis capabilities apply directly to deep research and light coding without competing against Codex on heavy kernel implementation.
- **Qwen3.6-27B for docs.** Cheaper inference for the highest-volume artifact lane; the trial scope on small coding tests whether the model is also viable for low-stakes code work.
- **Roster consolidation.** Four agents → three. Synthesis cost on parallel investigation drops (six pairwise comparisons → three). The "marginal value of a fourth same-tier agent" analysis from `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` §3 reasserts itself; the Janus addendum's §3.1 "marginal value is concrete" argument no longer applies once the deep-research lane consolidates onto Cerebro.

The Bible Article 4.3 framing ("every dependency is a liability") applied analogously to staffing carries forward and supports the consolidation. Three is the lean default; expansions require concrete capability-gap triggers.

---

## 5. Operational notes

### 5.1 Naming conventions

Unchanged in structure from the prior posture. Filenames:

- Single-recipient dispatches: `Plans/DISPATCH-YYYY-MM-DD-<topic>-<agent>.md` where `<agent>` is `cerebro`, `jarvis`, or `prometheus`.
- Multi-recipient parallel dispatches: `Plans/DISPATCH-YYYY-MM-DD-<topic>-parallel.md`.
- Per-agent parallel outputs: `Plans/<base>-<agent>.md`.
- Research artifacts: `Plans/RESEARCH-YYYY-MM-DD-<topic>-<agent>.md` (deep research from Cerebro).

The `-janus.md` and `-claude-3.md` suffixes are retired going forward; existing files with those suffixes remain as historical artifacts.

### 5.2 PM voice anchoring still required for Prometheus

Even though the role is unchanged, the model change means voice anchoring needs to be re-established. PM includes explicit voice anchoring in each new-Prometheus dispatch (reference prior artifact whose tone should match, specify honest-framing requirements, specify cross-reference fidelity requirements). Treat the anchoring as a calibration aid, not a stylistic preference.

### 5.3 PM remains author of PM-NOTEs

This note is itself a PM-NOTE; PM authors directly per long-standing pattern. PM-NOTEs, handoffs, decisions, and dispatches are PM artifacts and not delegated to Prometheus. Prometheus drafts artifacts where PL or PM is the consumer (ADRs, close reports, PHASE-N updates, INDEX sweeps); PM artifacts stay in PM's hands.

### 5.4 Operational postures inherited unchanged

- **Hybrid git posture** per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — unchanged.
- **Backup posture** per `Plans/PM-NOTE-2026-05-07-backup-posture.md` — unchanged.
- **Stash discipline** — unchanged; do not interact during routine work.
- **Negative-list discipline for parallel dispatches** — unchanged; codified during the rename-Sub-step-6 concurrency.
- **Honest measurement-infrastructure disclosure** — unchanged; Cerebro's run-82 NO_SERIAL row sets the pattern, Claude-3-now-Cerebro's microkernel-grep false-positive disclosure today reinforces it.

---

## 6. Forward state

### 6.1 Immediate near-term dispatches under new routing

- **Kernel-bible-alignment audit** (`git grep` for stale names in `kernel/`). Routes to Cerebro under §2.3. Light, read-only, exercises Cerebro's source-reconnaissance carry-forward.
- **`docs/design.md` audit** (small read + decide). Could route to Cerebro (research / decision-shaped) or Prometheus (doc-lane decision). PM lean: Cerebro, since the deliverable is a decision rather than a doc edit.
- **Kernel-side rename** (mechanical AltimitNeo → NeoPlutarch in kernel sources). Routes to Jarvis under §2.1. First Jarvis dispatch; calibration data point.
- **ADR-019 implementation** (SwitchMutex → IrqSafeMutex migration, post-ratification). Routes to Jarvis. If bundled with the kernel-side rename per the PM lean from the prior turn, the single Jarvis dispatch covers both.
- **Phase 5 PLAN-stage research** (ANSPEC-001 reading list per the prior Janus-addendum §4.1). Routes to Cerebro under §2.3 when Phase 5 opens.

### 6.2 Watch items for calibration

Following items will be observed in the first several dispatches under the new roster:

- **Jarvis kernel output quality.** Particularly: `unsafe` justification discipline, `// SAFETY:` comment fidelity, `no_std` constraint adherence, error-propagation hygiene per Bible 15.3.
- **Cerebro research-artifact format under the new lane.** Voice profile, cross-reference fidelity, synthesis structure.
- **New-Prometheus output quality.** Particularly: prose voice match, ADR structure conformance, source-audit accuracy.
- **Small-coding trial outcomes for Prometheus.** Whether the trial scope holds or contracts back to Jarvis.

PM revisits the roster posture if any of: a watch-item failure mode appears severe and persistent; a §4 capability-gap surfaces (e.g., formal-methods specialty); or PL directs a change.

---

## 7. Cross-references

**Supersedes:**
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md` — prior four-agent baseline (Cerebro/Prometheus/Claude-3, with §3 hold-off recommendation against fourth agent).
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` — Janus addition addendum.

Both prior notes stand as historical record of the prior posture. The calibration discipline and roster-economics reasoning in those notes informs this note; the specific roster, routing, and forward markers are superseded.

**Memory record:**
- This roster is also recorded in Claude's persistent memory for the NeoPlutarch project, per PL direction 2026-05-10 (evening). Memory edit text: "NeoPlutarch agent roster (as of 2026-05-10, supersedes handoff §1): Cerebro = Claude (lighter coding + deep internet research); Jarvis = Codex (heavy coding, kernel source); Prometheus = Qwen3.6-27B (documenting, amending, document cleanup, trial for small coding). Janus is retired — no longer in roster." The on-disk PM-NOTE (this file) is the authoritative record; the memory edit is a working-context convenience for fresh PM sessions.

**Authority:**
- Project-lead reconfiguration 2026-05-10 (evening, post-Sub-step-6 close): the three-agent roster with model bindings and primary lanes as enumerated in §1.

**Operational precedent:**
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md` — parallel-dispatch convention (still applies, narrowed to three-agent participation).
- `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — current git posture.
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md` §1 — prior roster snapshot, now superseded by this note for the agent identities.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance — supports model-specialization-for-criticality routing), 4.3 (every dependency a liability — applied here to staffing), 14, 15.3 (no `unwrap` / `expect` in kernel paths — Jarvis calibration item), 18.1.

---

**End of note.**

*Roster: Jarvis (Codex, heavy kernel coding), Cerebro (Claude, light coding + deep research), Prometheus (Qwen3.6-27B, docs + small-coding trial). Janus retired; Claude-3 absorbed into Cerebro. Calibration cycles open on all three under the new configuration: Jarvis fresh, Cerebro partial-transfer, Prometheus fresh-due-to-model-change. Prior two roster notes stand as historical record; this note is operative forward. PM revisits if a watch-item failure mode is severe and persistent or a §4-class capability gap surfaces.*
