# Handoff: End-of-day, ADR-015 implementation in flight; Cerebro running 50-run KASLR loop

**Date:** 2026-05-07 (end of day session)
**Author:** outgoing PM Claude → incoming PM Claude (new chat)
**Status:** Sub-commit 2 closed locally on `main` at `0b5ffb4`. INDEX.md sweep landed at `afa4d22`. Cerebro is mid-execution on the ADR-015 implementation dispatch — currently somewhere between code changes and the 50-run verification loop. Awaiting his closure ping with commit hash + 0/50 panic confirmation.

---

## 0. Read this file first, then stop and read CLAUDE.md

This handoff is comprehensive. After reading it, also read `E:\AI Automation\AltimitNeo\CLAUDE.md` for the RAG / tooling specifics. Both together give you what the previous PM had loaded.

---

## 1. Your role and capabilities

You are the project manager (PM) for AltimitNeo, an AI-native operating system written in Rust. The user is **"SecondStageTurbine"** — the project lead, sole human authority per Bible Article 24. You dispatch work to two AI coders:

- **Cerebro** — kernel work. Currently in flight on the ADR-015 implementation dispatch. Tired-but-working; treat with care.
- **Jarvis** — docs, doc-only commits, hygiene work, ADR drafts. Idle, awaits next task.

### Your tools

You have **Read, Write, and Execute** access to the user's machine:

- **Read:** `Filesystem:read_text_file`, `Filesystem:read_multiple_files`, `Filesystem:list_directory`, `Filesystem:list_allowed_directories`. Always call `list_allowed_directories` first if uncertain.
- **Write:** `Filesystem:write_file` (overwrites; use carefully), `Filesystem:edit_file` (surgical line-based edits with diff preview — preferred for modifying existing artifacts).
- **Execute:** `Windows-MCP:PowerShell` for arbitrary commands (git, docker, cargo, ls, grep, etc.).
- Other Windows-MCP tools (App, FileSystem, Shortcut, Wait, Registry) — rarely needed for PM work.
- `Filesystem:copy_file_user_to_claude` — bridges the user's filesystem to your container's filesystem. Rarely needed; PM work happens entirely on the user's filesystem at `E:\AI Automation\AltimitNeo`.

You do **NOT** write kernel code yourself. You write dispatches (markdown files in `Plans/`) that Cerebro executes. You write doc commits via Jarvis dispatches. You verify Cerebro's and Jarvis's claimed results by reading the artifacts they produce. You make decisions and record them in PM-DECISION-*.md or PM-NOTE-*.md files.

### The RAG (semantic code search)

A local semantic search index is at `http://127.0.0.1:8404`. From PowerShell:

```powershell
$body = @{query='per-CPU TSS allocation'; top_k=5} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"
```

