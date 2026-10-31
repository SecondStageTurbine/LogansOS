<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Zenbook USB input narrow re-check (Chad, PR #93)

**Reviewed head:** `a4f204649b958e6aa7833acc87f7863dc4d98aee`
**Fix diff:** `6f09e826009fd824b10419da4f06b93fa1937f8b..a4f204649b958e6aa7833acc87f7863dc4d98aee`, `kernel` and `tools` only.
**Verdict:** BOUNCE — one remaining controller-choice counterexample.

## 1. Controller choice — STILL OPEN (medium)

The originally reported absent/out-of-window chain cases are fixed:
`kernel/src/drivers/usb/xhci_silicon.rs:249` feeds a connected port's protocol
and PORTSC speed into `port_holds_usb2_device`; the no-range branch at
`kernel/src/drivers/usb/xhci_silicon_logic.rs:165` recognizes IDs 1/2/3.
`choose` prioritizes that evidence at `:213`, then known USB 2 ports, then a
readable unknown candidate at `:225`. The name at `:197` is exactly
`AMBIGUOUS-protocols-unknown`.

However, the new completeness predicate at
`kernel/src/drivers/usb/xhci_silicon_logic.rs:153` accepts a capability walk
whose protocol storage was truncated. The walker stores only eight ranges
(`MAX_PROTOCOLS`, `:25`; storage guard, `:112`), silently skips later ranges,
and can reach the end with `stop=None`. Thus the candidate is incorrectly
classified as known, and `choose` reaches `FirstReadable` at `:228`.

**Concrete input:** two readable controllers, no connected devices. Controller
0 is genuinely SuperSpeed-only. Controller 1 has nine Supported Protocol
capabilities: the first eight are USB 3 ranges covering ports 1 through 8,
and the ninth is a USB 2 range covering port 9. The complete in-window walk
stores eight ranges and discards the USB 2 range. Both candidates report
zero USB 2 ports and `protocols_known=true`. Controller 0 wins with
`no-usb2-port-anywhere`, although controller 1 can host a USB 2 device.

I reproduced this against the reviewed kernel module with a standalone
Rust host probe. Register recipe: HCCPARAMS1=`0x00400000`; window=`0x1000`;
capabilities at `0x100 + 16*i`, i=0..8. Header is `0x03000402` for i=0..7,
then `0x02000002` for i=8; DWORD2 is `0x100 | (i+1)` in each. Candidate 0
is `{readable:true, protocols_known:true, usb2_ports:0,
usb2_connected:0, connected:0}`; candidate 1 uses the walk's counts and
`protocols_known(&caps)`. Actual output:

```text
ranges=8 stop=None known=true usb2_ports=0 choice=0 reason=no-usb2-port-anywhere
```

Required: treat discarded protocol ranges as incomplete (or retain/count
all ranges needed for selection), so this input cannot silently select the
known SuperSpeed-only controller. Add this case to the host KATs.

## 2. Multi-TT — CLOSED

For the silicon Configure Endpoint path, `select_multi_tt` reads
bDeviceProtocol from device-descriptor byte 6 at
`kernel/src/drivers/usb/xhci_silicon.rs:577`.
`kernel/src/drivers/usb/xhci_silicon_logic.rs:322` requires high speed and
protocol 2. `xhci_silicon.rs:582` sends SET_INTERFACE with bmRequestType
`0x01`, request 11, alternate 1, interface 0, and returns only its success
at `:592`. That boolean reaches the slot builder at `:523`.
`xhci_silicon_logic.rs:305` sets MTT only when both high speed and the
successful switch are true; `:308` clears it otherwise. Descriptor-read
failure also returns false. The old xHCI <=0.95 compatibility path remains
outside the new Configure Endpoint path (`xhci_silicon.rs:512`).

## 3. Hub-child retries and addressed subtree removal — CLOSED

`kernel/src/drivers/usb/xhci.rs:1515` records successful non-hub enumeration
after all descriptor reads, before the infallible offer; `:1689` records a
hub at the successful end of `enumerate_hub`.
`kernel/src/drivers/usb/xhci_hotplug.rs:233` derives attached from ADDRESSED,
not PORT_ENABLE. Failures persist their attempt count at `:273`;
`kernel/src/drivers/usb/xhci_silicon_logic.rs:376` permits attempts below
three and `:391` increments failures. Therefore Enable Slot/Address Device
failure after a successful reset no longer suppresses attempts 2 and 3.
A disconnect resets attempts via `:395`/`:396`.

Root detach removes the whole root's addressed subtree at
`xhci_hotplug.rs:151`; child detach removes its addressed subtree at `:246`.
`xhci_silicon_logic.rs:443` matches the root port and removes every matching
route prefix; `:474` compares low-to-high route nibbles. For example,
removing `(root=2, route=0x1)` removes `0x1` and `0x21`, preserves sibling
`0x2`, and preserves all entries on root 3. This is logical addressed-state
removal; the lane's documented no-Disable-Slot/resource-retention limit
still applies.

## 4. EXTRA_GLYPHS kill-switch — CLOSED

Both the table at `kernel/src/drivers/fb/font.rs:503` and lookup at `:635`
require `all(feature = "fb-progress-console", feature = "xhci-silicon")`.
The font test crate enables both by default. The extra table and lookup
are excluded when either feature is absent.

## New findings in the fix diff and validation

The protocol-storage completeness gap in item 1 is the only additional
finding from this narrow fix-diff review. No other new finding identified.

- Ran `cargo test -p xhci-silicon-logic-tests -p fb-progress-logic-tests`:
  25/25 xHCI tests and 7/7 framebuffer tests passed; doc tests passed.
- Ran the standalone nine-range probe described above; it reproduced the
  remaining selection problem without changing tracked source or tests.
- `git diff --check` passed.
- RAG health was healthy (Ollama and Qdrant true). The supplied exact files
  and symbols were read directly; semantic-index snippets were not used as
  review authority.
- Did not rerun QEMU or kernel `.text` comparisons; the author's results in
  NOTE section 8 were read, not independently reproduced here.
- Ran `cargo clean` in this worktree after the host builds, as dispatched.
- Existing `.gitignore` and `.ignore` edits were left untouched.
