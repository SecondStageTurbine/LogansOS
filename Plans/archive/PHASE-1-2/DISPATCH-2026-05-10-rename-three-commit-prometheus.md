# DISPATCH: Three-commit rename sequence (AltimitNeo → NeoPlutarch) + first push to GitHub remote

**Date:** 2026-05-10
**Routed to:** Prometheus (docs specialist)
**Type:** Multi-phase dispatch — git remote creation + three-commit local sequence + push
**Triggering decisions:**
- Bible v1.2.0 amendment ratified by project lead (working copy edit complete; never git-committed)
- ADR-017 (WiFi support strategy) accepted by project lead (working copy file complete; never git-committed)
- Project rename to NeoPlutarch ratified by project lead after Janus TM pre-flight returned "Proceed with caveats"
- GitHub remote location decided: `github.com/SecondStageTurbine/NeoPlutarch` (private)
**Calibration profile:** Normal trust (Prometheus is the docs specialist; routing established).
**Coordination constraint:** Cerebro is mid-flight on Sub-step 5c (IRQ-safe Channel.state lock discipline). Kernel files MUST NOT be touched by this dispatch. Kernel-side rename is a tracked carry-forward to be executed AFTER SC-3B closes.

---

## 1. Background

Three pieces of project-governance work have accrued in the working copy without being git-committed:

1. **Bible v1.1.0** (2026-05-10 morning) — Articles 9.5–9.10 + Part VI / Article 28 (Product Accessibility). Filed via `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md`.
2. **Bible v1.2.0** (2026-05-10 midday) — Article 9.10 revised, Article 13 phase ledger updated for Phase 7.5, Article 17 source layout updated for `boot/firmware/`, Article 26 clarified, Article 28.5 updated. ADR-017 landed concurrently as `docs/ADR-017-wifi-support-strategy.md`. Filed via `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md`.
3. **Project rename to NeoPlutarch** (2026-05-10 evening) — TM-driven rename to escape `.hack` franchise IP exposure on "AltimitNeo" name. Verified clean by Janus pre-flight at `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` (verdict: "Proceed with caveats"; the caveats are namespace-strategy decisions baked into this dispatch, not blockers).

These constitute three distinct logical commits. They land as a sequence under the new GitHub remote, with the rename as the third commit so commits 1 and 2 capture the governance work as it was authored under the AltimitNeo name (honest history).

---

## 2. Prerequisites and pre-flight

### 2.1 Git state expected at start

