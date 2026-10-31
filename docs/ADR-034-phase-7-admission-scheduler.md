<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-034 — Phase 7 BUILD-2: admission-control inference scheduler (the first cross-tenant decision)

**Status:** Accepted (Stage-B QEMU PASS).
**Date:** 2026-06-08
**Builds on:** Phase 7 BUILD-1 (accelerator accounting, `906a587`, ADR-033, ANSPEC-001 §9.5); the THINK (Q1–Q3: inference request = unit, admission-control shape, temporal-first).
**Related:** ANSPEC-001 §9.6 (the admission-scheduling model, amended by this build).

## Context

BUILD-1 made the kernel *measure*. BUILD-2 makes it *decide* — the first **enforceable cross-tenant decision**: when two mutually-untrusting principals contend for one accelerator, the kernel admits one and **defers** the other, by policy the applications did not request. That deferral trace is structurally impossible above the kernel (a per-process userspace runtime sees only its own work and can never legitimately defer *another* principal). Lean (Article 15.7): the simplest scheduler that produces a real cross-tenant decision — not a fair-share engine.

## Decision

A per-`Device::Accel(n)` admission queue for inference requests, run-to-completion, with a minimal policy grounded in §9.5 accounting that produces a cross-tenant deferral.

1. **Per-request principal — the BUILD-1 seam resolved (ISC-1, load-bearing).** BUILD-1's `PrincipalGuard` global override was a single-active-principal stand-in. BUILD-2 attaches the principal to the **in-flight request** via a **per-device in-flight-principal** in the accounting module (`accounting::InflightGuard`): the scheduler sets the executing request's principal for its device; the accounting hooks read it *per device*. Because accelerators run to completion (one request per device at a time — temporal), one slot per device is exact, and concurrent requests on *different* devices are each attributed correctly. `current_principal` is now `current_principal(device)` with precedence **in-flight (per device) → global override (BUILD-1 fallback) → `current_pid()`/`KERNEL_PID`**. No global single-principal clash.

2. **Resource shape — admission control, NOT preemption.** Per-device request queue, run-to-completion, temporal isolation. The scheduler decides *who runs next* when the device is busy/over-quota; it never interrupts a running dispatch. Block-layer/batch-I/O shape, not CPU-scheduler shape.

3. **Two-file split for host-testability.** The pure policy — `Request {principal, priority, submit_seq, reserved}`, `Decision`, `DeferReason`, `admit_arrival`, `choose_next` (priority → under-quota → FIFO), `losing_reason` — lives in `kernel/src/ai/admission_policy.rs` with **no kernel deps** (host-unit-tested via `ai-ops-tests`). The kernel glue — per-device `QUEUES`, `submit`/`complete`, the decision trace, and the `over_quota` predicate reading §9.5 — lives in `admission.rs`.

4. **Quota grounded in §9.5 (ISC-4) — no parallel bookkeeping.** `over_quota(device, principal)` reads `accounting::usage(device, principal).dispatch_count >= DISPATCH_QUOTA` (a fixed dispatch budget for BUILD-2; elastic quota is later policy). Measure (§9.5) and decide (§9.6) stay separate concerns sharing one source of truth. Lock order: `QUEUES → TABLE` one-way (the accounting paths never take `QUEUES`).

5. **The decision trace (ISC-3, the gate).** `accel-sched: infer principal=PIDx device=0x… {ADMITTED | DEFERRED — PIDy <reason>}`. The load-bearing clause is *deferred because of another principal's claim* (reservation / device-busy / over-quota / lower-priority, each naming the other principal).

## Seams (named, not built)

- **`sys_inf_run` integration** — ~~the syscall does not yet route through the scheduler (the gate is exercised by the multi-tenant scenario). Wiring inference submission → admit/defer (blocking the syscall thread on deferral, waking on admission) is the natural next integration~~ **resolved by BUILD-2L (see Addendum below).** BUILD-2 lands the scheduler + the cross-tenant decision it produces; BUILD-2L wires it onto the live syscall path.
- **Usage/quota query syscall** (a tenant reading its own standing) — deferred; **its struct ABI triggers the byte-oracle rule when it lands** (BUILD-3+).
- **Multi-device placement** — single-device admission here; which-accelerator is later.
- **Spatial concurrency** (two tenants at once on one device) — BUILD-4 (Intel PPGTT). BUILD-2 is temporal.

## Gate / Stage-B result

QEMU, no hardware dependency (temporal admission is pure kernel logic). Stage A: builds on+off; host policy unit tests + the per-request-principal attribution + BUILD-1/Phase-6 regressions green; `shard-coverage`; fmt; clippy. **Stage-B (QEMU) PASS** — the `accel_admission` scenario:

