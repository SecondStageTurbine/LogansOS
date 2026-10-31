<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH: Gaming facet 3 -- substrate, compatibility, and the compartment

**Date:** 2026-07-21

**Author:** Jarvis (Codex; gaming panel facet 3)

**Status:** FINDINGS -- design research, no gate

**Scope:** the bounded gaming compartment, Proton on a non-POSIX host, the
cap-brokered game bundle, Steam/GOG and DRM, anti-cheat, and the latency
substrate

**Method:** semantic discovery used the repository RAG with the five dispatched
capability/input/audio/process/compositor queries, followed by direct reads of
the returned source and ratified ADRs. Compatibility findings use an audited
current Wine source snapshot and upstream project/partner documentation; token
search was used only to locate coupling, not as a substitute for reading the
relevant implementation.

## 0. Verdict

**Choose Option C, implemented as a capability-confined deployment of the
ratified ADR-023 Linux personality. Do not build a second Linux kernel, grant a
GPU to a guest, or port Win32 directly onto `libneo`.** **[High confidence.]**

The viable architecture is a **cap-virtualized Linux gaming compartment**:

- Windows game code runs directly on x86-64 through real Wine/Proton.
- Mesa and Proton see the Linux ABIs they already require, but those ABIs are
  personality translations over native kernel objects and native brokers.
- G1 remains the only display owner. The compartment receives a virtual render
  node and a virtual display endpoint, never KMS master, PCI BARs, or scanout.
- Audio, focused input, controller, network, storage, update, and cloud-save
  access are separate, attenuated capabilities. They are not implied by being a
  Linux process or by possessing a path name.
- A native supervisor can revoke the compartment root, terminate its process
  group, drain broker queues, destroy its G1 surface, and close its sockets.

This is container-like in **authority, lifecycle, and performance**, but it is
not a Linux namespace container and not a microVM. The CPU executes game code
directly; high-volume GPU, audio, input, and presentation traffic uses bounded
shared-memory queues and fences rather than a syscall or IPC round trip per
draw, sample, or event. Near-native performance remains a measurement gate, not
an architectural entitlement. **[High confidence on the design direction;
medium confidence that the eventual implementation will meet the proposed
performance gates.]**

Three boundaries must remain explicit:

1. **The personality is not implemented today.** ADR-023 is ratified design;
   current `sys_spawn` loads Limine modules, there is no disk-backed dynamic
   `exec`, and the current syscall/threading surface is far below Wine's closure.
2. **A kernel-resident personality enlarges the monolithic kernel TCB.** Object
   capabilities constrain the authority reachable by a correctly executing
   personality. They do not make a memory-safety or logic flaw in kernel-resident
   personality code harmless to the kernel. A microVM would give a stronger
   code-fault boundary.
3. **Modern Steam's Linux launch closure conflicts with one ADR-023 exclusion.**
   Valve's current Steam Runtime uses Linux namespace/container technology,
   while ADR-023 excludes namespaces. Real Proton can remain largely unmodified;
   the complete Steam launch stack cannot. It needs either a deliberately narrow
   personality extension or, preferably, an Altimit-owned runtime adapter that
   projects the sealed runtime and bundle without reintroducing general Linux
   namespaces.

The first credible local-game target is a **64-bit, DRM-free, offline Windows
game from GOG**, launched without Galaxy, through Wine/Proton and the same
brokers intended for Steam. Steam comes after the projected-runtime and updater
seams are proven. Kernel anti-cheat is out of scope by design. **[High
confidence.]**

## 1. Evidence and correction of the framing

The framing correctly identifies the five coupled mountains and Option C as the
leading direction. The repository already contains a more concrete load-bearing
decision: `docs/ADR-023-linux-personality-layer.md` ratifies a per-process Linux
personality whose syscalls translate into native kernel objects. It already
names Proton as a transitive beneficiary and rejects a VM as the primary
compatibility route. This finding therefore refines, rather than invents, the
compatibility strategy.

### 1.1 Current substrate that can be reused

