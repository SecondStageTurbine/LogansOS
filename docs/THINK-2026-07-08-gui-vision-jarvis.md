<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The GUI vision - the Intent Field

**Date:** 2026-07-09
**Author:** Jarvis
**Branch:** `codex/gui-vision-think-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-07-08-gui-vision-think-cerebro-and-jarvis.md`
**Scope:** Research/design only. No code. No QEMU. Host-only.

---

## Executive thesis

Do not build a desktop. Build an **Intent Field**.

The LogansOS GUI should be one living, capability-secured workspace where the owner and Arthur share the same screen, the same action trail, and the same truth. Conversation is not a side panel. `neo>` is not a separate terminal. Apps are not ambient rectangles competing for attention. They are **capability-bound instruments** that appear inside a focused field of intent, with their authority, liveness, provenance, and undo trail visible as part of the interface.

The first-principles sentence:

> LogansOS is a screen where intent becomes visible, reversible work.

That is the thing only this OS can build. macOS and Windows can make better windows. LogansOS can make the first GUI where the system's real substrate - Arthur, capabilities, reversibility, local inference, and owned hardware - is the interface.

The boldest bet: **the main unit of UI is not a window or an app; it is a Workfield** - a live visual object rooted in a user intent, containing the transcript turn, the deterministic `neo>` equivalent, the surfaces it summoned, the caps it consumed, the liveness state of any model/action work, and the undo/provenance trail. A five-year-old can point at the thing Arthur is doing. A power user can drop into its exact command graph.

---

## Local source spine

- `docs/bible.md` Article 1.5: five-year-old usability, conversation-first, usability as a first principle.
- `docs/bible.md` Article 1.6: beauty-as-craft, grammar-not-uniformity, distinct silhouettes as visual recognition.
- `docs/OPEN-THREADS.md` OT-52: own cap-native compositor, co-designed with OT-45.
- `docs/OPEN-THREADS.md` OT-45: visual design language, beauty-as-craft, Tahoe/Liquid-Glass icon caution.
- `docs/OPEN-THREADS.md` OT-26: Arthur agentic layer, transcript-as-UI, `neo>` duality, A1/A2 shipped, A3 next.
- `docs/OPEN-THREADS.md` OT-46: Arthur butler persona; voice deferred; push-to-talk/local-only.
- `docs/OPEN-THREADS.md` OT-44: apps as cap-bundles/WASM direction, install-is-data, H2 gated on OT-22.
- `docs/THINK-2026-07-07-usability-five-year-old-design.md`: F1-F8 five-year-old gate.
- `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md`: S1-S8/T1-T4 duality gate.
- `docs/VISION-arthur-persona.md`: authoritative Arthur register.
- `kernel/src/drivers/fb/console.rs`, `console_logic.rs`, `fb/mod.rs`, `init/boot_frame.rs`: current framebuffer/console/boot visual substrate.

RAG was healthy at read time, but `last_indexed` was `2026-07-05`, so I used it only for older code discovery and read the July 8 docs directly.

---

## ISC-1 - The paradigm: Intent Field, not desktop

### 1.1 The core form

The first screen after the v1 console grows into a **single full-screen field** with four native layers:

1. **The Intent Spine** - the conversation transcript, but made spatial and alive. Arthur turns, owner turns, `neo>` turns, clarifications, grants, failures, and undo notes appear in one durable trail. This preserves v1's transcript-as-UI and makes it visual rather than replacing it.
2. **Workfields** - live task regions spawned by intent. "Open the thing from yesterday" creates a workfield containing the candidates, the selected file, the cap checks, and the action trail. "Clean Downloads" creates a workfield containing a before/after preview and its undo drawer.
3. **Instruments** - app/client surfaces summoned inside a workfield. A text editor, image viewer, graph, file browser, or future third-party cap-bundle is not "launched"; it is **borrowed into the current intent** with explicit caps.
4. **The Deterministic Rail** - `neo>` is always present as the exact command layer. It can be expanded inline from any workfield: "show me the command graph", "copy as script", or simply `neo>`.

The owner never asks "which app do I need?" They ask for the thing. Arthur and/or `neo>` map intent to action. The GUI then makes the action inspectable, reversible, and beautiful.

### 1.2 What it feels like

Cold boot:

```text
LOGANS OS
Arthur: Good afternoon. I'm Arthur - I look after this machine for you.
        Tell me what you'd like, in your own words, and I'll see to it.
        If ever you'd rather do it yourself, "help" shows you the way.
```

