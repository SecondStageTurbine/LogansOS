<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #50): image A4 from main, touchpad BAR fix #113

**Result:** product image built from `main` commit `dec29417ddba17c0e49d6e2fc8a6b875cd1ad26b` on branch `codex/zenbook-image-a4-main-janus`. No source changes were made.

## Build

Command:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

Build log:

```text
[xtask] product image kernel: opt-level=3 perf-probes=false
```

A4 built successfully. A3 remains at `target/b1-model/A3-main-4d6dd94c.iso`.

| Artifact | Path | Size (bytes) | BLAKE3 |
|---|---|---:|---|
| A4 ISO | `target/b1-model/A4-main-dec29417.iso` | 633989120 | `be8d2914306ecb96fa460ecc302023b2c53f1dae29c361cdf1c042311b62adfb` |
| Staged kernel ELF | `target/iso-root/boot/kernel` | 35675440 | `242db8d3a86f4581b91ce07a21bbee151e5da3dbabf09583e5bacb526bfdab7a` |

## Feature and marker checks

`PRODUCT_IMAGE_KERNEL_FEATURES` at `tools/xtask/src/main.rs:4869` includes `freeze-watchdog` (`4884`), `key-autorepeat` (`4886`), `i2c-hid-touchpad` (`4889`), `rtl8125` (`4896`), and `rtl8125-dma-without-iommu` (`4897`). The enabled feature list in `target/b1-model/A4-main-dec29417-kernel-fingerprint.json` confirms all five.

Required source markers:

- Existing touchpad hold: `kernel/src/drivers/i2c_hid/touchpad.rs:1241` — `i2c-hid: TOUCHPAD NOT WORKING`.
- Ethernet summary: `kernel/src/drivers/net/rtl8125.rs:103` — `SUMMARY_HEADER` is `rtl8125-summary: ---- Ethernet evidence ----`.
- AVX2 census: `kernel/src/main.rs:854` — `ai: matmul avx2=`.
- BAR config before wake: `kernel/src/drivers/i2c_hid/touchpad.rs:354` — `i2c-hid: cfg before`.
- BAR config after wake: `kernel/src/drivers/i2c_hid/touchpad.rs:372` — `i2c-hid: cfg after`.
- BAR restoration: `kernel/src/drivers/i2c_hid/touchpad.rs:391` — `bar restored after D3->D0 reset`.
- Unassigned BAR outcome: `kernel/src/drivers/i2c_hid/touchpad.rs:398` — `bar0 unassigned by firmware - stopping`.

## PL hand-off and test plan

Write A4 to the **KINGSTON stick only**, using Rufus DD mode or Etcher as administrator. **Never select or write an internal disk on either machine.**

### ASUS Zenbook S14 UX5406AA

Unplug the USB mouse before booting the Kingston stick from the ASUS boot menu. If the touchpad works, move a finger during the 6-second check and verify pointer movement and taps at the desktop; photograph the `i2c-hid:` block if possible. If it still fails, photograph the full 30-second touchpad block and countdown, especially the `cfg before`/`cfg after` lines and the BAR restore or stop reason. If bring-up continues, capture the LPSS reset and DesignWare identify lines. Confirm A3 behavior remains: Backspace repeats, model name in the footer, no `<|im_end|>`, `avx2=Yes`, and Ethernet summary says `absent`.

### MSI MS-7E57 AMD main PC (optional)

Connect Ethernet, boot Kingston through the F11 one-time menu, and never write the PC's disks. Photograph the `rtl8125-summary` block during the 30-second hold, then try one Arthur question.