- HEAD: `2d8f65da77b4ea10f3d53226724e9381961c48c0` (Sub-step 5b — IPI infrastructure landed)
- Stashes preserved: `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment — DO NOT touch
- Working tree changes (uncommitted) include:
  - Files this dispatch will commit: `docs/bible.md`, `docs/ADR-017-wifi-support-strategy.md`, plus other `docs/` files for the rename
  - Files Cerebro may have changes on (Sub-step 5c WIP): `kernel/src/sched/ipc.rs`, `kernel/src/sched/mod.rs`, `kernel/tests/*` — DO NOT TOUCH

Verify state before proceeding:

```bash
cd /e/AI\ Automation/AltimitNeo
git rev-parse HEAD                                    # expect 2d8f65d...
git stash list                                        # expect 4 stashes
git status --porcelain | head -40                     # expect uncommitted changes; survey them
```

If `git status` shows ANY changes to files under `kernel/`, `boot/`, or `tools/`, those are Cerebro's WIP and must be excluded from every commit in this dispatch (use selective `git add` with explicit file paths; do NOT use `git add -A` or `git add .` at any point).

### 2.2 Coordination with Cerebro

Cerebro is currently working Sub-step 5c per `Plans/DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md`. Their work touches kernel source files. This dispatch DOES NOT TOUCH kernel source files.

If at any point this dispatch's instructions require touching a file Cerebro might be modifying (anything under `kernel/`, `boot/`, `tools/`), STOP and request PM clarification. Default behavior: defer the touch to a post-SC-3B follow-up dispatch.

### 2.3 What this dispatch does NOT do

- Does NOT rename `kernel/Cargo.toml` package name (currently `altimitneo` or similar). Deferred.
- Does NOT touch any file under `kernel/`, `boot/`, or `tools/`. Deferred.
- Does NOT modify any file under `Plans/` (historical records under the AltimitNeo name; preserved for honest history).
- Does NOT modify `ALTIMIT-real-world-implementation.md` (research artifact about `.hack` canon as design inspiration; preserved unchanged).
- Does NOT register any domains (project lead's task; requires personal payment + account).

---

## 3. Phase 1 — Create GitHub remote

### 3.1 Pre-creation check

Verify the target repo doesn't already exist:

```bash
gh repo view SecondStageTurbine/NeoPlutarch 2>&1 || echo "OK: repo does not exist yet"
```

Expected: `gh: Could not resolve to a Repository with the name 'SecondStageTurbine/NeoPlutarch'` (or similar 404). If the repo DOES already exist, STOP and request PM clarification — that's an unexpected state.

### 3.2 Create the repo

```bash
gh repo create SecondStageTurbine/NeoPlutarch \
    --private \
    --description "AI-native operating system in Rust. Clean-slate, capability-secured, single-node. Phase 2."
```

Notes on flags:
- `--private` per project lead's instruction. Can be flipped to public later via `gh repo edit --visibility public`.
- `--description` is an editorial line; if Prometheus prefers a different description, swap it. Default above captures the current bible posture.
- Do NOT pass `--clone` (we have an existing local repo to push to this remote, not a fresh clone).
- Do NOT pass `--push` (the remote is empty; we'll push explicitly after the three commits land).

### 3.3 Verify creation

```bash
gh repo view SecondStageTurbine/NeoPlutarch
```

Expected: shows the empty repo. Capture the remote URL — typically `https://github.com/SecondStageTurbine/NeoPlutarch.git`.

### 3.4 Wire the remote

Check existing remotes:

```bash
git remote -v
```

If `origin` exists and points to a different URL (e.g., a prior remote under the AltimitNeo name), rename it:

```bash
git remote rename origin altimitneo-legacy 2>/dev/null || true
```

Add the new remote as `origin`:

```bash
git remote add origin https://github.com/SecondStageTurbine/NeoPlutarch.git
git remote -v   # verify
```

If `origin` doesn't exist, the `add` is the only step needed.

---

## 4. Phase 2 — Three-commit sequence

### 4.1 Commit 1 — Bible v1.2.0 (Article 9 expansion + Article 28 + WiFi commitment)

**Scope:** `docs/bible.md` ONLY. The file is already at v1.2.0 in working copy.

**Verification before commit:**

```bash
# Confirm version footer reads v1.2.0
tail -10 docs/bible.md | grep "Version: 1.2.0"

# Confirm Article 28 exists
grep -c "Article 28 — Product Accessibility" docs/bible.md   # expect 1

# Confirm Article 9.10 has WiFi Phase 7.5 commitment
grep "WiFi — Phase 7.5" docs/bible.md   # expect 1 line
```

If any verification fails, STOP and request PM clarification.

**Stage and commit:**

```bash
git add docs/bible.md
git status --short   # confirm only docs/bible.md is staged
git commit -m "docs(bible): v1.2.0 — Article 9 expansion + Article 28 (Product Accessibility) + WiFi Phase 7.5 commitment

Adds Articles 9.5-9.10 (architecture coverage, hardware target classes,
input/display/storage/networking phase-binding) and new Part VI / Article 28
(Product Accessibility). Article 9.10 commits WiFi to Phase 7.5 with Intel
iwlwifi family per ADR-017. Article 13 phase ledger updated to include Phase 7.5.
Article 17 source layout updated to include boot/firmware/<vendor>/<chipset>/.
Article 26 clarified to acknowledge bundled vendor firmware as third-party content.

Rationale documents:
- Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md
- Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md

Bumps bible v1.0.5 → v1.2.0 (combined two same-day amendments)."
```

Verify:

```bash
git log -1 --stat
```

Expected: one commit changing `docs/bible.md` only.

### 4.2 Commit 2 — ADR-017 (WiFi support strategy)

**Scope:** `docs/ADR-017-wifi-support-strategy.md` ONLY. The file is already in working copy.

**Verification before commit:**

```bash
test -f docs/ADR-017-wifi-support-strategy.md
head -15 docs/ADR-017-wifi-support-strategy.md | grep "Status:.*Accepted"   # expect match
grep -c "Phase 7.5" docs/ADR-017-wifi-support-strategy.md   # expect ≥3
```

**Stage and commit:**

```bash
git add docs/ADR-017-wifi-support-strategy.md
git status --short   # confirm only ADR-017 is staged
git commit -m "docs(adr): ADR-017 — WiFi support strategy and phase-binding

Accepts the PM-recommended package: Intel iwlwifi as initial chipset family,
WiFi as Phase 7.5 deliverable, USB Ethernet stepping stones added to Phase 4
(CDC-ECM, CDC-NCM, RNDIS), bundled redistributable firmware in
boot/firmware/<vendor>/<chipset>/, vendor licenses preserved.

Decisions D1-A, D2-C, D3-A+B+C, D4-A, D5-A from
Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md.

Bible Article 9.10 amended concurrently in v1.2.0 to reflect Phase 7.5
phase-binding."
```

Verify:

```bash
git log -1 --stat
```

Expected: one commit adding `docs/ADR-017-wifi-support-strategy.md`.

### 4.3 Commit 3 — Project rename: AltimitNeo → NeoPlutarch (bible v1.3.0)

**Scope (positive list — files that ARE in scope):**

- `docs/bible.md` — bumped to v1.3.0; Article 25.1 amended for project name change; Article 25.2 clarified; preamble line revised; project-name occurrences throughout updated
- `docs/ADR-001-monolithic-kernel-reaffirmation.md` through `docs/ADR-017-wifi-support-strategy.md` — text replacement only (AltimitNeo → NeoPlutarch where it refers to the project)
- `docs/HANDOFF.md` — text replacement
- `docs/design.md` — text replacement
- `docs/inference-protocol.md` — text replacement (review for `.neom` references; subsystem prefix preserved)
- `docs/neo-model-format.md` — text replacement
- `docs/specs/*.md` — text replacement (review each)
- `docs/phases/*.md` — text replacement (review each)
- `README.md` (project root, if it exists) — text replacement; if no README.md exists, create a minimal one (see §4.3.6 below)

**Scope (negative list — files NOT in scope, do NOT touch):**

- ANY file under `kernel/` — Cerebro coordination
- ANY file under `boot/` — possible kernel coupling; deferred
- ANY file under `tools/` — same
- ANY file under `Plans/` — historical records preserved as authored
- `ALTIMIT-real-world-implementation.md` — research artifact; preserved
- `Cargo.lock`, `Cargo.toml` (root or kernel) — not touched

#### 4.3.1 Replacement rules (apply carefully)

The naive replacement is `AltimitNeo` → `NeoPlutarch`. Edge cases:

- **Preserve `Altimit OS` (with space)** — refers to the in-universe `.hack` operating system. This is referential speech in the bible preamble line "descended in spirit from Altimit OS." It stays as `Altimit OS`. Do NOT replace `Altimit OS` with anything.
- **Preserve `ALTIMIT` (all caps)** — refers to the in-universe canonical name when discussed in research/deep-research context. Do NOT replace.
- **Preserve `altimitneo` lowercase if it appears in code-fence references to package names like `altimitneo` crate** — these are deferred kernel-side renames, not in scope for this commit. Leave as-is.
- **Replace only the token `AltimitNeo`** (mixed case, exact spelling) → `NeoPlutarch` everywhere it appears outside the negative-list directories.
- **Preserve all `neo-` prefixes, `libneo`, `.neom`, `NeoFS`, `neo-init`** — these are subsystem identifiers per Article 25.2; they survive the project rename as established convention. The Article 25.2 amendment (see §4.3.4) makes this explicit.

Suggested approach for safety: use `git grep -l 'AltimitNeo' -- docs/ README.md` to enumerate touched files first, then do per-file edits with verification, rather than a global sed. If a global sed is used, scope it tightly (only `AltimitNeo` token, only the in-scope files):

```bash
# Enumerate first
git grep -l 'AltimitNeo' -- docs/ README.md 2>/dev/null

# Per-file replacement with verification (example pattern)
for f in $(git grep -l 'AltimitNeo' -- docs/ README.md 2>/dev/null); do
    echo "=== $f ==="
    grep -n 'AltimitNeo' "$f"
    sed -i 's/AltimitNeo/NeoPlutarch/g' "$f"
    echo "--- after ---"
    grep -n 'NeoPlutarch' "$f" | head -5
done
```

Verify after replacement:

```bash
# Should return 0 occurrences in in-scope files
git grep 'AltimitNeo' -- docs/ README.md 2>/dev/null

# Should preserve "Altimit OS" referential mentions
git grep 'Altimit OS' -- docs/   # expect at minimum the bible preamble line
```

#### 4.3.2 Bible v1.3.0 amendment specifics

Beyond the token replacement, Article 25.1 must be substantively amended (replacing it with project-name-update content), not just word-substituted. Current Article 25.1:

> 25.1 "AltimitNeo" is the project name. "Neo" alone is acceptable in casual reference; "AltimitNeo" in formal docs and commits.

Replace with:

> 25.1 "NeoPlutarch" is the project name. "NeoPlutarch" in formal docs and commits. The project was originally named "AltimitNeo" (Phase 0 through Phase 2 BUILD-3 sub-commit 2); a 2026-05-10 trademark-driven rename adopted "NeoPlutarch" to remove derivative-name exposure to Bandai Namco's `.hack` franchise. The rename is recorded in this commit; pre-rename project artifacts in Plans/ retain their original "AltimitNeo" naming as historical record.

Article 25.2 currently reads:

> 25.2 Subsystem prefixes: neo- for userland binaries (neo-init, neo-shell), libneo for the userland runtime, .neom for native model files, NeoFS for the filesystem.

Add a clarifying sentence:

> 25.2 Subsystem prefixes: neo- for userland binaries (neo-init, neo-shell), libneo for the userland runtime, .neom for native model files, NeoFS for the filesystem. These subsystem-name prefixes are preserved as established convention even after the 2026-05-10 project rename from "AltimitNeo" to "NeoPlutarch"; the "neo" prefix is now a project-internal convention, not a brand fragment.

Bible footer:

- Bump `Version: 1.2.0` → `Version: 1.3.0`
- Update `Last amended:` line to: `Last amended: 2026-05-10 (Article 25.1 amended — project rename from AltimitNeo to NeoPlutarch; Article 25.2 clarified — neo- subsystem prefixes preserved as established convention. Trademark-driven rename per Janus pre-flight at Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md.)`

#### 4.3.3 Bible preamble update

Current preamble first line:

> This document is the constitution of AltimitNeo — a clean-slate operating system descended in spirit from Altimit OS, written in Rust, targeting x86_64 (and later aarch64) ...

After replacement:

> This document is the constitution of NeoPlutarch — a clean-slate operating system descended in spirit from Altimit OS, written in Rust, targeting x86_64 (and later aarch64) ...

The `descended in spirit from Altimit OS` clause survives unchanged — this is the editorial decision to preserve the homage acknowledgment as factual referential speech (TM-defensible).

#### 4.3.4 Article 9.10 reference preservation

Article 9.10 references `boot/firmware/` and ADR-017. After rename, these references stay:

```
WiFi — Phase 7.5. Initial chipset family is Intel iwlwifi (AX2xx and BE2xx generations); WPA2/WPA3 client mode; hardware-default regulatory; bundled redistributable firmware in boot/firmware/. Power management, AP mode, mesh, WPA Enterprise, and additional chipset families (Realtek, Qualcomm, MediaTek) are explicit follow-ups requiring their own ADRs. See ADR-017.
```

No change needed beyond the project-name token replacement (which Article 9.10 doesn't actually contain).

#### 4.3.5 ADR file rename — text only, NOT filename

ADR filenames (`ADR-001-monolithic-kernel-reaffirmation.md`, etc.) are NOT renamed. Only their content references to "AltimitNeo" become "NeoPlutarch."

#### 4.3.6 README.md handling

Check whether `README.md` exists at project root:

```bash
test -f README.md && echo "exists" || echo "absent"
```

If exists: text-replace as per §4.3.1 rules.

If absent: create a minimal one. Suggested content:

```markdown
# NeoPlutarch

A clean-slate operating system in Rust, with AI/ML treated as first-class kernel concerns. Targets x86_64 (and later aarch64).

This is not a Linux derivative. It is not a Unix clone. The native binary format is ELF64 but the dynamic loader and ABI are our own.

## Status

In active development. Phase 2 (SMP + capability system + buddy/slab allocators + advanced TLB management) is the current planning horizon. See `docs/bible.md` for the project's foundational charter and `docs/phases/` for per-phase plans and verification logs.

## Documents

- `docs/bible.md` — project constitution
- `docs/ADR-NNN-*.md` — architecture decision records
- `docs/specs/` — technical specifications
- `docs/phases/` — per-phase plans and verification logs

## License

`MIT OR Apache-2.0` for project-authored code and documentation. Bundled vendor firmware (Phase 7.5+) retains its own vendor license. See `docs/bible.md` Article 26.

## Origin and homage

The project was originally named "AltimitNeo" (Phase 0 through Phase 2 BUILD-3 sub-commit 2); a 2026-05-10 trademark-driven rename adopted "NeoPlutarch" to remove derivative-name exposure to Bandai Namco's `.hack` franchise IP. The bible's preamble preserves the design-inspiration acknowledgment ("descended in spirit from Altimit OS") as referential speech. The deeper research artifact analyzing `.hack` canon as design inspiration is `ALTIMIT-real-world-implementation.md`.
```

Adjust as preferred — this is editorial and may benefit from more or less context.

#### 4.3.7 Stage and commit

```bash
# Stage in-scope files only
git add docs/bible.md
git add docs/ADR-*.md
git add docs/HANDOFF.md docs/design.md docs/inference-protocol.md docs/neo-model-format.md
git add docs/specs/*.md docs/phases/*.md
git add README.md   # if it exists or was created

# Verify nothing kernel/, boot/, tools/, or Plans/ is staged
git status --short | grep -E '^(M|A|D)' | grep -E '(kernel/|boot/|tools/|Plans/)' && \
    echo "ERROR: kernel/boot/tools/Plans changes staged; STOP" && exit 1

git status --short
```

If verification fails (kernel/boot/tools/Plans changes staged), unstage them with `git reset HEAD <file>` and STOP — the negative list is being violated and Cerebro coordination is at risk.

```bash
git commit -m "docs: project rename AltimitNeo → NeoPlutarch (bible v1.3.0)

Trademark-driven rename to remove derivative-name exposure to Bandai Namco's
.hack franchise IP. The original project name 'AltimitNeo' was a derivative
form of the in-universe ALTIMIT OS name; while 'AltimitNeo' is etymologically
distinct, the homage paper trail (bible preamble lineage acknowledgment,
ALTIMIT-real-world-implementation.md research artifact) makes the inheritance
impossible to disclaim if challenged. Pre-rename name selection rationale:
Plutarch is the Latinized name of the Greek historian Πλούταρχος (~45-120 CE),
etymologically derived from 'Pluto-realm / chief of riches' — preserving the
canon-thematic Pluto-association via real Greek-language ancestry rather than
via .hack franchise. Article 25.2's neo- subsystem prefix convention preserved
as established convention.

Trademark pre-flight: Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md
Verdict: Proceed with caveats. NeoPlutarch verified clean across USPTO public
records, GitHub orgs, npm/PyPI/crates.io/Go module namespaces, and primary
domain availability.

Scope of this commit: docs/, README.md, ADRs only. Pre-rename historical
artifacts in Plans/ preserved as authored under the AltimitNeo name. Kernel-side
references (kernel/Cargo.toml package name, boot/limine.cfg if applicable, any
kernel-source comments) deferred to a post-SC-3B follow-up commit to avoid
merge friction with Cerebro's in-flight Sub-step 5c work.

Bumps bible v1.2.0 → v1.3.0."
```

Verify:

```bash
git log -3 --oneline
git log -1 --stat
```

Expected: three commits (bible v1.2.0, ADR-017, rename), with commit 3 touching only docs/ + README.md.

---

## 5. Phase 3 — Push to new remote

```bash
# Push the local branch (likely 'main' or 'master') to the new remote
git branch --show-current   # capture current branch name; assume 'main'

# First push — set upstream
git push -u origin main   # or whatever branch name is current
```

Verify push:

```bash
gh repo view SecondStageTurbine/NeoPlutarch
gh repo view SecondStageTurbine/NeoPlutarch --web   # opens in browser; optional sanity check
```

Expected: GitHub repo shows the three commits in history. Latest commit is the rename. README is rendered.

---

## 6. Phase 4 — Post-push verification and carry-forwards

### 6.1 Verification checklist

- [ ] `gh repo view SecondStageTurbine/NeoPlutarch` shows the repo as private with the new commits
- [ ] Most recent three commits in `git log` correspond to (in order, most recent first): rename, ADR-017, bible v1.2.0
- [ ] `git grep 'AltimitNeo' -- docs/ README.md` returns 0 results
- [ ] `git grep 'Altimit OS' -- docs/` returns at minimum the bible preamble lineage line (preserved referential speech)
- [ ] `git grep 'AltimitNeo' -- Plans/` returns the pre-rename historical files (preserved as authored — this is correct)
- [ ] `git grep 'AltimitNeo' -- kernel/ boot/ tools/` returns whatever kernel-side references exist (NOT touched in this dispatch — deferred)
- [ ] Cerebro's WIP files in working tree are still uncommitted and undisturbed (verify with `git status` showing them as modified)

### 6.2 Carry-forwards (tracked for future dispatches)

Two follow-up dispatches will be needed AFTER Sub-step 5c closes and Cerebro hands the kernel back:

**Carry-forward 1 — Kernel-side rename completion.** A future small dispatch (likely PM-routed to either Cerebro or Prometheus depending on scope) does:
- `kernel/Cargo.toml` package name update (`altimitneo` → `neoplutarch`)
- Any kernel source comments referencing "AltimitNeo" by name
- `boot/limine.cfg` boot menu title (if applicable)
- `tools/xtask/` references (if applicable)
- A small bible amendment (no version bump needed) noting the kernel-side rename is now consistent with the v1.3.0 docs rename

**Carry-forward 2 — Bible Article 25.1 commit-message-style clarification.** Post-rename, all FUTURE commit messages should use "NeoPlutarch" not "AltimitNeo." Past commit messages stay as-authored (history is honest). No bible amendment needed for this — it's covered by 25.1's "in formal docs and commits" language.

### 6.3 Pre-rename historical artifacts

Confirm these files exist in `Plans/` and are NOT modified by this dispatch (they reference "AltimitNeo" by design, as historical records):

- `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md`
- `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md`
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`
- `Plans/DISPATCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`
- `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`
- All SC-3B-related dispatches and findings
- All earlier handoffs

```bash
git status --short | grep '^.M Plans/' && \
    echo "ERROR: Plans/ files modified; should be preserved unchanged" && exit 1
```

---

## 7. Failure handling

If any step fails (gh CLI auth issue, push rejected, verification fail):

1. STOP immediately.
2. Do NOT force-push to overwrite remote state.
3. Capture: failing command, full error output, current `git log -3 --oneline`, `git status --short`, `gh repo view` output.
4. File a brief note at `Plans/PROMETHEUS-FAILURE-2026-05-10-rename-dispatch.md` with the captured state.
5. Return to PM for triage. Do NOT attempt recovery without PM guidance — recovery sequencing for partial-state-changes can be subtle.

If push specifically fails because the remote is non-empty (unlikely if §3.1 verification passed but possible if there was a race):

```bash
gh repo view SecondStageTurbine/NeoPlutarch --json defaultBranchRef,pushedAt
```

If the remote shows commits we didn't push, STOP and request PM clarification — that's an unexpected state suggesting external interference.

---

## 8. Out of scope reminders

- Domain registration (`neoplutarch.org` etc.) — project lead's task; outside Prometheus scope
- crates.io / npm / PyPI namespace placeholder packages — optional; can be done later when actual packages are ready to publish
- Public announcement of the project — outside scope; remains private until project lead announces
- Updating any external references to the project (HN profile, blog posts, social media) — none currently exist; outside scope
- Squashing or rewriting history — DO NOT. The three-commit sequence is the desired history.

---

## 9. Cross-references

- `docs/bible.md` — current state v1.2.0 in working copy; this dispatch lands it as v1.2.0 (commit 1) then v1.3.0 (commit 3)
- `docs/ADR-017-wifi-support-strategy.md` — current state in working copy; this dispatch lands it (commit 2)
- `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md` — historical rationale for v1.2.0 amendment
- `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` — historical rationale for ADR-017
- `Plans/DISPATCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` — TM pre-flight dispatch
- `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` — TM pre-flight result, "Proceed with caveats" verdict
- `Plans/DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md` — Cerebro's active dispatch, kernel-side coordination constraint
- `ALTIMIT-real-world-implementation.md` — research artifact; preserved unchanged

---

## 10. Estimated execution time

- Phase 1 (gh repo create + remote wire): ~5 minutes
- Phase 2 commit 1 (bible v1.2.0): ~5 minutes including verification
- Phase 2 commit 2 (ADR-017): ~5 minutes including verification
- Phase 2 commit 3 (rename): ~30-60 minutes including the per-file replacement, edge-case verification (Altimit OS preservation), and Article 25 substantive edit
- Phase 3 (push): ~5 minutes
- Phase 4 (verification): ~10 minutes

**Total target: ~60-90 minutes.**

If execution exceeds 2 hours, STOP and report progress. Prolonged execution suggests either a snag or scope creep; PM triage helps.

---

**End of dispatch.**
