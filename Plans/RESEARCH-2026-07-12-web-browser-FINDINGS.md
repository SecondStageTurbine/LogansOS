<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# FINDINGS: the release-gating LogansOS web browser

**Date:** 2026-07-12
**Researcher:** Janus
**Dispatch:** `Plans/DISPATCH-2026-07-12-web-browser-headline-research-prometheus.md` (reassigned to Janus)
**Scope:** deep research + design; host-only; no implementation; no QEMU
**Upstream snapshots audited:** Servo `9c77cd97063589ae64a2d8f5527ff192385f2dfb`; Blitz `e4bc4675e99c17346c024a169f67107eee80a22f`; Boa `4f98f6446812f1c5923153f8c7e1beda3a891bfe`; Nova `4eea7c6fae180a8a2eed45c3f6acd0d9256522b1`; Kiesel `7e6cc72f2fae056ed88b1fa57c62a98877f2960a`; Ladybird `1fb86929fde87049f617eca81793fc667142ffc6`.

## Decision in one page

The release path should target **Path A: a capability-confined Servo embed, including its SpiderMonkey JS engine**. Path B should be built only as the deliberately named **CPU bring-up reader** and kept as a sovereign-component research hedge. It cannot satisfy the release gate on the evidence available today.

This is not a claim that Servo is already a Chrome-equivalent. Servo calls itself a prototype; its embedding crate is pre-1.0 with expected breaking monthly releases; IndexedDB, SharedWorker, accessibility, and other modern-web surfaces are still developing or experimental; and Servo's own download page says not to log into a bank yet. It is nevertheless the only evaluated path that joins a real DOM/web platform to a production-lineage JS engine. The shortest honest route is therefore to **port and contain an existing incomplete real engine**, not to assemble a new browser engine around a language interpreter.

Three corrections to the dispatch's candidate shorthand are load-bearing:

1. Current Servo does **not** paint pages as “WebRender on `wgpu`.” Its page paint/compositor closure names WebRender plus `surfman`/GL; `wgpu-core` is attached to Servo's optional WebGPU web API. WebRender is still aggressively GPU-oriented, so graphics acceleration remains on the credible ship critical path, but the compatibility surface is GL/WebRender unless LogansOS funds a backend change.
2. Current Blitz has explicit Vello CPU and hybrid renderers. A CPU reader is therefore technically credible without a GPU. Its own docs also say it has no JS engine, can run only about 20% of WPT subtests, and is not intended to provide the whole browser platform.
3. Kiesel is **Zig, not Rust**, and its default implementation includes C BDWGC and QuickJS-derived `libregexp`, plus Rust ICU4X/Temporal components. It does not belong in a “pure-Rust-only” fork.

### Recommendation

Adopt **A-target/B-scaffold**:

- **Ship target:** Servo + SpiderMonkey, inside a process/capability boundary that exposes no C++ or POSIX personality at the native ABI.
- **Bring-up floor:** Blitz HTML/markdown with its Vello CPU renderer, LogansOS-native fetch, no JS, visibly labeled “Reader — limited web.” It proves networking, text, input, compositor surfaces, downloads, and Arthur control. It is never counted as the day-1 browser gate.
- **Shared investment now:** pull OT-52's compositor/graphics acceleration path forward. Design it to serve WebRender/GL first and retain a route to `wgpu`/Vello. This also unlocks the T2/T3 GUI shelf.
- **Release acceptance:** a reproducible modern-site battery must pass on LogansOS, not merely upstream Servo or host Linux. Minimum classes: authenticated mail, a banking-class SPA using a non-production test account, WebAuthn/OAuth flow, collaborative editor, media site, Cloudflare-protected site, WebAssembly app, accessibility/keyboard flow, download/upload, and crash/restart isolation.

### Earliest honest date

With three to four experienced engineers staffed continuously and graphics/OS work proceeding in parallel, the earliest credible release-gate attempt is **18–24 months after kickoff: January–July 2028 from a July 2026 start**. This is an engineering estimate, not an upstream promise. The range assumes upstream Servo continues its present velocity and the six-week spike below does not expose a fatal unsupported-target issue. A one- or two-person serial effort is more honestly **30–48 months**.

The estimate comprises approximately **34–58 person-months** for the LogansOS-specific route: unsupported-target runtime/`std` work (8–14), graphics/compositor and WebRender/GL accommodation (8–14), Servo platform/embedding port (8–12), cap/process containment (5–8), and compatibility/hardening/release battery (5–10). Parallelism shortens calendar time but does not erase integration dependencies. This range does not price rewriting Servo or SpiderMonkey.

