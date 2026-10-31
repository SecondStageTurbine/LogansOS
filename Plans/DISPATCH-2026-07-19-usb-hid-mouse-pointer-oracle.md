<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: USB HID mouse — the pointer for the GUI (Milestone-A input) → Oracle

**Date:** 2026-07-19
**From:** PM (Claude)
**To:** Oracle (main-tier; Phase-7.5 connectivity/driver lane)
**Status:** DISPATCHED — kernel-touching USB driver ⇒ mandatory 4-shard `workflow_dispatch` QEMU gate + AAG. Milestone-A input: the **pointer** the coming GUI (OT-52) needs.

## Scope reframe — the KEYBOARD IS DONE; build the MOUSE (do NOT rebuild the keyboard)
USB HID **keyboard** input already works end-to-end on bare metal — the PL physically typed "Arthur" and "Hello World" on a USB keyboard and it registered in NeoPlutarch. It is `kernel/src/drivers/usb/hid_keyboard.rs` (boot-protocol keyboard class driver) + `hid_logic.rs` (`BootKeyboardDecoder`) on the xHCI host stack, proven by `neo-test-input-path-console` (`usb_hid_boot=ok neo_shell_reads=ok`). **Do not touch or rebuild it.** The one missing input device is the **pointer**: there is **no USB HID mouse driver** (`ls kernel/src/drivers/usb/` → `hid_keyboard.rs`, `hid_logic.rs`, no mouse). The PL's fleet is **USB-only, no PS/2** (standing decision, 2026-07-19), so a USB HID mouse is the pointer.

