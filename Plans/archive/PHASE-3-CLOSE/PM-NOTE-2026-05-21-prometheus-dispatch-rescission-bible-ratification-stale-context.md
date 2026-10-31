# PM-NOTE — Prometheus Dispatch Rescission (Bible Ratification Stale-Context Error)

Date: 2026-05-21
From: PM
Status: RESCINDS `Plans/DISPATCH-2026-05-21-bible-1.1.0-ratification-prometheus.md`

## 1. What Happened

PM issued a dispatch on 2026-05-21 instructing Prometheus to apply a "Bible v1.0.0 → v1.1.0 ratification" of a freshly-drafted "ADR-016: Linux Binary Compatibility via Personality Layer." The work flowed from an inline conversation with PL initiated by the question: *"while waiting on a bare metal I want to ask, I know this is not a Linux or Windows OS, but I dont want to force people to build something just for Plutarch, can we make it so we can easily port Linux apps and games to the OS?"*

PM (this session) responded with a multi-turn design conversation that produced:

1. `/mnt/user-data/outputs/ADR-016-linux-personality-layer.md` (drafted; PL "ratified")
2. `/mnt/user-data/outputs/RATIFICATION-2026-05-21-ADR-016-bible-1.1.0.md` (synthetic ratification record)
3. `Plans/DISPATCH-2026-05-21-bible-1.1.0-ratification-prometheus.md` (dispatch to Prometheus)

PL caught the workflow drift first ("why did I have to download it, you always write it and put it in the correct folder"). PM then read the live repo and discovered the dispatch contained multiple stale-context errors. PM rescinded the dispatch before Prometheus committed. Prometheus halted on PL's instruction with no edits made.

## 2. Stale-Context Errors Identified

This is the same class of error as `PM-NOTE-2026-05-20-prometheus-dispatch-rescission-stale-context-error.md`. PM was working from a context window seeded with handoff-class artifacts that predated extensive project evolution. Specifically:

- **Project name.** PM used "AltimitNeo" throughout the ADR, Bible amendments, and dispatch. The project was renamed to **NeoPlutarch** on 2026-05-10 (Bible Article 25.1; trademark-driven, removes `.hack` franchise derivative-name exposure). The directory on disk retains the pre-rename name `E:\AI Automation\AltimitNeo\`; the *project* does not.
- **Bible version.** PM proposed bumping "Bible v1.0.0 → v1.1.0". The current ratified Bible is **v1.6.0**, last amended 2026-05-11. The correct target for an additive amendment would be v1.6.0 → v1.7.0.
- **ADR number.** PM allocated "ADR-016" for the Linux personality work. **ADR-016 already exists** (`ADR-016-irq-safe-heap-and-scheduler-decoupling.md`, committed). The next free ADR is **ADR-023**.
- **Phase context.** PM framed the work as opening during the "Phase 1.4 / Phase 1.5 transition window." The project is **substantively at end-of-Phase-3** (BUILD-5b bare-metal validation remaining; Phase 4 pre-loaded; Phase 5+ awaiting). The Phase 1.4/1.5 framing is ~9 months out of date relative to project state.
- **File paths in dispatch.** PM cited `docs/adr/ADR-NNN-*.md` for ADR location. Per Bible Article 17 (amended 2026-05-11), ADRs are **flat in `docs/`** as `docs/ADR-NNN-*.md`. No `adr/` subdirectory exists.
- **Article 4.5 single-node clause.** PM's prior research-report content recommended adding a single-node-scope clause; PM's recent context did not register that **Article 4.5 already contains** "NeoPlutarch is single-node by design" (added in a prior amendment). The recommendation is already discharged.
- **Article 6.6 inference-governance clause.** PM's proposed Article 5.6 amendment text composed with assumptions about kernel inference governance that **Article 6.6 already addresses** with a positive framing (kernel exposes hooks; does not implement policy).
- **Article 28 Product Accessibility.** PM did not know about Article 28 at all. Article 28.2 explicitly references "the POSIX compatibility shim (Article 3)" as the mechanism by which "common command-line software with sane build systems can be ported with reasonable engineer effort." Article 28 is the article that *substantively answers* PL's question about not forcing people to rebuild software for NeoPlutarch — and it is already ratified Bible.

## 3. Effect On Other Lanes

No effect. The dispatch did not reach commit state. No code or doc edits were made by Prometheus. The dispatch file itself remains on disk as historical record per the 2026-05-20 precedent (do not delete the rescinded artifact; the rescission is the record).

Other lanes (Jarvis, Cerebro) are unaffected. No active dispatches to those agents are in flight.

## 4. PM Error Classification

This is a PM (Cerebro / research-session) process error, not an agent error. Prometheus had not yet acted on the dispatch when the rescission was issued — the wrong-file resolution from 2026-05-20 did not recur.

Process correction filed for future PM sessions:

> **The incoming PM session's FIRST action, before drafting any artifact that references project state, is to read the live repo's `docs/bible.md` for the current Bible version, list `docs/` for the current ADR inventory, and list `Plans/` for recent PM artifacts. Conversation context is starting context, not ground truth. The live repo is ground truth. This rule was filed on 2026-05-20 after the ANSPEC-002 incident; it was not honored on 2026-05-21 by the same class of failure. This second instance reinforces the rule.**

A specific sub-rule arising from this instance:

> **When PL asks a design question that could result in a new ADR or Bible amendment, PM's first step is to read the current Bible cover-to-cover before drafting any response, regardless of how recent PM's context appears to be. Project artifacts evolve faster than handoff materials reflect.**

## 5. Forward Action

The Linux-compatibility design conversation itself is not invalidated by the stale-context error — the engineering question PL raised ("can we run Linux apps and games without forcing people to rebuild for NeoPlutarch") is genuine and worth answering. What is invalidated is the *shape* of PM's response (ADR + Bible amendment now), which compounded errors on top of a question that may not warrant Bible-amendment-tier work at all.

PM surfaces three options to PL for choosing the actual posture, with PM's recommendation:

### 5.1 Option A — Forward-marker ADR (PM recommendation)

Draft **ADR-023: Linux Binary Compatibility Direction** as a forward-marker ADR in the shape of ADR-022 (`fault-recovery-direction-forward-marker`). The ADR ratifies design intent (personality-layer architecture, tier scoping, fork emulation approach, the explicit exclusion list, graphics-stack DRM-compat constraint, Proton-for-Windows-games-by-transitivity) but does not propose Bible amendments. Specific triggers for revisiting (Phase 8 THINK opens; Phase 6+ accelerator drivers need to commit to DRM-compat interface shape; or contributor proposes Windows or BSD personality) are enumerated.

Bible amendments are deferred. Article 3's current "POSIX compatibility shim" language is sufficient permission for the work; Article 28.2 already commits to the goal. Bible-amendment discipline per Article 27 — and the 2026-05-20 triage-packet lesson that substantively-redundant amendments dilute the precedent — points away from amendment-now.

Pre-Phase-8 hook commitments PM's prior ADR proposed (process struct personality byte, syscall entry dispatch byte, CI personality-stub target) can be folded into the ADR as forward markers without immediate implementation. They cost essentially nothing to honor when their phases arrive; they cost real PM/agent cycles to land prematurely.

This option matches existing project discipline closely. ADR-022 is the direct precedent.

### 5.2 Option B — Full ADR + Bible amendment, drafted correctly this time

Re-draft the work with all stale-context errors corrected: ADR-023, Bible v1.6.0 → v1.7.0, NeoPlutarch naming, Phase 3-end framing, ADRs flat in `docs/`, and explicit reference to existing Articles 4.5/6.6/28 so the new clauses don't duplicate or contradict ratified content.

The Bible amendment in this option would be a new Article 5.6 (personality subsystems as a formal mechanism) plus an Article 3 fourth-bullet edit (clarify "POSIX compatibility shim" language to point at the personality mechanism). Article 28.2's cross-reference to Article 3 would also need touching.

This option is heavier; PM's read is that it over-formalizes work that is many phases away from implementation and is permitted by current Bible language. The 2026-05-20 triage lesson argues against it.

### 5.3 Option C — Shelve until Phase 8 THINK

File the engineering design as a PM-NOTE or research-class artifact (the shape of `docs/research/ALTIMIT-real-world-implementation.md`), do not raise to ADR status now, revisit during Phase 8 THINK when the personality layer becomes near-term work.

This is the lightest-touch option. It preserves the design content without committing project documentation to a decision that's many phases ahead of its implementation window. It may be the *most* disciplined option given how far Phase 8 actually is.

### 5.4 PM recommendation

**Option A (forward-marker ADR-023).** Reasoning:

- Article 28.2 already references POSIX compat shim explicitly; the project has *already committed to the goal* at Bible level.
- Article 3's existing fourth-bullet language permits the work without amendment.
- ADR-022 establishes the forward-marker ADR shape as a working pattern; Linux compat fits it cleanly.
- A forward-marker ADR creates a citable document for the design direction (so contributors and future PM sessions don't re-derive it) without bloating the Bible.
- Pre-Phase-8 hook commitments (process struct byte, syscall entry byte, CI stub) get recorded as forward markers and harvested when their phases arrive.
- The 2026-05-20 triage-packet lesson — substantively-redundant amendments dilute precedent — applies directly here.

Option C is defensible if PL judges that even an ADR is premature; it costs least.

Option B is *only* the right call if PL wants Bible-level commitment to the personality mechanism *now*, before Phase 8 design substrate is in place. PM does not recommend this.

## 6. PM Posture Going Forward

PM accepts accountability for the second instance of the stale-context error class. The 2026-05-20 process correction was real and was not followed on 2026-05-21. PM will read the live Bible as first action in any future session that may produce architectural artifacts, before any drafting.

PM also notes the workflow drift PL caught (writing artifacts to `/mnt/user-data/outputs/` and expecting PL to download-and-relocate, instead of writing directly to `Plans/` and `docs/` via the Filesystem MCP tools). That drift compounded the error: had PM written directly to disk, the directory listing alone would have surfaced the existence of ADR-022, the docs/ flat ADR layout, and likely the v1.6.0 Bible. PM will write directly to the correct paths going forward.

Filed.
