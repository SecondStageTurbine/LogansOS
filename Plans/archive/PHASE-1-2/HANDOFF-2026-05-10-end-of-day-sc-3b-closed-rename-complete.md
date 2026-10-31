# Handoff: End-of-day 2026-05-10 — SC-3B closed, project rename to NeoPlutarch complete, Sub-step 6 in flight

**Date:** 2026-05-10
**Author:** PM
**Purpose:** Comprehensive context handoff to enable a fresh chat session to pick up NeoPlutarch project work without loss of context. New chat opens by checking the in-flight state of Sub-step 6 docs closure dispatch and the slogan addendum, then routing to the next-action queue once those land. Per Bible Article 14, this is operational handoff, not normative — no governance changes embedded.
**Substrate at handoff:** HEAD `965a1e0` (main, matches origin/main on the private `SecondStageTurbine/NeoPlutarch` GitHub remote). Backup: `Z:\AltimitNeo\2026-05-10-121704`. Four preserved stashes unchanged. Prometheus actively executing Sub-step 6 on a `claude/*` worktree session branch.

---

## 0. Snapshot

**Where we are:** SC-3B (Phase 2 BUILD-3 sub-commit 3, Q4 + Q6 + IPC retrofit) CLOSED. Closure commit `e2b3f4f` lands Sub-step 5c (IRQ-safe acquisition for `Channel.state`). Closure gate scored 200/200 PASS at NEO_SMP=4 and 25/25 PASS at NEO_SMP=2. The Hpw failure mechanism (preempt-while-holding-lock + IRQ-off same-CPU waiter = single-CPU self-deadlock) was named after a five-pass investigation arc (Phases A through D plus Janus second-pass research). Project renamed from AltimitNeo to NeoPlutarch on 2026-05-10 via TM-driven decision after Janus pre-flight verified the new name clean. Bible v1.0.5 → v1.1.0 → v1.2.0 → v1.3.0 landed today (Article 9 expansion, Article 28 Product Accessibility, ADR-017 WiFi Phase 7.5, project rename). Hybrid git posture established (GitHub remote at `SecondStageTurbine/NeoPlutarch` for namespace + offsite backup; local-only operational default per push-event policy).

**In flight:** Prometheus is executing `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — six-commit docs closure dispatch covering three ADR proposals (Bible 8.5.4 clarification, SwitchMutex IRQ-off by construction, owner metadata as production diagnostic), ADR-013 amendments, bible v1.4.0 (Article 16.4 bench-surface expansion), Redox DWRR PM-NOTE, PHASE-2.md update + INDEX.md sweep, and full README.md rewrite. After commit 6 lands, the README slogan addendum (`Plans/DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md`) executes as commit 7 (or as a standalone follow-up if push already happened).

**Immediate next action when new chat opens:** Check Prometheus's progress on Sub-step 6 via `git log --oneline -10` and inspection of `Plans/`. If Sub-step 6 is still running, monitor for completion. If complete, check the slogan addendum landed. If both landed and pushed, the queue opens for: (a) ADR-018/019/020 ratification decisions from project lead, (b) kernel-side rename dispatch, (c) Phase 2 BUILD-3 next sub-commit planning.

---

## 1. Context for the new chat session

The new PM session inherits an active project. Key things to know on day one:

**Project:** NeoPlutarch — clean-slate AI-native operating system, Rust monolithic kernel + userland, Limine-bootloaded x86_64 (aarch64 + riscv64 future), `#![no_std]`, no POSIX. Phase 2 BUILD-3 with SC-3B just closed. Bible at `docs/bible.md` v1.3.0 (current); v1.4.0 lands via Sub-step 6 commit 3 when Prometheus reaches it. Project root `E:\AI Automation\AltimitNeo\` (directory name unchanged from pre-rename; only project name changed). Backups `Z:\AltimitNeo\YYYY-MM-DD-HHMMSS\` (also unchanged on disk). GitHub remote at `https://github.com/SecondStageTurbine/NeoPlutarch` (private). Filesystem tools for read/write to user machine.

**Renamed today (2026-05-10):** Project was "AltimitNeo" through Phase 2 BUILD-3 sub-commit 2. TM-driven rename to NeoPlutarch on 2026-05-10 to remove derivative-name exposure to Bandai Namco's `.hack` franchise IP. Janus TM pre-flight at `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` verified NeoPlutarch clean across USPTO, EU IPO, JPO, GitHub orgs, npm/PyPI/crates.io/Go module namespaces, and four primary domains. Verdict: "Proceed with caveats" — the caveats were namespace-strategy decisions baked into the rename, not blockers. Article 25.1 amended in bible v1.3.0; Article 25.2 clarified that `neo-` subsystem prefixes (`libneo`, `.neom`, `NeoFS`, `neo-init`) survive the rename as established convention.

**Bible preamble preserved:** The lineage acknowledgment "descended in spirit from Altimit OS" is preserved as factual referential speech. `ALTIMIT-real-world-implementation.md` at project root is the deep-research artifact about `.hack` canon as design inspiration; preserved unchanged.

**PM identity:** the PL. PM voice is tight, source-grounded, calibrated about uncertainty, defers to project lead on ratifications. PM does not unilaterally amend bible, ADRs, or commits. PM produces dispatches and notes; agents implement.

**Agent roster:**
- **Cerebro** — kernel implementer (Claude Code-class). Has direct kernel source access, can `git grep`, can build, can run kernel tests in QEMU. Calibration high after run-82-disclosure positive signal and worktree-cleanup discipline. Most reliable for kernel work. Currently idle.
- **Prometheus** — docs specialist (Claude Code-class). Faithful to dispatch shape; needs explicit content control because they implement what's specified. Currently active on Sub-step 6 docs closure dispatch.
- **Janus** — deep research (deepseek-v4). Three dispatches in (SC-3B first-pass silent-timeout prior art, SC-3B second-pass release-on-switch prior art, NeoPlutarch TM pre-flight). Calibration firmed from "elevated" through "somewhat-elevated" toward normal trust per `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`. Currently idle.

