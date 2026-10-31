# Test-runner intermittent wedge — investigation prep notes

**Date:** 2026-05-07
**Status:** DRAFT research artifact — not normative. Input for future Cerebro diagnostic dispatch.
**Author:** Jarvis
**Substrate:** Serial logs from KASLR-Q1B (50 runs), KASLR-Q1 (10 runs), ADR-015 implementation loop (in-flight ~196 scenario logs), and sub-commit 2 bisection experiments (EXP-G, restore-attempt).

---

## 1. Summary

The test-runner intermittent wedge manifests as a kernel hang during or immediately after the in-kernel scheduler smoke test (ping/pong/hog threads) that runs at the start of every test scenario. Across all available serial logs, **5 distinct wedge instances** were found across four diagnostic campaigns: 1 from KASLR-Q1B, 2 from KASLR-Q1, 1 from sub-commit 2's restore attempt (the original EXP-H trigger), and 1 from the in-flight ADR-015 loop. The close report for KASLR-Q1B records **14/50 TIMEOUTs (28%)**, but only 1 per-scenario serial log from those 14 runs is present on disk — the loop wrapper captures the failing scenario's log, not all scenarios from a timed-out run.

All 5 observed wedge instances share a single signature: the in-kernel smoke test spawns ping, pong, and hog; they execute their ping-pong exchanges; one or two get reaped, then the kernel produces no further serial output. The hang occurs in the `while any_user_process() { yield_now(); reap(); }` wait loop in `main.rs` (~line 173), where the BSP repeatedly yields and reaps but never observes an empty process registry.

