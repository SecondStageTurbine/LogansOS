<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: DW-1 — the first real desktop (WM: two surfaces + keyboard focus switch) → Jarvis

**Date:** 2026-07-29
**From:** PM (Claude)
**To:** Jarvis (Codex — author of G1 **Slice 2** protocol verbs and **Slice 4** input routing, i.e. the exact ADR-073/074 surface DW-1 builds on; freed from the WiFi lane)
**Status:** DISPATCHED — **product lane.** This is the rung that turns "a compositor showing one transcript" into an actual **desktop**. Authoritative design: `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` (the cross-family panel: Oracle "Strata" ⟂ Janus 4-band, 11-point convergence). *Implementer deviation: the synthesis names Oracle as natural implementer (Strata's author), but Oracle now owns the WiFi lane — and DW-1 is composition+focus over the protocol verbs and input routing **you** authored, so the context fit is yours. Design authority stays the synthesis doc, not the implementer.*

## Why this is the product lane
Today `main` has a fully-merged G1 compositor (slices 1–5) and live glass (GPU-P1 slices 1–6) — but **exactly one client drawing one transcript**. No shell, no second window, no focus switching. DW-1 is the smallest rung that makes it a desktop, and it is **zero kernel delta** (no new syscalls, no new kernel objects — `focus.rs` needs *zero* changes). All policy is trusted userland, evolving `neo-init`'s `arthur_compositor`.

## Scope — the convergent smallest rung (synthesis §DW-1)
1. **Home/Spine root** + **two `ROLE_TEXT` `SurfaceCap`s** (two independent client surfaces, not one).
2. **Full-scene ordered composition** into one WM-owned buffer (the whole scene composited by the WM — not per-client direct present).
3. **Keyboard focus switch A↔B** via the existing `InputFocus` (ADR-074 kernel-routed input). Keyboard stays **WM-blind** — the WM chooses focus, never observes events (the anti-keylogger property; ACTIVATE vs RECEIVE root split).
4. **INV-WM-1 (the anti-X11 invariant, by grammar):** no verb may **name, enumerate, restack, or sample a peer**. This is the security core of the whole desktop — a window can never learn about or touch another window.

**Out of scope (do not pre-build):** focus history / Hold / move / resize (DW-2), the Home shell + intent field + `LaunchCap` (DS-1), **pointer + Secure-Attention-Key (DI-1 — separately ratified, carries its own cross-model check on the kernel input mechanism; do NOT pre-commit the ring-0-hit-route-vs-userland-forwarding decision)**, WM-owned living-glass chrome (DM-1).

## Required discriminators (both panel authors' — non-negotiable)
- **Focus isolation:** overlap two surfaces, switch focus, inject input → **only the focused client observes it** (the unfocused client must observe nothing).
- **Direct-present refusal:** **both** clients' direct-`present` attempts return **`EPERM`** (`scanout_authorized()` holds — only the WM scans out).
- **INV-WM-1 fuzz KAT:** a foreign-id / peer-restack fuzz asserts **`WrongOwner` / `CapRequired`** — prove a client cannot address a peer.
- **Stacking byte-oracle:** the composited scene is asserted against a hand-computed expected byte layout (independent of the compositor code — the standing byte-oracle rule; a writer+reader that agree can still both be wrong).
- **Focus atomicity:** count-conservation assert across a focus swap (no event lost, none duplicated).

## Kill-switch (mandatory, two levels — synthesis)
- **WM-off ≡ the one-surface G1 path**, bit-for-bit.
- **Atrium-off ≡ v1-console**, bit-for-bit (the existing `atrium-compositor-g1` default-off kill-switch is untouched).
Ship the regression proving both equalities. No new cross-cutting invariant — INV-WM-1 is *introduced with its guard in the same build* (the fuzz KAT above).

## Close-gate + merge
`scripts/worker-close-gate.sh` (rebase-fresh), green push-CI, and the **4-shard `workflow_dispatch` QEMU gate** on the exact SHA. **Runner sequencing:** DW-1 is kernel-touching-*light* but still QEMU-gated — **stagger it** behind the in-flight kernel gates (C1.1 is release-gating and holds priority; Slice-4/HD-530 is live). Coordinate before firing; do not pile a third heavy gate onto ~3 runners. **No auto-merge** — PM verify-before-merge + AAG (input-routing / capability-isolation lens, cross-family) before it lands. Branch: `codex/dw1-desktop-wm-jarvis`. Disk-reclaim after merge.

## Relay — Jarvis
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-29-dw1-desktop-window-manager-jarvis.md`, then the design authority `git show origin/main:docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` (§DW-1 + the 11-point convergence), plus `docs/ADR-073-atrium-surface-protocol-v1.md` and `docs/ADR-074-atrium-kernel-routed-input.md` (your slices).
- **One-line:** **Build DW-1 — the first real desktop.** Today we have a merged compositor with exactly ONE client drawing a transcript; DW-1 makes it a desktop: **Home/Spine root + two `ROLE_TEXT` SurfaceCaps + full-scene ordered composition into one WM-owned buffer + keyboard focus switch A↔B**, all **trusted userland policy in `neo-init`'s `arthur_compositor` with ZERO kernel delta** (no new syscalls; `focus.rs` unchanged). The security core is **INV-WM-1: no verb may name, enumerate, restack, or sample a peer** — anti-X11 by grammar. Keyboard stays WM-blind (WM picks focus, never sees events). Discriminators: focus isolation (only the focused client observes injected input), **both** clients' direct-present → `EPERM`, an INV-WM-1 foreign-id/peer-restack fuzz KAT asserting `WrongOwner`/`CapRequired`, a **stacking byte-oracle** (hand-computed expected scene bytes), and a focus-atomicity count-conservation assert. Kill-switch both levels: WM-off ≡ one-surface G1, Atrium-off ≡ v1-console, bit-for-bit. NOT in scope: move/resize/Hold (DW-2), the shell/intent field (DS-1), **pointer + SAK (DI-1 — separately ratified, don't pre-commit the ring-0 decision)**. Close-gate + 4-shard QEMU, but **stagger the gate** behind C1.1/Slice-4 (runner budget) — coordinate before firing. No auto-merge; PM + cross-family AAG.
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `atrium surface protocol create surface commit focus request`, `InputFocus activate receive route keyboard`, `arthur_compositor neo-init surface registry present`, `scanout_authorized EPERM direct present`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
