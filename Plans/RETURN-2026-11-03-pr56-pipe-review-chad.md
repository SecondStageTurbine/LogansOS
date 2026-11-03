<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 303 — Chad — PR #56 pipe cross-CPU review

MODEL: GPT-6.1-Sol. EFFORT: high. New token parks, cross-CPU wakes and teardown in the SMP core.

Reviewed head: `e60cfa1b1b5df6098b4b888c5aaa9e1614966f07` (`claude/l1a1-pipe-wiring`).
Fetched `pull/56/head:pr56` and reviewed `git diff origin/main...pr56`; RETURN branch:
`codex/pr56-review-chad`, based on `origin/main` at `ee5c755e0`.
Read the PR body and newest `PM:` comment (2026-11-03 14:00 UTC), including Dexter's F1/F2 resolution.
RAG was unavailable on this relay host; discovery and review used direct source reads.
All source line references below are at the reviewed head.

Verdict: **FAIL against dispatch item 5 as written.** The park/wake/teardown checks pass. The implemented
byte-stream contract permits readers to split a write; its writer-atomicity guarantee also requires that
the write fit the pipe's capacity. These distinctions need to be reflected in the review requirement and
the unconditional small-write claims, or the implementation must supply the stronger requested behavior.

1. **PASS — CP-14 check/token/release critical section.** `kernel/src/ipc/pipe.rs:235` takes the guard before
   checking the read; `:268` passes that same guard into `park`. The writer does the equivalent at `:281` and
   `:295`. `:338` installs the token through the fallible scheduler primitive. In
   `kernel/src/sched/scheduler.rs:2251`, the scheduler parks the thread and installs its token while both locks
   remain held; `:2262` transfers their release into the switch. The new stack is installed before either
   unlock in `kernel/src/arch/x86_64/context.rs:157` (XSAVE) and `:67` (non-XSAVE). A competing CPU therefore
   either changes the pipe before the check, or sees the installed token after unlock. Local `pipe_core`:
   `PIPE cp14 rounds=8 woken=8 window_hits=8`.

2. **PASS — CP-16 wake placement and lock order, including full-list purge.**
   `kernel/src/ipc/pipe.rs:99` delivers collected wakes; callers leave the guard's scope at `:185`, `:203`,
   `:223`, or explicitly drop it at `:241` / `:285`, before firing. Wait-key fires follow the same rule at
   `:107`. The full-list query at `:325` holds pipe then scheduler, matching the park at `:338` →
   `kernel/src/sched/scheduler.rs:2216`. `token_still_parked` takes only the selected scheduler lock
   (`scheduler.rs:2685`); generation and occupancy are checked at `:1402`. The wake path takes no pipe lock,
   and thread-owned Arc cleanup runs outside the scheduler guard (`scheduler.rs:1723`,
   `kernel/src/sched/thread.rs:273`). No reverse scheduler → pipe acquisition was found.

3. **PASS — kill, last close under a parked sibling, results and ownership (CP-33/CP-67, Dexter F1).**
   `kernel/src/sched/process.rs:5924` drains token-parked siblings under `browser-runtime`; the non-browser
   kill fallback is `:6027`. `kernel/src/sched/scheduler.rs:2526` extracts by pid. The syscall's owned Arc
   moves into the thread at `scheduler.rs:1716`, and `kernel/src/sched/thread.rs:273` drops it on reap.
   `process.rs:1845` releases endpoints through `Detached`, outside the capability-graph section;
   `kernel/src/ipc/pipe_logic.rs:232` closes an end permanently and `:253` wakes both lists on the last close.
   The own-end checks at `:272` / `:317` map to EBADF (`kernel/src/ipc/pipe.rs:396`); no readers gives EPIPE
   (`pipe_logic.rs:275`), and drained/no writers gives EOF (`:320`). A partially completed write returns its
   count (`kernel/src/arch/x86_64/syscall.rs:2334`). Local scenarios observed EOF, partial-write/EPIPE,
   `close_parked_read` and `close_parked_write` each returning -9 with `arc=1`, and killed reader/writer
   `returned=None` with `after=1`. Killed threads are retired, rather than returned an I/O result.

4. **PASS — bounded waiter list without eviction of live waiters.** `kernel/src/ipc/pipe.rs:318` first finds
   an unused slot; only a full list enters the purge at `:324`. It removes only tokens no longer parked on
   their generation, then returns EAGAIN if no slot exists (`:335`); it never wakes a live token to make room.
   The scheduler-token-table-full case also returns EAGAIN (`:349`). Local `pipe_park_kill` observed
   `PIPE full stale_before=8 extra=-11 live_total=8 arc=1`, and a stale token did not prevent the next live
   reader from receiving its byte.

