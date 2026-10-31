<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #72): Zenbook image A8 from main — Janus

Source: `8773020065f6909de01f6d800ce645d87bf0e8d2`.
Branch: `codex/zenbook-image-a8-main-janus`. Image build only; no review model or source changes.

**Verdict: PASS.** Clean build exited 0; all three required build-log checks passed.

## Build

Moved `target/b1-model` aside, ran `docker compose run --rm neo cargo clean`
(3,202 files, 2.1 GiB removed), then restored the directory before building:

```text
docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom
```

No extra flags; `NEO_IMAGE_UB_CHECKS` was not set. Complete local build log:
`target/b1-model/A8-main-87730200-build.log`.

Required log lines (the vocab line is wrapped here only for readability):

```text
[xtask] product image kernel: opt-level=3 ub-checks=off (NEO_IMAGE_UB_CHECKS=on keeps them)
[xtask] staged target/b1-model/model-qwen2.5-0.5b.vocab as NEOVOCB1 /vocab/model-qwen2.5-0.5b.vocb (5010512 bytes, blake3 7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015)
[xtask] ISO ready: /workspace/boot/neoplutarch.iso
```

The staged vocab's independently computed BLAKE3 matches `QWEN25_05B_VOCB`.
The build reports the MT7601U vendor firmware blob absent; that Wi-Fi lane fails
closed at runtime. This does not block the requested ISO build.

## Artifact verification

| Artifact | Bytes | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/A8-main-87730200.iso` | 635,273,216 | `0dae4e38b18722cdca53c63282f15db8100f90b0c00b8fce7ef7113dda651871` |
| `target/iso-root/boot/kernel` | 32,902,968 | `05fa14d74d47cf23b83539dce56faf40122feeb06dfdd3ad75b68b3f94da0af5` |
| `target/iso-root/vocab/model-qwen2.5-0.5b.vocb` | 5,010,512 | `7973d7392444d3faea8d558105c9722c9106483a5a44410fa27fc1c4e6cee015` |

Retained A6 (634,310,656 bytes), A6-hwp (634,312,704 bytes), and A7
(641,482,752 bytes) under their existing names in `target/b1-model`.
Copied `boot/neoplutarch.iso` to the A8 path above and verified both BLAKE3 hashes match.
Build-log markers occur at lines 269 (UB checks), 643–644 (vocab/pin), and 779 (ISO ready).

## Source markers

- `kernel/src/main.rs:593`: `debug-assertions={} overflow-checks={} ub-checks={}`.
- `userland/neo-arthur/src/main.rs:2755`: ` format=neovocb1`.
- `tools/xtask/src/main.rs:5038`: `PRODUCT_IMAGE_UB_CHECKS_OFF`, `-Zub-checks=no`.
- `kernel/src/ai/prefill_perf_logic.rs:62`: `sw={}% at={}%`.
- `kernel/src/boot_model_logic.rs:80`: `QWEN25_05B_VOCB`.

## PL hand-off

Write only the **KINGSTON** stick using Rufus DD mode or Etcher as administrator.
Never select an internal disk. The Zenbook's internal disk remains unwritten;
it is behind VMD and is not visible to this kernel. Unplug the USB mouse and use the touchpad.

1. If visible, photograph the optional early banner: `debug-assertions=on overflow-checks=on ub-checks=off`.
2. Click the Arthur bar and ask **"What's 2+2?"**; measure the box wait (expected well under one second; A7 about two seconds).
3. Stopwatch from Enter to the last answer word; compare with the overlay's compute time.
4. Move a finger on the touchpad throughout the answer; report smoothness, stutter, or freeze. If it moves, check `presents>0`.
5. Photograph the entire yellow overlay: start's `arthur/vocab/model`, perf's prefill/decode ms per token and `sw/at`, time's `pre/dec/presents/gap`, and CPU MHz with `hwp=on`. A7 baselines: 213 ms/token prefill, 319 ms/token decode, about 3,300 MHz.
6. Press keys during boot and type immediately at the desktop. Leave the pad untouched for about ten seconds and confirm `tp: loops=` stays unchanged.

## Environment

RAG health was healthy (`ollama=true`, `qdrant=true`); no semantic search was needed.
Pre-existing `.gitignore` and `.ignore` edits were preserved in the named stash
`relay-72-preserve-preexisting-ignore-changes` for restoration after both relays.
