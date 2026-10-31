<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW: Arthur answer latency — `prefill-perf` probes + fixes 1–3 (cross-family checklist)

**MODEL:** GPT-6-Luna. **EFFORT:** high. The change is contained: inference-path code, build tooling, and
diagnostic counters. It has no new syscall. It doesn't change the fork-join's claim, steal or barrier logic.

**PR:** #105, branch `claude/rtl8125-ethernet` (the name is unrelated to the lane; the session's designated
branch). The author is S4, Claude family. **Pin the head SHA** the PM relays with this file, and review that SHA
only.

**Read at that SHA:**
- `Plans/NOTE-2026-10-23-arthur-prefill-latency.md`: the findings, and "Fixes 1–3 as built";
- `kernel/src/ai/ops/qk_matmul.rs`: `AVX2_DECISION`, `init_avx2_decision`, `avx2::runtime_available{,_uncached}`;
- `kernel/src/main.rs`: the `init_avx2_decision()` call and the `bring_up_aps` call before it;
- `kernel/src/arch/x86_64/mod.rs` `enable_sse`, and `kernel/src/arch/x86_64/fpu.rs` `finish_xsave_init`
  (the XCR0 writes);
- `kernel/src/arch/x86_64/smp.rs` `ap_entry`;
- `kernel/src/ai/forward_logic.rs`: `prefill_last_logits`, and the `prefill-perf` probe in `matmul_vec_weight`;
- `kernel/src/ai/qwen_inventory.rs`: `advance`, `advance_without_logits`, `advance_inner`, `next_logits_for_token`;
- `kernel/src/arch/x86_64/syscall.rs`: `prefill_persistent`, `InventoryLogitsSource::prefill`, and the probes in
  `generate_inventory_greedy`;
- `kernel/src/ai/prefill_perf.rs`, the `prefill-perf` lines in `kernel/src/ai/par_matmul.rs` and
  `kernel/src/watchdog.rs`, and `kernel/Cargo.toml` `prefill-perf`;
- `tools/xtask/src/main.rs`: `PRODUCT_IMAGE_KERNEL_FEATURES`, `PRODUCT_IMAGE_KERNEL_OPT_LEVEL`,
  `build_product_image_kernel`, `cmd_image_kernel_build`, `parse_perf_probes`, `cmd_iso_with_model`,
  `build_member_at_opt_level`;
- `.github/workflows/ci.yml`: the `image-kernel-build` and `ai-ops-tests` steps; `scripts/worker-close-gate.sh`;
- `tools/ai-ops-tests/src/lib.rs`: `prefill_lm_head_skip_is_bit_identical`,
  `avx2_decision_cached_agrees_with_uncached`.

**Answer format:** for each item, PASS or FAIL, with the enforcing `file:line`. For a FAIL, give a concrete input
or sequence that shows it. Push the RETURN as `Plans/RETURN-2026-10-23-arthur-latency-review-<seat>.md`.

## A. Opt-level scope (fix 1)

1. Only `build_product_image_kernel` passes `kernel_opt_level: Some(..)`. Every other `BuildOptions` literal and
   `build_member` caller passes `None`, so scenario, desktop-run and default builds keep the `dev` profile's
   opt-level.
2. `iso --with-model` and `image-kernel-build` both build the kernel through `build_product_image_kernel`, with
   the single `PRODUCT_IMAGE_KERNEL_FEATURES` list. That list equals the pre-change inline list in
   `cmd_iso_with_model` (compare it with the parent commit).
3. `--perf-probes` is the only accepted extra argument, and it adds exactly `prefill-perf`. Without it, the
   feature list is unchanged.
4. The opt-level is applied as `CARGO_PROFILE_DEV_OPT_LEVEL` on that one cargo invocation only. Overflow checks and
   debug assertions stay at the `dev` defaults.
5. CI's `product-build` runs `cargo run -p xtask -- image-kernel-build`, which compiles the same kernel the image
   ships.

## B. AVX2 decision cache (fix 2), including the no-AVX path

6. Before `init_avx2_decision()` runs, `avx2::runtime_available()` returns `runtime_available_uncached()`, the old
   per-call check, unchanged.
7. After it runs, the cached value equals what `runtime_available_uncached()` returned on the BSP at that moment.
8. When the init runs, every online CPU has already written XCR0 the same way: the BSP in `enable_sse`, and each AP
   in `ap_entry` → `enable_sse` before it can poll for work. Name any path where an AP could run a matmul partition
   with an XCR0 different from the BSP's.
9. Without `phase9-avx2-matmul` (the AVX2 module not compiled), `init_avx2_decision()` caches "no", and every
   AutoAvx2 call takes the scalar path, as before.
10. With `xsave-fpu` off (FXSAVE builds), the decision is still the old CPUID/XCR0 check: the cache adds no new
    "yes".
