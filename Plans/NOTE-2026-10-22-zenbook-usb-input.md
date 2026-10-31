<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE — Zenbook S14 USB input: diagnosis, fixes, and the next run's discriminators (S2, 2026-10-22)

**Branch:** `claude/zenbook-usb-input` (from `main` `4e9ea28c`). **Features:** `xhci-silicon`, `usb-hotplug`
(both in `desktop`, so in image A). **Runbook:** `docs/runbook-baremetal-zenbook-s14.md` (photos are the
evidence on this target).

## 0. What the run showed

Image A (`main` `4e9ea28c`) booted to the desktop on the Zenbook (Panther Lake): x2APIC, the LAPIC tick and
the RTC all work on silicon. A USB keyboard and a USB mouse, plugged into a **USB-C port through a USB-A →
USB-C adapter**, gave no pointer and no typing. The mouse LED lit, so the port supplies VBUS. The desktop
drew over the boot log, so there was no evidence of what the USB stack did.

**Retest (16:20 UTC):** a USB keyboard straight into the **USB-A** port worked in the firmware (it chose the
boot entry) but not in our OS, and neither did the mouse. That is **what the diagnosis below predicts for the
image that ran** (`main`): `main` brings up only the first xHCI in scan order, the TCSS one at `00:0d.0`;
the USB-A port is on the south xHCI at `00:14.0`, which `main` never touches. Firmware drives `00:14.0`
itself, so the keyboard working there and not in our OS separates "the device and port are fine" from "our
OS never looks at that controller". This branch's census chooses the controller with the USB 2 device, so
the USB-A keyboard should bind on the next run. If it does not, the summary below says which of the PM's
suspects (a)–(e) it is (§3).

**The PL's Windows inventory confirms the two controllers** (the inventory file from #94 is kept in the private repo). The PCI instance ID's last field is (device << 3) | function:
- `PCI\VEN_8086&DEV_E331 … &68`: **00:0d.0**, the Type-C subsystem's xHCI. It sits beside the USB4 host
  routers `E333`/`E334` (00:0d.2/.3). This is the controller `main` brings up.
- `PCI\VEN_8086&DEV_E37D&SUBSYS_201F1043 … &A0`: **00:14.0**, the south xHCI, where the USB-A port and every
  USB 2 pin are.

The devices the PL used: keyboard `04D9:0006` is composite (`MI_00` keyboard, `MI_01` consumer control); its boot
keyboard interface is 0, which `hid-keyboard` claims. Mouse `093A:2510` is a separate single-interface device.

## 1. The PM's questions, answered from the code (`main` `4e9ea28c`)

