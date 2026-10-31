<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE for Janus, 2026-10-27: relays #109, #112, #91, #93, #96, #102, #106

**Why a bundle.** J.c. asked for one paste per coder. Each item is still its own review: it has its own dispatch
file (the full checklist), its own branch and its own RETURN file. Work them **in the order below**, and push each
RETURN before starting the next. All of them are post-merge reviews; a confirmed FAIL is handled revert-first.

## Setup (once)

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-27-review-bundle-janus.md   # this file
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Each dispatch file has an example query for its subject.

## The reviews, in order

**Model as run: GPT-6.1-Sol, effort high, for every item** (J.c., 2026-10-27: one model per bundle instead of
switching). The per-group model lines below are the routing table's minimum; Sol meets or exceeds each.

### Model: **GPT-6.1-Sol**, effort **high**

SMP-core or permission-model code, so it needs a cross-family lens (CLAUDE.md routing table).

1. **Relay #109: cross-family review of K1a-0, the capability-graph lock and re-parenting (#166)**, reviewed SHA `149d4a19`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-k1a0-cap-graph-review-janus.md`
   - Branch: `git checkout -B codex/k1a0-cap-graph-review-janus 149d4a19`
   - Deliverable: commit `Plans/RETURN-2026-10-27-k1a0-cap-graph-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

2. **Relay #112: cross-family review of K1a, the root-directory capability (#168)**, reviewed SHA `2922bf0c`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-k1a-rootdircap-review-janus.md`
   - Branch: `git checkout -B codex/k1a-rootdircap-review-janus 2922bf0c`
   - Deliverable: commit `Plans/RETURN-2026-10-27-k1a-rootdircap-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

### Model: **GPT-6-Luna**, effort **high**

Contained code or a design THINK (CLAUDE.md routing table).

3. **Relay #91: post-merge review of A9 finding A, the WM-liveness overlay and guard (#151)**, reviewed SHA `839b893e`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-a9-presents-liveness-review-janus.md`
   - Branch: `git checkout -B codex/a9-presents-liveness-review-janus 839b893e`
   - Deliverable: commit `Plans/RETURN-2026-10-27-a9-presents-liveness-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

4. **Relay #93: cross-family read of the OT-90 shutdown/restart THINK (PR #153)**, reviewed SHA `06ea9d75`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot90-power-think-review-janus.md`
   - Branch: `git checkout -B codex/ot90-power-think-review-janus 06ea9d75`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot90-power-think-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

5. **Relay #96: post-merge review of A9-D, the turning emblem while Arthur thinks (#155)**, reviewed SHA `d3603830`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-a9-busy-emblem-review-janus.md`
   - Branch: `git checkout -B codex/a9-busy-emblem-review-janus d3603830`
   - Deliverable: commit `Plans/RETURN-2026-10-27-a9-busy-emblem-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

6. **Relay #102: post-merge review of OT-87 R3, the GPT oracle, partition view and one-disk VM image (#160)**, reviewed SHA `7c771c0b`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r3-review-janus.md`
   - Branch: `git checkout -B codex/ot87-r3-review-janus 7c771c0b`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot87-r3-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

7. **Relay #106: post-merge review of OT-61 C2.0, std entropy (#164)**, reviewed SHA `58ab4966`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot61-c2.0-entropy-review-janus.md`
   - Branch: `git checkout -B codex/ot61-c2.0-review-janus 58ab4966`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot61-c2.0-review-janus.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

## Close condition

- One pushed RETURN per relay, at the paths above.
- When all are done, paste the PL one line per relay: the relay number, PASS or FAIL, and the RETURN commit SHA.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