The user says: "Find my drawing from yesterday."

The screen does not open a file manager. A workfield blooms from the transcript turn: three candidate objects, each with a distinct silhouette, timestamp, and tiny provenance trace. Arthur asks which one. The child points. The power user presses the rail and sees:

```text
neo> files where modified:yesterday kind:image | choose
```

The result appears as the same object either way. One substrate, two entry points.

### 1.3 Why this beats the desktop metaphor

The desktop metaphor starts with nouns the user must learn: windows, folders, apps, menus, dialogs, launchers, permissions. Intent Field starts with the user's sentence and reveals nouns only when they matter.

The desktop hides authority behind modal permission theater. Intent Field shows authority as part of the object: a surface has the caps it was granted, the workfield shows why it needed them, and denial offers the grant path.

The desktop lets action history dissolve. Intent Field keeps a visible action trail and undo shadow. OT-19 is not an internal safety feature; it is an object the owner can see.

The desktop makes the assistant an app. Intent Field makes Arthur a native presence, but always dismissable. Arthur can act, clarify, narrate, and step aside. The `neo>` rail is the Arthur-free path.

### 1.4 The design vocabulary

- **Workfield:** the visual container for a task/intent, not an app.
- **Instrument:** a client/app surface borrowed into a workfield under caps.
- **Intent Spine:** persistent conversation/command/action transcript.
- **Command Graph:** exact deterministic representation of the interpreted work.
- **Cap Halo:** visible authority boundary around a workfield/instrument.
- **Undo Shadow:** visible, time-bounded reversibility/provenance object.
- **Liveness Thread:** model/action progress indicator tied to real work, never fake motion.

---

## ISC-2 - Beauty: craft, distinction, and living material

### 2.1 The design language

The visual language should be **luminous instrument craft**: precise, calm, fast, and unmistakably alive.

Not generic glass. Not purple gradients. Not cards in cards. Not a Mac clone. The grammar:

- **Deep neutral field:** black/near-black or warm off-white depending on mode, but with enough chroma variety that the system does not become one-note.
- **Purposeful light:** light is data. A workfield's glow indicates liveness, focus, or authority; not decoration.
- **Distinct silhouettes:** every object category has shape identity. Files, apps, people, caps, undo, model work, and devices must be recognizable at thumbnail scale before labels are read.
- **Readable type before spectacle:** the transcript and command rail need a serious text face with clear numerals and punctuation. Display type is reserved for boot, empty states, and large workfield labels.
- **Motion as state transition:** motion shows cause and ownership - intent spawns workfield, cap grant widens halo, undo shadow attaches to mutation, liveness thread advances during real model work.
- **Material hierarchy:** foreground is crisp; background is quiet; translucent effects are used only where they preserve legibility and spatial relationship.

Apple's current HIG says icon recognition matters, and Apple itself provides Liquid Glass as a dynamic material. Rogue Amoeba's Tahoe critique is the caution: if the material and uniform shape crush silhouette, beauty becomes anti-usability. LogansOS should take the craft of layered material and reject enforced sameness.

### 2.2 Visual rules

**Rule B1 - Grammar, not uniformity.** Use shared spacing, optical rhythm, depth, and motion, but preserve object identity. A "file" should not become the same rounded tile as an "app" as a "cap" as a "device".

**Rule B2 - Authority is visible but not scary.** Capability boundaries should feel like ownership, not warnings. A green/red permission dialog is Windows thinking. A cap halo with an exact scope label is LogansOS thinking.

**Rule B3 - Liveness is beautiful because it is honest.** An Arthur forward pass gets a real liveness thread. A disk operation gets a real progress path or a small "waiting on device" state. No fake spinners.

**Rule B4 - The system never steals the work.** Liquid/blur/chrome must not compete with the user's content. Rams's "less, but better" applies: the material clarifies the product's structure or it goes.

**Rule B5 - Motion is short, causal, interruptible.** Apple's motion guidance emphasizes brief, precise feedback. The Intent Field should animate transitions because they explain where state came from, not because the GPU can.

### 2.3 Concrete visual primitives

