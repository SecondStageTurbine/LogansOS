<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE REPORT — Phase 7 device-boundary governance line (the BUILD-4X arc)

**Date:** 2026-06-08
**Status:** **COMPLETE on silicon.** The fourth governance line — spatial device-memory isolation between adversarial tenants — is enforced by the accelerator's own page tables on the Intel UHD 630 (OptiPlex 7080).
**Closing commit:** `c48d5d5` (BUILD-4X-3 Stage-B PASS).
**Authoritative:** ANSPEC-001 §9.8 (now ENFORCEMENT-PROVEN) + §9.7 (RO seam closed), ADR-032, the cited spec `Plans/RESEARCH-2026-06-08-gen9-execlist-lrc-bringup.md`.

## 1. What this closes
All four Phase-7 governance lines are now real on hardware:
1. **Measure** (§9.5) — per-principal accelerator accounting.
2. **Arbitrate, enforced** (§9.6) — a real tenant's `sys_inf_run` blocks for another's reservation/quota, on the live path.
3. **Own** (§9.7) — N principals → one physical tensor, RO-enforced, dedup'd.
4. **Isolate, enforced by silicon** (§9.8) — a second tenant's device-side access to the first's tensor page-faults; a shared RO tensor is hardware-unwritable by the grantee.

The thesis the project's design was built to reach — *"the kernel governs AI between mutually-untrusting tenants, enforced not by an op-boundary check a tenant could race but by the accelerator's own page tables"* — is instantiated. The kernel's tensor authority survives all the way down to GPU memory translation.

## 2. The arc (and how the project's discipline carried it)
| Step | Result |
|---|---|
| **BUILD-4-1** (`f17e4d6`/`ef46c2c`) | Enforcement-first reframe (PPGTT on the legacy ring) **falsified** on Gen9 over 4 OptiPlex iterations — all three root mechanisms (32-bit PDP, 48-bit PML4, Gen7 PP_DIR_BASE) read `select=GGTT`. The render engine binds PPGTT to the Logical Ring Context; legacy-ring PPGTT does not exist on this silicon. The self-proving gate + `select` capture made the negative **conclusive**, not a mystery. |
| **Deep research** (`Plans/RESEARCH-2026-06-08-...`) | 5 parallel lanes specified the GuC-free execlist/LRC path from cross-validated i915 (v4.4–v6.1) **+ a non-i915 Fuchsia Magma oracle** (closing the "two i915 readers agree even if both wrong" loophole). Caught an off-by-one in the context-image layout pre-silicon (and corrected the brief's "8-dword" WaIdleLiteRestore to 2). |
| **BUILD-4X-1** (`7626f62`) | GuC-free execlist submission runs a context on the UHD 630 with the context's PPGTT as the active address space — the inverse of 4-1's GGTT dead end. **2 iterations** (the research + the §7 decision tree replacing a likely-8-iteration blind slog). |
| **BUILD-4X-2** (`e78cc79`) | The Add retired `[4,6]` through the execlist context with its data buffers cleared from GGTT — tensor data resolves through the per-context PPGTT (`select=PPGTT`). The positive boundary. |
| **BUILD-4X-3** (`c48d5d5`) | Two tenants, two per-context PML4s: B's device-side access to A's tensor **page-faulted** (select=PPGTT, A's VA, B's PML4 active); B's own access **succeeded** (positive control); RW-on-RO **faulted**, RO-read succeeded. The adversarial boundary. |

What made this tractable rather than an open-ended blind slog: the **blind-hardware discipline** (every run conclusive on failure via the fault-address/`select`/CSB/PML4-root captures and the decision tree), the **deep-research lane** translating solved prior art instead of reverse-engineering silicon, and the **negative-test rigor** (the gate is the *fault*, with a positive control, because the worst failure — B silently reading A's data — looks like success).

## 3. The honest record — what was wrong and got corrected
- **My enforcement-first reframe was the wrong technical bet** — "PPGTT on the legacy ring" does not exist on Gen9; the silicon falsified it cleanly. The reframe still produced reusable PPGTT machinery (page tables, PTE oracle), but the device-boundary line cost the full execlist bring-up it was meant to avoid.
- **The research brief had an error** (WaIdleLiteRestore "8 dwords" → 2) caught by the lanes; and the LRC context-image had an off-by-one caught by the spec + Fuchsia oracle pre-silicon.
- **A "no-op = zero-PPGTT" premise was falsified** by 4X-1 iter1 (the execlist context routes pipeline/state pages through PPGTT too) — which the decision tree pinned instantly.

## 4. What this does NOT close (the honest bound — §9.8 unchanged)
- **Temporal fairness** remains **admission-controlled, not preemptive** — bounded by the hardware (Gen9 compute does not preempt). The kernel can prevent a tenant from *reaching* another's data (proven here) but cannot interrupt a tenant *monopolizing* the device with a long kernel. This gap is fundamental, not an implementation gap, and is unchanged by the 4X arc.
- **Enforcement fidelity is per-device.** This proves spatial isolation on Gen9 via PPGTT/execlist. A different accelerator's isolation hardware (or its absence) would require its own bring-up; the *authority model* (capabilities) is portable, the *enforcement mechanism* is per-silicon (§9.8).
- **Concurrency:** the boundary is *spatial* (separate page trees), proven with two *temporal* contexts. Concurrent multi-tenant execution is a later optimization, not required for the isolation guarantee.

## 5. Post-close markers
- Stateful-BTI / DRM-expressibility (ADR-023) — production surface model, deferred.
- Concurrent execlist scheduling (multiple contexts resident) — performance, deferred.
- The SMP bring-up flake on this OptiPlex (cold power-cycle clears it) — recorded hazard; cold-boot-and-retry, don't re-investigate.

## 6. Next
Phase 7 continues on a **complete enforcement substrate** — the policy layer that the architecture analysis identified as the mechanism→policy leap: **policy-driven placement** (the kernel *deciding* residency/migration, not just executing it) and the broader inference scheduler. The governance *mechanisms* are now all real and enforced; what remains is the *reasoning* on top of them.
