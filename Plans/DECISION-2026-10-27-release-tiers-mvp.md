<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: release tiers, and the MVP (PL, 2026-10-27)

**PL (J.c.), 2026-10-27:** "Yeah make that the MVP tier." This came after the PM mapped LogansOS against a full
desktop-OS checklist and proposed three tiers.

| Tier | Bar | Status |
|---|---|---|
| **0.1 "Arthur Preview"** | The five rungs of `Plans/MILESTONE-2026-07-29-logansos-0.1-arthur-preview.md` (boots on real hardware, a real desktop, talk to Arthur, online with eth0, work persists) **in one boot**. | **Not yet demonstrated.** All five rungs' code is on main, and each rung is proven on its own, but **no single boot has shown all five together**: the Zenbook boots have no network lease and no NeoFS volume, and the OptiPlex one-boot run (dispatches 3b 2026-08-26 and B1 2026-10-20) has no RETURN. The one-boot showcase is the outstanding 0.1 item. |
| **MVP (0.5)** | A computer you can set up and use: **install from the USB (Try or Install)**, **log in**, a desktop with **terminal, file manager, text editor and settings**, Arthur, **wired networking**, **clean shutdown and restart**. | The tier now being built. |
| **1.0** | The full desktop checklist, except self-hosting: adds **the browser** (the PL's 2026-07-12 rule, "an OS that can't browse isn't shippable", still holds), **WiFi**, **sleep and wake**, **safe updates** (OT-51), **media playback**. | The browser C-ladder runs in parallel (C1.2 in gate). |
| **Post-1.0** | Self-hosting: the OS rebuilds itself on itself. | Not started. |

## MVP tracking

| Piece | OT | State |
|---|---|---|
| Install (Try or Install, erase disk and install, Try opens read-only) | OT-87 (absorbs OT-65) | THINK in review (PR #149); R1 `xtask vm-image` building (S2) |
| Login / session (a human-user object, a credential store, a login and lock screen, a session broker) | **OT-66** (existing, 2026-07-29) | THINK queued for the next free seat |
| Text editor (with the text engine) | **OT-67** (existing, 2026-07-29) | THINK (text engine first) queued for the next free seat |
| Terminal, file manager, settings | **OT-88** (new) | THINK queued for the next free seat. It builds on the desktop (DW lanes), `neo-shell` and OT-57 (VT100) |
| Clean shutdown and restart (ACPI S5 and reset, apps and NeoFS flushed first); sleep and wake are 1.0 | **OT-90** (new) | THINK dispatched to S5 (who did the ACPI/AML work) |
| Wired networking at boot | — | Ethernet works (RTL8125, USB-Ethernet); confirm the product boot flow brings it up |
| Arthur | — | Working; the speed lane continues |

Each new OT gets a design THINK, then a cross-family read, before any build. That's the pattern OT-87 set.
