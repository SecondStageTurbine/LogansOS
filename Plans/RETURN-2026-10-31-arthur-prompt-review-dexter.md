<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #248 L1.3a (Arthur's actions behind the WM permission prompt): security review (Section 203)

**Seat:** Dexter · **Reviewed at:** #248 head `6f00f886` (the dispatch's head) · **PR head now:** `8b7511c6`
**Branch:** `dexter/arthur-prompt-review` · **Base:** `origin/main` = `924c189c`
**Answers:** `Plans/DISPATCH-2026-10-31-b35-b36-reviews.md` §203
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`). Sources read from the `pr248` tree.
**Scope:** static review of `git diff origin/main...6f00f886` (19 files, +1332/−24); I ran no gate. The branch was
rebased after the dispatch: the 19 PR files are byte-identical at `8b7511c6` (`git diff 6f00f886 8b7511c6 -- <the 19
paths>` differs only by main's advancement in `kernel/Cargo.toml`, `kernel/src/test_harness.rs`, `tools/xtask/src/
main.rs`), so these findings apply at both heads.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | A mutating action only behind a prompt the user answered with their own input; no forged answer/event | **PASS through the WM**; a direct `SYS_OPEN` still bypasses it until L1.1 — scope note 1 |
| 2 | "Read-only by default" enforced by the WM or kernel, not just Arthur's code | **PASS in the WM**; the kernel boundary is still L1.1 (note 1) |
| 3 | "Always" stores its grant WM-only under `.system/`; a client cannot add one | **PASS** |
| 4 | A client can draw a fake prompt, or the prompt can be answered while the lock screen is up | **PASS** — 4a: no; 4b: no (denied) |
| 5 | The prior-art section (CP table and survey) is present, and its claims hold | **FAIL** — survey present, licenses hold, but the "pitfall bank doesn't apply" claim is false; F1 is the CP-97/57 case (§5) |
| **F1** | Blocking finding: the prompt's path read parks the WM; the lock screen cannot engage/be served | **FAIL** (§4, blocking) |
| **F2** | Note: the prompt's route restore ignores the escaped state | note (§4) |

---

## 1. No unprompted mutating action through the WM; no forged answer — PASS

**The action path is the prompt.** The client's line must start with the verb and an absolute path
(`userland/neo-arthur/src/desktop_action_logic.rs:47-64`), and every parsed action goes through
`surface.permission(...)` before any file is touched (`userland/neo-arthur/src/desktop_actions.rs:61`). The WM answers
from the record or its own prompt (`userland/neo-init/src/desktop_session/permission.rs:228-237`); a write is refused
*before* the record or the prompt in a read-only build (`permission.rs:212-215`). A client can send
`Request::Permission { action: PERMISSION_WRITE, path_len: 10 }` for `/neofs/x` and gets status 32 with no cap; there is
no request sequence that yields a write cap in a build without `arthur-actions-write`, and that feature appears in no
scenario or image feature list (`tools/xtask/src/main.rs:6635-6660`, `:6904-6914`).

**The answer cannot be forged.** It comes only from the WM's own input focus: `ask` activates the WM's Home focus
(`permission.rs:350`), discards the queued stale keys (`:353`; `userland/neo-init/src/desktop_session.rs:3874-3890`),
then decodes events from `focus.events` (`permission.rs:377-392`). The client cannot put an event in that queue: there is
no production inject syscall (`SYS_INPUT_FOCUS_TEST_INJECT` is compiled only for `neo-test-*` scenarios,
`kernel/src/arch/x86_64/syscall.rs:318-328`), and a client cannot activate a focus at all — the kernel requires the
non-delegable `InputSystem` authority (CP-96), which the boot probe proves is `EPERM` for a client-created focus
(`userland/neo-arthur/src/showcase_surface.rs:946-970`). Handles are per-process, so a client cannot grant the WM a
handle naming the WM's own focus.

**Scope note 1 (not a defect of this PR, but the literal answer to item 1 is "yes, by direct syscall").** Until L1.1
(`cap-rooted-open`) is in image A, any client can call `SYS_OPEN`/`SYS_WRITE` directly: the root-form open only
consults a `RootDirCap` under `cap-rooted-open` (`kernel/src/arch/x86_64/syscall.rs:6391-6400`), and that feature is off
by default (`kernel/Cargo.toml:89-97`) and absent from `PRODUCT_IMAGE_KERNEL_FEATURES` (`tools/xtask/src/main.rs:6047+`;
it has `system-store` and not `file-caps`). So "read-only by default" bounds Arthur's *action path* and the WM's lending,
not a compromised Arthur. The THINK says this plainly (§2, §7) and the PR body repeats it; keep it in the ledger until
L1.1 lands.

## 2. Read-only is enforced by the WM — PASS

The refusal is in the WM, not in Arthur and not in a client-controlled flag: `if action == PERMISSION_WRITE &&
!cfg!(feature = "arthur-actions-write")` → status 32 before any prompt (`permission.rs:212-215`). The client holds no
file grant of its own at spawn; the only directory it is handed is its memory directory, derived to plain
`LOOKUP|READDIR` (`desktop_session.rs:2092-2140`). The lent cap is per action, with the action's rights
(`permission.rs:84-96`, `:173-185`), and is revoked through the cap graph at DONE (`:275-290`) and at session end
(`:293-297`). The kernel does not yet enforce a path boundary (note 1), but the item asks "WM **or** kernel": the WM
does.

## 3. "Always" is WM-only and a client cannot add one — PASS

- **The record:** `/neofs/.system/arthur-grants.json` (`permission.rs:41`).
- **WM-only by kernel, not convention:** every root-form open at or under a mount's `.system/` is `EPERM` unless the
  caller holds the `SystemStore` authority (`kernel/src/arch/x86_64/syscall.rs:8782-8795`, reached from
  `open_path_and_install` at `:6387-6390`). That cap is minted only by the kernel root path into neo-init
  (`kernel/src/sched/process.rs:1333-1341`, variant `:976-981`); it is refused at spawn (`syscall.rs:1902`) and at grant
  (`:7906`). The `login_record_guard` scenario asserts read, write, a `..` path, the directory and a new file are all
  `-1` for a non-WM process (`tools/xtask/src/login_scenarios.rs:440-462`).
- **No client-side write:** the path validator refuses `.system` as the first component (`permission.rs:98-114`), so the
  client cannot even name the record. The only way a grant is added is `record()` on `Choice::Always`
  (`permission.rs:150-170`, called at `:233`), and `Choice` comes only from the WM's key loop (`:387-390`).
- **Boot proof:** the scenario's boot 2 lends the Always path from the record with no prompt
  (`login_scenarios.rs:1083-1101`).

## 4. Fake prompt / prompt under the lock — PASS (4a, 4b), with F1 blocking in the same path

**4a. A client cannot draw the WM's prompt.** The prompt is painted into the WM's own plane (`permission.rs:362-370`)
and presented with the client layer off (`userland/neo-init/src/desktop_session/lock.rs:517-569`, `present(false, …)` at
`:552`). While `ask` runs, the WM is inside the `Request::Permission` arm (`desktop_session.rs:1735-1757`) and does not
serve the client's channel, so a commit sent during the prompt is not composited; the full-screen paint also covers the
window. A client can draw a *lookalike* in its own window before it asks — ordinary desktop phishing, not a WM bypass:
the WM's prompt is over it and the answer is read from the WM focus.

**4b. The prompt cannot be answered while the lock screen is up.** `ask` denies at entry if locked
(`permission.rs:346-349`), denies if `lock::service` engages while the prompt is up (`:373-375`), and does not restore
the client's route when locked (`:396-404`). The scenario drives exactly this: Super+L during a list prompt →
`PERMISSION prompt`, `LOCK locked reason=sak`, `PERMISSION answer=deny reason=locked`
(`login_scenarios.rs:1057-1061`).

**But the same path has a lock-availability defect — F1 (blocking).** The path is read with a blocking `SYS_RECV`
*after* the request header and *before* the `lock::locked()` check (`permission.rs:200-201`); `SYS_RECV` parks until a
matching send arrives (`kernel/src/arch/x86_64/syscall.rs:6045-6067`). A client can send the 64-byte
`OP_PERMISSION` message (`action = PERMISSION_READ`, `path_len = 1`) and withhold the one path byte. The WM parks in
`permission::serve`; `lock::service` (called at `desktop_session.rs:1490`) does not run again, so the idle lock cannot
engage. If the lock is already up, `lock::tick` (`lock.rs:328-366`) does not run either, so the password field is never
served and the user cannot unlock until the client sends the byte or dies. Super+L's kernel route still protects the key
(CP-97's `sak-lock-route`), so this is lock *availability* (screen stays unlocked / unlock denied), not key capture —
but the lock is a security control and the client that can park the WM is the party the prompt exists to contain. The
lock's own `recv_control` is polled with `lock::guard_input` for exactly this reason (`desktop_session.rs:2600-2618`);
the new receive is not. The `login_arthur_permission` client sends the path immediately in `permission()`
(`userland/neo-arthur/src/showcase_surface.rs:1107-1130`), so the scenario has no leg for a header without its path.
**Fix:** read the path with `SYS_TRY_RECV` in a loop that calls `lock::service`/`lock::guard_input` (the `recv_control`
shape), or refuse a permission request while locked before the read, and add the stall leg.

**F2 (note, same path): the prompt's route restore ignores the escaped state.** `ask` unconditionally reactivates the
client (`permission.rs:396-401`); with `desktop-pointer`, Super+Escape sets `escaped = true` and leaves the WM's focus
the route (`desktop_session.rs:1479`, `:5095-5145`). A client that sends a complete permission request while escaped,
answered by the user, gets the keyboard back at `permission.rs:397`, but `escaped` stays true and `focused` stays false
(the prompt never calls `set_focused`); `escaped_keys` then reads the WM's focus, which no longer receives
(`desktop_session.rs:5150-5163`). The user's next keys go to the client with the window drawn unfocused. Restore through
`home::swap_to`/`lock::note_route` (or pass the escaped state and re-activate the WM's focus).

## 5. Prior-art section: survey present and license claims hold; the CP claim does not — FAIL

**Present:** `docs/THINK-2026-10-30-l1.3-arthur-actions-prompt.md` §5 has the survey table (xdg-desktop-portal, Android
runtime permissions, SerenityOS `pledge`/`unveil`, Genode/Sculpt, macOS TCC), with what it does, fit, take, license and
last commit (SHA + date), and the "checked 2026-10-30 by shallow clone" line. The PR body points to it.

**Claims:** I spot-checked the xdg row against upstream raw source: `document-portal/permission-store.c` exists and its
header is `SPDX-License-Identifier: LGPL-2.1-or-later`, matching the table; the other rows' licenses are the standard
ones (Apache-2.0 AOSP, BSD-2-Clause SerenityOS, AGPL-3.0 Genode, proprietary TCC). I did not independently verify the
2026 commit SHAs/dates from here; the in-world shallow-clone line is the provenance claim.

**The CP half is false.** The PR body says "no SMP or lock code changed, so the pitfall bank doesn't apply" (section
"Prior art and pitfalls"). That is not the case: F1 is CP-57's shape (a blocking rendezvous wait in a server path,
`docs/concurrency-pitfalls.md:102-103`) and CP-97's shape (a security mode — here the lock screen's service — held only
in a user-space server, which a client can park in the kernel, `:142-144`). A CP table row for CP-57/97 ("does the
change introduce/guard/leave N/A, and which discriminator proves it") is missing, and the new path introduces the trap
unguarded. This is not a paperwork miss: F1 is what the missing row would have caught. **FAIL.**

## Outside the five (notes, no additional blocker)

1. **F1 is the blocking finding**; F2 should be fixed or explicitly scoped in the same pass. Everything else below is
   a note.
2. **Identity fields are unverified:** the `Request::Permission`/`PermissionDone` arms use the session's `identity` for
   the reply but never compare `request.identity` to it (`desktop_session.rs:1735-1769`), unlike `Request::Setting`
   (`:1855`). Not exploitable on a one-client channel; it makes the new messages' identity word decorative and would
   matter at a second client.
3. **"Always" can report persistence it did not get:** `record()` fails (returns `false`) when the record would exceed
   `RECORD_MAX` (4 KiB) or `MAX_GRANTS`, logs `record=failed` (`permission.rs:150-170`, `:233-235`), but `serve` still
   returns `STATUS_PERMISSION_GRANTED_ALWAYS` (`:250-256`). Fail-closed (no extra authority) and the grant holds for the
   session, but the user is told it was remembered when the next boot will prompt again. A 255-byte path makes the 4 KiB
   cap bite around a dozen entries.
4. **The record write happens before the cap is derived** (`permission.rs:233` then `:245`): an Always on a path that
   cannot be opened (or a file that does not exist yet) is recorded, then the open fails with status 33. The recorded
   path is still what the user saw; note only.
5. **The PR body's gate SHA is stale:** it cites `e6e59b09` on `origin/main` @ `72bc9067`; the branch is now
   `8b7511c6` on `924c189c` and the kernel feature/const change still requires the 4-shard `workflow_dispatch` at the
   rebased head before merge. Not a review finding; the PM gate must re-run.
6. **Test gaps (recommend adding with F1):** (a) header-without-path stall then Super+L/idle — F1; (b) Super+Escape,
   then a complete permission prompt answered, then a key — F2; (c) a client `SYS_OPEN` of a path outside its memory
   directory in the `file-caps` scenario, to make note 1's L1.1 dependency a falsifier rather than a paragraph. The
   scenario does cover once/deny/always/read-only/lock-during-prompt well (`login_scenarios.rs:1026-1101`).

## Disposition

**Not a security PASS as a whole.** The grant machinery is sound: I found no forged answer, no client-write path to the
record, and no unprompted write through the WM; the record is kernel-WM-only and the lent cap is revoked through the cap
graph. But **F1** is a concrete lock-availability defect a client can trigger with one request: a header without its
path parks the WM in `permission.rs:201`, the idle lock cannot engage, and while locked the password field cannot be
served. **F2** is a route-state desync on the same path. Item 5 fails because the CP table is absent and its N/A claim
is wrong — F1 is the CP-57/97 row. Recommend the branch bounce for F1 (polled path read + a stall leg) and F2; the
survey/licence content can stay as it is.

This was a code review. I ran no gate and claim no CI or QEMU result.
