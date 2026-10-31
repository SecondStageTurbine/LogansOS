<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #81): find out what the leftovers in two worker trees are, and save them → Jarvis, Chad

**MODEL:** GPT-5.6-Luna. **EFFORT:** medium. This is a tree-state audit, not a code review: list, save, report.

**Why.** Two reviews today found uncommitted content that was there before the review started, and J.c. doesn't know
what it is:
- **Jarvis's tree:** edits to `.gitignore` and `.ignore` (relay #76/#78).
- **Chad's tree:** an untracked folder `graft/` (relay #75).

Under the tree-state rule (`CLAUDE.md`, "Work is only real when it is on `origin`"), anything uncommitted may be the only
copy of someone's work. So the first step is to get a copy onto `origin`. **Delete nothing, revert nothing, stash
nothing.** A stash isn't on `origin`.

## Setup

Each of you works only in your own folder:

```
Jarvis — WORKTREE: E:\AI Automation\workers\jarvis
         BRANCH:   rescue/jarvis-tree-2026-10-26   (created below from where the tree is now)
Chad   — WORKTREE: E:\AI Automation\workers\chad
         BRANCH:   rescue/chad-tree-2026-10-26     (created below from where the tree is now)

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-26-tree-leftovers-jarvis-chad.md   # this file
```

RAG isn't needed for this. If you use it, health-check first with `Invoke-RestMethod http://127.0.0.1:8404/health`;
if it's down, note that in the RETURN (the PL manages it).

## Steps (both of you)

1. **Record the state before changing anything.** Paste the output of each command into the RETURN:
   ```
   git branch --show-current
   git log -1 --format="%H %ci %s"
   git status --porcelain
   git stash list
   git log --oneline "@{u}.." 2>$null
   ```
2. **Jarvis: the two files.** Paste `git diff -- .gitignore .ignore` in full, and the dates
   (`(Get-Item .gitignore).LastWriteTime`, `(Get-Item .ignore).LastWriteTime`). Then say in plain words what the edits do:
   - which paths they start or stop ignoring;
   - whether any file that `origin/main` tracks or would show in `git status` is now hidden by them. Run
     `git status --porcelain --ignored` with and without the edits to compare, and list any difference.
3. **Chad: the folder.** Paste these:
   - `Get-ChildItem graft -Recurse -File | Measure-Object Length -Sum` (file count and total size);
   - the first 50 paths (`Get-ChildItem graft -Recurse -File | Select-Object -First 50 FullName, Length, LastWriteTime`);
   - whether `graft` is its own git repo (`Test-Path graft\.git`). If it is, also paste
     `git -C graft remote -v`, `git -C graft status --porcelain`, `git -C graft log --oneline -5`, and
     `git -C graft log --oneline "@{u}.."`.
   Say what it looks like: whose work, which lane, and whether a copy already exists on `origin`
   (`git cherry`, or a matching branch) if you can tell.
4. **Save a copy to `origin`.** From the tree as it stands:
   ```
   git switch -c rescue/<you>-tree-2026-10-26
   ```
   - Jarvis: `git add .gitignore .ignore`, then commit `rescue: uncommitted .gitignore/.ignore edits found in the jarvis tree`.
   - Chad, if `graft/` is **not** its own repo and is under about 50 MB in total: `git add graft`, then commit
     `rescue: untracked graft/ found in the chad tree`. If it's its own repo, is bigger than that, or holds build
     output or binaries, **don't commit it**. Report it in the RETURN and leave it where it is. If it's its own repo
     with unpushed commits, say so; the PL decides where they go.
5. **The RETURN.** Add `Plans/RETURN-2026-10-26-tree-leftovers-<you>.md` (with the CC-BY-4.0 SPDX header) to the same
   rescue branch, commit, and `git push -u origin rescue/<you>-tree-2026-10-26`. Paste the SHA to the PL.
6. **Leave the tree as you found it.** After the push, `git switch` back to the branch from step 1. Jarvis: that
   switch puts the two files back to the branch's version, so bring the edits back as uncommitted changes with
   `git restore --source=rescue/jarvis-tree-2026-10-26 -- .gitignore .ignore`. Chad: `graft/` is still on disk either way. The PL and PM decide what to keep after reading
   your RETURN.

## Close condition

- The rescue branch is on `origin` with the RETURN (and the saved content, where step 4 says to commit it). Paste the
  SHA.
- Nothing is deleted. The tree's `git status --porcelain` is the same as in step 1.
