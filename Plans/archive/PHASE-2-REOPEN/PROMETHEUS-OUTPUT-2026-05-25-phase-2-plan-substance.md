# PROMETHEUS-OUTPUT: Phase 2 PLAN substance — BUILD-subphase enumeration

- **Author:** Prometheus (Qwen3.6-27B — documenting, amending, document cleanup lane)
- **Date:** 2026-05-25
- **Scope binding:** `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` (all seven THINK v0.2 dispositions + both PL-operational items ratified in full)
- **Repo state:** `origin/main` at `9129417`
- **Inputs consumed:**
  - `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) — ratified answered dispositions (normative)
  - `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` — PL ratification of all seven answers + Q4/Q6 operational items
  - `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (`8bdb4be`) — PM rulings on Q5 re-routing, Q7 ring
  - `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — full reasoning behind ratified dispositions
  - `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — binding scope-class calls
  - `Plans/PM-DISPOSITION-2026-05-24-f005-close.md` (`5884ffd`) — F-005 audit close
  - ADRs 007, 010, 013, 016, 019; Bible Articles 1.2, 4.2, 8.5.3, 12.2, 13, 14, 16.4
  - Source: `kernel/src/sched/{scheduler,process,thread}.rs`, `kernel/src/sync/{switch_mutex,irqsafe_mutex}.rs`, `kernel/src/ipc/mod.rs`, `kernel/src/arch/x86_64/syscall.rs`
  - RAG (`:8404`) — used for semantic discovery; all claims verified against live files

---

## PLAN Overview

Phase 2 closes with a formal-completion BUILD set (close-first per scope-class ratification §1.1). The objective: close the open Bible Article 13 bare-metal multi-CPU mandate, activate ADR-010's three reserved-bit syscalls (`sys_wait`, `sys_inspect`; `sys_close` deferred), stand up Bible Article 16.4's perf-regression harness, absorb the four audit (b) disposition tests, and ship two defense-in-depth items (ADR-016 A2 scheduler heap-decoupling, ADR-019 B/C type-level IRQ safety). Optimization items (Arc-aware slab, PCID/lazy-TLB) defer to follow-on or Phase 3 substrate.

The ratified two-subphase shape splits into **four operational subphases** for BUILD dispatch safety. The split is justified against Bible Article 14 /simplify: each subphase has a single dominant risk surface, an independent gate, and a clear anti-scope boundary. The four subphases are:

1. **Subphase 1 — Capability reserved-bit activation** (`sys_wait` + `sys_inspect`)
2. **Subphase 2 — ADR-016 A2: scheduler heap-decoupling** (fixed-capacity per-CPU ring)
3. **Subphase 3 — ADR-019 B/C: type-level IRQ safety** (IrqSafeMutex migration)
4. **Subphase 4 — Formal close** (perf harness, bare-metal verification, remaining audit (b))

Sequencing: 1 → 2 → 3 → 4. Subphases 2 and 3 are sequential by ratified ordering (A2 before ADR-019 B/C per THINK v0.2 Q7(d)). Subphase 1 is independent of 2/3 but precedes them for logical grouping (cap activation before defense-in-depth). Subphase 4 gates Phase 2 closure and requires a PL hardware session.

---

## Subphase 1 — Capability Reserved-Bit Activation

**Goal milestone:** `sys_wait(handle)` and `sys_inspect(handle)` are callable syscalls; WAIT and INSPECT rights bits are enforced on ProcessHandle operations; the full `cap_*` test suite plus new `wait_*` and `inspect_*` scenarios PASS at 200/200 NEO_SMP=4. `sys_close` is explicitly deferred; CLOSE bit remains reserved.

**In-scope items:**

- **`sys_wait(handle)` implementation.** New syscall number (19). Two code paths:
  - Non-blocking (`WNOHANG` flag): load target `exit_state`; if Exited/Zombie, call `take_completed_exit_code(pid)` and return the cached code. If Running, return a "still running" indicator.
  - Blocking: park the caller's Thread on a per-Process wait queue; `on_thread_reap` wakes one parked waiter after recording the exit code. Multiple parked waiters beyond the first wake with `-ESRCH`.
  - Rights gate: `semantic & WAIT != 0` on the ProcessHandle, returning `-EPERM` otherwise.
