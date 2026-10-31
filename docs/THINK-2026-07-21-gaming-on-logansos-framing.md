<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK (framing): Gaming on LogansOS — playing Steam/GOG games on a clean-slate cap-native OS

**Date:** 2026-07-21
**Author:** PM (Claude) — the framing/scaffolding pass; a cross-model panel (Prometheus/Cerebro/Jarvis) deepens each facet; PM synthesizes.
**PL goal (2026-07-21):** *"I am a gamer. I want LogansOS to play games — Steam, GOG, whatever. The deepest think we've done. Invent new tech if we must."*
**Status:** FRAMING — the honest terrain map + the strategic option space + the novel-tech provocations + the decision DAG. Deliberately ambitious AND rigorous: name the mountains truthfully, then find the tractable path up.

---

## 0. The honest headline
Gaming is the **most demanding workload an OS can host** and the **deepest single program on this roadmap — deeper than the browser.** It is not one project; it's **five mountains** stacked, three of which we have barely started and one of which (anti-cheat) is an *adversarial* wall that isn't fully solved even on Linux after 20 years. "The top AI coders" genuinely changes the *authoring* constant — but the constraints here are **structural** (a from-scratch modern GPU/Vulkan driver, the POSIX-substrate the whole PC-gaming stack assumes, the capability-vs-ambient impedance, DRM/anti-cheat adversaries), and structure doesn't yield to typing speed. So this THINK's job is to be *honest about the mountains* and then *find the phased path that makes them tractable* — including inventing the right cap-native primitives, per the PL's invitation. Reason in **dependency depth, iteration cycles, gates, and difficulty tier** (the no-human-time rule), never calendar.

## 1. What "run a Steam/GOG game" actually requires (the full stack)
Peel a running PC game and you find, top to bottom:
1. **The store/client** — Steam or GOG Galaxy: account/DRM, downloads/updates, the launcher, the runtime bundle. Steam is itself a large app with its own deps.
2. **The game binary** — usually a **Windows PE/x86_64** (run via Proton), sometimes **native Linux ELF**. Same CPU arch as us (x86_64) → **no CPU emulation needed** (the PL's "not an emulator" point holds).
3. **The Windows→host translation (Proton)** — Wine (Win32/NT API: kernel32/user32/gdi32/ntdll) + **DXVK** (Direct3D 9/10/11 → Vulkan) + **VKD3D-Proton** (Direct3D 12 → Vulkan) + patches. This is *the* PC-gaming compat layer.
4. **The graphics API** — the game (or DXVK/VKD3D) speaks **Vulkan** (or native GL). Vulkan is the modern lingua franca; everything funnels to it.
5. **The GPU driver** — a full, performant **Vulkan driver** on a real GPU. On Linux that's **Mesa** (RADV for AMD, ANV for Intel, NVK for NVIDIA) or NVIDIA's proprietary driver.
6. **The latency substrate** — low-latency **audio** (PipeWire-class), **controller/gamepad input** (HID + XInput/evdev + rumble), a **high-frame-rate presentation path** (compositor/vsync/tearing control, VRR), **threading/scheduling** for the game loop, big **mmap**/VRAM allocation, and **timers**.
7. **Anti-cheat** — for many multiplayer titles, **kernel-level anti-cheat** (Easy Anti-Cheat, BattlEye) that inspects the whole system; the studio must opt-in to Proton support, and some never do.

**Every layer 3–7 assumes a POSIX-ish, ambient-authority host with a mature GPU driver. We are none of those. That gap is the whole problem.**

## 2. The five mountains (and our starting altitude)
| Mountain | What it is | Our start | Difficulty |
|---|---|---|---|
| **M1 — GPU / Vulkan driver** | A full modern Vulkan driver on a real (ideally discrete AMD/NVIDIA) GPU | OT-63: a *minimal compute* device on an old Intel **Gen9 iGPU**. Not remotely a gaming GPU or a Vulkan driver. | **Extreme.** A from-scratch Vulkan driver for one modern GPU family is a multi-year megaproject; it's the tall pole for every local-play path. |
| **M2 — Proton/Wine compat** | DXVK+VKD3D+Win32 on our substrate | Nothing. And we're non-POSIX (the Wine blocker). | **Extreme.** Wine's 20-yr surface, minus the POSIX substrate it leans on. |
| **M3 — Steam-the-app + DRM** | The client, downloads, Steam DRM | Nothing. Steam assumes Linux/Windows. | **High.** A big app; DRM adds friction. GOG (mostly DRM-free) is easier. |
| **M4 — Anti-cheat** | Kernel-level EAC/BattlEye | Nothing — and it's a *fundamental clash* (§3). | **Adversarial wall.** Likely a documented dead-end for competitive MP; single-player + co-op is the reachable set. |
| **M5 — Latency substrate** | Audio, controller, frame-pacing, scheduling | Partial: USB-HID input ✓, a compositor (G1) ✓, no audio driver (OT-46 open), no gamepad, no VRR/frame-pacing. | **High but bounded** — the most "just build it" of the five. |

## 3. The capability impedance — gaming specifically, and the first-principles question
Two of our deepest charter properties collide with PC gaming:
- **Ambient authority.** A game (and Steam) assumes it can touch the filesystem, network, GPU, input, and often *other processes' memory* (overlays, anti-cheat) freely. Our model grants nothing unasked. A naive compat layer either **grants ambient authority** (punches the sovereignty moat, Article 1.7) or **confines the game** (breaks it).
- **Anti-cheat wants the opposite of us.** Kernel anti-cheat demands ring-0 visibility into the whole system to detect tampering. Our entire thesis is that *no component gets that.* These are irreconcilable by construction — so competitive-MP-with-kernel-AC is almost certainly **out of scope by design**, and that's an honest, defensible line (it's the same reason a hardened OS is the *last* place a rootkit-style AC wants to run).

