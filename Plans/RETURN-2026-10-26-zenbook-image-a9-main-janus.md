<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #84): Zenbook image A9 from main — Janus

Source: `149431a185ebd8c5d3fe0a0bd3539a3e72bc38e1`.
Branch: `codex/zenbook-image-a9-main-janus`. Image build only; no review model or source changes.

**Build verdict: PASS.** Clean build exited 0; required log lines, artifact pins, real build id, licence, and source markers passed. **USB write pending:** no KINGSTON stick is attached to this build host. No physical disk was written and no Zenbook boot was performed.

## Build

Moved `target/b1-model` aside within this worktree, ran `docker compose run --rm neo cargo clean` (4,489 files, 2.5 GiB removed), then restored it. The model, vocab and old ISOs survived.

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

No extra flags; `NEO_IMAGE_UB_CHECKS` was not set. Complete local log: `target/b1-model/A9-main-149431a1-build.log`.
Required log lines (vocab wrapping joined here):

```text
[xtask] product image kernel: opt-level=3 ub-checks=off (NEO_IMAGE_UB_CHECKS=on keeps them)
[xtask] staged target/b1-model/model-qwen2.5-0.5b.vocab as NEOVOCB1 /vocab/model-qwen2.5-0.5b.vocb (5010512 bytes, blake3 7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015)
[xtask] ISO ready: /workspace/boot/neoplutarch.iso
```

These occur at log lines 240, 614–615, and 696. The build noted the MT7601U firmware blob is absent; that Wi-Fi lane fails closed at runtime.

## Artifacts and checks

ISO: `E:\AI Automation\workers\janus\target\b1-model\A9-main-149431a1.iso`.

| Artifact | Bytes | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/A9-main-149431a1.iso` | 635,901,952 | `7e990c2f2a1b181f59cf064f18c582e33a3618c219dbd859866fcdb994cb7bed` |
| `target/iso-root/boot/kernel` | 32,894,592 | `288f19ae1ce67930333b9f25b54e1facc83c973f7739948f699a0bc365dcd0b9` |
| `target/iso-root/vocab/model-qwen2.5-0.5b.vocb` | 5,010,512 | `7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015` |

Independently hashed with `b3sum`. The copied ISO and `boot/neoplutarch.iso` match. Staged vocab matches `QWEN25_05B_VOCB` (`kernel/src/boot_model_logic.rs:80`).

**Real build id: PASS.** Extracting printable bytes from the staged kernel found:

```text
D  build 149431a185ebd8c5d3fe0a0bd3539a3e72bc38e1 | debug-assertions=?
```

The assertions value is formatted at runtime; the embedded commit is real. There are zero `unknown-build-id` occurrences in the staged ELF.

**Inter licence: PASS.** `target/iso-root/licenses/OFL-Inter.txt` exists, 4,380 bytes. `target/iso-root/licenses/NOTICE:61` names `/licenses/OFL-Inter.txt` (also named in source `NOTICE:61`).

Source markers:

- `tools/xtask/src/main.rs:4970`: `PRODUCT_IMAGE_KERNEL_FEATURES`; `:5010` includes `par-elementwise`; `:5015` documents the omitted `boot-evidence-holds`, which is absent from the feature list.
- `userland/neo-text/src/fonts.rs:10`: `INTER_REGULAR` embeds `Inter-Regular.ttf`.
- `kernel/src/ai/forward_logic.rs:408`: `attention_last`; `:434` calls `ops::gqa_causal_attention_last_strided`.

Retained unchanged names under `target/b1-model`: A6 (634,310,656 bytes), A6-hwp (634,312,704 bytes), A7 (641,482,752 bytes), A8 (635,273,216 bytes).

## PL hand-off

Read-only `Get-Disk` inventory found six internal SATA/NVMe drives (1–2 TB) and no USB disk. The ISO is ready for the **KINGSTON stick only**, using Rufus **DD image mode** or Etcher run as administrator. Verify its identity and approximately 30 GB capacity before writing. **Never select an internal disk.** The Zenbook internal disk is behind VMD and is not visible to this kernel; it must remain unwritten. Unplug the USB mouse and use the touchpad.

1. Stopwatch from selecting USB in the boot menu to desktop. A9 should print USB/touchpad summaries without the old 35–70 s evidence waits.
2. Optional early-banner photo: `build 149431a1… | debug-assertions=on overflow-checks=on ub-checks=off`. The ELF check above already verifies the real commit.
3. Check Inter in Arthur's bar, Home and Lens for clipping, overlap or misalignment.
4. Click Arthur's bar and ask **"What's 2+2?"**. Expect the box well under a second.
5. Stopwatch Enter to last word, against A8's about 1.9 s. Repeat with **"Tell me about the moon."** and record its time.
6. Keep a finger moving on the touchpad throughout the longer answer; report smoothness, stutter or freeze.
7. Photograph the entire yellow overlay after that answer: `perf` prefill/decode ms/token and `sw=`/`at=` (A8: 50/63 ms, 32%/17%); `time` `pre=`, `dec=`, `presents=`, `gap=`, `bar=` (A8: presents 81, bar 10%); `cpu` about 3,300 MHz, `hwp=on`.
8. Type immediately at desktop. Leave the pad untouched about 10 s and confirm `tp: loops=` stays unchanged.

RAG was not used; no health query or service change was needed. All work was performed in the assigned Janus worktree.
