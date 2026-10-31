<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Integrated showcase: Arthur as an Atrium compositor client

**Status:** Build contract, implemented with this document. **Date:** 2026-07-21.

## Decision and session flow

Arthur is an ADR-073 `TEXT` client surface. It never owns or writes the physical framebuffer in this session. `neo-init` owns the G1 compositor, its `SurfaceRegistry`, and the Limine GOP framebuffer. Arthur owns a 640x400 XRGB8888 `SharedPixelBuffer`, maps it read/write, rasterizes the greeting and conversation into it, and grants the compositor only `MAP_READ | QUERY | PRESENT`. The compositor imports that sibling, maps role 1 read-only/NX, and presents at `(64,64)` after each accepted commit.

The single boot session is:

1. The kernel mounts the out-of-band NeoFS model volume and starts `neo-init`.
2. `neo-init` brings up G1, creates a session channel, and spawns Arthur.
3. Arthur requests an ADR-073 `TEXT` surface, creates its client-owned buffer, renders `ARTHUR IS READY / ASK ME ANYTHING`, attaches, damages, and commits it. G1 presents the greeting; serial records the seam and an exact 8x16 `A`-cell pixel oracle.
4. Arthur loads the Qwen2.5-0.5B vocabulary and model while the surface, compositor, USB path, and framebuffer remain resident.
5. Slice B only: the compositor activates Arthur's ADR-074 `InputFocus`. Production USB HID reports enter the kernel input pump; only the focused Arthur RECEIVE root obtains the key events. Arthur echoes printable keys into its client mapping and commits them.
6. Enter changes the surface to `THINKING`, then Arthur invokes the existing real model-generation path. The reply is rasterized into the same buffer, committed with an answer serial, and composited by G1.
7. Arthur explicitly destroys the surface and drops the model. `neo-init` waits for the client and the kernel emits the one terminal scenario frame.

No greeting, status, prompt, or answer is rendered through `framebuffer-console`. Serial is diagnostic evidence, not a second display route.

## Build configuration and feature interactions

This is one configuration, not a relay between the old demos. `neo-test-arthur-compositor` brings in Arthur's OT-28 real-brain path plus `atrium-input-routing`, `usb-hid`, and the Atrium runtime. The userspace feature `arthur-compositor` brings in the real-brain boot feature and selects both the G1 server and Arthur client. `arthur-compositor-baremetal` adds the production keyboard loop; Slice A compiles the USB/InputFocus/model combination but intentionally does not inject a key.

| Resident subsystem | Selection | Interaction / guard |
|---|---|---|
| Limine framebuffer + G1 | `atrium-compositor-g1` lineage through the Atrium scenario dependencies | G1 is the only framebuffer owner. Integrated + `framebuffer-console` is a compile error. |
| ADR-073 surface protocol | `arthur-compositor` | Arthur receives a dedicated `SurfaceCap`; buffer authority stays client RW / compositor RO-NX. |
| USB HID + ADR-074 InputFocus | `usb-hid`, `atrium-input-routing` | ACTIVATE and RECEIVE roots remain split. A denied RECEIVE call through the activator is an in-session no-snoop oracle. |
| Qwen model + vocab | `arthur-real-brain-boot` | Existing `/neofs/model-qwen2.5-0.5b.neom` and sidecar loader; no new inference implementation. |
| Bare-metal evidence | `neo-test-bare-metal-mode`, NVMe log sink | Slice B uses SATA NeoFS for the model and the independent NVMe sink for durable breadcrumbs. |

This feature combination had no prior integrated scenario. The collision risks are framebuffer ownership, feature-selected duplicate scenario constants/drivers, the former console-interactive loop, and simultaneous heap pressure. The implementation makes the conflicting console route a compile-time error and excludes the integrated features from the standalone scenario selectors. There is one terminal frame and one selected userspace driver.

## Render and input seam

Arthur's surface creation follows the ADR-073 request/reply protocol: `CREATE(TEXT)` on the session channel, receive a dedicated surface endpoint, `IMPORT_ATTACH_BUFFER`, `COMMIT`, `FOCUS_REQUEST`, repeated `COMMIT`, then `DESTROY`. Each commit publishes a full-surface damage rectangle in this first integration; bounded cell-only damage is a later optimization, not needed to prove ownership or composition. The client serial monotonically advances, with a reserved final-answer serial as a falsifiable end-to-end marker.

