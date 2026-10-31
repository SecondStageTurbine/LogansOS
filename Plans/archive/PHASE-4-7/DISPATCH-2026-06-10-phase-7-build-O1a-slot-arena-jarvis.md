<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-O1a — the generational slot arena (the kernel owns; handles point) → Jarvis

**Date:** 2026-06-10
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `dffcd66`: `codex/phase-7-build-O1a-slot-arena-jarvis`)
**Status:** DISPATCHED. **QEMU NEO_SMP stress loop — a mechanism build with a race surface (mint vs claim vs free); no silicon, but a concurrency gate with discriminators.**
**Builds on:** P4a (`9599438`, the TensorSlot reservation + `guarded_clone`, §9.12, ADR-039) + the append-only `'static` daemon registry (`residency_daemon.rs:96`) this build generalizes + the invariant ledger I-4/I-9.
**Ratified design:** `Plans/THINK-2026-06-10-phase-7-build-O1-slot-indirected-ownership.md` (PL pre-authorized the PM recommendation). **This is O1a (the arena mechanism); O1b (Handle/Model conversion) is gated on O1a green** — same mechanism-before-consumer split as P4a/P4b.

## 0. The decision this build makes
Today the daemon's slot registry is append-only with `'static` slots (fine for the gate's fixed pool, wrong for process-lifetime tensors that come and go). O1a builds the **generational slot arena** that replaces it: the kernel becomes the **unique strong owner of a tensor at rest**, identified by a `SlotKey {index, generation}`; *use* mints a short-lived `Arc` through the proven P4a gate; *existence of a handle* (O1b) will no longer pin residency. O1a ships **only the arena + its race proof** — no Handle/Model changes yet. The point: make "the slot holds the only resting `Arc`, and `strong==1 && weak==0` means *no use in flight*" a true, race-proven mechanism, so O1b can convert the ownership surfaces onto it.

## 1. The arena (minimal, generational)
A slot arena keyed `SlotKey {index: u32, generation: u32}` under `SwitchMutex` (no new lock primitive; ordered under `sync::lock_order` — the lockdep will catch an out-of-order acquire):
- **`alloc(tensor) → SlotKey`** — install a tensor into a free (or freshly grown) slot; return its key at the slot's current generation. The slot now holds the unique resting `Arc`.
- **`free(key)`** — generation-checked; **drains via the P4a take-with-retry** (a free that races an in-flight migration waits for commit, never tears), then **bumps the slot's generation** and returns it to the free list. Idempotent against a stale key (generation mismatch ⇒ no-op).
- **`mint(key) → Option<Arc<Tensor>>`** — the `fget` analogue: generation check first (stale/reused ⇒ `None`, never aliases the new occupant), then `guarded_clone` through the P4a reservation (parks across a migration, re-resolves; `None` if the slot is draining). **Mint never returns a draining/dead tensor** (the `inc_not_zero` discipline).
- **Handle-count lifecycle (the GEM `handle_count` analogue):** each slot tracks how many handles name it; O1a provides `retain(key)`/`release(key)` and frees the slot on last release. O1a exercises this directly (O1b wires it to `Handle::Tensor` clone/drop).
- **The claim gate is unchanged and becomes *true*:** `strong == 1 && weak == 0` at a slot now means exactly "no mint in flight" — the daemon/eviction may claim an idle slot, and a `mint` arriving during the claimed migration parks on the P4a reservation (the race P4a proved 60×).

## 2. ISCs
- **ISC-1 — The arena (pure core + kernel glue).** The slot table, free list, generation counter, and the alloc/free/mint/retain/release protocol. Put the **generation arithmetic + key-match + free-list logic in a pure, host-testable file** (the `*_logic.rs` tier — kernel-free, path-`mod`'d by `tools/ai-ops-tests`); the `Arc`/`SwitchMutex`/`guarded_clone` glue in the kernel module. No new `unsafe`.
- **ISC-2 — Generalize the daemon registry onto the arena (no behavior change).** The append-only `'static` registry becomes the arena's iteration surface; the daemon walks live slots. **Kill-switch (Rule 1):** with the gate's existing fixed pool registered once and never freed, the daemon's behavior is **bit-for-bit P4b** — prove it (the full P4b stress family unchanged). The arena is a strict superset: never-freed slots ≡ the old append-only registry.
- **ISC-3 — Stale-key safety / ABA (REQUIRED — new invariant I-13).** A freed-then-reused slot must never be reachable through an old key. Generation mismatch ⇒ clean `None`/error, **never** a deref of the new occupant. Add the row **I-13 — "stale handles fail safe"** to `docs/complexity-governance.md` §1 with its guard (Rule 2: a new build adding a cross-cutting invariant names it and ships its guard in the same build). This is the one *new* invariant O1 introduces; everything else preserves.
- **ISC-4 — I-4 relax-and-re-prove (REQUIRED).** Amend I-4 in the ledger from "Arc-in-the-handle" to **"unique-strong-owner-at-rest: the slot arena holds the only resting `Arc`; all minting goes through the generation-checked `guarded_clone` gate."** The claim predicate (`strong==1 && weak==0`) is *unchanged* — what changes is that it now expresses "no use in flight" for arena-owned tensors. Re-prove it under the gate (the claim truth-table + the stress loop). Update the `evict.rs`/`reservation.rs` docs and ADR pointers.
- **ISC-5 — The race gate (REQUIRED — discriminators armed, the P4a precedent).** A multi-thread test: one thread `mint`s a slot repeatedly, one `claim`s/migrates it (the daemon role), one `free`s-and-`alloc`s (slot reuse), under **N ≥ 50 iterations at NEO_SMP=4**. Discriminators, all captured in-build:
  - **generation-mismatch counter** — expected **> 0** in the reuse-attack path (proves stale keys are actually generated) and **0 successful stale derefs ever** (a stale mint that returned a tensor is a hard failure);
  - **slot-balance** — `alloc` count == `free` count at teardown (the leak guard);
  - **no use-after-free** — a `mint` racing a `free` either gets the live tensor or `None`, never a draining one (the P4a storage-generation counter backs this);
  - the standing **P4a matrix stays armed** (parked-count quiesce, migrate-owner uniqueness, no lost wakeup) and must demonstrably fire (parks > 0).