5. **FAIL as written — small writes are not indivisible across readers; capacity is a second atomicity
   bound.** `kernel/src/ipc/pipe_logic.rs:333` reads `min(buffered, destination length)` and retains no write
   boundaries. Concrete input, reproduced against the unmodified state machine: `Pipe::new(512)`, write
   `b"AB"` once, then let reader A read one byte and reader B read one byte. Both reads succeed; A receives
   `b"A"`, B receives `b"B"`. This is the documented byte-stream design, but it fails the relay's literal
   “never split across readers” check. Separately, `pipe_logic.rs:282` makes writes atomic only when
   `len <= 512 && len <= capacity`. With capacity 1, writer A's blocking `b"AB"` stores A and waits; after
   a reader takes A, writer B can store X before writer A resumes with B, producing `AXB`. The same
   state-machine sequence was reproduced locally. The syscall explicitly loops over such partial chunks
   (`kernel/src/arch/x86_64/syscall.rs:2324`). Thus the PR/libneo's unconditional “writes <=512 are atomic”
   wording needs the capacity qualification already present in `docs/SDK-ABI.md:369`. For writes that fit
   capacity, **PASS** for writer non-interleaving: the whole small write requires room (`pipe_logic.rs:283`)
   and runs under one pipe guard (`pipe.rs:284`). **PASS** for the requested CP-99 precheck:
   `syscall.rs:2264` checks each destination chunk before taking bytes (`:2267`), then uses fault-recovering
   `user_copy` (`:2277`). This is a precheck, not a pin: an unmap during a park or between check and copy can
   still yield EFAULT after bytes leave the pipe, as follows from `syscall.rs:1613` and `:2277`.

6. **PASS — WAIT_KIND_PIPE readiness and CP-61 identity.** `kernel/src/ipc/pipe_logic.rs:159` implements
   bytes-or-EOF; `:169` implements room for `min(512, capacity)` or EPIPE. The scanner records the pipe's
   end-specific key and CapId (`kernel/src/arch/x86_64/syscall.rs:6556`); registration precedes the second
   scan (`:6649`, `:6660`), and changed identities cause re-registration (`:6678`). Removal fires the old
   key (`kernel/src/sched/process.rs:2085`). A stale-key fire causes a fresh scan rather than directly
   returning a ready bit. Local read/write readiness legs returned 1, and the write leg stayed unready after
   freeing only one byte. Rebind returned 1 with `identity_retries=1` and `arc=1/1`.

7. **PASS — kill-switch and three guard-removal controls.** The `pipe` dependency is explicit at
   `kernel/Cargo.toml:537`. The three syscall arms are conditional at
   `kernel/src/arch/x86_64/syscall.rs:1347`; without them the dispatcher returns ENOSYS (`:1410`). With
   wait-multiplex enabled and pipe disabled, kind 4 has no scanner arm (`:6555`) and reaches EINVAL
   (`:6563`). Handle and thread fields are conditional (`kernel/src/sched/process.rs:1044`,
   `kernel/src/sched/thread.rs:195`). These off-path results were checked in source; no dedicated local
   numeric-syscall off-path probe was added. All three local QEMU controls failed at the intended
   discriminator (each on all four attempts made by xtask's default retry policy):
   `neo-test-pipe-core-cp14-control` → `pipe_cp14_lost_wake` (`kernel/src/ipc/pipe.rs:262`,
   `kernel/src/test_harness_pipe.rs:813`); `wait-multiplex-no-identity-check` → `pipe_rebind_stranded`
   (`syscall.rs:6678`, `test_harness_pipe.rs:782`); `neo-test-pipe-park-kill-control` →
   `pipe_park_kill_arc_leaked leg=read` (`kernel/src/sched/scheduler.rs:1732`,
   `test_harness_pipe.rs:954`). Each xtask invocation returned 1; QEMU returned debug-failure status 35.

8. **PASS — required prior-art section, the PR body's CP table.** The table is present, with introduced,
   guard and discriminator columns for CP-14/16/33/61/67/85/99 and Dexter F1. The enforcing sites and local
   witnesses are recorded in items 1–7 above. CP-85 is appropriately N-A to a new exit path: this PR changes
   no exit implementation, and adds pipe release to the shared `Detached` cleanup (`process.rs:1845`). The
   CP-99 row proves fault recovery and a per-chunk precheck, not transactional preservation across a sibling
   unmap; item 5 records that limit. The committed design's corresponding table is at
   `docs/THINK-2026-11-02-l1a1-pipe-wiring.md:104`. No driver trap bank is required by this change.

Validation: `cargo test -p pipe-logic-tests`: **22 passed**. At the exact reviewed head, local Docker/QEMU
with `NEO_SMP=4`, `cargo neo-test pipe_core pipe_park_kill --no-meta`: **2 passed, 0 failed**, each on its
first attempt. The two state-machine counterexamples in item 5 were also run against the reviewed source.
Controls used the same local command, selecting `pipe_core` for CP-14 and identity, or `pipe_park_kill` for
the anchor, with `NEO_TEST_EXTRA_FEATURES` set to the one control feature named in item 7. No implementation
or harness source was changed for these runs. The controls' expected failures validate the discriminators;
they are not failures of the guarded build.

The GitHub API independently reports success for the exact-head
[full workflow-dispatch gate](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41630752577) and
[PR CI](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41630752537).
This RETURN changes documentation only; it does not modify the reviewed implementation or post a PR comment.