- **Cap Halos:** thin, colored rings/edges around instruments. The color is semantic by cap class, but shape/pattern distinguishes class for accessibility.
- **Undo Shadows:** a quiet ghost strip behind mutable objects, showing retained preimage/restore affordance.
- **Arthur Presence:** not an avatar. A slim liveness/attention mark on the spine, plus voice/text tone. The butler should not become a mascot.
- **Object Silhouettes:** file types and app bundles use distinct outlines, not just same-shaped icons. This is Article 1.6's five-year-old visual recognition.
- **Focus Depth:** the focused workfield gets clarity and contrast; inactive fields recede into lower-frequency texture, not blur mush.
- **Command Graph:** deterministic rail uses monospaced structure, typed object pipeline affordances, and cap-denial explanations that mirror Arthur's plain-language form.

---

## ISC-3 - Responsiveness: instant or it is not beautiful

### 3.1 Targets

These are product targets for the GUI phase, not claims about today's console:

| Surface | Target |
|---------|--------|
| Pointer/key/touch/voice activation feedback | Acknowledge in the next frame; target < 8.33ms at 120 Hz, < 16.67ms at 60 Hz |
| Local visual transition | 120 Hz-class where hardware supports it; no missed frame during normal interaction |
| Compositor frame budget | 8.33ms total at 120 Hz; reserve 2-3ms for composition on GPU path, lower for unchanged/damage-only frames |
| `neo>` input echo and completion UI | visually immediate; no model path involved |
| Arthur model work | liveness appears immediately; tokens/actions stream as they become real; never block pointer/typing/compositor |
| Under load | UI remains schedulable above bulk inference; model slowdown is acceptable, pointer/compositor starvation is not |

NVIDIA Reflex and modern frame-pacing work are the relevant lesson: queue depth and unbounded render-ahead are enemies of felt latency. Android's SurfaceFlinger/frame pacing docs show that presentation timing and phase alignment are first-class systems problems, not polish.

### 3.2 Pipeline

The GUI pipeline should be:

```text
input IRQ / HID
  -> input router tags event with target surface cap
  -> compositor intent queue (priority lane)
  -> client/instrument handles event or compositor handles shell gesture
  -> dirty region / scene mutation
  -> retained scene graph diff
  -> GPU composite, or software fallback
  -> present fence / presentation feedback
```

Critical choices:

- **Separate UI and inference scheduling.** Arthur can think slowly; the surface must not. The liveness thread is the honest bridge.
- **Retained scene graph for the shell/compositor.** Borrow from Core Animation: layers/surfaces hold geometry/state; composition can animate state changes without clients redrawing every pixel.
- **Damage-first repaint.** Recompose the parts that changed. The Wayland architecture's damage model is the right primitive.
- **Latest-input wins for continuous gestures.** Pointer movement, touch drags, and scroll should collapse stale events. Text input should not.
- **Presentation feedback.** The compositor must know when a frame became visible enough to compute real frame timing and catch jank.
- **Backpressure, not buffer hoarding.** Clients get a small, explicit frame credit. If they miss, the compositor preserves the last good surface rather than stalling the field.

### 3.3 Why LogansOS can beat legacy stacks

LogansOS has three structural advantages:

1. **No X11/Wayland/Mesa legacy round trips in the native path.** Our native app model is libneo/caps/cap-bundles, so the compositor can be designed around the actual substrate.
2. **Capability-native buffer ownership.** A surface buffer is a cap. A compositor does not infer authority from a socket, fd, or ambient process identity; it is handed a precise object.
3. **AI scheduler awareness.** The kernel already treats inference as a first-class workload. The GUI can reserve compositor/input priority while model work yields or degrades gracefully.

The bet is not "we can out-optimize everyone on day one." The bet is that a narrower, cap-native, AI-aware pipeline has fewer unavoidable latency costs than a compatibility desktop stack.

---

## ISC-4 - Compositor architecture: cap-native, ours, with honest Wayland borrowing

### 4.1 Decision

Build our own minimal cap-native compositor protocol. Do not adopt X11. Do not make Wayland the native protocol. Borrow Wayland's good primitives and leave the Linux-desktop assumptions behind.

### 4.2 Surface model

Core objects:

- `SurfaceCap` - authority to own/update a rectangular or shaped visual surface within a workfield.
- `BufferCap` - authority to write a pixel buffer or vector command buffer; derived from memory/DmaBuffer caps.
- `InputFocusCap` - authority to receive selected classes of input while focused.
- `PresentFenceCap` - feedback that a submitted frame reached, missed, or was superseded at presentation.
- `CaptureCap` - explicit screenshot/recording authority; absent by default.
- `DecorationCap` - authority for chrome/material supplied by the compositor, not by clients.

