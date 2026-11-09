<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 271 — #45 re-check and #47 review (OT-58 S2)

**Reviewed heads:**
- #45 `e4dd26e2ac35a3045b6bca1266c2cf9d3af790d3` (`origin/claude/ot58-s2-quick-tier`).
- #47 `7c93227139c0e1ce98b6d855251a6f296019f9db` (`origin/claude/ot58-s2b-batch`, stacked on #45; the
  #45-only diff is `e4dd26e2..7c932271`).

**Base:** `origin/main` at `792f0bb4558b4b002bc8e0e4ecfbab6b2657c59c` (the relay-271 dispatch).
**Model / effort:** DeepSeek, high.

Read: the full `e4dd26e2..7c932271` diff, the two heads' whole `ci.yml`, `scripts/batch-plan.sh`,
`scripts/batch-assemble.sh`, `scripts/revert-batch.sh`, `.github/actions/assemble-batch/action.yml`, the
approved THINK (`docs/THINK-2026-11-01-ot58-tiered-gate.md`, revision 2) and the new CLAUDE.md section, plus
relay 260's RETURN (`origin/dexter/pr45-review`).
Ran: `bash -n` on the three scripts; a PyYAML parse of `ci.yml` at both heads; the `batch-plan.sh` jq predicate
on a synthetic job list; `batch-assemble.sh` twice on a synthetic repo (same tree and commit); `revert-batch.sh`
on a synthetic history containing an unrelated docs merge. No QEMU run: this is a review, not a gate. No Rust
changed in the #45 or #47-only diffs, so the S1 `gate-plan` host tests from relay 260 still cover that code.

## 1. #45 re-check

### 1.1 Item 2 — PASS (with two notes)

The plan is obeyed at `e4dd26e2`:

- `nightly-skip` publishes `full` and `shards` (`ci.yml:103-110`).
- The tier step starts `full=true`, runs `gate-plan` only for a PR or a `tier: quick` dispatch, and sets
  `full=false` **only** when the output contains a line exactly `tier=quick` (`ci.yml:140-160`). A `full-alone`
  plan therefore keeps `full=true` and `shards=[1,2,3,4]`; the `neo-test` matrix reads that output
  (`ci.yml:222-228`), so a full-alone diff gets the 4 shards **on the same run**.
- A failed plan fails safe: `plan="$(cargo run ... || echo 'tier=full-alone')"` (`ci.yml:147`); the `||` keeps
  the step alive, the grep cannot match `tier=quick`, and `full` stays true. The only edge is a plan that prints
  `tier=quick` and *then* exits non-zero; the fallback line is appended but the grep still matches the partial
  line. That is narrow and not a realistic `gate-plan` path (it prints only at the end), but a `tail -n 1` or a
  check of the command status would close it.
- The full-only jobs follow: `rng-seed-boot` and `decoder-fuzz` are gated on `full == 'true'`
  (`ci.yml:571-575`, `:909-920`), so a quick PR skips them and a full-alone PR runs them.

Notes:

1. The QEMU job's container recomputes `cargo xtask gate-plan` and pipes it to `tee` (`ci.yml:503-505`), but
   nothing consumes the result, and the `bash -c 'set -e'` block has no `pipefail`. The matrix decision is the
   plan job's, so the requirement is met; but if `main` moves between the two jobs the container can disagree
   and still run the quick tier. Use the plan output as the single source of truth (pass it into the job) or
   check the container's plan too.
2. `SCENARIO_ONLY_EXCEPTION` is still `true` (`tools/xtask/src/main.rs:9728`), while THINK 2.3.1 says S1 builds
   **without** the exception until the PM answers question 3. No answer is recorded on `main`. This is relay
   260's secondary finding and neither #45 nor #47 changes it. Record the answer, or set the constant false.

### 1.2 Item 4 — FAIL / still open

At `e4dd26e2` a same-repo PR runs **7 job instances** on the quick tier, not the THINK's 3:

| Run | Job instances |
|---|---|
| Quick PR | 7: `nightly-skip`, `neo-test` quick, `spdx-lint`, `monolithic-identity`, `host-checks`, `product-build`, `feature-gated-coverage` |
| Full-alone PR | 15: the 7 above with 4 shards instead of 1 smoke (+3), plus `rng-seed-boot` (+1) and the 4 `decoder-fuzz` jobs (+4) |
| `main` push | 11: `nightly-skip`, `spdx-lint`, `monolithic-identity`, `host-checks`, `rng-seed-boot`, 4 `decoder-fuzz`, `product-build`, `feature-gated-coverage` (`neo-test` is dispatch/schedule/PR only, `ci.yml:191`) |

