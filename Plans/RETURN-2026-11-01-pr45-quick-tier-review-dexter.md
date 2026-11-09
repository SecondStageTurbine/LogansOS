<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 260 — review of #45 (OT-58 S2a, the quick tier in CI)

**Reviewed head:** `c4cffb0f2fd2788d80e2410cee44cb77ffe72e17` (`pull/45/head`, branch
`claude/ot58-s2-quick-tier`).
**Base:** `origin/main` at `a610ae8b58bfc506706dea8067103ab68bf7d8a3`.
**Model / effort:** DeepSeek, high.

Read: `docs/THINK-2026-11-01-ot58-tiered-gate.md` (revision 2), the full
`git diff origin/main...pr45`, the whole `ci.yml` at the head, and the S1 xtask code and host tests.
Ran: `cargo test -p xtask gate_plan` (4 passed), `cargo test -p xtask
smoke_scenarios_are_measured_and_within_the_cost_rule` (1 passed), `cargo test -p xtask
quick_tier_adds_changed_scenarios_within_the_budget` (1 passed), and `cargo xtask gate-plan --base
origin/main` at the reviewed head. No QEMU run: this is a review, not a gate.

## Checklist

1. **PASS — the smoke set is exactly THINK 2.2 and the host checks are unchanged.**
   `SMOKE_SCENARIOS` has the same 31 names, grouped the same way (`tools/xtask/src/main.rs:9648-9688`);
   every one has a timing row, every row is at most 35 s wall (`wait_any_core` is the largest at
   33.9 s; sum 461.1 s), and the host test enforces the count, default-suite membership,
   non-hardware-only, and the 35 s rule (`:22003-22048`). `host-checks` itself is untouched
   (`.github/workflows/ci.yml:593-614`): fmt, workspace tests, workspace clippy, plain + neo-test
   kernel clippy, and shard coverage all still run. `--tier quick` appends a diff's eligible changed
   scenarios under the same budget (`:9717-9755`, `:8722-8738`), and the 12-minute
   `timings.tsv` budget is a real red (`ci.yml:427-437`). Note, not a fail: `user_copy_fault` has
   no timing row yet, so it is not in the list; THINK 2.2 makes its addition conditional on a
   measurement.

