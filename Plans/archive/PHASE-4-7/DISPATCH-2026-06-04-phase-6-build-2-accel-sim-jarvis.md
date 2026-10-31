<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 6 BUILD-2 — `accel_sim` (logical multi-device + async de-risk) → Jarvis

**Date:** 2026-06-04
**From:** PM/Cerebro
**To:** Jarvis (worker tree, feature branch — e.g. `codex/phase-6-build-2-accel-sim-jarvis`)
**Status:** DISPATCHED — ready for BUILD
**Design contract:** ADR-030 (Phase 6 substrate — applies the *existing* `AccelDriver` trait; **no new ADR needed**), `Plans/THINK-2026-06-04-phase-6-build-2-virtio-accel.md` (the re-slice rationale + PL decision), ANSPEC-001 §9.2 (amended: BUILD-2 = `accel_sim`).
**Builds on:** BUILD-1 (`9d8c6cf`). Current `main` tip: see origin/main.

---

## 1. Objective

Add `accel_sim`: a **second, non-CPU** `AccelDriver` on `Device::Accel(1)` with its **own device-memory arena** and **simulated asynchronous completion**, registered alongside `accel_cpu` (`Accel(0)`). Pure kernel + test — no hardware, no host daemon, no virtio. Its purpose is to exercise the parts of the `accel` abstraction that `accel_cpu` *stubbed*, catching latent bugs before the heavier virtio transport build (BUILD-3).

`accel_cpu` is the **default** device, its "device memory" **is** host RAM (identity copies), and its `dispatch` is **synchronous/always-Complete** (`wait()` never sees `Pending`). `accel_sim` must be the opposite on all three counts, so the following become *genuinely exercised for the first time*:
- a **non-default, non-CPU** device driving a forward pass (does `ai::exec` actually work off a non-`accel_cpu` driver?),
- **real, non-identity** `copy_to/from_device` between distinct memory (the arena vs host buffers),
- **real async**: `poll` returns `Pending` then `Complete`, and `wait` must **drive the op to completion** — resolving the BUILD-1 deferred note where `wait()` returned `Ok` on `Pending`.

## 2. Decisions locked

- **Apply the existing trait — no redesign, no new ADR.** `accel_sim` implements the same `AccelDriver`. If async forces a trait-surface change (e.g. the BUILD-1 `&mut OpBatch` doesn't survive deferred completion, or `wait()`'s contract needs tightening), make the minimal change **and update ANSPEC-001 §9.1 + the BUILD-1 follow-up notes** — don't silently drift.
- **`wait()` contract is now real:** `wait(token, None)` blocks until the op is `Complete` (looping `poll`), and completion is what makes the output bytes valid. `ai::exec` must rely on `wait()` actually completing — if it ever assumed synchronous completion, fix it here.
- **Simulated async, kept lean.** No threads, no real interrupt machinery. The simplest deterministic model: `dispatch` records the pending op and returns a `Pending` token **without computing**; the work is performed on the first `poll`/`wait` (lazy completion), so a `poll` *before* `wait` deterministically observes `Pending` and `wait` observes `Complete` with correct output. Do **not** build speculative threading/IRQ scaffolding — that's BUILD-3's virtio transport.
- **`accel_sim` is a permanent reference device, not a throwaway** (Article 1.3 — no toy code). Ship it as a registered driver; it is acceptable (encouraged) to gate it behind a Cargo feature (e.g. `accel-sim`, default-on for dev/CI) so minimal/embedded builds can compile it out.

## 3. Implementation Success Criteria

