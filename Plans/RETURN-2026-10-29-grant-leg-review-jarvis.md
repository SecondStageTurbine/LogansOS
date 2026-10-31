<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CAP_GRAPH two-CPU grant leg, relay #146 / #193

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/grant-leg-review-jarvis`. **PASS.**
Reviewed `git show fec09323 --stat` and the full squash diff, plus the
grant/install/placement callees on main. References below use `fec09323`.

1. **PASS.** `kernel/src/sched/process.rs:2637` holds CAP_GRAPH from source
   lookup through `install_minted_locked(..., false)`. The new hook (:2663)
   stops exactly between them. `cap_graph_two_cpu_grant_leg`
   (`kernel/src/test_harness.rs:40340`) starts the grant on CPU1, then revoke
   on CPU2, samples revoke-not-done before release, and finally requires zero
   recipient handles. With drop(g)/barrier/re-lock, CPU2 can revoke the source
   while CPU1 retains its cloned Handle. The install's false parent-in-process
   flag accepts that stale parent, leaving a recipient handle; the first
   decisive result is `granted_child_survived_the_revoke`. The held witness
   independently refuses a barrier outside CAP_GRAPH, even if CPU2 is delayed.
   #193 records that exact falsifier result; this review did not rerun QEMU.
2. **PASS for this fixture's call graph.** All Pmap creation/registration and
   source installation happen before the barrier. While CPU1 holds it, the
   BSP reads atomics, constructs/places the CPU2 revoker, and waits two ticks.
   `scheduler::place_thread_on_cpu` takes only the target scheduler lock and
   sends its reschedule IPI; this placement targets CPU2, not CPU1. It does
   not take CAP_GRAPH or send a TLB shootdown. The yield/reap completion loop
   and Process/Pmap drops follow `release_grant_barrier`. Error exits after
   placement failure also release the barrier before fixture locals drop.
   The timer ISR does not run the reaper itself. No wait-for-CPU1/graph/shootdown
   cycle is introduced in the orchestrator. The barrier is a test-only
   exception to the production no-cross-CPU-wait-under-CAP_GRAPH rule.
3. **PASS — cfgs plus inspected author section evidence.**
   The enclosing `cap_graph_race` module (`process.rs:2402`), the call at
   :2662, the harness invocation and every new harness item are gated by
   `neo-test-cap-graph-revoke`. New comments/docs are the only ungated changes.
   #193 records identical `.text` and `.rodata` for default/desktop/usb-host,
   with three changed data bytes attributed to panic-location lines. No
   binary-pair rebuild or whole-ELF equality is claimed here.
4. **PASS.** `install_minted_locked` (`process.rs:2142`) checks a parent in
   the recipient only when `parent_in_proc` is true; `complete_grant` passes
   false because the source is in the grantor. The source lookup under the
   same graph guard supplies its liveness check. CP-87's corrected paragraph
   (`docs/concurrency-pitfalls.md:4009`) now makes exactly that distinction
   and explains why splitting the grant section reopens the window.

Independently queried
[batch 15](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40831274927):
completed/success at `5a37066afadd4f597b7ca212121846c2124a6238`.
RAG healthy; direct source review, no new kernel build or QEMU execution.
Timing bounds are test accommodations, not a general scheduler latency proof.