The raster is deliberately local and deterministic: an uppercase-folded 5x7 font doubled vertically into 8x16 cells, opaque background, 80 columns by 25 rows. Newline wraps, backspace clears one cell, and the oldest row scrolls upward when the cursor reaches the bottom. G1 never mutates client pixels. Slice A checks all 128 pixels in the first `A` cell after presentation; a serial-only greeting or raw framebuffer write cannot satisfy that oracle.

ADR-074 routing is equally explicit. The compositor receives only an ACTIVATE sibling; Arthur retains RECEIVE. Activation changes the kernel focus root, after which HID key events wake Arthur. Arthur decodes pressed keyboard events into the same ASCII fixture as the legacy HID path, writes the character, and commits. The compositor cannot snoop the event bytes, and Arthur cannot activate itself.

## Scanout authority and the Slice-A AAG respin

`PRESENT` on a `BufferCap` is object authority, not sufficient raw-scanout authority. When the integrated showcase arms enforcement, `SYS_FB_PRESENT` and the kernel-internal `SharedPixelBuffer::present` additionally require the process to hold a usable `GpuSubmitCap` on **both** backends. A Software framebuffer blit and a HardwareFlip submit both reach physical scanout, so neither is exempt. Refusal is `EPERM` before an epoch is issued, a commit is consumed, the framebuffer is touched, or a flip is armed.

The kernel mints the sole showcase scanout root into trusted `neo-init`. `neo-init` spawns Arthur with only the session channel; it neither inherits nor receives the `GpuSubmitCap`. This closes the fresh-buffer escape as well as the attached-surface path: attenuating one buffer would be insufficient because Arthur can call `SYS_BUFFER_CREATE` again.

Slice A therefore performs an adversarial in-session regression before importing the real surface. Arthur creates a fresh 1x1 full-rights buffer, maps it RW, writes poison pixel `0x00d123e4`, commits that one pixel, and attempts to present it at framebuffer `(0,0)`. The call must return `EPERM`. The kernel compares the destination pixel before and after the userspace session and requires exact equality. The compositor then presents Arthur's separate RO-imported surface successfully under its `GpuSubmitCap`, and the existing 128/128 `A`-glyph framebuffer oracle proves the authorized route still reaches scanout. These three outcomes—client `EPERM`, unchanged poison destination, compositor success—are the re-AAG capability oracle.

## Memory discriminator: 485 MB model plus compositor

The named risk is **`model-485MB-plus-compositor`**: the approximately 485 MB Qwen2.5-0.5B `.neom`, its 151,936-token vocabulary, the 1,024,000-byte client surface, framebuffer/compositor state, USB rings and reports, generation state, KV/scratch allocations, and kernel/userspace heaps coexist. OT-28 proved the model alone; Slice A proves the never-before-combined resident set in a 4096 MB QEMU guest, and Slice B repeats it on the OptiPlex.

Arthur records `sys_sysstat` memory/frame counts at `surface_resident` and `model_resident`, followed by `SHOWCASE footprint discriminator=model-485MB-plus-compositor`. Missing either breadcrumb, model-load failure, allocation failure, or a nonzero client exit is red. The bare-metal proof must retain those values in serial and the NVMe sink so an OOM can be separated from a later inference stall.

## Model staging and fail-closed policy (ISC-2)

The model and `.vocab` sidecar are not repository artifacts. Xtask stages the same OT-28 inputs supplied through `NEO_STAGE_B_NEOM` or `NEO_STAGE_B_GGUF` into a 768 MiB NeoFS image as `/model-qwen2.5-0.5b.neom` plus its adjacent sidecar. Slice A may use the hermetic fixture to gate the render and coexistence seam in CI. Slice B forbids that fallback: a blob-less checkout or unset real-artifact variables fails during bundle creation with a message naming the required 485 MB artifact. The ISO therefore stays blob-free; the SATA NeoFS volume carries the payload out of band.

## Ladder and gates

### Slice A — composited greeting

