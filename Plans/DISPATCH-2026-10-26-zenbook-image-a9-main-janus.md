<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #84): build image A9 from main, for the Zenbook → Janus

**Type:** image build only. No code change, no review. **Deliverable:** one pushed RETURN with the image's path, size
and BLAKE3, plus the marker checks below.

**Source:** `main` @ `149431a1` (Inter, #145, merged from the batch-2 gate `dbaf245e`, run `40383263248`, green).
Relative to A8 (`87730200`, relay #72), it adds (J.c.: "wait for both levers before building A9"):
- **#140 (lever 1c):** the cached decode step's attention reads the KV cache in place, with no copies.
- **#142, #143 (lever 2):** SwiGLU and the cached attention fork across cores (`par-elementwise`, on in image A).
  Attention forks once `n_q × seq × head_dim ≥ 16,384` (about 16 positions at 0.5B).
- **#141:** image A boots without the bring-up evidence holds (`boot-evidence-holds` off). The USB and touchpad summary
  lines still print, but the boot doesn't wait. QEMU boot-to-desktop went from 72.6 s to 7.4 s.
- **#145:** Arthur's proportional voice is **Inter** (replacing IBM Plex Sans). Plex Mono and the kernel's console font
  are unchanged. The Absent-model line now reads "Boot the LogansOS USB it came with."
- **#138:** a Docker build's kernel embeds the real commit as its build id (A7 and A8 showed `unknown-build-id`). A9 is
  the first image built after this.
- **#137, #144:** the review fixes from relays #70 and #71.

On the Zenbook, A8 showed:
- Enter→answer ≤ 2 s;
- prefill 50 ms/token and decode 63 ms/token;
- `presents=81`;
- `mv=36% sw=32% at=17% bar=10%`.

## Setup

```
WORKTREE: E:\AI Automation\workers\janus     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/zenbook-image-a9-main-janus    (for the RETURN only)
          git fetch origin && git checkout -B codex/zenbook-image-a9-main-janus 149431a185ebd8c5d3fe0a0bd3539a3e72bc38e1

git show origin/main:Plans/DISPATCH-2026-10-26-zenbook-image-a9-main-janus.md   # this file
git show origin/main:docs/runbook-baremetal-zenbook-s14.md                        # the Zenbook runbook
```

RAG (semantic code search) is available; it isn't needed for an image build. If you use it, health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in the RETURN; the PL manages it.

## Build

Run `cargo clean` first, as for A7 and A8, keeping `target/b1-model` aside so the model, the vocab and the old ISOs
survive. Then use the same command as relays #49–#72. Don't pass any flag, and don't set `NEO_IMAGE_UB_CHECKS`.

```
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
#   copy boot/neoplutarch.iso -> target/b1-model/A9-main-149431a1.iso
```

The build log must show these lines (quote them in the RETURN):
- `[xtask] product image kernel: opt-level=3 ub-checks=off (NEO_IMAGE_UB_CHECKS=on keeps them)`;
- the vocab staged as `NEOVOCB1` (5,010,512 bytes, BLAKE3 `7973d739…`, the same line as A8). If the conversion or pin
  fails, the build stops with "no image was written". Report that verbatim; don't work around it;
- `ISO ready`.

Keep the A6, A6-hwp, A7 and A8 ISOs.

## Checks to record in the RETURN

1. The size and BLAKE3 of the ISO and of the staged kernel ELF (`target/iso-root/boot/kernel`), and of the staged
   `.vocb` (5,010,512 bytes, matching `QWEN25_05B_VOCB`).
2. **The build id is real.** `strings target/iso-root/boot/kernel | Select-String "build 149431a1"` (or the Docker
   equivalent) prints the banner line with `149431a185ebd8c5d3fe0a0bd3539a3e72bc38e1`, not `unknown-build-id`. If it
   prints `unknown-build-id`, say so; that's a FAIL of #138 on this host.
3. **Inter's licence is in the image.** `target/iso-root/licenses/OFL-Inter.txt` exists, and `NOTICE` names
   `/licenses/OFL-Inter.txt`. Give its size.
4. These markers exist in the source; cite each `file:line`:
   - `PRODUCT_IMAGE_KERNEL_FEATURES` in `tools/xtask/src/main.rs` lists `par-elementwise` and not `boot-evidence-holds`;
   - `INTER_REGULAR` in `userland/neo-text/src/fonts.rs`;
   - `gqa_causal_attention_last_strided` in `kernel/src/ai/forward_logic.rs` (`attention_last`).

## What the PL checks (put this in the hand-off)

The Zenbook's internal disk is never written; it's behind VMD and isn't even visible. Unplug the USB mouse; use the
touchpad.

1. **Boot to desktop, by stopwatch.** Start when you pick the USB in the boot menu, stop when the desktop appears.
   A8 sat on the USB summary and the touchpad check for up to about 35–70 s; A9 shouldn't wait there at all.
2. **Banner (if it shows).** Early in boot the console prints `build 149431a1… | debug-assertions=on
   overflow-checks=on ub-checks=off`. It should be the commit now, not `unknown-build-id`. It's optional; the RETURN's
   check 2 covers it.
3. **Text in Inter.** Arthur's bar, Home and the Lens use the new face. It has rounder, more open letters than Plex. Say
   whether anything looks clipped, overlapping or misaligned.
4. **Box wait.** Click the Arthur bar and ask **"What's 2+2?"**. Expect Arthur's box in well under a second, as in A8.
5. **Stopwatch, Enter to last word.** Against A8's ~1.9 s. Then ask something with a longer answer, such as
   **"Tell me about the moon."**, and time that too. Levers 1c and 2 matter most on longer answers.
6. **Finger on the pad for the whole answer.** Say whether the pointer stays smooth, stutters, or freezes.
7. **Photograph the whole yellow overlay** after the longer answer, all its lines. What we read from it:
   - `perf`: prefill and decode ms per token against A8's 50 and 63, plus the `sw=` and `at=` shares (A8: 32% and 17%);
   - `time`: `pre=`, `dec=`, `presents=`, `gap=`, and `bar=` against A8's 10%;
   - `cpu`: about 3,300 MHz with `hwp=on`.
8. **Typing and touchpad**, as before: type at the desktop at once, and with your finger off the pad for ~10 s the
   `tp:` line's `loops=` stays the same.

## Close condition

- Commit `Plans/RETURN-2026-10-26-zenbook-image-a9-main-janus.md` on your branch and **push it**; paste the SHA.
- Hand-off: the **KINGSTON** stick only (Rufus DD mode, or Etcher run as administrator). **Never select an internal
  disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
