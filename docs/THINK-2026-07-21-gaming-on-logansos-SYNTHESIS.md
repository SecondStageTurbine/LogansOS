<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK (synthesis): Gaming on LogansOS — the panel verdict + the phased path

**Date:** 2026-07-21
**Author:** PM (Claude), synthesizing the 4-facet cross-model panel over the framing.
**Inputs:** `docs/THINK-2026-07-21-gaming-on-logansos-framing.md` (framing) + the four FINDINGS: landscape (Prometheus/Qwen), graphics (Cerebro/Claude), substrate (Jarvis/Codex), novel-tech (Janus/Codex).
**Status:** SYNTHESIS — the recommended direction + the honest corrections + the phased de-risking DAG. A cross-model cross-check of *this recommendation* is the next step before any build commitment.

---

## 0. The verdict
**Gaming on LogansOS is real, charter-consistent, and far more tractable than the framing feared — because the tallest mountain (the GPU driver) does not get *built*, it gets *delegated*.** All four facets, across three model families, **independently converged** on the same architecture — the strong cross-model signal we ran the panel for:

> **A bounded, capability-confined "gaming compartment" — a Linux personality that runs the *stock* Linux gaming stack (amdgpu + RADV + Proton + Steam) unmodified, with the *GPU passed through via VT-d/IOMMU* — so the entire driver stack lives *outside* our TCB, contained by our capability + IOMMU boundary. Kernel anti-cheat is out by design; the reachable library is ~50,000+ titles.**

The single sharpest insight (Cerebro, the GPU seat): **the real axis is not "adopt Mesa vs write our own" — both put a GPU driver in our kernel. The axis is BUILD vs DELEGATE, and the answer is DELEGATE.** Passthrough moves the driver mountain out of our trust base entirely and turns M1 (a from-scratch Vulkan driver, the megaproject) into *"build a bounded hypervisor + the cap-broker seam"* — which is Jarvis's compartment. The mountain doesn't get climbed; it gets *routed around*, and the routing (IOMMU-contained device passthrough) **strengthens** the sovereignty charter rather than breaching it.

## 1. The convergence (why we trust this)
| Facet (family) | Independent conclusion |
|---|---|
| Landscape (Qwen) | Option C; host *real* Proton (Wine's 3M-LOC POSIX coupling is not separable → run it as a process, don't reimplement); Mesa needs DRM we lack; ~50K titles reachable; kernel-AC out. |
| Graphics (Claude) | BUILD-vs-DELEGATE → **DELEGATE** via VT-d GPU passthrough; driver leaves the TCB; stock RADV maturity free; the spike is the *seam*, not a triangle. |
| Substrate (Codex/Jarvis) | Option C via the ADR-023 Linux personality, cap-confined behind native GPU/G1/audio/input/net/storage brokers; adopt real Proton. |
| Novel-tech (Codex/Janus) | Killed WASM-as-shortcut-to-the-Steam-catalog and attestation-as-anti-cheat-replacement (confirming the compartment + kernel-AC-out); the AI-native bets are the *differentiator*. |

Four angles, one architecture. That is a genuinely strong basis to build on.

