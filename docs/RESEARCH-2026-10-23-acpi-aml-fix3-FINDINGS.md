<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-86 Fix 3 (L4): the Zenbook touchpad through AML alone

Host only; the kernel is untouched. Fix 3 builds the L4 items in the scope proposal
(`Plans/PROPOSAL-2026-10-23-acpi-aml-fix3-scope.md`) on the vendored interpreter
(`third_party/acpi-loganfy`), on top of Fix 2 (merged as #104, `436674dd`). Every change carries a
`LOGANFY(Fix 3, <item>)` comment.

## Summary: the gates

| Gate | Result | |
|---|---|---|
| **Goal:** through AML alone, the Zenbook touchpad's values equal the kernel's quirk table | `tests/zenbook_touchpad.rs` loads the Zenbook S14 tables. It evaluates `ETPD._CRS`, `_DSM(hid-i2c)` fn 1 and fn 0, and `I2C5._ADR`, decodes `_CRS` with the crate's own decoder, and compares each value with `ZENBOOK_S14_QUIRK` (`kernel/src/drivers/i2c_hid/quirk_logic.rs`, mounted read-only by `#[path]`). The values: I2C5 @ `0x15`, 400 kHz, 7-bit, controller-initiated; GSI `0x69`, level, active-low, wake; HID descriptor register 1; PCI 00:19.1; `_CRS` byte-identical to the quirk's template | PASS |
| a byte KAT per item | `tests/fix3_kats.rs`, 25 programs. `iasl -d` decodes each hand-laid descriptor and field list as its comment says | PASS |
| no regression | Against `main`: 0 stock-clean evaluations lost and 0 clean evaluations changed value. 97 evaluations newly complete, each listed in `Plans/VALUES-2026-10-23-acpi-aml-fix3.md`. The ratchet (`fix1_never_regresses`) holds | PASS |
| census | `PANIC-CENSUS.md` regenerated: 58 sites, 0 unclassified; `tests/panic_census.rs` passes | PASS |
| fuzz | Seed `0x0786`, 200 000 cases, debug build with overflow checks, crate tree `b076a6e8` (= HEAD): **0 panics, 0 crashes**, and the same 17 hangs (L7 data) | PASS |
| kernel untouched | no file under `kernel/`, `userland/`, `tools/xtask/`, `.github/`; `Cargo.toml` and `Cargo.lock` unchanged | PASS |

Corpus, `acpi+fix1` column, `main` → this branch:
- Real-hardware DSDT loads went from 5/7 to **7/7**. The M75q and MSI DSDTs read IndexFields while they load.
- Real-hardware `_STA`/`_INI` clean went from 569/612 to **660/682**. There are more evaluations because two more DSDTs
  load. Every real-hardware `_STA`/`_INI` evaluation now completes (682/682); the 22 not clean are judge findings,
  not interpreter failures.
- All sets went from 626/669 to 717/739.
- Every Zenbook device probe completes, including the touchpad and touchscreen `_CRS` decode and `_DSM`.

## The items

| # | Commit | What changed | Oracle |
|---|---|---|---|
| 1 | `bf17f3a9` | Strings and Buffers compare byte by byte, at any length. Stock read the first 4 bytes as an Integer: a 16-byte UUID panicked and short operands compared numerically. The right operand is converted to the left's type, a String with its NUL. Integer compares are unchanged. | ACPICA `exmisc.c` `AcpiExDoLogicalOp` (lexicographic: bytes to the shorter length, then lengths); `exconvrt.c` `AcpiExConvertToBuffer` (String length + 1) |
| 2 | `ba29ba8e` | The GenericSerialBus descriptor (0x8E), I2C type, becomes `Resource::I2cSerialBus`. SPI, UART, CSI-2 and vendor types fail closed. | ACPICA `amlresrc.h` `AML_RESOURCE_SERIAL_COMMON`/`AML_RESOURCE_I2C_SERIALBUS`; `rsserial.c` `AcpiRsConvertI2cSerialBus`; ACPI 6.5 §6.4.3.8.2 |
| 3 | `6158fafb` | The GPIO descriptor (0x8C), `GpioInt`/`GpioIo`, becomes `Resource::Gpio` | ACPICA `amlresrc.h` `AML_RESOURCE_GPIO`; `rsserial.c` `AcpiRsConvertGpio`; `rsmisc.c` `ACPI_RSC_COUNT_GPIO_RES` (no vendor data means the source runs to the end); ACPI 6.5 §6.4.3.8.1 |
| 5, 6 | `218d6510` | IndexField and BankField read and write. An IndexField writes the datum's byte offset to the index register, then accesses the data register. A BankField writes the bank value, then accesses its region. A value too wide for its register is an error. | ACPICA `exfldio.c` `AcpiExFieldDatumIo`, `AcpiExRegisterOverflow`; `exprep.c` (`IndexField.Value`) |
| 7 | `eab0ebf6` | `Acquire` returns Zero when acquired and Ones on `MutexAcquireTimeout`. Stock contributed no value. | ACPI 6.5 §19.6 `Acquire`; ACPICA `exoparg2.c` (AE_TIME becomes a TRUE result) |
| 9 | `1aa42a36` | A buffer-typed field read is `ceil(bits/8)` bytes; stock gave `bits` rounded to 8 (Fix 2 findings §4) | ACPICA `exfield.c` `AcpiExReadDataFromField` (`ACPI_ROUND_BITS_UP_TO_BYTES`) |
| 8 | `0fc11f4e` | Store to a named Buffer or String. A Buffer target keeps its size, zero-filled or truncated; a zero-length target takes the source's length (`STORE_GROWS_EMPTY_BUFFER`). An Integer source contributes its integer-width bytes and a String source carries its NUL. A String target takes a String source; converting another type to a String fails closed. A self-store is a no-op. | ACPICA `exstoren.c` `AcpiExStoreObjectToObject`; `exstorob.c` `AcpiExStoreBufferToBuffer`, `AcpiExStoreStringToString`; `exconvrt.c` `AcpiExConvertToBuffer`; ACPI 6.5 §19.3.5 |
| LNot | `b48a1a8d` | `LNot` takes one operand. Stock parsed it with two, so `!X` swallowed the next term (see below). `LNot (Zero)` is Ones at the table's integer width. | ACPI 6.5 §20.2.5.4; ACPICA `acopcode.h` `ARGP_LNOT_OP`; `exoparg1.c`; `dswexec.c` |
| 10 | `f8151a06` | A plain `AccessAs` sets the access type of the later fields. The attribute matters only to serial-bus regions, which still fail closed after `Connection`. | ACPICA `dsfield.c` `AcpiDsGetFieldNames` (`AccessType & 0x07`) |

Two Fix 1 KATs pinned the old fail-closed results; they now assert the implemented answers:
- `d_long_buffer_comparison`;
- `d_generic_serial_bus_descriptor`.

`d_index_and_bank_fields` now asserts the access sequence.

## The LNot defect (stock, fixed in this PR)

Stock `acpi` 6.1.1 `mod.rs:1620` starts `LNot` in the arm of the binary comparisons, with `[TermArg; 2]`. ACPI 6.5
§20.2.5.4 has `DefLNot := LNotOp Operand`, and ACPICA `acopcode.h:102` has
`ARGP_LNOT_OP ARGP_LIST1 (ARGP_TERMARG)`. So `!X` swallowed the next term as a second operand.

This gave wrong answers, not just failures. `If (LNot (One)) { Return (One) } Return (2)` returned 1, because the
swallowed `Return` ran. As a `Store` operand it panicked on stock, and Fix 2 reported MalformedAml. I found it when
item 5 got Zenbook `EC0._INI` past its IndexField. The PM decided it is fixed here, as its own commit.

Corpus effect: one evaluation changes. Zenbook `\_SB_.PC00.LPCB.EC0_._INI` goes from FAIL to completing
(`Plans/VALUES-2026-10-23-acpi-aml-fix3.md`). No completed evaluation changed value.

It is a defect in the stock crate too; whether to report it upstream is a later decision.

## Decisions for J.c.

**Item 8 at its edges.** A correction first: the earlier claim that ACPICA reallocates a table-resident Buffer was
wrong. `AcpiDsBuildInternalBufferObj` heap-allocates every Buffer, so for a non-empty Buffer target ACPICA and the
spec agree: keep the size. Two questions remain:
- **A zero-length Buffer target.** ACPICA gives it the source's length. That is what's built, and
  `STORE_GROWS_EMPTY_BUFFER` is the switch.
- **A String target.** ACPICA always takes the source's length, and that is built. I could not read the spec
  text for String targets from the container.

## Known gaps, not fixed here

- **Fixed after the merge (found by Chad, cross-family review relay #48).** As merged in #107, an Integer or String
  left operand of `LEqual`/`LGreater`/`LLess` required a right operand of the same type and failed closed
  otherwise; ACPICA converts the right operand to the left's type in every case (`exmisc.c:470-487`). Fixed on
  `claude/acpi-aml-fix1` after #107: an Integer left reads a Buffer's first bytes and a String as hex
  (`AcpiUtImplicitStrtoul64`), a String left takes an Integer or Buffer as hex digits
  (`ACPI_IMPLICIT_CONVERT_HEX`), and a Package stays an error. Five KATs in `fix3_kats.rs`, each also run on
  ACPICA's `acpiexec`. No corpus value changes (`Plans/VALUES-2026-10-23-acpi-aml-fix3.md`).
  The same review fix covers `LAnd`/`LOr`, found while fixing it: their operands are `ARGI_INTEGER`
  (`acopcode.h:231,249`), so ACPICA reads a String as hex. Fix 3 as merged read its first 4 raw bytes, so
  `LAnd (One, "0")` was true; stock failed closed there. A sixth KAT pins it, also run on `acpiexec`.
- **Integer width in a revision-1 table.** ACPICA truncates every operator result in a revision-1 table
  (`dswexec.c`). The crate truncates only `Not` (Fix 1) and now `LNot`, so the comparisons still return 64-bit
  Ones there. No corpus table is revision 1.
- **Implicit conversion to a String on Store** (`AcpiExConvertToString`, hex formatting) fails closed.
- **No corpus evaluation exercises items 3, 9, 10 or the `Acquire` timeout path.**
  - Item 3: the Zenbook's GPIO descriptors are returned by methods that take arguments (`G_IO`, `IETM.DSCR`).
  - Item 9: no field read is over 64 bits.
  - Item 10: no field list has a plain `AccessAs`.
  - The `Acquire` timeout path needs a handler that times out, so I checked it by reading the code against
    `exoparg2.c`.

## Reproduce

```
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade        # report/coverage.*
cargo test -p acpi-aml-grade                                       # goal test, KATs, census, gates, ratchet
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- run --seed 0x0786 --cases 200000 --jobs 4 --report tools/acpi-aml-grade/report/fuzz-fix1.json
git diff origin/main -- third_party/acpi-loganfy/src               # the Fix 3 patch
```
