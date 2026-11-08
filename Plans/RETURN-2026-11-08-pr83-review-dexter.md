<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay 337: #83 (batch-plan stderr) review

**Seat:** Dexter. **Model:** DeepSeek, high. **Answers:** `Plans/DISPATCH-2026-11-08-relays-335-339-reroute.md` §337,
running §332 of `Plans/DISPATCH-2026-11-08-relays-329-332.md`.

**Reviewed:** PR #83 head `9187ff195` ("Merge remote-tracking branch 'origin/main' into claude/batch-plan-stderr");
its only content commit is `d4e092182` ("batch-plan: send ::error:: lines to stderr, so a rejected batch says why").
Merge-base with `origin/main` is `9fba021e1`; current `origin/main` is `0ecb5b117` (it advanced docs-only during the
review — four `Plans/` files). The PR is 2 files, +30/−4: `scripts/batch-plan.sh` and `tools/xtask/src/main.rs`.

**Overall: PASS.** All three refusals now write their `::error::` line to stderr, and stdout carries only the three
`key=value` plan lines. The plan step redirects only stdout to a file, so a failed run's reason reaches the Actions
log on stderr. The new test is green at the head and red on the pre-fix script. Two non-blocking notes: the test is a
source/CI consistency check rather than an execution of a refusal, and `:37`'s fetch-by-SHA has a pre-existing corner
outside this diff.

## Checklist, in the dispatch's order

| # | Dispatch item | Verdict | Enforcing refs (at `9187ff195`) |
|---|---|---|---|
| 1 | Each of the three refusals (`:41`, `:65`, `:88`) goes to stderr; stdout is only the `key=value` plan | **PASS** | `scripts/batch-plan.sh:41,65,88` (`>&2`); stdout writes only at `:104-106` |
| 2 | The CI plan step shows stderr in the log when the script fails | **PASS** | `.github/workflows/ci.yml:286` (stdout redirect only), step block `:284-288` |
| 3 | A test proves a refused batch prints its reason | **PASS**, with a limit note | `tools/xtask/src/main.rs:25845-25862` (assertions `:25848-25861`) |

## §1 — the three refusals, and stdout purity

- **All three go to stderr.** `scripts/batch-plan.sh:41` (base not on `main`), `:65` (a `full-alone` PR in a batch
  of more than one) and `:88` (a member not green on its head) each end in `echo "::error::…" >&2`, immediately
  before their `exit 1` at `:42`, `:66` and `:89`.
- **stdout is only the plan.** The script has exactly nine output statements (`grep -n 'echo\|printf'`): the six
  human-readable/error lines at `:41,49,65,88,91,102`, all `>&2`, and the three unredirected `key=value` lines at
  `:104-106` (`base=`, `members=`, `tree=`). Nothing else reaches the script's stdout: `gate-plan`'s output is
  captured at `:58`, `batch-assemble.sh`'s output is captured and filtered through `sed` at `:100`, and the `gh api`
  outputs are captured at `:54,78-81`. `git`/`gh` failures under `set -euo pipefail` (`:30`) write their own
  diagnostics to stderr.
- **Exercised end to end.** At the head, running the script with a base that is a branch tip rather than on `main`
  (`BASE_SHA=refs/heads/claude/batch-plan-stderr`, a local ref at `9187ff195`) gave **exit 1, 0 bytes on stdout**,
  and on stderr: `::error::base 9187ff19567d186964a04ffacde370d49b8071a3 is not on main`. A second run with
  `BATCH=83` and no `gh` on PATH likewise left stdout empty (`gh: command not found` on stderr), so stdout stays
  clean on a non-refusal failure too.
- Refusals `:65` and `:88` were read statically, not run: both need `gh` (not installed in this tree) and `:65`
  additionally needs a `full-alone` `gate-plan`. Their echo lines are identical in form to `:41`'s.

## §2 — the CI plan step shows stderr on failure

- The `Plan the batch` step (`.github/workflows/ci.yml:271-288`) runs under `shell: bash` with `set -euo pipefail`
  at `:285`. Its first line is `scripts/batch-plan.sh > "$RUNNER_TEMP/batch-plan.out"` at `:286`: **stdout only**
  is redirected; there is no `2>` or `2>&1` on the line. Stderr is inherited, and Actions streams it into the step
  log as it is written.
