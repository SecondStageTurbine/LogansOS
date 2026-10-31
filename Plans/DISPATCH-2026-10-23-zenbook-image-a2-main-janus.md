<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #41, rewritten): build Zenbook image A2 from main → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path,
size and BLAKE3, plus the marker checks below.

**Source:** `main` @ `6ff86bc0`. It carries everything image A (`cb165c11`, relay #36) had, plus:
- **#100 Zenbook touchpad rung 1:** I2C-HID over the Intel Serial-IO I2C, in mouse mode, matched to the Zenbook S14
  firmware only (feature `i2c-hid-touchpad`);
- **#106 key auto-repeat** (`4452783f`): holding a key repeats after 500 ms, at 30 per second (feature `key-autorepeat`);
- **#108 Arthur display fixes** (`a41bf335`):
  - no `<|im_end|>` in answers;
  - the footer reads `Qwen2.5 0.5B · runs on this machine · Esc to go back`;
  - answers run up to 256 tokens, and a capped one ends with ` [answer cut short]`;
- **#105 faster answers** (`6ff86bc0`):
  - the image kernel is built at opt-level 3;
  - AVX2 is decided once per boot by a per-CPU census;
  - the prompt skips lm_head except for its last token.
  On the bench, a prompt went from 208 s to 4.3 s.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a2-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a2-main-janus 6ff86bc0d2422b6c928460a31db3b14d5e0f37a3

git show origin/main:Plans/DISPATCH-2026-10-23-zenbook-image-a2-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN; the PL manages it.

## Build

The same command as relay #36. Do **not** pass `--perf-probes`; this is the plain product image.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

The build log must print `[xtask] product image kernel: opt-level=3 perf-probes=false`.
Copy `boot/neoplutarch.iso` to `target/b1-model/A2-main-6ff86bc0.iso`, and keep `A-main-cb165c11.iso`.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. The `[xtask] product image kernel: opt-level=3 perf-probes=false` line from the build log.
3. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` includes `freeze-watchdog`, `key-autorepeat` and
   `i2c-hid-touchpad`, and the fingerprint record confirms the set.
4. Every marker from relay #36 is still present: `usb-summary`, `WATCHDOG t=`, `ticks/s`, `BOOTMODEL preload`, and
   `i8042: present=`. The new markers also exist in the source. For each, cite the `file:line` you checked:
   - `i2c-hid:`, in `kernel/src/drivers/i2c_hid/touchpad.rs` (for example `i2c-hid: quirk={} match ...`);
   - `key-repeat: rep={} typematic={}`, in `kernel/src/drivers/ps2/i8042.rs`;
   - the watchdog line's ` rep= typematic=`, in `kernel/src/watchdog.rs`;
   - `ai: matmul avx2=` with `(census: N/M cpus reported)`, in `kernel/src/main.rs`;
   - `runs on this machine`, in `userland/neo-arthur/src/showcase_logic.rs`.

## What the PL looks for on the Zenbook (put this in the hand-off)

Everything from image A must still work:
- `BOOTMODEL preload … ok=true`;
- the built-in keyboard, and a USB keyboard and mouse;
- Ask, with no freeze.

Plus the new items:
1. **Touchpad:** during boot, the `i2c-hid:` lines show the quirk match, the PCI controller, the HID descriptor, the
   reset and the IRQ mode. **Photograph these lines whatever happens**; they show what went wrong if the touchpad
   fails. On the desktop, the touchpad moves the pointer and a click works. No gestures are expected yet.
2. **Backspace repeat:** type a word, then hold Backspace. It deletes one character, pauses about half a second, then
   keeps deleting. The watchdog corner line shows `rep=` rising.
3. **Boot line `ai: matmul avx2=Yes (census: 16/16 cpus reported)`** (or however many CPUs). `Undecided` or `No` on
   the Zenbook is worth a photo.
4. **Speed:** ask a short question and time it from Enter to the end of the answer; it should be seconds, not image
   A's ~3 minutes. Then ask for something long (for example "explain how a CPU cache works in detail") and time it to
   the end. If it hits the 256-token limit, it ends with `[answer cut short]`.
5. **Answer text:** no `<|im_end|>` at the end.
6. **Footer:** `Qwen2.5 0.5B · runs on this machine · Esc to go back`.

## Close condition

- Commit `Plans/RETURN-2026-10-24-zenbook-image-a2-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select the internal
  disk.** The Zenbook's Windows NVMe is never written.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
