<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: BUILD-8.1 close-gate — run the now-unblocked QEMU gate → the 8.1 coder

**Date:** 2026-06-15
**From:** PM (Claude)
**Branch:** `codex/phase-8-build-8.1-decode-loop-janus` (already built: the `generate` module + sampler + PCG32, host KATs **5/5** green at `8141078`).
**Status:** **UNBLOCKED — close it.** 8.1 was held on one thing: its QEMU `ai_generate_decode` scenario couldn't run because `/home/neo/limine` was missing in the runner-UID container. The CI-greening fix solved exactly that — **Limine now lives at `/opt/limine` with `LIMINE_DIR=/opt/limine`** (the same fix that greened CI #173). So the in-kernel gate can finally run.

## What this build is (recap, for the close)
The autoregressive decode loop + sampler — a new `generate` module driving `exec::run` per token (recompute-context, no KV cache; that's later), greedy/temperature/top-k/top-p on a seeded PCG32. Token-IDs in/out, f32, additive + feature-gated (default-off ⇒ bit-for-bit today). Host-tier already proven: sampler KATs, the PCG32-vs-published-sequence, last-position logits slice, and the **tiny hand-laid `.neom` greedy byte-oracle** — all 5/5 (PM-verified at merge-prep earlier).

## Close-gate tasks
1. **Rebase onto current `origin/main`.** The branch is based on `1bf1327`; main has moved a long way since (BUILD-LO, the daemon-race fix, the self-hosted-CI migration + `/opt/limine`, W2-prep). **Reconcile `kernel/src/test_harness.rs`** — BUILD-LO (the latency-oracle scenario) **and** the W1 driver both added entries to the same `SCENARIO`/mutual-exclusion/dispatch lists; 8.1's `ai_generate_decode` scenario slots into those same lists. It's a trivial additive reconcile, but do it cleanly (don't drop a sibling scenario).
2. **Re-run the host KATs** post-rebase: `cargo test -p ai-ops-tests generate_` → expect **5/5** (sampler modes, PCG32 sequence, the tiny-`.neom` greedy oracle, last-position slice, recompute-context loop).
3. **Run the QEMU gate — the thing that was blocked:** the `neo-test-ai-generate-decode` scenario at **NEO_SMP=4**, with **`LIMINE_DIR=/opt/limine`** (the new location; that's what unblocks it). This proves the **in-kernel** decode loop — `generate.rs` driving the real `exec::run` per token and reproducing the tiny-`.neom` greedy token sequence on the kernel path (not just the host-logic tier). This is 8.1's conclusive gate.
4. **Confirm the family stays green** with the feature off (kill-switch: `ai-generate` disabled ⇒ bit-for-bit) and on; `cargo fmt`/`clippy -D warnings` clean.
5. **Push the rebased + gated branch.** (Then PM `pm-merge-check` → `--ff-only` → `main`; the push-to-`main` runs the now-live QEMU family on the self-hosted runner, re-confirming `ai_generate_decode` + no regression. CI is a real gate again — let it run, no `[skip ci]` on this code push.)

## Scope guard
Just close 8.1 — the rebase reconcile + the QEMU gate run. **Do NOT** add the KV cache, tokenizer, or quantization (those are 8.2–8.5); **do NOT** touch `exec::run` or the governance hot-path (additive `generate` module only — surfaces disjoint from BUILD-LO/W1). If the rebased `test_harness.rs` conflict is non-trivial or the QEMU scenario fails for a *non-Limine* reason, BLOCKER-to-PM with the serial log rather than forcing it.

## On merge (PM)
Verify host KATs 5/5 + the QEMU `ai_generate_decode` green → `--ff-only` → ledger (ANSPEC §11.1 + ADR-053 are already in the branch). Then **BUILD-8.2 (the GGUF importer + BPE tokenizer)** is next — its dispatch is ready to write the moment 8.1 lands (grounded in the gap-analysis FINDINGS; the GGUF-importer format decision is ratified; note the corrected key-detail that the gap analysis surfaced for the *driver*, not 8.2).

---

## Relay — what to tell the 8.1 coder
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-15-phase-8-build-8.1-qemu-gate-close.md`
  ```
  git fetch origin
  git show origin/main:Plans/DISPATCH-2026-06-15-phase-8-build-8.1-qemu-gate-close.md
  ```
- **One-line:** 8.1 is built (host KATs 5/5); it was only held on the QEMU gate because Limine was missing — **that's fixed (`/opt/limine`, `LIMINE_DIR=/opt/limine`).** Rebase `codex/phase-8-build-8.1-decode-loop-janus` onto current `origin/main` (**reconcile `test_harness.rs` — BUILD-LO + W1 added scenarios to the same lists; add `ai_generate_decode` alongside, drop nothing**), re-run `cargo test -p ai-ops-tests generate_` (5/5), then run the **`neo-test-ai-generate-decode` QEMU scenario at NEO_SMP=4 with `LIMINE_DIR=/opt/limine`** — the in-kernel decode-loop proof that was blocked. Family green feature-off+on, fmt/clippy clean, **push (let CI run — no `[skip ci]`)**. PM merges on green; then 8.2 (GGUF importer).
