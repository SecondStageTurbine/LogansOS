# BLOCKER: Phase 5 Gate-2 inference bare-metal run stops in SMP=4 bare-metal-mode

**Date:** 2026-06-04
**From:** Jarvis
**To:** PM / PL
**Branch:** `codex/phase-5-baremetal-inference-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-06-04-phase-5-inference-baremetal-jarvis.md`
**Status:** RESOLVED LOCALLY - kernel swapgs entry fix validated; OptiPlex run still pending

## Summary

The requested xtask-only wiring is in place:

- `infer_transformer_forward` is accepted by `--bare-metal-mode`.
- The transformer model disk is staged on NeoFS over AHCI at device 0.
- The bare-metal log sink is staged on NVMe at device 4.
- The generated VMware manifest marks devices 1-3 as QEMU fillers to omit on the OptiPlex.

Normal QEMU regression still passes at `NEO_SMP=4`, so the new wiring does not perturb the existing scenario. However, the local `--bare-metal-mode` QEMU run fails reproducibly at `NEO_SMP=4` after AP bring-up and before an inference PASS can be read from `NEO_LOG`. Because the dispatch authorizes only test-infra wiring plus a hardware run, not kernel logic changes, I did not proceed to OptiPlex proof or change the oracle/kernel.

## Commands

Normal QEMU regression:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --no-meta
```

Result: PASS.

```text
target/neo-test/infer_transformer_forward.attempt1.serial.log
NEO_TEST/1 event=PASS name=infer_transformer_forward model=valid_transformer_block softmax_sum=10000 sys_infer_run=ok storage_freed=ok
```

Bare-metal bundle export:

```powershell
docker compose run --rm neo cargo xtask neo-test-build infer_transformer_forward --bare-metal-mode
```

Result: PASS. Bundle artifacts:

```text
target/neo-test/vmware/infer_transformer_forward/infer_transformer_forward.bare-metal.iso
target/neo-test/infer_transformer_forward.ahci.raw
target/neo-test/infer_transformer_forward.nvme-log.raw
target/neo-test/vmware/infer_transformer_forward/manifest.txt
```

Manifest excerpt:

```text
vmware_minimum_attachment=attach the transformer model disk as SATA/AHCI and the log sink disk as NVMe; omit qemu-filler disks
runtime_source=infer_transformer_forward mounts NeoFS from the AHCI model disk; block log sink targets the registered nvme block device
disk.0.role=scenario
disk.0.qemu_backend=ich9-ahci
disk.0.vmware_controller=sata-ahci
disk.4.role=log-sink
disk.4.qemu_backend=nvme
disk.4.vmware_controller=nvme
```

Bare-metal-mode QEMU repro:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --bare-metal-mode --no-meta
```

Result: FAIL, reproduced twice.

Serial log:

```text
target/neo-test/infer_transformer_forward.bare-metal.attempt1.serial.log
```

Key frames:

```text
[ kernel ] block: registered device index=0 driver=ahci name=ahci0p0 pci_bdf=Some((0, 3, 0))
[ kernel ] block: registered device index=4 driver=nvme name=nvme0n1 pci_bdf=Some((0, 7, 0))
NEO_TEST/1 event=INFO name=infer_transformer_forward bare_metal_log_sink=installed target_driver=nvme offset=0x10000000 size=16777216
NEO_TEST/1 event=INFO name=infer_transformer_forward bare_metal_log_sink_selftest=ok cursor=222
[ kernel ] smp: AP cpu_id=1 ready (lapic_id=1, processor_id=1)
[ kernel ] smp: AP cpu_id=2 ready (lapic_id=2, processor_id=2)
[ kernel ] smp: AP cpu_id=3 ready (lapic_id=3, processor_id=3)
NEO_TEST/1 event=BEGIN name=infer_transformer_forward phase=dispatch
[ kernel ] fs: mounted NeoFS readonly at /neofs selected_superblock=0 generation=1
[ kernel ] proc: spawned 'neo-init' (pid=1) entry=0x00000000004008f0 user_rsp=0x00007fffffffeff8 handles_inherited=0
[ trap  ] PAGE_FAULT (#PF) cr2=0x0000000000000018
[ trap  ] PAGE_FAULT rip=0xffffffffaae17f14 cs=0x8 rflags=0x86 rsp=0xffffffffab60a914 ss=0x0
NEO_TEST/1 event=FAIL name=infer_transformer_forward reason=unexpected_trap trap=page_fault
```

Relocation diagnostic:

```text
limine: executable virt=0xffffffffaada5000
rip 0xffffffffaae17f14 -> linked addr 0xffffffff80072f14
addr2line: kernel::arch::x86_64::per_cpu::current_cpu_id
           /workspace/kernel/src/arch/x86_64/per_cpu.rs:117
```

Bare-metal-mode QEMU control:

```powershell
docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --bare-metal-mode --no-meta
```

Result: PASS, including extracted `NEO_LOG` readback:

```text
target/neo-test/infer_transformer_forward.bare-metal-qemu.extracted.log
NEO_TEST/1 event=PASS name=infer_transformer_forward model=valid_transformer_block softmax_sum=10000 sys_infer_run=ok storage_freed=ok
```

## Assessment

The xtask wiring is sufficient to build and run the scenario in bare-metal mode, and the disk pairing is correct:

- AHCI model disk: registered as block device 0.
- NVMe log sink: registered as block device 4.
- Log-sink self-test: passes before scenario dispatch.
- `NEO_SMP=1 --bare-metal-mode`: passes and extracts the expected log.
- `NEO_SMP=4 --bare-metal-mode`: fails after AP bring-up and user process spawn.

This does not look like an oracle mismatch. The scenario does not reach a forward-pass output comparison under SMP=4; it trips a kernel page fault first. I did not modify the oracle or kernel inference logic.

## Needed From PM / PL

Direction on the next lane:

- authorize a kernel-side fix for the SMP=4 bare-metal-mode page fault before OptiPlex execution,
- authorize running the current bundle on the OptiPlex despite the local SMP=4 bare-metal-mode failure, or
- narrow Gate 2 to a different hardware/test configuration.

Until then, the Phase 5 Gate-2 metal PASS criteria remain unmet.

## Resolution - 2026-06-04

`Plans/DISPATCH-2026-06-04-swapgs-device-irq-page-fault-jarvis.md` authorized the kernel-side fix. The page fault was resolved by giving the external MSI/IPI vectors swapgs-aware entry stubs that branch on saved `CS.RPL`, swap only for ring-3 sources, and swap back before `iretq`.

Validated local gates:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --bare-metal-mode --no-meta
```

Result: PASS across repeated SMP=4 bare-metal-mode runs; final stable evidence:

```text
target/neo-test/infer_transformer_forward.bare-metal.smp4-final.serial.log
target/neo-test/infer_transformer_forward.bare-metal.smp4-final.extracted.log
NEO_TEST/1 event=PASS name=infer_transformer_forward model=valid_transformer_block softmax_sum=10000 sys_infer_run=ok storage_freed=ok
```

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test swapgs_irq_ring3 --no-meta
```

Result: PASS with a ring-3 delivered TLB-shootdown IPI regression pin:

```text
NEO_TEST/1 event=PASS name=swapgs_irq_ring3 user_exit=0 ipis_sent=128 cpu0_ipis_received=128 tlb_ipi_ring3_entries=125
```

No `cr2=0x18`, page-fault, or panic signature was present in the final SMP=4 bare-metal-mode serial/extracted logs. The local reproducer blocker is closed; the physical OptiPlex Gate-2 run can proceed under the Phase 5 runbook.