- **`sys_inspect(handle)` implementation.** New syscall number (20). Returns a fixed value-type snapshot struct by value: `exit_state` enum (Running/Exited/Zombie), cached `exit_code` if Exited, `thread_count` (always 1 under D13), open-channel-handle count, and process `name`. No Handle, CapId, pointer, or cap-inventory identity fields. Rights gate: `semantic & INSPECT != 0`.
- **libneo wrappers.** Add `sys_wait`, `sys_inspect` syscall numbers and wrapper functions. Add WAIT and INSPECT right constants.
- **Test scenarios:** `sys_wait_blocking` (parent spawns child, waits for exit code), `sys_wait_wnohang` (poll on running process returns still-running indicator), `sys_wait_already_exited` (wait after exit returns cached code, frees COMPLETED_EXIT_CODES slot), `sys_wait_multiple_waiters` (two ProcessHandles to same target: first gets code, second gets -ESRCH), `sys_inspect_running` (inspect running child returns Running state), `sys_inspect_exited` (inspect exited child returns Exited + exit code), `sys_inspect_rights_gate` (ProcessHandle without INSPECT returns -EPERM), absorb audit (b) `INV-CAP-11` (`cap_revoke_spawn_inheritance_descendants`).

**Dependencies and ordering:** No dependency on subphases 2–4. The data surface already exists in source: `exit_state`, `exit_code`, `record_exit_code()`, `take_completed_exit_code(pid)` in `process.rs`. The blocking case is the only new mechanism — a per-Process waiter parking site with wake from `on_thread_reap`.

**Anti-scope:** `sys_close` (deferred, no use case). Full FM-D14 structural retirement via handle-presence-gated slot lifetime (deferred to future ADR triggered by measurement). PID recycling (deferred per scope-class ratification §1.3).

**Acceptance criteria and verification gates:**
- `cargo k` PASS.
- All new `wait_*` and `inspect_*` scenarios PASS.
- Full `cap_*` suite PASS at NEO_SMP=4 (anti-regression).
- Frame leak audit clean.
- No `KERNEL PANIC`, no `BUG:`.

**Expected close artifacts:** Commit with syscall implementations, libneo wrappers, and test scenarios. Verification log.

**Recommended execution lane:** Jarvis/Codex (kernel implementation + tests). Prometheus for any ADR updates if the implementation reveals a deviation from THINK v0.2 dispositions (unlikely; the surface is narrow).

---

## Subphase 2 — ADR-016 A2: Scheduler Heap-Decoupling

**Goal milestone:** `PerCpuSchedulerInner.ready` and `.zombies` are fixed-capacity per-CPU rings of `Box<Thread>` (default 64 slots/CPU, configurable at scheduler init). Spawn returns `-EAGAIN` at capacity. No `alloc`/`dealloc` reachable while holding `PerCpuScheduler.inner`. The `INV-SYNC-5` (`sched_ready_queue_capacity_pressure`) test drives threads to the per-CPU bound and asserts graceful `-EAGAIN` with no realloc/alloc-under-lock/panic. Full suite PASS at 200/200 NEO_SMP=4.

**In-scope items:**

- **Ring structure.** New `ThreadRing` type: `[Option<Box<Thread>>; N]` with head/tail indices, O(1) push/pop at both ends. Two rings per CPU (ready, zombies). Pre-allocated at `PerCpuScheduler` init time — no heap allocation during scheduler operations.
- **Replace `VecDeque<Box<Thread>>` in `PerCpuSchedulerInner`.** Replace `ready: VecDeque<Box<Thread>>` and `zombies: VecDeque<Box<Thread>>` with `ready: ThreadRing<READY_CAP>` and `zombies: ThreadRing<ZOMBIE_CAP>`. The Box-pointer-stability invariant is preserved: the ring stores owned `Box` values, so moving a slot moves only the 8-byte pointer; the Thread heap allocation and its captured `rsp` never move.
- **Fallible spawn.** `push_ready()` checks capacity before insert; returns error when full. `spawn_user()` propagates `-EAGAIN`. Default cap: 64/CPU, configurable via a `PerCpuScheduler` field at init.
- **Arbitrary removal for `kill_thread_by_pid`.** O(N≤64) linear scan under briefly-held scheduler lock. Acceptable; D13's single-thread-per-process invariant means the ready queue is small in practice.
- **`INV-SYNC-5` test.** `sched_ready_queue_capacity_pressure` scenario: spawn threads to fill the per-CPU ring, assert graceful `-EAGAIN`, verify no alloc-under-lock (debug build checks + release build verification).

