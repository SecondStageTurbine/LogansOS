<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #160) → Jarvis: pre-merge cross-family AAG of OT-66 L2b, the kernel lock route (#216)

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. Cross-CPU atomics on the input route (the SAK latch against the flush), in
Claude-authored kernel security policy. The cross-family lens comes **before** the merge. Muse reviews items 2, 5 and 6
separately (#161).

**What it is.** PR #216, branch `claude/ot66-l2b-lock-route`, head `bf92cd59`. It's stacked on #208 (L2), and the
author rebases it onto `main` after #208 merges. Review the branch against its base:
`git fetch origin && git diff origin/claude/ot66-l2-lock...origin/claude/ot66-l2b-lock-route`.

The lock is kernel route state, behind `sak-lock-route` (= `sak-lock` + `atrium-pointer-forward`):
- `focus::LOCK_ROUTE` is one word holding the WM's registered lock focus and a locked bit.
- `latch_sak(KEY_L)` sets the bit lock-free, before any WM code runs.
- While locked, every route read (`route_event`, `flush`, RECEIVE/POLL, waits) is the lock focus, and pointer
  forwards go only to it.
- Unlock activates the restored owner and clears the bit under `DELIVERY`, with the epoch bumped.
- The ops ride `SYS_INPUT_SAK_TAKE(op, h)` and need input-system authority.

One pushed RETURN, a plain checklist (pass or fail, the enforcing `file:line`, a concrete interleaving or input for any
fail): `Plans/RETURN-2026-10-29-l2b-aag-jarvis.md`.

1. With `sak-lock-route` on and a lock focus registered, once `latch_sak(KEY_L)` returns, no client focus's
   `try_recv`, `is_active`, `receivable` or `wait_ready` is true until `release_lock_route`. Check across CPUs.
2. A key decoded after Super+L on the same keyboard is enqueued only to the lock focus, whatever `ACTIVE_INPUT_FOCUS`
   holds. Check `stage_event`, `flush` and `route_event`, and the epoch-check order in `flush`.
3. `engage_lock_route` is lock-free (no `DELIVERY`, no other lock), so it's safe from the SAK filter under a device
   lock with IF=0.
4. `register_lock_route`, `release_lock_route` and `drop_lock_route` keep the word consistent with each other and with
   `engage` in any interleaving (one-word `fetch_update`s; the `lock_word` KATs).
5. Only the input-system holder can register, engage or release (`sys_input_sak_take_impl` checks first). A client
   can't make its own focus the lock focus, or unlock. This composes with #210 (activate requires InputSystem).
6. While locked, `forward_record` delivers to no focus but the lock focus.
7. A dying lock focus while locked leaves no client route (`drop_lock_route` under `DELIVERY`, from `Drop`).
8. With `sak-lock-route` off, nothing changes (the kill-switch table; the `cfg`s).
9. The WM falls back to L2's swaps when the op returns a take value, and keeps those bits (`lock::arm`).
10. The prior-art and pitfall sections are present, and their claims hold: CP-97, and the sway `lock.c` row.

```
MODEL: GPT-6.1-Sol. EFFORT: high. (Cross-CPU kernel security policy: the pre-merge cross-family lens.)
WORKTREE: E:\AI Automation\workers\jarvis   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/l2b-aag-jarvis
          git fetch origin && git checkout -B codex/l2b-aag-jarvis origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-l2b-aag-jarvis.md
git diff origin/claude/ot66-l2-lock...origin/claude/ot66-l2b-lock-route
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
