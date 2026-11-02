<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 283 — #45/#47 re-check (the tiered gate)

**Reviewed head:** #47 `0f6ac19343f84f544ee7547e96533240133ea44b` (`origin/claude/ot58-s2b-batch`; contains #40 and
#45). #45's head is `cdee8f823b17798c2b8243668eca9584df92d5cf`.
**Base:** `origin/main` at `c65ee28ee9bb978db6e69f4224a363e114752ff7` (relay 284, docs-only on top of relay 283); the
stack's merge-base is `c0b0e249ba39413b60f24a6d997280977eb92293`.
**Model / effort:** DeepSeek, high.

Read at the head: the whole `7c932271..0f6ac193` diff for `.github/`, `scripts/`, `tools/xtask/src/`, `CLAUDE.md`
and the THINK; the 11:12 and 17:24 `PM:` comments on #47; the relay-271 RETURN.
Ran: `bash -n` on the three scripts; a PyYAML parse of `ci.yml`; `cargo test -p xtask --bin xtask` for the six new
tests (`scenario_outcome...`, `rerun_timeouts...`, `batch_assemble...`, `a_batch_is_always...`,
`a_failed_batch_plan...`, `revert_batch...`); `batch-assemble.sh 3e36788a 0f6ac193` (tree
`4aeb1e7015de565f022b8fe904586b97c633438c`, matching run 41458717816); a synthetic `revert-batch.sh` history with
an unrelated merge, a later batch and a docs commit. No QEMU run: this is a review, not a gate.

## Verdicts

| Item | Verdict | Evidence at `0f6ac193` |
|---|---|---|
| **#45 item 4** — job count and free-tier arithmetic | **PASS** (one table correction below) | `build-checks` is one job (`ci.yml:1094`), decoder fuzz one job (`:1024`), and `main`'s push is host-only (`:256`). A quick PR is 4 job instances / 3 at once (plan, `host-checks`, `build-checks`, the smoke shard); a full tier is 8 at once; `main`'s push is 3 / 2. The 20-job arithmetic `4 × 3 + 8 = 20` holds (`CLAUDE.md:651-652`, `THINK:317-319`). |
| **#47 2.3** — a quick batch passing as full | **PASS** | The batch branch runs before the plan: `ci.yml:197-206` rejects `tier: quick` ("a batch is the full tier"). `batch-report` says "Merge evidence: yes" only when `MODE=full`, `FULL=true`, K=4 and every job is green (`:1435`, `:1484`). Host test `batch_scripts_tests.rs:169` passes. Live: run 41432237688 (on `08ac48ca`; `tier: quick`, `batch: 47`) failed in the plan job with the other six jobs skipped. |
| **#47 2.4** — prefix bisect classification and timeout re-run | **PASS** | `ScenarioOutcome` / `classify` at `main.rs:9034`, `:9042`; the one re-run loop at `:9220-9222`; batch shards pass `--rerun-timeouts` when the tree's xtask has it (`ci.yml:600-602`); a bisect leg is one job of named scenarios (`:217`, `:607`); `batch-report` counts K from the plan (`:1411`) and prints `FAIL`, `FAIL-build`, `FAIL-timeout`, `rerun-pass` classes (`:1426-1429`). Host tests `main.rs:22951`, `:22995` pass. Live: run 41433308181 has one QEMU bisect job, the rest skipped. |
| **#47 2.6** — the revert takes only the batch's merges | **PASS** | `revert-batch.sh:49-71` takes only the first run of `batch: member 1, 2, …` merges, author `OT-58 batch`, second parent = the SHA in the subject (`:55-57`); `:79-85` makes one commit. Host test `batch_scripts_tests.rs:110` passes, and a synthetic history with an unrelated merge, a later batch and a docs commit kept all three and reverted only `m1`/`m2`. |

## New (not part of the relay-271 findings)

1. **`batch-report` still hard-codes 4 in its verdict.** `ci.yml:1435` is `[ "$k" -eq 4 ]`, while `missing` takes K
   from the plan at `:1411`. A K=6 batch (THINK §3 / S3) with all six shards green would print "Merge evidence:
   no" and fail the report job. Today's K=4 is unaffected, but "takes K from the plan" is only true for `missing`,
   not the verdict.
2. **THINK §3's job-instance count is wrong.** `THINK:318` says a full tier is `8 + K = 12` job instances. The
   workflow and the live runs show 9 for a full-alone PR's own run (the skipped `batch-report` is the 10th job
   entry) and 10 for a batch (`batch-report` runs). The "at once" column (`4 + K = 8`) and the 20-job arithmetic
   are right; `8 + K` double-counts the shards.
3. **CLAUDE.md's old kernel-touching close-gate still says 4 shards.** `CLAUDE.md:531-532` requires a
   `workflow_dispatch` QEMU run (all 4 shards) "for any kernel-touching build". The quick tier sends non-core
   `kernel/` paths (drivers, net, fs, acpi, init, …; `main.rs:10116`, `:10369`) to the smoke tier, so a
   kernel-touching driver PR's close-gate evidence is now the smoke cell, not the 4 shards. The new section says
   it replaces "the PR's own `workflow_dispatch` QEMU run" but never names kernel-touching builds, so the two
   rules conflict; say which wins (or scope the old line to the full-alone paths).
4. **The old merge-bar bullet is not updated.** `CLAUDE.md:554-555` still says "rebase-fresh + 4-shard gate
   green + PM verify-before-merge (independent CI re-verify + own `git show --stat`) + AAG clean + no
   regression"; the new merge bar (`:610-616`) says it replaces only the 4-shard clause, and its list does not
   restate "no regression" or the independent CI re-verify. If those still apply, say so; if not, that is a
   policy weakening by omission.
5. **2.6 depends on the assembler's fixed identity.** `revert-batch.sh:55` matches author `OT-58 batch`. The
   landing section says the PM may land "the same merges in the same order"; if those merges are recreated by hand
   rather than by `batch-assemble.sh`, the author is the PM and the revert will not find them. Land the assembled
   commit (or say the merges must keep the `OT-58 batch` identity).

Also fixed and spot-checked: 2.1 (tier-tied member evidence `batch-plan.sh:68-74`, base on main `:38-41`,
`--paginate` `:76-79`), 2.7 (`ci.yml:283-288` `shell: bash` plus a file, no `tee`), 2.8 (`ci.yml:104-105`
`permissions: contents: read`), and #45 note 1 (`ci.yml:234`, `:242` last `tier=` line).

## Verdict

The four requested fixes are in at `0f6ac193`; nothing in 2.3/2.4/2.6 is a merge blocker. The table correction
worth making before the stack lands is THINK §3's job-instance cell; the code correction is the hard-coded K=4 in
`batch-report` before S3. The CLAUDE.md close-gate and merge-bar duplication (items 3-4) should be reconciled in
the same docs pass: the rule change is real, but the file currently states both the old and the new bar.

No QEMU run was made.