**Dependencies and ordering:** Depends on subphase 1 only for anti-regression (cap activation should not interfere with scheduler structure changes). Independent of subphase 3. Must precede subphase 3 per ratified Q7(d): A2 settles the structure before B/C migrates the lock type.

**Anti-scope:** Intrusive linked list (ruled out by THINK v0.2 Q7 — Bible 4.2 `unsafe` budget). Pre-reserved `VecDeque` with spawn-time bound check (ruled out — disciplinary, not structural). Arc-aware slab integration (deferred per ADR-012 BUILD-2 post-close summary).

**Acceptance criteria and verification gates:**
- `cargo k` PASS.
- All existing SMP scenarios PASS at NEO_SMP=4 (anti-regression).
- `sched_ready_queue_capacity_pressure` (`INV-SYNC-5`) PASS — drives to capacity, asserts `-EAGAIN`, no alloc-under-lock.
- Frame leak audit clean.
- No `KERNEL PANIC`, no `BUG:`.

**Expected close artifacts:** Commit with ring implementation, scheduler refactor, and `INV-SYNC-5` test. Verification log. ADR-016 amendment recording A2 implementation choices.

**Recommended execution lane:** Jarvis/Codex (scheduler implementation). This is the structurally dense subphase — one dominant risk surface (the ring replacing VecDeque while preserving Box-pointer-stability and all existing scheduler ops).

---

## Subphase 3 — ADR-019 B/C: Type-Level IRQ Safety

**Goal milestone:** All mixed-context `SwitchMutex` callers migrated to `IrqSafeMutex` (sub-phase B). All `Channel.state` `without_interrupts(...)` sites migrated to `IrqSafeMutex` and manual wrappers removed (sub-phase C). The SC-3B 200/200 NEO_SMP=4 gate PASS on sub-phase C.

**In-scope items:**

- **Sub-phase B — Migrate mixed-context `SwitchMutex` callers to `IrqSafeMutex`.** Audit all `SwitchMutex` instances in production code (not test harness). Classify each as:
  - Mixed-IRQ-context (both process-context and ISR/IF=0 acquirers) → migrate to `IrqSafeMutex`
  - Exclusively process-context → keep as `SwitchMutex` with SAFETY-comment justification at declaration site
  Production `SwitchMutex` sites to audit: `Process.handles`, `Process.derivations`, `PerCpuScheduler.inner`, `Channel.state`, NeoFs inode fields (`metadata`, `root`, `entries`, `writeback_worker`), NeoFs cache `inner`, virtio_blk device state, NVMe controller inner, kthread registry and control state, block device registries, fs module fields. Each site classified; mixed-context sites migrated.
- **Sub-phase C — Migrate `Channel.state` and remove manual wrappers.** `Channel.state: SwitchMutex<[EndpointState; 2]>` → `IrqSafeMutex<[EndpointState; 2]>`. Remove the five `without_interrupts(...)` wrappers SC-3B sub-step 5c added (the discipline becomes structural via the type). This is the highest-risk migration — the exact lock surface SC-3B's Hpw mechanism lived on.
- **`INV-IPC-5` test.** `ipc_rendezvous_translate_fault` scenario runs as part of the full suite gate.

**Dependencies and ordering:** Depends on subphase 2 (A2 settles the scheduler queue structure first). Sub-phase B before C per ratified Q5(c): B establishes `IrqSafeMutex` usage broadly, C migrates the highest-risk `Channel.state` sites onto the now-established type. B and C touch disjoint locks (`SwitchMutex` instances vs `Channel.state`), so there's no double-migration risk.

**Anti-scope:** Single combined "lock-discipline" subphase (ruled out by THINK v0.2 Q5 — bundles catastrophic-risk Channel.state migration with lower-risk work). Any change to `SwitchMutex` type itself beyond the migration audit.

**Acceptance criteria and verification gates:**
- **Sub-phase B gate:** `cargo k` PASS, full test suite green at NEO_SMP=4, ADR-019 IrqSafeMutex acquire/release micro-bench captured (the existing `IRQSAFE_BENCH_MUTEX` fixture in `test_harness.rs` provides the bench).
- **Sub-phase C gate:** SC-3B 200/200 NEO_SMP=4 `smp_cross_cpu_rendezvous_stress` PASS (the established bar for the Channel.state surface). Full suite green. Frame leak audit clean. No `KERNEL PANIC`, no `BUG:`.

**Expected close artifacts:** Commit(s) with migration and wrapper removal. Verification log. ADR-019 amendment recording sub-phase B/C completion.

