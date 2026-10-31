<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #50): build image A4 from main, for the Zenbook (and the main PC) → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path,
size and BLAKE3, plus the marker checks below.

**Source:** `main` @ `dec29417`. It carries everything image A3 (`4d6dd94c`, relay #49) had. A3 on the Zenbook
(2026-10-24) showed:
- the touchpad hold;
- the Ethernet summary (`rtl8125: pci 10ec:8125 absent`);
- `avx2=Yes (census: 16/16)`.

The touchpad stopped at:

```
i2c-hid: pci 00:19.1 8086:e351 pmcsr=D3->D0 cmd=0x0002 bar0=0x0
i2c-hid: no usable BAR0 - stopping
```

It adds **#113**, the touchpad controller's BARs across D3hot→D0. With PMCSR No_Soft_Reset clear, waking the
controller from D3 resets its config, and BAR0 reads 0 afterwards. The driver now:
- prints `i2c-hid: cfg before bar0=… bar1=… cmd=… pmcsr=…(Dn) nsr=…` before touching PMCSR, and the same
  `cfg after …` line after D0 + 10 ms;
- if BAR0 was assigned before and reads 0 after, writes it back and prints
  `i2c-hid: bar restored after D3->D0 reset bar0=… bar1=…`;
- otherwise stops with exactly one of:
  - `bar0 unassigned by firmware - stopping`;
  - `bar0 is an I/O BAR - stopping`;
  - `bar0 changed across D3->D0 - stopping`.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a4-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a4-main-janus dec29417ddba17c0e49d6e2fc8a6b875cd1ad26b

git show origin/main:Plans/DISPATCH-2026-10-24-zenbook-image-a4-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN; the PL manages it.

## Build

Use the same command as relays #36, #41 and #49. Don't pass `--perf-probes`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The build log must print `[xtask] product image kernel: opt-level=3 perf-probes=false`.
Copy `boot/neoplutarch.iso` to `target/b1-model/A4-main-dec29417.iso`. Keep the A3 ISO.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. The `opt-level=3 perf-probes=false` line from the build log.
3. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` includes `freeze-watchdog`, `key-autorepeat`,
   `i2c-hid-touchpad`, `rtl8125` and `rtl8125-dma-without-iommu`, and the fingerprint record confirms the set.
4. These markers exist in the source; cite each `file:line`:
   - from A3:
     - `i2c-hid: TOUCHPAD NOT WORKING` in `kernel/src/drivers/i2c_hid/touchpad.rs`;
     - `rtl8125-summary: ---- Ethernet evidence ----` (`SUMMARY_HEADER`) in `kernel/src/drivers/net/rtl8125.rs`;
     - `ai: matmul avx2=` in `kernel/src/main.rs`;
   - new, all in `kernel/src/drivers/i2c_hid/touchpad.rs`:
     - `i2c-hid: cfg before`;
     - `i2c-hid: cfg after`;
     - `bar restored after D3->D0 reset`;
     - `bar0 unassigned by firmware - stopping`.

## What the PL checks (put this in the hand-off)

### Zenbook (internal disk never written; it is behind VMD and isn't even visible)

1. **Touchpad.** Unplug the USB mouse before booting.
   - **If it works:** a 6 s "move a finger" check comes after the USB summary. Move a finger on the touchpad, then
     at the desktop the pointer should move and taps should click. Photograph the `i2c-hid:` block if you can.
   - **If it still doesn't work:** the boot screen holds 30 s on the touchpad block with a countdown.
     **Photograph that whole block.** The lines that matter:
     - `i2c-hid: cfg before …` and `i2c-hid: cfg after …`: the raw BAR values on each side of the wake-up;
     - then one of these:
       - `bar restored after D3->D0 reset`: the fix applied, and the step it then stops at is the next layer;
       - `bar0 unassigned by firmware - stopping`: firmware never gave the controller an address, so a separate
         lane is needed (allocating one);
       - `bar0 changed …` or `I/O BAR`: something unexpected; the cfg lines show what;
     - if it gets further: `i2c-hid: lpss reset=…` and the DesignWare identify step. A stop there is the next
       suspect (the LPSS clock gate, then the controller ID `0x44570140`).
2. Everything A3 showed should still hold:
   - holding Backspace repeats;
   - the footer names the model;
   - no `<|im_end|>`;
   - `avx2=Yes`;
   - Ethernet summary shows `absent`.

### Main PC: MSI MS-7E57, AMD (optional, when there's time)

This is the same check as relay #49's main-PC section, and A4 covers it as well as A3 would:
- Ethernet cable plugged in;
- boot the Kingston stick from the F11 one-time menu, never writing the PC's disks;
- photograph the `rtl8125-summary` block during the 30 s hold;
- then try one Arthur question.

## Close condition

- Commit `Plans/RETURN-2026-10-24-zenbook-image-a4-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk** on any machine.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
