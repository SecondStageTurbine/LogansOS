<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-61 browser architecture cross-check — Chad

**Date:** 2026-07-20
**Reviewer:** Chad (Codex; independent cross-family review of Prometheus/Qwen)
**Reviewed:** `docs/RESEARCH-2026-07-20-ot61-browser-architecture-FINDINGS.md`
**Status:** CROSS-CHECK FINDINGS — documentation only, no gate

## 0. Verdict

**REFUTE the decision as stated.** **[High confidence]**

The license/adoption premise is sound, and reusing upstream browser algorithms is
still preferable to clean-room HTML/CSS. The proposed product, however, is not a
bounded `no_std` port of four separable crates. Servo's useful layout/paint/DOM
closure is a hosted, threaded browser engine; WebRender page paint consumes a GL
device/context contract absent from OT-63; Boa is neither `no_std` nor a drop-in
replacement for Servo's SpiderMonkey DOM bindings; and the claimed security
model grants authority to the wrong process and mistakes read-only pixels for
unreadable pixels. Those are load-bearing, not wording amendments.

Do not open a 6–9 month lane for `webrender + stylo + html5ever + Boa` on the
assumption that winit/GStreamer/`std` can simply be stripped. Replace the decision
with two explicit tracks:

1. an own, limited CPU reader using narrowly audited parser/style/layout pieces,
   with no modern-web claim; and
2. a separately costed Servo platform program, beginning with the already planned
   six-week host closure spike and choosing either bounded hosted-Rust/POSIX-like
   services plus upstream Servo/SpiderMonkey, or a funded maintained fork.

The credible prior estimate for the second track is the repository's existing
**18–24 month calendar / multi-person program**, contingent on the host closure
and graphics spikes—not 6–9 months. **[Medium confidence on duration; high on the
direction and missing work.]**

## 1. License spot-check

The adoption conclusion survives, but one classification in the reviewed paper
does not. Raw upstream files were fetched/cloned on 2026-07-20.

