<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gaming facet 4 -- novel technology, viability first

**Date:** 2026-07-21

**Author:** Janus (Codex; gaming panel facet 4)

**Status:** FINDINGS -- design research, no gate

**Scope:** technology that could make native gaming materially better on a
capability OS; not the Linux/Proton compartment architecture owned by Jarvis

**Method:** repository RAG discovery followed by direct reads of the Arthur,
AI-capability, GPU, compositor, framing, and other gaming-facet documents;
external claims were checked against primary project and standards sources.
The RAG was healthy (`ollama=true`, `qdrant=true`, 15,974 points) during the
pass.

## 0. Executive verdict

The opportunity is real, but it is not “put an LLM in a game” and it is not an
OS-wide DLSS clone. Incumbents already ship on-device game assistants and
OS-integrated neural super-resolution. LogansOS's defensible advantage is the
ability to make an AI/game relationship an **enforceable, revocable authority
graph** rather than an in-process SDK convention.

### The three bets I would make

1. **`GameObservationCap`: a typed, revocable game-semantic bus for Arthur and
   accessibility.** A game explicitly exports bounded observations such as
   objective state, player status, map hints, dialogue, and selected pixels or
   audio. Arthur receives only those observations, a governed inference budget,
   and explicitly separate output caps. It gets no ambient screen capture,
   microphone, network, save files, or synthetic input. This is a buildable
   extension of the capability and inference objects already present. The
   companion concept is not new; the structural privacy and revocation contract
   is. **VIABILITY VERDICT: viable-now for a native protocol spike; viable as a
   product after one game/engine opts into the semantic schema. [High]**

2. **Neo Game Components: WASI Component Model + WebGPU as a native game target,
   without a browser.** Use WebAssembly components as the portable code/package
   boundary, WIT resources as typed host interfaces, WASI's no-ambient-authority
   discipline, and WebGPU as the portable graphics contract. Map imports to
   native caps; use AOT-compiled Wasmtime artifacts on LogansOS. This is a
   plausible clean-slate console-like target, not a way to run today's Win32
   catalog. **VIABILITY VERDICT: viable-after graphics-class `wgpu-hal`, user
   threads, audio, and gamepad dependencies; the package/runtime spike is viable
   now on a reference host. [Medium-high]**

3. **`FrameContractCap`: admit and account one end-to-end latency envelope.** A
   game requests a period/deadline and bounded CPU, GPU, audio, and present
   budgets. The system admits or rejects the whole contract, timestamps each
   edge, and throttles an overrun without starving governance or the desktop.
   Deadline scheduling and GPU fences are prior art; the novel part is one
   revocable capability joining input age, CPU budget, GPU completion, audio,
   and presentation. It targets predictable tail latency and isolation, not
   magical extra FPS. **VIABILITY VERDICT: viable-after user threads, GPU queues
   with bounded reset/preemption, audio, and honest present events. A CPU/present
   simulator spike is viable now. [Medium]**

### The three ideas I would kill

1. **Kill transparent DLSS/FSR-class frame generation from the final scanout.**
   High-quality temporal upscaling and interpolation consume game motion vectors,
   depth, disocclusion/reactive information, frame history, and correct UI
   composition. The current LogansOS GPU path is fixed-kernel compute plus an
   early present path, not a graphics-class multi-frame inference device.
   Spatial scaling from pixels may eventually be useful, but it is not novel:
   Windows Auto SR is already OS-integrated and AI-based. **VIABILITY VERDICT:
   fantasy-because the transparent form lacks the inputs and suitable hardware;
   retain only an opt-in native `FrameIntelligenceCap` after the graphics stack.
   [High]**

2. **Kill cap-native attestation as a drop-in replacement for EAC/BattlEye.**
   Attestation can establish signed claims about a measured boot, bundle hashes,
   runtime, declared mods, and granted capabilities. It cannot establish the
   universal negative “no cheat influenced this match,” close all time-of-check
   gaps, or make today's vendors trust a new platform. Current Proton anti-cheat
   support itself requires studio enablement. **VIABILITY VERDICT:
   fantasy-because it cannot unlock the existing competitive catalog; retain a
   narrower native-game `IntegrityReceipt` after a hardware root of trust.
   [High]**

3. **Kill WASM/WebGPU or an off-the-shelf container as a shortcut to Steam.**
   WASM is a new binary target, not a Win32/POSIX compatibility layer, and
   WebGPU is an API, not a GPU driver. An embedder still supplies graphics,
   audio, input, networking, storage, scheduling, and platform integration.
   Godot's current web export documents real threading, audio, networking, and
   rendering constraints. **VIABILITY VERDICT: fantasy-because it does not run
   existing binaries or remove the device stack; valuable only as a new native
   target. [High]**

### Scorecard