**Filesystem tools available to PM:** Read/write to anywhere under `E:\AI Automation\AltimitNeo\` (verified). `.git/` is readable for refs inspection; useful for git-state checks without shell access. Do NOT use Filesystem tools to modify files Cerebro/Prometheus may be touching in-flight — coordinate via dispatch handoff instead.

**Bible articles invoked frequently:**
- 1.2 (correctness > coherence > performance)
- 1.3 (every commit is intended to ship)
- 1.4 (no parity-chasing; rightness over parity)
- 4.1 (monolithic kernel, NOT microkernel — load-bearing)
- 4.3 (every dependency is a liability)
- 8.1 (5-class priority scheduler: RT/Inference/Interactive/Compute/Background)
- 8.5 (critical-section discipline; 8.5.4 = no blocking while holding spinlock; SC-3B retroactively named the preempt-while-holding case under this clause)
- 9.5-9.10 (hardware target classes, phase-binding)
- 10.2 (NeoFS content-addressed)
- 12.1 (capability-based security from Phase 2)
- 13 (phase ledger — phases 0-1.5 complete, 2 in progress, 3-7+ ahead, 7.5 WiFi, 8+ userland)
- 14 (OBSERVE→THINK→PLAN→BUILD/VERIFY cadence)
- 15.2 (SAFETY comments)
- 17 (source layout with `boot/firmware/<vendor>/<chipset>/`)
- 23 (no telemetry, no embedded model, no chatbot persona — these are project values, not optional)
- 25.1 (project name is NeoPlutarch) / 25.2 (neo- prefix convention preserved as established convention)
- 26 (MIT OR Apache-2.0; bundled vendor firmware retains own license)
- 27 (amendments by PR with rationale + version bump + PL approval)
- 28 (Product Accessibility — "usable by real people on hardware they own")

---

## 2. This session — chronological summary

Session date 2026-05-10, ~14 hours of activity.

### 2.1 Started post-Sub-step-5b

Inherited state: Sub-step 5b (IPI infrastructure) commit `2d8f65d` was HEAD. SC-3B investigation was at Phase D entry — lock-ownership probe instrumentation dispatch had been drafted; Phase D execution was pending. Stashes preserved: `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment.

### 2.2 SC-3B Phase D execution + Hpw mechanism named

Cerebro executed Phase D lock-ownership probe (SwitchMutex owner metadata with seqlock discipline, multi-probe NMI lock readout, BLOCK_TRACE emit points). At run 43/50, captured the smoking gun: CPU0 in `_mm_pause` with IF=0, channel ch=3 locked across all three NMI probes, owner thread 0 (main) in `Ready` state on CPU0, pair-3 receiver pre_block but no BLOCK_TRACE.

PM named the mechanism: **Hpw — preempt-while-holding-lock + IRQ-off same-CPU waiter, producing single-CPU self-deadlock.** Specifically: the stress orchestrator's `wait_for_stress_endpoint_blocked` called `debug_endpoint_is_recv_blocked` which acquired `Channel.state` via plain `self.state.lock()` with preemption ENABLED. Timer preempted main while holding the lock. Receiver (IPC syscall, IF=0) tried to acquire the same lock; spun forever. Main couldn't run because the IF=0 spinner monopolized the CPU. Single-CPU self-deadlock.

Findings filed at `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`.

### 2.3 Janus second-pass research (parallel to Phase D)

Dispatched in parallel: prior art on release-on-switch lock primitives. Janus found Linux/FreeBSD/xv6/Zircon all have scheduler-special-case lock handoff disciplines; Redox is the closest Rust match via `switch_finish_hook`; Asterinas has a similar pattern. No exact prior art for Hpw's two-lock asm release shape but the broader bug class (ownership-metadata-drift + IRQ-discipline-mismatch) is well-documented.

Filed: `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`.

Janus calibration verdict: somewhat-elevated review intensity (per the spot-check step-down from first-pass). No anomaly observed. Citations consistent with substrate evidence. Calibration profile firming up across three dispatches toward normal trust.

### 2.4 Sub-step 5c implementation

PM drafted `Plans/DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md`. Scope: five `Channel.state` call sites in `kernel/src/ipc/mod.rs` that acquired the lock in preemptible process context. Each wrapped in `without_interrupts(...)`. No SwitchMutex changes. No scheduler changes. Phase D instrumentation reverted.

Cerebro executed Sub-step 5c. Commit `e2b3f4f` lands `build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state`. Pre-commit verification:
- `cargo k` PASS, 18 warnings, baseline delta 0
- `cargo neo-test all` at NEO_SMP=4 PASS (34 serial logs; 33 PASS + 1 deliberate-FAIL meta-test)
- 5 consecutive `smp_cross_cpu_rendezvous_stress` PASS at NEO_SMP=4 (timings 65.1-76.3s)

### 2.5 SC-3B closure gate (200/200 + 25/25)

Closure gate at NEO_SMP=4: **200/200 PASS**. Timings 62.8s min, 510.9s max, 112.9s avg.
Closure gate at NEO_SMP=2: **25/25 PASS**. Timings 78.4-88.3s, avg 82.4s.

**Run 82 measurement-infrastructure event:** Raw `results.csv` for NEO_SMP=4 contains one unscored `run=82 exit_code=137 classification=NO_SERIAL` row. Cause: host command timed out after run 81, leaving an orphan `docker compose run` for run 82. Cerebro stopped the orphan, reran run 82 cleanly (rerun passed and is the scored attempt). Scored gate count is 200 completed PASS boots. Honest distinction between raw CSV row count and scored gate count documented in verification log. Positive Cerebro calibration data point (could have buried the row or rationalized; chose explicit disclosure instead).

Final `cargo k` at HEAD post-rename `cca6889`: PASS, 18 warnings, delta 0.

Verification log: `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`.

### 2.6 Bible amendments (v1.0.5 → v1.1.0 → v1.2.0)

