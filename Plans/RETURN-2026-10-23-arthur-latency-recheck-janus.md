<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Arthur latency re-check — Janus, relay #47

Reviewed **6b9df9513cab88ad1de877077ee80fe3b2e5c5b1**, PR #105, on
`codex/arthur-latency-recheck-janus`. Scope: Round 2 R8/R10/R18/R19 and relay
#47 P1–P4 only. Compared the corrective commits against `b619e769` and read
the round-1 RETURN at `5466f68d`.

**Round-1 findings: R8 CLOSED, R10 CLOSED, R18 CLOSED, R19 CLOSED.**
PM checks: P1 PASS, P2 PASS with the timeout qualification below, P3 PASS,
**P4 FAIL / STILL OPEN**: CP-79 does not name the requested QEMU discriminator.

## Round-2 verdicts

- **R8 — PASS / CLOSED.** Each CPU calls `report_cpu_avx2` at the end of
  `kernel/src/arch/x86_64/mod.rs:207`, after the optional XCR0 setup at `:170`.
  Its local answer is obtained at `kernel/src/ai/ops/qk_matmul.rs:326` and
  reported at `:327`. AP entry calls `enable_sse` at
  `kernel/src/arch/x86_64/smp.rs:248`, before entering the scheduler at `:284`.
  The scheduler only polls for work later, at
  `kernel/src/sched/scheduler.rs:496` (or the shared idle-thread hook at
  `:1611`). `kernel/src/ai/ops/qk_matmul.rs:279` publishes Yes only for an
  exact, nonzero reported count with no veto. A false report atomically sets
  No and veto at `:267`, even after Yes. I found no CPU entry/poll path that
  bypasses its own report, and no later XCR0 write. The kernel does not call
  the host-only shortcut `init_avx2_decision` at `:339`.

- **R10 — PASS / CLOSED for the round-1 “no new Yes” finding.**
  `kernel/src/ai/ops/qk_matmul.rs:313` runs the same per-CPU CPUID/XCR0 check
  at `:1157`: missing OSXSAVE/AVX rejects at `:1159`, missing XMM/YMM XCR0
  bits rejects at `:1167`, and AVX2 is required at `:1172`. Any rejection
  vetoes the global answer. Undecided calls that CPU's old check at `:1152`.
  Thus the former BSP XCR0=7 / AP XCR0=3 case now yields No, as exercised by
  `tools/ai-ops-tests/src/lib.rs:8819`; the missing-XSAVE case is at `:8811`.
  **Wording qualification:** merely compiling with `xsave-fpu` off does not
  force the local check to false. Firmware-inherited OSXSAVE and XCR0=7 can
  still produce true, as before; there is no `xsave-fpu` condition in this
  check. What is now enforced is that the census never promotes a local No
  to Yes. This re-check does not assert that every conceivable FXSAVE boot
  state returns false, or change the pre-existing check's save-state policy.

- **R18 — PASS / CLOSED, mutation verified.** The host mounts the production
  file at `tools/ai-ops-tests/src/lib.rs:23`. Kernel prefill at
  `kernel/src/arch/x86_64/syscall.rs:3299` passes `want_logits` straight to
  `state.step` at `:3300`; `kernel/src/ai/qwen_inventory.rs:203` passes it
  straight to `DecodeCursor::step`. Neither adapter branches on the flag.
  `InventoryForward` has no such flag: its layer/output methods at
  `kernel/src/ai/qwen_inventory.rs:267` and `:284` call the same `token_layers`
  and `token_output`. The sole skip branch is in
  `kernel/src/ai/decode_logic.rs:63`, after the unconditional layer call at
  `:62`. The KAT uses that cursor at `tools/ai-ops-tests/src/lib.rs:8747`,
  checks position at `:8751`, head count at `:8757`, final logits at `:8762`,
  and next decode logits (consuming retained K/V) at `:8773`.
  The requested mutation now goes red; details below.

- **R19 — PASS / CLOSED.** The generation-end `log_generation` call and its
  serial adapter have been removed. `kernel/src/arch/x86_64/syscall.rs:3476`
  returns the generation result without a perf report. Its remaining gated
  perf calls at `:3429`, `:3435`, and `:3469` only update counters.
  `kernel/src/ai/prefill_perf.rs:44` uses relaxed atomic additions;
  `:49` resets relaxed diagnostic counters, including the atomic-only
  `par_matmul::reset_counters` at `kernel/src/ai/par_matmul.rs:128`.
  The only `write_line` caller is the gated watchdog overlay at
  `kernel/src/watchdog.rs:284`, formatting into its stack `Line` before
  painting. The module gate at `kernel/src/ai/mod.rs:113` and the call-site
  gates remove this instrumentation when the feature is off. No added perf
  generation path reaches the serial lock or changes a forward value.

## PM checks

