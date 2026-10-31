<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: one GPU/acceleration substrate for Servo and the Atrium — Janus design

**Date:** 2026-07-12
**Author:** Janus (independent cross-model panel contribution)
**Dispatch:** `Plans/DISPATCH-2026-07-12-gpu-accel-path-design-think.md`
**Scope:** design only; host-only; no code, QEMU, CI, or hardware run
**Independence:** produced without reading Oracle's THINK

## 0. Decision in one breath

Build **one kernel-owned Intel GT substrate with two adapters**, not a second graphics driver beside the existing AI driver: the current `AccelDriver` remains the compute adapter, while a new capability-gated graphics adapter supplies buffers, per-context PPGTT address spaces, typed queues, completion fences, hang/reset accounting, and presentation. G1's settled userland compositor and shared-pixel-buffer model remain the front door. The first pixels stay software-rendered. Hardware then proves fenced copy/clear and presentation before it attempts the Gen9 3D pipeline. Servo consumes an intentionally narrow **WebRender-profile GL/context/present adapter** (`surfman-neo` + `libneogl`), with WebRender's known shader set compiled offline and provenance-pinned. It is not a public general-OpenGL ABI. A custom `wgpu-hal`/Vello route stays open on the same resource, queue, fence, and shader abstractions, but it is not the first backend.

The security boundary is equally fixed: browser script/layout processes and ordinary Atrium clients never receive GPU-submit authority. A disposable paint service holds a restricted, quota-bound `GpuQueueCap`; the kernel constructs or validates every hardware batch, owns every GGTT/PPGTT entry, and accepts only GPU-visible addresses minted by the graphics DMA fence. Per-context PPGTT isolates consumers; a dedicated VT-d domain confines the whole iGPU to graphics-owned memory plus a tiny explicit privileged set needed to preserve the boot scanout. A fault quarantines acceleration and returns the compositor to software. It never waits while holding an AI-governance lock and never makes governance depend on GPU recovery.

## 1. Ground truth: what exists now

This design starts from the shipped silicon path, not from a generic GPU diagram.

### 1.1 Hardware and landed driver

The physical target is the Dell OptiPlex 7080's Intel UHD 630 / Gen9.5 iGPU at PCI `00:02.0`, device ID `0x9bc8`. The live implementation is `kernel/src/drivers/accel/accel_intel.rs`, registered as `Device::Accel(3)` behind default-off `accel-intel`.

Today it exposes an AI accelerator, not a graphics device:

- F32 `Add`, bounded `Matmul`, and `Gelu` through the `AccelDriver` trait;
- kernel-owned physically contiguous `DmaBuffer` storage and CPU copy-to/from-device;
- one in-flight asynchronous token at a time, completion initially observed through a bounded seqno poll;
- `preemptible() == false`; unsupported or oversized work falls back to `accel_cpu`;
- a single `IrqSafeMutex<IntelGpuInner>` serializes hardware access;
- no display/modeset, no raster command profile, no textures/samplers/render targets, no general shader compiler, no GL context, no graphics IRQ completion path, and no page-flip/present API.

Phase 6 proved the GuC-free **legacy render-ring + GGTT** compute path on silicon. The decisive iter4–8 history is already a trap bank:

- ring-only `PIPE_CONTROL` and seqno worked before EU dispatch worked;
- missing `STATE_BASE_ADDRESS` left instruction fetch parked;
- the no-op kernel separated dispatch setup from data access;
- iter6's Add raised `RING_FAULT=0x9e1` (invalid PTE);
- iter7 decoded a stable **GGTT** fault at the wrong A64 address, proving the cross-thread payload landed in the wrong GRFs;
- iter8 fixed the NEO indirect-data layout and retired `[1,2]+[3,4]=[4,6]` with `RING_FAULT=0`;
- BUILD-4b added matmul/GELU and CPU↔GPU migration, then closed Phase 6.

The driver is more capable today than the Phase-6 close alone suggests. Phase 7 BUILD-4X added GuC-free **execlist/LRC submission and a real 48-bit per-context PPGTT**:

- legacy-ring PPGTT was falsified: the engine still selected GGTT;
- an execlist context switched in and retired with its PPGTT active;
- tensor data cleared from GGTT completed through PPGTT only;
- the cross-tenant negative test faulted B's access to A's VA while B's own tensor succeeded;
- a read-only mapping allowed reads and blocked the write. Gen9 did **not** report that permission failure through `RING_FAULT`; the EU wedged and the unchanged output frame was the proof.

Therefore the core ingredients for isolated graphics contexts exist in demonstrated form: LRCs, PPGTT page construction, GGTT programming, command-stream byte oracles, forcewake, CSB observation, seqno completion, fault-address decode, and silicon evidence. They are embedded in a 3,214-line compute driver rather than factored into a reusable graphics core.

### 1.2 Display state

The owner-visible display is still the Limine-provided 32-bpp linear framebuffer. `kernel/src/drivers/fb/mod.rs` supplies clear, plot, fill, RGBA alpha blit, bitmap glyphs, and strings behind one framebuffer lock. The console scrolls by copying pixels. There is no compositor, double buffering, retained scene, vblank/page-flip control, GPU text, display-engine ownership, or reset recovery.

The existing GPU ADR deliberately treats this framebuffer as expendable during compute and avoids modesetting or scanout quiesce. That was correct for an isolated hardware proof and is unacceptable as the browser/desktop product contract. The graphics lane must preserve the boot framebuffer until an independently proven display slice replaces it.

