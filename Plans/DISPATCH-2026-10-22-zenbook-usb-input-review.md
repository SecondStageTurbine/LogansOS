<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: review of the Zenbook USB input lane (xHCI silicon bring-up + port service) → Codex reviewer

**MODEL: GPT-6-Luna — EFFORT: high.** Contained code: one subsystem (the USB host stack), no SMP core and
no new syscall (CLAUDE.md routing table).

**PR:** the `claude/zenbook-usb-input` PR, from `main` `4e9ea28c`. **Design and diagnosis:**
`Plans/NOTE-2026-10-22-zenbook-usb-input.md` on the branch.

```
git fetch origin
git show origin/claude/zenbook-usb-input:Plans/DISPATCH-2026-10-22-zenbook-usb-input-review.md
git show origin/claude/zenbook-usb-input:Plans/NOTE-2026-10-22-zenbook-usb-input.md
git diff origin/main origin/claude/zenbook-usb-input -- kernel tools
```

WORKTREE: `E:\AI Automation\workers\<reviewer>` (yours alone; never another coder's folder, never the repo root)
BRANCH: `codex/zenbook-usb-input-review-<reviewer>` (`git fetch origin && git checkout -B <branch> origin/claude/zenbook-usb-input`)

## What the branch does

Image A booted to the desktop on the Zenbook, but a USB keyboard and mouse on a USB-C port did nothing.
The kernel brought up only the first xHCI in PCI order, which on Intel parts since Tiger Lake is the Type-C
subsystem's SuperSpeed-only controller (device 0x0d); the USB 2 pins, where a keyboard or mouse appears,
are on the south xHCI (device 0x14). The branch adds, behind `xhci-silicon` and `usb-hotplug` (both in
`desktop`):
- a census of every xHCI and a choice of controller;
- D0, a forced BIOS handoff with firmware SMIs turned off, root-port power and a settle wait;
- hubs marked by Configure Endpoint, and a power-good wait before reading hub ports;
- a kernel thread that re-reads root and hub ports every 250 ms and enumerates late connections;
- the `usb-summary` block, held on the boot console when no input device binds.

Files: `kernel/src/drivers/usb/xhci_silicon_logic.rs` (pure), `xhci_silicon.rs`, `xhci_hotplug.rs`, the
cfg-gated hooks in `xhci.rs`, `mod.rs`, `hid_keyboard.rs`, `hid_mouse.rs`, `pci.rs`, `main.rs`, and the
font table in `drivers/fb/font.rs`.

## Check (pass/fail each, with the enforcing `file:line`; a concrete input for any fail)

1. **Controller choice.** `xhci_silicon_logic::choose`: with one controller it always returns it; with
   several it prefers a USB 2 device, then the most USB 2 ports, then the first readable. Is there an input
   where it picks a controller a keyboard or mouse cannot appear on, while another controller could?
2. **Capability walk.** `walk_ext_caps`: no read at or past the mapped window; the walk ends on a loop or
   corrupt chain; Supported Protocol port ranges are decoded per xHCI §7.2 (DWORD2 port offset and count).
3. **Census side effects.** `xhci_silicon::probe`: on controllers it does **not** choose, it only reads
   registers, may move D3hot → D0 and enables memory decode. Confirm it never enables bus mastering or
   writes an xHCI register there.
4. **Handoff.** `xhci_silicon::legacy_handoff`: ownership requested, forced on timeout, then USBLEGCTLSTS
   written with SMI enables 0, RsvdP preserved, RW1C events written 1 (`legctl_disable_smis`, xHCI
   §7.1.2). Are the bit positions right?
5. **Port power and settle.** `power_and_settle_ports`: PP set without writing 1 to PED or any RW1C bit;
   the waits are bounded.
6. **Hub slot update.** `xhci_silicon::mark_slot_as_hub`: on xHCI > 0.95 a Configure Endpoint with only
   A0 set, the slot context copied from the output context with Hub, Number of Ports and TTT set
   (`hub_slot_context`). Is anything in the copied slot context invalid as input (e.g. Slot State or
   USB Device Address in DWORD3, which this leaves zero)? Does the TTT encoding match USB 2.0 §11.23.2.1?
7. **Port service.** `xhci_hotplug::service_ports` / `service_hubs`:
   - every enumeration runs on the one service thread, after the boot scan, so no two threads issue
     commands or EP0 transfers at once (check every other caller of `submit_command`, `wait_event` and
     `control_read` in the image A feature set);
   - no lock is held across an enumeration (`HUBS` is taken out and put back; a child hub pushed during the
     pass is kept);
   - a port that keeps failing stops after 3 attempts until it disconnects;
   - a detached root port retires the hubs behind it, and nothing referenced by the controller is freed
     (HID devices replaced on re-plug are `mem::forget`).
8. **Hub child enumeration in the service.** `enumerate_hub_child` matches `enumerate_hub`'s per-port
   body (reset, wait for connected + enabled + reset done, Enable Slot, Address Device with route string
   and TT). Any difference that matters?
9. **Evidence hold.** `usb::input_evidence_hold_for`: bounded (5 s + 30 s in image A), ends early on a
   bind, lets the port-service thread run while it waits (`nap_ticks` parks or yields).
10. **Kill-switch.** With `xhci-silicon` and `usb-hotplug` off, is every change compiled out? (The author
    compared `.text` of the `product`, default and `usb-hid-mouse` kernels with `main`: identical.)
11. **I-14.** No AI-governance lock reachable from the new code; no new DMA path; the service's work per
    pass is bounded (NOTE §4).

Optional runs at `NEO_SMP=4`:
- `cargo test -p xhci-silicon-logic-tests -p fb-progress-logic-tests`;
- `cargo xtask neo-test usb_xhci_select` and `usb_hotplug` should PASS;
- with `NEO_TEST_EXTRA_FEATURES=neo-test-usb-xhci-select-control` → FAIL `hid_not_bound`;
- with `NEO_TEST_EXTRA_FEATURES=neo-test-usb-hotplug-control` → FAIL `hotplug_not_bound`.

RAG (semantic code search) is available for this project. Use it for conceptual "where is X done?"
questions; grep still wins for exact symbols. Health-check first with
`Invoke-RestMethod http://127.0.0.1:8404/health`, expecting `{"status":"healthy",...}`. If it is down or
degraded, the PL manages it: note that in your RETURN and continue with direct reads.
Optional self-serve start: `powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"`, then
`cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest`.
Example query: `$body=@{query='xHCI port scan enumerate_port hub child'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-22-zenbook-usb-input-review-<reviewer>.md` to your branch and push it.
Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- items 1–11, each pass/fail with `file:line`;
- each finding with severity, `file:line` and a concrete input.

Paste the pushed SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree; never delete
the worktree.