| Idea | Genuinely new? | Buildable on our substrate? | Moves the needle? | Verdict |
|---|---|---|---|---|
| Cap-scoped semantic bus + Arthur | The assistant is not; enforceable observation/output authority is | Yes, incrementally | Yes: private coach, narration, mod help, shared engine seam | **Viable now as a spike** [High] |
| WASI Component + WebGPU native target | Standards are not; their 1:1 cap binding and AI seam are a new platform composition | Yes, after named device/runtime dependencies | Yes for a future indie/native catalog; zero help for current Win32 binaries | **Viable after dependencies** [Medium-high] |
| End-to-end `FrameContractCap` | Individual schedulers/fences are not; cross-device admitted authority is | Plausible, but needs real devices and telemetry | Potentially: lower p99 latency and better isolation | **Viable after dependencies** [Medium] |
| Opt-in frame-intelligence service | Multi-vendor SR APIs exist; governed shared service is a new composition | Only after graphics-class compute and explicit game inputs | Potentially, especially on shared NPU/accelerator hardware | **Viable after dependencies** [Medium] |
| AI accessibility | Features exist; privacy-preserving semantic access is differentiating | Semantic path yes; vision/audio path later | Yes, and useful beyond games | **Viable now/after dependency by mode** [High] |
| AI librarian/install/mod assistant | Not new | Mostly; store/install substrate remains | Useful but not platform-defining | **Viable after content manager** [High] |
| Native-game integrity receipt | Remote attestation is not; cap-graph disclosure is a useful claim | Not securely until measured boot + hardware key | Niche trust tier, not legacy anti-cheat | **Viable after hardware root** [Medium] |
| Predictive OS input rewriting | No useful novelty | Technically possible later | Usually harms semantics, fairness, and trust | **Fantasy as a general primitive** [High] |
| Local/cloud streaming | Not new | After video decode, audio, input, and network QoS | Real early access to games | **Viable after dependencies; not novel** [High] |

## 1. What is actually special about the substrate

The current tree gives these proposals unusually concrete foundations:

- `docs/specs/neoplutarch-ai-capability-semantics-spec.md` defines separate
  `Tensor`, `Model`, `InferenceContext`, `AccelTask`, and `LogitPort` capability
  surfaces, including attenuated inference-only and observer patterns.
- `docs/specs/neoplutarch-ai-subsystem-spec.md` already treats model residency,
  inference deadlines, accelerator admission/accounting, migration, eviction,
  and per-principal governance as OS concerns. The current implementation has
  real Arthur boot/inference/action, explicit grants, transitive revocation,
  and accelerator isolation proofs. It is not merely a chatbot mock-up.
- G1 already has explicit surface and focus authority. The GPU program has a
  proven Gen9 compute foundation and a proposed cap-native resource/queue/fence
  model, but it does **not** yet have general graphics, WebGPU, Vulkan, audio,
  gamepads, or a mature high-rate present path.
- The capability system makes absence of authority a falsifiable property. A
  companion can be proven unable to open a socket or inject input, instead of a
  privacy policy merely promising not to do so.

This does not make LogansOS the only OS capable of on-device game AI. NVIDIA ACE
already offers on-device characters, advisors, companions, and inference
scheduling alongside graphics. Windows Auto SR already demonstrates an
OS-integrated neural graphics service. The differentiator must therefore be:

> A game and an AI exchange typed, least-authority capabilities whose lineage,
> budgets, observation scope, outputs, and revocation are enforced below both
> parties.

That is narrower than the framing's exclusivity claim and more defensible.

**Divergence from the framing:** “the only OS with an on-device brain” should
not be used as a market or architecture premise. The feature category already
exists. LogansOS can instead be the first to make the AI/game boundary a
user-visible, revocable object-capability contract. **[High]**

## 2. Bet 1 -- the governed game-semantic bus

### 2.1 Contract

Define a versioned host interface, provisionally `neo:game/observation@1`, with
resources that remain opaque capabilities rather than serializable ambient
handles:

```text
GameSessionCap
  +-- GameObservationCap<schema, fields, rate, retention>
  +-- FrameRegionCap<surface, rectangle, rate, retention>       optional
  +-- GameAudioTapCap<bus, rate, retention>                     optional
  +-- GameKnowledgeCap<signed corpus/version>                   optional
  +-- ArthurInferenceCap<model class, tokens/s, memory, deadline>
  +-- CoachOverlayCap<surface, z-policy, rate>                   output
  +-- NarrationOutCap<voice, bus, rate>                         output
  +-- GameSuggestionCap<typed advisory messages>                output
  +-- GameActionCap<enumerated actions, confirmation policy>    rare/optional
```

The game publishes **semantics**, not arbitrary process memory: an objective
identifier, named entities currently visible to the player, tutorial state,
inventory summaries, accessibility labels, or a deliberately selected surface
region. Schema fields have rate and retention limits. Sensitive fields can be
derived as a narrower child cap. Revoking the observation root immediately
blinds every derived observer.

Outputs are separate. The default coach can render its own G1 overlay, speak to
an output-only audio bus, or send suggestions that the game may ignore. It
cannot synthesize keyboard/controller input. If an accessibility use case
requires actions, the game exposes a small enumerated action vocabulary and the
user grants that cap separately; it never becomes generic input injection.

This is also the right anti-cheat boundary. A competitive game's server can
classify a session by which observation/action caps are active. A visual-only
narrator and a strategy agent with hidden-state access are different, explicit
facts. The mechanism does not decide fairness policy; it makes the facts
available without installing a rootkit.

### 2.2 Why this beats screen scraping

Screen-only inference is universal but weak: it spends compute reconstructing
facts the game already knows, loses occluded/temporal semantics, misreads dense
HUDs, and tempts the OS into ambient capture. A typed observation gives better
accuracy, lower model cost, deterministic privacy scope, localization-ready
labels, and clean revocation.