Lifecycle:

```text
client asks parent/workfield for a SurfaceCap
  -> parent grants region + input classes + cap label
  -> client allocates one or more BufferCaps
  -> client submits frame descriptor + damage rects + frame serial
  -> compositor validates cap + damage + size
  -> compositor composites final field
  -> compositor sends PresentFence feedback
```

The compositor owns final composition. A client never draws another client's pixels. A client never receives global input. Screen capture is a separate explicit cap.

### 4.3 Trust boundary

The compositor is a trusted userland service, not a policy-brain in the kernel. The kernel enforces:

- surface/buffer/input caps are unforgeable;
- client memory access is bounded;
- DMA buffers are fenced;
- input delivery goes only through routed caps;
- a dead client loses its surfaces.

The compositor decides layout and beauty. The kernel enforces "cannot exceed".

### 4.4 Software-first, GPU-next

Phase 1 of GUI compositor: software composite over the framebuffer with damage tracking and a tiny surface set. It proves the protocol and the paradigm.

Phase 2: GPU-composited surfaces through our accel path. Intel Gen9 work already proved real GPU compute and DmaBuffer/GGTT discipline in the AI lane; the GUI should reuse the ownership lessons, not Mesa/DRM.

Phase 3: GPU vector/text pipeline. Candidates to study:

- Skia Graphite's modern GPU backend and overdraw reduction ideas.
- Vello's Rust GPU-compute 2D renderer direction.
- Rive's vector path triangulation for animated vector surfaces.
- MSDF/SDF text for scalable UI labels where full font shaping is not needed.

Text shaping is a genuine hard part. For production text, do not pretend SDF alone solves shaping, hinting, fallback, complex scripts, and accessibility. Use the simple path for system labels; design a real text stack before localization is load-bearing.

### 4.5 Wayland steelman

The strongest case for Wayland's protocol:

- It is proven as a minimal compositor/client split.
- It already models client-rendered buffers, damage, and compositor-owned final output.
- It has ecosystem familiarity for developers.
- Future Linux app compatibility might become easier if we can run Wayland clients.

Why I still reject it as native:

- Wayland's practical stack assumes Linux KMS/evdev/DRM/GBM/EGL/Mesa and UNIX-style fd passing. Our stack is libneo, caps, DmaBuffer-like kernel objects, and our own accel path.
- Wayland solves client isolation at the compositor/protocol level. LogansOS has unforgeable kernel capabilities, so the native protocol should expose those directly rather than remapping them through Linux abstractions.
- Wayland is still desktop-shaped: surfaces, toplevels, popups, pointer constraints, decorations. Intent Field needs workfields, cap halos, undo shadows, Arthur liveness, and command graphs as first-class objects.
- OT-44 says native apps are cap-bundles/WASM/cross-built libneo apps, not existing libwayland clients. Compatibility is not the core strategy.

Adopt:

- compositor owns final output;
- client allocated buffers;
- explicit damage;
- presentation feedback;
- per-surface input focus;
- "no global snooping" posture.

Reject:

- Wayland protocol as native ABI;
- Mesa/DRM/GBM dependency;
- desktop toplevel/window model as the primary UX;
- implicit app trust via process/session.

Future fork: build a bounded Wayland-personality adapter only if the PL wants Linux app compatibility. That adapter should be kill-switched, non-native, and cap-translated.

### 4.6 X11 is dead on arrival

X.Org's own guide states the core X protocol grants a connected client broad access, including monitoring devices and keystrokes. That is structurally incompatible with capability security. X11 is useful here only as a cautionary tale.

---

## ISC-5 - Best practices and bleeding-edge bets

### 5.1 Proven practices to take

| Practice | Source | LogansOS use |
|----------|--------|--------------|
| Retained visual state/layers | Apple Core Animation | Compositor owns a retained scene graph; clients do not redraw everything for every transition. |
| Compositor final ownership | Wayland architecture, DirectComposition | Compositor owns final output and transforms; clients submit surfaces/buffers. |
| Client buffers + damage | Wayland architecture | BufferCap + damage rects are native objects. |
| Frame pacing + presentation feedback | Android SurfaceFlinger docs, Vulkan present wait/timing | GUI has real jank accounting and present fences. |
| Declarative/reactive updates | React render/commit, Flutter rendering pipeline, SwiftUI data dependencies | Workfields are state objects; UI is derived/diffed from state. |
| Brief/precise feedback motion | Apple HIG motion guidance | Liveness/motion explains state, never decorates. |
| Distinct, memorable icons | Apple HIG app-icon guidance and the Tahoe critique | Distinct silhouettes are a usability requirement, not nostalgia. |
| Utility-first craft | Dieter Rams/Vitsoe principles | Beauty serves use; material that harms clarity is cut. |
| Capability-oriented distribution | WASI/component model as adjacent prior art, OT-44 local decision | Cap-bundles declare authority; install is data; first run grants caps. |