`cargo neo-test arthur_compositor` builds the combined kernel/userspace configuration, attaches xHCI and USB keyboard devices without injecting input, mounts NeoFS, loads the model, captures `target/neo-test/arthur_compositor.ppm`, and checks the ordered transcript, denied-bypass poison oracle, and exact compositor framebuffer pixel oracle. Its terminal proof says `render_seam=proven scanout_gate=gpu_submit client_present=eperm framebuffer_unchanged=true compositor_present=allowed input=not_exercised`. This is the QEMU scenario included in the default four-shard gate.

### Slice B — JOINT OptiPlex capstone

With a real artifact selected, `cargo neo-test --build-only arthur_compositor_baremetal --bare-metal-mode --include-ignored` builds the ISO/disk bundle. The PL operates the physical boot select, keyboard, live screen, and PuTTY serial capture; Jarvis drives SSH preparation and analyzes the resulting log. The operator types a nonempty question and Enter. Required ordered evidence is focus activation, one or more routed keystrokes, inference start, real forward/decode/reply breadcrumbs, inference completion, answer commit/present, exact answer serial, surface destruction, clean model drop, and the Slice-B terminal frame. This is silicon-only and does not become QEMU proof by virtual key injection.

The authoritative SSH, `efibootmgr`, boot-select, fixed `C:\Users\<user>\neoplutarch-serial.log`, SATA model volume, and NVMe sink mechanics are in [runbook-baremetal-arthur-optiplex.md](runbook-baremetal-arthur-optiplex.md). The older auto-forward interaction in that runbook is replaced by this scenario's explicit keyboard question; its access and capture mechanics remain normative. The durable artifact is serial plus sink backup—no photo. The PL's live read confirms the composited answer is legible.

## Kill-switch and equality regression (Rule 1)

The integration is additive. With `arthur-compositor` absent, Arthur's route policy returns `StandaloneScenarios`; `neo-test-arthur-console` and `neo-test-atrium-interactive` retain their exact feature/user-feature records and behavior. Scanout enforcement defaults off, so standalone Software and HardwareFlip paths retain their prior ambient-present behavior unless their build explicitly arms the gate. Enabling the code without selecting the showcase also retains the standalone route. Host KATs cover all four authority truth-table entries and pin the old xtask mappings. The close gate reruns the existing standalone scenarios through the unchanged default suite. There is no global default-feature flip.

## I-14 fault containment

- **I-14(a), no AI-governance lock:** USB IRQ/pump, InputFocus receive, Arthur raster, SurfaceCap commit, and G1 present acquire no AI admission/residency/governance lock. Inference enters the already-governed model path only after the input IPC is complete. The serial progression from focus to `inference start` to commit distinguishes the boundary.
- **I-14(b), bounded degradation:** a stalled Arthur blocks only its dedicated surface/session channel. The compositor remains the framebuffer owner, other surfaces are independent, the input queue remains bounded/drop-oldest under ADR-074, and governance does not wait for a client commit. Slice A proves compositor/model coexistence without input; process exit and explicit destroy prove teardown. A watchdog/terminal-frame timeout makes a stuck seam red rather than silently green.
- **I-14(c), no new unfenced DMA:** integration adds no DMA engine or DMA mapping. USB uses the existing fenced arena/path. Arthur's model tensors and any acceleration placement remain under the existing residency/DMA governance. `SharedPixelBuffer` is CPU-mapped shared memory, compositor RO-NX, not a device DMA aperture.

## Concurrency-pitfall-bank consultation

The implementation adds orchestration and IPC but does not change scheduler, interrupt, atomic, IPI, or lock primitives. Against [concurrency-pitfalls.md](concurrency-pitfalls.md):

