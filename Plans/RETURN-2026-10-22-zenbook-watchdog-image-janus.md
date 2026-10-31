<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook freeze-watchdog test image A — Janus

Date: 2026-10-22  
Return branch: `codex/zenbook-watchdog-image-janus`  
Source built detached: `d9d766ab1c81333d5f1078a0eed0180b47d82c83` (PR #95 head)

**TEST IMAGE from an unmerged branch, for one Zenbook run only.** Image A only; no bundles were built.

## Build

Built with the pinned Qwen2.5-0.5B model and its adjacent vocab:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The plain Windows-host invocation compiled the kernel but could not assemble the ISO because `/home/neo/limine` is provided by the build container. The container build completed successfully. It noted the optional MT7601U firmware blob is absent, so `/modules/mt7601u-fw` was omitted and that Wi-Fi lane will fail closed.

Copied `boot/neoplutarch.iso` to `target/b1-model/A-watchdog.iso`. The two ISO files have identical size and BLAKE3:

| Artifact | Size (bytes) | BLAKE3 |
|---|---:|---|
| `target/b1-model/A-watchdog.iso` | 635963392 | `5adb694b931eab0ed9c8b8874f2b202929f7471220c74e70193bf163ffdd9891` |
| `target/iso-root/boot/kernel` | 38052576 | `f2630dedac3cac74cb1e6c18c5941dc81ecf888cb4fa9f886977ef3585a7ba16` |

Checked the staged kernel ELF strings for the requested USB summary and freeze-watchdog overlay markers. The ELF contains `usb-summary`, `WATCHDOG t=`, and `ticks/s`.

## Hand-off

Image path:

```text
E:\AI Automation\workers\janus\target\b1-model\A-watchdog.iso
```

J.c.: write this image to the **KINGSTON USB stick** with Rufus in **DD image mode** or Etcher as administrator. Verify the selected device is the KINGSTON stick before writing; **never select the 954 GB internal disk**. This image is for one Zenbook run only. No physical Zenbook boot or USB-stick write was performed by Janus.