THINK section 3 at this head says one seat's quick tier is 3 jobs, a K = 6 batch is 8, and `main`'s push run is
2. The commit does deliver the **13-cell feature matrix → one job** fold (`ci.yml:1143-1152`) and moves the fuzz
jobs and RNG boot to the full tier; that is the real 13→1 win, and it drops a quick PR from ~23 jobs to 7. But
the remaining fold the THINK specifies — `product-build`, SPDX, identity and the feature configurations in one
`Quick: build-checks` job, and `main`'s push run folded to host-checks + build-checks — is not done. The
free-tier arithmetic is therefore still 28 job instances for four quick seats (16 long-lived), not 12; four seats
plus a K = 6 batch is 24 long-lived and does not fit 20.

#47 does not complete the fold either; it revises THINK section 3 to the as-built 7/6 (see section 4). The PM
must either deliver the 3/2 the THINK was approved with, or explicitly approve the 7-job reality and correct the
THINK and CLAUDE.md. **Item 4 is open at the #45 head.**

## 2. #47 review — the batch tier

### 2.1 Plan and quick-green predicate — PASS, with findings

`scripts/batch-plan.sh` resolves each member's head once (`:44-46`), fetches the PR ref, and requires a completed
successful `CI` run on that exact SHA with a passing `QEMU smoke (quick tier)` job **or** four passing
`QEMU Integration Tests (shard ...)` jobs (`:49-66`). The jq predicate parses and returns true for a smoke job
(checked with a synthetic job list). It then requires `gate-plan` to say `tier=quick` unless the batch is a
single member, which is the full-alone batch-of-one case (`:69-74`). That is the intended shape.

Findings (none fatal by themselves):

- **The accepted evidence is not tied to the tier.** A full-alone member can be a batch of one, and the green
  check accepts a smoke run if one exists on its head; the plan should require the 4 shards when `gate-plan`
  says `full-alone` and the smoke job when it says `quick`. Concrete input: a full-alone head whose green
  `CI` run came from the pre-#45 workflow, where the quick job ran the smoke set unconditionally (relay 260
  item 2's finding); `batch-plan.sh` accepts that run and then allows the full-alone member only because the
  batch is a single member (`:69-74`). The same shape occurs for any `tier: quick` dispatch whose SHA is later
  classified full-alone. This is a fail-open in the member predicate.
- **`base_sha` is unvalidated** (`:31-34`). A typo or a feature-branch SHA makes the batch tree from a non-main
  base, and the merge report would name it. Add `git merge-base --is-ancestor "$base" origin/main`.
- `per_page=50` with no pagination (`:50`) could miss a run on a SHA with many runs; low risk, easy to fix.
- A `prefix`-only dispatch fires the plan step (`ci.yml:184`) but `batch-plan.sh` requires `BATCH` (`:28`) and
  exits. Either support `prefix=0` alone or do not fire the plan on a prefix-only dispatch.

### 2.2 Assembly and the tree-hash check — PASS

Every job that checks out code runs the composite action right after the checkout (`ci.yml:349-355`, and the same
block in `rng-seed-boot`, `spdx-lint`, `monolithic-identity`, `host-checks`, `decoder-fuzz`, `product-build` and
`feature-gated-coverage`); the action unshallows, fetches the base and each PR ref, assembles the tree, and fails
unless the tree equals the plan's. `scripts/batch-assemble.sh` uses a fixed author, committer and date and
`--no-ff` merges. I ran it twice on a synthetic repo and got the same tree and commit both times
(`tree=5b9f1e5e...`, `commit=6e101005...`). The plan's tree and every job's tree are checked; good.

### 2.3 Full-tier enforcement — FAIL (a quick batch can masquerade as a full batch)

The `tier` input and the `batch`/`prefix` inputs are independent (`ci.yml:13-31`). Nothing forces a batch
dispatch to `tier: full`. Concrete input: dispatch `CI` on `main` with `tier: quick` and `batch="45 47"`.
`gate-plan` on `main` has no diff, so the tier step sets `full=false` and `shards=["quick"]`
(`ci.yml:176-180`); the container's quick branch runs before the `BATCH` branch (`ci.yml:501-508`), so it runs
`cargo neo-test --tier quick` on the batch tree. `batch-report` then prints a green batch (`ci.yml:1331-1381`).
The run URL looks exactly like a batch gate, but only the smoke tier ran.

This is the most serious finding: the merge bar is “the batch run must be green on the batch tree”
(CLAUDE.md:603), and this run is not the full tier. Fix: when `inputs.batch != '' || inputs.prefix != ''`,
force `full=true` / `shards=[1,2,3,4]`, or fail the run if the selected tier is `quick`.

