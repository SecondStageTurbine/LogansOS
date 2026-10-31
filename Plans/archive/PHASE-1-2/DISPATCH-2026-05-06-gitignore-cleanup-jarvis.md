# Dispatch: `.gitignore` cleanup + delete empty scratch files at root

**To:** Jarvis (coder 2)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `e092217` (currently HEAD on `main`)
**Type:** Doc-only / hygiene. Two file changes max. Single commit on `main`.
**Estimated surface:** ~5–10 lines added to `.gitignore`, two zero-byte files deleted from root.

---

## 0. Standing instructions

Same as the dispatches you've been closing. Specifically:

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current` before committing.
2. **Run `git pull --ff-only origin main`** before starting in case anything lands while you work. (Cerebro is running a partial-rollback bisection in his uncommitted working tree; he is NOT committing. So no conflict expected.)
3. **Do NOT touch Cerebro's working tree changes.** They're modifications to `kernel/src/`, `tools/xtask/src/`, `docs/ADR-013-phase-2-smp-retrofit.md`. Leave them alone.
4. **Do NOT touch `target/verify/`.** Those are active investigation artifacts.
5. **Do NOT touch `Plans/`.** That directory is maintained separately.
6. **OBSERVE before writing.** §2 below is the mandatory check.

---

## 1. What this dispatch does

Two small, disciplined changes:

1. Add patterns to `.gitignore` to stop accidentally-tracked-looking scratch files from showing up in `git status` noise.
2. Delete two zero-byte placeholder files at repo root that are clearly accidents (likely from redirected commands that produced no stdout).

That's it. No content reorganization, no boot-log relocation, no historical artifact deletion. The boot-log files stay where they are; they just stop polluting `git status` once gitignored.

---

## 2. What this dispatch is NOT

- **Not a deletion of historical artifacts.** The `boot-log-*.txt` files are kept on disk; only their `git status` visibility is changed.
- **Not a working-tree reset.** Untracked files stay where they are unless explicitly listed for deletion in §3.2.
- **Not touching `Cerebro`'s in-flight work** (kernel sources, xtask, ADR-013).
- **Not touching `target/verify/`** (the EXP-* logs from the in-flight bisection).
- **Not a reorganization of `Plans/` or `docs/`.**
- **Not normative.** No Bible, ADR, or ANSPEC changes.

---

## 3. The specific changes

### 3.1 `.gitignore` additions

Read the current `.gitignore`. As of dispatch drafting, it contains:

```
/target
/.cargo-home
**/*.rs.bk
*.iso
*.img
*.bin
.DS_Store
.idea/
.vscode/
```

Add the following patterns at the end of the file (or in a new section — see §4 below for shape):

```
# Scratch artifacts — boot logs, build outputs, ad-hoc text capture at repo root.
# Kept on disk for local reference; not committed.
boot-log-*.txt
baseline-build.txt
build-*.txt

# Tool config directories for AI coders / IDE assistants
.qwen/
```

The reasoning, briefly:

- `boot-log-*.txt` matches the 8 boot-log files Cerebro produces during sub-commit verification. They're irreplaceable for debugging in the moment, but never committed; they shouldn't be in `git status`.
- `baseline-build.txt` and `build-*.txt` are zero-byte or short scratch outputs from `cargo k > file.txt`-style redirects. The two specific files at root today (`baseline-build.txt`, `build-1a.txt`) match.
- `.qwen/` is a tool config directory similar to `.idea/` and `.vscode/`. Cerebro's tool stores per-project allow-rules there.

If the current `.gitignore` already contains any of these patterns (e.g., a prior cleanup landed without me knowing), do NOT duplicate. Surface in close report.

### 3.2 Delete two zero-byte files

Two files at repo root are zero bytes. They were created by redirected commands that produced no stdout (e.g., `cargo k > baseline-build.txt 2>&1` where the actual output went to stderr). They serve no purpose:

```
baseline-build.txt   (0 bytes, 5/5/2026 8:44:27 PM)
build-1a.txt         (0 bytes, 5/5/2026 8:40:26 PM)
```

Delete both. They are NOT tracked by git, so the deletion does not appear in `git diff`; it just removes them from the working tree.

```powershell
Remove-Item baseline-build.txt -Force
Remove-Item build-1a.txt -Force
```

Verify both are gone:

```powershell
Test-Path baseline-build.txt   # should return False
Test-Path build-1a.txt         # should return False
```

---

## 4. What you do NOT delete

- **Do NOT delete the `boot-log-*.txt` files.** They are historical artifacts of sub-commit verification. They aren't in git, but they're irreplaceable for "what did the kernel emit at that exact moment in BUILD-3 progression." Once gitignored per §3.1, they stop showing in `git status` noise — that's the entire goal. Leave them on disk.
- **Do NOT delete `.qwen/settings.json` or `.qwen/settings.json.orig`.** Those are Cerebro's tool config; he uses them. Once `.qwen/` is gitignored, they stop appearing in `git status`.
- **Do NOT delete anything in `target/verify/`.** Cerebro's investigation depends on those logs.
- **Do NOT delete or move anything in `Plans/`.**
- **Do NOT delete or move anything in `docs/`.**
- **Do NOT touch `CLAUDE.md` or `README.md`** at root. Those are tracked, intentional documentation.

If you find the urge to delete more than what §3.2 specifies, **stop and surface to PM.** Each candidate deletion is its own decision; bundling creates blast radius for what should be a tight commit.

---

## 5. Pre-implementation OBSERVE pass

Before making changes:

1. **`git status --untracked-files=all`** — confirm the two zero-byte files exist and are untracked.
2. **`Get-Item baseline-build.txt build-1a.txt | Select-Object Name, Length`** — confirm both are zero bytes. If either is non-zero, **stop and surface**. Non-zero content means the file isn't an accident — it has actual data, and we need to think about it before deleting.
3. **`Get-Content .gitignore`** — read the current `.gitignore` content. Confirm the patterns in §3.1 are not already present.
4. **`Test-Path .qwen/`** — confirm the `.qwen/` directory exists at repo root. If it doesn't, the gitignore entry for it is harmless but unnecessary; include it anyway for forward-compat.
5. **`git ls-files boot-log-1a.txt`** — confirm boot-log files are NOT tracked. If any boot-log is somehow tracked (e.g., committed by accident in a previous round), stop and surface — gitignoring tracked files doesn't remove them from the index.

If anything contradicts this dispatch, surface in close report rather than silently rewriting.

---

## 6. Verification gates

### 6.1 Tracked-file diff confirmation

```
git diff --stat HEAD
```

Expected: exactly one file changed (`.gitignore`). The two file deletions are NOT in the diff because the files were untracked; they vanish from the working tree without leaving a trace in `git diff`.

### 6.2 `git status` should be cleaner

After commit:

```
git status --untracked-files=all
```

Expected: `boot-log-*.txt`, `baseline-build.txt` (now gone, but the pattern catches future ones), `build-*.txt`, and `.qwen/` no longer appear in the untracked list. Cerebro's working-tree modifications (the `M docs/...` and `M kernel/...` lines from his bisection work) MUST still appear — those are tracked files with modifications, unrelated to gitignore.

### 6.3 Zero-byte files gone

```
Test-Path baseline-build.txt   # False
Test-Path build-1a.txt         # False
```

### 6.4 Boot logs still on disk

```
Test-Path boot-log-1a.txt              # True
Test-Path boot-log-2-restore-smp4.txt  # True
```

(They are gitignored now, but still present on disk for reference.)

### 6.5 Commit shape

Single commit on `main`. Suggested commit message:

```
chore: gitignore scratch text files; delete two zero-byte placeholders

The repo root has accumulated scratch artifacts from sub-commit
verification (boot-log-*.txt) and tool config (.qwen/), all untracked.
git status noise was making it harder to spot real changes.

Add gitignore patterns:
  - boot-log-*.txt — verification capture; kept on disk, not committed
  - baseline-build.txt, build-*.txt — short scratch outputs from
    redirected build commands
  - .qwen/ — AI coder tool config (parallel to .idea/, .vscode/)

Delete two zero-byte placeholder files at repo root:
  - baseline-build.txt
  - build-1a.txt

Both were created by redirected commands that produced no stdout.
They serve no purpose. Boot-log-*.txt files (which DO contain useful
content) are kept on disk; gitignore just stops them appearing in
git status.
```

Adjust to match what you actually do. The substance must reflect the changes.

---

## 7. If your OBSERVE pass finds discrepancies

Possible surprises and what to do about each:

- **`baseline-build.txt` or `build-1a.txt` has non-zero content.** Stop. The zero-byte assumption is wrong. Surface the actual content and PM decides whether deletion is still right.
- **`.gitignore` already contains `boot-log-*.txt` or similar.** Stop. Surface — possibly a prior cleanup partially landed and we need to pick up where it left off, not duplicate.
- **A boot-log file IS tracked by git.** Stop. Surface. Removing from the index requires `git rm --cached`, a separate concern from gitignore.
- **`.qwen/` doesn't exist.** Harmless; include the gitignore entry anyway.
- **Cerebro's working-tree changes have grown to include something at repo root.** Stop. Surface. Don't accidentally interact with his in-flight work.

Surfacing is always the right move when reality doesn't match expectations.

---

## 8. Why this matters

Working-directory hygiene matters when sessions span hours and multiple investigators. `git status` noise makes it harder to spot real changes. Cerebro's bisection produces important diffs in `kernel/src/` and `docs/ADR-013-...md`; those need to stand out, not blend into 14 lines of untracked-file noise.

This is bookkeeping, like the `Plans/INDEX.md` dispatch was. Small, low-risk, pays off on every subsequent session.

This is parallel-safe with Cerebro's bisection — different files, different concern, no shared state.

---

**End of dispatch.**

*Standard close report shape: confirm `.gitignore` was modified, confirm two zero-byte files are gone, confirm boot-logs still on disk, confirm `git status` is cleaner. Paste the commit hash. ~10–15 minutes if you're efficient.*
