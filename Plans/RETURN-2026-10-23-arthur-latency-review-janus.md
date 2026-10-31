<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Arthur latency review — Janus

Reviewed PR #105 at **b619e769cc4f3f5a13c385e6ef501447689e04d9**, against
`Plans/DISPATCH-2026-10-23-arthur-latency-review.md` at that SHA.
Base: `5e40fd31b66b6de3ec2c0eb1e4f8ae18621d108e`.
Branch: `codex/arthur-latency-review-janus`.

**Result: 18 PASS, 4 FAIL (8, 10, 18, 19).** Items 8 and 10 are the same
cross-CPU cache assumption under two configurations. They describe controlled
asymmetric CPU/boot-state inputs, not a reproduced fault on the Zenbook.
Item 18 is a demonstrated host-test coverage gap, not evidence that the current
shared layer loop is wrong. Item 19 fails the literal no-lock contract at
reporting time; the timing accumulators themselves are lock-free.

## Checklist

1. **PASS** — `tools/xtask/src/main.rs:4889` is the only `Some` override.
   `BuildOptions::default()` at `:4797` leaves it `None`, the desktop literal
   explicitly uses `None` at `:15693`, and `build_member` forwards `None` at
   `:5889`. The existing Qwen3.5 label-specific opt-2 exception at `:5920`
   predates this change; this PR preserves it.
2. **PASS** — `tools/xtask/src/main.rs:4898` and `:6183` use the same builder;
   `:4875` clones the list at `:4841`. The nine feature strings, in order,
   match the deleted inline list in parent `281da6ad`'s `cmd_iso_with_model`.
3. **PASS** — `tools/xtask/src/main.rs:4901` accepts only `[]` or the singleton
   `--perf-probes`; duplicates, unknown flags and extra positional arguments
   are rejected. `:4879` appends exactly `prefill-perf` when true.
4. **PASS** — `tools/xtask/src/main.rs:5905` constructs a fresh `Command`;
   `:5924` sets only that child's `CARGO_PROFILE_DEV_OPT_LEVEL`. It does not
   mutate the parent environment or set overflow/debug-assertion overrides.
   `Cargo.toml:88` retains the dev profile (only panic is overridden there).
5. **PASS** — `.github/workflows/ci.yml:658` runs the requested blocking
   `cargo run -p xtask -- image-kernel-build` step, using the same builder.
6. **PASS** — `kernel/src/ai/ops/qk_matmul.rs:210` initializes the atomic to
   undecided; `:1049` falls through to `runtime_available_uncached()` until
   initialized. The CPUID/XGETBV body at `:1058` matches the base body.
7. **PASS** — `kernel/src/ai/ops/qk_matmul.rs:222` obtains the BSP's uncached
   answer and `:225` stores precisely YES or NO from that answer.
8. **FAIL** — `kernel/src/main.rs:790` precedes `:851`; BSP initialization is
   at `:557`, AP initialization at `kernel/src/arch/x86_64/smp.rs:242`, before
   polling via `kernel/src/sched/scheduler.rs:496`. This ordering is correct,
   including for an AP that finishes after the readiness timeout. However,
   ordering does not establish equal XCR0: `kernel/src/arch/x86_64/fpu.rs:167`
   chooses support independently on each CPU; `:177` returns without writing
   XCR0 on fallback. No AP capability agreement is checked before publishing
   the BSP-only global YES at `kernel/src/ai/ops/qk_matmul.rs:225`.
   Concrete asymmetric input and execution sequence: F1 below.
9. **PASS** — In a non-test kernel without `phase9-avx2-matmul`,
   `kernel/src/ai/ops/qk_matmul.rs:223` selects false; the AVX2 module and
   dispatch arms are absent (e.g. `:127`, `:1024`). AutoAvx2 therefore uses
   scalar. The `cfg(test)` exception intentionally keeps host KATs available.
10. **FAIL** — The BSP still uses the old check at
    `kernel/src/ai/ops/qk_matmul.rs:1058`, but the cached YES at `:1052` also
    applies to APs whose old local check would say NO. With `xsave-fpu` off,
    `kernel/src/arch/x86_64/mod.rs:159` and `:169` compile out the XCR0 setup;
    this code does not normalize or verify inherited per-CPU XCR0/OSXSAVE.
    Concrete FXSAVE configuration: F1 below. No new YES is introduced on the
    initializing BSP itself; the failure concerns another executing CPU.
11. **PASS** — `kernel/src/main.rs:849` has exactly the requested feature gate,
    and `:851` follows `bring_up_aps` at `:790`. There is no other kernel boot
    call (host tests and the host bench have their own explicit calls).
