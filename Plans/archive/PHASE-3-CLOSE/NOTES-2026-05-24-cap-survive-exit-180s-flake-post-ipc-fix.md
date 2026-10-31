# NOTES: cap_survive_exit 180s timeout flake (post PR #6 IPC fix)

- Author: PM (Cerebro/Claude)
- Date: 2026-05-24
- Status: Placeholder / monitoring. Not a diagnosis.
- Trigger origin: section "Outstanding follow-ups" of
  `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md`.

## 1. What we know

After PR #6 (`4dfb13f`) shipped the pid-aware `Channel::close_peer` fix,
PL's local verification at `NEO_SMP=4` ran the `cap_survive_exit`
scenario 5 times. 4/5 passed cleanly on attempt 1. 1/5 hit a 180-second
QEMU-level timeout, then the xtask's 4-attempt retry caught it on
attempt 2.

CI ran the full test suite clean once the disk-exhaustion workaround
(jlumbroso/free-disk-space@main) landed in the same PR — including
cap_survive_exit. No CI-visible reproduction of the 180s flake.

## 2. What this flake is NOT

- **Not** the original exit-code-255 / cross-process zombify race that
  PR #6 fixed. That manifested as init reaped without `sys_exit` and
  the kernel falling back to exit code 255. This is a flat 180-second
  QEMU watchdog timeout — no exit code, no kernel diagnostic, no
  reaper line. Different failure shape.
- **Not** the GHA runner disk exhaustion that masked the real bug for
  weeks. That had a consistent ~22-minute kill point on the runner
  side. This is QEMU-side, 180 seconds, locally reproducible only
  rarely.
- **Not** the Phase 1.5b SC-4 invariant (`q8c_close_peer_my_side_extract`)
  which still passes per PR #6 verification.

## 3. What we don't know

- Whether it's a kernel hang (some thread parked with no waker) or
  a slow-path that genuinely needs >180s under contention.
- Whether `NEO_SMP=4` is necessary to trigger or whether SMP=2 reproduces.
- Whether the trigger correlates with timing of grant + close ordering
  on different CPUs (which would put it in the same neighborhood as
  the PR #6 bug — a different cross-CPU IPC race not covered by the
  pid check).
- Whether the xtask retry's pass-on-attempt-2 means the bug self-clears
  or whether attempt 2 just happened to win a different scheduling
  race.

## 4. Trigger conditions for re-investigation

Promote from NOTES to active diagnosis if any of:

- **C1:** Reproduction rate climbs above ~1/5 (≈20%) in PL's local
  runs. Current 20% is at the edge; if it ticks up, retries stop being
  sufficient cover.
- **C2:** CI surfaces a single failure that the xtask 4-attempt retry
  doesn't mask. CI's full-suite green status is the load-bearing
  evidence right now; any breach is signal.
- **C3:** Any second flake of similar shape (flat watchdog timeout, no
  diagnostic) appears in a different scenario. Would suggest a kernel
  hang class, not a per-scenario quirk.
- **C4:** Pre-Phase-2 audit (forthcoming) flags an invariant whose
  failure mode could plausibly produce a silent hang in `close_peer`'s
  remaining branches (the non-`extracted_my` paths).

## 5. Suggested first diagnostic step when triggered

Run `cap_survive_exit` locally at `NEO_SMP=4` in a loop (e.g. 50
iterations) with serial logging captured to a file per iteration.
On a failed iteration, the serial log up to the watchdog cutoff is
the entry point — look for the last `sched:` or `ipc:` line emitted
before silence.

If the silence is mid-`close_peer`, the pid-aware fix may have a
sibling race in one of the other endpoint-side branches. If the
silence is on a thread waker / park boundary, it's likely a lost
wakeup that needs separate analysis.

## 6. References

- `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md`
  §"Outstanding follow-ups" item 4.
- Commit `4dfb13f` (PR #6) — the IPC fix this flake post-dates.
- `kernel/src/ipc/mod.rs:508` — `close_peer` post-fix.
