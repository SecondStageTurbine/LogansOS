<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 299 — Chad — PR #67 review

Reviewed head: `abbd20b443d065ff2ce98d4d34e699f248622a53`.
Compared with `origin/main` at `700268f6a1d295e4b9e607def39d9e5ecd0efd0f`
using `git fetch origin +pull/67/head:pr67` and `git diff origin/main...pr67`.
Overall: **FAIL**.

- **PASS — 1. Mutex kinds.** `userland/neo-crt/src/pthread.rs:378` counts
  recursive acquisitions; `userland/neo-crt/src/pthread.rs:437` decrements
  the depth and refuses non-owner unlocks with `EPERM`. Error-checking
  relock returns `EDEADLK` at `userland/neo-crt/src/pthread.rs:389`.
  `userland/neo-crt/src/pthread.rs:369` refuses destruction while owned
  with `EBUSY`. Host probes verified that another thread still gets
  `EBUSY` after only one of two recursive acquisitions is released, and
  that a non-owner cannot unlock an error-checking mutex.

- **FAIL — 2. Condvars: a late waiter can swallow another waiter's signal.**
  `userland/neo-crt/src/pthread.rs:616` releases the sequence lock before
  `userland/neo-crt/src/pthread.rs:617` notifies. A wake that leaves the
  sequence unchanged is consumed internally by the loop at
  `userland/neo-crt/src/pthread.rs:545`, instead of reaching the caller's
  predicate loop. Concrete input/schedule: an unrelated futex waiter X
  occupies a low kernel queue slot; waiter A parks on this condvar in a
  higher slot, waiting for predicate `ready`. X exits its wait, freeing
  the low slot. The producer sets `ready = true` under the user mutex,
  unlocks it, and calls `pthread_cond_signal`; preempt the producer after
  the sequence increment and before `notify_one`. Waiter B now waits on
  the same condvar and mutex for a different, still-false predicate. B
  records the incremented sequence and parks in the free lower slot.
  Resume the producer: the kernel wakes B, which sees its sequence
  unchanged and parks again. A remains asleep despite `ready == true`;
  no wake remains pending. This ordering is permitted by the actual
  first-free insertion at `kernel/src/sched/futex.rs:184` and ascending
  wake selection at `kernel/src/sched/futex.rs:323`. A deterministic
  scheduling model running the PR's wait/signal logic reproduced both
  waiters parked after signal returned; a rescue broadcast released them.
  The model substitutes a selectable std-condvar wake and inserts a
  scheduling gate between the increment and notify; this was not a
  target boot reproduction. The clock checks themselves pass:
  `userland/neo-crt/src/pthread.rs:583` requires `CLOCK_MONOTONIC`, and
  an already-expired `{0, 0}` deadline returns `ETIMEDOUT` and reacquires
  the user mutex through `userland/neo-crt/src/pthread.rs:549` and
  `userland/neo-crt/src/pthread.rs:561`.

- **FAIL — 3. Key destructors do not run on every participating thread's exit.**
  The only exit invocation is inside the `pthread_create` wrapper at
  `userland/neo-crt/src/pthread.rs:150`.
  `userland/neo-crt/src/pthread.rs:647` gives `Slots` no destructor, so
  initializing its std TLS does not register the pthread-key cleanup.
  Concrete input: create a key whose destructor increments an atomic
  counter; use `std::thread::spawn` to call
  `pthread_setspecific(key, nonnull)` and return; join that Rust thread.
  The destructor counter is **0**, expected **1**. This failed directly
  in the host probe. It is an explicitly documented exclusion at
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:229`, but leaves this
  dispatch's general thread-exit requirement unmet. For pthread-created
  threads, `userland/neo-crt/src/pthread.rs:725` correctly clears values
  before callbacks and performs four rounds; a rearming destructor ran
  exactly four times in the probe. Deletion/reuse with values still held
  is guarded by the live flag and generation at
  `userland/neo-crt/src/pthread.rs:669` and
  `userland/neo-crt/src/pthread.rs:695`; the reuse probe saw no old value.

- **PASS — 4. Once and valid join/detach lifetimes.**
  `userland/neo-crt/src/pthread.rs:767` serializes the 0/1/2 once state,
  runs the initializer outside the global lock, publishes completion
  with Release, and uses Acquire on readers. A 32-thread race ran the
  initializer exactly once. For a valid thread with one terminal
  join-or-detach operation, `userland/neo-crt/src/pthread.rs:187` and
  `userland/neo-crt/src/pthread.rs:204` take the handle once;
  `userland/neo-crt/src/pthread.rs:194` and
  `userland/neo-crt/src/pthread.rs:207` release the creator's Arc.
  The thread's own Arc lives in `SELF` at
  `userland/neo-crt/src/pthread.rs:105`. Weak-reference probes confirmed
  record reclamation after join and after detached exit, and join
  returned the start routine's value.

- **PASS — 5. libc++ thread API symbols and ABI.**
  `userland/neo-crt/cxx/__config_site:18` selects the pthread API;
  `userland/neo-crt/cxx/__config_site:22` selects monotonic clockwait.
  `userland/neo-crt/build.rs:207` includes the threaded sources, and
  `userland/neo-crt/cxx/pthread.h:18` declares the clockwait extension
  implemented at `userland/neo-crt/src/pthread.rs:598`. All 39 configured
  libc++ sources and the PR's C++ threading smoke compiled with the
  freestanding target and PR configuration. Undefined-symbol inspection
  of that archive and consumer object found every referenced
  pthread/sleep/yield/sysconf symbol provided by neo-crt, including the
  consumer's `pthread_create` and `pthread_cond_clockwait`. C-header
  static assertions and Rust layout probes agree on size/alignment:
  attr 56/8, mutex 40/8, condvar 48/8, rwlock 56/8; key and once are
  32-bit. Header definitions are at
  `third_party/ot61-crt/vendor/musl/include/alltypes.h.in:58` and
  `third_party/ot61-crt/vendor/musl/include/alltypes.h.in:85`.

- **FAIL — 6. CP table is present, but its lost-wakeup claim does not hold.**
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:215` contains the C3b
  table. The lost-wakeup row at
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:219` and its explanation
  at `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:190` fail on the
  concrete A/B/X schedule in item 2: reading before unlock prevents one
  race, but does not prevent a later waiter consuming the notification.
  The thread-exit row at
  `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md:222` also needs the
  pthread-created-thread restriction illustrated in item 3. CP-106's
  stream guard remains enforced: `userland/neo-crt/c/stdout.c:27` and
  `userland/neo-crt/c/stderr.c:27` keep `.lock = 0`, and
  `userland/neo-crt/src/lib.rs:277` provides the owner-aware blocking
  lock independently of how threads were created. The libc++ rebuild
  enables its synchronization paths as required by CP-106.

Validation: seven host probes passed and the foreign-thread destructor
probe failed as described above. Host probes used the fetched Rust source
with export attributes removed to avoid interposing on the host runtime;
the scheduling model's additional substitutions are described in item 2.
C/C++ checks used the bundled headers and the fetched configuration.
`git diff --check origin/main...pr67` passed. No QEMU run or complete
neoplutarch ELF link was performed; QEMU is unavailable on this worker.
