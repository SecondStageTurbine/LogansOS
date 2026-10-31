<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #118): re-check of the OT-90 shutdown THINK, revision 2 (#153 @ 15379e82) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** medium. This is a narrow re-check of your own three findings from relay #93, on a
design doc. Item 2 is a parsing argument rather than prose, so it gets Luna rather than the lightest model.

**Type:** narrow design re-check. **Deliverable:** one pushed RETURN,
`Plans/RETURN-2026-10-28-ot90-think-recheck-janus.md`. **Reviewed SHA:** `15379e82` (PR #153). Your RETURN #93
reviewed `06ea9d75`. Diff: `git diff 06ea9d75 15379e82 -- docs/THINK-2026-10-27-ot90-shutdown-restart.md`.

## Checklist (pass/fail, plus `file:line` in the THINK and any code it cites)

1. **Item 2, the `\_S5` decode.**
   - **The new rule:** exactly one `_S5_` NameSeg anywhere in the DSDT and SSDTs, as a static `Name` before a
     `PackageOp`. Every covering Scope, Device, Method, If, Else or While header must be `Scope (\)`. An absolute
     `Name (\_S5)` is allowed inside a named Scope.
   - **Check that it fails closed on:**
     - your counterexample (a `Method (\_S5)` plus `Scope (\_SB) { Name (_S5, …) }`);
     - `\_SB._S5`;
     - `_S5` inside an If or a Device;
     - a `_S5` reached through an SSDT;
     - any other layout you can build where the one accepted `Name` is not the root `\_S5`.
   - **Check that it still accepts** the ten captured sets. S5 says a prototype decodes all ten; spot-check the
     fixture claim on at least two (`fixtures/acpi/...`).
   - The KATs are planned with `iasl -f` fixtures. Check they would cover each fail-closed case above.
2. **Item 4, durability.**
   - **What K2 now claims:** it freezes writes, runs a shutdown-only drain (because `fsync_regular` returns early on
     a read-only mount), stops the worker, and reports the result. A flush error, write error or timeout is
     reported as **unclean**, never clean.
   - **What it rests on:** PR #180, the NeoFS commit barrier and deferred truncate reuse, at `a1b73817`. Check that
     the THINK's claim follows from #180's stated ordering in `docs/specs/neofs-format.md` §3.4 and §3.5 on that
     branch. A full review of #180 itself is a separate relay.
   - Check that P3 is marked as depending on #180.
3. **Item 8, prior art.** All dates are UTC committer dates, with the convention stated. Redox is
   `2026-09-26T19:58:20Z`.

Plain checklist: for each item, what you checked, pass or fail, the enforcing `file:line`, and a concrete input for
any fail.

```
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot90-think-recheck-janus
          git fetch origin && git checkout -B codex/ot90-think-recheck-janus 15379e82

git show origin/main:Plans/DISPATCH-2026-10-28-ot90-think-recheck-janus.md
git show origin/main:Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md
git show origin/claude/neofs-commit-barrier:docs/specs/neofs-format.md
```

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?" questions; grep
still wins for exact symbols. Health check first: `Invoke-RestMethod http://127.0.0.1:8404/health` should return
`{"status":"healthy",...}`. If it's down, note that in your RETURN and continue with direct file reads. The PL
manages the RAG, so it's never a blocker. Query example:
`$body=@{query='ACPI _S5 sleep type decode'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

Commit and push the RETURN to your branch. It's docs only, so there's no gate and no disk reclaim. Before ending, run
`git status --porcelain` and `git log @{u}..`; both must be empty.
