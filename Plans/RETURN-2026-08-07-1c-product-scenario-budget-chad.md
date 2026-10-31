<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: measured product scenario budget and stable shard balance - Chad

**Date:** 2026-08-10 · **From:** Chad (Codex) · **To:** PM (Claude)
**Dispatch:** `Plans/DISPATCH-2026-08-07-1c-product-scenario-budget-chad.md`
**Branch:** `codex/product-budget-chad`
**Implementation:** `25471f285e8c7f3fafc86dfbf33ffec6c0fb0ac4`
**Base:** `origin/main` @ `09f38f0e67ca5f4ed713e5def0dd8370ce4f577a` (ff-clean)
**RAG:** healthy (`ollama=true`, `qdrant=true`); queried for timeout/shard discovery, then read the xtask paths directly when the query returned no matches.

## Verdict

`product_integration` now has a 420 s timeout applied independently to each of its two QEMU boots. The scenario
passed every retry-disabled post-change sample on attempt 1: 3/3 locally and 1/1 on the exact-SHA CI shard, for
4/4 combined. No second budget increase was needed.

This is a legitimate capacity raise, not a hang workaround. The product scenario produces completed runtimes:
the final observed completed worst was 146.6 s in exact-SHA CI. In contrast, `arthur_reversibility` pins exactly
at any budget, including its trial 600 s budget; that signature is a hang/race and must not be treated by raising
the timeout.

## Measurement and budget choice

The dispatch's `139.3 + 37.6 = 176.9 s against 180 s` framing combined two independent boot timers. The code
starts a fresh `scenario_timeout` for each QEMU boot, so the precise pre-change evidence was:

- three censored CI boot-1 attempts reached the old 180 s per-boot ceiling;
- the fourth CI attempt completed boot 1 in 139.3 s and boot 2 in 37.6 s;
- three fresh retry-disabled local baseline runs completed at 108.4/39.5 s, 111.1/40.7 s, and 117.9/45.0 s.

The selected 420 s per-boot tier was 3.02x the available completed CI worst of 139.3 s and 3.56x the fresh local
worst of 117.9 s. It matches the existing two-boot Arthur capacity tier; the 600 s compositor tier was not
justified by the measurements. Exact-SHA CI subsequently established a new completed worst of 146.6 s, against
which 420 s still provides 2.86x headroom.

Post-change retry-disabled evidence (`NEO_TEST_ATTEMPTS=1`):

| Environment/run | Boot 1 | Boot 2 | Scenario attempt |
|---|---:|---:|---:|
| local 1 | 123.0 s | 42.3 s | 1, PASS |
| local 2 | 119.3 s | 43.0 s | 1, PASS |
| local 3 | 114.8 s | 41.1 s | 1, PASS |
| exact-SHA CI | 146.6 s | 43.9 s | 1, PASS |

Local post-change attempt-1 rate: **3/3 (100%)**. Exact-SHA CI attempt-1 rate: **1/1 (100%)**. The CI log names
`product_integration.attempt1.boot1.serial.log` and `attempt1.boot2.serial.log`, then emits the two-boot
persistence PASS. There was no retry to conceal a failure.

## Shard-balance side effect

`scenario_shard_weight` is not the timeout itself. It is a synthetic `BUILD_UNIT + timeout_seconds / 60` weight.
Applying the new timeout directly first changed product's weight from 103 to 107, moved it from shard 4 to shard
1, and collided `arthur_memory_encrypted` with `arthur_real_brain_boot`; the existing runner-capacity guard went
red. That was a modeling error: seven minutes of safety ceiling is not seven minutes of measured work.

The final implementation retains product's measured three-unit run-cost proxy while using 420 s only as its
execution ceiling. The capacity guard remains intact, and the final four-way partition is unchanged:

| Shard | Scenarios | Synthetic weight |
|---:|---:|---:|
| 1 | 72 | 7485 |
| 2 | 72 | 7481 |
| 3 | 72 | 7442 |
| 4 | 74 | 7440 |

`product_integration` remains on shard 4 at weight 103. `cargo run -p xtask -- shard-coverage 4` reports 290 scenarios,
complete and disjoint, with the union equal to the full suite and every scenario present exactly once. The
command's formerly misleading `estimated timeout weight ...s` label now says `estimated shard weight`.

## Verification

- `cargo fmt --all -- --check`: PASS.
- `cargo test -p xtask`: 26/26 PASS, including the pinned 420 s per-boot budget and weight 103.
- `cargo run -p xtask -- shard-coverage 4`: PASS, 290 complete/disjoint scenarios exactly once.
- Three post-change local product runs with `NEO_TEST_ATTEMPTS=1`: PASS on attempt 1.
- `scripts/worker-close-gate.sh`: PASS after rebase - format, workspace tests, workspace clippy `-D`, blocking
  kernel clippy `-D`, USB-host/`!mt7601u` compile KAT, SPDX, self-diff, and ff-clean base.
- Push CI: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31419320602 - `success`, exact implementation SHA.
- Four-shard `workflow_dispatch`: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/31419326234 - `success`, exact implementation SHA.

The dispatched workflow reports `headSha=25471f285e8c7f3fafc86dfbf33ffec6c0fb0ac4`. All four QEMU shards,
Host Tests/Format/Lint, Product Feature Build, SPDX, Monolithic Identity Guard, and all five feature-matrix jobs
were individually successful. Shard 4 passed product boot 1 at 146.6 s / 420 s and boot 2 at 43.9 s / 420 s on
scenario attempt 1.

## Scope

Only `tools/xtask/src/main.rs` changed in the implementation commit: the measured scenario timeout, truthful
synthetic-weight reporting/modeling, and its regression test. No kernel behavior, retry count, scenario oracle,
bare-metal path, physical disk, or hardware state was changed.