**Recommended execution lane:** Jarvis/Codex (systematic migration). The audit is mechanical but context-sensitive — each site must be classified for IRQ-safety before the type change. Prometheus for ADR-019 amendment.

---

## Subphase 4 — Formal Close: Perf Harness, Bare-Metal Verification, Remaining Audit (b)

**Goal milestone:** Bible Article 16.4 perf-regression harness standing up with tiered environments (QEMU-TCG proxies gating per-commit, bare-metal baseline captured). Bible Article 13 bare-metal multi-CPU mandate satisfied on OptiPlex 7080. Remaining audit (b) `F-004` (`smp_concurrent_reap_after_terminate`) absorbed. Phase 2 formally closed.

**In-scope items:**

- **Perf-regression harness (Q4).** Three tiers:
  - **Tier 1 (QEMU TCG):** Deterministic structural proxies run per-commit in CI. Counters: context-switch count, CR3-write count, allocation count, scheduling-decision count for a fixed workload. Any counter change = CI BLOCKER.
  - **Tier 2 (Bare-metal OptiPlex 7080):** Absolute-latency characterization captured during PL hardware session. Metrics: boot time, syscall latency ns, context-switch ns, scheduling fairness (Redox `schedrs`-adapted). n-trial median + IQR methodology. ADR-019 50ns IrqSafeMutex micro-bench folded in as one bare-metal benchmark.
  - **Tier 3 (VMware-VM):** Reproducible middle tier for relative trends when bare-metal sessions aren't available.
  - Regression policy: three tiers — proxy change = blocker, absolute regression beyond noise margin = warning + investigation, sustained/large regression = requires ADR.
- **Bare-metal multi-CPU verification (Q6).** OptiPlex 7080 session with ≥4 logical CPUs plus the target's full count. Must-pass scenario set: `smp_cross_cpu_rendezvous_stress` (200/200), `q7c_*` (TLB shootdown), `q8a/b/c_*` (cap-table-SMP + blocked-kill walk), `cap_*` suite under SMP, `ipc_heap_kill_smoke`. Evidence shape: full serial-log capture, frame-audit-clean post-run, zero `KERNEL PANIC` / `BUG:`, per-AP GS-base sentinel PASS. Procedure document modeled on `PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md`.
- **Audit (b) `F-004`.** `smp_concurrent_reap_after_terminate` scenario absorbed into the formal-close BUILD.

**Dependencies and ordering:** Depends on all of subphases 1–3 (the harness measures the completed close set; bare-metal verification validates the full stack). Q4 and Q6 share one PL hardware session for absolute-latency capture + SMP verification.

**Anti-scope:** AMD cross-vendor run (non-blocking stretch, not mandated). Perf harness code changes beyond standing up the tiered proxy/baseline infrastructure.

**Acceptance criteria and verification gates:**
- Tier 1 proxies integrated into CI and gating on every push.
- Bare-metal session PASS on OptiPlex 7080 with full evidence capture.
- `F-004` scenario PASS.
- All existing scenarios PASS at NEO_SMP=4 (anti-regression).

**Expected close artifacts:** Perf harness commit(s). Bare-metal procedure document. Bare-metal verification evidence log. Phase 2 close disposition artifact.

**Recommended execution lane:** Prometheus for perf harness scaffolding and procedure document authoring. PL executes the bare-metal hardware session per the written procedure. Jarvis/Codex for `F-004` test scenario if not already in tree.

---

## Audit (b) Mapping Table

| Audit (b) item | Scenario name | Subphase | Acceptance signal |
|---|---|---|---|
| `INV-CAP-11` (`cap_revoke_spawn_inheritance_descendants`) | Absorbed into `cap_*` suite | Subphase 1 | PASS at NEO_SMP=4 as part of cap reserved-bit activation gate |
| `INV-SYNC-5` (`sched_ready_queue_capacity_pressure`) | `sched_ready_queue_capacity_pressure` | Subphase 2 | Drive threads to per-CPU ring bound; assert graceful `-EAGAIN`, no realloc/alloc-under-lock/panic |
| `INV-IPC-5` (`ipc_rendezvous_translate_fault`) | Absorbed into full suite gate | Subphase 3 (C) | PASS as part of SC-3B 200/200 NEO_SMP=4 stress gate on Channel.state migration |
| `F-004` (`smp_concurrent_reap_after_terminate`) | `smp_concurrent_reap_after_terminate` | Subphase 4 | PASS at NEO_SMP=4 as part of formal-close BUILD |

