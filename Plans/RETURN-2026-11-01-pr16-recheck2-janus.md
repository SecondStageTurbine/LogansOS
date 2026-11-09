<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 253 / Janus: PR #16, item 1 only (firmware backing validation).

Reviewed fetched head `2395904a2d1401cf4dac377818a9b4108afe87b8`
(`git fetch origin +pull/16/head:pr16`), the relay 243 RETURN, and the
latest `PM:` comment (2026-11-01 13:55:58 UTC). All source line references
below refer to that head. RETURN branch `codex/pr16-recheck2-janus` starts
at `origin/main`, `65b42f3882943d237f44f95e6b9054e19b7c1b4e`.
Overall: FAIL.

- FAIL — Earlier item 1, firmware backing validation is still incomplete.
  The raw UEFI classification fixes the specific Limine conflation:
  `kernel/src/boot_time_logic.rs:196` excludes types 11 (MMIO), 12 (MMIO
  port space) and 8 (unusable), enforced at `:218`. The original
  `0xfee00000` input with a type-11 descriptor is now rejected by
  `in_firmware_memory`; the new KAT at
  `tools/boot-time-logic-tests/src/lib.rs:461` passes. However, the same
  allowlist accepts every UEFI type-0 reserved descriptor as firmware RAM.
  Type 0 does not establish readable RAM: UEFI GetMemoryMap explicitly
  uses it for hardware-backed regions the OS must not access
  ([UEFI specification source](https://github.com/UEFI/UEFI-Specification-Release/blob/main/source/07_Services_Boot_Services.rst#L1515)).
  Concrete parser input: one 48-byte descriptor with type 0, physical
  start `0xfee00000`, page count 1, and all other fields zero, describing
  reserved hardware; a checksum-correct FPDT points its FBPT there with
  physical width 36. `efi_firmware_regions` returns
  `[(0xfee00000, 0x1000)]`, and
  `in_firmware_memory(0xfee00000, 8, regions)` returns true (host probe).
  Thus the remaining backing distinction is still missing. Once capture
  works, `kernel/src/acpi/mod.rs:500` would accept that classification
  before `phys_slice` at `:505`; root/FPDT discovery uses it too at `:495`.
  Corroborate reserved RAM backing independently, or leave ambiguous
  reserved ranges unavailable. The capture regression below currently
  masks this unsafe classification in normal revision-0 boots; this is
  a source/parser result, not an observed MMIO read or hardware fault.
  The previously fixed physical-width and bounded root/FPDT guards remain
  at `kernel/src/boot_time_logic.rs:181` and `:184`, and
  `kernel/src/acpi/mod.rs:154`, `:160`, `:183`, `:300`, and `:306`.

- FAIL — New: `kernel/src/boot_time.rs:40` casts `&Response<EfiMemmapRespData>` rather than its dereferenced data, so `:41` reads `(revision, memmap, memmap_size)` as `(phys, size, desc_size)`; concrete normal response words `[0, 0xffff800000100000, 48, 48, 1]` take the empty-map return at `:42`, losing valid firmware timestamps (reproduced with the actual locked `limine` 0.6.3 response type); additionally the map pointer is already HHDM according to the [Limine v8 protocol](https://github.com/limine-bootloader/limine/blob/v8.x/PROTOCOL.md#L1149-L1163), so the extra physical-to-virtual conversion at `:45` also needs correction when fixing the response-field offset.

Validation: all 15 `cargo test -p boot-time-logic-tests --target
x86_64-unknown-linux-gnu --offline` tests passed on the fetched head.
A temporary host probe imported that head's unchanged pure logic and
locked `limine` 0.6.3, reproduced the response-layout failure, rejected
UEFI types 11/12/8, and accepted the type-0 reserved-hardware input above.
`git diff origin/main...pr16 --check` passed. No kernel build, QEMU,
physical boot, hostile-firmware boot or full CI gate was run here.
Items 2 and 4 were outside this re-check. This RETURN changes no implementation.
