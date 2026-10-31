<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 (second half) — policy-driven placement (the mechanism→policy leap)

**Date:** 2026-06-08
**Seat:** PM/Cerebro (design-questions pass; not a dispatch)
**Context:** The Phase-7 governance **mechanisms** are complete and silicon-enforced — measure (§9.5), arbitrate (§9.6), own (§9.7), isolate (§9.8, closed 2026-06-08). What remains is the **policy** layer: the kernel *reasoning* about placement/residency, not merely *executing* it. This is the distinction the architecture analysis kept returning to — *"the kernel owns residency today, but the kernel is not yet reasoning about residency."* This THINK opens the reasoning half.
**Related:** the Phase-7 THINK (`Plans/THINK-2026-06-07-phase-7-inference-scheduler-governance.md`), ANSPEC §9.5–9.8, ADR-030.

## 1. The leap, stated precisely
Today every placement is **explicit**: a caller/executor says "migrate tensor X to device D," "run this op on driver R." The kernel owns the *mechanism* (`tensor_migrate`, the AccelDriver registry, DeviceId residency) but makes no *decision*. Policy-driven placement means the kernel **decides** — *where a tensor should live, where an op should run* — from inputs only it can see globally (residency, §9.5 accounting, device availability, migration cost). The tell, from the conversation: a trace that contains **reasoning the application didn't request** —
```
placement: op=matmul → Accel(1)  (operands resident there; migration avoided)
residency: tensor X retained on Accel(1)  (next op needs it; evict cost > keep)
eviction: tensor Y → CPU  (LRU; Accel(1) at 95% quota, room needed for PID12)
```
"`retained because the scheduler reasoned it would be needed`" is the line that makes it look like resource management, not dispatch.

## 2. What exists (all the inputs; no deciding agent)
- **Residency:** `DeviceId` per tensor — the kernel knows where every tensor lives.
- **Usage:** §9.5 accounting — per-principal, per-device resident bytes + dispatch counts.
- **Devices:** the AccelDriver registry — what accelerators exist, their state.
- **The move:** `tensor_migrate` (storage-replacing, exactly-one-device) + the admission scheduler (§9.6).
- **Missing:** the *policy* that consumes these to make a placement decision. The executor currently dispatches to a fixed/default driver.

## 3. Why this is correctness, not just optimization (the migration-thrash point)
The architecture analysis flagged it: a mixed GPU/CPU forward pass where every fallback op bounces the tensor GPU→host→GPU can be **net-negative** — the accelerator makes it *slower*. So placement policy isn't a nicety; it's what makes "performance = incremental" (the CPU-fallback thesis) actually hold. **Phase 7's scheduler isn't merely optimization — it's correctness for the performance thesis.** The first policy decision should therefore attack *migration thrash*.

## 4. Candidate first policy decisions (design Q)
- **(A) Residency-aware op placement** — the executor consults a placement policy: "given this op's operands' residency + device availability/quota, which device runs it?" Policy: prefer the device where operands already reside (avoid migration); fall back to CPU when no accelerator benefit. **Recommended first build** — it's the clearest "resource-management decision," directly attacks migration thrash (§3), is QEMU-testable on the cpu/sim/virtio/intel backends (no new silicon), and produces the canonical `placement:` trace.
- **(B) Eviction policy** — when a device is at quota and a new tensor needs residency, the kernel decides which resident tensor to evict (LRU from §9.5). The classic OS resource-management decision (page eviction's analogue). Strong, but needs a memory-pressure scenario to exercise; natural second build.
- **(C) Cost-aware migration** — decide *whether* to migrate based on a cost model (transfer bytes vs compute benefit). More sophisticated; depends on (A)'s placement framework. Later.

## 5. Slicing
- **BUILD-P1 — residency-aware placement (A).** A placement-policy hook the executor consults before dispatch; inputs = operand residency (DeviceId) + §9.5 + registry; output = the chosen device + the logged decision. Gate: a forward pass where the policy keeps operands on their resident device (migration avoided) vs a fixed-device baseline that thrashes — asserted by the `placement:` trace + a migration count. QEMU.
- **BUILD-P2 — eviction (B).** Under device-memory pressure, LRU eviction with the `eviction:` decision trace. QEMU (a pressure scenario).
- **BUILD-P3 — cost-aware migration (C).** Optional/later.
Each gated on its **decision trace line** (the §1 reasoning), per the same "log test = the governance line" discipline that ran the mechanism builds.

## 6. Design questions for the PL
- **Q1 — accept (A) residency-aware placement as the first policy build (BUILD-P1)?** (Recommended — attacks migration thrash, the correctness-for-performance concern.)
- **Q2 — where does the policy live?** In-kernel (a `placement` module the executor consults) — consistent with the monolithic stance and §9.8's "policy uniform/in-kernel." The policy is pure/deterministic (host-testable), reading the registry/§9.5 as inputs. Confirm.
- **Q3 — the schedulable/placeable unit:** the **op** (where it runs) and the **tensor** (where it lives) — the two coupled decisions. Start with op placement driven by operand residency; tensor placement follows from op outputs. Confirm.

## 7. Honest scope note
This is **not** a hard bare-metal arc — it's policy logic over the proven mechanisms, QEMU-testable, far lower risk than the 4X bring-up. The value is conceptual, not silicon: it's the build where the logs *start to show the kernel reasoning*, which the architecture analysis identified as the moment the system "stops looking like an inference engine and starts looking like an operating system managing a new class of resource." The mechanisms made that *possible*; this makes it *visible*.

## 8. Recommendation in one line
Open the policy half of Phase 7 with **BUILD-P1 — residency-aware op placement**: an in-kernel placement policy the executor consults (inputs: operand residency + §9.5 accounting + the registry; output: the chosen device + a `placement:` decision trace), gated on a forward pass that avoids migration thrash where a fixed-device baseline incurs it — the first build where the kernel *decides* placement rather than executing it, and the trace begins to read like resource management.
