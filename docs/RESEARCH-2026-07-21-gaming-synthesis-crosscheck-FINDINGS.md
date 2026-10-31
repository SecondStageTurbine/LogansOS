<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gaming synthesis cross-check — performance-anchored FINDINGS

**Date:** 2026-07-21

**Reviewer:** Chad (Codex family; independent cross-family review)

**Under review:** `THINK-2026-07-21-gaming-on-logansos-SYNTHESIS.md`, its framing, and all four facet reports

**Verdict:** **RATIFY-AFTER-AMENDMENT** — no-gate

## 0. Verdict — can it be fast and fluid?

**Yes, but only for one architecture that the synthesis does not yet state: a hardware-virtualized Linux guest with exclusive GPU passthrough, direct guest submission, pinned/reserved vCPUs, shared-memory input/audio devices, and a fullscreen game-mode display lease in which Gamescope scans out directly.** In that mode, “near-native” is credible. It is not yet earned by the proposed `Gamescope -> G1` path.

**The current synthesis quietly combines two incompatible facet recommendations.** Graphics recommends a Linux guest with exclusive PCI passthrough; substrate recommends an ADR-023 in-kernel personality with a host-native virtual render node, says G1 always owns display, and explicitly rejects raw passthrough. One delegates `amdgpu`; the other must build or import a host GPU driver and a validating render broker. Their TCB, hardware topology, present path, and performance risks are different. This must be resolved before Phase 0 is described as validating “the architecture.” **[High confidence]**

My performance disposition is:

| Path | Honest delta versus the same game/Proton/Mesa on bare-metal Linux | Fluidity verdict |
|---|---|---|
| Passed-through GPU, guest Gamescope directly scanning out, vCPU/IRQ isolation | Engineering target: **0–5% median FPS loss in GPU-bound titles; 0–10% in CPU-bound titles; p99 frame time <=10% worse; no designed extra present frame.** These are acceptance envelopes, not measured LogansOS results. | **Credibly near-native**, contingent on measurement. |
| Same, but ordinary oversubscribed host scheduling or emulated hot-path devices | Average FPS may remain close while p99/p999 frame time and input/audio tails regress. A universal percentage cannot be claimed. | **Not yet credible.** Fix with vCPU/IRQ pinning, no vCPU steal, huge/static guest memory, and bounded RT audio work. |
| Gamescope frame exported to G1 for composition | Unless an explicit-fence, zero-copy import reaches immediate/direct scanout with no second queue, budget **0–1 refresh interval** of added display latency: 6.9 ms at 144 Hz, 8.3 ms at 120 Hz, 16.7 ms at 60 Hz. Cross-GPU import may also require a blit/copy and VRR policy does not automatically cross the VM/compositor boundary. | **Fails the “fast and fluid” promise as presently specified.** Fix: fullscreen game mode bypasses G1 composition; Gamescope owns the leased connector/scanout. |

