<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK — OT-61: can Servo's layout draw a static page without SpiderMonkey? And the rungs to the first page

**Date:** 2026-11-01. **Seat:** D (the browser lane). **Asked by:** the PM on #4.
**Pins:** `servo-layout` 0.7.0 (crates.io, published 2026-10-05), with the versions our stack already pins: Stylo
0.22.0, `selectors` 0.41.0, `html5ever` 0.40.1 (#4) and WebRender 0.70 (#25). The probe resolved all four to the
same versions, so layout 0.7.0 sits on exactly the stack we are building.
**Method:** a scratch crate with `servo-layout = "=0.7.0"` as its only dependency; `cargo generate-lockfile`, then
`cargo tree -e normal,build --target x86_64-unknown-linux-gnu` (`-i mozjs_sys` for the paths; the full tree for the
closure). Dependency flags come from the crates.io index, and the code facts from the published `.crate` sources
(`servo-layout`, `servo-layout-api`, `servo-script`, `servo-fonts`, all 0.7.0). Nothing was built for our target;
§5's first rung is the build that does that.

## 1. The answer

**No, not as published.** At 0.7.0 SpiderMonkey is a hard dependency of layout itself, not something that comes
in only through script or constellation crates a host could leave out:

1. `servo-layout` 0.7.0 depends on `servo-script` 0.7.0 (renamed `script`) as a **required** dependency: not
   optional, not behind a feature, not target-gated. `servo-layout` has one feature, `tracing`.
2. `servo-script` depends on `mozjs` (renamed `js`) and `servo-script-bindings`, both **required**; `mozjs`
   requires `mozjs_sys`.
3. Layout uses the dependency in its code, not only in its manifest. **19 files import, and 134 uses across 20 files name,** `script`'s
   concrete DOM wrappers: `script::layout_dom::{ServoLayoutNode, ServoLayoutElement,
   ServoDangerousStyleElement, ServoDangerousStyleDocument}`. `layout_impl.rs` builds a `ServoLayoutNode` from a
   `TrustedNodeAddress` (a raw pointer to script's GC-managed `Node`) at 22 call sites. A stub `servo-script`
   crate therefore can't stand in: it would have to supply working DOM types.

`cargo tree -i mozjs_sys` (x86_64 Linux, normal and build edges):

```
mozjs_sys v153.3.0-1
└── mozjs v0.26.4
    ├── servo-script v0.7.0
    │   └── servo-layout v0.7.0
    └── servo-script-bindings v0.7.0
        └── servo-script v0.7.0 (*)
```

`servo-script` is layout's **only** route to `mozjs`. Of layout's 53 direct dependencies, it is the only one
whose closure reaches `mozjs_sys`.

**But the seam is already drawn upstream.** `servo-layout-api` 0.7.0 defines the DOM as traits:
`LayoutNode<'dom>`, `LayoutElement<'dom>`, `DangerousStyleNode<'dom>`, `DangerousStyleElement<'dom>`, and a
`LayoutDomTypeBundle<'dom>` with four associated types naming the concrete ones. `script` implements them
(`ServoLayoutDomTypeBundle` in `servo-script/layout_dom/mod.rs`). The traits exist; layout 0.7.0 just names
script's types directly instead of being generic over the bundle. A comment in `servo_layout_node.rs` describes
that coupling as transitional. So separating layout from SpiderMonkey is a **patch to `servo-layout`** (§4,
option A), not a rewrite. That patch is J.c.'s call under ADR-078 (§4).

## 2. What the cut leaves (closure numbers)

| Closure (normal edges, x86_64 Linux) | Crates |
|---|---|
| `servo-layout` 0.7.0, everything | **459** (439 under `servo-layout`, plus the probe and edge-only build crates) |
| ... without `servo-script` | **376** |
| only reachable through `servo-script` | **63**: all 14 `mozjs*` crates, `servo-script-bindings`, `aws-lc-sys`/`aws-lc-rs` (C), `xml5ever`, `servo-xpath`, `servo-timers`, ICU calendar/time data, `backtrace` |
| of the 376, already vendored by #4 (`third_party/ot61-browser`) or #25 (`third_party/ot61-swgl`) | **151** |
| of the 376, **new** | **225** |

(The 828 in `docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md` is Servo's whole workspace lockfile, every
target and every edge kind. 459 is what `servo-layout` alone pulls for one target. On our target the Linux-only
crates drop out (`wayland-sys`, `x11-dl`, `yeslogic-fontconfig-sys`, `linux-raw-sys`, `dlib`, ...), so the real
number is lower. The first rung measures it.)

**The 376 are still heavy, and none of it is optional.** Every one of these comes in as a required dependency of a
`*-traits` or base crate that layout itself requires:

| Pulled in | Through | Why it matters on our target |
|---|---|---|
| `tokio`, `mio`, `socket2`, `hyper`, `h2`, `warp`, `webdriver` | `servo-malloc-size-of`, `servo-net-traits`, `servo-embedder-traits` (`webdriver` → `warp`) | Async runtime and sockets. `mio` has no `neoplutarch` backend. The content process has no dial capability (C2) and must never open a socket. |
| `ipc-channel` | `servo-base`, `servo-net-traits`, `servo-profile-traits`, `servo-malloc-size-of` | Cross-process channels over OS IPC; needs either its in-process backend or our channel. |
| `rustls`, `rustls-webpki` | `servo-net-traits` | Types only for layout, but it compiles the TLS stack. |
| `servo-media*`, `speexdsp-resampler`, `rustfft` | `servo-script-traits` → `servo-media-thread` | The media stack, for message types. |
| `webrender` (the renderer, not only `_api`) | `servo-geometry`, `servo-malloc-size-of` | Already vendored by #25. |
| `freetype-sys`, `harfbuzz-sys`, `fontsan` (+`libz-sys`) | `servo-fonts` | **Native C.** FreeType and HarfBuzz are wanted anyway for text. `fontsan` is the OTS font sanitizer (C++). |
| `surfman`, `glow` | `servo-paint-api`, `servo-canvas-traits` | GL context plumbing; SWGL already answers `gleam` (#22). |

**`servo-fonts` has no platform backend for our target.** Its `platform/mod.rs` selects a font source by `cfg`:
Linux/Android/FreeBSD (FreeType + fontconfig), macOS, Windows, OHOS. `x86_64-unknown-neoplutarch` matches none
of them, so the crate won't build until it has one. That is a narrow provider seam (where fonts come from), the
kind ADR-078 §2.1 allows us to fork: a FreeType backend with a fixed, bundled font list and no fontconfig.

## 3. The seams a no-script host implements

To run `servo-layout` without `servo-script`, a static-page host supplies:

1. **The DOM bundle.** One type for each of `LayoutNode`, `LayoutElement`, `DangerousStyleNode` and
   `DangerousStyleElement` (`servo-layout-api`, `layout_node.rs` and `layout_element.rs`), plus Stylo's
   `TNode`/`TElement`/`TDocument`/`TShadowRoot`, which those traits extend. **#4's `stylo_smoke` already implements
   the Stylo half** over an html5ever node table. The layout half adds the box-tree data slot
   (`LayoutDataTrait`), pseudo-element chains, and a constructor from a node address.
2. **The reflow driver.** `LayoutFactoryImpl` (`layout_impl.rs`) is how script drives layout: it builds a
   `Layout` from a `LayoutConfig` (pipeline ids, the paint API sender, a `FontContext`, an image cache, the
   script-traits channels) and is fed `ReflowRequest`s. A static host either builds those messages itself, or
   calls the box-tree → fragment-tree → display-list steps directly. Those steps are `pub(crate)` today (`BoxTree`,
   `FragmentTree`), so the second way means a larger patch. The first rung of §5 finds out which way is smaller.
3. **Fonts.** A `FontContext` over the bundled-font backend from §2.
4. **The output.** Layout emits a WebRender display list (`webrender_api`) through `servo-paint-api`. #25 already
   renders a WebRender display list through SWGL into a caller buffer, so this is plumbing between two built pieces.
5. **What stays inert.** Network (`net_traits`: resource fetches; a static page from the bundle needs none),
   images (the image cache can answer "not available"), media, WebDriver, devtools: types the build needs, with
   no behaviour reached by a static page. If one of them can't build for our target, it gets a target-cfg stub
   (the `windows-sys` precedent in #4), never a working ambient implementation.

## 4. The options, against ADR-078

ADR-078 §2.1: *"Fork only the narrow capability-sensitive provider seams (net/fs/entropy/process); NEVER
html5ever/Stylo/layout/DOM/WebRender/GC semantics — prefer upstreamable target-cfg patches."* §5: a failure at C3
"means reassess the SpiderMonkey port or a bounded WASI-derived path, not the DOM/layout closure."

| Option | What it is | First static page needs | Against ADR-078 | Cost |
|---|---|---|---|---|
| **A. Layout generic over the DOM bundle** (recommended) | Patch `servo-layout`: its 19 `use script::layout_dom::…` imports become a crate-local alias module (or a generic over `LayoutDomTypeBundle`). Upstream's script bundle stays the default; on our target the alias names a static host DOM. Drop the `servo-script` dependency on our target. | #4 + the rendering stack + the §5 rungs. **Not** C3. | Changes which type layout names, not what layout does. Upstream's own direction (the traits already exist in `servo-layout-api`). **But it is a patch to the layout crate, and ADR-078 says never fork layout.** Reading this as a type-plumbing patch rather than a semantics fork is **J.c.'s call**. | One patched crate (as for Stylo's thread handle in #4), re-applied on every Servo bump. The host DOM is ours, and reuses #4's Stylo node table. |
| **B. Wait for SpiderMonkey (C3)** | Use `servo-script`'s DOM unchanged. A static page still creates a JS runtime; it just runs no scripts. | C3 (#10's THINK: SpiderMonkey interpreter-only on neo-crt) **plus** the whole `servo-script` closure (459 crates, `aws-lc-sys`) on our target. | Fully within ADR-078: nothing forked. | The first page waits on the deepest rung in the ladder. Nothing forked, nothing thrown away. |
| **C. Blitz** (`blitz-dom` 0.3.0-beta.4, MIT OR Apache-2.0, published 2026-10-16, github.com/dioxuslabs/blitz) | Stylo + Taffy + Parley, no JavaScript, on the same `selectors` 0.41 and `markup5ever` 0.40 as ours. | Its own closure (Parley, Taffy, `anyrender`), not Servo's. | **A second layout engine.** ADR-078 rules that out. | Fast to first pixels, but every line of it is thrown away when Servo layout lands. Listed for completeness. |
| **D. Our own block/inline on Taffy** | Write layout. | Weeks of cycles to reach CSS 2.1 block/inline/float. | A second layout engine, written by us. | Rejected. |

**Recommendation: A**, held to two conditions so it stays a seam and not a fork:

1. **Only types move.** No layout algorithm is touched; the patch is reviewed as a diff of `use` lines and an alias
   module. A `cfg(not(target_os = "neoplutarch"))` build of the patched crate is byte-identical in behaviour (its
   alias names script's types), the kill-switch the complexity rules ask for.
2. **It converges on B.** When C3 lands, our target can name `servo-script`'s bundle again. The patch then
   matters only to the static-page host, and if upstream makes layout generic, it goes away.

If J.c. reads ADR-078 as forbidding even this, the answer is **B**, and the first static page waits for C3. Then
§5's L1 (fonts) and L3 (WebRender plumbing) still go ahead in parallel, because neither depends on the choice.

## 5. The rungs to a static page in a LogansOS window

Each rung is gated in QEMU by an oracle written independently of the code (the byte-oracle rule): values derived by
hand from the page and the font, never "whatever the renderer produced last time". "Frame hash" is the SHA-256 of
the RGBA buffer, but only after a few hand-derived pixel probes on the same frame pass. A hash alone is a
regression check, not a correctness oracle.

| Rung | What | Crates (new) | Native C | New PAL surface | Gate oracle |
|---|---|---|---|---|---|
| **Built / in review** | | | | | |
| #4 C2.4 | Stylo styles a parsed page | 138 vendored (95 new over C2.3b) + `windows-sys` stub | none (build-time Python) | none | `stylo_smoke`: computed display/colour/font-size/margin vs hand-derived values |
| #22 S1 | SWGL rasters into a caller-owned buffer | 18 | SWGL's generated C++ over neo-crt (#2) | none | `swgl_smoke`: pixels vs hand-derived values |
| #25 S2 | WebRender 0.70 on SWGL | 98 total | SWGL C++, `glslopt` (C++) | none | `webrender_smoke`: solid, clip, transform, image, gradient, text vs a hand-derived scene |
| #26 S3a | SWGL into a kernel Atrium buffer the compositor reads | 0 | — | std's Atrium calls (A4 manifest) | compositor-side readback == the hand-derived scene |
| #27 S3b | A separate compositor process gets a read-only Atrium view | 0 | — | std's grant call (A4) | read-only view: the hand-derived scene, and a write refused |
| #28 6a | The browser app's I/O surface | 0 | — | channel `try_recv`, input focus (A4) | `browser-io-smoke` |
| **New** | | | | | |
| **L0** | **Closure probe.** Vendor the 225 new crates; patch `servo-layout` (option A) and drop `servo-script` on our target; `cargo build` the closure for `x86_64-unknown-neoplutarch` in the closure CI job. **This rung decides the size of the rest.** | ≤225 (fewer once Linux-only crates drop) | FreeType, HarfBuzz, `fontsan`, zlib (compiled, not yet linked) | none expected; any crate that needs one is stubbed by target-cfg (the `windows-sys` precedent) and listed | The build either passes, or the RETURN lists every crate that doesn't build and why. A partial answer is still a result |
| **L1** | **Fonts.** A `servo-fonts` platform backend for our target: FreeType from a bundled font list, no fontconfig. Shape a string with HarfBuzz. | 0 beyond L0 | FreeType + HarfBuzz linked over neo-crt | read-only access to a bundled font (a sealed file handle, C2) | Glyph ids and advances for a fixed string == values read by hand from the font's `cmap`/`hmtx` tables (fetched raw) |
| **L2** | **Layout, no paint.** A static host DOM (html5ever + #4's node table) implementing the `servo-layout-api` bundle; drive one reflow of a hand-written page. | 0 | — | none | Fragment rectangles (in app units) for each box of a hand-checked page: block widths, margins collapsing, one line break, one float. Hand-computed |
| **L3** | **Layout → WebRender → SWGL.** The display list from L2 rendered by #25 into a buffer. | 0 | — | none | A few pixel probes (background edges, a border corner, one glyph's ink box from L1's advances) hand-derived, then the frame hash |
| **L4** | **The page in a LogansOS window.** The browser app (#28) renders L3 into its Atrium buffer (#26) and the compositor shows it (#27). | 0 | — | none beyond #26-#28 | The compositor's readback hash == L3's hash, and the scenario's framebuffer capture shows the page |

**The dependency order:** #2 → #22 → #25 → #26 → #27 → #28 for the window. #4 → L0 → L2. L1 needs L0's font
crates. L3 needs L2 and #25. L4 needs L3 and #28. L0 and L1 can start as soon as #4 merges, alongside the rendering
stack's merge queue.

**Where the cycles will go**, honestly: L0 is a build probe with an unknown number of "this crate doesn't build
for us" answers (tokio/`mio`, `ipc-channel`, `socket2` are the likely ones); each one is a target-cfg stub or a
narrow PAL seam. L2 is the deep one: the reflow driver (§3.2) is the least known seam, and the host DOM has to
satisfy layout's `unsafe` assumptions about node lifetimes (script keeps nodes alive in its GC; our host keeps them
in an arena that outlives the reflow). Expect several gate cycles there.

### 5.1 L0, first pass (local, 2026-11-01)

A local probe built `servo-layout` 0.7.0's 52 direct dependencies other than `servo-script` (and not layout itself)
for `x86_64-unknown-neoplutarch`: the overlay std, `-Z build-std`, crates.io sources, unpatched, `--keep-going`.
378 crates started compiling. Nine failed, and everything above them was left unbuilt, so this is the **first
layer** of answers, not the last:

| Crate | First error | Already solved in our tree? | The fix it needs |
|---|---|---|---|
| `stylo` 0.22.0 | `PlatformThreadHandle` undefined | **Yes**: #4's patched copy | none |
| `wr_glyph_rasterizer` 0.70.1 | no `platform::font` for our target | **Yes**: #25's patched copy (a `neoplutarch` font backend) | none |
| `getrandom` 0.4.3 and 0.2.17 | "target is not supported" | No | Its documented custom backend over our entropy call (a narrow entropy seam, which ADR-078 allows) |
| `mio` 1.2.4 | no `sys` backend (47 errors) | No | Async sockets for `tokio`. The content process has no dial capability, so either the `tokio` features layout's crates enable leave out `net`, or a target-cfg stub of the socket half |
| `socket2` 0.6.5 | no `sys` module | No | Same as `mio`: a stub, never a working socket |
| `libz-sys` 1.1.31 | `libc::off_t` missing; no zlib found | No | Build its bundled zlib (the `static` feature) with an `off_t` for our `libc`, or route `fontsan`/`freetype-sys` to `miniz_oxide` (Lane 2a) |
| `servo-allocator` 0.7.0 | `libc::{malloc, free, realloc}` missing | No | A target-cfg arm on std's allocator (upstream's own non-jemalloc path) |
| `surfman` 0.14.0 | no platform `default` module | No | A target-cfg stub: SWGL answers GL (#22), and surfman's contexts are never created |

**Where `mio` and `socket2` come from** (`cargo tree -e features -i tokio`, `-i mio`, `-i socket2`, x86_64 Linux):
`tokio`'s `net` and `signal` features are turned on by `warp` and `webdriver`, both reached only through
`servo-embedder-traits` (`webdriver` → `warp`); `socket2` comes from `hyper-util` under `servo-net-traits`.
`ipc-channel` 0.23.1 needs `mio` only on Linux/BSD/illumos, and it has an in-process backend (its
`force-inprocess` feature, or a target-cfg arm next to `wasi`/`unknown`). So the socket crates are pulled in by
WebDriver and HTTP **types**, not by anything layout does. The likely fix is to keep them off our target
(a target-cfg edge in the two traits crates, or `force-inprocess` plus stubs), not to port `mio`. Porting `mio` would
be the growing compat island ADR-078's A4 rule exists to stop.

**Second pass**, with the fixes we already have applied: `getrandom`'s custom backend (`--cfg getrandom_backend="custom"`,
and the 0.2 `custom` feature), `ipc-channel`'s `force-inprocess`, `libz-sys`'s `static` feature, and #4's and
#25's patched `stylo` and `wr_glyph_rasterizer`. `getrandom`, `stylo` and `wr_glyph_rasterizer` then build.
The same five still fail: `mio`, `socket2`, `libz-sys` (`off_t` is still missing from our `libc`), `servo-allocator`
and `surfman`. Everything above them still waits, so L0's next step is those five stubs or cfg edges. Only after
them does the probe reach `tokio`, `ipc-channel` and the `servo-*` crates.

**Third and fourth passes.** Local probe patches: `servo-allocator` gets an arm for our target using std's allocator,
and the `libc` crate gets a **types-only** arm for `target_os = "neoplutarch"`. The `libc` arm has the C types
(`c_int`, `size_t`, `off_t`, ...), modelled on its existing types-only `xous` arm, and **no functions**, so it adds
no syscall surface (A4 unchanged). With those two:
- `libz-sys` builds **unpatched**, and `fontsan` builds too. Its C++ (the OTS sanitizer) compiled from the third
  pass on; only its Rust side had failed, on `libc::{c_int, size_t, off_t}`. The missing `libc` arm was the shared
  cause of both failures.
- `servo-allocator` is different: it needs functions, not only types (`malloc`, `free`, `realloc`,
  `malloc_usable_size`). Those are neo-crt's C symbols (#2), so the choice is a function arm in `libc` linked
  against neo-crt, or the std-allocator arm the probe used. The std arm reports 0 for usable size, so Servo's
  memory reports would undercount.
- `ipc-channel` (in-process backend) builds.

**What still fails:** `mio`, `socket2` and `surfman`, the three the first pass already showed came in only
through types crates. Those three are L0's remaining work before the probe reaches `tokio` and the `servo-*`
traits crates.

`harfbuzz-sys` (C++) built for our target unchanged. `freetype-sys` and `fontsan` sit behind `libz-sys`, and
`tokio`, `ipc-channel` and the `servo-*` traits crates sit behind `mio`/`socket2`/`getrandom`, so they are the
probe's second layer. **None of the first layer needs a layout or DOM change**: each is a provider seam (entropy,
sockets, allocator, GL context, zlib) of the kind ADR-078 §2.1 allows, or is already patched by #4/#25.

## 6. What would prove this wrong

- **§1's answer:** a `servo-layout` 0.7.0 build with `servo-script` removed that compiles without the patch. The
  index metadata and the 134 uses in the source say it can't; L0 settles it.
- **Option A's premise ("only types move"):** if the L0 patch has to change anything but `use` lines and the alias
  module (layout calling a `ServoLayoutNode` inherent method that isn't in the traits, beyond `new`), then A is a
  bigger fork than stated here, and the recommendation goes back to J.c. with the diff.
- **The closure numbers:** they are for x86_64 Linux. L0 reports the real count for our target.
- **§5.1's "provider seams only":** if L0's second layer (`tokio`, `ipc-channel`, the `servo-*` traits crates) needs a change to a type layout passes around, not just a stub of a provider, the claim fails and the fix list grows.

## 7. Prior-art survey (the 2026-10-27 rule)

Checked 2026-11-01, from crates.io (the API and the index); no repository was cloned.

| Candidate | What it does | Licence | Last release | Fits? | What we'd take |
|---|---|---|---|---|---|
| `servo-layout` 0.7.0 | Servo's layout | MPL-2.0 | 2026-10-05 | It is the plan (ADR-078) | The crate, with option A's patch |
| `blitz-dom` 0.3.0-beta.4 | Stylo + Taffy + Parley DOM/layout, no JS | MIT OR Apache-2.0 | 2026-10-16 (beta) | A second engine; ADR-078 rules it out | Nothing, unless J.c. reopens ADR-078 |
| `taffy` 0.15.1 | Flexbox/grid/block layout | MIT | 2026-10-25 | Already in Servo layout's closure (0.14.0 there) | Through Servo layout only |
| `parley` 0.12.0 | Text layout (Linebender) | Apache-2.0 OR MIT | 2026-10-09 | Blitz's text stack; Servo uses its own | Nothing |

Exact bytes for L1's oracle (the font's tables) come from the font file fetched raw (`git`/`curl`), never a
summarizing fetch.

**The closure's crates (2026-11-03).** The 195 crates L0 vendors beyond Stylo's closure and the SWGL/WebRender trees
have their own licence, freshness and RustSec table: `docs/RESEARCH-2026-11-03-ot61-l0-layout-closure-SURVEY.md`.
All licences are on CLAUDE.md's list, and no known vulnerability affects them. Five `unic-*` crates carry RustSec
"unmaintained" notices, and `arrayref` must stay at 0.3.9.
