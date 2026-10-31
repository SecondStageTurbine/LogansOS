<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #94): final narrow check of the OT-87 installer THINK (revision 5) → Chad, on Astra

**MODEL:** GPT-6-Astra. **EFFORT:** medium. This is the last check, and it covers only your relay #90 items 5 and 6
plus its wording notes. Everything else passed in relay #90 and is out of scope.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `6f9ae4c8` (PR #149, revision 5 over
`13df6360`). If it passes, the PM merges the THINK.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot87-installer-think-recheck4-chad
          git fetch origin && git checkout -B codex/ot87-installer-think-recheck4-chad 6f9ae4c8

git show origin/main:Plans/DISPATCH-2026-10-27-ot87-installer-think-recheck4-chad.md   # this file
git show origin/main:Plans/RETURN-2026-10-27-ot87-installer-think-recheck3-chad.md     # your relay #90
git diff 13df6360 6f9ae4c8 -- docs/THINK-2026-10-27-ot87-installer.md
```

RAG isn't needed. If you use it, health-check with `Invoke-RestMethod http://127.0.0.1:8404/health`; if it's down,
note it in the RETURN (the PL manages it).

## Checklist

Answer each item PASS or FAIL with `file:line`.

1. **`fatfs` lives in the host ESP-template builder only.** Check each of these:
   - the builder reads back fatfs's output, verifies every slot chain is contiguous (failing the build otherwise),
     and generates the slot map from that output;
   - the bundle pin covers the templates;
   - W3 still copies the template whole, and W4 is still the NeoFS seed;
   - the kernel never formats FAT and never links `fatfs`;
   - §4.3, the R4 row, §11.2 and §11.4 agree.
2. **The curtin row** now maps clearing signatures to W4's NeoFS seed.
3. **§11.5, deferred items.** Encrypted installs and recovery credentials, and Secure Boot, are recorded as deferred,
   with reasons. The Zenbook's "Secure Boot off" is stated as a test prerequisite only. The installer's docs say the
   target needs Secure Boot off until that lane lands.
4. **Your wording notes.** "Newly submitted header", and the W6 restarted-primary case as its own trace.
5. **Nothing else changed** outside these sections.

## Close condition

- Commit `Plans/RETURN-2026-10-27-ot87-installer-think-recheck4-chad.md` on your branch and **push it**, with PASS/FAIL
  per item and an overall verdict. Paste the SHA to the PL.
- No build is needed. Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
