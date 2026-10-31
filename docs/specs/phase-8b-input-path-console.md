<!-- SPDX-License-Identifier: MPL-2.0 -->
# Phase 8b Input Path Console

Status: implementation note for OD-1 Phase 8b.

## Scope

Phase 8b adds the minimal physical console path needed to type into
`neo-shell` without serial:

- `framebuffer-console` routes fd=1 bytes to a UEFI framebuffer text console
  alongside serial output.
- `usb-hid` routes a USB HID boot-protocol keyboard to fd=0.

Both features are default-off. With both disabled, the kernel remains the
existing serial-only build.

## Framebuffer Console

The console is intentionally small: 8x16 bitmap glyph blits, cursor, newline,
backspace, tab-as-spaces, and scroll-on-overflow. It does not implement ANSI
escape sequences, colors as a terminal API, a TTY, a window system, or input
line discipline. `neo-shell` keeps owning line editing and command parsing.

## USB HID Keyboard

The HID path binds only interfaces with class `0x03`, boot subclass `0x01`,
keyboard protocol `0x01`, and an interrupt-IN endpoint. Bind sequence:

1. Configure the interrupt-IN endpoint on the existing xHCI pipe surface.
2. `SET_CONFIGURATION`.
3. `SET_PROTOCOL(boot)`.
4. Best-effort `SET_IDLE`, with STALL tolerated.
5. Poll fixed 8-byte boot reports and map new keycodes to ASCII.

There is no report descriptor parser, NKRO support, mouse support, LED/output
report handling, TTY, or POSIX shell behavior in this slice.

## I-14 Containment

HID rides the Phase 7.5 xHCI stack. The interrupt endpoint uses the existing
arena-fenced `EndpointPipe` API, and every report buffer address submitted to
the controller is a `FencedDmaAddr`. The xHCI ISR only routes completions into
the bounded per-endpoint queue; the HID driver drains that queue in process
context under the `USB_DEVICE` lock order. No new bus master, DMA path, ISR
AI-governance lock, or invariant is introduced.

## Stage-B Hardware Discriminator

The QEMU gate uses `qemu-xhci` plus `usb-kbd` and proves the software path
through `neo-shell`. Real-keyboard bare-metal is a PL-coordinated Stage-B
follow-on. Blind hardware diagnostics should separate:

- `SET_PROTOCOL(boot)` accepted vs stalled/ignored.
- Interrupt endpoint polling interval and completion cadence.
- LED/output-report requirements before reports start.
- NKRO/report-descriptor-only keyboards that do not provide boot reports.

## Arthur Console Integration

The `arthur_console` QEMU scenario composes the already-landed transport and
Arthur surfaces in one deterministic flow: `qemu-xhci` + `usb-kbd` deliver
boot-protocol HID keystrokes to fd=0, `neo-shell` launches `neo-arthur` with
`console-interactive`, Arthur owns the line editor until `exit`, and fd=1
mirrors Arthur's deterministic model reply to both serial and the framebuffer
console. The interactive loop calls the existing `respond_to_turn` pipeline;
feature-off Arthur builds keep the original shell-mediated channel relay and
deterministic `arthur: echo: hello arthur` behavior.

This adds no new I-14 vector. The keyboard is still a USB function behind the
existing xHCI arena fence, the ISR still avoids AI-governance locks, and
`framebuffer-console`/`usb-hid` remain default-off. There is no TTY, line
discipline, multi-principal, capability escalation, or new Arthur action in
this integration slice. Bare-metal confirmation logs `fb_console res=...`
`stride=...` and `hid_keyboard bound` breadcrumbs so display-vs-keyboard
failures localize in one boot.
