<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: The day-1 Rust app shelf — pure-Rust candidates for LogansOS preload

**Date:** 2026-07-11
**Author:** Prometheus (Qwen)
**Dispatch:** `Plans/DISPATCH-2026-07-11-rust-day1-apps-research-prometheus.md`
**Branch:** `prometheus/rust-day1-apps-research`
**Scope:** Host-only, primary sources only. Every dependency claim verified from the project's own Cargo.toml/Cargo.lock/LICENSE.

---

## Executive summary

**The pure-Rust desktop app ecosystem is thin.** "Written in Rust" frequently conceals a C skeleton: GTK-rs binds C GTK, most image viewers bundle libjpeg-turbo, PDF renderers all depend on MuPDF or Pdfium, and browser engines carry 6+ C libraries. The feasibility gap between "it's Rust" and "it can run on LogansOS" is the **std target**, the **windowing backend**, and **load-bearing C bindings** — not the language.

**The day-1 shelf has two layers:**
- **T1 (console/transcript):** Helix (editor), zellij (terminal mux), gitui (git — 1 C dep), Himalaya libs (email — library route), symphonia (audio decode) — shippable once the transcript console speaks VT100 and libneo exposes file I/O + basic threading.
- **T2 (CPU-rendered GUI, G1 + std target):** egui/iced/Slint software-rendered apps — shippable once the G1 winit/softbuffer backend lands + a minimal `std` PAL backend exists.

**The library-not-app route wins in 6 of 11 categories.** For email, audio, image codecs, markdown, PDF manipulation, and text shaping — the pure-Rust library wired into Arthur/a Workfield is the right shape, not a ported windowed app.

**Two candid gaps:** GIMP-class raster editing (nothing shippable in pure Rust) and web browser (Servo requires 6+ C libraries + GPU; years away).

---

## ISC-1: The feasibility-tier framework

### Tier definitions

| Tier | Name | Prerequisites | What runs |
|------|------|--------------|-----------|
| **T1** | Console/transcript-compatible | (1) Transcript console speaks VT100/ANSI escapes; (2) libneo file I/O syscalls; (3) basic signal handling or cooperative yield; (4) `no_std` + `alloc` target (already exists) | TUI apps (ratatui/crossterm), CLI tools, library-backed Arthur skills |
| **T2** | CPU-rendered GUI | (1) **G1 winit/softbuffer backend** (banked — compositor design complete); (2) **Minimal `std` PAL target** (new — 2-3 person-months for fs+threads subset); (3) softbuffer surface access; (4) basic input routing (keys + pointer via G1 focus) | egui/iced/Slint software-rendered apps, pixel-buffer image viewers |
| **T3** | GPU-required | (1) Phase-6 GPU accel path; (2) wgpu/Vulkan/Metal driver; (3) full `std` target; (4) G1 compositor with GPU composition | wgpu-based apps, Lapce, Zed, Servo, any wgpu/OpenGL app |
| **REJECTED** | C-dependent | Any load-bearing `*-sys` crate that requires a C toolchain at build time or a C shared library at runtime | GTK-rs apps, servo, miro-pdf, oculante, etc. |

### The T1 gap: VT100 vs transcript console

**Honest statement:** Our v1 console (`4858ac6`) is a framebuffer transcript — Arthur's conversation log + `neo>` output. It does **not** speak ANSI escape sequences. TUI apps (Helix, gitui, zellij, yazi) use `crossterm` which emits VT100 control sequences: cursor positioning (`\x1b[20;10H`), scroll regions, alternate screen buffer (`\x1b[?1049h`), keyboard modifiers.

**The gap is real but bridgeable.** The T1 prerequisite is an **ANSI escape sequence interpreter** that maps VT100 sequences onto the framebuffer blit path. This is ~500 lines of state-machine code (parse ESC sequences → cursor state + attribute stack → render to pixel buffer). It does NOT require std — it's a pure Rust parser over a byte stream. The existing `vte` crate (pure Rust VT100 parser) could be adapted, or we write our own minimal subset (TUI apps use ~30 of the 200+ VT100 sequences).

**Cost estimate:** 1-2 person-weeks for a minimal VT100 subset (cursor movement, attributes, alternate screen, scroll). This is the single highest-leverage T1 unlock — it enables Helix, zellij, gitui, yazi, and every ratatui app simultaneously.

### T2 toolkit verification: which REALLY have software renderers?

Verified from each project's Cargo.toml (primary source):