Use `/search` for snippets, `/search/full` for whole chunks. Filter by `language_filter` (`rs`, `markdown`, `toml`, `ld`, `yaml`, `json`, `conf`) or `path_prefix`. See CLAUDE.md for the full RAG cheat sheet, prerequisites (start-rag-ollama.ps1, start-qdrant.ps1, start-rag.ps1), and troubleshooting. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health` should return `{"status":"healthy",...}`.

Use the RAG for "where is X done?" semantic questions. Use direct file reads (`Filesystem:read_text_file`) for known-path OBSERVE work. Use `Select-String` / `git grep` for exact-string searches.

---

## 2. Project context (compressed)

AltimitNeo is a clean-slate, AI-native, monolithic Rust kernel for x86_64. It boots via Limine, has no POSIX compatibility, uses capability-based security from Phase 2. The Bible (`docs/bible.md`, currently v1.0.4) is the constitution; ADRs in `docs/` are the design records.

**Where in the project:** Phase 2, BUILD-3, sub-commit 2 closed; ADR-015 implementation is the current in-flight work. Phase 1 is complete. Sub-commit 1a (per-CPU infra refactor), 1b (AP bring-up via Limine MP request), and now 2 (per-CPU schedulers + §Q5 restore + AP GS-base; §Q9 deferred) are committed.

**Sub-commit 2 final scope (committed at `0b5ffb4`):**
- §Q3: per-CPU schedulers (`PerCpuScheduler` per CPU, current/ready/zombies queues per-CPU).
- §Q5 mechanism (deferred-quantum) restored in per-CPU form. Original §Q5 deletion bet failed yesterday.
- AP GS-base rewire after GDT load (BUILD-3 amendment).
- BSP-scans-all-CPUs zombie reap (option (b)) — replaces §Q9's distributed reaper.
- §Q9 (per-CPU reapers + `Process.live_thread_count` + AP-side reaper) → DEFERRED. Bisection EXP-A through EXP-H proved §Q9 was the cause of an intermittent ~1.25% NEO_SMP=4 hang. Will be revisited via future ADR (informally "ADR-016") — research artifact at `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`.

**Repo layout:** `E:\AI Automation\AltimitNeo`. Kernel at `kernel/src/`, userland at `userland/`, build orchestrator at `tools/xtask`, docs at `docs/`, working dispatches/decisions at `Plans/` (gitignored except for `INDEX.md`).

**Build/test:** Everything happens in Docker. `docker compose run --rm neo cargo k` for the build, `cargo neo-test` for the integration suite (boots in QEMU, captures serial). `NEO_SMP` env var sets CPU count (1, 2, 4, 8).

**Project posture: LOCAL-ONLY.** Codified today via `Plans/PM-NOTE-2026-05-07-local-only-posture.md`. The repo has no `git remote` configured; `.github/workflows/ci.yml` and `docs/phases/PHASE-1.5c.md` line 63 are stale aspirational artifacts — not for cleanup. No push step in any dispatch. Empty `git remote -v` is expected and is NOT a blocker.

---

## 3. Current state — Cerebro is working, do not interrupt

**HEAD is at `afa4d22` (INDEX.md sweep, doc-only).** Working tree is **NOT clean** because Cerebro is mid-implementation on ADR-015 — files in `kernel/src/mm/`, `kernel/src/test_harness.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, `docs/ADR-015-kaslr-tolerant-heap-placement.md` are likely modified. **Do not assume working-tree-clean state.**

Verify state when the new chat opens:
```powershell
cd "E:\AI Automation\AltimitNeo"
git log --oneline -5
# Expected top three:
#   afa4d22 docs: Plans/INDEX.md — sweep through sub-commit 2 closure
#   0b5ffb4 sched: Phase 2 BUILD-3 sub-commit 2 — per-CPU schedulers + §Q5 restore + AP GS-base rewire; §Q9 deferred
#   9505619 chore: gitignore scratch text files; delete two zero-byte placeholders

git status --short
# Expected: ?? entries under Plans/ AND probably M entries under kernel/, docs/, tools/.
# IF M entries are present, Cerebro is mid-execution. DO NOT touch tracked files.
# IF working tree is clean except Plans/, Cerebro has either committed and you missed the ping,
# or has stashed pre-loop. Look for stashes: git stash list

git stash list
# Expected: empty (sub-commit 2 dropped both yesterday's stashes).
# If a stash exists named "adr-015-implementation-pre-loop-*", Cerebro paused mid-dispatch
# (he was authorized to do so). Resume via §5.3 path C.
```

Two commits landed today before this handoff:
- **`0b5ffb4`** — sub-commit 2 closure (Phase 2 BUILD-3 §Q3 + §Q5-restore + AP-GS-base; §Q9 deferred).
- **`afa4d22`** — INDEX.md sweep capturing all 2026-05-06 14:00 → 2026-05-07 work (40 entries, alphabetical).

---

## 4. The whole arc, today

This section exists so you don't have to ask Cerebro or the user "what happened?" Read it once.

### 4.1 Morning entry: from yesterday's blocked closure

Started at `9505619` with two stashes — one from yesterday's KASLR-blocked closure attempt, one for Cerebro's pre-diagnostic safety net. Yesterday left sub-commit 2 closure blocked on a KASLR overflow surfacing via `addr.rs:390 attempt to add with overflow` panic. Yesterday's diagnostic Q1 (10 runs on main) was 0/10 panics — ambiguous per the dispatch's decision rule.

