<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Arthur perf — cover the inventory `sys_generate` forward + non-vacuous coverage gate → Oracle

**Date:** 2026-07-26
**From:** PM (Claude)
**To:** Oracle (perf-lane owner, `forward_perf.rs`)
**Status:** DISPATCHED — **kernel-touching, QEMU-gated (no hardware).** Rebuilds the intent of an orphaned PM-draft (`wip/pm-draft-arthur-perf-real-path-fix` @ `6bdd892`) as a proper coder-owned, gated change. **The draft is design-reference ONLY — do not cherry-pick/merge it (PM-authored, unreviewed); build it yourself, own the authorship, run the full gate. I will delete the draft branch once this dispatch is committed.**

## The real gap (worth fixing)
The `forward-perf` cycle-timing bracket currently wraps only the **graph** forward (`ai::exec::run`, the `sys_infer_run` path). But **inventory-backed `sys_generate`** (persistent-KV and reprefill modes) does **not** enter `exec::run` — it runs `qwen_inventory::InventoryDecodeState::advance` → `next_logits_for_token`. So the real Arthur generation path is **untimed**, and the `kv_persist_equivalence` showcase scenario **never ran a forward under QEMU** — it accepted `passes=0` all the way to silicon (a vacuous perf gate). This is the Arthur-perf bare-metal test's measurement path, so it matters for the honest tok/s read.

## Scope (the draft's intent, built properly)
1. **Bracket the inventory forward.** In `InventoryDecodeState::advance`, wrap the real per-token forward (`next_logits_for_token`, both `ot16-graph-walker` and non-walker cfgs) with `forward_perf::read_tsc()` before / `record_forward(delta)` after, `#[cfg(feature = "forward-perf")]`-gated — so `sys_generate`'s prefill AND decode are both measured. (The two timed paths — `exec::run` and `advance` — are disjoint; no double-count.) Update the `forward_perf.rs` module doc to name both entry points.
2. **Make the coverage non-vacuous.** In `run_sys_generate_kv_persist_equivalence` (gate it on `forward-perf` in `kernel/Cargo.toml`, as the draft does), snapshot `forward_perf::report()` before/after the equivalence check and **`fail("reason=inventory_forward_perf_coverage_vacuous")` if `measured_passes == 0 || measured_cycles == 0`** — the scenario runs the same inventory core as `sys_generate`, so it must exercise the bracket. Emit an `inventory_forward_perf=covered passes=.. cycles=..` breadcrumb.
3. **Honest provenance label** (the draft's `arthur_perf_and_parity` touch): `timing_provenance = "real_silicon"` under `neo-test-bare-metal-mode`, else `"tcg_cycles_virtualized"` — so the tok/s number is never mistaken for a real magnitude under TCG (the OT-7 caveat).

## Standing rules
- **Behavior-preserving except the new measurement + the (previously-absent) coverage assertion.** No change to inference numerics — the equivalence KAT and its negative control must still pass unchanged (kill-switch: `forward-perf` OFF ≡ prior). No new invariant.
- Confirm the `logits?` error-propagation is preserved exactly (the draft moved the `?` after the `record_forward` — make sure a forward *error* still returns `Err` and doesn't get mis-timed/swallowed).

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`), green 4-shard `workflow_dispatch` URL (the `kv_persist_equivalence` scenario now proves non-vacuous coverage), PM verify + light AAG (behavior-neutral except measurement; error-propagation preserved; kill-switch). Branch `codex/arthur-perf-inventory-coverage-oracle`. Disk-reclaim after merge.

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-arthur-perf-inventory-coverage-oracle.md`. Design-reference (do NOT merge it): `git show origin/wip/pm-draft-arthur-perf-real-path-fix:kernel/src/ai/qwen_inventory.rs` etc. — I'm deleting that branch; rebuild the intent as your own. Files: `kernel/src/ai/{forward_perf.rs,qwen_inventory.rs}`, `kernel/src/test_harness.rs`, `kernel/Cargo.toml`.
- **One-line:** **Extend the `forward-perf` timing bracket to the inventory `sys_generate` path (`InventoryDecodeState::advance` → `next_logits_for_token`, both cfgs, `#[cfg(forward-perf)]`), and make `kv_persist_equivalence` a non-vacuous perf coverage gate** (snapshot `forward_perf::report()` before/after; `fail` if `passes==0 || cycles==0` — the old gate never ran a forward under QEMU and accepted `passes=0` to silicon). Add the `real_silicon` vs `tcg_cycles_virtualized` provenance label. Behavior-preserving except the measurement + the new coverage assert; kill-switch `forward-perf`-OFF ≡ prior; **preserve the `logits?` error path exactly** (a forward error still returns Err, not mis-timed). Close-gate + green 4-shard URL.
- **RAG** (PowerShell): health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; e.g. `forward_perf record_forward read_tsc report`, `InventoryDecodeState advance next_logits_for_token`, `kv_persist_equivalence scenario`. If down, note it + grep/Read.
