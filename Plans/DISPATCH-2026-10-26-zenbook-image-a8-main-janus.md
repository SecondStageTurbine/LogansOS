<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #72): build image A8 from main, for the Zenbook → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path, size
and BLAKE3, plus the marker checks below.

**Source:** `main` @ `87730200`. It carries everything A7 (`745dd314`, relay #63) had. On the Zenbook, A7 showed:
- spawn→model 1.96 s (it was 10.7 s);
- prefill 213 ms/token and decode 319 ms/token at 3,301 MHz with HWP on;
- `presents=0` during generation.

It adds (J.c.: "wait for all of it before building A8"):
- **#128 (lever 1):** `soft_f32` add/sub/mul run on SSE, byte-identical to the integer path.
- **#130 (lever 1b):** attention computes each score once. Also new overlay shares `sw=` (SwiGLU) and `at=` (cached
  attention), and the MXCSR check moved from every add/mul to each op's entry.
- **#134:** the image's kernel is built with core's UB precondition checks off (`-Zub-checks=no`, J.c.'s decision).
  debug-assertions and overflow-checks stay on. GDN and sampling gained MXCSR entry checks, and the perf line is
  bounded.
- **#135:** Arthur maps a prebuilt binary vocab (`NEOVOCB1`, a `.vocb` converted at image build) instead of parsing
  the 5.4 MB text vocab. In QEMU, parse_ms went from ~2,800 to 120.
- **#127, #129, #131, #132:** model provenance runs IF-on; test fixes; the overlay's per-pid commit slots are one
  atomic word; the vocab hooks take no lock.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a8-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a8-main-janus 8773020065f6909de01f6d800ce645d87bf0e8d2

git show origin/main:Plans/DISPATCH-2026-10-26-zenbook-image-a8-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in the RETURN; the PL manages it.

## Build

Run `cargo clean` first, as for A7, keeping `target/b1-model` aside so the model, the vocab and the old ISOs survive.
Then use the same command as relays #49–#63. Don't pass any flag, and don't set `NEO_IMAGE_UB_CHECKS`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
#   copy boot/neoplutarch.iso -> target/b1-model/A8-main-87730200.iso
```

The build log must show three things:
- `[xtask] product image kernel: opt-level=3 ub-checks=off (NEO_IMAGE_UB_CHECKS=on keeps them)`;
- the vocab converted to `NEOVOCB1` and passing its pin. If the conversion or pin fails, the build stops with
  "no image was written". Report that verbatim; don't work around it;
- `ISO ready`.

Keep the A6, A6-hwp and A7 ISOs.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`). The `.vocb` staged in
   the ISO root (`model-qwen2.5-0.5b.vocb`), with its size and BLAKE3: it must be 5,010,512 bytes and match
   `QWEN25_05B_VOCB` in `kernel/src/boot_model_logic.rs`.
2. The xtask log lines above, quoted.
3. These markers exist in the source; cite each `file:line`:
   - the banner `debug-assertions={} overflow-checks={} ub-checks={}` in `kernel/src/main.rs`;
   - ` format=neovocb1` in `userland/neo-arthur/src/main.rs`;
   - `PRODUCT_IMAGE_UB_CHECKS_OFF` in `tools/xtask/src/main.rs`;
   - `sw=` and `at=` in `kernel/src/ai/prefill_perf_logic.rs`.

## What the PL checks (put this in the hand-off)

The Zenbook's internal disk is never written; it's behind VMD and isn't even visible. Unplug the USB mouse; use the
touchpad.

1. **Banner (if it shows).** Early in boot the console prints
   `build … | debug-assertions=on overflow-checks=on ub-checks=off`. If you catch it, photograph it. It's optional;
   the log check above covers it.
2. **Box wait.** Click the Arthur bar and ask **"What's 2+2?"**. Expect Arthur's box in well under a second (A7: about
   2 s).
3. **Stopwatch, Enter to last word.** Start when you press Enter and stop at the last word of the answer. This settles
   A7's question: the overlay said 8.4 s of compute against a stopwatch feel of ~2 s.
4. **Finger on the pad for the whole answer.** Keep moving your finger from Enter until the answer is finished. Say
   whether the pointer stays smooth, stutters, or freezes. If it moves, the overlay's `presents=` should be above 0.
5. **Photograph the whole yellow overlay** after the answer, all its lines. What we read from it:
   - `start`: `arthur=… vocab=… model=…`. The arthur→model gap should be a few ticks now;
   - `perf`: prefill and decode ms per token against A7's 213 and 319, plus the new `sw=` and `at=` shares. These
     decide whether levers 1c and 2 get built;
   - `time`: `pre=`, `dec=`, `presents=`, `gap=`;
   - `cpu`: about 3,300 MHz with `hwp=on`.
6. **Typing and touchpad**, as before: press a few keys during the boot screens, then type at the desktop at once.
   With your finger off the pad for ~10 s, the `tp:` line's `loops=` stays the same.

## Close condition

- Commit `Plans/RETURN-2026-10-26-zenbook-image-a8-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
