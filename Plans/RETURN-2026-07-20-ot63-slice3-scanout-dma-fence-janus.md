<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-63 Slice 3 scanout-DMA fence (Janus)

## Result

HardwareFlip scanout targets are arena-resident when fencing is enabled. The
arm seam mints `FencedDmaAddr` with `dma_arena::fence`; refusal queues no flip
and synchronously degrades to Software. The pending ring retains the witness,
so the future real register-programming path has no raw-address input.

## Falsifiable evidence

`cargo xtask neo-test atrium_scanout_dma_fence` passes in QEMU and reports:

```text
atrium_scanout_dma_fence=ok fenced_scanout_presented=yes out_of_arena_refused=yes software_degrade=yes governance_tensor_refused=yes no_unfenced_arm=yes killswitch_slice2=yes i14c1=held c2_slice5=not_claimed
```

- Arena-backed scanout: fence admits; HardwareFlip remains pending until the
  synthetic vblank, then completes.
- General-allocator scanout: fence refuses; no pending flip; Software completion
  is immediately observable.
- Governance/tensor negative control: the first page beyond arena bounds cannot
  mint a witness and therefore cannot call the fenced arm API.
- Fencing-off: existing Slice-2 scenario and allocation/arm branch remain the
  byte-for-byte behavioral baseline; no physical DMA is programmed in that rung.

No new byte layout was introduced. The build reuses `FencedDmaAddr`, so the
byte-oracle obligation remains with Slice 4–5's real Gen9 register programming.

## Boundary and review

This is I-14(c1), not c2. Real iGPU VT-d translated-domain placement, RMRR and
aperture reconciliation remain Slice 5 bare-metal. Real Gen9 `PLANE_SURF` and
vblank are also out of scope. CP-bank review found no new spin, nested lock,
IF-off wait, CP-29 allocator-on-arm path, or I-16 publication change.

## Gates

- Focused kernel compile: green.
- Focused QEMU scenario: green.
- Close-gate: green on `origin/main` `a835993e`; all host checks, kernel
  clippy, SPDX, and self-stat passed.
- Push CI: green — https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29769029052
- Four-shard workflow_dispatch: green (all four shards) —
  https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29769279812
- AAG no-unfenced-scanout lens: green. The only fencing-on `arm_flip` caller
  receives the private-constructor witness from `fence_scanout`; ownership and
  drop paths release arena spans exactly once; refusal cannot enter the ring;
  no allocator or new lock is introduced at arm/ISR time; c2 is not claimed.
  After the registration consolidation, the unfenced helper is explicitly
  `arm_synthetic_completion` (not an arm/scanout API); the sole `arm_flip` API
  is witness-taking, making the Slice-4 hardware seam structurally fenced.
