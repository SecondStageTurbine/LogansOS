# AAG: GPU P1 Slice-6 — Glass into G1 and userland compute ABI

| | |
|---|---|
| Status | Green only after `gpu_glass_slice6` QEMU scenario and all CI shards pass. No hardware claim. |
| Scope | G1 capture → blur/floor → tint → composite; post-Slice-5 three-cap compute ABI. |
| Branch | `codex/gpu-p1-slice6-glass-into-g1-chad` |
| Governing ADR | [ADR-077](ADR-077-gpu-p1-compute-service.md), Slice 6; OT-52 scanout authority remains unchanged. |

## Authority and ordering

Slice 5's graphics-arena narrowing is a precondition to the only root constructor. The kernel creates exactly three typed roots for trusted compositor-owned, distinct surfaces:

| Capability | Rights | Can do | Cannot do |
|---|---|---|---|
| `GpuCompute::Buffer` source | `BUFFER_READ` | Supply the full checked source range | write, queue submit, present |
| `GpuCompute::Buffer` destination | `BUFFER_WRITE` | Receive the pinned blur output | read source, queue submit, present |
| `GpuCompute::Queue` | `QUEUE_SUBMIT`, fixed 64 MiB quota | Run the sole pinned blur manifest | choose pipeline/address/geometry, present |
| `GpuCompute::Fence` | `FENCE_WAIT`/`FENCE_QUERY` | Observe completed token | submit or present |

There is no creation syscall. Generic derive and grant only intersect rights, and revoke removes the root and every descendant. `GpuCompute` values preserve their variant through derivation; an ordinary G1 `Buffer` is not convertible and fails the submit ABI closed.

GPU-reachable `SharedPixelBuffer` writes have a separate per-buffer mutable-view
guard. `contiguous_pixels_mut` CAS-acquires it and returns `EAGAIN` while a
writer is live; its RAII span releases it on drop. This is intentionally below
cap policy: two valid, grantable write-cap holders naming the same destination
cannot create aliasing `&mut` DMA views before `COMPUTE_IO_GATE` serializes
their compute stores.

## Fluidity and failure boundary

The composition path has no new display path: it stops at a completed `DispatchToken`. `GlassSurface` never calls `present`, flip-arm, or scanout. `SharedPixelBuffer::present` retains the established `scanout_authorized()` decision, so a compute fence cannot display a frame. This is the fence/present separation required by OT-52.

The CPU floor remains live in three cases: GPU kill-switch off, no trusted driver, and bounded submit/wait failure. The QEMU gate compares all of those composite bytes against one literal 8×8 `r=1` blur-plus-tint oracle. The public material remains fixed `r=8`; the reduced radius is test-only and does not enter the user ABI. Damage is committed as the changed rectangle expanded by the blur radius; the QEMU gate also observes the single authorized composite byte at that damage location.

The cited literal composite oracle is a golden-master regression snapshot, not
the independent proof. Before production code runs, a test-local reference
recomputes the fixed r=1 fixture with separate Q16 constants, explicit
clamp-edge loops, and independent integer tint arithmetic, then must match the
literal. Accelerator/CPU equality is therefore correctly scoped as
shared-arithmetic parity, while the literal/reference comparison remains the
independent specification check.

Reduce Transparency is exercised too: the scenario draws its live
`copy_backdrop` arm and requires the resulting composite bytes to equal the
independently tinted, unblurred source, with a CPU-floor fence and unexpanded
damage.

## Evidence required for close

- `gpu_glass_slice6`: userland successful submit/fence, derived no-submit denial, ordinary-buffer no-self-mint denial, actual cross-process least-rights grant, and root/descendant revocation.
- Same scenario: CPU-floor, accelerator-success, and forced-degrade all match the literal composite oracle; enabled success has a nonzero compute sequence.
- Same scenario: an overlapping second mutable destination view returns `EAGAIN`; the guard releases after the first writer drops.
- Same scenario: the independent r=1 fixture matches the literal oracle, and Reduce Transparency's `copy_backdrop` composition matches the independently tinted source.
- Same scenario: compute-only present returns `EPERM`; separately authorized present succeeds.
- Host `atrium-logic-tests`: exact integer tint KAT and existing blur/damage KATs.
- CI is QEMU-only. It does not demonstrate Intel EU execution, VRR, or hardware DMA containment; those remain the explicitly separate silicon gates.