### 5.2 Bleeding-edge bets I would take

**Bet 1 - Workfield as primary primitive.** No mainstream desktop has this because their substrate is app/window-centered. LogansOS can build it because Arthur/action/cap/undo are already system concepts.

**Bet 2 - Visible capability UI.** Permissions become spatial, visible, and inspectable. This can beat consent dialogs if it is calm and exact.

**Bet 3 - Reflex-style UI scheduling.** The compositor should be treated like a latency-sensitive real-time workload. Inference and bulk compute yield to input/composition, and the GUI reports actual frame timing.

**Bet 4 - GPU-compute 2D pipeline after MVP.** Vello/Graphite/Rive show credible directions for modern GPU 2D. We should not import them blindly, but the compositor should be designed so its scene graph can lower to a GPU command stream later.

**Bet 5 - AI-native liveness as a visual material.** The "thinking" state is not a spinner; it is a measured channel: tokens generated, action candidate, cap-check, wait-on-device, undo write, present complete. This is novel, legible, and defensible.

### 5.3 Bets I would not take early

- Full Wayland compatibility in the native compositor.
- A window manager that mimics macOS/Windows to feel familiar.
- Heavy translucent glass as a universal material.
- Voice-first GUI before push-to-talk STT and audio DMA fencing exist.
- GPU-only compositor before the software protocol proves the UX.
- On-box app ecosystem before OT-22 multi-principal and OT-44 H2 are ready.

---

## ISC-6 - AI-native integration: Arthur lives in the field

Arthur should not be a floating chat bubble. Arthur is the system's steward presence on the Intent Spine.

### 6.1 Arthur states

- **Available:** quiet mark, ready for text/voice/command.
- **Listening:** only on explicit push-to-talk or text focus; no wake-word surveillance.
- **Thinking:** real forward pass liveness; show token/action progress or honest stage.
- **Clarifying:** candidates become visual objects; user can speak, click, or type.
- **Acting:** action path lights through the command graph; cap-check and undo writes appear as stages.
- **Blocked:** denied cap or failed device gets a calm explanation plus next path.
- **Stepping aside:** `neo>` expands; Arthur does not hover or interfere.

### 6.2 Clarify-don't-guess as UI

The clarify branch should render as a choice surface, not only text.

Example:

```text
Arthur: I've two candidates from yesterday, sir - the budget spreadsheet or the roofing quote.
```

On screen: two candidate objects with previews, distinct silhouettes, source paths, and caps required to open. A child picks one. A power user can type:

```text
neo> choose 2
```

### 6.3 Safe-to-be-wrong as UI

When Arthur mutates:

- the object gets an undo shadow;
- the command graph marks the OT-19 copy-before-write stage;
- the result says what happened and how long recovery is available;
- high-stakes/irreversible actions require the exact confirmation surface from OT-18/OT-19.

This makes safety visible without turning every action into a consent ceremony.

### 6.4 Power-user duality

Every workfield should be convertible into a deterministic form:

- "Show command" reveals `neo>` pipeline.
- "Copy as script" turns the action graph into repeatable commands.
- "Run again" repeats the command graph, not the model interpretation.
- Denied commands name the missing cap and offer the grant route.

This honors the PL's object-pipeline decision: `neo>` emits typed records; table text is rendering, not wire format.

### 6.5 Voice future

When OT-46B opens, voice input should plug into the same field:

- push-to-talk only;
- STT-only first is acceptable: the transcript remains the output;
- voice waveform/liveness is local to the spine;
- no always-listening background mode;
- audio driver/DMA fencing is a build prerequisite.

The five-year-old test becomes literal only when voice input exists, but the GUI should be ready for it now.

---

## ISC-7 - Honest forks and buildable spine

### 7.1 Hard forks requiring PL ratification

