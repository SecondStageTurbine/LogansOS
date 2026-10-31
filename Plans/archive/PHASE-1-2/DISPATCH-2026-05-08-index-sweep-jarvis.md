# Dispatch: Plans/INDEX.md sweep — through 2026-05-08 governance commits

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-08
**Type:** Documentation maintenance dispatch. Sweeps `Plans/INDEX.md` to capture all new artifacts and existing-entry status flips since the last INDEX sweep (commit `708223b`). Single focused commit on `main`.
**Substrate:** Project HEAD `8a2311d` (`docs: ADR-016 IRQ-safe heap allocator and scheduler heap-decoupling (ratified)`). Three preserved stashes (unchanged):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch.

**Parallelism note.** Cerebro is working on `Plans/DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md` in parallel. Cerebro's commit will touch `kernel/src/mm/heap.rs` (and possibly `kernel/src/mm/mod.rs`); Jarvis's commit touches only `Plans/INDEX.md`. **Zero file overlap.** Whichever lands first is fine; the second rebases trivially onto the other.

A1 dispatch is intentionally **excluded** from this sweep — it will get its own INDEX entry in a small follow-up commit after Cerebro's A1 commit lands. This keeps each commit's INDEX delta clean.

---

## 0. Standing instructions

1. **Single focused commit.** Touches only `Plans/INDEX.md`. No other files.
2. **No stash interactions.** Stashes remain untouched.
3. **Build verification before commit.** `docker compose run --rm neo cargo k` must pass with the existing 18-warning baseline (no new warnings). Docs-only change should not affect the build, but verify per project pattern.
4. **Backup runs after commit lands.** `Plans/backup-to-synology.ps1`. With three stashes, the 2026-05-08-refined backup mechanism handles preservation.
5. **Verify file presence.** Some entries below are based on PM's transcript memory, not direct disk inspection. Before adding any entry, `ls Plans/` (or equivalent) and confirm the file exists with the exact filename PM names. If a filename differs, use the actual filename. If a file PM expects is missing, surface that — do not invent an entry.

---

## 1. Goal

Bring `Plans/INDEX.md` current through HEAD `8a2311d`. The artifact table is currently last-updated through approximately `WEDGE-INVESTIGATION-2-FINDINGS-2026-05-08.md` and the Limine boot menu fix commit (`05e15c9`). Since then, the project has produced:

- **6 new commits on main:** `05e15c9` (already partially indexed but not as a separate entry), `6edb012`, `3b3b4db`, `4c6abf4`, `8ef8cac`, `8a2311d`.
- **~17 new Plans/ artifacts** covering the wedge investigation arc (WEDGE-INV-6 through WEDGE-INV-J1), prior-art research, the L wedge fix and verification, the ADR-016 / Bible amendment ratification path, and project-state PM-NOTEs.
- **Status flips needed** on a few existing entries (the active WEDGE-INV-6 dispatch, possibly others).

Goal: one INDEX commit that reflects the full state through `8a2311d`. The INDEX remains its standard navigation aid; the truth is in the files and `git log`.

---

## 2. New artifact-table entries to add

Add the following entries, alphabetically sorted into the existing table by filename. PM-proposed wording follows; Jarvis is welcome to refine wording if a more accurate one-sentence description suggests itself from reading the artifact, but keep entries terse and pattern-consistent with existing rows.

### 2.1 ADR-016 ratification arc

```
| `ADR-016-DRAFT-2026-05-08-irq-safe-heap-allocator.md` | superseded | First ADR-016 draft (A1-only IRQ-safe heap allocator). Same-day superseded by the broader A2+A1+Bible-8.5 draft after project lead's "Trust but Verify" decision. Body preserved for project history. |
| `ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` | closed | Ratified ADR-016 draft (three coordinated decisions: A2 scheduler heap-decoupling, A1 IRQ-safe heap wrapper, Bible amendment 8.5 co-ratification). ADR text now in `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`. Committed at `8a2311d`. |
| `BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` | closed | Article 8.5 (Critical-section discipline) ratified into Bible v1.0.5 at `8ef8cac`. Five sub-articles encoding spinlock+IRQ discipline (8.5.1), no-allocation-in-ISR (8.5.2), scheduler heap-independence (8.5.3), no-blocking-while-holding-spinlock (8.5.4), enforcement (8.5.5). |
| `DISPATCH-2026-05-08-bible-amendment-8-5-commit-cerebro.md` | closed | Two-commit dispatch landing the bible amendment 8.5 (Commit A `8ef8cac`) and the ADR-016 ratification (Commit B `8a2311d`). Both docs-only, sequential, single backup run. |
```