Pixels and audio remain useful for legacy or artistic information, but should
be separately granted derived caps with visible capture indicators and short
retention. “Arthur sees the game” must never silently mean “Arthur sees the
whole desktop and hears the whole room.”

### 2.3 Product modes

| Mode | Minimum input | Output | Viability |
|---|---|---|---|
| Tutorial coach | Objective/player-state schema + signed game guide | Overlay or speech | **Viable now as a native demo** [High] |
| Private game master | Rich game-defined world/action schema | Suggestions or enumerated actions | **Viable after engine integration** [Medium-high] |
| Real-time narration | Accessibility labels/event stream; optional selected pixels | Audio out | **Viable after audio; semantic mode first** [High] |
| Adaptive controls | User performance signals + game-defined accessibility actions | Settings/action messages | **Viable after game opt-in; never covert** [Medium-high] |
| Screen-only legacy coach | Frame region + OCR/vision + guide corpus | Overlay/speech | **Viable after vision model and graphics capture, with lower reliability** [Medium] |

NVIDIA ACE proves that local advisors and companions are technically and
commercially plausible, including small models and inference co-scheduling.
Therefore “Arthur is a coach” is not itself novel. The cap-scoped observation,
retention, output, and revocation contract is the bet.

### 2.4 Smallest falsifiable spike

Build a tiny native turn-based game or deterministic game fixture on the host
reference runtime, then on LogansOS when convenient:

1. Publish a five-field observation schema and a signed help corpus.
2. Give Arthur an inference-only model cap and an overlay-only output cap.
3. Demonstrate a context-correct hint without screen capture.
4. Revoke the observation root mid-session; the next read must fail and no
   cached field may survive beyond the declared retention window.
5. Attempt filesystem, network, other-surface capture, and input injection;
   each must be denied independently.
6. Run a sibling game and prove that no observation crosses sessions.
7. Measure p50/p99 inference cost and prove an inference budget overrun cannot
   delay the game's frame contract or governance work.

**Go:** correct advice, bounded latency, immediate revocation, and all four
negative probes deny. **No-go/revise:** useful advice requires raw process
memory, ambient desktop capture, or generic input injection.

**VIABILITY VERDICT: viable-now for the protocol and turn-based coach spike;
viable-after audio/engine integration for the strongest accessibility and
game-master modes. [High]**

## 3. AI graphics -- retain the service seam, reject the magic version

### 3.1 Prior art changes the novelty claim

Microsoft describes Auto SR as OS-integrated AI super-resolution for existing
games. It coordinates NPU, CPU, GPU, and display and accepts roughly one frame
of added latency. DirectSR separately standardizes game-integrated access to
vendor upscalers. AMD's FidelityFX SDK shows why integrated temporal quality is
hard: frame interpolation consumes two back buffers plus depth, game motion
vectors, optical flow, and disocclusion inputs; its temporal upscalers depend on
game-provided history and metadata.

Consequently there are three different propositions:

| Proposition | Verdict |
|---|---|
| Universal spatial scaler operating on the final color frame | Buildable after a graphics compute path, but established prior art and limited quality. **Viable-after-GPU, not novel.** [High] |
| Transparent temporal SR/frame generation with DLSS/FSR-class quality from scanout alone | Missing information and compute; UI/latency/artifact failures are inevitable. **Fantasy-because the interface is underspecified.** [High] |
| Native `FrameIntelligenceCap` receiving explicit color/depth/motion/reactive/UI-plane resources and returning an image + fence | Technically sound service boundary; can share governed models and choose GPU/NPU/CPU. **Viable-after graphics-class GPU and engine integration.** [Medium-high] |

### 3.2 Proposed native service

```text
FrameIntelligenceProfile {
    spatial | temporal_sr | interpolation,
    input/output extent,
    max_history,
    latency_budget,
    quality_floor,
    allow_gpu | allow_npu | allow_cpu,
}

enhance(
    color: GpuImageCap,
    depth?: GpuImageCap,
    motion?: GpuImageCap,
    reactive?: GpuImageCap,
    ui_plane?: GpuImageCap,
    acquire: GpuFenceCap,
) -> (GpuImageCap, GpuFenceCap, EnhancementReceipt)
```

The service does not receive the game's general GPU context or memory. Imported
images are read-only unless explicitly output, history is bounded, and the
receipt reports the selected backend, model hash, actual latency, and fallback.
The compositor can decline an enhanced frame whose fence misses the present
deadline and present the real frame instead. Frame generation must never delay
input sampling or lie about the real simulation rate.

The governed-model substrate is useful here: one verified model can be shared,
memory pressure and accelerator time can be accounted, and a kill switch can
fall back to an ordinary frame. But current Gen9 compute is already the scarce
AI device, lacks a general graphics image/queue API, and would contend with
rendering. Model governance cannot create memory bandwidth or tensor hardware.

### 3.3 Smallest falsifiable spike

Do not start in the kernel. On a reference Vulkan/wgpu host:

1. Capture a legally redistributable sequence with color, depth, motion,
   reactive mask, UI plane, and ground-truth high-resolution frames.
2. Implement the proposed service boundary around one open temporal upscaler
   and one pixel-only spatial baseline.