Two same-day amendments folded into a combined v1.2.0 bump:

**v1.1.0:** Articles 9.5–9.10 (architecture coverage, hardware target classes, input/display/storage/networking phase-binding) + new Part VI / Article 28 (Product Accessibility). Filed via `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md`.

**v1.2.0:** Article 9.10 revised (WiFi committed to Phase 7.5, Intel iwlwifi family, WPA2/WPA3 client mode, bundled firmware). Article 13 phase ledger updated to include Phase 7.5. Article 17 source layout updated to include `boot/firmware/<vendor>/<chipset>/`. Article 26 clarified for bundled vendor firmware. Article 28.5 references Phase 7.5.

### 2.7 ADR-017 (WiFi support strategy)

PM drafted `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` and surfaced 5-question decision matrix. Project lead ratified the PM-recommended package:
- D1-A: Intel iwlwifi only (initial chipset family, ~60-70% modern x86 laptop coverage)
- D2-C: Phase 7.5 (preserves AI-native phases 5-7 as project spine)
- D3-A+B+C: All stepping stones (USB Ethernet CDC-ECM/NCM/RNDIS Phase 4, USB tethering, wired-install posture)
- D4-A: Bundled redistributable firmware in `boot/firmware/<vendor>/<chipset>/`, vendor licenses preserved
- D5-A: Article 9.10 amendment only (no separate Article)

Ratified ADR landed at `docs/ADR-017-wifi-support-strategy.md`. Estimated cost: 2-3 engineer-years for iwlwifi alone (this is real bare-metal driver work, not a port).

### 2.8 Janus TM pre-flight + rename ratification

PM raised the AltimitNeo TM concern after recognizing the `.hack` derivative-name exposure pattern. Dispatched Janus pre-flight: search USPTO, EU IPO, JPO public databases for AltimitNeo / NeoPlutarch / two backup names; check GitHub org availability; check package registries; check domain availability.

Janus returned verdict "Proceed with caveats." NeoPlutarch verified clean across all primary sources. Bare "Plutarch" has collisions (Plutonomicon/plutarch-plutus Haskell/Cardano eDSL with 133 stars; arXiv:2601.08930 quantum scheduling paper; Ibrahimsheikh02/PlutarchOS 8-star repo). Decision: use **NeoPlutarch** (full name) everywhere, never bare "Plutarch."

Ferraegis verified as cleaner backup but NeoPlutarch chosen — etymology preserves the canon-thematic Pluto-association via real Greek-language ancestry (Πλούταρχος = "ruler of Pluto-realm / chief of riches") rather than via `.hack` derivation.

Research artifact: `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`.

### 2.9 Three-commit rename dispatch executed

PM drafted `Plans/DISPATCH-2026-05-10-rename-three-commit-prometheus.md`. Three commits:
1. Bible v1.2.0 (Article 9 expansion + Article 28 + WiFi Phase 7.5 commitment) — lands as authored under AltimitNeo name
2. ADR-017 (WiFi support strategy) — lands as authored under AltimitNeo name
3. Project rename — bible v1.3.0 with Article 25.1 amendment, text-replace AltimitNeo→NeoPlutarch across docs/, ADRs, README.md

Negative-list discipline (kernel/, boot/, tools/, Plans/, `ALTIMIT-real-world-implementation.md` all preserved). "Altimit OS" referential mentions preserved as TM-defensible homage.

Prometheus executed in parallel with Cerebro's closure gate (long-running 200-boot run at NEO_SMP=4). The negative-list discipline made parallel execution safe by construction — disjoint file sets, no merge friction. Diff from `e2b3f4f` to `cca6889` touched only `README.md` and `docs/**`. **First proof that Cerebro × Prometheus parallel execution works at scale.** Pattern to internalize.

Commits landed locally; pushed to `SecondStageTurbine/NeoPlutarch` (push state described in §6 below).

### 2.10 SC-3B CLOSE-REPORT filed

PM drafted `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` (~370 lines). Canonical historical record covering: investigation arc (Phases A→B→C→D→Janus-second-pass), hypothesis tracking final state (Hd/Hr/Hc refuted, Hs adjacent, **Hpw CONFIRMED at Phase D run 43/50**), the fix (five wrapped call sites), verification gate results, Janus calibration data, parallel-execution coordination outcome, carry-forwards for Sub-step 6, cost/timing summary.

### 2.11 Redox DWRR PM-NOTE triage

