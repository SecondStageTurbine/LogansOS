<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c J3/J4 fix re-check #2 -- Jarvis

**Review type:** cross-family SMP/core code AAG / refutation

**Reviewed head:** `e6a75dee23c83cbdb0394528b672e9b0317c8a30` (`origin/claude/b1c-product-generation`)

**Fix commits:** `12827e00`, `b2e2f0bd`, `e6a75dee`

**Comparison head:** `056f89fe6a312ab57e873cb7e30bf62ae22dcf82`

**Verdict: BOUNCE.** The per-thread marker closes the Ready-queue version of J3 and the scoped J4 boundary/owner-safety issue, but J3 is still open through a second scheduler state: a marked `SYS_INFER_RUN` graph caller can park in the accelerator admission token table, and both forced terminate and the sibling fault-kill path zombify it without consulting `inference_lifecycle`. That reproduces the raw-stack `Arc` leak and can wedge the accelerator queue behind a dead principal. There is also a narrower cancellation hole: a torn-down waiter can win `BUSY` without a post-acquire cancellation check and run a full one-shot infer/masked forward.

## Requested closure status

| Item | Status | What I checked |
|---|---|---|
| **J1 -- interruptible/preemptible owner and non-killable release** | **CLOSED** | The shared-stack holder still runs through `with_interrupts_enabled`; `inference-stack` still implies `xsave-fpu`; the Ready-queue kill rule now skips every marked participant rather than one owner (`kernel/src/mm/inference_stack.rs:263-290,307-347`; `kernel/src/sched/scheduler.rs:327-363,2379-2412`). The holder marker lasts until its arm has returned and the stack guard has dropped. The earlier shootdown/preemption gate is unchanged. The remaining parked-thread finding below is not a shared-stack holder path. |
| **J3 -- waiter cannot be force-zombified while its stack owns resources** | **STILL OPEN** | The intended Ready waiter is fixed: all four dispatcher arms enter `inference_syscall`, the wrapper marks before invoking the arm, and `position_by_pid_killable` skips marked Ready threads (`kernel/src/arch/x86_64/syscall.rs:980-1018,1168-1203`; `kernel/src/sched/scheduler.rs:327-345`). I then checked every other forced-zombie path. `zombify_parked_thread_by_pid` and `zombify_thread` ignore the marker (`scheduler.rs:2027-2071`), and the graph route of marked `SYS_INFER_RUN` can enter exactly that token-parked state through accelerator admission (`syscall.rs:2794-2956,4161-4180`; `kernel/src/ai/admission.rs:144-224`). See J3-P below. |
| **J4 -- lifecycle through the boundary decision; coherent owner state** | **CLOSED (scoped)** | `inference_syscall` takes `torn_down` while marked, after the arm and its locals have returned, then clears the marker/count and exits if needed (`syscall.rs:1175-1197`). Entry increments the process count before marking; exit clears the thread marker before decrementing, so the two indicators overlap in the safety direction. Syscall IF=0 prevents a local preemption in either transition. A remote terminate after the false exit snapshot can still hit the generic Running/unqueueable syscall-return window, but CP-65 names that pre-existing system-wide residual and this patch does not add an inference resource leak there. `OWNER_TID`/`OWNER_PID` are separate atomics, so masking local interrupts does not literally give remote readers an atomic tuple; however both readers are `neo-test`-only and no product safety decision relies on them (`inference_stack.rs:178-197,307-347`). That wording should be narrowed, but it does not keep J4 open. |
| **F5 -- scoped shared inference stack, SMP/kill safe** | **STILL OPEN** | Stack exclusion, interruptibility, guard/canary, FPU preservation, and Ready-owner/waiter protection remain coherent. F5 cannot close while a marked inference syscall is still force-zombifiable from the token table, and the cancellation contract is incomplete for successful acquisition and one-shot forwards. |

## Blocking finding

### J3-P -- HIGH -- a marked graph-infer waiter is token-parked, force-zombified, leaked, and later admitted with no thread

