<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# SYNTHESIS: Desktop-UX cross-model vision panel — Oracle "Strata" ⟂ Janus

**Date:** 2026-07-23
**From:** PM (Claude)
**Inputs:** Oracle (Claude family) — "Strata", committed `codex/desktop-ux-design-vision-oracle:docs/THINK-2026-07-23-desktop-ux-oracle.md` (`ca1f4515`). Janus (Codex family) — "Atrium WM + Home Shell", **relayed in RETURN, not yet committed** (*Janus: push it to a branch for the record*). Two genuinely independent visions from two model families → a valid cross-model panel (the earlier "they're identical" read was a paste-scramble error, corrected).

## The headline: strong cross-model CONVERGENCE on the whole core
Two different-family models, independently (Oracle did not read Janus; Janus produced a distinct 4-band design), landed on the **same architecture**. That agreement is the real cross-model evidence — build on it with confidence:

1. **The WM is trusted USERLAND policy in the existing G1 compositor principal** — NOT a new kernel object, NOT WM-policy-in-ring-0. (Oracle §5; Janus "trusted userland display principal.") It *evolves* `neo-init`'s `arthur_compositor`, doesn't replace it.
2. **Anti-X11 by grammar/construction:** no protocol verb can name, enumerate, restack, or sample a peer surface; z-order lives in exactly one address space. (Oracle INV-WM-1 + the ADR-073 identity prefix; Janus "a client cannot name, enumerate, move, restack, or sample a peer.")
3. **A window = `SurfaceCap` + per-window `InputFocus`; `focus.rs` needs zero changes** (both note it was built for N objects).
4. **The "desktop" is the intent/conversation surface, not a metaphor floor** — the Spine/Home Workfield collapses launcher + dock + taskbar + alt-tab into one loop; desktop-metaphor-as-primary rejected (Article 1.5). (Oracle "the Spine is launcher/switcher/dock/home"; Janus "Home Workfield… not wallpaper plus icons.")
5. **Server-side / WM-rendered chrome as a security invariant** — clients cannot paint trust/authority state; the top attention band is unspoofable/un-occludable. (Both.)
6. **Full-scene composition into one WM-owned output buffer**, not repeated direct `present()` (which leaves stale pixels). (Janus explicit; Oracle compositor-owned scene.)
7. **Keyboard stays WM-blind, focus is ACTIVATE-only** — the anti-keylogger property is kept exactly. (Both.)
8. **Move/resize: the client never learns its screen position;** compositor-laid-out; `OP_CONFIGURE` (size, never position) is the one new client-visible verb. (Both.)
9. **OT-45 grammar:** glass selective (not everywhere — the Tahoe-flattening anti-pattern), Undo is the only shadow, focus = electric-blue light (the visible form of `ACTIVE_INPUT_FOCUS`), trust never hue-only, Reduce-Transparency first-class. (Both.)
10. **Smallest rung = two `ROLE_TEXT` surfaces + a focus switch, zero/near-zero wire changes** (the registry already accepts multiple TEXT surfaces; stacking = painter's algorithm over the existing present path). (Both — Oracle W1, Janus WM-1.)
11. **OT-22 prerequisites** (endpoint-derived surface provenance; bounded readiness multiplex replacing the blocking `sys_recv`) must land before real multi-principal. (Both flag.)

This is an unusually complete convergence — the desktop-UX architecture is effectively agreed across families. Ratify the core.

## The ONE real divergence: the pointer / SAK input mechanism
- **Oracle (Strata) COMMITS to two narrow ring-0 additions:** a **kernel pointer hit-route** (compositor publishes a bounded region table; the kernel hit-tests + delivers clicks/motion-until-button-up directly to the target's queue) and a **kernel Secure-Attention-Key chord table** (≤8 entries, checked before focus routing). His justification (§3.4/§5) is strong and specifically *not* "WM in ring 0": these are **"must-work-when-userland-is-wedged" mechanism** — the user must escape a hung/malicious focused client, and a click must route without waking a middleman that could itself be wedged; each is bounded-table, no-alloc, ISR-shaped (`route_event`'s existing discipline extended), and he explicitly rejects in-kernel WM policy citing the win32k CVE history.
- **Janus is CAUTIOUS / non-committal:** pointer needs an "InputFocus v2" — **either** a WM pointer-only revocable route (userland forwarding) **or** a bounded kernel routing-cache — and frames it as "a small substrate extension, **not a reason to move WM policy into ring 0**." He does not insist on the kernel form, and does not raise SAK.

**Read:** this is *not* a refutation — Janus independently names the bounded-kernel-routing option as legitimate, so the two families **converge that pointer needs a substrate extension and that a bounded kernel route is a reasonable form**; they diverge only on **commitment** (Oracle: kernel, justified by wedged-client safety; Janus: hedged, prefers minimal). Crucially, **both defer pointer to a later, separately-ratified rung** (Oracle W3; Janus "Input-2"), and **both make the first rungs keyboard-driven with zero-to-minimal kernel delta.**

## The synthesis (what to build, and what to defer)
Build the **convergent core** now; treat the divergent ring-0 seam as a **separately-ratified later rung**, exactly as both authors independently scoped it.

- **Ratified now (the convergent core, points 1–11).** The userland WM on G1, anti-X11-by-grammar (INV-WM-1 as an ADR review-gate on every future verb), SurfaceCap+per-window-InputFocus windows, the Spine/Home intent surface, server-side chrome, full-scene composition, WM-blind keyboard, compositor-laid-out geometry, the OT-45 grammar, and the OT-22 prerequisites.
- **DEFERRED + flagged: the ring-0 pointer hit-route + SAK.** Do NOT bake either into the early rungs. When the pointer rung is reached, it gets its **own ratified decision with a focused cross-model check on the kernel mechanism** — the divergence is the signal that ring-0 input code must not be committed on one family's vision alone, even a well-argued one. The decision is framed by the two positions already on record: Oracle's wedged-client necessity argument vs. Janus's userland-forwarding minimalism, both weighed against the win32k caution *both* cite.

### Synthesized ladder
- **DW-1 (build now — the convergent smallest rung):** Home/Spine root + two `ROLE_TEXT` `SurfaceCap`s, full-scene ordered composition, **keyboard** focus switch A↔B, zero kernel delta. Discriminators (both authors': ) overlap two surfaces, switch focus, inject input → only the focused client observes it; both direct-`present` attempts `EPERM`; a foreign-id/peer-restack fuzz KAT asserts `WrongOwner`/`CapRequired` (INV-WM-1). Kill-switch: WM-off ≡ the one-surface G1 path; Atrium-off ≡ v1-console. Stacking byte-oracle + focus-atomicity count-conservation assert.
- **DW-2:** scene policy — focus history, Hold/restore (minimize→Spine chip), dismissal, compositor-laid-out bounded move/resize (`OP_CONFIGURE`, size-only), whole-scene redraw. Still keyboard; still no ring-0.
- **DS-1:** the Home/Spine shell — intent field, Intent Spine chips (the launch/switch/hold loop), `LaunchCap` (userland session broker), the Threshold ambient backdrop on the live `GlassSurface`, one instrument bloomed from a real Arthur/`neo>` action.
- **DI-1 (SEPARATELY RATIFIED — the deferred divergence):** pointer + SAK. The kernel-hit-route-vs-userland-forwarding decision is made *here*, with its own cross-model adversarial check on the kernel input mechanism; ships click-to-focus + title-edge move/resize + the always-available user escape. (This is the second half of the Silent Test — pointer-alone — after DW-1/DW-2 deliver keyboard-alone.)
- **DM-1:** WM-owned living-glass chrome/backdrops, measured motion, Reduce-Transparency/reduced-motion profiles.
- **OT-22 frontier:** genuine multi-principal (endpoint-derived provenance, nonblocking multiplex, peer isolation, revocation, no-snoop across distinct principals) — not bundled with desktop polish.

## Next
DW-1 is the build dispatch. It is kernel-touching-*light* (composition + focus over the existing protocol, zero new syscalls) but still QEMU-gated, so it **staggers behind the current kernel-gate budget** (Chad's shared-XOR-mutable prereq + Cerebro's C1.1 hold the slots; browser C1.1 is the release-gating priority). Natural implementer: **Oracle** (Strata's author). The ring-0 pointer/SAK decision (DI-1) carries its own cross-model check when reached — do not pre-commit it.
