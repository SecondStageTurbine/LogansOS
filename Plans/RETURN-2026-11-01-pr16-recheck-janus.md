<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 243 / Janus: narrow re-check of PR #16, earlier relay 232 items 1, 2 and 4.

Reviewed fetched head `e36af6b1b58571393f8175c17dda5e17ac6d289e` with
`git fetch origin +pull/16/head:pr16` and `git diff origin/main...pr16`.
Read the earlier RETURN and the latest `PM:` comment (2026-11-01 10:38 UTC),
including its item-by-item claims. All repository line references below are
to that fetched head. RETURN branch `codex/pr16-recheck-janus` starts at
`origin/main`, `40011b83` fetched for this relay. Overall: FAIL (item 1).

- FAIL — Earlier item 1, firmware backing validation remains incomplete.
  The original physical-width input is fixed: checked addition of the whole
  4096-byte maximum extent at `kernel/src/boot_time_logic.rs:181` and the
  limit comparison at `:184` reject `0xf_ffff_fffc` with a 36-bit width.
  The root header/body guards at `kernel/src/acpi/mod.rs:154` and `:160`,
  the 1-MiB cap at `:144`, child-signature guard at `:183`, and FPDT
  guards at `:300` and `:306` also close the reported oversized-root read.
  However, `kernel/src/acpi/mod.rs:493` accepts every `MEMMAP_RESERVED`
  region as readable firmware memory. Reserved does not establish readable
  RAM: Limine's UEFI conversion explicitly maps `EfiMemoryMappedIO`,
  `EfiMemoryMappedIOPortSpace` and `EfiUnusableMemory` to that type
  ([upstream pmm.s2.c](https://github.com/limine-bootloader/limine/blob/v8.x/common/mm/pmm.s2.c#L344-L354)).
  Concrete input: a checksum-correct FPDT whose FBPT pointer is
  `0xfee00000`, physical width 36, and a boot-map entry
  `(base=0xfee00000, length=0x1000, type=MEMMAP_RESERVED)` representing
  MMIO. Both `fbpt_address` and `in_firmware_memory(address, 8, regions)`
  accept this input (host probe reproduced). The read at
  `kernel/src/acpi/mod.rs:520` therefore touches MMIO before a bad FBPT
  signature can reject it; mapping it at `:80` does not establish safe
  backing. The same classification feeds root/FPDT discovery at `:510`.
  Distinguish backed reserved firmware RAM from MMIO/unusable regions,
  or return unavailable when that cannot be established, before any read.
  This is a source/probe result, not an observed hardware fault.

- PASS — Earlier item 2, checked copy-out. Length and user-range checks
  remain at `kernel/src/arch/x86_64/syscall.rs:7535` and `:7538`; the syscall
  now calls `Pmap::write_user_bytes` at `:7554` and maps failure to EFAULT.
  `kernel/src/mm/pmap.rs:1104` holds the pmap lock across the complete
  preflight at `:1108` and physical-frame copy at `:1128`. An unmapped or
  read-only second page fails before any bytes are written. The walker
  checks PRESENT/USER and excludes huge pages at `:1159`, and requires a
  writable leaf at `:1166`; intermediate entries are constructed writable
  at `:672`. Writing through the HHDM avoids the stale user-VA permission
  fault described in the PM comment, and needs no SMAP opening. The RAII
  guard releases on error. Scenario legs at
  `kernel/src/test_harness.rs:33317` cover valid straddle, straddle into a
  hole with unchanged prefix, unmapped, read-only and past-user-half buffers.
  Those legs test the copy helper directly; I did not run the QEMU scenario.

- PASS — Earlier item 4, time conversion. `kernel/src/boot_time_logic.rs:317`
  rejects rates below 1 MHz, and `:320` uses checked `u64::try_from` after
  u128 arithmetic. The earlier `tsc_ms(1 << 63, 500)` input now returns
  `None`, asserted at `tools/boot-time-logic-tests/src/lib.rs:387`.
  Explicit unavailable text is exercised at `:403` and `:406`.
  Firmware-after-entry still selects the TSC-reset origin without unsigned
  subtraction, asserted at `:415` and `:418`. All these host tests passed.

- PASS — New effects of the fixes: no additional actionable regression found in this narrow pass; the reserved-region issue above is the remaining part of earlier item 1.

Validation: all 14 `cargo test -p boot-time-logic-tests --target
x86_64-unknown-linux-gnu --offline` tests passed on the fetched head.
From `kernel/`, `cargo check -Zjson-target-spec --offline -p
neoplutarch-kernel` passed both with and without `--features boot-time`.
`git diff origin/main...pr16 --check` passed. A temporary host probe imported
the unchanged PR logic and asserted the reserved-MMIO input above passes
both guards. No QEMU, physical boot, hostile-firmware boot or full CI gate
was run here. This RETURN changes no implementation.
