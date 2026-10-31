<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook USB input review (chad)

**Reviewed head:** `6f09e826009fd824b10419da4f06b93fa1937f8b`

**Verdict:** BOUNCE

## Findings

### 1. Controller choice — FAIL (medium)

`kernel/src/drivers/usb/xhci_silicon_logic.rs:183-199` chooses the first readable controller when the census reports no USB 2 ports. A concrete input is two readable controllers where the first is SuperSpeed-only and the second has USB 2 ports, but the second controller's Supported Protocol capability chain is absent, truncated, or out of the mapped window, so both candidates reach `choose` with `usb2_ports = 0` and `usb2_connected = 0`. `FirstReadable` selects the SuperSpeed-only controller, while a keyboard can appear on the other controller. This fallback does not use port speed-ID information to recover from missing protocol ranges.

### 2. Capability walk — PASS

`kernel/src/drivers/usb/xhci_silicon_logic.rs:95-130` checks that a full 12-byte Supported Protocol structure fits before reading DWORD2, bounds the walk at 64 capabilities, and decodes DWORD2 bits 7:0 as port offset and 15:8 as count. A zero next pointer ends the walk; a looping chain stops at the capability bound. No read is issued when the next capability is at or beyond the mapped window.

### 3. Census side effects — PASS

`kernel/src/drivers/usb/xhci_silicon.rs:191-211` may move D3hot to D0 and enable PCI memory decode before reading the candidate's MMIO registers. It does not write xHCI registers or enable bus mastering. Bus mastering is enabled only by the selected controller's bring-up at `kernel/src/drivers/usb/xhci.rs:621-625`.

### 4. BIOS handoff — PASS

`kernel/src/drivers/usb/xhci_silicon_logic.rs:204-235` uses OS-owned bit 24 and BIOS-owned bit 16, preserves RsvdP bits, clears all five SMI enables, and writes ones to the RW1C status bits 31:29. `kernel/src/drivers/usb/xhci_silicon.rs:415-430` requests ownership, forces it after the 500 ms wait, then applies the USBLEGCTLSTS write.

### 5. Port power and settle — PASS

`kernel/src/drivers/usb/xhci_silicon.rs:447-472` powers ports with PP clear while clearing PED and all RW1C bits from the write value, waits 20 ms if it powered any ports, then bounds the reconnect wait to 300 ms.

### 6. Hub slot update — FAIL (medium)

The slot context update uses Configure Endpoint with only A0 set and copies output-context DWORDs 0–2 at `kernel/src/drivers/usb/xhci_silicon.rs:506-539`. The TTT extraction from `wHubCharacteristics[6:5]` is correctly encoded into DWORD2[17:16] by `kernel/src/drivers/usb/xhci_silicon_logic.rs:278-282`; leaving input-context DWORD3 zero is valid because that output-only state/address is not part of the input slot fields. However, the caller hardcodes `multi_tt = false` at `kernel/src/drivers/usb/xhci_silicon.rs:512-517`, so every high-speed multi-TT hub is configured without MTT. Concrete input: a high-speed hub whose interface descriptor has `bDeviceProtocol = 2` (multiple transaction translators); its slot context must set MTT, but this path clears it.

### 7. Port service — FAIL (medium)

The service runs after the boot scan on one thread (`kernel/src/main.rs:763-766`, `kernel/src/drivers/usb/mod.rs:298-307`); command and EP0 enumeration in the service is therefore serialized, and `service_hubs` takes HUBS out during enumeration and merges hubs pushed during the pass (`kernel/src/drivers/usb/xhci_hotplug.rs:186-189, 262-275`). Root and hub objects are retained on retirement, and replaced HID objects are forgotten (`xhci_hotplug.rs:158-183`; `hid_keyboard.rs:130-140`; `hid_mouse.rs:164-174`). Root-port failures are tracked up to three attempts.

Hub-child failure retry tracking is wrong. `kernel/src/drivers/usb/xhci_hotplug.rs:213-219` reconstructs `attached` from PORT_ENABLE, and `:245` stores the attempts. Concrete input: a child port resets successfully and becomes enabled, then Enable Slot or Address Device fails. The attempt count increments once, but the next pass sees PORT_ENABLE and sets `attached = true`; `port_action` then does not retry. This violates the three-attempt policy for a connected port whose enumeration keeps failing.

### 8. Hub child enumeration — PASS

`kernel/src/drivers/usb/xhci_hotplug.rs:283-337` resets the child, waits for connected + enabled + reset-done, enables a slot, appends the child port to the route, and sets the parent hub slot/port/speed for Address Device. This matches the per-port sequence in `kernel/src/drivers/usb/xhci.rs:1618-1671`. The service variant reports a control-read error immediately and uses a bounded 500 ms reset wait; those differences do not invalidate the sequence.

### 9. Evidence hold — PASS

`kernel/src/drivers/usb/mod.rs:341-360, 363-385, 438-451` bounds the initial wait, optional hold, and input check; waits in 10-tick increments via `nap_ticks`, which yields or sleeps at `:313-326`. Image A supplies 500, 3000, and 600 ticks (5 s, 30 s, and 6 s) at `kernel/src/main.rs:822-823`. The port-service thread can run during these waits.

### 10. Kill-switch — FAIL (low)

The USB implementation is feature-gated, but not every change is compiled out when `xhci-silicon` and `usb-hotplug` are off. `kernel/src/drivers/fb/font.rs:498-503, 631-639` gates the new glyph table and lookup only on `fb-progress-console`. Concrete feature input: build with `fb-progress-console` enabled and both USB features disabled; `EXTRA_GLYPHS` and the lookup loop remain compiled. The reported byte-identical comparisons do not cover that feature combination unless they included `fb-progress-console`.

### 11. I-14 — PASS

The census is MMIO-only and does not grant bus mastering to unchosen controllers (`kernel/src/drivers/usb/xhci_silicon.rs:209-211`; PCI decode helper at `kernel/src/drivers/pci.rs:721-732`). Hub Configure Endpoint uses the existing fenced input context (`xhci_silicon.rs:519-539`). Port service work is bounded by the xHCI root-port count and the capped 15-port hub descriptor, with at most three recorded attempts per connection (`xhci_hotplug.rs:102-105, 200-245`; `xhci_silicon_logic.rs:310-312`). No governance-lock path or new DMA path was found in the reviewed lane.

## Required changes before merge

1. Improve controller fallback when Supported Protocol ranges are unavailable, or fail closed with an explicit ambiguity outcome.
2. Populate the hub's multi-TT capability from its interface descriptor and pass it into the slot-context builder.
3. Track hub-child enumeration success separately from PORT_ENABLE so failures still receive the remaining retries.
4. Gate the extra glyphs on the USB lane feature(s), or narrow the kill-switch claim and comparison scope.
