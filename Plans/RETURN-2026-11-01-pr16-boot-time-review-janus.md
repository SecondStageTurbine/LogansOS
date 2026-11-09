<!-- SPDX-License-Identifier: CC-BY-4.0 -->
Relay 232 / Janus. Section 232: PR #16, L7.1 boot time from power-on.

Reviewed fetched head `958e4fc1b7c7f98a039d96a78caeb37d4d3a4a21` with
`git fetch origin +pull/16/head:pr16` and `git diff origin/main...pr16`.
All source references below are to that PR head. RETURN branch
`codex/pr16-review-janus` starts at `origin/main`,
`10e10ee99b282ae163749748a9e4427ef5132deb`. Overall: FAIL.

Validation: `cargo test -p boot-time-logic-tests --target x86_64-unknown-linux-gnu --offline`
passed all 10 tests. Four additional
temporary host probes imported the unchanged PR logic and passed their
assertions: truncated/short/overlong record rejection, acceptance of the
out-of-range header extent in item 1, the conversion wrap in item 4, and
defined missing-firmware/firmware-after-entry results. From `kernel/`,
`cargo check -Zjson-target-spec --offline -p neoplutarch-kernel` passed
both without additional features and with `--features boot-time`.
`git diff origin/main...pr16 --check` passed. No QEMU scenario, hostile
firmware boot, physical boot, or byte-for-byte ISO comparison was run.
The bad-copyout finding is a source trace. This RETURN changes no implementation.

- FAIL — 1. The byte-slice record walkers are bounded, but firmware pointer
  extents are not. `kernel/src/boot_time_logic.rs:139` requires a complete
  record header; `:144` rejects lengths below four or beyond the remaining
  table, and `:150` advances by that checked positive length. FPDT length
  is checked against the supplied slice at `:163`; FBPT length is capped
  at 4096 at `:187` and checked against its slice at `:197`. Truncation,
  zero length, or a record extending past those slices cannot produce an
  out-of-bounds slice or an infinite walk. However, `:174` checks only the
  FBPT starting address against the physical-address width.
  `kernel/src/acpi/mod.rs:455` immediately reads eight bytes from that
  address, and `:456` reads the advertised length, without checking either
  complete physical range or whether it belongs to readable firmware memory.
  Concrete input: a valid, checksum-correct FPDT pointer record with
  `address = 0x0000000ffffffffc` and CPU physical width 36.
  `fbpt_address` returns `Ok(0xffffffffc)` (reproduced), although its eight-byte
  header ends beyond `0x1000000000`, the exclusive physical-address limit.
  The unsafe read therefore crosses the supported physical range before
  `fbpt_len` can reject anything. `phys_slice` at
  `kernel/src/acpi/mod.rs:75` rounds/maps the range without a physical-width
  check, then constructs a slice at `:85`; mapping a page does not validate
  backing memory. The reused root-table reader also has an inherited gap:
  `:140` accepts any root length >= 36 and `:150` reads it before its
  checksum. A readable XSDT header declaring length `0xffffffff` thus
  attempts a roughly 4-GiB read without an enclosing-region bound.
  Require checked complete extents and valid backing ranges before reads,
  including the root/FPDT discovery path.

- FAIL — 2. ABI byte oracle and short-length rejection pass; checked
  copyout does not. `kernel/src/boot_time_logic.rs:227` encodes an initialized
  80-byte little-endian record explicitly, independently of Rust struct
  padding. The hand-laid `RECORD` at
  `tools/boot-time-logic-tests/src/lib.rs:147` is checked against both encoder
  and decoder at `:184`; version, length and flags are checked at
  `kernel/src/boot_time_logic.rs:259` and `:263`.
  `kernel/src/arch/x86_64/syscall.rs:7535` returns EINVAL for length 79
  (or anything other than 80). But `:7538` calls `validate_user_buf`, whose
  checks at `:1481` cover only numerical range and addition overflow;
  there is no mapped-page, user-permission or write-permission check.
  `:7544` opens SMAP and `:7545` performs raw `copy_nonoverlapping`, with
  no fault recovery. Concrete input: any process on a `boot-time` kernel
  calls `SYS_BOOT_TIMES(0x0000400000000000, 80)` with that user-half range
  unmapped. Validation succeeds and the store faults in ring 0.
  `kernel/src/arch/x86_64/idt.rs:790` documents the non-returning fault
  path; `:843` contains only user-mode faults, and `:870` halts on this
  kernel-mode fault instead of returning EFAULT. An 80-byte buffer starting
  40 bytes before the end of a writable page with the next page unmapped
  also reaches this copy and can leave a partial write before the fatal
  fault. `kernel/src/test_harness.rs:33252` tests wrong length and a
  kernel-half pointer, which are rejected before copying and do not cover
  these inputs. Route the new syscall through a fault-recovering user-copy
  operation and test actual unmapped/read-only/cross-page user buffers.

