<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 BUILD-P3 — cost-aware proactive migration (the `residency:` retain/migrate decision)

**Date:** 2026-06-10
**Seat:** PM/Cerebro (design-questions + deep-research pass; not a dispatch)
**Context:** The policy layer is two builds deep and both are silicon-/QEMU-proven: **P1** residency-aware *placement* (§9.9 — the kernel decides *where an op runs*) and **P2** LRU *eviction* (§9.10 — the kernel decides *what leaves a full device*). Both are **reactive**: they decide at the moment of a placement, in response to where operands already are or to pressure that has already arrived. P3 is the **proactive** arm — the kernel deciding to *retain* a tensor it could evict, or to *migrate ahead of need*, because a cost model says the move pays off before the access happens. It is the third candidate from the policy-layer THINK (`THINK-2026-06-08-phase-7-policy-driven-placement.md` §4(C)), and the first policy build that has to confront concurrency.
**Related:** ANSPEC-001 §9.5 (accounting/recency), §9.9 (placement), §9.10 (eviction); `kernel/src/ai/evict.rs:127–133` (the documented SMP temporal-isolation assumption this build must revisit); ADR-036 (placement), ADR-037 (eviction).

---

## 1. The decision P3 makes (stated precisely)

P1 answers *"where does this op run?"* at dispatch. P2 answers *"who leaves when the device is full?"* at the moment room is needed. Both are driven by a request that has **already arrived**. P3 introduces the first decision the kernel makes about a request that **has not arrived yet**:

```
residency: tensor=7 device=Accel(1) RETAINED  reason=reuse-soon (next-use in 1 op, evict-cost 32KiB > keep-cost 0)
residency: tensor=4 device=Accel(1) → cpu     reason=cold-demote (no reuse predicted, headroom low: 240/256KiB)
residency: tensor=9 cpu → device=Accel(1)      reason=prefetch (predicted next-use, transfer 16KiB < idle headroom)
```

The tell from the policy-layer conversation — *"retained because the scheduler reasoned it would be needed"* — is exactly the `RETAINED` line. P3 is where the residency trace stops reading like bookkeeping and starts reading like a memory manager **anticipating** the workload. This is the proactive analogue of P2: P2 *reacts* to pressure; P3 *acts before* it, to avoid the pressure (and the eviction round-trip) entirely.

**The cost model is the new substance.** Every prior policy was a structural rule (prefer-resident; coldest-first). P3 is the first build with a genuine *cost/benefit comparison*: keep-cost (the device bytes held idle) vs evict-cost (a future re-migration if the prediction is wrong), and prefetch-benefit (a fault avoided) vs prefetch-cost (bandwidth + the bytes occupied speculatively). A wrong cost model doesn't corrupt — it thrashes. **That is the central risk, and the prior art's loudest warning** (see §3).

---

## 2. The load-bearing architectural fork (the one decision that defines what P3 *is*)

There are two genuinely different builds hiding under "proactive migration," and they are **mutually exclusive** for the first cut. They differ on *when* the proactive decision executes relative to the executor pass:

### Path A — **Synchronous, decision-only** (cost-aware retention at pass boundaries)
The cost model runs **inside the executor pass**, at the same seams P1/P2 already own: at each op's placement, and at P2's `ensure_room` consult. Instead of P2's pure "evict coldest until room," the policy now also decides — *synchronously, with the pass still the only mutator* — whether to **retain** a would-be victim (because reuse is imminent and a different victim costs less) or **demote** a cold tensor early (to bank headroom for a predicted next op). Prefetch, if included, is "migrate operand to the target device one op *earlier* than its first use, still within the same single-threaded pass." No new thread, no background daemon.

- **Preserves the §9.10 invariant verbatim:** "this executor pass is the only mutator of its value table; requests run to completion one-per-device." `evict.rs:127–133` stays true. **No new `unsafe`, no new lock-order edges, no concurrency.**
- **Host-testable as a pure policy** exactly like P1/P2: `decide_residency(snapshot, predicted_uses, capacity, costs) → {retain | demote | prefetch | noop}`. Byte-for-byte result-invariant (it changes *where*, never *what*).
- **Limitation:** the "proactive" horizon is bounded by what the pass can see — the op sequence it's executing. It cannot prefetch against a *future request from another tenant* because that lives outside the pass. That is real, but it is the honest scope of "the kernel reasons about residency" without yet taking on concurrent movement.