| Question | Answer on `main` | Evidence |
|---|---|---|
| Do we probe every xHCI or only the first? | **Only the first.** `pci::find_by_class(0x0C,0x03,0x30)` returns the first match in bus/device order, and the whole driver is one global controller. | `xhci.rs::find_xhci`, `pci.rs::find_by_class` |
| Are the USB-C ports on the TCSS xHCI, not the PCH xHCI? | **Their SuperSpeed lanes are; their USB 2 pins are not.** Since Tiger Lake, Intel client parts have a Type-C subsystem (TCSS) xHCI at bus 0 device **0x0d** that carries the Type-C ports' SuperSpeed lanes, and a south ("PCH") xHCI at device **0x14** that carries every USB 2 pin, including the Type-C connectors' D+/D−, and the USB-A ports. A keyboard or mouse is low/full speed, a USB 2 signal, so it appears on **0x14**. Bus 0 is scanned in device order, so **0x0d is "the first xHCI"**: we brought up the one controller that can never see the devices. | Linux `xhci-pci.c` lists the TCSS controllers separately from the PCH ones (Tiger Lake: `0x9a13` TCSS vs `0xa0ed` PCH); the Tiger Lake / Raptor Lake "Thunderbolt 4 USB Controller" IDs in `pci.ids`. Panther Lake IDs are not in `pci.ids` yet; the census prints them. |
| Is the BIOS/OS handoff right? | **Partly.** We requested ownership but on timeout logged `bios-owned` and continued with firmware still owning the controller. We never touched USBLEGCTLSTS, so **firmware SMIs stayed armed** (Linux disables them and clears latched SMI events). | `xhci.rs::legacy_handoff`; Linux `pci-quirks.c::quirk_usb_handoff_xhci` (principle) |
| Port power, USB 2 / USB 3 companion ports? | **Port power was never set** at the root (only as a side-effect of a port reset), so a controller with PPC=1 left ports unpowered after HCRST. USB 2 and USB 3 ports of one connector are **different root ports**; the code never knew which was which (no Supported Protocol walk). The boot scan read ports **~30 ms** after HCRST (3 × 200k spins) and never again; **port-change events were counted and dropped**, so a device that showed its connection later, or was plugged in at the desktop, was never enumerated. | `xhci.rs::scan_ports`, `drain_events` (`OTHER_EVENTS`) |
| Does a USB4/TBT port need a connection manager first? | **Not for this.** The Type-C port's USB 2 pins go straight to the south xHCI; no tunnel, mux or connection manager is involved. A SuperSpeed device on a USB-C port would need the Type-C mux set (firmware/PMC) and the TCSS xHCI, which we do not drive yet (§6). | — |
| MSI vs INTx? | **Not the blocker.** Every enumeration step polls the event ring (`wait_event` drains), and the HID pumps call `poll_events`. MSI only reduces latency. The summary now prints `msi=` and the IRQ count. | `xhci.rs::wait_event`, `hid_*::drain` |

Two more silicon gaps found while reading the hub path, relevant because a keyboard **and** a mouse on one
adapter suggests a hub (or a keyboard with a pass-through port):

- **Hub slots were marked by Evaluate Context.** On xHCI > 0.95 (all modern Intel, 1.2) Evaluate Context
  carries only Max Exit Latency, Interrupter Target and EP0 packet size (xHCI §4.6.7); Hub, Number of Ports,
  MTT and TT Think Time are taken only on **Configure Endpoint** (Linux `xhci_update_hub_device`, principle).
  A real controller can ignore our "this is a hub", and full/low-speed children behind a high-speed hub then
  have no transaction translator.
- **Hub ports were read ~10 ms after power-on** (400k spins). USB 2.0 needs bPwrOn2PwrGood (hub descriptor
  byte 5, 2 ms units) plus the 100 ms connect debounce before a port's connection bit means anything, so a
  real hub's children read as "not connected" and were never revisited.

**Most likely cause of this run: the first one** — the kernel brought up the TCSS xHCI. Every other gap is
plausible on its own and would hide behind it, so all are fixed in this branch rather than one per run.

## 2. What the branch changes

`xhci-silicon` (new child module `xhci_silicon.rs`, pure logic `xhci_silicon_logic.rs`):
1. **Census of every xHCI** before anything is reset: BDF, vendor:device, PCI power state, version, port
   count, the Supported Protocol port ranges (xHCI §7.2), and which root ports had a connection as
   firmware left them. **Choice** (`logic::choose`): one controller → it (as before); else the first with a
   device on a USB 2 port; else the most USB 2 ports; else the first readable. A SuperSpeed-only controller
   (TCSS) has no USB 2 ports, so it can only win when nothing else reads back.
2. **D0** for a controller in D3hot (PMCSR, 10 ms recovery).
3. **Handoff as Linux does it** (principle): request ownership; wait ≥ 500 ms; on timeout **force** it
   (BIOS-owned cleared); then **USBLEGCTLSTS: every SMI enable off, latched SMI events cleared**.
4. **Root-port power**: any port with PP=0 is powered, then 20 ms; then **wait ≥ 300 ms for the ports the
   census saw connected to reconnect** before the boot scan.
