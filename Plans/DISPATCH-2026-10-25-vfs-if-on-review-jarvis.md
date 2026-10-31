<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #58): post-merge review of the VFS syscalls running IF-on (#121) → Jarvis

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. It changes the interrupt state the file syscalls run under, and the lock
ordering around NeoFS `entries` and `MOUNTS` (SMP/IF core). It's Claude-authored, so it gets a Codex-family review.
**Do this after relay #55.**

**Type:** review only, post-merge (merge on green, review after, revert-first on a confirmed FAIL). **Deliverable:** one
pushed RETURN. **Reviewed SHA:** `a790c1f4` (main; the squash of PR #121, gated as `cf702601`, run `40188807691`,
23/23).

## Setup

```
WORKTREE: E:\AI Automation\workers\jarvis    (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/vfs-if-on-review-jarvis
          git fetch origin && git checkout -B codex/vfs-if-on-review-jarvis a790c1f41fecc42cf1a6a7fba0ecbb645496d41e

git show origin/main:Plans/DISPATCH-2026-10-25-vfs-if-on-review-jarvis.md   # this file
git show origin/main:Plans/NOTE-2026-10-25-vfs-syscalls-if-on.md
git diff 1a094c1d a790c1f4 -- kernel tools docs/concurrency-pitfalls.md
```

Read `docs/concurrency-pitfalls.md` CP-37, CP-56, CP-71 and CP-82 first.

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='vfs_body_if_on sys_fsync sys_open MOUNTS entries'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete sequence (which CPU/thread holds what, at
which IF).

1. **The interrupt-on window.** In `sys_fsync`, `sys_open`, `sys_readdir` and `sys_ftruncate`, only the NeoFS/VFS body
   runs with interrupts on. User-memory copies and handle-table updates stay at the syscall's IF, and the caller's IF
   is restored on every return path, errors included.
2. **No lock held across the switch.** No caller holds an interrupt-safe lock, or a lock an ISR takes, when
   `vfs_body_if_on` turns interrupts on.
3. **MOUNTS.** In the interrupts-on open path, every `MOUNTS` section runs under `without_interrupts`, so `MOUNTS` is
   never held preemptibly while mount, umount or model provenance can take it at IF=0. Check every `MOUNTS` access the
   new window can reach.
4. **`entries`.** After this change, every acquisition of NeoFS `entries` is either IF-on and preemptible on all paths,
   or uses a discipline that makes a same-CPU spin on a preempted holder impossible. List any path that still takes
   `entries` at IF=0.
5. **The scenario.** `phase3_neofs_syscall_if_on` puts a same-CPU `entries` holder in place for each of the four calls,
   asserts that each call completes no earlier than the holder's release, and fails or hangs without the change (the
   control `neo-test-vfs-syscall-if-off-control`). The control isn't in the gate list.
6. **Items left open.** The NOTE's (a) (model load / tensor write at IF=0) and (b) (umount / add_child dropping
   NeoFsState under MOUNTS) are real and correctly described. Say if you see another path of the same shape.

## Close condition

- Commit `Plans/RETURN-2026-10-25-vfs-if-on-review-jarvis.md` on your branch and **push it**, with PASS/FAIL per item
  and an overall verdict. Paste the SHA to the PL.
- No build is needed. A FAIL is reported only after you've checked it yourself.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
