<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PROPOSAL: OT-86 Fix 3 (L4) scope: the Zenbook touchpad through AML alone

**Lane:** S5 (Claude), `claude/acpi-aml-fix1` restarted from `main` `436674dd` (Fix 2 merged as #104).
**Status:** proposal for the PM; build starts on the items marked *in scope*. NO MERGE without J.c.

## The goal, as a gate

Through AML alone, under the grade's scripted zero model, on the Zenbook S14 set:

- `\_SB_.PC00.I2C5.ETPD._CRS` decodes, through the crate's own `resource_descriptor_list`, to an I2C serial bus
  connection on `\_SB.PC00.I2C5` at slave address `0x15`, 400 000 Hz, 7-bit, controller-initiated. It also decodes
  to an Extended Interrupt on GSI `0x69`: level, active-low, exclusive-and-wake.
- `_DSM(hid-i2c, rev 1, fn 1)` returns `Integer(1)`, the HID descriptor register. `fn 0` returns `Buffer {0x03}`
  (rev 1).
- Those values equal `ZENBOOK_S14_QUIRK` in `kernel/src/drivers/i2c_hid/quirk_logic.rs`. The test mounts that file
  with `#[path]`, as `tools/i2c-hid-logic-tests` does, so the kernel's own constants are the cross-check and the
  kernel tree is untouched.
- The touchscreen `TPL0` (`I2C4` @ `0x10`, GSI `0x6C`) is decoded and reported the same way, with no quirk to
  check it against.

Today `_CRS` already returns the right 44 bytes; only the GenericSerialBus descriptor decode is missing. `_DSM`
fails closed only on the 16-byte UUID compare.

## Items

| # | Item | In scope | Corpus today (`acpi+fix1`) | Oracle |
|---|---|---|---|---|
| 1 | Buffer comparison of any length for `LEqual`/`LGreater`/`LLess` (and their `LNot` forms) | yes | 2 failures + 4 `_DSM` probes | ACPICA `exmisc.c` `AcpiExDoLogicalOp`: lengths and `memcmp`; KATs on equal, unequal, prefix, longer/shorter |
| 2 | GenericSerialBus Connection descriptor (0x8E), I2C type: a new `Resource` variant | yes | 2 probes (ETPD, TPL0) | ACPI 6.5 §6.4.3.8.2; KAT = the Zenbook template bytes (DSDT `0x676B3`) and the quirk cross-check |
| 3 | GpioInt / GpioIo descriptor (0x8C): a new `Resource` variant | yes | 0 probes reach one; 48 in the Zenbook DSDT (`G_IO`, `IETM.DSCR`) | ACPI 6.5 §6.4.3.8.1; KAT = a template copied from the Zenbook DSDT |
| 4 | SPI / UART / CSI-2 serial bus types | no (stay fail-closed) | 0 | – |
| 5 | IndexField read/write: write the index register, then access the data field | yes | 11 reads + 4 writes | ACPICA `exfldio.c` index-field path; KAT asserts the recorded access sequence |
| 6 | BankField read/write: write the bank value, then access the region | yes | 0 (built with 5, same shape) | ACPICA `exfldio.c`; access-sequence KAT |
| 7 | `Acquire` returns its result: Zero when acquired, Ones on timeout | yes | 6 failures | ACPI 6.5 §19.6 `Acquire`; ACPICA `exoparg2.c` |
| 8 | Store to a named String/Buffer (§19.3.5) | yes, **J.c. decides the rule** (below) | unreachable in the corpus today | KAT per rule |
| 9 | Buffer-typed field read returns `ceil(bits/8)` bytes, not `bits` rounded to 8 | yes | value change on any field over 64 bits | KAT: the 72-bit field from the Fix 2 findings §4 |
| 10 | Plain `AccessAs` applied (access width for later fields), instead of parse-only | yes | 0 | ACPI 6.5 §20.2.5.2 |
| 11 | Field `Connection()` beyond parse-only: GenericSerialBus/GPIO OpRegion access | **no: propose L9** | 0 corpus hits | – |
| 12 | `Load`, `LoadTable`, `Notify`, `Match`, `CopyObject`, DataRegion | no (stay fail-closed; other L4 work) | `Notify` reached by fuzz only | – |

**Why 11 is out.** A field after `Connection()` lives in a GenericSerialBus or GeneralPurposeIo OpRegion. Reading
it means a region handler for that space that receives the connection resource and the access attributes. That is
the region-space work of L9 (EC first), and it runs through the mediated handler seam of L6. No corpus evaluation
reaches such a field today. Building it here would design the L6/L9 handler interface without its constraint (i).

## Decision for J.c.: item 8, Store to a Buffer or String

*(Corrected 2026-10-23. The first version said ACPICA reallocates a table-resident Buffer to the source's length.
That was wrong: `AcpiDsBuildInternalBufferObj` (`dsobject.c`) heap-allocates every Buffer, so none carries
`AOPOBJ_STATIC_POINTER`.)*

- For a **non-empty Buffer** target, a table-resident one included, ACPICA keeps the target's size. It zero-fills
  a shorter source and truncates a longer one (`exstorob.c` `AcpiExStoreBufferToBuffer`). That is also the spec's
  rule (ACPI 6.5 §19.3.5).
- The divergence that remains has two parts:
  - a **zero-length** Buffer target takes the source's length in ACPICA;
  - a **String** target always takes the source's length (`AcpiExStoreStringToString`). The spec text isn't
    reachable from the container, so whether the spec agrees is not settled.

**Built: the ACPICA behaviour.** The zero-length case sits behind `STORE_GROWS_EMPTY_BUFFER`, a one-line switch.

## Gates (as Fix 2's, plus the goal)

- The goal above, as a test on the Zenbook set, with the quirk cross-check.
- A byte KAT per item; access-sequence KATs for IndexField and BankField.
- No regression on any corpus set. The ratchet holds. Every evaluation that changes value is listed, item 9's
  above all.
- `PANIC-CENSUS.md` is kept current, so its test passes.
- 200k fuzz at seed `0x0786`: 0 panics and 0 crashes.
- Kernel untouched; close-gate passes.
- A cross-family review (GPT-6-Luna, high, plain checklist).
