<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 BUILD-P4 — concurrent residency (the kernel moves tensors while it works)

**Date:** 2026-06-10
**Seat:** PM/Cerebro (design-questions + deep-research pass; not a dispatch)
**Context:** The policy layer is reactive→proactive-complete *without concurrency*: P1 places (§9.9), P2 reclaims under pressure (§9.10), P3 anticipates pressure synchronously (§9.11). Every one of those decisions executes **inside the executor pass, which is the only mutator of its value table** — the invariant documented at `kernel/src/ai/evict.rs:126–135` and preserved verbatim by P3. BUILD-P4 is the build that **relaxes and re-proves** that invariant: a tensor moves between host and device *concurrently with live dispatch*, so the kernel can prefetch the next op's operands and demote cold tensors in the background instead of stalling a pass to do it. This is the first policy build with a real **SMP-race surface**.
**Related:** ANSPEC-001 §9.9–§9.11; `evict.rs:126–135` (the single-mutator invariant P4 relaxes); `accounting.rs` (`INFLIGHT` run-to-completion, `TABLE`/`RECENCY` under `SwitchMutex`); `admission.rs` (the documented lock order `QUEUES → TABLE → scheduler`; the `block_current_with_token_and_guard` park-under-lock pattern); ADR-019 (`SwitchMutex` IRQ-off-by-construction); the P3 THINK §3(c)/§4 (which named the migration-reservation primitive as P4's deliverable); the `sys_wait` SMP-race precedent (`c0a3b04`, root-caused via a 50/50 NEO_SMP=4 sweep).

---

## 1. The decision P4 makes — and the invariant it touches
P3's `residency:` trace reasons *ahead* but acts *synchronously*: a demote or a retain happens at an op boundary, with the device idle and the pass holding every reference. P4 makes the move **overlap** with work:

```
residency: tensor=9 cpu → device=Accel(1)  reason=prefetch  (predicted next-use op=matmul, daemon, transfer=16384)
residency: tensor=4 device=Accel(1) → cpu  reason=bg-demote (headroom low 240/256KiB, daemon, while Accel(1) busy with op=gelu)
```

The two words that did not appear before P4 are **`daemon`** and **`while … busy`**. That is the whole build: a mover that runs *next to* the executor, not inside it. And it is exactly the thing `evict.rs:131` warned about — *"If that discipline ever relaxes (BUILD-P3+ concurrency), this consult needs a device-level reservation."* P4 is where we build that reservation and earn the right to relax the invariant. **The deliverable is not "a faster mover" — it is a proof that concurrent residency movement cannot corrupt a tensor, lose accounting, deadlock, or change a result.**

---

## 2. The hard constraint that shapes everything: `migrate` needs exclusive ownership
`Tensor::migrate(&mut self, …)` requires `&mut`, and the P2/P3 glue obtains it through `Arc::get_mut` — which **refuses unless the caller holds the only strong reference and no `Weak` exists**. This is not an accident to be worked around; it is the property that let P2/P3 ship with **zero new `unsafe`**: the type system proves no one else can read a tensor's bytes while it is being moved. Two consequences for P4:

1. **A tensor an op is actively using can never be migrated concurrently** — the executing node holds a clone, so `strong_count ≥ 2`, so `get_mut` refuses. This is *correct and desirable*: you never want to move what the device is reading. P4's daemon therefore targets only **(a) cold tensors no pass references** (background demote) and **(b) prefetch staged into *new* storage** that becomes a referenced tensor only at the instant the swap commits.
2. **"Concurrent migration" in our world is not Linux's in-place PTE remap.** Linux moves the *same* page and redirects PTEs; our tensors *own* their storage (`TensorStorage::Host | Device`), so our move *replaces* storage. The reservation primitive therefore guards a **storage-swap commit point**, not a shared mapping — which is simpler to reason about (no rmap walk) but needs its own correctness argument (§4).

This is the P4 analogue of P3's central fork, and it has a clean recommended resolution (Fork-1 below).

---

## 3. Prior art — four mechanisms, and exactly what we borrow from each

### (a) Linux page migration — *the reservation/fault-and-wait protocol* (borrow the shape, not the mechanism)
Linux isolates the folio from the LRU and **bumps its refcount so it can't be freed mid-migration**, converts every PTE to a **migration entry** (a non-present placeholder), locks the new page so **accessors block on the page lock or on the migration entry**, copies, then drops the locks — *"processes waiting on the page lock will redo their page faults and reach the new page."* Recent patches add the lesson that the mover should **not block forever on a contended page — bail and try another** (`MIGRATE_SYNC_LIGHT`).
- **What we borrow:** the *state-machine shape* — a tensor gains a **migration state** `Resident → Reserved(Migrating) → Resident'`; a concurrent accessor that finds `Reserved` **blocks-and-retries** (the "redo the fault" analogue); and a **claimed bit** (the `folio_isolate_lru` / `PG_isolated` analogue) so two movers can't pick the same victim. And the *best-effort* discipline: **the daemon never blocks on a contended (referenced) tensor — it skips it.** Our refcount-bump-so-it-can't-be-freed is already given by the `Arc` we hold while moving.
- **What we do NOT borrow:** PTE migration entries / rmap walks — we have no shared tensor page tables; the swap is an `Arc`-pointer commit.
  Sources: [The Linux Kernel — Page migration](https://docs.kernel.org/mm/page_migration.html), [LWN — Page migration](https://lwn.net/Articles/157066/), [migrate_pages: don't wait forever locking pages (patchwork)](https://patchwork.kernel.org/project/linux-mm/patch/20230421151135.v2.3.Ia86ccac02a303154a0b8bc60567e7a95d34c96d3@changeid/).

### (b) kswapd / kcompactd / DAMON — *the watermark-driven background-daemon model*
kswapd wakes at a **low watermark** and reclaims in the background **until free memory reaches a high watermark**, then sleeps; it wakes kcompactd for compaction. DAMON_RECLAIM is *"proactive and lightweight reclamation under light memory pressure … reducing the latency spikes incurred by direct reclaim."* The fault/alloc slowpath wakes kswapd and may still do **direct reclaim** itself if the daemon hasn't caught up.
- **What we borrow:** the **two-watermark hysteresis** (P3 already computes a headroom target `H`; P4 makes it a *low* watermark that wakes the daemon and a *high* watermark where it sleeps — this is the anti-thrash governor the GPU-oversubscription literature demanded). And the **direct-reclaim backstop**: if the daemon hasn't banked room when a placement needs it, the *synchronous* P2/P3 path still runs — **P4 is strictly additive over P3; the daemon is an optimization that can always fall back to the proven synchronous reclaim.** This is the single most important risk-reducer in the build.
  Sources: [DAMON-based Reclamation](https://docs.kernel.org/admin-guide/mm/damon/reclaim.html), [kswapd/kcompactd watermark wakeup (kernel.org commit)](https://scm.linefinity.com/common/linux-stable/commit/5ecd9d403ad081ed2de7b118c1e96124d4e0ba6c).

### (c) NVIDIA UVM — *the overlap model, and the honest hardware boundary*
UVM overlaps kernel execution with migration: a faulting warp is stalled, the **fault is replayed** after the page arrives, and `cudaMemPrefetchAsync` stages data **before** access. But a sharp limitation surfaces: *"a single fault may trigger multiple evictions followed by fetches, but these transfers never overlap — eviction uses GPU→CPU bandwidth while fetching uses CPU→GPU, leaving half of PCIe idle."* And critically, overlap of *compute* with *DMA* requires the GPU's **copy engine** running independently of the EUs.
- **What we borrow:** the **fault-replay = block-and-retry** equivalence (an accessor that hits a `Reserved` tensor parks and re-resolves after commit — the kernel-side analogue of warp replay), and the prefetch-ahead idea.
- **What this tells us to DEFER:** *device-side* concurrent DMA — actually copying bytes over the bus *while the Intel GPU computes* — needs the Gen9 **blitter/copy engine** driven independently of the render ring. That is a silicon lift with its own blind-hardware risk. **P4 proves the *kernel-side* concurrency-coherence on accel_sim under QEMU SMP; real device-side compute/copy overlap is a named later hardware build.** (On accel_sim the "device memory" is kernel RAM, so the move is a `memcpy` under the reservation — the concurrency being proven is the *kernel data-structure coherence*, which is the actual hard part; the bus overlap is a hardware throughput optimization layered on top later.)
  Sources: [NVIDIA open-gpu-kernel-modules uvm_perf_prefetch.c](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/main/kernel-open/nvidia-uvm/uvm_perf_prefetch.c), [Interplay between HW Prefetcher and Page Eviction, ISCA'19](https://people.cs.pitt.edu/~debashis/papers/ISCA2019.pdf).

### (d) RCU vs seqlock — *and the discipline to NOT go lock-free prematurely*
RCU gives readers lock-free access with deferred (grace-period) reclamation; seqlock gives in-place updates with reader-retry on torn reads. Both shine at **high read-to-write ratios across many cores**.
- **What we borrow: the analysis, then the restraint.** Under a daemon, the residency tables (`TABLE`, `RECENCY`) become read-mostly (the daemon and passes read totals; writes are migrations). RCU/seqlock *would* reduce contention — **but LogansOS is single-node with a handful of tenants and a few cores; the contention RCU would save is negligible, while a hand-rolled RCU in a young kernel is exactly the kind of subtle concurrency bug the `sys_wait` race taught us to fear.** **Recommendation: keep `SwitchMutex` (proven, IRQ-off-by-construction, ADR-019); achieve correctness with the reservation state machine and short critical sections, not with a new lock-free primitive.** Revisit seqlock *only if* a NEO_SMP contention measurement shows the residency lock is hot — and treat that as its own dispatch, not a rider on P4. This is the same "resist premature mechanism" judgment that kept P1–P3 minimal.
  Sources: [InfoQ — Read-Copy-Update (RCU)](https://www.infoq.com/articles/read-copy-update/), [RCU & seqlock (Understanding the Linux Kernel)](https://www.linkedin.com/pulse/rcu-seqlock-understanding-linux-kernel-series-primitives-rajeev-misra).

---

## 4. The migration-reservation primitive (the mechanism P4 must build)
A per-`(device, tensor_id)` **migration reservation**: a small state machine, held under an existing `SwitchMutex` (no new lock primitive), with a **park/wake** for accessors that arrive mid-move (reusing the admission scheduler's proven `block_current_with_token_and_guard` park-under-lock pattern, so there is **no lost-wakeup window** — the token is installed while the lock is held and released atomically with the context switch).

```
state: Resident → Claimed → Migrating → committed (Resident' / rolled-back Resident)
```
- **Claim (CAS-style under the lock):** a mover claims a tensor only if it is `Resident`, **unreferenced by any pass** (`strong_count == 1`, `weak_count == 0` — the existing evictability gate), and not already claimed. Two movers cannot claim the same tensor (the `PG_isolated` analogue). A claim that loses is *skipped, never blocked* (the `MIGRATE_SYNC_LIGHT` lesson).
- **Migrate:** the claimed tensor is moved via the **existing `Tensor::migrate`** (no parallel residency path — same rule as P2/P3). Because it was claimed only when unreferenced, `get_mut` succeeds; because it is `Claimed/Migrating`, any pass that *tries* to take a new reference to it first checks the reservation and **parks** until commit (then re-resolves residency — the fault-replay analogue).
- **Commit:** the storage swap and the `TABLE`/`RECENCY` updates happen **atomically under the lock**; the reservation returns to `Resident`, parked accessors wake and re-resolve. **Accounting is updated in the same critical section as the swap** so totals can never tear (the §9.5 balance-to-zero invariant must hold under concurrency — a discriminator, §6).
- **Prefetch** is the mirror: stage CPU→device into *new* device storage behind the reservation; the tensor becomes device-resident at the commit point. If the predicted op never comes (misprediction), the staged copy is demoted back — bounded, byte-preserving, and counted (a misprediction-rate the gate can assert stays sane).

**This primitive is the entire correctness story.** It is small, lives under the existing lock, reuses the proven park primitive, and preserves the `Arc`-refcount safety property (a referenced tensor is never claimed, so a live operand is never moved). It does **not** require interior mutability over device storage (see Fork-1).

---

## 5. The load-bearing forks (the PM calls to ratify)

### Fork-1 — How does the daemon get a movable tensor? **(A) refcount-preserving** vs (B) interior-mutability reservation lock
- **(A) Refcount-preserving (RECOMMENDED).** The daemon only ever moves tensors it can claim *because nothing else references them* (cold demote of unreferenced tensors; prefetch into fresh storage). `Arc::get_mut` stays the mutation gate; the reservation state machine handles the *arrival* of a would-be referencer (park-and-retry). **Zero new `unsafe`, the P2/P3 safety property intact.**
- **(B) Interior-mutability lock over storage.** Put `TensorStorage` behind a lock so `migrate` can run behind a shared `&` while accessors block on the lock. This *would* allow moving a tensor that has live references — but it introduces a lock around the hottest data in the subsystem and the scariest `unsafe` (mutable device storage behind a shared ref), and it buys a capability (move-the-in-use) we explicitly do not want.
- **Call: (A).** It is strictly safer and loses only the ability to migrate an in-use tensor — which §2 shows we never want anyway.

### Fork-2 — What runs the daemon? dedicated kernel thread (RECOMMENDED) vs opportunistic deferred-work
- **Dedicated kernel thread (kswapd-style):** a real background mover woken at the low watermark, sleeping at the high watermark. This is the honest "concurrent" build and matches every prior-art daemon.
- **Opportunistic (run background work when a pass yields):** simpler, but it is not actually concurrent — it is P3 with extra trigger points, and it cannot prefetch *during* a long op.
- **Call: dedicated thread**, *but* gated behind the reservation primitive proven first (see the slice).

### Fork-3 — How much concurrency does P4 prove? **kernel-side coherence on accel_sim/QEMU (RECOMMENDED)** vs device-side concurrent DMA on silicon
- **Kernel-side (RECOMMENDED):** prove the reservation primitive + daemon are race-free and result-invariant under NEO_SMP on accel_sim (where "device memory" is kernel RAM; the move is a guarded `memcpy`). **This is the hard part** — the data-structure coherence, the park/wake, the accounting.
- **Device-side concurrent DMA** (Intel copy-engine overlap with compute) is a separate **hardware** build with blind-bring-up risk; name and defer it.
- **Call: kernel-side only for P4**; device-side overlap is a labelled later build.

### Fork-4 — Should P4 be one build or two? **Two (RECOMMENDED): P4a primitive, P4b daemon**
The same "don't bundle two unproven things in one confident dispatch" rule I applied to P3 (and the CLAUDE.md blind-hardware lesson) applies harder here, because P4 has *two* novel pieces and a non-deterministic gate:
- **P4a — the migration-reservation primitive (mechanism).** Build the state machine + claimed-bit + park/wake; prove it with a **synchronous two-thread concurrency test** (one thread migrates a tensor, another races to reference it) under NEO_SMP, with the §6 discriminators. **No daemon yet.** Falsifiable in isolation: a failure is a primitive bug, not a policy bug.
- **P4b — the async residency daemon (policy).** The kswapd-style thread that *uses* P4a to demote/prefetch in the background under the watermark governor; the `daemon` trace; the direct-reclaim backstop. Gated on P4a green.
- **Call: split.** P4a is the mechanism with the migration-race discriminator; P4b is the policy consumer.

---

## 6. Concurrency failure-mode + discriminator section (REQUIRED — the SMP analogue of the blind-hardware rule)
A non-deterministic race gate that only ever passes is as dangerous as a blind hardware run that designs only for success (CLAUDE.md). A single green NEO_SMP run is **not** conclusive; the gate must be a **stress loop** (N≥50 iterations, NEO_SMP=4, the `sys_wait` precedent) **and** every plausible race must be **self-diagnosing on failure**. The likely ways P4 fails *even if the primitive is "correct"*, each with its discriminator:

| # | Failure mode | Discriminator (instrument *before* the run) |
|---|---|---|
| 1 | **Use-after-migrate / TOCTOU** — an accessor reads storage the daemon already swapped | A **storage generation counter** per tensor; accessors assert the generation didn't advance across their read; a retry counter that is *expected* > 0 (proves the block-retry path actually fires) but every retry ends on a consistent generation. |
| 2 | **Lost wakeup** — daemon commits, parked accessor never wakes | Park strictly under the lock via `block_current_with_token_and_guard` (proven); a **quiesce assertion**: at pass end, parked-accessor-count == 0 and a watchdog tick bounds any park. |
| 3 | **Lock-order inversion / deadlock** — daemon takes `RECENCY→TABLE` while a pass takes `TABLE→RECENCY` | Pin and **document one global order** (extend the `admission.rs` `QUEUES → TABLE → scheduler` order to place the reservation lock); add a **debug-build acquire-order assertion** in the `SwitchMutex` wrapper (a lightweight lockdep) so an inversion *panics at the offending acquire*, not as a mystery hang. |
| 4 | **Double-claim** — two movers migrate the same victim | The CAS claim returns the loser a "skip"; assert **per-tensor migrate-owner uniqueness** (a tensor in `Migrating` has exactly one owner id; a second claim attempt is counted and must be 0 successful). |
| 5 | **Torn accounting** — §9.5 totals drift under concurrent migrate | Swap + `TABLE`/`RECENCY` update in **one critical section**; assert **per-migration delta** (`resident_after == resident_before − moved_bytes` on the source, `+moved` on the target) *and* the existing **balance-to-zero-on-teardown** under NEO_SMP=4. |
| 6 | **Misprediction storm** — prefetch stages tensors the op never uses, daemon thrashes | Count prefetch hits/misses; assert **miss-rate bounded** and that a misprediction's rollback is byte-preserving and self-limiting (the watermark hysteresis caps it). |
| 7 | **Result divergence** — the output changes under concurrency | The P1–P3 byte-oracle, now asserted **over the N-iteration stress loop**: every iteration's forward-pass output is byte-identical to the synchronous P3 reference. One divergent iteration fails the gate. |

**Capture-before-you-need-it:** the generation counter (#1), the parked-count/quiesce hook (#2), the acquire-order assertion (#3), and the per-migration delta log (#5) must exist *in the build*, not be added after a failing run. A race the gate can't localize from captured evidence is a defect in the dispatch.

---

## 7. Scope guard (what P4 is / is NOT)
- **IS:** a migration-reservation primitive (P4a) + an async background residency daemon (P4b) that demotes cold tensors and prefetches predicted-hot operands **concurrently with dispatch**, proven race-free and result-invariant on accel_sim under a NEO_SMP stress loop, **strictly additive over P3** (synchronous P2/P3 reclaim remains the always-correct backstop).
- **IS:** the build that **relaxes and re-proves** the `evict.rs` single-mutator invariant — and **re-documents** it (the note must now describe the reservation that makes concurrent movement safe).
- **NOT:** device-side concurrent DMA (Intel copy-engine overlap with compute) — named, deferred to a later hardware build (Fork-3).
- **NOT:** moving an in-use (referenced) tensor — impossible and undesirable by construction (§2, Fork-1A).
- **NOT:** a new lock-free primitive (RCU/seqlock) — keep `SwitchMutex`; revisit only on a measured contention finding, as its own dispatch (§3d).
- **NOT:** interior mutability over tensor storage (Fork-1B rejected).
- **NOT:** a change to *what* any op computes — result-invariant, same byte-oracle discipline as P1–P3, now under the stress loop.

---

## 8. Design questions for the PL
- **Q1 — Adopt the refcount-preserving reservation (Fork-1A): the daemon moves only unreferenced/cold tensors and prefetches into fresh storage, `Arc::get_mut` stays the mutation gate, zero new `unsafe`?** *(Recommended — §2, §5. Strictly safer; loses only move-the-in-use, which we never want.)*
- **Q2 — Split P4 into P4a (the migration-reservation primitive, with a synchronous two-thread concurrency test + the §6 discriminators) and P4b (the async daemon, gated on P4a green)?** *(Recommended — §5 Fork-4. Mechanism before policy; each independently falsifiable.)*
- **Q3 — Scope P4 to kernel-side concurrency coherence on accel_sim under a NEO_SMP stress loop, and name device-side concurrent DMA (Intel copy engine) as a separate later hardware build?** *(Recommended — §3c, §5 Fork-3. Proves the hard part without new blind-hardware risk.)*
- **Q4 — Keep `SwitchMutex` + the reservation state machine (no RCU/seqlock now); treat any lock-free change as a future dispatch gated on a measured contention finding?** *(Recommended — §3d. Resist premature lock-free in a young kernel.)*
- **Q5 — Make the daemon strictly additive with a synchronous direct-reclaim backstop (P2/P3 still runs when the daemon hasn't banked room), and govern it with the P3 headroom as a low/high watermark hysteresis?** *(Recommended — §3b. The single biggest risk-reducer: P4 can never be *worse* than P3.)*

---

## 9. If ratified — the slice (preview of the dispatches)
- **BUILD-P4a — migration-reservation primitive (mechanism).** Per-`(device,tensor)` reservation state machine (`Resident → Claimed → Migrating → committed`) under the existing `SwitchMutex`; claim = the P2/P3 evictability gate (`strong==1 && weak==0`) + CAS, contended claims skipped; accessor park/wake via `block_current_with_token_and_guard`; swap + §9.5 update in one critical section. Instrument the §6 discriminators (generation counter, parked-count quiesce hook, debug acquire-order assertion, per-migration delta, migrate-owner uniqueness). **Gate (QEMU, NEO_SMP stress loop, no silicon):** a two-thread test — one thread migrates a tensor while another races to reference it — run N≥50× at NEO_SMP=4; assert no use-after-migrate (gen counter), no lost wakeup (quiesce), no double-claim, accounting delta exact + balance-to-zero, no deadlock (order assertion), byte-identical reads. Extend the `evict.rs` invariant note + the `admission.rs` lock-order doc. Open **ANSPEC-001 §9.12** (the concurrent-residency model: the reservation primitive, claim/park/commit, the relaxed-and-re-proven single-mutator invariant) + an ADR.
- **BUILD-P4b — async residency daemon (policy consumer).** A dedicated kernel thread woken at the headroom low watermark, sleeping at the high watermark (kswapd/DAMON model), using P4a to **bg-demote** cold tensors and **prefetch** predicted-hot operands (the P3 structural predictor, now executed ahead-of-need); the `residency: … daemon …` trace; the **direct-reclaim backstop** (synchronous P2/P3 still runs if the daemon is behind — strictly additive). **Gate (QEMU, NEO_SMP stress loop):** a multi-tenant forward pass run concurrently with the daemon, N≥50× at NEO_SMP=4; assert byte-identical output every iteration (§6 #7), residency ≤ capacity throughout, eviction round-trips on the critical path reduced vs P3 (the daemon banked room ahead of need), misprediction rollback bounded, all P1–P3/accounting/admission/sharing regressions green. Open the §9.12 daemon subsection + an ADR.

With P4, the kernel stops *pausing to manage memory* and manages it *while it works* — the residency trace reads like a memory manager with a background reclaim/prefetch daemon, the same architecture a mature OS runs. And it gets there the safe way: a small reservation primitive proven against an explicit race-failure matrix first, a daemon that can always fall back to the proven synchronous path second, and the one invariant the whole policy layer rested on relaxed only *after* it has been re-proven under a NEO_SMP stress loop.