## Fixed acceptance bar

“Renders the real modern web” means more than HTML/CSS screenshots or Test262 conformance:

- executes production JavaScript with acceptable responsiveness;
- implements the DOM, events, navigation, fetch, cookies, storage, workers, modules, WebAssembly, forms, accessibility, media, and security policies sites actually compose;
- supports HTTP/2-class browsing behavior, a production trust store and update policy, redirects, compression, caching, proxies where required, and modern authentication flows;
- isolates hostile content, recovers a tab/renderer without taking down the shell, and constrains persistence/network/device access;
- remains usable on the release hardware under a defined memory/frame-time budget;
- passes the LogansOS site battery repeatedly with no telemetry or unexplained egress.

WPT and Test262 are necessary diagnostic evidence. Neither is, alone, a release claim.

## Method and three-vote verification

All external load-bearing claims were checked against project-owned repositories, manifests/locks, test dashboards, release notes, or documentation. Search summaries were used only to locate those sources. The two Cargo lock audits count the complete checked-in, cross-platform/all-feature closure, not the smaller platform/product subset; they price audit and port surface, not final binary contents.

| Claim | Vote 1 | Vote 2 | Vote 3 | Finding |
|---|---|---|---|---|
| Path A is the only near-term route to real JS-heavy sites | Servo directly uses `mozjs` and supplies DOM/layout/network | Servo ships a real embedding crate and monthly engine releases | Blitz has no JS; Boa/Nova/Kiesel are language engines without a browser DOM | Path A is the target, subject to site-gate proof |
| The C++ island is real and large | Servo root `Cargo.toml`: `mozjs = 0.18` | `servo/mozjs`: bindings to SpiderMonkey's C++ API, ESR140 lineage | Servo defaults include `js_jit`; upstream warns SpiderMonkey is very large/slow to compile | Do not claim a pure-Rust Servo product |
| Acceleration belongs on the ship critical path | Servo says WebRender rasterizes layouts using the GPU as much as possible | Servo manifests bind paint to WebRender + `surfman`; WebGPU separately uses `wgpu-core` | Blitz/Vello offers GPU/hybrid while its CPU backend proves only a floor | GPU is a performance/release prerequisite, not a literal first-pixel prerequisite |
| Path B cannot meet the modern-web gate now | Blitz: pre-alpha, no JS bindings/engine, intentionally omits browser features | Blitz WPT: only about 20% of subtests runnable; CSS score is partial even within that subset | Boa/Nova/Kiesel conformance does not supply DOM/Web APIs and integration | B works for docs/static/light content, not Gmail-class apps |
| Servo's LogansOS port is a platform project, not a crate drop | Servo net uses Tokio multi-thread runtime, filesystem, Hyper h1/h2, rustls | Servo uses IPC/shared memory, OS windows, GL contexts, fonts, persistent storage, threads | SDK-v0 has static ELF, no PT_TLS, no dynamic linking, no user-thread ABI, and `no_std` userland | Build a browser platform rung before expecting Servo to link |
| A from-scratch engine is not the fast path | Ladybird grew from a 2018 OS project/HTML viewer and remains pre-alpha | Ladybird reports eight paid full-time engineers and a developer/early-adopter alpha target in 2026 | Its own repo had roughly 78k commits at audit and still prioritizes real-site gaps | Rebuilding the browser platform is multi-year/program-scale |

## Current LogansOS substrate: useful, but far below a browser host

RAG was healthy (`ollama=true`, `qdrant=true`) and was used to discover the substrate; the cited files were then read directly.

What exists:

- a 32-bpp linear framebuffer with software clear/plot/fill/blit/glyph/string primitives;
- a designed software-first cap-native compositor sequence (`SurfaceCap`, `BufferCap`, damage, present feedback), not yet an implementation;
- TCP/DNS, TLS 1.3, HTTP/1.1 fetch foundations, CSPRNG, and socket handles;
- process address-space isolation, capability derive/grant/revoke machinery, file-right attenuation, and process IPC;
- a static ELF64 app ABI and `no_std` libneo wrappers.

What a browser still lacks:

- an implemented compositor, double buffering, retained scene graph, graphics device API, GL or WebGPU-compatible command path, production font/text stack, and GPU fault containment;
- user threads, PT_TLS, the broad `std` semantics expected by upstream crates, mature async polling/wakeup, shared-memory APIs suitable for a multiprocess engine, memory mapping/file semantics expected by SpiderMonkey, and process-service supervision;
- HTTP/2 breadth, a production trust-store/update mechanism, proxy and cache policy, browser storage/quota, MIME/content decoding breadth, media plumbing, WebAuthn, clipboard and accessibility services;
- a kernel-enforced outbound-network authority. Today `SYS_NET_DIAL` validates a name and directly installs a full-rights TCP socket. Arthur's `Grant::NetDial { host, port }` is a useful userland policy check, but a hostile renderer can call the syscall directly. Browser confinement therefore requires a kernel-consumed `NetDialCap` (or equivalent broker-only syscall authority), not only a UI grant record.

The gap is why “compile the Servo crate” is the wrong mental model. The first product is a **browser host platform** that Servo can inhabit without turning LogansOS into POSIX.

## Path A — Servo embed

### What exists today

Servo is active and fast-moving: the audited workspace is version 0.4.0; the project published its first crates.io embedding release in April 2026 and offers an LTS line. Its public API centers on `Servo`, `WebView`, builders/delegates, a rendering context, an event-loop waker, and explicit paint/present flow. This is a substantially better seam than treating `servoshell` as the product.

The seam is not stable in the 1.0 sense. Servo's book calls embedding documentation sparse/work-in-progress; regular monthly releases may break embedders; and the project has not defined 1.0. Servo continues to land hundreds of commits per month and real platform features, but IndexedDB, SharedWorker, accessibility and other surfaces were still experimental/in progress in 2026. Its own download page's bank warning is decisive evidence against claiming current production readiness.

**Real-site verdict today:** substantially more real web than Path B, and capable of browsing many ordinary sites and web apps, but **Gmail/banking-class release compatibility is unproven**. The six-week spike must run the actual acceptance battery on upstream Servo before the OS port is allowed to inherit an assumed green status.

### Dependency and native-code audit

The checked-in Servo `Cargo.lock` contains **1,112 package stanzas / 1,033 unique package names** across its full cross-platform/all-feature closure. This is not the final LogansOS product count; it is the review universe from which a target-specific closure must be cut.

| Surface | Direct/locked evidence | LogansOS consequence |
|---|---|---|
| JavaScript | `mozjs 0.18`, default `js_jit`, SpiderMonkey ESR140 C++ | C++ toolchain/runtime port, executable-memory/JIT policy, GC/thread/TLS integration, large source build |
| Page paint | WebRender 0.69, `surfman`, `glow`, platform GL/ANGLE edges | implement a supported GL/context/present route or fund a WebRender backend port |
| WebGPU API | optional `wgpu-core`/`wgpu-types` | separate from page paint; useful later for WebGPU sites and Vello convergence |
| Network | Hyper h1/h2, Tokio multi-thread + fs, rustls + AWS-LC provider | replace/port async runtime, DNS/socket/file surfaces and choose a sovereignty-reviewed crypto provider |
| Persistence | `rusqlite`/`libsqlite3-sys`, filesystem/cache/storage components | per-origin storage broker, quota, SQLite C audit or replacement decision |
| Text/image | FreeType/fontconfig/HarfBuzz system edges, image codecs | production font service and codec isolation; several C/system dependencies |
| Media | GStreamer family is optional but expected for common desktop media | initially exclude, then add a separately sandboxed media plan; no silent “modern web” overclaim |
| Platform/FFI | ANGLE, GLib/GStreamer, X11/Wayland/Windows/ObjC/OpenHarmony crates appear in the lock closure | most are target-eliminated, but every retained native edge needs an SBOM, reproducible build and cap boundary |

The manifest proves a narrower but more important point than language percentages: **Servo is a Rust browser engine around several native platform components, with one indispensable C++ JS engine**. “Rust engine” must never be rewritten as “pure Rust product.”

### The `std` surface

Servo assumes, directly or transitively:

- OS threads, thread-local storage, synchronization, timers and wakeups;
- files/directories, paths, environment/config resources, temporary/cache storage and memory mapping;
- sockets, DNS, HTTP/1+2, TLS, async I/O and a Tokio multi-thread executor;
- shared memory/IPC for multiprocess mode;
- native window/display handles, GL contexts, clipboard/input/accessibility hooks;
- fonts, locale/timezone, randomness, SQLite and optional media libraries;
- JIT executable memory and SpiderMonkey platform hooks.

SDK-v0 exposes useful primitives but not this contract. In particular, static ELF forbids PT_TLS and there is no user-thread ABI. Those are first-order blockers, not wrapper chores. The correct architecture is a bounded **browser platform adapter/std-analog**, not an OS-wide promise to emulate POSIX.

### Is SpiderMonkey a bounded component or a compatibility island?

This is the genuine PL fork. Both readings are defensible.

