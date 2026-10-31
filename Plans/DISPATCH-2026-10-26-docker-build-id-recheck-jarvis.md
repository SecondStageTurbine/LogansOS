<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #76): narrow re-check of your relay #74 item 3, after #138 → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. This is a narrow re-check of your own finding. Relay #74 item 3 showed
that a Docker build on the Windows host embeds `unknown-build-id`: the bind-mounted repo is owned by `0:0`, the
container user is `neo` (1000), and git refuses the tree as dubious ownership. #138 (S2) sets `safe.directory` for
`/workspace` through git's `GIT_CONFIG_*` environment variables, and `build.rs` now keeps re-running while git can't
answer. This needs a run on the real host, which is why it comes to you.

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `c3aae11c` (main; the squash of PR #138, merged from the batch-1 gate
`2da79dd3`, run `40363142380`, green).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/docker-build-id-recheck-jarvis
          git fetch origin && git checkout -B codex/docker-build-id-recheck-jarvis c3aae11c5953f5058fa476ecd9dfd6c93c0a0bde

git show origin/main:Plans/DISPATCH-2026-10-26-docker-build-id-recheck-jarvis.md   # this file
git show origin/main:Plans/RETURN-2026-10-26-build-id-review-jarvis.md              # your relay #74
git diff dde0d988 c3aae11c -- Dockerfile docker-compose.yml kernel/build.rs
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give the command and the wrong output.

1. **The real host.** In your worktree on the Windows Docker host, run
   `docker compose run --rm neo git rev-parse HEAD`. It prints the commit, not a "dubious ownership" error. Then run a
   kernel build in the container (`docker compose run --rm neo cargo xtask build`, or the plain kernel cargo build you
   used for #74). Its build id is the commit (`strings` on the kernel ELF, or the banner line in a QEMU boot).
2. **Every service.** All three services in `docker-compose.yml` set the three `GIT_CONFIG_*` variables. The
   `Dockerfile` sets the same, so `docker run` without compose also works. The value names only `/workspace`, not `*`.
3. **No config file changed.** Nothing writes to a gitconfig, on the host or in the image. A host git outside Docker
   behaves as before.
4. **The retry in `build.rs`.** When git can't answer, `build.rs` names a `rerun-if-changed` path that never exists,
   so cargo re-runs the script on every build until git answers. When git does answer, that path isn't named, so a
   plain incremental build doesn't re-run the script. Check both on the host if you can.
5. **CI unchanged.** CI's Docker job (it matches the host uid, `.github/workflows/ci.yml`) and its checkout builds still
   get the real id.

## Close condition

- Commit `Plans/RETURN-2026-10-26-docker-build-id-recheck-jarvis.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