| Project / raw source | Observed license | Finding |
|---|---|---|
| Servo [`LICENSE`](https://raw.githubusercontent.com/servo/servo/main/LICENSE) | MPL-2.0 | Corroborated |
| WebRender [`LICENSE`](https://raw.githubusercontent.com/servo/webrender/main/LICENSE) | MPL-2.0 | Corroborated |
| Stylo in Servo | Servo source files/MPL-2.0 | Corroborated for the Servo-carried code; retain per-file notices |
| html5ever [`LICENSE-MIT`](https://raw.githubusercontent.com/servo/html5ever/master/LICENSE-MIT) + [`LICENSE-APACHE`](https://raw.githubusercontent.com/servo/html5ever/master/LICENSE-APACHE) | MIT OR Apache-2.0 | **Divergence:** not MPL-2.0; still adoptable |
| Boa [`LICENSE-MIT`](https://raw.githubusercontent.com/boa-dev/boa/main/LICENSE-MIT) + [`LICENSE-UNLICENSE`](https://raw.githubusercontent.com/boa-dev/boa/main/LICENSE-UNLICENSE) | MIT OR Unlicense | Corroborated |

This is not a transitive-license audit. A pinned closure/SBOM remains required.
**[High confidence.]**

## 2. The proposed Servo subset is not bounded by removing the shell

### 2.1 What source inspection found

I inspected Servo `25b89d879cecef6e63203f70993ba22b5e7cd9ef` and current
html5ever/Boa source, applying the same test used in the iced spike: crate
boundaries do not count as portability boundaries unless the retained source and
resolved graph actually support `no_std`.

None of the retained Servo crates inspected declares `#![no_std]`. Direct hosted
coupling is present below servoshell:

- `components/paint` uses `std::env`, `std::fs`, wall-clock time, thread join
  handles, refresh-driver threads, `available_parallelism`, and rayon-backed
  painting.
- `components/net` uses a global async runtime, `std::thread`, Tokio, Hyper,
  rustls adapters, filesystem/hosts access, clocks, and threaded loaders.
- `components/fonts` uses threads, global synchronization, system font service,
  and platform discovery/loader assumptions.
- Servo's root graph pins WebRender 0.70 plus `surfman`, `gleam`/`glow`, rayon,
  Tokio, `ipc-channel`, fontconfig/Freetype, and mozjs. `wgpu-core` is also in the
  workspace, but for the WebGPU web API; it is not WebRender's page-paint backend.
- html5ever itself has direct `std` imports throughout tokenizer/tree-builder
  support (`Cow`, `Cell`, `VecDeque`, `io`) and exposes no no-std feature.
- Boa's engine has no no-std feature. Its parser uses `std::io`; its string, GC,
  AST, interner, and engine graph use `std` directly; dependencies explicitly
  enable `std` on `rustc-hash`, `indexmap`, and ICU support.

Some imports can be changed mechanically to `core`/`alloc`. The hard work is
policy and runtime semantics: threads/tasks/wakeup, TLS and allocation, clocks,
font discovery/shaping, images, DOM ownership/GC rooting, timers, event dispatch,
and crash/process boundaries. As with iced, carrying this against a moving
upstream is a maintained fork. Here it is much larger. **[High confidence.]**

### 2.2 Stylo is not layout, and four crates are not an engine

The recommendation collapses several different layers. Stylo supplies style
calculation; it does not by itself provide Servo's DOM, layout, paint, resource
loading, event loop, Web APIs, or JS bindings. Servo's layout consumes Servo DOM
and style types; paint consumes layout fragments, fonts, image/raster state and
WebRender API types. Replacing SpiderMonkey with Boa discards generated DOM
bindings, rooting/tracing integration, promises/microtasks, exception plumbing,
Web IDL conversions, and host-object lifecycle—not merely an interpreter call.

The proposed "minimal DOM bindings" may render a controlled demo, but it is not
the stated gate, “render a real HTTPS page ... basic JS.” Even basic pages require
subresource loading, URL/base resolution, CSSOM/DOM mutation invalidation,
script ordering, microtasks, timers, events, cookies, redirects, image decode,
and error/cancellation behavior. **[High confidence.]**

### 2.3 Recommendation change

Retain upstream algorithms only behind Altimit-owned seams for the limited
reader. For the compatibility browser, prefer a bounded **full Servo engine
closure** (including SpiderMonkey inside a disposable renderer) over an invented
Boa/DOM/Servo hybrid, if the six-week spike proves its platform closure can be
hosted without a general Linux personality. “Pure Rust” is not evidence of
`no_std`, and replacing the mature JS/DOM seam increases rather than bounds the
program. **[Medium-high confidence.]**

## 3. WebRender does not map onto today's OT-63 present seam

The reviewed paper says WebRender's “existing wgpu backend maps naturally” to
`GpuSubmitCap` and a `SharedPixelBuffer`. That is false for the audited Servo
architecture and overstates OT-63.

The repository's own GPU design (`docs/THINK-2026-07-12-gpu-accel-path-design-janus.md`
§4.1) records the actual seam: Servo page paint is WebRender over
`surfman`/GL; Servo's wgpu path is WebGPU content. The designed first adapter is
therefore `surfman-neo + libneogl`, with an inventoried WebRender GL call/shader
closure—not a swapchain redirect.

OT-63 currently provides a present-control seam: a `GpuSubmitCap` gate, a bounded
pending-flip ring, scanout-DMA fencing, and synthetic vblank completion pending
the real Gen9 flip. `kernel/src/atrium/present_backend.rs` explicitly says Slice 1
is QEMU-proven with **no GPU** and Slice 4 will add `PLANE_SURF` plus the real ISR.
It does not provide textures, samplers, render targets, shader pipelines, GL
object semantics, a graphics queue, buffer import, or WebRender command lowering.

Rendering “to a staging texture then blitting into `SharedPixelBuffer`” itself
requires the missing graphics allocation/copy/render API and a synchronization
contract. A scanout fence only constrains which physical buffer may be displayed;
it does not execute WebRender. **[High confidence.]**

The browser plan must depend on a separately gated graphics track:

- graphics contexts/PPGTT + VT-d domain;
- typed buffers/textures/render targets and import/export;
- quota-bound queues, fences, hang/reset and fault attribution;
- the pinned WebRender GL-profile adapter (or an explicitly funded upstreamable
  backend after the closure spike);
- compositor import/composition/present, with permanent CPU fallback.

This is the largest schedule divergence. It was already estimated separately in
the repository; it cannot be hidden inside a 3–4 week paint phase.

## 4. Schedule cross-check

The 6–9 month total is not credible for the stated real-page MVP on the current
substrate. The estimate prices already-solved web standards work but omits or
underprices integration that becomes engine work when the original bindings are
removed.

| Area | Reviewed estimate | Cross-check |
|---|---:|---|
| Parse/DOM + CSS/layout | 6–9 weeks | Omits a production DOM, style/layout invalidation, Servo type/ownership adaptation, font and image services |
| Paint/WebRender + G1 | 5–7 weeks combined | Depends on a missing WebRender-profile graphics device; OT-63 present is not that API |
| Boa + DOM | 6–8 weeks | Load-bearing underestimate; a new JS/Web-IDL/GC/DOM integration and Web API runtime |
| HTTPS/net | 2–4 weeks | A GET is plausible; browser fetch, redirects, cache/cookies, CSP/CORS, cancellation and broker isolation are not |
| Security | 3–4 weeks | Requires process/broker lifecycle, revocation, origin transitions, cross-origin frames, quotas and adversarial gates |

An own limited static reader can plausibly land much earlier. A modern-compatible
browser is a platform program. Use the previously documented 18–24 month
provisional calendar only after actual Servo/graphics closure spikes; attach team
size and person-months to every future estimate. **[Medium confidence on numeric
range; high that 6–9 months is unsupported.]**

## 5. Capability security: sound direction, wrong authority graph

### 5.1 Corroborated primitives

The live tree supports valuable parts of the model:

- `NetDialCap` carries immutable `(host, port)` scope and derivation can only
  attenuate it.
- `SharedPixelBuffer` maps client RW+NX and compositor RO+NX.
- `InputFocus` splits compositor activation from client receive authority.
- revocation has a derivation relationship, and `GpuSubmitCap` gates the present
  action when enforcement is armed.

Tab/site-instance isolation is coherent with these primitives. **[High
confidence.]**

### 5.2 Load-bearing corrections

**The renderer should hold no dial cap.** A page routinely fetches scripts,
styles, images and frames from many origins. Redirects, DNS rebinding, CORS/CSP,
credentials, service workers and navigation all require policy above a single
`host:port` socket. The network broker should hold attenuated `NetDialCap`s,
re-authorize every request/redirect, and return bounded response streams over
IPC. This matches `Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md`; the reviewed
paper regresses from it. Kernel host/port enforcement is defense in depth, not a
kernel implementation of the web same-origin policy. **[High confidence.]**

**RO is not no-snoop.** The compositor's `MAP_READ` / hardware RO mapping prevents
it from modifying client pixels; it explicitly permits reading them so it can
composite. A compromised compositor can copy/exfiltrate pixels it can map. The
true claim is cross-client write isolation plus least authority, not structural
confidentiality from the compositor. Any stronger claim needs a trusted display
path/encrypted composition design that does not exist. **[Certain.]**

**One surface per tab is not iframe/site isolation.** Cross-origin frames require
separate principals plus a compositor/browser-coordinator embedding protocol,
input routing, hit testing, navigation authority and lifecycle. A tab-level
buffer alone does not provide that graph. **[High confidence.]**

**Do not share mutable JS engine state across principals.** The reviewed grant of
read caps for “shared JS engine state” is underspecified and unsafe. Share
immutable code/pages where proven; keep heaps, DOM state, origins and caches
behind explicit brokers or per-principal objects. **[High confidence.]**

## 6. Contradiction check

The “no contradiction” claim does not survive unchanged.

- **Own G1 compositor:** compatible if WebRender outputs an imported surface;
  it does not imply WebRender can target `SharedPixelBuffer` without the missing
  graphics adapter.
- **Vello:** no product-level contradiction between compositor/chrome Vello and
  WebRender page paint. There is a schedule/substrate contradiction if “same GPU
  substrate” is treated as already implemented: Vello needs wgpu/compute while
  Servo page paint currently needs GL.
- **kTLS:** compatible as transport machinery, but a TLS record layer/HTTPS GET
  is not the browser fetch service or web security policy.
- **Rust/no_std posture:** direct contradiction with adopting Servo layout/paint,
  html5ever and Boa *as shipped* into no-std userland. This requires a hosted
  userland runtime decision or maintained ports.
- **Capability model:** compatible with a brokered multi-process browser;
  contradicted by claims that renderers should directly hold origin dial caps or
  that RO compositor access supplies pixel confidentiality.

Therefore the architecture can be made consistent only after changing the
runtime, graphics, broker and schedule decisions. **[High confidence.]**

## 7. Load-bearing divergences and corroborations

1. **Divergence — portability:** the proposed four-crate `no_std` subset is not a
   bounded port; hosted coupling exists throughout the retained engine graph.
2. **Divergence — GPU:** WebRender page paint is GL/surfman, not Servo's wgpu
   WebGPU path; OT-63 present/scanout is not a WebRender device.
3. **Divergence — JS:** Boa replacement creates a new DOM/Web-IDL/GC/Web API
   integration and invalidates the 6–8 week estimate.
4. **Divergence — security:** use a network broker with renderer-zero-dial;
   compositor RO protects integrity, not confidentiality.
5. **Corroboration:** upstream licenses are adoptable (with html5ever corrected to
   MIT/Apache-2.0), tab/site principals fit the cap primitives, and own G1 plus
   separate Vello/WebRender renderers is conceptually consistent once the two GPU
   adapters are actually built.

## 8. Evidence and method

- RAG health was `healthy` (`ollama=true`, `qdrant=true`) on 2026-07-20. Queries:
  `webrender wgpu gpu`, `net tcp tls_record fetch`, `atrium present_backend
  GpuSubmit scanout`, and capability/NetDial origin scope. Results were used only
  for discovery; cited internal files and source were then read directly.
- Servo source audited at commit
  [`25b89d8`](https://github.com/servo/servo/tree/25b89d879cecef6e63203f70993ba22b5e7cd9ef),
  including root/servoshell and layout, paint, script, net and fonts manifests and
  source.
- Current upstream html5ever and Boa repositories were shallow-cloned and their
  manifests/source searched for `no_std`, direct `std`, threading/runtime and
  platform dependencies.
- Internal ground truth: `kernel/src/atrium/present_backend.rs`,
  `kernel/src/atrium/buffer.rs`, `kernel/src/arch/x86_64/syscall.rs`,
  `kernel/src/sched/process.rs`,
  `docs/THINK-2026-07-12-gpu-accel-path-design-janus.md`, and
  `Plans/RESEARCH-2026-07-12-web-browser-FINDINGS.md`.