5. **Hubs**: Configure Endpoint (slot context copied from the output context + Hub, Number of Ports, TTT)
   on xHCI > 0.95; **bPwrOn2PwrGood × 2 + 100 ms** after powering a hub's ports.
6. **The USB summary** (`usb-summary:` lines) and the **evidence hold**: before the desktop takes the screen,
   image A waits up to 5 s for a keyboard or mouse to bind, prints the summary, and if nothing bound
   **holds the screen up to 30 s** (a device binding during the hold ends it and prints the summary again).

`usb-hotplug` (new child module `xhci_hotplug.rs`): a kernel thread re-reads every root port and every
known hub port **every 250 ms** and enumerates what is connected but not enumerated (3 attempts per
connection; a disconnect resets the count). A re-plugged keyboard/mouse replaces the old one; the old
one is **leaked, not dropped**, because no Disable Slot is issued and the controller may still reference
its contexts.

Time without a calibrated clock: USB comes up before the timer is calibrated, so every new wait is a TSC
budget sized for a **10 GHz** TSC (`tsc_cycles_for_ms`): at least the stated time on any real part, about
5× longer on a 2 GHz one. Hub/port waits only add time when a device is present or slow.

Boot console font: `fb-progress-console` builds now carry glyphs for `= q z j J Q X Z _ + ! # % * ; < " '`
(the old subset drew `?` for every `=` in a `key=value` line). Other builds' font is unchanged.

## 3. Failure-mode + discriminator bank for the next run

The screen at the end of boot shows the `usb-summary:` block. One photo of it separates the PM's suspects:

| Suspect | Summary line(s) that decide it |
|---|---|
| (c) the south xHCI is never probed | `xhci controllers=N chosen=[i] reason=…`; the census row `[i] 00:14.0 vid:did …`; `fw-ports` shows the keyboard's port as connected under firmware |
| (a) handoff / SMI | `handoff=os-owned|forced|no-legacy-cap legctl=0x…` (after the write: SMI enables 0) |
| (b) controller init | per controller `hcs1= hcs2= hcc1= csz= ppc= msi= msix= fw-state=`; `chosen live state=running|halted|host-system-error usbcmd= usbsts= irq=msi|polled dma-arena=…`; `cmds=completed/submitted` |
| (d) port enumeration | live `port N connected enabled= speed= pls= raw=`; `hub slot= root-port=`; `device port= slot= speed vid:pid class= driver=`; or a `port N enumeration failed: <step>` line above |
| (e) reports never delivered | `keyboard=bound reports= … | mouse=bound reports= …`, then the **input check**: `INPUT CHECK - press a key and move the mouse now (6 s)` → `input check RECEIVED|NOTHING RECEIVED: keyboard reports=N mouse reports=N`. Bound + NOTHING RECEIVED ⇒ the interrupt endpoint never completed; RECEIVED but no input at the desktop ⇒ the desktop's input path |

The block ends with a hold (up to 30 s) both when nothing binds and when the input check receives nothing.
Read it top to bottom:

