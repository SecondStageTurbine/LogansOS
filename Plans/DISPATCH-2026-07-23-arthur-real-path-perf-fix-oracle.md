<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur real-path perf measurement repair → Oracle

**Date:** 2026-07-23  
**From:** PM  
**To:** Oracle  
**Priority:** Release-blocking bare-metal follow-up; build now, QEMU gate only after the PM/PL grants a runner slot behind C1.1.  
**Starting lane:** `codex/arthur-perf-lane-fast-forward-oracle` at `e78bf47be7a4113071be0a87b6c7bd498fecd4e3`

## Read first

```powershell
git fetch origin
git show origin/main:Plans/DISPATCH-2026-07-23-arthur-real-path-perf-fix-oracle.md
git show origin/main:Plans/DISPATCH-2026-07-23-baremetal-3-tests.md
git show origin/main:Plans/PM-HANDOFF-2026-07-23.md
```

Use the semantic-search RAG at `http://127.0.0.1:8404` for unfamiliar code paths; the PL owns RAG startup. If it is unavailable, note that once and fall back to exact `rg`/direct reads.

## Finding to fix

TEST 1 proved the complete Slice-B path on silicon, but did **not** produce its required tok/s measurement:

```text
SHOWCASE perf forward_ms=0 tok_per_s=0 threads=12 avx2=true passes=0 total_cycles=0 tsc_hz=2303820860
```

The TSC bracket in `kernel/src/ai/exec.rs` measures graph-backed `exec::run`. Arthur's actual showcase uses:

```text
sys_generate
  → generate_inventory_greedy
  → InventoryLogitsSource::{prefill,feed}
  → qwen_inventory::InventoryDecodeState::advance
  → forward_logic
```

That path never enters `exec::run`. The old premise in `74076acd` was false. QEMU also never exercised the measurement: non-bare-metal `arthur_compositor` performs no real forward, so `passes=0` was accepted until silicon.

The silicon proof itself remains GREEN: physical HID → InputFocus → real 0.5B forward → G1 answer; leak audit clean; threaded and AVX2 kill-switch parity byte-identical; `avx2_available=true`.

## Build scope

1. Instrument the **shared inventory token-position forward** used by both persistent-KV and reprefill generation. Prefill and decode must both count. Preserve graph-backed `exec::run` measurement for `sys_infer_run`; avoid double-counting any path.
2. Make a real inventory-forward QEMU scenario non-vacuously prove the timing bracket. The natural gate is `sys_generate_kv_persist_equivalence`, because it runs the same persistent and reprefill cores with a committed tiny inventory model. It must FAIL if the pass/cycle deltas are zero.
3. In the bare-metal Arthur scenario:
   - `passes=0`, `total_cycles=0`, or `tsc_hz=0` is a terminal FAIL, never an informational success;
   - label provenance explicitly (`tcg_cycles_virtualized` vs `real_silicon`);
   - keep the measured unit honest: token-position forwards/second unless a separately counted generated-token denominator is added.
4. Correct comments/docs that claim every forward funnels through `exec::run`.

**Non-goals:** no forward math, sampling, EOS, cache policy, tokenizer, compositor, threading, or AVX2 behavior change. Do not tune performance in this build; restore trustworthy measurement only.

## PM draft — evidence only, never a merge source

The PM crossed the seat boundary and preserved an untrusted sketch at:

```text
wip/pm-draft-arthur-perf-real-path-fix
6bdd89231dc6bee61ed19029b9ef1e8b07a2a655
```

It compiled and its exact QEMU scenario passed with `inventory_forward_perf=covered passes=71 cycles>0`. Inspect it if useful, but independently re-derive, rewrite/reject as needed, and return **your own branch and commit**. The PM draft must never be merged or cherry-picked as authority.

## Required invariants / ISC

- **Kill-switch:** `forward-perf` OFF is bit-for-bit the prior layer; no TSC read or accumulator update remains.
- **Correctness neutrality:** instrumentation cannot change logits, token IDs, cache state, errors, or output bytes.
- **Completeness:** persistent prefill/feed and reprefill are all covered; graph-backed inference remains covered; no double count.
- **Failure accounting:** state explicitly whether failed forwards count, and make the policy consistent across graph and inventory paths.
- **Concurrency/I-14:** consult `docs/concurrency-pitfalls.md`; justify atomic ordering and overflow behavior. Measurement must add no lock, allocation, or governed-resource lifetime.
- **Honesty:** QEMU values are not silicon magnitudes. The bare-metal line must say `real_silicon`.

## Gates and deliverable

Before RETURN:

1. Rebase fresh onto current `origin/main` while preserving the two original perf-lane commits cleanly.
2. Run `scripts/worker-close-gate.sh`.
3. Run the exact `sys_generate_kv_persist_equivalence` QEMU scenario and show the nonzero coverage breadcrumb.
4. Fire the 4-shard `workflow_dispatch` only after the PM/PL grants a runner slot. All four shards must run and pass on the exact head SHA.
5. Build `arthur_compositor_baremetal`; record ISO size, SHA-256, manifest, source SHA, and a discriminator bank for zero calibration/zero passes/count mismatch.
6. Commit and push everything. **Green without a CI URL is not done.**

RETURN: branch, exact head SHA, own `git show --stat`, close-gate result, exact-scenario evidence, 4-shard CI URL, ISO hash/manifest, kill-switch proof, and the deletable worktree path.

## Silicon close after PM verification

After exact-SHA CI + PM AAG and before claiming the deliverable closed, rerun the Arthur ISO jointly on the OptiPlex. Required evidence: serial + sink agreement; nonzero calibrated `tsc_hz`, passes, cycles, forward-ms and throughput; pass count reconciled to logged prefill/feed operations; `provenance=real_silicon`; unchanged parity and leak-audit PASS.