G1 is design-complete but unbuilt. Its settled shape is not reopened here:

- SurfaceCap is an IPC channel/session, not a new kernel graphics object;
- the only shared pixel object is kernel-owned, with client RW and compositor RO/NX sibling mappings;
- scene, damage, pacing, focus policy, and composition live in the userland compositor;
- `InputFocus` routing remains the G1 kernel mechanism;
- compositor-off equals the v1 console.

### 1.3 DMA and IOMMU state — the important non-claim

I-14(c) exists, but it does **not** fence the GPU today.

- `dma_arena` reserves one contiguous arena and mints `FencedDmaAddr` only after `dma_arena::fence(phys,len)` validates the span.
- Connectivity drivers allocate through `DmaBuffer::alloc_in_arena`; the Intel accelerator still uses ordinary `DmaBuffer::alloc_zeroed`.
- VT-d initially identity-domains every PCI device. It moves only devices classified as `Connectivity` into the arena-only fence domain.
- The ledger explicitly records GPU/NVMe/AHCI/virtio as identity-domain substrate under VT-d.

Thus the current GPU has PPGTT tenant isolation but remains able, at the VT-d layer, to DMA across identity-mapped system RAM. PPGTT and VT-d solve different threats. The graphics build may not claim I-14(c) until both layers apply to its real submission path.

## 2. The architecture

### 2.1 One hardware owner, two adapters

Refactor toward this ownership tree in behavior-preserving slices:

```text
IntelGt (only owner of BAR0/MMIO, forcewake, engines, GGTT, reset epoch)
  |-- IntelComputeAdapter : existing AccelDriver (Add/Matmul/GELU)
  `-- IntelGraphicsAdapter: GraphicsDevice (buffers/contexts/queues/fences/present)
```

`IntelGt` is a physical-device object, not a public ABI. It owns:

- probe identity and supported Gen9.5 register profile;
- forcewake and power-domain lifetime;
- render/blitter engine state and the one legal MMIO programming path;
- LRC/execlist creation, context IDs, CSB consumption, PPGTT roots and GPU TLB invalidation;
- trusted GGTT ranges for ring/LRC/HWSP/boot-scanout plumbing;
- submission epochs, bounded completion, fault attribution, quarantine and reset;
- arbitration between latency-critical graphics and batch AI work.

The current monolith must not be rewritten in one patch. First extract the already-proven pure packet/page-table builders and an ownership wrapper without changing emitted bytes. Existing Phase-6/7 oracles are the regression authority. Only after one owner exists may a graphics adapter submit.

Why this is mandatory: two drivers independently programming forcewake, `RING_MODE`, GGTT PTEs, ELSP, CSB pointers, or reset state would create an unreviewable cross-driver race. “One substrate” means one hardware owner, not merely similar buffer structs.

### 2.2 Kernel mechanism surface

The graphics adapter exposes capability objects, not raw DRM ioctls or raw batch buffers:

| Capability/object | Authority | Holder |
|---|---|---|
| `GpuDeviceRoot` | enumerate profiles, create contexts/queues, quarantine/reset | kernel + trusted graphics service only |
| `GpuContextCap` | one PPGTT, reset epoch, resource namespace and budget | compositor or disposable paint service |
| `GpuBufferCap` | a typed buffer/texture/render target with MAP/READ/WRITE/IMPORT rights | derived to the minimum consumer |
| `GpuQueueCap` | submit one named command profile within byte/time/resource quotas | compositor or paint service; never script/layout clients |
| `GpuFenceCap` | await/query one generation-tagged completion; no submit right | any party needing present/completion feedback |
| `ScanoutTargetCap` | target the privileged boot/display surface | compositor only; never a browser renderer |

The names are design names; the build decides whether they are new handle variants or attenuated rights on fewer root objects. The invariants do not depend on that encoding:

- child rights are a subset of parent rights;
- IDs carry a generation and reset epoch;
- a queue is permanently bound to one context and command profile;
- buffers name immutable size/format/usage and cannot change identity through mapped bytes;
- submission references handles, offsets and lengths, never physical or GPU virtual addresses;
- kernel code resolves resources, maps PPGTT, lays the hardware batch in kernel-owned memory, and rings the doorbell;
- undefined flags/reserved fields are must-be-zero and versioned for forward growth.

No ordinary Atrium app needs a queue. It receives the settled G1 shared pixel buffer, draws in software, and commits through its SurfaceCap channel. Only the compositor and purpose-built render services receive queue authority.

### 2.3 Userland service split

```text
Atrium client --SurfaceCap/SharedPixelBuffer--> compositor
                                                    |
                                                    +-- CPU raster/composite --> fb::blit_rgba
                                                    |
                                                    `-- restricted GpuQueueCap --> IntelGraphicsAdapter

Servo script/layout process --display list--> disposable WebRender paint service
                                              (surfman-neo + libneogl)
                                                    |
                                             restricted WebRenderQueueCap
                                                    |
                                              rendered GpuBufferCap
                                                    |
                                         compositor RO import + present
```

The split is a confinement decision. SpiderMonkey/script/layout holds no GPU queue, MMIO, scanout, or physical-memory authority. A separate paint service runs WebRender and is disposable. Its queue profile accepts only the resources it owns and the WebRender command vocabulary. The compositor receives the finished buffer read-only, then owns final composition/presentation.

If the paint service is compromised, it can at worst waste its quota, corrupt its own buffers, or hang its context. It cannot submit arbitrary MI commands, address another PPGTT, target scanout, or reach another process's memory.