**Bounded-component case:**

- one engine implementation sits behind Servo's Rust bindings;
- it runs only inside per-site renderer processes with no network or filesystem caps;
- it exposes no Linux/Windows ABI and does not make the native SDK imitate POSIX;
- source, compiler, patches and artifacts are owner-auditable/reproducible;
- a renderer crash is disposable and a compromised renderer cannot mint authority;
- future Boa/Nova substitution remains an internal engine experiment, not an ABI promise.

On that contract, SpiderMonkey resembles a complex codec or driver: a large, security-sensitive implementation component behind a narrow authority boundary.

**Compatibility-island case:**

- its JIT/GC/platform requirements force PT_TLS, VM, signals, threading and filesystem behavior to spread through general userland;
- C++ exceptions/RTTI/allocator/toolchain assumptions leak into the SDK;
- Servo becomes dependent on a Linux personality or ambient host services;
- the component gains direct network/file/device access or becomes non-replaceable at the public ABI;
- prebuilt opaque archives replace reproducible owner builds.

On that outcome, it is precisely the compat island Article 5.7 warns against.

**Janus recommendation:** ratify SpiderMonkey conditionally as a bounded component, with the five bounded-component properties above as acceptance criteria. If the six-week port spike shows they cannot be met without a general POSIX personality, return to the PL; do not silently move the boundary.

## Path B — Blitz plus a non-C++ language engine

### Renderer reality

Blitz is a useful and impressively modular HTML/CSS renderer, not a browser engine. Its own README says:

- pre-alpha, with many bugs/missing features;
- designed for HTML/CSS and intentionally not the entire browser feature set;
- no JS bindings/engine;
- high-level HTML wrapper currently lacks interactivity;
- networking, shell and painting are modular.

It composes Stylo, Taffy, Parley, html5ever and Vello-family renderers. Its current tree offers Vello CPU, GPU and hybrid paths, so a LogansOS CPU reader can render before acceleration exists. The status dashboard is candid: only about 20% of total WPT subtests can run without JS; within the CSS subset, 47.15% of tests and 42.65% of subtests pass at the audited page, with major partial/missing areas including positioning, writing modes, 3D transforms, text effects, SVG styling, containment and advanced scrolling.

The full Blitz lock has **896 package stanzas / 813 unique names**. The all-feature closure includes OpenSSL, SQLite and Skia C/C++ edges. A sovereignty-minimal product must select Vello CPU/hybrid, replace `blitz-net`'s default `reqwest` native-TLS path with libneo networking, and exclude optional Skia/persistence edges. Therefore “Blitz is pure Rust” is true only for a carefully selected renderer closure, not the repository's entire default/all-feature universe.

### JS candidates

| Engine | Language/purity | Current primary-source state | Browser consequence |
|---|---|---|---|
| Boa | Rust; full workspace lock includes platform/tooling edges, while the core is Rust | calls itself experimental; >90% Test262 (v0.21 reported 94.12%); bytecode interpreter; example runtime has fetch/URL/timers and WinterTC minimum APIs | strongest Rust candidate for controlled scripting, but no browser DOM/layout/event binding, site isolation, JIT-class performance, WebAssembly/browser platform integration |
| Nova | Rust | May 2026 dashboard: 40,515 pass, **79.9%**; not fast by its own README; incomplete RegExp, no WebAssembly | research hedge; not a modern-site engine |
| Kiesel | **Zig**, not Rust | early/limited by its README; Zig VM; default C BDWGC + QuickJS `libregexp`; Rust ICU4X/Temporal | neither pure Rust nor ship-ready; exclude from the pure-Rust ratifiable option |

Test262 percentages cover ECMAScript language conformance. Gmail-class compatibility also requires a huge, synchronized DOM/Web API, security, event-loop, storage, worker, module, WASM and networking implementation. None of these three forms a maintained browser integration with Blitz. Joining one to Blitz is not “wiring JS”; it is starting a new web engine program.

### What works, honestly

**Credible with Blitz CPU and no general JS:** local markdown/HTML, documentation, simple news/blog pages, Hacker News-class server-rendered content, controlled first-party forms, and Arthur-fetched/readability views. CSS gaps will still cause visible defects.

**Potential with Boa and deliberately small host APIs:** controlled first-party mini-apps and progressive-enhancement scripts written to the supported contract.

**Not credible now:** Gmail, modern banking SPAs, Slack/Discord-class apps, collaborative editors, JS-first commerce, Cloudflare/browser-integrity flows, large framework applications, and web apps depending on IndexedDB/workers/WASM/media/WebAuthn/full accessibility.

