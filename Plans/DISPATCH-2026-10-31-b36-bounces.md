<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Batch 36 bounces: #253 (seat B), #259 and #260 (Dexter)

The Claude AAG on batch 36 (`claude/pm-gate-2` at `c2e379bc`) found two confirmed FAILs on #253 and two doc
errors on #259. The PM re-checked each in code before bouncing. #248 and #260 had notes only.

**For every branch below: rebase onto current `origin/main` before pushing.** Main has moved (#246 and the
`site/` changes). A branch cut before the `site/` changes must not touch `site/`; check `git diff --stat
origin/main...HEAD` shows no `site/` path before you push. Then run `scripts/worker-close-gate.sh`.

## Seat B: #253 (the action log, head `124ea1e7`), two FAILs

1. **Chain repair breaks for any log over 4 KiB.** `action_log.rs:156-159` rewrites the good prefix with
   `login::write_whole`, which does one `sys_write` (`login.rs:237`). The kernel rejects any write over
   `MAX_FILE_IO` = 4096 with EINVAL (`kernel/src/arch/x86_64/syscall.rs:6723`). Input: about 50 records (5 KiB),
   power cut mid-append leaves a torn tail; every later append finds the break, the rewrite fails, and the log is
   stuck for good. Fix: write in chunks of at most 4096 (a `write_all` loop), then truncate and fsync. Add a test
   or boot scenario with a log over 4 KiB and a torn tail.
2. **An "Always" grant can be saved with no log entry.** In `permission.rs` serve, `record()` runs right after the
   prompt (line ~272), but `ALLOWED_ALWAYS` is only noted after `derive()` succeeds (line ~305); the derive-failure
   path (line ~292) calls `log()` but not `note()`. Input: read `/neofs/nonexistent.txt`, press 2: the grant is
   saved and the log holds only `Asked`. Fix: note the answer before `derive()`, or record only after it succeeds,
   and note the refusal on the failure path.

Notes to answer in the PR (fix or say why not): append failures are ignored and the action still runs; dropping
whole records off the end is undetectable; a mid-file break drops every later valid record; at the 1 MiB cap the
log stops instead of rotating (THINK section 2 promised rotation); revokes at the next request or session end are
not logged (kind 8); non-UTF-8 intent text is dropped silently.

## Seat B: #248 follow-ups (no bounce; a follow-up PR or a reply on #248)

- Enter typed ahead can answer the prompt as Allow before the user sees it. Ignore input queued before the prompt
  was drawn, or require a fresh key-down.
- A standing ("Always") grant is still lent while the screen is locked.
- Early-error returns in serve skip `permission::end`.

## Section 207: Dexter, fix #259's doc errors (branch `dexter/sdk-abi-refresh`, head `fc663850`)
1. `SYS_THREAD_JOIN` returns 0 and writes the exit code through `exit_out`
   (`kernel/src/arch/x86_64/syscall.rs` around 7595-7622). The doc says it returns the exit code.
2. `SYS_GETRANDOM` also returns EINVAL (len over the max), EFAULT and ENOMEM (around line 5492). The doc lists only
   ENOSYS.
3. `SYS_FUTEX_WAIT` is missing EFAULT for an address that doesn't translate (around line 7271).
4. Re-check every other row's error list against the kernel the same way while you're there, and list any other
   corrections in the PR.

Rebase onto `origin/main`, fix, push to the same branch, and reply with the new head SHA.

## Section 208: Dexter, rebase and re-measure #260 (branch `dexter/size-budgets`, head `e0b8121e`)
1. Rebase onto `origin/main` (about 990 lines of kernel and userland code landed since your base) and re-measure
   `.text` and the ISO size. Report both numbers and the headroom left under each budget.
2. The blocking job now downloads xorriso with `apt-get download` and clones Limine on every run, and `|| true`
   hides a failed download. Make a failed download fail the step loudly (Limine is already pinned to `v8.6.0-binary`; keep it).
3. Confirm the job still fits its 30-minute timeout (cite the CI run's step times).

Push to the same branch and reply with the new head SHA.

## Seat B: #248 bounce (Dexter 203, PM-verified), which also blocks #253

**F1 (blocking): a client can park the WM and keep the lock screen out of service.** The WM's main loop polls for the
`OP_PERMISSION` header, but `permission::serve` then reads the path with a blocking `sys_recv`
(`userland/neo-init/src/desktop_session/permission.rs:201` at `8b7511c6`), before any lock check. #253 adds a second
blocking read for the intent. Input: send the 64-byte header with `path_len = 1` and withhold the byte. The WM parks
in `serve`; `lock::service` never runs again, so the idle lock can't engage, and if the lock is already up its
password field is never served. (Super+L's kernel route still protects the keys; this is the lock's availability.)
- Fix: read the path and intent with `SYS_TRY_RECV` in a bounded loop that keeps calling `lock::service` /
  `lock::guard_input`, as `recv_control` does (`desktop_session.rs:2600-2618`). Or carry the path and intent in the
  same message as the header, so there is no second read at all. Refuse the request if it's incomplete.
- Add a scenario leg: a header without its path, then Super+L and the idle lock, which must still lock and unlock.

**F2: the prompt's route restore ignores the escaped state.** `ask` always reactivates the client
(`permission.rs:396-401`). After Super+Escape (`escaped = true`), an answered prompt gives the client the keyboard
while the window is drawn unfocused. Restore through `home::swap_to` / `lock::note_route`, and add the leg.

**Item 5: the CP table.** The PR says the pitfall bank doesn't apply. F1 is CP-57 (a blocking wait in a server
path) and CP-97 (a security mode held in a user-space server a client can park). Add the CP-57/CP-97 rows to the
PR body: introduced, now guarded, by which leg.

Notes (fix or answer in the PR): the `identity` word in Permission/PermissionDone is never compared with the
session's; "Always" returns GRANTED_ALWAYS even when `record()` fails at its 4 KiB / MAX_GRANTS cap.

Then rebase #253 on the fixed #248 (or fold F1's intent read into the same fix) and push both. Check that neither
branch touches `site/`.
