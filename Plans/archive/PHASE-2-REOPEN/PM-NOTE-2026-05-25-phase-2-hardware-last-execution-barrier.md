# PM-NOTE: Phase 2 hardware-last execution barrier

- Author: PM
- Date: 2026-05-25
- Status: Binding sequencing clarification from PL instruction
- Applies to: Phase 2 BUILD/VERIFY after `Plans/PL-RATIFICATION-2026-05-25-phase-2-plan.md`

---

## 1. PL Instruction

PL clarified the desired Phase 2 execution posture:

> "I want all of phase 2 to be done, right up to hardware testing. I want hardware testing to be the last part of Phase 2. So lets make sure ALL of phase 2 is done completly until we cant do anything else, but hardware testing."

PM interprets this as a hardware-last execution barrier:

- Finish every software, QEMU, VMware, perf-harness, procedure, documentation, and close-prep item that can be completed without physical hardware.
- Do not spend the OptiPlex hardware session while any non-hardware Phase 2 item remains open.
- The final Phase 2 work item should be physical hardware evidence capture and its close disposition.

This note does not change the ratified four-subphase PLAN. It constrains execution order so subphase 4's hardware portion happens last.

## 2. Pre-Hardware Completion Checklist

Before PM routes the OptiPlex hardware session, the following must be complete or explicitly accepted as blocked by hardware:

| Area | Required pre-hardware state |
|---|---|
| **Subphase 1: capability reserved-bit activation** | `sys_wait`, `sys_inspect`, exact `INV-CAP-11` handling, QEMU/scenario verification, and close report accepted |
| **Subphase 2: ADR-016 A2 scheduler heap-decoupling** | fixed-capacity scheduler rings, `INV-SYNC-5`, QEMU/SMP verification, and close report accepted |
| **Subphase 3: ADR-019 B/C IRQ safety** | B/C migration, `INV-IPC-5`, SC-3B QEMU stress gate, micro-bench proxy evidence, and close report accepted |
| **Perf harness non-hardware tier** | QEMU-TCG deterministic proxy gate implemented and runnable; VMware/reproducible middle-tier path documented or explicitly deferred by PM |
| **F-004 and remaining audit chain** | any QEMU-reproducible portion executed; only hardware-only evidence left unresolved |
| **Hardware procedure** | written PM procedure, capture paths, expected evidence vocabulary, pass/fail strings, and escalation path prepared |
| **Close documentation prep** | required ADR close amendments and Phase 2 close integration plan prepared as post-evidence updates, without reopening ratified design choices |

If any row above is still actionable without hardware, the hardware session waits.

## 3. Allowed Final Hardware Remainder

Once the barrier is satisfied, the remaining Phase 2 work should be hardware-only or hardware-evidence consumption:

- OptiPlex 7080 bare-metal multi-CPU verification at >=4 logical CPUs and at the target's full logical CPU count.
- Hardware-tier absolute-latency/perf baseline capture.
- Bare-metal capture of any ADR-019 micro-bench evidence assigned to the shared hardware session.
- Hardware-only `F-004` evidence if QEMU cannot fully satisfy it.
- Close report evidence ingestion and PM/PL close disposition.

If the hardware session exposes a software bug, Phase 2 returns to the appropriate BUILD/VERIFY worker, fixes the issue, reruns non-hardware gates, and only then repeats the hardware evidence step.

## 4. Non-Phase-2 Boundary

NVMe FLUSH durability diagnosis remains governed by `Plans/NOTES-2026-05-23-nvme-driver-flush-and-completion-review.md`. It may briefly interrupt scheduling when the serial-console adapter is installed, but it is not folded into the Phase 2 hardware-last barrier unless PL explicitly routes it as Phase 2 work.

## 5. Dispatch Rule

Future Phase 2 dispatches should cite this note when they touch subphase 4 or hardware scheduling. The worker instruction should be:

> Complete every non-hardware action first. Route hardware only when the only remaining Phase 2 evidence requires the OptiPlex.
