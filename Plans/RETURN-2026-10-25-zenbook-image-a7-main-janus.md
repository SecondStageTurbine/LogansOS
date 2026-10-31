<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #63): Zenbook image A7 — Janus

**Verdict: PASS — build and requested source checks complete.** No code change or hardware run.
Source: `745dd314dd8e74e60839fb2529d3187222a93f5e` on
`codex/zenbook-image-a7-main-janus`. Read the dispatch and Zenbook runbook from `origin/main`.

## Clean build and artifacts

Ran `docker compose run --rm neo cargo clean` successfully before building (17,700 files, 8.8 GiB removed).
Temporarily moved `target/b1-model` outside `target`, then restored it after clean, preserving the model,
vocab, A6/A6-hwp and earlier images. No old compiled userland objects were retained.

Ran successfully, exit 0:
`docker compose run --rm neo cargo xtask iso --with-model target/b1-model/model-qwen2.5-0.5b.neom`.
Neither `--hwp-performance` nor `--perf-probes` was passed. Log confirms
`[xtask] product image kernel: opt-level=3` and ends with `ISO ready`.
Log: `target/b1-model/A7-main-745dd314-build.log`.

| Artifact (under `E:\AI Automation\workers\janus`) | Bytes | BLAKE3 |
| --- | ---: | --- |
| `target/b1-model/A7-main-745dd314.iso` | 641482752 | `46b536b4793bfbca6200ce75c3dfbe3416c518cbe9af228bf0d1ef0d1d715dc6` |
| `target/iso-root/boot/kernel` | 36156392 | `1c1d44bbf4ccaca8a512c2aafb4f9427a25f40819cc6cd1e4dea5e5785e42d58` |

Copied `boot/neoplutarch.iso` to the named A7 artifact; both ISO BLAKE3 values match.
A6 remains 634310656 bytes; A6-hwp remains 634312704 bytes.
Build notes: MT7601U firmware is absent, so its module is omitted and that Wi-Fi path fails closed.
Cargo emitted profile-package-match notes and compiler warnings; the build completed successfully.
RAG health was healthy (`ollama=true`, `qdrant=true`); no semantic queries were needed.

## Requested source checks (at the source SHA)

All PASS:

- `PRODUCT_IMAGE_KERNEL_FEATURES`, `tools/xtask/src/main.rs:4912`: includes `freeze-watchdog` at 4927,
  `i2c-hid-touchpad` at 4932, `i2c-hid-bar-assign` at 4934, `prefill-perf` at 4947,
  `matmul-steal-grace-time` at 4949, and ends with `tick-sleep` at 4953 and `hwp-performance` at 4959.
- Root `Cargo.toml`: `[profile.dev.package.neo-arthur]` at 132 / `opt-level = 3` at 133;
  `core` at 134 / 135; `alloc` at 136 / 137; `compiler_builtins` at 138 / 139.
- ` parse_ms=`: `userland/neo-arthur/src/main.rs:2728`.
- `vocab_open` / `vocab_closed`: `kernel/src/watchdog_timeline_logic.rs:33` and 34 (format inputs at 72 and 73).
- `ps2={} pre={}`: `kernel/src/watchdog.rs:413`.
- `tp: {} mode={} loops={}`: `kernel/src/drivers/i2c_hid/touchpad.rs:1374`.

## PL hand-off

Write this ISO to the **KINGSTON USB stick only**, using Rufus **DD mode** or Etcher as administrator.
Never select an internal disk. The Zenbook internal disk must never be written; the dispatch identifies
it as behind VMD and invisible to this image. Follow `docs/runbook-baremetal-zenbook-s14.md`.
Unplug the USB mouse and use the touchpad.

1. Ask **"What's 2+2?"** at the Arthur bar. Time the box appearing and the completed answer separately.
   Expected box wait is about 1 s (A6-hwp about 10 s); answer time should remain about 7 s.
2. Move a finger on the touchpad while the answer is written; report smooth movement, stutter, or freeze.
3. Photograph the whole yellow overlay after the answer: the small `vocab=O..C` gap, prefill/decode
   ms per token (A6-hwp 208/304), actual `think=` / `presents=`, and CPU around 3287 MHz with HWP on.
4. Press keys during boot and type immediately at the desktop. Leave the pad untouched for about 10 s
   and check that `tp:` `loops=` stays constant.

## Repository state

Only this RETURN is included in the commit. Pre-existing local changes to `.gitignore` and untracked
`.ignore` were preserved, so `git status --porcelain` cannot be empty without altering unrelated work.
Push/upstream verification is performed after committing this report.