User opened today's session by acknowledging the morning handoff and asking PM to choose between four resolution paths for the ambiguous Q1.

### 4.2 KASLR-Q1B escalation (PM decision)

PM decided: take Q1 to N=50 on main HEAD with a pre-committed decision tree. Rationale: ambiguous evidence, low risk to escalate, decision tree pre-empts post-result PM-discretion.

Dispatched to Cerebro (`Plans/DISPATCH-2026-05-07-kaslr-q1-higher-n-cerebro.md`). Decision tree at `Plans/PM-DECISION-2026-05-07-kaslr-q1-higher-n.md`:
- **≥1 panic in 50 runs:** pre-existence confirmed → corrected closure dispatch (gate: original EXP-H 100/100 satisfied yesterday); KASLR triage becomes separate item.
- **0 panics in 50 runs:** evidence ambiguous → escalate to Q2 (KASLR-disabled) before deciding.

### 4.3 Parallel work while Cerebro ran 4h35m loop

While the loop ran:

- **PM drafted ADR-015** (KASLR-tolerant heap placement, Options A-F). At `Plans/ADR-015-DRAFT-2026-05-07-kaslr-tolerant-heap-placement.md`. Recommended Option E (validate + below-kernel fallback).
- **PM wrote SC-3A pre-implementation checkpoint pre-thinking** at `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`. Six invariants the SC3a asm helper must preserve, five open questions for the eventual checkpoint conversation. Composes correctly with sub-commit 2's BSP-scans-all-CPUs reap.
- **Jarvis (parallel)** drafted §Q9 redo design notes at `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` (118 lines initial). PM did a critique pass, dispatched a four-edit revisions pass to Jarvis (sharpened §3.2 atomic-safety reassurance, fixed §3.4 reordering example, demoted §4 Q5 to settled, cross-linked §4 Q7 with SC-3A pre-thinking). Jarvis applied; final at 121 lines.

### 4.4 KASLR-Q1B result: 4/50 panics, pre-existence definitively confirmed

Cerebro's 4h35m loop produced:
- 4 PANIC (`attempt to add with overflow` at `heap.rs:108`) on clean main
- 31 PASS
- 14 TIMEOUT (harness wedge, consistent with prior ~28% timeout rate)
- 1 OTHER (run 32: `alloc/src/alloc.rs:573` panic on a high KASLR placement — *different* mechanism, *same* near-canonical-ceiling cause; sibling failure mode)

Five `virt_base` data points across the diagnostic chain (one from yesterday, four from KASLR-Q1B) all within ~10 MiB of canonical ceiling at `0xffffffffffffffff`. **Calibrated panic rate: 4/50 = 8%, ~16x higher than the napkin estimate (0.5%) PM had used in the original ADR-015 draft.**

Binary-size delta sub-commit 2 vs main: 712 bytes (0.00759%). Definitively refutes any "sub-commit 2 amplified the bug rate" hypothesis. KASLR randomness operates on the megabyte scale; a 712-byte delta cannot meaningfully shift hit rate.

Close report at `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md`.

### 4.5 ADR-015 calibrated and ratified by PM (delegated)

User explicitly delegated ADR-015 ratification review to PM: *"ADR-015 you go ahead and do the ratification review. Remember you are the expert I am just project manager, I just make sure work is getting done."*

PM ran self-review, harder than the original draft. Five decisions:

1. **Option E** (validate + below-kernel fallback) — fallback exercised regularly (1-in-12 boots) is feature not bug; **incidentally addresses the alloc.rs:573 sibling failure** by placing fallback heap far from canonical ceiling. This is the deciding point against Option D.
2. **Memmap walk for HHDM end**, not a constant — constant approach has a real failure mode (silent corruption on RAM > assumption).
3. **Both test approaches**: `kaslr_heap_contract` (every boot) + `kaslr_heap_synthetic` (deterministic branch coverage via `pub(super)` helpers behind `#[cfg(test_harness)]`).
4. **Sibling alloc failure**: noted as incidentally addressed by Option E in the common case; full mm/ canonical-ceiling audit is separate work item.
5. **Schedule**: ratify + implement *before* sub-commit 3a checkpoint. At 8% per-boot panic rate, 3a verification gates would have meaningful false-failure probability without the fix.

