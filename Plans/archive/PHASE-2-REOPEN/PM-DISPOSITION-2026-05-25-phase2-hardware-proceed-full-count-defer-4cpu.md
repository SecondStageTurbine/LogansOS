# PM-DISPOSITION: proceed with full-count hardware tier while 4-CPU tier remains blocked

- **Date:** 2026-05-25
- **Author:** PM
- **Input disposition:** `Plans/PM-DISPOSITION-2026-05-25-phase2-hardware-4cpu-pass-ambiguity.md`
- **Current pushed HEAD:** `2f1aa14c308640484749a17e05b66c2e0c548e96`
- **Status:** ACCEPT OPTION A

---

## 1. Decision

PM accepts Option A from Jarvis's 4-CPU ambiguity disposition:

- proceed with the OptiPlex 7080 full-logical-CPU hardware scenario tier now;
- do not label those runs as the required `>=4` CPU-limited tier;
- keep the physical 4-CPU tier explicitly blocked until a later PM/PL decision either accepts a formal deferral or dispatches narrow CPU-limit tooling.

This decision is only to keep hardware evidence moving. It does not close Phase 2 hardware verification and does not satisfy the missing 4-CPU row.

---

## 2. Basis

Jarvis confirmed from live source at `2f1aa14`:

- `NEO_SMP` is QEMU-only.
- The scenario ISO builder copies `boot/limine.conf` without encoding a CPU-count limit.
- `boot/limine.conf` has no physical-media CPU limit setting.
- `kernel/src/arch/x86_64/smp.rs::bring_up_aps()` starts every Limine-reported AP up to `per_cpu::MAX_CPUS`.
- No existing boot-argument parser or physical CPU limiter was found.

Therefore a 12-logical-CPU OptiPlex run cannot honestly be recorded as the `>=4` CPU-limited tier.

---

## 3. First Authorized Full-Count Scenario

Jarvis prepared the first full-count scenario ISO:

| Field | Value |
|---|---|
| CPU tier | Full logical CPU count on OptiPlex 7080, expected 12 |
| Scenario | `smp_per_cpu_scheduler_smoke` |
| ISO path | `target/neo-test/vmware/smp_per_cpu_scheduler_smoke/smp_per_cpu_scheduler_smoke.iso` |
| SHA256 | `4E44C089113FC79FBA7AA2F7C1FA9D20931357BDB77DCAC62F9B2DC4CB1E70FA` |
| Expected PASS marker | `NEO_TEST/1 event=PASS name=smp_per_cpu_scheduler_smoke` |

PM independently verified the local ISO hash matches Jarvis's manifest.

---

## 4. Evidence Rule

Every full-count run must be labeled with CPU count `12` in the filename and report table.

Do not use the `4` CPU-count label until there is a real physical-media CPU-limit mechanism or a separate PM/PL disposition saying otherwise.

---

## 5. Next PL Instruction

PL may run the first full-count scenario now:

```text
ISO:
E:\AI Automation\AltimitNeo\target\neo-test\vmware\smp_per_cpu_scheduler_smoke\smp_per_cpu_scheduler_smoke.iso

SHA256:
4E44C089113FC79FBA7AA2F7C1FA9D20931357BDB77DCAC62F9B2DC4CB1E70FA

Serial log:
E:\AI Automation\AltimitNeo\Plans\evidence\phase2-bare-metal-optiplex-7080-20260525-195901-12-smp_per_cpu_scheduler_smoke.serial.log

Serial settings:
COM3 / 38400 / 8N1 / All session output

PASS marker:
NEO_TEST/1 event=PASS name=smp_per_cpu_scheduler_smoke
```

Stop on any unexpected `KERNEL PANIC`, `BUG:`, `PAGE_FAULT`, `event=FAIL`, frame-audit issue, GS-base sentinel mismatch, AP bring-up failure, or hang after serial progress.

---

**End of disposition.**
