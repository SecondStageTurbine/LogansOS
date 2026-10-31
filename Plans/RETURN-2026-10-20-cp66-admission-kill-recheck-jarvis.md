<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-66 admission-park kill recheck (Jarvis / Codex)

**Reviewed:** `claude/admission-park-kill-fix` at `9c7c6bd81b4715d235a41226176ba23a71e20342`  
**Verdict:** **MERGE-WITH-NITS**  
**J3-P:** **CLOSED**

The fix closes the original leak-and-wedge: a process teardown can no longer raw-zombify a thread whose stack owns the graph request's `Arc`s. Pending callers are withdrawn and woken to unwind; a current holder is allowed to finish, release accounting/admission state, and then exits at the syscall boundary. I found no correctness blocker after tracing the scheduler, both termination families, admission locking, and the real syscall/output path.

## Findings

### N1 — low: the fault-kill lock claim is stale

`kernel/src/sched/process.rs:2772` still says `fault_kill_current_user` takes no AI-governance lock, but the function now calls `admission::cancel_pid` at `kernel/src/sched/process.rs:2811`, which takes `QUEUES` at `kernel/src/ai/admission.rs:353` and then the scheduler lock when waking a token (`kernel/src/ai/admission.rs:365`).

**Repro:** compare the rustdoc claim at `process.rs:2772` with the call and lock sites above. This is documentation drift, not a lock-order defect: both cancellation and completion use `QUEUES -> scheduler`, the park path installs its token under the same order, and this exception path cannot already hold `QUEUES`.

**Suggested follow-up:** describe the path as allocation-free and limited to futex/wait purge, admission cancellation, and scheduler removal rather than saying it takes no AI-governance lock.

### N2 — low: the leak scenario records heap drift but does not gate on it

The scenario captures and prints `heap0` / `heap_delta` (`kernel/src/test_harness.rs:18067`, `:18173-18182`) but only gates exact model/input `Arc` restoration and a frame tolerance (`:18188-18217`).

**Repro:** introduce an unrelated small heap leak after the baseline; the scenario can still pass if the `Arc` counts and frame tolerance hold. This does not weaken the J3-P discriminator: the original raw-stack leak is caught exactly by the model/input strong counts, while the negative control also demonstrates the dead-principal wedge.

**Suggested follow-up:** either bound heap drift explicitly or label it diagnostic-only.

## Numbered adversarial check

1. **Every force-zombify route.** `kill_deferred` protects a marked thread in both Ready removal (`kernel/src/sched/scheduler.rs:316-320`) and token-table removal (`:1175-1181`, predicate at `:1287-1288`). The fault path calls `cancel_pid` before repeatedly draining those queues (`kernel/src/sched/process.rs:2802-2818`). Reaper IPC/process-wait drains do call marker-blind `zombify_thread`, but a graph inference caller cannot simultaneously be parked in an IPC endpoint or process-wait queue; admission wait and accelerator reservation waits are scheduler-token parks and therefore covered. `wait_any` is purged before token draining. I found no third route that can own the admission request's stack frames and bypass the two protected scheduler scans.

2. **No park after cancellation.** `submit_and_park` resolves the process before taking `QUEUES`, reads `exiting` while holding `QUEUES`, drops the temporary process `Arc`, then either refuses or atomically installs the pending request and parks (`kernel/src/ai/admission.rs:196-247`). Teardown publishes `exiting` before `cancel_pid` (`kernel/src/sched/process.rs:2984-2997`; fault path `:2797-2811`). Therefore cancellation either sees an installed request, or submission sees `exiting`. A missing registry entry is treated as torn down (`admission.rs:201-209`). A failed forced-exit CAS returns before claiming teardown (`process.rs:2976-2983`); the successful/natural owner remains responsible, and natural last-thread exit cannot race a still-live sibling into a new admission park.

3. **Marker windows and interrupt state.** `run_scheduled` marks before submission and clears only after a cancel return or after `complete` (`kernel/src/ai/admission.rs:266-284`). Thus the waiter owns request `Arc`s only while marked, and a holder remains marked through accounting, work, and queue release. The syscall enters with interrupts disabled; product's preemptible accelerator dispatch temporarily enables them at `kernel/src/drivers/accel/accel_cpu.rs:134` and restores the prior disabled state on return, so `complete`, marker clear, output handling, and the boundary check are not timer-preemptible. Default does not enable them. Killing after marker clear but before the boundary therefore does not create a new Ready/token-removal window. The known Running/unqueued syscall-return residual is unchanged.

