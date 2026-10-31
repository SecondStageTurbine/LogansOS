<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE for Jarvis, 2026-10-27: relays #89, #99, #103, #92, #97

**Why a bundle.** J.c. asked for one paste per coder. Each item is still its own review: it has its own dispatch
file (the full checklist), its own branch and its own RETURN file. Work them **in the order below**, and push each
RETURN before starting the next. All of them are post-merge reviews; a confirmed FAIL is handled revert-first.

## Setup (once)

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-27-review-bundle-jarvis.md   # this file
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Each dispatch file has an example query for its subject.

## The reviews, in order

**Model as run: GPT-6.1-Sol, effort high, for every item** (J.c., 2026-10-27: one model per bundle instead of
switching). The per-group model lines below are the routing table's minimum; Sol meets or exceeds each.

### Model: **GPT-6.1-Sol**, effort **high**

SMP-core or permission-model code, so it needs a cross-family lens (CLAUDE.md routing table).

1. **Relay #89: cross-family review of OT-61 C1.2, per-thread TLS (PR #150)**, reviewed SHA `c06b3a99`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot61-c1.2-tls-review-jarvis.md`
   - Branch: `git checkout -B codex/ot61-c1.2-tls-review-jarvis c06b3a99`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot61-c1.2-tls-review-jarvis.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

2. **Relay #99: cross-family review of OT-61 C1.3, std threads and exit_group (#158)**, reviewed SHA `908b35b5`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot61-c1.3-std-threads-review-jarvis.md`
   - Branch: `git checkout -B codex/ot61-c1.3-review-jarvis 908b35b5`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot61-c1.3-review-jarvis.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

3. **Relay #103: cross-family review of OT-61 C1.3b, exit_group across CPUs (#161)**, reviewed SHA `35e01723`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot61-c1.3b-exit-group-review-jarvis.md`
   - Branch: `git checkout -B codex/ot61-c1.3b-review-jarvis 35e01723`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot61-c1.3b-review-jarvis.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

### Model: **GPT-6-Luna**, effort **high**

Contained code or a design THINK (CLAUDE.md routing table).

4. **Relay #92: post-merge review of OT-87 R1, `xtask vm-image` (#152)**, reviewed SHA `22fd551f`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-ot87-r1-vm-image-review-jarvis.md`
   - Branch: `git checkout -B codex/ot87-r1-vm-image-review-jarvis 22fd551f`
   - Deliverable: commit `Plans/RETURN-2026-10-27-ot87-r1-vm-image-review-jarvis.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

5. **Relay #97: post-merge review of A9-B, the repetition guard for greedy decode (#156)**, reviewed SHA `500606e9`.
   - Read the checklist: `git show origin/main:Plans/DISPATCH-2026-10-27-a9-repetition-guard-review-jarvis.md`
   - Branch: `git checkout -B codex/a9-repetition-guard-review-jarvis 500606e9`
   - Deliverable: commit `Plans/RETURN-2026-10-27-a9-repetition-guard-review-jarvis.md` on that branch and **push it**, with PASS/FAIL per item, `file:line`, and
     an overall verdict.

## Close condition

- One pushed RETURN per relay, at the paths above.
- When all are done, paste the PL one line per relay: the relay number, PASS or FAIL, and the RETURN commit SHA.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