### 2.4 Scheduling: UI deadlines dominate AI batches

The iGPU render engine is already an AI accelerator and will become the display engine. The owner-visible rule is:

1. present/compositor work has the highest device priority;
2. browser paint follows;
3. opportunistic AI compute runs only inside the remaining budget.

Current hardware work is non-preemptible from the driver's perspective. Until Gen9 context preemption is proven, the scheduler must bound each accepted AI/graphics batch tightly enough that one job cannot miss multiple frame deadlines. A long or unbounded job is refused or routed to CPU, not admitted on hope. Admission uses measured worst-case GPU time plus a margin; present work never waits behind an unbounded AI kernel. Later LRC priority/preemption is an optimization with its own silicon proof, not a prerequisite smuggled into the first graphics slice.

## 3. Software-render floor

The software path is the first build because it proves the consumer contract without adding GPU risk.

Define a small renderer/presenter seam in userland:

```text
RasterBackend
  create_target(descriptor) -> buffer
  render(scene_or_display_list, damage, target) -> fence/result
  present(target, damage) -> PresentFence event

CpuRasterBackend  -> SharedPixelBuffer -> compositor -> fb::blit_rgba
GpuRasterBackend  -> GpuBufferCap      -> compositor -> present
```

The exact CPU library is replaceable. Vello CPU, tiny-skia, or a smaller local raster core may implement it; the ABI is pixels/damage/fence, not a crate's scene type. The floor must provide:

- premultiplied RGBA target with immutable dimensions/stride/format;
- bounded damage rectangles;
- real completion: fence fires only after the composite/blit finished;
- deterministic clipping and integer-safe extent arithmetic;
- compositor budget/drop accounting;
- a framebuffer-console escape path independent of the compositor.

This unblocks Atrium's first composited pixels and the limited Blitz reader. It is not sufficient for Servo/WebRender performance, WebGL/WebGPU, high-refresh effects, or the release browser gate.

## 4. WebRender/GL-first route

### 4.1 The actual Servo seam

The audited Servo tree binds page paint to WebRender 0.69 plus `surfman`/GL (`glow`/platform GL edges). Servo's `wgpu-core` is tied to its optional WebGPU web API; it is not today's page compositor. Therefore “implement wgpu first” does not make current Servo render.

Janus chooses **an accommodated GL/context/present path**, not an out-of-tree WebRender backend fork, for the first browser integration:

1. `surfman-neo` implements native context/surface creation over `GpuContextCap`, offscreen targets, resizing, buffer import, and compositor-owned present.
2. `libneogl` exposes only the GL entry points and object semantics WebRender actually calls through `glow`.
3. GL objects map to `GpuBufferCap`, texture/sampler/pipeline objects, and typed command records; no GL call emits raw Gen9 commands in user memory.
4. WebRender's finite build-time shader variants are compiled offline for Gen9 by a pinned, permissively licensed Intel/Mesa compiler toolchain. Source, compiler revision, options, source hash and output hash are checked in as provenance. Runtime compile/link accepts only a canonical source/variant hash in that manifest and fails closed on an unknown shader.
5. Kernel-owned builders lower the accepted WebRender profile to PRM-cited Gen9 3D packets and bind only resources present in the submitting context's PPGTT.

This is a **WebRender profile**, not OpenGL conformance. It deliberately avoids a general runtime shader compiler in the first slice and keeps upstream WebRender's public GL expectations intact. It must inventory the exact WebRender call/shader closure at the pinned Servo revision before implementation; “likely enough GL” is not a gate.

### 4.2 Why not port WebRender to a private backend first?

A private backend looks smaller until maintenance begins. It forks the point Servo actively evolves, must reproduce WebRender's GL device semantics, and still needs Gen9 shaders, resources, synchronization and present. The GL-profile adapter localizes divergence in `surfman-neo`/`libneogl`, which are explicit platform seams.

The backend-port alternative remains a kill condition: if the six-week Servo host/closure spike proves WebRender relies on GL behavior too dynamic to manifest and validate, stop and compare a funded upstreamable WebRender device backend against a bounded Mesa userspace port. Do not quietly grow `libneogl` into an unbounded GL implementation.

### 4.3 What this does not yet unlock

WebRender acceleration does not equal full WebGL. Arbitrary site-provided GLSL cannot use the manifest-only shader path. WebGL needs a sandboxed runtime compiler, a broader validated graphics profile, resource limits and conformance gates. WebGPU needs Servo's `wgpu` path. The modern-site browser battery must label these features independently; no page-paint success may be reported as WebGL/WebGPU success.

## 5. Keep the wgpu/Vello route open

The native graphics object model intentionally resembles the irreducible WebGPU concepts—device, queue, buffer/texture, pipeline, command encoder, fence—without adopting WebGPU as the kernel ABI.

Later, add a LogansOS `wgpu-hal` backend over the same userland `neo-gfx` service:

- `wgpu-core` retains validation/state tracking;
- a `wgpu-hal` Neo backend maps resources and queues to caps;
- Naga handles WGSL validation/lowering, while the sandboxed shader compiler service produces Gen9 code;
- the kernel still validates resource ownership, usage, bounds, profile and batch structure;
- Vello GPU then consumes ordinary wgpu, and Servo's WebGPU implementation uses the same backend.

Vello itself says it requires compute shaders and a wgpu context; wgpu currently expects Vulkan, Metal, D3D12 or GL backends. LogansOS has none. “Pure Rust” does not remove the hardware backend. The Neo backend is a real later build, estimated separately, not a flag enabled after GL.

