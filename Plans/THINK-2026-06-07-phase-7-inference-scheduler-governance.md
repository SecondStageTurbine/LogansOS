<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 — Inference scheduler / multi-tenant governance (the decision-making phase)

**Date:** 2026-06-07
**Seat:** PM/Cerebro (phase-opening design-questions pass; not a dispatch)
**Context:** Phase 6 closed 2026-06-07 (`fff256d`) — the `accel` driver class + a real Intel GPU running matmul/add/GELU with mixed-precision CPU fallback + tensor migration, bare-metal verified. Phase 6 proved **sovereignty of execution**. Phase 7 is **sovereignty of decision-making**.
**Related:** ADR-030 (driver-class substrate), ADR-032 (Intel Gen9 driver; names execlist/PPGTT as the next mechanism), ANSPEC-001 §9 (→ §9.5+ this phase), bible Articles 2.2/2.3/2.4/6.4/6.5.

## 1. The thesis Phase 7 tests
Phase 6 showed the kernel can **own** AI execution. Phase 7 must show the kernel can **govern** it — and that the governance is something a userspace+vendor-runtime stack structurally cannot reproduce. The kernel earns its seat next to CPU time and memory pages only if it can make **enforceable cross-tenant decisions over device-resident tensors** between mutually-untrusting principals. That is the whole research question; everything below serves it.

The necessity argument (worth stating, because it bounds the design): a userspace tensor-broker *can* do placement, migration, even capability semantics. What it cannot do is make those decisions **enforceable at the hardware boundary**, because the CUDA/OpenCL performance model hands the device to userspace. LogansOS's leverage is the **fusion**: capability policy + kernel-owned device memory. Neither half clears the bar alone; the fusion is the part the userspace world can't have. So Phase 7's deliverables must always land on the *enforceable* side, not the merely-decidable side.

## 2. The success metric is a log line (per the governance test)
Each build is "done" when the silicon/QEMU trace contains a specific governance decision that is structurally impossible above the kernel. This is the gate, stated up front:

| Build | The trace that proves it | What it demonstrates |
|---|---|---|
| Accounting | `accel: PID 7 used N ms / M bytes on Accel(1)` | the kernel *sees* per-principal accelerator use (can't govern what you can't measure) |
| Arbitration | `infer PID 12 deferred — PID 7 reservation active (quota 80%)` | a cross-tenant decision: A's work waits because of B's claim |
| Sharing/dedup | `tensor X shared RO {PID 7, PID 12} — 1 physical copy (dedup hit)` | global-view ownership a per-process runtime is blind to |
| Boundary enforcement | `PID 12 RW denied at device boundary; PID 7 RO granted — concurrent` | the *enforceable* form: authority survives to device-memory translation |

The last line is the thesis instantiated. The first three are demonstrable on the **proven** substrate; the fourth requires new hardware-isolation work (§5).

## 3. What exists vs. what's greenfield
- **Exists (Phase 6 + earlier):** `AccelDriver` trait + registry; `DeviceId` residency + `DeviceStorage`; `tensor_migrate`; the inference syscall (`SYS_INFER_RUN`, Phase 5); tensors as **capability-gated handles** (Phase 5); the **capability system** with grant/derive/revoke through a derivation graph (Phase 2); NeoFS content-addressing (Phase 3). Single-device dispatch.
- **Greenfield (confirmed: no scaffolding today):** any accelerator accounting, any scheduling/admission/quota/reservation, any cross-process tensor sharing path, any per-context device-memory isolation.

The capability machinery is the most under-leveraged existing asset — tensor sharing/dedup is mostly *wiring an existing mechanism*, not building a new one.

## 4. Design questions (resolve before dispatching)

- **Q1 — The schedulable unit.** An inference *request* (a `SYS_INFER_RUN` invocation), an op-batch, or a model-residency reservation? **Recommendation: the inference request.** It is request-shaped and bounded — which is *exactly* why inference (unlike training) is tractable as a kernel-arbitrated resource. Training stays out of scope (bible). The bounded shape is what lets the kernel reason at all.

