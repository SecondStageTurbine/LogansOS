<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — Relay 212, Jarvis: PR #254 / L7.1

Reviewed head `04bddd7a65d529cd4954758a0addeba5e5cabc3b` with
`git diff origin/main...pr254`, including the PR description. References below
are to that head, not the RETURN branch. Overall: **FAIL**.

Validation: `cargo test -p boot-time-logic-tests --target x86_64-pc-windows-msvc`
passed all 7 tests. A separate host probe importing the unchanged
`kernel/src/boot_time_logic.rs` reproduced the contradictory-epoch,
zero-firmware, missing-firmware and missing-rate outputs below. The bad-pointer
finding is a source trace through the syscall and fatal page-fault handler;
I did not run a new kernel/QEMU scenario or a Zenbook boot. The PR's existing
kernel scenario tests wrong length and a kernel-half pointer, not an unmapped
user-half pointer. No implementation changes are included in this return.

- **1. FAIL — power-on origin is assumed, not established.**
  `kernel/src/acpi/mod.rs:439` obtains the firmware times from ACPI FPDT's
  FBPT pointer and its basic boot performance record;
  `kernel/src/boot_time_logic.rs:165` reads the five nanosecond timestamps.
  Missing FPDT returns `NoFpdt` (`kernel/src/acpi/mod.rs:443`), the kernel
  prints the reason (`kernel/src/boot_time_logic.rs:278`), and the syscall
  clears the firmware-present flag and zeroes those fields
  (`kernel/src/boot_time_logic.rs:197`). However,
  `kernel/src/boot_time_logic.rs:247` divides the absolute raw TSC by a rate;
  neither an origin adjustment nor a validity check relates that counter to
  firmware time. `kernel/src/boot_time_logic.rs:319` still labels it
  `From power-on`, including with no firmware. **Concrete input:** the PR's
  own `times()` fixture, TSC rate 2.8 GHz, entry `0x1_0000_0000`, model
  `0x1_8000_0000`, desktop `0x2_0000_0000`, firmware EBS exit
  4,120,000,000 ns, and current TSC 56,000,000,000. It prints
  `From power-on: firmware 4.12 s, model ready 2.30 s, desktop 3.06 s, Home 20.00 s.`
  The supposed model and desktop times precede firmware exit; the existing
  KAT explicitly accepts this (`tools/boot-time-logic-tests/src/lib.rs:203`).
  The runbook asks a human to notice a bad epoch, but the code does not
  invalidate the number. Also, `kernel/src/boot_time.rs:75` can use a
  CPUID-0x16 estimate, while `snapshot` at line 84 exports only its numeric
  rate, so Home cannot distinguish an estimate from a measured rate.

- **2. FAIL — byte oracle/version/length PASS; bad-pointer copyout FAIL.**
  `kernel/src/boot_time_logic.rs:13` defines version 1 and length 80;
  line 197 encodes into a fully initialized byte array, and line 229
  rejects a wrong length/version or unknown flags. The hand-laid `RECORD`
  fixture is independent of the encoder and checked in both directions
  (`tools/boot-time-logic-tests/src/lib.rs:117`, line 147).
  `kernel/src/arch/x86_64/syscall.rs:7497` rejects lengths other than 80
  with EINVAL. But `validate_user_buf` at line 1481 checks only numerical
  bounds/overflow; it never verifies mappings, user permission or write
  permission. Line 7507 performs an unguarded raw copy with SMAP opened.
  **Concrete input:** from any process on a `boot-time` kernel, invoke
  `SYS_BOOT_TIMES(0x0000400000000000, 80)` with that user-half range unmapped.
  Bounds validation succeeds and the store faults in ring 0. The page-fault
  entry cannot resume (`kernel/src/arch/x86_64/idt.rs:790`); only user-mode
  faults kill the caller (line 843), and this kernel-mode fault reaches
  `halt_forever` (line 870), rather than returning EFAULT. A buffer crossing
  from a writable page into an unmapped page is another failing input.
  The test at `kernel/src/test_harness.rs:32828` uses a kernel buffer, which
  the bounds check rejects before a copy and therefore misses this case.