### 2.4 Prefix bisect — FAIL against THINK 2.5 / F13

`prefix=N` only slices the member list (`batch-plan.sh:36-39`); the matrix is still `[1,2,3,4]` because the tier
step defaults to full, and each shard runs `cargo neo-test --shard "$SHARD/4" --keep-going`
(`ci.yml:507-508`) — all ~449 scenarios, not the failing ones. THINK 2.5 says a prefix run “runs only the failing
scenarios (`cargo neo-test <names>`, in one job)”; CLAUDE.md:623 repeats that (“one job per round, running only
the failing scenarios”). As built each round is four full shards (~82 min at K = 4), not one job. The first
failing prefix still names the owner, so the binary search works, but the cost model and the CLAUDE.md claim are
wrong. There is also no input for the failing scenario names, so the report cannot hand them to the next round.

### 2.5 Batch report / classification — PARTIAL

`batch-report` lists every non-`pass` row and flags a shard with no timing table as infra (`ci.yml:1363-1379`).
It does not classify build failures vs scenario failures vs timeouts, and `--keep-going` writes only `FAIL`, with
no `build` tag (`tools/xtask/src/main.rs:8887-8896`, `:8963-8974`), while THINK 2.5 says S2 records build failures
as `FAIL` rows tagged `build`. The “timeout is re-run once” rule (CLAUDE.md:626) is procedural; no job implements
it or identifies a timeout in the report. The PM can still do both by hand, but they are not “mechanical”.
`missing=$(( 4 - ... ))` is also hard-coded to 4 (`ci.yml:1376`); with S3's K = 6 it will falsely flag two
missing shards as infra.

### 2.6 Landing / “one action from green” — FAIL (the revert over-reverts)

`scripts/revert-batch.sh` selects **every** first-parent merge after the base (`:39-40`) and reverts all of them
(`:46-52`). It does not filter to the batch's merges. Repro (synthetic repo, run today):

```
base d16a241; an unrelated docs-only PR is merged as e6c2c6c;
batch merges 79d5f0d (member 1) and 8d39eed (member 2);
scripts/revert-batch.sh d16a241
```

