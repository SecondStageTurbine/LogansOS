<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook image A2 from main — Janus, relay #41

**BUILD COMPLETE. KINGSTON FLASH PENDING MANUAL HAND-OFF.** Image build only;
no code changes, review, or Zenbook boot/test was performed.

Source: `6ff86bc0d2422b6c928460a31db3b14d5e0f37a3` exactly. Branch:
`codex/zenbook-image-a2-main-janus` (RETURN only). Read the full rewritten
relay #41 dispatch and Zenbook runbook from `origin/main` before building.
RAG was not used.

## Build and artifacts

Ran successfully, exit 0:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

No `--perf-probes`. The retained log is
`target/zenbook-image-a2-main-6ff86bc0-build.log`; line 114 is exactly:

```text
[xtask] product image kernel: opt-level=3 perf-probes=false
```

All paths below are relative to `E:\AI Automation\workers\janus`.

| Artifact | Size (bytes) | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/A2-main-6ff86bc0.iso` | 633,444,352 | `db1ebc44d0d078c785a2bd598acd4c5d63ca822de6c61f5e9c8e447550e443fc` |
| `target/iso-root/boot/kernel` | 35,130,256 | `1c495b777b466b97597278ddb3d7be2d60730ef51a957e452d7b519200cbf62d` |
| Preserved `target/b1-model/A-main-cb165c11.iso` | 636,307,456 | `a3447c287b940e8c4dfb6293c474030d641f73918180db04bb6e0947976ed6d4` |

Copied the completed `boot/neoplutarch.iso` to the A2 path; both have the same
BLAKE3. The previous A image was hashed before and after and is unchanged.
`b3sum.exe` produced the hashes. `xorriso -report_el_torito plain` confirms
the A2 ISO has both bootable BIOS and UEFI entries, plus the hybrid protective
MBR/GPT/APM boot record. No physical device was used for this verification.

The build completed with 43 kernel warnings. It also reported that the optional
MT7601U firmware blob was absent and omitted `/modules/mt7601u-fw`; no firmware
was added and no WiFi hardware result is claimed.

## Features and markers

`PRODUCT_IMAGE_KERNEL_FEATURES` at `tools/xtask/src/main.rs:4869` includes
`freeze-watchdog` at `:4884`, `key-autorepeat` at `:4886`, and
`i2c-hid-touchpad` at `:4889`.

The fresh Cargo fingerprint is
`target/x86_64-neoplutarch/debug/.fingerprint/neoplutarch-kernel-b651d1455b2edc7e/bin-kernel.json`.
Preserved a copy at `target/b1-model/A2-main-6ff86bc0-kernel-fingerprint.json`.
Its expanded `features` includes all three required features, plus
`phase9-avx2-matmul`, `threaded-matmul`, `real-lm`, `boot-model-preload`,
`model-boot-module`, `fb-progress-console`, `lapic-timer`, and `xsave-fpu`.
**`prefill-perf` is absent.**

Required source markers, checked at the pinned source:

| Marker | Source location |
| --- | --- |
| `usb-summary:` | `kernel/src/drivers/usb/xhci_silicon.rs:620` |
| `WATCHDOG t=` and `ticks/s=` | `kernel/src/watchdog.rs:237` |
| `BOOTMODEL preload ... ok=` | `kernel/src/boot_modules.rs:552` |
| `i8042: present=` | `kernel/src/drivers/ps2/i8042.rs:248` and `:264` |
| `i2c-hid: quirk={} match ...` | `kernel/src/drivers/i2c_hid/touchpad.rs:277` |
| `key-repeat: rep={} typematic={}` | `kernel/src/drivers/ps2/i8042.rs:552` |
| Watchdog ` rep={} typematic={}` | `kernel/src/watchdog.rs:276` |
| `ai: matmul avx2=... (census: ... cpus reported)` | `kernel/src/main.rs:854` |
| `runs on this machine` | `userland/neo-arthur/src/showcase_logic.rs:313` |

Also checked the staged kernel ELF bytes: every listed kernel marker is
present. The staged `target/iso-root/modules/neo-arthur` contains
`runs on this machine` and `[answer cut short]`. These are build checks,
not claims that the target boot or user interactions passed.

## KINGSTON hand-off

The requested ISO is ready at:

```text
E:\AI Automation\workers\janus\target\b1-model\A2-main-6ff86bc0.iso
```

Initial disk enumeration showed no USB device. Later enumeration identified:

- Disk **6**, **Kingston DataTraveler 3.0**, serial **4E0482D05EC9**.
- Bus **USB**, capacity **124,117,843,968 bytes** (128 GB class).
- `IsBoot=false`, `IsSystem=false`.
- Rufus 4.15.2396 showed **one device**, `Multiple Partitions (I:) (J:) [128 GB]`.

This stick's observed capacity differs from the runbook's approximate 30 GB;
the actual model, USB bus, serial, and non-system flags above identify the
connected KINGSTON. No internal SATA/NVMe disk was selected or written.

Opened the installed Rufus using the computer-use skill. Its SELECT control
did not open the ISO picker after observation, activation, refresh, and one
retry. **No Flash/START action was issued; no write or read-back is claimed.**
Asked the operator to select the exact A2 path above and write in **DD image
mode** to this **KINGSTON only**. Awaiting completion of that manual step;
the Zenbook's internal Windows NVMe must never be selected.

## PL checks on the Zenbook

1. Boot A2 from the KINGSTON UEFI entry. Photograph the boot evidence:
   `BOOTMODEL preload ... ok=true`, USB summary, watchdog, and i8042 lines.
   Confirm the built-in keyboard plus USB keyboard and mouse work, and Ask
   completes without freezing.
2. **Photograph the `i2c-hid:` lines whatever happens:** quirk match, PCI
   controller, HID descriptor, reset, IRQ mode, and touchpad check/replay.
   On the desktop, verify pointer motion and click. Gestures are not expected.
3. Type a word and hold Backspace: one deletion, about a 500 ms pause, then
   repeated deletion at about 30/s. Photograph watchdog `rep=` increasing.
4. Photograph `ai: matmul avx2=Yes (census: 16/16 cpus reported)` (or the
   actual CPU count). Photograph `No` or `Undecided` if either appears.
5. Time a short question from Enter to answer completion, then a long one
   such as “explain how a CPU cache works in detail.” Record both times;
   seconds rather than image A's roughly three minutes is the expectation,
   not a result established by this build. At 256 tokens, verify
   ` [answer cut short]` appears.
6. Verify answers do not end in `<|im_end|>` and the footer reads
   `Qwen2.5 0.5B · runs on this machine · Esc to go back`.

All build artifacts and logs remain in this Janus worktree. Only this RETURN
is committed. Generated graft telemetry is kept under ignored `target/`
when closing the tree; prior local-state stashes are retained.
