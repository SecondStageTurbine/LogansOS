<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gaming facet 2 — the graphics mountain (M1) — FINDINGS

**Date:** 2026-07-21
**Author:** Cerebro (Claude family; OT-63 GPU owner — the GPU/Vulkan seat on the gaming panel)
**Dispatch:** `Plans/DISPATCH-2026-07-21-gaming-facet-graphics-cerebro.md`
**Framing:** `docs/THINK-2026-07-21-gaming-on-logansos-framing.md` (PM). This FINDINGS pressure-tests the framing's M1 and **diverges from its §5 "OT-63 is step-0 toward games" through-line** — see §5 below.
**Status:** Deep design research, NO-GATE. Feeds the PM gaming synthesis (with Prometheus/landscape + Jarvis/substrate). Confidence-tagged; **[C-H]** high, **[C-M]** medium, **[C-L]** low/inference.

---

## 0. Executive recommendation

**The load-bearing reframe: the pivot is NOT "adopt Mesa vs write Vulkan from scratch." Both of those put a GPU driver *in our kernel*. The real decision is BUILD-the-driver vs DELEGATE-the-driver, and the winning move is to delegate it entirely — out of our TCB — into a VT-d-contained Linux gaming compartment.** [C-H]

Three facts force this:

1. **The driver you must acquire is a *complete, performant* Vulkan 1.3 driver (DXVK master pushes 1.4), not a subset.** DXVK + VKD3D-Proton require full Vulkan 1.3 core + `robustness2` + `transform_feedback` + **1,000,000-descriptor `UpdateAfterBind` bindless** + `push_descriptor` + timeline semaphores; miss any and games *do not launch*. And the hard 80% (pipeline caching / async compile to kill shader stutter, correct memory types incl. ReBAR, barrier correctness at scale, multi-queue async, submission throughput) is exactly what a "passes-CTS" driver still lacks. This is the RADV effort — a decade of many engineers — not a bounded build. [C-H]
2. **Running *stock RADV* natively = porting the Linux `amdgpu` kernel driver, forever.** RADV's kernel surface looks small (~20 `amdgpu` ioctls + core-DRM syncobj/PRIME), but behind it is TTM (VRAM/GTT memory manager), GPUVM (per-process page tables), the PM4 CS validator, drm_sched, fence/IRQ, and GFX9 power/clock/firmware (PSP/SMU) bring-up. **Every precedent that runs stock Mesa+RADV on a non-Linux kernel — FreeBSD `drm-kmod`, NetBSD — *imports the actual amdgpu source on a LinuxKPI shim and tracks upstream every release*.** There is **no** precedent for a bounded ioctl-shim that replaces amdgpu, because the cost lives in the driver, not the ABI. [C-H]
3. **The one path that does NOT put a GPU driver in our kernel is delegation: GPU passthrough (VFIO/VT-d) to a confined Linux compartment that runs the *stock* amdgpu+Mesa+Proton+Steam stack against the hardware directly.** Our kernel provides only IOMMU-isolated device assignment + the cap-broker seam — and the isolation mechanism *is our existing I-14(c2) VT-d work, generalized*. The GPU-driver mountain moves wholesale out of our TCB, and we inherit RADV's Vulkan 1.4 GFX9 maturity for free. This is the framing's **Option C, made concrete as a passthrough compartment** — and it is *charter-strengthening*, not a charter breach. [C-H on the mechanism; C-M on the single-iGPU topology, §2]

**Recommendation, ranked:**

