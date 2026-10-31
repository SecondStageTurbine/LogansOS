<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur perf `tok_per_s=0` truncation — fix + host KAT → Dexter

**Date:** 2026-07-27
**From:** PM (Claude)
**To:** Dexter (DeepSeek — on trial)
**Status:** DISPATCHED — bounded, self-contained, **host-KAT-scoreable** (the correctness proof is a host unit test, not a QEMU run). Low blast radius. The scoreable core is host-only; the kernel wiring is trivial.

## The defect (from tonight's bare-metal Arthur PASS)
The Arthur bare-metal serial reported a real, coherent LLM answer with:
`SHOWCASE perf forward_ms=4428 tok_per_s=0 threads=12 avx2=true passes=31 total_cycles=316269292046 tsc_hz=2303841730 provenance=real_silicon`
`tok_per_s=0` is **integer truncation** — the real rate is sub-1 (~0.23 passes/sec at `forward_ms=4428`), and integer division floors it to `0`, so the headline throughput number is **lost** on every sub-1-tok/s run (exactly the regime a CPU-only 0.5B model lives in). `forward_ms` preserves the datum, but the `tok_per_s` field is useless as reported.

## Scope
1. **Factor the rate computation into a pure, host-testable function.** The tok/s (or pass/s) rate is computed from `passes`/`forward_ms` (and/or `total_cycles`/`tsc_hz`). Extract that arithmetic into a pure `fn` (no I/O, no `no_std` obstacle to a host `#[test]`) so it can be unit-tested directly on the host.
2. **Report sub-1 rates without truncation.** Emit a **milli-tok/s** integer (`tok_per_s_milli`, rate × 1000) *or* a fixed-point form (e.g. `tok_per_s=0.226`) — pick one, justify it, keep it parseable. A rate of ~0.226 must render as `226` (milli) or `0.226`, never `0`. Preserve the existing fields; add/replace only the truncating one.
3. **Host KAT (the scoreable deliverable).** A host unit test asserting the pure function on known inputs — e.g. `forward_ms=4428, passes=1 → 226 milli-tok/s` (or the fixed-point equivalent), plus a fast case (`>1 tok/s` still correct) and a guard case (`forward_ms=0` doesn't divide-by-zero). This KAT is the proof the truncation is gone; make it un-gameable (assert the exact expected value from hand arithmetic, not a round-trip of the function under test).
4. **Wire it into the kernel perf line.** Trivial: swap the truncating computation for the pure fn's output in the `SHOWCASE perf …` emitter. The real serial confirmation rides the **next** Arthur bare-metal boot (like the perf-coverage did) — you do not need a bare-metal run to close this.

## Standing rules (trial — get the ritual exactly right)
- **SPDX header** on every new source file (`// SPDX-License-Identifier: MPL-2.0` first line for `.rs`).
- **Close-gate ritual — one command:** `scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`, the full host-check set in CI order, self-stat, evidence block). It is kernel-touching (the emitter), so the **4-shard `workflow_dispatch` QEMU gate** applies — but **stagger it**: fire it only after the higher-priority WiFi/Slice-4 gates clear (ask before firing if runners look busy; the host KAT + push-CI is your primary correctness signal and can land first). **Paste the green CI URL(s)** in your RETURN — "green" without the URL is not done.
- **Kill-switch / invariant:** N/A (behavior-preserving reporting fix; the *number* changes representation, the perf math does not). State that explicitly.
- Branch: `codex/arthur-perf-tokps-fix-dexter`. Disk-reclaim after PM confirms merge.

## Relay — what to tell Dexter
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-27-arthur-perf-tokps-truncation-dexter.md`.
- **One-line:** The Arthur bare-metal perf line reports `tok_per_s=0` — integer truncation of a sub-1 rate (real ~0.226). **Factor the rate math into a pure host-testable fn, report it without truncation (milli-tok/s `226`, or fixed-point `0.226`), and prove it with a host KAT** on hand-computed known inputs (incl. a `forward_ms=0` guard). Then wire the fn into the `SHOWCASE perf …` emitter (trivial; real serial confirmation rides the next Arthur boot). SPDX headers, `scripts/worker-close-gate.sh`, paste the green CI URL; the QEMU gate applies (kernel-touching) but **stagger it behind the WiFi/Slice-4 gates** — the host KAT + push-CI is your primary signal. Behavior-preserving (no kill-switch/invariant needed — say so).
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `SHOWCASE perf tok_per_s forward_ms emit`, `forward_perf tok per s rate compute`, `provenance real_silicon perf line`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