The earlier software path remains valuable: Vello CPU/Blitz and the compositor can stay alive while wgpu is absent or the GPU is quarantined.

## 6. DMA, address spaces and capability containment

### 6.1 Two translation boundaries, two threats

```text
process handles/rights
  -> per-context PPGTT: context A cannot address context B
    -> VT-d graphics domain: the iGPU cannot DMA outside graphics-owned physical pages
      -> system RAM
```

- **PPGTT** enforces intra-GPU tenancy and buffer permissions. Phase 7 proves this boundary on silicon.
- **VT-d** constrains a buggy/malicious device or driver address that escapes the PPGTT/GGTT design. Today the iGPU is identity-domained; graphics must change that.

Neither substitutes for the other.

### 6.2 Evolve the arena without breaking connectivity

The current `dma_arena` is a single connectivity arena and its `FencedDmaAddr` has no domain tag. Reusing it unchanged would let the GPU's VT-d domain see USB/NIC buffers and would force large textures into one physically contiguous boot reservation.

Generalize it compatibly to **named arenas/extents**:

- keep the existing connectivity arena and `fence()` behavior for current callers;
- add a dedicated `Graphics` arena identity and domain-tagged witness (conceptually `FencedDmaAddr<Graphics>` or `{arena_id,generation,span}`);
- permit several boot-reserved physical extents or page-granular spans rather than requiring one huge contiguous texture allocation;
- retain checked total arithmetic, allocation balance, refusal counters and private witness construction;
- require every function that writes a GGTT/PPGTT leaf or hardware address to accept the graphics witness, never raw `PhysAddr`.

The first hardware slice may use a bounded boot-reserved extent for simplicity, but the browser memory plan must size it from measured texture/surface demand and reject cleanly on exhaustion. It may not silently fall back to unfenced buddy pages.

### 6.3 The VT-d graphics domain

Move PCI source ID `00:02.0` into its own VT-d domain only after mapping:

1. all graphics-arena extents;
2. the kernel-owned GGTT/LRC/HWSP/page-table/command pages the engine must fetch;
3. the existing Limine framebuffer/scanout pages as a **privileged exception** available only to the compositor/admin context;
4. any Intel-reserved/stolen-memory or DMAR RMRR ranges proven necessary from PCI/DMAR/PRM state.

The exception is important. Narrowing the iGPU to only the generic arena before accounting for its active boot scanout can blank or wedge the only visible console. The Stage-B plan must first inventory the IGD DMAR scope, stolen-memory registers and current plane surface, then build the domain. Unknown required ranges fail the acceleration slice closed; software rendering continues on the boot framebuffer.

GGTT remains a trusted global space for ring/LRC/HWSP/scanout plumbing only. Client data and render targets are PPGTT-only wherever hardware permits. A renderer never receives a GGTT address.

### 6.4 Command submission

Raw user batch submission is forbidden. The accepted record contains:

- queue/context handle + expected reset epoch;
- named command profile (`Copy`, `Composite`, `WebRender-vN`, later `Wgpu-vN`);
- resource handle table with declared READ/WRITE/RENDER_TARGET/TEXTURE usages;
- bounded operations/rectangles/draw counts and an estimated deadline budget;
- input fences and one output fence.

Validation proves:

- handles are live, generation-correct and owned/derived into this context;
- every range is within the immutable buffer descriptor and usage rights;
- no write targets an RO/imported/cross-context resource;
- command count, dimensions, multiplication and total bytes are bounded;
- shader/pipeline hash belongs to the queue profile;
- every GPU VA is kernel-selected inside the context PPGTT;
- every physical leaf carries a graphics `dma_arena::fence(...)` witness;
- reserved/unknown fields are zero;
- the resulting batch cannot address MMIO, GGTT outside its trusted window, privileged scanout, or secondary arbitrary batch buffers.

The kernel publishes payload/state/batch bytes, performs the required cache and DMA ordering, then updates the ring/ELSP doorbell. Completion uses the fence's generation + reset epoch, not a bare wrapping seqno.

### 6.5 Revocation and lifetime

Closing/revoking a context:

1. marks it `DYING` with Release; new submissions fail;
2. cancels queued but unsubmitted work;
3. waits boundedly for in-flight completion without holding a GPU/arena/context lock;
4. on timeout, attributes guilt and resets/quarantines the engine;
5. fails all affected fences with the reset epoch;
6. removes PPGTT bindings and invalidates GPU TLBs;
7. only then unpins/reclaims buffers and page tables.

An in-flight request pins its context, PPGTT and every referenced buffer. IRQ completion queues deferred reclamation; it never frees complex objects in hard-IRQ context.

## 7. I-14 analysis

### I-14(a) — lock/IRQ isolation

- GPU interrupt/fault paths acquire **no AI governance lock** and do not call admission, tensor, model, inference or residency policy.
- Split the current `IrqSafeMutex<IntelGpuInner>` before adding graphics IRQs. Process-context submission/configuration uses a leaf `SwitchMutex`; the hard-IRQ path reads/clears bounded device status, writes a fixed-size completion record, updates atomics, and schedules deferred work.
- No forcewake poll, seqno wait, reset, buffer free, PPGTT teardown, wakeup or VT-d repoint runs under an IF-off GPU lock.
- GPU locks are leaf and never nested with DMA-arena, IOMMU, compositor or scheduler locks. If implementation cannot avoid nesting, it must widen and document I-5's currently exhausted `1..=31` ordered band before use; it may not invent level 32 silently.

