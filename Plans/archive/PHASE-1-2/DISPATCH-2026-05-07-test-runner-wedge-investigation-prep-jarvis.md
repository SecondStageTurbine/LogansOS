# Dispatch: Test-runner intermittent wedge — investigation prep (read/draft only)

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-07
**Type:** Read/draft work. NO source modifications, NO commits. Produces one Plans/ artifact. Targeted to be safe to run in parallel with Cerebro's ADR-015 50-run loop.
**Substrate:** Cerebro is currently mid-execution on `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` — including §3 implementation (kernel source files modified) and §6 verification loop (writing serial logs). He is reading and writing intensively to `target/verify/ADR-015-loop-run-*.serial.log` and using `target/` for build cache. **Expected `git status` state during your work:** `M` entries on `kernel/Cargo.toml`, `kernel/src/mm/heap.rs`, `kernel/src/mm/mod.rs`, `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs` — these are Cerebro's in-flight ADR-015 changes and are NORMAL. Do NOT surface them as a blocker. The directories you must NOT touch are exactly the ones with those `M` entries (plus `target/`).

---

## 0. Standing instructions

1. **Read-only on tracked code.** Do not modify any file in `kernel/`, `userland/`, `tools/`, `docs/`, `boot/`, or any other tracked directory. The only writes are to a single new file in `Plans/`.
2. **Read-only on `target/verify/*.serial.log`.** Cerebro's loop is actively writing to `target/verify/ADR-015-loop-run-*.serial.log`. Read those files freely (concurrent reads are safe), but do not delete, move, or modify them.
3. **No `cargo` invocations.** Do not run `cargo build`, `cargo k`, `cargo neo-test`, or any other command that touches `target/`. Cerebro's loop owns the build cache for the duration. PowerShell `Get-Content`, `Select-String`, `Get-ChildItem`, `Measure-Object`, etc. are all fine.
4. **Cerebro's `M` entries are EXPECTED and NORMAL.** When you run `git status`, you will see `M` entries on the kernel and tools files listed in the Substrate paragraph above. These are Cerebro's in-flight work. Do NOT surface them as a blocker. Do NOT touch them. Do NOT stage or unstage them. They simply exist for the duration of your work.
5. **No commits.** This work produces one Plans/ artifact. It does not get committed (Plans/ is gitignored except for `INDEX.md`). The artifact will be referenced by a future Cerebro diagnostic dispatch when ADR-015 closes.
6. **Stop and surface** if anything is unexpected: the logs you expect aren't there, the format isn't what's described below, you see `M` entries on files OTHER than the five Cerebro is working on, or you hit a question the dispatch doesn't answer.
7. **Time budget: ~1.5–2h.** This is read-and-organize work, not deep debugging.

---

## 1. The question this prep work answers

The triage queue carries an entry: *"Test runner intermittent wedge investigation — Cerebro's domain, deferred. Manifests as the 14-of-50 (~28%) timeout rate seen in KASLR-Q1B."*

We don't yet have a hypothesis. This dispatch produces the substrate that lets us form one. Specifically: by the time the artifact lands, anyone reading it should be able to answer:

- Which scenarios wedge, and how often is each one wedging?
- Where in the scenario does the hang appear (last serial line printed before timeout)?
- Are there pre-hang anomalies in the serial output that distinguish wedged runs from clean runs?
- Is the hang signature the same across all wedge instances, or are there multiple distinct wedge modes?
- What kernel state appears to be pending when the wedge happens (e.g., partial reap, pending IPC, pending interrupt)?
- Does the wedge correlate with KASLR placement, with scenario ordering, with anything else we can extract?

These questions feed a future Cerebro diagnostic dispatch. **You do not solve the wedge; you give us a credible starting point for solving it.**

---

## 2. Pre-flight

```powershell
cd "E:\AI Automation\AltimitNeo"

# Confirm Cerebro's loop is still running by checking that target/verify/ has growing
# ADR-015-loop-run-* files. You'll work around them, not on them.
Get-ChildItem "target/verify" -Filter "ADR-015-loop-run-*.serial.log" -ErrorAction SilentlyContinue | Measure-Object | Select-Object Count

# Confirm the KASLR-Q1B logs exist (these are your primary evidence base).
Get-ChildItem "target/verify" -Filter "KASLR-Q1B-*.serial.log" -ErrorAction SilentlyContinue | Measure-Object | Select-Object Count
# Expected: ~150 files (50 runs × 3 scenarios per run on average; the close report says 50 runs total, the file
# count will be higher because each run captures multiple scenarios).

# Confirm the close reports exist.
Test-Path "Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md"
Test-Path "Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md"
# Expected: True for both.

# Confirm git state is consistent with Cerebro's in-flight ADR-015 work.
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: 5 lines, all `M`, on:
#   kernel/Cargo.toml
#   kernel/src/mm/heap.rs
#   kernel/src/mm/mod.rs
#   kernel/src/test_harness.rs
#   tools/xtask/src/main.rs
# These are Cerebro's in-flight work. NORMAL. Do NOT surface as a blocker.
# If you see ANY OTHER `M`, `A`, or `D` entries on tracked files, STOP and surface to PM.
```

