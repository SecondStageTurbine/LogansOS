<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# G1 Slice 5 — interactive capstone close

## Phase 1 contract

`atrium_interactive` closes the live loop across real user processes. The
client (`neo-worker`) owns the `InputFocus` RECEIVE root and the RW mapping of
its `SharedPixelBuffer`. The compositor (`neo-init`) receives only an
ACTIVATE-only focus sibling and a RO/NX, PRESENT-only buffer sibling. It cannot
read an input event or write client pixels.

The initial surface is the opaque neutral Reduce-Transparency floor ratified by
ADR-024 Amendment §3. It is the accessibility, CPU-only, and GPU-absent render,
not a placeholder. OT-63 may replace the present backend with ordered
backdrop-blur behind `atrium::present_backend`; the surface/input/damage
contract does not change. Glass is not built here.

The existing Slice-2 `COMMIT` verb is unchanged. Slice 5 adds syscall `0x1AD`,
`BUFFER_COMMIT_DAMAGE`, carrying exactly one eight-byte `DamageRectV1` (`x`,
`y`, `width`, `height`, each little-endian `u16`). Empty, overflowing, and
out-of-buffer rectangles fail closed. One pending rectangle per buffer bounds
each present; a later client commit replaces its own pending work rather than
growing an unbounded compositor queue.

The commit/damage record and ready bit are published and consumed under one
leaf lock. Pixel writes precede the commit's Release publication; present
consumes it with AcqRel, copies only the rectangle under the framebuffer lock,
then publishes the existing epoch fence after the copy. A client must wait for
that fence before reusing affected pixels. There is no IF-off wait, scheduler
wake edge, DMA path, or governance lock. This preserves the CP-2/3,
CP-14/15/16, CP-17, CP-29, and I-16 guards described in
`docs/concurrency-pitfalls.md`.

Fault containment is local: input remains capped at 64 events with
drop-oldest; damage is one validated rectangle; a never-committing client
causes no present; a wild pixel write can affect only its own buffer and only a
committed rectangle reaches scanout. The QEMU negative control deliberately
writes magenta outside the glyph cell and proves that framebuffer pixel stays
the opaque floor.

The two-layer compositor kill-switch remains unchanged. Feature/runtime off
routes to the pinned v1 console digest and all Atrium syscalls return `ENOSYS`
before allocation.

## Phase 2 — joint OptiPlex close

This phase requires the PL physically at the OptiPlex and the coder driving the
run. Follow `docs/runbook-baremetal-arthur-optiplex.md`: PuTTY logs serial to
`C:\Users\<user>\neoplutarch-serial.log`; the NVMe sink at offset 256 MiB is the
backup; trigger the one-shot target boot with `efibootmgr -n 0001 && reboot`.
The PL types `a` on the USB keyboard and confirms the glyph appears by eye.
The serial log is the durable proof. There is no framebuffer photo.

Required serial breadcrumbs and discriminators:

| Failure mode | Required serial discriminator |
|---|---|
| USB keyboard enumerates but reports do not pump | `SLICE5 HID pump=<n> reports=<n> bytes=<n>`; pump rising with reports stuck at zero isolates xHCI/endpoint delivery. |
| Kernel sees the key but the workfield does not | `SLICE5 route key=0x04 focus=<id> delivered=<0|1> dropped=<n>`; a focused id with `delivered=0` isolates InputFocus routing/timing. |
| Client renders but no visible present occurs | `SLICE5 commit surface=<id> epoch=<n> glyph=a` followed by `SLICE5 present epoch=<n> outcome=<n> blit_complete=1`; missing completion isolates GOP/present. |
| Wrong region appears or adjacent pixels change | `SLICE5 damage submitted=8,8,8,16 composited=8,8,8,16`; a mismatch isolates validation/translation rather than HID or glyph raster. |

The success line is:

`SLICE5 REPLY: a focus=<id> commit_epoch=<n> damage=8,8,8,16 present=complete hid_reports=<n>`

Trap-mine principle: keep inherited Limine GOP (no modeset) and the existing
arena-fenced xHCI HID path. Intel PRM GOP and xHCI prior art inform the
diagnostic split only; no external implementation is copied. A missing success
line or any discriminator mismatch fails the joint close even if the PL saw a
transient glyph.