### I-14(b) — liveness degradation

- Every poll/wait has a deadline and names queue/context/seqno/epoch/RING_FAULT/CSB/head/tail.
- A hang marks the guilty context, stops new submit, fails its fences, attempts a bounded engine reset, and quarantines hardware acceleration if reset/restore does not prove clean.
- The compositor switches to `CpuRasterBackend`; the v1 framebuffer console remains independently callable. If a whole-GT reset disrupts scanout, serial/block log remains authoritative and the display-restore slice must reconstruct the boot surface before claiming graceful visual fallback.
- Governance scheduling, capability revocation, filesystems and Arthur do not await GPU recovery. Losing acceleration reduces rendering, not system authority.
- Queue quotas and maximum batch time prevent an untrusted paint service from turning “legal work” into indefinite denial of display.

### I-14(c) — memory isolation

- All graphics buffers, command/state pages and PPGTT pages are graphics-arena allocations with a typed `dma_arena::fence` witness at every hardware-programming boundary.
- VT-d maps only graphics extents plus the explicit privileged scanout/reserved set; an out-of-domain DMA must fault and identify source ID `00:02.0`.
- Per-context PPGTT maps only that context's derived buffers; positive own-access and adversarial cross-context/RO-write gates remain permanently armed.
- GGTT client-data mappings are forbidden; scanout/GGTT operations require compositor/admin authority.
- The negative gate pairs every expected fault with a positive control. A timeout is not accepted as isolation unless the destination remains unmodified and the captured hardware signature matches the known Gen9 permission-fault behavior.

## 8. Normative concurrency-pitfall consultation

`docs/concurrency-pitfalls.md` was read as normative. The design maps all CP classes so later dispatches can copy the applicable rows and sharpen them against real code.

| CP | Introduces / guards / N-A | Required discipline and permanent discriminator |
|---|---|---|
| CP-1 ABBA | **Introduces** device/context/queue/arena/IOMMU locks | leaf/no-nesting design; otherwise explicit I-5 levels and forced two-thread inverse-order test that must trip lockdep pre-fix |
| CP-2 IF-off wait | **Introduces risk** through IRQ completion and current `IrqSafeMutex` | no wait/poll/reset under IF-off; debug IF-off dwell counter + grep/audit of GPU IRQ locks |
| CP-3 IRQ lock across wait | **Introduces risk** | IRQ path bounded status only; completion/reset/reclaim deferred; timeout test asserts interrupts progress during a hung GPU |
| CP-4 wrong CPU shootdown target | **Guards/inherits** when shared-buffer mappings revoke | use the existing pmap shootdown API, never a graphics-private IPI path; existing no-omission/ready-mask gates stay armed |
| CP-5 freed tables/lazy CPU | **Guards/inherits** during buffer/Pmap teardown | borrowed mappings and PPGTT/CPU page tables remain pinned until CPU shootdown + GPU fence/TLB invalidation complete; teardown/reuse poison test |
| CP-6 invalidation ordering | **Introduces GPU analogue; inherits CPU rule** | CPU PTE removal follows existing sequence; PPGTT edit performs PRM-defined cache/TLB invalidation before frame reuse; stale-translation negative test |
| CP-7 ACK/EOI | **Introduces analogous GPU IRQ rule** | capture cause/CSB, acknowledge/clear device source, publish completion, then LAPIC EOI exactly once; counters assert one cause→one EOI and no repeated storm |
| CP-8 published-not-ready target | **Introduces** contexts/queues/buffers | publish handle Release only after VT-d domain, PPGTT, queue and reset epoch are ready; forced concurrent lookup before-ready must fail cleanly |
| CP-9 store→load reorder | **Introduces** shared command/completion rings | real CPU/DMA fences around payload→owner/doorbell and device completion→payload; ring litmus with adversarial interleaving |
| CP-10 compiler vs CPU fence | **Introduces risk** | no `compiler_fence` used as DMA/MMIO ordering; grep gate plus hand-reviewed doorbell helper |
| CP-11 wrong atomic ordering | **Introduces** ready/dying/completion/reset flags | Release publication, Acquire consumption; Relaxed allowed only for counters; normative Relaxed control-flag grep gate |
| CP-12 ABA/slot reuse | **Introduces** cap handles, context IDs, seqnos | generation + reset epoch on every lookup/fence; stale-handle and seqno-wrap/reuse attacks resolve to clean failure |
| CP-13 torn multiword read | **Introduces** CSB/fault/completion snapshots | snapshot under one leaf lock or seqlock generation; fault record coherence test never mixes context/VA/epoch |
| CP-14 lost wakeup | **Introduces** fence waits | install waiter token under queue state lock, recheck completion before park, wake after state transition; forced complete-between-check-and-park test with park/retry balance |
| CP-15 stranded wakeup | **Introduces** IRQ→waiter completion | use scheduler's proven wake route, not a private EOI-only reschedule assumption; no-timer-AP dispatch-within-N-ticks gate remains armed |
| CP-16 wake under foreign lock | **Introduces risk** | collect completed fence waiters under queue lock, drop it, then wake; lockdep/forced completion asserts no GPU/arena/IOMMU lock held at wake |
| CP-17 publication-before-ready | **Introduces** every resource registration | explicit monotonic READY published last; reset is revocable readiness, so each submit also proves matching live epoch; forced create/reset/submit race |