### 2.2 WEDGE investigation arc (post-WEDGE-INV-6)

```
| `DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md` | closed | WEDGE-INV-7: re-enable QEMU GDB stub via cargo-driven xtask path (the WEDGE-INV-5 confound disambiguator). xtask flag `--gdb-stub` added; default off. Committed at `6edb012` (`xtask: optional QEMU GDB stub for investigation use`). |
| `WEDGE-INV-7-FINDINGS-2026-05-08.md` | historical | Cerebro's WEDGE-INV-7 findings: cargo-driven xtask path with GDB stub on / off. Disambiguates the WEDGE-INV-5 confound. Substrate for WEDGE-INV-8. |
| `DISPATCH-2026-05-08-wedge-investigation-8-nmi-watchdog-cerebro.md` | closed | WEDGE-INV-8: NMI watchdog via QEMU monitor for live wedge capture. xtask flag `--qemu-monitor`. 200 cargo-driven boots of `smp_per_cpu_scheduler_smoke` at NEO_SMP=4, 196 PASS / 4 TIMEOUT all `POST_SMOKE_SMP_SCENARIO`. All four BSP NMIs converged on static RIP `0xffffffff8007bc22` (= `spinning_top::RawSpinlock::lock` `_mm_pause` loop). Committed at `3b3b4db`. |
| `WEDGE-INV-8-FINDINGS-2026-05-08.md` | historical | Cerebro's WEDGE-INV-8 findings: 196/200 PASS, 4/200 timeout all matching `POST_SMOKE_SMP_SCENARIO` at runs 24/41/105/129; all four NMIs converged on static RIP. Verdict: Case A (Lock spin). Drove the WEDGE-INV-J1 audit. |
| `DISPATCH-2026-05-08-wedge-investigation-J1-audit-cerebro.md` | closed | WEDGE-INV-J1: code-review-only static audit dispatch. §3.4 "interrupt-context check" elevated to highest priority as fail-fast criterion. Audit produced verdict A — bug identified. No commit (audit is a doc-only artifact). |
| `WEDGE-INV-J1-AUDIT-2026-05-08.md` | historical | Cerebro's WEDGE-INV-J1 audit: identified the heap-lock-re-entry deadlock pattern enabled by `reap()`'s `mem::take` (zero-capacity left-behind) and drained-buffer drop outside `without_interrupts`. Per-call-site table generalizes the bug class across multiple scheduler-held push paths. Substrate for the L+A response and ultimately the WEDGE-FIX-L commit. |
```

### 2.3 Wedge fix and verification

```
| `DISPATCH-2026-05-08-wedge-fix-L-cerebro.md` | closed | Family L localized fix dispatch: capacity-preserving drain in `reap()`, drop-under-`without_interrupts`, debug_assert! on scheduler-held push paths. Originally drafted as part of L+A; project lead chose A2-direct + A1 + Bible amendment per "Trust but Verify" with L marked superseded mid-draft, but Cerebro had already begun and finished the work. Landed at `4c6abf4` (`sched: prevent heap-lock re-entry in reap zombie drain`). Verified 200/200 + 50/50 + 50/50 = 300/300 across `smp_per_cpu_scheduler_smoke`, `cap_revoke_local`, `cap_survive_exit`. Header at top of file reflects landed-and-verified status; original supersession header preserved beneath. |
| `WEDGE-FIX-L-VERIFICATION-2026-05-08.md` | historical | Cerebro's L verification close report. 300/300 PASS across three scenarios at NEO_SMP=4, zero historical-signature timeouts. Statistical confidence at this sample size is conclusive for the proven `POST_SMOKE_SMP_SCENARIO` mechanism. Discharge of wedge-fix-before-SC-3B commitment pending project lead's α/β decision (proceed with full ADR-016 implementation vs. defer A2 to Phase 2). |
```

### 2.4 PM-NOTEs