---

## Hardware/Perf Session Plan

**Session target:** OptiPlex 7080 (Intel, ≥4 logical CPUs). PL executes a written procedure document. PM authors the procedure pre-BUILD; PL executes in-BUILD; PM consumes evidence. Modeled on `PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md`.

**What the OptiPlex session must capture:**

1. **Bare-metal multi-CPU verification (Q6):** SMP-specific scenario set at ≥4 cores and full logical-CPU count. `smp_cross_cpu_rendezvous_stress` 200/200, `q7c_*`, `q8a/b/c_*`, `cap_*` suite under SMP, `ipc_heap_kill_smoke`. Full serial-log capture, frame-audit-clean post-run, per-AP GS-base sentinel PASS.
2. **Absolute-latency baseline (Q4):** Boot time, syscall latency ns (representative set: `sys_spawn`, `sys_send`, `sys_recv`, `sys_wait`, `sys_inspect`), context-switch ns, scheduling fairness (Redox `schedrs`-adapted). n-trial median + IQR.
3. **ADR-019 50ns micro-bench (Q4(d)):** IrqSafeMutex lock/unlock-cycle cost on real hardware. Uses the existing `IRQSAFE_BENCH_MUTEX` fixture in `test_harness.rs`.

**Where VMware fits:** Reproducible middle tier for relative trend tracking between bare-metal sessions. Runs the same scenario set and perf counters as bare-metal, providing a stable comparison baseline when PL hardware sessions aren't available.

**Where QEMU-TCG proxies gate:** Per-commit CI. Deterministic structural counters (context-switch count, CR3-write count, allocation count, scheduling-decision count) for a fixed workload. Any counter change = CI BLOCKER. No absolute timing under TCG.

**Procedure document requirements:** The procedure must specify boot steps, scenario execution order, evidence-capture instructions (serial-log capture path, frame-audit verification command), and the PASS vocabulary (zero PANIC/BUG, per-AP sentinels, per-scenario PASS markers). PM authors this as a pre-BUILD deliverable for subphase 4; Prometheus is the recommended lane for procedure drafting.

---

## /simplify Pass

**Explicitly cut or deferred from Phase 2 PLAN:**

| Item | Status | Rationale |
|---|---|---|
| `sys_close` | Deferred (Q1) | No Phase-2 use case. CLOSE bit reserved at zero ABI cost. Revisit trigger: Phase 4 network sockets. |
| Full FM-D14 structural retirement | Deferred (Q2(e)) | Phase 2 ships practical bound (first-consumer-wins `sys_wait`). Handle-presence-gated slot lifetime is a future ADR triggered by `COMPLETED_EXIT_CODES` residual-growth measurement. |
| AMD cross-vendor bare-metal run | Non-blocking stretch (Q6(d)) | OptiPlex 7080 satisfies Bible Article 13. Cross-vendor would validate vendor-specific APIC/INVLPG behavior but is not mandated. |
| PID recycling | Deferred (scope-class §1.3) | `NEXT_PID: AtomicU32` remains monotonic. Revisit trigger: pid exhaustion becomes a real workload concern. |
| Spectre-v2 / IBPB on context switch | Deferred (scope-class §1.5) | Dedicated ADR at future trigger. Not Phase-2-close scope. |
| PCID / ASID enablement | Deferred (scope-class §1.2) | Correctness baseline satisfies Bible Article 13's "advanced TLB management." Forward-marked for Phase 2+/Phase 3+. |
| RCU primitives | Deferred | `RwLock` is disciplinary, not structural. RCU is a real dependency (Bible 4.3); deferred until Phase 5+ scale justifies it. |
| Load balancing / work stealing | Deferred (ADR-013 anti-scope) | BUILD-3's correctness goal was "multi-CPU works"; performance optimization comes later. |
| x2APIC support | Deferred (ADR-013 anti-scope) | BUILD-3 uses xAPIC mode. x2APIC deferred. |
| Hot-plug CPUs | Deferred (ADR-013 anti-scope) | CPU count determined at boot. |
| Arc-aware slab integration | Deferred (ADR-012 BUILD-2 post-close) | Three known resolution paths exist; none selected. CHANNEL_CACHE and PROCESS_CACHE declared but unused. |
| NVMe FLUSH durability diagnosis | Paused | Governed by `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md`. Resumes when serial-console adapter is installed. Not Phase 2 close scope. |

---

## Risks and Stop Conditions

