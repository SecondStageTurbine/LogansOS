<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #82): save the stashes that may hold real work → Jarvis, Chad

**MODEL:** GPT-5.6-Luna. **EFFORT:** medium. Follow-up to relay #81: save, list, report. No code review.

**Why.** Relay #81 showed both trees' `.gitignore`/`.ignore` edits and Chad's `graft/` folder are only the local graft
code-search tool's regenerable cache, and main now ignores them the same way (`316948c7`). But the stash lists hold
entries that may contain real work, and a stash exists only on that disk. Two look like more than graft's cache:
- **Chad `stash@{12}`**: `On codex/rung5-boot-neofs-product-flow-chad: chad-rung5-optional-memory-wip`;
- **Jarvis `stash@{2}`**: `On codex/model-load-if-on-review-jarvis: relay60 preserve pre-existing Jarvis local changes and graft cache`.

**Push copies, nothing else.** Don't drop, pop, apply or clear any stash, and don't change the tree.

## Setup

```
Jarvis — WORKTREE: E:\AI Automation\workers\jarvis    (yours alone)
Chad   — WORKTREE: E:\AI Automation\workers\chad      (yours alone)
BRANCH:  none to check out. The copies are pushed straight from the stash refs (below), and the RETURN goes on your
         existing rescue branch rescue/<you>-tree-2026-10-26.

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-26-stash-rescue-jarvis-chad.md   # this file
```

RAG isn't needed. If you use it, health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`; if it's
down, note that in the RETURN (the PL manages it).

## Steps

1. **Every stash, briefly.** For each entry in `git stash list`, paste its label and
   `git stash show --stat --include-untracked stash@{N}` (if your git doesn't take `--include-untracked`, use
   `git stash show --stat stash@{N}` and say so). Then sort each stash into one of two kinds:
   - **cache only:** every path is under `graft/`, `.gitignore` or `.ignore`;
   - **has other files:** anything else.
2. **Push a copy of every stash with other files**, Chad's `stash@{12}` and Jarvis's `stash@{2}` among them:
   ```
   git push origin "stash@{N}:refs/heads/rescue/<you>-stash-N-2026-10-26"
   ```
   A stash is a commit, so this pushes it as is, including its index and untracked parents. Paste each pushed SHA.
   Cache-only stashes don't need a copy.
3. **For each pushed stash, say what it is.** List the non-cache paths and give a few lines of `git stash show -p`
   for each. Then check whether that work is already on main:
   - `git cherry origin/main <stash-sha>`;
   - or, for single files, compare against `origin/main:<path>`.
   Mark each one: already on main, superseded (say by what), or not on main.
4. **The RETURN.** Add `Plans/RETURN-2026-10-26-stash-rescue-<you>.md` (CC-BY-4.0 SPDX header) to
   `rescue/<you>-tree-2026-10-26` without switching your working branch:
   ```
   git fetch origin rescue/<you>-tree-2026-10-26
   git worktree add "$env:TEMP\<you>-rescue" origin/rescue/<you>-tree-2026-10-26
   # write the RETURN there, commit, then:
   git -C "$env:TEMP\<you>-rescue" push origin HEAD:rescue/<you>-tree-2026-10-26
   git worktree remove "$env:TEMP\<you>-rescue"
   ```
   Paste the SHA.

## Close condition

- Each stash with other files is on `origin` as `rescue/<you>-stash-N-2026-10-26`, and the RETURN is pushed. Paste the
  SHAs.
- `git stash list` and `git status --porcelain` are the same as before you started. Nothing is dropped or deleted.