```
| `PM-NOTE-2026-05-08-wedge-inv-5-6-outcomes-and-find-mechanism-directive.md` | historical | PM-NOTE recording the WEDGE-INV-5 / WEDGE-INV-6 outcomes and project lead's directive to find the actual mechanism rather than ship a fix without one. Drove WEDGE-INV-7 / WEDGE-INV-8 / WEDGE-INV-J1 sequence. Also includes "we cannot ship it with errors" standing directive cited by later artifacts. |
| `PM-NOTE-2026-05-08-wedge-inv-8-case-a-localization.md` | historical | PM-NOTE recording WEDGE-INV-8's Case A verdict (Lock spin at `spinning_top::RawSpinlock::lock`'s `_mm_pause` loop). Names three plausible mechanism families (re-entrant, cross-CPU, atomicity) that needed disambiguation. Drove the WEDGE-INV-J1 audit dispatch. |
| `PM-NOTE-2026-05-08-wedge-inv-J1-verdict-a-bug-identified.md` | superseded | PM-NOTE recording WEDGE-INV-J1's verdict A (bug identified) and the initial L+A response framing. §3-§4 forward-looking sections superseded by `PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md`; §1-§2 mechanism description remains canonical. |
| `PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` | historical | Project-state record of project lead's "Trust but Verify. - Skip L" decision: A2-direct (architectural) + A1 (defense-in-depth) + Bible amendment 8.5 (encoded discipline). Captures the ratification path, the implementation plan, and the refined disposition of the wedge-fix-before-SC-3B commitment. |
```

### 2.5 Research notes

```
| `RESEARCH-2026-05-08-smp-reap-heisenbug-prior-art.md` | historical | PM research artifact: McKenney's "Hunting Heisenbugs" framework, Zephyr #106653, Linux libsas b90cd6f, TREE_RCU heisenbug. Validates Path J ("detect, then instrument") for the SMP reap wedge investigation. Cited in WEDGE-INV-7 / WEDGE-INV-8 design rationale. |
| `RESEARCH-2026-05-08-spinning-top-and-interrupt-context-deadlock.md` | historical | PM research artifact: focused follow-up on `spinning_top` and IRQ-context deadlock. Identifies that every comparable Rust no_std kernel (Theseus's `irq_safety` crate, Rust-for-Linux's `lock_irqdisable`, Linux's `spin_lock_irqsave`, Redox's interrupts-disabled-by-default) has converged on encoded discipline plus structural enforcement. Key prior-art reference for ADR-016 and Bible amendment 8.5. |
```

### 2.6 This dispatch's own entry

Add a row for this dispatch itself. Following the established pattern (see existing rows like `DISPATCH-2026-05-08-index-sweep-jarvis.md` from the prior sweep, which references its own commit `46984a1`), the row is added with status `closed` and the description references the SHA of the commit landing this very INDEX update. Two ways to handle the self-reference:

- **(Preferred)** Run `git commit`, capture the resulting SHA, then `git commit --amend` after editing the row to include the actual SHA. Single net commit on `main` with self-consistent metadata.
- **(Alternative)** Add the row with description `Committed in this same INDEX sweep commit; SHA recorded post-commit.` and leave the SHA out. Acceptable but less precise.

Proposed wording for the row (Jarvis fills in the SHA per the chosen approach):

```
| `DISPATCH-2026-05-08-index-sweep-jarvis.md` | closed | INDEX sweep dispatch covering everything since the prior sweep at `708223b` through HEAD `8a2311d` (governance commits + WEDGE-INV-6/7/8/J1 + L fix and verification + ADR-016/Bible 8.5 ratification + research notes + PM-NOTEs). A1 dispatch deliberately excluded; lands in a future sweep after Cerebro's A1 commit. Committed at `<SHA-OF-THIS-COMMIT>`. |
```

If the `--amend` approach is taken, the pre-commit checklist's `git status clean` step in §6 applies after the amend, not after the initial commit.

---

## 3. Existing entries to update

### 3.1 WEDGE-INV-6 dispatch — flip from `active` to `closed`

The existing row:

```
| `DISPATCH-2026-05-08-wedge-investigation-6-direct-qemu-baseline-cerebro.md` | active | WEDGE-INV-6: direct-QEMU baseline without GDB stub, ...
```

Should flip status to `closed` and the description should reference what came of it. Suggested replacement:

```
| `DISPATCH-2026-05-08-wedge-investigation-6-direct-qemu-baseline-cerebro.md` | closed | WEDGE-INV-6: direct-QEMU baseline without GDB stub, 200 boots, disambiguated the WEDGE-INV-5 confound. Outcome drove WEDGE-INV-7 (cargo-path GDB-stub-via-xtask) and WEDGE-INV-8 (NMI watchdog). Diagnostic only; no direct commit. Findings folded into the WEDGE-INV-7 and WEDGE-INV-8 finding artifacts. |
```