### Cost and verdict

- Blitz CPU reader integrated with Logans networking/compositor/Arthur: **4–8 person-months**.
- Controlled Boa scripting for first-party content: another **4–8 person-months**, explicitly not general web compatibility.
- A defensible Blitz+Rust-JS modern browser: **80–150+ person-months and at least 3–5 years with a sustained specialist team**, with a high risk that the web advances faster than the implementation. Ladybird's years, eight paid engineers, roughly 78k commits and still-pre-alpha state are the reality check.

Path B maximizes source-language sovereignty only after pruning native edges, but misses the fixed release requirement. It is a scaffold and research hedge, not the ship target.

## Graphics and acceleration: shared leverage, corrected architecture

### What the browser actually needs

The minimum production graphics contract is:

1. device discovery and an owner-controlled driver binding;
2. process-scoped graphics contexts/queues with memory and command quotas;
3. shareable, fenced `DmaBuffer`-class surfaces and explicit map/unmap;
4. texture/buffer allocation, upload and format negotiation;
5. shader/module validation plus render/compute pipeline creation;
6. command submission, completion fences, timeouts, reset and fault attribution;
7. compositor import, damage, frame pacing and present feedback;
8. a software fallback for recovery and the limited reader.

For Servo, the shortest compatibility target is WebRender over an accommodated GL/context surface. `wgpu` is still required for the WebGPU web API and is strategically attractive, but it does not replace Servo's current WebRender/GL port by declaration. For Blitz/Vello, `wgpu`/hybrid is the direct acceleration route.

### Why pull it forward

Acceleration is a **credible-performance and compatibility prerequisite** for the release browser: WebRender intentionally uses the GPU heavily, browser animation/composition is latency-sensitive, and WebGL/WebGPU/canvas/media are part of real sites. It is not required for the first reader pixels because Blitz has a CPU renderer and the compositor can begin in software.

This is shared infrastructure, not browser-only spend. The same surface unlocks:

- OT-52 GPU composition and later vector/text;
- T2/T3 GUI applications and high-refresh workfields;
- canvas/WebGL/WebGPU content;
- image/media processing and accessibility magnification;
- reuse of the existing accelerator ownership, fencing and fault-containment lessons.

The browser changes GUI sequencing: GUI-0/GUI-1 software protocol proof remains sound, but **GPU composition and the browser-oriented graphics API move immediately behind it**, rather than waiting until the rest of the GUI shelf is mature.

## Browser confinement by construction

The browser should be a graph of mutually suspicious services, not one process holding every owner capability.

```text
owner/Arthur -> BrowserControlCap -> browser UI/coordinator
                                      |-> SiteInstance(origin A): renderer + JS, no net/fs/device caps
                                      |-> SiteInstance(origin B): renderer + JS, no net/fs/device caps
                                      |-> network broker: NetDialCap subset + TLS/trust policy
                                      |-> storage broker: derived per-origin StoreCap
                                      |-> download/upload broker: one explicit owner-selected FileCap
                                      |-> image/media decoder workers: byte-in/buffer-out only
                                      `-> GPU broker/compositor: bounded surface/queue caps, no net/fs