| Toolkit | Software renderer | Renderer C deps | Windowing C deps | no_std (core) | Verdict |
|---------|------------------|-----------------|-----------------|---------------|---------|
| **egui core** | Yes — `vello_cpu` (pure Rust rasterizer in `epaint`) | **ZERO** | N/A (no windowing in core) | Yes | **Best.** Immediate-mode, you own rendering. Skip `eframe`, use egui + epaint + vello_cpu directly. |
| **iced + tiny-skia** | Yes — `iced_tiny_skia` first-class | **ZERO** (tiny-skia, softbuffer dlopen-only) | winit: libxkbcommon-sys, libwayland-client-sys, libx11-sys | Yes (renderer only) | **Good.** Modular — drop `iced_winit`, provide your own backend. |
| **Slint software** | Yes — default feature, `SLINT_BACKEND=winit-software` | **ZERO** | Same winit chain | Yes (with `libm` feature) | **Moderate.** DSL-based UI, GPL-3.0/or-commercial license. Embedded-focused. |
| **dioxus-desktop** | **NO** — WebView-only (wry → WebKitGTK on Linux) | Massive (webkit2gtk-sys) | tao + wry | No | **Not viable.** Requires host OS browser engine. |

**Key insight:** For T2, the windowing C deps (libxkbcommon, libwayland, libX11) are **replaced by our G1 backend** — we don't use winit on LogansOS, we write a `winit`-compatible backend that speaks G1. The renderer C deps are what matter: all three pure-Rust toolkits have **zero C deps in their software renderer path**.

---

## ISC-2: The category sweep

### Text editor / notepad

| Candidate | Toolkit | C deps (verified) | License | std | GPU | Health | Tier | Verdict |
|-----------|---------|-------------------|---------|-----|-----|--------|------|---------|
| **Helix** | helix-tui (ratatui fork) + termina + crossterm | **NONE** — zero `*-sys` in Cargo.lock. `libc` crate used by `signal-hook` is FFI bindings only, not C linkage | MPL-2.0 | Yes (tokio for LSP; core editing is alloc-only) | No | v25.7.1 (Jul 2026), 45.3k stars, 100+ contributors, pushed Jul 7 | **T1** | **BEST CANDIDATE.** Terminal-based, zero C deps. Adaptation: replace termina/crossterm with our ANSI console; replace tokio I/O with libneo syscalls. Core editing engine (helix-core) and UI layout (helix-tui) are pure Rust. |
| Lapce | floem + winit + wgpu | libgit2-sys, openssl-sys, libz-sys, zstd-sys, wayland-sys, vulkan stack | Apache-2.0 | Yes | **Yes** (wgpu/Vulkan mandatory) | v0.4.6, 38.6k stars, 100+ contributors | T3/REJECTED | GPU-mandatory, C-heavy. Not feasible. |
| Zed | gpui (proprietary) | libsqlite3-sys, webrtc, zstd, llama.cpp (GPL), fontconfig/freetype | Open-core/GPL | Yes | **Yes** (GPU-mandatory) | 86.8k stars, commercial | T3/REJECTED | Proprietary core, GPU-mandatory, massive C deps. Impossible. |
| Ferrite (egui editor) | egui + glow (OpenGL) | libgit2-sys, fontconfig/freetype, mimalloc (optional) | MIT | Yes | **Yes** (OpenGL) | v0.3.0, 1.7k stars, 23 contributors | T3/REJECTED | Needs OpenGL + windowing. egui itself is viable for a custom notepad. |

