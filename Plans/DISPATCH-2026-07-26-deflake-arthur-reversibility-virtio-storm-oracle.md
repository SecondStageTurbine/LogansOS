<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Deflake `arthur_reversibility` — kill the virtio-blk serial storm that times out the gate → Oracle

**Date:** 2026-07-26
**From:** PM (Claude)
**To:** Oracle (free after arthur-perf; knows the arthur/inference area)
**Status:** DISPATCHED — **kernel/xtask-touching, QEMU-gated. THE KEYSTONE — this flake taxes EVERY 4-shard gate, and is currently the sole red blocking the WiFi `f4a0940` merge.** Prometheus classified it PREEXISTING-FLAKE (independently PM-confirmed: `f4a0940` never touches the scenario; artifacts byte-identical). Fix the flake at the source so all gates run clean.

## Root cause (Prometheus's diagnosis, PM-accepted)
`arthur_reversibility` normally passes in ~11-15s, but its 4 built-in 420s attempts can **all** time out when a **virtio-blk completion storm** floods QEMU. The NeoFS writeback kthread races arthur's I/O path; the completion queue floods, **each completion spams the serial console**, and QEMU's single-threaded emulation crawls (serial I/O is slow in TCG). Observed on the comparator SHA across ryzen-3/4/5 — it is infrastructure, not candidate code.

## Scope — deflake, do NOT paper over a real hang
1. **Cut the serial storm (the highest-leverage lever).** The per-completion `virtio-blk: completion` serial prints are what slow QEMU to a crawl (flooding the slow TCG serial path). **Gate/rate-limit/remove that per-completion serial output** so a completion storm doesn't self-inflict the slowdown — keep it behind a debug flag or a bounded counter, not one line per completion. This alone likely removes most of the slowdown.
2. **Right-size the timeout as a backstop.** After #1, measure the worst-case storm duration and set the `arthur_reversibility | arthur_multistep_agentic` budget (`tools/xtask/src/main.rs:759`) above it with margin (only that line — do not blanket-bump the other 420s scenarios). If #1 makes the scenario reliably ~15s, a modest headroom is enough; if the storm still adds real time, size for it. **Do not set it so high a genuine hang wastes 4×budget** — the goal is "survives the storm," not "hides a hang."
3. **Prove the deflake.** The bar is **reliable green**: re-run `arthur_reversibility` (and `arthur_multistep_agentic`) green across **multiple invocations / a full 4-shard gate** with no 420s (or new-budget) timeout. If you can reproduce the storm locally, show it no longer times out post-fix.

## NOT in scope (tracked follow-ups, not this lane)
- The deeper NeoFS-writeback-vs-arthur-I/O **race** itself (the true root cause of the storm) — investigate separately; this lane makes the gate reliable, it does not have to eliminate the race.
- Do not change arthur's agentic logic or the reversibility semantics — behavior-preserving except the serial-output rate-limit + the timeout constant.

## Standing rules
- Behavior-preserving (no inference/agentic-logic change; only serial-output volume + the timeout backstop). No new invariant. Kill-switch: the debug serial output can be re-enabled behind a flag.
- **Promotion (escaped-defect §4):** this IS the promotion of the escaped/flaky scenario — a flake that reddens the gate gets fixed so it can't recur, not waved through per-run.

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh on `origin/main`), green 4-shard `workflow_dispatch` URL **with the reliability evidence** (multiple green arthur_reversibility runs / no timeout), PM verify + light AAG (behavior-preserving; the serial change doesn't alter I/O correctness, only its logging; timeout is a test-harness constant). Branch `codex/deflake-arthur-reversibility-oracle`. **This unblocks the WiFi `f4a0940` re-fire and every future gate — top priority.** Disk-reclaim after merge.

## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-26-deflake-arthur-reversibility-virtio-storm-oracle.md` + Prometheus's report (the virtio-blk-storm diagnosis). Files: the virtio-blk completion serial path (`kernel/src/drivers/block/` — grep `virtio-blk: completion`), `tools/xtask/src/main.rs:759`.
- **One-line:** **Deflake `arthur_reversibility` — it's the sole red blocking WiFi `f4a0940` and taxes every gate.** Root cause (Prometheus): a virtio-blk completion storm spams the QEMU serial console, slowing single-threaded TCG until the 420s budget times out (normal pass ~11-15s). Fix: **rate-limit/gate the per-completion `virtio-blk: completion` serial output** (the storm's self-inflicted slowdown), then right-size ONLY the `arthur_reversibility|arthur_multistep_agentic` timeout (`xtask:759`) above the worst-case storm as a backstop — not so high a real hang wastes 4×budget. Prove **reliable green** across multiple runs / a full 4-shard gate, no timeout. Behavior-preserving (serial volume + timeout constant only; no arthur-logic change); the deeper NeoFS-writeback race is a separate tracked follow-up. Close-gate + green 4-shard URL with reliability evidence.
- **RAG** (PowerShell): health-check first; e.g. `virtio-blk completion serial_println writeback`, `NeoFS writeback kthread flush`, `scenario_timeout arthur_reversibility xtask`. If down, note it + grep/Read.
