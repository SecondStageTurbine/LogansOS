# PM-NOTE: ADR-015 implementation dispatch revised in place for post-INDEX-swap pre-flight

**Date:** 2026-05-07
**Type:** Project-history record. Documents the in-place revision of `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md`. Not a new decision; just a record of why the dispatch was edited and what changed.

---

## What happened

Cerebro picked up `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` and ran §2 pre-flight. The expected `git log --oneline -2` text named `0b5ffb4` and `9505619` as the top two commits. Actual main HEAD was `afa4d22` (Jarvis's INDEX.md sweep), with `0b5ffb4` and `9505619` underneath. Cerebro stopped per dispatch §0 standing instruction 4 ("Stop and surface if anything is unexpected"). Right call.

The mismatch was benign: `afa4d22` is doc-only (`Plans/INDEX.md`, 1-file delta of +29/-4 lines), landed at 14:52 today, and was already accounted for in the project's broader state (referenced in the EOD handoff and elsewhere). The dispatch was written at ~15:04 — *before* the INDEX swap landed — and its expected-log text simply did not get updated when Jarvis's commit landed shortly after. Two-commit pre-flight with stale expected text, in other words.

## Why surgical revision rather than a re-dispatch

The revision is purely textual — the substrate (sub-commit 2 closure, ADR-015 ratification, project local-only, the change set itself) is unchanged. Re-dispatching would imply the *plan* changed, which it did not. In-place revision matches the pattern from `Plans/PM-NOTE-2026-05-07-local-only-posture.md` (yesterday's revision of the sub-commit 2 closure dispatch when the project's local-only posture was codified).

## What changed in the dispatch

Five surgical edits, no code-side changes:

1. **Top-of-file `Revised:` line (line 6).** Records the in-place revision and reason; preserves project-history readability.
2. **Substrate paragraph (line 8).** Mentions `afa4d22` as current `main` HEAD alongside `0b5ffb4` (sub-commit 2).
3. **§2 pre-flight expected-log block (lines 40–45).** Was `git log --oneline -2` showing `0b5ffb4` / `9505619`; is now `git log --oneline -3` showing `afa4d22` / `0b5ffb4` / `9505619`.
4. **§7 post-commit verify (lines 577–582).** Was `git log --oneline -3` with three lines; is now `git log --oneline -4` with four lines (the new ADR-015 commit on top of `afa4d22`).
5. **§7 commit-message `Refs:` line (line 570).** Now names `afa4d22 (INDEX.md sweep, parent commit)` explicitly so the commit history's lineage is self-documenting.
6. **§8 close-report template `Parent:` line (line 598).** Names the full parent chain `afa4d22 → 0b5ffb4 → 9505619`.

The file's body — the 6-file change set, the §3 implementation prescriptions, §4–§6 verification, §9 "what you do NOT do", §10 time budget, §11 rationale — is untouched.

## What did NOT change

- The change set (six files: `kernel/src/mm/mod.rs`, `kernel/src/mm/heap.rs`, `kernel/src/test_harness.rs`, `kernel/Cargo.toml`, `tools/xtask/src/main.rs`, `docs/ADR-015-kaslr-tolerant-heap-placement.md`).
- The verification gates (build clean, two single-run scenarios PASS, 50-run KASLR loop with 0/50 panics).
- The closure-gate evidence threshold (50-run loop is the load-bearing claim; two consecutive PANIC → STOP).
- The schedule per ADR-015 ratification Decision 5 (this dispatch lands before SC-3A pre-implementation checkpoint).
- The five ratification decisions per `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md`.

## Lesson, mild

When the project state moves underneath a written dispatch (here: Jarvis's parallel INDEX swap landed between dispatch authorship and Cerebro picking it up), the dispatch's pre-flight expected-text is the canonical spot for the mismatch to surface. Cerebro's "stop and surface" discipline caught it cleanly. The fix is a small in-place revision; no PM decision was required, just the housekeeping of keeping the dispatch's expected state in sync with the actual repo state.

The same pattern showed up yesterday with the local-only revision. Both are routine and not signs of process breakdown — they're the cost of running parallel work streams (Cerebro + Jarvis) on a fast-moving substrate. Worth a re-read in five days when memory of the saga has faded.

## Cross-references

- `Plans/DISPATCH-2026-05-07-adr-015-implementation-cerebro.md` — the revised dispatch.
- `Plans/PM-DECISION-2026-05-07-adr-015-ratification.md` — the five ratification decisions this dispatch implements.
- `Plans/PM-NOTE-2026-05-07-local-only-posture.md` — the prior in-place-revision precedent.
- `afa4d22` — Jarvis's INDEX.md sweep commit (the parent that the revision now reflects).
- `0b5ffb4` — sub-commit 2 closure (sub-grandparent).

---

**Note recorded.** Cerebro can resume from §2 pre-flight on the revised dispatch. Expected-log text now matches actual `main` HEAD.