- **P1 — PASS.** `kernel/src/arch/x86_64/smp.rs:66` uses Limine's count,
  capped at MAX_CPUS. With two counted CPUs but only the BSP reporting true,
  `Avx2Census::publish(2)` cannot satisfy the equality at
  `kernel/src/ai/ops/qk_matmul.rs:279`; the initial state remains Undecided.
  `:1152` then checks locally on every call. A false report instead produces
  No immediately, which is also safe. The short-count/zero-count KAT is
  `tools/ai-ops-tests/src/lib.rs:8826`. Skipped/unstarted APs cannot run work;
  an AP arriving later reports before polling. There is no timeout-to-Yes.

- **P2 — PASS for publication/report ordering, with a timeout qualification.**
  `kernel/src/main.rs:790` brings up APs before publishing at `:852`.
  The per-AP acquire wait is `kernel/src/arch/x86_64/smp.rs:199`; the backstop
  acquire wait is `:222`. Normal readiness is signaled at
  `kernel/src/sched/scheduler.rs:478`, after AP `enable_sse`/report, before
  polling at `:496`. **These waits are bounded:** `smp.rs:203` and `:233`
  permit degraded return, so “every AP is past enable_sse when bring_up_aps
  returns” is not an unconditional guarantee. P1's exact census count,
  rather than that assumption, supplies safety in the degraded case. No AP
  can poll/run a partition before its own report, regardless of timeout.

- **P3 — PASS.** All changes to the combined census word go through
  `kernel/src/ai/ops/qk_matmul.rs:255`, an AcqRel `fetch_update` with Acquire
  failure ordering. Its closure may retry; each completed update is one
  successful atomic read-modify-write, not separate decision/count stores.
  A veto sets both veto and No at `:267`. Publish only changes an undecided,
  unvetoed word at `:279`, so a veto before publication blocks Yes, and a
  veto after publication replaces Yes with No. Later true reports preserve
  the low bits, and later publishes cannot revive Yes. Count packing at
  `:264`/`:269` cannot carry into the low decision/veto bits: shifting left
  32 always clears those bits. Strictly, this is not a saturating 32-bit
  count: after 2^32 reports its high field would wrap to zero. Kernel boot
  reports once per CPU, far below that bound; no count overflow is reachable
  through the reviewed boot call sites. The concurrent-veto KAT at
  `tools/ai-ops-tests/src/lib.rs:8847` passed (200 rounds, eight threads).

- **P4 — FAIL / STILL OPEN (documentation).**
  `docs/concurrency-pitfalls.md:3571` accurately describes the trap and
  `:3579` describes the census guard. However, its entire discriminator at
  `:3590`–`:3593` lists only three host KATs. It does not name
  `threaded_matmul_parity`, `qemu64`, `-cpu max`, or the expected No/Yes
  breadcrumbs required by P4. Re-checked the complete CP-79 section and its
  summary row at `:125`; the QEMU discriminator is absent from both.
  **Concrete sequence:** read CP-79 beginning at `:3569` through the end of
  the file and follow its discriminator: it directs only host tests and
  provides no requested QEMU path. The scenario does exist at
  `tools/xtask/src/main.rs:3086` and its implementation at
  `kernel/src/test_harness.rs:23024`. A corrected entry should specify the
  two CPU models and expected census states, with AVX2 kernels explicitly
  compiled: `kernel/Cargo.toml:1003`'s scenario feature itself does not
  enable `phase9-avx2-matmul`. Extra scenario features are supported at
  `tools/xtask/src/main.rs:4925`. Without that feature, even `-cpu max`
  correctly cannot yield an AVX2 Yes. No QEMU runs are claimed here.

## Executed validation

RAG health returned **healthy**, `ollama=true`, `qdrant=true`. Exact symbols
and the pinned files were then read directly; discovery did not substitute
for inspecting the census or decode implementation.

Using the installed nightly `2026-04-16` binaries, with CARGO_HOME, target,
TEMP and TMP under this Janus worktree:

```text
cargo test -p ai-ops-tests --features phase9-real-quant,phase9-avx2-matmul --release --locked --offline
```

**Unmodified result: 260 unit tests and 21 integration tests passed; zero
doc tests.** This includes the three census KATs and the prefill KAT. It does
not establish real-model, silicon, or asymmetric-VM execution.

For R18, temporarily inserted this immediately before
`kernel/src/ai/decode_logic.rs:62`:

```rust
if !want_logits {
    self.position += 1;
    return Ok(None);
}
```

Ran the same command filtered to `prefill_lm_head_skip_is_bit_identical`.
**Observed: exit 101, 0 passed / 1 failed**, at
`tools/ai-ops-tests/src/lib.rs:8697`, `Err(CachePosition)`: the last prompt
token at position 2 finds that positions 0 and 1 never wrote their caches.
This is the requested round-1 mutation, now detected by the mounted
production step. Restored the file byte for byte in `finally` and verified
`git diff --exit-code -- kernel/src/ai/decode_logic.rs`.

Ran `cargo clean --target-dir target/arthur-recheck`: removed 278 files,
43.6 MiB. No retained code changes, image builds, machine disk operations,
or Zenbook writes. The pre-existing local `.gitignore`, `.ignore`, and graft
cache were preserved in a named Git stash, `janus pre-existing graft local
state before Arthur latency recheck close`, to meet the clean-worktree close
condition without committing or discarding them. Only this RETURN is in the
review commit.