The resulting commit deleted `d` (the unrelated docs merge's file) as well as `m1` and `m2`. That is not
“reverts the batch's merges”; it can revert another lane's landed merge, which is worse than leaving `main` red
for one action. The batch merge commits have a fixed committer (`OT-58 batch`) and subject `batch: member N`
(`batch-assemble.sh:35-40`), so the fix is to filter on those (or take the batch's landing SHA range). This is a
must-fix for the escaped-defect contract's “one action from green”.

### 2.7 The plan step can mask a failed plan (operational) — FAIL

`Plan the batch` is `run: scripts/batch-plan.sh | tee -a "$GITHUB_OUTPUT"` (`ci.yml:202`). The step has no
`shell: bash`, so GitHub's default `bash -e` (no `pipefail`) applies; the pipeline's status is `tee`'s 0 even
when `batch-plan.sh` exits 1 (`batch-plan.sh:63-65`). The plan job then succeeds with empty
`batch_base`/`members`/`tree`; the four shards and every other batch job start and fail at the assemble action's
`git fetch origin ""` (or an empty base), burning ~15 jobs and looking like infra. Concrete input: dispatch
`batch="45"` when #45's head has no green run. Fix: add `shell: bash` to the step (pipefail), put
`set -o pipefail` before the pipeline, or drop the `tee` and write the outputs directly.

### 2.8 Security note (defense-in-depth)

The batch jobs execute assembled PR code in a `workflow_dispatch` run. Only `nightly-skip` has a `permissions:`
block (`ci.yml:115-118`); the other jobs inherit the repo default token, and `actions/checkout` persists it in
`.git/config`. A batch member's build script could read it. Recommend a top-level `permissions: contents: read`
(or the same per job) for every batch job; none of them needs write. This is not necessarily a blocker if the
repo's default token is read-only, but the batch tier is the first dispatch path that runs PR code, so it should
be explicit.

## 3. CLAUDE.md rule changes vs THINK section 6

| THINK §6 row | Stated? | Matches? | Notes |
|---|---|---|---|
| Merge bar (batch tree; base, heads, tree hash, URL; PM `--stat` per PR) | Yes, CLAUDE.md:603-611 | Yes | Matches §6 (`THINK:362`). |
| Close-gate URL (quick run; batch-of-one for full-alone; batch URL is merge evidence) | Yes, CLAUDE.md:588-589 and 596-599 | Mostly | See the caveat below. |
| AAG (per PR + batch composed diff for 2+ kernel members; cross-family per PR for SMP-core) | Yes, CLAUDE.md:632-635 | Yes | No weakening; the cross-family SMP-core lens stays per PR because those are full-alone. |
| Escaped-defect contract (one revert command, one commit; owner; ~4 h) | Yes, CLAUDE.md:629-630 | Yes in text | The script does not do it correctly (§2.6). |
| QEMU stagger (superseded for hosted; still for self-hosted) | Yes, CLAUDE.md:637-639 | **No** | It says “at most 4 quick tiers” while the revised THINK §3 (`THINK:319`) says at most 2. Four quick tiers × 4 long-lived + a batch's 8 = 24 > 20; the THINK's 2 is the arithmetic-correct rule. Fix this line to 2. |
| Tree identity (everything except `Plans/`, `docs/`, `*.md`) | Yes, CLAUDE.md:641-642 | Yes | Stricter than the old narrow list; the older section above still names the narrow list, but the new text says “not only”, so the broader check wins. |
| Unchanged rules (hermeticity, timeouts, infra-red, cross-family, close-gate script, verify-before-merge) | Not restated | Yes | Nothing in the new section changes them. The close-gate URL replacement is the intended §6 change. |
| The new invariant (nothing lands unless a full-tier run's tree equals `main` outside docs) | Parts, not named | Partial | THINK:378-380 names it; CLAUDE.md states the merge bar and the landing check but never the invariant. One sentence would make the guard explicit. |

**Close-gate caveat.** THINK §6 says the full-alone PR's close-gate evidence is “its batch-of-one run” and “the
batch run URL is the PM's merge evidence” (`THINK:363`). CLAUDE.md:596-599 instead says the full-alone PR gets
the full tier on its own PR run, and “its landing is a batch of one, whose plan accepts the 4 green shards as its
green run”. That is safe only if the PR head is rebase-fresh onto the batch base, because only then does the
assembled tree equal the head tree. Neither `batch-plan.sh` nor CLAUDE.md asserts that. Require the batch-of-one
run on the assembled tree (as §6 says), or assert `git merge-base --is-ancestor <base> <head>` in the plan before
accepting the PR's own run. Otherwise the merge evidence can be a full-tier run on a stale tree.

## 4. The THINK section 3 revision at the #47 head

The head commit (`7c932271`) replaces the approved 3/2 design with 7/6 and calls it measured. The 7 is consistent
with the code. The 6 is not:

- **`main` push runs 11, not 6.** The tier step sets `full=true` on a push (it starts true and only a PR or
  `tier: quick` dispatch can clear it, `ci.yml:159-180`), so `rng-seed-boot` (`:625-629`) and the four
  `decoder-fuzz` jobs (`:995-1000`) run on `main`, in addition to `nightly-skip`, SPDX, identity, host, product
  and the feature job. The comment at `:998` says “not a per-push one”, but a `main` push is a full-tier push.
  The row `THINK:309` (6) and the mix row `THINK:324` (landing's `main` run + 4 seats = 19) are therefore wrong;
  the actual `main`-push peak is 11 job instances.
- The mix table counts “long-lived slots” only; four seats × 7 jobs is 28 job instances, not 16. If “slot” means
  a job that runs for minutes, say so; the 20-job limit counts all running jobs and queues the rest. The original
  §3 counted all jobs.
- CLAUDE.md says 4 quick tiers beside a batch; §3 says 2. One of them must change (the arithmetic supports 2).
- S2's gate says “`main`'s push run folded to 2 jobs” (`THINK:352`); #45 and #47 did not fold it. The §3 revision
  should not silently drop an S2 deliverable; it needs the PM's explicit sign-off and a corrected table.

## Verdict

- **#45: item 2 PASS; item 4 still open.** The feature-matrix fold is real, but the THINK §3 at this head still
  asks for 3 quick / 2 `main`, and the code is 7 / 11. #45 is not mergeable as S2a against the THINK as approved;
  either deliver the fold or get the §3 revision approved.
- **#47: not mergeable as S2b until (1) a batch cannot run `tier: quick` (§2.3), (2) the plan step cannot mask
  a failed plan (§2.7), and (3) `revert-batch.sh` reverts only the batch's merges (§2.6).** The prefix bisect
  behavior (§2.4), the build-vs-scenario classification (§2.5), and the CLAUDE.md 4-vs-2 contradiction (§3)
  should be fixed or explicitly accepted. The CLAUDE.md rule changes otherwise match THINK §6 and do not weaken
  the AAG, the cross-family SMP-core lens, or the close-gate; the escaped-defect text is fine but its revert tool
  is not.
- The batch plan/assembly/tree-hash path is in good shape; the deterministic assembly tested clean.

No QEMU run was made: this is a review, not a gate.
