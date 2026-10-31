<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: swapgs-aware device IRQ/IPI entries

**Date:** 2026-06-04
**From:** Jarvis
**To:** PM / PL
**Branch:** `codex/phase-5-baremetal-inference-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-06-04-swapgs-device-irq-page-fault-jarvis.md`
**Status:** PASS - local Gate-2 QEMU reproducer unblocked; OptiPlex run not executed here

## Summary

The SMP=4 bare-metal-mode page fault was fixed in the IDT entry path, not in inference logic or the oracle. External MSI/IPI vectors now use swapgs-aware asm entry stubs that mirror the timer discipline: branch on saved `CS.RPL`, swap GS only for ring-3 sources, run the Rust body with kernel GS, and swap back before `iretq`.

The earlier xtask wiring from `b4f3724` is preserved: `infer_transformer_forward --bare-metal-mode` stages the transformer model on AHCI/NeoFS and the log sink on NVMe.

## Changes

- `kernel/src/arch/x86_64/idt.rs`
  - Replaced plain `set_handler_fn` registration with `set_handler_addr` for `nvme_msi_interrupt`, `ahci_msi_interrupt`, TLB-shootdown IPI, reschedule IPI, and spurious.
  - Added a shared `swapgs_irq_entry` asm macro with saved-CS.RPL branching and symmetric ring-3 swapgs entry/exit.
  - Kept the Rust handler bodies unchanged in behavior, except for test-only ring-3 entry counters.
- `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs`, `kernel/Cargo.toml`, `userland/neo-init`, `userland/neo-worker`
  - Added `swapgs_irq_ring3`, a regression scenario that runs a ring-3 user spinner while CPU1 sends TLB-shootdown IPIs to CPU0.
  - The scenario asserts that at least one TLB-shootdown IPI entered from ring 3 and completed without a fault.

## Vector Audit

- NVMe MSI: confirmed Gate-2 trigger; can arrive while user code runs and reaches per-CPU state through the NVMe completion path. Fixed.
- AHCI MSI: same external device-IRQ entry class and storage handler shape. Fixed.
- TLB-shootdown IPI: can arrive while the target CPU is in ring 3 and calls `current_per_cpu()`. Fixed and directly pinned by `swapgs_irq_ring3`.
- Reschedule IPI: can arrive while the target CPU is in ring 3. Current body only EOIs, but it is the same async IPI entry class, so it now uses the same audited wrapper.
- Spurious interrupt: can arrive asynchronously. Current body only EOIs, but it now uses the same wrapper to avoid keeping one latent no-swapgs external vector.

No broader trap refactor was needed.

## Evidence

Targeted regression:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test swapgs_irq_ring3 --no-meta
```

```text
target/neo-test/swapgs_irq_ring3.attempt1.serial.log
NEO_TEST/1 event=PASS name=swapgs_irq_ring3 user_exit=0 ipis_sent=128 cpu0_ipis_received=128 tlb_ipi_ring3_entries=125
```

Original reproducer, repeated cleanly at SMP=4:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --bare-metal-mode --no-meta
```

Observed PASS in the initial run, repeat2, repeat3, and a final stable evidence run. Final stable evidence:

```text
target/neo-test/infer_transformer_forward.bare-metal.smp4-final.serial.log
target/neo-test/infer_transformer_forward.bare-metal.smp4-final.extracted.log
NEO_TEST/1 event=PASS name=infer_transformer_forward model=valid_transformer_block softmax_sum=10000 sys_infer_run=ok storage_freed=ok
```

Final SMP=4 hashes:

```text
78BCA332D19AF77A50F1C952A8478EE59F5DFF09EFE475B70A91EB085D57A6FD  target/neo-test/infer_transformer_forward.nvme-log.raw
D877D9E3BD1E3F4B3751438779C3EE3E04FFF7DA471D5E706DA9DCB8D7EC63A8  target/neo-test/infer_transformer_forward.bare-metal.smp4-final.extracted.log
5BE662FD6378DA07E5A9C99B6BAC5FC68518EE81F39C0C4C907B5FD217D5AB86  target/neo-test/infer_transformer_forward.bare-metal.smp4-final.serial.log
```

Bare-metal-mode SMP=1 regression:

```powershell
docker compose run --rm -e NEO_SMP=1 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test infer_transformer_forward --bare-metal-mode --no-meta
```

Result: PASS, including bare-metal log extraction PASS.

Normal SMP=4 suite:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test all --no-meta
```

Result: PASS. The suite log includes both `swapgs_irq_ring3` PASS and normal `infer_transformer_forward` PASS.

Host checks:

```powershell
docker compose run --rm neo cargo fmt --all --check
docker compose run --rm neo cargo test --workspace --exclude neoplutarch-kernel
docker compose run --rm neo cargo clippy --workspace --exclude neoplutarch-kernel -- -D warnings
git diff --check
```

Result: PASS. Direct `cargo` is not on this Windows shell's PATH, so the cargo host checks were run through the repo's Docker `neo` toolchain.

## Notes

No inference op logic, `.neom` format handling, or byte oracle was changed. Gate 2 can now proceed to the OptiPlex boot with the corrected local SMP=4 bare-metal-mode reproducer as the pre-metal guard.