If KASLR-Q1B logs are missing, close reports are missing, or the `M` entries are on files OTHER than the five listed above, STOP and surface to PM.

---

## 3. The evidence base

You will read three categories of artifact:

### 3.1 Primary: KASLR-Q1B serial logs

Path: `target/verify/KASLR-Q1B-*.serial.log`

These are 50 runs of the default suite on clean main HEAD `0b5ffb4`. Per the close report (`Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md`):

- 4 PANIC runs (the addr.rs:390 overflow we are now fixing in ADR-015 — irrelevant to wedge investigation)
- 31 PASS runs
- 14 TIMEOUT runs (THE WEDGE — this is your data)
- 1 OTHER run (alloc.rs:573 sibling failure — irrelevant to wedge investigation)

The TIMEOUT runs are your primary evidence. Filter the file list for those by inspecting which runs the close report flags as TIMEOUT, then read those serial logs.

### 3.2 Secondary: ADR-015-loop-run-* logs (in flight)

Path: `target/verify/ADR-015-loop-run-*.serial.log`

These are being written right now by Cerebro's loop. Read them freely — concurrent reads are safe. Treat anything in there as "live data, may be incomplete." If a run is mid-flight at the moment you read its log, the file may be cut off; that's expected and not a problem for your purposes (you're cataloguing, not classifying).

By the time you finish §4 below, Cerebro will have produced more of these logs. Do a second pass over them at the end of your work to capture any additional wedge instances.

### 3.3 Context: prior close reports and PM-DECISIONS

These give you the project's historical view of the wedge:

- `Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md` — the most recent encounter; lists which runs timed out and which scenarios wedged.
- `Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md` — sub-commit 2 closure; references the wedge in passing.
- `Plans/CLOSE-REPORT-2026-05-06-*.md` (any that exist) — sub-commit 2's bisection runs, which probably also encountered wedges.
- `Plans/PM-DECISION-*` files from 2026-05-06 onward — context for what we already know about the wedge.

You don't need to read every PM-DECISION exhaustively. Skim for any mention of "TIMEOUT," "wedge," "hang," "harness," `smp_per_cpu_scheduler_smoke`, or similar.

### 3.4 Known wedge signature (from PM's spot-check)

When PM checked one ADR-015-loop-run-1 wedge log just before writing this dispatch, the last serial output before the hang was:

```
NEO_TEST/1 event=BEGIN name=smp_per_cpu_scheduler_smoke phase=dispatch
[ kernel ] sched: spawned 'smp-hog-0' (id=7)
[ kernel ] sched: spawned 'smp-hog-1' (id=8)
[ kernel ] sched: spawned 'smp-hog-2' (id=9)
[ kernel ] sched: spawned 'smp-hog-3' (id=10)
[ kernel ] sched: reaped 'smp-hog-0' (id=7)
[harness times out here]
```

This is the *partial-reap* signature: smp-hog-0 reaped, smp-hog-1/2/3 never reaped. It points strongly toward a kernel issue (one CPU's reap completes, others stall) rather than a pure harness-timing problem. This is a starting hypothesis but **not** a confirmed one — your job in §4 is to verify whether all 14+ wedge instances share this signature, or whether there are multiple distinct wedge modes.

---

## 4. The work — extract, organize, write

Produce a single Plans/ artifact:

**Path:** `Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md`

**Shape:** matches `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` (read it first to see the convention — same heading style, same section order, same prose register).

**Required sections (exact order):**

### §1. Summary

A 3–5 sentence high-level: how many wedges, which scenarios, what the dominant signature is, what the leading hypothesis is. This is the top-of-mind summary for a future PM or Cerebro who picks this up cold.

### §2. Observed symptoms

A table of every wedge instance you find. Columns:

| Run ID | Source loop | Scenario that wedged | Last serial line before hang | Time-to-timeout | Pre-hang anomalies (if any) |
|---|---|---|---|---|---|

"Source loop" is `KASLR-Q1B`, `ADR-015`, `sub-commit-2-bisect-EXP-X`, etc. — whatever loop produced the run.

"Pre-hang anomalies" is anything weird in the serial output before the hang point — unusual ordering, missing expected lines, repeated lines, kernel print interleaving issues. If nothing's anomalous, write `none`.

This table is the evidence base for everything else. Aim for completeness over brevity.

### §3. Wedge signatures

Group the table-rows from §2 into distinct signatures. For each signature:

- Give it a short name (e.g., `smp-hog-partial-reap`).
- Describe the kernel-state-at-hang in plain prose (e.g., "one of N spawned `smp-hog-*` threads reaps, others do not; no further serial output").
- List which runs exhibit it.
- Note which kernel subsystems are most plausibly implicated (scheduler? IPC? interrupt routing? per-CPU state?).

If all wedges share one signature, say so — that's a strong simplifying observation. If there are multiple distinct signatures, that's also load-bearing — it tells us the wedge is not one bug but a class of bugs.

### §4. Correlation checks

Try at least these correlations:

- **Wedge vs. KASLR placement.** The PASS/PANIC/TIMEOUT classification per run from KASLR-Q1B can be cross-referenced with the kernel-image base address printed at boot in each serial log. Is the wedge correlated with high or low KASLR rolls? With any specific address pattern?
- **Wedge vs. scenario ordering.** Does the wedge always appear in the same scenario, or does it appear in multiple scenarios? If multiple, is it always the Nth scenario in the suite ordering, or always the same scenario regardless of position?
- **Wedge vs. NEO_SMP value.** All KASLR-Q1B runs were `NEO_SMP=4`. Are there earlier wedge sightings (in sub-commit 2 bisection) at other SMP values?
- **Wedge vs. boot-time anomalies.** Any pattern in the early-boot serial output (before scenario dispatch) that distinguishes wedged from clean runs?

State each correlation result honestly: "correlated," "not correlated," "ambiguous, more data needed." Do NOT over-claim correlations from small samples.

### §5. Hypotheses

Propose 2–4 candidate causes, each with:

- Name (e.g., "AP-side reaper missing for non-BSP-spawned threads")
- Mechanism description (1–2 sentences)
- What evidence would confirm/refute it

Be concrete. "Race condition somewhere" is not a hypothesis. "AP reapers do not see threads spawned on the BSP, so smp-hog-1/2/3 (running on APs) never get reaped" is a hypothesis.

This is where the §Q9 deferred-work design notes (`Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`) become highly relevant — §Q9 is *exactly* about per-CPU reap semantics, and sub-commit 2 deferred §Q9 in favor of a BSP-scans-all-CPUs reap as a stopgap. There's a real possibility the wedge is the BSP-scans-all-CPUs reap occasionally missing AP-resident zombie threads. State this hypothesis explicitly if the evidence supports it.

### §6. Proposed diagnostic dispatch sketch

A skeletal Cerebro dispatch that would advance the investigation. Don't write the full dispatch — just the structure:

- What questions would the dispatch answer (Q1, Q2, ...)?
- What changes to the kernel or harness would be needed (purely diagnostic, no fixes)?
- What runs would the dispatch perform (e.g., "20 NEO_SMP=1 runs to test if wedge is SMP-dependent")?
- What's the decision tree on the results?

PM will turn this skeleton into a real dispatch when Cerebro has bandwidth (post-ADR-015-closure).

### §7. Cross-references

List every artifact you read while preparing this. Standard cross-reference pattern from §Q9 redo notes.

---

## 5. Constraints on the artifact

- **Length:** 200–500 lines. The §Q9 redo notes are 121 lines and that's a tight design doc; wedge investigation has more raw data, so it'll be longer. But don't pad. If §2's table runs to 30 rows, that's fine; don't add narrative for narrative's sake.
- **Tone:** matter-of-fact, evidence-first. No conclusions stronger than the data supports. Use "appears to," "consistent with," "would suggest" when correlations are tentative; use "is," "shows," "indicates" only when the evidence is unambiguous.
- **Math/numbers:** all rates as fractions (e.g., "14/50") plus parenthetical percentage (e.g., "14/50 (28%)"). All addresses in `0xffff_ffff_xxxx_xxxx` format. All run IDs verbatim (e.g., `KASLR-Q1B-main-run-43`, not "run 43 of the KASLR loop").
- **No tracked-code references unless verified:** if you cite kernel source (e.g., "the reap path in `kernel/src/sched/mod.rs:1234`"), open the file and verify the line number. Wrong line numbers age badly. **Note: read the file from disk for line numbers, not from the diff — Cerebro has uncommitted changes to several `kernel/src/mm/` and `kernel/src/test_harness.rs` files. If you find yourself wanting to reference any of those files specifically, do so by symbol name (e.g., `try_above_kernel`) rather than line number, since the line numbers are unstable until ADR-015 commits.**
- **Not normative:** like the §Q9 redo notes, this is a research artifact, not a decision document. It informs future decisions; it does not bind the project.

---

## 6. Verify and report

```powershell
# Verify the artifact was written.
Test-Path "Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md"
# Expected: True

# Verify only the artifact you wrote was added; no tracked-file changes from your work.
git status --short | Where-Object { $_ -notmatch "^\?\?" }
# Expected: same 5 `M` lines that were there at pre-flight (Cerebro's in-flight files).
# If you see ADDITIONAL `M`, `A`, or `D` entries — anything beyond the five Cerebro
# files — STOP and surface to PM. That would mean you accidentally modified a tracked
# file.

# Confirm the artifact is among the new untracked Plans/ entries.
git status --short | Select-String "TEST-RUNNER-WEDGE"
# Expected: one line, `?? Plans/TEST-RUNNER-WEDGE-INVESTIGATION-NOTES-2026-05-07.md`.
```

Plans/ is gitignored. The artifact stays untracked. PM will reference it from a future Cerebro diagnostic dispatch and from INDEX.md updates.

Ping PM when complete. Report:

- Artifact path and line count.
- Number of wedge instances catalogued in §2.
- Number of distinct signatures identified in §3.
- Leading hypothesis from §5.
- Whether Cerebro's ADR-015 loop produced any new wedge instances during your work (if you were able to incorporate them into the artifact).

---

## 7. What you do NOT do

- Do not modify any tracked source file. (Cerebro's `M` entries on the five files listed in §0 are HIS work; do not touch them.)
- Do not run `cargo` for any reason.
- Do not delete or move files in `target/verify/`.
- Do not interrupt Cerebro's loop. If you observe odd behavior in his runs (panics, OTHER classifications), surface to PM but do not act on it directly.
- Do not propose fixes. The artifact is for understanding, not action. The "proposed diagnostic dispatch sketch" in §6 is the closest you get to action, and even there you sketch the dispatch — you don't write it.
- Do not commit anything. Plans/ is gitignored.
- Do not stage or unstage Cerebro's `M` entries. They are his.
- Do not push or attempt to set up a remote. Project is local-only.

---

## 8. Time budget

- 10 min: pre-flight + reading the §Q9 redo notes for shape reference.
- 15 min: enumerate the wedge instances from KASLR-Q1B's close report and verify each by reading the corresponding serial log.
- 30 min: read the ~14 wedge serial logs end-to-end, capturing the §2 table data.
- 15 min: scan ADR-015-loop-run-* for any additional wedge instances.
- 15 min: skim earlier close reports and PM-DECISIONS for prior wedge sightings.
- 30 min: write §1 through §7 of the artifact.
- 10 min: re-read for accuracy, fix any line-number references, ping PM.

**Total: ~2h.** If something takes substantially longer, surface mid-work — likely indicates either a data anomaly worth investigating or a misunderstanding of the dispatch.

---

## 9. Why this matters

The 28% timeout rate has been background noise across multiple verification campaigns: sub-commit 1b, sub-commit 2 bisection, KASLR-Q1B, and now the in-flight ADR-015 loop. Every time a verification gate runs, ~28% of the runs produce TIMEOUTs that we have to mentally categorize as "harness wedge, not the bug we're testing." That noise floor erodes the signal-to-noise of every gate.

It's also likely related to §Q9's deferral. §Q9 was the per-CPU reapers + Process.live_thread_count + AP-side reaper; sub-commit 2 deferred it after bisection EXP-A through EXP-H showed §Q9 caused a different intermittent hang. The current substrate uses BSP-scans-all-CPUs reap as a stopgap. The wedge signature PM observed (`smp-hog-0` reaps, smp-hog-1/2/3 don't) is suspiciously consistent with "BSP scan occasionally misses AP-resident zombies." If that hypothesis pans out, the wedge investigation and the §Q9 redo (`Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`) might converge into a single piece of work.

So: this isn't just hygiene. It's prep for what may be the hardest open question in BUILD-3.

---

**End of dispatch.**
