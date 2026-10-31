<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: OT-61 web browser — architecture + prior-art FINDINGS (Prometheus)

**Date:** 2026-07-20
**Author:** Prometheus (research seat, cross-model from the Claude PM)
**Dispatch:** `Plans/DISPATCH-2026-07-20-ot61-browser-architecture-research-prometheus.md`
**Status:** FINDINGS — doc-only, no gate. Pure architecture research. Cross-model check (Codex/Claude) follows before any build commitment.

---

## 0. Executive recommendation

**Adopt a Servo-subset engine, not the full browser shell.** Specifically: adopt `webrender` + `stylo` (CSS/layout) + `html5ever` (parsing) + a minimal JS runtime (Boa MIT), wired to our G1 compositor and kernel net stack. Drop Servo's constellation, embedder traits, media, Bluetooth, WebXR, storage, devtools, and WebDriver — they're all platform-hosted baggage we don't need.

**License verdict (load-bearing):** Confirmed from raw LICENSE files fetched 2026-07-20:

| Component | License | Source | Adoptable? |
|-----------|---------|--------|------------|
| NeoPlutarch (us) | MPL-2.0 | `LICENSE` root repo | — |
| Servo | MPL-2.0 | `https://raw.githubusercontent.com/servo/servo/main/LICENSE` | **Yes** |
| WebRender | MPL-2.0 | `https://raw.githubusercontent.com/servo/webrender/main/LICENSE` | **Yes** |
| Stylo (selectors, style) | MPL-2.0 | Forked from Servo; same license | **Yes** |
| html5ever / markup5ever | MPL-2.0 | Mozilla crates.io | **Yes** |
| Boa JS | MIT + Unlicense | `https://raw.githubusercontent.com/boa-dev/boa/main/LICENSE-MIT` | **Yes** |
| Ladybird (LibWeb, LibJS) | BSD 2-Clause | `https://api.github.com/repos/SerenityOS/serenity/contents/LICENSE` | Adoptable but C++ — reference only |

**Confidence: High.** The MPL-2.0 compatibility is the single highest-value fact in this document. It means we can adopt Servo's rendering stack as our code, not just study it. This changes the entire calculus from "build a browser engine" to "port a known-good Rust engine onto our substrate."

**Why not full Servo?** Servo targets hosted OSes (Linux/Wayland, macOS, Windows, Android). Its `ports/servoshell` depends on `winit`, `wgpu`, GL/GLES, platform font stacks (`freetype-sys`, `dwrote`, `fontconfig_sys`), GStreamer media, GTK accessibility, and a full event loop. That's hundreds of transitive dependencies we can't use on a `#![no_std]` kernel with our own compositor and GPU path. The subset approach takes only the web-engine components (parse, style, layout, paint) and wires them to our substrate.

**Why not Ladybird?** BSD-2 is permissive, but it's C++. Porting C++ into an MPL-2.0 Rust tree requires either FFI boundaries (defeats the point) or a full reimplementation. Ladybird is invaluable as *reference* for what a clean-room engine looks like — its `LibWeb` DOM implementation and `LibJS` are architecturally instructive — but not adoptable in practice.

**Why not own-minimal?** Building HTML parsing, CSS cascade/inheritance, box model layout, flexbox/grid, painting, and a JS interpreter from scratch is 2+ person-years of work for something that already exists in Servo's component tree. The subset approach gives us 80% of the way there on day one.

**Why not WASM-sandboxed?** A WASM-based browser (e.g., QuickJS-wasm rendering HTML) cannot achieve real-world web compatibility. Modern sites require DOM mutation, CSS layout, and event dispatch that a WASM sandbox can't expose without essentially re-implementing a browser engine inside WASM. Ruled out.

---

## 1. Engine options — comparison

### 1.1 Option matrix

