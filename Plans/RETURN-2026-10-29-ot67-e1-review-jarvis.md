<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-67 E1 editor core, relay #155 / #196

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/review-bundle-2026-10-29c-jarvis`. **FAIL.**
Read squash `57f23455` and all twelve PR checklist items. This crate and
Unicode fixture are unchanged from that squash to review main `87458b7e`.
Paths below are under `userland/neo-edit-core/src/` unless qualified.

1. **PASS.** `gap.rs:98` validates insert offsets; delete uses validated
   slice boundaries (:111). move_gap (:119) uses overlap-safe copy_within;
   reserve (:136) preserves the tail when growing. Both exposed UTF-8 halves
   stay valid. Exhaustive boundary insert/delete and growth tests pass.
2. **PASS.** `lines.rs:52` shifts later starts and splices newline starts;
   :66 removes starts whose preceding newline was deleted and shifts survivors.
   The randomized fresh-scan index oracle passes.
3. **PASS.** `document.rs:286` is the common post-construction buffer/index
   mutation path; undo (:302) replaces inserted with removed bytes and redo
   (:309) reapplies the exact edit. Their selection snapshots are restored.
   Seeded document runs, undo-all and redo-all pass.
4. **PASS for the stated removed+inserted byte bound.** `history.rs:112`
   removes redo tail, records/merges, then trims oldest records until
   bytes<=cap (:126). Saved position moves with trimming or becomes None;
   oversized single edits cannot remain in history. This payload-byte cap
   does not purport to count VecDeque/String metadata or capacity overhead.
5. **PASS.** Save/move/undo/redo break open runs (`history.rs:102`, :108,
   :194, :204; `document.rs:129`). A saved position is never merged across
   (:138). Typing's whitespace-ending check (:149) ends the preceding word
   when a new non-whitespace insertion arrives. Existing coalescing tests pass.
6. **FAIL — movement is UAX #29, but edit results can leave the caret inside
   a grapheme.** `cursor.rs:42`, :50 use GraphemeCursor, and the document's
   previous-line case (:198) handles CR LF as one cluster. The 766-case
   movement/snap corpus passes. However `document.rs:269` sets the after
   caret to range.start+insert.len() without considering newly joined clusters.
   Executed input: load **U+0301**, type **"a"** at offset zero. Text becomes
   `a\u{301}`; caret is byte **1**, while `snap(1)` is **0** and the valid
   cluster boundaries are 0 and 3. An assertion head==snap(head) fails.
   Undo/redo also retain the invalid after snapshot. Add edit-seam cluster
   cases, not only movement through an already-fixed text.
7. **PASS.** `tests.rs:388` pins Unicode version/header/length/FNV and case
   count. Independently computed SHA-256 of
   `third_party/unicode-ucd-17/GraphemeBreakTest.txt`:
   `e2d134d2c52919bace503ebb6a551c1855fe1a1faec18478c78fff254a1793ec`,
   matching VENDORED.md. The installed dependency's tables declare (17,0,0).
8. **FAIL for normalization across edit boundaries; PASS for load/save
   oracles.** `format.rs:50` rejects invalid UTF-8/NUL/>4MiB; :62/:96 preserve
   BOM and homogeneous LF/CRLF bytes, normalizing mixed input as specified.
   The round-trip tests pass. But `document.rs:229`, :234 normalize only the
   new string. Executed input: load lone **CR**, move to document end, type
   **LF**. `Document::text()` contains **CR LF**, violating the claimed
   LF-only internal line-ending invariant. The added assertion fails.
   This also affects paste and joins between old/new text. Validate the edit
   seams while preserving the explicit lone-CR round-trip contract.
9. **PASS.** `document.rs:335` checks each candidate's char boundary before
   byte equality/ASCII folding. Valid UTF-8 query equality cannot end inside
   a code point. Forward/backward candidate chains (:346) visit each possible
   start at most once with the documented wrap rule. Hand cases pass.
10. **PASS within the Measure contract.** `wrap.rs:26` partitions the line,
    hangs trailing spaces, and permits one overwide grapheme rather than an
    empty row. Cache edits (:100) splice exactly affected old/new lines;
    width change invalidates all. Vertical motion (:150) clamps at document
    ends and selects the nearest boundary at the retained goal x. Hand and
    randomized wrap/cache/motion tests pass. Measure must provide ordered
    valid boundary offsets as its documented contract requires.
11. **PASS.** `lib.rs:13` is no_std outside tests. Host `cargo check --lib`
    succeeds with the dependency's default features disabled. Only this
    userland crate adds unicode-segmentation; kernel/Cargo.toml names neither
    dependency and no app/image yet links the new core.
12. **PASS.** The prior-art row exists in #196. Inspected the downloaded
    official 1.13.3 crate: Cargo.toml declares MIT OR Apache-2.0 and no normal
    dependencies, lib.rs is unconditionally no_std, tables.rs declares Unicode
    17. The lockfile adds one dependency-free unicode-segmentation package.
    GitHub resolves `afe6c8e63f62` to
    `afe6c8e63f627b09b0cb26801fc9c934d06e41d6`, timestamp
    2026-10-27T00:46:53Z (Oct 26 in the project's local timezone).
    The vendored data's LICENSE is Unicode License v3. Upstream v-prefixed
    raw release URLs returned 404, so release claims were checked against the
    actual Cargo-downloaded crate, not guessed from those URLs.

Executed on `87458b7e`: `cargo test -p neo-edit-core --locked --quiet`
(23 passed), `cargo check -p neo-edit-core --lib --locked --quiet` (passed),
fixture SHA-256 check, then two temporary property tests for items 6/8
(both failed with the exact outputs above). Original test bytes were restored.
Independently queried
[batch 16](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40843436986):
success at `fb039fbdd6b925e4aaf346025e6789f34e0cd908`. RAG healthy.
No implementation fix is included. Keep these edit-boundary failures visible
before E2 relies on the core's cursor/normalization invariants.
