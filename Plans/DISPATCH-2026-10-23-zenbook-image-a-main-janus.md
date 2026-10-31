<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #36): build Zenbook image A from main → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path,
size and BLAKE3, plus the marker checks below.

**Source:** `main` @ `cb165c11` (the first `main` carrying all four Zenbook lanes):
- #93 USB input (`5f5685b5`): the xHCI a keyboard/mouse is on, port service, the `usb-summary` block;
- #98 i8042 built-in keyboard (`334f5ba4`);
- #95 freeze watchdog overlay (`829af5a8`);
- #99 boot-model preload, the freeze fix (`cb165c11`).

This is a **merged-`main`** image, not a test branch, so it is the first image whose result counts as an acceptance
milestone for those four lanes.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a-main-janus cb165c116dfc3f8148acd5ad2da8d7466a1abfbc

git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-image-a-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                       # the Zenbook runbook
```

## Build

The same way as your `d9ad3424` watchdog image (the container provides Limine):

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

Copy `boot/neoplutarch.iso` to `target/b1-model/A-main-cb165c11.iso`.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. The kernel ELF contains these strings (each marks one lane being built in):
   - `usb-summary` (#93);
   - `WATCHDOG t=` and `ticks/s` (#95);
   - `BOOTMODEL preload` (#99);
   - the i8042 driver's console/log marker (#98; grep the source for its exact string and cite it).
3. Confirm the `xtask iso --with-model` feature list it printed includes `boot-model-preload` and `freeze-watchdog`.

## What the PL looks for on the Zenbook (put this in the hand-off)

- **At boot, before the desktop:** a `BOOTMODEL preload path=/models/model-qwen2.5-0.5b.neom ok=true … stack_depth=`
  line. Boot takes longer by the model load. The watchdog overlay may show `model load-hash` / `load-build` meanwhile.
- **On the desktop:** the built-in keyboard types, and the USB keyboard/mouse work behind the hub.
- **Ask Arthur a question:** the pointer must keep moving and the screen must not freeze. An answer must arrive.
  Open a second Arthur and ask again; it must answer too.
- **If it freezes anyway:** take two photos of the bottom-left watchdog overlay about 10 s apart.

## Close condition

- Commit `Plans/RETURN-2026-10-23-zenbook-image-a-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off text for J.c.: write the ISO to the **KINGSTON** stick only (Rufus DD mode or Etcher as administrator).
  **Never select the internal disk.** The kernel never writes the Zenbook's internal NVMe.
- **Disk reclaim:** keep the ISO until the PL's run is done; `cargo clean` afterwards if space is needed.
