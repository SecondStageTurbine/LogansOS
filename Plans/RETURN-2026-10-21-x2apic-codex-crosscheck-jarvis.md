# RETURN: CP-68 x2APIC cross-family review

**Verdict: BOUNCE**

Reviewed CP-68 as `e77d8d273d1e2f4c7f2596744d492077d1176859`, relative to `e42d2483`. The fetched source branch has since merged `origin/main` at `0f31c474`; the return is being committed on top of that current branch tip. RAG was unavailable (`127.0.0.1:8404` refused connections twice); `graft ask` was used for semantic discovery, then the implementation and call sites were read directly. The Intel SDM Vol. 3A x2APIC register map, ICR, ordering, and mode-transition rules were checked against the manual.

## Finding

### P2 — AP can attempt an illegal x2APIC-to-xAPIC transition

**Location:** `kernel/src/arch/x86_64/apic.rs:99-110` (`init_local_apic`); the asymmetric check is at `:142-153` (`init_local_x2apic`).

The x2APIC-BSP branch checks each AP's `IA32_APIC_BASE` and halts if the AP is not in x2APIC mode. The xAPIC-BSP branch has no corresponding AP check: it unconditionally removes `X2APIC_ENABLE` and writes `IA32_APIC_BASE`. If an AP reaches `init_ap` with x2APIC enabled while the BSP is in xAPIC mode, this is a direct x2APIC→xAPIC transition, which the SDM defines as illegal (#GP); the only legal route is through the disabled state. The response's single x2APIC flag and ordinary QEMU runs do not establish a per-CPU invariant that excludes this mismatch. The NOTE itself treats mixed per-CPU modes as a case worth guarding.

**Repro:** boot with BSP `IA32_APIC_BASE.X2APIC_ENABLE=0`, but one released AP with that bit set. During `init_traps_ap`, `apic::init_ap` reaches the xAPIC branch and writes the mode bit directly. Expected behavior should be a named mismatch halt (or an explicitly valid transition); current behavior raises #GP. Add a per-AP mode check before the xAPIC write and cover the mismatch path.

## Review checklist

1. **Mode handling and MMIO:** The normal request uses `MP_FLAG_X2APIC`; `xapic-only` restores flags `0`. BSP mode is selected from its `IA32_APIC_BASE` bit and disagreement with Limine is logged. The x2APIC init branch does not write `IA32_APIC_BASE`; `read_reg`/`write_reg` select MSRs in x2APIC mode, including SVR, EOI, and the feature-gated one-shot timer. `reg_ptr` is reached only in the xAPIC branch. `init_bsp_apic` runs in `main` before block, network, or USB device probes; BSP interrupts remain disabled during earlier trap setup, and exception/NMI paths do not call LAPIC EOI. Thus I found no current early caller using the default `mode()==xAPIC` before BSP mode is published. The AP mismatch asymmetry above is the mode finding.

2. **ICR and ordering:** x2APIC uses one 64-bit write to MSR `0x830`; the destination occupies bits 63:32, while the low word is vector plus level-assert. This is fixed delivery, physical destination, edge-triggered, no shorthand. There is no delivery-status poll. `mfence; lfence` precedes the ICR write, matching the SDM's prescribed sequence for ordering prior stores before a non-serializing x2APIC APIC-register WRMSR. Shootdown payload slots are published with `AcqRel` swaps before the send; the reschedule test observes the receiving CPU's counter. EOI follows handler work. For shootdowns, the handler's release decrement and sender's acquire wait establish the payload/ACK ordering; a subsequent IPI can remain pending until EOI. I found no separate EOI ordering defect in those paths.

3. **32-bit IDs and xAPIC mask:** `MpInfo::lapic_id`, `register_lapic_id`, the per-CPU `AtomicU32` table, lookup, AP bring-up, and x2APIC destination remain `u32`; the x2APIC ICR helper carries all 32 destination bits. The xAPIC helper masks to the architectural 8-bit destination. I did not establish whether Limine can return a MADT x2APIC ID above 255 when the xAPIC-only request has successfully disabled x2APIC. That configuration is not tested by the supplied control; if it can occur, the xAPIC path needs to reject such a target instead of aliasing it. This remains an unconfirmed note, not an additional finding.

4. **MSI:** Both MSI constructors call `msi_address_for_bsp`; IDs above 255 return `None` before the MSI capability/table is programmed. NVMe errors if MSI-X and MSI both fail; xHCI has a poll path; AHCI records MSI as unavailable and carries that state into port setup; virtio block/accel attempt MSI-X then MSI. The drivers initialize after `init_bsp_apic`, so the fallback address 0 before registration is not used in the normal boot path; Limine specifies an MP response even on a single-processor system when the request is supported. MSI is deliberately BSP-targeted, matching this kernel's current interrupt handling; this is not CPU-affinity routing. The 8-bit destination limit remains an expected limitation for BSP IDs above 255, with refusal rather than truncation.

5. **AP bring-up and barriers:** `init_traps_ap` loads the IDT and per-CPU state before calling `apic::init_ap`; the x2APIC mismatch halts before AP ready signaling and before `mark_tlb_shootdown_ready`. The BSP's bounded ready wait can report a degraded AP, and the ready/shootdown masks do not include an AP that never completed setup. This covers the x2APIC-BSP/xAPIC-AP direction; the reverse direction is the finding above.

6. **Mode resolution:** The BSP MSR must govern which register interface the kernel uses; trusting a contradictory Limine report could issue a faulting x2APIC MSR access or dead MMIO access. The code logs the disagreement and follows the hardware. A per-AP check is still needed for both mismatch directions.

7. **Scenario and KATs:** `x2apic_smp_ipi` is selected with `max,+x2apic`, checks `mode()` before counting any IPIs, requires three ready APs, waits for each target's reschedule receive counter, then performs 16 shootdown rounds and checks receive/timeout counts. The first guard makes the `xapic-only` control fail with `apic_mode_not_x2apic`; it cannot pass by counting an xAPIC IPI. Repeated reschedule receipts also require the EOI path to release the vector, and SVR is initialized before the test. The KAT expected values checked in source match the SDM map (`0x80b`, `0x80f`, `0x830`, `0x832`, `0x838`, `0x83e`, `0x802`), the full-width ICR examples, and MSI IDs 0/255 plus refusal above 255. The branch NOTE reports QEMU 10.2.3 runs, including plain `-cpu max` in x2APIC and `qemu64` controls in xAPIC. I did not rerun builds or tests in this review.

8. **Kill-switch / xAPIC compatibility:** `xapic-only` sets the old MP request flags to zero. The xAPIC ICR high/low values are unchanged by the helper (`(target & 0xff) << 24`, same low word); xAPIC MMIO register reads/writes and EOI values retain their previous effects. MSI ID 0 retains `0xfee00000`. This is behavioral/write-value equivalence; source code is refactored and is not literally byte-identical.

9. **Other xAPIC-only paths and rollout:** I found no separate raw LAPIC MMIO access outside `apic.rs`; all register callers route through `read_reg`/`write_reg`, including the test-only LAPIC timer. No other MSI doorbell address is constructed in drivers. PIC IRQ0 remains the system tick, which is a platform/legacy-interrupt dependency noted by the author, not an xAPIC MMIO dependency. Requesting x2APIC does change every x2APIC-capable `-cpu max` run; the branch NOTE reports that matrix passed on QEMU 10.2.3 and calls for OptiPlex acceptance rerun. The MSI destination refusal for a BSP ID above 255 can disable device interrupts on a high-ID system; existing driver fallback/error behavior applies.

## Validation

Static review only; no build or tests were run. No changes were made to implementation files.
