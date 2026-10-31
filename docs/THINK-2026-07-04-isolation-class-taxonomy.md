<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Isolation-class taxonomy for NeoPlutarch — graded isolation as a kill-switch knob

**Date:** 2026-07-04
**Source:** HongMeng IC0/IC1/IC2 lesson (`THINK-2026-07-03-harmonyos-hongmeng-lessons.md` §2), the kill-switch discipline (`complexity-governance.md` Rule 1), Bible Articles 1.2, 4.1, 5.6, 6.6
**Status:** Design THINK — no code, no build. Decides whether and how to adopt isolation-class-as-a-knob for NeoPlutarch.

---

## 0. The question

HongMeng makes security/performance trade-offs a **deployment-time knob** via graded isolation classes (IC0 kernel-adjacent / IC1 in-kernel services / IC2 address-space-isolated userspace), with a proven-safe fallback: "demote everything to IC2 under threat." That maps onto our kill-switch discipline.

**Do we need this, or is it a HongMeng-ism that doesn't fit our architecture?**

---

## 1. D1 — Is isolation-class-as-a-knob a real fit for us?

**Short answer: PARTIALLY — the *concept* is useful, but the *taxonomy* needs to be ours.**

HongMeng's IC0/IC1/IC2 exists because it's a **microkernel with a rich userspace service layer** — services like the filesystem manager, device manager, and input manager can run at different isolation levels depending on threat posture. Moving them in-kernel (IC1) trades safety for IPC frequency; moving them to userspace (IC2) trades latency for safety.

NeoPlutarch is a **monolithic kernel** (Bible Article 4.1). The kernel IS the trusted space. There are no "in-kernel services" that can be demoted to userspace — everything in the kernel is already at the highest privilege. The userspace services (neo-init, neo-arthur, neo-worker, neo-shell) are already address-space-isolated by construction.

