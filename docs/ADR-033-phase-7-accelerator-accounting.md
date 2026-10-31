<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-033 — Phase 7 BUILD-1: per-principal accelerator accounting (the governance measurement floor)

**Status:** Accepted (Stage-B QEMU PASS). Opens Phase 7.
**Date:** 2026-06-08
**Builds on:** Phase 6 (accel driver class CPU/sim/virtio + real Intel GPU, `fff256d`); the THINK `Plans/THINK-2026-06-07-phase-7-inference-scheduler-governance.md`.
**Related:** ANSPEC-001 §9.5 (the accounting model, amended by this build), ADR-030 (AccelDriver trait), ADR-010 (capabilities — the BUILD-3 principal seam).

## Context

Phase 7 proves the kernel can *govern* accelerator work. You cannot arbitrate what you cannot measure, so the first build is pure **measurement**: per-principal accounting of accelerator usage. No scheduling decision, no quota, no sharing, no isolation — just the kernel *seeing*, per principal, how much of each accelerator each one used. Lean by design (bible Article 15.7): the table and the hooks, nothing more.

## Decision

A per-`(Device::Accel(n), principal)` accounting record, updated from the existing dispatch and residency paths, dumpable as a governance trace, proven correct when ≥2 principals use an accelerator. **No decision is made on the numbers** — measurement only.

1. **Two-file split for host-testability.** The pure arithmetic — `AccelUsage { resident_bytes, dispatch_count, dispatch_time_ticks }` and `AccountingTable` (a `BTreeMap<(device_abi, pid), AccelUsage>` with `add_resident`/`sub_resident`/`record_dispatch`/`usage`/`forget_principal`) — lives in `kernel/src/drivers/accel/accounting_table.rs` with **no kernel dependencies** (only `alloc`), so `tools/ai-ops-tests` path-`mod`s it and unit-tests the increment/decrement/attribution edge cases. The kernel glue (the global table, principal resolution, the dispatch wrapper, the trace) lives in `accounting.rs`. The key is `(u32 device ABI, u32 pid)` because `Device` is not `Ord` but `Device::as_abi()` and `ProcessId` are `u32`.

2. **Hooks at the trait boundary, not per-backend.** Device-agnostic uniform coverage:
   - **Residency** is captured at the **`DeviceStorage`** boundary — the residency unit every backend's `alloc`/`free` shares. The owning principal is captured at construction (`+len`) and carried in the `DeviceStorage`; `Drop` decrements (`−len`) the *same* principal regardless of the freeing context (every backend's `free` just drops the storage). This covers `copy_to/from_device`, `tensor_migrate`, and direct device-alloc uniformly. `accel_sim`'s op-scratch uses raw arena offsets (not `DeviceStorage`), so transient dispatch scratch is correctly *not* counted as residency.
   - **Dispatch** is captured by `accounting::dispatch_and_wait(driver, batch, timeout)` — the common submit→completion wrapper the inference executor (`exec::run_with_driver`) routes through, including the `Unsupported → accel_cpu` fallback (accounted on the device that actually ran it). The op is *known done* when `wait` returns, so count + time are recorded then; a failed wait records no phantom dispatch.

3. **Principal = `ProcessId`; system sentinel = `KERNEL_PID`.** Attribution reads `accounting::current_principal()` = an explicit scoped override if set, else the running thread's pid (`sched::current_pid()`). Inference runs synchronously in the caller's thread, so for a real inference syscall the running pid *is* the caller — no syscall change needed; the scheduler already threads it. Kernel-internal dispatch (no userspace process) resolves to `KERNEL_PID` (= `ProcessId(0)`), the **system principal** — accounting **never panics on "no process."** A `PrincipalGuard` (RAII) is the explicit-principal channel used by the ≥2-principal scenario (and available for future per-request threading). **No new authority primitive** — `ProcessId` is the unit; the capability-principal mapping (for *shared* tensors) is a labelled **BUILD-3 seam**.

4. **Always-on substrate.** Accounting is not feature-gated — it is cheap (a `BTreeMap` update under a `SwitchMutex`, off the dispatch/residency cold paths) and always present. No new `unsafe`. Lock discipline: the principal is resolved (scheduler lock) *before* taking the table lock — the table lock is never held across `current_pid()`.

5. **The governance trace gate.** `accounting::trace()` emits one `accel-acct: principal=PID… device=0x… resident_bytes=… dispatches=… time_ticks=…` line per `(device, principal)` — the build's success metric.

## Seams (named, not built)

- **Per-request principal across concurrent cross-tenant dispatch** — the global scoped override is correct for this build's synchronous, one-principal-at-a-time inference (the QEMU gate runs principals sequentially); a principal that survives concurrent dispatch is **BUILD-2** scheduler state.
- **Capability-principal attribution** (shared-tensor ownership) — **BUILD-3** (tensor sharing).
- **Finer `dispatch_time`** — 100 Hz ticks are coarse (a fast dispatch may read 0); `dispatch_time` is explicitly non-deterministic / non-exact, so this is sufficient for the measurement floor. A TSC-resolution clock is a later refinement if exact-ish timing is ever wanted.
- **A query syscall** (a tenant reading its own usage as a struct) — **out of scope** here (it triggers the byte-oracle rule for its struct ABI); it belongs in BUILD-2 when quota state needs to be readable.

## Gate / Stage-B result

QEMU, **no hardware dependency** (pure kernel bookkeeping). Stage A: builds feature-on (ISO) + feature-off; the host accounting unit test + existing accel/infer regressions green; `shard-coverage`; fmt; clippy. **Stage-B (QEMU) PASS** — the `accel_accounting` scenario: two principals (`PID7`, `PID8`) each migrate a distinct-size tensor onto `accel_sim` and run a distinct number of dispatches; accounting shows distinct, exact per-`(device, principal)` `resident_bytes`/`dispatch_count`, balances `resident_bytes` to 0 on migrate-off, attributes a guard-less dispatch to the system principal (`KERNEL_PID`), and emits the trace:

```
accel-acct: principal=PID0 device=0x80000001 resident_bytes=0 dispatches=1 time_ticks=0
accel-acct: principal=PID7 device=0x80000001 resident_bytes=0 dispatches=2 time_ticks=1
accel-acct: principal=PID8 device=0x80000001 resident_bytes=0 dispatches=3 time_ticks=0
event=PASS name=accel_accounting … resident_bytes=exact dispatch_count=exact dispatch_time=monotonic balanced_to_0=ok system_principal=KERNEL_PID measurement_only=ok
```

## Consequences

The kernel now *sees*, per principal, accelerator residency + dispatch usage — uniformly across all backends, attributed to `ProcessId` with a system sentinel, observable as a governance trace. This is the floor BUILD-2 (admission-control scheduler — the first cross-tenant *decision*) stands on. Resisting scope creep into a scheduler was the discipline here: this is the scale that measures, not the hand that arbitrates.
