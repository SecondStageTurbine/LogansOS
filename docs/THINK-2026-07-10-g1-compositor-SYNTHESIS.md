<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: G1 compositor — SYNTHESIS of the two independent designs (Prometheus × Chad → the ratifiable G1 design)

**Date:** 2026-07-10
**Author:** PM (Claude)
**Inputs:** `docs/THINK-2026-07-10-g1-compositor-prometheus.md` (panel A, Qwen, `79ed68b`) · `docs/THINK-2026-07-10-g1-compositor-chad.md` (panel B, Codex, `2e61846`) — produced under judge-panel independence (neither read the other), from the shared ratified base (the Atrium synthesis §4/§5, OT-52's cap taxonomy, Chad's G0 spec, the design language, the text-engine THINK, and the real kernel code).
**Status:** the PM synthesis. **The convergent core is three-family cross-model evidence** (Qwen designed, Codex designed, Claude judged — per the standing cross-model rule) and is treated as settled; the two divergences are resolved below with reasons, as PM recommendations the PL can override at the G1 build-dispatch ratification.

---

## 0. One breath
Two independent designs, two model families, one architecture ratified in advance — and they converged on **every structural choice**: IPC channels carry the protocol (no new control syscalls); a SurfaceCap **is** a channel, not a new kernel object; **pixels need exactly one new kernel primitive** (a shared-page buffer object) while everything else — scene graph, registry, focus *policy*, damage, pacing — is userland; zero-copy is both sides mapping the same physical pages with the compositor **read-only**; present/liveness events are measured, never theater; and compositor-off ≡ the v1 console bit-for-bit. They diverged on exactly two things — **who routes input** and **how deep the buffer object's lifetime mechanics go** — and each divergence has a clear winner-plus-graft.

## 1. The convergent core (settled by three-family agreement)
| # | Both designs independently chose | Why it's right |
|---|---|---|
| C1 | **IPC channels as the protocol carrier**; no new syscalls for control verbs | Kernel stays mechanism-only (Art 6.6); surface/scene/focus policy is userland |
| C2 | **SurfaceCap = a dedicated IPC channel** (a convention, not a new kernel kind) | The channel *is* the session; peer-death = surface-death for free |
| C3 | **One new kernel pixel-buffer object** (`Shm` / `SharedPixelBuffer` — same concept) created by one new syscall | Pixels are the one thing IPC can't carry zero-copy; the kernel must own the frames' lifetime |
| C4 | **Zero-copy = dual mapping of the same physical pages; client RW, compositor READ-ONLY** | A client scribble corrupts only its own pixels; the compositor structurally cannot write client buffers |
| C5 | **Scene graph, surface registry, focus policy, damage accumulation, frame pacing = userland compositor state** | Policy out of ring 0; the compositor is killable/dismissable like any tenant |
| C6 | **Honest present pacing**: software vsync from measured composite time; fences fire on real blit completion; FrameDropped is an event, not a hidden stall | Latency-is-beauty + the measured Liveness Thread (no fake frame callbacks) |
| C7 | **No-snoop is structural**, not policy — input reaches exactly one capability holder | The X11 anti-pattern made impossible by construction |
| C8 | **Kill-switch: compositor-off ≡ the v1 console (`4858ac6`) bit-for-bit**; OT-22-ready namespacing; G0 regions migrate to surfaces; T0 text = an ordinary client buffer; I-14 fault modes enumerated and contained (userland compositor faults degrade to "no composition," never governance) | The standing disciplines, both applied without being asked twice |