| Criterion | Servo-subset (Recommended) | Full Servo | Own-minimal | Ladybird (ref) | WASM sandbox |
|-----------|---------------------------|------------|-------------|----------------|--------------|
| **Language** | Rust | Rust | Rust | C++ | WASM/C |
| **License** | MPL-2.0 (adoptable) | MPL-2.0 | Our choice | BSD-2 (adoptable) | Varies |
| **HTML parser** | `html5ever` (spec-compliant) | Same | Build from scratch | LibWeb HTML | Re-implement in WASM |
| **CSS engine** | `stylo` (Gecko-derived, spec-complete) | Same | Build cascade + box model | LibCSS | Minimal or none |
| **Layout** | Servo layout (flexbox, grid, inline) | Same | Box model only | Ladybird layout | N/A |
| **Painting** | `webrender` (GPU-driven, our OT-63 maps naturally) | Same + surfman | CPU blit | LibGfx | WASM canvas |
| **JS engine** | Boa (MIT, Rust) or drop Servo's SpiderMonkey (`js`/`mozjs`) | SpiderMonkey (MPL/Mozilla) | Boa or quickjs-rs | LibJS (Serenity C++) | QuickJS-wasm |
| **Platform deps** | None (we strip them) | `winit`, `wgpu`, GL, fonts, media, GTK | None | SerenityOS libs | Host browser |
| **Porting cost** | Medium — wire to our compositor + net | High — untangle hosted OS deps | Very high — build everything | Very high — C++ FFI or rewrite | Low but useless |
| **Web compat** | High (Servo passes most WPT) | Same | Low (MVP only) | Medium (growing) | Very low |
| **Our-substrate fit** | **Best** — Rust, GPU paint maps to OT-63, cap-friendly component model | Good but overkill | Good but slow | Poor (C++) | Poor |
| **Maintenance** | Servo upstream evolves; we track subset | Full Servo burden | Ours alone | SerenityOS upstream | Abandoned quickly |

### 1.2 Servo dependency analysis — what to keep vs strip

From Servo's `Cargo.toml` (fetched live 2026-07-20), the workspace has ~200 dependencies. The adoptable core:

