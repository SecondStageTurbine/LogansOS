<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #51): image A5 from main, touchpad BAR0 assign #114

**Result:** product image built from `main` commit `5880e07591beb429caf0400988fbf79d19b25454` on branch `codex/zenbook-image-a5-main-janus`. No source changes were made.

## Build

Command:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

Build log:

```text
[xtask] product image kernel: opt-level=3 perf-probes=false
```

A5 built successfully. A4 remains at `target/b1-model/A4-main-dec29417.iso`.

| Artifact | Path | Size (bytes) | BLAKE3 |
|---|---|---:|---|
| A5 ISO | `target/b1-model/A5-main-5880e075.iso` | 634097664 | `e1911282197df8b412ad47045fddbf721d872587fa4b9f6f652c7bf69cacf19f` |
| Staged kernel ELF | `target/iso-root/boot/kernel` | 35784512 | `6b065465e4774625f6370f12c89d0f2bce89f93f79839a7eed3b1c1f7543a182` |

## Feature-set and marker checks

`PRODUCT_IMAGE_KERNEL_FEATURES` at `tools/xtask/src/main.rs:4869` includes `freeze-watchdog` (`4884`), `key-autorepeat` (`4886`), `i2c-hid-touchpad` (`4889`), `i2c-hid-bar-assign` (`4891`), `rtl8125` (`4898`), and `rtl8125-dma-without-iommu` (`4899`). The enabled Cargo feature list in `target/b1-model/A5-main-5880e075-kernel-fingerprint.json` confirms all six.

Required source markers:

- Existing BAR config log: `kernel/src/drivers/i2c_hid/touchpad.rs:354` — `i2c-hid: cfg before`.
- Existing touchpad hold: `kernel/src/drivers/i2c_hid/touchpad.rs:1473` — `i2c-hid: TOUCHPAD NOT WORKING`.
- Ethernet summary: `kernel/src/drivers/net/rtl8125.rs:103` — `SUMMARY_HEADER` is `rtl8125-summary: ---- Ethernet evidence ----`.
- BAR sizing: `kernel/src/drivers/i2c_hid/touchpad.rs:716` — `bar0 size=`.
- BAR exclusions: `kernel/src/drivers/i2c_hid/touchpad.rs:813` — `bar0 excluded ranges=`.
- BAR candidate: `kernel/src/drivers/i2c_hid/touchpad.rs:834` — `bar0 candidate=`.
- BAR assignment: `kernel/src/drivers/i2c_hid/touchpad.rs:866` — `bar0 assigned=… by os`.
- DesignWare identification: `kernel/src/drivers/i2c_hid/touchpad.rs:503` — `i2c-hid: dw ver=`.

## PL hand-off and test plan

Write A5 to the **KINGSTON stick only**, using Rufus DD mode or Etcher as administrator. **Never select or write an internal disk on either machine.**

### ASUS Zenbook S14 UX5406AA

Unplug the USB mouse before booting Kingston. If the touchpad works, move a finger during the 6-second check and confirm pointer movement and taps at the desktop; photograph the `i2c-hid:` block if possible. If it still fails, photograph the full 30-second touchpad block and countdown, including `cfg before`/`cfg after`, `bar0 unassigned by firmware - assigning`, BAR size and excluded ranges, candidate(s), and `bar0 assigned=… by os`. Capture the following LPSS reset and DesignWare version, or any all-ones identify result.

Confirm the A4 behaviors remain: keyboard works, `avx2=Yes`, Ethernet summary says `absent`, and the model loads.

### MSI MS-7E57 AMD main PC (optional)

Connect Ethernet, boot Kingston through the F11 one-time menu, and never write the PC's disks. Photograph the `rtl8125-summary` block during the 30-second hold, then try one Arthur question.
