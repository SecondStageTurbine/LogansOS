<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #248 L1.3a, the 203 findings' fixes: re-check (Section 213)

**Seat:** Dexter · **Reviewed at:** #248 head `6b27864a` (the dispatch's head) · **PR head now:** `ece32968`
**Branch:** `claude/l1.3-arthur-actions` · **Base:** `origin/main` = `ae3df788`
**Answers:** `Plans/DISPATCH-2026-10-31-b38-bounces-and-user-copy-fault.md` Section 213
**RAG:** healthy (`{"status":"healthy","ollama":true,"qdrant":true}`); I read the PR tree directly.
**Scope:** static review of `git diff origin/main...6b27864a` (19 files, +1568/−23). No QEMU gate and no host gate
was run for the PR; the evidence below is code inspection plus the dispatch's and PR's own recorded runs. `site/` is
not in the diff.

**Head note.** The current PR head `ece32968` is `6b27864a` plus `origin/main`'s Plans commits: `git diff
6b27864a ece32968` names only `Plans/` files (the 6b tree is missing the relay-213 dispatch and four banked RETURNs).
Every source line cited below is byte-identical at both heads. The `6b` tree is based on `6551e179`, two commits
behind the current `origin/main` (`76ab9883`, `ae3df788`); the source at `ece32968` is based on `ae3df788`. For the
source review this changes nothing; the PM's gate still has to run on the rebased branch.

## Verdict

| # | Item | Verdict |
|---|---|---|
| 1 | F1: are the path and intent reads polled and bounded, with the lock still served? Does the stall leg exercise it, and would it fail the old code? | **PASS for the path read at this head.** The intent read is not in `6b27864a` (nor in `ece32968`); it exists only on the stacked #253, where it is polled but has no stall leg. |
| 2 | F2: after Super+Escape, does an answered prompt restore the route to its real owner? | **PASS for the pre-request escape the scenario drives.** One edge is not covered: an escape pressed during the path wait is consumed and dropped. |
| 3 | Item 5: are the CP-57 / CP-97 rows present and accurate? | **FAIL (accuracy, documentation):** the rows are present in the PR body, but two claims overstate the guard. The F1 code itself is sound. |
| 4 | Notes: is `identity` checked on both messages, and is an Always that could not be saved answered as once? | **PASS.** Both checks are in code. The identity-mismatch and record-full paths have no scenario. |

---

## 1. F1: the path read is polled and bounded; the intent read is not at this head — PASS (path), test gap (#253)

**The old blocking read is gone from this path.** `permission::serve` now calls `recv_part` for the path
(`userland/neo-init/src/desktop_session/permission.rs:251`). `recv_part` (`:203-220`) uses
`libneo::sys_try_recv`; on `EAGAIN` it runs `lock::service(scene)` (`:209`), drains the pointer/SAK latch with
`scene.pointer_idle()` under `desktop-pointer` (`:210-211`), checks the deadline (`:212-214`), and yields
(`:215`). The deadline is `PART_NS` (`:186-192`): 2 s in the product, 45 s under `arthur-actions-test` for the
scenario. A missing or wrong-length message is refused with `STATUS_PERMISSION_PATH` (33) and
`PERMISSION refused reason=incomplete` (`:251-255`). That is a bounded wait, not a park.

**The lock is still served while the WM waits.** `lock::service` is the scene-aware service path: it calls
`lock_input` when the lock is due/asked for, `show` to paint the screen, and `tick` to serve the password field
(`lock.rs:242-262`; `lock_input` at `:285`, `show` at `:311`, `tick` at `:332`). `lock::guard_input` is correctly not used here: it is
the helper for waits that cannot reach the `Scene` (the control-byte and handshake waits,
`desktop_session.rs:2619-2633`); `recv_part` can and does call the full `service`. The boot test's `PART_NS` is the
long value only because the scenario must let the lock engage and be unlocked during the wait (`lock.rs:55-60`:
`IDLE_NS` is 30 s under `arthur-actions-test`).

**The intent read.** There is no intent read in #248 at either head: `atrium_protocol::Permission` is
`identity, action, path_len` (`userland/libneo/src/atrium_protocol.rs:320-325`), and `permission::serve` reads one
raw message. The "intent" is #253's L2.1 addition: its head `5ad4ab1c`
(`origin/claude/l2.1-action-log`) adds `intent_len` to the header and reads the intent with the same `recv_part`
(`permission.rs:311-315` at that head). So:

- #248's path read is fixed and bounded; **#248 cannot exercise the intent read because it has none.**
- #253's intent read is code-polled, but its scenario reuses the path-only stall leg: it sends a header with
  `path_len = 1` and withholds the path (`tools/xtask/src/login_scenarios.rs:234-243` at 6b, and the same leg on
  the L2.1 head). It never sends the path and withholds the intent, which was the 204 review's concrete input.
  **A regression that made only the intent read blocking would still pass that leg.** Add a second stall leg
  (`path_len = 1`, `intent_len = 1`, send the path byte, withhold the intent) before claiming the intent read is
  proven. This is a test-coverage gap in #253, not a code defect in #248.

**Does the new stall leg exercise F1, and would it fail the old code? Yes.** In boot 1 the script sends the
stall header, then Super+L; the expected sequence requires `LOCK locked reason=sak`, `LOCK prompt=password`,
`LOCK unlocked failures=0`, `PERMISSION refused reason=incomplete`, and
`DESKTOP arthur permission stall status=33` (`login_scenarios.rs:232-244`; expected lines `:1103-1108`). In boot 2
it sends the same header and then no key: the expected sequence requires `LOCK locked reason=idle`, the password,
`PERMISSION refused reason=incomplete`, and status 33 (`:256-262`; expected `:1145-1149`). Both lock lines are
emitted by `lock_input`/`show` from `lock::service`; the old code parked in `sys_recv` before any `lock::locked()`
or `lock::service` call, so neither line can appear and `require_serial_sequence` times out. The PR body records
the same falsification (F1 reverted to the blocking read -> timeout); I did not re-run QEMU.

**Bounds and residuals.**

- Per request the WM can be held up to `PART_NS` for the path (plus another `PART_NS` for the intent on #253),
  so a malicious client can burn 2 s of WM busy-polling per request (90 s in the test build). It is bounded; the
  `sys_sched_yield` loop is a poll, not a `sys_wait_any` deadline wait. Acceptable for this fix; if a tighter bound
  is wanted, `sys_wait_any` on the channel with the deadline would avoid the spin.
- `recv_part` maps every non-`EAGAIN` error to refusal (`:217`), so `ENOSYS` is a refusal rather than the blocking
  fallback `recv_control` has (`desktop_session.rs:2619-2633`). `SYS_TRY_RECV` is gated on the kernel
  `atrium-pointer-forward` feature (`kernel/src/arch/x86_64/syscall.rs:350`). The product image and the
  `neo-test-arthur-permission` scenario both carry that feature, but `arthur-actions` itself does not require it.
  If `arthur-actions` is ever built against a kernel without `atrium-pointer-forward`, every request with a path is
  refused. Note, not a defect of image A.
- `now_ns` uses `unwrap_or(0)` (`permission.rs:194-196`); if `CLOCK_MONOTONIC` ever failed, the deadline would
  never be reached and the loop would be unbounded. The clock is core kernel state; note only.

## 2. F2: the answered prompt restores the WM's focus after Super+Escape — PASS (pre-request escape)

**The fix.** `ask` now takes `escaped` (`permission.rs:403-410`), chooses the owner as `focus.activator` when
escaped and `client` otherwise (`:466`), and restores it through `super::home::swap_to` (`:467`) so
`lock::note_route` records the owner for an unlock. It logs `PERMISSION route restored to=wm` / `=client`
(`:468-474`). `swap_to` defers while locked and activates otherwise (`desktop_session.rs:3869-3882`;
`lock::defer_route`/`note_route` at `lock.rs:181-199`), so a lock that engaged during the prompt still restores the
right owner at unlock.

**The scenario.** `HMP_SUPER_ESCAPE` follows `DESKTOP arthur escape probe armed` (`login_scenarios.rs:220`); the
client then waits 4 s (`desktop_actions.rs:187-199`) before asking for `/neofs/escape.txt`, so the prompt comes up
with `escaped = true`. The expected sequence checks `DESKTOP pointer escape focus=spine via=sak`,
`PERMISSION prompt ...`, `PERMISSION route restored to=wm`, `PERMISSION answer=once ...`,
`PERMISSION done ...`, then `DESKTOP pointer focus client=arthur via=enter` (`login_scenarios.rs:1094-1100`). The
last line is the real discriminator: if the restore went to the client, the Enter would go to Arthur and the WM
would never log the refocus. The old code's unconditional `sys_input_focus_activate(client)` would fail both the
missing `route restored to=wm` line and the refocus line; the PR body's F2-reverted run times out.

**Uncovered edge (note, not the dispatched F2).** `escaped` is snapshotted in the `Request::Permission` arm
(`desktop_session.rs:1737-1739`) *before* `permission::serve` enters `recv_part`. If the user presses
Super+Escape *during* the path wait, `recv_part`'s `lock::service`/`pointer_idle` calls `pointer::take_sak(false)`;
`take_sak` consumes the `SAK_ESCAPE` bit and, with `arthur_focused = false`, returns `None`, so the escape is
dropped rather than applied (`desktop_session.rs:5001-5021`, `pointer::take_sak`). `escaped` stays false and the prompt
restores to the client. The same applies to Super+Tab/Space during the wait. The pre-fix parked WM would have left
the SAK bit pending for the main loop; the polled wait now consumes it. The scenario's escape is before the
request, so F2 passes; add a leg that presses Super+Escape while a stalled request is waiting if this edge is to be
covered. This is a behavior change in the wait, not a failure of the dispatched pre-request F2 fix.

## 3. Item 5: the CP-57/CP-97 rows are present but overstate — FAIL (accuracy)

The rows are in the PR body ("Concurrency pitfalls (CP table)"; `gh pr view 248`), not in the THINK. The bounce
asked for the PR body, so presence is fine. Two claims are not accurate as written.

**CP-57.** The row's class is "A blocking rendezvous wait in a server path." The canonical CP-57 in
`docs/concurrency-pitfalls.md:103` is a **server-initiated send** on a synchronous-rendezvous channel. The new
path read is a blocking **receive**, the same family of "the server parks on a client-controlled rendezvous," and
the 203 review and b36 bounce named it CP-57's shape; the row is usable, but it should say "receive" if the
"Class" column is meant to name the canonical pattern.

More important, the row says "the lend handshake (`grant_over` → `recv_control`) was already polled." The receive
halves are polled (`desktop_session.rs:2619-2633`), but `grant_over` starts with `sys_send(chan, b"O")` and
`send_reply_status` is a synchronous `sys_send` (`desktop_session.rs:2601-2606`, `:2266-2280`). A client that
sends a complete request and then does not receive parks the WM in that send; the F1 legs never test it. The row
should say "the handshake's receive halves are polled; the offer/reply send remains a synchronous send
(pre-existing CP-57; the kernel `sak-lock-route` protects key capture, not lock-screen availability)."

**CP-97.** The row says "A parked WM stopped serving the lock screen. Now the WM keeps engaging and serving the
lock throughout." That is true for `recv_part` and `ask`, and it is what the F1 legs prove. It is false for the
synchronous sends above (`send_reply_status`/`grant_over`), where the WM still parks with no `lock::service`
running. CP-97's key-capture property is guarded by the kernel route (`sak-lock-route`), which the row correctly
says was never affected; the *lock-screen availability* property is guarded only in the waits that call
`lock::service`. Scope the sentence to "throughout the path/intent wait" and name the send residual. The F1 legs do
not test the send path, so "throughout" is more than the evidence.

This is documentation accuracy, not a regression in the new path; the F1 code fix is sound. If the PR body is a
deliverable, correct it; no code change is implied.

## 4. Notes: identity on both messages; an unsaved Always answered as once — PASS

**Identity is checked on both messages.** `Request::Permission` passes `ours: request.identity == identity`
(`desktop_session.rs:1749`), and `Request::PermissionDone` checks `done.identity == identity` (`:1769`), replying
`CapRequired` on a mismatch. Both are real checks, not just the reply's identity. The `Permission` check runs
after the path read (`permission.rs:251` then `:259`), deliberately so a refused request leaves no raw path on the
channel; a mismatched identity can therefore consume one bounded `PART_NS` before refusal. That is bounded and the
channel is the client's own, so it is a note, not an authority bypass. A mismatched `PermissionDone` does not call
`permission::done`, so a lent cap stays until the next request or the session's end; the client already owns it,
so this adds no authority.

**An unsaved Always is answered as once.** `if choice == Choice::Always && !record(action, path)` logs
`record=failed` and changes the `Choice` to `Once` (`permission.rs:292-300`); the status that follows is
`STATUS_PERMISSION_GRANTED` (0), not `STATUS_PERMISSION_GRANTED_ALWAYS` (`:314-320`). The cap is still lent for
this action, and the user is not told it was remembered. This is the correct fail-closed answer.

**Test gaps.** No scenario fills the 4 KiB / `MAX_GRANTS` record to drive `record = failed`; no scenario sends a
mismatched `Permission`/`PermissionDone` identity; and, per item 1, #253 has no intent-stall leg. The five prompts
and the two boots in `login_arthur_permission` cover once/deny/always/read-only/lock-during-prompt and the F1/F2
legs well, but not these three.

## Disposition

**F1 and F2 are fixed for the dispatched defects.** The path read is polled and bounded, `lock::service` runs
during the wait, and both stall legs (Super+L and the idle lock) exercise it and fail the old blocking read. The
answered prompt restores the WM's focus after a pre-request Super+Escape, through `swap_to` so the lock records the
owner, and the scenario's Enter-refocus line is a real discriminator. The two notes (identity; unsaved Always) are
both in code.

**One documentation FAIL:** the CP-57/CP-97 rows are present but overstate the guard for the synchronous-send half;
correct the PR body (or accept the scoped wording) before citing them as proof. **No code bounce is implied by
this re-check.** The remaining test gaps are the #253 intent-stall leg, the record-full Always, the identity
mismatch, and the escape-during-wait edge.

I ran no QEMU gate and claim no gate result. The branch remains kernel-touching through its `neo-test` feature and
scenario wiring, so the 4-shard `workflow_dispatch` at the rebased head is still the PM's before merge.
