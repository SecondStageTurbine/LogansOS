<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH — Seat A: kill vs exit (#204 OT-93, then #219 OT-98)

**Date:** 2026-10-29 · **From:** PM · **Seat:** A (a new cloud coding session; it replaces the old S5)

This seat inherits S5's lane from the other account. Read, in order:
`Plans/HANDOFF-2026-10-29-pm.md` §3–§4 (the S5 rows), `Plans/RETURN-2026-10-29-ot93-aag-janus.md` (#149),
`Plans/RETURN-2026-10-29-ot98-aag-janus.md` (#174), the PR #219 conversation (the Claude AAG FAIL-A/B), and
`docs/concurrency-pitfalls.md` CP-30, CP-33, CP-85/86, CP-91, CP-95, CP-98.

## Environment

This container has QEMU and Limine. Run scenarios as CI does:
`LIMINE_DIR=/opt/limine NEO_SMP=4 cargo neo-test <scenario>`. The local QEMU is 8.2 (CI uses 10.2). A result that
differs from CI goes in your report; don't explain it away.

## Work, in order

### 1. #204 (`claude/ot93-kill-ready-in-kernel` @ `1138d919`): finish

F2 is fixed per S5's report (`DYING` holds the process until the last survivor is reaped). Still to do:
- the PR body's pitfall table: CP-33 qualified per #149 F1 (survivors are bounded **except** an infallible device
  wait, now OT-99); CP-85/86 must not imply bounded device cancellation;
- confirm OT-99 is filed in `docs/OPEN-THREADS.md` and CP-95's wording is fixed;
- run `kill_ready_in_kernel` (it must report `wait_after_survivor_reaped=1`) and
  `neo-test-kill-wait-early-control` (it must fail) at SMP=4;
- `scripts/worker-close-gate.sh`, then push.

### 2. #219 (`claude/ot98-kill-vs-exit` @ `2de132e7`, stacked on #204)

- **Leg C, the SMP=4 panic S5 diagnosed from S1's logs** (`claude/s1-kill-at-exit-logs` @ `ee0921ab`,
  `Plans/evidence/`). Pin the harness to CPU 0 and the child to CPU 1. Hold the child Running inside `SYS_EXIT`,
  before `record_exit_code`. `terminate` must count it as a survivor and defer; on release, the waiter completes at the
  reap. The control without the survivor count must panic. Fix the wrong comment ("every user thread runs on the BSP,
  where this kill runs").
- **Janus #174 F1:** the exit code is written only by the winning claim. Serialize the winning CAS with the payload's
  publication, with an ordering argument for the reaper. Add a leg with two exit producers already past the entry
  checks (and a fault exit racing SYS_EXIT); the loser must change nothing.
- **Janus #174 F2:** a waiter extracted from its target's queue but not yet Ready must stay discoverable to the
  kill scan (or be an explicit, retired outcome). Check whether the FAIL-B fix at `2de132e7` (wake under the waiters
  lock) closes both of Janus's schedules: the one-CPU preempted reaper and the cross-CPU unregistered Q. Add a
  completion-to-Ready barrier leg either way.
- **Janus #174 F3:** SYS_KILL's production path doesn't enforce the CP-91 interrupts-on Arc-drop claim. Either hand
  the last Arc to the deferred drop or correct the claim.
- CP-98's text (add FAIL-A/B and F2), the PR body, and the close-gate.
- **Discriminator run:** `kill_at_exit` plus all its controls at SMP=4, at least 5 consecutive passes of the positive
  scenario, and every control failing as intended.

## Close condition

- Both branches pushed; `scripts/worker-close-gate.sh` passes on each; the evidence block is in each PR.
- A short report comment on each PR, starting `RETURN (seat A):`, with the head SHA, the scenario results, and any
  old→new SHA mapping.
- **Don't merge.** The PM gates #204 and #219 together in one 4-shard batch, and #219 gets a cross-family re-check
  (Janus) before merge.
- Commit and push at every checkpoint. No model identifiers in commits.

## Afterwards (if time remains): #171 (Chad, b19)

An in-flight wait has no claim pin against revoke, and a failed inherited table leaks claimant counts, so the exit
code is kept forever. Read `Plans/RETURN-*` for #171 on main. Its own branch and PR, off `main` once #204 has merged.