- **ISC-1 — Driver + registration.** `kernel/src/drivers/accel/accel_sim.rs`, registered at `accel::init()` on `Device::Accel(1)` alongside `accel_cpu(0)`; `device_count() >= 2`; descriptor reports a distinct memory kind (add an `AccelMemoryKind` variant, e.g. `DeviceArena`) and `AccelDispatchKind::Asynchronous` (add the variant). SPDX + docs.
- **ISC-2 — Own device-memory arena.** A distinct allocation pool (NOT the host tensor path); add a non-`Host` `DeviceStorageBacking` variant (the enum already anticipates this). `copy_to/from_device` perform **real byte copies** between host buffers and the arena — verified non-identity (mutating the source after copy does not change the arena copy, and vice-versa).
- **ISC-3 — Async completion.** `dispatch` returns a `Pending` token; `poll` returns `Pending` before completion and `Complete` after; `wait` drives to completion. A test observes `poll == Pending` immediately post-`dispatch`, then `wait` → `Complete` with correct output. `preempt` may stay `Unsupported`.
- **ISC-4 — `ai::exec` runs on a selected non-default driver.** Add a minimal internal hook (e.g. `run_with_driver(model, inputs, &dyn AccelDriver)`; the public `run` keeps using `default_driver()`). A scenario runs a forward pass (reuse the MLP fixture) **on `accel_sim`** and asserts the output is **byte-identical** to the `accel_cpu`/CPU oracle (cross-device equivalence + proves `exec` doesn't assume `accel_cpu`).
- **ISC-5 — Real migration round-trip.** `tensor_migrate` Cpu→`Accel(1)`→Cpu is now a **real copy** (bytes verified identical after the round-trip, unlike `accel_cpu`'s identity copy). Exercise `Accel(0)`↔`Accel(1)` too (goes via Cpu per the existing migrate logic).
- **ISC-6 — Resolves the BUILD-1 async notes.** Document (in code) the now-real `poll`/`wait` contract; confirm `&mut OpBatch` still works under deferred completion or adjust minimally (+ ANSPEC §9.1 note). The BUILD-1 "`wait()` returns `Ok` on `Pending`" looseness must be gone.
- **ISC-7 — Errno + lifetime.** Arena freed on `DeviceStorage` drop, no leak (a scenario allocates/frees in a loop and checks); unsupported op/dtype on `accel_sim` → clean error, no panic.
- **ISC-8 — Scenarios.** New QEMU scenarios at `NEO_SMP=4`: `accel_sim_forward_equiv` (forward pass on `accel_sim` == oracle), `accel_sim_migrate_roundtrip` (real copy), `accel_sim_async_poll` (Pending→Complete). Host `ai-ops-tests` coverage where it fits.
- **ISC-9 — Standards/lean/gate.** No new external dependency; SAFETY on any new `unsafe` (arena alloc); no XMM regressions; rustfmt; host clippy (`--exclude neoplutarch-kernel`) clean.

## 4. Close gate

- `git fetch origin && git rebase origin/main`; re-run the gate.
- **BUILD-1 transparency unbroken:** `infer_mlp/transformer/embedding_forward` still byte-identical through `accel_cpu` (the default path is unchanged), AND the three new `accel_sim_*` scenarios green at `NEO_SMP=4`.
- **Cross-device equivalence:** `accel_sim` forward output == `accel_cpu` output == hand oracle.
- Host-checks green (fmt / `test --workspace --exclude neoplutarch-kernel` / `clippy --workspace --exclude neoplutarch-kernel -- -D warnings`).
- Single squashed commit; push to your **feature branch, not `main`** — PM merges via `pm-merge-check` + `--ff-only`. Close report with ISC pass/fail, the equivalence evidence, and any `AccelDriver`/`OpBatch` trait adjustment (with the ANSPEC §9.1 note).

## 5. Scope guard

`accel_sim` + the `exec` non-default-driver hook + tests, **only**. **No virtio, no vhost-user, no real threads/IRQ async** (that's BUILD-3). No `.neom`/op-math/oracle changes. No touching `accel_cpu`'s behavior (its scenarios must stay byte-identical). Stays clear of Prometheus's clippy lane by construction (all your files are in `drivers/accel/**`, `ai/exec.rs`, `test_harness.rs` — its exclusion set).