| Option | What it is | Verdict |
|---|---|---|
| **DELEGATE via GPU-passthrough compartment (C-passthrough)** | LogansOS = a bounded hypervisor + VT-d-isolating cap-broker; a confined Linux guest runs the whole stock gaming stack on the passed-through GPU | **RECOMMENDED.** The only path that keeps the multi-hundred-KLOC, forever-moving GPU driver OUT of our kernel while getting *thousands* of games. Reuses VT-d/I-14. [C-H] |
| **Native stock RADV via a Linux-DRM personality (a)** | Port `amdgpu` on a LinuxKPI shim into our kernel; run stock Mesa/RADV | **FALLBACK / not recommended as primary.** Proven (FreeBSD) but re-imports the Linux driver ABI + a perpetual upstream-tracking tax into the core — the exact clean-slate escape, inverted. Acceptable only if fully quarantined, at which point it is strictly worse than passthrough. [C-H] |
| **From-scratch Vulkan driver (b)** | Write our own RADV-equivalent + all of Vulkan 1.3 | **REJECT for gaming.** Strictly dominated: you rebuild what RADV+amdgpu give free on a mature GFX9 target AND owe the full Vulkan-1.3-performant surface. Multi-year megaproject with no near-term game. [C-H] |
| **Cloud/streaming (E) + native/portable (D)** | (framing's fast win / long tail) | Complementary, not M1's call — noted in §7. |

**The GPU to target:** the PL's **M75q AMD Vega 11 (Raven Ridge, GFX9/gfx902)** is the *right driver-maturity target* — a first-class, years-hardened RADV target, Vulkan 1.4 on Mesa 25.0, real Proton gaming today at ~720p-low. **But** it is the box's *only* GPU (boot/console), and single-iGPU passthrough is the hard, honest unknown (§2). The clean passthrough topology wants **two GPUs** (iGPU for the host, a discrete AMD RDNA2+ card for the guest) — which also buys RT hardware for AAA. Gen9 (our OT-63 compute GPU) is **not** a gaming target and does not transfer (§5).

**The de-risking spike (go/no-go before any commitment):** *not* "render a triangle on RADV" (known to work) — the only real unknown is **the seam**. The spike is **`vkcube` on the Vega through VT-d-isolated passthrough** (or a virtio-gpu/Venus split), proving (a) stock RADV runs unmodified against the passed device, (b) our IOMMU contains its DMA, (c) the single-iGPU-passthrough problem is solvable on the target box — or surfacing that it isn't (§6).

---

## 1. The pivot, reframed: build vs delegate

The dispatch frames the choice as **adopt-Mesa (a) / from-scratch (b) / Linux-DRM-personality (c)**. Research collapses (a) and the *native* reading of (c) into the same project, eliminates (b), and surfaces a fourth option the framing gestured at (Option C) as the actual winner. Here is the honest cost of each, in dependency-depth / kernel-demand / charter-fit (no calendar, per the standing rule).

### 1.1 From-scratch Vulkan (b) — rejected

A from-scratch Vulkan driver for GFX9 is two megaprojects stacked: (i) the **hardware driver** RADV+amdgpu already is (ISA codegen — SPIR-V→NIR→ACO→GFX9 ISA — plus PM4 command building, memory/VA management, sync), and (ii) the **full Vulkan 1.3 surface DXVK/VKD3D demand** (§3). You would rebuild, worse, what MIT-licensed RADV gives free on a mature target. **Reject for gaming.** (Note: from-scratch *compute* for a *fixed* workload — the OT-63 glass blur — is a completely different, bounded thing; see §5. Do not conflate.) [C-H]

### 1.2 Native stock RADV via a Linux-DRM personality (a) — the "port amdgpu forever" tax

RADV is pure user-mode; its kernel dependency is the `amdgpu` uAPI on the **render node** (`/dev/dri/renderD*`), through libdrm_amdgpu. The surface *count* is deceptively small [C-H, raw uAPI headers]:
- **~20 `amdgpu` ioctls** (`GEM_CREATE/MMAP/VA/OP/USERPTR/WAIT_IDLE`, `CS`, `INFO`, `CTX`, `BO_LIST`, `WAIT_CS/FENCES`, `VM`, `FENCE_TO_HANDLE`, `SCHED`, `USERQ*`, …), each multiplexing sub-ops (**CS carries 10 chunk types**; **INFO has ~27 query codes**; GEM_VA has MAP/UNMAP/CLEAR/REPLACE).
- **+ core-DRM** `VERSION`/`GET_CAP`/`GEM_CLOSE`/PRIME + **11 `SYNCOBJ_*`** (the Vulkan fence/timeline backbone).

But behind those ioctls is a **stateful hardware-management driver**: TTM (VRAM/GTT placement, eviction, migration, pinning), GPUVM (`amdgpu_vm.c` page tables), the CS validator (`amdgpu_cs.c` BO-list reservation + PM4 IB parse + sync wiring), drm_sched → HW rings, fence/IRQ, and GFX9 power/clock/firmware (PSP/SMU). **The cost is implementing all of that correctly for GFX9 — not parsing 20 ioctls.** [C-H]

The precedent is unambiguous and is the decisive pricing datum: **FreeBSD (`drm-kmod`) and NetBSD run stock Mesa+RADV by importing the *actual Linux amdgpu source* and compiling it against a LinuxKPI shim (workqueues, DMA, PCI, mm, sched, IRQ), then tracking upstream Linux every release.** If a bounded ioctl-shim were sufficient, FreeBSD — which had every incentive — would have written one; it didn't, because you can't. Fuchsia avoided the Linux driver only by defining its own **Magma** MSD and porting Mesa per GPU family (relocating, not deleting, the hardware-management work). gVisor's "thin" nvproxy just **forwards ioctls to a real host driver**. [C-H]

**So "adopt Mesa under a Linux-DRM personality," read as *native*, IS "port `amdgpu` into our kernel and track it forever."** What it demands of our kernel: a LinuxKPI-class shim, the render-node ABI, KMS (~12–15 atomic ioctls, §4) if we own the display, and TTM/GPUVM/CS/fence/PSP/SMU living in-tree. What it costs the charter: it **re-imports the Linux driver ABI + a perpetual upstream tax into the clean-slate core** — the precise thing the charter escaped. It *works* (FreeBSD proves it), but it is the heavy option, and once you quarantine it enough to be charter-safe you have essentially built a compartment anyway — at which point passthrough (§1.3) dominates it. **Fallback only.** [C-H]

### 1.3 DELEGATE via a GPU-passthrough compartment (C) — recommended

The framing's Option C ("a bounded gaming compartment… GPU-passthrough… near-native container, not a slow VM") is the answer, made concrete. **LogansOS becomes a bounded hypervisor + cap-broker; a confined Linux guest runs the entire *stock* stack (Linux kernel + amdgpu + Mesa/RADV + Proton/DXVK/VKD3D + Steam) directly against a GPU passed through under VT-d isolation.** The GPU driver never enters our kernel. [C-H on mechanism]

Why this is the charter-*strengthening* path, not a breach:
- **The driver mountain leaves our TCB.** We own zero lines of GPU driver. RADV's Vulkan 1.4 GFX9 maturity, its caching/bindless/barrier correctness — all free, all outside our trust boundary.
- **The isolation mechanism is *already ours*.** I-14(c2) is exactly "put a device in a VT-d domain whose second-level page tables map only pages it's allowed to touch." A passthrough domain is that, scaled to the guest's full GPU working set. The gaming compartment's GPU DMA is contained by the same fence we built for glass — the guest can scan neither governance/tensor memory nor sibling processes. **A cap-native OS gives games *more* isolation than Windows/Linux** (framing §3's provocation), delivered by the IOMMU we already have groundwork for.
- **It is a bounded island, kill-switched, opt-in.** No game, and no Linux personality, touches the cap-native core except through the narrow brokered seam (GPU-assign cap, input cap, audio cap, save-file cap, optional net cap). Article-1.7-preserving by construction.

What it demands of our kernel (the honest bill — a hypervisor, but a *bounded* one, unlike the *unbounded* driver): CPU virtualization (VMX/EPT or equivalent), **VFIO-style device assignment**, **per-guest VT-d/IOMMU domains + interrupt remapping** (extends our VT-d work), a virtio transport for the brokered devices, and the cap-broker seam. This is real, but it is *finite and well-understood* prior art (KVM/VFIO) to reason from — the opposite of the *infinite, forever-moving* amdgpu-tracking tax. **You trade an unbounded driver problem for a bounded hypervisor problem.** [C-H]

**The two delegation flavors (a real sub-decision for the panel/spike):**
- **C-passthrough (VFIO):** the guest's stock `amdgpu` drives the *real* passed-through GPU. Best performance (near-native), zero driver in our kernel. Cost: virtualization + the single-GPU-passthrough problem (§2). **Primary recommendation.**
- **C-paravirtual (virtio-gpu + Venus / drm native context):** the guest runs *thin* Venus (~11 vendor-agnostic virtio-gpu ioctls; Vulkan serialized SPIR-V-through) and a **host** runs real RADV. This is ~¼ the ioctl surface and dodges single-GPU-passthrough — **but the host must run real RADV, which for a bare-metal LogansOS reduces to option (a).** So C-paravirtual only helps if we're *already* willing to host RADV natively; it is not an independent escape from the driver. Its value to us is narrower: as the *guest↔host GPU seam design* if we ever do run a host driver, and as the cleaner spike vehicle (§6). [C-H]

**Bottom line of §1:** adopt-vs-from-scratch is the wrong axis. **Delegate the driver via a VT-d-contained passthrough compartment (C-passthrough); keep native-amdgpu-port (a) as a quarantined fallback; reject from-scratch (b).** The pivot glass/browser/games "share" is *containment + present + cap-broker* primitives — **not** a shared driver (§5).

---

## 2. Which GPU to target — and the single-iGPU honesty

**Driver maturity says Vega 11.** The M75q's **Vega 11 (Raven Ridge, GFX9/gfx902)** is a first-class RADV target: Mesa supports "all GCN… from GCN 1," GFX9 predates RDNA with years of hardening, and current Mesa (25.0) exposes **Vulkan 1.4**, Khronos-conformant. Real Proton/DXVK/VKD3D gaming works on it *today* on Linux. Feature-wise `gfx902` == discrete Vega (`gfx900`); the APU's unified memory (no dedicated VRAM, GTT carveout) is a *performance* characteristic RADV handles, not a support gap. **The ceiling is raw iGPU power** (704 SP, shared bandwidth) → ~720p / low-settings / indie + older + many current titles, **not** AAA-ultra. Known hard gap: **GFX9 has no ray-tracing hardware** — RADV exposes DXR only via slow software emulation (`radv_emulate_rt`), so RT titles are out on Vega. [C-H]

**Topology says: mind the "only GPU" trap.** [C-M — the decisive practical caveat] The Vega 11 is the M75q's *sole* GPU: the host boots on it, consoles on it, and (today) OT-63 glass would composite on it. **GPU passthrough hands a device to the guest exclusively** — so passing the Vega to the gaming compartment means the host loses its display. Single-GPU passthrough on Linux is notoriously fiddly (device reset quirks, host goes headless, iGPU-vs-dGPU reset differences); on an iGPU it is harder still. This is the **#1 spike risk** (§6). Three ways out, honestly priced:
- **Two GPUs (recommended for a clean proof):** host on the iGPU, pass a **discrete AMD RDNA2/3 card** to the guest. Clean isolation, *and* the discrete card buys RT hardware + real AAA headroom. **But** the M75q is a ThinkCentre *Tiny* — no full-length PCIe slot — so this needs a *different box* (a small-form-factor desktop with a PCIe x16 slot, or an eGPU over Thunderbolt/OCuLink). **Hardware recommendation: for the gaming proof, target a box that can host iGPU-for-host + discrete-AMD-for-guest.**
- **Single-iGPU passthrough (accept jank):** host releases the Vega to the guest during "game mode," runs headless (serial/remote) meanwhile. Possible, ugly, and the exact thing the spike must prove or refute on the M75q.
- **Time-share via C-paravirtual** (host keeps the GPU, guest gets virtio-gpu/Venus): dodges exclusive assignment but needs host RADV (= option a). Not an independent win (§1.3).

**Discrete AMD (RDNA2/RDNA3) is the better ultimate target** for a real gaming machine — RT hardware, far more perf, still first-class RADV, and it makes passthrough clean. The Vega 11 is the right *de-risking* target (mature driver, PL owns it) with the topology asterisk; a discrete AMD card is the right *product* target. **NVIDIA:** RADV/Mesa's NVK exists but is younger; NVIDIA's own driver is proprietary and passthrough-hostile (historically Code-43-gated in VMs). **Prefer AMD** for the open-driver + passthrough-friendliness the whole thesis leans on. [C-M]

---

## 3. What DXVK / VKD3D-Proton actually demand — and why delegation gets it for free

This section is the reason from-scratch and "minimal Vulkan" are non-starters, and the reason delegation is so attractive: **the requirement is a *complete, performant* Vulkan 1.3 driver, and RADV already is one.** [C-H, primary sources: DXVK + VKD3D-Proton repos/wikis]

**The framing fact:** so many once-optional extensions were promoted into Vulkan 1.2/1.3 *core* (`buffer_device_address`, timeline semaphores, `dynamic_rendering`, `extended_dynamic_state1/2`, `descriptor_indexing`, `custom_border_color`, `vertex_attribute_divisor`, `4444_formats`) that requiring a 1.3 driver silently makes them all mandatory. **You cannot cherry-pick a subset; the floor is "implement essentially all of Vulkan 1.3 correctly."**

- **DXVK (D3D 8/9/10/11 → VK):** current stable **2.7 requires Vulkan 1.3** (`VK_KHR_maintenance5` now mandatory; master pushes **1.4** + `maintenance6`). Hard-required: **`VK_EXT_robustness2`** (games *abort* without it), **`VK_EXT_transform_feedback`** (D3D10/11 stream-out; D3D9), `depth_clip_enable`, `load_store_op_none`, ≥256B push constants, descriptor indexing, 8/16/64-bit ints, `scalarBlockLayout`. zlib-licensed → adoptable/linkable.
- **VKD3D-Proton (D3D12 → VK):** **Vulkan 1.3 required.** The defining requirement: **descriptor indexing with ≥1,000,000 `UpdateAfterBind` descriptors** for all types except UBO ("essentially all of `VkPhysicalDeviceDescriptorIndexingFeatures`") — D3D12's bindless heap maps onto this; a driver that caps `maxUpdateAfterBind` low simply won't run D3D12. Plus `robustness2`, `push_descriptor`, `samplerMirrorClampToEdge`, `shaderDrawParameters`, timeline semaphores (the D3D12 fence model maps directly). Perf-critical (not startup-blocking): `mutable_descriptor_type` / `descriptor_buffer` for cheap bindless. Title-gated: `VK_KHR_ray_tracing_pipeline`+`acceleration_structure` (DXR — needs RT hardware, so Vega is out), `mesh_shader`. LGPL-2.1 → adopt only as an isolated dynamically-linked module (§7).
- **The 80% CTS doesn't test:** pipeline caching + async/library compile (shader-comp stutter is Proton's #1 felt problem), correct memory types incl. a **DEVICE_LOCAL|HOST_VISIBLE (ReBAR)** heap for uploads, barrier/sync2 correctness at scale, multi-queue async compute, and **high submission throughput / low per-submit CPU cost**. A conformant-but-naive driver runs the CTS and hitches through every game. [C-H]

**The implication is the whole argument:** meeting this bar is precisely "be RADV." Delegating to stock RADV gets *all* of it — the 1M-descriptor bindless, the caching, the ReBAR paths, the barrier correctness — for free and outside our TCB. Building it (native port or from-scratch) means owning it. This is why M1's answer is *delegate*.

---

## 4. The presentation / frame path — Gamescope seam + I-14 at gaming volume

**Adopt the Steam Deck's present model: a Gamescope-style micro-compositor owns the game's swapchain; do not make every game's native swapchain correct against our display engine.** [C-H, primary: Gamescope/Deck sources]

- **Gamescope** installs a **Vulkan WSI layer** (`VK_LAYER_FROG_gamescope_wsi`) that intercepts the game's `vkCreateSwapchain`. The game thinks it presents to a normal surface; really it hands frames to Gamescope, which **forces FIFO**, **owns the pacing** (it decides when to flip, decoupling game render-rate from scanout), and does **integer/FSR upscaling, overlays (Steam/FPS HUD), rotation, HDR, VRR/adaptive-sync, per-game fps-limit** in one composite pass, compositing **directly to DRM/KMS** in embedded mode. BSD-2 → adoptable. [C-H]
- **Why this fits us:** it *centralizes* the entire messy present/VRR/frame-pacing/overlay problem into **one** controlled component and hands games a trivial forced-FIFO surface — a far smaller, more auditable seam than making N games' native swapchains correct against our KMS. In the passthrough compartment, **Gamescope runs *inside* the guest** against the passed-through GPU; the finished frames reach the user either (i) directly, when the compartment owns the physical display during "game mode" (cleanest, the Deck model), or (ii) as a **shared dma-buf handed back to our G1 compositor** for a windowed/overlay game-mode (the compartment scans out to an offscreen surface; G1 imports it). Option (i) is the near-term; option (ii) is the "game-in-a-window on the cap-native desktop" future and defines the **Gamescope↔G1 seam** the panel should design.
- **KMS scope, if the host ever owns display for the compartment:** the atomic-KMS surface a compositor needs is *bounded* (~12–15 ioctls: GETRESOURCES/CONNECTOR/CRTC/PLANE, ADDFB2, `ATOMIC`, + PRIME) — unlike the driver, KMS is enumerable. Buffer sharing is **dma-buf/PRIME** (render on `renderD128` → export fd → import on `card0` → ADDFB2 → atomic flip, zero-copy). GBM allocates scanout-capable buffers (delegating to the Mesa backend). [C-H]
- **I-14 / DMA containment at gaming volume — the clean transfer of our own work:** a full GPU driver moves *enormous* DMA (textures, vertex/index buffers, command streams, ReBAR uploads) at high frequency — vastly more than the glass blur. In the passthrough model this is **not new invariant work; it is exactly I-14(c2) at scale**: the guest's GPU lives in a VT-d domain whose second-level tables map only guest-owned physical pages + interrupt remapping. The IOMMU hardware-contains the device *itself* — the same fence, sized to the guest's working set. Our "graphics VT-d domain covering *all* GPU-reachable allocations" (the OT-63 Slice-5 obligation, ADR-077 v2 amendment 5) is the *direct precursor primitive*; gaming is its maximal instance. The connectivity-arena→named-graphics-arena generalization the compute path needs is the same generalization a passthrough domain needs. **This is the one place OT-63 genuinely transfers — the containment, not the driver.** [C-H]

---

## 5. The honest OT-63 transfer assessment — a divergence from the framing

**Divergence flagged [C-H]:** the framing §5 states *"the GPU graphics API we're building is literally step 0 toward games"* and treats glass/browser/games as "the same tall pole: a real GPU driver." **From the GPU seat, that is half-right and half-misleading, and the misleading half matters for sequencing.**

What genuinely transfers from OT-63 to gaming:
- **Containment primitives (strong):** VT-d/IOMMU domains + `dma_arena`/`FencedDmaAddr` → the passthrough isolation domain (§4). This is real, load-bearing reuse.
- **The cap-broker pattern (strong-conceptual):** `GpuSubmitCap` / `authority_allows` / the kill-switch discipline → the gaming-compartment cap-bundle (GPU-assign / input / audio / save / net brokers). The *shape* transfers; the objects differ.
- **Hardware bring-up discipline (moderate):** PCI enumeration, MMIO mapping, the blind-hardware/discriminator method, the present-fence *concept*.
- **The present/compositor seam (moderate):** the G1 present-epoch fence + `present_backend` → the Gamescope↔G1 seam design.

What does **NOT** transfer — and this is the load-bearing correction:
- **Zero driver code.** OT-63 is **Intel Gen9 (GFX9-unrelated)** — execlists, PPGTT, the EU ISA, the ocloc/IGC toolchain, the compute-submit device. Gaming is **AMD GCN5 (gfx902)**, a *different ISA, different command format (PM4), different memory model, different everything*. The Gen9 driver we're building is not a subset, superset, or step toward an AMD Vulkan driver. **None of the driver transfers.** [C-H]
- **A compute-submit device is not a Vulkan driver.** OT-63's endpoint is a *fixed, pinned, manifest-only* compute pipeline (one blur kernel) — the deliberate opposite of Vulkan's *arbitrary user shaders + full 3D pipeline + swapchain + descriptor model at 1M-bindless scale* (§3). Extending Gen9-compute to Gen9-graphics (Janus's GFX ladder, Prometheus's FINDINGS) is a legitimate path **for glass and browser-on-our-own-driver** — but it is *not* the path to games, because games need the AMD stack and (per §1) that stack is best **delegated, not built**.
- **Therefore glass/browser and games diverge at the driver.** Glass's answer = *our own* minimal Gen9 (or later AMD) compute/graphics driver. Games' answer = *delegate to stock RADV in a compartment*. They **share containment + present + cap-broker**, they **do not share a driver**, and **OT-63 is not on the critical path to games.** The two run as parallel tracks that reuse primitives, not as one pole where finishing OT-63 advances gaming. Treating gaming as "the maximal extension of the GPU program we're on" over-credits the transfer and risks sequencing gaming *behind* a Gen9 graphics driver it doesn't need. [C-H — the key divergence for the PM synthesis to weigh]

**Bounded honesty on the other side:** the framing's *deeper* point survives — a real GPU capability is rung-0 of every local path — but the *acquisition strategy* differs by facet: build-our-own for glass/browser (bounded, fixed workloads), **delegate for games (unbounded, arbitrary AAA)**. The unifying asset is **the IOMMU containment + cap-broker**, which both need and which OT-63 is genuinely building.

---

## 6. The de-risking spike — the smallest go/no-go

**Do not spike "a triangle on RADV" — RADV-on-Vega rendering games is *known* to work on Linux. The only unproven thing is the SEAM to our OS.** The spike must attack the seam and the single-iGPU-passthrough risk, in ascending order of what they prove. [C-H on framing; C-M on M75q specifics]

**Spike 0 (free, hours — establish the baseline, no LogansOS):** on a stock Linux install on the M75q, confirm `vulkaninfo` reports Vulkan 1.4 on RADV/Vega 11, and `vkcube` + one real Proton/DXVK game runs. Establishes the target-GPU maturity claim on *this exact hardware* and the perf ceiling (fps at 720p-low). Pure baseline; zero new code.

**Spike 1 (the load-bearing pivot proof — the go/no-go): `vkcube` on the Vega through VT-d-isolated passthrough.** Stand up a minimal VM (start with KVM/QEMU as the *reference* hypervisor to prove the hardware path, before asking what LogansOS must provide) that **VFIO-passes the Vega 11** to a confined Linux guest running **stock amdgpu + RADV**, and render `vkcube`. Prove, concretely:
- (a) **stock RADV runs unmodified** against the passed device (no driver in the host) — the delegation thesis;
- (b) **the guest's GPU DMA is IOMMU-contained** — verify a deliberate out-of-domain access faults (the I-14(c2)-at-scale claim);
- (c) **single-iGPU passthrough is solvable on the M75q** — reset behavior, host-goes-headless recovery — **or surface that it is not**, which is the pivotal negative result (→ recommend the two-GPU box, §2).
- **Go/no-go:** if `vkcube` renders through contained passthrough, the *entire* Linux gaming stack (Proton/DXVK/VKD3D/Steam) is "just software on top of a working GPU," and M1 reduces to "build the bounded hypervisor + cap-broker seam" (Jarvis's substrate facet) — the driver mountain is *delegated and closed*. If iGPU passthrough is infeasible on the M75q, the result is equally valuable: it sets the hardware requirement (two-GPU box / discrete AMD) before any commitment.

**Spike 2 (the seam-cost measurement, after Spike 1 passes): what must *LogansOS* provide?** Enumerate, against Spike 1's working KVM/VFIO reference, the exact kernel surface LogansOS would need — VMX/EPT, VFIO device assignment, per-guest VT-d domains + interrupt remapping, virtio transports for the brokered input/audio/display — and confirm it is the *bounded* hypervisor bill of §1.3, not the *unbounded* driver bill. This priced surface is the input to the phased DAG.

**Optional Spike 1′ (the paravirtual alternative): `vkcube` via virtio-gpu + Venus.** Run the guest with *thin* Venus over virtio-gpu against a host RADV (on a Linux reference host). Proves the ~11-ioctl guest seam and the cleaner design — but remember (§1.3) it presupposes a host driver, so for us it informs the *seam design*, not an independent escape. Run it only to compare seam complexity, not as the primary go/no-go.

**What the spike must NOT do:** re-implement any amdgpu/RADV code, or claim a KVM/QEMU pass is a LogansOS pass (it proves the *hardware path* is real; Spike 2 prices *our* version). Label the split honestly, exactly as the OT-63 QEMU-vs-silicon split is labeled.

---

## 7. License landscape (principle only; MPL-2.0 core; counsel before shipping)

[C-H component licenses; C-M compatibility reasoning] The delegation model keeps the awkward licenses *outside* our codebase entirely (they run as stock binaries in the guest), which is itself a licensing advantage.

| Component | License | For a delegate-in-a-compartment model |
|---|---|---|
| **Mesa / RADV** | **MIT** | Runs as stock in the guest — zero obligation on us. (If we ever *ported* it — option a — MIT is fully adoptable/studyable.) |
| **DXVK** | **zlib** | Stock in the guest; permissive if ever linked. |
| **Gamescope** | **BSD-2** | Stock in the guest; adoptable if we ever want the compositor logic. |
| **VKD3D-Proton, Wine** | **LGPL-2.1** | Stock, dynamically-linked *inside the guest* — the LGPL boundary is entirely within the Linux compartment, never touching our MPL core. This is the cleanest possible isolation of the copyleft components. |
| **Linux kernel (guest), amdgpu** | **GPL-2.0** | **Runs as a stock guest kernel — never linked into or ported into our MPL kernel.** This is the load-bearing licensing win of delegation over the native-port (a): option (a) drags GPL amdgpu source into our tree (dual-license/FreeBSD-style headaches); passthrough keeps GPL entirely in the guest. |

Delegation's licensing story is strictly cleaner than a native port: the entire GPL/LGPL Linux gaming stack stays *stock, in the guest*, behind the compartment boundary. **No-GPL-lift is trivially satisfied — we lift nothing; we run it.** [C-H]

---

## 8. Confidence, divergences, sources

**Confidence summary:**
- The reframe (delegate vs build; from-scratch rejected; native-port = amdgpu-forever) — **[C-H]**, primary-sourced (raw uAPI headers, FreeBSD/NetBSD/Fuchsia/gVisor precedent, DXVK/VKD3D requirement docs).
- DXVK/VKD3D = full performant Vulkan 1.3/1.4 floor — **[C-H]**, project repos/wikis.
- Vega 11 = mature RADV target, Vulkan 1.4, ~720p-low, no RT hardware — **[C-H]** driver maturity; **[C-M]** per-device perf.
- Single-iGPU-passthrough is the pivotal hardware risk — **[C-M]**, the honest caveat the spike must resolve.
- Gamescope present model + KMS-is-bounded + dma-buf seam — **[C-H]**.
- I-14(c2) → passthrough domain transfer — **[C-H mechanism]**.

**Divergences from the framing (for the PM synthesis to weigh):**
1. **§5 "OT-63 is step-0 toward games" is over-credited.** Zero *driver* transfers (Intel Gen9 ≠ AMD GCN5); a compute-submit device ≠ a Vulkan driver. Games are best **delegated**, so OT-63 is **not on the gaming critical path** — glass/browser and games share containment + present + cap-broker, not a driver. Sequencing gaming *behind* a Gen9-graphics extension would be a mistake.
2. **The pivot axis in the framing (adopt-Mesa vs from-scratch vs Linux-DRM-personality) collapses:** the native reading of all three is "a driver in our kernel"; the real winner is the **fourth** option the framing gestured at (Option C) — **delegate via a VT-d-contained passthrough compartment.** The driver should leave our TCB, not be adopted into it.
3. **"Near-native container, not a slow VM" (framing Option C) is achievable specifically via GPU passthrough** — but with a real hardware asterisk (single-iGPU on the M75q). The recommendation adds the two-GPU / discrete-AMD topology the framing didn't price.

**Corroboration with the framing:** the phased de-risking posture, the "competitive-MP-kernel-AC is out by design" line, the cap-brokered-game-bundle provocation, and "a cap-native OS gives games *more* isolation" are all confirmed and *strengthened* by the passthrough model (the IOMMU is the mechanism that makes the provocation real).

**Primary sources** (via the two research sweeps feeding this facet):
- amdgpu uAPI: `amdgpu_drm.h`, `drm.h` (raw, torvalds/linux); libdrm_amdgpu `amdgpu.h`; Mesa `ac_linux_drm.c` / `radv_amdgpu_cs.c`.
- RADV/winsys/Venus: docs.mesa3d.org/drivers/{radv,venus}.html; `virtgpu_drm.h`; RADV null-winsys (`RADV_FORCE_FAMILY`).
- DRM/KMS/GBM/dma-buf: kernel.org drm-uapi/drm-kms/dma-buf docs; `gbm.h`; `drm_prime.c`.
- Precedent: freebsd/drm-kmod + LinuxKPI primer; NetBSD drm2; fuchsia.dev Magma; gVisor nvproxy.
- DXVK: github.com/doitsujin/dxvk (wiki Driver-support / Feature-support; 2.7 release notes; robustness2/transform_feedback issues).
- VKD3D-Proton: github.com/HansKristian-Work/vkd3d-proton (README "Drivers"); Khronos `VK_EXT_descriptor_buffer`.
- Gamescope/Deck: ValveSoftware/gamescope; Arch wiki Gamescope; gamescope-deep-dive.
- Licenses: each component's LICENSE (Mesa MIT, DXVK zlib, VKD3D-Proton/Wine LGPL-2.1, Gamescope BSD-2).
- Vega 11: docs.mesa3d.org/drivers/radv.html; Mesa 25.0 Vulkan-1.4; RADV `radv_emulate_rt`.

*This is one NO-GATE research artifact. It is itself cross-checked (cross-model panel + PM synthesis) before any commitment; the Spike-1 go/no-go gates any build.*
