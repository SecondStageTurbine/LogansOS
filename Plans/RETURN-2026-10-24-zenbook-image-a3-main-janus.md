<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #49): image A3 from main for Zenbook + main-PC Ethernet test

**Result:** product image build completed on branch `codex/zenbook-image-a3-main-janus`, from `main` commit `4d6dd94c3fff551a968bd9129602f9c4fcf9b8bb`. No source changes were made.

## Build

Command:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

Build log:

```text
[xtask] product image kernel: opt-level=3 perf-probes=false
```

The ISO built successfully. A2 remains at `target/b1-model/A2-main-6ff86bc0.iso`.

| Artifact | Path | Size (bytes) | BLAKE3 |
|---|---|---:|---|
| A3 ISO | `target/b1-model/A3-main-4d6dd94c.iso` | 633974784 | `cb142708c28a801311bffcae35fcb1ffb1100c5d9dca50d1b003d2e5c8be7a6c` |
| Staged kernel ELF | `target/iso-root/boot/kernel` | 35661440 | `ab5f9c0b4acd96ed155c79509f772cfb4f840fb06535838c6c215efb76ae4809` |

## Feature and marker checks

`PRODUCT_IMAGE_KERNEL_FEATURES` in `tools/xtask/src/main.rs:4869` includes `freeze-watchdog` (`4884`), `key-autorepeat` (`4886`), `i2c-hid-touchpad` (`4889`), `rtl8125` (`4896`), and `rtl8125-dma-without-iommu` (`4897`). The Cargo kernel fingerprint record at `target/b1-model/A3-main-4d6dd94c-kernel-fingerprint.json` lists all five in its enabled `features` field.

Required source markers:

- Backspace repeat: `kernel/src/drivers/ps2/i8042.rs:552` — `key-repeat: rep={} typematic={}`.
- AVX2 census: `kernel/src/main.rs:854` — `ai: matmul avx2=`.
- Model footer: `userland/neo-arthur/src/showcase_logic.rs:313` — `runs on this machine`.
- Touchpad failure hold: `kernel/src/drivers/i2c_hid/touchpad.rs:1154` — `i2c-hid: TOUCHPAD NOT WORKING`.
- Ethernet summary: `kernel/src/drivers/net/rtl8125.rs:103` — `SUMMARY_HEADER` is `rtl8125-summary: ---- Ethernet evidence ----`.
- Ethernet outcome markers: `kernel/src/drivers/net/rtl8125.rs:1248` (`dhcp discover attempt=`), `1271` (`dhcp=timeout`), `1275` (`lease ip=`), and `1297–1313` (`gw-arp=` outcome strings).

## PL hand-off: test both machines from the KINGSTON stick

**Never select or write an internal disk on either machine.** Write the A3 ISO to the Kingston USB stick only (Rufus DD mode or Etcher as administrator).

### ASUS Zenbook S14 UX5406AA

Boot the Kingston entry from the ASUS boot menu. With the USB mouse unplugged, after the USB summary, photograph the full 30-second `i2c-hid: TOUCHPAD NOT WORKING …` countdown block. If the touchpad responds, the alternate check is a 6-second “move a finger” check. Just above it, confirm and photograph `rtl8125: pci 10ec:8125 absent` in the Ethernet summary. Also check the A2 behaviors: Backspace repeats, the footer names the model, generated answers omit `<|im_end|>`, and `avx2=Yes` is reported.

### MSI MS-7E57 AMD main PC

Connect Ethernet before boot. Use the one-time boot menu (F11) to boot Kingston; leave boot order unchanged. A USB keyboard and mouse may remain connected. During the touchpad 30-second hold, photograph the `rtl8125-summary` block, including the bind line (`dma=arena-only`), last DHCP discover attempt, lease IP or `dhcp=timeout`, and counters ending in `gw-arp=ok` or the reported failure state. Then let the desktop start and ask Arthur one question to observe answer speed on the 9900X.
