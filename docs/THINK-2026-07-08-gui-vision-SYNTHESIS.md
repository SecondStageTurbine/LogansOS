<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The GUI vision — SYNTHESIS of the two independent visions (PM)

**Date:** 2026-07-08
**Author:** PM (Claude), synthesizing the two independent judge-panel visions
**Inputs:** `docs/THINK-2026-07-08-gui-vision-cerebro.md` ("The Atrium") + `docs/THINK-2026-07-08-gui-vision-jarvis.md` ("The Intent Field"), both merged to `main`; the dispatch `Plans/DISPATCH-2026-07-08-gui-vision-think-cerebro-and-jarvis.md`; OT-52/OT-45/OT-26/OT-46/OT-44/OT-22/OT-18/OT-19; Bible 1.5/1.6; `design-system.md` (ADR-024, §12 The Threshold); the v1 console (shipped).
**Status:** SYNTHESIS for PL ratification. The two visions were produced independently (neither read the other). **The headline finding: they converge on the entire hard core and diverge on exactly one thing — the organizing unit — and that divergence is a *composition*, not a contest.** This doc ratifies the convergence, resolves the divergence, and lists the genuine forks the PL owns.

---

## 0. The synthesized vision in one breath

**A continuous, capability-secured, Arthur-co-inhabited surface — a *room you live in* (Cerebro's Atrium) — organized into intent-rooted *Workfields* (Jarvis's unit), where every object is a capability you can see and hold, every mutation leaves a visible undo you can scrub back to, Arthur is a felt presence rather than a face, and the whole thing is instant because we own the stack. Intent becomes visible, reversible work.**

The single first-principles sentence both landed on, merged: **LogansOS is the first screen where the system's real substrate — Arthur, capabilities, reversibility, local inference, owned hardware — *is* the interface.** That is the thing only this OS can build. macOS/Windows make better windows; we make a different kind of interface.

---

## 1. THE CONVERGENCE — the ratified spine (both agreed, independently → highest confidence)

Two experts who could not see each other's work reached the same conclusions on all of this. Treat it as settled direction, not open design:

