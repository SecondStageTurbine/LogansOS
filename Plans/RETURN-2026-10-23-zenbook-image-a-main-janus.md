<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook image A from merged main — Janus

Built 2026-10-23 in `E:\AI Automation\workers\janus` from exactly
`cb165c116dfc3f8148acd5ad2da8d7466a1abfbc`, on
`codex/zenbook-image-a-main-janus`. Image build only; no code changes or review.
The RETURN is the only committed change. Pre-existing `.gitignore` and `.ignore`
local edits were preserved and excluded.

## Build and artifacts

Command:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The build compiled userspace and kernel, staged the model and vocabulary, and
finished with xorriso's `Writing to 'stdio:/workspace/boot/neoplutarch.iso' completed successfully.`
and `[xtask] ISO ready: /workspace/boot/neoplutarch.iso` (310697 sectors).
The PowerShell stderr/Tee wrapper reported exit 1 despite those successful completion
messages; Docker's stderr was rendered as `NativeCommandError`. No compiler or
ISO-generation failure appeared. Local log: `target/zenbook-image-a-main-build.log`.

All paths below are relative to `E:\AI Automation\workers\janus`.

| Artifact | Size (bytes) | BLAKE3 |
|---|---:|---|
| `target/b1-model/A-main-cb165c11.iso` — hand-off copy | 636307456 | `a3447c287b940e8c4dfb6293c474030d641f73918180db04bb6e0947976ed6d4` |
| `boot/neoplutarch.iso` — build output | 636307456 | `a3447c287b940e8c4dfb6293c474030d641f73918180db04bb6e0947976ed6d4` |
| `target/iso-root/boot/kernel` — staged kernel ELF | 38397800 | `199ad62aaf39afca9bd73ce6c32cb0093738e93b88e82ff3332491ee169b2579` |

Hashes computed with `b3sum`. The copied ISO matches the build output.
Keep the ISO until the PL's run is done; only then use `cargo clean` if space is needed.

## Marker and feature checks

Read the staged ELF as bytes, verified ELF magic `7f 45 4c 46`, and searched for
each literal ASCII marker. All passed (offsets are zero-based byte offsets).

| Lane | Marker | Result | Offset |
|---|---|---|---:|
| #93 USB | `usb-summary` | PASS | 4166336 |
| #95 watchdog | `WATCHDOG t=` | PASS | 4193795 |
| #95 watchdog | `ticks/s` | PASS | 4193824 |
| #99 preload/freeze fix | `BOOTMODEL preload` | PASS | 4177449 |
| #98 i8042 | `i8042: present=` | PASS | 4172610 |

The exact routed i8042 source format string is:

```text
i8042: present={} selftest={} port1={} kbd={:?} irq1=gsi{}/pin{}{}{} vector={:#x} fadt_boot_arch={} keys={}
```

Source: `kernel/src/drivers/ps2/i8042.rs:246`, in `log_probe`; the unrouted variant
is at line 262. The stable literal prefix checked in the ELF is `i8042: present=`.

**Feature-list evidence:** this revision's normal `xtask iso --with-model` output
does not print a feature list, so the dispatch's literal printed-list check cannot
be confirmed as phrased. Instead, the command's source list at
`tools/xtask/src/main.rs:6046` includes:

```text
desktop,atrium-pointer-forward,real-lm,model-boot-module,boot-model-preload,fb-progress-console,lapic-timer,freeze-watchdog
```

The freshly generated Cargo feature record
`target/x86_64-neoplutarch/debug/.fingerprint/neoplutarch-kernel-0b0c201a8cd391a9/bin-kernel.json`
independently confirms both `boot-model-preload` and `freeze-watchdog`, plus
`ps2-keyboard` and the USB features. Required feature inclusion: PASS.
The staged `boot/limine.conf` contains both model and vocabulary module paths.

Build limitation: the optional MT7601U firmware blob was absent, so xtask omitted
`/modules/mt7601u-fw` and warned that MT7601U Wi-Fi will fail closed at runtime.
No bare-metal acceptance run was performed; this is the image hand-off.

## Hand-off to J.c. / PL

Write `E:\AI Automation\workers\janus\target\b1-model\A-main-cb165c11.iso`
to the **KINGSTON stick only**, using Rufus **DD mode** or Etcher as administrator.
**Never select the internal disk.** Per the Zenbook runbook, the kernel never writes
the Zenbook's internal NVMe. No disk-writing operation was performed for this build.

On the Zenbook:

- Before the desktop, look for
  `BOOTMODEL preload path=/models/model-qwen2.5-0.5b.neom ok=true … stack_depth=`.
  Boot takes longer for model loading; the watchdog may show `model load-hash`
  or `load-build` meanwhile.
- On the desktop, verify that the built-in keyboard types and that the USB
  keyboard and mouse work behind the hub.
- Ask Arthur a question: the pointer must keep moving, the screen must not freeze,
  and an answer must arrive. Open a second Arthur and ask again; it must answer too.
- If it freezes anyway, take two photos of the bottom-left watchdog overlay
  about 10 seconds apart.

The dispatch's built-in-keyboard expectation applies to this image; the older
runbook text saying the internal keyboard is unsupported predates the merged
i8042 lane. Hardware behavior remains for the PL to verify.