### Path B — **Asynchronous background mover** (a residency daemon)
A kernel thread migrates tensors **concurrently with live dispatch** — demoting cold tensors and prefetching predicted-hot ones while another op runs on the device. This is the Linux `kswapd`/AutoNUMA/TPP model (background promotion/demotion) and the CUDA UVM async-prefetch model.

- **Directly relaxes the documented invariant.** The pass is no longer the only mutator; a tensor can migrate *out from under* an in-flight op's residency snapshot. This is precisely the case `evict.rs:131` flags: *"If that discipline ever relaxes (BUILD-P3+ concurrency), this consult needs a device-level reservation."*
- **Requires importing the hardest correctness problem in the whole policy layer** — concurrent migration safety — into the same build as the brand-new cost model. The prior art's answer to that problem is well-defined but non-trivial (the Linux *migration-entry* protocol, §4), and our `tensor_migrate` is storage-**replacing**, not a PTE-remap, so we'd need either a device-level migration reservation or a migration-in-progress placeholder that makes a concurrent accessor block-and-retry.

**PM recommendation: Path A for P3; carve Path B into its own named build (P4 — concurrent residency management).** Rationale below in §6. Briefly: every prior policy build won by being *pure policy over a proven mechanism, host-testable, result-invariant, zero new unsafe, zero concurrency* — and that discipline is exactly why P1/P2 landed clean and falsifiable. Path A keeps that contract; Path B breaks all five properties at once and couples an unproven cost model to an unproven concurrency mechanism, so a gate failure couldn't be localized to one or the other. The project's own standing rule against confident "one-hop" dispatches (CLAUDE.md, the blind-hardware lesson) applies by analogy: do the cost model and the concurrency seam as *separate, individually-falsifiable* builds.

---

## 3. Prior art — four traditions, and what each tells us