11. `init_avx2_decision` is called only under `any(phase9-quantized-matmul, phase9-real-quant)`, and only after
    `bring_up_aps`.
12. The KAT asserts cached == uncached and AutoAvx2 == Scalar bit for bit on this host. CI runs it (the
    `ai-ops-tests` feature step).

## C. lm_head skip equivalence (fix 3)

13. `prefill_last_logits` calls `step(token, true)` for exactly the last token and `false` for all the others. An
    empty prompt returns an empty vector, matching the old loop.
14. `advance_without_logits` runs the same embedding and layer loop (both the walker and non-walker paths), at the
    same `position`, with the same caches, and increments `position` exactly as `advance` does. It returns before
    `output_norm`/`output.weight`.
15. `advance` still returns the full logits, and the finiteness check, for every call: decode and the last prompt
    token.
16. Both `KvGenMode::Persistent` and `PersistentCorruptedKat` use `prefill_persistent`. `ReprefillBaseline` is
    unchanged, so the equivalence KAT still compares the new prefill against the untouched baseline.
17. `forward-perf`'s `record_forward` still counts one pass per prompt token, so pass counts are unchanged.
18. The host KAT compares the last-position logits bit for bit, the lm_head call count, and the next position's
    layer output (which reads every cached K/V). Check that it would fail if `advance_without_logits` skipped a
    layer or a cache write.

## D. Probe kill-switch identity (`prefill-perf`)

19. Every probe site is `#[cfg(feature = "prefill-perf")]`. The module is not compiled without the feature. The
    probes take no lock, and do not change any value the forward computes.
20. The probe in `par_matmul::run_generic` only reads the TSC before and after the barrier loop and adds to a
    relaxed counter. It does not reorder or remove any `pending`/`done` access, the steal CAS or the fail-stop arm.
21. The PR's identity table shows `.text` identical to `main` with the feature off, for the sets the fixes don't
    touch, and states which sets changed because of fixes 2–3. Compare this with the commit split: the probe-only
    commit `28545b71` was identical for all six sets, including the exact image kernel set.
22. `prefill-perf` is in no image. `iso --with-model` adds it only with `--perf-probes`.

## Round 2: narrow re-check of items 8, 10, 18, 19

**MODEL:** GPT-6.1-Sol. **EFFORT:** medium. A narrow re-check of the reviewer's own earlier findings.

Round 1 (Janus, relay #44) failed items 8, 10, 18 and 19. Re-check only those, at the new head SHA the PM relays.
Each item gets PASS or FAIL with the enforcing `file:line`; give a concrete input or sequence for a FAIL.

**Read at that SHA:** `kernel/src/ai/ops/qk_matmul.rs` (`Avx2Census`, `AVX2_CENSUS`, `local_avx2_check`,
`report_cpu_avx2`, `publish_avx2_decision`, `avx2::runtime_available`); `kernel/src/arch/x86_64/mod.rs`
`enable_sse`; `kernel/src/arch/x86_64/smp.rs` `ap_entry`, `bring_up_aps`, `online_cpu_count`;
`kernel/src/main.rs` (the `publish_avx2_decision` call); `kernel/src/ai/decode_logic.rs`;
`kernel/src/ai/qwen_inventory.rs` (`InventoryDecodeState::step`, `InventoryForward`, `token_layers`,
`token_output`); `kernel/src/arch/x86_64/syscall.rs` `prefill_persistent` and `generate_inventory_greedy`;
`kernel/src/ai/prefill_perf.rs`; `tools/ai-ops-tests/src/lib.rs` (the census KATs and
`prefill_lm_head_skip_is_bit_identical`); `docs/concurrency-pitfalls.md` CP-79.

- **R8.** Every CPU that can run a matmul partition reports its own `local_avx2_check()` after its own XCR0 write
  and before it can poll for work. "Yes" is published only when every online CPU has reported and none said no.
  Name any CPU that could run AVX2 code with its own check saying no.
- **R10.** With `xsave-fpu` off, or on a CPU without XSAVE, that CPU's local check says no, so the decision is
  "no" (or undecided until publish, which runs the local check). The census adds no "yes" the old check would
  not give on every CPU.
- **R18.** The host KAT mounts `kernel/src/ai/decode_logic.rs`, the file whose `DecodeCursor::step` and
  `prefill_last_logits` the kernel prefill runs. Apply your round-1 mutation to it (skip the layers and only
  bump the position when no logits are wanted) and confirm the KAT goes red. Check that nothing between
  `prefill_persistent` and `DecodeCursor::step` branches on `want_logits`.
- **R19.** No `prefill-perf` code takes a lock on the generation path. The report reaches the screen only through
  the watchdog overlay. With the feature off, nothing of it is compiled.