- PASS — 3. Missing FPDT has a defined unknown representation.
  `kernel/src/acpi/mod.rs:445` returns `NoFpdt` for no RSDP, and `:449`
  does so when no FPDT is found. `kernel/src/boot_time.rs:38` caches that
  result; `:91` exports `firmware: None` rather than fabricated timestamps.
  `kernel/src/boot_time_logic.rs:231` leaves the firmware-present flag clear,
  `:238` initializes its wire fields to zero, and `:266` decodes those
  fields as absent when the flag is clear. Thus the zeros are explicitly
  unavailable fields, not a measured zero duration. `:314` prints
  `firmware handoff unknown (no-fpdt)`; `:68` rejects a power-on origin
  without a handoff, and `:386` omits firmware from the summary. Host tests
  and the additional missing-firmware probe confirm this behavior.
  `kernel/src/test_harness.rs:33229` asserts QEMU's `NoFpdt` result, but
  that kernel scenario was not run in this review.

- FAIL — 4. Unknown-rate and ordinary firmware-after-kernel cases are
  handled, but the final time conversion can wrap. At
  `kernel/src/boot_time_logic.rs:286`, rate zero returns `None`, and `:345`
  and `:426` display an explicit no-rate fallback. `:73` compares entry
  against firmware handoff rather than subtracting unsigned timestamps;
  for 1-GHz TSC, entry 1,000,000,000 and firmware EBS exit
  4,000,000,000 ns, the reproduced summary is
  `Since TSC reset: desktop 2.00 s, Home 3.00 s.` No negative interval
  or underflow is generated. However, `:289` multiplies/divides in u128
  and then uses unchecked `as u64`, silently truncating a large quotient.
  Concrete input: `tsc_ms(1u64 << 63, 500)` returns `Some(0)` (reproduced),
  although the exact result is `2^64` milliseconds. There is no lower
  bound on a nonzero CPUID-0x15 rate:
  `kernel/src/arch/x86_64/lapic_timer_logic.rs:23` accepts
  EAX=2, EBX=1, ECX=1000 as 500 Hz, and the fallback at
  `kernel/src/boot_time.rs:70` exports it if calibration is unavailable.
  This is a synthetic extreme counter/rate input, not an observed normal
  hardware rate. Use a checked conversion of the quotient and represent
  an unrepresentable duration as unavailable (or explicitly saturate).

- PASS — 5. The feature remains opt-in, with the prior default behavior
  preserved by the source/configuration gates. `kernel/Cargo.toml:77`
  leaves the default feature set unchanged; `:205` introduces `boot-time`
  only as an explicit feature. Kernel modules at `kernel/src/main.rs:56`
  and `:64`, stamps at `:661`, `:1023` and `:1079`, the firmware reader at
  `kernel/src/acpi/mod.rs:440`, and the syscall arm at
  `kernel/src/arch/x86_64/syscall.rs:1112` are gated. Without it the
  syscall retains the unknown-syscall behavior. The product feature list
  at `tools/xtask/src/main.rs:6161` is unchanged; `:6281` adds the feature
  only when the flag is set. The flag starts false at `:6312`, is reset
  false at `:6322`, and is set only by `--boot-time` at `:6324`.
  The userland feature additions are conditional at `:7267`; Home's row
  is gated at `userland/neo-init/src/desktop_session.rs:4202`, login's
  helper returns `None` when off at
  `userland/neo-init/src/desktop_session/login.rs:661`, and Arthur's line
  is gated at `userland/neo-arthur/src/main.rs:610`. Default and feature-on
  kernel checks pass. This is a source/configuration and compile verdict;
  it does not claim measured binary or ISO identity.