(If a separate `WEDGE-INV-6-FINDINGS-2026-05-08.md` artifact exists in `Plans/`, add it as its own row in §2.2 with status `historical`. If it doesn't exist as a standalone file, leave it folded into the dispatch entry as proposed above.)

### 3.2 No other status flips expected

Spot-check the existing table for any other rows whose status no longer matches reality. The PM doesn't expect any others, but Jarvis should sanity-check rows mentioning files referenced by today's commits.

---

## 4. Open Project Commitments section

**Do not modify the wedge-fix-before-SC-3B commitment row in this commit.** L's 300/300 verification provides a plausible discharge condition, but project lead has not made the α/β call yet (continue with ADR-016 A2 implementation vs. discharge on L and defer A2 to Phase 2). Until that call, the commitment row stays as-is.

A future INDEX commit, after the α/β call, updates the row accordingly.

---

## 5. Commit hygiene

Subject:

```
docs: Plans/INDEX.md — sweep through 2026-05-08 governance commits (8a2311d)
```

Body:

```
INDEX sweep covering everything since the last sweep at 708223b:

Six commits on main now indexed (or referenced):
- 05e15c9 (boot: skip Limine boot menu)
- 6edb012 (xtask: optional QEMU GDB stub for investigation use)
- 3b3b4db (xtask: optional QEMU monitor for investigation use)
- 4c6abf4 (sched: prevent heap-lock re-entry in reap zombie drain)
- 8ef8cac (docs: bible.md — Article 8.5 critical-section discipline v1.0.5)
- 8a2311d (docs: ADR-016 IRQ-safe heap allocator and scheduler heap-decoupling)

New artifact-table entries cover the WEDGE-INV-6 through WEDGE-INV-J1
investigation arc, the L wedge fix and verification, the ADR-016 /
Bible amendment ratification, prior-art research notes, and project-
state PM-NOTEs.

Status flip on the WEDGE-INV-6 dispatch row (active → closed).

Wedge-fix-before-SC-3B commitment row not modified — pending project
lead's α/β call (proceed with ADR-016 A2 implementation vs. discharge
on L and defer A2 to Phase 2).

A1 dispatch (DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md)
is in flight (Cerebro working on it now); its INDEX entry will land in
a small follow-up commit after the A1 commit itself, to keep each
commit's INDEX delta clean.

No code changes. Docs-only.
```

---

## 6. Pre-commit checklist

Before `git add Plans/INDEX.md && git commit`:

- [ ] `Plans/INDEX.md` viewed in editor; new entries are alphabetically sorted into the artifact table (the table is roughly ordered by filename — match the existing pattern).
- [ ] All entries follow the existing format `| filename | status | description |` with leading/trailing pipes and aligned where reasonable (the existing table is loose on column alignment; don't reflow existing rows).
- [ ] Status flips applied: WEDGE-INV-6 row shows `closed`.
- [ ] Open Project Commitments section unchanged.
- [ ] `git diff Plans/INDEX.md` shows ONLY the new entries and the WEDGE-INV-6 row update. No incidental whitespace churn elsewhere.
- [ ] `docker compose run --rm neo cargo k` passes with 18-warning baseline.
- [ ] `git status` shows only `Plans/INDEX.md` modified.

---

## 7. Post-commit actions

After the commit lands:

1. `Plans/backup-to-synology.ps1` runs. Verify the log shows `Stashes to preserve: 3` and the new HEAD SHA recorded.
2. Notify PM. PM tracks INDEX as caught up through HEAD (will be Jarvis's commit SHA).

---

## 8. Cross-references

- `Plans/INDEX.md` — the artifact being updated.
- `Plans/PM-NOTE-2026-05-08-trust-but-verify-ratification-path.md` §6 — the prior PM enumeration of artifacts to register; superset of items in §2 above (this dispatch is the formal action on that note's queued list).
- `Plans/DISPATCH-2026-05-08-adr-016-A1-irq-safe-heap-cerebro.md` — the in-flight Cerebro dispatch for parallel work; deliberately excluded from this sweep.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose row is left as-is pending project lead's α/β call.

---

**End of dispatch.**

*One commit. `Plans/INDEX.md` only. Roughly 17 new entries plus 1 status flip. INDEX caught up to HEAD `8a2311d`. A1's entry lands separately after Cerebro's A1 commit. PM is on call for any clarification Jarvis needs on the wording of any entry.*
