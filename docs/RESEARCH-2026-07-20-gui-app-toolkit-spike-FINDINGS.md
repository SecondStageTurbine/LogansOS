<!-- SPDX-License-Identifier: CC-BY-4.0 -->
> **PM SUPERSEDING NOTE (2026-07-21):** Per the standing no-human-labor-time rule (CLAUDE.md), the **person-month figures throughout this doc are deprecated** and must not be carried forward as effort estimates — read them only as relative difficulty/dependency signal. The finding stands, re-expressed: the **hybrid** direction = *clean-room with iced as an executable reference* (own the `no_std` runtime/API/a11y seam; do NOT fork iced crates). Its cost vs. pure clean-room **converges** (a good reference is a modest discount on layout/behavior design, not a structural saving), and any effort figure is **gated on the decisive second spike** (real glyph → own-Vello → `SharedPixelBuffer` + a derived damage rect), not committed until that spike measures it.

# RESEARCH: GUI application-toolkit spike — iced over the Atrium seam

**Date:** 2026-07-20

**Author:** Chad (Codex)

**Status:** BOUNDED SPIKE FINDINGS — host/offscreen only; no QEMU gate
**Branch:** `codex/gui-app-toolkit-spike-chad`

## 0. Recommendation

**Choose a hybrid: own the cap-native `no_std` application runtime, renderer
adapter, accessibility contract, and long-lived widget API; adapt only narrowly
selected iced algorithms/behavior under that API. Do not adopt `iced_core` or
`iced_widget` as shipped.** **[High confidence]**

The PoC establishes a useful positive result: iced's real widget/layout tree and
`UserInterface` can consume a non-winit event source and render through a
project-owned backend without `iced_wgpu`, `wgpu`, or winit. The future backend can
translate the same quad/text calls into own-Vello scene primitives and write the
result into the client's `SharedPixelBuffer` before G1 damage/commit.

The adoption result is nevertheless **no-go**. The reusable crates are hosted
Rust, not `no_std`: they use `std` throughout, and the real widget crate pulls
`iced_renderer`/`iced_graphics` plus the hosted cosmic-text/font stack even when no
wgpu feature is enabled. Porting that moving graph would be a permanent fork.
Additionally, the dispatched premise that iced provides AccessKit integration is
false for inspected iced 0.13.1 and 0.14.0: neither source tree mentions
`accesskit`. Accessibility must be a project-owned semantic seam with AccessKit
considered separately for hosted bridges/reference.

This recommendation does **not** revisit the settled compositor/glass decision.
The compositor remains own-Vello. This document covers application widgets only.

## 1. Layering and renderer neutrality

### 1.1 What is genuinely separable

iced 0.14 has real architectural seams:

- `iced_core::Renderer` asks for layers, transforms, and filled quads.
- `iced_core::text::Renderer` adds associated paragraph/editor types and text draw
  calls. It has no wgpu or winit types in its trait contract.
- `iced_core::Widget` owns layout, state-tree reconciliation, event update, draw,
  and widget operations through generic `Theme` and `Renderer` parameters.
- `iced_runtime::UserInterface` explicitly documents implementing an own event
  loop. It builds layout, updates from slices of `iced_core::Event`, collects
  messages, and draws through the generic renderer.
- `iced_winit` is a separate shell and conversion crate. It is replaceable; the
  PoC does not depend on it.
- `iced_wgpu` is an optional rendering implementation. It is replaceable; the PoC
  does not depend on it.

Therefore an own-Vello application backend is structurally credible: implement
quad/layer/transform emission as Vello scene construction, implement text
paragraph/editor measurement on the chosen Altimit text stack, render offscreen,
then expose the RGBA/GPU buffer through G1. **[High confidence on the seam; medium
confidence on production Vello effort because this spike intentionally used the
allowed software stand-in.]**

### 1.2 What is not a small `iced_core` subset

The useful unlock UI needs more than geometry types. A real `TextInput` requires
widget state, Unicode grapheme editing, focus/cursor behavior, time, clipboard,
theme/catalog machinery, paragraph hit testing, and renderer-specific paragraph
and editor associated types. `iced_widget` 0.14 depends unconditionally on
`iced_renderer`; that depends on `iced_graphics`. The resolved host graph also
includes cosmic-text and its hosted font discovery/mapping dependencies.

