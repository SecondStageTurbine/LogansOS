<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RELAY #178 → Jarvis: narrow re-check of your #156 item 3 (C2.3a Condvar check)

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of your own finding.

Your #156 item 3 (`Plans/RETURN-2026-10-29-ot61-c2.3a-review-jarvis.md`): `servo_host_smoke` check 5 passed with
`notify_one` removed. The fix merged as #223, `45bb4b32` (test-only; `userland/std-real-smoke/src/servo_host.rs`).
Read it with `git show 45bb4b32`.

Say CLOSED or STILL OPEN, with the enforcing `file:line`:
1. The waker sets the predicate only after it holds the mutex and sees the waiter's `parked` mark, so the predicate is
   false when the wait begins.
2. The check bounds the time waited (a wake within 5 s of a 10 s timeout), so a wait that ran to its timeout and then
   saw the predicate set can't pass.
3. Anything new in the diff. The PR notes the same race in `threads.rs` check 11; say whether it should be its own fix.

Commit `Plans/RETURN-2026-10-29-c23a-recheck-jarvis.md`, push it, and paste the SHA.
