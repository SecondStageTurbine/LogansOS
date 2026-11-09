<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 247: PR #30 hot-unplug and PR #23 changed-code re-check

Reviewed heads: #30 `9c97b0799dda196d7204ff5f4b3cb42a3370e127`; #23 `600f45a923b522fb56cf880422950f3a9f67105c`.

## PR #30: hot-unplug

- [x] PASS — Earlier hot-unplug FAIL is fixed. Root-port removal calls `detach_subtree` (`kernel/src/drivers/usb/xhci_hotplug.rs:148-154`); it removes the bound device and calls its driver's detach callback (`kernel/src/drivers/usb/mod.rs:184-206`). MSC marks `gone`, unregisters the block registry slot, waits for the in-flight transport lock, and retires the transport (`kernel/src/drivers/usb/msc.rs:584-610`). Reads check `gone` during command waits and after the final command, returning `NoDevice` (`kernel/src/drivers/usb/msc.rs:139-154, 315-320, 340-345`); the registry slot becomes a `DetachedDevice` returning `NoDevice` (`kernel/src/drivers/block/mod.rs:678-733`). Concrete input: unplug the stick during a block read, then look up the former registry index and retry the old handle; the read fails and the slot returns `NoDevice`.
- New consequence of the fix: detached disks and their DMA transports are intentionally retained in `RETIRED_DISKS` to avoid freeing controller-visible buffers (`kernel/src/drivers/usb/msc.rs:577-582, 601-610`).

## PR #23: changed code

- [x] PASS — CBW and CSW layouts, byte order, and literal fixtures still agree with the 31-byte / 13-byte formats (`kernel/src/drivers/usb/msc_logic.rs:52-77, 105-125`; `tools/usb-msc-logic-tests/src/lib.rs:23-64, 96-148`).
- [x] PASS — CSW signature, tag, excessive residue, invalid status, and invalid lengths are rejected (`kernel/src/drivers/usb/msc_logic.rs:105-125`; `tools/usb-msc-logic-tests/src/lib.rs:96-148`).
- [x] PASS — Capacity replies are length-checked before indexing; zero block size and `u64::MAX` last-LBA overflow are rejected. READ(10) chunks remain capped at 128 blocks with checked 32-bit LBA bounds (`kernel/src/drivers/usb/msc_logic.rs:197-225, 302-310`; `tools/usb-msc-logic-tests/src/lib.rs:150-196, 245-255`).
- [x] PASS — BOT reset recovery remains class reset, clear bulk-IN halt, then clear bulk-OUT halt. Data-stage stalls still proceed to CSW; a stalled CSW is retried once (`kernel/src/drivers/usb/msc_logic.rs:336-344, 401-420`; `tools/usb-msc-logic-tests/src/lib.rs:389-430`).
- [ ] FAIL — The requested Linux `usb-storage` trap-mining section and source-based checks still do not exist. The test header names BOT/SBC/SPC specifications and `usbd-storage` 3.0.0 as its cross-read (`tools/usb-msc-logic-tests/src/lib.rs:6-12`); the logic file only mentions Linux quirks and spin-up behavior (`kernel/src/drivers/usb/msc_logic.rs:128-133, 270-280`). Concrete omitted case: READ CAPACITY(10) reports last LBA 7 and 512-byte blocks, but READ(10) of LBA 7 returns CHECK CONDITION due to an off-by-one capacity quirk; there is no Linux-derived trap or discriminator for it. The capacity fixture only checks last-LBA-plus-one decoding (`tools/usb-msc-logic-tests/src/lib.rs:150-177`).
- New code note: the changed logic adds a six-opcode allowlist and bounded handling for UNIT ATTENTION and spin-up; unsupported START STOP UNIT requests cause binding to fail instead of sending the command (`kernel/src/drivers/usb/msc_logic.rs:128-139, 260-294`; `tools/usb-msc-logic-tests/src/lib.rs:564-638`).
