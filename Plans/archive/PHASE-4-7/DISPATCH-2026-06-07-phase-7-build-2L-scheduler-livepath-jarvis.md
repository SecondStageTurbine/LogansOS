<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-2L — scheduler live-path wiring (demonstrated → enforced) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `a9bc28c`: `codex/phase-7-build-2L-scheduler-livepath-jarvis`)
**Status:** DISPATCHED. **Roadmap note:** inserted before BUILD-3 (PM call) — it converts BUILD-2 from *demonstrated* to *enforced*, and establishes the live-path integration pattern BUILD-3 will reuse.
**Builds on:** BUILD-2 (`ed9bcbb`, admission scheduler) + BUILD-1 (`906a587`, accounting). ANSPEC-001 §9.5/§9.6, ADR-033/034.

## 0. The gap this closes
BUILD-2's scheduler makes the right cross-tenant decision — but only in the multi-tenant *scenario*; `sys_inf_run` doesn't route through it, so a real tenant's inference bypasses the scheduler. The decision is **demonstrated, not enforced.** This build routes the live inference syscall through the scheduler so the decision actually **binds real tenants**: a deferred principal's `sys_inf_run` *blocks* until admitted. That is the difference between "the kernel can decide" and "the kernel's decisions are real" — the enforceable half of the governance thesis.

## 1. Scope
Route `sys_inf_run` (and the in-kernel inference dispatch path it drives) through the BUILD-2 admission queue, with proper **block-on-defer / wake-on-admit** thread handling. **No new policy, no new mechanism** — BUILD-2's decision logic stands unchanged; this build only puts real syscalls *under* it. Still temporal/run-to-completion (Q2), QEMU-validated, no silicon.

## 2. ISCs
- **ISC-1 — Route the live path through the queue.** On `sys_inf_run`, enqueue the request to the target device's BUILD-2 admission queue (tagged with the per-request principal, ISC-1 of BUILD-2). The scheduler's `choose_next` governs execution; on completion the slot releases and the next is admitted. The existing `run_with_driver` dispatch + §9.5 accounting must stay on this path (accounting still attributes per principal under real concurrency).

- **ISC-2 — Block-on-defer / wake-on-admit (the load-bearing integration).** A deferred request must **park the calling thread** using the kernel's existing thread-block/wake primitives — **no busy-wait, no spin**. When the scheduler admits it (the holding principal completes / reservation clears / quota frees), the thread is **woken** and runs. This crosses the AI-subsystem ↔ process-scheduler boundary; reuse the established blocking/wake mechanism (the same class the IPC/syscall blocking paths use), don't invent a new one. Correctness: no lost wakeups, no deadlock (mind the QUEUES→TABLE lock order from BUILD-2; a parked thread must not hold a lock the admitter needs), no priority inversion that starves.

- **ISC-3 — The live-path gate (enforced, not scenario).** A QEMU scenario with **two real processes** issuing `sys_inf_run` concurrently for the same device: one is admitted and runs; the other **actually blocks** (parked — assert it is not spinning/consuming CPU) until the first completes, then is woken and completes. The BUILD-2 decision trace fires **on the live syscall path**, and the *order of completion* matches the policy. The deferred process's wall-clock shows it waited. This is the "enforced" proof.

- **ISC-4 — No-regression + liveness.** All BUILD-1/BUILD-2 tests + the Phase-6 regressions stay green. Liveness: every admitted-eventually request completes (no permanent starvation under the policy); a principal that never reserves still makes progress. Single-tenant `sys_inf_run` (no contention) must be unaffected — no added latency when the device is free (fast-path: empty queue → admit immediately, no parking).

- **ISC-5 — Standards + spec.** SPDX; no new `unsafe` (the block/wake must use safe existing primitives); `cargo fmt`; host suite + the live-path gate scenario + all regressions green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. Amend **ANSPEC-001 §9.6** with the live-path enforcement contract (submit→block-on-defer→wake-on-admit, the no-busy-wait/liveness guarantees); note in ADR-034 (or a short follow-on) that the scheduler is now on the live path. **No new syscall ABI** is expected (this routes the *existing* `sys_inf_run`); if the syscall surface changes, that struct triggers the byte-oracle rule — but prefer routing without an ABI change.

## 3. Gate (two-stage — QEMU, no OptiPlex)
Pure kernel integration; no blind-hardware run (no failure-mode/discriminator section).
- **Stage A:** builds (on+off); block/wake unit + integration tests; regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** two concurrent `sys_inf_run` callers — one runs, one parks (verified not-spinning) until admitted, both complete in policy order, decision trace on the live path. No silicon.

## 4. Scope guard
Live-path wiring only. **Do NOT** change BUILD-2's policy/decision logic; **do NOT** add tensor sharing/dedup (BUILD-3), device-memory isolation (BUILD-4), multi-device placement, preemption, or a usage-query syscall. **Do NOT** busy-wait — parking via existing primitives is mandatory. The job is to make the *existing* decision *bind real syscalls*, nothing more.

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. With this, BUILD-2's cross-tenant decision is **enforced on real inference** — the governance thesis's first fully-real instance. BUILD-3 (capability tensor sharing + dedup) follows, reusing this live-path/block-wake pattern.