## 2. Divergence 1 — input routing: **adopt Chad's kernel-routed `InputFocus`** (graft Prometheus's revocation analysis)
- **Prometheus:** InputFocusCap = a *convention* (`INPUT_RECV` right on the client's channel); HID events flow **HID → compositor → focused client's channel** (the compositor forwards).
- **Chad:** InputFocusCap = a **new kernel `InputFocus` object** with a root/leaf split — the compositor's root *activates/deactivates* focus (`sys_input_focus_activate`, an atomic route transfer); the client's receive-only leaf gets events **directly from the kernel**; the compositor **chooses** focus but never sees or forwards events.
- **Resolution: Chad's.** Three reasons. (1) It satisfies the ratified line *literally* — "input-security == input-routing, one code path" becomes one **kernel** path; with forwarding, the compositor is an input middleman and the enforcement is split between kernel and compositor code. (2) Chad's fork-table argument is decisive: compositor-forwarding leaves the **ambient raw-input path (fd 0) alive underneath** unless every caller is special-cased — the active-cap comparison retires it structurally. (3) Multi-principal (OT-22): a forwarding compositor sees every principal's keystrokes; kernel routing means even the compositor can't snoop — the stronger property for the world G1 must grow into. **Cost accepted:** a second new kernel object kind (two total, vs Prometheus's one). **Grafted from Prometheus:** the mid-frame revocation analysis (revoke-while-event-in-flight is harmless; define which surfaces consume input in the protocol) — it transfers cleanly onto the kernel route.
- *Cross-model note:* this is the one place the Qwen and Codex priors genuinely differed (minimalism-of-kernel-objects vs structural-security-in-the-kernel). The synthesis takes the security-dominant branch because input is the OT-22 frontier's sharpest edge.

## 3. Divergence 2 — the buffer object: **one object, Prometheus's rights model + Chad's lifetime mechanics**
Same concept (C3), different depth. The synthesis takes both halves:
- **From Prometheus — the rights model:** client-`WRITE` and compositor-`READ` are **sibling derivations from the same root**; neither can revoke the other (REVOKE requires the root); clean I-15 subset invariants per kind.
- **From Chad — the lifetime mechanics (load-bearing; a build without these ships a double-free):** an **immutable descriptor** stored in the kernel object (unchangeable via mapped memory); the **Pmap ownership split** — a mapping records `BorrowedBuffer`, removes PTEs on unmap/exit, and never frees borrowed leaf frames in `Pmap::Drop` (reusing `sys_map_region` frames *without* this ownership change would **double-free shared frames at process exit** — the catch of the panel); a mapping **pins the buffer Arc** until PTEs are gone, so destruction can't free frames still mapped or held by an in-flight present; compositor mapping **hardware read-only/NX**; standard TLB-shootdown discipline on unmap/revoke.

## 4. The merged shape (what the G1 build dispatch specifies)
- **Kernel additions (exactly two object kinds + their syscalls):** the shared pixel-buffer object (create/map; immutable descriptor; BorrowedBuffer Pmap ownership; Arc pinning; RO/NX compositor mappings) and the `InputFocus` object (root/leaf; atomic activate/route-transfer). Nothing else enters ring 0.
- **Protocol:** the union of the two verb tables over IPC channels (create-surface [role-typed, per Chad: `WORKFIELD_ROOT`/`TEXT`; G1 accepts exactly one full-screen root], attach/import-buffer, commit, present + fence, focus-request, destroy), each verb with caps-required stated; wire envelope per Chad §2.2; cap transfer on today's IPC per Chad §2.3.
- **Compositor (userland):** registry, scene, damage (full-repaint slice-1, rects later), pacing (adaptive, measured), focus *policy* (it decides; the kernel enforces), composition via the existing `fb::blit_rgba` path.
- **Slices (merged, smallest-first):** 1 one-surface proof (create→map→commit→blit, kill-switch armed) → 2 the protocol verbs complete → 3 the buffer lifetime mechanics + scribble-containment proof → 4 kernel `InputFocus` + focus transfer → 5 damage rects + pacing → 6 G0-Workfield-as-surface migration → 7 I-14 enumeration proven + kill-switch bit-for-bit regression. Kernel-touching slices (1, 3, 4) carry the QEMU gate + AAG.
- **Standing gates carried in:** kill-switch equality (off ≡ v1 console, bit-for-bit, proven not asserted); the I-14 section required at build; OT-22-readiness asserted at the protocol level (namespaced ids, cap provenance) with zero redesign expected.

## 5. Status + what the PL ratifies
This synthesis **is** the G1 design. It goes to the PL for ratification **at the G1 build dispatch** (when the GUI phase opens, post-OptiPlex, co-sequenced with OT-22) — nothing to decide today. The two divergence resolutions (§2, §3) are PM engineering judgments, overridable at that ratification; everything in §1 is three-family convergent and should be treated as settled absent new evidence. The GUI spine is now **design-complete through G1**: paradigm → design language → text engine → G0 spec → G1 design, each a short step from its build dispatch.