CP-4/5/6 are not dismissed as “graphics N-A”: shared-buffer unmap and revoke use CPU page tables, while PPGTT has its own invalidation/reuse hazards. The graphics build inherits the CPU guards and adds GPU-specific invalidation proof.

## 9. Intel GPU prior-art trap mine

Read Intel PRMs and Linux i915/Mesa for principles only. Linux/i915 is GPL and **no implementation code, layout helper, macro or control flow may be copied** into the MPL kernel. Intel-defined packet/register layouts are re-derived from the PRM and pinned by independent byte oracles, as Phase 6 already does. Mesa is useful for shader/compiler and field-experience study; any build-time tool use requires an explicit license/provenance record.

| Trap | Why it matters here | Capture-before-run discriminator |
|---|---|---|
| Wrong address space (GGTT vs PPGTT) | already caused the 4-1 dead end; ring/state/data may resolve through different paths under LRC | active root, GTT-select bit, fault VA, and “client data absent from GGTT” assertion on every test |
| Stale GGTT PTE survives warm reboot/reset | the project already learned clear ≠ skip | clear trusted slots on init/teardown, readback, invalidate, and negative probe of the old VA |
| PPGTT edit without GPU TLB/cache invalidation | can access a freed/reused frame after unbind | generation-tagged remap poison test; old VA must fault, never read the new occupant |
| Reserved/MBZ packet bits or wrong packet length | malformed commands can hang the engine with no useful fault | PRM hand-laid bytes, length/MBZ validator, packet boundary dump before doorbell |
| Publication before tail/ELSP doorbell | device sees partial commands/state | payload hash + Release/DMA fence + post-submit command snapshot; fault injection delays owner bit |
| Forcewake imbalance or wrong domain | MMIO reads return stale/zero or GT remains awake | RAII acquire/release counters balanced on every error; ACK poll bounded and logged |
| CSB pointer wrap/overflow or accepting another context's event | false completion/use-after-free | log read/write pointers and every nonzero entry; require our context ID and switch-in→complete/idle sequence |
| Bare seqno wrap/ABA | old completion can satisfy a reused token | 64-bit software generation + reset epoch; wrap/reuse host oracle |
| Buffer/page-table freed before request retires | GPU use-after-free | request pins full resource closure; free-before-fence adversarial test sees zero physical reuse |
| Permission fault is not `RING_FAULT` on Gen9 | 4X-3 proved an RO write can wedge with no ring fault | positive RO-read + all-RW write controls; blocked write requires unchanged destination plus timeout signature |
| Hang/reset destroys innocent contexts | reset is broader than the guilty batch and reverses readiness | guilt attribution, reset epoch, all affected fences fail, contexts reject submit until rebuilt |
| Reset or VT-d repoint disrupts active scanout | iGPU owns the only visible framebuffer | capture plane/GGTT/scanout state before narrowing/reset; restore probe; serial/block log authoritative |
| Stolen/reserved memory omitted from IOMMU domain | IGD may access firmware/scanout memory outside ordinary arena | decode PCI stolen-memory + DMAR/RMRR; out-of-domain test only after privileged-set inventory |
| Cache/MOCS/LLC assumption wrong | CPU/GPU see stale texture, command, or fence data | pattern fill→GPU read/write→CPU read with explicit flush variants; no “coherent because iGPU” assertion |
| Engine-global state raced by AI and graphics | current compute and future render share forcewake/ring/reset | one `IntelGt` owner; deterministic interleaved AI/paint submission oracle; no direct adapter MMIO |
| Non-preemptible batch starves frame | current driver reports non-preemptible | maximum batch timer; reject/CPU-fallback over budget; frame-deadline stress with AI active |
| Raw batch permits MI register/memory writes | compromised browser can escape its buffers | no raw batch ABI; opcode/profile allowlist and kernel-selected addresses; forbidden-op adversarial corpus |
| Surface/stride/tiling/alignment mismatch | scanout or sampler walks adjacent memory | checked descriptor arithmetic, PRM alignment oracle, guard pages around render target |
| Fake present completion | render seqno does not mean scanout/displayed | distinguish RenderDone, CompositeDone and Presented; Presented comes only from real blit/page-flip/vblank evidence |
| Modeset/watermark ordering | an incorrect plane/pipe/watermark sequence can blank/hang display | modeset is a later independent slice with full state capture and rollback; boot mode remains untouched before it |

OSDev's Intel pages are useful as warning labels, not specifications: they emphasize stolen memory, GGTT-backed plane surfaces, EDID/GMBUS, pipe timing and watermarks, and explicitly warn that modern display programming is difficult. Intel Kaby Lake PRM Volumes 2/5/6/7/11/12/14/16 remain the authority.

## 10. Build slices and sequencing

Each slice is independently kill-switched and carries its own positive control, negative control, byte oracle, fault tree and hardware-evidence plan when silicon begins.

### GFX-0 — software floor (first pixels)

- Land the CPU `RasterBackend`/present seam over G1's shared buffer and `fb::blit_rgba`.
- Full-screen and damage-rect composition, real present events, drop/budget counters.
- No GPU probe, no DMA, no new Intel code.
- **Unblocks:** Atrium composited first pixels and limited Blitz reader.
- **Does not unblock:** Servo/WebRender performance or release browser.
- Estimate: 2–3 person-months.

### GFX-1 — one Intel GT owner, behavior unchanged

