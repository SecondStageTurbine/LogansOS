<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #164–#166) → Chad: NeoFS frees, C2.2b, and the threads_tls fix

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). #165 is memory
core with a new syscall, so it's Sol high anyway.

All three merged after batch 18 (run `40867249786`, green at `bcab0517`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-chad.md`.

## #164: #206 `78991d10`, NeoFS supersede-free. Slug `neofs-frees`

You reviewed #180's cut KAT (#135). Muse does the security review (#162).
1. **The cut model** (`tools/neofs-commit-logic-tests`) gains `write_then_write`. Does every cut and every failed
   flush mount, and does the model fail without deferred reuse?
2. **C1.** `free_superseded` checks the whole list (already free, named twice, block 0, past the end) before setting
   any bit. Does mount's live-set walk refuse a second owner? Check the two crafted-image scenarios.
3. **U1.** `commit_child_update` compares the generation as well as the inode block. Does the ABA stage in
   `neofs_overwrite_frees` fail without it?
4. **Coverage.** 100 overwrites keep `used` constant, and main's code adds +11 per write. Does the scenario prove
   the reuse happens, not just that the count is flat?

## #165: #205 `3bcc2f99`, OT-61 C2.2b, obj-quota. Slug `ot61-c2.2b`

The PR's checklist:
1. Is every object kind charged at its choke point (handle install, spawn with the parent paying, thread create, the
   unwaited exit-code insert, buffer pins, sockets), with the declared weights KAT-checked to be at least the real
   size?
2. Can two CPUs charging one chain leave a level above its limit?
3. Is every charge released exactly once?
4. Does `SYS_SPAWN_BUDGETED2` (0x54) leave `SYS_SPAWN_BUDGETED` untouched? Does `SYS_MEM_INFO` v2 match its byte
   oracle?
5. With `obj-quota` off, are `.text` and `.rodata` identical?

## #166: #207 `d422dfa0`, the threads_tls step-6 retry. Slug `threads-tls-retry`

A light, test-only check.
1. Is the retry bounded (16), and does it retry only when the probe reads exactly B, the value a switch-in restores?
2. Does a dead selector load still fail 8006?
3. Is any discriminator weakened?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\chad   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29d-chad
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29d-chad origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b18-chad.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all three RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
