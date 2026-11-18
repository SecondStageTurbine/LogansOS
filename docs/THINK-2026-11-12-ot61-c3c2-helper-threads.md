<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: C3c-2, SpiderMonkey's helper threads, a compacting GC and OOM containment

**Seat:** D. **Date:** 2026-11-12. **Status:** design for review, no code yet. **Ladder:**
`Plans/DISPATCH-2026-10-30-seat-d-browser.md` rung 3 (C3). **Builds on:** C3c-1 (#107, `63b3e02a`), C3b (neo-crt
pthreads, #67) and `docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` §3.2, which names this step.

## 1. What this rung has to answer

ADR-078 §3's C3 row asks for four things. C3c-1 proved the first:

| C3 row | C3c-1 (#107) | C3c-2 (this THINK) |
|---|---|---|
| One realm evaluates JS | Done: eval, a closure, irregexp, `normalize`, `Date`, each against a hand-written value | Kept as is |
| GC compacts | Not shown: `JS_GC` is a normal, non-shrinking GC | A shrinking GC moves objects; survivors intact; chunks given back (§4.2) |
| Helper threads join | Off: patch 0003 sets `gCanUseExtraThreads = false` | The helper pool starts, runs tasks and joins at `JS_ShutDown` (§4.1) |
| OOM/crash kills only the content process | Not shown | An over-budget allocation and a forced crash, each with helper threads alive, end only the smoke process; every frame comes back (§4.3) |

This is **SMP-touching userland**: real threads on several CPUs (the gate runs `NEO_SMP=4`), inside one process,
through neo-crt's pthreads. So it carries a CP table (§5) and needs a **cross-family AAG lens** before it merges.

## 2. Prior art (standing rule, 2026-10-27)

No new third-party code. Everything here is SpiderMonkey's own (vendored in #87, MPL-2.0) running on neo-crt (#67,
#88). The survey for the engine itself is THINK #10's (`docs/THINK-2026-10-31-ot61-c3c-spidermonkey.md` §2).
What this rung borrows is **pattern only**:

- **SpiderMonkey's POSIX threading backend** (`js/src/threading/posix/`, `mozglue/misc/*_posix.cpp`), the backend
  Firefox runs on Linux. It's already compiled into C3c-1; only the switch is off.
- **`InternalThreadPool`** (`js/src/vm/InternalThreadPool.cpp`): `max(cpus, 2)` threads (`HelperThreads.cpp:97-101`;
  the CPU count clamped to 8, `:88-94`), each with a stack of 2 MiB less two pages (`InternalThreadPool.cpp:23`).
  It starts in `JSRuntime::init` (`vm/Runtime.cpp:176`) and is joined in `JS_ShutDown`.
- **The embedder-pool alternative**, `JS::SetHelperThreadTaskCallback` (`js/public/HelperThreadAPI.h:27`): the
  embedder runs `HelperThreadTask`s on its own threads. Not in our frozen bindings today (§6, decision D1).

## 3. What changes

1. **Patch 0003's `Runtime.cpp` hunk** drops `__neoplutarch__` from the `gCanUseExtraThreads = false` branch, so our
   target takes upstream's POSIX default (`true`). The patch gets smaller, not larger. The `wasm::EnsureEager...`
   hunk beside it stays: wasm stays off.
2. **The generator re-runs** (the frozen `generated/patched/` tree changes in that one file). The drift check proves
   no other frozen output moved.
3. **The smoke** (`userland/mozjs-smoke`) gains the checks in §4 and ends by dropping its runtime and engine, so
   `JS_ShutDown` runs and joins the pool before `exit(0)`. C3c-1's smoke exits without shutting the engine down,
   which C3c's trap T12 said it did: that gap gets closed here.
4. **neo-crt** gains a test-visible thread record (§4.1): threads created, threads exited and joins completed, and
   per-thread condvar wakes. Plain counters, no new lock.
5. **The scenario** `mozjs_smoke` stays the gate; two crash variants (§4.3) are new scenarios beside it.

## 4. The checks

### 4.1 Helper threads start, work and join

| Check | Reads | Passes when |
|---|---|---|
| 20 | neo-crt's created count after `JS_NewContext`, and `JS_GetGCParameter(JSGC_HELPER_THREAD_COUNT)` | At least 2 threads made (`max(cpus, 2)`, `HelperThreads.cpp:97-101`), and the GC's count between 1 and that. *(Corrected after the first gate run, #113: `JSGC_HELPER_THREAD_COUNT` is the GC's share of the pool for parallel work, `cpus × JSGC_HELPER_THREAD_RATIO` clamped, not the pool's size, so the first draft's "both equal" failed with 1 and 2. That run also showed std's `available_parallelism` returning `NotFound` on our target, so neo-crt's `sysconf` reports 1 CPU and SpiderMonkey sizes for one even at `NEO_SMP=4`; the smoke prints both.)* |
| 21 | neo-crt's per-thread condvar wakes, summed over the helper threads, before shutdown | ≥ 1 after the compacting GC of §4.2. The pool only wakes a helper to run a task or to exit, and exit hasn't happened yet, so a wake means a task ran off the main thread |
| 22 | neo-crt's joined and exited counts after `JS_ShutDown` returns | Both equal check 20's count; no helper thread is left |
| 23 | The process's exit code and the kernel's frame count | 0, and every frame back (as C3c-1) |

Check 21 is the one that tells "the pool started" apart from "the pool did work". A version where helper threads
start but every task still runs on the main thread passes 20 and 22 and fails 21.

**Check 26, the control for check 21** (an assertion in the scenario, not a printout). Parallel marking is
**disabled** (`JSGC_PARALLEL_MARKING_ENABLED` = 0) and compaction is off (`JSGC_COMPACTING_ENABLED` = 0). The smoke
then makes a hundred small objects, too few to start a GC, so no helper has a task. The scenario requires the
helper-wake count to stay where it was: **0 new wakes**. Both counts are read once the thread record has stopped
moving (no change over five reads 10 ms apart), so a helper still starting or settling can't land in the window.
If check 26 fails, the wake count is measuring something other than tasks, and check 21 is redesigned before it's
trusted (that's the falsification test for this discriminator). Both settings are restored afterwards. *(Corrected
on review: the first draft said parallel marking "enabled", which would produce wakes.)*

### 4.2 The GC compacts

1. Allocate about 8 MiB of objects and strings and keep every 16th (C3c-1's pattern, larger). Nearly every arena
   then holds a few survivors, so a normal GC frees few arenas: only compaction can pack them.
2. Drop the rest, then a normal full GC (`PrepareForFullGC` + `NonIncrementalGC(cx, GCOptions::Normal, ...)`), then
   a shrinking one (`GCOptions::Shrink`).
3. **Check 27:** `JSGC_COMPACTING_ENABLED` reads 1 (SpiderMonkey's default). The check fails, not skips, if it's off.
4. **Check 24:** `JSGC_BYTES` (the heap in use) after the shrinking GC is below its value after the normal GC, and
   `JSGC_UNUSED_CHUNKS` is 0 (Shrink releases empty chunks). The chunk totals are printed too. *(The first draft
   compared `JSGC_TOTAL_CHUNKS`. Heap bytes are the direct measure: with survivors in nearly every arena, only a GC
   that moved them can free arenas, while chunk counts can also fall from empty-chunk expiry alone.)*
5. **Check 25:** every survivor still has its index and string. Compaction moves objects and rewrites every pointer
   to them, so a wrong pointer update shows up as a wrong value, not a crash later.

Compaction is also the helper threads' main customer here: relocation and pointer updating run as parallel GC
tasks, so §4.2 is what gives check 21 something to count.

### 4.3 OOM and crash kill only the content process

Each runs with the helper pool started (check 20 first), so a death has live threads to retire:

| Variant | How | Passes when |
|---|---|---|
| `mozjs_smoke_oom` | The smoke allocates JS objects in a loop past its 256 MiB spawn budget | Either SpiderMonkey reports a catchable OOM and the smoke exits with its own code, or the allocator aborts (exit 134). Both are fine for the C3 row; the scenario prints which. The kernel sees the process gone, every frame back, and **the scenario carries on** to a second spawn that passes |
| `mozjs_smoke_crash` | `MOZ_CRASH` through a test hook in the smoke while a GC task is running | Exit 134 (neo-crt's `abort`); same kernel checks |

The kernel-side check is the C2.2 budget check C3c-1 already makes (`used == baseline`, no underflows), plus
`any_user_process()` false afterwards: no helper thread survives its process (CP-85).

## 5. Concurrency: the CP table (pitfall bank consulted, CP-1…CP-112)

| Class | C3c-2 | Guard / discriminator |
|---|---|---|
| CP-85, a process exit that retires only the calling thread | **Touched.** `exit(0)` after shutdown has one thread left; the OOM and crash variants exit with helper threads alive | CP-85's guard: with `browser-runtime`, `SYS_EXIT` from a process with more than one live thread retires them all (and CP-86's cross-CPU retire). Discriminator: §4.3's `any_user_process()` false and frames back with the pool running |
| CP-107, a sequence condvar that wakes after dropping its lock | **Touched.** SpiderMonkey's `ConditionVariable` is neo-crt's `seqcond` (C3b), now with several real waiters | C3b's guard (wake under the sequence lock). Discriminator: check 22, a join that never completes hangs the scenario into its timeout |
| CP-14, a lost wakeup | **Touched.** The pool parks idle helpers on a condvar and wakes them per task | Same as CP-107; check 21 also fails if wakes go missing and tasks fall back to the main thread |
| CP-58, a periodic rescuer hides a missing wake | **Touched.** Helper waits use `pthread_cond_timedwait` (`CLOCK_MONOTONIC`, from the frozen `HAVE_CLOCK_MONOTONIC`) | Check 21 counts **wakes**, not timeouts; neo-crt records the two apart, and a wait that only ever times out counts 0 |
| CP-84, a single-thread fallback for per-thread state | **Touched.** `MOZ_THREAD_LOCAL` uses pthread keys, now on several threads | C3b items 23 and 29. Discriminator: check 25 runs on data the helpers touched |
| CP-106, a C library's "unlocked until threads exist" streams | **Touched** if a helper prints (assertion text) | C3a's guard: neo-crt's streams always lock |
| CP-111, a last reference dropped under a global lock, when its `Drop` waits | **Touched.** `mozjs`'s `JSEngine::drop` calls `JS_ShutDown()` (which joins the pool) while holding `ENGINE_STATE` (`mozjs/src/rust.rs:236-247`) | No helper thread takes `ENGINE_STATE` (it's Rust-side; helpers are C++), so the join can't wait on it. Stated, not proven; the cross-family lens checks it |
| CP-11, a wrong atomic ordering | Upstream's own code; unchanged by us | Not re-proven here; it's Firefox's tested Linux path |
| Kernel cross-CPU (scheduler, futex, reap) | **Exercised**, not changed: four CPUs of helper threads in one process | The existing kernel guards; any regression shows in the four-shard gate |

The kernel isn't modified, so the SMP-core rule (a cross-family lens for kernel concurrency) doesn't strictly
trigger. **The seams that are new** (SpiderMonkey's pool on neo-crt's pthreads and futex condvar, thread exit under
`JS_ShutDown`, process exit with helpers alive) **still get a cross-family lens** before merge, as the C3c THINK
said.

## 6. Decisions for J.c. / the PM

- **D1: the pool.** (a) SpiderMonkey's `InternalThreadPool` on neo-crt pthreads (what Firefox runs, and what this
  THINK assumes), or (b) `SetHelperThreadTaskCallback` with the smoke's own `std::thread` pool. (b) makes check 21
  exact (the smoke counts tasks) but needs a bindings change and isn't what Servo will run. **Recommendation: (a).**
- **D2: the kill-switch.** The rung stays opt-in as a whole (`mozjs` is outside the root workspace; default builds are
  bit-identical). For "helper threads off ≡ C3c-1": (a) keep it build-time only (the old patch hunk, restorable),
  or (b) a one-line patch exporting `js::DisableExtraThreads()` as a C symbol, so the smoke can run both modes in one
  build. **Recommendation: (b)**, a scenario `mozjs_smoke_single` that calls it before `JS_NewContext` and must
  match C3c-1's output with check 20 reading 0 threads created.

## 7. The build, in parts

- **C3c-2a:** patch 0003's hunk, the generator re-run, the neo-crt thread record (host-tested), smoke checks 20-25,
  the clean shutdown. Gate: the generator workflow, the closure build and the four shards with `mozjs_smoke`.
- **C3c-2b:** the OOM and crash variants (§4.3) and, if D2(b), `mozjs_smoke_single`.
- **Before merge:** the cross-family AAG lens on §5's touched rows.

## 8. What would prove this design wrong

- Check 21's control (§4.1) showing wakes with no work to do: the wake count isn't a task count.
- A shrinking GC on our target that leaves the heap's bytes where the normal GC left them (check 24), while
  survivors are intact: compaction isn't happening.
- `JS_ShutDown` returning with helper threads still counted live (check 22): the join isn't what we think it is.