- Extract MMIO/forcewake/GGTT/LRC/PPGTT/CSB/fault/seqno primitives under `IntelGt`.
- Existing `AccelDriver` delegates through it; emitted bytes and Phase-6/7 behavior remain identical.
- Add deterministic arbitration skeleton, but no graphics command.
- **Gate:** every current Intel host oracle plus exact command/PTE hashes; existing silicon scenarios unchanged at the next hardware window.
- Estimate: 2–4 person-months.

### GFX-2 — graphics DMA/VT-d domain

- Generalize named DMA arenas/witnesses; allocate graphics buffers only from `Graphics`.
- Inventory IGD DMAR scope, stolen/reserved memory and active Limine scanout.
- Build the GPU-specific VT-d domain; keep privileged scanout separate from client PPGTT data.
- Positive in-arena DMA plus negative out-of-domain DMA, with governance and framebuffer survival controls.
- Acceleration refuses to start if VT-d/domain proof is absent.
- Estimate: 3–5 person-months.

### GFX-3 — cap objects + fixed copy/clear + shadow present

- Create context/buffer/queue/fence capability logic and host-testable validation.
- First hardware profile is fixed `Copy/Clear`, using blitter or a provenance-pinned minimal render kernel; no arbitrary shaders.
- Render to a graphics buffer, wait by bounded poll, then CPU-copy through the software compositor to the boot framebuffer.
- Proves resource lifetime, PPGTT, command validation, fence truth and software fallback without modesetting.
- **Unblocks:** first real accelerated Atrium primitive, not yet WebRender.
- Estimate: 3–5 person-months.

### GFX-4 — IRQ completion, hang/reset/quarantine

- Add bounded GPU interrupt capture and deferred completion/wakeup.
- Add generation/epoch fences, hangcheck, guilty-context accounting, reset/rebuild and permanent quarantine fallback.
- Prove an injected hung batch cannot stall governance or the software compositor.
- This slice is mandatory before any hostile browser content reaches an accelerated path.
- Estimate: 2–4 person-months.

### GFX-5 — Gen9 3D/WebRender-profile GL offscreen

- Inventory the pinned Servo/WebRender GL calls and shader variants.
- Land PRM-derived Gen9 3D state, textures/samplers/render targets/blend/scissor/draw for that profile.
- Add offline compiler provenance and manifest-only runtime shader link.
- Implement `libneogl` and `surfman-neo`; render a WebRender scene offscreen to `GpuBufferCap`.
- Positive visual oracle + invalid shader/op/resource corpus + hang/reset controls.
- **Unblocks:** accelerated WebRender raster, initially CPU-presented.
- Estimate: 7–12 person-months.

### GFX-6 — GPU composite/present without taking modeset ownership

- Let the trusted compositor target/import the current boot framebuffer through `ScanoutTargetCap` or perform a GPU blit into it.
- Add double-buffered shadow surfaces, damage, frame pacing and honest RenderDone/CompositeDone/Presented events.
- Preserve firmware/Limine mode; no EDID/pipe/watermark reprogramming yet.
- **Unblocks:** credible accelerated Atrium and Servo page paint on the existing mode.
- Estimate: 3–5 person-months.

### GFX-7 — full browser hardening / general graphics profiles

- Site battery with accelerated WebRender; memory pressure, resize, context loss, paint-service crash and reset loops.
- Decide separately whether WebGL uses a sandboxed runtime compiler/general GL profile or waits for the wgpu backend.
- Build a custom Neo `wgpu-hal` backend, then Vello GPU and Servo WebGPU, only after the GL-first gate is real.
- Estimate: 8–16 person-months beyond the WebRender critical path.

### GFX-8 — display ownership/modeset (only if required)

- EDID/GMBUS, plane/pipe/link/watermark state, vblank/page flip, suspend/resume and rollback.
- Separate blind-hardware dispatch; never bundled into first acceleration.
- The boot mode remains the recovery contract until this slice independently proves replacement.

### Honest critical path

- **Desktop first pixels:** GFX-0.
- **Desktop first hardware acceleration:** GFX-3; credible smooth presentation: GFX-6.
- **Limited browser reader:** GFX-0.
- **Servo accelerated page rendering:** GFX-1 through GFX-6.
- **WebGL/WebGPU-heavy modern-site classes:** GFX-7 and browser-specific gates.

The WebRender critical path is approximately **20–33 person-months** through GFX-6. With three specialists working across software compositor, kernel/Intel memory, and WebRender/GL integration, the optimistic calendar is 9–14 months after architecture ratification. This is a design estimate to be replaced by the GFX-1/GFX-5 closure inventories; it may extend the browser study's prior graphics allowance and must be fed back into the 1.0 date rather than hidden.

## 11. Kill-switch and fallback contract

Use two independent switches:

1. **Compositor switch:** off means the existing v1 framebuffer console path, bit-for-bit, as G1 already requires.
2. **Graphics acceleration switch (`gfx-intel`, default off):** off means the compositor and reader run entirely through `CpuRasterBackend`; no graphics queue is registered, no iGPU VT-d repoint occurs, and existing `accel-intel` compute behavior is unchanged.

Runtime quarantine is a third state, not a compile-time replacement:

```text
Disabled -> software only
Enabled(epoch N) -> accelerated queues allowed
Quarantined(reason, epoch N+1) -> all fences failed; software only until reboot/owner recovery
```

Proof obligations:

