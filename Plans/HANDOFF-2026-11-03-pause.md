<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Paused 2026-11-03 ~22:20 UTC (J.c.: usage at 95%, resumes Saturday)

Every scheduled check-in (the PM's and all seats') was disabled; nothing on this account wakes on its own. GitHub
Actions runs already dispatched finish by themselves and cost no Claude usage. The M910q relay runner (Codex and
DeepSeek reviewers) uses its own quotas and may still return queued reviews; they wait on their `codex/*` branches.

## On main today (in order)
#2, #59, #62, #61, #60, #69, #64 (docs), #15, #7, #68, #66 (`870b769a`). Previews: `cb71ab69` (F4/F9/F10) and
`08a3f30f` (F8); J.c. was told about both.

## In flight when paused
- **Batch "67"** (Seat D, pthreads; head `905f7a9a`, base `870b769a`, expected tree `15d2ca3a`) was running. On
  resume: if its run is green and main hasn't moved by code, land it with `scripts/batch-assemble.sh 870b769a
  905f7a9a` (check `tree=`); otherwise re-dispatch `CI` on main, tier full, batch "67".

## Landing queue after #67 (each a batch of one; review done, needs a green PR run at its head)
1. #57 (`6af77861`, Chad PASS carries; Seat E)
2. #65 (`30a3f3e8`, Janus PASS; Seat D)
3. #8 (`8e81cb4d`, Janus 309+311 PASS; retarget to main first; Seat C) → #14 (`73453e31`, review-complete)

## Reviews queued and not yet returned (relay runner)
- 312: Jarvis re-checks #58 (`aa5150e6`, Seat F's drag fix `7c1d8c40`).
- 313: Chad re-checks #56 item 5 (`71bc2dca`, Seat B's squashed pipe branch).
- 314: Janus reviews #70 (setup menu S1, `4f3885a7`, Seat I).

## Seats' next work (from their last check-ins)
- B: #56 re-check → L1a.1 step 2b. C: #8 → #14. D: browser restack of #25-#28 as `-v2` branches, #55 onto main, L4
  layout run, the #67 destructor-iteration follow-up. E: #57, then the NeoFS cache index follow-up. F: #58. I: #70
  review, S2 (#71, settings store).

## Resume steps
1. Re-enable or recreate the PM loop (it must reschedule itself first) and each seat's check-in.
2. Each loop: find fixes by comparing each PR's head with its reviewed head (`git log --no-merges <reviewed>..pr<N>
   ^origin/main`), not only by RETURN branches. Three re-checks were missed for hours on 11-03 for lack of this.