| Existing seam | Current useful property | Gaming use | Missing work |
|---|---|---|---|
| Capability table and derivation | Handles carry object, rights, identity, and derivation; explicit derive/grant/revoke | Construct a least-authority graph and revoke a session | A group-lifecycle/revocation receipt if repeated bundle teardown proves awkward |
| `sys_spawn` | Child receives only explicitly inherited handles | Correct default-deny launch invariant | Disk-backed `exec`, dynamic ELF, TLS, COW fork, Linux thread/process semantics |
| `NetDialCap` | Immutable exact host/port attenuation; connected socket becomes the handle | Defense-in-depth for fixed game endpoints | A policy broker for discovery, DNS, CDNs, Steam endpoints, UDP, listening, and dynamic authorization |
| File rights | Separate read/write/lookup/readdir/execute and reserved mmap/mount bits | Split immutable content, updates, saves, cache, and mods | Remove the temporary root-namespace `open`; projected roots, overlay semantics, mmap and durable transactions |
| G1 `SurfaceCap` and `SharedPixelBuffer` | Client-write/compositor-read mappings and explicit surface protocol | Final composition and lifecycle | Imported GPU images, swapchains, explicit fences, real vblank/flip timing, high-rate composition |
| G1 `InputFocus` | Kernel routes input only to the active focus leaf; compositor cannot consume it | Focused keyboard/pointer delivery | Linux input projection, generic HID/controller, hotplug, output/rumble, timestamped queues |
| `GpuSubmitCap` | Scarce raw-present action can be gated | A useful denial boundary | It is a present cap, not Vulkan, Mesa, a render node, or a general GPU queue |
| USB HID | Real boot-protocol keyboard and mouse with bounded queues and I-14 DMA fencing | Initial focused input | Report-descriptor HID, multi-interface devices, gamepads, analog controls, output reports, wireless |
| Scheduler | Per-CPU runnable queues and ordinary time slicing | Basic process execution | User threads, affinity, QoS, bounded real-time audio, game-loop latency controls, pressure accounting |
| Audio | OT-46 design direction and DMA-fencing principles | Architectural starting point | A real HDA/USB Audio driver and low-latency broker/service do not yet exist |

The current `GpuSubmitCap` name must not lead to a false equivalence. It gates a
present/flip seam. A gaming graphics device still needs contexts, per-process
address spaces, buffers/images, command queues, validation, fences, fault
attribution, reset, quotas, and a Vulkan-capable userspace driver.

### 1.2 Divergences from the framing

1. **"GPU passthrough" is the wrong model.** The compartment should see a
   cap-virtualized DRM render node whose ioctls lower to native GPU objects. Raw
   PCI/MMIO, KMS, physical scanout, and a whole assigned GPU remain forbidden.
2. **"Contained Linux" is an ABI/personality island, not a Linux kernel.** One
   kernel and one native resource graph remain in charge.
3. **SteamOS's immutable base is not the isolation seam.** Immutability reduces
   system drift. It does not by itself prevent a store client or game from
   reaching user files, processes, devices, or network authority available in
   its session. The relevant distinction is per-bundle object authority and
   revocation.
4. **An unmodified Proton is plausible; an unmodified Steam launch closure is
   not.** The pressure-vessel/namespace conflict is a concrete integration item,
   not a future detail.
5. **Capability confinement is not kernel fault containment.** Calling the
   kernel-resident personality "isolated" without this qualifier overstates the
   boundary.

## 2. The gaming compartment

### 2.1 Trust and authority graph

```text
native policy / user consent
          |
          v
  game-manager (trusted) ---- signed GameBundle manifest + hashes
          |
          +---- CompartmentRoot / process-group kill and revocation
          |          |
          |          +---- store/update principal
          |          |       depot staging + transactional install + update net
          |          |
          |          +---- game principal
          |          |       content RX; private prefix/save/cache RW; game net
          |          |
          |          +---- bundle helpers
          |                  wineserver/runtime/overlay, bundle-local IPC only
          |
          +---- native brokers (not compartment children)
                     GPU | G1 display | audio | input | network | storage/cloud
```

The store and game are **sibling principals**, not one ambient Linux login. The
store can stage and transactionally replace verified content but cannot inspect
the running game's private memory. The game can read/execute its verified
content and write only its prefix/save/cache roots; it cannot mutate its runtime
or installer. `wineserver` and launch helpers are bundle-local peers. An overlay
is a separately granted peer-inspection/injection privilege, disabled by
default, not an incidental consequence of sharing a UID.

The native `game-manager` is the only component allowed to:

- verify the manifest, content hashes, compatibility runtime, and policy;
- derive the requested capabilities from user/session roots;
- instantiate the projected Linux root and synthetic device namespace;
- start, suspend, resume, and kill the complete compartment process group;
- collect an auditable grant/revocation receipt; and
- commit a save transaction after orderly or forced shutdown.

It does **not** parse game network protocols, execute Wine code, compile shaders,
or sit on per-frame data paths.

### 2.2 The broker seam

