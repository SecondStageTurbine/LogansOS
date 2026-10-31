<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-045: Phase 7.5 BUILD-0 — the Fenced DMA Arena (the I-14(c1) guard)

**Status:** Accepted
**Date:** 2026-06-11
**Design:** `Plans/THINK-2026-06-11-phase-7.5-opening-connectivity-lane.md` (PL-ratified: staged hybrid — fence now, VT-d at 7.5-V; USB-first; QEMU-first)
**Ledger:** I-14(c1) moves mandated → **proven**; I-14(c2) stays mandated-not-yet-proven (7.5-V)

## The threat split (the honesty clause)

I-14(c) — "a driver's DMA cannot reach governance memory" — is two different
threats with two different defenses:

- **(c1) OUR errant programming:** the kernel hands the device a wrong
  address (the near-certain failure class of a from-scratch xHCI/NIC
  bring-up: a mis-computed ring pointer, a stale descriptor, an off-by-one
  span). **This build contains it** — software fence, no hardware needed.
- **(c2) a MISBEHAVING device:** firmware that DMAs wherever it likes. **No
  software check can contain this** — the device does not consult our
  validation. That is the IOMMU's job: 7.5-V (VT-d), required before
  `iwlwifi`-class firmware. This build claims NOTHING about (c2).

## The mechanism (the Linux restricted-DMA-pool / swiotlb pattern)

1. **A pre-registered region:** `dma_arena::init(pages)` reserves one
   contiguous buddy block, once (a second init is refused — the fence is a
   boot-time fact, not a movable target). Bounds live in atomics: the gate
   is **lock-free** — a containment check that cannot become a contention
   point — and an uninitialized arena **fails closed** (`ArenaUnset`
   refuses everything).
2. **Arena-backed allocation:** `DmaBuffer::alloc_in_arena(len)` draws
   page-spans from the region (first-fit, coalescing free — the pure
   `SpanTable`, host-tested); `Drop` returns the span to the arena, never
   the buddy. Connectivity drivers (7.5-A onward) use ONLY this. Existing
   DMA users (GPU/NVMe/AHCI/virtio-blk/net) keep `alloc_zeroed`, outside
   the arena, untouched — the new `from_arena` field is inert (`false`) on
   every pre-7.5 path: **arena-off ≡ today bit-for-bit (Rule 1)**.
3. **The gate:** `dma_arena::fence(phys, len) → Result<FencedDmaAddr,
   DmaFault>` — pure fence-bounds arithmetic (`dma_arena_logic::validate`,
   total: zero-length, unset arena, and overflow are explicit refusals,
   never wraps), counted (admits / refusals).
4. **The single-gate property (the I-14(c1) argument):** `FencedDmaAddr`
   has private fields and exactly one constructor — the gate. The
   connectivity descriptor-programming surface accepts only this type, so
   a driver that skips validation **cannot type-check a device programming
   call**. The same construction-proof as the tensor arena's `mint` (I-4)
   and `SlotKey` (I-13): the boundary is enforced by what can be
   constructed, not by review vigilance.

## The proof

- **Host truth table:** inside / both edges flush / both edges straddled /
  fully outside / zero-length / unset (fails closed) / overflow at both the
  address and the fence — plus the span allocator's first-fit, coalescing,
  double-free refusal, exhaustion, and balance.
- **The adversarial QEMU gate (`dma_arena_fence`):** kill-switch leg
  (uninitialized ⇒ gate refuses everything, today's path untouched); the
  in-arena buffer fences Ok and reaches the mock device byte-verified;
  EVERY out-of-arena address — including one aimed at live kernel-owned
  memory holding a sentinel, both fence edges, raw low memory, the
  overflow corner, and a 100-address deterministic spray — refused with
  `DmaFault`; the mock device's address log shows **0 escapes ever**; the
  sentinel intact; span and buffer balance exact at teardown.

## Scope honored

No VT-d/IOMMU (7.5-V). No driver (7.5-A consumes this). No existing DMA
user moved. No (c2) claim anywhere. Zero new `unsafe` beyond `DmaBuffer`'s
existing zeroing/slice justifications applied to arena frames; one leaf
`SwitchMutex` instance (no new primitive).

## Audited carries (Forge — zero CRITICAL; the named 7.5-A obligation)

- **The `physical_addr()` seam (MAJOR-2, the load-bearing carry):** the
  witness-type construction proof covers `FencedDmaAddr` itself; the
  SYSTEM-wide single-gate property additionally requires that every
  connectivity descriptor-programming function take `FencedDmaAddr` and
  never a raw `PhysAddr`. `DmaBuffer::physical_addr()` stays `pub` (legacy
  drivers program with it legitimately; it also FEEDS the gate), so the
  property is upheld by signature discipline until 7.5-A makes it
  structural. **7.5-A gate obligation:** its dispatch must include the
  discriminator that the xHCI descriptor-programming path's signatures
  consume `FencedDmaAddr` only — grep-assertable, reviewed at the gate.
- **Total-arithmetic doctrine extended to the glue (MAJOR-1, fixed
  in-build):** `alloc_span`/`release_span` now use checked arithmetic like
  the gate (a span that cannot be checked cannot be handed out or freed).
- **The leaked-span gauge (MINOR-1, fixed in-build):** a refused span
  release is counted in an always-on counter — the balance signal exists
  outside neo-test builds.

## Consequences

- The lane opens on a contained footing: 7.5-A (xHCI/USB) programs devices
  exclusively through `FencedDmaAddr` from build one.
- The I-14 row's (c) vector is half-proven, with the remaining half
  (device misbehavior) explicitly tracked to 7.5-V — the ledger states
  exactly what is and is not contained.
