<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #49): build image A3 from main, for the Zenbook and the main PC → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path,
size and BLAKE3, plus the marker checks below.

**Source:** `main` @ `4d6dd94c`. It carries everything image A2 (`6ff86bc0`, relay #41) had, which passed on the
Zenbook on 2026-10-24:
- Backspace repeat: `rep=10 typematic=7`;
- the footer names the model, and answers end without `<|im_end|>`;
- `avx2=Yes (census: 16/16)`;
- answers took about 30 s.

It adds:
- **#109:** the RTL8125 2.5GbE driver, with `rtl8125-dma-without-iommu` (PL decision, 2026-10-24). One image tests
  both machines:
  - on the AMD main PC (MSI MS-7E57) the driver binds with `dma=arena-only`;
  - on the Zenbook it prints `absent` and does nothing.
- **#111:** when the touchpad isn't working, the boot screen holds for **30 s** on `i2c-hid: TOUCHPAD NOT WORKING …`
  with a countdown. Shift on the built-in keyboard ends it.
- **#112:** the RTL8125's boot lines are re-printed under `rtl8125-summary: ---- Ethernet evidence ----` just before
  the touchpad check, so they're still on screen during that hold.
- **#110:** host-only ACPI interpreter fixes. They don't change the image.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a3-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a3-main-janus 4d6dd94c3fff551a968bd9129602f9c4fcf9b8bb

git show origin/main:Plans/DISPATCH-2026-10-24-zenbook-image-a3-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN; the PL manages it.

## Build

Use the same command as relays #36 and #41. Don't pass `--perf-probes`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The build log must print `[xtask] product image kernel: opt-level=3 perf-probes=false`.
Copy `boot/neoplutarch.iso` to `target/b1-model/A3-main-4d6dd94c.iso`. Keep the A2 ISO.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. The `opt-level=3 perf-probes=false` line from the build log.
3. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` includes `freeze-watchdog`, `key-autorepeat`,
   `i2c-hid-touchpad`, `rtl8125` and `rtl8125-dma-without-iommu`, and the fingerprint record confirms the set.
4. These markers exist in the source; cite each `file:line`:
   - from A2:
     - `key-repeat: rep={} typematic={}` in `kernel/src/drivers/ps2/i8042.rs`;
     - `ai: matmul avx2=` in `kernel/src/main.rs`;
     - `runs on this machine` in `userland/neo-arthur/src/showcase_logic.rs`;
   - new:
     - `i2c-hid: TOUCHPAD NOT WORKING` in `kernel/src/drivers/i2c_hid/touchpad.rs`;
     - `rtl8125-summary: ---- Ethernet evidence ----` (`SUMMARY_HEADER`) in `kernel/src/drivers/net/rtl8125.rs`;
     - `lease ip=`, `dhcp=timeout` and `gw-arp=` in the same file.

## What the PL checks (put this in the hand-off)

### Zenbook (internal disk never written; it is behind VMD and isn't even visible)

1. Everything A2 showed still holds:
   - holding Backspace repeats;
   - the footer names the model;
   - no `<|im_end|>`;
   - `avx2=Yes`.
2. **Touchpad:** with the USB mouse unplugged, after the USB summary screen the boot screen holds 30 s on the
   touchpad block. It ends with `i2c-hid: TOUCHPAD NOT WORKING - <reason> verdict=… - stopped at: …` and a countdown.
   **Photograph that whole block.** If the touchpad works instead, there is a 6 s "move a finger" check.
3. Just above it, the Ethernet summary shows `rtl8125: pci 10ec:8125 absent`. That is expected on the Zenbook.

### Main PC: MSI MS-7E57, AMD (the first LogansOS boot on this machine)

1. Plug in the Ethernet cable before booting. Boot the **Kingston** stick from the board's one-time boot menu
   (F11 on MSI boards). Don't change the default boot order, and **never write the PC's disks**: the image carries
   no disk-write path, and every block write goes through the write guard.
2. A USB keyboard and mouse can stay plugged in.
3. Near the end of boot, the touchpad check finds no touchpad and holds the screen 30 s. During that hold,
   **photograph the `rtl8125-summary: ---- Ethernet evidence ----` block** above it:
   - the bind line: `rtl8125: <bus:dev.fn> xid=… mac=… link=up <speed>/<duplex> dma=arena-only bme=… irq=polled`;
   - the last `rtl8125: … dhcp discover attempt=…` line;
   - `rtl8125: … lease ip=a.b.c.d router=… dns=…`, or `dhcp=timeout`;
   - the counters line, ending in `gw-arp=ok` (the router answered) or `gw-arp=timeout|no-tx|tx-failed|…`.
   Whatever it shows, the photo tells us what happened.
4. Then let it reach the desktop and try one Arthur question, to see how fast answers are on the 9900X.

## Close condition

- Commit `Plans/RETURN-2026-10-24-zenbook-image-a3-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk** on any machine.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
