<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #59): post-merge review of the MOUNTS graveyard (#122) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. This changes lock discipline around the global mount table, and where a
filesystem's teardown (which parks and joins a kthread) runs relative to that lock. That's SMP/IF core, and the code is
Claude-authored, so it gets a Codex-family review. **Do this after relays #53 and #56.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `b12df62a` (main; the squash of PR #122, gated as `e744701c`, run `40194763872`,
23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad      (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/mounts-graveyard-review-chad
          git fetch origin && git checkout -B codex/mounts-graveyard-review-chad b12df62ae5dd056c602a7791b683d4b4bdfff9e2

git show origin/main:Plans/DISPATCH-2026-10-25-mounts-graveyard-review-chad.md   # this file
git show origin/main:Plans/NOTE-2026-10-25-vfs-syscalls-if-on.md
git diff 27a1ad30 b12df62a -- kernel tools docs/concurrency-pitfalls.md
```

Read `docs/concurrency-pitfalls.md` CP-37 and CP-56 first.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='lock_mounts MountsGuard graveyard remove_child install_root_child'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete sequence.

1. **One way in.** Every acquisition of `MOUNTS` in the tree goes through `lock_mounts()` / `MountsGuard`, with no
   direct `MOUNTS.lock()` left.
2. **Displaced entries go to the graveyard.** Every `MountTable` method that can drop a filesystem (`remove_child`,
   `install_root_child`, `add_child`, `set_neofs_typed`, and anything else that overwrites or removes an entry) parks
   the displaced value in the graveyard instead of dropping it in place.
3. **Unlock, then drop.** `MountsGuard::drop` releases `MOUNTS` before dropping the graveyard, on every exit path,
   early returns and `?` included. No graveyard value can survive to a later guard, and no NeoFsState is dropped while
   `MOUNTS` is held.
4. **The `add_child` claim.** The PR says `add_child`'s `AlreadyExists` drop is never the last NeoFS reference, because
   `vfs_mount_at_parent`'s `_neofs_typed` outlives the guard. Confirm that, and say whether the path is covered anyway.
5. **The scenario.** `phase3_umount_drop_outside_mounts` holds a same-CPU NeoFsState drop with interrupts on and
   asserts that `MOUNTS` can be taken at IF=0 while that drop is still paused. The control
   `neo-test-mounts-drop-under-lock-control` hangs, and the control isn't in the gate list.
6. **No new lock order.** Dropping the graveyard after the unlock doesn't create a new ordering between `MOUNTS` and the
   NeoFS locks, or the writeback kthread's join, that another path takes in the reverse order.

## Close condition

- Commit `Plans/RETURN-2026-10-25-mounts-graveyard-review-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