| Pitfall | Classification | Guard and discriminator |
|---|---|---|
| CP-1 / CP-16 lock order and foreign-lock wake | **Guarded; no new kernel lock** | IRQ pump enqueues before waking; the client never holds a surface/compositor lock while entering inference. Ordered focus→inference→commit breadcrumbs plus existing lockdep/panic output distinguish a cycle. |
| CP-2 / CP-3 IF-off wait | **N/A introduced** | No new IF-disabled region or cross-CPU wait. USB uses its proven short IRQ path; all new waits are userspace channel/focus waits. Existing shootdown timeout discriminators remain armed. |
| CP-12 generation/ABA | **Guarded** | ADR-073 identity generation and monotonically increasing client serial reject stale surface requests; exact create/commit/destroy replies and final answer serial are the discriminator. |
| CP-14 lost wakeup / CP-15 stranded wake | **Guarded by existing scheduler token+wake path** | Keyboard report count plus `SHOWCASE keystroke` proves IRQ→queue→Arthur wake. Reports increasing without a keystroke localizes the fault to focus/wake rather than USB. |
| CP-17 publication-before-ready | **Guarded** | Arthur does not announce greeting until import, initial damage/commit, present fence, and focus request all succeed. G1 does not activate until it owns the focus activator. Ordered seam breadcrumbs are the discriminator. |
| CP-18 single-in-flight device | **Guarded by existing USB pump** | No second submitter is added. HID `reports`, `bytes`, and `ep_errors=0` terminal counters expose endpoint serialization failures. |
| CP-19 through CP-29 AP/idle/IPI classes | **N/A introduced** | No AP, runqueue, idle, IPI, TLB, or atomic-publication code changes. Existing four-core shard suite remains the regression discriminator. |

## Blind-hardware failure/discriminator bank

Capture starts before boot and every seam emits serial to both the primary log and NVMe sink.

| Failure beyond happy path | Discriminator / localization |
|---|---|
| Combined model + compositor + USB set OOMs | Last `surface_resident` / `model_load start` / `model_resident` sysstat line; absence of the 485 MB footprint line; allocator panic or nonzero Arthur exit. |
| Model volume/artifact missing or malformed | Host build fails closed when variables are absent; on target, NeoFS mount, vocab token count, model open/size, and load-resident lines isolate mount vs sidecar vs model validation. |
| G1 never comes up or loses framebuffer ownership | Missing `boot framebuffer=ready owner=g1-compositor` or `compositor up`; framebuffer-console combination is rejected at compile time. |
| Arthur surface create/import fails | Last of `surface created` and `seam buffer=client-rw compositor-ro-nx`; dedicated return code in the client/init exit line. |
| Greeting is logged but not composited | `greeting composited` without the exact 128/128 pixel oracle or PPM bright-pixel capture; direct serial cannot satisfy framebuffer readback. |
| Arthur bypasses G1 with a fresh self-created buffer | Missing `client_present=EPERM`, a changed `(0,0)` poison destination, or missing authorized compositor-present line. The three-part oracle separates unarmed gate, framebuffer mutation, and missing compositor authority. |
| Input never reaches Arthur | Compare USB `reports/bytes/ep_errors`, `focus activated`, and first `keystroke`. Reports without keystroke means focus/queue/wake; no reports means USB enumeration/endpoint. |
| Input snoops to compositor or wrong client | Activator RECEIVE must return `EPERM` without changing the poison buffer; focused-surface identity and no-snoop line are required. |
| Typed glyphs fail to present | Keystroke exists but the subsequent accepted commit/present is absent; commit return code and client serial localize raster/damage vs G1 present. |
| Inference hangs only with compositor resident | `inference start` followed by existing per-token forward/sample breadcrumbs distinguishes slow progress from a true stall; no new breadcrumb before timeout localizes scheduler/memory interaction. |
| Model load fails after compositor is up | `surface_resident` present, then vocab/open/load line stops before `model_resident`; sysstat delta shows available memory at the boundary. |
| Reply generated but not visible | `reply="..."` and `inference complete` without answer serial / `answer composited`; this localizes the failure to raster/commit/present, not inference. |
| Surface teardown or model drop wedges | Answer is composited but Slice-B terminal frame is absent; last destroy/wait/drop breadcrumb and process exit code discriminate ownership cleanup. |
| Primary serial capture is lost | Recover the same stream from the NVMe `NEO_LOG_v1` sink at the runbook's fixed offset; neither a photo nor screen recollection substitutes for the log. |

## Close gate

Because the change touches the kernel, completion requires the repository close-gate ritual, fresh rebase/equality and SPDX checks, the commit's own `git show --stat`, and a green four-shard `workflow_dispatch` QEMU URL. Slice A participates in that gate. Slice B closes only after the JOINT serial/sink proof and live composited-screen read; it remains honestly hardware-only.
