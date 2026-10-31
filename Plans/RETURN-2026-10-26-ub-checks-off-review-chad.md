<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #71): product UB checks and relay #68 fixes (#134)

Reviewed SHA: `2c4aeb3195b53acc6df7108b47501d5934f05937`.
Branch: `codex/ub-checks-off-review-chad`.
Overall verdict: **FAIL for a new test-only soft-f32-only compatibility regression**, detailed below. **All six requested production/build/fix-forward checks PASS**, and relay #68 items 4 and 5 are closed for the product configuration. No product arithmetic or build-scope failure found. Review only; no kernel fix or revert committed. The confirmed additional failure is reported to the PL for disposition under the dispatch's revert-first policy.

| Item | Verdict | Evidence at reviewed SHA |
|---|---|---|
| 1. Product-only flag, build-std and existing rustflags | PASS | Only `tools/xtask/src/main.rs:5008` sets kernel_target_rustflags to Some; it belongs to build_product_image_kernel, reached by image-kernel-build at `:5021` and iso --with-model at `:6333`. Default BuildOptions at `:4842` / `:4871`, ordinary build_member at `:6015`, and run-desktop at `:15892` supply None. The child-command environment at `:6063` is target-specific and does not mutate xtask's own process environment, host builds, or later scenario commands. `kernel/.cargo/config.toml:6` enables rebuilds of core/compiler_builtins/alloc; `:14` holds the target link flags. An isolated Cargo probe using the pinned toolchain, copied exact target JSON/config, and the same target environment variable confirmed that core, alloc and the probe all receive -Zub-checks=no **and** all existing PIC/PIE/no-dynamic-linker/page-size flags. CI feature arms invoke cargo clippy directly (`.github/workflows/ci.yml:887`) without this product override, including product-real-lm's feature arm at `:820`. Scope assumes ordinary environment settings, not an external RUSTFLAGS override. |
| 2. Debug and overflow checks retained | PASS | The builder changes only dev opt-level at `tools/xtask/src/main.rs:6061` and UB target flags at `:6076`; it never disables debug assertions or overflow checks. Workspace `Cargo.toml:88` keeps the dev profile and its normal enabled checks. Both off/on variants of the isolated opt-level-3 target probe compiled constant assertions that cfg!(debug_assertions) and cfg!(overflow_checks) are true; verbose rustc commands for core, alloc and the probe show debug-assertions=on. |
| 3. Visible and compiler-derived | PASS | `tools/xtask/src/main.rs:5003` logs the selected ub-checks mode and `:6075` prints the actual added target flags. `kernel/src/main.rs:595` / `:596` / `:597` use compiler cfg!(debug_assertions), cfg!(overflow_checks), cfg!(ub_checks), rather than the environment variable or a hard-coded off label. The probe compiled assertions for ub_checks=false with the flag and true without it. Cargo rebuilt core/alloc with different artifact fingerprints between modes; no stale checks-on core was reused for the off case. No boot-banner execution was performed. External conflicting compiler flags could make the xtask's intended-mode log differ from the result; the kernel banner still reports its actual compiler cfg. |
| 4. Image kill-switch | PASS | `tools/xtask/src/main.rs:4992` reads only NEO_IMAGE_UB_CHECKS, trimmed and compared to lowercase on. At `:5012`, on omits the disabling flag; the repository's ordinary dev configuration then restores checks, as verified by the probe's on variant. Exact repository search found no other reader of that variable. This switch does not override an independently supplied compiler flag explicitly disabling checks. |
| 5. MXCSR coverage and oracle under product configuration | PASS | Every public computational GDN entry checks immediately: `kernel/src/ai/ops/gdn.rs:66`, `:221`, `:285`, `:318`, `:458`, `:483`, `:522`; public KAT helpers delegate to these checked entries. Sampling checks at `kernel/src/ai/generate_logic.rs:591`. `tools/ai-ops-tests/src/mxcsr_entry_check_tests.rs:33` catches panics and restores MXCSR; `:38` disables primitive checks via the cfg(test)-only seam at `soft_f32.rs:123`. Its loop at `mxcsr_entry_check_tests.rs:255` checks 20 cases for no panic at 0x1F80 and an MXCSR panic at 0x5F80. Default run passed. Removing only GDN RMSNorm's entry check made it fail specifically for that operation; source restored byte-for-byte, then the full default suite passed. The standalone non-test #68 probe now returns 0x3f3b26a2 under 0x1F80 and panics at the entry assertion under 0x5F80, exit 101. Additional oracle configuration regression below. |
| 6. Universal display bound and boundary tests | PASS | `kernel/src/ai/prefill_perf_logic.rs:39` bounds each share to 0..=100 even for inconsistent snapshots/zero denominator; `:48` / `:50` cap each count/time field at 9,999,999. `:58` formats seven fields of at most 7 decimal digits and five shares of at most 3 digits. Constant text plus those widths is exactly 141 ASCII bytes, below the 192-byte watchdog capacity. The test at `tools/ai-ops-tests/src/prefill_perf_line_tests.rs:52` covers all-max, my old counterexample and an input attaining the exact 141-byte bound. All four formatter tests pass. Replaying the old non-test probe produces 127 bytes instead of 260, with shares clamped to 100 and counts capped. |

