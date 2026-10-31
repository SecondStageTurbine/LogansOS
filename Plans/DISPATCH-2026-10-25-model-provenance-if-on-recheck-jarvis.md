<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #65): narrow re-check of your relay #58 items 4 and 6, after #123 and #127 → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own earlier findings. Relay #58 item 4
(IF=0 syscall paths to NeoFS `entries` through model load and model provenance) and item 6 (the NOTE's open-items list)
were addressed by #123 (relay #60 covers its `sys_model_load` part) and #127 (lane D, this relay). Item 5, the
scenario's held-at-call-start check, is a separate fix from S2 and isn't in scope here. **Do this after relays #60 and
#62.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `4a6bd93a` (main; the squash of PR #127, gated as `5015b3da`, run `40226670884`,
green).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/model-provenance-if-on-recheck-jarvis
          git fetch origin && git checkout -B codex/model-provenance-if-on-recheck-jarvis 4a6bd93a81e4247fbbec3b514d2c8a4637e706e8

git show origin/main:Plans/DISPATCH-2026-10-25-model-provenance-if-on-recheck-jarvis.md   # this file
git show origin/main:Plans/RETURN-2026-10-25-vfs-if-on-review-jarvis.md                   # your #58
git show origin/main:Plans/NOTE-2026-10-25-model-load-if-on-design.md
git diff 745dd314 4a6bd93a -- kernel tools docs/concurrency-pitfalls.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='model_still_current validate_current FileProvenance current_generation entries'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete sequence (CPU, IF state, which lock is
held).

1. **Every provenance check is IF-on.** Every syscall caller of `Model::validate_current` (model query, infer run,
   and every generate variant) now goes through `model_still_current`. Grep for any remaining direct
   `validate_current()` call reachable from a syscall at IF=0.
2. **No guard across the switch.** `model_still_current` and `model_query_desc` hold no handle-table or other lock
   when interrupts are turned on: `model_lookup` returns an owned `Arc` and its guard has ended.
3. **MOUNTS inside provenance.** `FileProvenance::validate_current` now takes `MOUNTS` under `without_interrupts`.
   Check that this short section can't be reached with `entries` held, and that the NeoFS generation lookup after it
   runs IF-on.
4. **Boot-module models.** A boot-module model's provenance has no mount and no generation, so its check returns at
   once and touches neither NeoFS nor `MOUNTS`. Confirm from the code that Arthur's boot-module path is unaffected.
5. **The scenario's new leg.** The `phase3_neofs_syscall_if_on` model-query leg (`debug_sys_model_query_for_test`)
   contends on `entries` through provenance, and the control hangs on it. Note anything else in the scenario that
   item 5 of #58 still affects; that's tracked separately.
6. **Your item 6.** The NOTE's open-items list is corrected: tensor write removed, model query provenance and the
   other last-reference drop sites you named either fixed (#122's graveyard, this lane) or listed as open.

## Close condition

- Commit `Plans/RETURN-2026-10-25-model-provenance-if-on-recheck-jarvis.md` on your branch and **push it**, with
  PASS/FAIL per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
