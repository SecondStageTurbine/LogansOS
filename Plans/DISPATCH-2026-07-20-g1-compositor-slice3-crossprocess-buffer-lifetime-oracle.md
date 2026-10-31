<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: G1 compositor Slice 3 — cross-process buffer lifetime + the C3 permission proof → Oracle

**Date:** 2026-07-20
**From:** PM (Claude)
**To:** Oracle (main-tier; free lane, parallel with Jarvis's Slice 4 + Cerebro's OT-63)
**Status:** DISPATCHED — kernel-touching (`SharedPixelBuffer` lifetime across processes) ⇒ 4-shard `workflow_dispatch` QEMU gate + AAG. **SMP/teardown-touching ⇒ CP-bank consultation required.** **buffer.rs is co-owned with Cerebro's OT-63 present backend — coordinate the seam, rebase-fresh (see Coordination).** Slice 3 of the G1 ladder (`docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`).

## FIRST — inventory what already exists (do NOT rebuild it)
`kernel/src/atrium/buffer.rs` **already ships** most single-surface buffer mechanics. Read it (and RAG `SharedPixelBuffer Arc pin borrowed Pmap`) before writing a line. Already present, do **not** re-implement:
- **Arc-pinning / Drop-after-unmap** — each mapping holds a clone of the buffer `Arc` inside its `Pmap` (`map_borrowed_region` pins it); `Drop` runs only once every mapping's PTEs are gone + TLBs shot down.
- **Borrowed-leaf frames** — `Pmap::Drop` removes the PTE but never frees the frame (`BIT_9`).
- **The dual-PTE permission model** — compositor mapping = hardware **RO + NX**; client mapping = **RW + NX** (W^X holds both). This is the C3 core, already built.
- **Immutable descriptor** — a client scribble corrupts only its own pixels, never the descriptor; `create(desc: BufferDescV1) -> Arc<Self>` with a Rung-0 byte-oracle on `BufferDescV1`.

**Slice 3 is the DELTA on top of this, not a rebuild.** If your design re-creates any of the above, you've mis-scoped — stop and re-read buffer.rs.

## What Slice 3 actually builds — the cross-PROCESS generalization + the adversarial lifetime proof
Rung-0/Slice-1 proved the lifetime for **one process's** buffer. Slice 2 shipped the **attach verb** that records a `buffer_cap_provenance` token but explicitly deferred the buffer *lifetime* mechanics. Slice 3 makes the buffer's guarantees hold **across the client→compositor process boundary**, and proves them adversarially:
1. **Cross-process import (wire the attach verb to real mapping).** A client's `SharedPixelBuffer`, granted via Slice-2's attach rendezvous, is mapped **RO+NX into the compositor process** so the compositor composites directly from the client's pixels (the zero-copy dual-PTE — the Qubes C3 win, no grant-table negotiation). The client keeps its RW+NX mapping. Both point at the same borrowed frames.
2. **Lifetime holds across processes (the load-bearing proof).** The buffer must outlive **every** mapping in **every** process. Prove: client `destroy`/drops-its-cap / peer-death (Slice-2 wiring) while the compositor still maps ⇒ the frames stay live until the compositor's mapping is also torn down (the Arc pin already gives you this *within* a process — Slice 3 proves it *across* processes). No use-after-free where the compositor reads freed/reused frames.
3. **ABA-safe reuse across processes.** A buffer id/slot reused after teardown must not let a stale client handle alias a later buffer's frames (the ISC-39 / monotonic-id lineage — mirror the `completed_epoch` and `InputFocus` non-reuse discipline).

## The Qubes-C3 permission proof (the relocated attack surface — the point of the slice)
The Qubes pass (`docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` §5.2/C3) established: we don't have Xen's cross-VM grant-table surface (QSB-046/231) — but that surface **relocates** into our kernel's dual-PTE setup. Slice 3 must **prove** the relocated surface is airtight:
- **Compositor genuinely cannot write** — the compositor-role PTE is RO+NX and a compositor write **faults** (a falsifiable scenario: compositor attempts a write to the imported buffer → page fault, not a silent corruption). Assert it, don't claim it.
- **No third principal can map** — only the owning client (RW) + the attach-granted compositor (RO) hold mappings; a process that never received the grant has **no** path to map the frames. Prove a non-grantee's map attempt is refused.
- These two proofs ARE the C3 hardening. Cite them in the RETURN as "C3 relocated-surface proof."