**Fork A - Native protocol only vs bounded Wayland adapter.** Recommendation: native cap protocol first; optional Wayland adapter later as a personality, not a core contract.

**Fork B - Workfield-only vs optional freeform windows.** Recommendation: workfield-first. If windows exist, they are an instrument mode inside workfields, not the system metaphor.

**Fork C - Surface buffer format.** Pixel buffers first, vector command buffers later. Do not block MVP on vector scene interchange.

**Fork D - Compositor trust placement.** Recommendation: trusted userland compositor with kernel-enforced caps. Do not put layout/beauty policy in ring 0.

**Fork E - Text stack.** Recommendation: bitmap/atlas labels for MVP; plan a real text shaping/rendering slice before localization/accessibility pressure.

**Fork F - Visual cap model.** Recommendation: visible cap halos by default for system/workfield transitions, subtle when stable. PL must choose how prominent authority should be in everyday use.

**Fork G - GPU path.** Recommendation: software protocol MVP, GPU composition as the second proof, GPU vector/text as the third.

### 7.2 Real blockers

- **OT-22 multi-principal frontier.** A compositor is where principals' surfaces coexist. Do not build multi-client GUI until the principal/cap story is ready.
- **Audio drivers for voice.** Voice is Phase-N until HDA/USB-audio and DMA fencing exist.
- **App installation H2.** Third-party GUI apps need cap-bundle install/verify/place/register/first-run-grant. OT-44 H2 and OT-22 gate this.
- **Scheduler/compositor priority.** GUI responsiveness requires explicit scheduling posture against inference and bulk compute.
- **Accessibility.** Distinct silhouettes help, but full accessibility needs keyboard navigation, reduced motion/transparency, screen-reader path, high contrast, and voice/text alternatives.
- **Physical legibility.** OT-41's OptiPlex proof style must become a standing GUI gate: QEMU proves flow; hardware proves visual truth.

### 7.3 Smallest first GUI slice

**GUI-0: Intent Field prototype, single-principal, no app ecosystem.**

Goal: prove the paradigm on real hardware without opening multi-principal complexity.

Scope:

- full-screen framebuffer surface;
- v1 transcript becomes Intent Spine;
- one workfield type: "find/open file" or "clean folder";
- Arthur turn + `neo>` equivalent + candidate objects + cap denial/grant display;
- OT-19 undo shadow for one reversible mutation if available;
- real liveness thread during model/action work;
- no third-party app clients;
- no general compositor protocol yet.

This slice answers: does Intent Field feel obviously better than a terminal and less stale than a fake desktop?

### 7.4 Build sequence

| Stage | Deliverable | Gate |
|-------|-------------|------|
| **GUI-0** | Single-principal Intent Spine + one workfield on framebuffer | QEMU flow + framebuffer nonblank + physical legibility; F1-F8 and S/T duality check |
| **GUI-1** | Software cap-native compositor MVP: SurfaceCap, BufferCap, input focus, damage, present feedback | Host KATs for caps/damage; QEMU two-surface smoke; no global input/capture |
| **GUI-2** | Workfield protocol: command graph, cap halo, undo shadow, liveness thread as native compositor objects | A1/A2/A3 action-path gates plus visual proof |
| **GUI-3** | One first-party instrument: file/object browser or text editor inside a workfield | Same substrate visible from Arthur and `neo>` |
| **GUI-4** | OT-44 H2 cap-bundle visual install/first-run grant surface | Install-is-data proof; no vendor code at install; cap manifest visible |
| **GUI-5** | GPU composition through our accel path | frame pacing/present feedback; 120 Hz-class target on capable hardware; software fallback |
| **GUI-6** | Visual language full pass: typography, icon/silhouette system, motion/material rules, accessibility profiles | Article 1.6 gate; reduced motion/transparency; high contrast; hardware proof |
| **GUI-7** | Optional compatibility adapter(s): Wayland/personality if ratified | Bounded, kill-switched, non-native, cap-translated |

### 7.5 Acceptance bar

The first real GUI phase should not close unless it proves:

1. The owner can boot and immediately know what to do.
2. Arthur's state is visible and honest.
3. `neo>` is visible and instant.
4. A workfield exposes action, authority, and undo without a modal permission ritual.
5. No client can see pixels/input it lacks caps for.
6. UI feedback survives inference load.
7. The visual language preserves distinct silhouettes.
8. Physical framebuffer proof shows it is actually legible and beautiful on the PL's machine.

---

## Return summary

