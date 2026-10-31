<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Phase 7 BUILD-O1 — slot-indirected ownership (handles point, the kernel owns)

**Date:** 2026-06-10
**Seat:** PM/Cerebro (deep think + prior art; PL pre-authorized the PM recommendation — "do a deep think and go with your recommendation" — so this THINK records the decision and the dispatch follows immediately)
**Context:** BUILD-C1 closed the first governance cycle and returned four candidate lines: (1) the Handle/Model slot-indirection ownership redesign (the BLOCKERed ISC-4), (2) the Phase-6 SIMD32 backport, (3) device-side copy-engine DMA overlap, (4) the production inference-serving surface. This THINK picks **(1)** and designs it.
**Related:** ADR-041 (the ISC-4 deferral: "real wiring means the handle/model types adopt slot indirection… an ownership redesign requiring its own dispatch, THINK, and gate"); the invariant ledger I-4/I-9 (`docs/complexity-governance.md`); ANSPEC §9.12 (TensorSlot/`guarded_clone`); `kernel/src/sched/process.rs:463` (`Handle::Tensor { tensor: Arc<Tensor> }`), `kernel/src/ai/model.rs:42` (`weights: Vec<Arc<Tensor>>`), `kernel/src/ai/residency_daemon.rs` (the append-only `'static` slot registry).

---

## 1. Why this line, and not the other three (the decision)

One structural fact decides it. The I-4 claim gate — a tensor is movable iff `Arc::strong_count == 1 && weak_count == 0` — is the safety floor of the whole policy arc. But **both production ownership surfaces hold the `Arc` directly**: a process handle is `Handle::Tensor { tensor: Arc<Tensor> }` and a model is `weights: Vec<Arc<Tensor>>`. Therefore every tensor a process or model holds has `strong_count ≥ 2` *for the lifetime of the handle* — not while it's being **used**, while it merely **exists**. Under I-4 such a tensor is never claimable, never evictable, never daemon-managed.

The consequence, stated honestly: **the proven policy layer governs only executor-pass intermediates in production.** Model weights — the *largest* memory object in any AI workload, the thing §9.10 eviction and the §9.12 daemon exist for — are permanently outside their reach. The 60×-proven machinery is real, but its production coverage is the gate's managed pool. C1's ISC-4 hit exactly this wall and correctly refused to paper over it with registration calls.

Against the alternatives: **(2) SIMD32** is contained perf polish, orthogonal, can ride along any time. **(3) DMA overlap** makes tensor *movement* faster — but the tensors that matter can't currently be moved at all, so it would optimize a path production can't take. **(4) the serving surface** is where the project is headed, but shipped now it would serve tenants whose memory the kernel provably cannot manage — the embarrassing inversion of the project's thesis. **O1 is the build that converts the policy arc from proven-in-gate to real-in-production, and it unblocks (4) while giving (3) something real to move.** It is also the named, BLOCKERed debt — the discipline that returned it as "its own THINK + dispatch" owes it that THINK.

## 2. The design problem, precisely