### (a) CUDA Unified Memory / GPU memory oversubscription — *the closest analogue*
This is literally our problem: a device with bounded memory, oversubscribed, with a host tier to spill to. The findings:
- The CUDA runtime's **default victim policy is LRU** — i.e. our P2 is the industry baseline, which validates the P2 cut.
- **Prefetch (`cudaMemPrefetchAsync`) proactively migrates data to the device before access, reducing page faults** — this is exactly P3's prefetch arm, and confirms it's a real, separable win on top of LRU.
- The decisive warning: **"during oversubscription, aggressive prefetching causes page thrashing for irregular applications … forcing the runtime to keep evicting pages."** NVIDIA's research answer is a **tree-based neighborhood pre-eviction** coupled to the prefetcher, beating LRU-4KB by ~93% *and* LRU-2MB by ~18.5% — but only because eviction and prefetch are **coordinated**. **Takeaway for us:** P3 must not prefetch/retain against a noisy signal; the cost model needs a *reuse prediction* it can trust, and a **thrash guard** (a hysteresis / "don't prefetch if it would force an eviction of something predicted-hot") is a first-class requirement, not a refinement.
  Sources: [NVIDIA — Improving GPU Memory Oversubscription Performance](https://developer.nvidia.com/blog/improving-gpu-memory-oversubscription-performance/), [Ganguly et al., ISCA 2019 — Interplay between Hardware Prefetcher and Page Eviction](https://people.cs.pitt.edu/~debashis/papers/ISCA2019.pdf), [Performance Evaluation of Advanced Features in CUDA Unified Memory (arXiv:1910.09598)](https://ar5iv.labs.arxiv.org/html/1910.09598).

### (b) Linux tiered memory — AutoNUMA, TPP — *the proactive-demotion architecture*
The OS-design precedent for "move pages between a fast and a slow tier based on access behavior, transparently":
- **AutoNUMA** samples a subset of memory, takes **NUMA hint faults**, and promotes remotely-accessed pages to local memory. The signal is *access faults*, not an oracle — a cheap, sampled approximation of "hot."
- **TPP (Meta, ASPLOS'23)** is the strongest design match: **proactive demotion** of cold pages to the slow tier, **decoupled allocation/reclaim to maintain a headroom of free pages on the fast tier**, and **reactive promotion** of pages that turn out hot in the slow tier. Crucially TPP **separates demotion (proactive, background) from promotion (reactive)** — and works *without prior knowledge of the access pattern.*
- **Takeaway for us:** TPP's "maintain headroom proactively, promote reactively" maps cleanly onto our split: P3-A can do **proactive cold-demote to bank headroom** (so the *next* placement doesn't trigger a P2 eviction round-trip) synchronously, while leaving aggressive prefetch/promotion conservative. And TPP's decoupling is itself an argument that **demotion and promotion are separable builds** — supporting the P3(retain+demote) / P4(concurrent prefetch) split.
  Sources: [TPP (arXiv:2206.02878)](https://arxiv.org/pdf/2206.02878), [LWN — Transparent Page Placement for Tiered-Memory](https://lwn.net/Articles/876993/).

### (c) Linux page-migration mechanism — *the answer to the SMP seam, when we get to it*
How Linux moves a page that might be accessed concurrently, without corruption:
- The page is **locked**, and its PTE is replaced with a **migration entry** — a special non-present marker. A concurrent accessor faults on the migration entry, **waits on the page lock, then redoes the fault** and reaches the new location. `PG_isolated` under `lock_page()` prevents two CPUs isolating the same page.
- **Takeaway for us:** this is the *exact* mechanism Path B needs and the reason `evict.rs:131` says "needs a device-level reservation." Our `tensor_migrate` replaces storage rather than remapping a PTE, so our analogue is a **migration-in-progress reservation** on the `(device, tensor_id)`: a concurrent op that resolves a tensor mid-migration must block-and-retry (or be admitted only after the move commits). This is buildable — but it is a *mechanism* build (a new synchronization primitive on the residency path), which is why it belongs in P4, not bundled into the cost model.
  Sources: [The Linux Kernel — Page migration](https://docs.kernel.org/mm/page_migration.html), [LWN — Making kernel pages movable](https://lwn.net/Articles/650917/).

### (d) LLM-serving memory management — *the workload P3 actually serves*
Since LogansOS is AI-native, the relevant "applications" are inference engines, and the state of the art is all about exactly this tiering:
- **FlexGen** stores weights/attention/activations on CPU and **swaps in only when data is needed and not on the GPU** — reactive, like P2.
- **vLLM / PagedAttention** blocks the KV cache into fixed pages for non-contiguous residency — the *granularity* lesson: move in fixed blocks, not whole tensors, to bound migration cost (mirrors CUDA's 64KiB basic block).
- **InfiniGen** keeps the KV cache in CPU memory and uses **predictive prefetching that anticipates which tokens will be needed before they are required** — this is the proactive arm done with a *workload-specific* predictor.
- **Takeaway for us:** the kernel cannot know token-level access patterns (that's the application's job), but it *can* see the **op dependency sequence within a forward pass** — that is our trustworthy, low-noise reuse signal (operand of op N+1 is predicted-hot during op N). P3's predictor should be **structural (the op graph), not statistical** — which is both more honest (no learned model in the kernel) and exactly what Path A can see synchronously.
  Sources: [Pie: Pooling CPU Memory for LLM Inference (arXiv:2411.09317)](https://arxiv.org/pdf/2411.09317), [A Survey on KV Cache Management (arXiv:2412.19442)](https://arxiv.org/pdf/2412.19442).

---

## 4. The SMP temporal-isolation seam (the question that forced this THINK)

The documented assumption, verbatim from `kernel/src/ai/evict.rs:127–133`:

> *the snapshot→select→migrate sequence is non-atomic across the accounting/recency locks, which is sound because accelerator requests run to completion one-per-device (temporal isolation — see `accounting.rs` INFLIGHT) and this executor pass is the only mutator of its value table. If that discipline ever relaxes (BUILD-P3+ concurrency), this consult needs a device-level reservation; until then the alloc/dispatch OOM backstops in `exec.rs` are the safety net.*

Reading P3 against this:
- **Under Path A, the assumption holds unchanged.** P3-A's retain/demote/early-prefetch all execute *inside* the pass that owns the value table; the device is still run-to-completion, one request at a time. P3-A is a strict extension of the P2 consult — it adds *reasons* (retain, cold-demote) and possibly *moves the same kind of migration earlier*, but it introduces **no second mutator**. The invariant comment can stay; P3-A should *update it to record that the proactive decision was added without relaxing the single-mutator property.*
- **Under Path B, the assumption is exactly what breaks**, and prior art (§3c) tells us the fix: a device-level migration reservation / migration-in-progress placeholder so a concurrent accessor blocks-and-retries. **That mechanism is P4's deliverable.** Bundling it into P3 means the gate can't isolate a cost-model bug from a concurrency bug.

This is the crisp resolution of the open question carried out of P2: **P3 (Path A) does not need to revisit the SMP seam — it preserves it. The seam is revisited by P4, as a dedicated mechanism build, and §3(c) already names the prior-art mechanism it will implement.** That is the honest answer: we don't relax the invariant until we have the primitive that makes relaxing it safe, and we build that primitive on its own falsifiable gate.

---

## 5. The cost model (what P3-A actually computes)

Minimal, structural, host-testable — in the P1/P2 mold. Inputs are all already in place (the reason this is the next build):
- **Reuse prediction (structural, not statistical):** from the op sequence the executor is running, a tensor is *predicted-hot* if it is an operand of an upcoming op within a small lookahead window `W` (start `W = 1`: "needed by the next op"). This is the trustworthy, low-noise signal §3(d) argues for — no learned model, no access-counter sampling, just the dependency graph the pass already holds.
- **Recency (from P2):** the §9.5 recency extension — coldest-first remains the demotion order among *not-predicted-hot* tensors.
- **Capacity / headroom (from §9.5):** `resident / capacity`; a **headroom target** (TPP's idea — keep a small free margin so the next placement avoids an eviction round-trip).
- **Migration cost (bytes):** transfer size = the tensor's bytes; the only "cost" unit the kernel can honestly measure without a perf model. Keep-cost = bytes held idle; evict-cost = bytes that must re-migrate if the prediction was right.

The decision (pure function, deterministic):
1. **Retain** a would-be P2 victim if it is predicted-hot within `W` (evicting it just forces a re-migration next op — net-negative). Pick a *different* (cold, larger-or-equal) victim instead; if none, fall through to P2's existing behavior.
2. **Cold-demote** proactively only when headroom is below target *and* the demoted tensor is **not** predicted-hot — banking room for the predicted next placement. Bounded: never demote below what the predicted-hot set needs.
3. **Early-prefetch** (optional, gated, off by default for the first cut): stage a predicted-hot operand to the target device one op early *only if* it would not force eviction of another predicted-hot tensor (the §3(a) thrash guard, made a hard precondition).
4. **No-op** otherwise — P3 is a strict superset of P2: with `W=0` / no prediction, it reproduces P2 exactly.

**Thrash guard (first-class, per §3(a)):** never make a proactive move whose own predicted consequence is a future eviction of a predicted-hot tensor. A proactive migration that the model itself predicts will be undone is a bug, not a tradeoff — the same result-invariance discipline P1/P2 used, applied to *movement* instead of *result*.

---

## 6. Why Path A is the recommended P3 (the PM call to ratify)

1. **It keeps the five properties every prior policy build won on:** pure policy over a proven mechanism, host-testable, byte result-invariant, zero new `unsafe`, zero concurrency. P1 and P2 landed clean and *falsifiable* because of exactly these; the gate could attribute any failure to the policy logic alone.
2. **It decouples the two hard, independent risks.** The cost model (new, can thrash) and concurrent migration (new, can corrupt) are *orthogonal* problems. The blind-hardware lesson in CLAUDE.md is explicitly about not bundling an unproven thing with another unproven thing in one confident dispatch: do them as separate, individually-diagnostic builds. Path A is the cost model alone, on a deterministic gate; P4 is the concurrency mechanism alone, with its own §3(c)-derived discriminator.
3. **Prior art independently supports the split:** TPP separates proactive demotion from reactive promotion; CUDA's research shows prefetch is only safe when *coordinated* with eviction (i.e. the cost model must be solid *before* you let movement run async). Building the cost model first, synchronously, is the prerequisite for ever doing it concurrently.
4. **It is a real, demonstrable governance step on its own:** the `residency: … RETAINED reason=reuse-soon` and `cold-demote` traces are genuine proactive reasoning — the kernel acting on a prediction, banking headroom, refusing a net-negative eviction. That is a complete, honest milestone, not a stub.

The cost: P3-A's proactive horizon is bounded by the executor pass (it can't prefetch across tenants or ahead of a not-yet-submitted request). That is the precise, honest scope, and P4 (the concurrent residency daemon, on the migration-reservation primitive) is the named build that lifts it.

---

## 7. Scope guard (what P3 is NOT)

- **NOT** a background/async mover, a kernel residency daemon, or concurrent-with-dispatch migration → that is **P4**, gated on the migration-reservation primitive (§4).
- **NOT** a learned/statistical predictor or access-counter sampling → the predictor is **structural** (the op dependency graph), kernel-honest, no model in the kernel.
- **NOT** a perf/bandwidth cost model → cost is measured in **bytes** (the only unit the kernel can assert without a timing oracle); no wall-clock, no estimated transfer latency.
- **NOT** a change to *what* any op computes → result-invariant, same byte-oracle discipline as P1/P2.
- **Does NOT** relax `evict.rs`'s single-mutator invariant → it preserves and re-documents it.
- **Does NOT** add eviction-to-another-accelerator, cross-tenant prefetch, or sub-tensor block migration (the vLLM-paging granularity idea is noted as future, not in scope).

---

## 8. Design questions for the PL

- **Q1 — Adopt Path A (synchronous, decision-only cost-aware retention + bounded proactive cold-demote) as BUILD-P3, and carve the async background mover + migration-reservation primitive into a separate BUILD-P4?** *(Recommended — §6. Keeps the proven-mechanism / host-testable / result-invariant / no-new-unsafe / no-concurrency contract intact and decouples the cost-model risk from the concurrency-correctness risk.)*
- **Q2 — Reuse signal: structural (op-dependency lookahead window `W`, start `W=1`) rather than statistical/access-counter?** *(Recommended — §3(d), §5. Trustworthy, low-noise, no learned model in the kernel.)*
- **Q3 — Include early-prefetch (decision arm 3) in P3, or defer it to P4 and ship P3 as retention + proactive cold-demote only?** *(PM lean: ship P3 as **retain + cold-demote** (the TPP "proactive demotion to bank headroom" half, fully synchronous and low-thrash-risk); defer **prefetch/promotion** to P4 where it can be both async and properly coordinated with eviction per §3(a). This makes P3 the cleanest possible cost-model-only gate.)*
- **Q4 — Is "cost measured in bytes only" (no timing/bandwidth model) the right honesty bar for now?** *(Recommended — §5, §7. The kernel asserts what it can measure.)*

---

## 9. If ratified — the slice (preview of the dispatch)

- **BUILD-P3 (Path A, Q3-lean):** extend the P2 `ensure_room` consult and the placement seam with a pure `residency_policy::decide(...)` that adds **retain** (don't evict a predicted-hot tensor; pick a colder victim) and **proactive cold-demote** (bank headroom below target, never demoting predicted-hot). New `residency:` trace lines. Pure/host-testable (`tools/ai-ops-tests` path-`mod`), result-invariant, strict superset of P2 (with no prediction → P2 verbatim). Gate (QEMU, no silicon): a forward pass where a predicted-hot tensor that P2 *would* have evicted is **retained** (and a different cold tensor demoted instead), the proactive demote banks headroom so the next placement takes **zero** eviction round-trips, residency stays ≤ capacity throughout, no predicted-hot tensor is ever moved out from under its op, and the output is **byte-identical** to the P2/unbounded reference. Open **ANSPEC-001 §9.11** (the proactive residency model — structural reuse prediction, retain/demote decision, the headroom target, the thrash guard, result-invariance) + an ADR. No blind-hardware run → no failure-mode/discriminator section.
- **BUILD-P4 (Path B, the named follow-on):** the **migration-reservation primitive** (the §3(c)/§4 device-level placeholder that makes a concurrent accessor block-and-retry) + the async residency daemon (prefetch/promotion concurrent with dispatch). This is the build that *relaxes and re-proves* the `evict.rs` single-mutator invariant — a mechanism build with its own concurrency gate (NEO_SMP sweep, the migration-race discriminator).

With P3, the kernel stops merely *reacting* to memory pressure and begins *anticipating* it — refusing a net-negative eviction, banking headroom for work it predicts is coming. That is the last reactive→proactive step the policy layer can take *without* concurrency; P4 is the one that takes it concurrent, safely, on a primitive the prior art has already mapped.