1. **Not a desktop.** No windows/icons/menus/taskbar as the primary metaphor (the 1973 filing-cabinet paradigm; unbeatable at its own game). Intent-first, not app-first.
2. **Intent → visible, reversible work.** You express what you want (speak/type/point); the right surface appears where your attention is; you never hunt an app or a file dialog. (Cerebro: "Summon, not navigate." Jarvis: "ask for the thing.")
3. **Capabilities are VISIBLE — security becomes the beautiful part.** Every other OS's ugliest surface (consent dialogs, UAC, permission theater) becomes ours' most beautiful: the honest, directly-manipulable structure of what you can do. (Cerebro: *materials carry capability* — owned/granted-with-tether/untrusted have distinct materials; Jarvis: **Cap Halos** — scoped authority rings.) **This is the single most only-we-can-do idea, and both independently named it.**
4. **Reversibility is a VISIBLE OBJECT.** OT-19 stops being an internal safety feature and becomes something you point at. (Cerebro: time-as-axis, *scrub* the timeline; Jarvis: **Undo Shadows** behind mutable objects.) "Safe to be wrong," made felt.
5. **Arthur is a PRESENCE, not an avatar.** Felt through how the substrate behaves (light gathers, the room's breath quickens when he works — the v1 liveness indicator generalized), heard in the OT-46 butler register — never a cartoon face. Always dismissable (`neo>` is the Arthur-free path).
6. **Liveness is HONEST.** Tied to a real forward pass / real device work, never a fake spinner. (Jarvis sharpens it: liveness as a *measured channel* — tokens / action-candidate / cap-check / wait-on-device / undo-write / present-complete.)
7. **The transcript/spine is the durable backbone; `neo>` is always-present as the exact deterministic layer.** Both preserve the shipped v1 transcript-as-UI + the S1-S8/T1-T4 duality verbatim: `neo>` stays predictable/composable/never-adaptive, gains a body, renders structured output (the object-pipeline decision) without changing its contract.
8. **Compositor: OUR OWN cap-native protocol (OT-52 ratified by both).** Adopt Wayland's *primitives* (compositor-owns-final-composition, client-allocated buffers, explicit damage, presentation feedback, per-surface input focus, no-global-snooping); **reject** its protocol-as-ABI, its Mesa/DRM/GBM/EGL stack, and its desktop toplevel/window model. X11 is dead on arrival (global input/keystroke snooping = incompatible with capability security). A bounded, kill-switched, *untrusted* Wayland-compat personality is a *later* PL-gated concession, never the foundation (OT-44 ReactOS-treadmill discipline).
9. **Latency IS beauty — "instant or it's not beautiful."** Sub-frame input-to-photon (target <8.33 ms at 120 Hz, toward the single-digit-ms perceptual floor). Compositor + input are **real-time priority workloads the kernel cooperates with** — and both noted the unfair advantage: we already run inference-deadline/QoS governance (Phase-7/OT-15), so the *same* deadline machinery guarantees the compositor's frame deadline (on Linux the compositor is just another process). Predictive input to mask residual latency; software-composite first, GPU on **our** accel path (Phase-6, never Mesa/DRM) second.
10. **Beauty = grammar, not uniformity.** Shared physics (light/material/motion/spacing/type) that *mandates* per-thing distinct silhouettes — the Tahoe/Liquid-Glass cautionary tale (enforced sameness = anti-beauty AND anti-usability). Core-Animation retained-mode + interruptible motion; Rams restraint; honest light. Distinct silhouettes = the five-year-old visual test.
11. **Buildable spine: software-first, grows from the v1 console, kill-switchable per slice, gated on OT-22 multi-principal.** (§5 merges the two nearly-identical build ladders.)

---

## 2. THE DIVERGENCE — and why it's a composition, not a contest

The one real difference: **the organizing unit.**

- **Cerebro's Atrium** — the atom is the **cap-surface** (a capability = a screen region), living in one continuous *spatial room you inhabit*; the paradigm is four verbs (Summon / Hold / Scrub / Instant); time is a first-class scrub axis; and the killer enabler is **GPU vector text** (Slug/Vello-class, resolution-independent) that turns the transcript from a bitmap console into an infinitely-zoomable *surface* — "the transcript grows a body."
- **Jarvis's Intent Field** — the atom is the **Workfield**: a live *task-bundle* rooted in one intent, containing that intent's transcript turn + its exact `neo>` command-graph equivalent + the surfaces it summoned + the caps it consumed + the liveness + the undo trail. Four layers (Intent Spine / Workfields / Instruments / Deterministic Rail); a concrete cap taxonomy (SurfaceCap/BufferCap/InputFocusCap/PresentFenceCap/CaptureCap/DecorationCap) and named primitives (Cap Halo, Undo Shadow, Liveness Thread, Command Graph).

**They compose cleanly, and each supplies exactly what the other lacks:**
- Jarvis's **Workfield is the missing mid-level container** for Cerebro's atomic cap-surfaces — the unit that binds "an intent and everything it touched" into one inspectable, reversible, re-runnable thing.
- Cerebro's **continuous room (Atrium) is the space** Jarvis's Workfields live in — and Cerebro's *time-scrub axis* + *emotional "where you live" framing* + *§12-Threshold aesthetic reconciliation* give the Intent Field a soul and a resting state.
- Cerebro's **GPU-vector-text** is the rendering enabler *both* need (without it neither the Atrium nor the Workfield escapes "terminal"); Jarvis's **cap taxonomy + named primitives** are the concrete vocabulary Cerebro's abstractions need to become buildable.

**Synthesized paradigm (recommended):** *the room is the Atrium; the unit within it is the Workfield; the atom of a Workfield is the cap-surface.* Intent blooms a **Workfield** in the continuous space; the Workfield holds its cap-surfaces (each a visible capability with a Cap Halo / material), its Command Graph (the exact `neo>` equivalent), its Liveness Thread (honest, measured), and its Undo Shadow (scrub-back reversibility). Arthur is the room's felt co-inhabitant who summons and arranges Workfields under your grants. **Naming is a PL call** (Fork 5) — "Atrium" carries the where-you-live warmth; "Intent Field" carries the what-it-does clarity; the *unit* is the Workfield either way.

---

## 3. BEST-OF-BOTH — what we keep from each (the graft)

| From **Cerebro's Atrium** | From **Jarvis's Intent Field** |
|---|---|
| The **four-verb paradigm** (Summon / Hold / Scrub / Instant) as the crisp mental model | The **Workfield** as the primary UI unit (intent-bundle: turn + command-graph + surfaces + caps + liveness + undo) |
| **Materials carry capability** (owned/tethered/untrusted have distinct *materials*, revoke = cut the tether) | The **named primitive vocabulary**: Cap Halo, Undo Shadow, Liveness Thread, Command Graph |
| **Time as a scrub axis** — the past is up-and-to-the-left, undo is a place | The **compositor cap taxonomy** (SurfaceCap/BufferCap/InputFocusCap/PresentFenceCap/CaptureCap/DecorationCap) — the concrete surface model |
| **GPU vector text** (Slug — patent-freed — / Vello) as *the* enabler that makes the transcript a zoomable surface | **Liveness as a measured channel** (tokens/action/cap-check/device-wait/undo/present) — honest by construction |
| The **§12 Threshold reconciliation** (existing cyberpunk design-system = the resting "intelligence as weather" state; craft-bar = classic-Mac) — resolves the aesthetic fork | The **"bets I would NOT take early"** discipline + the honest **text-shaping-is-hard** flag (SDF ≠ shaping/hinting/complex-scripts) |
| The **"where you live" emotional framing** (the Atrium = a warm lived-in room) | **Reflex-style UI scheduling** framing + the concrete **acceptance-bar** (8 must-pass criteria) |

Both cite the same craft canon (Core Animation retained-mode + interruptible motion, Rams restraint, the Tahoe cautionary tale, damage-tracking, frame-pacing/present-feedback, declarative/reactive app content) — that's the shared best-practice floor.

---

## 4. THE COMPOSITOR DECISION — SETTLED (both agree; OT-52 refined)

No fork here — both independently reached the identical conclusion, so I'm recording it as the ratified OT-52 refinement:
- **Build our own minimal cap-native compositor.** Surface model = Jarvis's cap taxonomy (SurfaceCap/BufferCap/InputFocusCap/PresentFenceCap; CaptureCap absent-by-default; DecorationCap = compositor-owned chrome). Compositor owns final composition; clients allocate buffers (shared *by capability*, not by dma-buf FD handshake); input focus is a capability check (input-security == input-routing, one code path); retained scene graph + damage tracking.
- **Trusted userland compositor, kernel-enforced caps** (Article 6.6: the compositor decides layout/beauty; the kernel enforces cannot-exceed). Not a policy-brain in ring 0.
- **I-14 containment is a required section of the compositor build** (Cerebro flagged it): (a) the compositor/input/vsync path holds no AI-governance lock; (b) a runaway client degrades to software/Tier-0 composition, never a governance stall; (c) client buffers cap-gated + any device DMA through the fenced arena under VT-d/IOMMU. Kernel-touching ⇒ mandatory QEMU gate.

---

## 5. THE CONSOLIDATED BUILDABLE SPINE (both ladders merged; each slice additive, kill-switchable ≡ prior layer, grows from v1, gated on OT-22 for multi-principal)

- **G0 — the paradigm prototype (single-principal, no compositor protocol yet).** v1 transcript → the Intent Spine; **one** Workfield type ("find/open a file" or "clean a folder") with Arthur turn + the exact `neo>` command-graph + candidate cap-surfaces + cap-denial/grant display + one Undo Shadow + an honest Liveness Thread, on the raw framebuffer. **Answers the only question that matters first: does this feel obviously better than a terminal and un-fake vs a desktop?** (QEMU flow + framebuffer-nonblank + F1-F8 + S/T duality + the OT-41 bare-metal legibility proof.)
- **G1 — the cap-native compositor MVP (software).** One composited cap-surface, the cap taxonomy, damage tracking, present-pacing. Kill-switch: compositor-off ≡ today's raw console. *(Kernel-touching — QEMU gate + I-14.)* = OT-52 build-1.
- **G2 — GPU vector text.** Slug/Vello-class GPU-outline text → resolution-independent zoomable type — the leap from terminal to surface. (Largely userland; the hard-text-shaping caveat applies — real shaping stack before localization.)
- **G3 — two surfaces + cap-routed input + the Workfield protocol** (Cap Halo, Undo Shadow, Liveness Thread, Command Graph as native compositor objects). *(Kernel-touching — QEMU gate + I-14.)*
- **G4 — intent summons a Workfield.** Speak/type → Arthur materializes a Workfield (rides the shipped OT-26/A1-A2 loop + a visual surface). **The smallest slice that proves it's a paradigm, not a desktop.**
- **G5 — GPU composition on our accel path** (frame-pacing/present-feedback, 120 Hz-class, software fallback).
- **G6 — the full design language pass** (materials/motion/type, capability-visible silhouettes, the §12 Threshold generalized, accessibility profiles: reduced-motion/transparency, high-contrast, keyboard-nav). Article 1.6 gate + hardware legibility proof.
- **G7+ — the frontier:** multi-principal (OT-22, one pillar at a time), OT-44 H2 cap-bundle visual install/first-run-grant, voice input (OT-46B), the optional bounded Wayland-compat island (if ratified).

---

## 6. FORKS FOR PL RATIFICATION (where the two agreed to recommend, and the genuine choices)

1. **Aesthetic identity.** Both recommend *reconcile*: classic-Mac **craft bar** + neon-as-light-and-meaning + the §12 Threshold "intelligence as weather" as the resting state (this softens ADR-024's cyberpunk-maximalism commitment). **This is the character of the thing you'll live in — your call.** Recommendation: adopt the reconciliation.
2. **Own-protocol-only vs a later Wayland-compat island.** Both: own cap-native as the native path; a bounded/kill-switched/untrusted Wayland *personality* only later, only under real app-catalogue pressure, only if you want Linux-app compat. Recommendation: own-first, defer the island decision.
3. **Intent primary, or additive to pointer/keys?** Both: **additive** — intent is a first-class *peer* of pointer/touch/keys, never a replacement (protects the five-year-old *and* the power user's `neo>` muscle memory). Recommendation: additive.
4. **Workfield-only vs freeform windows.** Jarvis's fork: Workfield-first; if windows ever exist they're an *instrument mode inside* a Workfield, not the system metaphor. Recommendation: Workfield-first.
5. **The name** — "Atrium," "Intent Field," or something new. (The *unit* is the Workfield regardless.) **Your call — it's where you live.**
6. **Voice (OT-46B) sequencing** — the literal five-year-old test (a child speaks before typing) needs STT + audio-DMA fencing; both gate it honestly (prove the text/pointer surface first, design so voice slots in). Recommendation: design-for-it-now, build-it-Phase-N.

**Hard blockers (not forks — real gates):** OT-22 multi-principal (a compositor is definitionally multi-principal), audio drivers for voice, OT-44 H2 for third-party apps, and OT-41's bare-metal legibility discipline promoted to a standing GUI gate (QEMU proves flow; hardware proves it's *actually* beautiful on your screen).

---

## 7. WHAT I RECOMMEND YOU SIGN OFF ON

1. **The synthesized paradigm** (§0/§2): a capability-secured, Arthur-co-inhabited *room* of intent-rooted **Workfields**, where caps are visible, reversibility is scrubbable, Arthur is felt, and it's instant because we own the stack.
2. **The compositor decision** (§4): own cap-native (Jarvis's cap taxonomy), Wayland-primitives-not-stack, X11-dead. This can update the OT-52 row from "direction" to "ratified architecture."
3. **The buildable spine** (§5), starting at **G0** — the single-Workfield paradigm prototype on the existing framebuffer — as the first GUI slice, whenever the GUI phase opens after the headline goal (boot-the-OptiPlex-talk-to-Arthur) lands.
4. **The six forks** (§6) — I've marked each recommendation; the aesthetic identity (#1) and the name (#5) are the two that are genuinely *yours* to feel out.

This stays future-phase (v1 is the text console; the compositor is gated on OT-22). But the vision is now ratifiable, the compositor architecture is settled, and the smallest-first slice is defined — so when the GUI phase opens, whoever builds it inherits a direction, not a blank page. And the thing it's aiming at is the one you asked for: not a prettier Mac, but the first interface where the OS's real substrate is the beauty — a place worth living in.

*Synthesis THINK. No code. The two source visions are preserved verbatim on `main` (`gui-vision-cerebro.md`, `gui-vision-jarvis.md`).*