- **3. PASS — unrestricted read and no unintended data disclosure in the record.**
  `kernel/src/arch/x86_64/syscall.rs:1113` dispatches the call without a
  capability check; line 7495 has no caller/privilege restriction. Every
  process can read it when the feature is enabled; without the feature
  the syscall arm is absent and the normal ENOSYS fallback applies.
  `kernel/src/boot_time_logic.rs:197` emits exactly 80 initialized bytes:
  version/flags, rate, boot TSC stamps and firmware timestamps. It emits
  no kernel pointer, FBPT physical address, padding, credentials or
  process-private data. Timing disclosure is the intended interface;
  ring 3 already reads the TSC in `userland/libneo/src/lib.rs:1367`.
  This disclosure verdict does not excuse the availability failure in item 2.

- **4. FAIL — image-A wiring is opt-in and works as wired, but an unrecorded firmware time becomes zero seconds.**
  `tools/xtask/src/main.rs:6136` adds the kernel feature when `--boot-time`
  is set; line 7072 adds neo-init `boot-time` and Arthur `arthur-boot-time`
  for `desktop-with-model`. The product ISO path builds both at line 8052.
  Thus it is in the explicitly requested evidence image
  (`iso --with-model <neom> --boot-time`), not default image A; the runbook
  documents that at `docs/runbook-baremetal-zenbook-s14.md:158`.
  Home caches the row on first show
  (`userland/neo-init/src/desktop_session.rs:4109`), inserts it second,
  preserves the last row, and leaves the original rows intact when the
  syscall returns no data (line 4170). No TSC rate gives an explicit
  `Home at TSC ... (no TSC rate).` fallback, not a fabricated duration.
  **Concrete input:** a valid FBPT basic boot record with
  `ebs_exit_ns = 0` (the module documents zero as not recorded at
  `kernel/src/boot_time_logic.rs:20`), with the 2.8 GHz rate and current
  TSC 56,000,000,000. The parser returns `Some(FirmwareBoot)` without
  per-field availability checks; `write_summary` at line 320 prints
  `firmware 0.00 s` as though it were measured. Reproduced:
  `From power-on: firmware 0.00 s, model ready 2.30 s, desktop 3.06 s, Home 20.00 s.`
  Missing the entire FPDT omits the firmware part, but still makes the
  unchecked power-on claim described in item 1.

- **5. PASS — the prior-art section exists and its limited FPDT layout/refusal claims hold.**
  PR #254's **Prior art** section cites ACPI 6.5 section 5.2.24 and Linux
  `drivers/acpi/acpi_fpdt.c` at `abf8c334` (read for principle, no vendoring).
  I fetched that exact upstream source: physical-width validation is at
  lines 151–157, signature/length and bounded record checks at lines
  183–216, and the boot-record layout/first-duplicate handling at lines
  262–276. Corresponding enforcing sites are
  `kernel/src/boot_time_logic.rs:128` (FPDT pointer), line 154 (FBPT
  length), line 108 (record walk), and line 161 (first boot record).
  The Zenbook raw FPDT and independent hand-laid FBPT/ABI fixtures are at
  `tools/boot-time-logic-tests/src/lib.rs:22`, line 63, and line 117;
  their KATs pass. This verifies the cited parser principles, not an
  identical Linux implementation: the short boot-record policy differs
  (skip here, reject in Linux). The section explicitly covers only FPDT
  layout; it provides no proof of the TSC power-on origin failed in item 1.
  Primary cross-checks: [Linux at the cited revision](https://raw.githubusercontent.com/torvalds/linux/abf8c334/drivers/acpi/acpi_fpdt.c),
  [systemd's FPDT reader](https://github.com/systemd/systemd/blob/main/src/shared/acpi-fpdt.c)
  (rejects a zero EBS-exit timestamp), and
  [Linux's TSC adjustment checks](https://github.com/torvalds/linux/blob/master/arch/x86/kernel/tsc_sync.c)
  (counter adjustments require handling, not an assumed epoch).
