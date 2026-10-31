<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: B1c J1/J2 fix re-check -- Jarvis

**Review type:** cross-family SMP/core code AAG / refutation  
**Reviewed head:** `056f89fe6a312ab57e873cb7e30bf62ae22dcf82` (`origin/claude/b1c-product-generation`)  
**Fix commit:** `cebe04b5`  
**Base:** `c074037e3d82ee302e77707953ab186efbea4a64` (`origin/main`)

**Verdict: BOUNCE.** The original shootdown deadlock is fixed and J2/F1 are closed, but J1/F5 remain open: the non-killable-owner scheme does not cover a second inference caller waiting for the shared stack, and the owner marker is dropped before the syscall-boundary exit decision. Both holes are in the new force-kill design, and neither new discriminator exercises a kill.

## Requested closure status

| Item | Status | What I checked |
|---|---|---|
| **J1 -- interruptible/preemptible owner and non-killable release** | **STILL OPEN** | The original IF=0 shootdown failure is closed: `run_checked` invokes the closure through `with_interrupts_enabled` (`kernel/src/mm/inference_stack.rs:236-244,282-283`); the timer moves its frame from IST to the interrupted stack before scheduling (`kernel/src/arch/x86_64/timer.rs:1-28,187-302`); `inference-stack` implies `xsave-fpu` (`kernel/Cargo.toml:688-694`); and the new scenario forces 16 CPU1-to-BSP shootdowns plus timer/context-switch progress (`kernel/src/test_harness.rs:34589-34691`). The force-kill half is incomplete; see J3/J4 below. AP owners remain interruptible but not preemptible because only the BSP receives IRQ0, which the NOTE now states honestly. |
| **J2 -- literal bounds ordering through the real arms** | **CLOSED** | Traced all four implementations. Plain generation checks the complete register policy before pointers, model lookup, or allocation (`kernel/src/arch/x86_64/syscall.rs:3295-3347`). Constrained and masked check the register prompt length before their fixed descriptor, then the full policy before variable copies/allocation/lookup (`:3449-3488`, `:3697-3738`). Inventory infer necessarily classifies the model, then reads one handle into `[u8;4]`, looks up the tensor, and calls metadata-only `admit_input` before constructing `inputs` (`:2728-2823`; `kernel/src/ai/qwen_inventory.rs:47-56`). The probe activates a registered user pmap/CR3 on CPU1 and invokes those exact implementation arms; its 14 refusal cases sample the live heap counter before pushing the result, and its two admitted controls prove the counter moves (`syscall.rs:2072-2297`; `test_harness.rs:34289-34378`). It does not traverse the assembly `syscall_entry`, but it does traverse every ordering-relevant arm and real user-copy helper. |
| **F1 -- one bounds gate, first at each ABI** | **CLOSED** | `check_prompt_len` and `check_generate_request` state the attainable order explicitly and share the 2,048/256 policy (`kernel/src/ai/generate_logic.rs:21-155`). No refused case reaches a variable-size copy, heap allocation, or KV allocation; inventory infer's route/tensor lookups are allocation-free and are the documented prerequisite for discovering its tensor-derived bound. Host bounds/KV tests passed, and the boot probe is shaped to distinguish EINVAL from EFAULT/EBADF and zero allocations from admitted allocations. |
| **F5 -- scoped shared inference stack, including SMP/kill safety** | **STILL OPEN** | Stack switching, timer-frame relocation, XSAVE context switching, GLOBAL mapping, canary/guards, IRQ-safe heap allocation, and try-only `PAR_BUSY` are coherent. The single shared stack remains exclusive across preemption because the saved thread RSP points into it and contenders cannot pass `BUSY`. The missing lifecycle coverage is material: a contending caller is voluntarily schedulable and killable while its ordinary stack owns Rust resources; see J3. The release-to-boundary marker gap is J4. |

## New findings

### J3 -- HIGH -- force-kill can zombify an inference-stack waiter and leak every owned local on its suspended kernel stack

The patch makes only the current **holder** non-killable. `Held::acquire` publishes one `OWNER_TID`, and `kill_thread_by_pid` skips only that exact tid (`kernel/src/mm/inference_stack.rs:258-266`; `kernel/src/sched/scheduler.rs:2349-2379`). A second caller that loses `BUSY` calls `yield_now()` on every retry (`inference_stack.rs:300-313`). When that yield switches, the waiter is an ordinary Ready thread but is not `owner_thread()`; force-kill therefore removes it and makes it Zombie.

That waiter is not destructor-empty. Before plain generation reaches the shared-stack call, its ordinary syscall stack owns an `Arc<Model>` and the copied prompt `Vec` (`kernel/src/arch/x86_64/syscall.rs:3347-3363`). Constrained generation additionally owns the descriptor vectors/candidate slices/context (`:3497-3566`); masked generation owns the mask storage, model Arc and context (`:3755-3779`); inventory infer owns model/input tensor Arcs (`:2739-2823`). Reaping the zombified thread frees its raw kernel-stack backing and selected `Thread` fields; it cannot run destructors for arbitrary Rust frames that merely happen to occupy that byte buffer (`kernel/src/sched/thread.rs:100-121,185-202`). The source's own CP-30 comment says exactly why a suspendable force-zombified kernel stack must not own Arcs.

A concrete reachable interleaving needs no timer on the waiter:

1. A owns the inference stack.
2. B has copied its request and cloned the model, loses `BUSY`, and `yield_now()` places B in Ready while another runnable thread is available.
3. C force-kills B's process. `position_by_pid_except(..., owner_thread())` excludes A, not B, so B is zombified and reaped.
4. B never resumes to drop its prompt/candidate buffers or model/tensor Arcs. One leaked model Arc can retain the whole real model allocation after process teardown; repeated smaller request buffers leak as well.

