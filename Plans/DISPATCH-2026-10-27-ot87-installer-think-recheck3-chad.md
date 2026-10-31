<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #90): third narrow re-check of the OT-87 installer THINK (revision 4) → Chad, on Astra

**MODEL:** GPT-6-Astra. **EFFORT:** medium. The routing table: a lane that has bounced 2+ rounds on GPT-6.1-Sol moves
to Astra (medium for a narrow re-check). This round covers your relay #88 findings, plus the new prior-art survey
(§11), under J.c.'s rule of 2026-10-27 (CLAUDE.md, "Unfamiliar features: survey open source before writing from
scratch").

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `13df6360` (PR #149, revision 4 over
`1c68712b`).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-installer-think-recheck3-chad
          git fetch origin && git checkout -B codex/ot87-installer-think-recheck3-chad 13df6360

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-installer-think-recheck3-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-27-ot87-installer-think-recheck2-chad.md     # your relay #88
git diff 1c68712b 13df6360 -- docs/THINK-2026-10-27-ot87-installer.md
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input and the wrong result.

1. **W1 ordering (#88 item 1).** Every GPT copy is written array first: the array, flush, read back, then the
   header, flush, read back. That covers W1, W2 and both halves of W6. Replay your header-only-survives input. Is
   the erase case (a foreign primary header over our array during W2) also safe?
2. **The failed-flush model (#88 item 1).** A failed flush may persist any subset of what it covered. Check that:
   - "no further submission" (certain) is kept apart from "the marker's outcome" (uncertain);
   - the claim holds that the worst case is an installed disk with a fully durable payload;
   - the on-screen message never reports success.
   Replay the W6-backup-flush-fails-after-persisting trace.
3. **The unreported-swap bound (#88 item 2).** A reported swap keeps its bound (in-flight requests), which now
   requires reliable media-change reporting per driver, listed as R4 work. An unreported same-identity swap masked
   by the current step is stated as possibly undetected for the rest of the install. The host-KAT row asserts
   "not detected". Is the statement now true, and complete?
4. **The nit.** R4b's kill-switch cell.
5. **The prior-art survey (§11).** It's the new standing rule's item:
   - each row has an approach, a license and a last commit date with a SHA;
   - spot-check at least three of them against the upstream repositories;
   - only MPL-compatible code is borrowed (vendoring `fatfs`, MIT);
   - GPL, AGPL and LGPL projects are used for design only.
   Is anything a modern installer needs missing from the survey?
6. **Anything new** in revision 4 that's wrong.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-installer-think-recheck3-chad.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
