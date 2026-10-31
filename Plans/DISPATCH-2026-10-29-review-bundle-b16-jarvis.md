<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #153–#156) → Jarvis: copy bounds, OT-90 P2, the editor core, servo-host-smoke

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). On their own:
#153 would be Sol medium, and #154–#156 GPT-6-Luna, high or medium.

All four merged after batch 16 (run `40843436986`, green at `fb039fbd`). Review each squash on `main`:
`git show <sha> --stat` and `git show <sha>`. One pushed RETURN per relay, each a plain checklist (pass or fail, the
enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-<slug>-review-jarvis.md`.

## #153: #201 `1faa0bec`, the copy-bounds fix: a cross-check of the 25-site table. Slug `copy-user-bytes`

Muse does the security review (#150). Yours is the independent check of the table in the PR body.
1. For each of the 25 `copy_user_bytes` call sites in `kernel/src/arch/x86_64/syscall.rs`, is the bound in the table
   the one the code enforces, and is it checked before the copy?
2. Does every caller handle the new `Err` (ENOMEM) without a partial side effect?
3. Does `user_copy_bounds` fail on the old code? The PR says it panics with "memory allocation of 1073741824 bytes
   failed".

## #154: #195 `83d8e74f`, OT-90 P2, the S5 write and the reset chain. Slug `ot90-p2`

Behind `power-control`, off by default. Before the merge, a two-lens Claude AAG found F1–F5, which are fixed (the PR
body has an F1–F5 table). Design: `docs/THINK-2026-10-27-ot90-shutdown-restart.md` §3–§4, revision 6.
1. **The PM-timer wait** (`power_logic::pm_wait`, F1). Does it end on 24-bit and 32-bit timers for every wait
   length, and does `long_waits_end_on_a_24_bit_pm_timer` fail on the old start-to-now difference?
2. **Register access.** Are the PM1a/PM1b write order, the SLP_TYP/SLP_EN composition, the 16-bit port checks, the
   PCI-config bus-0 check, and `phys_ok` for SystemMemory (F2) right? Are register pages premapped at init, so the S5
   path never maps (F3)?
3. **Interrupts.** Is IF masked only for each run of writes, and reopened for the waits and the halt (CP-64)?
4. **The scenarios' judge** (`tools/xtask/src/qmp.rs`). It requires exactly the expected boots (F4) and the 6 s
   wait-ended line (F5). Can any scenario pass vacuously?
5. **The kill-switch.** With `power-control` off, `.text` is identical for default, desktop and product (the PR's
   table).

## #155: #196 `57f23455`, OT-67 E1, `neo-edit-core`. Slug `ot67-e1`

Host-only, in no image. The PR body has a 12-item checklist with `file:line` and the covering test for each: gap
boundaries, the line index, the single apply path and undo/redo, the history bound, coalescing breaks, the UAX #29
stops (including CR LF at a line start), the raw Unicode 17 file and its sha256, the text test and byte-exact round
trips, find, wrap, `no_std` and staying out of the kernel, and the prior-art row's claims. Work through all twelve.

## #156: #199 `81d72147`, OT-61 C2.3a, servo-host-smoke and CP-94. Slug `ot61-c2.3a`

1. Can `SyscallHeap` (`neo-std-pal`) hand out memory outside a region it mapped, or extend a region by a map that
   doesn't start at its top? (CP-94)
2. Does `dealloc` always reach the region that allocated the pointer, and do nothing for a pointer no region owns?
3. Does every role check in `servo_host.rs` fail when its property is false? For example: a held RootDirCap, a
   non-empty env, a dial that succeeds.
4. Can the harness pass if the twin's abort took down the smoke, or if the smoke finished before the twin died?

```
MODEL: GPT-6.1-Sol. EFFORT: high. (A bundle: one model throughout, per the bundle rule.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/review-bundle-2026-10-29c-jarvis
          git fetch origin && git checkout -B codex/review-bundle-2026-10-29c-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-review-bundle-b16-jarvis.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push all four RETURNs. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
