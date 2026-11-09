<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #23 USB mass storage logic review

Reviewed head: `0de729f2b9a855b45fa554f9f72cecfd8140c9e0`

- [x] PASS — CBW and CSW layouts, byte order, and hand-laid fixtures match the specified 31-byte / 13-byte formats. Evidence: `kernel/src/drivers/usb/msc_logic.rs:52-77,105-125`; `tools/usb-msc-logic-tests/src/lib.rs:23-64,96-148`.
- [x] PASS — CSW signature, tag, residue greater than requested, and invalid status are rejected. Evidence: `kernel/src/drivers/usb/msc_logic.rs:105-125`; `tools/usb-msc-logic-tests/src/lib.rs:96-148`.
- [x] PASS — Capacity reply lengths are constrained before indexing; zero block size and `u64::MAX` last-LBA overflow are rejected. READ(10) chunks are capped at 128 blocks and checked for 32-bit LBA overflow. Evidence: `kernel/src/drivers/usb/msc_logic.rs:184-218,274-286`; `tools/usb-msc-logic-tests/src/lib.rs:150-196,245-255`.
- [x] PASS — BOT reset recovery runs class reset, clear bulk-IN halt, then clear bulk-OUT halt. A data-stage stall still proceeds to CSW; a stalled CSW is cleared and retried once. Evidence: `kernel/src/drivers/usb/msc_logic.rs:312-320,369-400`; `tools/usb-msc-logic-tests/src/lib.rs:370-417`.
- [ ] FAIL — The PR has no Linux `usb-storage` trap-mining section or source-based checks. Its test header instead says it cross-reads `usbd-storage` (device-side) for the byte oracle. Evidence: `tools/usb-msc-logic-tests/src/lib.rs:6-12`. Concrete omitted case: READ CAPACITY(10) reports last LBA 7 and 512-byte blocks, but a READ(10) of LBA 7 returns CHECK CONDITION because the device has an off-by-one capacity quirk; the PR contains no Linux-derived trap/discriminator for that behavior.
