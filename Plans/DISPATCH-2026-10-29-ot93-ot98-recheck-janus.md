<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW BUNDLE (relays #179–#180) → Janus: narrow re-checks of #149 (#204) and #174 (#219)

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. A bundle, so one model throughout (CLAUDE.md bundle rule). On their own these
are narrow re-checks of your own findings (Sol medium), but they're SMP-core kill and wait code that must clear a
cross-family lens **before** merge.

**Updated 10/30: review the fixed heads.** Both PRs bounced once more on the PM AAG (see #219's PR comment: a deferred kill that hangs `sys_wait`, the reap-window ordering, the IPC transit window) and seat A pushed fixes. Fetch them:
`git fetch origin +pull/204/head:pr204 +pull/219/head:pr219`. #204's head is `22c4a830`; #219's is `12bfae54`, stacked on
#204. Read each PR's `RETURN (seat A):` comment for the claimed fixes.

## #179: your #149 on #204 (`Plans/RETURN-2026-10-29-ot93-aag-janus.md`). Slug `ot93-recheck`

Say CLOSED or STILL OPEN, with the enforcing `file:line` and, for STILL OPEN, a concrete interleaving:
1. F2: neither an existing `sys_wait` waiter nor a new wait completes until every surviving thread of the killed
   process has left the kernel and been reaped (`DYING`, `wait_after_survivor_reaped=1`).
2. F1, now OT-99: is it filed, and do CP-33 and CP-85/86 no longer claim bounded device cancellation?
3. Anything new in the diff since `42be6f81`, the head you reviewed.

## #180: your #174 on #219 (`Plans/RETURN-2026-10-29-ot98-aag-janus.md`). Slug `ot98-recheck`

1. F1: only the winning exit claim writes the exit code. Does a second SYS_EXIT, a fault exit or a forced kill that
   loses change nothing, and does the reaper read the winner's code?
2. F2: is a waiter between extraction from its target's queue and becoming Ready now discoverable by a concurrent
   kill, or handled explicitly, in both of your schedules (the one-CPU preempted reaper, and the cross-CPU
   unregistered target)?
3. F3: is CP-91's interrupts-on Arc drop on SYS_KILL enforced, or is the claim corrected?
4. Leg C: is a child Running on another CPU inside SYS_EXIT, before `record_exit_code`, counted as a survivor, so the
   kill defers? Is the wrong "every user thread runs on the BSP" comment gone?
5. Run `kill_at_exit` and its controls at `NEO_SMP=4` if you can, and report the counts.
6. Anything new in the diff since `82a7cffe`, the head you reviewed.

One pushed RETURN per relay: `Plans/RETURN-2026-10-29-<slug>-janus.md`. Paste both SHAs.

## Also check (both relays): the PM AAG's three items from #219's PR comment

1. A deferred kill can't hang `sys_wait` when the survivor is reaped between the survivor count and the `DYING`
   insert (the "natural reap skipped" path).
2. The `terminate` recount can't miss a thread between the reaper's `pop_front` and `finish_kill`.
3. The IPC `close_peer`/send-handoff transit window: is it covered, fixed or filed?
