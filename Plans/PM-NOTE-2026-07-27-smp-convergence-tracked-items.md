<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM NOTE 2026-07-27 — tracked items from the C1.1 / 2(b) / WiFi / Slice-4 convergence

Durable follow-ons and process changes surfaced this session. Not funded mid-flight (current lanes finish first); recorded so they don't evaporate.

## Gates verified green (exact SHA, independent CI re-verify)
- WiFi `cea89e6a` (run 30232311619), Slice-4 EU `0eae1fc9` (run 30232776508), 2(b) `b405a65a` (run 30237924062) — all `workflow_dispatch` 4-shard success. **All merges HOLD** (WiFi→LINK-UP+I-14 AAG; Slice-4→M710q boot+DMA-fence AAG; 2(b)→after C1.1+cross-family AAG; C1.1→check-7 not green).

## Process changes (adopt now)
1. **Dispatches allocate the CP (concurrency-pitfall) number.** Stacked branches silently collided: Oracle's C1.1 fix and Cerebro's 2(b) both authored "CP-37"; Cerebro renumbered his to CP-38. **CP-37 = non-IRQ-safe spinlock taken from both IF=1 and IF=0 (same-CPU self-deadlock); CP-38 = running-sibling retirement on forced termination.** The `docs/concurrency-pitfalls.md` merge from C1.1 + 2(b) must land BOTH without clobber. Going forward the dispatch names the CP number.

## Strategic lanes (flagged, not yet funded)
2. **Cross-CPU user-thread placement.** Measured fact (Oracle's C1.1 instrumentation): ALL user threads live on cpu0 (768 ticks cpu0, 0 on cpu1-3; 0 IPIs ever). Our scheduler is not placing user threads on the APs, so "SMP" is not exercising true cross-CPU user scheduling — this is why 2(b)'s negative control is non-deterministic (~1-in-3), and it likely masks real cross-CPU bugs. Likely the next big SMP lane after the current C-ladder work. A prerequisite for a deterministically-gated 2(b) negative control.
3. **Hermeticity de-flake sweep.** `lo_isc1_decide_is_o_devices` is a load-sensitive perf assertion that flakes when the runner is busy (passes locally, main green, green on re-fire) — a gate-invariant violation (add484c: no host-variable inputs). Same class as `arthur_reversibility`. Sweep load-sensitive perf assertions: make load-independent or reclassify out of the merge-blocking tier.

## Accepted residuals (2(b), PL/PM-ratified 2026-07-27)
4. **2(b) negative control ships ignored, not gated** — teeth as captured frame-audit evidence (+0 with 2(b) vs +141 leak panic without). Deterministic gating needs item #2 (cross-CPU placement).
5. **2(b) ring-3-spinner residual** — a pure ring-3 thread that never syscalls depends on the retire-IPI alone; the bounded drain is a small remote window. Closure (a timer-tick kill-flag hook) declined to avoid destabilizing the timer ISR's no-pushes-before-call asm-alignment invariant. The retire-IPI bounds it (not unbounded). The cross-family AAG must confirm the window is truly bounded (no interleaving where the spinner outruns the IPI); revisit the timer hook only if it isn't.

## Queued verification (PM)
- **2(b) cross-family AAG** (Codex/Qwen on IPI + return-boundary ordering + the #5 residual) — at 2(b) merge, after C1.1.
- **WiFi I-14 containment AAG** + **Slice-4 DMA-fence/mem-safety AAG** — at their respective merges, after the bare-metal proofs.