**The provocation the PL invited:** don't just port the ambient PC model — ask *what the RIGHT way to game on a capability OS is.* A game is an ideal **cap-sandbox citizen**: it wants exactly a **brokered bundle** — a GPU submit cap, an audio-output cap, a controller-input cap, a bounded save-file cap, a network cap (or none) — and *nothing else.* A cap-native game runtime could give games **more** isolation than Windows/Linux (no game can scan your files or spy on siblings) while giving *them* everything they actually need through explicit brokers. That's a genuinely novel and *charter-strengthening* framing — gaming as the showcase of cap-brokered device access, not a hole in it.

## 4. The strategic option space (evaluate each honestly)
- **A — Full native stack from scratch** (our Vulkan driver + our Proton-equivalent + our Steam-equivalent). Maximal charter purity; **deepest program imaginable**; M1+M2+M3 all from zero. The THINK maps its DAG but it is not a near-term answer.
- **B — Adopt the Linux gaming stack via a bounded "Linux personality."** Build a **Linux-syscall + POSIX + DRM/KMS-shaped** compatibility substrate so *real* Mesa/Vulkan + Proton/DXVK + Steam run largely unmodified. This is **how you actually get thousands of games fastest** — you stand on the Linux gaming ecosystem (this is essentially the **SteamOS/Steam Deck** model, the strongest existing precedent: a Linux appliance that games beautifully). Cost: it **re-imports POSIX + the Linux driver ABI** — the exact thing the clean-slate charter escaped. The key question this THINK must answer: **can the Linux personality be a *bounded, kill-switched, cap-contained subsystem* (a "gaming compartment") rather than a contamination of the core?** If yes, this may be the pragmatic *and* defensible path.
- **C — A bounded gaming compartment** (the refined B). A confined, opt-in "**game mode**": an isolated Linux-compatible subsystem with its own POSIX+driver substrate, brokered to the cap-native core through a narrow, kill-switched seam — like a first-class, GPU-passthrough, near-native container, *not* a slow VM. Charter-consistent (bounded island, Article-1.7-preserving) *if* the isolation seam is real. This is likely the **leading candidate** and deserves the deepest design.
- **D — Native / portable games only.** Court native cap-native games or a portable runtime (Vulkan-portable, WASM+WebGPU). Charter-pure, but **no existing library** — you're building an ecosystem from zero. A long-tail complement, not the headline answer.
- **E — Cloud / streaming** (GeForce Now, xCloud). Games run remotely; LogansOS needs only a **streaming client + network + a decent browser/decoder**. Sidesteps M1–M4 entirely. Not "native gaming," but a **real, near-term partial win** (and it rides the browser/GPU-decode work). Worth naming as the fast path to "you can game on LogansOS *today-ish*."