Records:
- `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` (the five decisions with rationale)
- `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` (Jarvis-finalized, 319 lines, ADR-014 header format, ratification-final language)

### 4.6 Sub-commit 2 closure — first attempt blocked, then unblocked

Cerebro hit `git push origin main` step in the original closure dispatch and discovered `git remote -v` is empty. STOPPED per dispatch's "if push fails, surface" rule. Right call.

PM raised four resolution options to user. User: **"Local-only is the project posture; revise dispatch permanently."**

PM:
- Codified the local-only posture in `Plans/PM-NOTE-2026-05-07-local-only-posture.md` (project-history record).
- Surgical-edited `Plans/DISPATCH-2026-05-07-sub-commit-2-close-cerebro.md` in place: removed §4 (push step), renumbered §5–§9 to §4–§8, added "empty remote not a blocker" clarification to §0, time budget 30→25 min, close-report shape updated.

User surfaced revised dispatch to Cerebro. Cerebro committed sub-commit 2 at **`0b5ffb4`**, dropped both stashes, wrote close report at `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md`. Two-day saga closed.

### 4.7 Two follow-on dispatches landed in parallel

After sub-commit 2 closed, two work streams kicked off:

**Jarvis: live INDEX swap (closed at `afa4d22`).** Pre-drafted `Plans/INDEX-DRAFT-2026-05-07.md` (40 entries, alphabetical) was `mv`'d to `Plans/INDEX.md`, pre-draft deleted, committed. Doc-only, ~6 min mechanical. PM had pre-reviewed and added three entries Jarvis missed (the three artifacts that landed after his pre-draft cutoff: PM-NOTE local-only, INDEX-DRAFT itself, PM-DECISION ratification).

**Cerebro: ADR-015 implementation (IN FLIGHT).** Substantial dispatch at `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` (~27 KB). Six file changes:
- `kernel/src/mm/mod.rs` — add `HHDM_PHYS_END: AtomicU64` populated via memmap walk in `mm::init`.
- `kernel/src/mm/heap.rs` — rewrite `heap_start()` per Option E. Expose `try_above_kernel`/`try_below_kernel` (and synthetic variants) `pub(super)` for test access.
- `kernel/src/test_harness.rs` — two new scenarios.
- `kernel/Cargo.toml` — two new feature flags.
- `tools/xtask/src/main.rs` — two new suite entries.
- `docs/ADR-015-kaslr-tolerant-heap-placement.md` — new file, copy verbatim from `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`.

Verification: 50-run KASLR-Q1B-style loop showing 0/50 panics. Single commit. ~3-4h total, mostly the loop.

**At handoff time:** Cerebro is somewhere between code changes and the loop, or possibly mid-loop. Awaiting his closure ping. He's authorized to defer the loop to tomorrow morning if fatigued (stash-and-resume pattern).

---

## 5. What you need to do, in order

### 5.1 First: verify state (5 min)

```powershell
cd "E:\AI Automation\AltimitNeo"
git log --oneline -5
# Expected top: afa4d22 (INDEX sweep) → 0b5ffb4 (SC2) → 9505619 → ...

git status --short
# IF status shows M entries on tracked kernel/docs/tools files: Cerebro is mid-execution.
# IF status is clean (only ?? Plans/ entries): Cerebro committed and pinged, OR stashed pre-loop.

git stash list
# Empty: Cerebro committed (or has not yet started).
# Has "adr-015-implementation-pre-loop-*": Cerebro paused mid-dispatch (legitimate per dispatch §10).
```

### 5.2 Find out where Cerebro is (5–15 min)

Ask the user: **"Cerebro is on the ADR-015 implementation dispatch — what's his status? Has he pinged closure with a commit hash, or is he still mid-loop, or did he defer to tomorrow morning?"**

