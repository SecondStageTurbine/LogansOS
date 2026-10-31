# DISPATCH: Phase 2 BUILD subphase 4 - formal close pre-hardware work

- From: PM
- To: Phase 2 worker lane
- Date: 2026-05-25
- Lane: Kernel/test tooling + documentation/procedure
- Status: READY FOR PL ROUTING
- Prior gate: `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-3-close.md`
- PLAN source: `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- Hardware-last constraint: `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` (`b3bb488`)

---

## 1. Mission

Complete every Phase 2 subphase 4 action that can be completed before the OptiPlex hardware session.

The milestone is narrow:

- stand up the non-hardware perf-regression tier with deterministic QEMU-TCG proxy counters
- capture a committed QEMU proxy baseline and wire proxy drift as a blocker
- add or verify exact `F-004`: `smp_concurrent_reap_after_terminate`
- write the OptiPlex 7080 bare-metal procedure and evidence vocabulary
- prepare the Phase 2 close path up to, but not including, hardware execution

This dispatch does not authorize hardware execution. Hardware remains behind the hardware-last barrier until PM verifies that only hardware evidence remains.

## 2. Authoritative Inputs

Read these before editing:

- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-3-close.md` - subphase 3 accepted; preserve its gates.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-2-close.md` - subphase 2 accepted; preserve scheduler-ring gates.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-build-subphase-1-close.md` - subphase 1 accepted; preserve wait/inspect/capability gates.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` - subphase 4 scope, gates, anti-scope.
- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-hardware-last-close-prep.md` - pre-hardware checklist.
- `Plans/PM-NOTE-2026-05-25-phase-2-hardware-last-execution-barrier.md` - binding execution order.
- `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md` - PL ratification of the four-subphase PLAN.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` section 2.2 - downstream BUILD clarifications.
- `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md` - procedure style reference.

Use live source as truth. Use RAG only for semantic discovery; verify all claims against files in the checkout.

## 3. Required Work

### 3.1 Non-Hardware Perf Harness Tier

Implement the QEMU-TCG deterministic proxy tier required by Bible Article 16.4 and the ratified Phase 2 plan.

Required proxy counters for one fixed workload:

- context-switch count
- CR3-write count
- allocation count
- scheduling-decision count

Required behavior:

- The proxy workload must be runnable through the repo's normal test/xtask path.
- The baseline values must be captured in a committed artifact or close report section with the exact command used.
- CI or the existing test runner must fail when committed proxy expectations drift unexpectedly.
- The close report must define the update path for legitimate baseline changes: investigation first, then an intentional baseline update.
- Timing under QEMU-TCG is not an acceptance signal. Use structural counts, not elapsed-time thresholds.

If an exact counter is already present in source, reuse it. If new instrumentation is needed, keep it narrow, deterministic, and test-only where possible.

### 3.2 VMware/Reproducible Middle Tier

Document the VMware middle-tier path or explicitly propose a PM deferral.

Minimum acceptable output:

- what VMware is expected to run
- how it differs from the QEMU-TCG proxy tier and the OptiPlex bare-metal tier
- what artifact will hold VMware baseline evidence later
- whether the current repo has an executable VMware-shaped harness or only a procedure path

Do not claim a VMware scenario sweep exists unless it is actually runnable.

### 3.3 `F-004`: `smp_concurrent_reap_after_terminate`

Add or verify the exact scenario:

- `smp_concurrent_reap_after_terminate`

Expected property: concurrent termination/reap paths under SMP do not lose cleanup, double-reap, leak parked state, corrupt process accounting, or panic.

Run the QEMU-reproducible portion at `NEO_SMP=4`. If part of the property is genuinely hardware-only, state the boundary precisely in the close report and procedure; do not hide a missing QEMU scenario behind the hardware session.

### 3.4 Hardware Procedure Preparation

Write:

- `Plans/PROCEDURE-2026-05-25-phase-2-bare-metal-optiplex-7080-verification.md`

The procedure must specify:

- target assumptions: OptiPlex 7080, Secure Boot posture, UEFI boot path, media expectations
- boot steps and recovery/escalation path
- scenario execution order for `>=4` logical CPUs and full logical CPU count
- serial-log capture path and naming convention
- frame-audit verification command or exact expected marker
- PASS/fail vocabulary, including `KERNEL PANIC`, `BUG:`, frame-leak markers, and per-AP GS-base sentinel evidence
- required bare-metal perf captures: boot time, syscall latency, context-switch latency, scheduling fairness, and ADR-019 IrqSafeMutex micro-bench
- what PM must ingest after the hardware session before Phase 2 can close

The procedure is preparation only. Do not execute it in this dispatch.

### 3.5 Close Report And Barrier Update

File a close report under `Plans/` after the pre-hardware work.

Recommended name:

- `Plans/CLOSE-REPORT-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md`

The close report must state which hardware-last checklist rows are satisfied and which rows remain hardware-only.

### 3.6 Preserve Accepted Subphases

Subphases 1, 2, and 3 are accepted and must stay green:

- wait/inspect scenarios
- exact `INV-CAP-11`
- full `cap_*` regression suite
- `sched_ready_queue_capacity_pressure`
- exact `INV-IPC-5`: `ipc_rendezvous_translate_fault`
- `irqsafe_mutex_if_restore` and `irqsafe_mutex_bench`
- `smp_cross_cpu_rendezvous_stress` sanity gate

Avoid touching scheduler, IPC, capability, filesystem, or block-driver behavior except where a narrow perf counter or `F-004` scenario requires it.

## 4. Required Tests

Run and report:

- `docker compose run --rm neo cargo k`
- the new or existing QEMU-TCG perf proxy command, including exact baseline output
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test smp_concurrent_reap_after_terminate --no-meta`
- `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test sched_ready_queue_capacity_pressure ipc_rendezvous_translate_fault irqsafe_mutex_if_restore irqsafe_mutex_bench smp_cross_cpu_rendezvous_stress --no-meta`
- subphase 1 wait/inspect scenarios at `NEO_SMP=4`
- full `cap_*` regression suite at `NEO_SMP=4`
- full non-ignored `cargo neo-test all --no-meta` at `NEO_SMP=4`, unless PM explicitly accepts a narrower equivalent before close
- serial-log scan for `event=FAIL`, `KERNEL PANIC`, `BUG:`, and frame-leak markers over the current successful logs
- `git diff --check`

