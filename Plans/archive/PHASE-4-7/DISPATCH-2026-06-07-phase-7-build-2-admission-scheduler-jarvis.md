<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-2 — admission-control inference scheduler (the first cross-tenant decision) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `6046957`: `codex/phase-7-build-2-admission-scheduler-jarvis`)
**Status:** DISPATCHED.
**Builds on:** Phase 7 BUILD-1 (`906a587`, accelerator accounting) + the THINK (Q1–Q3 confirmed: inference request = unit; admission-control shape; temporal-first). ANSPEC-001 §9.5 (accounting), ADR-033.

## 0. The point of this build
BUILD-1 made the kernel *measure*. BUILD-2 makes it *decide* — the first **enforceable cross-tenant decision**: when two mutually-untrusting principals contend for one accelerator, the kernel admits one and **defers** the other, by policy the applications didn't request. That deferral trace is the line that's structurally impossible above the kernel (a per-process userspace runtime sees only its own work and can never legitimately defer *another* principal). This is "sovereignty of decision-making," minimal first instance. **Lean (Article 15.7): build the simplest scheduler that produces a real cross-tenant decision — not a fair-share engine.**

## 1. Resource shape (confirmed, non-negotiable for this build)
**Admission control, NOT preemption.** Accelerators run to completion; the kernel decides *who runs next* when the device is busy or a principal is over quota — it does **not** interrupt a running dispatch. So: a per-device **request queue**, run-to-completion, **temporal** isolation (one tenant's work runs at a time per device). No timeslicing, no preemption, no mid-dispatch migration. This is the block-layer/batch-I/O shape, not the CPU-scheduler shape.

## 2. Scope
A per-`Device::Accel(n)` admission queue for inference requests, with a **minimal** policy that produces a cross-tenant deferral, grounded in BUILD-1 accounting. Inference request = the schedulable unit (Q1).

## 3. ISCs
- **ISC-1 — Per-request principal that survives concurrent dispatch (resolve the BUILD-1 seam).** BUILD-1's `PrincipalGuard` global override is a single-active-principal stand-in. BUILD-2 needs the principal **attached to the request** so concurrent requests from different principals are each attributed correctly through the async queue/dispatch path. The accounting hooks from BUILD-1 must now read the principal *from the in-flight request*, not a global. This is the load-bearing correctness change — everything else is policy on top.

- **ISC-2 — The admission queue + minimal policy.** Per device: a queue of pending inference requests, each tagged `{principal, priority, submit_time}`. Admission decision when the device is busy or a principal is over its share:
  - **per-principal quota** — derived from BUILD-1 accounting (e.g. in-flight + recent resident-bytes / dispatch budget per principal); a principal over quota yields to one under.
  - **priority** — a simple request priority field (higher admits first).
  - **reservation** — a principal may hold a device reservation that defers others while active.
  Keep it minimal: **priority + quota + reservation, run-to-completion.** **Do NOT** build fair-share rotation, aging, work-stealing, or preemption (preemptive thinking — out of scope per §1). The policy must be *enough to defer one principal for another*, no more.

- **ISC-3 — The cross-tenant decision trace (the gate).** On a deferral, emit the decision with its *reason* and the *other principal* it yielded to — e.g. `accel-sched: infer principal=PID12 device=Accel(1) DEFERRED — PID7 reservation active (or: PID12 over quota, PID7 admitted)`. The clause that matters is "deferred **because of** another principal's claim." On admission: `ADMITTED principal=PID7 …`.

- **ISC-4 — Quota grounded in BUILD-1 accounting (no parallel bookkeeping).** The scheduler reads the §9.5 accounting table for its quota decisions — do not introduce a second usage counter. Accounting (measure) and scheduling (decide) stay separate concerns sharing one source of truth.

- **ISC-5 — Multi-tenant test infrastructure + the gate scenario (QEMU).** New: a way to spawn **contending** inference workloads from ≥2 principals that exceed single-device capacity, forcing an admission decision. The gate scenario asserts: both requests complete (no starvation — the deferred one runs after), the **decision trace fires with the correct principal/reason**, and the admission order matches the policy (priority/quota/reservation). Determinism: drive the contention deterministically (explicit reservation or quota threshold), not by race.

- **ISC-6 — Standards + spec.** SPDX; no new `unsafe`; `cargo fmt`; host suite (policy unit tests — quota/priority/reservation decision logic, incl. the principal-from-request attribution) + the multi-tenant gate scenario + all BUILD-1/Phase-6 regressions green; `shard-coverage` complete; `clippy -D warnings` (CI/Linux). Open **ANSPEC-001 §9.6** (the admission-scheduling model — the resource shape, the policy inputs, the run-to-completion/temporal rule, the decision semantics) + a short ADR (Phase-7 scheduler, next number). Note the §9.6 deferrals explicitly (no preemption, no multi-device placement — that's later).

## 4. Deferred seams (name, don't build)
- **Usage/quota query syscall** (a tenant reading its own standing) — useful, but the *decision* gate doesn't need it; defer until a tenant must react to its quota. **When it lands, its struct ABI triggers the byte-oracle rule.**
- **Multi-device placement** (which accelerator, not just which-next-on-one) — BUILD-3+/Phase-7 later; BUILD-2 is single-device admission.
- **Spatial concurrency** (running two tenants *at once* on one device with enforced boundaries) — that's BUILD-4 (PPGTT). BUILD-2 is temporal: one at a time, the scheduler picks order.

## 5. Gate (two-stage — QEMU, no OptiPlex)
No hardware dependency (temporal admission is pure kernel logic; no blind-hardware run → no failure-mode/discriminator section).
- **Stage A:** builds (on+off); policy unit tests + per-request-principal attribution tests + regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the contending-principals scenario fires the cross-tenant deferral trace with correct principal+reason, both requests complete in policy order, accounting stays correct under concurrent attribution. No silicon.

## 6. Scope guard
Admission decision only. **Do NOT** preempt or timeslice (§1); **do NOT** add tensor sharing/dedup (BUILD-3) or device-memory isolation (BUILD-4); **do NOT** add multi-device placement or a query syscall (deferred seams); **do NOT** duplicate accounting (read §9.5). The hand that arbitrates *order* — not the hand that interrupts, shares, or isolates.

## 7. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. BUILD-3 (capability-mediated tensor sharing + dedup — global-view ownership) follows; BUILD-4 (per-context spatial isolation, the Intel PPGTT lift) hardens enforcement to concurrent.
