<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: single-boot product-integration bare-metal artifact - Chad

**Date:** 2026-08-11 · **From:** Chad (Codex) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-11-3a-product-integration-baremetal-chad.md`
**Branch:** `codex/product-integration-baremetal-chad`
**Implementation:** `71641e94df0f5d375c441660e2c416856174d530`
**Base:** `origin/main` @ `b2c83678` (ff-clean)
**RAG:** healthy (`ollama=true`, `qdrant=true`); used first for log-sink, reboot-guard, and bare-metal precedent discovery, followed by direct reads of every changed and audited path.

## Verdict and execution model

`product_integration_baremetal` is an ignored, physical-only scenario that composes the existing five-rung
product feature graph and exports a bare-metal ISO plus an NVMe log sink. It is **one boot per invocation**.
There is no two-boot host driver, boot-index injection, runner change, or relaxed guard. The PL invokes the same
ISO twice by hand; `neo-init` selects boot 1 or boot 2 from `/neofs/.arthur/memory/product-0.1.persist` on the
physical AHCI `sda` volume.

The two additive feature aliases are:

- kernel: `neo-test-product-integration-baremetal = ["neo-test-product-integration"]`;
- neo-init: `product-integration-baremetal = ["product-integration"]`.

The userland assembly routes the alias to the existing DW-1 worker role and Arthur
`arthur-compositor-product-integration` role. This closes the otherwise silent OT-75 surface where a registered
scenario can compile while one child receives the wrong feature/authority composition.

## Exact-name call-site audit

| Dispatched site / function | Decision for `product_integration_baremetal` | Reason |
|---|---|---|
| timeout (`scenario_timeout`, former `:802`) | **YES: 3600 s** | Hardware-only long tier, matching `arthur_compositor_baremetal`. Existing `product_integration` remains 420 s; no existing budget changed. |
| reboot classification (`is_reboot_persistence_scenario`, former `:4290`) | **NO, deliberately absent** | Disk state selects the arm and the operator supplies the second boot. The existing bare-metal rejection for true host-orchestrated reboot scenarios remains byte-for-byte untouched. |
| shard cost (`scenario_shard_weight`, former `:4340`) | **No product special proxy; nominal weight 160** | The 3600 s timeout gives `100 + 3600/60`. More importantly, `select_test_scenarios` filters `ignored:true` before default sharding, so the canonical four-way partition is unchanged. |
| QEMU CPU (`scenario_needs_qemu_max`, former `:5784`) | **YES** | Defensive parity with the inherited Arthur/product composition if explicitly invoked under QEMU; no effect on the ignored default gate. |
| QEMU memory (`qemu_memory_for_scenario`, former `:6080`) | **YES: 4096M** | Same inherited compositor/model footprint; harmless for build-only bare-metal export and absent from default QEMU execution. |
| storage staging (`scenario_uses_storage_disk`, former `:6657`) | **NO** | Xtask must not create or attach a NeoFS scenario image. The prepared physical `sda` is the volume under test. A separate sink-only helper creates local evidence media. |
| brain-model staging (`prepare_storage_disks`, former `:7029`) | **NO** | The real model is already on physical AHCI `sda`; no fixture or model is staged by this build. |
| mkfs variant (`neofs_image_variant_for_scenario`, former `:7329`) | **N/A / `None`** | No scenario disk is staged, so there is nothing to mkfs. This build never opened or wrote `sda`. |
| expected-output arm (`require_scenario_specific_output`, former `:9119`) | **NO** | `ignored:true` means CI never runs this scenario in QEMU. Hardware serial and the NVMe block log are the evidence; adding an unexecuted host oracle would manufacture assurance. |
| sink allowlist / disk export | **YES: `neo-test-log-sink-target-nvme` and NVMe-sink-only** | AHCI is occupied by the NeoFS volume being proved. The independent boot NVMe owns `NEO_LOG_v1` at 256 MiB. |
| user feature assembly (additional OT-75 surface) | **YES: init alias + DW-1 worker + Arthur product role** | The alias inherits the disk-state driver while both children receive the exact roles used by QEMU product integration. The feature-on artifact build proves all four user binaries compile. |

The generated manifest has five local images only to preserve the established QEMU device indices: devices 0-3
are 16 MiB fillers marked `omit-for-vmware`; device 4 alone is `role=log-sink`, `controller=nvme`. There is no
`role=scenario` disk and no generated AHCI/NeoFS image.

## Kill-switch equality claim

With `neo-test-product-integration-baremetal` and `product-integration-baremetal` **off**, the compiled program
content for every pre-existing kernel/userland feature set is bit-for-bit unchanged: Cargo selects the same
pre-existing features and the same cfg-active source as before this change. In particular, QEMU
`product_integration` retains `ignored:false`, reboot-persistence classification,
420 s per-boot timeout, weight 103, QEMU max, 4096M, `arthur-memory` staging, and its existing boot-1/boot-2
oracles. This equality claim excludes the xtask executable and ISO filesystem metadata/timestamps: xtask
necessarily contains the new additive scenario registration, feature routing, and sink mapping.

## What the gate cannot prove

`ignored:true` means the four-shard QEMU gate **never executes `product_integration_baremetal`**. The separate
local artifact build proves that the feature-on kernel/userland composition compiles. The green gate proves the
default 290-scenario partition remains complete/disjoint and existing QEMU scenarios do not regress. Neither
proves that the new variant works on the OptiPlex. The first execution is the PL-owned physical boot in item 3.
No CI or local result below is presented as bare-metal runtime evidence.

## Blind-hardware discriminators

1. **Stale/wrong ISO before boot.** The kernel carries one unique stamp:
   `PRODUCT build_variant=product_integration_baremetal mode=single_boot_per_invocation sink=nvme`.
   Exact pre-boot check:

   ```sh
   strings -a product_integration_baremetal.bare-metal.iso | grep -F -m1 'PRODUCT build_variant=product_integration_baremetal mode=single_boot_per_invocation sink=nvme'
   ```

   Absence means stop: the artifact is stale or wrong. The built ISO contained exactly one copy, plus the rung,
   receiver-prompt, DW-1 focus, boot-2, and eth0-failure literals. Its local SHA-256 was
   `57ec997cd4b1f889e4e3fdf726ed4706452d7e30ff8f15a4fe1588956867ec73`.

2. **Boot 1 short-circuits because memory is non-empty.** Seeing
   `stage=readback ... source=prior_boot` on the intended first boot is the discriminator. It means desktop and
   Arthur never ran, so rungs 2 and 3 were not exercised. The run is **not idempotent**: after any completed boot
   1, another boot selects boot 2. Retrying boot 1 requires the PL's controlled volume-reset procedure first;
   merely rebooting is not a retry.

3. **Correct boot 2 is intentionally short and returns nonzero success.** After the common boot/mount, build
   stamp, rung-1, and rung-4 lines, neo-init emits one userland readback line:
   `NEO_TEST/1 event=INFO name=product_integration stage=readback bytes=39 source=prior_boot byte_equal=true fsync_prior_boot=true`.
   It then returns **78**. The kernel converts that distinguished nonzero result into terminal PASS
   `second_boot=prior_disk_byte_readback`. There should be no DW-1 focus, typed prompt, inference, or Arthur
   answer sequence on boot 2; their absence is expected, not a failure.

4. **AX88179 left connected.** The product latch fails before rung-1/rung-4 emission with
   `reason=product_eth0_expected_clean_no_dongle_degrade`. That literal is present in the ISO. Zero rung tokens
   plus this reason means “dongle present,” not “instrumentation missing” or “boot broke.”

5. **Sink wrong/unreadable.** The allowlist selects `neo-test-log-sink-target-nvme`; the manifest exports only
   device 4 as NVMe and reserves a 16 MiB region at `0x10000000` (256 MiB). A fresh sink is zero there; runtime
   writes the `NEO_LOG_v1` magic there. Post-run recovery remains:

   ```sh
   dd if=<boot-nvme-device> bs=1M skip=256 count=16 status=none | strings
   ```

   Missing `NEO_LOG_v1` after a run classifies the evidence channel as absent/unreadable; it is not permission
   to infer scenario success from display behavior.

## Verification and scope

- RAG health: healthy before discovery.
- `cargo test -p xtask`: 27/27 PASS.
- `cargo clippy -p xtask --all-targets -- -D warnings`: PASS.
- `cargo run -p xtask -- shard-coverage 4`: PASS; 290 scenarios, complete/disjoint exactly once; unchanged
  counts/weights `72/7485`, `72/7481`, `72/7442`, `74/7440`.
- Feature-on artifact build:
  `docker compose run --rm neo cargo xtask neo-test-build product_integration_baremetal --include-ignored --bare-metal-mode`: PASS.
- Bare-metal manifest: NVMe log sink only, no scenario disk; ISO stamp/discriminator inspection: PASS.
- `scripts/worker-close-gate.sh`: PASS after rebase.
- Push CI: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31454930975 - `success`, exact implementation SHA.
- Exact-SHA four-shard `workflow_dispatch`: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31454931270 - `success`.

Both CI runs report `headSha=71641e94df0f5d375c441660e2c416856174d530`. In the dispatch run, all four
QEMU shards, Host Tests/Format/Lint, Product Feature Build, SPDX, Monolithic Identity Guard, and all five
feature-matrix jobs were individually successful. As designed, none executed the ignored bare-metal scenario.

No bare-metal boot occurred. No physical device, `sda`, or hardware state was opened or modified. No reboot
guard was weakened; no existing budget was raised; OT-82 was not changed. I-14 fault containment is N/A: no
new driver or device-initiated DMA path was added, and sink selection reuses the existing NVMe log-sink path.
The concurrency pitfall bank is N/A: no cross-CPU, interrupt, lock, atomic, TLB-shootdown, or scheduler-wake
code changed. The byte oracle is N/A: there is no new on-disk or on-wire byte layout.
