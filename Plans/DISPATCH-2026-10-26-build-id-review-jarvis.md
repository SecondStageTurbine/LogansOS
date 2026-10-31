<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #74): post-merge review of the kernel build id following HEAD (#136) → Jarvis

**MODEL:** GPT-5.6-Luna. **EFFORT:** medium. This is build tooling only: `kernel/build.rs` and xtask, with no kernel
runtime code. It's Claude-authored, so it gets a Codex-family review; the light tier fits. **Do this after relay #67**
(done).

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `093e1c9c` (main; the squash of PR #136, gated as `c572f7ae`, run `40276380128`,
green).

**Why it exists.** S4 saw an incremental kernel build embed the *previous* commit's build id. `build.rs` only re-ran
when `NEO_BUILD_ID` changed, not when HEAD moved.

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/build-id-review-jarvis
          git fetch origin && git checkout -B codex/build-id-review-jarvis 093e1c9c9b57d3bf9428f2ead7e2e23b2fb682d1

git show origin/main:Plans/DISPATCH-2026-10-26-build-id-review-jarvis.md   # this file
git diff 00fd2ab2 093e1c9c -- kernel tools
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give the commands and the wrong id.

1. **xtask passes the id.** Every xtask kernel build (`build_member_at_opt_level`) sets `NEO_BUILD_ID` from
   `git rev-parse HEAD`. With tracked changes, the id is the first 26 characters plus `-dirty`, which fits the
   32-byte log sink. Untracked files don't make it dirty. A caller's own `NEO_BUILD_ID` still wins.
2. **Plain cargo builds re-run.** `build.rs` asks cargo to re-run when HEAD, `logs/HEAD`, the ref HEAD names, or
   `packed-refs` change. It resolves them with `git rev-parse --git-path`, so worktrees work. It names only paths
   that exist, so nothing forces a rebuild every time.
3. **No repo, CI and Docker.** With no git repo, the build still succeeds and gives no id (or a stated placeholder).
   Check how CI's checkout and the Docker build (the repo mounted in the container) behave: no failure, and the
   right id.
4. **The tests.** `tools/xtask/src/build_id_tests.rs` builds a scratch repo and checks four cases:
   - two commits give two ids;
   - checking out the first commit again gives its id back;
   - a tracked edit gives a `-dirty` id within 32 bytes;
   - no repo gives no id.
5. **Merge with #134.** In `build_member_at_opt_level`, #134's `target_rustflags` block and this build-id block both
   survived the merge. The image's `-Zub-checks=no` still reaches the kernel build.

## Close condition

- Commit `Plans/RETURN-2026-10-26-build-id-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- You may run `cargo test -p xtask build_id` if you want. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