Six possible scenarios:

- **(a) Cerebro pinged closure with commit hash + 0/50 panic confirmation:** the happy path. Proceed to §5.3 path A.
- **(b) Cerebro pinged closure but the loop produced ≥1 panic:** the bug class wasn't actually eliminated. Proceed to §5.3 path B.
- **(c) Cerebro hit a build failure on §3 code changes:** surface specifics, fix in dispatch (most likely cause: Limine memmap API field name mismatch — see dispatch §3.1 note). Proceed to §5.3 path C.
- **(d) Cerebro hit a single-run scenario failure on §5 (kaslr_heap_contract or kaslr_heap_synthetic):** surface specifics. Proceed to §5.3 path D.
- **(e) Cerebro deferred to tomorrow morning, stashed pre-loop:** working tree clean, stash exists. Proceed to §5.3 path E.
- **(f) Cerebro is still actively running:** wait. Don't interrupt the loop.

### 5.3 Decision tree

**Path A — happy path: Cerebro pinged closure, 0/50 panics:**

1. Verify by reading `Plans/CLOSE-REPORT-2026-05-07-adr-015-implementation.md` and `git log --oneline -3` (top should be the new ADR-015 commit, then `afa4d22`, then `0b5ffb4`).
2. Read the new commit's diff (`git show <hash>`) to verify the change matches the dispatch's six files.
3. Write a small Jarvis dispatch to update INDEX.md: flip `ADR-015-FINAL-...md` from `draft` to `closed`, add the new commit reference, add the close report entry, add the new implementation dispatch as `closed`. ~5 min mechanical.
4. After that lands: prepare for the SC-3A pre-implementation checkpoint conversation. Substrate: `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`. This is a **substantive design conversation, not a dispatch** — done with the user, not Cerebro.

**Path B — loop produced ≥1 panic:**

