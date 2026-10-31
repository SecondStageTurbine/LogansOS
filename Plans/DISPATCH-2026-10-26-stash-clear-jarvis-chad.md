<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #83): clear your stashes, now that the ones with real work are on origin → Jarvis, Chad

**MODEL:** GPT-5.6-Luna. **EFFORT:** low. Housekeeping: J.c. asked for the stashes to be cleared (2026-10-26).

**Why it's safe.** Relay #82 pushed a copy of every stash that held anything besides graft's cache:
- Chad `stash@{12}` → `rescue/chad-stash-12-2026-10-26` @ `973160b9f0d1e41537f53116098a2d68d77823a0`;
- Jarvis `stash@{2}` → `rescue/jarvis-stash-2-2026-10-26` @ `f2dd94c5a0585b89e9b6469746b46ca8abe61fc3`.

Every other stash in both trees is graft cache only (`graft/`, `.gitignore`, `.ignore`), which the tool rebuilds.
The RETURNs are on main: `Plans/RETURN-2026-10-26-stash-rescue-{chad,jarvis}.md`.

## Setup

```
Jarvis — WORKTREE: E:\AI Automation\workers\jarvis    (yours alone)
Chad   — WORKTREE: E:\AI Automation\workers\chad      (yours alone)
BRANCH:  stay on the branch you're on; nothing is committed.

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-26-stash-clear-jarvis-chad.md   # this file
```

RAG isn't needed. If you use it, health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`; if it's
down, note that (the PL manages it).

## Steps

1. **Check the copy is on origin, and stop if it isn't.**
   ```
   git ls-remote origin refs/heads/rescue/<you>-stash-*-2026-10-26
   ```
   It must print the SHA above. Also check that `git stash list` still has the same number of entries as in your
   #82 RETURN (Chad 13, Jarvis 3), with the same labels. If anything differs (a new stash, a different SHA, a missing
   ref), **clear nothing** and tell the PL what you saw.
2. **Clear.** `git stash clear`, then `git stash list` (it must print nothing).
3. **Report back in chat** (no RETURN file is needed for this): the `ls-remote` line, the stash count before, and the
   empty list after. `git status --porcelain` is unchanged by this.

## Close condition

`git stash list` is empty, and the rescue copy is on origin as named above.
