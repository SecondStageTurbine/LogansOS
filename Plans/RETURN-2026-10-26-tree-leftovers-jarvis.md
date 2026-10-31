<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay #81, Jarvis tree leftovers

Saved on `rescue/jarvis-tree-2026-10-26`, created from the current tree at
`90fe0905bad7a2e6681616637059e6206791784a`, without rebasing onto main.
Worktree: `E:\AI Automation\workers\jarvis` only.
The two files are saved in commit `a0194a02` with the dispatch's exact commit message.
No deletion, revert, or stash operation was performed during this relay.

## State before changes

`git branch --show-current`:
```text
codex/boot-evidence-holds-review-jarvis
```

`git log -1 --format="%H %ci %s"`:
```text
90fe0905bad7a2e6681616637059e6206791784a 2026-10-26 15:29:05 -0400 Add boot evidence holds review return
```

`git status --porcelain`:
```text
 M .gitignore
?? .ignore
```

`git stash list` (existing entries, left untouched):
```text
stash@{0}: On codex/build-id-review-jarvis: relay76-preserve-preexisting-graft-ignore
stash@{1}: On codex/build-id-review-jarvis: relay74-preserve-preexisting-graft-ignore
stash@{2}: On codex/model-load-if-on-review-jarvis: relay60 preserve pre-existing Jarvis local changes and graft cache
```

`git log --oneline "@{u}.." 2>$null`: empty output (no unpushed commits).

## Exact edits and dates

`git diff -- .gitignore .ignore` in full:
```diff
diff --git a/.gitignore b/.gitignore
index 05d20e20..9c6520bd 100644
--- a/.gitignore
+++ b/.gitignore
@@ -50,3 +50,6 @@ __pycache__/
 # so they must be ignored here to keep `git status` clean. See
 # docs/runbook-per-seat-worktrees.md.
 /Workers/
+
+# graft's local graph cache — regenerable, not committed (run `graft build`).
+/graft/
```

That command omits `.ignore` because it is untracked. Its complete new-file
diff, from `git diff --no-index -- /dev/null .ignore`, is:
```diff
diff --git a/.ignore b/.ignore
new file mode 100644
index 00000000..615e581e
--- /dev/null
+++ b/.ignore
@@ -0,0 +1,5 @@
+# graft's cards are gitignored but should stay greppable: ripgrep reads
+# .ignore before .gitignore, so this re-admits the tree to search only.
+!graft/
+graft/.cache/
+graft/.graph/
```

`(Get-Item .gitignore).LastWriteTime`, formatted as ISO 8601:
`2026-10-26T15:29:47.8912885-04:00`.
`(Get-Item .ignore).LastWriteTime`, formatted as ISO 8601:
`2026-10-26T15:29:47.9927611-04:00`.
These dates reflect restoration after #78, not evidence of original authorship.

The `.gitignore` edit ignores the root `graft/` directory and all its contents
in Git. It stops ignoring no paths. The `.ignore` file is for search tools
such as ripgrep: it re-admits `graft/` for search, then excludes its `.cache/`
and `.graph/` subdirectories. It does not change Git's ignore behavior.
The comments identify these as local graft graph/search configuration; Git
does not establish who originally typed them. They predated #76/#78 and were
preserved through those relays.

## Hidden-file comparison

With the edits, `git status --porcelain --ignored`:
```text
 M .gitignore
?? .ignore
!! .cargo-home/
!! boot/neoplutarch.iso
!! graft/
!! target/
```

Without the edits, the same status command:
```text
!! .cargo-home/
!! boot/neoplutarch.iso
!! graft/
!! target/
```

To avoid changing either real file, the second command used a temporary
comparison tree with a copy of `HEAD:.gitignore`, no `.ignore`, copied
top-level files, directory junctions to the existing directories, and a
copied index selected by `GIT_INDEX_FILE`. Git used the real repository's
metadata, including its existing info/exclude. The temporary tree is at
`C:\Users\<user>\AppData\Local\Temp\jarvis-relay81-baseline-2wbly22e\tree`;
both status commands succeeded. No baseline checkout or restore was applied
to the real worktree. An earlier scratch setup attempted cross-volume
hard links, failed before comparison, and was replaced with copies; it
also remains on disk. No temporary files were deleted.

The only output differences are ` M .gitignore` and `?? .ignore` themselves.
No additional path becomes hidden in this tree: the existing
`.git/info/exclude:9` already contains `/graft/` (commented "Local semantic
graph cache (relay 74)"). Baseline `git check-ignore -v graft` confirms:
```text
E:/AI Automation/workers/jarvis/.git/info/exclude:9:/graft/ graft
```
With the edits it reports `.gitignore:55:/graft/` instead.
`git ls-tree -r --name-only origin/main -- graft .ignore` returned no paths;
no file tracked by `origin/main` is under the newly ignored root directory.
`git ls-files -ci --exclude-standard` also returned no paths: no currently
tracked file matches the current ignore rules. Git ignore rules do not
hide changes to already tracked files.

Git emitted `warning: unable to access 'C:\Users\<user>/.config/git/ignore':
Permission denied` during both status modes. The comparison uses the same
accessible configuration on both sides; no conclusion is claimed about
unreadable global patterns.

## Save and handback

The rescue branch contains the two saved files plus this RETURN. After its
push, handback follows dispatch step 6: switch back to
`codex/boot-evidence-holds-review-jarvis`, then use
`git restore --source=rescue/jarvis-tree-2026-10-26 -- .gitignore .ignore`
to bring back the saved edits as uncommitted files. This restoration is the
dispatch's explicit handback step, not a revert of the edits. Final status
must match the two lines recorded above; the existing stashes remain intact.
The final push and handback results are reported with the pushed SHA in chat.

RAG was not used. The required graft discovery command ran before searches;
its results were not needed for this tree-state audit. Requested model
GPT-5.6-Luna was unavailable in this session; the current model performed
the audit, as disclosed before changes. No code review or build was needed.