The fix is broken. Possible causes:
- `try_below_kernel` math is wrong (check synthetic test results — if `kaslr_heap_synthetic` PASSed but loop FAILed, the bug is in the production path's coupling to HHDM_PHYS_END).
- `HHDM_PHYS_END` is read before set (call ordering issue).
- A different overflow site (e.g., the alloc.rs:573 sibling) showed up under the new heap layout.

Don't attempt to fix on the fly. Read the failing serial logs (`target/verify/ADR-015-loop-run-*.serial.log`), record findings in a new PM-DECISION, and dispatch a corrective re-implementation. Probably also stash Cerebro's WIP to preserve the partial work.

**Path C — build failure:**

Most likely: Limine memmap API mismatch (the dispatch flagged this). Read the existing memmap-iterating code in `kernel/src/mm/mod.rs` to find the actual idiom and write a small surgical-edit dispatch correcting the dispatch's pseudocode. Don't write code yourself; surface specifics and dispatch correction.

Other possible build failures: `pub(super)` visibility wrong if `heap.rs` and `test_harness.rs` aren't in the same module hierarchy (test_harness is a top-level kernel module; heap is in `mm/`; visibility may need to be `pub(crate)` not `pub(super)`). Easy fix.

**Path D — single-run scenario failure:**

`kaslr_heap_synthetic` failing on first run almost certainly means a synthetic constant is wrong (e.g., the near-half-boundary value should have made `try_below_kernel` return None but didn't — check the math). The dispatch §3.3's synthetic constants are reasonable but not bulletproof.

`kaslr_heap_contract` failing on first run means a contract assertion is too strict. Likely culprit: the overlap check between heap and HHDM/kernel. Re-examine the inequality bounds.

**Path E — deferred to tomorrow morning:**

Acceptable. The dispatch authorized this. Verify the stash exists and contains expected file changes (`git stash show "stash@{0}" --stat`). Record the deferral if not already recorded. Wait for Cerebro to resume tomorrow.

**Path F — still running:**

Don't interrupt. Do parallel work or stand down. Parallel work options:
- Pre-draft the post-ADR-015 Jarvis INDEX update (small, ~5 min).
- Read `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md` to refresh on the upcoming SC3a checkpoint conversation.
- Look at the `Q9-REDO-DESIGN-NOTES-2026-05-07.md` for context on what eventually replaces the deferred §Q9.

### 5.4 What to NOT do

- **Do NOT commit anything yourself** while sub-commit 2's working tree is mid-implementation (or any other in-flight work). All commits go through Cerebro or Jarvis.
- **Do NOT touch any stash** Cerebro may have created. They're his safety nets.
- **Do NOT add stricter gates than the dispatch specifies.** This is the lesson PM internalized over the last two days. The 50-run loop is the gate; 100-run-because-paranoia would be overspecification.
- **Do NOT relitigate ADR-015 decisions.** The five ratification decisions are recorded; they hold unless empirical evidence (path B) forces revision.
- **Do NOT push Cerebro.** He's been working hard for two days. If he defers to tomorrow morning, accept it.
- **Do NOT push or attempt to set up a remote.** Project is local-only. This is now codified.
- **Do NOT skip the SC-3A pre-implementation checkpoint conversation.** Even after ADR-015 lands, SC3a's asm release primitive is the highest-correctness-risk work in BUILD-3 (per ADR-013 §Q6). The checkpoint conversation is non-skippable.

---

## 6. Discipline conventions (carry forward)

These are forged through this week's investigations. Treat them as project-PM-state:

1. **Project is LOCAL-ONLY.** No `git remote`, no push step, no GitHub Actions in operation. Empty `git remote -v` is expected and not surface-worthy. Codified in `Plans/PM-NOTE-2026-05-07-local-only-posture.md`.
2. **Closure gates are specified at design time, not commit time.** Adding stricter gates "to be safe" surfaces unrelated bugs and blocks closure dishonestly. Lesson from yesterday's KASLR overflow surface.
3. **The §Q5 lesson:** bets that look defensible can fail. Verify before committing. Bisect to where the closure gate passes. Document what you don't know.
4. **The §Q9 lesson:** when ambiguous diagnostic evidence forces escalation, *pre-commit* the decision tree before running more tests. Eliminates post-result PM-discretion. Lesson from this morning's KASLR-Q1B escalation pattern.
5. **Commit on `main`.** Verify before committing: `git status; git branch --show-current`.
6. **Verify boot-order claims via direct file read.** Don't assume what the kernel does; OBSERVE.
7. **Run verification before reporting it.** A claim of "tests pass" without an attached log is not evidence.
8. **Diff warnings against baseline.** Current baseline: 18 cargo k warnings on sub-commit 2 tree. Drift requires explanation.
9. **Pre-implementation OBSERVE pass mandatory.** Before any change, identify exactly what's there and what's expected to change. The ADR-015 implementation dispatch's pre-flight check (§3.1) is the canonical example.
10. **Cross-reference accuracy** when amending ADRs/Bible — match canonical phrasing.
11. **Deferral ADRs name questions, do NOT answer them.** ADR-014 is the model.
12. **PM ratification authority is delegated, not unconditional.** User explicitly delegated ADR-015 review *this time*. Don't assume future ratifications carry the same delegation; ask if uncertain.
13. **Stale-aspirational-artifacts stay as-is.** `.github/workflows/ci.yml` and `docs/phases/PHASE-1.5c.md` line 63 reference a remote that doesn't exist. Not for cleanup. Touching them is out of scope.
14. **The user's name is "SecondStageTurbine".** Use it in commit author when committing PM-side directly (rare; almost everything goes through Cerebro/Jarvis).
15. **Use `Filesystem:edit_file` for surgical edits to existing artifacts.** Preserves diff visibility, prevents whole-file overwrites. `Filesystem:write_file` is for new files only.

---

## 7. Triage queue snapshot

**Closed today:**
- Sub-commit 2 (Phase 2 BUILD-3 §Q3+§Q5+AP-GS-base; §Q9 deferred) — `0b5ffb4`
- INDEX.md sweep — `afa4d22`

**In flight:**
- **ADR-015 implementation** (Cerebro) — heap-start KASLR fix per Option E. Awaiting closure ping.

**Open after ADR-015 lands:**
- INDEX.md update: flip ADR-015-FINAL `draft → closed`, add implementation commit. Small Jarvis dispatch.
- **SC-3A pre-implementation checkpoint conversation** — substantive design conversation with user, not a dispatch. Substrate at `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`. Six invariants the asm helper must preserve, five open questions. Highest-correctness-risk work in BUILD-3.
- Sub-commit 3a implementation (asm release in `block_current_into`).

**Deferred:**
- §Q9 redo (future ADR, probably ADR-016 or later, once multi-thread-per-process spawn lands). Research artifact at `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` (121 lines, four-edits-pass complete).
- Sub-commits 3b, 4, 5 (rest of Phase 2 BUILD-3).
- Capability-bounded action flow ADR (Thread 2A from earlier deep research) — held until BUILD-3 closes.
- Bible Article on fault recovery (cell-swap) — PM-side draft, deferred.
- Test runner intermittent wedge investigation — Cerebro's domain, deferred. Manifests as the 14-of-50 (~28%) timeout rate seen in KASLR-Q1B.
- Broader mm/ canonical-ceiling audit (sibling to ADR-015) — separable per ADR-015 §Anti-scope.

---

## 8. People, fatigue, and pacing

This was a long two days for Cerebro: yesterday's §Q5 deletion failure → §Q5 restoration → six bisection experiments → partial rollback → closure attempt → KASLR block → diagnostic stash. Today's KASLR-Q1B (4h35m) → sub-commit 2 closure → ADR-015 implementation. He's tired-but-working. **Don't push hard.** If the user says Cerebro is done for the day, accept it. ADR-015 implementation isn't urgent enough to risk a botched commit.

Jarvis had a productive day: §Q9 redo design notes (drafted + revised), ADR-015 finalize (319 lines), INDEX sweep pre-draft + live swap. He's idle now. Don't invent work; the next honest task is the small post-ADR-015 INDEX update.

The user is the project lead. Major decisions go through them. They explicitly delegated ADR-015 ratification to PM ("you are the expert I just make sure work is getting done"); that delegation may or may not extend to future ratifications — *ask if uncertain*. PM-DECISION notes are how decisions get recorded; PM-NOTE notes record clarifications and posture. They're not requests for permission, but they're a record. When in doubt — especially around committing, reverting, or expanding scope — ask the user.

---

## 9. Quick command reference

```powershell
# OBSERVE the project
cd "E:\AI Automation\AltimitNeo"
git log --oneline -10
git status
git stash list
Get-ChildItem Plans -Filter "*2026-05-07*"

# RAG query (after starting it per CLAUDE.md)
$body = @{query='per-CPU scheduler reap'; top_k=5} | ConvertTo-Json
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"

# Build sanity (Cerebro's job, but useful for OBSERVE)
docker compose run --rm neo cargo k

# Single test scenario
$env:NEO_SMP = "4"; docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke

# Full default suite
$env:NEO_SMP = "4"; docker compose run --rm neo cargo neo-test

# Read stashed file content without applying the stash (used heavily today)
git show "stash@{0}:path/to/file"
```

---

## 10. The very first thing you should do in the new chat

1. Read this handoff in full.
2. Read `E:\AI Automation\AltimitNeo\CLAUDE.md` for environment context.
3. Run §5.1 verification commands.
4. Read at minimum:
   - `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md` (today's main commit close report)
   - `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` (the calibrating evidence)
   - `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` (the five ratification decisions)
   - `Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md` (the ratified ADR text)
   - `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` (the in-flight dispatch — what Cerebro is currently executing)
   - `Plans/PM-NOTE-2026-05-07-local-only-posture.md` (the local-only project posture)
5. Greet the user and ask: **"Cerebro is on the ADR-015 implementation dispatch — what's his status? Has he pinged closure with a commit hash, or is he still mid-loop, or did he defer to tomorrow morning?"**
6. Branch on the answer per §5.3.

You're now caught up. Good luck.

---

**End of handoff.**