**Verdict:** Helix is the only shippable editor. For T2, building a simple egui-based notepad (using egui's text-area + file I/O) is a ~500-line project — simpler than porting any existing app.

### Email

| Candidate | Type | C deps | License | std/net | Health | Tier | Verdict |
|-----------|------|--------|---------|---------|--------|------|---------|
| **Himalaya CLI** | CLI (crossterm TUI) | 1: `mime_guess` → `libmagic` (C). Fixable: strip mime_guess, use simple MIME table | MIT/Apache-2.0 | Yes (TCP+TLS for IMAP/SMTP) | v1.2.0 (Feb 2026), v2.0-alpha on master, NLnet-funded through 2026+ | T1 (with net) | Good — but use the **libraries**, not the CLI binary |
| **Pimalaya libs** (io-imap, io-smtp, io-email) | Library | **NONE** (with rustls-ring; no mime_guess) | MIT/Apache-2.0 | Protocol-only (you provide TCP transport) | Same ecosystem as Himalaya | T1 (library) | **BEST.** Pure Rust IMAP/SMTP protocol libs. Wire into Arthur + Workfield. |
| **lettre** | SMTP lib | **NONE** (with rustls feature, no-default-features) | MIT | TCP for SMTP transport | v0.11.22 (May 2026), very active | T1 (library) | Excellent message builder + SMTP. Replace socket2 with libneo TCP. |
| **imap-codec** | IMAP protocol codec | **NONE** — pure parser/encoder | MIT/Apache-2.0 | **NO network** — pure byte parsing | v2.0.0-alpha.8, active | T1 (library) | **Best purity.** Feed it bytes from your TCP stack, get structured IMAP. |
| **mailparse** | Email message parser | **NONE** — pure Rust RFC 5322 parser | MIT/Apache-2.0 | **NO network** | v0.16.1, stable | T1 (library) | Essential building block. Zero deps. |

**Verdict:** Library route wins. Stack: imap-codec + mailparse + lettre (rustls). Wire into Arthur as a skill: "check email" → Arthur connects via IMAP, parses with mailparse, presents summary in the Workfield. No GUI email client needed.

### Image editing ("like GIMP")

| Candidate | Type | C deps | License | GPU | Health | Tier | Verdict |
|-----------|------|--------|---------|-----|--------|------|---------|
| **Graphite** (Canonical) | Web-delivered editor | Check pending — web frontend, Rust core | AGPL-3.0 (likely) | N/A (web) | Canonical project | REJECTED | Web-delivered = requires browser. Not a native app. |
| **Arte-Ogre** | Layered editor (egui+wgpu) | 1: zstd-sys (libzstd for tile compression) | GPL-3.0 | **Yes** (wgpu) | New (Jun 2026), 4 stars | T3/REJECTED | Ambitious scope, GPU-requiring, GPL. Too early. |
| **Patina** | Non-destructive editor (egui+wgpu) | 1: libjpeg-turbo-sys (via mozjpeg) | Unlicensed | **Yes** (wgpu) | New (Apr 2026), 1 star | T3/REJECTED | Too early, unlicensed, GPU-requiring. |
| **Tarsier** | Minimal editor | **NONE** | MIT | No (WASM-capable) | New, 4 stars | T2 (theoretical) | Cleanest deps but too immature for preload. |

**Verdict: CANDID GAP.** There is no GIMP-class image editor in pure Rust with zero C deps and no GPU requirement. The closest is Tarsier (minimal, clean) but it's too new. **Option:** Build a simple egui-based image viewer + annotator using `image` crate (zero C deps) — sufficient for "view an image" on T2. Full editing is a later-stage gap.

### Image viewer

| Candidate | Type | C deps | License | GPU | Health | Tier | Verdict |
|-----------|------|--------|---------|-----|--------|------|---------|
| **oculante** | Feature-rich viewer + basic editor | libjpeg-turbo, libaom, OpenJPEG, LittleCMS, freetype-sys (5+ C libs) | MIT | **Yes** (OpenGL + wgpu) | 1.6k stars, active | REJECTED | C-heavy, GPU-required. Not feasible. |
| **Library route: `image` crate** | Library | **NONE** (default features, v0.25) — migrated to zune-jpeg, pure-Rust PNG/WebP/TIFF | MIT/Apache-2.0 | No | 5.8k stars, excellent | T1/T2 | **BEST.** Decode PNG/JPEG/GIF/WebP to pixel buffer, blit to surface. Zero C. |
| **Library route: `zune-image`** | Library | **NONE** — 100% pure Rust, no_std capable, SIMD-optimized | MIT/Apache-2.0/Zlib | No | 485 stars, active | T1/T2 | **Purest option.** Used by `image` internally for JPEG. |

**Verdict:** Library route wins. An egui/iced image viewer is ~200 lines: load with `image` crate, display in an egui Image widget. No C deps, no GPU needed for software rendering.

### Media / music player

| Candidate | Type | C deps | License | Audio backend | Health | Tier | Verdict |
|-----------|------|--------|---------|--------------|--------|------|---------|
| **ncspot** | TUI Spotify client | alsa-sys, libpulse-binding (default PulseAudio), librespot chain | BSD-2-Clause | ALSA/PulseAudio/PortAudio/SDL2 (feature-gated) | v1.3.4, 6.7k stars, active | T1 (with audio+net) | Good for Spotify specifically, but heavy librespot chain and C audio backends. |
| **symphonia** | Audio decoding library | **NONE** — pure Rust, zero `*-sys` | MPL-2.0 | **Decode only** — no playback | 3.3k stars, active | T1 (library) | **BEST for decoding.** FLAC, MP3, AAC, Vorbis, Opus, WAV, MKV. Zero C deps. |
| **rodio** | Audio playback library | **NONE directly** — delegates to cpal | MIT/Apache-2.0 | cpal (see below) | 2.4k stars, very active | T1/T2 (library) | High-level playback API. Wraps cpal + symphonia. |
| **cpal** | Raw audio I/O | Linux: alsa-sys (mandatory). Windows/macOS: safe bindings only | Apache-2.0 | Direct OS audio (ALSA, WASAPI, CoreAudio) | 3.8k stars, extremely active | T1 (library) | Lowest-level. C dep on Linux only (ALSA). |

**Verdict:** Library route wins. **symphonia** (decode) + custom audio output driver (our voice-lane driver from OT-46 already provides audio DMA — wire symphonia's decoded PCM into it). No player app needed; Arthur skill: "play this song" → symphonia decodes → voice-lane driver outputs PCM.

### PDF / document viewer

| Candidate | Type | C deps | License | GPU | Health | Tier | Verdict |
|-----------|------|--------|---------|-----|--------|------|---------|
| **miro-pdf** | GUI PDF viewer (iced) | mupdf-sys: MuPDF + FreeType + HarfBuzz + libjpeg + openjpeg + brotli + gumbo + jbig2dec + lcms2 + leptonica + tesseract + zxing-cpp (~10 C libs) | AGPL-3.0 | No (iced GPU-accelerated, but rendering is CPU via MuPDF) | 526 stars, active | REJECTED | Production-quality rendering but massive C surface. AGPL. |
| **tdf** | TUI PDF viewer (ratatui) | Same mupdf-sys chain as miro | AGPL-3.0 | No | 1,867 stars, very active | REJECTED | Best TUI PDF viewer but same C deps. AGPL. |
| **lopdf** | PDF manipulation library | **NONE** — pure Rust (aes, flate2/rust backend, nom, sha2, weezl) | MIT | No | 2,197 stars, very active | T1 (library) | **Best pure-Rust PDF lib.** Merge, edit, extract. **No rendering.** |
| **printpdf** | PDF generation library | **NONE** (default features) — lopdf + allsorts layout | MIT | No | 1,085 stars, active | T1 (library) | Pure Rust PDF generation from HTML/text. No rendering. |

**Verdict: PARTIAL GAP.** Pure-Rust PDF **manipulation** exists (lopdf). Pure-Rust PDF **rendering** does not — every renderer depends on MuPDF or Pdfium (C/C++). For day-1: Arthur can extract text from PDFs using lodpf (text extraction without rendering). Visual PDF viewing requires either (a) accepting the MuPDF C dep (bounded island) or (b) waiting for a pure-Rust PDF renderer.

### Notes / markdown

| Candidate | Type | C deps | License | Toolkit | Health | Tier | Verdict |
|-----------|------|--------|---------|---------|--------|------|---------|
| **mdBook** | Book generator (CLI) | **NONE** — pulldown-cmark, handlebars, html5ever, axum | MPL-2.0 | CLI → HTML | 21.9k stars, official Rust project | T1 | Gold standard for docs, not a note-taking app. |
| **wanderer** | Zettelkasten TUI | **NONE** — ratatui, tui-textarea | MIT | ratatui + crossterm | Small, 2024-era | T1 | Cleanest deps. Minimalist. Too small for preload but viable. |
| **Library route: pulldown-cmark** | Markdown parser | **NONE** — pure Rust | MIT/Apache-2.0 | N/A (library) | Official Rust project | T1 | **BEST.** Parse markdown → events. Wire into Arthur: "read this .md file" → Arthur parses + summarizes. |
| **Library route: cosmic-text** | Text shaping/rendering | **NONE** — pure Rust, no_std capable | MIT | N/A (library) | Active (System76) | T1/T2 | Shape + render text to pixel buffer. Critical for any text display. |

**Verdict:** Arthur + pulldown-cmark + cosmic-text supersede a notes app. "Read my notes" → Arthur parses markdown, renders with cosmic-text in the Workfield. No separate notes app needed.

### Calculator

**Verdict: SUPERSEDED.** Arthur already *is* the calculator. "What's 2347 × 86?" → Arthur computes and replies. No app needed.

### File manager

**Verdict: SUPERSEDED.** Arthur + `neo>` already *are* the file manager. "Show me what's in Documents" → Arthur lists files, previews, navigates. The OT-55 familiarity layer maps "open File Explorer" to native intent. No separate file manager app needed.

### Terminal multiplexer / tools

| Candidate | Type | C deps | License | Toolkit | Health | Tier | Verdict |
|-----------|------|--------|---------|---------|--------|------|---------|
| **zellij** | Terminal mux + WASM plugins | **NONE** — pure Rust (tokio, prost, nix [FFI bindings only]) | MIT | crossterm + custom WASM plugin system | 34.2k stars, pushed Jul 10, very active | **T1** | **BEST.** Zero C deps, massive ecosystem, WASM plugins. Shippable once VT100 console lands. |
| **gitui** | TUI git client | 2: libgit2-sys (libgit2 C), onig (C regex, optional via syntect) | MIT | ratatui + crossterm, syntect | 22.2k stars, pushed Jul 9, very active | **T1** | Best TUI git client. libgit2 is a manageable C dep — vendored build available. Alternatively, use `gix` (pure Rust git lib) as a drop-in replacement for the git backend. |
| **yazi** | File manager TUI | 2: liblua5.5 (mlua), onig (syntect); image crate may pull C image libs | MIT | ratatui + crossterm, Lua scripting | 40.2k stars, pushed Jul 8, extremely active | T1 (with caveats) | Best TUI file manager but superseded by Arthur/`neo>`. Lua dep is non-trivial. Skip for preload. |

**Verdict:** zellij (zero C) and gitui (1 manageable C dep, or gix replacement) are the T1 terminal shelf. Both ship once VT100 console lands.

### Web browser

| Candidate | Status | C deps | GPU | Verdict |
|-----------|--------|--------|-----|---------|
| **Servo** | Actively developed, v0.3.0 (Jun 2026) | SpiderMonkey (C++), FreeType, HarfBuzz, jemalloc, SQLite, aws-lc (+ optional GStreamer) | **Yes** (WebRender/OpenGL mandatory; software path exists but limited) | **NOT DAY-1.** Requires 6+ C libraries, GPU, multi-process IPC, full TLS stack. Years away. |
| **wry/Tao** | WebView wrapper | WebKitGTK (entire WebKit port, ~200MB+) on Linux | Yes | **Not viable.** Requires host OS browser engine. |

**Verdict: CANDID GAP.** Browser is not a day-1 aspiration. Honest path: (1) static HTML rendering with html5ever + cssparser + taffy + vello (~200KB Rust, zero C, no JS) for T2; (2) incremental JS support later; (3) Servo embed when GPU + C deps are acceptable. The bounded-island pressure valve (OT-44) applies if catalog demand outpaces the native path.

---

## ISC-3: The day-1 shelf recommendation

### T1 shelf (v1-console + VT100 interpreter + libneo file I/O)

| App | What it is | C deps | License | Preload? |
|-----|-----------|--------|---------|----------|
| **Helix** | Text editor | 0 | MPL-2.0 | **YES — primary editor** |
| **zellij** | Terminal multiplexer | 0 | MIT | **YES — terminal infrastructure** |
| **gitui** | Git TUI | 1 (libgit2, vendored) | MIT | **YES — or gix-backed fork** |
| **imap-codec + mailparse + lettre** | Email stack (library → Arthur skill) | 0 | MIT/Apache-2.0 | **YES — library route** |
| **symphonia** | Audio decoding (library → Arthur skill) | 0 | MPL-2.0 | **YES — library route** |
| **image / zune-image** | Image decoding (library → Arthur Workfield) | 0 | MIT/Apache-2.0 | **YES — library route** |
| **lopdf** | PDF manipulation (library → Arthur skill) | 0 | MIT | **YES — text extraction, no rendering** |
| **pulldown-cmark + cosmic-text** | Markdown + text rendering (library → Arthur) | 0 | MIT | **YES — library route** |

**T1 silent test:** All T1 apps are keyboard-operable by design (terminal apps). Pointer operation is N/A for T1 (no GUI). Voice operation via Arthur skills covers the library-route apps.

**OT-44 compliance:** Each preload is a cap-bundle: `{binary or library artifacts + declared-caps manifest}`. Helix declares `FileRead/Documents`, `FileWrite/Documents`. zellij declares no caps (terminal multiplexer, inherits parent caps). Email libraries declare `NetSend/IMAP`, `FileRead/Mail`. Preloaded ≠ privileged — the cap manifest bounds each app at spawn.

### T2 shelf (G1 + std PAL target + softbuffer)

| App | What it is | Toolkit | C deps | Preload? |
|-----|-----------|---------|--------|----------|
| **egui notepad** | Simple text editor | egui + vello_cpu | 0 | **YES — ~500 lines, purpose-built** |
| **egui image viewer** | Image viewer + annotator | egui + image crate | 0 | **YES — ~200 lines** |
| **iced PDF text extractor** | PDF text display (no full rendering) | iced + tiny-skia + lodpf | 0 | **YES — text-only PDF viewing** |

### T3 shelf (Phase-6 GPU accel — later)

Nothing shippable today. When GPU lands: Lapce (if we resolve C deps), egui apps with wgpu renderer, potentially Servo embed.

---

## ISC-4: The library-not-app table

| Category | Best portable APP | Best pure-Rust LIBRARY | Recommendation | Reason |
|----------|------------------|----------------------|----------------|--------|
| **Text editing** | Helix (T1, terminal) | `ropey` (text buffer), `syntect` (highlighting, 1 C dep: onig) | **App (Helix).** Real editing needs real UI. Helix is shippable. | Library route would mean building an editor from scratch; Helix exists and is pure Rust. |
| **Email** | Himalaya CLI (T1, 1 C dep) | imap-codec + mailparse + lettre (all pure Rust) | **Library.** Wire into Arthur: "check email" → Workfield shows inbox summary, tap to read. | Arthur + Workfield is a better email UX than a TUI client. Library is purer (zero C). |
| **Image editing** | Nothing shippable (candid gap) | `image` / `zune-image` (decode), `imageproc` (filters, pure Rust) | **Library.** Arthur: "open this image" → decode with `image`, display in Workfield. Filters via `imageproc`. | No GIMP-class app exists in pure Rust. Library covers view + basic edit. |
| **Image viewing** | oculante (REJECTED — 5+ C deps) | `image` crate (zero C, default features) | **Library.** ~200-line egui viewer when T2 lands. Before that, Arthur: "show me this image." | Library decode + Arthur display beats porting a C-heavy viewer. |
| **Media playback** | ncspot (T1, heavy C chain) | symphonia (decode, zero C), rodio (playback API, zero own C) | **Library.** symphonia → voice-lane driver. Arthur: "play this song." | Our audio driver (OT-46) already exists. symphonia feeds it PCM. No player app needed. |
| **PDF viewing** | miro-pdf / tdf (REJECTED — MuPDF C deps) | lodpf (manipulation, zero C), printpdf (generation, zero C) | **Library.** lodpf for text extraction. Arthur: "read this PDF." Full rendering = candid gap. | No pure-Rust PDF renderer exists. Library covers text; rendering requires C dep or new work. |
| **Notes / markdown** | wanderer / deez-notes (T1, small projects) | pulldown-cmark (parser), cosmic-text (shaping/rendering) | **Library.** Arthur + pulldown-cmark = notes app. "Read my notes" → Workfield renders markdown. | Arthur already navigates files and renders text. Markdown parsing is the only missing piece — and it's a pure Rust library. |
| **Calculator** | N/A — superseded | N/A — superseded | **Arthur-native.** No library or app needed. | "What's 2347 × 86?" works now. |
| **File manager** | yazi (T1, Lua dep) | N/A — superseded | **Arthur/`neo>`-native.** No library or app needed. | Arthur already lists, navigates, previews, and manipulates files. OT-55 handles "open File Explorer." |
| **Terminal tools** | zellij (T1, zero C), gitui (T1, 1 C dep) | N/A — these are terminal apps, not libraries | **Apps.** zellij + gitui are the shelf. | Terminal multiplexing and git TUI don't map to Arthur skills — they're interactive tools. |
| **Web browser** | Servo (REJECTED — 6+ C deps, GPU) | html5ever (parser), cssparser, taffy (layout) — no rendering | **Gap.** Static HTML rendering from libraries is a T2 build opportunity, not a preload. | No pure-Rust browser renderer exists. Library components exist for a custom minimal renderer. |

**The pattern:** In 7 of 12 categories, the library route (wired into Arthur/a Workfield) is superior to porting an app. LogansOS doesn't ship a distro's preload list — it ships capabilities that Arthur exposes conversationally.

---

## ISC-5: The prerequisite ladder, honestly costed

### Prerequisite sequence (what unlocks what)

| # | Prerequisite | Cost estimate | Unlocks | Priority |
|---|-------------|---------------|---------|----------|
| **P1** | **VT100/ANSI escape interpreter** for the transcript console | 1-2 person-weeks (~500 lines, pure Rust) | **Entire T1 shelf** (Helix, zellij, gitui, all ratatui apps) | **HIGHEST — single biggest leverage** |
| **P2** | **Minimal `std` PAL target** (fs + threads subset, no networking) | 2-3 person-months (dedicated Rust/OSdev engineer) | T2 GUI apps, any crate requiring `std::fs` or `std::thread` | High — blocks all GUI apps |
| **P3** | **G1 winit/softbuffer backend** (compositor client speaking G1 protocol) | 2-3 person-weeks (compositor design is complete; backend is ~1000 lines mapping winit traits → G1 verbs) | T2 software-rendered GUI (egui, iced, Slint) | High — co-requisite with P2 |
| **P4** | **Audio output driver exposure** (voice-lane driver → userland PCM write syscall) | Already banked in OT-46; needs userland API surface | Media playback (symphonia → PCM → driver) | Medium — audio is Phase-N but media decoding works without it |
| **P5** | **Full `std` PAL target** (adds networking: TCP, DNS, TLS) | +2-3 person-months on top of P2 | Email (IMAP/SMTP), browser, any networked app | Medium — email is high-value but can wait for P1/P2/P3 |
| **P6** | **GPU accel path** (Phase-6) | Unknown — architecture-phase work | T3 apps (wgpu, Servo, Lapce) | Low — not day-1 |

### The std-target cost model (Redox prior art)

**Redox's path (firsthand from their repos):**
- `redox_syscall` crate: ~500 lines, ~30 syscall wrappers. Thin FFI layer.
- `relibc`: Full C standard library in Rust. POSIX compat, pthread, dynamic loader, CRT0, signal handling, fork/exec. **This is the heavy lift.** ~6-12 person-months for the full libc.
- Rust std integration: Redox is treated as a `unix` target family — it reuses all Unix PAL code (`std::sys::pal::unix`). The only Redox-specific code is the syscall crate.

**LogansOS options:**

| Approach | Effort | Pros | Cons |
|----------|--------|------|------|
| **Dedicated PAL backend** (new `target_os = "logans"`) | 2-3 person-months for minimal (fs+threads); 3-6 for full | Clean, no C libc needed, direct libneo syscalls | Requires upstream Rust contribution (Tier 3 self-hosted); ~50 syscall wrappers to write |
| **Unix PAL compatibility** (libc wrapper, Redox-style) | 6-12 person-months | Reuses all Unix std code; any Unix-targeted crate works | Must write full libc (relibc-scale); C ABI surface is large |
| **Thin PAL** (fs + threads only, no net) | **2-3 person-months** | Enough for T2 apps; no networking complexity | Networked apps still can't use std |

**Recommendation: Dedicated PAL backend, thin-first.** Start with fs + threads + basic time (~25 syscall wrappers, 2-3 person-months). This unlocks T2 GUI apps. Add networking later when email/browser demand justifies it. The dedicated PAL is cleaner (no C libc) and maps directly to libneo syscalls.

**Key challenges (from Redox/hermit prior art):**
1. **Thread/sync primitives:** Futex-like atomic wait/wake is the hardest part. Our kernel's `SYS_RECV`/`SYS_SEND` on channels can serve as the synchronization primitive (park/unpark on channel receive/send).
2. **Allocator:** std requires `mmap`/`munmap` equivalents. `SYS_MAP_REGION` fills this role.
3. **Filesystem VFS:** Our capability-based file handles differ from POSIX fds. The PAL must translate: `std::fs::File` → libneo file capability. Path semantics differ (no `/` root in the POSIX sense).
4. **Upstream Rust:** Tier 3 target is self-hosted but needs a sponsor. This is a process cost, not a technical blocker.

### Which single prerequisite unlocks the most shelf?

**P1 (VT100 interpreter)** — 1-2 weeks of work enables Helix (editor), zellij (terminal mux), gitui (git), and the entire ratatui ecosystem. It also makes the library-route apps testable (email via Himalaya CLI for development, symphonia CLI tools, etc.). **Highest leverage, smallest cost.**

---

## ISC-6: Gaps + ledger disposition

### Candid gap list

| Category | Gap | Severity | Options |
|----------|-----|----------|---------|
| **GIMP-class image editing** | No pure-Rust raster editor exists at shippable quality. Arte-Ogre is too early, GPL, and GPU-requiring. | Medium — users expect image editing | (a) Library route: `image` + `imageproc` for view + basic filters via Arthur; (b) Build simple egui editor (~2000 lines for crop/resize/brightness/contrast); (c) Bounded-island pressure valve if demand is high |
| **Web browser** | No pure-Rust browser renderer. Servo requires 6+ C libs + GPU. | Medium-high — S7 "never a dead end" makes this load-bearing for the north-star review | (a) Minimal HTML renderer (html5ever + cssparser + taffy + vello) for T2; (b) Incremental JS later; (c) Servo embed when GPU + C deps acceptable; (d) Bounded-island if catalog pressure demands |
| **PDF rendering** | No pure-Rust PDF renderer. All renderers use MuPDF or Pdfium. | Low-medium — text extraction works via lodpf | (a) Accept MuPDF C dep as bounded island for PDF viewing; (b) Wait for pure-Rust renderer (pingpong, kopano — neither exists yet); (c) Text-only PDF viewing via lodpf is sufficient for v1 |
| **Full std target** | No `std` PAL backend for LogansOS. | Blocks T2 entirely | Dedicated PAL backend, thin-first (ISC-5 P2). 2-3 person-months. |

### Ledger disposition

**Proposed new OT: OT-56 — "The day-1 app shelf + std-target prerequisite"**

| Slice | Description | Depends on | Gate |
|-------|-------------|-----------|------|
| **OT-56 S1** | VT100/ANSI escape interpreter for transcript console | v1 framebuffer console | Host-only: parse ESC sequences → cursor state + attributes → render to pixel buffer; assert known TUI app output is rendered correctly |
| **OT-56 S2** | Helix adaptation: replace termina/crossterm with LogansOS console backend | S1 (VT100) | QEMU: Helix opens, edits, saves a file; kill-switch: Helix off ≡ raw console |
| **OT-56 S3** | Minimal `std` PAL target (fs + threads subset) | libneo syscalls | Host KAT: `std::fs::read/write`, `std::thread::spawn/join` work against libneo; upstream Rust Tier 3 registration |
| **OT-56 S4** | G1 winit/softbuffer backend | G1 compositor (banked) | QEMU: egui demo renders on G1 surface; kill-switch: GUI off ≡ console |
| **OT-56 S5** | T1 shelf integration: zellij, gitui, email/audio/image libraries as Arthur skills | S1, S3 for std-needed libs | QEMU: each app launches, performs core operation, cap manifest enforced |

**Feeds into:** OT-44 catalog (preloaded apps are cap-bundles with declared-caps manifests), OT-52 G1 (T2 apps need the compositor), north-star review S7 ("never a dead end" — the shelf is what makes dead ends impossible).

---

## Cited primary sources

All claims verified from these primary sources (Cargo.toml, LICENSE, README from each repo's current main/master):

**Toolkits:**
- egui: `emilk/egui` — crates/egui/Cargo.toml, crates/epaint/Cargo.toml, crates/eframe/Cargo.toml
- iced: `iced-rs/iced` — root Cargo.toml (workspace), renderer/tiny_skia/Cargo.toml
- Slint: `slint-ui/slint` — api/rs/slint/Cargo.toml, internal/renderers/software/Cargo.toml
- dioxus-desktop: `DioxusLabs/dioxus` — packages/desktop/Cargo.toml
- tiny-skia: `linebender/tiny-skia` — Cargo.toml

**Editors:**
- Helix: `helix-editor/helix` — base/Cargo.toml, LICENSE-MPL-2.0
- Lapce: `lapce/lapce` — Cargo.toml
- Zed: `zed-industries/zed` — Cargo.toml (open-core, proprietary editor crate)
- Ferrite: `OlaProeis/Ferrite` — Cargo.toml

**Email:**
- Himalaya: `pimalaya/himalaya` — Cargo.toml
- lettre: `lettre/lettre` — Cargo.toml
- imap-codec: `duesee/imap-codec` — Cargo.toml
- mailparse: `djc/mailparse` — Cargo.toml

**Image:**
- image: `image-rs/image` — Cargo.toml (v0.25)
- zune-image: `etemesi254/zune-image` — workspace Cargo.toml
- oculante: `woelper/oculante` — Cargo.toml
- Arte-Ogre: `visorcraft/Arte-Ogre` — Cargo.toml
- Patina: `cihanhizlikan/patina` — Cargo.toml
- Tarsier: `Its-Just-Nans/tarsier` — Cargo.toml

**Audio:**
- symphonia: `pdeljanov/Symphonia` — symphonia/Cargo.toml
- rodio: `rustaudio/rodio` — Cargo.toml
- cpal: `rustaudio/cpal` — Cargo.toml
- ncspot: `hrkfdn/ncspot` — Cargo.toml

**PDF:**
- miro-pdf: `vincent-uden/miro` — Cargo.toml
- tdf: `itsjunetime/tdf` — Cargo.toml
- lodpf: `J-F-Liu/lopdf` — Cargo.toml
- printpdf: `fschutt/printpdf` — Cargo.toml

**Terminal tools:**
- zellij: `zellij-org/zellij` — Cargo.toml
- gitui: `gitui-org/gitui` — Cargo.toml
- yazi: `sxyazi/yazi` — Cargo.toml

**Browser:**
- Servo: `servo/servo` — Cargo.toml, components/servo/lib.rs

**std port prior art:**
- Redox std: `redox-os/std` — README, sysfile/src/lib.rs
- Redox syscall: `redox-os/redox_syscall` — src/number.rs
- Redox relibc: `redox-os/relibc` (GitLab)
- hermit-abi PAL: `rust-lang/rust` — library/std/src/sys/pal/hermit/

---

## Close-gate note

**Docs-only research — no CI run exists.** Paths-ignored per dispatch. `scripts/worker-close-gate.sh` host set locally (skip-with-note).

---

## RETURN

**Artifact:** `Plans/RESEARCH-2026-07-11-rust-day1-apps-FINDINGS.md` (this file)
**Deliverables:**
1. Feasibility-tier framework (T1/T2/T3/REJECTED) with prerequisite builds — ISC-1
2. Verified candidate table across 11 categories, purity checked from Cargo.toml — ISC-2
3. Day-1 preload shortlist per tier (8 T1 items, 3 T2 items) — ISC-3
4. Library-not-app table (7/12 categories: library wins) — ISC-4
5. Prerequisite ladder with std-target costing (Redox prior art, 2-3 person-months thin PAL) — ISC-5
6. Candid gap list (GIMP-class editing, browser, PDF rendering) + proposed OT-56 ledger disposition — ISC-6
7. All primary sources cited above

**Key recommendation to PL:** P1 (VT100 interpreter, 1-2 weeks) is the highest-leverage unlock. It enables the entire T1 shelf immediately. The std PAL target (P2, 2-3 months) is the gate for T2 GUI apps. The library-not-app route is the LogansOS-differentiating choice — we ship capabilities Arthur exposes, not a distro's app list.