**THINK path:** `docs/THINK-2026-07-08-gui-vision-jarvis.md`

**Boldest paradigm bet:** Workfield, not window. The GUI is an Intent Field where every visible object is rooted in a user intent and carries its command graph, caps, liveness, and undo trail.

**Compositor bet:** own cap-native protocol; borrow Wayland's compositor/buffer/damage lessons but reject Wayland as native.

**Beauty bet:** luminous instrument craft; grammar-not-uniformity; distinct silhouettes; material only where it clarifies state.

**Responsiveness bet:** compositor/input are priority workloads; Arthur/model work is always honest and never blocks the field.

**First buildable slice:** single-principal Intent Field prototype on the existing framebuffer path, before multi-client compositor and before app ecosystem.

**CI note:** docs-only. No QEMU should exist. Run host close gate only per dispatch.

---

## Sources

### External sources

- Apple Human Interface Guidelines - Materials: https://developer.apple.com/design/human-interface-guidelines/materials
- Apple Human Interface Guidelines - App icons: https://developer.apple.com/design/human-interface-guidelines/app-icons
- Apple Human Interface Guidelines - Motion: https://developer.apple.com/design/human-interface-guidelines/motion
- Apple Core Animation Programming Guide - Core Animation Basics: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CoreAnimation_guide/CoreAnimationBasics/CoreAnimationBasics.html
- Apple Liquid Glass overview: https://developer.apple.com/documentation/technologyoverviews/liquid-glass
- Apple "Meet Liquid Glass" WWDC25: https://developer.apple.com/videos/play/wwdc2025/219/
- Rogue Amoeba, "Free The Icons": https://weblog.rogueamoeba.com/2026/06/26/free-the-icons/
- Vitsœ / Dieter Rams, "Good design": https://www.vitsoe.com/us/about/good-design
- Wayland architecture: https://wayland.freedesktop.org/architecture.html
- X.Org guide - core X protocol security model: https://www.x.org/guide/communication/
- Microsoft DirectComposition overview: https://learn.microsoft.com/en-us/windows/win32/directcomp/directcomposition-portal
- Android Open Source Project - Frame pacing: https://source.android.com/docs/core/graphics/frame-pacing
- NVIDIA Reflex SDK: https://developer.nvidia.com/performance-rendering-tools/reflex
- NVIDIA Reflex latency pipeline article: https://www.nvidia.com/en-us/geforce/news/reflex-low-latency-platform/
- Vulkan `VK_KHR_present_wait`: https://docs.vulkan.org/refpages/latest/refpages/source/VK_KHR_present_wait.html
- Khronos `VK_EXT_present_timing` background: https://www.khronos.org/blog/vk-ext-present-timing-the-journey-to-state-of-the-art-frame-pacing-in-vulkan
- React render/commit: https://react.dev/learn/render-and-commit
- Flutter architectural overview/rendering pipeline: https://docs.flutter.dev/resources/architectural-overview
- SwiftUI model data/declarative UI: https://developer.apple.com/documentation/swiftui/model-data
- Skia Graphite announcement: https://blog.google/chromium/introducing-skia-graphite-chromes/
- Vello GPU-compute 2D renderer: https://github.com/linebender/vello
- Rive Renderer open-source announcement: https://rive.app/blog/rive-renderer-now-open-source-and-available-on-all-platforms
- MSDF generator: https://github.com/chlumsky/msdfgen
- WebAssembly Component Model: https://component-model.bytecodealliance.org/
- WASI overview: https://wasi.dev/

### Local sources

- `Plans/DISPATCH-2026-07-08-gui-vision-think-cerebro-and-jarvis.md`
- `docs/bible.md`
- `docs/OPEN-THREADS.md`
- `docs/THINK-2026-07-07-usability-five-year-old-design.md`
- `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md`
- `docs/VISION-arthur-persona.md`
- `docs/RESEARCH-2026-07-08-ot46-voice-substrate-FINDINGS.md`
- `docs/THINK-2026-07-05-ot44-sdk-v0.md`
- `Plans/THINK-2026-06-21-arthur-agentic-action-design.md`
- `Plans/DISPATCH-2026-07-08-v1-console-liveness-surface-janus.md`
- `kernel/src/drivers/fb/console.rs`
- `kernel/src/drivers/fb/console_logic.rs`
- `kernel/src/drivers/fb/mod.rs`
- `kernel/src/init/boot_frame.rs`