- **Q2 — Resource shape: admission control, NOT preemptive timeslicing (the load-bearing call).** Accelerators largely run to completion; mid-kernel preemption is coarse/absent. So the kernel can decide *who runs next* but not *interrupt who runs now*. Phase 7's scheduler is therefore **admission + priority + quota + reservation + accounting** — the storage/batch-I/O vocabulary the kernel already implements (block layer, IO admission), **not** the CPU-scheduler vocabulary (timeslice, preemption, fair-share rotation). The "chair" is smaller and a different shape; that is a different *species* of OS resource, not a failure. Confirm this framing — it shapes the entire scheduler.

- **Q3 — Enforcement: temporal-first, spatial as hardening (the strategic sequencing call).** Two ways to keep tenants from reading each other's device memory: **temporal** (serialize tenants on the device, flush between — works on any accelerator, no new driver work) or **spatial** (per-context page tables / PPGTT / SR-IOV — concurrent, but a major per-architecture driver lift). **Recommendation: temporal-first.** It demonstrates the governance thesis (arbitration, sharing, accounting all work under serialized execution) without the PPGTT rewrite, and it is the *only* option on accelerators that can't partition memory at all. Spatial isolation is the build that *upgrades* enforcement from serialized to concurrent and makes the boundary-enforcement trace (§2 row 4) non-vacuous. Policy stays uniform/in-kernel; enforcement varies per-driver — and the kernel must **honestly represent** which fidelity it can deliver per device (claiming concurrent governance on a serialize-only device would be a lie).

- **Q4 — In-kernel vs. userspace policy (honesty about necessity).** The necessity argument clears only a narrow core in-kernel: the **enforcement point + device-memory ownership**. Tensor *policy/semantics* could live in a userspace server (the seL4 stance). LogansOS puts the scheduler in-kernel because it is **monolithic by conviction** (rejects the IPC tax + the microkernel split) — a legitimate design choice, but one layered *on top of* the genuine necessity, not demanded by it. The THINK records this so the phase doesn't over-claim "all of this must be in ring 0." It must not.

- **Q5 — The capability model for cross-process tensors.** Use the existing Phase-2 machinery uniformly: a tensor capability **derived** with reduced rights (RO), **granted** to another process via capability-mediated IPC, **revoked** through the derivation graph. No new authority primitive — tensors are already cap objects. The new work is the *device-side* enforcement that a granted RO cap cannot become a device-memory write.

## 5. Proposed build sequence (dependency-honest)
Front-load the demonstrable thesis on the proven substrate; defer the hard hardware-isolation lift to where it *upgrades* rather than *gates*.

- **BUILD-1 — Accelerator accounting.** Per-principal (per-process/capability) accounting of accelerator residency (bytes) and dispatch time on each `Device::Accel(n)`. Pure bookkeeping, no hardware dependency, host+QEMU testable. *Foundation: you cannot arbitrate what you cannot measure.* Gate: the accounting trace line.

- **BUILD-2 — Admission-control inference scheduler.** A per-device request queue with priority, per-principal quota, and reservation; the kernel orders inference requests (temporal, run-to-completion — Q2/Q3). Produces the first **cross-tenant arbitration decision**. Needs new multi-tenant test infrastructure (contending inference workloads). Gate: the deferral trace line. This is the core of "sovereignty of decision-making."

- **BUILD-3 — Capability-mediated tensor sharing + dedup.** Cross-process tensor cap grant/derive/revoke (Q5) + single-physical-copy dedup for shared model/weight tensors (leaning on NeoFS content-addressing + tensor refcounting). Mostly wiring existing mechanisms. Gate: the shared-RO / dedup-hit trace line. *Nearest milestone — most existing substrate.*

