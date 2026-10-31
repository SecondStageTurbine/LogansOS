# PM-DISPOSITION: Phase 2 PLAN substance close

- Author: PM
- Date: 2026-05-25
- Status: Prometheus PLAN substance ACCEPTED. **Phase 2 PLAN is ready for PL ratification.**
- Inputs consumed:
  - `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`) - four-subphase BUILD enumeration
  - `Plans/DISPATCH-2026-05-25-phase-2-plan-substance-prometheus.md` (`9129417`) - active dispatch
  - `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` (`ae3787f`, amended by route correction at `9129417`) - ratified THINK v0.2 substrate

---

## 1. Acceptance-Criteria Verification

Per `Plans/DISPATCH-2026-05-25-phase-2-plan-substance-prometheus.md` section 4:

| Criterion | Status | Evidence |
|---|---|---|
| **C1** All seven THINK v0.2 dispositions reflected without reopening choices | PASS | Q1-Q7 all map into the PLAN overview and subphases; deferrals (`sys_close`, full FM-D14, AMD stretch, etc.) stay deferred |
| **C2** Concrete BUILD subphases or justified subdivisions with milestones, dependencies, anti-scope, gates, artifacts, lanes | PASS | Four operational subphases, each with goal milestone, in-scope items, dependencies, anti-scope, acceptance gates, expected close artifacts, and execution lane |
| **C3** Four audit (b) items mapped | PASS | `INV-CAP-11` -> subphase 1; `INV-SYNC-5` -> subphase 2; `INV-IPC-5` -> subphase 3; `F-004` -> subphase 4 |
| **C4** Q4/Q6 shared hardware/perf session specified | PASS | OptiPlex session captures SMP verification, absolute-latency baseline, and ADR-019 micro-bench; QEMU-TCG gates per-commit; VMware is middle tier |
| **C5** /simplify pass explicit and preserves deferrals/anti-scope | PASS | Twelve cut/deferred items named, including `sys_close`, full FM-D14, AMD stretch, PID recycling, Spectre-v2/IBPB, PCID/ASID, RCU, load balancing, x2APIC, hot-plug, Arc-aware slab, NVMe FLUSH durability |
| **C6** No code changes, no ADR drafting, no bible amendments, no BUILD dispatches | PASS | Commit `e6a4cdf` adds one documentation artifact only |
| **C7** Deliverable filed and pushed | PASS | `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` at `e6a4cdf` on `origin/main` |

All seven criteria are met. No dispatch deviation requires a corrective worker pass.

## 2. PM Rulings

### 2.1 Four-subphase split accepted

Prometheus split the ratified two-subphase THINK synthesis into four operational BUILD subphases:

1. Capability reserved-bit activation
2. ADR-016 A2 scheduler heap-decoupling
3. ADR-019 B/C type-level IRQ safety
4. Formal close: perf harness, bare-metal verification, remaining audit (b)

PM accepts the split. This is a PLAN-level operational subdivision, not a change to the ratified THINK v0.2 substance. It preserves the two synthesis buckets while giving each BUILD dispatch one dominant risk surface and one clean gate.

The sequencing is accepted: 1 -> 2 -> 3 -> 4. Subphase 2 before 3 is ratified by THINK v0.2 Q7(d). Subphase 4 gates formal closure because it requires PL hardware evidence.

### 2.2 Clarifications for downstream BUILD dispatches

Two notes should carry into later dispatches:

- **INV-CAP-11 naming:** Subphase 1 can absorb `INV-CAP-11` through the `cap_*` suite, but the BUILD dispatch should name the exact scenario or add one if needed. Do not let `INV-CAP-11` disappear behind a broad suite label.
- **ADR amendments as close documentation:** The "ADR-016 amendment" and "ADR-019 amendment" items in expected close artifacts are documentation closeouts after implementation, not new design work in this PLAN substance pass and not authorization to reopen ratified choices.

## 3. Ratification-Ready PLAN Summary

PM recommends PL ratify the Phase 2 PLAN as follows:

| BUILD subphase | Goal | Primary gate |
|---|---|---|
| **1. Capability reserved-bit activation** | Implement `sys_wait` + `sys_inspect`; keep `sys_close` deferred; absorb `INV-CAP-11` | New wait/inspect scenarios + `cap_*` regression gate at NEO_SMP=4 |
| **2. ADR-016 A2 scheduler heap-decoupling** | Replace ready/zombie `VecDeque<Box<Thread>>` with fixed-capacity per-CPU rings; graceful `-EAGAIN` at capacity | `sched_ready_queue_capacity_pressure` + full SMP regression |
| **3. ADR-019 B/C type-level IRQ safety** | Migrate mixed-context `SwitchMutex` surfaces and `Channel.state` to `IrqSafeMutex`; remove manual wrappers | B: full-suite + micro-bench; C: SC-3B 200/200 `NEO_SMP=4` |
| **4. Formal close** | Stand up tiered perf harness; run OptiPlex bare-metal multi-CPU verification; absorb `F-004` | QEMU-TCG proxy CI gate + OptiPlex evidence + `F-004` PASS |

Subphases 1-3 can proceed in QEMU/CI while the PL hardware session is scheduled. Subphase 4 cannot close without the OptiPlex evidence because Bible Article 13's bare-metal multi-CPU mandate is the formal close-gate.

## 4. Next Steps

1. **PL ratifies the Phase 2 PLAN** - recommended wording: "ratify the four-subphase PLAN as stated, with PM clarifications in section 2.2."
2. **PM dispatches BUILD subphase 1** to Jarvis/Codex: capability reserved-bit activation (`sys_wait`, `sys_inspect`, `INV-CAP-11`).
3. In parallel with subphase 1/2/3 execution, **PM prepares subphase 4 procedure scaffolding** for the PL hardware session when the serial-console / OptiPlex path is available.
4. NVMe FLUSH durability diagnosis remains governed by `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md` and should not be folded into Phase 2 PLAN.

## 5. References

- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`) - accepted PLAN substance
- `Plans/DISPATCH-2026-05-25-phase-2-plan-substance-prometheus.md` (`9129417`) - active dispatch
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` (`ae3787f`, amended `9129417`) - ratified THINK v0.2
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) - ratified answered dispositions
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-polish-close.md` (`4469c6b`) - THINK v0.2 polish acceptance
- `Plans/PM-DISPOSITION-2026-05-24-f005-close.md` (`5884ffd`) - audit close unblocker