## Build — a USB HID boot-protocol mouse class driver, MIRRORING the keyboard
The keyboard driver is your template; the mouse reuses the same infrastructure:
1. **Enumerate/claim the mouse interface** on the existing xHCI host stack: HID class (`bInterfaceClass=3`), boot subclass (`bInterfaceSubClass=1`), **mouse protocol (`bInterfaceProtocol=2`)**. `SET_PROTOCOL(boot)` on the interface (same as the keyboard's boot-protocol setup). A composite keyboard+mouse device may expose both interfaces — claim the mouse one without disturbing the keyboard's.
2. **Interrupt-IN endpoint via the SAME arena-fenced pipe** the keyboard uses (`EndpointPipe`, `FencedDmaAddr` report buffers, `dma_arena`) — this is how I-14 is inherited (see below).
3. **Decode the boot-protocol mouse report** in a pure `hid_logic`-style decoder (mirror `BootKeyboardDecoder`): byte 0 = button bitmap (bit0 left / bit1 right / bit2 middle), byte 1 = **signed** dX (i8), byte 2 = **signed** dY (i8), optional byte 3 = wheel. **Trap-mine the real variants** (below) — not every mouse sends exactly 3 bytes.
4. **Feed pointer events into the input path** — a bounded pointer-event queue (buttons + relative dX/dY [+ wheel]) that the input consumer reads, exactly as the keyboard feeds keystrokes into the console/shell. **SCOPE GUARD:** deliver the *events*; **routing a pointer event to the focused compositor surface is OT-52's job — do NOT build surface-routing here.** Coordinate the event shape with the OT-52 compositor so it can consume it later.

## Governance fault-containment analysis (I-14 — MANDATORY for a connectivity/driver dispatch)
The mouse **inherits I-14 from xHCI exactly like the keyboard** — state each vector:
1. **Lock isolation (a).** The mouse ISR/interrupt-EP path acquires **no** AI-governance lock (`INFLIGHT`/`RECENCY`/`ARENA`/reservation/`TABLE`); it only routes completions into xHCI's bounded per-endpoint queue (grep-checkable + assert). Confirm.
2. **Liveness degradation (b).** A misbehaving/spamming mouse cannot monopolize a CPU or stall governance — the interrupt-EP is bounded (`REPORTS`/`PENDING_MAX`-style caps like the keyboard); a flood is dropped, not queued unboundedly.
3. **Memory isolation (c).** Every report buffer is a `FencedDmaAddr` through `dma_arena::fence(...)` under the existing xHCI IOMMU containment — **no new bus master, no new DMA path** (identical to the keyboard's I-14 inheritance, which its header states explicitly). Confirm no unfenced DMA.

## Prior-art trap-mining (MANDATORY for a driver/blind-hardware dispatch — mine before you code)
Seed the failure bank from documented USB-HID-boot-mouse gotchas (USB HID spec 1.11 §B.2 boot mouse; Linux `drivers/hid/usbhid` + `hid-input.c`; OSDev USB HID):
- **Boot-report length varies:** the HID boot mouse report is *nominally* 3 bytes (buttons, dX, dY) but many mice send **4** (adding a wheel byte) and some report descriptors declare more — **do not hard-assume 3**; read the endpoint's `wMaxPacketSize`/report length and decode defensively.
- **Signed deltas:** dX/dY are **i8 two's-complement** — a common bug is treating them unsigned (mouse jumps right/down only). Byte-oracle a negative delta.
- **SET_PROTOCOL(boot) required:** without it some mice send the (variable) *report*-protocol format, not boot format → garbage decode. The keyboard already does this; mirror it.
- **Composite HID device:** keyboard+mouse on one physical device (very common) → claim the mouse interface by its `bInterfaceProtocol=2`, and do not break the keyboard interface's existing claim.
- **NKRO/idle-rate & wheel-only reports:** some send a report with all-zero movement (just a button/wheel change) — decode must handle no-movement reports without spurious events.
Seed each into the failure/discriminator bank with its serial breadcrumb.

## Kill-switch + byte-oracle
- **Kill-switch (Rule-1):** the mouse behind a feature (extend `usb-hid` or a `usb-hid-mouse` sub-feature); **mouse-disabled ≡ the current keyboard-only behavior, bit-for-bit** (an armed regression: with mouse off, `input_path_console` is byte-unchanged).
- **Byte-oracle (I-12/on-wire layout):** the boot mouse report is an on-wire byte layout ⇒ a **hand-laid byte fixture** asserted independently of the decoder (e.g. `[0x01, 0x05, 0xFB, 0x02]` → left-button=down, dX=+5, dY=−5, wheel=+2), including a **negative-delta** case (the i8 trap) and a **4-byte-with-wheel** case. Mirror the keyboard decoder's byte-oracle. Pull the exact report layout from the **USB HID spec**, not a summarizing fetch.

## Gate + close
- A **hermetic QEMU scenario** mirroring `input-path-console`: inject HID mouse boot reports → assert the decoder emits the right buttons + signed dX/dY [+ wheel] pointer events + I-14 arena-fenced counters + init continues. Feature-off equality proves the kill-switch.
- **Bare-metal follow-on (joint, like the keyboard's):** plug in a real USB mouse on the OptiPlex, move it / click → the pointer events register on serial. Reference `docs/runbook-baremetal-arthur-optiplex.md` (the standing bare-metal run procedure — the **serial log is the proof, no photo**; JOINT PL+coder). This is the on-silicon confirmation, same pattern as the keyboard's bare-metal proof.
- `scripts/worker-close-gate.sh` (rebase-fresh; full host-check set; SPDX; Step-4 self-stat). Kernel-touching ⇒ the 4-shard `workflow_dispatch` QEMU gate is mandatory. **Runner note:** one 4-shard wave at a time — stagger behind any concurrent kernel gate. RETURN pastes the green push-CI + 4-shard URLs on the exact head + the I-14 analysis + the trap bank.
- **Disk reclaim (after PM confirms merge):** delete this build's tree once on `origin/main` + PM-confirmed.

## AAG (PM, pre-merge — driver)
Lenses: **I-14 / memory-safety / DMA-fencing** (the report buffers are `FencedDmaAddr`, no new master — the load-bearing one for a driver), **ABI/byte-oracle** (the boot mouse report decode is byte-correct incl. signed deltas + variable length), **completeness** (the trap bank's variants handled; the QEMU scenario proves decode; bare-metal is the silicon close), **concurrency** (the interrupt-EP path takes no governance lock, bounded queue).

---
## Relay — what to tell Oracle
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-19-usb-hid-mouse-pointer-oracle.md`. **Template to mirror (do NOT rebuild):** `kernel/src/drivers/usb/hid_keyboard.rs` + `hid_logic.rs` (the proven keyboard driver — same xHCI stack, same arena-fenced I-14 pattern, same `UsbClassDriver` trait, same decoder shape). PM artifacts live on `origin/main`; your tree is on a feature branch — `git show`, not a local open.
- **One-line:** The USB HID **keyboard is already done** (you typed to Arthur on it) — build the missing **USB HID boot-protocol MOUSE** (the pointer the GUI needs), mirroring `hid_keyboard.rs` on the same xHCI arena-fenced stack. Decode the boot mouse report (buttons + **signed** dX/dY + optional wheel — trap-mine the length/sign variants), feed pointer events into a bounded input queue (deliver events; **surface-routing is OT-52's job, not here**). Include the I-14 fault-containment analysis (inherit from xHCI, no new DMA master), a byte-oracle for the report layout (incl. a negative-delta + 4-byte-wheel case, from the USB HID spec not a fetch), and a kill-switch (mouse-off ≡ keyboard-only bit-for-bit). Hermetic QEMU scenario (inject reports → assert decode + I-14) + a joint bare-metal plug-in-a-mouse confirmation (runbook; serial log, no photo). 4-shard QEMU gate + driver AAG.
- **RAG:** use it for the USB/HID/xHCI seams — health-check `Invoke-RestMethod http://127.0.0.1:8404/health`; e.g. `hid_keyboard UsbClassDriver claim endpoint`, `xhci interrupt endpoint pipe FencedDmaAddr`, `BootKeyboardDecoder report decode`, `usb_parse interface protocol`. If down, note it + grep/Read (never a blocker; the PL owns RAG startup).