```
accel-sched: infer principal=PID7 device=0x80000001 priority=5 ADMITTED
accel-sched: infer principal=PID12 device=0x80000001 DEFERRED — PID7 reservation active
accel-sched: infer principal=PID12 device=0x80000001 priority=5 ADMITTED      # PID7 ran, PID12 admitted in order
accel-sched: infer principal=PID9 device=0x80000001 priority=5 ADMITTED
accel-sched: infer principal=PID7 device=0x80000001 DEFERRED — PID9 device busy
accel-sched: infer principal=PID12 device=0x80000001 DEFERRED — PID9 device busy
accel-sched: infer principal=PID12 device=0x80000001 priority=5 ADMITTED      # under-quota PID12 beats over-quota PID7 (read from §9.5) despite PID7's earlier FIFO
accel-sched: infer principal=PID7 device=0x80000001 priority=5 ADMITTED       # over-quota PID7 admitted last
event=PASS … gate=PID12_deferred_for_PID7_reservation quota=under_quota_PID12_beats_over_quota_PID7 run_to_completion=ok both_complete_in_policy_order=ok per_request_principal=ok measurement_grounded=accounting_9.5
```

Phase 1 is the headline cross-tenant decision (reservation). Phase 2 proves the quota read from §9.5 (the under-quota principal is admitted before the over-quota one despite earlier FIFO). Both requests complete in policy order — no starvation. Accounting attributes each request's work to its own principal through the per-device in-flight path.

## Consequences

The kernel now *arbitrates order* across tenants on an accelerator — admits one, defers another, by a policy (priority/quota/reservation) the applications did not request, with the decision observable and the quota grounded in §9.5. This is sovereignty of decision-making, minimal first instance. BUILD-3 (capability-mediated tensor sharing + dedup — global-view ownership) follows; BUILD-4 (per-context spatial isolation, the Intel PPGTT lift) hardens enforcement from temporal to concurrent.

## Addendum — BUILD-2L: the scheduler on the live syscall path (block-on-defer / wake-on-admit)

**Status:** Accepted (QEMU PASS). **Date:** 2026-06-08. **Builds on:** BUILD-2 (this ADR). **Amends:** ANSPEC-001 §9.6 (adds the *Live-path enforcement* contract). **No new ABI.**

BUILD-2 landed the scheduler but exercised it single-threaded (`submit`/`complete` driven by hand in the gate scenario); the named seam was *"`sys_inf_run` does not yet route through the scheduler."* BUILD-2L closes that seam: the live inference syscall is now **bound** by the admit/defer verdict.

1. **`sys_inf_run` routes through the queue (ISC-1).** The syscall wraps its `exec::run` in `admission::run_scheduled(device, principal, …, work)`, where `device` is the default driver's device and `principal` is the caller — the *same* `{device, principal}` the §9.5 accounting hooks attribute against, so scheduling and measurement cannot disagree.

2. **Block-on-defer / wake-on-admit via existing primitives (ISC-2, load-bearing).** A deferred caller **parks** on the kernel's existing wake-token blocking primitive (`block_current_with_token_and_guard`) — no busy-wait, no new `unsafe`, no new lock type (the per-device queue's `SwitchMutex` guard already satisfies the primitive's `ContextSwitchRelease` bound). The defer decision and the park happen under a *single* queue-lock acquisition: the pending request carrying the caller's wake token is installed by the primitive's `install` closure while the lock is held, then released atomically with the context switch — so a concurrent `complete` cannot wake a token that is not yet stored (no lost wakeup). On completion, `complete` admits the next request and `wake_thread_token`s its parked caller. The parked caller holds **no** lock the admitter needs; lock order is one-way (`QUEUES → TABLE` for the quota read, `QUEUES → scheduler` for the wake), never inverted (no deadlock).

3. **Single-tenant fast path (ISC-4).** An empty queue / free device admits immediately with no parking and no added latency — the governance machinery costs nothing in the common, uncontended case.

### BUILD-2L gate / Stage-B result

QEMU, no hardware dependency. New scenario `accel_admission_livepath` drives `run_scheduled` — the exact entry `sys_inf_run` routes through — from **two genuinely concurrent callers**: caller A (admitted, fast path) holds the device while caller B (a kthread) issues its own `run_scheduled`, is **deferred (device busy)**, and **parks**. The harness asserts B is **token-parked** (off the ready queue — the not-spinning proof; a busy-waiting B would be Ready/Running) and has *not* run its work; on A's completion the scheduler **wakes** B, which then runs and completes. Asserts: B parked-not-spinning, completion order A-then-B (B never ran before admission), each dispatch attributed to its own principal (§9.5 per-device in-flight), queue drained. The `accel-sched: infer principal=PID102 device=… DEFERRED — PID101 device busy` / `… ADMITTED` decision trace fires **on the live path**.

The kernel's cross-tenant arbitration is no longer a side scenario — it governs the syscall an application actually calls.