4. **Cancellation wake.** `cancel_pid` holds `QUEUES` while removing each request and waking its token with `ECANCELED` (`kernel/src/ai/admission.rs:349-376`), matching `complete`'s lock order (`:293-337`). The wake-token table's owning CPU was initialized when the thread parked; cross-CPU wake uses that initialized scheduler. CP-31 re-entry capacity is the same established token-wake path already used by `complete`. Serialization on `QUEUES` resolves the apparent race: if `complete` wins, it removes the request, records it as running, and wakes it normally, after which `cancel_pid` counts the holder; if cancellation wins, it removes and wakes the waiter, so completion cannot later admit it. `complete` also retries when a selected token is stale/gone (`:320-337`).

5. **No live-process false cancellation.** The only admission wake carrying `ECANCELED` (`-125`) is `cancel_pid` (`kernel/src/ai/admission.rs:365`; constant at `kernel/src/errno.rs:80`). Normal `complete` wakes with zero (`admission.rs:321`). An exhaustive exact search found no other `-125`/`ECANCELED` token waker, so the verdict at `admission.rs:244` is unambiguous.

6. **Killed holder safety.** The marked holder's stack retains the model/input `Arc`s until `exec::run` returns. `InflightGuard` is scoped around only the work and drops before `complete` (`kernel/src/ai/admission.rs:278-283`), so accounting is released before queue handoff. `complete` always clears the old running/reservation state and either admits a wakeable successor or retries. If teardown already removed the process, the post-inference output path's `current_process_for_syscall` fails before arena allocation/capability installation (`kernel/src/arch/x86_64/syscall.rs:2734-2755`); the local output `Arc`s then drop normally. The scheduler thread retains the address-space anchor needed to finish the kernel call. I found no dereference of the removed registry entry and no UAF.

7. **Terminate accounting.** Non-browser product is single-threaded: if `in_admission != 0`, that sole thread is either the cancelled waiter or finishing holder, so skipping channel extraction at `kernel/src/sched/process.rs:3039` is sound; it cannot also be channel-parked. Browser-runtime still loops over every Ready/token-parked sibling and drains every channel sibling (`:3004-3018`, `:3065-3078`). The assertion at `:3079-3083` is existential and can still be satisfied by one handled class while the already-known Running/unqueued residual exists, but CP-66 does not introduce or worsen that residual.

8. **Tests are non-vacuous.** The stand-ins bind the real pid/pmap (`kernel/src/test_harness.rs:17957`) and invoke the exact `SYS_INFER_RUN` wrapper/boundary; they use real model, tensor, handle, and user-page setup. Waiter legs exercise the production park/token cancellation and both terminate/fault families. The holder hook is inside the `run_scheduled` work closure (`kernel/src/arch/x86_64/syscall.rs:3753-3756`) and yields while both the marker and `queue.running` are live; that deterministically produces the scheduler state a timer preemption inside product `dispatch_cpu_op` would produce. With cancellation disabled, the control raw-zombifies the waiter, retains the stack `Arc`s, admits the dead pid, and wedges before the later holder leg; the assertions at `kernel/src/test_harness.rs:18188-18210` fail for those intended reasons. N2 above is only an additional coverage nit.

9. **Composition with B1c (#74).** The two markers cover different lifetimes and should compose without nesting conflicts: B1c's inference lifecycle protects generate/inventory and its narrowed inference-stack arms, while CP-66 owns graph admission and its cooperative unwind. Merge both Ready and token predicates as:

   ```rust
   let protected = (crate::ai::admission::CANCEL_ON && thread.admission_section)
       || thread.inference_lifecycle;
   thread.pid == pid && !protected
   ```

   Use the inference-lifecycle feature/control predicate from #74 if that branch gates it separately. Keep CP-66's outer `sys_infer_run_syscall` teardown boundary for graph requests. An inventory/generate call may exit through B1c's inner wrapper first; graph is not marked by B1c after its scoped narrowing, so neither path double-handles the same graph request.

10. **Other checks.** I verified the cancellation errno mapping (`kernel/src/arch/x86_64/syscall.rs:2313-2322`), wrapper dispatch (`:1019`, `:2473-2494`), queue handoff backstop, feature wiring, and the full diff against `origin/main`. No additional correctness finding survived re-verification.

## Validation

- RAG health: healthy (`ollama=true`, `qdrant=true`); used for semantic discovery, followed by direct source reads.
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features product` — PASS.
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features neo-test-admission-park-kill-threaded` — PASS (13 pre-existing dead-code warnings).
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features neo-test-admission-park-kill-control` — PASS (same warnings).
- `cargo test -p ai-ops-tests` — PASS, 264 tests (243 unit + 21 hostile-oracle), 0 failed.
- QEMU scenarios were not run: `LIMINE_DIR` is unset and `xorriso` / `qemu-system-x86_64` are unavailable on this worker. The PM runtime gate must supply the positive and negative-control executions.

