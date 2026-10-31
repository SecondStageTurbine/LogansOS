# DISPATCH: F-005 cross-process zombify in forced-kill blocked-walk (Jarvis)

- Author: PM
- Date: 2026-05-24
- Agent: Jarvis (Codex — heavy coding / kernel source lane)
- PL ratification: implicit. The PM-DISPOSITION
  (`Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md`) names
  F-005 as a blocking item for Phase 2 PLAN; fix dispatches for blocking
  audit defects are operational decisions under PM authority per the
  2026-05-10 operational-posture memo.
- Scope class: kernel fix dispatch (Jarvis lane).
- Bible amendment: none. The fix preserves existing invariants and
  restores INV-IPC-8 lockstep by construction; no ADR or bible change
  required.
- Blocking: Phase 2 PLAN start gated on this dispatch landing on
  `origin/main` with CI green per PM-DISPOSITION §1.

---

## 1. The defect

**F-005** (audit finding): the forced-kill path's blocked-thread extraction
has the same cross-process-zombify bug PR #6 (commit `4dfb13f`) fixed in
the natural-exit path. PR #6 only made `close_peer`'s `extracted_my`
branch pid-aware; the parallel extraction path used by `process::terminate`
(`Channel::extract_my_side_if_blocked`) was untouched.

**Location:**
- `kernel/src/ipc/mod.rs:468` — `Channel::extract_my_side_if_blocked(side)`
  extracts whatever thread is parked on the named side with no pid check.
- `kernel/src/sched/process.rs:909-977` — `terminate(pid)` calls the
  above as part of the blocked-walk that lifts the Phase-1.5b
  -EINVAL-on-Blocked restriction (per ADR-013 Q8.c). The verbatim
  now-false assumption is stated at process.rs:955-958: *"Phase 1.5b has
  one thread per process. If the target was not Ready, the first parked
  thread found through its channel handles is the target."*

**Why now-false:** Phase 2 BUILD-1 grant rendezvous (ADR-010) made it
possible for a foreign process's thread to be parked on a donor's
channel side via a granted derivation. The original assumption was
correct in Phase 1.5b (one thread per process, no grants, parked thread
on your side is always yours). It is false post-BUILD-1.

**Failure mode:** A force-kills B. B's own thread is blocked elsewhere
(e.g., parked on a different channel B holds). A grantee thread C (pid≠B)
is parked on B's channel side. `terminate(B)` walks B's handles, calls
`extract_my_side_if_blocked` on each, finds C parked, zombifies C, sets
`found_blocked=true`, and breaks the walk. C is wrongly terminated. B's
own thread is never reaped via this branch.