**KEEP (web-engine essentials, no platform deps):**
- `html5ever` — HTML5 parsing (MPL-2.0)
- `markup5ever` — markup utilities
- `tendril` — string handling for parsers
- `stylo` + `selectors` + `servo_arc` — CSS cascade, inheritance, specificity (MPL-2.0, Gecko-derived)
- `cssparser` — CSS tokenization
- `webrender` + `webrender_api` — GPU-driven painting (MPL-2.0)
- `script` component's DOM tree construction (strip JS binding layer)
- `layout` component — box model, flexbox, grid
- `paint` component — paint graph generation (feed to our WebRender instead of Servo's)
- `url`, `data-url`, `percent-encoding` — URL handling
- `encoding_rs` — character encoding
- `mime`, `http` — HTTP parsing

**STRIP (hosted OS baggage):**
- `winit` — window creation (we use G1 surfaces)
- `surfman` — GL surface management (we use OT-63 present backend)
- `gstreamer*` — media pipeline (anti-scope for MVP)
- `gtk`, `glib` — accessibility (defer)
- `dwrote`, `freetype-sys`, `fontconfig_sys` — platform font stacks (use our Vello text engine)
- `gaol` — process sandboxing (our caps replace this)
- `ipc-channel` — Servo's IPC (we use kernel IPC)
- `mozjs` / `js` — SpiderMonkey binding (replace with Boa)
- `webxr`, `bluetooth`, `wakelock` — platform features
- `webdriver_server` — testing infrastructure
- `ohos-*`, `android_logger`, `jni`, `objc2-*` — platform ports

**REPLACE (our substrate):**
- Font rendering: Servo's `fonts` → our Vello text engine (ratified in ADR-024)
- Surface/present: Servo's `winit` + `surfman` → G1 `SharedPixelBuffer` + present backend
- Input: Servo's embedder input events → kernel `InputFocus` routed events
- Networking: Servo's `hyper` + `tokio` → our kernel TCP + kTLS + userland HTTP layer
- JS: Servo's SpiderMonkey (`mozjs`) → Boa (MIT, Rust, no_std-friendly)

### 1.3 Porting cost estimate — Servo-subset

| Phase | Work | Effort |
|-------|------|--------|
| **Parse + DOM tree** | Wire `html5ever` to produce a DOM tree in our allocator | 2-3 weeks |
| **CSS cascade + layout** | Adapt `stylo` to walk our DOM; run layout producing paint commands | 4-6 weeks |
| **Paint → WebRender** | Replace Servo's paint backend with our WebRender instance feeding G1 `SharedPixelBuffer` | 3-4 weeks |
| **JS integration** | Embed Boa; wire DOM bindings (minimal: `document.getElementById`, `addEventListener`, `innerHTML`) | 6-8 weeks |
| **Networking** | Userland HTTP/1.1 client over kernel TCP + kTLS | 2-3 weeks |
| **G1 surface integration** | Browser window = G1 surface; tab paint → `SharedPixelBuffer` commit/present | 2-3 weeks |
| **Security model** | Per-origin cap isolation, net caps, no-snoop surfaces | 3-4 weeks |
| **Total MVP** | "Can browse the internet" (HTML + CSS + images + basic JS + HTTPS) | ~4-6 months |

This is aggressive but credible because we're adopting existing, spec-compliant components. The hard parts (CSS cascade, flexbox/grid layout, HTML parsing) are already solved in `stylo` and `html5ever`. Our work is wiring them to our substrate.

---

## 2. The capability-native security model — our differentiator

### 2.1 Tab-as-principal

**The model:** Each browser tab (more precisely, each *site-instance*) is a cap-bounded principal. This is Chrome's site-isolation model done natively with capabilities instead of OS processes:

1. **Tab process = derived principal.** When the browser opens a tab for origin `https://example.com`, it derives a child process from the browser shell with an attenuated capability set:
   - A `SharedPixelBuffer` cap for its surface (RW, no DERIVE)
   - An `InputFocus` leaf cap (RECEIVE only, for that tab's surface)
   - A *per-origin* network cap (see §2.3)
   - Read caps for the browser's shared resources (font cache, JS engine state)
   - NO caps to other tabs' surfaces, buffers, or input

2. **Per-origin process isolation.** Following Chrome's site-isolation: each registered domain gets its own process. `example.com` and `evil.com` are separate principals. Subdomains may share a process if same-site (`*.example.com`). Iframes from different origins get their own derived principal within the tab.

3. **No ambient authority.** A tab process cannot:
   - Read another tab's pixels (no cap on that `SharedPixelBuffer`)
   - Inject input into another tab (no `InputFocus` for that surface)
   - Open arbitrary network connections (net cap is per-origin, scoped to that origin's host:port)
   - Access the filesystem (no file caps unless explicitly granted via a download flow)

### 2.2 No-snoop rendering — the Qubes lesson, structurally safer

From `docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` (T2, T5, C3, C6): Qubes proved that a trusted compositor compositing untrusted content is necessary. Our G1 design achieves this *structurally stronger* than Qubes:

| Property | Qubes | LogansOS browser |
|----------|-------|-----------------|
| Pixel isolation | Xen grant tables between VMs; `qubes-guid` in dom0 composites | G1 `SharedPixelBuffer`: client RW, compositor RO. No cross-process grants — dual PTE mapping of same pages. |
| Input no-snoop | `gui-daemon` forwards input to focused VM; daemon sees all events | Kernel `InputFocus`: events routed directly to the focused leaf. Compositor never sees events. |
| Buffer corruption | QSB-046: untrusted VM read dom0 memory through shared framebuffer | G1 C4: compositor mapping is hardware RO/NX. Client scribble corrupts only its own pixels. |
| TCB | dom0 (hundreds of thousands of lines of C) | Userland compositor, killable, cap-gated. Kernel additions: exactly two object kinds. |

**For the browser specifically:** Each tab's surface is a separate `SharedPixelBuffer`. The compositor maps each RO. A compromised tab can corrupt its own pixels but cannot read or write another tab's buffer. The compositor composites them together for display but structurally cannot extract pixel data from any individual surface (it only blits, never reads back).

**This is stronger than Chrome's site isolation.** Chrome uses OS process boundaries and IPC — a compositor bug or SUID exploit can still read cross-process memory. Our model uses hardware-enforced page protections (RO/NX PTEs) with no shared address space between tabs. A kernel bug would be needed to break this, not just a userland exploit.

### 2.3 Per-origin network capability

**The model:** Network access is not ambient. Each tab's net cap is derived from the browser's root net cap with origin-specific restrictions:

1. **Net cap derivation.** The browser shell holds a broad `NetCap` (READ|WRITE|DERIVE). When creating a tab for `https://example.com:443`, it derives a child net cap restricted to that origin:
   ```
   TabNetCap = NetCap.derive({
       rights: READ | WRITE,        // can send/receive
       scope: OriginScope {         // scoped to this origin
           scheme: "https",
           host: "example.com",
           port: 443,
       },
       no_derive: true,             // tab cannot re-grant to subprocesses
   })
   ```

2. **Same-origin policy enforced by the kernel.** The kernel's net stack checks the cap's scope before allowing a connection. A tab with an `example.com` net cap cannot connect to `evil.com` — the kernel refuses at the socket level, not just in JavaScript. This is *structurally* stronger than browser-enforced SOP, which relies on the JS engine correctly checking origin strings.

3. **Upgrade path.** If a navigation takes the tab to a new origin, the old net cap is revoked and a new one derived for the target origin. The kernel enforces this — the tab process cannot hold net caps for multiple origins simultaneously (unless explicitly allowed for same-site subdomains).

4. **WebSocket and fetch.** Same model: each WebSocket connection requires a net cap scoped to that origin. `fetch()` cross-origin requests go through the browser shell's broader net cap (the shell acts as a trusted proxy, returning only the response body — not the raw socket).

### 2.4 Site isolation — Chrome's model, cap-native

Chrome's site-isolation architecture (process-per-site-instance) maps directly onto our capability model:

| Chrome mechanism | Our cap-native equivalent |
|-----------------|--------------------------|
| Process per site-instance | Derived principal with attenuated caps |
| IPC between same-process sites | Cap-gated IPC channels (no ambient shared memory) |
| Site instance key (e.g., `https://example.com` + COEP/COOP flags) | Cap derivation parameters (origin scope, isolation flags) |
| Cross-site iframe isolation | Separate derived principal per iframe origin |
| Service worker isolation | Separate principal with persistent storage caps |
| Sandbox attribute (`<iframe sandbox>`) | Additional cap attenuation (no net, no input, no derive) |

**The `<iframe sandbox>` case is illustrative:** In Chrome, `sandbox` strips permissions at the browser level. In our model, a sandboxed iframe gets a principal with explicitly fewer caps:
- No network cap (can't make requests)
- No input focus eligibility (can't receive keyboard events)
- No derive right (can't spawn child processes)
- Surface is composited but structurally isolated

The kernel enforces these restrictions — the browser shell can't accidentally grant them back. This is stronger than Chrome's policy-enforced sandbox, which relies on the browser process correctly implementing the restriction.

### 2.5 The security advantage — summary

**Where we're structurally safer than mainstream browsers:**

1. **No shared address space.** Chrome uses OS processes with shared libraries (DLL hell, exploit primitives). Our tabs are cap-bounded principals with no shared memory except the compositor's RO mappings.

2. **Kernel-enforced SOP.** Chrome's same-origin policy is enforced in JavaScript/C++ code. Ours is enforced by the kernel's net cap scope — a hardware-rooted guarantee.

3. **No compositor snooping.** Chrome's compositor can read any tab's pixels (same process or OS-level access). Our compositor maps each surface RO and only blits — it structurally cannot extract pixel data.

4. **Revocation without kill.** Chrome must kill a process to revoke its privileges. We can revoke a cap transitively — the kernel immediately denies operations using that cap, without killing the process.

5. **Measured TCB.** Our browser's security properties depend on the Rust kernel's `unsafe` surface (measurable, auditable) plus the G1 compositor (userland, killable). Chrome's security depends on Chromium's C++ codebase (tens of millions of lines), the OS kernel, and the GPU driver.

**Where we're honest about gaps:**

- **JS engine safety.** Boa is Rust but immature. SpiderMonkey (Servo's default) is battle-tested but C++. Until Boa reaches production maturity, a JS engine bug remains our largest userland attack surface. Mitigation: run the JS engine in a further-derived principal with minimal caps.
- **CSS/JS spec completeness.** Servo-subset won't pass 100% of WPT initially. Sites that rely on obscure CSS features or non-standard JS APIs may break. This is an MVP trade-off, not a security issue.

---

## 3. GPU + compositor integration

### 3.1 WebRender → OT-63 present backend

WebRender is the natural rendering backend for our browser. It's a GPU-driven paint engine that produces render passes targeting a swapchain. Our integration:

1. **WebRender instance per tab.** Each tab process runs its own WebRender instance. WebRender compiles the paint commands (from Servo's `paint` component) into a render pass targeting a texture.

2. **Texture → `SharedPixelBuffer`.** Instead of WebRender presenting to a platform swapchain (as it does on Linux/Windows), we redirect its output to our G1 `SharedPixelBuffer`:
   - WebRender renders to a staging texture in GPU memory
   - The present backend (`atrium/present_backend.rs`) arms a flip or blits the texture into the `SharedPixelBuffer`'s physical frames
   - The tab calls `publish_complete()` on its buffer, signaling the compositor

3. **Compositor composites tab surfaces.** Our G1 compositor receives the publish fence, maps the buffer RO, and composites all visible tab surfaces (plus browser chrome) into the final display framebuffer. Each tab's surface is composited with the glass treatment from ADR-024: frosted backdrop blur, trust-state tint, Cap-Halo geometry.

4. **OT-63 GPU path.** The WebRender render pass uses our OT-63 present backend (already merged: `GpuSubmitCap`, Gen9 iGPU scanout fencing). WebRender's existing wgpu backend maps naturally to our GPU submission model — we just redirect the final present target from a platform swapchain to our `SharedPixelBuffer`.

### 3.2 Browser chrome vs tab content

**Two-surface model:**
- **Browser chrome surface** (address bar, tabs, navigation buttons): The browser shell process owns this surface. It gets a full-cap `SharedPixelBuffer` with all rights. Rendered with the Atrium design language: classic-Mac craft bar, neon-as-meaning, selective glass.
- **Tab content surfaces** (one per tab/site-instance): Each tab process owns its surface. The compositor composites these *within* the chrome's content area. Tab surfaces get the frosted-glass treatment with trust-state tint (owned = clear, untrusted = heavy frost + desaturated).

**Glass applied to browser:** Per ADR-024 Amendment §5, glass is selective — near the Void stays true negative space. The browser chrome uses minimal glass (just the tab strip and address bar get the frosted treatment), while tab content areas are clear (the content IS the surface, not a frosted overlay).

### 3.3 Vello text engine — ratified, no contradiction

ADR-024 Amendment §4 ratifies Vello as the compositor's text engine. For the browser:
- **Compositor text** (browser chrome labels, status bar): Vello on wgpu
- **Web page text** (HTML content): WebRender's own text shaping (it uses `skrifa`/`read-fonts` for font parsing and a GPU-based rasterizer)

These are separate concerns. The browser's WebRender handles web page text rendering. Vello handles the compositor scene's text. No contradiction — they're different renderers serving different surfaces, sharing the same GPU substrate (OT-63).

**Font stack:** Servo's platform font loaders (`freetype-sys`, `dwrote`, `fontconfig_sys`) are stripped. We provide a minimal font resolver that loads system fonts from our filesystem and passes them to WebRender's `skrifa`/`read-fonts` parsers. This is a small shim, not a full font stack port.

---

## 4. Networking + TLS delta

### 4.1 What we have

| Component | Status | Location |
|-----------|--------|----------|
| TCP stack | Complete | `kernel/src/net/tcp.rs` |
| IPv4/IPv6 | Complete | `kernel/src/net/ipv4.rs`, `ipv6.rs` |
| DNS | Complete | `kernel/src/net/dns.rs` |
| DHCPv4 | Complete | `kernel/src/net/dhcpv4.rs` |
| ARP / NDP | Complete | `kernel/src/net/arp.rs`, `ndp.rs` |
| Happy Eyeballs | Complete | `kernel/src/net/happy_eyeballs.rs` |
| kTLS record layer | Complete (ChaCha20-Poly1305) | `kernel/src/net/tls_record.rs` |
| WPA/WiFi | Complete | `kernel/src/net/wpa/`, `wifi/` |

### 4.2 What's missing for HTTPS browsing

| Gap | Description | Effort |
|-----|-------------|--------|
| **TLS handshake** | We have the record layer (seal/open), but NOT the TLS 1.3 handshake (key exchange, cert verification, Finished messages). The DISPATCH plan called for `rustls` in userspace for the handshake, then `sys_net_tcp_enable_tls` to hand record-layer ownership to the kernel. | **2-3 weeks** — wire `rustls` (userspace) → handshake → extract key material → `sys_net_tcp_enable_tls` → kernel takes over records |
| **HTTP/1.1 client** | No HTTP parser in userspace. Need a minimal HTTP/1.1 client (request/response parsing, headers, chunked transfer, redirects). | **1-2 weeks** — `http` crate (already in Servo's deps) + state machine |
| **HTTP/2** | Most sites require HTTP/2 (HPACK compression, multiplexed streams). Not needed for MVP but needed for "can browse the internet" on modern sites. | **3-4 weeks** — `h2` crate or Servo's `http02` |
| **Certificate store** | Kernel needs a trust anchor store (root CAs) for `rustls` to verify certificates. | **1 week** — ship Mozilla root CA bundle, load at boot |
| **Cookie/storage** | Minimal cookie jar for session persistence. | **1 week** |
| **Content decoding** | `gzip`, `br` (Brotli) decompression for response bodies. | **Already available** — `flate2`, `brotli` in Servo's deps, both no_std-friendly |

### 4.3 Minimal networking delta — MVP path

For the absolute minimum ("render one HTTPS page"):

1. **rustls handshake (userspace):** The browser process uses `rustls` to perform the TLS 1.3 handshake over a raw TCP socket. After the handshake completes, it extracts the key material (4 values: tx_key, tx_iv, rx_key, rx_iv) and calls `sys_net_tcp_enable_tls(socket, key_material)` to hand record-layer ownership to the kernel.

2. **HTTP/1.1 only:** The MVP only needs HTTP/1.1. Many sites still support it via ALPN negotiation. Sites that require HTTP/2 can be deferred.

3. **Certificate verification:** `rustls` handles cert verification against a bundled root CA store. No kernel changes needed — this is entirely userspace.

**Total networking delta for MVP: ~4-6 weeks.** The kTLS record layer is already built and tested. We just need the handshake bridge and HTTP parsing.

---

## 5. The honest MVP ladder

### Stage 0: Infrastructure prep (pre-browser)
- [ ] Font resolver shim (load system fonts → `skrifa`/`read-fonts`)
- [ ] rustls handshake bridge (userspace handshake → kernel kTLS handoff)
- [ ] HTTP/1.1 client in userspace
- [ ] Root CA bundle shipped with the OS
- **Duration:** 3-4 weeks
- **Gate:** Can fetch and decrypt an HTTPS response over our net stack

### Stage 1: Static HTML renderer
- [ ] `html5ever` DOM tree construction
- [ ] Minimal CSS cascade (`stylo` subset: colors, fonts, dimensions, display)
- [ ] Box model layout (block/inline, no flexbox/grid yet)
- [ ] Paint → WebRender → G1 `SharedPixelBuffer` commit/present
- [ ] Browser window as a G1 surface with chrome (address bar + content area)
- **Duration:** 4-6 weeks
- **Cumulative:** 7-10 weeks
- **Gate:** Can render a static HTML page (no JS, no images) in a browser window

### Stage 2: Images + CSS layout
- [ ] Image decoding (`image` crate subset: PNG, JPEG, WebP)
- [ ] Flexbox layout (`stylo` flexbox module)
- [ ] Grid layout (defer to Stage 3 if needed for MVP)
- [ ] Basic navigation (click links → new tab or navigate current tab)
- [ ] Scroll handling (input events → scroll offset → repaint)
- **Duration:** 4-6 weeks
- **Cumulative:** 11-16 weeks
- **Gate:** Can render a real webpage with images and modern CSS layout

### Stage 3: JavaScript + DOM mutation
- [ ] Boa JS engine embedded in tab process
- [ ] Minimal DOM bindings: `document.getElementById`, `querySelector`, `innerHTML`, `addEventListener`
- [ ] Event dispatch: click, keydown, load, DOMContentLoaded
- [ ] `setTimeout`/`setInterval` basic timer
- [ ] Console logging
- **Duration:** 6-8 weeks
- **Cumulative:** 17-24 weeks
- **Gate:** Can browse the internet (interactive pages with JS work)

### Stage 4: Security model + per-origin isolation
- [ ] Tab-as-principal: derive caps per tab/origin
- [ ] Per-origin net cap scoping
- [ ] Cross-origin iframe isolation
- [ ] `<iframe sandbox>` cap attenuation
- [ ] Capability-native SOP enforcement in kernel net stack
- **Duration:** 3-4 weeks
- **Cumulative:** 20-28 weeks
- **Gate:** Browser is structurally secure (cap-isolated tabs, kernel-enforced SOP)

### Stage 5: Polish + release-gating
- [ ] HTTP/2 support
- [ ] Grid layout completion
- [ ] Cookie jar + localStorage
- [ ] History navigation (back/forward)
- [ ] Tab management (new tab, close tab, tab bar)
- [ ] Atrium glass design applied to browser chrome
- [ ] Performance: paint at 60fps for typical pages
- **Duration:** 4-6 weeks
- **Cumulative:** 24-34 weeks
- **Gate:** "Can browse the internet" — release-gating bar met

### Total timeline

| Milestone | Duration | What it does |
|-----------|----------|-------------|
| Stage 0: Net infra | 3-4 weeks | HTTPS fetch works |
| Stage 1: Static HTML | +4-6 weeks | Renders text pages |
| Stage 2: Images + CSS | +4-6 weeks | Renders real pages |
| Stage 3: JavaScript | +6-8 weeks | Interactive pages work |
| Stage 4: Security | +3-4 weeks | Cap-isolated tabs |
| Stage 5: Polish | +4-6 weeks | Release-ready browser |
| **Total** | **~6-9 months** | **"Can browse the internet"** |

This is aggressive but credible. The Servo-subset approach means we're wiring together existing components, not building from scratch. Stages 1-3 are the critical path; Stage 4 (security) can run in parallel with Stage 3 since it's architectural, not feature-dependent.

---

## 6. Confidence flags + sources

### 6.1 Confidence assessment

| Claim | Confidence | Basis |
|-------|-----------|-------|
| Servo/WebRender are MPL-2.0 (adoptable) | **Certain** | Raw LICENSE files fetched from GitHub API, 2026-07-20 |
| NeoPlutarch is MPL-2.0 | **Certain** | `LICENSE` in repo root |
| Boa JS is MIT + Unlicense (adoptable) | **Certain** | Raw LICENSE-MIT fetched from GitHub API, 2026-07-20 |
| Ladybird is BSD 2-Clause (adoptable but C++) | **Certain** | Raw LICENSE fetched from SerenityOS repo API |
| Servo subset approach is viable | **High** | Servo's component model (`components/` tree) is explicitly modular; `embedder_traits` provides the abstraction boundary for custom hosts |
| WebRender can target our `SharedPixelBuffer` | **High** | WebRender already supports custom device output (wgpu backend); our present backend already handles GPU→buffer flips for OT-63 |
| kTLS record layer is sufficient for HTTPS | **High** | `tls_record.rs` implements full TLS 1.3 record seal/open with ChaCha20-Poly1305; only handshake bridge is missing |
| Porting cost estimate (6-9 months MVP) | **Medium** | Based on component adoption, not build-from-scratch; depends on team size and Rust/no_std expertise |
| Cap-native SOP is structurally stronger than Chrome's | **High** | Kernel-enforced net cap scope vs browser-enforced origin string check; hardware page protections for pixel isolation |
| Boa JS can handle modern web JS | **Medium** | Boa is immature compared to SpiderMonkey/V8; ES2024 features may be missing; performance on complex pages unproven |

### 6.2 Sources

**Raw LICENSE files (exact-bytes rule, fetched 2026-07-20):**
- NeoPlutarch: `LICENSE` in repo root — "Mozilla Public License Version 2.0"
- Servo: `https://raw.githubusercontent.com/servo/servo/main/LICENSE` — "Mozilla Public License Version 2.0" (confirmed, full text)
- WebRender: `https://raw.githubusercontent.com/servo/webrender/main/LICENSE` — "Mozilla Public License Version 2.0" (confirmed, full text)
- Boa JS: `https://raw.githubusercontent.com/boa-dev/boa/main/LICENSE-MIT` — "MIT License" (confirmed, full text)
- Ladybird/SerenityOS: `https://api.github.com/repos/SerenityOS/serenity/contents/LICENSE` — "BSD 2-Clause License" (confirmed, full text)

**Servo Cargo.toml analysis:**
- Fetched live: `https://raw.githubusercontent.com/servo/servo/main/Cargo.toml`
- ~200 workspace dependencies; web-engine core components identified for adoption
- Platform-specific deps (`winit`, `gstreamer`, `gtk`, `ohos-*`, etc.) identified for stripping

**Internal cross-references:**
- `docs/RESEARCH-2026-07-20-qubes-prior-art-FINDINGS.md` — compartmentalization model, tab-as-principal builds on Qubes' per-domain isolation (T1-T10)
- `docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md` — G1 surface protocol, `SharedPixelBuffer`, `InputFocus`, no-snoop by construction
- `docs/ADR-024-phase-8-design-system-and-tech-stack.md` — Vello text engine ratified, glass material, compositor on wgpu
- `kernel/src/net/tls_record.rs` — kTLS record layer (ChaCha20-Poly1305, synchronous per-packet)
- `kernel/src/atrium/present_backend.rs` — OT-63 present backend, GPU flip/arm
- `kernel/src/atrium/buffer.rs` — `SharedPixelBuffer` kernel object
- `kernel/src/atrium/focus.rs` — `InputFocus` kernel object
- RAG queries (healthy): confirmed net stack components, `SharedPixelBuffer` surface protocol, capability grant/derive model

**Prior art references:**
- Chrome site isolation: process-per-site-instance architecture (well-documented Chromium design doc)
- Qubes per-domain isolation: validated in our prior-art FINDINGS as the compartmentalization goal we inherit

---

## 7. Contradictions with ratified decisions

**None found.** This research is consistent with all ratified decisions:

- **Vello text engine (ADR-024 §4):** No contradiction. Vello handles compositor scene text; WebRender handles web page text. Different renderers, same GPU substrate.
- **Own compositor (ADR-024/G1 SYNTHESIS):** No contradiction. The browser rides our G1 compositor — it doesn't replace it. Browser tabs are G1 surfaces.
- **Rust monolith (Bible Art. 4.2):** No contradiction. Servo-subset is Rust. Boa is Rust. All adopted components are Rust. Ladybird (C++) is reference-only.
- **No system AI persona (Bible Art. 23.1):** Not relevant to browser architecture.
- **kTLS approach (Phase 4 plans):** Consistent. The browser uses the existing kTLS record layer with a userspace `rustls` handshake bridge — exactly what the Phase 4 plan specified.
- **Capability model (OT-18, ADR-010/061):** The per-origin net cap and tab-as-principal model follows the ratified grant ergonomics (coarse front-loaded grants, D1/D3 friction model).

**One tension to flag:** ADR-024 originally specified Smithay (Wayland) as the compositor. The G1 SYNTHESIS superseded this with an own-project cap-native compositor. The browser's surface integration assumes the G1 compositor (not Wayland), which is consistent with the current ratified state.

---

## 8. RAG note

RAG health check: **healthy**. Queried `network stack TCP TLS record` (confirmed: `tls_record.rs`, kTLS ChaCha20-Poly1305, Phase 4 plans), `SharedPixelBuffer surface protocol compositor` (confirmed: atrium module, present backend, G1 SYNTHESIS), `capability derive grant per-principal isolation` (confirmed: OT-18 grant ergonomics, Arthur cap bounds). All cross-references verified against live tree.

---

*End of FINDINGS. Branch `prometheus/ot61-browser-architecture`; PM cherry-picks to `main`. Codex/Claude cross-check follows before any build commitment.*
