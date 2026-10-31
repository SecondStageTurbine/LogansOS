# PM-DISPOSITION: 12-CPU OptiPlex tier satisfies Phase 2 hardware CPU minimum

- **Date:** 2026-05-25
- **Author:** PM
- **PL decision:** Forgo separate 4-CPU-limited physical tier; continue 12-logical-CPU full-count tier
- **Related prior artifacts:**
  - `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-4cpu-pass-ambiguity.md`
  - `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-proceed-full-count-defer-4cpu.md`
  - `Plans/RESEARCH-2026-05-25-phase2-4cpu-tier-without-bios.md`
- **Status:** ACCEPT PL DIRECTION; 4-CPU-limited tier no longer required for this hardware session

---

## 1. Decision

PM accepts PL's direction to forgo the separate 4-CPU-limited OptiPlex tier and continue the 12-logical-CPU full-count hardware sweep.

The 12-logical-CPU OptiPlex tier satisfies the Phase 2 hardware requirement to run on at least 4 logical CPUs. The separate 4-CPU-limited row was an internal procedure expansion intended as a lower-count comparator, not an external industry-standard requirement.

Future reporting must be explicit:

- record the executed hardware CPU tier as `12 logical CPUs`;
- state that no separate 4-CPU-limited physical tier was run;
- state that the 4-CPU-limited tier was superseded by PL/PM decision because the full-count tier exceeds the minimum hardware CPU requirement;
- do not claim evidence for an exact 4-CPU-limited run.

---

## 2. Rationale

The original procedure split the hardware scenario pass into:

1. `>=4` logical CPUs;
2. full logical CPU count.

On this target, full logical CPU count is 12. A 12-logical-CPU run is a stronger SMP exposure than a 4-logical-CPU run for AP bring-up, scheduler, IPC, TLB shootdown, process teardown, and capability-race surfaces.

The only value of a separate 4-CPU-limited tier would be comparator coverage for a smaller AP set. That is useful but not necessary for the current hardware evidence objective once PL has accepted the full-count tier as satisfying the minimum.

---

## 3. BIOS Finding

Dell's OptiPlex 7080 BIOS documentation lists:

- `Multi Core Support`: `All`, `1`, `2`, `3`;
- `Hyper-Thread Control`: enabled or disabled.

Therefore a firmware-mediated 4-logical-CPU configuration may be possible by setting `Multi Core Support = 2` with Hyper-Threading enabled. PM did not execute this firmware change. PL chose not to use this path for the current hardware session.

---

## 4. Current Full-Count Evidence So Far

The following full-count OptiPlex 7080 scenarios have passed and are logged:

| # | Scenario | Evidence |
|---:|---|---|
| 1 | `smp_per_cpu_scheduler_smoke` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-195901-12-smp_per_cpu_scheduler_smoke.serial.log` |
| 2 | `smp_same_cpu_ipc` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-201500-12-smp_same_cpu_ipc.serial.log` |
| 3 | `smp_block_release_smoke` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-202500-12-smp_block_release_smoke.serial.log` |
| 4 | `smp_send_blocks_first_then_completes` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-203500-12-smp_send_blocks_first_then_completes.serial.log` |
| 5 | `smp_recv_blocks_first_then_completes` | `Plans/evidence/phase2-bare-metal-optiplex-7080-20260525-204500-12-smp_recv_blocks_first_then_completes.serial.log` |

Each verified log shows:

- `smp: Limine reports 12 cpu(s)`;
- `smp: started 11 AP(s)`;
- the scenario-specific `NEO_TEST/1 event=PASS ...` marker.

---

## 5. Supersedes Prior Blocked Language

This disposition supersedes the earlier operational language that treated the 4-CPU-limited tier as blocking formal hardware progress.

The correct current posture is:

- 12-CPU full-count tier remains active and is the accepted CPU-count tier for this OptiPlex session;
- exact 4-CPU-limited physical tier is not run and must not be represented as run;
- no source change is needed solely to unblock the current hardware session.

---

**End of disposition.**