**Why no existing test catches it:** `q8c_blocked_kill_walk` spawns the
blocker with the target's own pid (`new_kernel_for_process_test(...,
target_pid, ...)`). The userland scenario `sys_kill on Blocked worker`
in `neo-init` main.rs:70-72 kills a worker blocked on its own channel.
Neither test exercises a foreign grantee parked on the killed process's
side. The cross-process forced-kill case has zero coverage — same
"passes for the wrong reason" pattern as F-002.

---

## 2. The required fix (acceptance criteria, not implementation prescription)

**C1 — F-005 not reachable.** A test scenario representative of the
failure mode (A force-kills B; foreign grantee C parked on B's channel
side; B's own thread blocked elsewhere) MUST result in:
  - C left parked, then woken with `-EPIPE` via the existing pid-aware
    `close_channel_peers` path (already correct post-PR #6).
  - C is NOT zombified.
  - B's own thread is correctly reaped via the appropriate branch
    (either the ready-queue scan succeeds because B's thread is Ready
    elsewhere, or the blocked-walk finds B's own thread parked
    somewhere else B holds, or terminate's fallback path handles it
    correctly).
  - `sys_kill(handle_for_B)` returns 0.
  - C's process is unaffected and continues running until its own
    natural exit or kill.

**C2 — INV-IPC-8 lockstep restored by construction.** Both extraction
paths from a parked channel side must share the same pid-aware
discipline. The acceptable shapes are:
  - **Preferred:** unify both call sites into a single helper (e.g.,
    `extract_parked_if_owned(side, expected_pid) -> Option<Box<Thread>>`)
    that takes the expected pid and returns `Some(thread)` only when
    `thread.pid() == expected_pid`. Both `close_peer`'s `extracted_my`
    branch and the forced-kill walk route through this helper. A third
    extraction site landing in a future BUILD inherits the pid check by
    construction.
  - **Acceptable alternative:** if a unified helper is structurally
    awkward (Jarvis's call after reading the code), add `expected_pid:
    ProcessId` to `extract_my_side_if_blocked`'s signature and
    return `None` for foreign waiters. Document in the function's doc
    comment that this discipline is shared with `close_peer` and that
    any future extraction primitive must carry the same parameter.
    Update process.rs:955-958's verbatim comment to reflect the new
    invariant.

Either shape satisfies C2 as long as it prevents a third extraction site
from landing without the pid check. The preferred shape is structurally
stronger; the alternative requires discipline at every future call site.

**C3 — Test coverage for the foreign-parked forced-kill scenario.**
A new test in the kernel test harness (likely `q8c_blocked_kill_foreign_parked`
or similar; Jarvis's naming call) that:
  - Spawns process A and process B.
  - A grants a channel derivation to B such that B receives a handle to
    one side of A's channel.
  - B parks a thread on A's side via `sys_send` or `sys_recv` on the
    granted handle (this is the foreign-grantee-parked-on-A's-side
    state).
  - A's main thread is blocked elsewhere (parked on a different channel
    A holds, OR not — the test should cover at minimum the case where
    A's blocked state is such that the bug would have fired).
  - C (an external process; could be neo-init holding A's
    ProcessHandle) force-kills A via `sys_kill`.
  - Assertions:
    - `sys_kill` returns 0.
    - A is correctly reaped (no resource leaks per existing leak audit).
    - B's thread is woken with `-EPIPE` and is NOT zombified.
    - B is still alive after A's reap.

If the kernel test harness can't fully express the foreign-grantee
setup, a userland test in `neo-init` is acceptable. Discuss with PM if
structural blockers appear.

**C4 — No regression.** All existing tests PASS:
  - The full `cargo neo-test` suite under `NEO_SMP=4`, with no flake.
  - Specifically including: `q8c_blocked_kill_walk`, `cap_survive_exit`,
    `ipc_heap_kill_smoke`, `cap_grant_basic`, `cap_revoke_*`,
    `expect_pass_but_fail` (meta-test).
  - PR #6's behavior preserved: natural-exit cleanup still wakes
    foreign waiters with -EPIPE and zombifies only the exiting
    process's own threads.

**C5 — Comment alignment.** The verbatim now-false assumption at
process.rs:955-958 ("If the target was not Ready, the first parked
thread found through its channel handles is the target") MUST be
removed or rewritten to reflect the new pid-aware discipline.

**C6 — Close report.** Filed at
`Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` with:
  - Commit hash(es) of the fix and the test additions.
  - Implementation shape chosen (preferred unified helper or alternative
    parameter-only).
  - Test name(s) added.
  - CI run hash showing the full suite green at `NEO_SMP=4`.
  - Any deviations from this dispatch's acceptance criteria, with
    rationale.
  - Brief note on whether INV-IPC-8 lockstep is now structurally
    guaranteed (preferred shape) or discipline-guarded (alternative
    shape).

---

## 3. Files in scope

- `kernel/src/ipc/mod.rs` — `extract_my_side_if_blocked` (l.468) and
  potentially `close_peer` (the `extracted_my` branch PR #6 touched) if
  the unified-helper shape is chosen.
- `kernel/src/sched/process.rs` — `terminate` (l.909-977), specifically
  the verbatim comment at l.955-958 (C5) and the call site to
  `extract_my_side_if_blocked`.
- Test files for the new scenario.

Out of scope:
- Other capability operations (derive/grant/revoke).
- The (b) test items in PM-DISPOSITION §5 — those are separate BUILD-N
  work.
- The (d) deferred items (Phase 4 self-kill, ADR-019 sub-phases B/C).
- Driver subsystem, NeoFS, etc.

---

## 4. Implementation notes for Jarvis

- The bug shape is identical to PR #6's seed bug. PR #6 (commit
  `4dfb13f`) is the implementation reference. Same kernel area, same
  approach (pid-aware extraction), just the parallel code site PR #6
  missed.

- The unified-helper shape (preferred) lets you delete the now-pid-aware
  logic from `close_peer`'s `extracted_my` branch in favor of routing
  it through the shared helper. This is also a structural fix for
  INV-IPC-8 lockstep. If you go this route, verify that PR #6's tests
  still pass after the consolidation — they should, because the
  behavior is preserved, but it's worth explicit confirmation.

- The test setup for C3 may require careful sequencing: getting B's
  thread parked on A's side, then ensuring A's own thread is in a
  killable-but-not-ready state, then triggering the kill from a third
  process. The kernel test harness's existing q8c machinery (per
  Cerebro's source-side scan) should support most of this. If you hit
  a structural blocker (e.g., the harness can't express foreign-process
  parking on a granted handle), surface as a NOTES document and PM
  re-scopes the test deliverable; do not stretch the harness in this
  dispatch.

- The verbatim comment at process.rs:955-958 is documented as
  "now-false" in the audit. After the fix, it should be either removed
  (if the new code makes the discipline self-evident) or replaced with
  a comment naming the new pid-aware invariant and the audit reference
  (`Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md`).

- Status code 1 from `git push` is the cosmetic stderr decoration
  pattern documented in past commits; not an error.

---

## 5. Close

When the fix is on `origin/main`, CI green at full suite + `NEO_SMP=4`,
and `Plans/CLOSE-REPORT-2026-05-24-f005-forced-kill-zombify.md` is
filed, Jarvis returns with the commit hashes. PM consumes the close
report; if everything is in order, PM files
`Plans/PM-DISPOSITION-2026-05-24-f005-close.md` confirming the audit's
blocking item is resolved and Phase 2 PLAN may proceed.

If during implementation Jarvis surfaces an ambiguity or a structural
issue not covered by C1–C6, file as a NOTES document and PM responds
within the dispatch's lifetime. Do not proceed with workarounds without
PM acknowledgment.

---

## 6. References

- `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` — the
  audit closure naming F-005 as blocker.
- `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md` §F-005 —
  the Day 2 finding with full failure-mode analysis.
- `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` — Day 1
  partial; F-003 (INV-IPC-8 lockstep) is addressed by this fix.
- Commit `4dfb13f` — PR #6, the close_peer pid-aware fix. Pattern
  reference.
- Commit `e2b3f4f` — SC-3B sub-step 5c. Source for INV-SYNC-1
  discipline that the test scenario must not regress.
- ADR-004 §"Lessons" — *"Shared cleanup paths prevent drift."* This
  fix is the second-instance restoration of that lesson.
- ADR-013 §Q8.c — the cross-CPU blocked-thread walk this fix corrects.