1. **Subphase 2 ring implementation regressions.** Replacing `VecDeque` with a fixed-capacity ring changes the container for every scheduler operation (spawn, yield, wake, kill, exit, reap). The Box-pointer-stability invariant must be preserved — the ring stores owned `Box` values, so this is structurally sound, but the migration touches every enqueue/dequeue site. **Mitigation:** anti-regression gate is the full SMP suite at 200/200 NEO_SMP=4. Stop condition: if any existing SMP scenario fails after the ring swap, halt and diagnose before proceeding to subphase 3.

2. **Subphase 3 C (Channel.state) migration regression.** Migrating `Channel.state` from `SwitchMutex` to `IrqSafeMutex` touches the exact lock surface SC-3B's Hpw mechanism lived on. A regression here is catastrophic (silent IPC corruption under SMP). **Mitigation:** SC-3B 200/200 NEO_SMP=4 stress gate is the established bar. Stop condition: if `smp_cross_cpu_rendezvous_stress` fails, halt and diagnose before proceeding to subphase 4.

3. **PL hardware session availability gates Phase 2 closure.** Subphase 4 cannot close without the bare-metal OptiPlex session (Bible Article 13 mandate). **Mitigation:** subphases 1–3 can complete and be verified in QEMU while the PL session is scheduled. The hardware session is a verification gate, not an implementation dependency. If the session is delayed beyond a reasonable window, PM should flag to PL that Phase 2 closure is blocked on evidence capture.

4. **`sys_wait` blocking mechanism interacts with existing rendezvous parking.** The per-Process waiter parking site for `sys_wait` is new infrastructure. It must compose correctly with the existing IPC rendezvous parking (parked Thread in EndpointState) and the reaper wake path. **Mitigation:** the park/wake wiring is small and well-defined (the parked-Thread mechanism is the rendezvous primitive). Stop condition: if `sys_wait` blocking produces a hang or missed wakeup under SMP, halt and diagnose before proceeding.

5. **Perf harness TCG proxy counters may drift on unrelated changes.** Deterministic counters under TCG are reliable for algorithmic regressions, but a change to test-harness infrastructure or QEMU version could shift baseline counts without an actual algorithmic regression. **Mitigation:** the three-tier policy handles this — proxy change = blocker with investigation path; if the investigation determines the counter shift is environmental, the baseline is updated with documentation.

---

## References

- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) — ratified answered dispositions (normative)
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` — PL ratification of all seven answers + Q4/Q6 operational items
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (`8bdb4be`) — PM rulings on Q5 re-routing, Q7 ring
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — full reasoning behind ratified dispositions
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — binding scope-class calls
- `Plans/PM-DISPOSITION-2026-05-24-f005-close.md` (`5884ffd`) — F-005 audit close
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` — A2 architectural goal and constraints
- `docs/ADR-019-switchmutex-irq-off-by-construction.md` — IrqSafeMutex type and sub-phase A/B/C structure
- `docs/ADR-013-phase-2-smp-retrofit.md` — per-CPU scheduler topology, FM-D5/D8/D13/D14
- `docs/ADR-010` (capability foundation) — reserved bits, Principle 2
- `docs/ADR-007` (process termination) — `sys_wait`, exit_state CAS, Implementation note 6
- `kernel/src/sched/process.rs` — `exit_state`, `exit_code`, `COMPLETED_EXIT_CODES`, `take_completed_exit_code`, `on_thread_reap`
- `kernel/src/sched/scheduler.rs` — `PerCpuScheduler`, `VecDeque<Box<Thread>>` ready/zombie queues, `READY_INITIAL_CAPACITY=8`
- `kernel/src/sync/irqsafe_mutex.rs` — `IrqSafeMutex` type (sub-phase A landed at `27d1b99`)
- `kernel/src/sync/switch_mutex.rs` — `SwitchMutex` type (migration target for sub-phase B)
- `kernel/src/ipc/mod.rs` — `Channel.state: SwitchMutex<[EndpointState; 2]>` (sub-phase C migration target, 11 acquisition sites)
- `kernel/src/arch/x86_64/syscall.rs` — syscall numbers (current max: SYS_UMOUNT=18)
- `kernel/src/test_harness.rs` — `IRQSAFE_BENCH_MUTEX` fixture (line 1539), existing SMP test infrastructure
- `Plans/PROCEDURE-2026-05-20-phase-2-and-3-vmware-hypervisor-vm-verification.md` — procedure document template for bare-metal session
