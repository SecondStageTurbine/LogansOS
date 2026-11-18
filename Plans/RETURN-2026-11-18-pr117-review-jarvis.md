<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #117 consolidation review — Jarvis

**Reviewed:** `474c19e71e90e61cb4946a4327d6c11f6c620931` (against `fb08219d1`).

## 1. Boot-frame fold

- **PASS** — With `boot-eth0` off, `boot_frame` retains the original status string and drawing path; the only feature-specific additions are cfg'd out (`kernel/src/init/boot_frame.rs:115-124,137-142`). The module is now unconditionally the consolidated file (`kernel/src/init/mod.rs:13`).
- **PASS** — With it on, the status line calls `boot_eth0_status_line()`, which makes the same `write_status_line` call as the deleted fork (`kernel/src/init/boot_frame.rs:118,137-141`; prior fork at the parent commit).
- **PASS** — No code module references `boot_frame_eth0.rs` or its `#[path]`; the only remaining source mention is the explanatory comment (`kernel/src/init/boot_frame.rs:112-113`).

## 2. Tier contract

- **PASS** — `parse_neo_test_args` preserves the old argument loop and returns the parsed struct; help prints and returns `Ok(None)` (`tools/xtask/src/main.rs:11593-11673`).
- **PASS** — `cmd_neo_test` destructures every former parser variable and runs the same validation after parsing (`tools/xtask/src/main.rs:11675-11735`).
- **PASS** — The regression compares no-tier against `--tier full` both before and after shard, keep-going, rerun-timeouts, and named-scenario arguments (`tools/xtask/src/main.rs:30020-30048`).

## 3. Label changes

- **PASS** — The Arthur emitter and xtask's Slice A serial needle both use `model-plus-compositor source=qwen2.5-0.5b` (`userland/neo-arthur/src/main.rs:642-644`; `tools/xtask/src/main.rs:23355-23384`). No Rust scenario or workflow tool retains the old literal.
- **FAIL** — The Slice B runbook still requires the old literal (`docs/runbook-sliceB-arthur-showcase-optiplex.md:141`). Concrete case: run `cargo run -p xtask -- neo-test arthur_compositor`; its runtime breadcrumb is now `SHOWCASE footprint discriminator=model-plus-compositor source=qwen2.5-0.5b`, so the documented step-12 evidence check looking for `model-485MB-plus-compositor` does not match. The related THINK record also retains the old literal (`docs/THINK-2026-07-21-integrated-showcase-arthur-compositor.md:56`).

## 4. Rustup pin

- **PASS** — All nine fallbacks changed consistently: two in `ci.yml` and seven in `ot61-closure.yml`; none of the old `sh.rustup.rs` bootstrap pipelines remain in those workflows (`.github/workflows/ci.yml:986-992,1265-1271`; `.github/workflows/ot61-closure.yml:92-98,521-527`).
- **PASS** — The checksum check is a failing command in each job's default Linux Bash `run` step, so a mismatch fails the step (`.github/workflows/ci.yml:980-992`; `.github/workflows/ot61-closure.yml:86-98`). I verified the pinned digest against the downloaded official 1.29.3 `x86_64-unknown-linux-gnu/rustup-init`: `b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55`.
- **PASS** — Direct invocation retains the old arguments exactly: `-y --default-toolchain none --profile minimal --no-modify-path` (`.github/workflows/ci.yml:992`; `.github/workflows/ot61-closure.yml:98`).

## 5. Ledger re-measure

- **PASS** — The new figures follow the stated same-method census: the static-lock definition and exclusions are specified at `docs/complexity-governance.md:93-96`, and the pure-code categories and physical-line basis at `104-108`; the updated results are at `9-18`. The ratios round correctly (8,123/16,908 = 48.0%; 13,133/21,918 = 59.9%).
- **PASS** — Spot-checked three rows against live source: I-3's coherence/reservation discussion (`kernel/src/ai/evict.rs:158-178`), I-15's attenuation function (`kernel/src/sched/file_cap_logic.rs:193-218`), and I-16's ready-CPU mask in `shootdown` (`kernel/src/mm/tlb_shootdown.rs:240-249`).

**Review result:** one documentation mismatch in item 3; other requested checks pass.
