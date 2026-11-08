<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 338: #73 / #82 item-5 re-check (the true off leg)

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §338.
**Reviewed:** PR #73 head `4b2628c0` (`docs/THINK-2026-11-08-neofs-cache-index.md`) and PR #82 head `c30975a3`
(the cache/index code is at `cc04384f`; `git diff cc04384f7..c30975a3 -- kernel/src/fs/neofs` is empty — the
later commit only merges `origin/main`). Against `origin/main` `0ecb5b117`.
**Overall: PASS.** Relay 326's item-5 FAIL is fixed: the `-off` scenario now builds the previous layer, the two
timings compare the same workload on the same machine, and §9's conclusion follows. Three wording fixes are
listed at the end; none changes the build or the verdict.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing lines |
|---|---|---|---|
| 1 | The `-off` leg compiles the index out: no `neofs-cache-index` cfg active, `check_index` doesn't run | **PASS** on the property (one literal correction) | `kernel/Cargo.toml:81,1728`; `cache.rs:146-150,224-228,417-420,436-439,799-820,829-832,877-880,935-938` |
| 2 | The two timings compare the same image and workload, with the index the only difference | **PASS** (two builds, not one image — see the note) | `THINK:202-211`; `kernel/Cargo.toml:1728`; `test_harness.rs:34442-34450` |
| 3 | §9's conclusion follows from the new numbers, and the leg is named for what it is | **PASS** (one stale comment; one loose phrase) | `THINK:198-214,225`; `kernel/Cargo.toml:1722-1723`; `cache.rs:793-796`; `tools/xtask/src/main.rs:6039-6043` |
| 4 | The PR #73 body no longer recommends the `BTreeMap` | **PASS** | PR #73 body, head `4b2628c0` |

## 1. PASS — the off leg is the previous layer, with one literal wording correction

The fix is the `not(feature = "neo-test-neofs-cache-index-off")` conjunct on every index gate:

- the field and its initialiser: `cache.rs:146-150`, `224-228`;
- `forget_range`'s rebuild and remove paths: `cache.rs:417-420`, `436-439`;
- the lookup branch: `cache.rs:799-820` (the fallback is the scan);
- `check_index` itself, so it does not exist in the off build: `cache.rs:829-832`;
- `push_block`'s upkeep + check: `cache.rs:877-880`;
- `evict_locked`'s removal + check: `cache.rs:935-938`.

The scenario enables `neo-test-neofs-cache-index-off` (`kernel/Cargo.toml:1728`), which is the feature the gates test.
I built the kernel at `c30975a3` with `--features neo-test,neo-test-neofs-cache-index-off`: it succeeds, and the ELF
has no `BlockIndex`, `check_index` or `cache_logic` symbols and no "neofs cache index" assertion strings.

The literal clause "no `neofs-cache-index` cfg is active" is not true, though. `neofs-cache-index` is still in
`default` (`kernel/Cargo.toml:81`), and `neo-test-neofs-cache-index-off` only depends on the evict scenario feature
(`:1728`) — it cannot turn another Cargo feature off. The `-v` build passes both
`--cfg 'feature="neofs-cache-index"'` and `--cfg 'feature="neo-test-neofs-cache-index-off"'`. What the fix does is
exclude every *use* with the second cfg, not disable the index feature.

That is not just cosmetic: I built the true previous layer at the same head (`--no-default-features` plus
`kernel-diagnostic`, `accel-sim`, `inference-preemptible`, `user-copy-fixup`, `neo-test`,
`neo-test-phase3-neofs-cache-evict`) and compared the two kernels:

- `.text` is the same size in both (`0x2ebaa4`) and the defined-text symbol sets are identical; neither has
  `BlockIndex`/`check_index` symbols;
- the remaining section deltas are small (`.rodata` +0x40, `.data` -0x20, `.bss` +0x10, plus debug metadata), which
  is the differing cfg set and scenario-name constant, not index codegen.

So the `-off` scenario is the previous layer in the emitted code. The wording should say "no index code is active"
or "the index is compiled out", not "no `neofs-cache-index` cfg is active"; alternatively build the scenario with
`--no-default-features` so the feature really is absent. Either way, `check_index` does not run.

## 2. PASS — same workload and machine; the two arms are necessarily two kernels