| Resource | Compartment-visible contract | Native authority retained | Structurally unavailable |
|---|---|---|---|
| GPU | A Linux DRM-render-node-shaped endpoint; Mesa ioctls create scoped contexts, buffers, queues, and fences | GPU broker validates submissions, owns IOMMU mappings, quotas, reset, fault attribution, and import/export | PCI config/BAR/MMIO, arbitrary DMA, KMS master, physical scanout, another client's context |
| Display | A local Wayland/X11-compatible endpoint, preferably Gamescope-shaped; submitted images carry explicit acquire/release fences | G1 owns surfaces, z-order, focus, composition, policy, and scanout | Framebuffer console, direct scanout without an explicit lease, capture of other surfaces |
| Audio | PulseAudio/PipeWire-compatible endpoint backed by shared PCM rings, timestamps, and bounded wake primitives | Native audio broker owns mixing, device DMA, format policy, and xrun accounting | HDA/USB device registers, arbitrary capture; microphone absent unless separately consented |
| Keyboard/pointer | Synthetic evdev endpoints fed only by the active `InputFocus` | Kernel/native input broker owns physical devices, focus, filtering, and timestamp normalization | Unfocused keystrokes, secure-attention input, physical USB endpoint |
| Controller | Synthetic evdev/hidraw-compatible device with input queue; separate output/rumble right | Input broker parses HID, maps controls, rate-limits output, owns hotplug | Other devices, arbitrary USB control transfers, firmware update |
| Network | Bundle profile endpoint for TCP/UDP/DNS and connected sockets | Network broker authorizes destination class, LAN/listen/broadcast, quotas, and revocation; kernel caps remain defense in depth | Raw packets, promiscuous mode, ambient LAN, unrelated listeners, host network configuration |
| Content/runtime | Sealed, hash-addressed, read/execute projections | Storage broker owns objects and verification | Native root, arbitrary path traversal, mutation by the game |
| Save/cache/prefix | Separate bounded writable roots; save commits are transactional | Broker enforces quota, durability, versions, and optional cloud sync | Other games' data, user documents, store credentials |
| Time/system view | Monotonic/realtime clocks and sanitized `/proc`, `/sys`, `/dev` projections | Personality/brokers synthesize only required facts | Native PID topology, kernel addresses/logs, arbitrary device inventory |

This is **control-plane virtualization with direct bounded data planes**. Policy
changes, allocation, device creation, and teardown cross broker IPC. Mapped GPU
queues, fences, audio rings, and input rings carry the hot path. There must be no
broker message per GPU draw or audio sample and no mandatory full-frame copy.

The render-node ABI is an adapter, not raw passthrough. Mesa may believe it is
opening `/dev/dri/renderD128`; the personality maps that fd and its ioctl set to
`GpuContextCap`, `GpuBufferCap`, `GpuQueueCap`, and `GpuFenceCap`-like native
objects. G1 receives importable images from that graph and decides whether to
composite or grant a revocable fullscreen direct-scanout lease. Even the latter
never transfers KMS or global display authority.

### 2.3 Namespace illusion without ambient authority

The Linux view is a projection assembled from handles:

- `/usr`, the Steam runtime, Proton, and game content are immutable image trees;
- the Wine prefix, shader cache, saves, and temporary files are distinct writable
  mounts with quotas;
- `/proc` exposes only compartment PIDs and sanitized process information;
- `/sys` exposes only compatibility facts required by userspace;
- `/dev/dri`, `/dev/input`, audio sockets, and other nodes are broker adapters,
  not physical-device handles; and
- path lookup begins at the projected root capability, never the current native
  root-namespace stopgap.

The word "mount" here describes the Linux ABI view. The game is never given the
native `MOUNT` authority. Construction is performed by the trusted manager or
storage broker before launch.

### 2.4 Lifecycle and kill switches

The kill path is ordered and bounded:

1. stop new broker allocations and network dials;
2. stop/suspend the compartment process group;
3. revoke the session derivation root and all child caps;
4. cancel/drain GPU and audio work with bounded waits, resetting only the guilty
   GPU context when possible;
5. destroy the G1 surface and focus leaf, close sockets, and remove projections;
6. commit only a valid save transaction; discard update staging and ephemeral
   cache state after a forced kill; and
7. emit the manifest/hash, granted-authority, fault, and teardown receipt.

Independent switches are required:

- build-time personality support;
- boot/deployment enablement;
- per-user launch consent;
- per-session root revocation;
- GPU-acceleration quarantine with a diagnostic software fallback;
- network, microphone, mods, overlay, peer-inspection, and controller-output
  toggles; and
- an all-gaming-compartments emergency disable that leaves native G1 and the
  cap-native desktop intact.

## 3. Why not a VM

"VMs are slow" is not a sufficient argument. Hardware-virtualized CPU execution
can be close to native, and a microVM supplies a stronger fault boundary for a
large Linux syscall implementation. The trade is the device and integration
path:

| Property | Personality compartment | Linux microVM |
|---|---|---|
| CPU | Direct native scheduling and address spaces | Hardware virtualization can be near-native but adds a second scheduler/kernel |
| GPU | Native broker objects and shared queues; G1 keeps display ownership | Full passthrough monopolizes a consumer GPU and bypasses G1; paravirtual graphics adds another ABI, synchronization layer, and feature-lag risk |
| Audio/input/display | Native focus, policy, and shared-memory broker seams | Virtio/emulated devices plus host/guest translation; integration still needs brokers |
| Resources | One kernel, one page cache/resource graph | Duplicate kernel, page cache, memory accounting, boot/runtime image, and failure lifecycle |
| Cap-native AI and services | Native caps can be selectively granted | Guest RPC re-exports services and can obscure native object provenance |
| Code-fault containment | Personality code remains kernel TCB | Guest kernel faults are normally contained by the VMM boundary |

Full GPU assignment is particularly mismatched. Typical consumer GPUs do not
offer a useful SR-IOV partition; reset and IOMMU safety are device-specific; the
guest wants KMS ownership; and the assigned device cannot simultaneously be the
native G1 display device. A virtual GPU avoids assignment but recreates much of
the render-node translation while retaining the second kernel.

Therefore the personality compartment is the leading local path. Retain a
microVM as a **fallback security architecture** if the audited Linux personality
cannot be made acceptably small and memory-safe, or for deliberately hostile
legacy workloads that do not need the primary GPU. This is a real trade, not a
claim that capabilities substitute for a hypervisor boundary. **[High
confidence.]**

## 4. Proton on a non-POSIX OS

### 4.1 Upstream coupling result

