<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Gaming design — facet 1, the landscape/prior-art → Prometheus

**Date:** 2026-07-21
**From:** PM (Claude)
**To:** Prometheus (research seat, Qwen — cross-model family 1 of 3 on the gaming panel)
**Status:** DISPATCHED — **deep research, NO-GATE** (a FINDINGS feeding the PM gaming synthesis). One of three cross-model facets (you=Qwen, Cerebro=Claude, Jarvis=Codex) on the PL's gaming design — the deepest THINK on the roadmap. **Read the framing first:** `git show origin/main:docs/THINK-2026-07-21-gaming-on-logansos-framing.md` (the 5 mountains, the 5 options A–E, the through-line, the novel-tech provocations).

## Your facet — the anatomy + the precedent + the license reality
Map the **existing PC-gaming stack** precisely so the panel builds on truth, not vibes. Answer:
1. **Proton's anatomy** — exactly what **Wine + DXVK + VKD3D-Proton** each do, where the boundaries are, and what each *assumes about the host* (POSIX, threads, fds, the loader, X11/Wayland, the Vulkan driver). Which layers are cleanly separable vs deeply host-welded (apply the same std-coupling rigor from the browser/toolkit cross-checks).
2. **The Mesa/Vulkan reality** — how Linux gets a Vulkan driver (Mesa RADV/ANV/NVK; NVIDIA proprietary); what a Vulkan driver *is* in scope; the honest "adopt Mesa vs from-scratch" delta at the driver layer.
3. **The SteamOS / Steam Deck precedent** — the closest existing model (a Linux appliance that games near-flawlessly): what SteamOS actually is (Arch + Proton + Gamescope + a read-only immutable base), how it isolates/updates, and what it teaches a *bounded gaming compartment* (Option C).
4. **The license landscape (load-bearing — raw LICENSE, not WebFetch).** Wine (LGPL-2.1), VKD3D (LGPL-2.1), **DXVK (zlib — permissive!)**, Mesa (MIT), Gamescope, the Steam runtime. For each: license, and **what adoption into an MPL/cap-native world looks like** (LGPL dynamic-link vs static; can these be *used* by a bounded personality without license contamination of the MPL core?). This gates Options B/C.
5. **Anti-cheat reality** — how EAC/BattlEye work at the kernel level, why Proton support is *studio-opt-in*, which title classes work vs never will, and the honest verdict on the framing's "competitive-MP-with-kernel-AC is out of scope by design" line.
6. **The reachable target set** — with DRM-free (GOG) + Proton single-player/co-op, roughly how much of the real library is reachable *without* solving anti-cheat, and what's permanently walled.

## Deliverable + close
- `docs/RESEARCH-2026-07-21-gaming-facet-landscape-FINDINGS.md` (CC-BY-4.0): per-question, primary-sourced, confidence-tagged; the license table (raw-source verified); an explicit "adopt vs from-scratch, per layer" ledger; and your read on which of Options A–E the *landscape evidence* favors. Flag where you diverge from the framing.
- No-gate; push to `Workers\prometheus` on `prometheus/gaming-facet-landscape`; PM cherry-picks + synthesizes with Cerebro's (graphics) + Jarvis's (substrate) facets.

## Relay — what to tell Prometheus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-21-gaming-facet-landscape-prometheus.md` + the framing `…:docs/THINK-2026-07-21-gaming-on-logansos-framing.md`. Work in `Workers\prometheus`.
- **One-line:** **Gaming panel, facet 1 (landscape/prior-art).** Map the real PC-gaming stack for the panel: **Proton's anatomy** (Wine/DXVK/VKD3D — what each does + what it assumes about the host, which layers separate cleanly); the **Mesa/Vulkan** driver reality (adopt vs from-scratch); the **SteamOS/Steam Deck** precedent (the bounded-Linux-gaming-appliance model → informs Option C); the **license landscape** (Wine/VKD3D LGPL, **DXVK zlib**, Mesa MIT — raw-source-verified, and what adoption into an MPL/cap-native world costs — LGPL dynamic-link vs contamination); the **anti-cheat** reality (kernel EAC/BattlEye, studio-opt-in, the out-of-scope-by-design verdict); and the **reachable target set** (GOG DRM-free + Proton single-player/co-op = how much of the library without solving AC). Deliverable `docs/RESEARCH-2026-07-21-gaming-facet-landscape-FINDINGS.md`; feeds the PM gaming synthesis alongside Cerebro (graphics) + Jarvis (substrate).
- **RAG:** `GPU accel intel_gen9 vulkan`, `net tls fetch` (for streaming context), `OT-44 cap bundle install`, `OT-61 browser webrender`. Health-check `Invoke-RestMethod http://127.0.0.1:8404/health` first; if down, note it + grep/Read (PL owns RAG startup; never a blocker). **Exact bytes rule for licenses: read the raw LICENSE files, never a summarizing fetch.**