This is precisely the concurrency case the NOTE says is untested: "no test runs two generates at once" and "no runtime test kills a preempted owner" (`Plans/NOTE-2026-10-20-b1c-product-generation-s1.md:529-531`). The owner-only test cannot disprove the waiter failure because it never creates a contender or calls `terminate`.

**Required fix/gate:** track every thread that has entered the inference syscall lifecycle, not just the one that won `BUSY`, and make force-kill cooperative for those threads. A killed waiter must resume/cancel, drop all locals, and exit before process teardown considers it gone; it must not run the expensive forward after observing teardown. Add a two-caller test that parks A as holder, proves B yielded as a waiter, kills B, reaps, checks model/heap/frame baselines, then proves the shared stack still serves another call. The same gate should kill A while preempted and prove boundary exit and clean release.

### J4 -- MEDIUM -- owner protection ends before the syscall-boundary exit, leaving a true-SMP accounting race

`Held::drop` clears `OWNER_PID`, then `OWNER_TID`, then `BUSY` (`kernel/src/mm/inference_stack.rs:271-276`). Only after the syscall implementation has returned does the outer dispatcher call `after_inference`, which checks `torn_down` and exits (`kernel/src/arch/x86_64/syscall.rs:983-1014,1160-1172`). Thus "the owner record and lock release together" is not the same as "the owner remains protected until it exits."

On the normal syscall path the local CPU has IF restored to 0 during this tail, so a same-CPU killer cannot interleave. A remote CPU can: after the marker is cleared but before `after_inference`, the target is Running, absent from Ready/parked queues, and no longer satisfies `owned_by_pid`. `terminate` then reaches its "neither Ready nor parked" debug assertion (`kernel/src/sched/process.rs:3030-3077`). This is relevant to the requested browser/SMP lens because browser IPC wake explicitly migrates threads to the completer's CPU (`kernel/src/sched/scheduler.rs:1836-1935`), so the scheduler supports user work on more than one CPU even though today's desktop normally starts on the BSP.

There is a second atomicity weakness for any IF=1 caller of the public stack API: the fast-path `BUSY` claim precedes the two owner stores, and release clears three independent atomics. A timer can make such a caller Ready in either partial state. Current ring-3 inference syscalls arrive with IF=0, so this is not the primary product interleaving, but the API neither asserts nor documents that prerequisite.

**Required fix/gate:** keep a separate per-thread inference-lifecycle marker through the `after_inference` decision (distinct from stack ownership), or make terminate account for a Running inference participant without relying on three independently visible atomics. Publish/clear the holder tuple with preemption masked or as one coherent state. The kill-during-owner scenario must run on a different CPU and cover the last-release window, not merely inspect the static skip.

## Shootdown/preemption refutation results

- **Shootdown-ready mask:** the fix leaves the CPU ready and turns IF on, so real TLB IPIs can be accepted. The scenario targets CPU0 from CPU1, counts received IPIs/timeouts, requires BSP readiness before and after, and has a pre-fix IF=0 control. This closes the original J1 failure.
- **Timer/context switch on the foreign stack:** timer entry first copies the iret frame from its IST to the interrupted RSP, then saves GPR/FPU state there; scheduler `context_switch` therefore saves an RSP inside the shared stack for the owner. `BUSY` excludes a second owner until that continuation returns. `xsave-fpu` preserves AVX/YMM state across the switch. I found no TSS.rsp0 or idle-stack alias: ring-3 entry still uses the thread's ordinary kernel stack, and an interrupt taken inside the inference closure is ring-0-origin.
- **Locks/workers:** the heap and frame allocator mask interrupts around their locks; inventory forward does not enter the admission scheduler; `PAR_BUSY` is a non-blocking CAS and a loser runs serially; AP work slots use publish/claim/done atomics and the caller drains each claimed slot. Preempting the BSP holder can delay a job but does not expose a second writer. An inference owner on an AP is not timer-preempted, and if it is also the nominal AP slot it eventually steals that still-pending partition itself; this is a throughput loss, not a correctness deadlock.
- **Discriminator specificity:** the positive scenario proves actual ACK/EOI progress, timer ticks, a co-runner/context switch, IF restoration, readiness, canary, and release. The control requires both the CPU1 timeout breadcrumb and the expected-panic breadcrumb in xtask. `panic_failed` itself ignores the panic payload and accepts any CPU1 panic after the timeout counter rises, but the preceding required timeout line makes this a low-risk test-hardening note, not a separate blocker.

## Verification

- RAG health: **healthy** (`ollama=true`, `qdrant=true`). Its query was used for discovery; final claims above were checked against source at the reviewed head.
- `cargo test -p ai-ops-tests -p module-reclaim-logic-tests`: **PASS** -- 250 AI-op tests, 21 hostile-oracle integration tests, and 26 module-reclaim/stack tests.
- `NEO_SMP=4 cargo xtask neo-test inference_stack_shootdown inference_stack_shootdown_control real_lm_generate_boot inference_stack_overflow`: kernel/userspace and the first scenario compiled, but runtime was **NOT RUNNABLE on this worker**. Xtask exhausted four attempts before QEMU because `/home/neo/limine` is absent (`Set $LIMINE_DIR or build the dev container`). The remaining scenarios were consequently not launched. This is an environment limitation, not evidence for the verdict.

## Merge gate

Do not merge this head. J2/F1 can stay closed. Close J3 with cooperative cancellation/non-killability for **waiters as well as the holder**, and close J4 by carrying the lifecycle marker through the boundary-exit decision. Gate both with a real two-caller, cross-CPU force-kill scenario that proves destructor/resource recovery and stack reuse.