| # | What the summary shows | Meaning | Next step |
|---|---|---|---|
| F1 | `xhci controllers=2 chosen=[1] reason=usb2-device-connected`, `device ... driver=hid-keyboard` / `hid-mouse`, `keyboard=bound mouse=bound`, and the desktop responds | Fixed: it was the controller choice (or settle/hub) | Done; note which BDF is which in the runbook |
| F2 | `chosen=[1]` (the 0x14 controller), its census row shows `ccs=` with ports, but `no device enumerated` and a `port N enumeration failed: <reason>` line above | The controller is right; enumeration fails. The reason names the step: `port reset timeout`, `enable-slot: no completion`, `address-device refused` | `no completion` with `irqs=0 events=0` ⇒ the event ring is not being written (DMA or the ring setup); `address-device refused` ⇒ slot-context layout (CSZ is printed) |
| F3 | Census `ccs=-` on **every** controller, `no root port reports a connection` | Nothing is seen at the port: the adapter/hub is unpowered, or the port the device is on belongs to a controller we did not bring up | Try the **USB-A port** (south xHCI by construction). If USB-A works and USB-C does not, the USB-C USB 2 pins are not on the chosen controller on this board |
| F4 | `reason=most-usb2-ports` (no connection seen at census) and `reconnected=0/0`, but a `usb-hotplug: port N connected ... enumerating` line during the hold | The device connected late (after the census); the port service caught it | Expected with hubs/adapters; F1 if it binds |
| F5 | `xhci: hub slot=... marked by configure-endpoint ... code=<not 1>` | The controller refused the hub slot update | The code names it (5 = TRB error, 17 = parameter error); hub path needs work, a direct plug should still work |
| F6 | `xhci: hub slot=... port=N reset did not enable child` or `hub ... port status read failed - hub retired` | Hub found, child reset failed | Plug the device directly (no hub) to split hub vs device |
| F7 | `device ... driver=none` for the keyboard/mouse | Enumerated but not claimed: not a boot-protocol HID interface (`class=` shows it), or a composite device claimed whole by the other driver | The vid:pid and class print; a gaming mouse with a keyboard interface binds as `hid-keyboard` (known limit) |
| F8 | `handoff=forced` | Firmware never released the controller; we took it and turned its SMIs off | Informational; if F2 follows, firmware SMM may still be interfering |
| F9b | `reason=AMBIGUOUS-protocols-unknown` and a census row with `protocols=UNKNOWN` | No controller showed a USB 2 device and the protocol ranges could not be read; the chosen controller is a fallback, not a known match | Plug the keyboard in **before** power-on (a connected port's speed then decides) and photograph the census rows |
| F9 | Census row shows `D3->D0` or `unreadable` | The controller was powered down; we woke it (or could not read it) | `unreadable` on the 0x14 controller ⇒ BAR/decode problem, report the row |
| F11 | `keyboard=bound` / `mouse=bound`, `input check NOTHING RECEIVED` after typing during the check | The device bound but its interrupt endpoint never completed a transfer | `irq=polled` rules out a missed interrupt (every pump drains the event ring); look at `ep-errors=` and the endpoint `configure slot=… interval=…` line above |
| F12 | `input check RECEIVED`, but the desktop does not respond | The kernel receives input; the desktop's input path does not consume it | An Atrium/WM lane, not USB |
| F10 | No `usb-summary` block at all | Boot never reached the desktop step, or `xhci-silicon` is not in the image | The last lines on screen say where boot stopped |

Every line is printed with the boot console's 2× glyphs; the hold keeps it on screen up to 30 s.

## 4. Governance fault-containment (I-14, connectivity/driver rule)

- **(a) Lock isolation.** The ISR is unchanged (`handle_interrupt` → `drain_events`, `IrqSafeMutex` + atomics).
  The new code runs in thread context only: the census/handoff at boot and the port service thread. Its
  locks are `spin::Mutex` statics private to the USB modules (never taken from an ISR), plus the existing
  `CONTROLLER`/`HUBS` `SwitchMutex`es ordered at `USB_XHCI` (above every AI-governance level). No
  AI-governance lock is reachable: grep `INFLIGHT|RECENCY|ARENA|TABLE` in `xhci_silicon*.rs`,
  `xhci_hotplug.rs` — none.
- **(b) Liveness.** The port service sleeps 25 ticks between passes; each pass is bounded (max 255 root
  ports, 15 children per hub, 3 attempts per connection, every wait budgeted). A chattering port costs at
  most 3 enumeration attempts per connect/disconnect cycle; a hub whose status reads fail is retired.
  USB degrades, governance does not.
- **(c) Memory isolation.** No new DMA path. Census reads MMIO only and grants **no** bus mastering to the
  controllers it does not choose. Configure Endpoint uses the hub slot's existing arena-fenced input context.
  Every DMA address is still a `FencedDmaAddr`.

## 5. Pitfall-bank consultation (`docs/concurrency-pitfalls.md`)

| Class | This change | Guard / discriminator |
|---|---|---|
| CP-37 / CP-56 (mixed IF on a spinlock) | Touches the HID `PRIMARY` swap | The swap keeps the `without_interrupts` + `IrqSafeMutex` discipline; the leaked old device is forgotten outside the lock. New `spin::Mutex` statics are never taken from an ISR, so IF is uniformly 1 |
| CP-58 (periodic rescuer hides a missing waker) | **Introduces a poller on purpose** | Polling PORTSC is the mechanism, not a rescue: port-change events were never consumed (counted and dropped). Named so a future event-driven path does not leave the poll in as a hidden rescuer |
| CP-59 (hidden spinner keeps a CPU out of idle) | The port service thread | With `wait-multiplex` it parks (`sleep_ticks`); without it, it yields until the tick moves, the same as the boot path's own pre-multiplex loop, which already spins on the BSP in desktop builds. Named |
| Commands/EP0 from two threads | New thread issues commands | All enumeration runs on the one service thread after boot; the boot scan finishes before it starts; HID pumps use routed endpoints only (no `PENDING` consumer) |
| Use-after-free of DMA contexts on detach | New detach path | Nothing is dropped on detach: replaced HID devices are `mem::forget`, retired hubs kept in `RETIRED_HUBS` |

No new pitfall class is added; CP-58 and CP-59 apply as named.

## 6. Not in this branch (known limits)

- **One controller at a time.** The choice brings up the controller where input appears. SuperSpeed devices
  on the USB-C ports (TCSS xHCI) are not driven; doing both needs the driver's global state per controller
  (a refactor of `xhci.rs`), plus the Type-C mux, which firmware/PMC may or may not have set.
- **No teardown.** No Disable Slot: each re-plug uses a new slot (the controller enables at most 16 here).
- **Composite single-slot keyboard+mouse devices** are claimed whole by one HID driver (pre-existing).
- **The internal keyboard and touchpad** are I2C/THC HID, not USB (NOTE-2026-10-21 §4b).

## 7. Evidence on QEMU (this branch)

| Check | Result |
|---|---|
| `cargo test -p xhci-silicon-logic-tests` | 18/18 (ext-cap walk incl. window + loop bounds, the choice table incl. the Zenbook case, handoff/SMI/PMCSR writes, hub slot context, port decisions) |
| `cargo test -p fb-progress-logic-tests` | 7/7 (new: every USB-summary character has a glyph) |
| `usb_xhci_select` (xHCIs at 0x0d and 0x14, keyboard on 0x14, mouse behind a hub) | PASS: `chose [1] 00:14.0 reason=usb2-device-connected`, hub `marked by configure-endpoint ... code=1`, both bound |
| `+ neo-test-usb-xhci-select-control` (scan-order choice) | FAIL `hid_not_bound`, after the evidence hold |
| `usb_hotplug` (plug root port, type, unplug, re-plug behind a hub, type) | PASS: both plugs enumerated by the service, both keys delivered |
| `+ neo-test-usb-hotplug-control` (no service) | FAIL `hotplug_not_bound` |
| `fb_progress_console`, `fb_panic_holding_serial` | PASS (font change) |
| Kill-switch: `.text` of `product`, default and `usb-hid-mouse` kernels vs `main` | byte-identical |
| Desktop ISO (`desktop,fb-progress-console,lapic-timer`) in QEMU, no input device | summary printed and held; a keyboard hot-added during the hold is enumerated by the service, ends the hold, desktop starts |

## 8. Review round 1 (Chad, Codex GPT-6-Luna high, BOUNCE at `6f09e826`) — what changed

RETURN: `Plans/RETURN-2026-10-22-zenbook-usb-input-review-chad.md` at `b0f9c314`. Items 2–5, 8, 9, 11 passed.

| # | Finding | Fix | Where | KAT |
|---|---|---|---|---|
| 1 | With the second controller's Supported Protocol chain absent/truncated, `choose` fell back to the first readable controller, which can be the SuperSpeed-only one | (a) a connected port with no protocol range counts as a USB 2 device from its PORTSC speed ID (1 full, 2 low, 3 high; xHCI §7.2.1 defaults), so a keyboard still decides the choice; (b) new outcome **`AMBIGUOUS-protocols-unknown`**: with no USB 2 device and no known USB 2 port, a controller whose ranges are unknown is chosen over one **known** to be SuperSpeed-only; (c) the census row prints `protocols=UNKNOWN` and the reason is on screen. The old `first-readable` now means "every range known, no USB 2 port anywhere" and is named `no-usb2-port-anywhere` | `xhci_silicon_logic.rs::{choose, port_holds_usb2_device, protocols_known}`, `xhci_silicon.rs::probe` | `unknown_protocols_beat_a_known_superspeed_only_controller`, `port_speed_identifies_usb2_devices_without_protocol_ranges`, `protocols_known_needs_a_complete_walk_with_a_range` |
| 2 | MTT hard-coded false | The hub's device descriptor is read; a high-speed hub with bDeviceProtocol 2 gets SET_INTERFACE(0, alternate 1) and **MTT only if that succeeds** (a multi-TT hub runs single-TT until switched, USB 2.0 §11.23.1, so MTT=0 was correct for an unswitched hub; now it is switched). Logged as `mtt=` | `xhci_silicon.rs::{select_multi_tt, mark_slot_as_hub}`, `wants_multi_tt` | `multi_tt_only_for_a_high_speed_protocol_2_hub` |
| 3 | Hub-child "attached" came from PORT_ENABLE, so an enabled child whose Enable Slot/Address Device failed was never retried | An explicit **addressed set** of finished enumerations by (root port, route string), recorded at the end of `address_and_describe` (after every descriptor read) and of `enumerate_hub`; a child is attached iff its route is in the set. Detach removes the subtree (a root detach, the whole root port) | `xhci_silicon_logic.rs::{AddressedSet, route_is_under}`, `xhci_hotplug.rs::{note_addressed, service_hubs}`, cfg-gated hooks in `xhci.rs` | `route_subtrees`, `addressed_set_insert_remove_and_bound`, `an_enabled_child_whose_enumeration_fails_is_retried` |
| 4 | Extra glyphs compiled with `fb-progress-console` alone | Gated on `fb-progress-console` **and** `xhci-silicon` | `fb/font.rs` | `usb_summary_characters_all_have_glyphs` (test crate enables both) |

**Why not bring up every candidate controller.** The driver's controller state is global and single: `CONTROLLER`, the
event ring (`EVENT`/`PENDING`), the endpoint routes, the published op/runtime bases, one MSI vector, `HUBS`, and
`ClaimedDevice`/`EndpointPipe` carry no controller index. Driving two controllers means making all of that
per-controller and threading the index through every enumeration and class-driver call: a refactor of most of
`xhci.rs` and every class driver, with its own review and gate, not a cheap or contained change. The ambiguity it
would remove is narrow (no USB 2 device at census **and** no readable protocol ranges), it is now named on screen,
and the speed-ID fallback covers the case that matters here (a keyboard plugged in at boot). It is the first item
of §6's follow-ups.

**Kill-switch, re-measured** (each build checked to succeed; `.text` vs `main` `4e9ea28c`): identical for default,
`usb-hid-mouse`, `fb-progress-console`, `fb-progress-console,lapic-timer`, `product` and
`product,fb-progress-console,lapic-timer`.

**QEMU at the fix head:** `usb_xhci_select` PASS (`hub ... mtt=false`, QEMU's hub is full-speed), `usb_hotplug`
PASS, both controls FAIL as named. Host KATs: `xhci-silicon-logic-tests` 25/25, `fb-progress-logic-tests` 7/7.

