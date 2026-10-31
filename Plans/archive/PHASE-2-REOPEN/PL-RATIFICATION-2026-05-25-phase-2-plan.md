# PL-RATIFICATION: Phase 2 PLAN

- Recorded by: PM
- Ratification received: 2026-05-25
- Ratifying authority: Project Lead
- Ratification form: "ratify the four-subphase PLAN as stated, with PM clarifications in section 2.2."
- Source artifact: `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`)
- PM acceptance artifact: `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` (`b7281d0`)
- Status: RATIFIED in full

---

## 1. Disposition

PL ratified the four-subphase Phase 2 PLAN exactly as accepted by PM. The four-subphase split is accepted as an operational BUILD subdivision of the ratified THINK v0.2 synthesis, not a reopening of THINK substance.

The accepted sequencing is:

1. Capability reserved-bit activation
2. ADR-016 A2 scheduler heap-decoupling
3. ADR-019 B/C type-level IRQ safety
4. Formal close: perf harness, bare-metal verification, remaining audit (b)

Subphase 2 before subphase 3 remains binding per THINK v0.2 Q7(d). Subphase 4 remains the formal Phase 2 closure gate because it requires PL hardware evidence.

## 2. Ratified BUILD Subphases

| BUILD subphase | Goal | Primary gate |
|---|---|---|
| **1. Capability reserved-bit activation** | Implement `sys_wait` + `sys_inspect`; keep `sys_close` deferred; absorb `INV-CAP-11` | New wait/inspect scenarios + `cap_*` regression gate at NEO_SMP=4 |
| **2. ADR-016 A2 scheduler heap-decoupling** | Replace ready/zombie `VecDeque<Box<Thread>>` with fixed-capacity per-CPU rings; graceful `-EAGAIN` at capacity | `sched_ready_queue_capacity_pressure` + full SMP regression |
| **3. ADR-019 B/C type-level IRQ safety** | Migrate mixed-context `SwitchMutex` surfaces and `Channel.state` to `IrqSafeMutex`; remove manual wrappers | B: full-suite + micro-bench; C: SC-3B 200/200 `NEO_SMP=4` |
| **4. Formal close** | Stand up tiered perf harness; run OptiPlex bare-metal multi-CPU verification; absorb `F-004` | QEMU-TCG proxy CI gate + OptiPlex evidence + `F-004` PASS |

## 3. PM Clarifications Adopted

PL explicitly ratified the PLAN with the PM clarifications in `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` section 2.2:

- **INV-CAP-11 naming:** Subphase 1 may absorb `INV-CAP-11` through the `cap_*` suite, but the BUILD dispatch must name the exact scenario or add one if needed. `INV-CAP-11` must not disappear behind a broad suite label.
- **ADR amendments as close documentation:** The "ADR-016 amendment" and "ADR-019 amendment" expected close artifacts are post-implementation documentation closeouts. They are not new design work in this PLAN pass and do not authorize reopening ratified choices.

## 4. What This Unblocks

This closes the Phase 2 PLAN ratification gate and authorizes BUILD dispatches to begin.

Immediate next action: PM dispatches BUILD subphase 1 to Jarvis/Codex for capability reserved-bit activation:

- implement `sys_wait`
- implement `sys_inspect`
- keep `sys_close` deferred
- add or explicitly confirm the exact `INV-CAP-11` scenario: `cap_revoke_spawn_inheritance_descendants`

## 5. What This Does Not Do

- Does not close Phase 2. Phase 2 closes only after BUILD/VERIFY execution, evidence consumption, and close disposition.
- Does not authorize skipping the subphase gates or collapsing the four-subphase structure without a new PM/PL disposition.
- Does not reopen `sys_close`, full FM-D14 structural retirement, PID recycling, Spectre-v2/IBPB, PCID/ASID, RCU, load balancing, x2APIC, hot-plug, Arc-aware slab integration, or NVMe FLUSH durability.
- Does not start subphase 4 hardware execution without a written PM procedure and PL hardware session.

## 6. References

- `Plans/PROMETHEUS-OUTPUT-2026-05-25-phase-2-plan-substance.md` (`e6a4cdf`) - ratified PLAN substance.
- `Plans/PM-DISPOSITION-2026-05-25-phase-2-plan-substance-close.md` (`b7281d0`) - PM acceptance and section 2.2 clarifications.
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.2.md` (`ae3787f`, route correction `9129417`) - ratified THINK v0.2 substrate.
- `Plans/PHASE-2-THINK-v0.2-2026-05-24.md` (`349151e`) - ratified answered dispositions.
