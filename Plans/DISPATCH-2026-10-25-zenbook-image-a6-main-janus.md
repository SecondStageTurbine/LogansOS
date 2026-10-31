<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #54): build images A6 and A6-hwp from main, for the Zenbook → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with both images' paths,
sizes and BLAKE3, plus the marker checks below.

**Source:** `main` @ `911a75bc`. It carries everything image A5 (`5880e075`, relay #51) had. On the Zenbook, A5 got the
built-in touchpad working: the pointer moves and taps click. It also showed three problems:
- typing into Arthur stayed dead for about 5 minutes if keys were pressed during the boot screens;
- "What's 2+2?" took 57 s (35 s before the answer surface appeared, then 22 s);
- the touchpad's service thread spun all the time.

It adds:
- **#115:** the touchpad service thread parks on its interrupt instead of spinning. A `tp:` line on the yellow overlay
  shows `mode=` and `loops=`.
- **#116 + #117:** Arthur latency instrumentation (`prefill-perf`, now in image A). After each answer the overlay shows
  prefill/decode ms, the fork-join, a timeline, per-CPU MHz (APERF/MPERF) and the HWP state. It also adds a time-based
  matmul steal grace (`matmul-steal-grace-time`), corrected after Chad's review.
- **#118:** keys pressed during the boot screens are dropped when the desktop takes the keyboard (`pre=N` on the
  overlay, `i8042: dropped N pre-desktop bytes`), plus a startup timeline line (`start tk …`).
- **A6-hwp only:** `--hwp-performance` also turns on the CPU's hardware P-state control (HWP) with a performance
  preference. A reset undoes it, and Windows sets its own on its next boot. It's one image of an A/B pair, to show how
  much of the slowness is the CPU running slow.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a6-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a6-main-janus 911a75bc61601a8ffbe73476c789c929240739f8

git show origin/main:Plans/DISPATCH-2026-10-25-zenbook-image-a6-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in the RETURN; the PL manages it.

## Build

Build both images, one after the other, with the same command as relays #49–#51 (no `--perf-probes`).

```
# A6
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
#   copy boot/neoplutarch.iso -> target/b1-model/A6-main-911a75bc.iso
# A6-hwp
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom --hwp-performance
#   copy boot/neoplutarch.iso -> target/b1-model/A6hwp-main-911a75bc.iso
```

Each build log must print `[xtask] product image kernel: opt-level=3 …`. Keep the A5 ISO.

## Checks to record in the RETURN

1. For each image, the size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`). The
   two kernels must differ, since one has `hwp-performance`.
2. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` includes `freeze-watchdog`, `key-autorepeat`,
   `i2c-hid-touchpad`, `i2c-hid-bar-assign`, `rtl8125`, `rtl8125-dma-without-iommu`, `prefill-perf` and
   `matmul-steal-grace-time`. The A6-hwp build log or fingerprint shows `hwp-performance` added; the A6 one doesn't.
3. These markers exist in the source; cite each `file:line`:
   - `ps2={} pre={}` in `kernel/src/watchdog.rs`;
   - `dropped {} pre-desktop bytes` in `kernel/src/drivers/ps2/i8042.rs`;
   - `tp: {} mode={} loops={}` in `kernel/src/drivers/i2c_hid/touchpad.rs`;
   - `bar0 assigned=` in the same file.

## What the PL checks (put this in the hand-off)

The Zenbook's internal disk is never written; it's behind VMD and isn't even visible. Boot **A6 first**, then
**A6-hwp**. On each:

1. **Keys during boot.** During the boot screens (the usb-summary hold, the touchpad screen), press a few keys,
   **including Enter**. At the desktop, click the Arthur bar and type: the letters should appear at once. On the
   yellow overlay, `pre=` should be above 0, and the `model` line should stay `gen#0` until you ask something.
2. **Touchpad.** It still moves and clicks. With your finger off the pad for ~10 s, the `tp:` line's `loops=` stays the
   same (the thread is asleep).
3. **Speed.** Ask **"What's 2+2?"** and time it, both until the answer surface appears and until the answer is
   finished. Then **photograph the whole yellow overlay** (all its lines). Lines 4–8 hold the timing breakdown, the
   per-CPU MHz and the HWP state. Do the same on A6-hwp. The two photos side by side are the A/B.

## Close condition

- Commit `Plans/RETURN-2026-10-25-zenbook-image-a6-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator), one image at a time. **Never
  select an internal disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
