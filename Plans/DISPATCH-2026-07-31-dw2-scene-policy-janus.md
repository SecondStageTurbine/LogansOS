<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: DW-2 — desktop scene policy (focus history, Hold/restore, dismissal, bounded resize) → Janus

**Date:** 2026-07-31
**From:** PM (Claude)
**To:** Janus (Codex — **co-author of the desktop-UX vision** (the 4-band "Atrium WM + Home Shell" doc that converged with Oracle's "Strata"); this rung is your own design)
**Status:** DISPATCHED — **product lane, 0.1 rung 2 depth.** Design authority: `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` §DW-2. **Stacks on DW-1** (Jarvis, `codex/dw1-desktop-wm-jarvis` @ `7520337b`) and merges after it.

## Where this sits
DW-1 gives the first real desktop: Home/Spine root, two `ROLE_TEXT` surfaces, full-scene composition, keyboard focus switch A↔B — zero kernel delta, all trusted userland policy in `neo-init`'s WM. **DW-2 makes it usable**: the scene gains *policy* — remembering, hiding, restoring, dismissing, and resizing windows — still keyboard-only, still no ring-0.

## Scope (synthesis §DW-2, verbatim intent)
1. **Focus history** — a focus order the WM maintains, so focus-switch is "next/previous in history," not a fixed A↔B toggle. Closing or hiding the focused window restores focus to the right predecessor.
2. **Hold / restore** — "minimize" as **Hold**: a held window leaves the scene and becomes a **Spine chip**; restoring returns it to the scene with its buffer intact. (The Spine is the intent surface, not a taskbar metaphor — see §DS-1 for where it's going; DW-2 only needs the chip.)
3. **Dismissal** — closing a window: the surface is destroyed, its cap released, the scene recomposed, focus handed on per history.
4. **Bounded move/resize — compositor-laid-out, `OP_CONFIGURE` SIZE-ONLY.** This is a load-bearing design decision from the cross-model convergence: **the WM decides layout; a client never positions itself.** `OP_CONFIGURE` carries size only — no coordinates. A client that wants to be somewhere asks for nothing; the WM places it.
5. **Whole-scene redraw** — recomposition on any scene change (focus, hold, restore, dismiss, resize), composited into the single WM-owned buffer.

## Hard constraints (do not relax)
- **INV-WM-1 stands:** no verb may **name, enumerate, restack, or sample a peer.** Every new verb here must be expressible without a client learning that another window exists. This is the anti-X11 security core; if a feature seems to need peer knowledge, the design is wrong, not the invariant.
- **Keyboard stays WM-blind** — the WM chooses focus, never observes key events (ADR-074 ACTIVATE vs RECEIVE split; the anti-keylogger property).
- **Zero kernel delta expected** — `focus.rs` and the kernel objects should need no change, exactly as DW-1. If you believe a kernel change is required, **stop and say so** rather than adding one; that's a design escalation, not a build decision.
- **NO pointer, NO Secure-Attention-Key.** That is **DI-1**, which is **separately ratified** and carries its own cross-model adversarial check on the kernel input mechanism. **Do not pre-commit the ring-0-hit-route-vs-userland-forwarding decision** in any way — not in code, not in a "for later" hook.
- **No shell / intent field / `LaunchCap`** — that's DS-1.

## Kill-switch + invariants
- **DW-2 OFF ≡ DW-1 behavior, bit-for-bit** (regression proving equality). DW-1's own switches stay intact: WM-off ≡ one-surface G1; Atrium-off ≡ v1-console.
- No new cross-cutting invariant expected. INV-WM-1 is *preserved*, not re-introduced — extend DW-1's fuzz KAT to cover every new verb.

## Required discriminators
- **Focus-history correctness:** hold or dismiss the focused window → focus lands on the correct predecessor, deterministically (not "whatever was next in an array").
- **Hold/restore fidelity:** a held window's buffer survives and restores **byte-identical** (a stacking byte-oracle before hold vs after restore).
- **Dismissal releases the cap** — prove the surface cap is actually dropped, not leaked (a count-conservation assert across create/dismiss cycles).
- **`OP_CONFIGURE` is size-only** — a KAT asserting the verb *cannot* express a position; a client attempting to place itself gets no such verb.
- **INV-WM-1 fuzz extended** to every DW-2 verb: foreign-id / peer-restack / peer-sample attempts return `WrongOwner` / `CapRequired`.
- **Scene byte-oracle** for the composited result after each policy operation.

## Close-gate + merge
`scripts/worker-close-gate.sh`, green push-CI, and the **4-shard `workflow_dispatch` QEMU gate** on the exact SHA. **Merges after DW-1** — rebase onto DW-1's final merged SHA before your own gate. **No auto-merge**: PM verify + AAG (capability-isolation / input-routing lens, cross-family — you're Codex, so the lens will be a Claude seat). Disk-reclaim after merge.

**Network note (2026-07-31):** the PL's network is down, so runners and pushes are unavailable. **Build and `docker neo-test` locally, commit at every checkpoint**, and push when connectivity returns. Do not let this work exist as a single uncommitted copy.

## Relay — Janus
- **WORKTREE:** `E:\AI Automation\workers\janus` (yours alone — never another coder's folder, never the repo root)
- **BRANCH:** `codex/dw2-scene-policy-janus`, based on DW-1: `git fetch origin && git checkout -B codex/dw2-scene-policy-janus origin/codex/dw1-desktop-wm-jarvis`
- **Read:** `git show origin/main:Plans/DISPATCH-2026-07-31-dw2-scene-policy-janus.md`, then your own design authority `git show origin/main:docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` (§DW-2 + the 11-point convergence), plus `docs/ADR-073-atrium-surface-protocol-v1.md` and `docs/ADR-074-atrium-kernel-routed-input.md`.
- **One-line:** **Build DW-2 — the rung that makes the desktop usable, from your own design.** DW-1 (Jarvis, stacking under you) gives two surfaces + a keyboard focus switch; DW-2 adds **scene policy**: focus **history** (next/previous, correct successor on hide/close), **Hold/restore** (minimize → Spine chip, buffer restores byte-identical), **dismissal** (surface destroyed, **cap released** — prove it with count-conservation), **bounded move/resize via `OP_CONFIGURE` SIZE-ONLY** (the WM lays out; a client can never position itself — assert the verb cannot express a position), and **whole-scene redraw** into the single WM-owned buffer. Still **keyboard-only, WM-blind, zero kernel delta**. **INV-WM-1 holds — no verb may name/enumerate/restack/sample a peer**; extend DW-1's fuzz KAT to every new verb. Kill-switch DW-2-OFF ≡ DW-1 bit-for-bit. **NOT in scope: pointer + SAK (DI-1, separately ratified — don't pre-commit the ring-0 decision even as a hook), and the shell/intent field/LaunchCap (DS-1).** If you think a kernel change is needed, stop and tell me — that's a design escalation. Network is down: build + test locally, **commit at every checkpoint**, push when it returns.
- **RAG** (PowerShell) health-check first (`Invoke-RestMethod http://127.0.0.1:8404/health`); e.g. `atrium surface protocol OP_CONFIGURE commit destroy`, `InputFocus activate receive focus route`, `arthur_compositor scene composition surface registry`, `SurfaceCap release destroy cap count`. If down, note it in the RETURN + grep/Read (PL owns RAG startup — never a blocker).
