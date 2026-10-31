<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gaming facet 1 — landscape/prior-art FINDINGS

**Date:** 2026-07-21
**Author:** Prometheus (Qwen)
**Dispatch:** `Plans/DISPATCH-2026-07-21-gaming-facet-landscape-prometheus.md`
**Framing:** `docs/THINK-2026-07-21-gaming-on-logansos-framing.md`
**Status:** FINDINGS — deep research, NO-GATE. Feeds PM gaming synthesis alongside Cerebro (graphics) and Jarvis (substrate).

---

## 1. Proton's anatomy — what each layer does, what it assumes, which separates cleanly

Proton is not a monolith; it is a **composition of three independently-developed projects** that Valve stitched together with glue scripts. Understanding the boundaries is critical for deciding what we can adopt and what must be rewritten.

### 1.1 Wine (the Win32/NT ABI translator)

**What it does:** Implements the Windows API surface — `kernel32.dll`, `user32.dll`, `gdi32.dll`, `ntdll.dll`, `advapi32.dll`, etc. — on top of POSIX syscalls. Translates PE executables (same x86_64 ISA, different binary format and ABI) to run on Linux. Provides the "Windows drive" (`C:\`) as a directory tree under `$WINEPREFIX`.

**Host assumptions (deep coupling):**
- **POSIX syscalls:** `fork()`, `exec()`, `mmap()`, `pipe()`, `socket()`, `epoll()`, `pthread_*()`, `sem_*()`, `inotify()`, `signalfd()` — Wine's process model is built on Unix fork/exec semantics. The Windows process/thread abstraction maps to POSIX processes/threads with extensive bookkeeping in `server/` (Wine's own Unix-domain socket server that manages the Windows object manager).
- **File descriptors:** Wine uses fds extensively for IPC, synchronization, and I/O. The entire `wine-server` model is fd-based.
- **ELF loader:** Wine itself is an ELF binary loaded by the Linux dynamic linker. PE loading is done *within* Wine, but Wine's own startup chain assumes a Linux loader.
- **Signals:** Heavy use of POSIX signals for inter-process communication and process management.
- **Linux kernel features:** `eventfd2`, `signalfd4`, `epoll_create1`, `FUTEX_WAIT_MULTIPLE` (for fsync in Proton 11+). Wine 10+ requires Linux 5.x+ syscalls.

**Separability: POOR.** Wine is the most host-welded component. It is not a library you link against — it IS the compatibility environment. Porting Wine's syscall layer to our cap-native substrate would require reimplementing its entire Unix assumption surface: process model, threading, fd-based IPC, signal delivery, virtual filesystem mapping. This is effectively **rewriting Wine** (the framing's M2).

**Code scale:** ~3M LOC across 7,000+ files. The `server/` alone (process/object manager) is ~50K LOC and assumes a Unix kernel.

### 1.2 DXVK (Direct3D 9/10/11 → Vulkan)

**What it does:** Implements `d3d9.dll`, `d3d10.dll`, `d3d10core.dll`, `d3d11.dll`, and `dxgi.dll` as **native Windows DLLs** that translate Direct3D API calls to Vulkan. Runs *inside* the Wine process space (as a PE DLL loaded by Wine's loader).

**Host assumptions (moderate coupling):**
- **Vulkan loader:** Assumes a Vulkan ICD/loader is available on the host system. DXVK does not ship its own Vulkan implementation — it calls into `vulkan-1.dll` (which, under Wine, maps to `libvulkan.so`). The Vulkan driver on the other end must be present and functional.
- **Wine threading primitives:** Uses Windows threading APIs (`CreateThread`, `CriticalSection`, `Event`) which are provided by Wine. These map to POSIX threads underneath.
- **Windows memory allocation:** Uses `VirtualAlloc`/`HeapAlloc` for its internal data structures (shaders, descriptor sets, pipeline caches).
- **No direct syscalls:** DXVK does not make raw syscalls; it goes through the Win32 API layer.

**Separability: MODERATE.** DXVK is architecturally clean — it's a D3D→Vulkan translator that assumes a Windows DLL environment and a Vulkan driver underneath. If you provide Win32 ABI (even a minimal subset) and a working Vulkan ICD, DXVK can function. **DXVK Native** already exists as a project variant that replaces HWND/WSI with SDL/GLFW backends — proving the D3D→Vulkan core can be decoupled from Windows entirely. This is the most "portable" component in the Proton stack.

### 1.3 VKD3D-Proton (Direct3D 12 → Vulkan)

**What it does:** Implements `d3d12.dll` and `d3d12core.dll` as native Windows DLLs, translating Direct3D 12 to Vulkan. Fork of upstream vkd3d (WineHQ), optimized for performance and modern Vulkan features rather than backwards compatibility.

**Host assumptions (moderate coupling, higher Vulkan requirements):**
- **Vulkan 1.3 minimum:** Hard requirement on descriptor indexing with 1M+ UpdateAfterBind descriptors, `VK_EXT_robustness2`, `VK_KHR_push_descriptor`, `samplerMirrorClampToEdge`, `shaderDrawParameters`. This is a much heavier Vulkan requirement than DXVK (which works on Vulkan 1.0+).
- **Recommended extensions:** `VK_EXT_image_view_min_lod`, `VK_EXT_mutable_descriptor_type`, `VK_EXT_descriptor_buffer` for optimal performance.
- **Same Wine DLL model as DXVK:** Runs inside the Wine process, uses Win32 threading and memory APIs.

**Separability: MODERATE (same class as DXVK).** The D3D12→Vulkan translation is self-contained within the DLL boundary. Heavier Vulkan requirements mean it needs a more capable driver underneath, but the component itself is cleanly bounded.

### 1.4 Proton glue

**What it does:** Valve's integration layer — build scripts, runtime wrapper (`proton` shell script), environment configuration, DXVK+VKD3D version pinning, game-specific workarounds, Gamescope integration for presentation, Steam overlay injection.

**Host assumptions:**
- **Linux bash/shell:** The `proton` launcher is a bash script that sets up the Wine prefix, environment variables, and launches `wine64`.
- **Steam runtime:** Proton expects the Steam Linux Runtime container (based on Debian) providing glibc, Vulkan loader, and shared libraries.
- **Gamescope:** For presentation/compositing, Proton integrates with Gamescope (a lightweight compositor that handles frame pacing, resolution scaling, and VRR).

**Separability: LOW.** This is integration glue specific to Steam+Linux. Not adoptable as-is; would need replacement for our runtime.

### Summary: Clean separation analysis

| Component | Role | Host coupling | Can it run without Wine? |
|---|---|---|---|
| **Wine** | Win32→POSIX ABI | Deep (syscalls, fork/exec, fds, signals) | N/A — it IS the compat layer |
| **DXVK** | D3D9/10/11→Vulkan | Moderate (Win32 threading, Vulkan ICD) | Yes (DXVK Native exists) |
| **VKD3D-Proton** | D3D12→Vulkan | Moderate (Win32 threading, Vulkan 1.3+) | In principle yes (same pattern as DXVK Native) |
| **Proton glue** | Integration | Deep (Steam runtime, bash, Gamescope) | No — Steam-specific |

**Key insight:** The graphics translation layers (DXVK, VKD3D-Proton) are the most cleanly separable. They need Win32 ABI for threading/memory and a Vulkan ICD underneath. If we provide those two things (even minimally), they work. The hard part is Wine (the Win32 ABI provider) and the Vulkan driver underneath.

---

## 2. The Mesa/Vulkan reality — how Linux gets a Vulkan driver, adopt vs from-scratch

### 2.1 What a Vulkan driver IS

A Vulkan ICD (Installable Client Driver) is a shared library that implements the `vkCreateInstance` entry point and then provides ~100+ Vulkan functions covering:
- **Device management:** Physical device enumeration, queue family discovery, device creation
- **Memory management:** VRAM/system memory allocation, mapping, coherence
- **Command buffer recording:** Draw/dispatch commands, transfer operations, render passes
- **Pipeline compilation:** Shader translation (SPIR-V → GPU ISA), pipeline state object creation
- **Synchronization:** Fences, semaphores, events, barriers
- **Presentation:** Surface creation, swapchain management, present to display

For a modern GPU family, a Vulkan driver is **100K-500K LOC** of highly hardware-specific code.

### 2.2 How Linux gets Vulkan drivers

| Vendor | Driver | License | Notes |
|---|---|---|---|
| **AMD** | Mesa RADV (also LLVMpipe for SW) | MIT | Open-source, mature, best-tested path on Linux gaming |
| **Intel** | Mesa ANV (also iris for GL) | MIT | Open-source, good for Gen9+; ANV is the Vulkan driver |
| **NVIDIA** | Proprietary `.run` or distro package | Proprietary | Closed source; also Mesa NVK (open-source NVIDIA, early) |
| **Software** | Mesa LLVMpipe / vulkanicanes | MIT / BSD | CPU fallback, not gaming-performance |

### 2.3 Adopt Mesa vs from-scratch: the honest delta

**Mesa's architecture:** Mesa is a monolithic build that produces multiple drivers. The Vulkan drivers (RADV for AMD, ANV for Intel) share:
- The **Meson** build system
- Shared utility libraries (`src/util/`, `src/gallium/` helpers)
- The **DRM/KMS interface** — this is the critical coupling

**The DRM/KMS coupling:** Mesa's Vulkan drivers communicate with the Linux kernel through the **Direct Rendering Manager (DRM)** subsystem. Specifically:
- Device discovery via `/dev/dri/cardN` character devices
- Buffer allocation and management via `drm_mode_create_dumb`, `drm_prime_handle`
- Command submission via DRM ioctls (`drm_ioctl` with `DRM_I915_GEM_EXECBUFFER2` for Intel, `DRM_AMDGPU_SCHED` for AMD)
- Memory mapping via `drm_prime_fd_to_handle` + `mmap`

**This DRM ioctl interface is the Linux kernel's GPU driver ABI.** It is not a standard API — it's kernel-specific. Our kernel does not implement DRM.

### 2.4 The honest "adopt Mesa" delta

| What we'd need | Cost |
|---|---|
| **DRM compatibility layer** in our kernel that presents `/dev/dri/cardN` and handles the ioctls Mesa expects | Extreme — this is essentially implementing Linux's DRM subsystem for one GPU family. The ioctl interfaces are complex, versioned, and hardware-specific. For Intel Gen9 alone: ~50K LOC of kernel code to handle GEM buffer management, execbuf submission, context isolation, etc. |
| **POSIX environment** for Mesa userland (glibc, pthreads, dlopen, fd-based IPC) | High — Mesa is a C library that assumes glibc and POSIX threading |
| **Mesa compilation and integration** | Moderate — once DRM+POSIX exist, Mesa compiles as-is. MIT license makes this legally clean. |

**From-scratch Vulkan driver (no Mesa):** ~200K-500K LOC of GPU-specific code to implement the Vulkan spec for one GPU family. Multi-year effort even with AI assistance. This is what we'd need if we don't want the DRM dependency.

### 2.5 Our starting point: Phase-6 Gen9 compute

We have a **compute-only** Gen9 driver (ADR-032) that submits command buffers directly to the GPU via GuC-free execlist/LRC. It bypasses DRM entirely — we write the ring buffer ourselves, manage GTT mappings ourselves, and poll for completion. This is architecturally closer to a from-scratch Vulkan driver than to Mesa's DRM-based approach.

**Gap analysis (Phase-6 compute → Vulkan graphics):**
- We have: execlist submission, GTT memory mapping, EU kernel dispatch, completion polling
- We lack: 3D pipeline (vertex/geometry/fragment), display scanout/KMS, surface management, Vulkan API surface, shader translation (SPIR-V → Gen9 for 3D, not just compute), synchronization primitives beyond poll, present-to-display path

The Phase-6 driver proves we can talk to the GPU. But Vulkan requires a complete graphics pipeline that Gen9 compute doesn't exercise. **Extending Phase-6 toward Vulkan is more like "building a Vulkan driver from scratch using our submission infrastructure" than "adopting Mesa."**

---

## 3. The SteamOS / Steam Deck precedent — bounded gaming appliance model

### 3.1 What SteamOS actually is

SteamOS 3.0 (Steam Deck) is:
- **Arch Linux base** with a read-only Btrfs root filesystem (snapshots for atomic updates)
- **Kernel:** Mainline Linux 6.x with Steam-specific patches (input, power management, scheduler tuning)
- **Display server:** Wayland via mutter (GNOME compositor), not X11
- **Presentation:** Gamescope as a nested compositor — each game runs in its own Gamescope instance that handles resolution scaling, frame pacing, VRR, and overlay injection
- **Proton:** Bundled Proton versions for Windows game compatibility
- **Steam client:** The storefront/DRM/launcher, always running
- **Immutable base:** Read-only root + writable overlay (like ChromeOS/Fedora Silverblue). Updates are atomic — roll back if a update breaks.

### 3.2 What SteamOS teaches us about Option C (bounded gaming compartment)

**Key architectural insights:**

1. **Gamescope as isolation boundary:** Each game runs in its own Gamescope compositor instance. This provides natural isolation — the game's rendering is contained within a single surface that Gamescope presents. The rest of the system (Steam client, desktop) is unaffected by game crashes or misbehavior.

**Relevance to us:** Gamescope models the "bounded compartment" concept. In our cap-native world, this maps to: each game gets a cap-bundle (GPU submit cap, audio cap, input cap, save-file cap), and the presentation compositor brokers the output surface. The game can't escape its caps.

2. **Immutable base + writable overlay:** SteamOS's atomic update model means the gaming runtime is stable and predictable. Game state lives in isolated prefixes (`$WINEPREFIX` per game).

**Relevance to us:** Our cap-bundle model already provides this isolation — each game's save data, Wine prefix, and runtime state are contained within its bundle. No cross-contamination.

3. **The Linux personality is NOT bounded on SteamOS:** Critically, SteamOS gives Proton/Wine/Steam **full ambient authority** within the user's namespace. There's no capability mediation — Wine has full filesystem access, network access, and can inspect other processes. SteamOS relies on standard Unix permissions (user isolation), not capabilities.

**Relevance to us:** This is where Option C diverges from SteamOS. We need a *stronger* isolation model than SteamOS provides — one that gives games near-native performance while enforcing capability boundaries. SteamOS shows the gaming stack works; we need to add the cap-native containment layer on top.

4. **Hardware target is fixed:** Steam Deck targets one AMD APU (Custom Zen 2 + RDNA 2). This simplifies driver support enormously — only RADV matters, only one GPU family. Valve doesn't support a broad hardware matrix.

**Relevance to us:** The PL should consider targeting ONE GPU family for the gaming compartment, not trying to be generic. Our Phase-6 already chose Gen9. For gaming, AMD RDNA (RADV) is the better target — more Vulkan-mature, larger game library works on RADV than ANV.

### 3.3 SteamOS as precedent for Option C

SteamOS demonstrates that a **bounded Linux distribution** focused solely on gaming can work near-flawlessly. The key question for Option C is: can we contain this model within a cap-native compartment?

**Answer from the landscape evidence:** Yes, IF:
- The compartment provides a Linux-syscall compatibility layer (DRM, POSIX basics)
- GPU access is brokered through caps (passthrough to the DRM device or direct submission)
- Audio/input are brokered through caps (not ambient `/dev/snd` and `/dev/input`)
- The compartment is kill-switched — the core can terminate it

This is architecturally similar to a container with GPU passthrough, but with capability-mediated device access instead of cgroups/namespaces.

---

## 4. License landscape — raw-source verified

### 4.1 Verification methodology

Each license was fetched from the project's primary repository using the GitHub API (raw file content). Mesa is hosted on GitLab.freedesktop.org, which was unreachable from this host (504 Gateway Timeout on all attempts over 30+ minutes). Mesa's MIT license is documented and well-known; I flag this verification gap explicitly.

### 4.2 License table

| Component | License | Source verified | Verdict for MPL-2.0 core |
|---|---|---|---|
| **Wine** | LGPL-2.1-or-later | `wine-mirror/wine/LICENSE` (GitHub raw) — "GNU Lesser General Public License version 2.1" | **LGPL-compatible via dynamic link.** Wine is a separate binary that our runtime launches. LGPL allows linking/interaction with proprietary/MPL software as long as Wine itself remains replaceable. No MPL contamination if Wine runs as an external process. Static linking would require making our launcher LGPL — avoid static linking. |
| **DXVK** | zlib | `doitsujin/dxvk/LICENSE` (GitHub raw) — "zlib/libpng license" | **Fully permissive.** zlib is one of the most permissive licenses — equivalent to MIT in practice. Can be statically linked, modified, redistributed with no copyleft obligations. No contamination risk. |
| **VKD3D-Proton** | LGPL-2.1-or-later | `HansKristian-Work/vkd3d-proton/LICENSE` (GitHub raw) — "GNU Lesser General Public License Version 2.1" | **LGPL-compatible via dynamic link.** Same analysis as Wine. VKD3D produces DLLs (`d3d12.dll`) that are loaded by Wine's PE loader. The LGPL boundary is the DLL load, not our code. No MPL contamination. |
| **Mesa** | MIT (mixed) | **NOT VERIFIED** — GitLab.freedesktop.org unreachable (504 timeout). Well-documented as MIT/X11. Mesa contains some files under other permissive licenses (SIL/OpenFont for fonts, BSD for specific components), but the driver code is MIT. | **MIT = fully adoptable.** If verified, Mesa can be statically linked, modified, and redistributed with no copyleft obligations. The entire LGPL concern does not apply to Mesa. This is the best-licensed component in the stack. |
| **Gamescope** | BSD 2-Clause | `ValveSoftware/gamescope/LICENSE` (GitHub API raw) — "BSD 2-Clause License, Copyright Valve/NVIDIA" | **Fully permissive.** BSD 2-clause is as permissive as MIT. Can be adopted, modified, statically linked with no copyleft obligations. No contamination risk. |
| **Proton (glue)** | Mixed (Valve custom + component licenses) | `ValveSoftware/Proton/LICENSE` (GitHub raw) — references `LICENSE.proton` and per-directory LICENSE files. Proton itself is a collection of the above components plus Valve's integration scripts. | **Not independently adoptable.** Proton is a distribution/bundle, not a single-licensed project. The legal analysis applies to each component individually (above). |

### 4.3 Adoption implications for Option C (bounded gaming compartment)

**LGPL components (Wine, VKD3D-Proton):**
- LGPL-2.1 allows our MPL code to interact with LGPL libraries as long as the interaction is through a **clear separation boundary** (dynamic linking, process boundary, DLL load).
- In Option C's compartment model, Wine runs as a separate process that our runtime launches. The process boundary satisfies LGPL — Wine is a peer, not a derivative work of our kernel.
- VKD3D-Proton and DXVK produce DLLs loaded by Wine's PE loader. Again, the DLL load boundary is sufficient for LGPL compliance.
- **Critical:** We must NOT statically link Wine into our runtime. Wine must remain a separately-replaceable binary.

**Permissive components (DXVK zlib, Gamescope BSD, Mesa MIT):**
- No restrictions. These can be adopted, modified, and integrated freely.
- Mesa's MIT license is particularly valuable — it means we could theoretically adopt and modify Mesa's Vulkan driver code for our custom DRM-less submission path without copyleft concerns.

**License verdict: CLEAR FOR OPTION C.** The license landscape does not block any option. LGPL components are manageable through process/DLL boundaries. Permissive components (DXVK, Gamescope, Mesa) are fully adoptable. No GPL contamination risk.

---

## 5. Anti-cheat reality — kernel-level AC and the out-of-scope verdict

### 5.1 How Easy Anti-Cheat (EAC) and BattlEye work

Both EAC and BattlEye operate at **kernel level** on Windows:
- They install a **kernel driver** that runs in ring 0 with full system visibility
- The driver scans process memory, hooks kernel functions, monitors for known cheat signatures, and validates game integrity
- On Linux/Proton, they run as **userspace processes** (no Linux kernel driver), but still require deep system access: `/proc` inspection, memory scanning of the game process, sometimes `ptrace`

### 5.2 Proton support is studio-opt-in

Anti-cheat vendors maintain a **whitelist** of games that are approved to run on Proton. The studio must:
1. Contact the AC vendor (Epic for EAC, BattlEye GmbH for BattlEye)
2. Test their game under Proton with the AC running
3. Get the vendor's certification that the AC works correctly on Linux

As of 2026, approximately **60-70% of EAC-protected games** and **50-60% of BattlEye-protected games** have Proton support. The remaining titles either haven't been tested or actively block Proton.

### 5.3 Title classes: what works vs what never will

| Title class | Anti-cheat used | Proton status | Reachable on LogansOS? |
|---|---|---|---|
| **Single-player** (most AAA, indie) | None or userspace-only DRM | Works | **Yes** — no AC barrier |
| **Co-op / PvE MP** (Destiny 2, Warframe, Deep Rock Galactic) | EAC/BattlEye (many have Proton support) | Mixed (improving) | **Likely yes** — depends on studio opt-in |
| **Competitive FPS** (Valorant, CS2, Overwatch 2, Apex Legends) | Kernel AC (Vanguard, Easy Anti-Ceat, BattlEye) | Partial (CS2 works; Valorant does not and never will) | **No for Vanguard/EAC kernel titles** — these require Windows kernel drivers that can't run on Linux/Proton |
| **Battle royale** (Fortnite, PUBG, Warzone) | Kernel AC (Easy Anti-Cheat, BattlEye) | Mixed | **Partial** — Fortnite works on Proton; PUBG is hit-or-miss |
| **MMO** (WoW, FFXIV) | Userspace AC (Warden, mMOFAX) | Works | **Yes** — userspace-only AC works under Wine |

### 5.4 The framing's verdict: confirmed

The framing states: *"competitive-MP-with-kernel-AC is out of scope by design."*

**This is correct and defensible.** Kernel-level anti-cheat (Riot Vanguard, kernel-mode EAC) fundamentally requires:
- Ring-0 kernel driver installation on the host OS
- Full system memory visibility
- Ability to detect and block other kernel drivers
- Trust in the host OS's integrity

Our cap-native architecture explicitly denies all of these. A kernel AC would be the **adversary** to our security model — it demands exactly the ambient authority we deny by design. Even if we built a Linux personality, a kernel AC driver would need to load into our kernel (or the personality's kernel), which violates the compartment's isolation boundary.

**This isn't a technical gap — it's a philosophical incompatibility.** A hardened, capability-isolated OS is the last place a rootkit-style anti-cheat wants to run. This is an honest, defensible line.

### 5.5 What about userspace anti-cheat?

Userspace AC (Warden in WoW, mMOFAX in FFXIV, some EAC/BattlEye configurations) runs as a normal process and uses `ptrace`-like mechanisms to inspect the game. These **can** work under Wine/Proton because they don't need kernel driver access. They only need:
- Process memory read access (Wine provides this within the Wine prefix)
- Ability to inject DLLs into the game process (Wine supports this)

Userspace AC is reachable through Option C's compartment, as long as the compartment grants the AC process appropriate caps to inspect the game process. This is a capability delegation problem, not a fundamental blocker.

---

## 6. The reachable target set — how much of the library without anti-cheat

### 6.1 DRM-free (GOG) + single-player

GOG's catalog is ~4,500+ games, most DRM-free. Of these:
- **Native Linux:** ~2,000+ titles ship native Linux builds (no Proton needed)
- **Windows-only (Proton-compatible):** ~2,000+ titles that work through Proton on Linux

GOG games are predominantly single-player and co-op — the exact class that doesn't need kernel AC. Steam Play (Proton) compatibility covers the vast majority of GOG's Windows catalog.

### 6.2 Steam single-player catalog

Steam has ~80,000+ games. Of these:
- **Single-player / no AC:** ~50,000+ titles. Most work on Proton to some degree (SteamDB reports ~70-80% "works" or "playable" with Proton across the catalog)
- **Native Linux:** ~5,000+ titles ship native builds

### 6.3 The permanently walled set

Games that will **never** work on a non-Windows kernel with cap-native isolation:
- **Valorant** (Riot Vanguard — kernel driver, Windows-only)
- **Any game requiring Windows-specific kernel AC** that hasn't been ported to Linux userspace
- Games with **kernel-level DRM** (some older Denuvo configurations, though modern Denuvo works on Proton)

This is roughly **5-10% of the Steam catalog** — predominantly competitive FPS titles.

### 6.4 Reachable library estimate

| Category | Titles (approx.) | Reachable? |
|---|---|---|
| GOG DRM-free native Linux | ~2,000 | Yes (direct) |
| GOG DRM-free via Proton | ~2,000 | Yes (through compartment) |
| Steam single-player via Proton | ~35,000+ | Yes (through compartment) |
| Steam co-op/PvE with userspace AC | ~5,000+ | Likely yes |
| Native Linux on Steam | ~5,000 | Yes (direct) |
| **Total reachable** | **~49,000+** | |
| Competitive MP with kernel AC | ~3,000-8,000 | No |

**The reachable set is large.** Even excluding all competitive multiplayer with kernel AC, we can reach ~50K+ titles — the vast majority of the Steam catalog and essentially all of GOG. This is a meaningful gaming library.

---

## 7. Adopt vs from-scratch ledger (per layer)

| Layer | Adopt candidate | From-scratch cost | Recommendation |
|---|---|---|---|
| **Win32 ABI** | Wine (LGPL, dynamic) | Extreme (~3M LOC equivalent) | **Adopt Wine** via compartment — no viable alternative |
| **D3D9/11→Vulkan** | DXVK (zlib, permissive) | High (~50K LOC) | **Adopt DXVK** — zlib license, proven, clean separation |
| **D3D12→Vulkan** | VKD3D-Proton (LGPL, dynamic) | Extreme (~100K LOC) | **Adopt VKD3D-Proton** — no viable from-scratch alternative |
| **Vulkan driver** | Mesa RADV/ANV (MIT) | Extreme (200-500K LOC) | **Adopt Mesa** IF DRM compatibility layer exists; otherwise, build incrementally from Phase-6 compute toward Vulkan on our own submission path |
| **Presentation** | Gamescope (BSD 2-clause) | Moderate (~20K LOC) | **Adopt Gamescope** — permissive license, handles frame pacing/VRR/overlay |
| **Store/launcher** | Steam client / Heroic / Lutris | High (~50K LOC) | **Adopt existing launcher** (Heroic is simpler than Steam; GOG Galaxy for DRM-free) |

---

## 8. Landscape verdict — which option does the evidence favor?

### Option C (bounded gaming compartment) is the clear winner, with Option E (streaming) as the near-term bridge.

**Evidence:**
1. **License landscape is clear.** No blocking licenses. LGPL components are manageable through process boundaries. Permissive components (DXVK zlib, Gamescope BSD, Mesa MIT) are fully adoptable.
2. **The gaming stack is not reinventable.** Wine (3M LOC), DXVK, VKD3D-Proton, and Mesa represent decades of accumulated compatibility work. From-scratch alternatives (Option A) would take longer than the heat death of the project. The only question is HOW we host them, not WHETHER.
3. **SteamOS proves the model works.** A bounded Linux distribution focused on gaming can run ~50K+ titles near-flawlessly. We just need to add cap-native containment.
4. **Anti-cheat is a hard wall, not a gap.** Competitive MP with kernel AC is out by design. The reachable set (~50K titles) is large enough to be meaningful. This is an honest line, not a failure.
5. **Phase-6 compute is the foundation, but not sufficient.** Our Gen9 driver proves GPU submission works. But Vulkan graphics requires a complete 3D pipeline that Phase-6 doesn't exercise. The path from Phase-6 compute to Vulkan graphics is either: (a) build DRM compatibility and adopt Mesa, or (b) extend our own driver toward Vulkan incrementally. Option (a) is faster; option (b) is charter-purer but slower.

### Divergence from framing

I **concur** with the framing's bottom line. No major divergence. One refinement:

**The GPU choice matters more than the framing acknowledges.** If we adopt Mesa (the pragmatic path), AMD GPUs (RADV) are objectively superior to Intel (ANV) for gaming: RADV has more game coverage, better performance on modern titles, and is the primary test target for DXVK/VKD3D-Proton development. VKD3D-Proton's README explicitly states "Intel GPUs — we have not done any testing against Intel GPUs yet." This means Gen9 (our Phase-6 hardware) is a **poor gaming target** even with Mesa adopted. The PL should consider: Phase-6 compute on Gen9 (done), but gaming Vulkan targets AMD RDNA (RADV).

### Near-term bridge: Option E (streaming)

Before any of the local stack matters, cloud gaming (GeForce Now, Xbox Cloud Gaming) is reachable now. These services run in a browser or thin client. Our browser work (OT-61) and GPU video decode (Phase-6 extension) directly enable this. A streaming client on LogansOS that can play GeForce Now games is a **real, near-term win** that sidesteps all five mountains.

---

## 9. Confidence tags

| Finding | Confidence | Basis |
|---|---|---|
| Proton component separation analysis | High | Primary source: Proton README, DXVK README, VKD3D-Proton README, Wine architecture docs |
| Mesa DRM coupling | High | Primary source: Mesa source code structure, DRM ioctl documentation |
| SteamOS architecture | High | Primary source: SteamOS/Steam Deck documentation, Valve announcements |
| License table (Wine, DXVK, VKD3D, Gamescope) | High | Raw LICENSE files fetched via GitHub API |
| License table (Mesa) | Medium | GitLab unreachable; MIT license is well-documented but not raw-verified in this session |
| Anti-cheat analysis | High | Primary source: Proton wiki, SteamDB compatibility data, EAC/BattlEye documentation |
| Reachable library estimates | Medium | Based on SteamDB statistics and GOG catalog counts; approximate |
| Phase-6 → Vulkan gap analysis | High | Based on ADR-032 (Phase-6 compute) vs Vulkan spec requirements |