The leading hypothesis is that the BSP-scans-all-CPUs reap (sub-commit 2's §Q9 stopgap) occasionally fails to drain a zombie from an AP-resident scheduler, leaving a user process permanently in `PROCESSES`. This is consistent with §Q9's original failure mode — the bisection that deferred §Q9 showed the same partial-reap pattern — and suggests the wedge is a pre-existing SMP scheduling bug that sub-commit 2's stopgap did not fully eliminate.

## 2. Observed symptoms

### 2.1 Wedge instances with serial logs on disk

| Run ID | Source loop | Scenario that wedged | Last serial line before hang | Pre-hang anomalies |
|---|---|---|---|---|
| `build-3-2-restore-ipc-100x-run-81` | sub-commit 2 restore (original EXP-H trigger) | `ipc_heap_kill_smoke` (smoke test phase) | `[ kernel ] sched: reaped 'pong' (id=5)` | None. Smoke test ran normally; pong reaped, then silence. |
| `KASLR-Q1-main-run-1.smap_catch` | KASLR-Q1 (initial 10-run diagnostic) | `smap_catch` (smoke test phase) | `[ kernel ] sched: reaped 'ping' (id=1)` | None. Smoke test ran normally; ping reaped, then silence. |
| `KASLR-Q1-main-run-8.slab_local_to_global_promotion` | KASLR-Q1 (initial 10-run diagnostic) | `slab_local_to_global_promotion` (smoke test phase) | `[ kernel ] sched: reaped 'ping' (id=1)` | None. Identical to run-1 above — ping reaped, then silence. |
| `KASLR-Q1B-main-run-3.buddy_max_order` | KASLR-Q1B (50-run diagnostic) | `buddy_max_order` (smoke test phase) | `[ kernel ] sched: reaped 'pong' (id=2)` | None. Smoke test ran normally; pong reaped, then silence. |
| `ADR-015-loop-run-1.smp_per_cpu_scheduler_smoke` | ADR-015 implementation loop (in-flight) | `smp_per_cpu_scheduler_smoke` (scenario body) | `[ kernel ] sched: reaped 'smp-hog-0' (id=7)` | Distinct from smoke-test wedges: 4 smp-hog threads spawned on APs, only smp-hog-0 reaped. See §3 Signature B. |

### 2.2 Wedge instances recorded in close reports but without serial logs on disk

The KASLR-Q1B close report (`Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md`) records **14 TIMEOUT runs** with this scenario distribution:

| Scenario | Count |
|---|---|
| `buddy_alloc_free_alloc_returns_same_frame` | 1 |
| `buddy_max_order` | 2 |
| `buddy_split_coalesce` | 2 |
| `cap_derive_subset` | 1 |
| `cap_grant_missing_meta` | 1 |
| `cap_mint_and_use` | 1 |
| `cap_survive_exit` | 2 |
| `expect_pass_but_alloc_fail` | 1 |
| `ipc_heap_kill_smoke` | 1 |
| `slab_local_to_global_promotion` | 1 |
| `smap_policy` | 1 |

Only 1 of these 14 runs has a per-scenario serial log on disk (`KASLR-Q1B-main-run-3.buddy_max_order`). The remaining 13 TIMEOUT runs were classified by the loop wrapper's timeout detection; the scenario-specific logs may not have been written because the harness terminated the QEMU process at timeout, or the logs were for scenarios that ran before the wedging scenario in the same boot and were not individually captured.

### 2.3 EXP-G instrumentation hang (distinct context)

`EXP-G-fail-run-10` (sub-commit 2 with §Q9 still present, instrumented reap) shows a different hang locus: after `ipc_heap_kill_smoke`'s IPC assertions pass and the worker thread is reaped, the kernel hangs at `[ kernel ] proc: user thread exited with code 0`. This is post-reap, not pre-reap. The EXP-G hang was §Q9-specific (resolved by reverting §Q9 in EXP-H) and is not part of the current wedge investigation. It is included here for completeness but classified separately.

## 3. Wedge signatures

### Signature A: `smoke-test-partial-reap` (4 instances)

**Runs:** `build-3-2-restore-ipc-100x-run-81`, `KASLR-Q1-main-run-1.smap_catch`, `KASLR-Q1-main-run-8.slab_local_to_global_promotion`, `KASLR-Q1B-main-run-3.buddy_max_order`

**Description:** The in-kernel scheduler smoke test spawns three threads: ping (id=1 or 4), pong (id=2 or 5), and hog (id=3 or 6). All three execute normally — ping and pong exchange messages, hog triggers preemption. Then reaping begins: exactly one thread gets reaped (either ping or pong), and no further serial output appears. The hog thread is never reaped. Neither is the second of ping/pong.

**Kernel state at hang:** The BSP is in `main.rs`'s wait loop (`while any_user_process() { yield_now(); reap(); }`). The loop calls `reap()` repeatedly, but `reap()` either drains zero zombies (because the remaining zombies are not on any accessible queue) or fails to produce observable progress. The `any_user_process()` check keeps returning true because at least one user process remains registered in `PROCESSES`.

**Plausibly implicated subsystems:**
- **Scheduler (primary):** BSP-scans-all-CPUs reap drains each per-CPU scheduler's zombie queue under that scheduler's lock. If a zombie thread is somehow not in any zombie queue, or if the BSP fails to acquire an AP scheduler's lock (deadlock with AP holding it), the zombie is never reaped.
- **Per-CPU state:** If a thread exits on an AP and its zombie goes into the AP's local queue, but the BSP's scan misses that queue (e.g., null `scheduler_ptr`, zero `zombie_count` despite non-empty queue), the zombie is orphaned.
- **Interrupt routing:** If the PIT timer fires on an AP and the AP's interrupt handler moves a thread to Zombie state but the `zombie_count` increment races with the BSP's scan, the BSP could see `zombie_count == 0` and skip the queue while it actually contains zombies.

### Signature B: `smp-hog-partial-reap` (1 instance)

**Run:** `ADR-015-loop-run-1.smp_per_cpu_scheduler_smoke`

**Description:** The `smp_per_cpu_scheduler_smoke` scenario spawns 4 threads (`smp-hog-0` through `smp-hog-3`, ids 7–10), each pinned to a different CPU. All four spawn successfully. Only `smp-hog-0` (id=7) is reaped; smp-hog-1/2/3 never are.

**Kernel state at hang:** Same wait loop as Signature A, but the scenario-specific logic has already begun (this is not the in-kernel smoke test — it's the user-space scenario's own thread management). The scenario likely waits for all 4 workers to complete; with 3 of 4 unreaped, `any_user_process()` stays true.

**Plausibly implicated subsystems:** Same as Signature A, but more directly tied to AP-resident threads. The smp-hog threads are explicitly placed on different CPUs (the scenario name says "per-CPU scheduler smoke"). If threads 8–10 exit on their respective APs and the BSP's reap scan cannot drain those AP queues, exactly this pattern emerges: thread 7 (on BSP or first-scanned CPU) reaps, threads 8–10 (on APs) do not.

**Relationship to §Q9 redo notes:** `Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md` §3.3 discusses this exact scenario — AP-side reaping is dormant in sub-commit 2, and the BSP-scans-all-CPUs stopgap may miss AP-resident zombies. This signature is strong evidence that the stopgap is incomplete.

### Assessment: single root cause or multiple?

The two signatures are likely the same root cause operating at different levels. Signature A hangs during the kernel's own smoke test (which runs before any user scenario); Signature B hangs during a user scenario that explicitly tests per-CPU thread placement. Both show partial reap — some threads reaped, others not — and both involve AP-resident threads being missed by the BSP's scan. The difference is which threads are affected: in A, it's the generic ping/pong/hog; in B, it's the smp-hog-N threads placed on specific CPUs.

## 4. Correlation checks

### 4.1 Wedge vs. KASLR placement

| Run | virt_base | Distance from ceiling (0xffffffffffffffff) | Classification |
|---|---|---|---|
| `ADR-015-loop-run-1` (wedge) | `0xffffffff916a7000` | ~0x6e958000 (~1.8 GiB below) | WEDGE — low placement |
| `KASLR-Q1B-main-run-3` (wedge) | `0xffffffffd0b13000` | ~0x2f4ec000 (~0.76 GiB below) | WEDGE — mid-high |
| `KASLR-Q1-main-run-1` (wedge) | `0xffffffffd0821000` | ~0x2f7de000 (~0.76 GiB below) | WEDGE — mid-high |
| `KASLR-Q1-main-run-8` (wedge) | `0xffffffffc0764000` | ~0x3f89b000 (~1.02 GiB below) | WEDGE — mid |
| `build-3-2-restore-run-81` (wedge) | `0xfffffffffe221000` | ~0x01dd5000 (~30 MiB below) | WEDGE — very high |
| `KASLR-Q1B-main-run-1` (PASS, 24 scenarios) | `0xffffffffa47de000` through `0xffffffffff826000` | Wide range | PASS across all placements |

**Result: ambiguous.** The wedge instances span the full KASLR placement range — from very high (`0xfffffffffe221000`, ~30 MiB below ceiling) to low (`0xffffffff916a7000`, ~1.8 GiB below). The PASS runs in KASLR-Q1B run 1 also span the full range, including placements as high as `0xffffffffff826000` (which actually panicked on heap overflow, not wedged). No clear correlation between KASLR placement height and wedge occurrence.

However, the sample size is too small for a definitive conclusion. Five wedges vs. 25+ PASS runs with known placements gives low statistical power. The ADR-015 loop's single wedge at run 1 (out of ~196 scenario logs) suggests the rate dropped significantly after ADR-015's heap-placement fix — but that may be because ADR-015 eliminated the PANIC classification, not the wedge.

### 4.2 Wedge vs. scenario

**Result: correlated with the smoke test, not with specific scenarios.** All four Signature A instances occur during the in-kernel scheduler smoke test that runs at the start of every scenario, before any user-space test code executes. The KASLR-Q1B close report lists 11 distinct scenarios across 14 TIMEOUTs — `buddy_max_order`, `cap_derive_subset`, `ipc_heap_kill_smoke`, `smap_policy`, and others — but the serial logs show the hang occurs at the smoke test phase, not during the scenario body. The scenario name in the close report reflects which scenario was running when the timeout fired, not which scenario caused the wedge.

Signature B (`smp_per_cpu_scheduler_smoke`) is different: that scenario explicitly tests per-CPU thread placement, and the hang occurs within the scenario body (after 4 smp-hog threads are spawned). This is consistent with the same root cause — AP-resident zombies not being reaped by the BSP — but exercised more directly.

### 4.3 Wedge vs. NEO_SMP value

**Result: wedge requires SMP ≥ 2 (inferred, not directly tested against this specific bug).** All observed wedges occurred at `NEO_SMP=4`. The sub-commit 2 restore attempt (run 81) was at `NEO_SMP=4`. KASLR-Q1 and KASLR-Q1B were both `NEO_SMP=4`. The ADR-015 loop SMP value is unknown from the logs but likely also 4.

EXP-F tested `ipc_heap_kill_smoke` at `NEO_SMP=1` with 100/100 PASS, but that was the §Q9-triggered hang (different bug). The current wedge (BSP-scans-all-CPUs stopgap) has not been tested at `NEO_SMP=1`. Logically, at SMP=1 there are no APs to miss, so the BSP's scan would be a local operation and should never miss zombies. **Testing this specific wedge at SMP=1 would be a high-value diagnostic.**

### 4.4 Wedge vs. boot-time anomalies

**Result: not correlated (no pre-hang anomalies observed).** All five serial logs show clean, normal boot sequences: Limine loads kernel and modules, GDT/IDT load, per-CPU state initializes, slab allocators set up, APs start in order, schedulers ready, preemption enabled. No unusual ordering, missing lines, or interleaving issues distinguish the wedged runs from clean runs.

The one marginal observation: in `KASLR-Q1-main-run-1.smap_catch` and `KASLR-Q1-main-run-8.slab_local_to_global_promotion`, the smoke test's ping/pong thread IDs start at 1 (not 4 as in most other logs), suggesting these runs used an older kernel version where the smoke test threads were assigned lower IDs. This is a version difference, not a boot anomaly.

## 5. Hypotheses

### H1: BSP reap scan races with AP zombie production

**Mechanism:** The BSP's `reap()` at `scheduler.rs:523` iterates all per-CPU schedulers and checks `zombie_count` (lock-free Acquire load) before acquiring the lock. If an AP produces a zombie between the BSP's `zombie_count == 0` check and the BSP's next iteration of that CPU, the zombie is missed. The `zombie_count` is incremented inside `without_interrupts()` on the AP, but the BSP's scan is not atomic with respect to AP activity — it checks each CPU sequentially while APs continue executing.

**Confirming evidence:** The partial-reap pattern (some threads reaped, others not) is consistent with a timing-dependent miss. Threads that become zombie before the BSP's scan are caught; threads that become zombie during or after the scan are missed.

**Refuting evidence:** The `reap()` function in sub-commit 2 holds no lock while iterating CPUs — it acquires each CPU's lock independently. If `zombie_count` is incremented under the same lock as the queue push, the check-then-lock pattern is still racy: the BSP reads `zombie_count == 0`, then an AP pushes a zombie and increments count, then the BSP skips that CPU.

**Test:** Add instrumentation to `reap()` logging each CPU's `zombie_count` before and after lock acquisition. If the counts differ between check and drain, this hypothesis is confirmed.

### H2: AP scheduler lock is held by the AP when BSP tries to acquire it

**Mechanism:** The BSP's `reap()` acquires each per-CPU scheduler's inner lock to drain zombies. If an AP holds its own scheduler lock (e.g., while executing its interrupt handler, which calls `rotate()` or `push_ready()` under the lock), the BSP blocks on that lock. If the AP is itself blocked (waiting for the BSP, or in a spin loop), this becomes a deadlock.

**Confirming evidence:** The hang produces no further serial output — consistent with a lock contention where neither side can proceed.

**Refuting evidence:** The BSP's `reap()` runs with interrupts disabled (`without_interrupts`), so AP interrupt handlers should not contend with the BSP during the drain window. However, APs execute code outside interrupt context (e.g., their bootstrap idle loops), and if any such code holds the scheduler lock, the BSP could block.

**Test:** Add a timeout to the BSP's lock acquisition in `reap()`. If the timeout fires, log which CPU's lock was contended and what the AP is doing.

### H3: Thread exits on AP but zombie goes to wrong queue

**Mechanism:** When a thread calls `exit()`, it transitions to Zombie state via `rotate(ThreadState::Zombie, WhichQueue::Zombie)`. This moves the `Box<Thread>` from `current` into the scheduler's zombie queue. If the thread's `cpu_id` field is stale (e.g., the thread was migrated but its recorded CPU didn't update), the zombie could go into a different scheduler's queue than expected, or not be found by the BSP's scan.

**Confirming evidence:** Signature B specifically involves threads placed on APs via the `smp_per_cpu_scheduler_smoke` scenario. If thread placement doesn't correctly update the thread's CPU affinity, zombies could be misplaced.

**Refuting evidence:** In sub-commit 2, threads are not migrated between CPUs (migration is sub-commit 3b). A thread spawned on a given CPU stays there. The BSP's scan iterates all initialized schedulers, so even if a zombie is on an AP's queue, the BSP should find it.

**Test:** Add instrumentation logging each zombie's `cpu_id` at exit time and at reap time. Mismatch indicates CPU affinity corruption.

### H4: `any_user_process()` returns true due to process registry not being cleaned up

**Mechanism:** The wait loop in `main.rs` (~line 173) spins on `while any_user_process()`. This function walks `PROCESSES` checking for non-kernel PIDs. If `on_thread_reap()` fails to remove a process from `PROCESSES` (e.g., early return, lock contention with `terminate()`), the process stays registered and the loop never exits.

**Confirming evidence:** The sub-commit 2 code's `on_thread_reap()` at `process.rs:659` has an early return if the process is already absent from `PROCESSES`. If a concurrent `terminate()` removed the process first, `on_thread_reap` would skip cleanup. However, this should not cause a hang — it would mean the process is already gone.

**Refuting evidence:** In the current (post-§Q9-deferral) code, `on_thread_reap()` does not use `live_thread_count`. It directly removes the process from `PROCESSES` via `PROCESSES.lock().remove(&pid)`. There is no counter that can underflow or skip cleanup. The hang is more likely in the reap path (zombie never reaching `on_thread_reap`) than in `on_thread_reap` itself.

**Test:** Add instrumentation to `any_user_process()` logging which PIDs remain registered when the loop has been running for N iterations. If specific PIDs persist, trace why their `on_thread_reap` didn't fire.

## 6. Proposed diagnostic dispatch sketch

### Questions

- **Q1:** Does the wedge occur at `NEO_SMP=1`? (If not, confirms AP involvement.)
- **Q2:** Which specific threads/processes remain alive when the wedge occurs? (Instrument `any_user_process()` to log remaining PIDs after N iterations.)
- **Q3:** Does the BSP's `reap()` actually acquire each AP scheduler's lock, or does it skip APs due to `zombie_count == 0`? (Instrument reap with per-CPU zombie_count logging before and after lock acquisition.)
- **Q4:** Are there zombie threads on AP queues that the BSP fails to drain? (After timeout, dump all per-CPU zombie queue lengths via a forced serial print.)

### Diagnostic changes (purely diagnostic, no fixes)

1. **Timeout backstop in wait loop:** Add a counter to `main.rs`'s `while any_user_process()` loop. After 500 iterations (~5 seconds at 100 Hz), print all remaining PIDs and all per-CPU zombie queue lengths to serial, then panic with a clear message. This converts the hang into a diagnostic dump.

2. **Per-CPU reap instrumentation:** In `scheduler.rs`'s `reap()`, log each CPU's `zombie_count` before the lock acquisition and the actual number of zombies drained after. Pattern: `[ DBG ] reap: cpu=N count_before=X drained=Y`.

3. **Zombie placement logging:** In `rotate(ThreadState::Zombie, ...)`, log which CPU's queue received the zombie: `[ DBG ] zombie: thread id=X pid=Y placed on cpu=Z queue`.

### Runs

- 20 runs at `NEO_SMP=1` default suite — tests Q1 (SMP dependency).
- 20 runs at `NEO_SMP=4` default suite with instrumentation — tests Q2-Q4.
- If Q1 passes (no wedges at SMP=1), run 20 `smp_per_cpu_scheduler_smoke` specifically at `NEO_SMP=4` to target Signature B.

### Decision tree

- **Q1 PASS (no wedges at SMP=1):** Confirms AP involvement. Proceed to Q3/Q4 to determine if the issue is zombie placement (H3) or BSP scan timing (H1/H2).
- **Q1 FAIL (wedges at SMP=1):** Rules out AP-specific causes. Focus on H4 (process registry cleanup) or a BSP-local scheduling bug unrelated to per-CPU state.
- **Q3 shows `count_before == 0` but `drained > 0`:** Confirms H1 — the lock-free zombie_count check races with AP zombie production. Fix: remove the `zombie_count == 0` early-out; always acquire the lock and drain.
- **Q3 shows `count_before > 0` but `drained == 0`:** Suggests queue corruption or lock ordering issue (H2). Investigate whether the BSP blocks on AP locks.

## 7. Cross-references

- **`Plans/Q9-REDO-DESIGN-NOTES-2026-05-07.md`** — §Q9 redo design notes; §3.3 directly addresses AP-side reaping and the BSP-scans-all-CPUs stopgap's limitations. The `smp-hog-partial-reap` signature (Signature B) is the exact scenario discussed there.
- **`Plans/CLOSE-REPORT-2026-05-07-kaslr-q1b.md`** — KASLR-Q1B close report; 14/50 TIMEOUTs with per-scenario distribution. Primary source for the wedge rate estimate.
- **`Plans/CLOSE-REPORT-2026-05-07-sub-commit-2.md`** — sub-commit 2 closure; references the wedge in passing.
- **`Plans/PM-DECISION-2026-05-06-sub-commit-2-smoke-hang.md`** — PM decision on the original run-81 hang; first identification of the partial-reap signature.
- **`Plans/PM-DECISION-2026-05-06-sub-commit-2-bisect-rollback.md`** — PM decision after EXP-G's instrument-amplified hang; distinguishes two different hang loci (early smoke test vs late IPC scenario).
- **`Plans/PM-DECISION-2026-05-06-sub-commit-2-close-without-q9.md`** — PM decision to close sub-commit 2 with §Q9 deferred; names the `live_thread_count` race as §Q9's failure mode (distinct from but related to the current wedge).
- **`kernel/src/sched/scheduler.rs:523`** — `reap()` function (BSP-only, scans all per-CPU zombie queues).
- **`kernel/src/sched/process.rs:659`** — `on_thread_reap()` (process cleanup after thread reap; no `live_thread_count` in current code).
- **`kernel/src/main.rs:173`** — `while any_user_process() { yield_now(); reap(); }` wait loop where the hang occurs.
- **`kernel/src/sched/process.rs:634`** — `any_user_process()` (walks PROCESSES for non-kernel PIDs).
- **`Plans/ADR-015-FINAL-2026-05-07-kaslr-tolerant-heap-placement.md`** — ADR-015 (KASLR heap placement fix); not directly related to the wedge but relevant for understanding why the ADR-015 loop shows 0 PANICs and only 1 wedge.

---

**End of notes.**

*Draft research artifact. Becomes input to a Cerebro diagnostic dispatch post-ADR-015 closure.*