- feature-off binary excludes new graphics Intel code and preserves v1/G1 software fixtures;
- boot/runtime acceleration-off produces the same compositor protocol and framebuffer output for exact clear/copy/blit fixtures;
- forced GPU timeout flips to Quarantined within a bound, wakes every waiter, and the next software frame presents;
- GPU arena allocation failure refuses acceleration, never allocates unfenced memory;
- VT-d unavailable or the IGD domain incomplete refuses hardware graphics;
- a whole-GT reset does not silently claim framebuffer fallback if scanout failed—serial/block evidence must say `display_restore_failed` and governance must remain live.

CPU and GPU antialiasing need not be pixel-identical for arbitrary vector scenes; the equality promise is **functional path and authority equivalence**, with byte-exact fixtures for operations whose math is exact. Visual renderer differences get golden/tolerance policy, never a false bit-identical claim.

## 12. Ratifiable architecture points

1. **One `IntelGt`, two adapters.** No independent graphics and AI owners of the same MMIO/engines.
2. **WebRender-profile GL first.** Preserve current Servo/WebRender's `surfman`/GL seam; no private WebRender backend unless the closure spike falsifies the bounded profile.
3. **Manifest-only shaders first.** Offline, pinned Gen9 shader compilation for WebRender; arbitrary WebGL shaders are a later sandboxed profile.
4. **No raw renderer submit.** Only compositor/disposable paint services hold restricted queue caps; kernel chooses addresses and validates/builds hardware batches.
5. **Dedicated GPU DMA + VT-d domain.** Extend `dma_arena` to named/tagged graphics extents; preserve scanout through a tiny privileged mapping set; PPGTT separates contexts.
6. **Software is permanent.** It is bring-up, recovery and low-risk reader mode, not throwaway scaffolding.
7. **Modeset later.** Accelerate into/present onto the firmware-established mode before taking pipe/link/watermark ownership.
8. **wgpu/Vello later on the same substrate.** Keep the object model compatible; do not place it before the WebRender/GL release path.

## 13. Primary sources read directly

### LogansOS ground truth

- `kernel/src/drivers/accel/accel_intel.rs`
- `kernel/src/drivers/accel/{mod.rs,intel_gen9_4a.rs,intel_gen9_execlist.rs,intel_gen9_ppgtt.rs}`
- `docs/ADR-032-phase-6-intel-gen9-gpu-compute-driver.md`
- `Plans/evidence/phase6-build4a-iter{4,5,6,7,8}-*`
- `Plans/evidence/phase6-build4b{1,2}-*`
- `Plans/evidence/phase7-build4X{1,2,3}-*`
- `docs/specs/neoplutarch-ai-subsystem-spec.md` §§9.7–9.8
- `kernel/src/drivers/{dma_arena.rs,dma_arena_logic.rs}` and `kernel/src/drivers/block/dma.rs`
- `kernel/src/drivers/iommu/{mod.rs,vtd_logic.rs,vtd_regs.rs}`; `docs/ADR-050-phase-7.5-vtd-iommu.md`
- `kernel/src/drivers/fb/{mod.rs,console.rs}`
- `docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md`
- `docs/complexity-governance.md` I-14
- `docs/concurrency-pitfalls.md` CP-1…CP-17 (normative)
- `Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md` (Janus's Servo dependency/GL audit)

### Intel / graphics prior art

- Intel Kaby Lake Open Source Graphics PRM landing page and Volumes 2a–d, 5, 6, 7, 11, 12, 14, 16: https://www.intel.com/content/www/us/en/docs/graphics-for-linux/developer-reference/1-0/kaby-lake.html
- Intel Kaby Lake Volume 5, Memory Views: https://www.x.org/docs/intel/KBL/intel-gfx-prm-osrc-kbl-vol05-memory_views.pdf
- Linux i915 driver documentation—contexts, PPGTT, requests, seqnos, lifetime and execlists (principle only; GPL code never copied): https://docs.kernel.org/5.17/gpu/i915.html
- Linux DRM/i915 uAPI—contexts, VMs, engines, MBZ/extension discipline: https://docs.kernel.org/gpu/driver-uapi.html
- Linux DRM reset semantics—hang detection, reset epoch/context-loss propagation: https://docs.kernel.org/next/gpu/drm-uapi.html
- Linux i915 VM_BIND design—bind/unbind, TLB invalidation and fence ordering: https://docs.kernel.org/6.1/gpu/rfc/i915_vm_bind.html
- Mesa source-tree guide—`iris` for Gen8+ and the shader compiler strata (study/build-tool candidate, not kernel import): https://docs.mesa3d.org/sourcetree.html
- OSDev Intel HD Graphics—field warnings on stolen memory, GGTT-backed planes, EDID, pipe timing and watermarks (warning source, Intel PRM remains authority): https://wiki.osdev.org/Intel_HD_Graphics

### Servo / future route

- Servo audited tree and manifests at `9c77cd97063589ae64a2d8f5527ff192385f2dfb`: https://github.com/servo/servo/tree/9c77cd97063589ae64a2d8f5527ff192385f2dfb
- Servo graphics statement on WebRender's GPU-oriented page raster: https://servo.org/blog/2025/08/22/this-month-in-servo/
- wgpu—native backends and Naga/hal architecture: https://github.com/gfx-rs/wgpu
- Vello—wgpu compute requirements and CPU/GPU renderer posture: https://github.com/linebender/vello

## 14. Close note

This is one documentation artifact. Per dispatch, no code, QEMU or CI run exists. Documentation-only validation is `git diff --check`, exact one-file diff inspection, source/link checklist, and RETURN-to-artifact comparison; cargo/Docker/QEMU/CI are intentionally skipped with this note.
