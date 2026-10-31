<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #54): Zenbook images A6 and A6-hwp

**Source:** `main` @ `911a75bc61601a8ffbe73476c789c929240739f8`  
**Build host:** Janus  
**RAG:** not used (image build only).

## Images

Both commands completed successfully. Each log contains the required product-image kernel opt-level line.

| Image | ISO | Bytes | BLAKE3 | Kernel ELF | Bytes | BLAKE3 |
|---|---|---:|---|---|---:|---|
| A6 | `target/b1-model/A6-main-911a75bc.iso` | 634310656 | `75644f800a4f7e8519e7ff38665530fa54b81e58c43d2f51adf7c5ee631337d8` | extracted from this ISO | 35995952 | `a4f873524071e03525b9373cd7a18cc5ae948420c95727b7c93360819ded2e82` |
| A6-hwp | `target/b1-model/A6hwp-main-911a75bc.iso` | 634312704 | `88a8257c3abee030d8ca52a2941596119046551623d3e4ab1162593b3a3211a3` | staged kernel ELF | 35999016 | `6f0082d8b4ecc1c66e8f2faa85579e197ad373dd01771014f970cc8bd7b3d876` |

Build logs: `target/b1-model/A6-main-911a75bc-build.log` and `target/b1-model/A6hwp-main-911a75bc-build.log`.

The logs report:

- A6: `[xtask] product image kernel: opt-level=3 hwp-performance=false`
- A6-hwp: `[xtask] product image kernel: opt-level=3 hwp-performance=true`

The two kernel ELFs differ (size and BLAKE3 both differ). The optional MT7601U firmware blob was absent from the checkout, so the build omitted `/modules/mt7601u-fw`; Wi-Fi requiring that firmware will fail closed at runtime.

## Feature and marker checks

`PRODUCT_IMAGE_KERNEL_FEATURES` in `tools/xtask/src/main.rs:4879` includes:

- `freeze-watchdog` (`:4894`)
- `key-autorepeat` (`:4896`)
- `i2c-hid-touchpad` (`:4899`)
- `i2c-hid-bar-assign` (`:4901`)
- `rtl8125` and `rtl8125-dma-without-iommu` (`:4908-4909`)
- `prefill-perf` (`:4914`)
- `matmul-steal-grace-time` (`:4916`)

The HWP feature is appended when `--hwp-performance` is supplied (`tools/xtask/src/main.rs:4941`); the build logs above confirm it is false for A6 and true for A6-hwp.

Requested source markers:

- `ps2={} pre={}` - `kernel/src/watchdog.rs:369`
- `dropped {} pre-desktop bytes` - `kernel/src/drivers/ps2/i8042.rs:549`
- `tp: {} mode={} loops={}` - `kernel/src/drivers/i2c_hid/touchpad.rs:1371`
- `bar0 assigned=` - `kernel/src/drivers/i2c_hid/touchpad.rs:899`

## Kingston hand-off and PL checks

No Kingston DataTraveler was attached to the build host during this run. Read-only disk inventory showed six internal SATA/NVMe drives and no removable USB disk, so no disk was written. The ISOs are ready for raw writing to the Kingston stick only, using Rufus in DD mode or Etcher as administrator. Confirm the target is the approximately 30 GB Kingston device before writing; never select an internal disk.

Boot A6 first, then A6-hwp, following `docs/runbook-baremetal-zenbook-s14.md`. On each image, check that keys pressed during boot are dropped at desktop handoff and typing into Arthur works immediately (`pre=` above zero); check the touchpad moves and clicks and its `loops=` value stays steady with no touch for about 10 seconds; then ask "What's 2+2?" and time both answer-surface appearance and completion. Photograph the full yellow overlay on both images for the A/B timing, per-CPU MHz and HWP state. The stick write and these physical-device checks remain pending because the Kingston and Zenbook were not present on the build host.
