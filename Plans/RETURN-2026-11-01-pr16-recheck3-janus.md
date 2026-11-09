<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 262 / Janus: PR #16, re-check of relay 253 (item 1 and the response cast).

Reviewed fetched head `df8548ab99475942df298cc5279664a4e0564fbe`
(`git fetch origin +pull/16/head:pr16`), the relay 253 RETURN, and the
latest `PM:` comment (2026-11-01 21:50:06 UTC). All source line references
below refer to that head. RETURN branch `codex/pr16-recheck3-janus` starts
at `origin/main`, `f2210a32a0f2e9131dff098fe3ed4a25ccf91594`.
Overall: FAIL (item 1).

- FAIL — Earlier item 1, firmware backing validation remains incomplete.
  `kernel/src/boot_time_logic.rs:209`, enforced at `:263`, now rejects
  the earlier type-0, attribute-0 reserved-hardware input, and the UC-only
  variant. The regression KAT at
  `tools/boot-time-logic-tests/src/lib.rs:490` passes. However, accepting
  reserved memory solely because it supports write-back caching still
  does not establish that the OS can read it. UEFI defines the attributes
  as capabilities, which need not describe the current configuration,
  and defines WB as a cache-policy capability
  ([GetMemoryMap attribute definitions](https://github.com/UEFI/UEFI-Specification-Release/blob/main/source/07_Services_Boot_Services.rst#L1540-L1585)).
  It separately requires hardware-backed regions withheld from OS access
  to be described as reserved, without a WB exception
  ([GetMemoryMap description](https://github.com/UEFI/UEFI-Specification-Release/blob/main/source/07_Services_Boot_Services.rst#L1635)).
  Concrete input: a 48-byte descriptor, type 0, physical start
  `0x6b920000`, one page, attribute `0x8`, other fields zero, representing
  cacheable RAM reserved against OS access; a checksum-correct FPDT points
  its FBPT there at physical width 36. A host probe of the unchanged PR
  logic returns `[(0x6b920000, 0x1000)]`, and
  `in_firmware_memory(0x6b920000, 8, regions)` returns true. Attribute
  `0x2008` (WB plus read-protection capability) is also accepted; RP is
  itself a capability, so this does not claim that bit alone means active
  read protection. The point is that neither descriptor establishes the
  actual accessibility. `kernel/src/acpi/mod.rs:500` then authorizes the
  header read at `:505`; root/FPDT discovery uses the same predicate at
  `:495`. This does not require firmware to lie about cacheability:
  reserved RAM can honestly support WB while remaining unavailable to
  the OS. Corroborate readable firmware backing independently or leave
  ambiguous reserved ranges unavailable. This is a source/parser result,
  not an observed hardware fault. The physical-width and bounded-table
  fixes remain in place.

- PASS — Earlier new bug, the response cast and extra HHDM conversion.
  `kernel/src/boot_time.rs:40` coerces the wrapper to
  `&limine::request::EfiMemmapRespData` before reading fields at `:43`;
  `kernel/src/boot_time_logic.rs:224` therefore starts after the revision
  word. The actual locked limine 0.6.3 response KAT at
  `tools/boot-time-logic-tests/src/lib.rs:519` returns
  `(0xffff800000100000, 4800, 48)` and demonstrates the old incorrect
  tuple. `kernel/src/boot_time.rs:47` uses `efi_memmap_virt`, whose branch
  at `kernel/src/boot_time_logic.rs:235` preserves an existing HHDM
  address; canonical-address and endpoint-mapping checks precede the
  slice at `kernel/src/boot_time.rs:64`. The HHDM/physical-address KAT at
  `tools/boot-time-logic-tests/src/lib.rs:539` passes.

- PASS — New: no additional actionable regression found in this narrow pass; the remaining reserved-memory issue is earlier item 1.

Validation: all 18 `cargo test -p boot-time-logic-tests --target
x86_64-unknown-linux-gnu --offline` tests passed in a detached worktree
at the reviewed head. A temporary host probe imported its unchanged
`boot_time_logic.rs`, confirmed attribute 0 and UC are rejected, and
confirmed WB and WB|RP are accepted as above.
`git diff origin/main...pr16 --check` passed. No kernel build, QEMU,
physical boot, hostile-firmware boot or full CI gate was run here; the
latest PM's gate claim was not independently re-run. Items 2 and 4 were
outside this re-check. This RETURN changes no implementation.
