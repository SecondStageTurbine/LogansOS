<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #63): build image A7 from main, for the Zenbook → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path, size
and BLAKE3, plus the marker checks below.

**Source:** `main` @ `745dd314`. It carries everything A6-hwp (`911a75bc` + `--hwp-performance`, relay #54) had. On the
Zenbook, A6-hwp showed:
- typing worked at once, with keys pressed during boot (`pre=78`);
- the touchpad parked;
- "What's 2+2?" took about 10 s until Arthur's box appeared, then about 7 s for the answer (A6 without HWP: 33 s and
  23 s).

It adds:
- **#124:** `hwp-performance` is in image A by default, so this is **one image**, not an A/B pair. The old
  `--hwp-performance` flag is accepted and does nothing.
- **#126 (the box-wait fix):** most of the ~10 s before Arthur's box was Arthur parsing its 5.4 MB vocab with an
  unoptimized `core`/`alloc`/`compiler_builtins` (a `build-std` dev build). These four packages are now built at
  opt-level 3. In QEMU the parse went from 171 s to 2.8 s. The overlay's start line now shows `vocab=O..C` (the vocab
  open and close ticks) between `arthur=` and `model=`. The `time` line's `think=` and `presents=` now count Arthur's
  own commits.
- **#119–#123 and #125:** freeze fixes (tick-sleep for the USB threads, the write-guard judge, the VFS/MOUNTS/model-load
  syscalls run with interrupts on, and an IRQ-safe AI arena lock). Nothing visible should change except that nothing
  hangs.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a7-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a7-main-janus 745dd314dd8e74e60839fb2529d3187222a93f5e

git show origin/main:Plans/DISPATCH-2026-10-25-zenbook-image-a7-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in the RETURN; the PL manages it.

## Build

Use the same command as relays #49–#54. Don't pass `--hwp-performance` or `--perf-probes`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
#   copy boot/neoplutarch.iso -> target/b1-model/A7-main-745dd314.iso
```

The build log must print `[xtask] product image kernel: opt-level=3`. Keep the A6 and A6-hwp ISOs.

**Rebuild userland from clean.** The change is in the root `Cargo.toml` profile, so a stale `target/` could reuse
old unoptimized `core`/`alloc` objects. Run `cargo clean` (inside the container) before the build, or delete the userland
target directory. Say in the RETURN which one you did.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`).
2. In `tools/xtask/src/main.rs`, `PRODUCT_IMAGE_KERNEL_FEATURES` ends with `tick-sleep` and `hwp-performance`, and
   includes `freeze-watchdog`, `i2c-hid-touchpad`, `i2c-hid-bar-assign`, `prefill-perf` and
   `matmul-steal-grace-time`.
3. The root `Cargo.toml` has `[profile.dev.package.neo-arthur]`, `[profile.dev.package.core]`,
   `[profile.dev.package.alloc]` and `[profile.dev.package.compiler_builtins]`, each at `opt-level = 3`; cite the lines.
4. These markers exist in the source; cite each `file:line`:
   - ` parse_ms=` in `userland/neo-arthur/src/main.rs`;
   - `vocab_open` / `vocab_closed` in `kernel/src/watchdog_timeline_logic.rs`;
   - `ps2={} pre={}` in `kernel/src/watchdog.rs`;
   - `tp: {} mode={} loops={}` in `kernel/src/drivers/i2c_hid/touchpad.rs`.

## What the PL checks (put this in the hand-off)

The Zenbook's internal disk is never written; it's behind VMD and isn't even visible. Unplug the USB mouse; use the
touchpad.

1. **Box wait (the fix).** At the desktop, click the Arthur bar, ask **"What's 2+2?"** and time two things: until
   Arthur's box appears, and until the answer is finished. Expect the box in about **1 s** (A6-hwp: about 10 s). The
   answer itself should take about the same as A6-hwp's ~7 s (this image doesn't change the per-word speed).
2. **Pointer during the answer.** While the answer is being written, move your finger on the touchpad. Say whether
   the pointer keeps moving smoothly, stutters, or freezes until the answer is done.
3. **Photograph the whole yellow overlay** after the answer, all its lines. What we read from it:
   - the `start` line: `vocab=O..C` should be a small gap now;
   - the `perf` line: prefill and decode ms per token, compared with A6-hwp's 208 and 304;
   - the `time` line: `think=` and `presents=` are real numbers now (A6-hwp showed `think=-` and `presents=0`);
   - the `cpu` line: still about 3,287 MHz with HWP on.
4. **Typing and touchpad**, as on A6: press a few keys during the boot screens, then type at the desktop at once.
   With your finger off the pad for ~10 s, the `tp:` line's `loops=` stays the same.

## Close condition

- Commit `Plans/RETURN-2026-10-25-zenbook-image-a7-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
