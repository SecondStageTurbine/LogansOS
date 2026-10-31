<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Runbook: per-seat coder worktrees (one tree per seat, never shared)

**Status:** STANDING OPS (PL-adopted 2026-07-20). Supersedes the ad-hoc practice of coders sharing the single `E:\AI Automation\AltimitNeo` working tree.
**Owner:** PL (runs the coder CLIs + the local `.git`). The cloud PM never touches these trees.

## Why (the hazard this kills)
Multiple CLI coders sharing **one physical working tree** collide. Three incidents in the 2026-07-20 session alone:
- Prometheus's doc commit landed on **Oracle's** branch (wrong branch checked out at commit time).
- An Oracle Slice-3 commit was pushed onto **Janus's** vault branch (shared tree + auto-push hook), reddening its CI.
- Jarvis (Slice-5) checked out the **same physical tree** Cerebro was mid-build in; Cerebro's in-flight work was only safe because it was already a committed local ref.

The failure modes are structural, not carelessness: (a) a second seat's `git checkout` silently moves the branch out from under the first seat's uncommitted work — **a machine/tree event away from losing the only copy** (see CLAUDE.md "the PC-swap near-miss"); (b) a `post-commit` **auto-push hook** on a shared tree pushes whatever branch happens to be checked out, cross-pushing one seat's commit onto another seat's branch. With five main-tier seats now able to run concurrently, two seats on one tree **will** keep colliding.

**The fix:** one persistent worktree per *seat* (not per *build*). Bounded (one folder per seat, forever), reuses each seat's own `target/` for incremental compiles, and makes the collision class impossible.

## Layout
All coder worktrees live under `E:\AI Automation\AltimitNeo\Workers\<seat>`:

```
E:\AI Automation\AltimitNeo\            <- the ANCHOR tree (leave it parked on `main`; do NOT code here)
E:\AI Automation\AltimitNeo\Workers\
    cerebro\      <- Cerebro's persistent worktree
    jarvis\       <- Jarvis's
    janus\        <- Janus's
    oracle\       <- Oracle's
    chad\         <- Chad's
    prometheus\   <- Prometheus's
```

- The **anchor tree** (`E:\AI Automation\AltimitNeo`) holds the shared `.git` and stays parked on `main` as a read-only reference — **no coder works in it.**
- `Workers/` is nested inside the anchor tree's working dir, so it is **git-ignored** (`.gitignore: /Workers/`) to keep the anchor's `git status` clean. The nested worktrees still function normally — git tracks them via `.git/worktrees/`.

> **git-on-PATH (Windows):** git is not reliably on the spawned-shell PATH on this machine — invoke by full path: `& "C:\Program Files\Git\bin\git.exe"`. Quote paths with spaces (`"E:\AI Automation\AltimitNeo\Workers\cerebro"`). Commands below are PowerShell.

## One-time setup (per seat)
From the anchor tree, create each seat's worktree once. Park each on `main` initially; the per-dispatch step re-points it.

```powershell
Set-Location "E:\AI Automation\AltimitNeo"
& "C:\Program Files\Git\bin\git.exe" fetch origin
# one line per seat (creates the folder + a worktree parked on origin/main, detached):
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\cerebro"  origin/main
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\jarvis"    origin/main
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\janus"     origin/main
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\oracle"    origin/main
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\chad"      origin/main
& "C:\Program Files\Git\bin\git.exe" worktree add --detach "E:\AI Automation\AltimitNeo\Workers\prometheus" origin/main
```

Each seat's CLI then runs **only** in its own `Workers\<seat>` folder.

## Per-dispatch workflow (each seat, in its own tree)
For every new dispatch, the seat re-bases its own tree onto fresh `main` and cuts the build branch — reusing its own `target/` (incremental, no re-clone):

```powershell
Set-Location "E:\AI Automation\AltimitNeo\Workers\cerebro"
& "C:\Program Files\Git\bin\git.exe" fetch origin
& "C:\Program Files\Git\bin\git.exe" checkout -B codex/<the-build-branch> origin/main
# ...work, commit, run the close-gate, push -u origin codex/<the-build-branch> ...
```

Because each seat is on its **own** branch in its **own** tree, no other seat's checkout or push can touch it. The close-gate's rebase-freshness step (`scripts/worker-close-gate.sh`) still applies — branch off *current* `origin/main`.

## Auto-push hook caveat
If a `post-commit` auto-push hook is used, it must push **the tree's own current branch** (`git push origin HEAD` / `@{u}`), never a hard-coded branch — and with per-seat trees it can only ever see that seat's branch anyway, which removes the cross-push vector entirely. Prefer no auto-push at all for build branches; a WIP push is free insurance (a branch push does not fire the QEMU gate) but should be explicit.

## Disk hygiene
- One tree per seat is **bounded** — six seats ⇒ six `target/` dirs, stable forever, versus the old one-folder-per-build growth (which silently ate 123 GB once; see CLAUDE.md "Disk hygiene").
- Reclaim a seat's `target/` any time with `cargo clean` in that tree (≈80% of the folder) without losing the checkout.
- Never delete a tree whose branch is unpushed/unmerged — verify the work is on `origin` first (merged-on-`main` = safe to clean; unpushed in-flight = the only copy, keep).

## Cross-references
- CLAUDE.md → "Disk hygiene — reclaim the build tree after merge" (the *structural* fix: "one reusable folder per **worker**, not per **build**" — this runbook is that fix, made concrete).
- CLAUDE.md → "Git Worktrees - PM Seat vs Worker Tree" (the cloud PM is a separate clone and does **not** share these trees; this runbook is worker-side only).
- `scripts/worker-close-gate.sh` → the rebase-fresh + full-host-check close ritual each seat still runs in its own tree.
