<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 375: #95 (relay-341 notes on #82, folded in)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:**
`Plans/DISPATCH-2026-11-09-relay-366-and-seat-c.md` §375.

**Reviewed:** PR #95, branch `claude/neofs-cache-index-notes`, head
`5fe0eb139680b55f4ef3097d70b3cabb98821b33`. The PR's own change is one commit,
`0a9cc68a4` ("neofs cache index: Dexter's relay-341 notes (wording and THINK
accuracy)"); the head only merges `origin/main` at `6975f72ee` into it. Against
the merge-base `6975f72ee`, the diff is two files, **+15/-11**:
`docs/THINK-2026-11-08-neofs-cache-index.md` (+14/-10) and
`kernel/src/test_harness.rs` (+1/-1). `origin/main` at review time is
`c09f53503`; the only later commits touch `Plans/`, so they do not affect this
PR.

**Overall: PASS on all seven notes.** Every note from relay 341 is answered at
the line below, and the only code-file change is the one comment line in
`kernel/src/test_harness.rs:1231`. The remainder is THINK wording. A comment
cannot change compiled behaviour, so no build gate is claimed or needed for the
review; the PR's CI is the merge evidence.

| Note | Subject | Verdict | Where answered |
|---|---|---|---|
| 1 | `-off` harness comment should say "index compiled out" | **PASS** | `kernel/src/test_harness.rs:1230-1231` |
| 2 | THINK §9's `phase3_neofs_cache_index_off` table row still carried the old 40.0 s figure; name `b1b49a31` | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:229` |
| 3 | "No cfg is active" is wrong; say "no index code is active" | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:206-209` |
| 4 | `forget_range` no-evict is `retain` + `rebuild`, i.e. O(n log n), not O(n) | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:71-74` |
| 5 | Release duplicate path keeps lookups correct but the length invariant is debug-only | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:51-55` |
| 6 | `removed` signature is `removed(entries, slot, block)` | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:88-90`; code `kernel/src/fs/neofs/cache_logic.rs:102` |
| 7 | Label the 92 s arm as measured at `b1b49a31`, not the head's run | **PASS** | `docs/THINK-2026-11-08-neofs-cache-index.md:187` |

## Per-note detail

1. **PASS — the harness comment now matches the other three.** The old text was
   "or (`-off`) with every lookup by scan"; it now reads "or (`-off`) with the
   index compiled out (the previous layer)" (`test_harness.rs:1230-1231`). That
   is the wording I asked for, and it is consistent with `kernel/Cargo.toml`'s
   off feature, `cache.rs`'s gating comment and `tools/xtask`.
2. **PASS — the stale 40.0 s row is qualified and names its commit.** The row is
   now "40.0 s at `b1b49a31`, where it was scan lookups with the index still
   kept; the true off leg since `cc04384f` is 40.4, 38.4 s"
   (`THINK:229`). The stale number can no longer be read as the current off-leg
   time, and the true pair is in the same cell. This is exactly the fix relay
   341 asked for.
3. **PASS — "no index code is active" replaces the false cfg claim.** §9 now
   says `neofs-cache-index` is still enabled but every use is excluded at
   compile time (`THINK:206-208`), which is true at the head: every index use is
   gated by `all(feature = "neofs-cache-index", not(feature =
   "neo-test-neofs-cache-index-off"))` (e.g. `cache.rs:143-149,414-424,826-832`).
   It no longer claims the cfg itself is unset.
4. **PASS — the `forget_range` cost sentence now names the rebuild.** §3's
   "Removal stays O(n)" now adds that the no-evict path goes from `retain` (O(n))
   to `retain` plus `rebuild` (O(n log n)), rare, with the §9 measurements still
   ahead (`THINK:71-74`). `rebuild` does sort and dedup
   (`cache_logic.rs:121-133`), so the complexity is stated correctly.
5. **PASS — the release/debug distinction is stated as it is.** §2 now says a
   release duplicate still gets the scan's first-match lookup, that the
   one-entry-per-block length invariant is debug-checked only by design, and
   that the miss-before-push rule makes the path unreachable
   (`THINK:51-55`). That matches `pushed`'s first-slot behaviour
   (`cache_logic.rs:79-93`), the `debug_assert!` in `check_index`
   (`cache.rs:834-856`), and the model KAT.
6. **PASS — the THINK signature now matches the code.** §4 says
   `removed(entries, slot, block)` (`THINK:88-90`), and that is the definition at
   `cache_logic.rs:102`. The extra `entries` slice is what lets the duplicate
   fallback find the next first slot (`cache_logic.rs:102-118`), so the THINK
   now describes the API exactly.
7. **PASS — the timing arm is labelled with the commit it was measured at.** The
   table row now says "index with the incremental debug checks, measured at
   `b1b49a31`" and adds that the build PR's head `c30975a3` merged main's
   unrelated code (`THINK:187`), instead of implying the 92 s is a head
   measurement. One precision point, non-blocking: `cache.rs` did gain the
   `-off` cfg gates at `cc04384f` between `b1b49a31` and `c30975a3`
   (`git diff b1b49a31 c30975a3 -- kernel/src/fs/neofs` is `cache.rs` +33/-10,
   all `all(feature, not(off))` gates and comments; `cache_logic.rs` is
   unchanged). Those gates are inactive in the measured index-on arm, so the
   compiled index code for the 92 s boot is unchanged and the label remains
   correct. If the sentence is ever tightened, "the index-on code is unchanged"
   would be the fully literal form.

## The only code change is a comment — PASS

`git diff --stat 6975f72ee 5fe0eb13` is exactly:

```
 docs/THINK-2026-11-08-neofs-cache-index.md | 24 +++++++++++++----------
 kernel/src/test_harness.rs                 |  2 +-
 2 files changed, 15 insertions(+), 11 deletions(-)
```

The `test_harness.rs` hunk is one comment line:

```diff
-// or (`-off`) with every lookup by scan.
+// or (`-off`) with the index compiled out (the previous layer).
```

There is no executable change: no `cfg`, no logic, no feature, no test change.
The docs file keeps its `CC-BY-4.0` SPDX line and the source file keeps
`MPL-2.0`.

## Verification performed

- `git fetch origin +pull/95/head:refs/pr/95`; confirmed head `5fe0eb13` and
  that its first-parent notes commit is `0a9cc68a4`.
- `git diff --stat`/`--name-status` from the merge-base `6975f72ee`; read the
  full `0a9cc68a4` diff, the full §2/§3/§4/§9 of the THINK at the head, and the
  changed harness comment in place (`test_harness.rs:1230-1231`).
- Checked each answer against the code at the head: `cache_logic.rs:79-93`
  (`pushed`), `:102-118` (`removed`), `:121-133` (`rebuild`), and
  `cache.rs:143-149,414-424,798-806,826-856,874-886,932-941` (the gated uses
  and `check_index`).
- `git diff --check 6975f72ee refs/pr/95`: clean (exit 0). SPDX present in both
  changed files.
- No build or test run: the only code-file change is a comment and the rest is
  `docs/`. #95's CI (paths-ignore for `docs/**` and `*.md`) plus the PM's merge
  check are the merge evidence; running a host build here would not exercise
  any changed line.
- RAG: not available in this environment; the review used `git show`, `git
  grep` and direct reads, which is sufficient for a two-file wording change.

## Verdict

**PASS, all seven relay-341 notes answered at `5fe0eb13`.** The only code
change is the one comment line in `test_harness.rs`; every other edit is THINK
wording that now matches the code, and the timing table is correctly anchored
to `b1b49a31`. The one precision caveat in note 7 does not change the measured
index-on code or the label.
