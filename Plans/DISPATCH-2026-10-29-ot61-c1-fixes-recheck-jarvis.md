<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #131) → Jarvis: re-check of your C1.2 and C1.3b FAILs, fixed by #184

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your own findings, on SMP-core code.

#184 merged after batch 13 (run `40799093977`, green at `0395982f`) as `7636372b`. Review it on `main`:
`git show 7636372b --stat` and `git show 7636372b`. Your RETURNs:
`Plans/RETURN-2026-10-27-ot61-c1.2-tls-review-jarvis.md` (items 2, 3, 7) and
`Plans/RETURN-2026-10-27-ot61-c1.3b-review-jarvis.md` (findings 1 and 2). Everything is under `browser-runtime`, which
no product image builds.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, a concrete input for any fail):
`Plans/RETURN-2026-10-29-ot61-c1-fixes-recheck-jarvis.md`.

1. **C1.2.** After a user `mov fs, <selector>`, does every return to ring 3 have the hardware FS.base equal to
   `Thread.fs_base`? Check `load_user_fs_base` in both prep paths and `set_current_fs_base`. Is any per-CPU FS state
   left?
2. **Finding 1.** Does every kernel caller of `wait_on_key` return `ESRCH` to the syscall boundary, with nothing it
   owns still held?
3. **Finding 2 (CP-93).** From the final `retire_current_if_dying` in `syscall_dispatch` to SYSRET or IRETQ, can
   anything set IF=1?
4. **The discriminators.** Do `threads_tls` check 8 and `threads_exit_group` roles 5 and 6 fail without their fixes?
   See the PR's falsifier table.
5. **The kill-switch.** With `browser-runtime` off, the kernel `.text` and `.rodata` are identical. Only panic-location
   line numbers differ in `.data`.

```
MODEL: GPT-6.1-Sol. EFFORT: medium. (A narrow re-check of your own SMP-core findings.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot61-c1-fixes-recheck-jarvis
          git fetch origin && git checkout -B codex/ot61-c1-fixes-recheck-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot61-c1-fixes-recheck-jarvis.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
