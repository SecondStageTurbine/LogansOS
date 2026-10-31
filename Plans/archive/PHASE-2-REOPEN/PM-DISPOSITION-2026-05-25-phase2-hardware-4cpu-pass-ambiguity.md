# PM-DISPOSITION: Phase 2 hardware 4-CPU pass ambiguity

- **Date:** 2026-05-25
- **Author:** Jarvis
- **Source commit inspected:** `2f1aa14c308640484749a17e05b66c2e0c548e96`
- **Handoff:** `Plans/HANDOFF-2026-05-25-phase-2-hardware-sweep-to-jarvis.md`
- **Status:** BLOCKED for the physical 4-CPU tier; first full-count scenario media prepared

---

## 1. Disposition

Existing tooling does not provide a confirmed way to build or boot a physical scenario ISO on the OptiPlex 7080 while limiting NeoPlutarch to exactly 4 logical CPUs.

Do not label a 12-logical-CPU OptiPlex run as the required `>=4` CPU-limited pass. The full-count tier can proceed separately, but the 4-CPU tier remains blocked until PM/PL either accepts an explicit deferral or dispatches a narrow CPU-limit implementation.

## 2. Live Source Findings

RAG was used for source navigation, then live source was inspected directly.

Findings:

- `tools/xtask/src/main.rs` uses `NEO_SMP` only when launching QEMU with `-smp` in the `neo-test` run paths.
- The scenario ISO build path stages the kernel, modules, and `boot/limine.conf`; it does not encode `NEO_SMP` into the physical ISO.
- `write_limine_conf()` only rewrites the `kaslr:` line when a KASLR override is requested.
- `boot/limine.conf` contains no kernel command-line CPU limit or scenario CPU-count setting.
- `kernel/src/arch/x86_64/smp.rs::bring_up_aps()` iterates over every CPU Limine reports, skipping only the BSP and CPUs beyond `per_cpu::MAX_CPUS`.
- No live source hit showed a kernel boot-argument parser or physical-media CPU limiter.

Conclusion: the handoff's ambiguity is real.

## 3. PM Options

Option A: Run the full-count hardware scenario sweep now and explicitly defer the 4-CPU hardware pass until CPU-limit tooling exists.

This keeps PL moving on hardware evidence without mislabeling the missing tier. The final Phase 2 report must list the 4-CPU tier as deferred/blocking formal close unless PM/PL accepts the deferral as sufficient.

Option B: Dispatch a narrow implementation task to add a boot-time CPU limit for scenario ISO media.

This is the clean path if Phase 2 close requires both CPU-count rows before hardware execution continues. The implementation should include an ISO-visible setting, kernel-side AP bring-up limit enforcement, and QEMU/physical scenario evidence.

Recommendation: use Option A only if PM explicitly accepts the 4-CPU deferral; otherwise dispatch Option B before asking PL to run the full required matrix.

## 4. First Full-Count Batch Manifest

Prepared one full-count scenario ISO. This is not evidence for the 4-CPU tier.

| Field | Value |
|---|---|
| CPU tier | Full logical CPU count on OptiPlex 7080, expected 12 |
| Scenario | `smp_per_cpu_scheduler_smoke` |
| Build command | `docker compose run --rm neo cargo neo-test --build-only smp_per_cpu_scheduler_smoke` |
| Build result | PASS |
| ISO path | `target/neo-test/vmware/smp_per_cpu_scheduler_smoke/smp_per_cpu_scheduler_smoke.iso` |
| Manifest path | `target/neo-test/vmware/smp_per_cpu_scheduler_smoke/manifest.txt` |
| SHA256 | `4E44C089113FC79FBA7AA2F7C1FA9D20931357BDB77DCAC62F9B2DC4CB1E70FA` |
| El Torito check | BIOS and UEFI boot images present; UEFI image path `/boot/limine-uefi-cd.bin` |

## 5. PL Next Boot Instruction

Use this only if PM/PL chooses to proceed with the full-count tier while the 4-CPU tier is deferred.

1. Flash this ISO in Rufus DD image mode:

```text
E:\AI Automation\AltimitNeo\target\neo-test\vmware\smp_per_cpu_scheduler_smoke\smp_per_cpu_scheduler_smoke.iso
```

2. Record this SHA256 before flashing:

```text
4E44C089113FC79FBA7AA2F7C1FA9D20931357BDB77DCAC62F9B2DC4CB1E70FA
```

3. Start PuTTY logging before power-on:

```text
E:\AI Automation\AltimitNeo\Plans\evidence\phase2-bare-metal-optiplex-7080-20260525-195901-12-smp_per_cpu_scheduler_smoke.serial.log
```

4. Serial settings:

```text
COM3 / 38400 / 8N1 / All session output
```

5. Boot target: OptiPlex 7080 UEFI USB, Secure Boot disabled.

6. Wait for:

```text
NEO_TEST/1 event=PASS name=smp_per_cpu_scheduler_smoke
```

Also preserve the SMP bring-up lines showing Limine's reported CPU count and AP start count. Stop and file the full serial log immediately on unexpected `KERNEL PANIC`, `BUG:`, `PAGE_FAULT`, `event=FAIL`, frame-audit issue, GS-base sentinel mismatch, AP bring-up failure, or hang after serial progress.

---

**End of disposition.**