This is not welded to wgpu/winit, but it is welded to a sizable hosted library
graph and iced's internal traits. Substituting Vello is feasible. Treating the
result as a small, stable, cap-native dependency is not. **[High confidence]**

## 2. `std` / `no_std` finding

Neither `iced_core`, `iced_runtime`, nor `iced_widget` 0.14 declares `#![no_std]`.
Source inspection found direct `std` use in core geometry, element/widget trees,
IDs/atomics, images/paths, theme, window events, text editor, runtime tasks/debug,
and widget implementations. The dependency graph adds other hosted assumptions,
notably font database discovery and memory mapping.

Some calls are mechanically replaceable with `core`/`alloc`; others are policy
work:

- filesystem/path-backed images and fonts;
- wall-clock/animation timing;
- thread/future/executor assumptions;
- clipboard and input-method services;
- dynamic widget state (`Any`, `Arc`, IDs/atomics);
- full text shaping, font discovery, and editor hit testing.

A one-time patch is insufficient because iced evolves its renderer/widget traits
between releases. A no-std port that tracks upstream is effectively a maintained
fork. **Estimate: 8–14 person-months to reach an unlock-quality maintained fork,
then 1–2 person-months per significant upstream update, with high schedule and
semantic-drift risk. [Medium confidence; implementation breadth is known, velocity
depends on staffing and the final text stack.]**

## 3. Event-loop and G1 impedance

The runtime itself is not winit-owned. The PoC maps a reduced G1 event:

```text
InputEventV1(TEXT_BYTE, byte, focus_generation)
  -> reject unless generation == active InputFocus generation
  -> iced_core::Event::Keyboard(KeyPressed { text, ... })
  -> TextInput state/update
  -> application Message::Password(new_value)
  -> redraw through project renderer
  -> client SharedPixelBuffer + bounded damage commit
```

RAG discovery and direct reads confirm the live G1 boundary: the client owns the
`InputFocus` RECEIVE root and RW mapping of its `SharedPixelBuffer`; the compositor
holds only ACTIVATE and RO/NX PRESENT authority. Current `InputEventV1` exposes
decoded `TEXT_BYTE`, focus gained/lost, modifiers, ticks, and focus generation.
Thus widget processing correctly belongs client-side.

Two integration gaps remain:

1. G1 currently has decoded byte/focus semantics, while iced models richer
   keyboard identity, press/release, modifiers, pointer, touch, and IME events.
   Unlock text works; full keyboard navigation, shortcuts, pointer-only operation,
   composition, and international text require the planned versioned G1 event
   expansion.
2. There are two focus layers: kernel-routed surface focus and intra-app widget
   focus. The app runtime must explicitly seed/traverse widget focus (the PoC uses
   a synthetic pointer press) and clear it on `FOCUS_LOST`.

Neither gap requires winit. Both require an Altimit-owned adapter and policy.
**[High confidence]**

## 4. Accessibility / AccessKit

The examined iced tags (`0.13.1` commit `9bfbd7c` and `0.14.0` commit `3997291`)
contain no AccessKit dependency or source reference. iced therefore does not
currently provide the reusable accessibility-tree machinery assumed by the
dispatch. Its widget behavior is still useful prior art, but adopting iced does
not buy an accessibility implementation. **[High confidence]**

AccessKit itself is a useful permissive semantic vocabulary and platform-adapter
project, but its standard platform adapters target hosted accessibility APIs.
Altimit needs an own, renderer-independent semantic tree/protocol first: role,
name, value, state, bounds, actions, focus, relationships, and incremental updates.
That tree must survive glass/opaque rendering and be authorized across cap
boundaries. A future hosted compatibility bridge could emit AccessKit nodes, and
AccessKit algorithms/types may be adopted where their dependency audit permits;
it should not define the kernel/G1 ABI. **[Medium-high confidence]**

## 5. PoC result

The isolated crate is `tools/gui-app-toolkit-spike` and contains its own
`[workspace]`, so it is not a root-workspace member and does not perturb kernel or
userland gates.