The numbers above are proposed gates, not predictions disguised as evidence. The synthesis should say **“designed for near-native; unproven until same-hardware frame-time and latency A/B”**, not simply “near-native.” Linux VFIO gives a device direct access subject to IOMMU mappings; that prior art validates the mechanism, not LogansOS’s unbuilt VMM, scheduler, or brokers ([Linux VFIO documentation](https://www.kernel.org/doc/html/v6.6/driver-api/vfio.html)). **[High confidence]**

## 1. Layer-by-layer pressure test

### 1.1 GPU passthrough

With full PCI assignment, the guest `amdgpu` driver must program BARs, queues, doorbells, and command submissions directly. The host handles lifecycle, IOMMU mappings, interrupt remapping, reset, and faults; it **must not trap or copy each submission**. IOMMU address translation and interrupt remapping are real costs, but normally small beside rendering. The larger risks are nested CPU page-table walks, VM exits on accidentally emulated MMIO/config operations, interrupt delivery, guest memory/NUMA placement, and vCPU steal.

The synthesis’s “GPU-submit/passthrough cap” is therefore misleading. For the recommended path it should be a **`GpuAssignmentCap` control-plane authority**, not a submit data-plane broker. Assignment, reset, revoke, and domain-map changes cross the host. Draws/submits do not. If LogansOS traps every doorbell or validates/copies command streams, this ceases to be the passthrough architecture whose near-native evidence is being invoked. **[High confidence]**

Required gates:

- record VM exits per frame and prove no steady-state exit per GPU submit;
- compare median, p95, p99, and p999 frame times, not only average FPS;
- test GPU-bound and CPU-bound titles, shader compilation, asset streaming, and reset/fault recovery;
- verify interrupt-remapped MSI/MSI-X and deliberate out-of-domain DMA faulting;
- pin the guest’s memory and CPU topology appropriately; do not benchmark an oversubscribed guest.

### 1.2 Cap-broker seam

There is no host GPU-submit broker on the fast path in full passthrough. Other brokers can remain control-plane plus shared-memory data planes:

- input: a timestamped SPSC/shared ring plus event notification, batched without polling stalls;
- audio: shared PCM rings, guest/host clock correlation, and bounded wakeups;
- storage/network: virtio-style queues; important to hitching and online play, but not a per-frame pixel path;
- presentation: avoid it in the primary fullscreen path; a future windowed path needs exported images and explicit acquire/release fences.

Each message-per-event broker can be correct and still feel bad if it schedules two processes or two kernels before delivery. Proposed additive gates are **p99 <=2 ms for focused controller delivery above a native client** and **broker processing itself <=0.25 ms p99** under GPU/storage load. USB polling remains separate: a 1 kHz device has a 1 ms interval; a 125 Hz device has an 8 ms interval before OS processing. **[Medium confidence on thresholds; high on measurement shape]**

### 1.3 Gamescope -> G1 presentation

This is the largest fluidity risk. Gamescope’s embedded/SteamOS role is a display compositor: its upstream project describes direct DRM/KMS operation and display features including adaptive sync; composition and direct scanout decisions live there ([Gamescope upstream](https://github.com/ValveSoftware/gamescope)). Passing its completed image to another independently paced compositor creates a second ownership and queue boundary. “dma-buf” alone does not solve pacing: zero-copy memory sharing, explicit synchronization, direct-scanout eligibility, VRR range/control, HDR metadata, cursor/overlay policy, and back-pressure all have to survive. With a guest discrete GPU and host iGPU, cross-device dma-buf import is not a universal zero-cost contract.

**Amendment:** in fullscreen game mode, G1 remains the policy/recovery authority but grants a revocable **connector/scanout lease**; guest Gamescope becomes the active display owner and scans out through the passed GPU. There is no Gamescope-to-G1 frame handoff. Secure attention revokes the lease and restores G1. A cable/monitor input topology capable of connecting the passed GPU is an explicit hardware requirement. Windowed “game inside Atrium” is a later, separately measured mode and must not inherit the near-native claim. **[High confidence]**

Acceptance is: no host full-frame copy, no second queued frame, fixed-refresh pacing first, then VRR with measured refresh cadence across the supported range. One million frames under load should show no unexplained pacing discontinuity attributable to the handoff.

### 1.4 Input and audio

Input brokering need not cost perceptible latency, but it is unimplemented and cannot be assumed free. Preserve the physical interrupt/poll timestamp, route only to the active game lease, write directly into a mapped guest ring, and wake the target vCPU without a general broker scheduling round-trip. Passing through a whole USB controller is the latency-reference configuration, but it weakens host focus/revocation integration; use it as an A/B oracle, not necessarily the product design.

Audio is feasible through a PipeWire/virtio-snd-shaped shared ring. At 48 kHz, 128 frames are 2.67 ms and 256 frames are 5.33 ms **per quantum**; total audible latency includes guest buffering, broker/mixer buffering, device DMA, and hardware. Gate both 128- and 256-frame modes, target <=20 ms measured output latency, and require zero xruns during a 30-minute mixed CPU/GPU/storage stress test. A broker that copies samples is less dangerous than a broker that adds another full quantum or misses its wakeup. **[Medium confidence]**

### 1.5 CPU and scheduling

Hardware virtualization does not guarantee stable frame times. The VMM must expose host CPU features, stable high-resolution clocks, topology, affinity, and efficient futex/ntsync behavior. Game vCPUs need reserved physical CPUs or a demonstrably equivalent no-steal policy; emulator/broker threads and GPU/audio interrupts need deliberate placement; audio needs bounded real-time priority. Shader-cache and page-fault activity must not run on the latency-critical game-loop CPU.

Gate `vCPU steal = 0` during the measured run, report host and guest scheduler latency, and test contention. An unloaded `vkcube` pass proves none of this. **[High confidence]**

## 2. Architecture claims

### 2.1 BUILD versus DELEGATE and the TCB

**Ratified, with narrower wording.** Full passthrough genuinely keeps `amdgpu`, DRM scheduler, RADV, Wine, Proton, and games out of the **sovereign host kernel TCB**. It does not make them trusted; it moves them into a disposable guest security domain. The host still trusts a substantial new VMM/device-assignment TCB: VMX/EPT, interrupt injection, IOMMU programming, PCI configuration mediation, firmware/reset handling, virtio endpoints, and guest-memory accounting. Minimize emulated devices and parsers.

The TCB win disappears under the substrate facet’s virtual-render-node architecture: a host driver plus command validation, GPU memory management, scheduling, faults, and reset return to host authority. That architecture may integrate with G1 more elegantly, but it is BUILD/import, not DELEGATE. The synthesis must select the passthrough microVM if it wants its TCB conclusion. **[High confidence]**

### 2.2 Two GPUs and single-iGPU passthrough

**Ratified as a product requirement.** A single GPU can be detached while the host runs headless and later reset/rebound, so it is not physically impossible. It is a poor default product contract: recovery is fragile, the native desktop disappears, reset support varies, and there is no simultaneous G1 path. A serial/remote/software console is useful for bring-up and recovery, not a fluid consumer experience.

The honest supported topology is host iGPU plus passed-through discrete AMD GPU, with the display physically reachable from the guest GPU. The M75q’s sole Vega iGPU is a mechanism spike and negative-test platform, not evidence for the final UX. **[High confidence]**

### 2.3 “~50K reachable” and “~3–8K kernel-AC-walled”

**Not ratified.** The facet’s arithmetic is not a catalog study. Its categories overlap (native Steam titles can also be in Steam single-player; GOG titles can also exist on Steam), “single-player” does not imply Proton compatibility, compatibility varies by build/launcher/codec/DRM, and “co-op with userspace AC” is not an enumerated dataset. Likewise, thousands of “competitive MP with kernel AC” titles are asserted without a title list or reproducible query. Several examples in the facet are factually unstable or internally mixed, which further weakens the roll-up.

Replace both counts with: **“a potentially large catalog, bounded per title by Proton, architecture, DRM, middleware, and anti-cheat; size unverified.”** Before publishing a number, pin a Steam/GOG snapshot, define deduplication and “reachable,” join title-level compatibility and anti-cheat evidence, and publish the query/data. Kernel-AC-required titles remain correctly unsupported by policy regardless of the count. **[High confidence]**

### 2.4 OT-63 is not step 0

**The correction is directionally right but slightly over-stated.** Zero Intel Gen9 compute-driver code transfers to AMD `amdgpu`/RADV, so games must not wait for OT-63’s driver ladder. However, shared IOMMU-domain machinery, DMA fault handling, explicit fences, capability assignment/revocation, real vblank/present telemetry, compositor recovery, and input focus are load-bearing prerequisites or reusable validation patterns. Gaming is off the **driver** critical path, not independent of the containment/present substrate. **[High confidence]**

## 3. Mesa license — raw verification

**Verified directly from upstream Git source at commit `61aaad5a1add1bcea822c9ad8334dd52cb950254` (2026-07-21 fetch).** `docs/license.rst` states that the core library and most Mesa code use MIT, but the distribution contains multiple components and individual files can have other licenses; it directs users to the per-file SPDX identifier and `licenses/` directory. `licenses/MIT` contains the MIT license text. Upstream identifies the canonical repository here: [Mesa source repository](https://docs.mesa3d.org/repository.html); the rendered upstream license matches the raw file: [Mesa license and copyright](https://docs.mesa3d.org/license.html).

Therefore **“Mesa is MIT” is acceptable shorthand only with a mixed-tree caveat.** Option C should record the exact Mesa/RADV build closure and scan per-file SPDX identifiers and linked dependencies. More importantly, passthrough runs the Linux/Mesa stack as guest software; “MIT Mesa” does not make the guest kernel permissive or remove GPL distribution obligations. License cleanliness and host TCB separation are distinct claims. **[High confidence; not legal advice]**

## 4. Required synthesis amendments

1. Select **microVM/Linux guest + exclusive full GPU passthrough** as the delegated architecture; stop combining it with ADR-023’s native render-node personality.
2. Replace `GpuSubmitCap` on the hot path with a control-plane `GpuAssignmentCap`; state that guest submits and GPU DMA do not traverse a host broker.
3. Make fullscreen game mode **bypass G1 composition** through a revocable connector/scanout lease. Treat Gamescope-to-G1 windowed presentation as later and performance-unproven.
4. State the two-GPU plus physical-display-path requirement for supported local gaming; retain single-iGPU passthrough only as a bring-up experiment.
5. Change “near-native” to a measured gate: same hardware/build, 0–5% GPU-bound median-FPS target, p99 frame time <=10% worse, no extra queued frame/copy, input p99 <=2 ms above native, audio <=20 ms with zero stress xruns. Failure names the layer and blocks the claim.
6. Delete the `~50K` and `~3–8K` counts until backed by a reproducible, deduplicated title dataset.
7. Qualify Mesa as mostly/core MIT with per-file/component licenses, and retain exact-build license closure.
8. Split Phase 0: Linux/KVM/VFIO establishes hardware/reset/baseline; a LogansOS VMM spike establishes LogansOS overhead and isolation. `vkcube` alone is not a gaming-performance gate.

With those amendments, the strategic direction is strong: delegate the moving GPU/Linux gaming stack to a disposable guest, keep the sovereign host small, and make the primary game path as short as the SteamOS path it seeks to match. Without them, “near-native” rests on passthrough evidence while the documented frame path adds an unpriced compositor boundary.