12. **PASS** — `tools/ai-ops-tests/src/lib.rs:8761` checks cached/uncached
    agreement and `:8793` checks Q4_K AutoAvx2/Scalar bit identity.
    `.github/workflows/ci.yml:532` and `scripts/worker-close-gate.sh:83` enable
    the real-quant/AVX2 KAT feature combination. This KAT passed locally.
    Its comment mentions Q8_0 too, but this particular test only performs the
    Q4_K dot comparison; other existing AVX2 KATs remain in the suite.
13. **PASS** — `kernel/src/ai/forward_logic.rs:793` iterates once per token,
    with `last = idx + 1 == prompt.len()` at `:794`. The empty input never
    invokes the closure and returns the initially empty vector at `:799`.
14. **PASS** — `kernel/src/ai/qwen_inventory.rs:185` delegates to the same
    `advance_inner` with false. `:197` captures the same position; `:208` and
    `:220` pass the same retained state through walker/non-walker paths.
    Both complete their layer loops at `:299`/`:314` before the early return
    at `:346`, preceding output_norm/output.weight at `:349`. Successful
    calls increment position once at `:243`, exactly as `advance` does.
15. **PASS** — `kernel/src/ai/qwen_inventory.rs:172` always requests logits;
    `:359` computes the head and `:374` rejects nonfinite logits before
    returning `Some`. Decode uses `advance` at
    `kernel/src/arch/x86_64/syscall.rs:3377`; final prefill does so at `:3301`.
16. **PASS** — `kernel/src/arch/x86_64/syscall.rs:3348` and `:3356` use
    `prefill_persistent`, including the negative control before corruption.
    Baseline prefill/feed at `:3363`/`:3379` are unchanged. Its wrapper in
    `kernel/src/ai/qwen_inventory.rs:73` still calls `advance` for every token.
17. **PASS** — `kernel/src/ai/qwen_inventory.rs:237` tests `Result::is_ok()`;
    `Ok(None)` still calls `record_forward` at `:238`, so successful skipped
    heads count one forward per token. The nearby “produced logits” comment
    at `:231` is now stale; the executable condition is correct.
18. **FAIL** — `tools/ai-ops-tests/src/lib.rs:8723`, `:8728`, and `:8738`
    assert the requested counts/logits/next-layer output, but both branches
    use the test's own `layer_stack` at `:8676`. The skip branch at `:8714`
    never invokes production `advance_without_logits`. The host crate mounts
    `forward_logic` at `:31`, but does not mount `qwen_inventory`. Concrete
    mutation F2 below was executed and the KAT remained green.
19. **FAIL** — Compilation gates are correct at
    `kernel/src/ai/mod.rs:105`, `forward_logic.rs:1083`,
    `qwen_inventory.rs:294`, `par_matmul.rs:414`,
    `arch/x86_64/syscall.rs:3432` and `watchdog.rs:284` (paths under
    `kernel/src/` except the first). The accumulators at
    `kernel/src/ai/prefill_perf.rs:44` only use relaxed atomics and do not
    alter computed forward values. However, the generation-end probe at
    `kernel/src/arch/x86_64/syscall.rs:3481` calls `log_generation`, whose
    serial adapter at `kernel/src/ai/prefill_perf.rs:142` reaches
    `SERIAL.lock()` at `kernel/src/drivers/console/serial.rs:116`.
    Concrete input F3 below. The all-probes “no lock” claim is too broad.
20. **PASS** — `kernel/src/ai/par_matmul.rs:414` and `:501` bracket the
    existing barrier. The diff adds only these two gated statements. Loads at
    `:436`, `:437`, steal CAS at `:453`, done store at `:464`, fail-stop panic
    at `:484`, pending clear at `:496`, and busy release at `:503` retain their
    order and behavior. `prefill_perf.rs:91` only adds relaxed counters.
21. **PASS (reported identity evidence)** — PR #105's Identity table at review
    time names base `5e40fd31` and head `b619e769`: default/product/boot-eth0
    identical; desktop/image A/exact ISO set changed by fixes 2–3. It explicitly
    records all six sets identical at probe-only `28545b71`. The commit diff
    agrees with the split: that commit adds only probe instrumentation; the
    cache and head-skip arrive in `281da6ad`, build override in `b619e769`.
    Source anchors: `Plans/NOTE-2026-10-23-arthur-prefill-latency.md:178`
    (fixes as built), `Plans/DISPATCH-2026-10-23-arthur-latency-review.md:88`
    (identity criterion). The table itself is PR metadata, not a file with a
    line number: <https://github.com/SecondStageTurbine/LogansOS/pull/105>.
    I compared the claims and commit split; I did not independently rebuild
    the six ELF pairs or claim fresh hash reproduction.