The test uses iced 0.14's real `TextInput`, `Button`, column layout, state tree, and
`UserInterface`. `PixelRenderer` is project code implementing iced's generic
renderer and text-renderer traits; it rasterizes quads and fixed-cell glyph blocks
directly to an RGBA buffer. It is intentionally a trivial software stand-in for a
Vello scene adapter, as allowed by the timebox. No winit, `iced_wgpu`, or `wgpu`
dependency is present.

The test:

1. lays out the password field and Unlock button at 240×100;
2. establishes G1 focus generation 7;
3. translates a generation-7 decoded `a` byte into iced's renderer-neutral event;
4. observes `Message::Password("a")` from the real text field;
5. draws both widgets offscreen and asserts that the RGBA buffer contains rendered
   pixels.

Result: **PASS** (`1 passed`). This proves shell and backend replacement, not
production typography, Vello GPU readback, accessibility, full G1 input, or
`no_std` portability. **[High confidence]**

## 6. License exact-bytes classification

Licenses were fetched with `curl` from raw repository files on 2026-07-20 (not a
rendered web page). SHA-256 is recorded so the reviewed bytes are reproducible.

| Project / raw file | SHA-256 | Classification |
|---|---|---|
| iced 0.14.0 `LICENSE` | `FC9086BA4EBA4B77E4C603906362742BC3715FFA2C7810C6074FE7780D3A0398` | MIT; **adoptable** |
| AccessKit `LICENSE-MIT` | `23F18E03DC49DF91622FE2A76176497404E46CED8A715D9D2B67A7446571CCA3` | MIT half of MIT/Apache-2.0; **adoptable** |
| AccessKit `LICENSE-APACHE` | `62C7A1E35F56406896D7AA7CA52D0CC0D272AC022B5D2796E7D6905DB8A3636A` | Apache-2.0 half; **adoptable** |
| Vello `LICENSE-MIT` | `ADF157330F722FE7CFD5964A2A6974EFF851B718228C6735724A11706E6F4DEC` | MIT half; **adoptable** |
| Vello `LICENSE-APACHE` | `A6CBA85BC92E0CFF7A450B1D873C0EAA2E9FC96BF472DF0247A26BEC77BF3FF9` | Apache-2.0 half; **adoptable** |
| wgpu `LICENSE.MIT` | `DC0D97139E8205818C703741C7BE7CB3B96888BD5917B8D6FC6133731E403C21` | MIT half; **adoptable** |
| wgpu `LICENSE.APACHE` | `A6CBA85BC92E0CFF7A450B1D873C0EAA2E9FC96BF472DF0247A26BEC77BF3FF9` | Apache-2.0 half; **adoptable** |

These classifications cover the named upstream projects, not every transitive
crate. Any production adoption still needs a locked dependency/license audit.

## 7. Options: effort and risk

| Option | Estimated effort to production app floor | Risk | Finding |
|---|---:|---|---|
| Adopt iced core/widgets over our seam | 8–14 person-months plus ongoing fork tax | **High**: `std` port, large graph, upstream drift, missing a11y | **No-go as shipped** |
| Hybrid: own API/runtime/a11y; adapt selected layout/editing behavior and use own-Vello backend | 12–20 person-months | **Medium**: must specify semantics and text/input early, but dependencies and ABI stay owned | **Recommended** |
| Own toolkit without selectively adapting permissive prior art | 24–40 person-months | **High**: focus, editing, layout, testing, and a11y correctness rebuilt together | Viable fallback, avoid unless adaptation proves more coupled |

The hybrid estimate is lower than clean-room own-toolkit by roughly **12–20
person-months**, principally from using iced as executable prior art and adapting
bounded algorithms/tests for layout, focus behavior, and text editing under
Altimit-owned interfaces. It is higher than a hosted iced integration because it
properly prices the `no_std`, G1, own-Vello, and accessibility boundaries.

Before committing the app-UI build, run the planned different-family review and a
second bounded spike: define the Altimit semantic-node ABI plus a `no_std + alloc`
flex/row/column layout kernel, then adapt one text-editing state machine without
pulling iced's crate graph. That is the decisive proof for the recommended hybrid.