The current model conflates two different things in one refcount:
- **"Someone may use this later"** (a handle exists, a model is loaded) — this should NOT pin residency. Existence is cheap; the kernel should be free to demote the bytes and bring them back on use.
- **"Someone is using this now"** (an op holds the operand, a syscall is mid-copy) — this MUST pin residency (I-9: never move what's in use).

`Arc<Tensor>`-in-the-handle makes both look identical (`strong_count ≥ 2`), so the kernel must conservatively treat *existence* as *use*. The fix is the oldest move in OS design: **indirection.** Handles stop holding the object and start *naming* it; the kernel becomes the unique owner; use mints a short-lived reference through a checked gate.

## 3. Prior art — three traditions, one converged shape

**(a) DRM GEM/TTM — the GPU-memory ownership model this problem is isomorphic to.** A `drm_gem_object` carries **two counters**: `kref` (kernel references) and `handle_count` (userspace handles). Userspace holds opaque `u32` handles, never pointers. TTM **evicts unpinned buffer objects even while userspace holds handles to them**; what prevents eviction is not the handle but the **pin** — set around actual use (command submission), released after, with `dma_resv` as the reservation/synchronization object. ([GEM — LWN](https://lwn.net/Articles/283798/), [DRM Memory Management — kernel.org](https://www.kernel.org/doc/html/v5.10/gpu/drm-mm.html).) This is *exactly* our split: handle-existence ≠ pinned; in-flight-use = pinned; and our P4a reservation is our `dma_resv`. Every mature GPU stack on earth made this move — we are re-deriving the industry-standard ownership model, with capability rights layered on top.

**(b) The Linux fd table — handle indirection + mint-on-use.** An fd is an integer index; the kernel's `fdtable` maps it to a `struct file` whose lifetime is its own refcount. Use mints a temporary reference (`fget`), drops it after (`fput`); the lookup-races-last-put hazard is closed by `atomic_long_inc_not_zero` — *mint fails cleanly if the object is dying*. ([files.rst — kernel.org](https://docs.kernel.org/filesystems/files.html).) Our analogue already exists and is already race-proven: **`guarded_clone` through the P4a TensorSlot** is our `fget` — it parks if a migration is in flight and re-resolves, and it can fail cleanly if the slot is drained. O1 doesn't have to invent the mint gate; P4a built and stress-proved it.

**(c) Generational indices (slotmap) — the stale-handle/ABA defense.** When slots are reused, a stale handle must not silently resolve to the *new* occupant. The standard fix: the key is `(slot, generation)`; freeing bumps the generation; a mismatched lookup fails at runtime instead of aliasing. ([generational-arena](https://github.com/fitzgen/generational-arena), [slotmap](https://docs.rs/slotmap).) The current daemon registry is append-only `'static` (fine for the gate pool, wrong for process-lifetime tensors that come and go) — production slots need alloc/free/reuse, so they need generations.

The converged shape: **handle = capability + generational slot key; the slot owns the unique `Arc`; use = `guarded_clone` (fget); the claim gate then means exactly "no use in flight."**

## 4. The design (what O1 builds)

1. **The slot arena.** A generational slot arena replaces the append-only registry: `alloc(tensor) → SlotKey {index, gen}`, `free(key)` (bumps gen, drains via the P4a take-with-retry), `mint(key) → Option<Arc<Tensor>>` (generation check, then `guarded_clone` — parks across migrations, `None` for stale/drained keys). The arena's slots are the **unique strong owners at rest**.
2. **Handles point.** `Handle::Tensor` stores `SlotKey` + cached immutable metadata (id, shape, dtype — so queries don't mint) + the existing cap rights. Handle clone/share copies the key (the GEM `handle_count` analogue is the arena's per-slot handle count, used for free-on-last-close). `Model::weights` becomes `Vec<SlotKey>` the same way.
3. **Use mints.** Every path that touches bytes (`sys_inf_run` operand resolution, `sys_tensor_query`'s data reads, sharing's hash) mints via the arena and drops the Arc when done. While minted: `strong_count ≥ 2` → unclaimable → **I-9 pin-on-use now falls out of the same mechanism for production tensors as for pass intermediates.**
4. **The claim gate is unchanged and becomes *true*.** `strong == 1 && weak == 0` at the slot now means precisely "no use in flight" — model weights idle between inferences become claimable, demotable by the daemon, re-fetched on next use (placement/migration already handle the return trip; a use arriving *during* the move parks on the P4a reservation — that exact race is the one P4a proved 60×).
5. **Sharing/dedup, first cut: unchanged semantics.** The §9.7 dedup registry keeps its `Weak<Tensor>`; deduped tensors therefore stay unclaimable (`weak > 0`) — exactly today's behavior, zero regression, and re-keying dedup to slots is named follow-on work, not smuggled in.
6. **I-4 is amended relax-and-re-prove (Rule 2), and one new row is added.** I-4 becomes "unique-strong-owner-at-rest: the slot arena holds the only resting `Arc`; all minting goes through the generation-checked `guarded_clone` gate." New row **I-13 — stale handles fail safe:** a freed/reused slot can never be reached through an old key (generation mismatch ⇒ clean error, never aliasing). Its guard ships in the same build (the generation-mismatch counter + a reuse-attack unit test). Zero new `unsafe` remains the corollary — indirection *removes* `Arc`s from surfaces; it adds none.

## 5. The split (the P4a/P4b lesson applies verbatim)

Two unproven things — the arena mechanism and the ownership conversion — do not ride one dispatch.
- **O1a — the generational slot arena (mechanism).** The arena + SlotKey + mint/free/generation protocol + handle-count lifecycle, integrated with the P4a reservation. Host-testable truth tables (mint-after-free fails, gen wraps safely, free-while-minted defers drain, claim-vs-mint under the existing stress shape) + a NEO_SMP race gate (mint racing claim racing free), with the P4a discriminators extended by the **generation-mismatch counter** (expected > 0 in the attack test, **0 successful stale derefs ever**) and a **slot-balance check** (allocated == freed at teardown — the leak guard).
- **O1b — the conversion (consumer).** `Handle::Tensor` + `Model` adopt SlotKey; syscall semantics audited (`sys_tensor_query`, handle clone, share/grant, teardown frees slots); the production gate: **a loaded model's weights, idle, are demoted by the daemon under pressure and transparently re-fetched on the next inference — byte-identical output, while a mid-inference weight is never touched.** That scenario is the entire point of the build, in one line of gate.
- **Kill-switch (Rule 1), both halves:** O1a unused ≡ no behavior change anywhere (pure addition); O1b's equivalence proof is the full prior family unchanged — with no pressure and the daemon off, every syscall-observable result is bit-identical to `Arc`-in-handle semantics (the indirection is invisible except to eviction).

## 6. Failure modes the dispatches must discriminate (capture-before-you-need-it)

| Mode | Discriminator |
|---|---|
| Stale-key aliasing (ABA) | generation-mismatch counter; the reuse-attack test demands mismatches occur and **zero** resolve |
| Use-after-free via mint-vs-free race | mint = `inc_not_zero`-style: fails cleanly on drained slot; assert no mint ever returns a draining tensor (gen counter + storage generation from P4a) |
| Slot leak (handle close never frees) | slot balance: allocated == freed at teardown, per scenario |
| Claim-vs-mint starvation (daemon can never claim a hot slot) | expected behavior — assert claim *skips* (P4a) and the op proceeds; starvation of the *daemon* is acceptable, of the *op* is not |
| Syscall semantics drift | the O1b equivalence family: every prior scenario's syscall-visible results bit-identical |
| Mid-inference weight eviction | the I-9 production assertion: minted weights unclaimable for the mint's duration — the gate runs inference *under* daemon pressure |

## 7. What this is not (scope guard for both dispatches)

Not the serving surface (O1 *unblocks* it); not dedup re-keying (named follow-on); not DMA overlap or SIMD32 (carried, orthogonal); not a new lock primitive or any new `unsafe`; not a semantic change to capabilities/rights (rights ride the handle exactly as today); not a change to *what* any op computes (I-1 stands over everything, as always).

---

**Recommendation (taken, per PL pre-authorization):** open BUILD-O1 as O1a (arena mechanism) now, O1b (conversion) gated on O1a green. The dispatch for O1a follows this THINK to `main` immediately.