The marker is broader than the shared stack: the dispatcher wraps **every** `SYS_INFER_RUN` call in `inference_syscall` (`kernel/src/arch/x86_64/syscall.rs:980-991`). For a non-inventory graph model, `sys_infer_run_impl` owns an `Arc<Model>` and a `Vec<Arc<Tensor>>`, then routes through `run_graph_infer` (`:2805-2897,2940-2956`). With a default accelerator, that calls `admission::run_scheduled`; a contending request executes `submit_and_park`, which puts the marked `Thread` in the scheduler token table (`:4161-4180`; `kernel/src/ai/admission.rs:144-200,210-224`; `kernel/src/sched/scheduler.rs:1690-1746`). This is product-reachable: `product` includes `accel-sim` and `boot-neofs` (`kernel/Cargo.toml:110-116`), and `sys_model_load_impl` still accepts ordinary NeoFS `.neom` files outside `/boot/models/` (`syscall.rs:2422-2526`), including graph models.

Both kill families bypass the new protection:

- normal terminate invokes `zombify_parked_thread_by_pid` when the Ready scan does not take the thread, in both the single-thread and browser loops (`kernel/src/sched/process.rs:2995-3014,3033-3087`);
- `fault_kill_current_user` uses the same parked-thread zombifier for siblings (`process.rs:2780-2827`);
- `zombify_parked_thread_by_pid` selects only by pid and changes the thread to Zombie without checking `thread.inference_lifecycle` (`kernel/src/sched/scheduler.rs:2048-2071`). Direct channel/process-wait extraction has the same marker-blind `zombify_thread` endpoint (`:2027-2046`), although the current inference arms do not themselves channel-park.

A concrete product interleaving is:

1. Process A holds the accelerator in graph `SYS_INFER_RUN`.
2. Process B enters graph `SYS_INFER_RUN`; the wrapper marks it, and the arm clones its model/tensor `Arc`s before admission parks it with a wake token.
3. Process C terminates B. The Ready scan cannot find B; `zombify_parked_thread_by_pid` removes and zombifies it despite the marker. Reap drops selected `Thread` fields but not Rust frames in the raw stack (`kernel/src/sched/thread.rs:190-207`), so the model/tensor/process `Arc`s and request vectors leak and `inference_participants` is never decremented.
4. A completes. `admission::complete` removes B's pending request, records B as `queue.running`, and ignores the failed/stale wake result (`kernel/src/ai/admission.rs:232-260`). No B thread exists to call `complete`, so later inference requests defer behind a dead principal indefinitely.

The process participant count is only used to satisfy terminate's accounting assertion; it neither prevents the parked zombification nor makes terminate wait (`process.rs:3052-3056,3080-3082`). Thus the new accounting can report the target handled while the lifecycle protocol has actually been bypassed.

**Required fix/gate:** make every force-zombie extraction lifecycle-aware, or limit the marker to paths whose every suspension state participates in cooperative cancellation and add an explicit admission-queue cancellation/purge protocol. A regression must hold a graph inference request in `admission::submit_and_park`, kill it through both `terminate` and the browser/fault sibling path as applicable, prove its destructors/count run, prove the pending request is removed rather than admitted stale, and prove a following graph inference is served.

## Additional finding

### C1 -- MEDIUM -- successful stack acquisition has no cancellation linearization point

`Held::acquire` checks `cancel_requested()` only after a **failed** `BUSY` CAS. If the process is terminated after that check/yield and the old holder releases before the waiter retries, the waiter wins the next CAS and returns `Some(Held)` without checking teardown (`kernel/src/mm/inference_stack.rs:307-334`). `try_run_checked` then calls `f` unconditionally (`:263-290`).

Plain generation checks cancellation before prefill and constrained generation checks at the top of each token loop (`kernel/src/arch/x86_64/syscall.rs:3186-3199,3692-3704`), so they unwind after this race. The other two real arms do not: inventory `SYS_INFER_RUN` calls `qwen_inventory::run` directly, and masked generation calls one full `next_logits` directly (`:2940-2956,3882-3895`). A killed waiter can therefore run a full prompt forward before the boundary finally exits. The kill gate cannot distinguish this: it keeps A parked while killing B and uses plain `SYS_GENERATE`, whose closure has the extra check (`kernel/src/test_harness.rs:34866-34906`).

This is leak-safe because the lifecycle marker remains set, but it contradicts the stated "waiter returns ECANCELED without running the forward" contract and can retain the global stack/model for one long forward after `terminate` returned. A holder killed during an already-running prefill, one-shot masked/infer forward, or claimed `par_matmul` work similarly has no cancellation point until that forward ends; the matmul barrier intentionally fail-stops rather than abandoning a live AP writer (`kernel/src/ai/par_matmul.rs:412-490`).

