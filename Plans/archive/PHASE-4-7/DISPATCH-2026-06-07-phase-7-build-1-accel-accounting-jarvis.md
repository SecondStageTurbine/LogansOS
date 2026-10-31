<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-1 — accelerator accounting (the measurement floor) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `3aeedef`: `codex/phase-7-build-1-accel-accounting-jarvis`)
**Status:** DISPATCHED. Opens Phase 7.
**Builds on:** Phase 6 (`fff256d`) + the THINK `Plans/THINK-2026-06-07-phase-7-inference-scheduler-governance.md` (Q1–Q3 confirmed by PL: admission-control resource shape, temporal-first enforcement, the inference request as schedulable unit).

## 0. Why this build, why first
Phase 7 proves the kernel can *govern* accelerator work. You cannot arbitrate what you cannot measure — so the first build is pure measurement: **per-principal accounting of accelerator usage.** No scheduling decisions, no quotas enforced, no sharing, no isolation. Just: the kernel *sees*, per principal, how much of each accelerator each one used. It's the floor every later build (arbitration, quota, reservation) stands on. Lean by design (Article 15.7) — this build adds the table and the hooks, nothing more.

## 1. Scope
A per-`(Device::Accel(n), principal)` accounting record, updated from the existing dispatch and residency paths, dumpable as a governance trace line, proven correct when **two or more principals** use the accelerator. **No decision is made on the numbers this build** — measuring only.

- **Principal = `ProcessId`** for this build (the process that invoked the inference / owns the dispatched work). Capability-principal alignment is deferred to BUILD-3 (tensor sharing); note the seam, don't build it. Kernel-internal dispatches (test scenarios, regression oracles with no userspace caller) attribute to a **system/kernel principal** sentinel — accounting must never panic on "no process."
- **Devices:** all `Device::Accel(n)` — `accel_cpu`, `accel_sim`, `virtio_accel`, `accel_intel`. Device-agnostic: it hangs off the `AccelDriver`/residency path, not any one backend.

## 2. What's measured
Per `(device, principal)`:
- **`resident_bytes`** — bytes currently resident on the device for this principal. Incremented on `copy_to_device` / migrate-onto-device; decremented on `copy_from_device` / migrate-off / free. **Deterministic — asserted exactly.**
- **`dispatch_count`** — number of `AccelDriver::dispatch` completions attributed to this principal on this device. **Deterministic — asserted exactly.**
- **`dispatch_time`** — accumulated wall-clock from submit to completion. **Real but non-deterministic — logged in the trace, asserted only as monotonic / non-negative, never exact.**

## 3. ISCs
- **ISC-1 — The accounting table.** A new module (e.g. `kernel/src/drivers/accel/accounting.rs` or `kernel/src/ai/accounting.rs`) holding a lock-guarded map `(DeviceId, principal) → AccelUsage { resident_bytes, dispatch_count, dispatch_time }`. Bounded/observable; no unbounded growth (entries keyed by live principal + device). SAFETY/locking discipline per the kernel's existing patterns (no new `unsafe`).

- **ISC-2 — Hooks, at the trait boundary not per-backend.** Account from the common paths so every backend is covered uniformly:
  - **residency:** `copy_to_device` / `copy_from_device` / the `tensor_migrate` storage-replacement / `DeviceStorage` alloc+free → adjust `resident_bytes` for the owning principal.
  - **dispatch:** `AccelDriver::dispatch` completion (the point where the op is known done) → `dispatch_count += 1`, `dispatch_time += elapsed`.
  Existing accel scenarios/oracles must stay green — they just also get accounted.

- **ISC-3 — Principal attribution.** Thread the originating `ProcessId` to the accounting point (the process that called the inference syscall / owns the tensor). Where there is no userspace caller (kernel-internal dispatch), attribute to the system principal. Do not invent a new authority concept — `ProcessId` is the unit this build; the capability-principal mapping is a labelled BUILD-3 seam.

- **ISC-4 — The governance trace gate.** A dump that emits, per `(device, principal)`, the accounting line — e.g. `accel-acct: principal=PID7 device=Accel(1) resident_bytes=… dispatches=… time_us=…`. This is the build's success metric (the THINK's BUILD-1 gate line).

- **ISC-5 — Correctness oracles (the real proof).** A QEMU integration scenario where **≥2 processes** each run inference on an accelerator and the accounting shows **distinct, correct per-(device,principal)** values:
  - `resident_bytes` and `dispatch_count` asserted **exactly** against the known workload.
  - **Accounting balances:** after each principal frees/migrates-off and exits, its `resident_bytes` returns to 0 — accounting itself must not leak (mirror the migration `allocs==frees` discipline).
  - `dispatch_time` asserted only `> 0` / monotonic, never exact.
  - Plus host unit tests for the table arithmetic (increment/decrement/attribution edge cases incl. the system-principal path).

- **ISC-6 — Standards + spec.** Default behavior (accounting is always-on, cheap, no feature gate — it's substrate); SPDX; no new `unsafe`; `cargo fmt`; host suite + the new accounting tests green; `shard-coverage` complete; `clippy -D warnings` (CI/Linux canonical); builds feature-on + feature-off. **Open ANSPEC-001 §9.5** as the first Phase-7 governance subsection — the accounting model (what's measured, the principal definition, the system-principal rule), explicitly noting it is *measurement only, no policy*. A short ADR (the Phase-7 governance-substrate ADR, next number) records the accounting design + the principal/seam decisions.

## 4. Gate (two-stage — note: **QEMU, no OptiPlex**)
This build has **no hardware dependency** — it's pure kernel bookkeeping, validated in QEMU/host. (No blind-hardware run → no failure-mode/discriminator section required.)
- **Stage A (you):** builds (on+off); accounting unit tests + existing accel/infer regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the ≥2-principal accounting scenario shows distinct, exact `resident_bytes`/`dispatch_count` per principal, balances to 0 on teardown, and emits the governance trace line. No silicon needed.

## 5. Byte-oracle note
This build introduces **no on-disk/on-wire format** — the gate is a text trace line, not a binary struct, so no byte oracle is required. **If** you add a syscall that returns accounting as a struct (a tenant reading its own usage), that struct ABI triggers the byte-oracle rule — but a query syscall is **out of scope here** (it belongs in BUILD-2, when quota state needs to be readable). Keep BUILD-1 to the table + hooks + trace.

## 6. Scope guard
Measurement only. **Do NOT** make or enforce any decision on the numbers (no admission, no quota, no reservation, no deferral — that is BUILD-2); **do NOT** touch tensor sharing/dedup (BUILD-3) or device-memory isolation (BUILD-4); **do NOT** change the Phase-6 dispatch/residency *behavior* — only *observe* it. No new authority primitive; `ProcessId` is the principal. Resist scope creep into a scheduler — this is the scale that measures, not the hand that arbitrates.

## 7. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM runs `scripts/pm-merge-check.sh` → `--ff-only` → `main`. BUILD-1 lands; BUILD-2 (the admission-control scheduler — the first cross-tenant *decision*) follows, standing on this accounting.