3. Measure GPU time, memory, p99 completion, added input-to-photon latency, and
   objective/curated artifact cases (disocclusion, particles, HUD, rapid pan).
4. Remove each semantic input and quantify quality loss.
5. Saturate the accelerator with an Arthur request and prove admission rejects
   or degrades one client rather than missing both contracts.

**Go:** the typed service matches in-engine quality within an agreed bound and
meets a realistic frame budget on target-class hardware. **No-go:** cross-client
copies dominate, semantic inputs still cannot cross the engine boundary, or
shared AI/render work has no bounded admission policy.

**VIABILITY VERDICT: an opt-in service is viable-after a real graphics device,
explicit-sync image import, and target-class accelerator. Transparent
DLSS-class processing from scanout is fantasy. [High]**

## 4. AI accessibility and the librarian

### 4.1 Accessibility is the best first user value

The semantic bus should be justified first by accessibility, not spectacle:

- event and environment narration without cloud disclosure;
- developer-authored semantic labels augmented by a local model;
- dialogue simplification, reading assistance, and translation;
- user-controlled hint frequency and difficulty suggestions;
- remapping an accessibility device into **game-declared actions**, not generic
  key injection; and
- a machine-readable receipt of exactly what the assistant observed and did.

This is buildable in layers. A deterministic event narrator needs no large
vision model. A selected-region visual narrator needs capture, vision, and
audio. Adaptive difficulty must remain an advisory or game-owned action because
an OS silently changing game rules would be indistinguishable from cheating and
would undermine player agency.

**Smallest spike:** extend the coach fixture with semantic labels and three
accessibility profiles; prove usable narration without a frame capture cap, then
grant one region and demonstrate that revocation removes visual enrichment but
not semantic narration.

**VIABILITY VERDICT: semantic accessibility is viable-now as a protocol spike
and viable-after-audio as a product; screen/audio perception is viable after
those device/model dependencies. [High]**

### 4.2 Arthur as game librarian is useful, not a moat

Arthur can plausibly:

- explain a bundle manifest and requested capabilities in plain language;
- find compatible games from a local signed catalog;
- import a DRM-free installer into staging;
- propose storage cleanup, update, or rollback operations;
- explain a mod's declared file/network/process privileges;
- resolve load-order conflicts from structured metadata; and
- generate a plan whose mutating steps pass through existing confirmation,
  grant, transaction, and undo paths.

This is a good Arthur application because it is asynchronous, tolerant of CPU
inference, and naturally capability-bounded. It does not solve Steam, DRM,
Wine, graphics, or unstructured malicious mods. Hallucinated package names and
instructions make free-form shell execution unacceptable; every mutation must
lower to a typed package-manager operation over signed identifiers.

**Smallest spike:** give Arthur a three-game signed fixture catalog, one update,
one conflicting mod, and a storage quota. Require it to produce a typed install
plan; deny an undeclared network destination and an out-of-bundle write; undo
the applied plan byte-for-byte.

**VIABILITY VERDICT: viable-after a native content/package manager and signed
manifest schema; useful near-term, but not a platform-defining invention.
[High]**

## 5. Anti-cheat reframed -- a smaller integrity product survives

### 5.1 What attestation can truthfully say

The IETF RATS architecture separates an Attester that produces evidence, a
Verifier that appraises it against reference values and policy, and a Relying
Party that makes its own authorization decision. That is the correct model: an
attestation is a set of scoped claims, not proof of moral purity.

A future LogansOS `IntegrityReceipt` could bind a server nonce to:

- hardware-root/firmware/boot measurements;
- kernel, game-runtime, and attesting-environment identities;
- immutable game/component and asset hashes;
- the initial cap-bundle digest and security-relevant policy;
- declared mods, overlays, debuggers, Arthur observations/actions, and input
  transformations;
- GPU/compartment reset or integrity-fault counters; and
- receipt freshness and expiry.

The server or independent verifier chooses policy. A mod-friendly server can
accept a known mod set; a tournament can reject action-capable assistants while
allowing narration. Evidence should disclose the minimum claims needed because
RATS explicitly warns that evidence and results may contain identifying or
privacy-sensitive information.

### 5.2 What it cannot say

It cannot prove all of these negatives:

- no mutable code or data changed after the measurement;
- no legal observation was used by an external human or second machine;
- no malicious USB/peripheral generated plausible input;
- no display-camera or network-side assistance exists;
- no game logic vulnerability was exploited; or
- no privileged flaw let the target lie to an insufficiently isolated attester.

Measured boot reports a starting state. Microsoft's TPM documentation is
explicit that its measurement chain describes firmware and OS initialization,
not which applications are in use or how Windows is being used. Continuous
runtime claims need a protected attesting environment, event sources, and
freshness; even then they establish named facts, not the absence of every cheat.
Server-authoritative simulation, behavioral detection, replay review, and game
design remain necessary.

The current tree contains no complete TPM/measured-boot/secure-attester chain.
A software-key prototype can validate message formats, but it cannot validate
the security proposition because a compromised kernel could forge it.

### 5.3 Compatibility reality

Steam's official Proton guidance says supported anti-cheat middleware still
requires the developer to enable/configure support and ship the appropriate
library. A LogansOS receipt cannot compel an existing studio, league, EAC, or
BattlEye policy to trust it. Nor should LogansOS reproduce invasive client
inspection under a friendlier name.