- **BUILD-4 — Spatial per-context device isolation (the hard, hardware-dependent build).** Execlist/LRC + PPGTT on the Intel driver (named in ADR-032) so concurrent tenants have *enforced* device-memory boundaries — making the boundary-enforcement trace non-vacuous. This is a major driver lift (treat it like the BUILD-4a bring-up: high-risk, likely multi-iteration, **failure-mode/discriminator section mandatory**). Resolves the temporal→spatial enforcement upgrade and the per-device fidelity model (Q3). May itself slice (PPGTT mechanism, then per-context switch, then the deny-at-boundary test).

Ordering rationale: BUILD-1→3 demonstrate that *the kernel governs* using temporal isolation and existing capabilities — the thesis is shown before the hardest hardware work. BUILD-4 turns "governs (serialized)" into "governs (concurrent, hardware-enforced)" — the strongest form, but not a prerequisite for the conceptual proof.

## 6. Genuine tensions / risks (name them now)
1. **The scheduler is meaningless without enforcement teeth.** Admission decisions only matter if a deferred/denied principal *cannot* bypass them — which loops back to device-memory ownership (already true: no userspace BARs) and, for concurrency, to BUILD-4's per-context isolation. A scheduler over a device any tenant can directly touch would be advisory theater. Keep policy and enforceability coupled in every build.
2. **The PPGTT driver tax is per-architecture and is the deepest layer of the driver tax.** Governance can never be cleaner than the driver can enforce. BUILD-4 on Intel does not transfer to AMD — each accelerator's isolation hardware differs (PPGTT vs SR-IOV vs *nothing* → temporal-only). This is a legitimacy cost, not just a scaling cost.
3. **Multi-tenant testing is new infrastructure.** Every prior phase tested one workload; Phase 7 needs *contending* principals with capability isolation. The test harness itself is a deliverable (BUILD-2).
4. **Don't let policy outrun mechanism (lean-patch, Article 15.7).** No quota engine before accounting can measure; no spatial isolation before temporal proves the thesis. Build the floor before the ceiling.
5. **Honest fidelity reporting is itself a kernel responsibility.** Part of "the kernel governs AI" is the kernel *knowing and truthfully exposing* how strongly it can govern each device (spatial vs temporal vs none).

## 7. Standing-rule requirements (apply per build)
- **Blind-hardware rule:** BUILD-4 (and any bare-metal scheduler/isolation run) requires the expected-failure-mode + discriminator section before DISPATCHED.
- **Byte-oracle rule:** any on-disk/on-wire format introduced (an accounting record, a reservation descriptor, a scheduler-state dump) needs a hand-laid byte oracle.
- **Independent-verification:** the governance trace lines are the gates; they must be asserted, not eyeballed.

## 8. Open decisions for the PL (before BUILD-1 dispatch)
1. **Q2** — confirm the **admission-control (not preemptive) resource shape**. (The single framing call.)
2. **Q3** — confirm **temporal-first, spatial-as-hardening** enforcement sequencing.
3. **Q1** — confirm the **inference request** as the schedulable unit.
4. Build-order: accept the BUILD-1→4 sequence (accounting → arbitration → sharing/dedup → spatial isolation), or resequence (e.g., sharing/dedup before arbitration, since it has the most substrate).

## 9. Recommendation in one line
Open Phase 7 with **accelerator accounting** (BUILD-1) → **admission-control inference scheduler** producing the first cross-tenant deferral (BUILD-2) → **capability-mediated tensor sharing + dedup** (BUILD-3), all on temporal isolation over the proven substrate, then **per-context spatial isolation** (BUILD-4, the hard Intel PPGTT lift) to make the device-boundary enforcement concurrent and non-vacuous. The thesis is demonstrated by BUILD-3 and *hardened* by BUILD-4 — and each build is gated on producing the specific governance log line that proves a userspace stack structurally couldn't.