The off feature transitively enables `neo-test-phase3-neofs-cache-evict` (`kernel/Cargo.toml:1728`), so the off
scenario's `cfg!(feature = "neo-test-phase3-neofs-cache-evict")` is true: it runs `run_neofs_cache_evict` with
`FILES = 40` and `evict_on = true`, exactly `phase3_neofs_cache_evict`'s legs (`test_harness.rs:34442-34450`).
The only feature difference between the two arms is the off flag (the scenario-name constant is the only other
source delta), and THINK:206-211 says the runs are interleaved on the same machine.

"Same image" is loose: a compile-time feature necessarily produces two kernels, and these are two `TestScenario`
builds. What the pair establishes is "same source tree and build configuration, the index the only difference",
which is the property the timing needs. Since `cc04384f`, that is true; at `b1b49a31` it was not, which is the
defect relay 326 found. A same-image runtime switch was the other option in that RETURN; the author took the
true-off build, which was the primary recommendation.

## 3. PASS — the conclusion follows; two stale labels

The new pair is off 40.4 / 38.4 s against index-on 37.9 / 38.3 s (THINK:208-211). The index arm is never slower
(mean 38.1 s against 39.4 s), and the off arm's own 2.0 s spread is the size of the run-1 delta. For this cache
(≤256 clean blocks), "the index, upkeep included, costs no more than the scan" and "no size threshold is needed"
follow (THINK:213-214). The sample is only two runs per arm, so the "within run-to-run noise" sentence rests on
that spread; it is a fair reading, not a statistical claim.

The leg is named correctly in the build sources: "the kill-switch leg ... with the index compiled out entirely (no
upkeep, no checks: the previous layer)" (`kernel/Cargo.toml:1722-1723`), `cache.rs:793-796`, and
`tools/xtask/src/main.rs:6039-6043`. Two leftovers:

- `test_harness.rs:1200-1201` still says "or (`-off`) with every lookup by scan" — true but incomplete now; say
  "with the index compiled out", as the other three comments do.
- THINK:225's row still carries the old `phase3_neofs_cache_index_off` = 40.0 s figure and labels it "index still
  kept" without naming `b1b49a31` in the same cell; the parenthetical does give the new 40.4/38.4, but the row
  reads as current. Name the older head in the row, as THINK:186-187 already does for the 37.2 s figure.

Neither affects the conclusion.

## 4. PASS — the PR #73 body describes the sorted `Vec`

Fetched the PR body at head `4b2628c0` (GitHub API, `updated_at` 2026-11-08T11:39:25Z). It says: "The index is one
`Vec<(block, slot)>` sorted by block, not a `BTreeMap`. The `BTreeMap` version was tried and discarded." It also
records the insert cost, the unchanged cache order, the lock order, the one-entry invariant, the incremental checks,
and the kill-switch. The old recommendation is gone; `BTreeMap` appears only as the discarded alternative and in
option C's description.

## Verification performed

- Read the THINK at `4b2628c0`; read `cache.rs` / `cache_logic.rs` / `test_harness.rs` / `xtask` at `cc04384f` and
  `c30975a3`; confirmed `git diff cc04384f7..c30975a3 -- kernel/src/fs/neofs` is empty.
- `git fetch origin +pull/73/head:refs/pr/73 +pull/82/head:refs/pr/82`; PR #73 head `4b2628c0`, PR #82 head
  `c30975a3`. Read the PR #73 body from the GitHub API at that head.
- Built at `c30975a3` in a temporary worktree: the off scenario (`--features neo-test,neo-test-neofs-cache-index-off`)
  and the true previous layer (`--no-default-features` + the five defaults + the evict scenario). Both succeed; the
  symbol and section comparison is above. This is a compile check, not a QEMU run.
- Did not re-run the timings; the numbers are the author's, and this re-check is about their provenance and the
  build they describe.
- RAG: no response at `http://127.0.0.1:8404/health`; used grep/read, not a blocker.
- Working tree: only this RETURN on `dexter/pr73-recheck2`; the pre-existing untracked `pr59-review/` directory is
  untouched and not committed. The temporary `c30975a3` worktree was outside the repo and is not part of this
  branch.

## Recommendation to the PM

**PASS — the relay-326 item-5 defect is resolved.** The off leg is the previous layer in the emitted code, the
workload and machine are the same, and §9's conclusion follows. Merge #73/#82 on this evidence. Optional docs-only
cleanups, none blocking: (a) replace "no `neofs-cache-index` cfg is active" with "no index code is active" (or build
the off scenario with `--no-default-features`), (b) say "same source/build apart from the index" instead of "same
image", and (c) update `test_harness.rs:1200-1201` and the THINK:225 row.