[Wine describes itself](https://www.winehq.org/?page=documentation) as a
compatibility layer that translates Windows API calls into POSIX calls. Proton
is not a self-contained NT kernel: [Valve's Proton tree](https://github.com/ValveSoftware/Proton)
builds on Wine and a Linux SDK/runtime, produces both 32- and 64-bit Wine
components, and integrates Vulkan and Linux synchronization facilities.

I sparse-cloned current Wine source at commit
`7364e9021229f000670b42acfd8e2fb2d6c00cd7` (2026-07-20) and audited
`dlls/ntdll/unix`, `server`, `loader`, `winebus`, and the PulseAudio/ALSA paths.
The first three scopes alone contain **68 C files / 93,032 measured lines**:

| Scope | C files | Measured lines | Representative host coupling observed |
|---|---:|---:|---|
| `dlls/ntdll/unix` | 23 | 49,097 | pthread creation/TLS, signals, mmap/mprotect, dynamic loading, `/proc`, clocks, futex/ntsync, fork/exec |
| `server` | 45 | 43,935 | fd/object server, Unix sockets, poll/epoll, timers, processes, synchronization, `/dev/ntsync` |

This count is not a syscall count or an implementation estimate. It establishes
that POSIX coupling is architectural and distributed, not a narrow host shim.
Additional inspected evidence includes Wine's PulseAudio driver using pthreads,
condition variables, `poll`, and PulseAudio APIs, and `winebus` using udev,
hidraw descriptors, polling, input classification, and HID output/feature
reports.

### 4.2 Minimum compatibility closure

The ADR-023 Tier 1/2/3 inventory is directionally correct. A credible Proton
spike needs at least these interacting families:

| Family | Required semantics, not merely names | Current gap |
|---|---|---|
| ELF/runtime | disk-backed `execve`, PT_INTERP dynamic linker, relocations, TLS, `dlopen`, environment/auxv, file-backed executable mappings | Current native spawn loads named Limine modules; dynamic ELF is unsupported |
| Processes | fork with COW, vfork/exec ordering, wait, process groups/sessions, credentials, `/proc` view | No Linux process model |
| Threads | `clone`, TLS and TID lifecycle, robust futex lists, signals per thread, affinity, priorities | No public native user-thread creation API or Linux thread semantics |
| Synchronization | futex wait/wake/requeue/PI-adjacent behavior, eventfd, ntsync or a correct fallback, clocks/timeouts | Native channel/blocking primitives are not ABI substitutes |
| Signals | masks, handlers, alternate stack, synchronous faults, restart behavior, signalfd | Missing Linux signal model |
| Memory | mmap/munmap/mprotect/madvise, shared/file mappings, brk, COW, large sparse address spaces, executable-policy transitions | Partial native memory surface, not Linux semantics |
| FDs/I/O | dup, pipes, Unix sockets, ioctl/fcntl, poll/select/epoll, tty, timerfd/inotify | Native handles require a Linux fd/object translation layer |
| Filesystem | Linux path and metadata semantics, symlinks, case behavior, xattrs, locks, `/proc`/`sys`/`dev`, overlay/prefix | Projected compartment root and compatibility metadata absent |
| Network | BSD sockets including UDP, DNS, nonblocking connect, readiness, options | Existing exact TCP dial cap is useful enforcement but not the application ABI |
| Graphics | Vulkan loader, Mesa/libdrm render-node ABI, shader cache, dma-buf-like import/export and explicit sync | No Vulkan graphics device or DRM personality today |
| Audio/input | PulseAudio/PipeWire socket semantics; udev/evdev/hidraw/controller and hotplug projection | Boot keyboard/mouse only; no audio |
| Multiarch | 32-bit executable/runtime support or a validated new-WoW64 arrangement | Kernel/userland are x86-64-only |

The 32-bit issue is a catalog boundary, not cleanup. A first proof may select a
known 64-bit game, but a general Proton claim requires either i386 personality
support or evidence that the pinned Wine/Proton new-WoW64 configuration covers
the target library and anti-tamper closure without a 32-bit Unix host process.

### 4.3 Adopt versus reimplement

The honest decision is:

- **Adopt real Wine/Proton, DXVK/VKD3D-Proton, Mesa, and the required runtime in
  the compartment.** Keep patches at explicit platform adapters: personality
  ABI gaps, render node, display, audio, input, and runtime projection.
- **Do not reimplement Win32 on `libneo`.** Doing so assumes responsibility for
  Wine's distributed loader, process/thread, NT object, registry, COM, graphics,
  input, audio, and application-quirk surface while still needing Vulkan and
  game-specific validation. It is a permanent divergent platform, not a thin
  translation library.
- **Do not describe Proton as fully unmodified.** The game-facing Wine/DXVK
  binaries should remain close to upstream, but host adapters and the Steam
  runtime launcher necessarily differ.

Valve documents that the [modern Steam Runtime uses Linux namespace/container
technology](https://github.com/ValveSoftware/steam-runtime). ADR-023 deliberately
excludes namespaces. The preferred resolution is a **projected-runtime adapter**:
the native manager prepares an immutable runtime/game root, writable prefix and
synthetic devices, then launches the runtime with that already-resolved view.
It implements the outcome pressure-vessel needs without exposing general
namespace creation, arbitrary bind mounts, or a Linux container control plane to
the game. If source audit proves a small namespace subset unavoidable, that is
an ADR amendment and a new security gate, not a silent compatibility syscall.

## 5. The cap-brokered `GameBundle`

`GameBundle` should initially be a **sealed launch manifest plus a verified cap
derivation graph**, not a giant kernel object that aggregates every authority.
The kernel should continue enforcing small typed objects. The manager interprets
the signed policy and emits an auditable receipt.

Suggested manifest fields:

```text
bundle_id, version, publisher, content_hashes
runtime_id/hash, proton_version, mesa_driver/profile
entrypoint, arguments, environment allowlist, architecture
principal graph: store | game | wineserver | optional overlay
immutable roots; prefix/save/cache/temp/mod roots and quotas
GPU profile and memory/queue limits; display/fullscreen policy
audio out; optional mic; keyboard/pointer; controller and rumble
network profile: offline | service set | internet, plus LAN/listen policy
DRM/store integration; cloud-save policy; anti-cheat classification
debug/trace/overlay permissions; fault and teardown policy
```

The manifest requests policy; it does not mint authority. The manager may deny
or attenuate each request. Every granted object descends from a session root and
has a principal owner. Store/update and running-game authority must be sibling
branches so that updater compromise does not imply game inspection and game
compromise does not imply installer mutation.

No new universal `GameBundleCap` is necessary for the first implementation. If
atomic teardown across heterogeneous derivation trees cannot be expressed
reliably, add a small **`RevocationSet`/`CompartmentRoot` lifecycle primitive**:
membership permits only group suspend/kill/revoke/account, never use of the
member capabilities. That preserves typed least authority and gives one bounded
kill switch. **[Medium-high confidence; validate against the E1 revocation
implementation before ratification.]**

## 6. Steam, GOG, updates, and DRM

### 6.1 Steam is several principals, not just a launcher

Hosting Steam requires the client UI/runtime, authentication and credentials,
content/depot downloads, install/update transactions, launch, overlay, cloud
saves, and DRM integration. Valve's documentation separates
[depots](https://partner.steamgames.com/doc/store/application/depots),
[build delivery](https://partner.steamgames.com/doc/store/application/builds),
and [Steam Cloud](https://partner.steamgames.com/doc/features/cloud). These map
naturally to separate authorities:

- a credential broker supplies short-lived account/session material to the
  store principal, never to arbitrary games;
- an update worker gets network plus a staging root and transactional replace
  authority, not write access to live verified content;
- the game gets read/execute content plus its own prefix/save/cache roots;
- cloud sync receives a declared save projection after quiesce/exit rather than
  ambient access to the prefix; and
- rollback retains the last verified content generation.

Valve notes that the Steam Linux overlay uses `LD_PRELOAD` in its
[Linux integration documentation](https://partner.steamgames.com/doc/store/application/platforms/linux?l=english).
That is process injection by design. It must be an explicit bundle-local
peer-inspection cap, disabled in the first milestone and impossible across
bundle or native-principal boundaries.

[Steam DRM](https://partner.steamgames.com/doc/features/drm) verifies ownership
and normally expects Steam to launch the title, but Valve characterizes the
wrapper as protection against casual copying rather than strong anti-piracy.
The compartment can support this only after the client/IPC/launch contract is
present. Third-party DRM and anti-tamper are separate compatibility dependencies
and may remain unsupported even when the Steam wrapper works.

### 6.2 GOG is the better first acquisition path

GOG's documentation states that the client is optional and games must remain
usable without authentication or a client; it provides offline installers and a
[DRM-free model](https://docs.gog.com/introduction/). That makes a verified
offline installer/content import the narrowest real Windows-game proof.

[GOG Galaxy is offered for Windows and macOS](https://www.gog.com/galaxy), not
Linux, and GOG's [SDK DLC-discovery documentation](https://docs.gog.com/sdk-dlc-discovery/)
explicitly marks that path unsupported on Linux. Do not make Galaxy the
compatibility target. Import an offline game, seal its content, launch it with
no network by default, and add optional GOG-service/cloud adapters later where
the title and available SDK support them.

## 7. Anti-cheat and integrity

### 7.1 Compatibility verdict

Valve's official [Proton/Steam Deck guidance](https://partner.steamgames.com/doc/steamhardware/proton)
says Easy Anti-Cheat and BattlEye Proton support depends on title/vendor opt-in
and integration, and that kernel-space anti-cheat is unsupported and not
recommended. The resulting policy is firm:

- user-space anti-cheat may run inside the game principal if the studio enables
  and validates its Proton path;
- kernel drivers, physical-memory inspection, cross-principal process access,
  arbitrary debug attachment, raw input-device inspection, or a global kernel
  hook are never granted; and
- unsupported anti-cheat fails closed with an explicit catalog reason. The OS
  must not spoof Windows support, weaken the core, or advertise a title based on
  reaching its menu.

Competitive multiplayer that requires kernel anti-cheat is therefore **out of
scope by design**. This is not a backlog item hidden behind the compatibility
program. **[High confidence.]**

### 7.2 A future cap-native studio contract

A studio could explicitly target a different integrity model:

1. measured boot attests the kernel, personality, broker, and policy versions;
2. verified content hashes cover the game, Proton/runtime, Mesa/shaders where
   stable, and permitted mods;
3. the manager signs a per-session receipt containing the exact capability
   manifest, debug/overlay posture, IOMMU state, and fault/reset history;
4. a native integrity broker answers a narrow challenge without exposing other
   principals or general machine telemetry; and
5. the game server decides whether that measurement profile is acceptable.

This is a promising cap-native primitive because the proof says what the game
**could reach**, not merely what processes happened to be running. It is not a
drop-in implementation of EAC/BattlEye, requires studio/server integration, and
cannot prove the absence of external capture devices, USB automation, hostile
firmware, or analog side channels. Do not claim universal cheat prevention.
**[Medium confidence as a future studio-facing design; high confidence that it
does not unlock today's kernel-AC catalog.]**

## 8. Latency substrate gaps

### 8.1 Low-latency audio

[PipeWire's scheduling model](https://pipewire.pages.freedesktop.org/pipewire/page_scheduling.html)
uses a real-time data-loop thread, shared memory for activation/buffers, eventfd
wakeups, and a negotiated quantum. Its
[real-time module](https://pipewire.pages.freedesktop.org/pipewire/page_module_rt.html)
also exposes priority and a CPU-time limit. The equivalent native service needs:

- HDA and/or USB Audio playback with dedicated, IOMMU-fenced DMA arenas;
- a broker-owned mixer with per-stream shared PCM rings, monotonic period
  timestamps, bounded wakeups, back-pressure, underrun/overrun counters, and
  fault isolation;
- format negotiation, channel layouts, volume, mixing and bounded resampling;
- a scheduler class that permits bounded real-time audio work while enforcing a
  CPU-time budget so a client cannot starve the system;
- separate output and capture capabilities, with microphone absent by default;
  and
- a PulseAudio/PipeWire-compatible compartment endpoint so Wine's upstream
  audio driver does not need an Altimit-specific game-facing ABI.

Provisional acceptance gates: 48 kHz stereo at 128- and 256-frame quanta;
measured output latency at or below 20 ms; zero xruns in a 30-minute mixed
CPU/GPU stress run; revocation that silences and frees the stream without
restarting the device. These are proposed measurement gates, not current
results. **[Medium confidence.]**

### 8.2 Controller/gamepad

The current boot-protocol keyboard/mouse work establishes bounded USB queues,
DMA fencing, and the focus route, but a gamepad is not a larger mouse. Wine's
current `winebus` path consumes udev/hidraw discovery, HID report descriptors,
hotplug, polled input, and output/feature reports. The Linux
[gamepad protocol documentation](https://docs.kernel.org/input/gamepad.html)
also defines buttons, absolute axes, and force-feedback/rumble conventions.

Required substrate:

- generic HID report-descriptor parsing and multi-interface device claim;
- axes, hats, analog triggers, buttons, dead-zone/calibration, stable IDs,
  hotplug, and monotonic timestamps;
- a normalized native controller object plus synthetic evdev/hidraw projections
  for Wine/SDL;
- an independently revocable, rate-limited controller-output cap for rumble,
  LEDs, and permitted feature reports;
- common controller mappings and test vectors for Xbox, DualShock/DualSense,
  and generic HID pads; and
- Bluetooth/wireless as a later acquisition path, not an assumption in the
  first wired-USB gate.

Provisional gate: p99 focus-to-compartment event availability no more than 2 ms
above a native focused client under storage and GPU load, with no unfocused
event delivery and no stuck rumble after revoke. **[Medium confidence.]**

### 8.3 Frame pacing, graphics, scheduling, and memory

G1's current bounded present queue and explicit focus semantics are useful, but
synthetic vblank and a software framebuffer path do not establish game-grade
presentation. The design needs:

- real vblank/flip timestamps and completion interrupts;
- Vulkan-class swapchain/image import and explicit acquire/release fences;
- FIFO and mailbox-style scheduling, bounded back-pressure, dropped/late-frame
  counters, and stable clock correlation with input/audio;
- composition without a mandatory full-frame CPU copy;
- a revocable fullscreen direct-scanout lease that still leaves G1 as policy and
  recovery owner; and
- variable-refresh-rate support after fixed-refresh pacing is measured.

[Gamescope](https://github.com/ValveSoftware/gamescope) is useful prior art: it
contains games in an Xwayland/Wayland-facing compositor, can accept frames
without a copy, and uses Vulkan/DRM for final presentation. Altimit should copy
the **shape** of the compartment-facing display contract, not transfer DRM/KMS
ownership out of G1.

Scheduler and VM work is equally load-bearing: real user threads; fast
futex/ntsync; affinity and priority; bounded audio real-time budgets; high
resolution timers; large sparse/file-backed mappings; shader-cache I/O; pinned
GPU memory quotas; pressure notification; per-bundle accounting; and an OOM
policy that kills the guilty bundle before native desktop services.

Performance gates must compare the same pinned game/Proton/Mesa build with a
same-hardware Linux baseline. Proposed gates are median FPS at least 95% of the
baseline, p99 frame time no more than 10% worse, no broker-introduced full-frame
copy or extra queued frame, and no cross-client GPU/input leakage. Thresholds
are provisional until the first render-node prototype produces measurements.
**[Medium-low confidence on the numbers; high confidence that percentile and
copy-count gates are required.]**

## 9. De-risking DAG and stop gates

Reason in dependency gates, not calendar promises:

1. **Personality closure gate:** run pinned dynamic x86-64 Linux test programs
   exercising loader/TLS, threads, signals, COW fork/exec, futex/epoll, Unix and
   Internet sockets, file mappings, and projected `/proc` with no native root
   authority.
2. **Wine closure gate:** boot a sealed 64-bit Wine runtime, start `wineserver`,
   execute a console PE, and prove group kill/revocation. Stop if required
   upstream divergence cannot be isolated to maintained adapters.
3. **GPU acquisition gate:** run a pinned Mesa driver against the virtual render
   node, pass the required Vulkan conformance subset, attribute a malicious/bad
   command stream, and reset one context without killing G1. Stop before Proton
   graphics if this requires raw passthrough or global KMS.
4. **Display gate:** present a Vulkan image through the compartment endpoint into
   a G1 surface with explicit fences, measured copy count, real vblank timing,
   and software diagnostic fallback.
5. **Latency gate:** sustain audio and wired-controller acceptance tests while
   GPU and storage are loaded; prove focus, microphone, rumble, and network
   revocation independently.
6. **First real-game gate:** launch a pinned 64-bit DRM-free offline GOG title
   with networking disabled, survive save/restart/update rollback, and collect
   frame/audio/input percentiles against Linux.
7. **Steam-runtime gate:** run the Steam client/update principal through the
   projected-runtime adapter, then prove Steam DRM, overlay-off launch, cloud
   saves, and transactional depots without general Linux namespaces.
8. **Catalog gate:** publish per-title results with architecture, DRM,
   anti-cheat, graphics, controller, and performance discriminators. A title is
   supported only through repeatable gameplay, save/reload, and teardown, not a
   successful launch screen.

These gates share the GPU and G1 program with glass/browser work, but gaming
must not claim those programs' progress as Vulkan conformance or game latency.

## 10. Options A--E

| Option | Substrate finding | Confidence | Recommendation |
|---|---|---|---|
| A -- native Win32/graphics/store replacement | Recreates Wine, Vulkan/Mesa, and store ecosystems while still needing all latency work | High | Reject as the compatibility route; retain only native APIs for future games |
| B -- broad Linux personality | Ratified ADR-023 supplies the right ABI direction, but an unbounded shared Linux environment would recreate ambient authority | High | Use only as the mechanism inside C, never as a machine-wide compatibility mode |
| C -- bounded gaming compartment | Best reuse/performance/security trade if render/audio/input data planes are cap-virtualized and the TCB caveat is accepted | High | **Primary local-play program** |
| D -- native/portable games | Strong isolation and maintainability, but no existing Windows catalog | High | Parallel ecosystem track and excellent broker conformance workload |
| E -- cloud/streaming | Avoids local Proton, GPU-driver breadth, Steam DRM, and anti-cheat; still needs network, decode, audio, input, and presentation | High | Near-term complement, not evidence that local gaming is solved |

The substrate evidence therefore favors **C + E now, with D as the native
long-tail**. B is an implementation ingredient of C, not a separately acceptable
ambient mode. A should not absorb compatibility-program resources.

## 11. Risks that remain after choosing C

1. **GPU driver acquisition remains rung zero.** The compartment does not make
   a Vulkan/Mesa driver appear; it only gives that work a defensible boundary.
2. **Kernel TCB growth may dominate.** ADR-023's syscall/ioctl surface is large.
   Memory-safe implementation, generated ABI tests, fuzzing, and kill-switches
   are required; a microVM fallback remains legitimate.
3. **Linux behavior is more than syscall numbers.** Loader, fd readiness,
   signals, COW, path metadata, `/proc`, udev, and timing interactions will drive
   title-specific failures.
4. **Steam endpoint authority is dynamic.** Exact host/port caps do not express
   CDNs, matchmaking, relay, peer traffic, or redirects. The broker policy needs
   auditable profiles without becoming unrestricted Internet by convenience.
5. **DRM and anti-tamper are publisher-controlled.** Technical compatibility
   does not imply permission, opt-in, or support.
6. **Latency regressions hide in tails.** Median FPS alone misses audio xruns,
   input stalls, queued frames, shader-cache pauses, and GPU reset behavior.
7. **Writable prefixes are executable state.** Wine prefixes, mods, and shader
   caches require integrity, quota, rollback, and provenance rules; "save-file
   cap" alone is too coarse.

## 12. Bottom line

The clean-slate property to preserve is not the absence of every Linux ABI. It
is the absence of **ambient, unaccountable authority**. ADR-023 can host the real
gaming ecosystem without turning the native desktop into Linux if its gaming
deployment is a bounded personality compartment: direct CPU execution, virtual
render/audio/input/display contracts, typed caps, projected namespaces, split
store/game principals, and one revocable lifecycle root.

That design is materially different from a VM, raw GPU passthrough, a global
POSIX mode, or SteamOS immutability. It also has honest limits: a larger kernel
TCB, a substantial POSIX closure, no present-day Vulkan/audio/gamepad substrate,
Steam-runtime adaptation, 32-bit uncertainty, and no kernel anti-cheat. Prove
those limits in the dependency gates above, beginning with a 64-bit offline
DRM-free game. If the render node or personality cannot remain bounded, stop and
re-evaluate the microVM trade before importing more of the stack.

**Final recommendation: Option C primary; Option E early complement; Option D
parallel ecosystem; kernel-AC titles explicitly unsupported. [High confidence.]**
