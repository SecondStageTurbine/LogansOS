<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-P4b — the async residency daemon (the kernel manages memory while it works) → Jarvis

**Date:** 2026-06-10
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `1f9d8f5`: `codex/phase-7-build-P4b-residency-daemon-jarvis`)
**Status:** DISPATCHED. **QEMU NEO_SMP stress loop — policy consumer of the proven P4a primitive; the concurrency mechanism is already race-proven, but the daemon adds its own race surface (teardown, daemon-vs-pass victim contention, watermark oscillation) so the gate is again a stress loop with discriminators.**
**Builds on:** P4a (`9599438`, the migration-reservation primitive, §9.12, ADR-039 — gate green 60× NEO_SMP=4) + P3 (§9.11 — the structural predictor, headroom target H, retain/cold-demote) + P2 (§9.10 reactive eviction) + kthreads (`sched/kthread.rs`) + the documented lock order (`sync::lock_order`).
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md` §9 (PL ratified Q1–Q5, 2026-06-10). **Three pre-recorded P4a→P4b seams (from ADR-039/code — all three are in scope):** (1) daemon teardown must **wake-not-kill** parked waiters; (2) daemon surfaces use **TensorSlot**; (3) **H is pass-global** — P4b must make the watermark per-device (or explicitly justify keeping it global in the ADR).

## 0. The decision this build makes
P4a proved a tensor can move while the system races to use it. P4b puts that primitive to work: a **dedicated kernel thread** (the kswapd/DAMON model) that, *concurrently with live dispatch*, **bg-demotes** cold tensors when a device's free headroom falls below a low watermark and **prefetches** predicted-hot operands ahead of need — so the executor's critical path stops paying for reclaim. The trace gains the two words P1–P3 never had:
```
residency: tensor=4 device=Accel(1) → cpu  reason=bg-demote  (daemon, resident=240K/256K < low-watermark, while busy)
residency: tensor=9 cpu → device=Accel(1)  reason=prefetch   (daemon, predicted next-use, headroom ok)
```
**Strictly additive (Q5):** the synchronous P2/P3 path is the always-correct **direct-reclaim backstop** — if the daemon is asleep, behind, or disabled, every placement still succeeds exactly as today. P4b can make the critical path cheaper; it can never make anything fail.

## 1. The policy (minimal, watermark-driven)
- **Watermarks (per-device — seam 3):** from the device capacity `C` and the P3 headroom target `H`: **low = free < H** wakes the daemon; **high = free ≥ 2H** (or `C/4`, whichever is smaller — pick and justify in the ADR) puts it back to sleep. Hysteresis is the anti-thrash governor: the daemon never oscillates on the boundary. Replace P3's pass-global `H` with the per-device figure (or document why global is still right).
- **Daemon loop:** woken (by a placement that finds free < low, or by its own periodic tick) → for each capacity-bounded device below high: claim-and-demote the **coldest unreferenced** tensors via the **P4a reservation** (claim skips anything referenced/contended — never blocks, never touches what a pass holds) until high watermark or nothing claimable; then prefetch (below).
- **Prefetch (bounded, behind the thrash guard):** when a pass has published its upcoming-ops window (the P3 structural predictor, `W=1`), the daemon may stage a predicted-hot CPU-resident operand to the target device **into fresh storage behind the reservation** — only if free stays ≥ low watermark after the transfer (the §9.11 thrash guard: a prefetch must never trigger reclaim). A misprediction (op never consumes it) is rolled back by the daemon (demote it; byte-preserving) and **counted**.
- **Direct-reclaim backstop (Q5):** the placement path is unchanged — if room is short at placement time, synchronous P2/P3 reclaim runs exactly as today (the daemon merely makes that case rare). The daemon and a synchronous reclaim racing for the same victim is **already safe by construction**: both go through the P4a claim, the loser skips.
- **Teardown (seam 1):** daemon shutdown (and any test-harness teardown) must **commit-or-rollback in-flight reservations and wake every parked waiter** before the thread exits — wake-not-kill. A parked accessor outliving the daemon is a lost-wakeup bug.

## 2. ISCs
- **ISC-1 — The daemon (kthread).** A dedicated kernel thread (existing `sched/kthread.rs` substrate) with the watermark wake/sleep protocol, walking capacity-bounded devices, demoting via the **P4a TensorSlot reservation only** (seam 2 — no parallel mutation path, no direct `Arc` surgery). Park/wake for the daemon itself uses the proven wake-token primitive; lock acquisition respects `sync::lock_order` (the lockdep is already in the build — it will catch you).
- **ISC-2 — Per-device watermarks (seam 3).** Low/high per device from `capacity_bytes()` + the headroom figure; the placement path signals the daemon at low. Host-testable as a pure function (`watermark_state(capacity, free, h) → {Sleep, Reclaim}` with hysteresis asserted: no single-byte oscillation).
- **ISC-3 — Prefetch behind the reservation (bounded).** Stage into fresh device storage via the reservation; commit makes the tensor device-resident; misprediction → daemon rollback (demote), **byte-preserving and counted**. The thrash-guard precondition (never reclaim to prefetch) is a hard gate in code, not a heuristic.
- **ISC-4 — Teardown wake-not-kill (seam 1, REQUIRED).** Daemon stop = drain: finish/rollback in-flight reservations, wake all parked waiters, then exit. Assert in the gate: after daemon stop, parked-count == 0 (the P4a quiesce hook) and every reservation is `Resident`.
- **ISC-5 — Strictly additive (Q5, REQUIRED).** Daemon disabled (boot flag / not started) → every scenario behaves exactly as P3 (bit-for-bit, the P1–P3 regressions prove it). Daemon enabled → no placement ever *fails* that would have succeeded; the backstop path still runs when the daemon is behind.
- **ISC-6 — Result-invariance under the daemon (REQUIRED).** The forward-pass output is **byte-identical** with the daemon running vs disabled — across the whole stress loop. One divergent iteration fails the gate.
- **ISC-7 — The decision trace + the gate (QEMU NEO_SMP stress loop).** The `residency: … daemon …` traces above. **Gate scenario:** a multi-tenant memory-pressure forward pass (the P2/P3 pressure workload shape) run **concurrently with the daemon**, **N ≥ 50 iterations at NEO_SMP=4**. Assert every iteration: (a) byte-identical output vs the daemon-disabled reference (ISC-6); (b) residency ≤ capacity throughout; (c) **critical-path reclaim reduced** — synchronous evictions/demotions inside `ensure_room` strictly lower than the P3 baseline on the same workload (the daemon banked the room ahead of the placement); (d) prefetch misprediction count bounded and every rollback byte-preserving; (e) teardown quiesce (ISC-4); (f) no lost wakeup / no deadlock (the P4a discriminators stay armed: generation counter, parked-count, owner-uniqueness, per-migration delta, lockdep); (g) P1–P4a regressions green at NEO_SMP=4.
- **ISC-8 — Standards.** SPDX; **no new `unsafe`**; `cargo fmt`; host suite (watermark + daemon-policy unit tests + regressions) green; `shard-coverage`; `clippy -D warnings`; builds on+off (and with the daemon compiled out or not started — ISC-5's disabled mode must be a supported configuration). Extend **ANSPEC-001 §9.12** with the daemon subsection (watermarks, the demote/prefetch loop, the backstop, teardown, the trace) + **ADR-040**.

## 3. Gate (two-stage — QEMU NEO_SMP stress loop, no OptiPlex)
**Expected-failure-mode analysis (the daemon's own race surface — the P4a matrix stays armed; these are the *new* modes):**
| # | Failure mode | Discriminator |
|---|---|---|
| 1 | **Teardown leak** — daemon exits leaving a parked waiter or a `Migrating` reservation | the quiesce hook + reservation-state sweep at stop (ISC-4); a watchdog bounds any park |
| 2 | **Daemon-vs-backstop victim race** — both reclaim the same tensor | safe by claim/skip (P4a owner-uniqueness counts it); assert successful double-claims == 0 |
| 3 | **Watermark oscillation** — demote→prefetch→demote ping-pong | per-device demote/prefetch counters per iteration; assert bounded (hysteresis working), prefetch-of-a-just-demoted-tensor == 0 within an iteration |
| 4 | **Prefetch-induced pressure** — prefetch causes the reclaim it was meant to avoid | the thrash-guard hard gate + assert no synchronous eviction immediately follows a prefetch commit on the same device |
| 5 | **Daemon starvation / priority inversion** — daemon never scheduled under load, gate "passes" vacuously | assert the daemon actually ran (bg-demote count > 0 across the loop) and the critical-path reduction (c) is non-vacuous |
| 6 | **Lost wakeup on the daemon itself** — placement signals low watermark, daemon never wakes | wake via the proven token primitive; heartbeat the daemon loop (the P4a gate-run-3 lesson: instrument the silence before it happens) |
- **Stage A (you):** builds (on+off+daemon-disabled); unit tests + regressions; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the ISC-7 stress loop, all assertions green every iteration.

## 4. Scope guard
The daemon riding the proven primitive — nothing else. **Do NOT** modify the P4a reservation protocol (consume it; if it's insufficient, BLOCKER back to PM rather than patching the proven core); **do NOT** add device-side concurrent DMA (Intel copy engine — later hardware build, Q3); **do NOT** add new prediction machinery (the P3 structural `W=1` predictor only); **do NOT** prefetch across tenants or speculate beyond the published window; **do NOT** add RCU/seqlock or new lock primitives (Q4); **do NOT** change *what* any op computes (ISC-6); **do NOT** make the daemon load-bearing (ISC-5 — the backstop must remain complete).

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A + the stress loop, push. PM `pm-merge-check` → `--ff-only` → `main`. **P4b closes the concurrent-residency line and with it the policy layer's roadmap as THINK'd** (P1 place → P2 reclaim → P3 anticipate → P4 concurrent); the named remaining hardware lift is device-side copy-engine DMA overlap (deferred per Q3). With P4b, the residency trace reads like a mature OS memory manager: a background daemon banking headroom and staging data ahead of need while inference runs — and the critical path stops paying for it.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-10-phase-7-build-P4b-residency-daemon-jarvis.md`
- **THINK (ratified design):** `Plans/THINK-2026-06-10-phase-7-build-P4-concurrent-residency.md` (§9 P4b slice + §6 race matrix)
- **Branch:** `codex/phase-7-build-P4b-residency-daemon-jarvis` off `origin/main` `1f9d8f5`.
- **One-line:** The kswapd-style daemon on the P4a primitive: per-device low/high watermark hysteresis (seam 3 — H goes per-device), bg-demote coldest-unreferenced via TensorSlot claim/skip (seam 2), bounded `W=1` prefetch into fresh storage behind the thrash-guard hard gate, synchronous P2/P3 as the always-complete direct-reclaim backstop, teardown = drain + wake-not-kill (seam 1). Strictly additive: daemon-disabled ≡ P3 bit-for-bit. Gate = the multi-tenant pressure pass concurrent with the daemon, N≥50 at NEO_SMP=4, byte-identical every iteration + critical-path reclaim strictly lower than P3 + the six new daemon discriminators armed. Extend §9.12 + ADR-040. QEMU only, no silicon.