## I-14(c) scanout-DMA seam (coordinate with Cerebro — do NOT build it here)
When the GPU (Cerebro's OT-63) scans out from a `SharedPixelBuffer`, that device DMA must route through `dma_arena::fence` (I-14(c)). **Slice 3 does not own the scanout DMA** — it owns the buffer's frames. Your job: ensure the buffer's frames are **fence-compatible** (allocated so the OT-63 scanout can fence them) and **state the seam** in the RETURN so Cerebro's OT-63 Slice 3 (scanout fencing) drops in without redesign. Coordinate; do not implement scanout DMA here.

## Kill-switch (Rule-1) + byte-oracle (I-12)
- **Kill-switch:** cross-process-import-**off** ≡ the current single-process buffer behavior bit-for-bit (with import disabled, the Rung-0/Slice-1 path is byte-unchanged; armed regression, not asserted).
- **Byte-oracle:** `BufferDescV1` already has its Rung-0 fixture — reuse it. If Slice 3 adds any new on-wire import handle/descriptor layout, ship a hand-laid fixture for it (independent of writer+reader).

## I-14 fault-containment + CP-bank consultation (required sections)
- **I-14:** (a) the import/map/teardown path takes **no** AI-governance lock; (b) a client that never tears down can't stall the compositor or kernel (bounded — the Arc pin bounds the leak to that buffer's frames, reclaimed on the last unmap); (c) no **new** DMA path in Slice 3 (scanout DMA is the OT-63 seam above).
- **CP-bank (`docs/concurrency-pitfalls.md`):** cross-process teardown is concurrency — the unmap→PTE-removal→**TLB shootdown** path (buffer.rs already shoots down on Drop) crossing processes/CPUs touches the shootdown/lock-ordering classes (CP-1/CP-29 lineage). State, per class it touches: introduce / guard / N-A + the discriminator. New pitfalls → back into the bank.

## Coordination — buffer.rs is shared with Cerebro (mandatory)
buffer.rs is actively owned by Cerebro's OT-63 present backend (Slice 1 merged `abd8ea6`… wait — Slice 1 is `codex/ot63…`, merging shortly). **Rebase-fresh on `origin/main` right before your close-gate** (Slice 1 + GPU Slice 2 may land in buffer.rs while you work), keep your changes **additive** to the lifetime layer (avoid the present/fence path — that's Cerebro's), and if you must touch the present path, coordinate so the two lanes don't thrash the same lines. The Jarvis-Slice2 ∥ Cerebro-OT63 parallel worked because the file-seams were disjoint; keep yours disjoint from the present path.

## Gate + close + AAG
- Host KATs for the lifetime/permission logic + any new byte-oracle; a **QEMU scenario** proving: cross-process import + composite-from-client-pixels, the lifetime-holds-across-processes teardown (client drops while compositor maps → no UAF), the **C3 proofs** (compositor write faults; non-grantee map refused), and the kill-switch equality. `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set; SPDX; Step-4 self-stat). 4-shard `workflow_dispatch` gate — **one wave at a time, stagger behind Jarvis's Slice 4 + Cerebro's OT-63** (fleet is ~3–4 runners; the Spectrum-outage/contention episode is why). RETURN: green push-CI + 4-shard URLs + the C3-proof evidence + the I-14(c) scanout seam note for Cerebro.
- **Disk reclaim** after PM-confirmed merge.
- **AAG (PM, cross-model lenses):** memory-safety/DMA (the cross-process mapping + teardown, no UAF, no `unsafe` misuse), concurrency (cross-process teardown/TLB-shootdown ordering; ABA-safe reuse), ABI/byte-oracle (any new import layout), completeness (**the C3 proofs are real + falsifiable — compositor write genuinely faults, non-grantee genuinely refused**; kill-switch armed).

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-20-g1-compositor-slice3-crossprocess-buffer-lifetime-oracle.md`. Design of record: `git show origin/main:docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`. **Read buffer.rs FIRST** (`git show origin/main:kernel/src/atrium/buffer.rs`) — most single-surface lifetime mechanics (Arc-pin, dual-PTE RO/RW, immutable descriptor, Drop-after-unmap) **already exist**; Slice 3 is the cross-PROCESS delta + the adversarial proof, NOT a rebuild. The C3 rationale: `git show origin/main:docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` §5.2.
- **One-line:** G1 Slice 3 — generalize the existing single-surface `SharedPixelBuffer` lifetime to **cross-process**: wire Slice-2's attach token to a real **RO+NX map of the client's buffer into the compositor** (zero-copy dual-PTE, the Qubes-C3 win), prove the **lifetime holds across processes** (client drops while compositor maps → no UAF; ABA-safe reuse), and **prove the relocated C3 surface airtight** — compositor write **faults** (falsifiable), non-grantee map **refused**. Inventory buffer.rs first (Arc-pin/dual-PTE/immutable-descriptor already exist — don't rebuild). Kill-switch: import-off ≡ single-process behavior bit-for-bit. Required I-14 fault-containment + CP-bank consultation (cross-process teardown/TLB-shootdown). **Coordinate buffer.rs with Cerebro** (rebase-fresh, keep additive to the lifetime layer, stay off the present/fence path). State the **I-14(c) scanout-DMA seam** for Cerebro's OT-63 (don't build it). Host KATs + QEMU scenario + 4-shard gate (stagger behind Jarvis + Cerebro) + AAG.
- **RAG:** `SharedPixelBuffer Arc pin borrowed Pmap`, `dual PTE compositor RO client RW`, `dma_arena fence scanout`, `TLB shootdown Pmap drop`, `atrium attach buffer provenance`. Health-check first; note-and-fall-back if down (PL owns RAG startup).