## 5. The through-line: the GPU graphics API is rung-0 of every local path
Everything local (A/B/C) sits on a **real GPU graphics/compute API + Vulkan-class driver**. We are *already building* the first rungs of that for glass (OT-63 P1: compute device → ocloc kernel → silicon). **Gaming is the maximal extension of the GPU program we're on** — glass (Vello compute), browser (WebRender/GL), and games (Vulkan) are the same tall pole: *a real GPU driver.* So the honest sequencing is: **the GPU graphics API we're building is literally step 0 toward games**, and the gaming THINK should treat "which GPU, how far to Vulkan, adopt-Mesa-vs-from-scratch" as the pivot it shares with glass and the browser.

## 6. Novel-tech provocations (the "invent new tech" invitation — for the panel to chase)
1. **Cap-brokered game bundle** — a game as a confined cap-bundle whose *only* powers are brokered GPU-submit / audio-out / controller-in / save-file / (optional) net caps. Gaming as the *showcase* of device-brokering, not a breach of it.
2. **A "gaming compartment" isolation model** that gives near-native GPU/input/audio performance *without* a slow VM and *without* ambient authority — the core research question of Option C. (Is it GPU-passthrough-to-a-contained-Linux-personality? A cap-brokered Proton? A new primitive?)
3. **Anti-cheat, reframed** — since kernel-AC is out, what *cap-native* integrity story could a studio trust instead (attested cap-bundles, a brokered integrity oracle)? Probably won't satisfy today's AC vendors, but worth naming.
4. **The GPU driver acquisition strategy** — from-scratch Vulkan (megaproject) vs. adopting **Mesa** (MIT-ish, permissive — the realistic path) under our substrate vs. a bounded Linux-DRM personality that lets Mesa run as-is. The single highest-leverage decision.

## 7. What the cross-model panel must produce (assignments)
Three families, three facets (cross-model rule → the strong convergence/divergence signal):
- **Prometheus (Qwen) — landscape/prior-art:** the *exact* anatomy of Proton/DXVK/VKD3D/Wine/Mesa/Steam-runtime + the **SteamOS/Steam Deck** precedent; the **license landscape** (Wine/VKD3D LGPL, DXVK zlib, Mesa MIT — what's adoptable into an MPL world and how); what "adopt vs from-scratch" costs at each layer; the anti-cheat reality (which titles/AC work on Proton, which never will).
- **Cerebro (Claude, GPU/compositor owner) — the graphics mountain (M1):** Vulkan-on-our-GPU; **adopt-Mesa vs from-scratch vs Linux-DRM-personality**; which GPU family to target (the PL's hardware: AMD Vega/M75q? a discrete card?); how it extends OT-63 + the shared GPU-API pivot; DXVK/VKD3D's Vulkan requirements; frame-pacing/VRR/present.
- **Jarvis (Codex) — the substrate & compat mountain (M2–M5) + the compartment:** the Win32/Proton layer on non-POSIX; the **bounded Linux-personality / gaming-compartment** design (Option C) — the isolation seam, GPU/input/audio brokering, kill-switch; the capability-vs-ambient + anti-cheat impedance; Steam-the-app + DRM; the latency substrate (audio OT-46, gamepad, scheduling).
- **PM synthesis** → the deep THINK with a **recommended direction + a phased de-risking DAG** (what to *prove* before any big commit) + honest go/no-go gates; then a cross-model cross-check of the recommendation.

## 8. Bottom line (framing verdict, pre-panel)
- Gaming is **real but the deepest program on the roadmap.** The near-term honest answer is almost certainly **not** "port Wine + write a Vulkan driver" — it's a **phased de-risking**: (i) keep building the **GPU graphics API** (shared with glass/browser — rung 0); (ii) design the **bounded gaming compartment** (Option C) as the charter-consistent way to host the Linux gaming stack near-native without contaminating the core; (iii) prove a **cap-brokered GPU/input/audio path** on real hardware; (iv) land a **near-term partial win** (cloud/streaming, Option E, and/or a single native/GOG-DRM-free title through the compartment) so "you can game on LogansOS" becomes *true early*, then deepen.
- The line we hold honestly: **competitive multiplayer with kernel anti-cheat is out of scope by design** (it's irreconcilable with the security thesis) — single-player, co-op, and DRM-free (GOG) are the reachable, and very large, target set.
- The panel's job is to pressure-test this framing, price the mountains in cycles/gates, and either confirm the phased path or find a better one. **Ambition intact; rigor intact.**
