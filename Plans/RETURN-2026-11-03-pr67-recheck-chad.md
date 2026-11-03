<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 308 — Chad — PR #67 re-check

Reviewed head: `905f7a9aa8aa0c0c559e5a888ecc972e7b599f7d`
(fix `f19aeb9a4cc776899a71a7e4a501cb40d9cf6002`). Fetched with
`git fetch origin +pull/67/head:pr67`; reviewed `git diff origin/main...pr67`
against `origin/main` at `25bbad3bbaaa2de09f37f81c4952d4dbd2224a8f`.
Read the newest PM comment and the preceding Seat D fix/test notes.
Overall: **PASS for the three dispatched re-checks**.

- **PASS — 2. Condvar lost wakeup.**
  `userland/neo-crt/src/seqcond.rs:106` signals and
  `userland/neo-crt/src/seqcond.rs:114` broadcasts with the sequence mutex
  held through notification, dropping it only afterwards. A late waiter
  cannot record the new sequence and enter the wait queue during that
  interval; existing waiters recorded the old sequence at
  `userland/neo-crt/src/seqcond.rs:82`, before releasing the user mutex.
  Production entry points use this implementation at
  `userland/neo-crt/src/pthread.rs:601` and
  `userland/neo-crt/src/pthread.rs:607`.
  Discriminator: `tools/neo-crt-logic-tests/src/lib.rs:199` mounts the
  actual sequence module and uses a newest-waiter-first model to attempt
  the earlier A/B schedule. It passes; its old-order control at
  `tools/neo-crt-logic-tests/src/lib.rs:208` reproduces the lost signal.
  Independently restoring unlock-before-notify in a temporary copy of
  the production `signal` makes the positive discriminator fail.

- **PASS — 3. Key destructors on threads outside the pthread wrapper.**
  `userland/neo-crt/src/pthread.rs:642` now implements `Drop` for `Slots`,
  registered through the std TLS at
  `userland/neo-crt/src/pthread.rs:661`. Cleanup clears each non-null
  live-generation value before calling its destructor through
  `userland/neo-crt/src/pthread.rs:651` and
  `userland/neo-crt/src/pthread.rs:746`. The std-thread exit path invokes
  TLS cleanup at
  `userland/neoplutarch-std-overlay/std/thread_neoplutarch.rs:101`.
  Discriminator: `userland/cxx-runtime-smoke/src/main.rs:85` creates a key,
  sets `0x5eed` from a plain `std::thread`, joins, and requires one callback;
  check 29 is invoked at `userland/cxx-runtime-smoke/src/main.rs:314` and
  required by `tools/xtask/src/main.rs:18092`.
  An independent host probe of the fetched key implementation reports
  **1 callback**; the identical probe on the previously reviewed
  `abbd20b443d065ff2ce98d4d34e699f248622a53` reports **0** and fails.
  Returning from process main remains process exit, as documented; this
  does not exercise a thread-exit path.

- **PASS — 6. CP table's lost-wakeup row matches the fix.**
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:228` now requires both
  read-before-user-unlock and notify-under-sequence-lock, consistent with
  the code above. Its explanation at
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:190` describes the old
  late-waiter failure. The row names the positive KAT and old-order
  control; the independently mutated positive KAT fails as described in
  item 2. `docs/concurrency-pitfalls.md:4717` records the same trap as
  CP-107.

- **New:** a destructor that rearms on every call runs **5** times (old head: **4**) when the host probe calls the wrapper's `run_key_destructors` and then exits: the fourth callback leaves a value for TLS `Drop`; this contradicts the “nothing left” assumption at `userland/neo-crt/src/pthread.rs:637` and qualifies the four-round account at `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:231`.

Validation: `cargo test -p neo-crt-logic-tests` passed all five tests;
the restored-old-order positive KAT failed as expected. Host key probes
used the exact fetched key section with export attributes removed to
avoid interposing on the host pthread runtime, plus imports/constants
and a test harness; the old-head control used the identical harness.
`git diff --check origin/main...pr67` passed. No target build or QEMU run
was performed in this re-check; QEMU is unavailable on this worker.