`SYS_GENERATE`, constrained, and masked use `ECANCELED` as their cancellation sentinel. `SYS_INFER_RUN` instead passes `InferenceError::InvalidInput`, which maps to `EINVAL` (`syscall.rs:2622-2630,2943-2964`). A torn-down real user thread exits in the wrapper and does not observe either errno, so this is not a separate ABI leak, but the implementation does not provide one uniform ECANCELED result that a gate can assert.

**Required fix/gate:** establish a cancellation check after winning `BUSY` and immediately before entering the closure, and put an explicit pre-forward check in the infer and masked closures (or give `try_run` a cancellation-aware closure contract). Add a release-race discriminator: tear B down while it is a waiter, release A before B's next CAS, and assert B performs zero model-forward work.

## Remaining requested checks

- **Marker coverage:** all four dispatch arms are wrapped before their implementation locals. I found no product caller of `inference_stack::{run,try_run}` outside the four real arms; kernel/test callers use the non-cancelling `run` contract. The uncovered state is not another stack caller but graph `SYS_INFER_RUN` parking before/around `run_graph_infer`.
- **Kill paths:** Ready-queue force-kill respects the marker. Token-parked zombification does not (J3-P). Current inference arms do not channel-park, so channel extraction is not independently reachable from the real-LM forward; graph admission is the concrete parked route. Fault-kill inherits J3-P for an inference sibling.
- **Cancellation visibility:** `torn_down` reads the registry/exiting state with Acquire and is safe without a long-held lock (`kernel/src/sched/process.rs:1587-1599`). Once teardown is published, checks observe it; the problem is missing check placement, not a stale relaxed read. Kernel pid 0 is explicitly never cancelled (`inference_stack.rs:292-298`).
- **Boundary:** arm locals and `Held` drop before the marked exit snapshot. The post-snapshot generic Running window remains and is documented. No marked thread can be timer-preempted between marker clear and boundary exit because the inference stack restores syscall-entry IF=0.
- **Owner tuple:** local IF masking prevents this CPU from publishing a half tuple and then being preempted. Separate atomics do not make a coherent remote snapshot, but only test code reads them now; the kill and terminate paths use the lifecycle marker/count instead.
- **Gate:** the BSP's preemption-disable section makes both synthetic callers Ready while CPU 1 terminates them; the model-Arc and frame baselines plus the leaking control have teeth for the original Ready waiter. The stand-in carries the target pid/pmap and executes the exact wrapper/arm, but it is not registered as a ring-3 process thread and never exercises admission parking. Therefore it can pass with J3-P and C1 present.
- **Feature-off/regressions:** with `inference-stack` off, the new `Thread`/`Process` fields and kill branch are cfg-absent, and `inference_syscall` calls the old arm directly; heap/MAX_ORDER/thread-stack policy is untouched by this fix. I verified semantic source equivalence, not a binary hash. Product real-LM, the kill feature, and the agentic feature combinations all compile. The prior overflow/shootdown code is not changed by these commits; QEMU runtime verification was unavailable as recorded below.

## Verification

- RAG health: **healthy** (`ollama=true`, `qdrant=true`). I used it for discovery and checked every finding against this worktree's source.
- `cargo test -p ai-ops-tests -p module-reclaim-logic-tests`: **PASS** -- 250 AI-op tests, 21 hostile-oracle integration tests, and 26 module-reclaim/stack tests.
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --no-default-features --features product,real-lm,model-boot-module`: **PASS** (warnings only).
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --features neo-test-inference-kill`: **PASS** (warnings only).
- `cargo check -p neoplutarch-kernel --target x86_64-unknown-none --features neo-test-arthur-agentic-actions`: **PASS** (warnings only).
- `NEO_SMP=4 cargo xtask neo-test inference_stack_kill inference_stack_kill_control`: **NOT RUNNABLE on this worker.** `LIMINE_DIR` is unset, `/home/neo/limine` does not exist, and neither `xorriso` nor `qemu-system-x86_64` is installed in the Windows/Ubuntu execution environment. Old runner `target/iso-root` directories contain staged Limine payload files, not a complete runnable tool environment. I did not treat this environment limitation as evidence for either verdict.

## Merge gate

Do not merge this head. Close J3-P with lifecycle-aware token/admission cancellation and a real parked-graph-infer kill discriminator. Close C1 with a post-acquire/pre-forward cancellation point and a release-race gate. J1 and scoped J4 can stay closed; F5 remains open until both paths are covered.