The surviving product is a **native-game integrity tier**: privacy-minimized,
nonce-bound facts about the platform and declared authority graph. It could be
stronger than kernel anti-cheat on those named facts and much less invasive, but
it solves a different problem for games that deliberately adopt it.

### 5.4 De-risking ladder

1. **Format spike, not security proof:** define a canonical receipt, reference
   values, nonce/expiry, selective disclosure, and a hostile verifier test bank.
2. **Software attester negative proof:** demonstrate replay, stale-result,
   policy-confusion, and post-measurement mutation attacks; label the result
   forgeable by privileged code.
3. **Hardware-root spike:** after TPM/secure boot exists, seal an attestation key
   to the boot chain and include a native game's bundle/cap digest.
4. **Runtime challenge:** mutate a declared mod or grant a forbidden action cap;
   the next fresh receipt must change or fail.
5. **Studio discriminator:** ask one willing open-source multiplayer project to
   write server policy. If no developer values the claims enough to integrate,
   stop before building a broad service.

**VIABILITY VERDICT: viable-after a hardware root of trust for opt-in native
games; fantasy as a replacement for legacy kernel anti-cheat or as proof of “no
cheating.” [High]**

## 6. Bet 2 -- Neo Game Components

### 6.1 Why WASI fits, and what it does not provide

WASI already starts components with no ambient authority and exposes only host
grants. The Component Model provides typed, versioned interfaces and
language-neutral composition. That is a much closer semantic match to LogansOS
than POSIX or Win32.

Wasmtime makes the runtime path plausible rather than hypothetical: its
`no_std` subset includes runtime and Component Model support, with a custom
platform interface for virtual memory and basic OS operations. It does not
include Cranelift or Winch on `no_std`; modules must be AOT-compiled elsewhere.
That is acceptable for signed game packages and is an important constraint:
LogansOS should execute a verified AOT artifact tied to the original component,
compiler, target, and runtime version, not add a JIT to the initial TCB.

WebGPU supplies a validated, portable resource/pipeline/command model. It does
not supply a Gen9/AMD/NVIDIA implementation. The existing GPU design's proposed
`wgpu-hal` Neo backend, resource caps, queues, fences, WGSL validation, and
graphics DMA domain remain the hard device work.

### 6.2 Proposed package

```text
NeoGamePackage/
  manifest.cbor             signed identity, versions, requested capabilities
  game.component.wasm       portable canonical program
  game.x86_64.cwasm         signed AOT cache, reproducibly tied to canonical wasm
  assets/                   immutable content-addressed resources
  shaders/                  WGSL + validated/pinned translations/cache metadata
  interfaces/               WIT worlds and game observation schemas
  accessibility/            labels, action vocabulary, narration metadata
```

The initial WIT world should be deliberately smaller than a browser or POSIX:

```text
neo:game/input@1             focused, timestamped actions; no device enumeration
neo:game/audio@1             bounded output buses; capture is separate
neo:game/graphics@1          WebGPU surface/device profile + explicit present
neo:game/storage@1           immutable assets + save/cache resources
neo:game/network@1           absent by default; endpoints/services are grants
neo:game/time@1              monotonic/frame clocks, no ambient system inventory
neo:game/observation@1       optional Arthur/accessibility semantics
neo:game/integrity@1         package identity and declared session facts
```

There is no DOM, JavaScript shell, ambient home directory, raw HID, raw GPU,
global clipboard, process list, or unrestricted socket API. Browser security
headers and same-origin restrictions are not the runtime. Native host services
implement the WIT interfaces directly over capabilities and shared-memory data
planes.

### 6.3 Why it could be a platform, not another toy runtime

The standards reduce three adoption costs:

1. existing languages can target WASM components;
2. WebGPU/wgpu offers a portable graphics programming model; and
3. engines already have web/WASM experience, even though current exports expose
   limitations and may use WebGL rather than WebGPU.

LogansOS can add reasons to choose this target:

- one signed package runs on LogansOS and conforming desktop/web hosts;
- installation permissions are exact and comprehensible;
- deterministic save/mod boundaries and instant revocation;
- an optional, standard Arthur/accessibility interface;
- AOT startup with no browser, JS, ads, or web-origin machinery; and
- a fixed console-like performance profile with explicit latency contracts.

But the no-library problem remains serious. Studios will not port for security
purity alone. A credible offer needs an engine export path, excellent tools,
stable input/audio/graphics profiles, distribution, crash telemetry, and users.
The first target should be an open-source or owned indie title, not an appeal to
AAA studios.

### 6.4 De-risking ladder

1. **Host package spike:** run a WASI component with only time, one asset, and
   one save resource. Deny filesystem and network probes. Validate AOT/canonical
   component hash binding.
2. **Portable game spike:** package a small open-source game or purpose-built
   Pong-like title. Run the same component on reference Wasmtime and the custom
   LogansOS embedder, initially with software pixels and deterministic audio
   events.
3. **WebGPU triangle, then frame:** connect a `wgpu-hal` Neo device. Prove an
   invalid shader/command cannot escape the component's GPU resources and one
   component cannot address another's buffers.