Project lead surfaced the Redox OS DWRR scheduler announcement (April 2026, RSoC 2026 by Akshit Gaur). PM triaged via web-search (Redox blog 403'd direct fetch; content captured via Phoronix, OSnews, Madshrimps, Wikipedia DRR/DWRR article). Triage filed at `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md`. Carry-forward declared: Article 16.4 bench-surface expansion to include scheduling fairness with Redox `schedrs` as canonical reference; ADR citation source for future Phase 2 (or Phase 7) within-class scheduler algorithm choice.

### 2.12 Hybrid git posture established (after PM continuity error owned)

Project lead questioned the GitHub push: "I asked about doing this earlier and you pushed back saying to stay locally on drive, I think you mentioned the bible said not to push to git." PM read `Plans/PM-NOTE-2026-05-07-local-only-posture.md` and acknowledged:

- The bible never said anything about local-only. That was a PM-NOTE, not a bible amendment.
- The 2026-05-07 local-only posture was real and documented. PM failed to surface it when project lead's GitHub-create instruction came in this session. PM continuity error: posture documented in `Plans/` but not actively in context can be missed when current instructions implicitly override it.

Project lead chose hybrid posture. PM filed `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` codifying:
- GitHub remote exists at `SecondStageTurbine/NeoPlutarch` (private) for namespace reservation + offsite backup at deliberate intervals
- Default dispatch posture remains local-only; push steps are explicit per-dispatch when included
- Push events: phase closures, major sub-commit closures, project-governance landings, explicit PL direction
- 2026-05-07 local-only note marked superseded (in the narrow sense that a remote now exists; operational discipline of "commits land locally by default" carries forward)

PM committed to targeted-search discipline going forward: before drafting a dispatch that touches an operational concern (git remote, backup, CI, deployment), search Plans/ for prior posture PM-NOTEs on that concern. Calibration data captured in §11 below.

### 2.13 Sub-step 6 docs closure dispatch drafted and forwarded

PM drafted `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` (~1100 lines). Six commits:

1. Three ADR proposals filed in Plans/ (Bible 8.5.4 clarification → tentative ADR-018; SwitchMutex IRQ-off by construction → tentative ADR-019; Owner metadata as production diagnostic → tentative ADR-020). Each ~150-200 lines, self-contained, with rationale, decision context, open questions for ratification.
2. ADR-013 §Q4/§Q6/§Q7 amendments (SC-3B closure markers + structural lessons).
3. Bible v1.4.0 (Article 16.4 bench-surface expansion citing Redox `schedrs`).
4. Redox DWRR PM-NOTE commit (file already on disk; verify + add + commit).
5. PHASE-2.md update + INDEX.md sweep for 2026-05-09/10 backlog.
6. README.md full rewrite to align with bible v1.4.0 — fixes microkernel→monolithic, ALTFS→NeoFS, `.neo-model`→`.neom`, stale phase roadmap, cloud-routed inference removal, `docs/design.md`→`docs/bible.md` reference correction, less on-the-nose `.hack` framing.

After commit 6 + push.

### 2.14 README slogan addendum dispatch filed

Project lead disliked the legacy "The world runs on NEO." slogan and asked for replacement. Sub-step 6 commit 6 removed it but didn't substitute. PM picked **"Intelligence as a kernel primitive."** — distillation of Bible Article 1.1's opening question. Filed addendum at `Plans/DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md` to add the slogan as a blockquote line under the README H1, executable as Sub-step 6 commit 7 (riding along with §9 push) or as standalone follow-up if push already happened.

### 2.15 Worktree cleanup (Cerebro)

Five Claude Code worktree session branches existed pre-cleanup. Four at `cca6889` (the rename HEAD), one at `0a547b2` (a CI Docker permission fix that was actually an ancestor of main once main was advanced). Cerebro removed all five worktrees and four branches; `wonderful-allen-6ea696` initially preserved pending investigation, then deleted after analysis showed it was a clean ancestor. Final state: single-branch, single-worktree, repo at `965a1e0`.

PM caught a misreading mid-cleanup (treated `cca6889` as latest and `0a547b2` as divergent, when actually `0a547b2` was main's immediate parent — already merged). Calibration data captured in §11 below.

---

## 3. Ratified decisions this session

These are the active normative outputs of the session. New chat treats as authoritative.

### 3.1 SC-3B CLOSED

Phase 2 BUILD-3 sub-commit 3 (Q4 + Q6 + IPC retrofit + cross-CPU scenarios) closed via commit `e2b3f4f` on 2026-05-10. Hpw mechanism named, structurally foreclosed for affected `Channel.state` call sites. Verification: 200/200 NEO_SMP=4, 25/25 NEO_SMP=2, `cargo k` PASS delta 0. Closure report: `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`.

### 3.2 Project rename to NeoPlutarch

TM-driven rename ratified after Janus pre-flight returned "Proceed with caveats." Bible v1.3.0 amended Article 25.1 (project name) and clarified Article 25.2 (neo- subsystem prefixes preserved as established convention). Three-commit rename dispatch executed; README.md is currently stale (predates current architectural state) — Sub-step 6 commit 6 rewrites it.

### 3.3 ADR-017 (WiFi support strategy)

Accepted 2026-05-10 with the PM-recommended D1-A / D2-C / D3-A+B+C / D4-A / D5-A package. WiFi committed to Phase 7.5; Intel iwlwifi family as initial chipset; USB Ethernet stepping stones in Phase 4; bundled redistributable firmware in `boot/firmware/`; Article 9.10 amended concurrently in bible v1.2.0.

### 3.4 Bible v1.0.5 → v1.3.0 (current); v1.4.0 lands via Sub-step 6 commit 3

Current state at HEAD `965a1e0`: bible v1.3.0. After Sub-step 6 commit 3 lands and pushes: bible v1.4.0 (Article 16.4 bench-surface expanded for scheduling fairness with Redox `schedrs` as canonical reference).

### 3.5 Hybrid git posture

GitHub remote at `SecondStageTurbine/NeoPlutarch` (private) for namespace reservation + offsite backup at deliberate intervals. Default dispatch posture is local-only; push steps are explicit per-dispatch when included. Push events: phase closures, major sub-commit closures, project-governance landings, explicit PL direction. Routine commits accrue locally; ride along with next push event.

Codified at `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`. Supersedes `Plans/PM-NOTE-2026-05-07-local-only-posture.md` (the 2026-05-07 file itself preserved unchanged as historical record; its frontmatter not modified — superseded status recorded only in the 2026-05-10 file's frontmatter).

### 3.6 README slogan replacement

"Intelligence as a kernel primitive." — picked from three candidates the bible's own language suggested. Lands via slogan addendum dispatch after Sub-step 6 commit 6 lands.

### 3.7 NeoFS preserved as content-addressed (not ZFS-inspired)

Article 10.2 specifies NeoFS as content-addressed. Project lead's direction this session confirmed: NeoFS is NOT ZFS-inspired despite superficial naming similarity to ZFS-flavored filesystems. Pure content-addressed semantics.

---

## 4. Forward queue (immediate next actions for new chat)

In execution order:

### 4.1 Monitor Sub-step 6 + slogan addendum completion (immediate)

Prometheus is executing Sub-step 6 docs closure dispatch (~2.5 hours total) and will then run the slogan addendum (~5 minutes). New chat session's first action: check current state.

```bash
git log --oneline -15                              # see how many of the 6 Sub-step 6 commits + slogan have landed
git rev-parse HEAD                                 # current local HEAD
cat .git/refs/remotes/origin/main                  # remote HEAD (read via Filesystem if no shell)
ls -la Plans/ADR-PROPOSAL-2026-05-10-*.md          # expect 3 ADR proposals
ls -la Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md   # already on disk
test -f README.md && grep -c "monolithic" README.md       # expect 1+ post-rewrite (currently 0 if pre-rewrite)
tail -10 docs/bible.md                             # check version footer; expect v1.4.0 post-Sub-step-6 commit 3
```

If Sub-step 6 + addendum landed and pushed: queue opens to §4.2.
If still in flight: wait, monitor, optionally surface progress observations to PL.
If failed: read `Plans/PROMETHEUS-FAILURE-2026-05-10-*.md` (created on failure per dispatch protocol) and triage.

### 4.2 ADR ratifications (project lead decision pending)

Once Sub-step 6 lands, three ADR proposals sit in `Plans/` awaiting PL ratification:

- **ADR-018 proposal** (`Plans/ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md`) — Bible Article 8.5.4 clarification explicitly naming the preempt-while-holding case.
- **ADR-019 proposal** (`Plans/ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md`) — type-system enforcement of IRQ-safe lock discipline. Companion to ADR-018.
- **ADR-020 proposal** (`Plans/ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md`) — lock owner metadata as production-stable diagnostic affordance.

Each has "Open questions for ratification" section with specific decisions. PL ratifies each (with or without edits, or rejects). On ratification: actual ADR files land in `docs/`, ADR-011 supersession marker added (post-ADR-018 ratification), bible Article 8.5.4 amendment lands (post-ADR-018 ratification, bumps to v1.5.0).

PM lean: ADR-018 + ADR-019 ratify together (conceptually linked — declarative discipline + structural enforcement). ADR-020 can ratify independently.

### 4.3 Kernel-side rename dispatch

Mechanical AltimitNeo → NeoPlutarch in:
- `kernel/Cargo.toml` package name
- `kernel/src/**` source comments referencing AltimitNeo
- `boot/limine.cfg` boot title (if applicable)
- `tools/xtask/**` references

**Two sequencing options:**

- **Minimal-now (post-Sub-step-6):** Small Cerebro dispatch immediately after Sub-step 6 lands. ~30 minutes execution. Project name on disk matches docs.
- **Bundle with ADR-019 implementation (PM lean):** Hold kernel-side rename until ADR-019 ratifies; combine the AltimitNeo→NeoPlutarch text rename with the SwitchMutex→IrqSafeMutex migration in one Cerebro dispatch. Saves a round-trip; cleaner commit history.

PL decision when ADR-019 ratification surfaces.

### 4.4 Kernel-bible-alignment audit (potential)

Cerebro dispatch (medium scope) to find ALTFS / `.neo-model` / `neo-inference` / "microkernel" references in `kernel/` code or test code and triage against bible v1.3.0+. Surface as separate audit if it turns out any of these names are baked into kernel source (e.g., test names, comments, internal module names).

PM doesn't know the answer to this without an audit — current evidence is from the README which I drafted to remove these references in docs. Kernel-side may or may not have them. Cerebro could `git grep` for each pattern in a few minutes.

PM lean: file as carry-forward after Sub-step 6 closes; combine with kernel-side rename if audit surfaces additional fix scope.

### 4.5 `docs/design.md` audit (potential)

`docs/design.md` was referenced as "The architecture spec. Read this first." in the pre-rewrite README. Bible Article 27 makes `docs/bible.md` canonical. If `docs/design.md` exists and predates the bible, it's competing-authoritative-document territory — either update to match bible, delete as stale-cruft, or preserve as historical with explicit "superseded by docs/bible.md" header.

Small audit; ~10 minutes Prometheus or PM read + decide.

### 4.6 Phase 2 BUILD-3 next sub-commit

What's after SC-3B in BUILD-3? Per `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §10: BUILD-3 has Q7 (TLB management) and Q8 (capability table SMP composition) remaining. Specifically:

- BUILD-3 sub-commit 4: Q7.b/c — CR3 cache + IPI TLB shootdowns
- BUILD-3 sub-commit 5: Q8.a/b — capability table SMP composition
- BUILD-3 sub-commit 6: Q8.c — blocked-thread-kill walk

After Q7 + Q8 land, Phase 2 BUILD-3 closes. Then BUILD-4 (ADR-016 A2 scheduler heap-decoupling) and Phase 2 closure.

PM doesn't have detailed plans for any of these yet. THINK-stage work needed when SC-3B closes fully and ADR ratifications complete.

### 4.7 Domain registration (project lead task)

Janus pre-flight noted NeoPlutarch clean across `.org`, `.dev`, `.io`, `.com` domains (all NXDOMAIN at pre-flight time). Pre-announcement reservation recommended now that the project name is visible in git history. ~$10-15/year. PL action, not Prometheus.

PM lean: `neoplutarch.org` matches the project's non-commercial, foundation-style positioning. `.dev` or `.io` are reasonable alternates.

---

## 5. Active artifacts (this session's outputs)

All in `E:\AI Automation\AltimitNeo\Plans\` unless noted. Working-tree artifacts; commits per §6 git state.

**Investigation arc closure:**
- `SC-3B-PHASE-D-FINDINGS-2026-05-10.md` — Phase D capture; Hpw smoking-gun evidence.
- `RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus second-pass.
- `DISPATCH-2026-05-10-sc-3b-phase-d-lock-ownership-cerebro.md` — Phase D dispatch.
- `DISPATCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus second-pass dispatch.

**Sub-step 5c (closure commit):**
- `DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md`
- `SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — Cerebro's verification log; canonical gate record.

**SC-3B closure:**
- `CLOSE-REPORT-2026-05-10-sc-3b.md` — canonical historical record.

**Bible amendments + ADR-017:**
- `AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md` — v1.1.0 rationale.
- `ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` — ADR-017 rationale; ratified.

**Project rename:**
- `DISPATCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`
- `RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` — Janus TM pre-flight.
- `DISPATCH-2026-05-10-rename-three-commit-prometheus.md` — three-commit rename dispatch.

**Hybrid posture:**
- `PM-NOTE-2026-05-10-hybrid-git-posture.md` — codifies hybrid; supersedes 2026-05-07 local-only.

**Sub-step 6 (in flight):**
- `DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — six-commit docs closure dispatch.
- `PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` — Redox DWRR triage; carry-forward source.
- `DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md` — slogan addendum.

**This handoff:**
- `HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md`

**ADR proposals (to be filed by Sub-step 6 commit 1; check status):**
- `ADR-PROPOSAL-2026-05-10-bible-8-5-4-preempt-while-holding-clarification.md` (post-commit-1)
- `ADR-PROPOSAL-2026-05-10-switchmutex-irq-off-by-construction.md` (post-commit-1)
- `ADR-PROPOSAL-2026-05-10-owner-metadata-production-diagnostic.md` (post-commit-1)

**Source artifacts referenced (read-only):**
- `target/verify/sc3b-sub-step-5c-gate/neo-smp4/results.csv` — 200-boot raw CSV (one NO_SERIAL row at run 82; see §2.5)
- `target/verify/sc3b-sub-step-5c-gate/neo-smp2/results.csv` — 25-boot raw CSV
- Boot serial logs across both gates.

**Sub-step 6 dispatch will create (post-execution; commit 5 sweep):**
- `docs/phases/PHASE-2.md` modifications
- `Plans/INDEX.md` sweep (current INDEX.md state may be substantially behind 2026-05-09/10 backlog)

---

## 6. Git state at handoff

**Local HEAD on main:** `965a1e0` — `ci: NEO_SMP=4 for SMP integration tests` (most recent commit).

**Recent commit chain (newest first):**

```
965a1e0  ci: NEO_SMP=4 for SMP integration tests
0a547b2  ci: fix Docker container permission denied on /workspace
cca6889  docs: project rename AltimitNeo → NeoPlutarch (bible v1.3.0)
57439a6  docs(adr): ADR-017 — WiFi support strategy and phase-binding
04f4a95  docs(bible): v1.2.0 — Article 9 expansion + Article 28 + WiFi Phase 7.5 commitment
e2b3f4f  build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state  ← SC-3B closure
2d8f65d  build-3 sub-step 5b: add reschedule IPI infrastructure
[earlier history]
```

**Remote state:** `origin/main` at `965a1e0`, matches local. First push event executed during rename dispatch; subsequent CI fixes (`0a547b2`, `965a1e0`) also pushed.

**Working tree:** clean of tracked changes (Prometheus is working in a `claude/*` worktree session branch; main worktree at root is undisturbed). Untracked `Plans/` artifacts from this session are present per §5.

**Sub-step 6 in-flight notes:**

Prometheus's session creates a new `claude/*` worktree under `.claude/worktrees/<session-name>/` and commits there. When new chat opens, expect to find a new session branch under `refs/heads/claude/*`. After Sub-step 6 completes:
- The session branch advances to include the six (or seven, with addendum) new commits.
- Local main does NOT auto-advance; PL fast-forwards manually (per the pattern established this session).
- Push to origin/main happens after fast-forward.

To check Sub-step 6 progress without running shell commands: read `.git/refs/heads/claude/<session-name>` via Filesystem tool to get the session branch HEAD, then check `Plans/` for the expected new files (3 ADR proposals + ADR-013 amendments + bible v1.4.0 + INDEX.md sweep + README rewrite).

---

## 7. Stash state

Four preserved stashes (unchanged across all commits this session):

```
stash@{0}: On main: sc-3b-fix-6-1-attempt
stash@{1}: On main: wedge-investigation-8-nmi-watchdog
stash@{2}: On main: wedge-investigation-2-instrumentation
stash@{3}: On main: h1-experiment-instrumentation
```

`stash@{0}` is the prior-session Fix 6.1 attempt (per-pair direction alternation in stress test). Fix 6.1 turned out to mask but not resolve the underlying Hpw mechanism that SC-3B Phases A-D investigated; Sub-step 5c is the structural fix. `stash@{0}` is preserved as historical record.

`stash@{1}` is the NMI watchdog instrumentation; useful prior art for ADR-020 (owner metadata as production diagnostic) which proposes ship-stable equivalent of Phase D's owner-metadata instrumentation.

`stash@{2}` and `stash@{3}` are wedge-era; not currently candidates for new work but preserved for historical reference.

Do not interact with stashes during routine work. Stashes are reserved for explicit investigation phases.

---

## 8. Conventions reaffirmed and updated

**Inherited from prior sessions (still in force):**

- **Bible 1.3:** every commit is intended to ship. No scaffolding-only commits.
- **Bible 4.3:** every external dependency is a liability. Kernel-owned types preferred where stability matters.
- **Bible 8.5.4:** no blocking while holding a spinlock. Q6's asm-release-on-new-stack is structural conformance. **SC-3B retroactively named the preempt-while-holding case under this clause; ADR-018 proposal clarifies the discipline explicitly.**
- **Bible 14:** OBSERVE → THINK → PLAN → BUILD/VERIFY cadence. /simplify gate before BUILD.
- **No amend without explicit request.** Standing rule.
- **No stash interactions during regular work.** Stashes reserved for investigation phases.
- **Backups after every commit:** `Plans\backup-to-synology.ps1` handles preservation including all stashes.
- **Multi-recipient dispatches use `-parallel.md` suffix.**
- **Per-agent output filenames** for parallel work: `{base}-{agent}.md` pattern.
- **Independence in parallel work:** agents don't coordinate before submitting. PM synthesizes after all return.
- **PASS frame fields enumerated explicitly** when introducing new scenarios.
- **Stop conditions honored** when fired.
- **Statistical framing for closure gates:** 200/200 is the standard.

**Established or refined this session:**

- **Hybrid git posture:** local-only by default; pushes are explicit per-dispatch at meaningful events (phase closures, major sub-commit closures, project-governance landings, explicit PL direction). Codified at `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`.
- **Negative-list discipline for parallel dispatches:** Cerebro × Prometheus parallel execution made safe by explicit negative-lists in dispatches (kernel/boot/tools off-limits to docs dispatches; docs/Plans/README off-limits to kernel dispatches). First proof at scale during SC-3B closure + rename concurrency.
- **README dispatch scope must anticipate stale content:** the rename dispatch said "if README exists: text-replace" which was too narrow. Existing READMEs (or other long-lived docs) may need content rewrite, not just renaming. PM error owned in §11.
- **Targeted Plans/ search before dispatches touching operational concerns:** PM commits to searching `Plans/` for prior posture PM-NOTEs before drafting dispatches that touch git remote, backup, CI, deployment, or other operational concerns documented in prior sessions. Codified at `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` §calibration-note.
- **Honest measurement-infrastructure disclosure:** Cerebro's run-82 NO_SERIAL row handling sets the pattern. Distinguish raw CSV row count from scored gate count; explain measurement-infrastructure events explicitly in verification logs; never bury or rationalize anomalous rows.
- **Calibration recurrence trigger for agents:** spot-check first-pass output for a new agent; firm up trust over 2-3 dispatches; reach normal review intensity unless an anomaly surfaces. Janus's calibration profile firmed up this session per this pattern.

---

## 9. PM tone and operational notes

- **Voice:** tight, source-grounded, calibrated about uncertainty. Honest about agent calibration without pejorative framing. Defers to PL on ratifications.
- **Length:** dispatches are substantial (300-1100 lines). PM-NOTEs are tight (100-400 lines). Brief responses (30-80 lines) for status updates and forward-queue confirmations.
- **No emoji.** Markdown headings and tables for structure.
- **Cross-references at end of every artifact.** File paths are absolute or project-rooted.
- **Calibration data is honest.** Agent calibration surfaced without pejorative framing.
- **Decisions surface explicitly to PL.** PM proposes; PL ratifies. Multi-option decision surfacing (a/b/c options) is the default for non-trivial choices.
- **Filename conventions:** `DISPATCH-YYYY-MM-DD-{topic}-{agent}.md`; `PM-NOTE-YYYY-MM-DD-{topic}.md`; `RESEARCH-YYYY-MM-DD-{topic}-{agent}.md`; `HANDOFF-YYYY-MM-DD-{state}.md`; `CLOSE-REPORT-YYYY-MM-DD-{sub-commit}.md`.
- **Lowercase-casual in conversation** with PL; formal in written artifacts.
- **Willing to push back** when PM thinks a direction is wrong; honest about being uncertain; admits errors directly.

---

## 10. Open items / future-phase forward state

- **Sub-step 6 close** (in flight) — Prometheus executing.
- **README slogan addendum** (queued) — runs after Sub-step 6 commit 6.
- **ADR-018/019/020 ratification decisions** (project lead, post-Sub-step-6) — three proposals filed; PL ratifies each.
- **Kernel-side rename** (queued post-ratification or post-Sub-step-6 depending on bundle decision).
- **Kernel-bible-alignment audit** (potential follow-up).
- **`docs/design.md` audit** (small, potential follow-up).
- **BUILD-3 sub-commit 4 (Q7.b/c — CR3 cache + IPI TLB shootdowns)** — Phase 2 work after SC-3B closes fully.
- **BUILD-3 sub-commit 5 (Q8.a/b — capability table SMP composition)** — after Q7.
- **BUILD-3 sub-commit 6 (Q8.c — blocked-thread-kill walk)** — after Q8.a/b.
- **BUILD-4 (ADR-016 A2 — scheduler heap-decoupling)** — Bible 8.5.3 conformance milestone. Not gated by SMP retrofit.
- **Phase 2 close** — pending all BUILD-N sub-commits.
- **Phase 3** — NeoFS (content-addressed; NOT ZFS-inspired per project-lead clarification).
- **Phase 4** — Network stack, IPv6, sockets-equivalent. Includes USB stack landing and USB Ethernet adapter support per ADR-017 §D3.
- **Phase 5-7** — AI-native phases (tensor primitive, accelerators, inference scheduler) per ANSPEC-001.
- **Phase 7.5** — WiFi (Intel iwlwifi family, WPA2/WPA3 client) per ADR-017.
- **Phase 8+** — Userland.
- **Domain registration** (PL task) — `neoplutarch.org` recommended.

---

## 11. Calibration lessons from this session

Three calibration errors owned during this session. Documented here so new-Claude doesn't repeat them.

### 11.1 Git-state-from-refs misreading

PM read `.git/refs/heads/main` and `.git/refs/remotes/origin/main` and `.git/refs/heads/claude/*` via Filesystem tools to assess push state. Found SHAs but didn't immediately verify topological relationships before constructing a narrative.

Initial narrative: "origin/main is `965a1e0`, local main is `965a1e0`, the four `claude/*` branches at `cca6889` are local-only and ahead of main." This was wrong — `cca6889` was actually an ancestor of `965a1e0`, and the session branches pointed to a past state, not a future one. The user's git log (run from terminal) showed the actual ordering.

**Lesson:** when reading git refs directly without `git log` output, SHAs alone don't tell topological relationships. Default to "I don't know the relationship without checking ancestry" rather than constructing narrative.

When new-Claude reads git refs via Filesystem, use one of these to verify ancestry:
- Ask PL to run `git log --oneline -10` and paste output
- Read `.git/logs/HEAD` (reflog) for sequence-of-commits-on-HEAD-over-time signal
- Read `.git/packed-refs` if present for combined refs view
- Defer to PL's terminal output when available

### 11.2 PM continuity error on local-only posture

When PL instructed "We'll have Prometheus create the git for us under github.com/SecondStageTurbine," PM silently incorporated the instruction into the rename dispatch without surfacing the 2026-05-07 local-only posture from `Plans/PM-NOTE-2026-05-07-local-only-posture.md`. This was a real PM error — the local-only posture was documented, PM had access to read it, and the new instruction implicitly overrode it. PM should have flagged the posture shift and asked for explicit override.

**Lesson:** before drafting a dispatch that touches an operational concern (git remote, backup, CI, deployment, ANY area where the project might have documented prior posture), do a targeted Plans/ search. Suggested searches:
- For git-related: `grep -li 'remote\|push\|posture' Plans/PM-NOTE*.md Plans/PM-DECISION*.md`
- For backup-related: `grep -li 'backup\|synology' Plans/PM-NOTE*.md`
- For CI-related: `grep -li 'ci\|github.actions' Plans/PM-NOTE*.md`
- For any operational concern: search by likely keyword in Plans/PM-NOTE/PM-DECISION artifacts

Surface found postures to PL in the drafting turn. Ask for explicit confirmation if current instruction overrides prior posture.

The current operational postures the project has documented (as of this handoff):
- **Git posture:** hybrid per `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md`
- **Backup posture:** Synology snapshots after each commit per `Plans/PM-NOTE-2026-05-07-backup-posture.md` (closed question; do not re-raise)
- **CI posture:** `.github/workflows/ci.yml` exists; CI status unstated; not assumed operational
- **Stash discipline:** preserved; do not interact during routine work

### 11.3 README dispatch design too narrow

The rename three-commit dispatch told Prometheus "if README exists: text-replace per §4.3.1 rules; if absent: create minimal one." This was too narrow — the existing README clearly predated the post-Phase-1.5 bible state (referenced microkernel, ALTFS, `.neo-model`, neo-inference, stale phase roadmap, cloud-routed inference, docs/design.md as canonical). The dispatch should have anticipated this and instructed full rewrite. Prometheus followed the dispatch faithfully; the dispatch design was the problem.

**Lesson:** for long-lived high-visibility documents (README, top-level CONTRIBUTING, etc.), default to "verify current content matches current architectural state; rewrite if stale" rather than "text-replace if exists." Especially when the project has evolved substantially since the document was written.

Sub-step 6 commit 6 fixes the README via full replacement. The fix includes a self-documenting commit message that lists every inconsistency the rewrite resolves — honest history.

---

## 12. Cross-references

**This handoff:**
- `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md`

**Substrate (read-order for new chat):**
1. `docs/bible.md` — foundational charter. v1.3.0 at handoff; v1.4.0 post-Sub-step-6 commit 3.
2. `Plans/PM-NOTE-2026-05-10-hybrid-git-posture.md` — current operational posture for git.
3. `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md` — SC-3B closure record.
4. `Plans/DISPATCH-2026-05-10-sub-step-6-docs-closure-prometheus.md` — in-flight dispatch.
5. `Plans/DISPATCH-2026-05-10-readme-slogan-addendum-prometheus.md` — queued addendum.
6. This handoff.

**Other key artifacts (read-as-needed):**
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — Janus calibration baseline.
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` — Janus calibration framework.
- `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` — scheduler prior-art carry-forward.
- `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` — TM pre-flight.
- `ALTIMIT-real-world-implementation.md` — design-inspiration research artifact (preserve unchanged).
- `docs/ADR-013-phase-2-smp-retrofit.md` — Phase 2 SMP retrofit plan; SC-3B closure markers added by Sub-step 6 commit 2.
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` — IRQ-safe heap allocator.
- `docs/ADR-017-wifi-support-strategy.md` — WiFi Phase 7.5.
- `docs/specs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) — AI subsystem spec.
- `docs/phases/PHASE-2.md` — Phase 2 plan; SC-3B sub-step 5c closure marker added by Sub-step 6 commit 5.
- `Plans/INDEX.md` — master Plans/ index; substantially behind 2026-05-09/10 backlog; Sub-step 6 commit 5 sweeps.

**Per-agent dispatches and outputs (SC-3B arc):**

Investigation:
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`

Phase-specific:
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`
- `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md`
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`

Janus research:
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`

Verification:
- `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md`
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md`

**Key source files (kernel-side, read-only from PM perspective):**
- `kernel/src/arch/x86_64/context.rs` — context_switch_with_release2 asm.
- `kernel/src/sched/scheduler.rs` — block_current_into_with_guard, wake helpers.
- `kernel/src/ipc/mod.rs` — five Sub-step-5c-wrapped Channel.state acquisition sites.
- `kernel/src/sync/switch_mutex.rs` — SwitchMutex.
- `kernel/src/test_harness.rs` — stress test scenario.

---

## 13. Opening message for the new chat

Suggested PL opening to the new chat session:

> "Continuing NeoPlutarch project. Read `Plans/HANDOFF-2026-05-10-end-of-day-sc-3b-closed-rename-complete.md` for full context. Sub-step 6 docs closure (Prometheus) and README slogan addendum may still be in flight or just landed. Check git log and Plans/ for current state. Once Sub-step 6 + addendum land and push, queue opens for ADR-018/019/020 ratification decisions, kernel-side rename, and Phase 2 BUILD-3 next sub-commit planning."

The new PM session reads the handoff (§1 first for orientation, §4 for forward queue, §11 for calibration warnings), checks git/disk state, and picks up at §4.1.

---

**End of handoff.**

*Session ends with SC-3B CLOSED via commit e2b3f4f (200/200 NEO_SMP=4, 25/25 NEO_SMP=2), Hpw mechanism named, project renamed AltimitNeo → NeoPlutarch via bible v1.3.0, ADR-017 WiFi Phase 7.5 accepted, hybrid git posture established with GitHub remote at SecondStageTurbine/NeoPlutarch (private), three ADR proposals filed for SC-3B carry-forwards (Bible 8.5.4 clarification, SwitchMutex IRQ-off by construction, owner metadata as production diagnostic), bible v1.4.0 (Article 16.4 bench-surface expansion) staged in Sub-step 6 dispatch, README rewrite + slogan addendum staged in Sub-step 6 commits 6+7. Forward queue clear. New chat picks up at §4.1 (check Sub-step 6 + addendum status).*
