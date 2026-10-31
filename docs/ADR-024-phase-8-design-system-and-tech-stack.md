# ADR-024: Phase 8 Desktop Design System & Tech Stack

| | |
|---|---|
| **Status** | ✅ Accepted (ratified 2026-05-28 by PL) — **AMENDED 2026-07-19 (glass material, ratified by PL); see Amendments** |
| **Date** | 2026-05-28 |
| **Authority** | PM + Design Lead |
| **Supersedes** | None |
| **Superseded by** | - |
| **Amendments** | (1) 2026-07-08 — the Atrium reconciliation (cyberpunk-maximalism → classic-Mac craft bar + neon-as-*meaning* + the §12 "Threshold" resting state; Arthur felt-not-avatared), `docs/THINK-2026-07-10-atrium-design-language.md`. (2) **2026-07-19 — the frosted-glass material** (this ADR's material substrate is glass; trust-state-as-tint; the Reduce-Transparency invariant), ratified by PL 2026-07-19, `docs/THINK-2026-07-19-atrium-glass-frost-amendment.md`. |
| **Related** | Bible Articles 1.2 (correctness priority), 4.2 (Rust monolith), 8.1 (multi-class scheduler), 23.1 (no system AI persona); ANSPEC-001 (AI subsystem); `docs/research/ALTIMIT-real-world-implementation.md` (visual design principles, cyberpunk aesthetic); Phase 8 project plan; OT-52 (Atrium compositor), OT-45 (visual design language), OT-63 (the GPU the glass needs) |
| **Ratified by** | Project Lead (2026-05-28; glass amendment 2026-07-19) |

---

## Context

AltimitNeo is complete through Phase 7 (inference scheduler, multi-tenant model serving). Phase 8 is "Userland: window system, shell, package management, native applications." The kernel is mature. Now the user-facing experience must match the OS's innovative internals.

The ambient pressure in the industry is toward "AI-augmented desktops" (Windows 11 24H2, GNOME AI extensions, macOS Copilot integration). Bible Article 23.1 rejects this posture: AltimitNeo has no system persona. But AltimitNeo's substrate *does* run AI workloads at kernel level. Phase 8 must celebrate that distinction in its visual design and UX.

Current state:
- **Phase 7 complete:** Kernel can run multiple inference jobs, schedule across accelerators, manage tensors.
- **Phase 8 scope:** Build a desktop environment that makes this visible and usable.
- **Design direction:** Cyberpunk aesthetic (neon, dark, information-dense, real-time metrics visible) rather than macOS minimalism or Windows Metro.
- **Tech approach:** All Rust userland (egui, wgpu, Smithay), no C/C++ dependencies in core UI layer.

The design mockup (created 2026-05-28 by design team) shows:
- Neo-shell (terminal + inference runner)
- Neo-monitor (system dashboard with real-time GPU/tensor/inference metrics)
- 3D tensor-flow visualization
- Unified design system (colors, typography, spacing, components)
- Sidebar navigation emphasizing AltimitNeo's unique subsystems (TENSOR STORE, MODELS, TERMINAL, DOCS, SETTINGS)

This ADR proposes:
1. **Formal commitment to the cyberpunk visual identity** as first-class design constraint
2. **Tech stack selection** (egui, wgpu, Smithay, Alacritty fork) with rationale
3. **Phasing breakdown** (8.1–8.5, 13 months) with realistic milestones
4. **GPU metrics strategy** (FFI to vendor APIs + sysinfo + sysfs fallback)

---

## Decision

**Phase 8 will deliver a complete, production-grade desktop environment with the following characteristics:**

### 1. Visual Identity

- **Aesthetic:** Cyberpunk (neon on dark, information-dense, no hidden metrics)
- **Color palette** (sampled from the LogansOS identity — a blue lens-flare star inside silver ring arcs):
  - Background: #0a0e27 (void)
  - Primary accent (system): #3c8ef4 (electric blue)
  - Structure / secondary: #a8adb2 (silver — ring arcs, borders, secondary text)
  - Alert / fatal **only**: #ff00ff (magenta)
  - Success: #00ff41 (lime green)
  - Warning: #ffaa00 (amber)
  - Text: White or Silver on dark (high contrast)
- **Typography:**
  - Body/system: IBM Plex Mono (monospace, native to cyberpunk)
  - UI labels: Inter (sans-serif, minimal)
  - Baseline: 12px, 1.5 line-height
- **Spacing grid:** 8px base unit (all padding, gaps, margins multiples of 8px)
- **Philosophy:** Make the system visible. Real-time metrics (CPU, GPU, memory, inference load, deduplication savings) are displayed prominently, not hidden. The user should feel they are piloting a real machine.

### 2. Tech Stack

| Layer | Decision | Rationale |
|-------|----------|-----------|
| **Rendering** | wgpu + winit | Cross-platform GPU abstraction (Vulkan/Metal/DX12). 100% Rust. Production-grade (Mozilla WebGPU). Gives pixel-perfect control. |
| **UI Framework** | egui | Immediate-mode UI. Flexible layout for custom dashboards. Perfect for real-time metrics. 100% Rust. |
| **Compositor** | Smithay (Wayland) | Standard modern compositor abstraction. Rust-native. Alternative: custom minimal compositor; deferred to 8.5. |
| **Terminal** | Alacritty (fork + theme) | Already Rust + wgpu. Apply neon theme. Faster than building from scratch. |
| **3D visualization** | bevy OR wgpu + glam | Bevy: full game engine, proven for real-time 3D. Wgpu: minimal deps, hand-written shaders. Decision deferred to 8.4. |
| **System metrics** | sysinfo + vendor APIs | sysinfo for CPU/mem/network (100% Rust). Vendor FFI (NVIDIA NVML, AMD ROCM, Intel GPU tools) for real-time GPU load. |
| **Icon/asset system** | Custom or design-token system | Unified design system stored as structured data (colors, spacing, component specs). |

**All code is Rust.** No C/C++ dependencies in core UI layer except vendor GPU APIs (thin FFI wrappers).

### 3. Phasing (Phase 8.1–8.5, ~13 months)

| Sub-phase | Duration | Deliverable | Dependencies |
|-----------|----------|-------------|--------------|
| 8.1 | 3 months | Basic Wayland compositor + egui shell (no 3D) | wgpu, winit, egui, Smithay bindings |
| 8.2 | 3 months | Neo-monitor dashboard (real-time CPU, GPU, tensor, inference metrics) | sysinfo, GPU vendor APIs |
| 8.3 | 2 months | Terminal emulator (fork Alacritty, apply neon theme) | Alacritty codebase + rustfmt refactor |
| 8.4 | 3 months | 3D tensor-flow visualization (WebGPU shaders + camera controls) | bevy or wgpu + glam |
| 8.5 | 2 months | Icon set, design system polish, component library, documentation | Design assets (PNG, SVG) + Rust component lib |
| **Total** | **13 months** | Production-grade Phase 8 desktop | All above |

### 4. Core Applications

Minimum viable Phase 8:

1. **neo-shell:** Terminal + inference job runner. Syntax for binding models, inputs, outputs.
2. **neo-monitor:** Dashboard. Real-time metrics (CPU, GPU, memory, inference load, dedup savings, I/O).
3. **neo-editor:** Text editor for model imports, scripts. (Minimal; could defer to Phase 8.5+.)
4. **neo-docs:** Built-in kernel documentation + lore + API reference.
5. **neo-settings:** System configuration (theme variants, GPU assignment, inference priorities).

Anti-scope:
- No graphical file manager (yet). Userspace can ship one; kernel doesn't require it.
- No complex application ecosystem. Phase 8 is the base desktop; Phase 8+ apps come later.

### 5. Governance

- **Visual design decisions** (color palette, typography, spacing) are owned by the design lead and documented in `docs/design-system.md` (to be created).
- **Tech stack decisions** (egui vs. iced, Smithay vs. custom, bevy vs. wgpu) are owned by the PM + architecture review.
- **Component implementations** (buttons, inputs, progress bars, alerts) follow the design system spec; any divergence requires a design review before merge.

---

## Rationale

### Why Cyberpunk, Not Minimal or Skeuomorphic?

**Rejected alternatives:**
- macOS Aqua style (skeuomorphic, visual depth) — too much visual noise; doesn't scale to information density AltimitNeo requires.
- Windows 11 Fluent Design (glass morphism + pastel) — fine for generic desktop; doesn't celebrate AltimitNeo's unique mission (AI-native kernel).
- Minimalist monochrome (GNOME, elementary) — beautiful but invisible. Hides system activity.

**Cyberpunk chosen because:**
1. It is **honest about complexity.** A neon grid says "this is a powerful machine with things happening." Users of an AI-native OS *want* to see the work.
2. It **matches the lore.** The .hack franchise (inspiration for this project's ALTIMIT aesthetic) is fundamentally cyberpunk. A cyberpunk desktop is on-brand.
3. It **scales to information density.** Cyberpunk UIs are *designed* to show many metrics simultaneously (GPU load, tensor memory, inference queue, network I/O) without looking cluttered.
4. It **differentiates AltimitNeo.** No other OS ships a cyberpunk desktop by default. It makes the first-boot experience memorable.

### Why egui + wgpu, Not iced or GTK?

**Rejected alternatives:**
- **iced** (Elm-style declarative UI): Type-safe, elegant. Weaker layout control. Harder to build the custom dashboards neo-monitor requires. Better for form-based apps; worse for real-time visualization.
- **GTK** (C, GNOME native): Powerful, mature. Introduces a C dependency. Heavy. Not Rust-idiomatic. Misaligns with monolithic Rust kernel philosophy.

**egui + wgpu chosen because:**
1. **Pixel-perfect control.** Immediate-mode rendering means we can layout exactly as the design spec demands, no fighting a declarative abstraction.
2. **Real-time performance.** egui is optimized for constant redraw (60+ FPS dashboards). Immediate mode is the right paradigm for "show real-time metrics."
3. **100% Rust.** Type safety all the way. No FFI surprises (except vendor GPU APIs, which are thin wrappers).
4. **Proven.** Bevy, Egui itself, and many games use this stack successfully.

### Why Smithay (Wayland) for Compositor, Not X11 or Custom?

**Rejected alternatives:**
- **X11:** Legacy, deprecated, insecure by design. Does not align with "crash-immune by construction" (Article 1.2).
- **Custom minimal compositor:** 100% control. 12+ months of work. Higher risk.

**Smithay chosen because:**
1. **Standards-based.** Wayland is the modern standard. Every toolkit is moving toward it.
2. **Rust-native.** Smithay is the canonical Rust Wayland library. We're not learning a foreign codebase.
3. **Reference implementation available.** Smithay ships example compositors we can fork and theme. Reduces implementation risk.
4. **Future-proof.** As new GPU/accelerator hardware ships (CXL, advanced NPU), Wayland protocols will evolve faster than a custom compositor.

### Why Fork Alacritty, Not Build From Scratch?

**Rejected alternatives:**
- **Build from scratch:** Rust + vte (parser) + wgpu (rendering) ≈ 3 months (credible). But Alacritty is 95% of what we need.
- **Use another terminal:** kitty (C), foot (C), GNOME Terminal (C/GTK).

**Alacritty fork chosen because:**
1. **Already 100% Rust + wgpu.** No translation needed.
2. **Mature and battle-tested.** Alacritty is used in production across thousands of setups.
3. **Only customization needed is theming.** Apply the neon color palette, add custom keybindings for neo-shell integration. ~2 months.
4. **Path to future differentiation.** Once Alacritty ships, we can add AltimitNeo-specific features (inference status inline, tensor watch windows, etc.).

### Why FFI to Vendor GPU APIs for Metrics?

**Rejected alternatives:**
- **Sysfs only (Linux /sys/class/drm, etc.):** Cross-platform nightmare. Fragile. Low-frequency updates.
- **Vendor libraries directly in kernel:** Couples kernel ABI to GPU driver versions. Article 2 (AI-aware) does not mean "couple AI to NVIDIA's release cycle."

**Vendor FFI + sysinfo chosen because:**
1. **Real-time accuracy.** NVIDIA NVML, AMD ROCM, Intel GPU tools give true per-device utilization, memory, temperature.
2. **Thin boundary.** Vendor APIs are C; we wrap with Rust FFI and expose safe Rust interfaces. Standard practice.
3. **Fallback strategy.** If vendor API is unavailable, sysfs/debugfs gives approximate data.
4. **Responsibility separation.** The kernel does not couple to NVIDIA. The userspace neo-monitor does, which is correct — a GPU dashboard depends on GPU vendors.

---

## Trade-offs & Risk Mitigation

### Risk: egui Rendering Performance

**Concern:** Immediate-mode rendering every frame costs CPU. At 60 FPS on a 4K monitor, could be sluggish.

**Mitigation:**
1. Start on 1080p (Phase 8.1–8.3). Benchmark on reference HW (Thinkpad X1 / RTX 4090).
2. If egui CPU usage > 10%, profile and optimize:
   - Cull off-screen panels
   - Use retained-mode layer for static graphs
   - Batch rendering commands
3. Fallback: Switch to wgpu + hand-written layout in 8.4 if needed. (Low likelihood; egui is known-good for dashboards.)

### Risk: Smithay Compositor Complexity

**Concern:** Building a compositor is hard. Smithay abstracts it, but bugs are possible.

**Mitigation:**
1. Start minimal: single-monitor, no advanced features (fractional scaling, vaapi compositing).
2. Use Smithay's reference compositor as a template; don't innovate on windowing until 8.5+.
3. Defer advanced features (multi-monitor, HiDPI) to Phase 8.5+.

### Risk: GPU Vendor API Incompatibility

**Concern:** NVIDIA NVML might be unavailable on AMD or Intel systems.

**Mitigation:**
1. Conditional compilation: AMD systems get AMD ROCM, Intel get Intel GPU tools, NVIDIA get NVML.
2. Always fallback to sysfs + sysinfo if vendor API absent.
3. Test matrix: CI runs on NVIDIA (GitHub Actions), AMD (internal hardware), Intel Arc (future).

### Risk: 13-Month Timeline Slips

**Concern:** UI/UX work is inherently unbounded (scope creep, design iteration).

**Mitigation:**
1. Lock design spec (colors, typography, spacing) at start of 8.1. No changes without an ADR + PL approval.
2. Use "minimum viable" criterion: ship Phase 8.1 with neo-shell + basic window manager, even if design polish is incomplete.
3. If a sub-phase slips, defer to 8.5+ (not a blocker for 8.1–8.4 delivery).

---

## Anti-Decisions

The following are explicitly NOT decided by this ADR:

- **Wayland vs. custom compositor.** Smithay is the proposal; if a credible custom implementation materializes (e.g., fork of Orbital), revisit at 8.5+.
- **Theming system.** Do we ship only neon, or also light variants? Decision deferred to 8.5+. MV is neon only.
- **Application ecosystem.** Phase 8 delivers the base desktop. Third-party apps, package management, app store — Phase 8+ concern.
- **Accessibility.** Cyberpunk aesthetic can be harsh on low-vision users. High-contrast variants and keyboard navigation are Phase 8.5+ work.
- **Localization.** Phase 8 is English. i18n is Phase 8.5+.

---

## Success Criteria

Phase 8 is complete when:

1. **Boots to desktop:** System boots into neo-shell + neo-monitor on QEMU and bare metal.
2. **Real-time metrics visible:** neo-monitor displays real CPU, GPU, memory, inference load, dedup savings with <100ms update latency.
3. **Inference from shell:** User can type `neo-infer --model llama-2-70b --prompt "Hello"` in neo-shell and see the inference progress and output.
4. **Visually coherent:** Every panel (shell, monitor, editor, docs, settings) follows the design system. No visual inconsistency.
5. **Cyberpunk aesthetic evident:** First-time user feels "this is a real, powerful, AI-native OS," not "generic Linux reskin."
6. **Performance acceptable:** Desktop is responsive (UI update latency <16ms at 60 FPS on reference HW).
7. **Documented:** Design system spec, component library, and architecture docs are in `docs/design-system.md`.

---

## Related Decisions

- **ADR-022 (Fault recovery):** Phase 8 desktop has no fault-recovery requirements; deferred to Phase 9+.
- **ADR-014 (Kernel observability):** Phase 8 desktop *consumes* the kernel's observability APIs (§11 of ANSPEC-001). No changes to kernel observability needed.
- **ALTIMIT research report (informational):** §A (visual identity principles), §B (design system), §E–F (fault recovery, capability composition) informed this ADR.

---

## Timeline & Checkpoints

| Date | Checkpoint |
|------|------------|
| 2026-06-30 | ADR-024 ratified; design-system.md published; 8.1 development begins |
| 2026-09-30 | 8.1 + 8.2 complete: neo-shell + neo-monitor on QEMU |
| 2026-11-30 | 8.3 complete: terminal emulator shipped |
| 2027-02-28 | 8.4 complete: 3D tensor visualization |
| 2027-04-30 | 8.5 complete: icons, polish, documentation; Phase 8 ship candidate |
| 2027-05-31 | Phase 8 production release (tentative) |

---

## Amendment 2026-07-19 — The frosted-glass material (RATIFIED by PL)

Full text: `docs/THINK-2026-07-19-atrium-glass-frost-amendment.md`. Ratified direction (normative):

1. **Frosted glass IS the Atrium's material substrate.** The cap-surface material (owned / tethered / denied / untrusted / undo-armed) renders as translucent frosted glass (capture backdrop → downsample → blur → vibrancy tint → composite — the classic-Mac `NSVisualEffectView` idiom), *ours* because the surface behind the glass is a cap-gated region, not another app's snoopable pixels.
2. **Trust-state IS the glass tint** — *security is the beautiful part.* Owned = clear/light frost + Electric-Blue vibrancy; tethered = the visible leash; undo-armed = the Undo Shadow; denied = heavy frost + magenta edge; untrusted = heaviest frost + desaturated gray. **Colorblind-safety (Janus cross-check correction — the amendment's original "frost-density is THE hue-independent channel" was insufficient):** frost-density is **one** redundant channel but **not enough** — trust MUST also carry **hue-independent, mode-surviving cues: Cap-Halo geometry, patterns, connection lines (the tether), glyphs, and labels** that read identically in glass, degraded-solid, and any future mode. No trust state may rely on hue (or on frost-density alone). The exact palette is a ratified-direction starting point, tunable.
3. **The Reduce-Transparency invariant (NON-OPTIONAL).** Frost is the material; **legibility is the invariant.** **(Janus refinement:)** the degraded path is NOT a solid *trust-color* alone (color alone fails colorblind + AAA) — it is a **contrast-tested opaque NEUTRAL surface + the persistent semantic cues from §2** (geometry/glyph/label), so state and AAA both survive losing the blur. This one path does three jobs — accessibility floor, low-end/CPU-only performance floor, OT-63-GPU-degraded fallback — but **user-preference and automatic GPU-degradation must remain DISTINCT causes** of the same semantic-equivalent opaque render (a sticky a11y choice ≠ a dynamic perf/GPU state).
4. **Glass is GPU-gated** (real-time backdrop blur needs OT-63; the degraded path is the always-available floor). **Renderer (Janus cross-check, three-family-settled with nuance): the COMPOSITOR scene + glass/backdrop-blur pipeline = own-project, on Vello/wgpu — NOT iced** (backdrop blur is an order-dependent compositor render-graph op, not a widget concern; both families agree). **The application widget toolkit is deliberately NOT pre-committed:** rebuilding layout/focus/keyboard-nav/text-editing/accessibility-semantics is a far larger job than the compositor scene, so preserve a **backend-neutral app-UI seam** and run a bounded spike against **iced's permissive (MIT) core/widget/layout pieces** — adopt them only if they emit our scene/accessibility primitives *without* winit/Wayland ownership or a permanent fork (egui for diagnostics only). Browser leverage is a **shared GPU/rendering SUBSTRATE + primitive vocabulary**, NOT one renderer serving both (their scene models differ). (Corrects the amendment's blanket "not iced" + "one renderer" — iced is not intrinsically Wayland.)
5. **Reconciliation (confirmed, with the Janus selectivity refinement):** breaks nothing in the Atrium grammar — the Undo Shadow stays the only drop shadow (glass depth is blur/translucency, not drop shadows), neon-as-meaning + ≥70% negative space + grammar-not-uniformity + the classic-Mac craft bar + the §12 Threshold all survive — **but ONLY when glass is applied SELECTIVELY: near the Void stays *true* negative space (not frosted), and neon meaning renders SHARP ABOVE the blur (never blurred into mush).** Pervasive glass would eat the calm negative space + muddy the neon; selective glass composes. Explicitly **NOT** the macOS Tahoe/"Liquid-Glass" *enforced-sameness* failure (distinct silhouettes mandated).

**Document end.**

*This ADR is normative per Bible Article 18.1. Status: ACCEPTED (ratified by PL 2026-05-28; frosted-glass material amendment ratified by PL 2026-07-19). Superseding ADRs or Bible amendments that change Phase 8 scope will update this document's status.*