```

### Required rules

- **Origin is a kernel-visible label on delegated caps, not authority invented by JS.** A site instance receives only IPC channels, bounded shared buffers and a surface/input route.
- **No ambient network.** A kernel-enforced `NetDialCap` limits scheme/host/port and is held by the network broker. Redirects, DNS rebinding, service workers and subresources are re-authorized by the broker; a connected socket cap is not a wildcard dial cap.
- **No ambient filesystem.** Downloads require an owner-selected directory/file capability; uploads grant one read-only file cap to the broker. Renderer paths are virtual names, never global paths.
- **Per-origin persistence.** Cookies/cache/IndexedDB/localStorage live behind a derived origin storage cap with quota and explicit clearing. Private sessions receive ephemeral caps that die with the session.
- **Per-site process disposal.** Cross-origin isolation should prefer separate site instances. A renderer/JS compromise loses only that origin's ephemeral state and channels. Restart must not restart Arthur or the shell.
- **Decode and GPU isolation.** Image/media/font parsing occurs in disposable workers. Renderers submit validated display/graphics commands through a quota-enforcing broker; no direct PCIe/BAR/DMA authority.
- **Capabilities follow user gestures.** Clipboard, camera/mic, location, notifications, WebAuthn, uploads/downloads and screen capture are one-purpose, time-bounded grants, visible in the browser/Intent Field.
- **Revocation is live.** Closing a tab or revoking an origin must close sockets, invalidate storage/surface caps and cancel broker work. Audit records are UX mirrors, never the authority.

This is stronger than a conventional renderer sandbox only if the kernel closes the current ambient-dial hole. Otherwise “capability browser” is branding over a bypass.

## Sovereignty, telemetry and Arthur

Article 1.7 requires the owner to remain root: no provider account, model gateway, policy update or cloud state may be required for core browsing; install/update is verified data; effects are capability-bound and auditable.

Both paths can be shipped without vendor telemetry because they are source-available components under our control. Neither gets a privacy pass from its language. Acceptance requires:

- an SBOM and license/provenance record for every retained crate/native library;
- source-built or strictly attested reproducible artifacts, including SpiderMonkey;
- all default outbound endpoints removed; no crash, metrics, search-suggestion or update egress without an owner-visible cap;
- owner-controlled search provider, trust-store update, proxy, DNS and content-policy configuration;
- a network-egress test proving a blank profile contacts only owner-requested origins;
- no privileged remote-debug or Arthur automation listener.

Arthur and the owner should drive **the same engine** through `BrowserControlCap` operations: create isolated view, navigate, query accessibility/semantic tree, capture owner-approved content, summarize, fill a form, and propose a submit. Arthur receives no hidden network or filesystem authority. A submit, download, upload, credential use or permission prompt remains a capability-disposed effect. The owner can always drive the browser directly, and `neo>` remains the assistant-free path.

Path A serves this sooner because Servo already exposes an embedding/WebView control seam and WebDriver work. Path B offers tighter internal control but only over reader-class content for the foreseeable horizon.

## Sequence to the earliest modern-web-capable browser

### Step 0 — ratify the three forks (now)

Fix the target and prevent the reader from becoming a success-metric substitute.

### Step 1 — six-week, host-only/browser-platform spike (highest-leverage first step)

This is the single highest-leverage first move because it can falsify the recommended route before the expensive OS port:

1. build the exact audited Servo revision on a supported host from source, with the proposed product feature set;
2. produce `cargo tree`/SBOM/native-toolchain closure for that feature set, not the all-feature lock approximation;
3. run and record the modern-site battery on upstream Servo;
4. map every required OS facility to SDK-v0, a bounded browser adapter, or a genuine new ABI; reject a general POSIX personality;
5. prototype the `Servo`/`WebView` delegate boundary and the process/broker split on the host;
6. decide whether WebRender-over-GL or a funded backend adaptation is the shorter Logans graphics route.

Exit green only if upstream Servo clears a meaningful portion of the site battery and SpiderMonkey/Servo can remain behind the bounded-component criteria. Otherwise return to the PL with the exact failures.

### Step 2 — start the two shared platform tracks in parallel

**Runtime track:** user threads, PT_TLS, VM/JIT mapping policy, time/poll/wakeup, shared buffers, process supervision, browser `std` adapter, production trust-store and the kernel-enforced `NetDialCap`.

**Graphics track:** software cap-native compositor first; immediately follow with the browser graphics contract, one accelerated device path, fences/reset/quotas, and WebRender/GL presentation. Preserve `wgpu` as the WebGPU/Vello path.

### Step 3 — ship the explicitly limited reader

Port the minimum Blitz HTML/markdown closure onto the Vello CPU renderer and Logans networking. Integrate direct owner navigation and Arthur read/summarize. Display a persistent limited-reader identity; never report the release browser gate green from this milestone.

### Step 4 — land Servo in capability-shaped slices

1. offscreen single-page render through the software compositor;
2. accelerated WebRender page paint and input;
3. SpiderMonkey execution/JIT and crash disposal;
4. brokered HTTP/TLS, redirects and downloads with no ambient FS/network;
5. per-origin storage/cookies/cache and process isolation;
6. accessibility/semantic tree and Arthur control;
7. WebAssembly, workers, WebGL/WebGPU and media in separately gated increments.

### Step 5 — release hardening

Run WPT subsets plus the site battery on every browser change; fuzz parsers/brokers; exercise renderer, network, decoder and GPU crashes; audit egress; measure memory/frame time; verify revocation; and perform physical-hardware usability/security validation. The release gate turns green only here.

## Prior art: firsthand lessons

### Servo / Igalia

- Active Rust engine with an actual embedding API, a production-lineage JS engine and monthly velocity.
- Still a prototype/pre-1.0 embed with visible modern-platform gaps and an explicit bank warning.
- Lesson: reuse the engine, but own product hardening and do not translate upstream momentum into a LogansOS compatibility claim.

### Ladybird

- Began as a SerenityOS HTML viewer, grew into a separate browser project, and remains pre-alpha after years of work.
- Reports eight paid full-time engineers and a 2026 developer/early-adopter alpha target; repo history was roughly 78,000 commits at audit.
- Uses per-tab WebContent, RequestServer and fresh ImageDecoder processes with aggressive sandboxing.
- Lesson: building the missing web platform is a program, while the service split is directly relevant to cap-native confinement.

### Blitz / Dioxus

- Credible modular HTML/CSS and native-app renderer with Stylo/Taffy/Parley/Vello; explicit CPU/GPU/hybrid choices.
- Candidly pre-alpha, without JS and without the full browser platform.
- Lesson: excellent bring-up reader and component source, not evidence that a sovereign Chrome replacement is near.

### Boa, Nova and Kiesel

- Language conformance is improving: Boa is above 90% Test262; Nova was 79.9% at its May 2026 dashboard.
- Their own docs expose decisive gaps: experimental status/runtime scope for Boa; performance, RegExp and WASM gaps for Nova; early-stage and mixed Zig/C/Rust reality for Kiesel.
- Lesson: a JS engine is necessary but not sufficient. The browser integration/platform around it is the larger missing object.

### Small/independent OS lesson

Ladybird's origin as an OS HTML viewer is the clearest firsthand example: the reader can prove pixels and fetch early, but modern-web work eventually becomes a dedicated browser organization. LogansOS should exploit that lesson by keeping the scaffold honest and borrowing an active engine rather than allowing the scaffold to acquire an impossible release promise.

## Ratifiable forks for RETURN

### Fork 1 — JS engine / sovereignty posture

| Choice | Meaning | Consequence |
|---|---|---|
| **A. Bounded SpiderMonkey (recommended)** | Servo target; C++ JS only inside cap-confined disposable renderers; no public compat ABI; reproducible source build | only credible 2028-class release route; accepts one carefully governed native component |
| B. Pure-Rust-only | Blitz + Boa/Nova program; no C++ engine | maximizes language purity; moves modern-web ship plausibly 3–5+ years out and may still fail |
| C. Phased without commitment | reader now; defer JS target | preserves optionality but does not create a release path; risks normalizing the scaffold |

**Ratification question:** Is SpiderMonkey acceptable as a bounded component under the explicit containment criteria, subject to the six-week spike, or is source-language purity allowed to move the release gate by years?

### Fork 2 — pull GPU/acceleration forward?

| Choice | Meaning | Consequence |
|---|---|---|
| **A. Yes (recommended)** | software compositor proof, then browser graphics/accel immediately; shared with GUI shelf | preserves the earliest Servo path and unlocks T2/T3 GUI leverage |
| B. No | reader stays CPU; graphics remains later | blocks credible Servo performance/WebGL/WebGPU and therefore blocks first release |

**Ratification question:** Does the browser move the accelerated graphics path immediately behind the software compositor proof? Janus recommends yes.

### Fork 3 — day-1 bring-up scaffold

| Choice | Meaning | Consequence |
|---|---|---|
| **A. Blitz CPU reader + Arthur on same content service (recommended)** | HTML/markdown/simple web, explicit “limited” badge, no general JS claim | exercises the right renderer/network/control seams and can later remain a safe-reader mode |
| B. Terminal-only fetch/render | minimal libneo HTTP + text/markdown in console | cheapest, but proves little of compositor/input/browser integration |
| C. No interim surface | all effort on Servo/platform | avoids distraction but delays user-visible integration feedback |

**Ratification question:** Is 4–8 person-months for the reusable, explicitly limited Blitz reader justified, or should the interim floor stay terminal-only? It must never substitute for the ship gate.

## Kill conditions and re-evaluation triggers

Return to the PL rather than widening scope if any of these fire:

- upstream Servo fails most of the defined modern-site battery even on a supported host;
- Servo/SpiderMonkey requires a general Linux/POSIX personality rather than a bounded adapter;
- reproducible source builds or security-update ownership cannot be established;
- a supported WebRender/graphics path cannot be expressed over the planned cap/fence model;
- renderer isolation cannot close ambient network/filesystem/device authority;
- the 18–24 month calendar estimate exceeds the first-release window after the spike is costed from actual breakpoints.

Pure-Rust engines should be re-evaluated annually or when one demonstrates all three: >95% current Test262, a maintained DOM/Web API integration with a renderer, and passing results on the LogansOS modern-site battery class. Test262 alone is not the trigger.

## Primary sources

### LogansOS

- `docs/bible.md`, Article 1.7 (owner-rooted sovereignty) and Article 5.7 (compatibility boundary)
- `docs/SDK-ABI.md` (static ELF, no PT_TLS/dynamic linking, syscall surface)
- `docs/OPEN-THREADS.md` (OT-61, OT-52, OT-44, OT-57)
- `docs/THINK-2026-07-08-gui-vision-jarvis.md` §§4.1–4.5, 7.4 (current framebuffer/compositor/GPU sequence)
- `docs/THINK-2026-07-05-ot44-sdk-v0.md` §§4–5 (cap bundles, `no_std`, WASM/runtime posture)
- `kernel/src/arch/x86_64/syscall.rs::sys_net_dial_impl` (current ambient dial behavior)
- `userland/libneo/src/action.rs` (`NetDial`/`TcpSocket` userland grant checks)
- `userland/libneo/src/tls.rs` and `docs/SDK-ABI.md` (TLS/HTTP/network substrate)

### Servo / SpiderMonkey

- Servo repository and audited manifests/lock: https://github.com/servo/servo/tree/9c77cd97063589ae64a2d8f5527ff192385f2dfb
- Servo root manifest: https://github.com/servo/servo/blob/9c77cd97063589ae64a2d8f5527ff192385f2dfb/Cargo.toml
- Servo crate manifest: https://github.com/servo/servo/blob/9c77cd97063589ae64a2d8f5527ff192385f2dfb/components/servo/Cargo.toml
- Script/net/paint manifests: https://github.com/servo/servo/tree/9c77cd97063589ae64a2d8f5527ff192385f2dfb/components
- Servo Book, embedding: https://book.servo.org/embedding/overview.html
- Servo crates.io/LTS release posture: https://servo.org/blog/2026/04/13/servo-0.1.0-release/
- Servo May 2026 activity and embedding/API work: https://servo.org/blog/2026/06/30/may-in-servo/
- Servo April 2026 modern-platform work: https://servo.org/blog/2026/05/31/april-in-servo/
- Servo download page and bank warning: https://servo.org/download/
- Servo graphics statement about WebRender GPU use: https://servo.org/blog/2025/08/22/this-month-in-servo/
- Servo MozJS repository/build reality: https://github.com/servo/mozjs

### Blitz / Vello

- Blitz audited repository: https://github.com/DioxusLabs/blitz/tree/e4bc4675e99c17346c024a169f67107eee80a22f
- Blitz README/architecture and explicit scope: https://github.com/DioxusLabs/blitz/blob/e4bc4675e99c17346c024a169f67107eee80a22f/README.md
- Blitz workspace manifest/renderer/network closure: https://github.com/DioxusLabs/blitz/blob/e4bc4675e99c17346c024a169f67107eee80a22f/Cargo.toml
- Blitz WPT status: https://blitz.is/status/wpt
- Blitz CSS/HTML status: https://blitz.is/status
- Vello GPU-compute renderer: https://github.com/linebender/vello

### JS candidates

- Boa audited repository/README: https://github.com/boa-dev/boa/tree/4f98f6446812f1c5923153f8c7e1beda3a891bfe
- Boa v0.21/Test262 report: https://boajs.dev/blog/2025/10/22/boa-release-21
- Boa runtime and WinterTC crates: https://github.com/boa-dev/boa/tree/4f98f6446812f1c5923153f8c7e1beda3a891bfe/core
- Nova audited repository/shortcomings: https://github.com/trynova/nova/tree/4eea7c6fae180a8a2eed45c3f6acd0d9256522b1
- Nova Test262 dashboard: https://trynova.dev/test262
- Kiesel audited repository/README: https://codeberg.org/kiesel-js/kiesel/src/commit/7e6cc72f2fae056ed88b1fa57c62a98877f2960a

### Ladybird

- Ladybird audited repository/README: https://github.com/LadybirdBrowser/ladybird/tree/1fb86929fde87049f617eca81793fc667142ffc6
- Ladybird process architecture: https://github.com/LadybirdBrowser/ladybird/blob/1fb86929fde87049f617eca81793fc667142ffc6/Documentation/ProcessArchitecture.md
- Initiative history and unfinished-state statement: https://ladybird.org/posts/announcement/
- 2018 origin / HTML-viewer history: https://ladybird.org/posts/fork/
- team size and alpha target: https://ladybird.org/
- current pre-alpha/security posture: https://ladybird.org/posts/changing-how-we-develop-ladybird/

## Close note

This artifact is documentation-only. No code or QEMU run is appropriate. The dispatch requires the local host close set with docs-only skips noted, followed by a diff of this artifact against the dispatch; those results belong in the RETURN.
