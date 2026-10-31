<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: C2.3a Condvar wake re-check, relay #178 / #223

Reviewer: Jarvis. Branch: `codex/review-bundle-b23-b24-jarvis`.
Reviewed squash `45bb4b32e1d2ac0937bdb794c4d205e09a856df1` on fetched
main `8c1b7a1d`. **CLOSED: #156 item 3's two false-pass schedules.**
References below are at that main; the relevant source equals the squash.

1. **CLOSED — predicate false when the wait begins.**
   `userland/std-real-smoke/src/servo_host.rs:144` initializes both marks
   false. The waiter sets `parked` while holding the mutex (:164–167), and
   releases it through the wait. The waker locks that same mutex and checks
   `parked` before setting the predicate (:150–154). It cannot pre-set the
   predicate before the wait's first predicate check. An early-scheduled
   waker drops the mutex and yields until the mark is visible.
2. **CLOSED — reaching the timeout cannot pass.**
   `servo_host.rs:166–168` measures with `Instant`, waits with a 10-second
   deadline and requires elapsed time strictly below 5 seconds, alongside
   the true predicate and `!timed_out()`. This catches the specific
   `wait_timeout_while` behavior where a predicate set during an unnotified
   wait makes its final timeout result report false after the full deadline.
   Independently compiled a host probe extracted from the actual check,
   changing only the notification into a toggle: notification on passed
   (253 microseconds); notification off returned `woke=false` after 10.000 s.
   The probe used host std, not the target PAL or a new QEMU boot.
3. **No new blocking change in the squash. Separate fix recommended for
   threads check 11.** `userland/std-real-smoke/src/threads.rs:188–195`
   sleeps 30 ms before setting its predicate; elapsed time does not prove
   the waiter has started. Concrete schedule: delay the main thread until
   the signaller finishes, remove `notify_one`, then call `wait_while`.
   The predicate is already true and check 11 passes without a wait. A host
   probe forced this schedule by joining the signaller first and reproduced
   the pass with no notification. Fix this independently with a waiter
   handshake and a bounded wake assertion, as #223 does; it is outside
   #223's single-file diff. The new smoke assertion still uses the ordinary
   Condvar API, which permits spurious wakes; it closes the reported
   scheduling/timeout false passes rather than proving all possible wakes
   came from this notification. Extreme scheduling delays can also fail
   the 5-second bound conservatively.

Validation: inspected the entire test-only squash and unchanged neighboring
timeout check; both host probes compiled and exited successfully. RAG health
was healthy (`ollama=true`, `qdrant=true`). No implementation changes.
