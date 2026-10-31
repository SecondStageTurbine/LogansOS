<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# ADR-076: Arena-backed scanout and witness-only flip arming

**Status:** Accepted (OT-63 Rung 1 Slice 3, 2026-07-20)

## Decision

When scanout fencing is enabled, `SharedPixelBuffer::create` allocates its
page-rounded storage as one contiguous `DmaBuffer::alloc_in_arena` span. The
buffer keeps the existing `Vec<PhysFrame<Size4KiB>>` mapping/CPU view, while an
`Arc<DmaBuffer>` owns and releases the arena span. This is additive to the
buffer protocol and avoids admitting arbitrary general-allocator memory at
flip time.

At HardwareFlip arm, the buffer's complete page span must pass
`dma_arena::fence(phys, len)`. Only the returned `FencedDmaAddr` can enter the
fencing-on `arm_flip` API and pending ring. Refusal does not queue a flip: the
CPU composition already performed by `present` is synchronously published as
the I-14(b) Software fallback. Thus a refused target does not stall and no raw
scanout address reaches a future register-programming seam.

The scanout-fencing toggle defaults off. Off uses the exact Slice-2 allocation
and a separately named synthetic-completion path; it is not a hardware arm and
programs no DMA. The only `arm_flip` API is witness-taking. The ratified
accel-on posture enables the fence before creating flip targets. Slice 4 must
remove the synthetic-completion branch when it introduces real register
programming; the load-bearing hardware seam is structurally witness-only now.

## I-14 fault-containment analysis

- **Lock isolation (I-14(a)).** `dma_arena::fence` reads atomic arena bounds and
  performs total arithmetic. It takes no span-table, framebuffer,
  AI-governance, or tensor lock. The only subsequent lock is the pre-existing
  bounded `IrqSafeMutex` pending ring.
- **Liveness degradation (I-14(b)).** Refusal and ring-full both synchronously
  publish the present epoch after the CPU blit. There is no park, retry loop, or
  IRQ-masked wait.
- **Memory isolation (I-14(c1)).** An enabled flip cannot enter the ring without
  a `FencedDmaAddr`. QEMU proves an arena-backed buffer admits and presents, a
  general-allocator buffer refuses and degrades, and the first page beyond the
  arena (the governance/tensor negative control) refuses. The arena gate also
  counts both admits and refusals.

This guards kernel descriptor/address programming, not a malicious device that
invents an address. I-14(c2)—moving the real Gen9 iGPU out of the identity VT-d
domain into an arena-only translated domain, with RMRR and aperture handling—is
Slice 5 bare-metal. No c2 hardware-containment claim is made here.

## Concurrency-pitfall bank

The fence executes in present context before the pending-ring lock. It adds no
spin, no lock nesting, no IF-off wait, and no allocator call at arm time. The
ISR remains unchanged: pop under the `IrqSafeMutex`, release it, then perform
the lock-free `publish_complete` Release operation. CP-2 is unchanged, CP-29 is
not reopened (arena allocation happens at buffer creation, not AP/ISR flip
arm), and the I-16 monotonic `fetch_max` publication remains the sole completion
signal.

## Byte oracle and later seams

Slice 3 reuses `FencedDmaAddr` and defines no on-wire, in-memory descriptor, or
register byte layout, so no new byte oracle is required. Slice 4–5 owns the real
Gen9 `PLANE_SURF`/vblank programming and its byte oracle; Slice 5 owns c2.