22. **PASS (default images)** — `kernel/Cargo.toml:219` declares a default-off
    feature; no preset enables it. The product list at
    `tools/xtask/src/main.rs:4841` omits it; only the explicit measurement
    flag adds it at `:4880`. An explicitly requested probe image naturally
    contains it, as allowed by the second sentence of the criterion.

## Concrete failing inputs / sequences

### F1 — A BSP answer does not prove an AP answer (items 8 and 10)

For item 8, use two virtual CPUs with independently controlled CPUID state.
BSP: leaf 1 XSAVE/AVX present, leaf D supports mask 7, leaf 7 AVX2 present.
AP: SSE2 present but XSAVE/AVX absent, initial XCR0=1. Enable
`xsave-fpu,phase9-avx2-matmul,threaded-matmul`. BSP writes XCR0=7; AP takes
`prepare_cr4_for_xsave`'s fallback and never writes XCR0. It can still reach
the AP scheduler and poll. The BSP caches YES. Publish a Q4_K matmul with
at least 512 output columns so it partitions; when that AP claims a slot,
the global cached YES selects AVX2 on a CPU whose local old check says NO.
The resulting unsupported instruction can fault. Even if normal deployment
CPUs are homogeneous, this input is not rejected by the shown boot code.

For item 10, build with `--no-default-features` and explicitly select
`phase9-avx2-matmul,threaded-matmul`, leaving `xsave-fpu` off. Use two AVX2
CPUs handed off with CR4.OSXSAVE set, but BSP XCR0=7 and AP XCR0=3. Without
the feature, `enable_sse` leaves those XCR0 values alone. Cache YES on BSP,
then execute the same partition sequence on the AP. Its old local check
would reject XCR0=3, whereas the new shared cache returns YES.

These are source-derived counterexamples with specified CPU/boot inputs;
neither was executed on hardware or an asymmetric-CPUID VM in this review.
The ordinary product path on CPUs with matching XSAVE support writes mask 7
on all CPUs before they poll; I found no later XCR0 write in this tree.
Acceptance needs an enforced uniform-capability/state precondition, an
all-worker safe decision, or a local fallback check where uniformity is absent.

### F2 — A broken production skip method leaves the host KAT green (item 18)

Temporarily replace the body expression at
`kernel/src/ai/qwen_inventory.rs:185` with:

```rust
self.position += 1;
Ok(())
```

This skips every layer and every K/V write. Run:

```text
cargo test -p ai-ops-tests --features phase9-real-quant,phase9-avx2-matmul --release --locked --offline prefill_lm_head_skip_is_bit_identical
```

**Observed:** 1 passed, 0 failed (257 filtered out), with the existing prompt
`[151644, 8948, 198]` and next token `872`. Cargo does not rebuild this
unmounted production module. The production source was restored byte for
byte immediately afterward, verified with `git diff --exit-code`.
The QEMU equivalence scenario is a separate useful check; it does not make
this host KAT sensitive to production layer/cache regressions.

### F3 — Successful generation takes the serial lock (item 19)

Build the product kernel with `--perf-probes`, then perform a valid inventory
generation with a nonempty prompt and `max_new=1` (for example prompt token
`198` on the admitted Qwen model). On successful completion it reaches
`log_generation` and `serial::_print`, which disables interrupts and takes
`SERIAL.lock()`. No contention is needed to show the contract violation.
This is a source-traced call path, not a reproduced deadlock. Either scope
the no-lock contract to counter collection or change the reporting path.

## Validation and boundaries

- Ran pinned nightly `2026-04-16`, native Windows host:
  `cargo test -p ai-ops-tests --features phase9-real-quant,phase9-avx2-matmul --release --locked --offline`.
  **258 unit tests and 21 integration tests passed; 0 doc tests.** Both named
  latency KATs passed. Tests requiring optional real-model fixtures are not
  evidence of real-model execution merely because their wrappers return OK.
- Ran the F2 negative mutation as described above; it remained green.
- Used direct installed toolchain binaries, workspace `.cargo-home`, and
  workspace target/TEMP/TMP directories. No image, installer, partitioning,
  flashing, raw disk operation, QEMU boot, or machine-internal-disk write was
  requested by this review's commands.
- Ran `cargo clean --target-dir target/arthur-review`: removed 278 files,
  43.4 MiB. Existing unrelated target artifacts were not part of this build.
- PR body/head read through GitHub: head matched the pin. No fresh kernel
  build, silicon timing, six-set identity rebuild, or four-shard gate claimed.
- Existing `.gitignore` modification and untracked `.ignore` were preserved
  and excluded from the review commit. No production code change is retained.