- **ISC-6 — Standards.** Strict superset (never-freed slots ≡ P4b, ISC-2); SPDX; **no new `unsafe`**; `cargo fmt`; host suite (arena logic truth tables + the P1–P4b/`infer_*`/accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings`; builds on+off; lockdep clean. **Open ADR-042** (the slot arena: SlotKey, generations, mint/free/retain, the I-4 amendment + I-13 addition, the race proof). ANSPEC §9.12 gains a short "ownership: the slot arena" note; the full ownership-model spec section lands with O1b.

## 3. Gate (two-stage — QEMU NEO_SMP stress loop, no OptiPlex)
**Expected-failure-mode analysis (the arena's race surface — P4a matrix stays armed, these are the *new* modes):** stale-key aliasing (ISC-3 gen counter), use-after-free via mint-vs-free (the `inc_not_zero` mint + storage-gen), slot leak (the balance check), generation wraparound (assert the u32 gen wrap is benign — a wrapped slot is indistinguishable only after 2³² reuses; document and bound).
- **Stage A (you):** builds (on+off); arena truth-table unit tests + regressions; clippy; fmt; shard-coverage; the kill-switch equivalence (never-freed ≡ P4b).
- **Stage B (QEMU):** the ISC-5 race gate, N ≥ 50 at NEO_SMP=4, every assertion green every iteration; the P4b family unchanged.

## 4. Scope guard
The arena mechanism + its race proof only. **Do NOT** touch `Handle::Tensor` or `Model::weights` (that is **O1b** — gated on this green); **do NOT** re-key the §9.7 dedup registry (named follow-on — dedup keeps its `Weak`, deduped tensors stay unclaimable exactly as today); **do NOT** change syscall semantics, capabilities, or rights; **do NOT** add a new lock primitive or any `unsafe`; **do NOT** change the P4a reservation protocol (consume `guarded_clone`/take-with-retry; if insufficient, BLOCKER to PM); **do NOT** change *what* any op computes (I-1).

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A + the stress loop, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **BUILD-O1b** (the conversion: `Handle::Tensor` + `Model::weights` adopt `SlotKey`, use-sites mint via the arena, handle clone/drop drive `retain`/`release`; the production gate — *a loaded model's idle weights are demoted under pressure and transparently re-fetched on next inference, byte-identical, while a mid-inference weight is never touched*). With O1a, the kernel owns tensors at rest through a generation-safe arena — the foundation that lets O1b put model weights under the proven policy layer for the first time.

---

## Relay — what to tell Jarvis
- **Dispatch (authoritative, on `main`):** `Plans/DISPATCH-2026-06-10-phase-7-build-O1a-slot-arena-jarvis.md`
- **THINK (ratified design + prior art):** `Plans/THINK-2026-06-10-phase-7-build-O1-slot-indirected-ownership.md`
- **Branch:** `codex/phase-7-build-O1a-slot-arena-jarvis` off `origin/main` `dffcd66`.
- **One-line:** Build the generational slot arena only (no Handle/Model changes — that's O1b). `SlotKey{index,generation}`, `alloc`/`free`(drain-then-bump-gen)/`mint`(gen-check then P4a `guarded_clone` = our `fget`)/`retain`/`release`(free-on-last); pure-tier generation+free-list logic, kernel glue for the `Arc`/reservation. Generalize the daemon's append-only registry onto it with a kill-switch proving never-freed ≡ P4b bit-for-bit. Add invariant **I-13** (stale handles fail safe) + relax-and-re-prove **I-4** (unique-owner-at-rest), both in `complexity-governance.md`. Gate = mint-vs-claim-vs-free race, **N≥50 at NEO_SMP=4**, discriminators armed: gen-mismatch counter (>0 generated, **0 stale derefs**), slot-balance (alloc==free), no use-after-free, P4a matrix still firing. Zero new `unsafe`, keep `SwitchMutex`. Open ADR-042. QEMU only.