## Additional confirmed failure: oracle rejects the supported integer switch

Command on the unmodified reviewed source:

```text
cargo test -p ai-ops-tests --features soft-f32-only mxcsr_entry_check_tests
```

Result: exit **101**, 0 passed, 1 failed:

```text
ops::add: no MXCSR entry check fired under 0x5F80 (got None)
```

`tools/ai-ops-tests/Cargo.toml:12` exposes the integer-only feature. Under it, `kernel/src/ai/ops/soft_f32.rs:79` makes use_hardware false, and the entry assertion at `:92` is intentionally skipped. That is correct behavior for integer soft_f32 operations and preserves the existing kill-switch contract. The newly added test unconditionally requires an MXCSR panic at `tools/ai-ops-tests/src/mxcsr_entry_check_tests.rs:262`, so it fails a supported host-test configuration even though ops::add behaves correctly. Its module is gated only on cfg(test) (`tools/ai-ops-tests/src/lib.rs:17114`), with no hardware-path feature guard.

This is a test-only regression, not evidence that the product lacks entry checks or that the integer path must assert. The oracle needs to account for soft-f32-only (e.g. gate the hardware-assertion expectation to the hardware configuration). I did not change it in this review branch. I did not run a release-profile test; the test's expectation also depends on debug assertions being enabled, as the guard's own cfg states.

## Local evidence and limits

- `cargo test -p ai-ops-tests`: **277 passed, 1 ignored** unit tests; **21 passed** integration tests. The ignored exhaustive exp test was not run. This successful full run was after restoration of the temporary entry-check removal.
- Separate default MXCSR test: 1 passed; formatter tests: 4 passed.
- Negative control: remove only gdn_rmsnorm_gated's entry assertion, run the MXCSR oracle: exit 101, `gdn::gdn_rmsnorm_gated: no MXCSR entry check fired under 0x5F80 (got None)`. Restored in a finally block and checked against Git.
- Standalone host binary, without cfg(test), compiled with `rustc --edition 2024 -C opt-level=0 -C debug-assertions=yes`: replayed the previous GDN and formatter probes, with corrected outcomes above.
- Isolated build-std probe: `cargo +nightly-2026-04-16 check -Zjson-target-spec --offline -vv` in a temporary no_std library with copied kernel/.cargo/config.toml and x86_64-neoplutarch.json, dev opt-level 3. Off sets CARGO_TARGET_X86_64_NEOPLUTARCH_RUSTFLAGS to `-Zub-checks=no --cfg expect_ub_off`; on omits that variable. Both exit 0. Constants assert debug/overflow checks on and the expected ub_checks cfg. Verbose commands confirm core/alloc/probe all retain the five target options while only off includes -Zub-checks=no. This verifies Cargo propagation using the actual pinned compiler, not a full product image build or link/boot.
- Read the dispatch, #68 RETURN, requested diff, build entry points, target/profile configs, CI arm invocation, assertion test/seam and formatter. RAG health was healthy; exact known paths were read directly. No QEMU, silicon run, or full product image build.

Only this RETURN is committed. Graft cache files were preserved in named stashes during relay #70 and again if needed at close. Final close checks after push: empty `git status --porcelain` and empty `git log '@{u}..'`.
