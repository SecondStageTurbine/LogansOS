<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #51): build image A5 from main, for the Zenbook (and the main PC) → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path,
size and BLAKE3, plus the marker checks below.

**Source:** `main` @ `5880e075`. It carries everything image A4 (`dec29417`, relay #50) had. A4 on the Zenbook
(2026-10-24) showed:

```
i2c-hid: cfg before bar0=0x00000004 bar1=0x00000000 cmd=0x0004 pmcsr=0x000b(D3) nsr=1
i2c-hid: cfg after  bar0=0x00000004 bar1=0x00000000 cmd=0x0004 pmcsr=0x0008(D0) nsr=1
i2c-hid: bar0 unassigned by firmware - stopping
```

So the controller had no soft reset and the D0 write worked, but firmware never assigned BAR0 (a 64-bit BAR at 0).

It adds **#114**, feature `i2c-hid-bar-assign`, now in image A's feature list. When firmware left BAR0 unassigned,
the driver assigns it itself:

1. It sizes the BAR with decode and bus mastering off (`i2c-hid: bar0 size=… 64bit=true`).
2. It excludes every range already in use: other functions' BARs, bridge windows, memory-map entries and VMD's
   BARs (`i2c-hid: bar0 excluded ranges=N (bars= windows= memmap= vmd=) fw64=[…) ceiling=…`).
3. It picks the lowest clear aligned slot at or above firmware's lowest 64-bit BAR
   (`i2c-hid: bar0 candidate=… from=fw-64bit-span|above-fw-span`).
4. It reads the slot before claiming it. Anything that answers is rejected (`… answered … - rejected`) and the
   next slot is tried.
5. It programs the slot (`i2c-hid: bar0 assigned=… by os`).

Only the touchpad controller's own BAR is ever written, and bus mastering stays off. Then the existing path
continues: the LPSS reset, then the DesignWare identify (`i2c-hid: dw ver=…`).

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a5-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a5-main-janus 5880e07591beb429caf0400988fbf79d19b25454

git show origin/main:Plans/DISPATCH-2026-10-24-zenbook-image-a5-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN; the PL manages it.

## Build

Use the same command as relays #36, #41, #49 and #50. Don't pass `--perf-probes`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The build log must print `[xtask] product image kernel: opt-level=3 perf-probes=false`.
Copy `boot/neoplutarch.iso` to `target/b1-model/A5-main-5880e075.iso`. Keep the A4 ISO.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. The `opt-level=3 perf-probes=false` line from the build log.
3. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` includes `freeze-watchdog`, `key-autorepeat`,
   `i2c-hid-touchpad`, `i2c-hid-bar-assign`, `rtl8125` and `rtl8125-dma-without-iommu`, and the fingerprint record confirms the set.
4. These markers exist in the source; cite each `file:line`:
   - from A4, in `kernel/src/drivers/i2c_hid/touchpad.rs`:
     - `i2c-hid: cfg before`;
     - `i2c-hid: TOUCHPAD NOT WORKING`;
   - `rtl8125-summary: ---- Ethernet evidence ----` in `kernel/src/drivers/net/rtl8125.rs`;
   - new, in `touchpad.rs`:
     - `bar0 size=`;
     - `bar0 excluded ranges=`;
     - `bar0 candidate=`;
     - `bar0 assigned=` (`by os`);
     - `i2c-hid: dw ver=`.

## What the PL checks (put this in the hand-off)

### Zenbook (internal disk never written; it is behind VMD and isn't even visible)

1. **Touchpad.** Unplug the USB mouse before booting.
   - **If it works:** a 6 s "move a finger" check comes after the USB summary. Move a finger on the touchpad, then
     at the desktop the pointer should move and taps should click. Photograph the `i2c-hid:` block if you can.
   - **If it still doesn't work:** the boot screen holds 30 s on the touchpad block with a countdown.
     **Photograph that whole block.** The lines that matter, in order:
     - `cfg before` / `cfg after` (the same as A4);
     - `bar0 unassigned by firmware - assigning`;
     - `bar0 size=…`;
     - `bar0 excluded ranges=… fw64=[…) ceiling=…`;
     - one or more `bar0 candidate=… from=…` lines (`answered … - rejected` means another device was there, so it
       was skipped);
     - `bar0 assigned=… by os`;
     - then `i2c-hid: lpss reset=…`, then either:
       - `i2c-hid: dw ver=…`: the controller answers at the new address, and whatever stops after that is the
         next layer;
       - `lpss bar0=0xffffffff (reads all-ones …)` or `all-ones` from the identify step: the laptop doesn't route
         that address to the controller, so the next lane reads the host bridge's address window from ACPI.
2. Everything A4 showed should still hold:
   - the keyboard works;
   - `avx2=Yes`;
   - Ethernet shows `absent`;
   - the model loads.

### Main PC: MSI MS-7E57, AMD (optional, when there's time)

This is the same check as relay #49's main-PC section, and A5 covers it:
- Ethernet cable plugged in;
- boot the Kingston stick from the F11 one-time menu, never writing the PC's disks;
- photograph the `rtl8125-summary` block during the 30 s hold;
- then try one Arthur question.

## Close condition

- Commit `Plans/RETURN-2026-10-24-zenbook-image-a5-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk** on any machine.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