## 2. The honest corrections the panel forced (kill the framing's errors)
- **OT-63/glass is NOT "step-0 toward games."** *(Cerebro, divergence from framing §5 — accepted.)* **Zero GPU-driver transfer**: Intel Gen9 ≠ AMD GCN5, and a compute-submit device ≠ a Vulkan driver. Glass, browser, and games share **containment + present + the cap-broker seam** — *not a driver*. **Games are off the OT-63 critical path.** (What OT-63 *does* give gaming: the **VT-d/I-14(c) work** — the passthrough isolation — and the **G1 present seam**. That transfer is real; the "driver" transfer was my framing's error.)
- **Wine is not separable — host it, don't reimplement.** *(Prometheus.)* Option A (reimplement Win32 on libneo) is dead; Proton runs as a process inside the compartment. DXVK (zlib)/VKD3D (LGPL) *are* separable but you get them free inside the guest anyway.
- **WASM/WebGPU is not a shortcut to the existing catalog.** *(Janus, killed.)* It's a *new-platform* bet (Option D), not a way to run today's Steam library. Don't conflate them.
- **Attestation does not replace kernel anti-cheat for studios.** *(Janus, killed.)* The competitive-MP-kernel-AC set is walled — accept it, don't chase a cap-native substitute that AC vendors won't honor.

## 3. The architecture (the recommended shape)
A **gaming compartment** = a confined Linux personality (ADR-023) that:
- runs **stock amdgpu + RADV + Proton + Steam** unmodified (cleanest licensing — the GPL/LGPL stack stays in the guest, never touches the MPL core; process/DLL boundaries confirmed by the license sweep);
- gets the **GPU via VT-d/IOMMU passthrough** — the driver is in the guest, the guest's GPU DMA is IOMMU-contained (our I-14(c2) work *at scale*), so a compromised game/driver cannot reach the sovereign core;
- reaches the outside world **only through native cap-brokers** — a GPU-submit/passthrough cap, an audio-out cap, a controller-in cap, a save-file cap, a net cap (or none) — nothing ambient;
- presents through a **Gamescope-style swapchain-intercepting micro-compositor** (FIFO/pacing/VRR/upscale/overlay) that hands frames to **G1** over a defined seam.

**What LogansOS must actually build** (the honest scope): a **bounded hypervisor** (VT-d device passthrough, guest memory isolation) + the **cap-broker seam** (GPU/input/audio/net/storage) + the **Gamescope↔G1 present seam** + the Linux-personality plumbing. That is a **deep, multi-phase program** — but it is a *bounded* systems program (hypervisor + brokers + IOMMU-at-scale), **not** the unbounded from-scratch-GPU-driver megaproject the word "gaming" first implies. That's the difference between "impossible" and "hard-but-real."

## 4. The hardware crux (the pivotal risk + the ask)
**Passthrough needs the host to give the GPU to the guest — so a single-GPU box is the pivotal risk.** The M75q's **Vega 11 is the box's *only* GPU**; passing it to the guest leaves the host headless. Cerebro's spike is designed to prove single-iGPU passthrough is solvable *or* surface that it isn't. The **clean topology is two GPUs**: a modest iGPU/host GPU for LogansOS + a **discrete AMD (RDNA) card passed to the guest** — which *also* buys ray-tracing and AAA-class performance (Vega 11 is ~720p-low, no RT). **Recommendation: the gaming target is a two-GPU machine with a discrete AMD card** — the spike runs on the M75q Vega to de-risk the mechanism cheaply first.

## 5. The differentiator: AI-native gaming (why this could beat SteamOS, not just match it)
Janus's viable bets are the reason LogansOS gaming is worth doing *distinctively*, not just as a SteamOS clone:
- **A governed "game-semantic bus"** — Arthur (private, on-device, cap-bounded) sees *only* what a game explicitly brokers (state, a screen region), enabling an AI companion/coach/game-master and **real-time AI accessibility** (narration, adaptive input) as *OS services* no other platform can offer privately.
- **`FrameContractCap`** — an end-to-end, capability-typed frame path (game → compartment → G1) as a first-class contract.
- **A WASI-Component + WebGPU native game platform** — the charter-pure long game (Option D), made compelling by the AI-native hook: a "LogansOS game" target that's more secure + portable, with Arthur integration devs can't get elsewhere.
- **Killed (don't chase):** transparent scanout-only DLSS-class processing (fantasy at our GPU scale), attestation-as-AC-replacement, WASM-as-catalog-shortcut.

## 6. The reachable target set (the honest boundary)
**~50,000+ titles reachable** without kernel-AC: GOG DRM-free (~4K), Steam single-player (~35K), co-op with userspace AC (~5K), native Linux (~5K). **Permanently walled by design: ~3–8K competitive-MP-with-kernel-AC titles** (Vanguard/kernel-EAC — they demand the ring-0 visibility our thesis forbids; that's a coherent, defensible line, and the *right* one for a sovereign OS). The catalog LogansOS *can* play is the overwhelming majority.

## 7. The phased de-risking DAG (what to prove before committing)
1. **Phase 0 — the spike (go/no-go, cheap, on the M75q Vega):** `vkcube` on the Vega through **VT-d-isolated passthrough** — prove (a) stock RADV runs unmodified in a confined guest, (b) the guest's GPU DMA is IOMMU-contained (nothing reaches the core), (c) single-iGPU passthrough is solvable on the M75q — *or* surface that it needs two GPUs. **This is the pivot gate; everything downstream depends on it.**
2. **Phase 1 — the bounded hypervisor + cap-broker seam** (Jarvis's compartment): guest isolation + the GPU/input/audio/net/storage brokers + the Gamescope↔G1 present seam.
3. **Phase 2 — a real title:** one Proton (or native-Linux, or GOG DRM-free) game running end-to-end in the compartment on real hardware (ideally the two-GPU box).
4. **Phase 3 — the AI-native layer:** the game-semantic bus + `FrameContractCap` — the differentiator.
- **Near-term bridge (parallel, Option E):** **cloud/streaming** (GeForce Now / xCloud) rides the browser + GPU-decode work already in progress and sidesteps all five mountains — so *"you can game on LogansOS"* becomes **true early** while the compartment matures.

## 8. Bottom line + next step
- Gaming is **real and charter-consistent.** The panel dissolved the scary version (from-scratch GPU driver) by **delegating the GPU via IOMMU passthrough**, leaving a **bounded hypervisor + cap-broker** program that *strengthens* the sovereignty charter. The reachable library is huge; the walled set (kernel-AC competitive MP) is an honest, defensible exclusion. The **AI-native hook** makes it distinctive, not derivative.
- It is still a **deep, multi-phase, hardware-gated program** (two GPUs; the passthrough spike is the pivot) — not near-term, and *not* on the current Milestone-A/glass critical path (Cerebro's correction stands: OT-63 shares containment/present, not a driver, with gaming).
- **Recommended next step:** a **cross-model cross-check of THIS synthesis** (the recommendation is load-bearing and, notably, the GPU seat *diverged* from my framing — exactly where a cross-check earns its keep), plus resolving the one open data flag (Mesa's MIT license was not raw-verified — GitLab 504'd). Then, if ratified: **schedule Phase-0 (the passthrough spike)** as the cheap go/no-go — it's the single experiment that turns "we think" into "we know."

## 9. Cross-check verdict + amendments (Chad/Codex ← PM/Claude, `docs/RESEARCH-2026-07-21-gaming-synthesis-crosscheck-FINDINGS.md`) — RATIFY-AFTER-AMENDMENT
The synthesis is **ratified** (Option C / delegate-via-passthrough / two-GPU / kernel-AC-out all hold) with five load-bearing amendments — folded in here:
1. **"Fast and fluid" is CREDIBLE — but conditional (the PL's central question, answered).** Near-native performance holds *only* with: **direct GPU passthrough (no per-submit broker)** + **pinned vCPUs** + **shared-memory input/audio** + **Gamescope scanning out *directly* in game mode**. These are engineering choices, not fundamental overhead — the layers do not cost frames if built this way.
2. **The present-path fix (fluidity-critical).** A *mandatory* Gamescope→G1 composition hop would cost an extra refresh interval + break VRR/frame-pacing. **FIX: a revocable, exclusive fullscreen *display lease* that bypasses G1 composition in game mode** — the game owns scanout directly (SteamOS-native fluidity), and LogansOS can revoke the lease at any time (cap-native sovereignty). This replaces "Gamescope hands frames to G1" everywhere in §3.
3. **Pick ONE architecture — the passthrough-microVM (§3 clarified).** The synthesis conflated *passthrough-microVM* with *native-render-broker* (incompatible). The ratified model is the **passthrough-microVM**: the guest holds the GPU directly; the "brokers" are the microVM device model + the display lease + coarse input/audio/net/storage caps — **NOT** per-submit native render brokers (which would kill performance, per amendment 1).
4. **The library numbers are soft (§6 caveat).** The ~50K-reachable / ~3–8K-walled estimates are unsupported and double-count categories. The *shape* holds (huge reachable set; competitive-MP-kernel-AC walled by design); the specific counts need firming before they're quoted as fact.
5. **Mesa license (§1 flag resolved-with-caveat).** Chad raw-verified: **Mesa core/most code is MIT**, with **per-file/component license exceptions** — so a **per-component license audit of exactly what we ship** is required before adoption (the clean-licensing claim is broadly right, not blanket). Two-GPU is confirmed the honest product requirement (single-iGPU = bring-up only).

**Post-amendment status:** gaming is a **ratified, cross-model-verified direction** — real, charter-consistent, fast-and-fluid *if* the display-lease present path + zero-per-frame-broker discipline are honored. Not near-term; hardware-gated (two GPUs); off the current Milestone-A/glass critical path. **Phase-0 (the vkcube-through-VT-d-passthrough spike) is the ratified go/no-go** whenever the PL chooses to open the lane.
