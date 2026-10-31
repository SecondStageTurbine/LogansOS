<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #161) → Muse: OT-66 L2b, the kernel lock route: input isolation (#216)

**Reviewed SHA:** PR #216's branch `claude/ot66-l2b-lock-route`, head `bf92cd59` (pre-merge, stacked on #208). Read the
files at that SHA from the repo, not a summary. **Deliverable:** a RETURN the PL pastes back, banked as
`Plans/RETURN-2026-10-29-l2b-review-muse.md`. Write it as a plain checklist: pass or fail, the enforcing `file:line`,
and a concrete input for any fail.

**Context:**
- The design is `docs/THINK-2026-10-27-ot66-login-session.md` §5.2 (the exclusive lock state).
- L2 (#208) held the lock in the WM's state, so a WM parked in a synchronous send to a stalled client couldn't lock
  input.
- L2b makes the lock kernel route state: Super+L sets a locked bit lock-free in the SAK latch path, and while it's
  set every route read goes to the WM's registered lock focus.
- #210, a separate PR, makes route activation require InputSystem.

1. A key decoded after Super+L is enqueued only to the lock focus, whatever `ACTIVE_INPUT_FOCUS` holds
   (`kernel/src/atrium/focus.rs`: `stage_event`, `flush`, `route_event`).
2. Only the input-system holder can register a lock focus, engage the lock or unlock
   (`sys_input_sak_take_impl`). Can a client make its own focus the lock focus, unlock, or keep the lock from engaging?
3. While locked, does any pointer event (`forward_record`) or wait wake (`wait_ready`) reach a client?
4. If the lock focus dies while locked, is no client route left (fail closed)? Can a client cause the WM's lock focus
   to die?
5. After unlock, does a key typed while locked ever reach the restored client?
