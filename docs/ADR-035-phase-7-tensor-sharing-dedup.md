<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-035 — Phase 7 BUILD-3: capability-mediated tensor sharing + content-hash dedup (global-view ownership)

**Status:** Accepted (Stage-B QEMU PASS).
**Date:** 2026-06-08
**Builds on:** BUILD-1 (`906a587`, accounting, ADR-033, §9.5); BUILD-2/2L (`ed9bcbb`/`1042219`, enforced admission scheduler, ADR-034, §9.6); the THINK (Q5: reuse the existing capability machinery uniformly).
**Related:** ANSPEC-001 §9.7 (the tensor-sharing/dedup/ownership model, opened by this build); §9.5 (amended — the no-double-count rule).

## Context

§9.5 made the kernel **measure**; §9.6 made it **arbitrate** (admit one tenant, defer another). BUILD-3 adds the third governance capability: **global-view ownership** — the kernel sees, and enforces, that multiple principals reference **one physical copy** of an immutable tensor. This is a fact only the kernel can know: a per-process userspace runtime sees only its own handles and would re-copy. It is the clearest "kernel does what a library structurally can't." Lean (Article 15.7): share and dedup *references*, enforce *rights* — do not build a new allocator, a new authority primitive, or hardware isolation (that is BUILD-4).

## Decision

Capability-mediated read-only sharing of CPU-resident immutable tensors, plus content-hash dedup on model load, reusing the Phase-2 capability machinery unchanged.

1. **The refcount is the existing `Arc<Tensor>` (ISC-1).** A tensor is already an `Arc`-refcounted capability object (Phase 5). "N handles → 1 physical copy" is a cross-process *reference*, not a new allocator: a granted/deduped capability clones the `Arc` over one physical allocation. Last handle dropped → physical free (via `Tensor`'s `Drop`). Each `Tensor` carries a monotonic `tensor_id` — the stable identity the sharing registry and the cross-process reader scan key on.

2. **Explicit RO sharing reuses derive/grant/revoke (ISC-2).** A principal derives a **read-only** tensor capability (`readonly_tensor_rights()` — full meta-rights, semantic `USE` without `WRITE`) and grants it over the existing capability IPC; the grantee gets an RO handle to the *same physical storage*. No new authority primitive. The cross-process grant-completion path notifies the sharing layer for `Handle::Tensor`, registering the share and emitting the ownership trace.

3. **Rights enforced at the op boundary — not by convention (ISC-2, load-bearing).** A new semantic right `tensor_rights::WRITE` distinguishes RO from RW. The mutating op `sys_tensor_write` (scalar ABI: handle, offset, ptr, len — **no struct, no byte-oracle**) denies (`EPERM`) any caller whose capability lacks `WRITE`, *independent of* the tensor object's own `mutable` flag. So an RO grantee to an otherwise-mutable tensor still cannot write it — the right, not the object, gates the mutation. The enforcement is factored (`tensor_write_checked(pid, …)`) so the syscall (current pid) and the gate (an explicit reader pid) exercise the same check. Revocation through the derivation graph removes a grantee's access (and its place in the reader set); revoke and last-reference free interact cleanly (the `Arc` keeps surviving references alive — no use-after-free; the last drop frees and deregisters — no leak).

4. **Content-hash dedup on load (ISC-3).** On model load, each immutable weight's BLAKE3 content hash (the same primitive NeoFS content-addressing uses) is probed against a `hash → Weak<Tensor>` table: a hit whose shape/dtype match shares the existing physical copy (refcount++, no allocation, no copy); a miss allocates fresh and registers the candidate. Dedup is RO-by-construction (weights are frozen).

5. **The reader set is derived, not stored.** The set of principals referencing a physical tensor is computed on demand by scanning capability tables — for a direct `Handle::Tensor` or a `Handle::Model` whose weights hold the id. Because the scan reads live tables, a revoked or dropped capability is reflected immediately with **no** hook into the generic revoke/drop machinery. The registry stores only the accounted facts (owner, bytes) and the dedup hash.

6. **Accounting counts the physical copy once — the §9.5 amendment (ISC-4).** A shared/deduped tensor's bytes are attributed in §9.5 **exactly once, to the owning/first principal**, when it enters the registry; every other referencing principal is a reader contributing **zero** resident bytes. Total `resident_bytes` across principals equals the physical bytes resident, regardless of reader count — sharing never inflates residency. Released once, when the last reference drops. (Shared CPU-resident residency is attributed on the CPU device; the pure attribution rule lives in `sharing_logic.rs`, host-unit-tested.)

## Seams (named, not built)

- **RW / coherent shared tensors** — a separate hard coherence problem; never in this build. RO only.
- **Concurrent device-side RO-vs-RW enforcement at the hardware boundary** — for device-resident shared tensors the handle is shared and access is kernel-mediated (accelerator memory is never user-mapped), but enforcing concurrent isolation *at the device* is the **per-context PPGTT lift (BUILD-4)**. §9.7 is CPU-resident first; this is the noted seam.
- **Usage/ownership query syscall** (a tenant reading the shared-reader set) — deferred; **its struct ABI triggers the byte-oracle rule when it lands**.

## Gate / Stage-B result

QEMU, no hardware dependency (CPU-resident, pure kernel logic). Stage A: builds on+off; host policy unit tests (the no-double-count attribution rule) + sharing/dedup/refcount/revoke + BUILD-1/2/2L + Phase-6 regressions green; `shard-coverage`; fmt; clippy. **Stage-B (QEMU) PASS** — the `accel_tensor_sharing` scenario, two paths:

- **Explicit grant:** owner creates a *mutable* tensor, derives RO caps, grants to two readers → **one physical allocation** (alloc count), **3 RO readers** (scan), a **RW by an RO grantee denied** (`EPERM`) while the owner's WRITE on the same tensor succeeds, **revocation removes a reader** (3 → 2), **last-ref-drop frees** the one copy, and §9.5 residency **counted once** (owner = bytes, readers = 0; released to 0 on free).
- **Dedup on load:** two principals load identical `.neom` weights → the second load **allocates nothing** (deduped), **2 readers** of the shared weight, residency **not double-counted** (first loader = bytes, second = 0).

The ownership trace `tensor <id> shared RO {PID..} — 1 physical copy` fires on both paths.

## Consequences

Three of the four governance lines are now real: the kernel **measures** (§9.5), **arbitrates** (§9.6), and **owns the global view** (§9.7) — admitting that N principals share one physical copy, enforcing read-only rights at the op boundary, and never double-counting the shared bytes. BUILD-4 (per-context PPGTT spatial isolation — the hard Intel driver lift) is the last line: it makes the device-boundary enforcement non-vacuous and upgrades enforcement from temporal to concurrent.
