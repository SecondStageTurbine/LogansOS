<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE for Chad, 2026-10-27: relays #95, #98, #100, #107, #110, #104

**Why a bundle.** J.c. asked for one paste per coder. Each item is still its own review: it has its own dispatch
file (the full checklist), its own branch and its own RETURN file. Work them **in the order below**, and push each
RETURN before starting the next. All of them are post-merge reviews; a confirmed FAIL is handled revert-first.

## Setup (once)

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-27-review-bundle-chad.md   # this file
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Each dispatch file has an example query for its subject.

## The reviews, in order

**Model as run: GPT-6.1-Sol, effort high, for every item** (J.c., 2026-10-27: one model per bundle instead of
switching). The per-group model lines below are the routing table's minimum; Sol meets or exceeds each.

### Model: **GPT-6-Luna**, effort **high**

Contained code or a design THINK (CLAUDE.md routing table).

1. **Relay #95: post-merge review of OT-87 R2, Arthur's model from the installed NeoFS volume (#154)**, reviewed SHA `2f066957`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r2-model-from-neofs-review-chad.md`
   - Branch: `git checkout -B codex/ot87-r2-model-from-neofs-review-chad 2f066957`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot87-r2-model-from-neofs-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

2. **Relay #98: cross-family read of the OT-66 login / session THINK (#157)**, reviewed SHA `d49acd0f`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot66-login-think-review-chad.md`
   - Branch: `git checkout -B codex/ot66-login-think-review-chad d49acd0f`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot66-login-think-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

3. **Relay #100: post-merge review of A9-C, scrolling Arthur's reply with the keyboard (#159)**, reviewed SHA `675495c9`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-a9-arthur-scrollback-review-chad.md`
   - Branch: `git checkout -B codex/a9-arthur-scrollback-review-chad 675495c9`
   - Deliverable: commit `Plans/RETURN-2026-10-27-a9-arthur-scrollback-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

4. **Relay #107: post-merge review of OT-87 R4.2, the install bundle and ESP templates (#165)**, reviewed SHA `0d2608f9`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r4.2-bundle-review-chad.md`
   - Branch: `git checkout -B codex/ot87-r4.2-review-chad 0d2608f9`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot87-r4.2-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

5. **Relay #110: post-merge review of NFS-W1, kernel NeoFS large-file writes (#167)**, reviewed SHA `fa99f551`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-nfs-w1-large-write-review-chad.md`
   - Branch: `git checkout -B codex/nfs-w1-review-chad fa99f551`
   - Deliverable: commit `Plans/RETURN-2026-10-27-nfs-w1-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

### Model: **GPT-6.1-Sol**, effort **high**

SMP-core or permission-model code, so it needs a cross-family lens (CLAUDE.md routing table).

6. **Relay #104: cross-family review of A9-C2, mouse-wheel and touchpad scrolling (#162)**, reviewed SHA `9a1c35bb`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-a9-arthur-wheel-review-chad.md`
   - Branch: `git checkout -B codex/a9-arthur-wheel-review-chad 9a1c35bb`
   - Deliverable: commit `Plans/RETURN-2026-10-27-a9-arthur-wheel-review-chad.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

## Close condition

- One pushed RETURN per relay, at the paths above.
- When all are done, paste the PL one line per relay: the relay number, PASS or FAIL, and the RETURN commit SHA.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