4. **Latency and completeness:** controller, audio, save, network opt-in, and
   present under a `FrameContractCap`; compare native x86 and AOT-WASM CPU cost.
5. **Engine discriminator:** implement one upstreamable engine export template.
   If the port requires a large bespoke fork or misses the performance floor,
   stop calling this an ecosystem strategy.

**Go:** same signed package runs on two hosts, has near-native CPU behavior for
the chosen game, renders through a confined WebGPU device, and an engine can
export it without a permanent fork. **No-go:** runtime overhead is not the main
risk; a bespoke graphics/audio/engine fork that no upstream will carry is.

**VIABILITY VERDICT: viable-after graphics/audio/thread dependencies as a
native indie platform. It is not a compatibility path for existing Steam/GOG
binaries. [Medium-high]**

## 7. Leapfrogs and traps

### 7.1 What actually does work for us

| Candidate | What it buys | What remains | Verdict |
|---|---|---|---|
| WASI Component Model | Portable code/package boundary, typed imports, no ambient authority | Custom OS embedding, devices, engines, distribution | **Real partial leapfrog** [High] |
| Wasmtime `no_std` + AOT | Mature validator/runtime machinery without initial JIT | Custom platform ABI, artifact provenance, resource limits | **Real partial leapfrog** [High] |
| WebGPU/wgpu | Portable modern graphics API, WGSL validation, existing ecosystem | A real hardware backend, memory isolation, presentation, performance | **Real API leapfrog, not a driver leapfrog** [High] |
| Existing temporal SR SDKs/algorithms | Known inputs, quality baseline, service prototype | Hardware, licensing review, engine integration, cap wrapping | **Real algorithm leapfrog after GPU** [Medium-high] |
| Local/cloud streaming protocols | Removes local game/anti-cheat/driver requirement for remote play | Decode, network jitter, audio/input, service/account integration | **Real access leapfrog, not novel/local execution** [High] |

Streaming deserves a pragmatic lane even though it is outside the invention
thesis. A native client with video-decode, output audio, focused controller, and
attenuated service network caps can make gaming useful before local
compatibility closes. It should be measured by decode latency, jitter recovery,
and controller-to-server-to-photon time, not described as native gaming.

### 7.2 What does not do the hard part

- **WASM does not translate PE/Win32 or ELF/POSIX binaries.** Recompilation or a
  source-level engine export is required.
- **WebGPU does not provide the GPU driver.** Validation and portability end at
  the backend boundary.
- **A microVM/container does not provide the application ABI or devices.** It
  may package Linux, but GPU ownership, display, input, audio, storage, network,
  lifecycle, and trust seams remain. Jarvis prices that compartment; this facet
  does not redesign it.
- **A “Vulkan-portable” native target is less portable than the name suggests.**
  It still needs a full Vulkan driver and a native ABI/package story. WebGPU is
  the smaller, safer profile for a new platform; the legacy compartment can
  expose Vulkan where required.
- **Godot web export is evidence of source portability, not zero-cost porting.**
  Its current documentation notes WebGPU is not yet supported, Web builds use
  WebGL 2, threads have deployment constraints, and audio/network/gamepad
  behavior differs. A browser export cannot simply be renamed a Neo package.

**VIABILITY VERDICT: the standards/runtime pieces reduce the amount of new
technology, but none is a WiFi-FullMAC-style provider of the whole game device
stack. [High]**

## 8. Bet 3 -- cap-native performance primitives

### 8.1 The `FrameContractCap`

Today's systems already have multimedia priority classes, deadline schedulers,
GPU queues, semaphores, fences, presentation timing, and audio periods. Linux
`SCHED_DEADLINE`, for example, uses runtime/deadline/period with admission
control and bandwidth isolation. Vulkan exposes asynchronous queue submission
and explicit semaphore/fence dependencies. A new OS cannot honestly call those
ideas novel.

The clean-slate opportunity is to make the **whole frame pipeline one admitted,
revocable resource contract**:

```text
FrameContractRequest {
    period_ns,
    end_to_end_deadline_ns,
    cpu_runtime_ns,
    gpu_budget_ns,
    audio_runtime_ns,
    max_input_age_ns,
    memory_bytes,
    degradation_policy,
}

FrameToken {
    input_cutoff,
    cpu_deadline,
    gpu_deadline,
    audio_deadline,
    present_epoch,
    budget_remaining,
}
```

The broker admits a contract only when reservations and recovery bounds can
coexist with governance and display survival. Each frame token carries a common
clock and causality chain:

```text
physical input timestamp
  -> focused action dequeue
    -> simulation complete
      -> GPU queue submit / completion fence
        -> compositor accept
          -> presented epoch
```

Audio work for the same period carries its own deadline but shares the contract
budget and telemetry. The cap grants reservation authority, not higher global
priority. It can be attenuated, suspended, or revoked. Overrun policy is named:
drop visual quality, reuse the prior frame, reduce rate, fall back to software,
or terminate the guilty session. It may never borrow unbounded time or wait
under governance locks.

The gain is measurable: lower p99 input-to-present latency under competing
loads, fewer audio underruns, and bounded interference. Average FPS may not
increase and can fall slightly because admission reserves slack.

### 8.2 Smallest falsifiable spike

Before GPU/audio hardware exists, implement a deterministic host simulator or
QEMU scenario with:

- two synthetic game loops with different periods;
- timestamped input and an Atrium-style present event;
- CPU pressure and an adversarial budget overrun;
- a synthetic GPU fence and audio deadline; and
- governance/desktop sentinel tasks.

Compare ordinary priority scheduling to the admitted contract. Report p50,
p95, p99, and maximum input-to-present, missed audio periods, starvation, and
contract rejection. The negative oracle must prove an impossible request is
rejected rather than silently admitted.

**Go:** p99 improves under contention, overruns are contained to the guilty
contract, impossible sets reject, and sentinels remain live. **No-go:** the
cross-device abstraction merely adds IPC/timestamp overhead or cannot make a
guarantee because GPU reset/preemption remains unbounded.

**VIABILITY VERDICT: viable-after bounded device queues for hardware use;
viable-now as a scheduling/telemetry spike. [Medium]**

### 8.3 Sealed zero-copy queues are necessary, not magic

High-rate data paths should use mapped rings and typed resources:

- kernel/broker control plane creates a queue, buffer set, budget, and fence
  namespace;
- the client writes commands or samples into bounded shared memory;
- validation converts a sealed command bundle into device work;
- DMA mappings include only resources named by the queue's capabilities; and
- completion advances a generation-tagged fence without a per-draw broker IPC.

This is compatible with Jarvis's compartment data-plane conclusion and the
GPU resource design. It is not unprecedented—modern graphics and audio APIs
already use shared queues and explicit sync. The LogansOS value is the precise
authority, accounting, revocation, and DMA containment attached to each queue.

**VIABILITY VERDICT: viable-after the relevant device backends; required for
performance, but not a standalone novel bet. [High]**

### 8.4 Kill general predictive input

The OS should not predict and inject future game actions to hide latency. That
changes game semantics, creates false inputs, conflicts with competitive
integrity, and makes accessibility behavior unauditable. Legitimate narrower
uses are already recognizable:

- compositor cursor prediction or pose reprojection that does not alter game
  simulation;
- user-selected filtering/debounce for an accessibility device; and
- a game-defined action predictor whose outputs the game explicitly accepts.

All three require an opt-in action schema and receipt. None justifies a global
“AI input optimization” privilege.

**VIABILITY VERDICT: fantasy as a transparent general gaming primitive; viable
only as explicit display or accessibility transforms. [High]**

## 9. The long game -- define a platform, not a private universe

An AI-native/cap-native LogansOS game platform is viable only if “our own” means
**our contract over shared standards**, not a new language, shader ISA, engine,
asset format, and distribution universe.

### The platform promise

- package once as a signed WebAssembly component plus portable assets/WGSL;
- declare exact storage, network, input, audio, graphics, and AI authority;
- receive a fixed performance profile and honest frame telemetry;
- integrate private Arthur coaching/accessibility through one optional semantic
  API; and
- run without a browser, ambient POSIX account, kernel anti-cheat, or vendor AI
  SDK in the game process.

### What must be stable before courting developers

1. A versioned WIT game world and compatibility policy.
2. A working engine export path maintained close to upstream.
3. WebGPU-class graphics, audio, gamepad, storage, network, and debugging.
4. Reproducible AOT packages, shader caches, crash symbols, profiling, and
   deterministic capability prompts.
5. A software renderer/reference host so developers need not boot LogansOS for
   every iteration.
6. At least one excellent owned/open game proving performance and AI/accessibility
   value, not a collection of toy demos.

The platform should court small native/indie projects that value portability,
privacy, mod isolation, and accessibility. It should coexist with the legacy
gaming compartment, which supplies catalog breadth. Expecting the native
platform to replace Steam before it has users and tools is circular.

### Stop conditions

Stop or narrow the ecosystem claim if:

- no mainstream open engine can export without a permanent large fork;
- the WebGPU profile cannot expose required performance without unsafe/raw
  escape hatches;
- AOT-WASM CPU or threading behavior misses the selected game's budget;
- developers treat the Arthur interface as uncompensated platform-specific work;
  or
- the first three external projects do not retain the target after a prototype.

**VIABILITY VERDICT: viable-after the device/runtime foundation as a long-game
indie/native complement; fantasy as the near-term headline catalog. [Medium]**

## 10. Dependency and decision ladder

```text
NOW
  |
  +-- GameObservationCap schema + Arthur coach/accessibility fixture
  |      `-- go: standardize engine-neutral semantic vocabulary
  |
  +-- WASI Component/AOT host package fixture
  |      `-- go: implement minimal LogansOS custom platform
  |
  +-- FrameContract simulator + negative admission proofs
  |      `-- go: carry timestamps/contracts into device APIs
  |
  +-- IntegrityReceipt format/red-team only
         `-- wait: no security claim before hardware root

DEVICE FOUNDATION
  user threads + audio + gamepad + honest present + GPU resource/queue/fence
  |
  +-- software/native game component
  +-- semantic narration with audio
  +-- hardware FrameContract proof
  `-- WebGPU component triangle / confined cross-client negative

GRAPHICS-CLASS HARDWARE
  WebGPU backend + explicit-sync images + bounded reset/preemption + DMA domain
  |
  +-- opt-in FrameIntelligence service trace spike
  `-- first engine export + real indie title

HARDWARE ROOT / SECURE BOOT
  `-- native IntegrityReceipt trial with one willing server