2. **FAIL — the quick job never reads `gate-plan`, so a `full-alone` diff still runs the quick tier.**
   `ci.yml:418` tees `tier=...` to `target/neo-test/gate-plan.txt`, then `ci.yml:419` runs
   `exec cargo neo-test --tier quick ...` unconditionally. No later step reads `tier=full-alone`
   (the only `gate-plan` references in the workflow are the comment at `:267` and the command at
   `:418`). `cmd_gate_plan` prints `tier=full-alone` and returns `Ok(())`
   (`tools/xtask/src/main.rs:10190-10196`), so the exit status cannot signal it, and the `| tee`
   pipeline would mask a non-zero status anyway without `pipefail`. `cargo neo-test --tier quick`
   also does not check the tier itself (`:8722-8738`). **Concrete input:** at the reviewed head,
   `cargo xtask gate-plan --base origin/main` prints
   `tier=full-alone` for core paths `.github/workflows/ci.yml`, `tools/xtask/data/hosted-timings.tsv`
   and `tools/xtask/src/main.rs`, and exits 0; the head's PR workflow would nevertheless run
   `QEMU smoke (quick tier)` with the smoke set (31 plus any eligible changed scenarios), not the
   4-shard full suite. That contradicts THINK 2.1 ("A job whose plan says `full-alone` stops at
   once with a notice") and the 2.3 fail-safe at the point where it matters. The S2b batch plan
   re-checks the tier, so a full-alone PR cannot enter a multi-PR batch, but the quick job itself
   still runs and the "stops at once" requirement is unimplemented. Fix: write the plan to
   `$GITHUB_OUTPUT` (its formatter already says it is for that, `:10201-10215`), stop the quick
   job with a `::notice` on `full-alone` (or run the full tier), and make `gate-plan`'s exit
   status honest or use `set -o pipefail`. Also have `cmd_neo_test` refuse a full-alone diff when
   `--tier quick` is requested locally.
   - **Secondary:** `SCENARIO_ONLY_EXCEPTION` is `true` (`:9701`), while THINK 2.3.1 says S1
     builds **without** the exception until the PM answers question 3 (`THINK:225`, questions at
     `:395`, `:417`). No answer is recorded on `main`. **Concrete input:** a diff that only adds a
     new `TestScenario` block and gated harness lines gets `tier=quick`; without the exception it
     is full-alone. If the PM has answered question 3, record it; otherwise set the constant to
     `false`.

3. **PASS, with a sequencing note — the full gate still runs for the other seats.**
   `workflow_dispatch` defaults to `tier: full` (`ci.yml:7-15`), the matrix is `[1,2,3,4]` for a
   dispatch (`:121-124`), and `neo-test` still runs on dispatch/schedule with the 4-shard matrix
   (`:156`, `:189-193`). `host-checks`, `product-build`, `spdx-lint`, `monolithic-identity` and
   the feature matrix are unchanged. Note: `needs` changed from
   `[nightly-skip, host-checks, product-build]` to `[nightly-skip]` (`:132`), so a full dispatch
   now starts the 4 QEMU shards before host/product finish; a red host check burns four QEMU jobs
   and adds concurrent load. No coverage is lost, but the old fail-fast ordering is gone.
   Consider a separate full-gate job (or a plan output) that restores the `needs` for `tier: full`.

4. **FAIL — the job count and free-tier arithmetic do not match THINK section 3.**
   A same-repo PR push now runs **23 job instances**, not the 3 in the THINK: `nightly-skip` (1),
   `neo-test` quick (1), `spdx-lint` (1), `monolithic-identity` (1), `host-checks` (1),
   `decoder-fuzz` (4), `product-build` (1), `feature-gated-coverage` (13)
   (`ci.yml:98,127,529,572,593,780-788,830,1012-1030`). Peak concurrency is about 12 because the
   feature arms are capped at `max-parallel: 2` (`:1028`), but the free tier counts jobs, and four
   seats would be 92 job instances (48 at peak) against the 20-job account-wide limit. THINK 3
   says one seat's quick tier is 3 jobs, four seats are 12, and four seats plus a K=6 batch is 20.
   `main`'s push run is also not folded to 2 jobs: it runs 22 job instances (the QEMU job is
   excluded on push). This is also J.c.'s stated requirement in
   `Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`: "Target 3-5 jobs, about 15 minutes."
   **Concrete input:** open or push any same-repo PR; the 23-job set above runs, including the 13
   feature arms even when `gate-plan` says `full-alone`. Fix: fold `product-build`, SPDX,
   identity and the 13 feature arms into one `Quick: build-checks` job sharing one target dir
   (as THINK 2.1 specifies), gate the feature matrix to the full tier, name the check-runs
   `Quick: host-checks`, `Quick: build-checks`, `Quick: smoke` as THINK 2.4 specifies (and update
   S2b's `batch-plan.sh`, which currently looks for `QEMU smoke (quick tier)`), and fold
   `main`'s push run to host-checks + build-checks. The stacked `claude/ot58-s2b-batch` branch
   adds batch wiring but does not fold the matrix either, so this is not just a later-slice
   detail.

5. **PASS — the workflow security surface is not widened.**
   No new write permissions, no `pull_request_target`, no repository secret. The only
   `permissions:` block is `actions: read` on `nightly-skip` (`ci.yml:101-102`), used for the
   existing schedule API call with `github.token` (`:110-111`). Fork PRs are excluded from the
   QEMU job by the `head.repo.full_name == github.repository` guard (`:156`), and `GATE_BASE` is
   derived from `github.base_ref`, i.e. the base repo's branch, never the fork's head ref.
   `feature-gated-coverage` still has no fork guard and `synchronize` now triggers it on every
   fork push (`:1015`), but it uses no secrets and a fork `pull_request` token is read-only, so
   there is no escalation. Defense-in-depth only: put `GATE_BASE` in a step `env:` rather than
   inline interpolation (`:411`), so a future name change cannot reach the shell.

## Verdict

**Not mergeable as S2a while items 2 and 4 fail.** The xtask half is in good shape (the smoke
list, budget, classification and totality tests hold), but the workflow half activates a 23-job
"quick" tier that never consults its own tier filter. The minimum fixes are: (a) make the quick
job stop (or run full) on `tier=full-alone`, with an honest exit status; (b) fold the PR jobs to
the THINK's 3 and the `main` push run to 2, with the `Quick:` check-run names. Item 2's secondary
exception flag needs the PM's question-3 answer either way.
