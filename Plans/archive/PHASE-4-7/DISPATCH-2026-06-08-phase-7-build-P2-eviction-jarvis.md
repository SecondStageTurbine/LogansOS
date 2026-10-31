<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-P2 — eviction under device-memory pressure (the kernel makes room) → Jarvis

**Date:** 2026-06-08
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `cdb9dd3`: `codex/phase-7-build-P2-eviction-jarvis`)
**Status:** DISPATCHED. **QEMU — pure policy over proven mechanisms; no silicon, low risk (P1-shaped).**
**Builds on:** BUILD-P1 (`dbe741d`, residency-aware placement, ANSPEC §9.9, ADR-036) + §9.5 accounting + the AccelDriver registry. P2 is the named eviction seam in §9.9/ADR-036.

## 0. The decision this build makes
P1 decides *where an op runs*. P2 handles the consequence: when placement wants a tensor resident on an accelerator that's **out of room**, the kernel **decides which resident tensor to evict** to make space — the tensor-residency analogue of OS page eviction. The policy is **LRU**: evict the least-recently-used *unpinned* resident tensor (migrate it back to CPU), repeat until there's room, then place the new one. If nothing evictable yields room, **fall back to CPU placement** for the op (never thrash or fail). The `eviction:` decision trace is the new governance line — the kernel reasoning about a *scarce* resource.

## 1. The policy (minimal, first eviction)
On a placement that needs `N` bytes resident on device `D` (capacity `C`, current resident `R` from §9.5):
1. If `R + N ≤ C` → no eviction (P1 path unchanged).
2. Else select victims: **LRU among `D`'s resident, *unpinned* tensors** (lowest last-use tick), evict (migrate to CPU) one at a time until `R + N ≤ C`. Reason: `lru`.
3. **Pin protection (REQUIRED):** the operands and output of the *current* op are **never evictable** — you cannot evict what you're about to use. Evicting a needed tensor = corruption/fault.
4. If even after evicting all unpinned tensors there's still no room (`N` alone > evictable capacity) → **place the op on `accel_cpu`** (graceful fallback), reason: `no-room-cpu-fallback`. Do not loop.
**Keep it minimal:** LRU + pin protection + CPU fallback. No working-set heuristics, no cost model (that's P3 proactive migration), no eviction-to-another-device (evict to CPU only).

## 2. ISCs
- **ISC-1 — Recency tracking (the one new input).** §9.5 tracks resident bytes + dispatch counts; LRU needs **last-use recency** per resident tensor. Add a monotonic last-use tick per `(device, tensor)`, updated whenever an op touches that tensor. Minimal, pure-ish; host-testable.
- **ISC-2 — The eviction policy (pure, host-testable).** `select_victims(device, capacity, resident_set_with_recency, pin_set, new_bytes) → (Vec<victim>, reason)` — LRU among unpinned, or the CPU-fallback signal. Unit-tested across: no-eviction, single-LRU-victim, multi-victim, pinned-protected, no-room-fallback.
- **ISC-3 — Wire eviction into the placement/residency path.** When P1's placement chooses `D` and residency on `D` would exceed capacity, consult the eviction policy, **migrate each victim off `D` to CPU** (reuse `tensor_migrate`/the existing residency mechanism), then proceed with the placement. Capacity = the device's real bound (e.g. `accel_sim`'s 256 KiB arena is the natural testbed; or a configured cap).
- **ISC-4 — Result-invariance + no-evict-needed (REQUIRED — same discipline as P1).** Eviction changes *where a tensor lives*, never its value: an evicted tensor migrated to CPU has identical bytes; if needed again it re-migrates (or is recomputed) correctly. **The forward-pass output must stay byte-identical regardless of eviction**, and **no pinned/needed tensor is ever evicted**. A wrong result or an evicted-needed tensor is a *bug*, not a perf tradeoff.
- **ISC-5 — The decision trace + the gate scenario (QEMU).** Trace: `eviction: tensor=<id> device=Accel(n) → cpu reason=<lru|no-room-cpu-fallback> (resident=<R>/<C>, need=<N>)`. Gate: a memory-pressure forward pass on a bounded device (more live tensors than fit) where placement forces eviction — assert (a) the `eviction:` trace fires with LRU victim(s), (b) **residency stays within capacity** (`R ≤ C` always), (c) **no pinned/needed tensor evicted**, (d) **output byte-identical** to the unbounded/CPU reference. Drive the pressure deterministically (sized tensors + known capacity), not by race.
- **ISC-6 — Standards.** Always-on substrate but a *strict superset* (no eviction when there's room → P1 behavior exactly); SPDX; no new `unsafe`; `cargo fmt`; host suite (eviction unit tests + recency + the pressure scenario + P1/`infer_*`/accounting/admission/sharing regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. Open **ANSPEC-001 §9.10** (the residency-pressure eviction model — LRU, pin protection, CPU fallback, result-invariance, the trace) + a short ADR.

## 3. Gate (two-stage — QEMU, no OptiPlex)
Pure policy logic; no hardware dependency (no blind-hardware run → no failure-mode/discriminator section).
- **Stage A (you):** builds (on+off); eviction + recency unit tests + the pressure scenario + regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the pressure forward pass shows the `eviction:` LRU decisions, residency bounded by capacity, no needed tensor evicted, and byte-identical output. No silicon.

## 4. Scope guard
Reactive eviction-to-CPU under memory pressure only. **Do NOT** add proactive/cost-aware migration (BUILD-P3 — eviction *reacts* to pressure; P3 *anticipates*), eviction-to-another-accelerator, working-set/frequency heuristics, or new ops; **do NOT** change *what* any op computes (ISC-4); **do NOT** evict pinned/needed tensors; **do NOT** duplicate §9.5 (extend it with recency, read it for residency). Make room by evicting the coldest unpinned tensor; nothing more.

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. Then **BUILD-P3** (cost-aware proactive migration — the `residency:` retain/migrate decision: keep a tensor resident because it'll be needed, or migrate ahead of need when the cost pays off). With P2, the kernel manages a *scarce* device resource under pressure — the eviction decision that, with placement (P1), completes the core of "an OS managing intelligence as a resource."
