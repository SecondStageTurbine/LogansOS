<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-34 threaded-matmul — parallel per-token inference across the APs (a faster Arthur) → Oracle

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Oracle (main-tier — kernel/SMP; the OT-34 fix is **pre-inoculated** by Prometheus's CP-19…CP-28 trap-mining + CP-29)
**Status:** DISPATCHED — the first **Milestone-A** step ("JC can sit down and *use* it"). PL framing: **a faster Arthur is a better Arthur.** Kernel-touching SMP change ⇒ mandatory 4-shard `workflow_dispatch` QEMU gate + the concurrency AAG. This is the archetypal SMP-touching dispatch the OT-62 pitfall-bank-consultation rule + the concurrency-AAG exist for.

## The goal
Re-enable **threaded matmul** so a per-token forward pass runs its matmuls **in parallel across the application processors** (the 11 APs on the OptiPlex), turning Arthur's ~4 s/token scalar-single-threaded speed into a multi-core speedup. This is the genuine minutes→seconds lever the fast-gen premise-correction identified as the real bottleneck (per-token matmul, not algorithmic quadratics).

## The bug this fixes (Cerebro reverted it on the bare-metal run)
Enabling threaded-matmul **deadlocked** the 12-core bare-metal scheduler smoke test: the **AP idle-hook parks APs before the cross-CPU scheduling finishes → the run hangs at the spawned hog** (the parallel matmul worker is stranded on an AP that parked before it observed the work; the join barrier never clears). Cerebro tried + reverted it. This is a real scheduler/AP-hand-off race, **not** a matmul-math bug.

## MANDATORY — pitfall-bank consultation (OT-62; this is the pre-inoculation, USE IT)
Before writing code, read `docs/concurrency-pitfalls.md` **CP-19…CP-29** (Prometheus mined CP-19…CP-28 for exactly this class; CP-29 is the AP-init deadlock Cerebro just fixed). For **each** trap your change touches, state in the RETURN: does it **introduce / guard / leave-N-A**, and **which discriminator proves it**. The load-bearing ones for this build:
- **CP-19 (idle loop missing a work-poll hook)** — `idle_thread_entry` (`scheduler.rs:~1249-1283`) has no `ap_poll`; `run_ap_bootstrap` (`:~328-358`) does. **This is the OT-34 direct cause** per the bank: an AP that reaches a park path without polling the published work slot sleeps through the assignment. Guard: the AP must drain the matmul-work-slot poll before any park/halt; if you route APs through a different loop, the poll hook migrates with it (compile-gate it).
- **CP-20 (AP parks before cross-CPU spawn completes)** — **the hang root.** The work-enqueue and the wake must be ordered by a **release-acquire chain** (or the IPI-as-fence, SDM §11.6.1) so the AP cannot wake, see an empty slot under TSO store-buffer reordering, and re-park. State the exact ordering your parallel-matmul dispatch uses.
- **CP-25 / CP-26 (AP busy-yield starves BSP / partition join-barrier races with AP parking)** — the join barrier must use Release/Acquire and be parity-tested; the fork-join must be fail-STOP (the OT-34 F1 precedent), never fall-through-into-freed-state.
- **CP-21 (EOI-only reschedule IPI vs a busy AP)** and **CP-27 (reschedule IPI in per-CPU init)** — confirm not regressed.
- **CP-29 (serialized AP bringup) + I-16 (shootdown-ready mask)** — **do NOT regress these.** The serialized bringup (`smp.rs`) and the `shootdown_ready_cpus_mask` are load-bearing; your parallel-dispatch must not reintroduce concurrent-AP-init contention or shootdown-to-IF=0-AP. (See the CP-29 guard note: serialization covers allocator contention, I-16 covers the IF=0 shootdown — keep both.)
New pitfalls learned on this gate are **added back to the bank** (it grows, never pruned).

## Kill-switch + determinism (Rule-1 — NON-NEGOTIABLE: a faster-but-different Arthur is a BROKEN Arthur)
1. **Kill-switch:** threaded-matmul **OFF** ≡ the current single-threaded scalar path, **bit-for-bit** (it's the default today; keep it the provable disabled mode).
2. **Deterministic parallel reduction:** threaded-matmul **ON** MUST produce **bit-identical token ids** to the single-threaded path — a **deterministic column-partition** (fixed reduction order independent of how threads interleave / which AP finishes first). Floating-point reduction order changes the result; a non-deterministic reduction that "usually matches" is a FAIL.
3. **Armed parity KAT (in-gate):** a hermetic QEMU scenario runs the SAME prompt through single-threaded and threaded matmul under `NEO_SMP` and asserts **identical token ids** (the fast-gen `sys_generate_kv_persist_equivalence` KAT is the shape to mirror — same greedy argmax, exact token compare, hard-fail on divergence). Add a **negative control** (perturb the partition/reduction order → assert divergence) so the parity KAT is provably falsifiable, per the B2b / fast-gen precedent.

## Failure-mode + discriminator bank (SMP; capture-before-you-need-it)
Design for the hang, not just the green:
| # | Failure | Discriminator |
|---|---|---|
| F1 | **AP-parking deadlock** (the reverted bug) | Instrument spawn→work-published→AP-observed→join. A hang localizes to which stage stalled (AP parked before observe = CP-19/CP-20; join never clears = CP-26). Serial breadcrumb at each. |
| F2 | **Non-deterministic result** (threaded ≠ single) | The parity KAT hard-fails with the first-divergence token index → a reduction-order bug, not a deadlock. |
| F3 | **Livelock / BSP starvation** (CP-25) | BSP makes no forward progress while APs busy-yield → bounded-spin warn + the residency/liveness backstop. |
| F4 | **Regressed CP-29/I-16** (concurrent-init contention or IF=0 shootdown returns) | The CP-29 discriminator (serial freeze at `gdt: cpu=N loaded`) or a shootdown-timeout — must NOT reappear. |

## Bare-metal caveat (the CP-29 lesson)
The OT-34 deadlock manifested on **real 12-core silicon**; QEMU's serialized TCG may not fully reproduce a timing-dependent AP-parking race (that is exactly why CP-29 was bare-metal-only). So: **prove what QEMU can** — the threaded≡single **parity** (deterministic, in-gate), no-regression, and no-deadlock under `NEO_SMP=4` (and `NEO_SMP=8` stress per OT-47 if runner capacity allows, drained-box). If the deadlock does not reproduce under QEMU, the fix is designed **from the CP bank** (principled, not trial-and-error) and its **final deadlock-gone proof is a bare-metal SMP run** on the OptiPlex — in which case reference `docs/runbook-baremetal-arthur-optiplex.md` (the standing bare-metal run procedure; the serial log is the proof — **no photo**) and coordinate a joint run with the PL. State in the RETURN whether QEMU reproduced it or a bare-metal run is required to close.

## Scope guard
The parallel matmul + the AP work-dispatch/scheduling fix ONLY. No unrelated scheduler refactor. Do not regress CP-29's serialized bringup or I-16's shootdown-ready mask. The forward math is unchanged (only the reduction is parallelized deterministically) — the parity KAT enforces this.

## AAG (PM, pre-merge — kernel/SMP)
Concurrency lens (the load-bearing one): the work-dispatch ordering (CP-19/CP-20 release-acquire), the join barrier (CP-26), no CP-29/I-16 regression, no new deadlock/livelock. ABI/determinism lens: the parity KAT is genuinely falsifiable (negative control) + the reduction is deterministic. Memory-safety: the parallel workers' shared/owned state, no data race on the accumulation buffers.

## Gate + close
- `scripts/worker-close-gate.sh` (rebase-fresh onto current `origin/main` — it has CP-29 + the consolidation; the full host-check set; SPDX; Step-4 self-stat). Kernel-touching ⇒ the pre-merge 4-shard `workflow_dispatch` QEMU gate is mandatory. **Runner note:** one 4-shard wave at a time on the fleet — if Janus's keyboard-input lane opens concurrently, **stagger** the two QEMU gates. RETURN pastes the green push-CI + 4-shard `workflow_dispatch` URLs on the exact head, the CP-bank consultation table, and (if applicable) the bare-metal validation.
- **Disk reclaim (after PM confirms merge):** delete this build's tree once on `origin/main` + PM-confirmed (never an unpushed tree); `cargo clean` interim.

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-ot34-threaded-matmul-faster-arthur-oracle.md`. **MANDATORY pre-read:** `git show origin/main:docs/concurrency-pitfalls.md` (CP-19…CP-29 — the pre-inoculation Prometheus mined for exactly this) and the CP-29 guard note in `smp.rs`. PM artifacts live on `origin/main`; your tree is on a feature branch — `git show`, not a local open.
- **One-line:** Re-enable **threaded matmul** (parallel per-token inference across the APs = a faster Arthur) and fix the AP-parking deadlock Cerebro reverted (AP idle-hook parks before cross-CPU scheduling finishes → hang at the spawned hog). **Consult CP-19…CP-29 first** and state introduce/guard/N-A per trap. **Rule-1 kill-switch:** threaded-OFF ≡ single-threaded bit-for-bit; threaded-ON must produce **bit-identical token ids** via a **deterministic column-partition reduction** (a non-deterministic "usually matches" reduction is a FAIL) — proven by an **armed, falsifiable** threaded≡single parity KAT in the QEMU SMP gate (mirror the fast-gen `sys_generate_kv_persist_equivalence` shape + a negative control). Do NOT regress CP-29 (serialized bringup) or I-16 (shootdown-ready mask). Design the fix from the bank (principled); if the deadlock is bare-metal-only (CP-29-class), the final proof is a joint bare-metal OptiPlex run (runbook — serial log, no photo). 4-shard QEMU gate + concurrency AAG.
- **RAG:** use it for the scheduler/matmul seams — health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; e.g. `threaded matmul AP dispatch`, `run_ap_bootstrap ap_poll work slot`, `fork join barrier matmul`, `column partition reduction deterministic`, `idle_thread_entry park`. If down, note it + grep/Read (never a blocker; the PL owns RAG startup).