If a command must be adjusted to match the repo's current runner, record the exact command and rationale in the close report.

## 5. Acceptance Gates

Minimum gates before close report:

- `cargo k` PASS.
- QEMU-TCG proxy harness is runnable and has committed baseline evidence.
- Proxy drift is wired as a blocker in CI or the existing test runner.
- VMware middle-tier path is documented or explicitly proposed for PM deferral.
- `smp_concurrent_reap_after_terminate` (`F-004`) PASS at `NEO_SMP=4`, or a precise hardware-only remainder is justified.
- Hardware procedure document exists and contains boot steps, scenario order, capture paths, PASS/fail strings, and escalation path.
- Accepted subphase 1-3 gates remain PASS.
- No `KERNEL PANIC`, no `BUG:`, and no frame-leak marker in exercised successful serial logs.
- No OptiPlex or other hardware execution was performed.

## 6. Anti-Scope

Do not do any of the following in this dispatch:

- execute the OptiPlex hardware session
- claim bare-metal evidence from QEMU, VMware, or theory
- reopen ADR-016, ADR-019, THINK v0.2, or the four-subphase PLAN
- change `sys_wait`, `sys_inspect`, `sys_close`, or capability ABI behavior
- broaden scheduler or IPC changes beyond narrow instrumentation and the `F-004` scenario
- add PCID/lazy-TLB/load-balancing performance work
- fold in NVMe FLUSH durability diagnosis
- start Linux or Windows personality work

## 7. Close Report

The close report must include:

- commit id(s)
- files changed
- perf proxy workload and counters
- baseline values and exact command output
- blocker behavior for proxy drift
- VMware middle-tier disposition
- `F-004` implementation/evidence
- hardware procedure path and summary
- pre-hardware barrier checklist status
- accepted subphase anti-regression results
- serial-log scan result
- known blockers or caveats
- exact remaining hardware-only work

## 8. Tell-Worker Text

Phase 2 BUILD subphase 3 is accepted. Please begin subphase 4 pre-hardware work per `Plans/DISPATCH-2026-05-25-phase-2-build-subphase-4-formal-close-pre-hardware.md`. Scope is all formal-close work that can be completed before the OptiPlex session: QEMU-TCG proxy perf harness with structural counters and baseline drift gate, exact `F-004` scenario `smp_concurrent_reap_after_terminate`, VMware middle-tier documentation or PM deferral proposal, OptiPlex 7080 hardware procedure preparation, and a close report that separates complete pre-hardware rows from hardware-only remainder. Do not execute hardware. Preserve accepted subphase 1-3 gates.
