<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Phase 7 BUILD-3 — capability-mediated tensor sharing + dedup (global-view ownership) → Jarvis

**Date:** 2026-06-07
**From:** PM/Cerebro
**To:** Jarvis (new branch off `origin/main` `c82b77b`: `codex/phase-7-build-3-tensor-sharing-dedup-jarvis`)
**Status:** DISPATCHED.
**Builds on:** BUILD-1 (`906a587`, accounting), BUILD-2/2L (`ed9bcbb`/`1042219`, enforced scheduler) + the THINK (Q5: use the existing capability machinery uniformly). ANSPEC-001 §9.5/§9.6.

## 0. The point of this build
The second governance capability: **the kernel sees, and enforces, that multiple principals reference one physical tensor.** Two paths to the same fact —
- **Explicit:** principal A *grants* a read-only tensor capability to principal B (Phase-2 grant/derive/revoke), one physical copy, kernel-enforced rights.
- **Implicit (dedup):** two principals load the *same* (content-identical, immutable) model weights → the kernel keeps **one** physical copy, refcounted.

Both produce the trace `tensor X shared RO {PID7, PID12} — 1 physical copy`. This is **global-view ownership**: a fact only the kernel can know, because a per-process userspace runtime sees only its own handles and would re-copy. It's the clearest "kernel does what a library structurally can't."

## 1. Design calls (made — THINK Q5)
- **Read-only sharing only.** Shared/deduped tensors are immutable to sharers (model weights are the canonical case — already immutable snapshots). **RW-shared (mutable coherent sharing) is out of scope** — it's a separate hard coherence problem, deferred.
- **CPU-resident first.** Sharing/dedup of CPU-resident tensors (where the kernel fully controls access). Device-resident shared tensors: the *handle* is shared and access is mediated by the kernel (accelerator memory is never userspace-mapped anyway), but **concurrent device-side RO-vs-RW enforcement at the hardware boundary is BUILD-4** (PPGTT). Note the seam; don't build it.
- **Reuse the capability machinery — no new authority primitive.** A shared tensor is a capability **derived** with reduced rights (RO), **granted** via capability-mediated IPC, **revoked** through the derivation graph (Phase 2). Tensors are already cap objects (Phase 5).

## 2. ISCs
- **ISC-1 — Refcounted shared tensor storage.** The underlying tensor storage becomes shareable by reference: N capability handles → 1 physical allocation, refcounted. Last handle dropped → physical free. RO sharers cannot mutate. (The tensor primitive is already refcounted — extend it to cross-process reference, not a new allocator.)

- **ISC-2 — Explicit capability-mediated RO sharing.** A principal holding a tensor cap derives an **RO** capability and grants it to another principal over the existing capability IPC. The grantee gets an RO handle to the *same physical storage* (not a copy). Rights are **enforced**: a RO grantee that issues a *mutating* op on the shared tensor is **denied** at the syscall/op boundary (not just by convention). Revocation through the derivation graph removes the grantee's access; revoke + last-ref interact cleanly (no use-after-free, no leak).

- **ISC-3 — Content-hash dedup on load.** When a principal loads a model/tensor whose **content hash** matches an already-resident immutable tensor (lean on NeoFS content-addressing + the `.neom` snapshot), the kernel **shares the existing physical copy** (refcount++) instead of allocating a second. Dedup is RO-by-construction (immutable weights). A dedup hit emits the trace.

- **ISC-4 — Accounting must not double-count (interacts with §9.5).** A shared/deduped tensor's bytes are **one physical copy** — `resident_bytes` must reflect the physical reality (counted once), while the *readers* are tracked. Decide and document the attribution rule: the physical copy is accounted once (e.g. to the owning/first principal or a shared bucket), and sharers are recorded as readers, **not** as additional resident bytes. Whatever the rule, total `resident_bytes` across principals must equal physical bytes resident (no inflation from sharing). This is the §9.5 amendment this build makes.

- **ISC-5 — The gate scenario (QEMU).** ≥2 principals share one tensor (one path explicit-grant, one path dedup-on-load); assert: **one physical copy** (allocation count, not handle count), **N RO readers** in the trace, a **RW attempt by an RO grantee is denied**, **revocation** removes a reader, **last-ref-drop frees** the physical copy, and `resident_bytes` is **not double-counted**. The trace line `tensor … shared RO {…} — 1 physical copy` fires.

- **ISC-6 — Standards + spec.** SPDX; no new `unsafe`; `cargo fmt`; host suite (sharing/dedup/refcount/revoke unit tests + the gate scenario + BUILD-1/2/2L + Phase-6 regressions) green; `shard-coverage`; `clippy -D warnings` (CI/Linux); builds on+off. Open **ANSPEC-001 §9.7** (the tensor-sharing/dedup/ownership model — RO-only, the dedup-by-content rule, the rights-enforcement-at-op-boundary contract, the no-double-count accounting rule, the device-resident BUILD-4 seam) + a short ADR. **Byte-oracle:** if a new "share tensor" syscall with a struct ABI is introduced, byte-oracle it; prefer reusing the existing capability-grant IPC (no new ABI).

## 3. Gate (two-stage — QEMU, no OptiPlex)
Pure kernel logic, CPU-resident — no hardware dependency (no blind-hardware run → no failure-mode/discriminator section).
- **Stage A:** builds (on+off); sharing/dedup/refcount/revoke unit tests + regressions green; clippy; fmt; shard-coverage.
- **Stage B (QEMU):** the ≥2-principal sharing+dedup scenario — one physical copy, N RO readers, RW-by-RO denied, revoke works, last-drop frees, no accounting double-count, the ownership trace fires. No silicon.

## 4. Scope guard
RO sharing + dedup only. **Do NOT** build RW/coherent shared tensors; **do NOT** build concurrent device-side hardware isolation (BUILD-4 PPGTT — note the seam for device-resident shared tensors); **do NOT** touch the scheduler/admission logic (BUILD-2/2L) or introduce a new authority primitive (reuse Phase-2 caps); **do NOT** double-count shared residency. Share and dedup *references*; enforce *rights*; don't isolate *hardware* (that's next).

## 5. Merge (PM, on Stage-B PASS)
Close-gate: `git fetch origin && git rebase origin/main`, re-run Stage-A, push. PM `pm-merge-check` → `--ff-only` → `main`. After BUILD-3, three of the four governance lines are real (accounting, arbitration-enforced, ownership). BUILD-4 (per-context PPGTT spatial isolation — the hard Intel driver lift) is the last: it makes the device-boundary enforcement line non-vacuous and upgrades enforcement from temporal to concurrent.