```

No AI-graphics claim should pull the graphics device forward out of order. No
attestation claim should pull TPM work forward merely to imitate legacy
anti-cheat. The observation schema, AOT component package, and frame-contract
simulator are the cheap discriminators that deserve pursuit now.

## 11. Recommended synthesis position

For the panel synthesis, separate **catalog strategy** from **native advantage**:

- Jarvis's bounded compatibility work is how existing games may run; these
  findings do not replace or redesign it.
- Neo Game Components are how a future game can be *of* LogansOS rather than
  merely hosted by it.
- `GameObservationCap` is the first native differentiator because it is useful
  before a gaming-class GPU and simultaneously serves coaching, accessibility,
  privacy, and declared competitive policy.
- `FrameContractCap` is the plausible clean-slate performance advantage, but it
  must earn its keep with p99 measurements under contention.
- AI frame processing is downstream of graphics and explicit game inputs. It is
  a service profile, not the reason to start the GPU program.
- Attestation is an honest, narrow integrity receipt for opt-in native games,
  not an answer to the legacy anti-cheat wall.

The bold position is therefore not “invent every layer.” It is:

> Reuse WASI, WebAssembly Components, WebGPU, mature AI algorithms, and remote
> attestation vocabulary; invent only the capability contracts that bind game
> semantics, inference, frames, and integrity into enforceable user authority.

That combination is novel enough to matter and bounded enough to test.

## 12. Primary sources

### External

- [NVIDIA ACE for Games](https://developer.nvidia.com/ace) -- on-device game
  characters/advisors and inference scheduling alongside graphics.
- [Microsoft Automatic Super Resolution](https://devblogs.microsoft.com/directx/autosr/)
  -- OS-integrated AI SR, hardware coordination, and latency tradeoff.
- [Microsoft DirectSR](https://devblogs.microsoft.com/directx/directsr-preview/)
  -- common game-integrated API for vendor super-resolution implementations.
- [AMD FidelityFX SDK](https://github.com/GPUOpen-LibrariesAndSDKs/FidelityFX-SDK)
  and [frame interpolation manual](https://gpuopen.com/manuals/fidelityfx_sdk/techniques/frame-interpolation/)
  -- temporal/frame-generation inputs and swapchain/pacing integration.
- [WASI](https://wasi.dev/) -- no-ambient-authority capability sandbox and
  Component Model entry point.
- [WebAssembly security model](https://webassembly.org/docs/security/) --
  sandbox and host-controlled embedding boundary.
- [WebGPU Recommendation](https://www.w3.org/TR/webgpu/) -- portable validated
  GPU resource, pipeline, and command interface.
- [Wasmtime platform support](https://docs.wasmtime.dev/stability-platform-support.html)
  and [minimal platform example](https://docs.wasmtime.dev/examples-minimal.html)
  -- `no_std`, AOT, and custom-OS embedding constraints.
- [Godot web export documentation](https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_web.html)
  -- concrete WASM engine portability and graphics/thread/audio/network limits.
- [IETF RFC 9334, RATS Architecture](https://www.rfc-editor.org/rfc/rfc9334.html)
  -- attester/verifier/relying-party roles, evidence, appraisal, freshness, and
  privacy/security limits.
- [Microsoft: How Windows uses the TPM](https://learn.microsoft.com/en-us/windows/security/hardware-security/tpm/how-windows-uses-the-tpm)
  -- measured-boot scope and remote attestation.
- [Steamworks: Proton and anti-cheat](https://partner.steamgames.com/doc/steamhardware/proton)
  -- studio enablement/configuration remains required.
- [Linux `SCHED_DEADLINE`](https://docs.kernel.org/scheduler/sched-deadline.html)
  -- runtime/deadline/period, CBS/EDF, bandwidth isolation, and admission.
- [Vulkan specification](https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html)
  -- asynchronous queues and explicit semaphore/fence ordering.

### Internal sources read directly

- `docs/THINK-2026-07-21-gaming-on-logansos-framing.md`
- `docs/specs/neoplutarch-ai-subsystem-spec.md`
- `docs/specs/neoplutarch-ai-capability-semantics-spec.md`
- `docs/RESEARCH-2026-07-20-gpu-graphics-compute-api-design-FINDINGS.md`
- `docs/ADR-073-atrium-surface-protocol-v1.md`
- `kernel/src/atrium/{mod.rs,buffer.rs,present_backend.rs}`
- `kernel/src/drivers/{dma_arena.rs,accel/}`
- Prometheus, Cerebro, and Jarvis gaming-facet FINDINGS available in the shared
  worktrees during this pass.

## 13. Confidence legend and close

- **[High]:** supported by current source/spec behavior or primary external
  documentation; major conclusion is unlikely to change with implementation
  detail.
- **[Medium-high]:** mechanism has strong precedent and a bounded local mapping,
  but a named integration or performance proof is missing.
- **[Medium]:** plausible architecture whose value depends on measurements,
  hardware behavior, ecosystem adoption, or an unbuilt dependency.

This is one research artifact. Per dispatch, no code, QEMU, or CI gate was run.
Every surviving proposal has a falsifiable spike and a stop condition; none is
permission to silently expand game or Arthur authority.