- The successful-path `cat` lines at `:287-288` run only after the script returns zero (`set -e`), so on a refusal
  the step fails at `:286` and the error line is already in the log. That is exactly the failure mode the commit
  message describes: with stdout in the file and the file never printed on failure, an error on stdout vanished.
- Pass. The redirection is the one the new test pins (`tools/xtask/src/main.rs:25858-25861`).

## §3 — the test, its teeth, and its limit

- **The test.** `batch_plan_errors_reach_the_log` at `tools/xtask/src/main.rs:25840-25862` (fn at `:25845`) reads
  `scripts/batch-plan.sh`, asserts the collected `::error::`/`::warning::` lines are non-empty (`:25852`), requires
  every one to end in `>&2` (`:25853-25855`), and asserts `.github/workflows/ci.yml` still contains the exact
  `scripts/batch-plan.sh > "$RUNNER_TEMP/batch-plan.out"` redirection (`:25856-25861`).
- **Green at the head.** `cargo test -p xtask batch_plan_errors_reach_the_log` at `9187ff195`:
  `1 passed; 0 failed; … 196 filtered out`.
- **Red on the previous script.** Re-running with only the three `>&2` suffixes removed (the pre-fix text) fails at
  `:25854` with `not on stderr:   echo "::error::base ${base} is not on main"` —
  `test result: FAILED. 0 passed; 1 failed`. The test is discriminating for the regression it targets, as the
  commit message claims.
- **Limit (not a fail).** It is a source-and-CI consistency check, not an execution of a refused batch: it would
  still pass if an `echo … >&2` were moved after its `exit 1` (dead code), or if a future error line used single
  quotes (`'::error::…'`), since the filter keys on `"::error::` (`:25850`). The manual run in §1 covers the real
  refusal path. A shell-level test of refusal `:41` (the only one reachable without `gh`/`cargo`) would close that
  gap if a stricter guard is wanted later; it is not needed for this fix.

## Verification performed

- Fetched `refs/pull/83/head` and read both commits in `9fba021e1..9187ff195`; confirmed the diff against the
  merge-base and against current `origin/main` is the same 2 files, +30/−4, with no unrelated hunks.
- Read the full `scripts/batch-plan.sh` at `9187ff195` and inventoried every output statement, the three `exit 1`
  sites and the command substitutions.
- Read the `Plan the batch` step in `.github/workflows/ci.yml` at the head (inputs, `shell: bash`, the run block).
- Ran the new test at the head (green) and against the pre-fix script text (red at `:25854`), both reported above.
- Ran the script's refusal `:41` end to end; captured stdout and stderr separately (0-byte stdout, error on stderr).
- Read the base version of both files (`9fba021e1`) to confirm the diff is only the `>&2` suffixes, the comment
  block `:25-28` and the test; the script's mode is unchanged (`100755`).
- Did not run QEMU: this PR is a shell script plus a host test, with no kernel or `tools/` build-path change beyond
  the test itself. No source change was made by this review.
- RAG health at `http://127.0.0.1:8404/health` was unavailable; used `grep`/`read` throughout. Not a blocker.

## Notes (not checklist items)

- **Pre-existing, outside this diff.** `scripts/batch-plan.sh:37` fetches `BASE_SHA` by value
  (`git fetch --no-tags origin "$base"`). In this tree, `git fetch origin <older-main-sha>` failed
  (`fatal: couldn't find remote ref <sha>`), while the default path — `base` = the `origin/main` tip that `:35-36`
  just fetched — got past `:37` and reached the next step. So a batch dispatched with an explicit *older* `base_sha`
  may exit 128 before the plan (git's fatal is on stderr, so it is not silent). Not introduced or touched by #83;
  flagging it for a separate dispatch.
- **Land note.** This branch contains only this RETURN. The pre-existing untracked `pr59-review/` directory in the
  Dexter tree is not staged and is not part of the branch. PR #83 is based on `9fba021e1`; `origin/main` has since
  moved docs-only to `0ecb5b117`, so the PM's normal rebase/merge check still applies to the PR.
