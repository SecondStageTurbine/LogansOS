<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #92): OT-87 R1 `xtask vm-image` (#152)

**Reviewer:** Jarvis (Codex)
**Model / effort:** GPT-6-Luna / high
**Review branch:** `codex/ot87-r1-vm-image-review-jarvis`
**Reviewed commit:** `22fd551f9244d902953184474d95d0a9171055da`
**Overall verdict: PASS**

## Checklist

1. **PASS — `boot.raw` is image A, unchanged.** `write_vm_disks` copies the generated ISO to `boot.raw`, then `check_vm_layout` compares the bytes directly. Its checks cover the protective MBR, GPT header, ESP type GUID, and all kernel NeoFS superblock offsets; the KATs exercise each rejection. (`tools/xtask/src/main.rs:16422-16442`; `tools/xtask/src/vm_image.rs:57-102`; `tools/xtask/src/vm_image_tests.rs:38-108,119-144`.)

2. **PASS — `data.raw` is a 128 MiB NeoFS volume and persists by default.** Creation invokes `neofs-mkfs create --variant arthur-memory --size-mib 128`; layout validation checks the expected size and the three imported kernel superblock offsets. Existing data is retained unless `--fresh-data` is set. The three-boot scenario also requires the guest to mount device 1 on each boot, exercising the mount-required volume structure. (`tools/xtask/src/main.rs:7873-7924,11935-11957,16422-16442`; `tools/xtask/src/vm_image.rs:25-29,57-93`; `kernel/src/drivers/block/write_guard_logic.rs:31-34`.)

3. **PASS — firmware is pinned for the gate.** Docker verifies both bundled OVMF files with `sha256sum -c`; xtask has corresponding BLAKE3 pins and returns `INFRA` on mismatch when the scenario requires pins. The container uses QEMU's bundled firmware; the Dockerfile adds no separate firmware download. The existing gate was independently queried by the parent: run `40440467911` completed successfully with all 22 jobs green, including the image build and QEMU shard 3. I did not independently hash the firmware files in a local container. (`Dockerfile:48-72`; `tools/xtask/src/vm_image.rs:38-49,154-193`; `tools/xtask/src/main.rs:7873-7877,16469-16477`; [gate run](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40440467911).)

4. **PASS — the scenario tests persistence and disk selection.** Boot 1 must emit `stage=write`; the fresh-disk control must not emit `stage=readback`; boot 3 on the original data disk must emit it. Each boot requires the NeoFS mount on device 1, and a BLAKE3 digest guards the boot disk before and after all boots. Thus broken persistence prevents the third read-back, while boot-disk mounting fails the device-index assertion (and the boot disk is also checked for NeoFS magic before boot). The control guest may take its normal first-boot write path on an empty disk; the asserted absence of `stage=readback` is the control. (`tools/xtask/src/main.rs:7873-7966`; `tools/xtask/src/vm_image.rs:57-93`.)

5. **PASS — optional; not run by reviewer.** PR #152 reports `vm-image` output matching the ISO and a successful `vm-run --headless` boot with NeoFS mounted from device 1; this is author evidence, not a rerun in this review. (`README.md:159-178`; [PR #152](https://github.com/SecondStageTurbine/LogansOS/pull/152), “Evidence”.)

6. **PASS — prior-art survey is present.** PR #152’s “Prior-art survey” table includes license and last-commit information and states that nothing is vendored. ([PR #152](https://github.com/SecondStageTurbine/LogansOS/pull/152), “Prior-art survey”.)

## Verification

- Reviewer rerun: `cargo test -p xtask vm_image` — 12 passed, 0 failed.
- RAG health check passed; graft was used for discovery, then the pinned source files were read directly.
- The parent independently queried run `40440467911`: completed successfully, all 22 jobs green, including QEMU shard 3. I did not rerun QEMU locally.
- The PR description explicitly scopes out the intro flag for R1; this review’s persistence verdict covers the memory-file behavior listed in the dispatch checklist.