**The mapping that works:** Our equivalent of "isolation class" is not about *where* a component runs (kernel vs userspace — that's fixed by the monolithic architecture), but about **how tightly coupled a kernel subsystem is to other subsystems**. A tightly-coupled subsystem (e.g., the AI governance layer) shares locks, data structures, and execution context with the core scheduler. A loosely-coupled subsystem (e.g., the WiFi driver) is fault-contained by I-14 but still runs in-kernel.

**The mapping that doesn't work:** HongMeng's "promote to IC1 for speed" — moving a userspace service into kernel space. We don't do this. Our Rust-first stance and Bible Article 1.2 (Correctness > Coherence > Performance) make this a hard no. A service running in-kernel is in the TCB. Period.

**Verdict D1: ADEPT the concept (kill-switchable isolation posture), but REJECT the IC0/IC1/IC2 taxonomy as-is.** Our isolation classes must reflect our monolithic architecture, not HongMeng's microkernel service-placement model.

---

## 2. D2 — The NeoPlutarch isolation-class taxonomy

We define **three isolation classes** that map onto our actual architecture:

### Class 0 — Core (non-demotable)

The kernel core that CANNOT be isolated from anything because it IS the execution substrate. Scheduling, memory management, page tables, interrupt handling, syscall entry/exit, the capability system. These are always-in-TCB by definition. No kill-switch applies — you can't "demote" the scheduler to userspace.

**Kill-switch posture:** N/A. These are always-on. Their correctness is proven by FV, byte-oracles, and AAG.

### Class 1 — Coupled (kill-switchable coupling)

Kernel subsystems that are **tightly coupled to the core for performance but can be decoupled under threat**. The primary candidate: **the AI governance layer** (tensor management, model residency, inference scheduling).

Currently, the AI governance runs in-kernel with direct access to the scheduler and memory manager. It holds locks in the I-1..I-15 band. If an adversarial model or a compromised inference context could exploit the AI governance to escalate privileges, the TCB is breached.

**Kill-switch posture:** The "demote" option for Class 1 is **not moving it to userspace** (that would break the AI-native premise — Article 2 says tensors are kernel primitives). Instead, the demotion is **capability tightening + lock isolation**:
- Strip all non-essential capabilities from inference contexts
- Isolate AI locks from core locks (already partially done by I-14)
- Disable cross-subsystem optimization paths (e.g., residency-aware placement that touches the scheduler directly)

The kill-switch regression proves: "AI governance with tightened coupling ≡ current behavior for the safe subset of operations."

### Class 2 — Isolated (always fault-contained)

Kernel subsystems that are **designed to be fault-contained from day one**. Connectivity drivers (WiFi, USB), personality layers (Linux ABI), and any future hardware drivers. These are already I-14-gated: a fault here cannot mutate or stall the AI invariants.

**Kill-switch posture:** Already isolated. The kill-switch is **feature disable** — turn off WiFi, unload the personality layer. No coupling to prove.

### The safe-fallback rule

A class change from N → N+1 (more isolation) must be **kill-switch-gated**:
1. The more-isolated configuration must be proven safety-preserving (no new attack surface)
2. For Class 1 → "tightened" demotion: prove the restricted AI governance still produces correct inference output (byte-oracle) and doesn't violate I-1..I-13
3. The regression is the kill-switch: `isolation_class=2` mode ≡ current behavior for all operations that remain enabled

---

## 3. D3 — The hard reject: no IC1-style in-kernel co-location

**Principle:** A validated or verified service is NOT co-located into kernel space for performance. Period.

**Why:**
1. **TCB inflation:** Every line of code in the kernel is trusted code. Adding a "fast path" that moves a userspace service in-kernel expands the TCB — and every TCB line is a potential exploit target. DarkNavy's evaluation of HarmonyOS confirmed this: HongMeng's isolation strength (microkernel boundary) is its best defense, but its **memory-corruption defense lags** precisely because IC1 services run in the kernel address space with C-level memory safety.
2. **Memory-safety cost:** We are Rust-first. Rust's safety guarantees apply to ALL code, kernel and userspace. Moving a service in-kernel doesn't make it faster in any way that justifies the TCB expansion — our IPC is already efficient (syscall + capability check, not microkernel message passing).
3. **Article 1.2:** Correctness > Coherence > Performance. A fast wrong answer is worse than a slow right one. An in-kernel service that's hard to verify correctly violates this ordering.
4. **The HarmonyOS cautionary tale:** HarmonyOS NEXT **dropped PAC+CFI** (memory-safety mitigations from HarmonyOS 4.2) during the microkernel transition. A performance/isolation refactor silently dropped a memory-safety mitigation. We must never let this happen.

**If a service ever needs kernel-adjacency:** It must be (a) Rust-safe by construction, (b) AAG-audited with adversarial discriminators, and (c) the **default class is the isolated one** — any promotion to tighter coupling is kill-switchable back to isolation.

---

## 4. D4 — Interaction with I-14 and the lock-order band

### I-14 (subsystem fault containment)

I-14 already mandates that a connectivity-driver fault cannot mutate or stall the AI invariants. Our Class 2 (isolated) subsystems are I-14-gated by construction. The isolation-class taxonomy **extends I-14 to Class 1 (coupled)**: under threat, the AI governance can be decoupled from the core, effectively promoting it to I-14-level isolation.

The relationship: I-14 is the **floor** (Class 2 minimum). The isolation class is the **knob** (how much coupling above the floor is permitted).

### Lock-order band (OT-13, I-5)

A class change must not violate the lock order. Currently, AI locks sit below USB locks in the `1..=31` lockdep domain. If the AI governance is "demoted" to tighter isolation, it must still respect I-5 — no new lock inversions.

**Rule:** A class change cannot add a new lock or reorder existing locks. The decoupling is achieved by **removing shared state**, not by adding synchronization.

---

## 5. D5 — Verdict and build sketch

### Verdict: REJECT as active framework; retain as design reference

**Isolation-class-as-a-knob is largely N/A for a monolithic kernel.** Our isolation model IS Rust module discipline + I-14 fault-containment — not address-space service placement. There are no "in-kernel services" that can be demoted to userspace (Bible 4.1), so the graded-isolation knob has no mechanical lever to turn. The concept is useful as a **design lens** when evaluating external architectures (e.g., "HongMeng's IC0/IC1/IC2 doesn't map to us because we're monolithic"), but it should not be formalized as code, carried as an active governance framework, or added to the invariant ledger.

**Hold nothing new.** Neither formalize as a compile-time flag nor carry as an active policy. This THINK is the reference for why.

### No build warranted

There is no build to dispatch. The taxonomy is documented here as a design reference — it explains why HongMeng's IC0/IC1/IC2 doesn't apply to us and what our isolation model actually is (Rust modules + I-14). Consistent with Bible 4.1 and ADR-001.

---

## 6. Open PL-ratification point

**Question:** Do we want "isolation class" as a first-class, documented property of kernel subsystems (a column in the invariant ledger), or is this THINK sufficient documentation without formalizing it into the governance machinery?

This is a **documentation vs. mechanism** question. The taxonomy is useful as a design lens (evaluating new subsystems: "what class does this belong to?"), but formalizing it as a compile-time flag adds complexity that may not pay off until we have a subsystem that genuinely needs multiple coupling levels.

**Recommendation:** Keep this THINK as the reference. Add the isolation-class consideration to the dispatch template (every future subsystem dispatch states its isolation class). Formalize into mechanism only when a build actually needs the kill-switchable coupling.

---

## 7. Summary

| Decision | Verdict | Rationale |
|----------|---------|-----------|
| **D1** — Isolation-class-as-knob fit? | **ADOPT concept, REJECT HongMeng taxonomy** | Monolithic architecture ≠ microkernel service placement. Our classes are about coupling, not address space. |
| **D2** — Our taxonomy | **Class 0 (core), Class 1 (coupled), Class 2 (isolated)** | Maps to our actual architecture. Kill-switchable demotion = capability tightening, not address-space relocation. |
| **D3** — No IC1 in-kernel co-location | **HARD REJECT** | TCB inflation + memory-safety cost = HarmonyOS's demonstrated weak axis. Rust/FV-first + Article 1.2. |
| **D4** — I-14 + lock band interaction | **Class 2 = I-14 floor; class change removes shared state, never adds locks** | Decoupling by subtraction, not by adding synchronization. |
| **D5** — Build recommendation | **DEFER** — governance framework, not a feature. Document in THINK; formalize when a build needs it. | No subsystem currently needs multiple coupling levels. Low priority. |

*This is a design THINK. No code was modified.*
