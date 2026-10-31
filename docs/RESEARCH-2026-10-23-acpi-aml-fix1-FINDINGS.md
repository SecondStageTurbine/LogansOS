<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-86 Fix 1 (L3): `acpi` 6.1.1 value-kind fixes, measured against the rung-0 rebaseline

Host only. The kernel is untouched. The patched crate is a vendored copy of `acpi` 6.1.1
(`third_party/acpi-loganfy`), graded beside the stock crate in the same run (column `acpi+fix1`).
The rung-0 findings are `docs/RESEARCH-2026-10-22-acpi-aml-corpus-FINDINGS.md`; their §4b is the
rebaseline every number here is measured against.

## Summary: the gates

From `tools/acpi-aml-grade/report/coverage.md` (section "OT-86 Fix 1 gates"). The test
`fix1_gates_hold_against_the_rebaseline` enforces it, and `committed_report_matches_the_harness`
checks that the report is reproducible.

| Gate | Rebaseline (stock `acpi`) | `acpi+fix1` | |
|---|---|---|---|
| OptiPlex DSDT loads | fails (both sets) | **loads (both sets)** | PASS |
| `ObjectNotOfExpectedType` (table loads + `_STA`/`_INI`) | 83 | **0** | PASS |
| QEMU wrong answers (PCI_Config reads from the wrong offset) | 24 | **0** | PASS |
| no regression on any corpus set (stock-clean evaluations lost) | – | **0** | PASS |
| `acpi` panics (every phase, probes included) | 27 | **0** | PASS (≤ 27) |
| unclassified failures | 0 | **0** | PASS |
| report reproducible | – | yes | PASS |
| kernel untouched | – | yes: no file under `kernel/`, `userland/` or `.github`; default kernel ELF identical (§8) | PASS |
| *real hardware `_STA`/`_INI` clean (info)* | *335/446 = 75.1%* | ***569/612 = 93.0%*** | |
| *real hardware DSDT loads (info)* | *3/7* | ***5/7*** | |

The stock column in the same run still reproduces the rebaseline exactly (83 / 24 / 27 / 335 of 446 /
3 of 7); the test checks that too, so a gate can never be met against a moved baseline.

## 1. How the patched crate is carried

- **A vendored copy, renamed:** `third_party/acpi-loganfy`, package `acpi-loganfy`, library still `acpi`.
  The harness links it next to the stock crate (`acpi_stock = { package = "acpi", version = "=6.1.1" }`,
  `acpi_fix1 = { package = "acpi-loganfy", path = ... }`) and grades both, plus `aml` 0.16.4, which stays stock.
- **Why not `[patch]`:** a `[patch.crates-io]` replaces the stock crate for the whole workspace, so stock
  grading would disappear. Two copies under different names keep stock grading in the same run, and the
  harness runs one grader body for both (`grade_acpi6.rs` `include!`s `acpi6_body.rs` with `acpi` bound to
  each copy).
- **The diff is reviewable:** the first commit (`7645a1b3`) vendors `src/` byte-identical to the crates.io
  tarball. Built from that commit, the `acpi+fix1` column equals the stock column on all 10 sets (commit
  `d7569caf`). Every change after it is Fix 1, one commit per item, each tagged `LOGANFY(Fix 1, <item>)`
  in the source. Read it with `git diff 7645a1b3 -- third_party/acpi-loganfy/src`.
- **Licence:** upstream's MIT/Apache-2.0 files are kept; see `third_party/acpi-loganfy/VENDORED.md`.
  The directory is excluded from the workspace, so first-party clippy/fmt settings don't apply. It is
  formatted with upstream's own `rustfmt.toml`.

## 2. The items

Stock line numbers refer to `acpi-6.1.1/src/aml/mod.rs` unless named otherwise. "KAT" names a test in
`tools/acpi-aml-grade/tests/fix1_kats.rs`. Each KAT is a hand-laid AML byte program run on both crates,
and each pins the stock defect as well as the fixed answer.

| Item | Stock defect | Fix | ACPI 6.5 | KAT(s) | ACPICA |
|---|---|---|---|---|---|
| **G1** | `Local`/`Arg` push `Reference{LocalOrArg}` (`:1437-1448`). The If/While predicates (`:712`, `:978`), Sleep/Stall (`:827`, `:832`), Fatal (`:581`), the Wait timeout (`:514`), Create\*Field/CreateField operands (`:747`, `:769-770`), the BankField value (`:958`), ConcatRes (`:463`) and the Mid source (`:2076`) used it unread. Increment/Decrement (`:420-437`) required a bare Integer object. | Unwrap the transparent reference at each site. Increment/Decrement read the value, store it back through the SuperName and return it. | §19.6 `If`, `While`, `Sleep`, `Stall`, `CreateField`…, `Mid`, `Increment`/`Decrement`; §19.3.5 | `g1_if_predicate_is_a_local`, `g1_while_increment_decrement_on_locals`, `g1_sleep_stall_mid_take_locals`, `g1_g3_create_field_over_a_local_buffer` | agrees (operands are resolved to values before use) |
| **G2** | `DerefOf` tests `typ() == Reference` (`:810`), but `typ()` recurses through references (`object.rs:278`), so it was never true. A String operand `unwrap()`ed `as_string()` on a Local's reference (`:813`, a panic). | Match the object, after unwrapping a Local/Arg. RefOf/Index → the referenced value; String → the named object's value. | §19.6 `DerefOf` | `g2_derefof_index_package` | agrees |
| **G3** | `Index` on a Buffer gives a BufferField reference, and `DerefOf` handed back the BufferField unread. A named BufferField as a TermArg was passed through unread (`:1524`). | `read_named_value`: a field unit is read; a buffer field becomes an Integer if it fits the integer width (32 bits for revision 1), else a Buffer. | §19.6 `Index`, `CreateField`; §19.3.5 | `g3_derefof_index_buffer_is_the_byte`, `g3_named_buffer_field_is_read_as_a_termarg` | agrees (`AcpiExReadDataFromField`: integer if ≤ integer width) |
| **G4** | not separate: these fail with G2 and pass once G2 is fixed | – | – | – | – |
| **(a)** | PCI_Config `do_native_region_read/_write` pass the field offset without `region.base` (`:2528`, `:2594`); SystemMemory/IO add it. | Add the base. | §19.6 `OperationRegion` | `a_pci_config_field_access_adds_the_region_base` (stock reads 0x01, fixed 0x61) | agrees |
| **(b)** | See §3. | | | `b_error_exit_unwinds_the_context_stack`, `b_reentrant_evaluation_returns_to_its_own_caller` | n/a (interpreter structure) |
| **(c)** | `Connection` (`:1738`) and extended `AccessAs` (`:1743`) in a field list are `todo!()`. The OptiPlex GenericSerialBus fields use both. | Parse only: `0x02 NameString \| 0x02 BufferData`, and `0x03 AccessType Attrib Length`. Every later field in that list is `access_unimplemented`, and any access fails closed with `LibUnimplemented`. | §20.2.5.2; §19.6 `Connection`, `AccessAs` | `c_connection_fields_parse_and_fail_closed` (OptiPlex bytes verbatim) | encoding agrees (`psargs.c` `AcpiPsGetNextField`) |
| **(d) RefOf/Index store** | `do_store` `RefOf => todo!()` (`:2318`); `Index` results are RefOf-kind too. | New `ReferenceKind::Index`. A package element is replaced by a copy (no conversion); a buffer byte gets the source's low byte; RefOf stores to the named object with its implicit conversion. | §19.6 `Index`, `RefOf`, `Store`; §19.3.5 | `d_store_to_index_of_package_replaces_the_element`, `d_store_to_index_of_buffer_writes_the_low_byte`, `d_store_through_refof_converts_to_the_target_type` | agrees (`AcpiExStore`, `AcpiExStoreObjectToIndex`) |
| **(d) unary arity** | Not/FindSetLeftBit/FindSetRightBit start with TermArg + Target (`:1576-1580`) but `do_unary_maths` takes one argument (`:1804`), a panic. FromBCD/ToBCD have the same defect (`:1282`, `:2167`, `:2184`). | Take both, store to the Target, return. **Also fixed, because a correct arity would otherwise turn the panic into a wrong answer:** `Not` was a logical not (now bitwise, 32 bits in revision-1 tables), and `FindSetLeftBit` returned `leading_zeros + 1` (now the one-based MSB position). | §19.6 `Not`, `FindSetLeftBit`, `FindSetRightBit`, `FromBCD`, `ToBCD`; §20.2.5.4 | `d_unary_ops_take_a_target_and_compute_per_spec` | agrees (`exoparg1.c`) |
| **(d) fail closed** | IndexField/BankField read/write `todo!()` (`:2387`, `:2447`); buffer compare copies into 4 bytes (`:1889`); GenericSerialBus descriptor `unimplemented!` (`resource.rs:89`). | `LibUnimplemented`, after a `LOGANFY fail-closed: <feature>` warn line the grade classifies by. | – | `d_index_and_bank_fields_fail_closed`, `d_long_buffer_comparison_fails_closed`, `d_generic_serial_bus_descriptor_fails_closed` (OptiPlex `RES1` bytes verbatim) | – |

**Not implemented: Store to a named String/Buffer (§19.3.5).** The scope said to implement it at the
panic sites Fix 1 makes reachable. Under Fix 1 no corpus evaluation reaches `do_store`'s
"Stores to objects like" panic (`:2339`); it occurs 0 times in `coverage.json`.
- **Why the earlier scratch reached it:** the one real case was HP `WMIS._INI`. It does
  `DerefOf (SEN1 [Zero]) [Zero] = Local0`, a store through `Index` into a package. The scratch patch
  treated that as a store *into* the old element. §19.6 `Index` (and ACPICA's `AcpiExStoreObjectToIndex`)
  replace the element instead, so with that fix the String store never happens. The other 5 cases were
  contamination, removed by (b).
- **For whoever implements it (Fix 2 / L4): a narrow spec-vs-ACPICA divergence.** *(Corrected 2026-10-23,
  from S5's reading of ACPICA `e85aa3be`; the earlier text said ACPICA grows table-resident buffers, which was
  wrong.)* For a non-empty Buffer target, a table-resident one included, ACPICA keeps the target's size: it
  zero-fills a shorter source and truncates a longer one, which is the spec's rule. `AcpiExStoreBufferToBuffer`
  (exstorob.c:57-66) reallocates only a zero-length target or one flagged `AOPOBJ_STATIC_POINTER`, and
  `AcpiDsBuildInternalBufferObj` heap-allocates every Buffer, so no Buffer carries that flag. The remaining
  divergence is: (1) a zero-length target Buffer takes the source's length; (2) a String target takes the
  source's length (`AcpiExStoreStringToString`, exstorob.c:150-210).

## 3. (b), reported separately

**Root cause.** `context_stack` is interpreter-wide. A nested call pushes the caller's context (`:891`)
and adds a MethodLocals level (`:884`). Only a normal method exit pops them (`:904` `Return`,
`:1005-1008` end of method).
- **Error exit:** an error or panic exits without unwinding, so the next evaluation's `Return` pops a
  stale caller context and keeps executing an unrelated method.
- **Re-entrant evaluation:** `pci_address_for_device` evaluates `_ADR`/`_SEG`/`_BBR` while a PCI_Config
  field access is in progress. That evaluation's `Return` pops the *outer* evaluation's caller.

**Fix.** `do_execute_method` records the stack depth on entry. The execution only pops contexts it pushed.
On an error exit it drops them and removes the method level of every method it entered, including the
one `evaluate` entered.

**Measured on its own** (commit `010356a7` → `4ece897f`, `acpi+fix1` column):

| Measure | without (b) | with (b) |
|---|---|---|
| real hardware `_STA`/`_INI` clean | 565/612 | **569/612** |
| real hardware `_STA`/`_INI` completing | 587/612 | **591/612** |
| `fails closed: IndexField read` | 15 | **11** (4 were stale-context runs) |
| stock-clean evaluations lost | 0 | 0 |
| value changes | – | 1: Zenbook `\_SB.PC00.LPCB.EC0.BAT0._STA` `[Uninitialized]` → `Integer(15)` |

**The KATs show both paths directly:**
- `b_error_exit_unwinds_the_context_stack`: after `\OUTR` fails inside a nested method, stock's `\GOOD`
  returns `OUTR`'s `One` instead of its own 0x42.
- `b_reentrant_evaluation_returns_to_its_own_caller`: on stock, `_ADR`'s value lands in the caller
  `OUT2`. `OUT2`'s `Return` then becomes `_ADR`'s result (0x30005), so PCI function 5 is read and `OUT2`
  never completes. Fixed: function 0 is read and `OUT2` returns 5.

**Observed, not fixed (no corpus failure traces to it):** an explicit `Return` never removes the
method's MethodLocals level; only falling off the end does. Names a method creates therefore persist
across calls, and the next call replaces them silently (`AML name collision ... Replacing object`, at
trace level).

## 4. What changed that was clean on stock

There are no regressions: every evaluation that is clean on stock is clean under Fix 1. Five changed
value, all listed in the report's gate section:
- HP `HPET._STA`: `[Uninitialized]` → 0;
- M910q `I2C1._STA`: `[Uninitialized]` → 0;
- M910q `PMIC._STA`: `[Uninitialized]` → 0;
- MSI `GPP0.HDAU._STA`: 1 → 15;
- MSI `GPP7._STA`: 1 → 0.

**How they were checked.** For each one I traced both crates' op-by-op execution. Each trace is
identical up to the method's own final `Return`. There stock pops a stale caller context left by an
earlier failed evaluation and keeps running other code (more `M460` calls and field reads), so its
returned value is not this method's. Fix 1 returns at that `Return`.

**What that makes them.** All five are stock contamination, the class (b) closes. They show up before
(b) too, because Fix 1's other items remove the earlier failures that left the stale contexts behind.

## 5. New failures the fixes expose

Fixes G1–G3 let execution reach code it never reached before. Every failure it hits is classified, and
none of them is a stock-clean evaluation lost.

- **110 out-of-region SystemMemory accesses, all through method-local OperationRegions.** This is the
  Intel GPIO pad pattern on the OptiPlex 7080 and the M910q.
  - **The mechanism:** `GPC0`/`GPC1`/`SPC0`/`SHPO`/`SRXO` each declare `OperationRegion (PDW0,
    SystemMemory, GADR (...) + pad * 0x10, 4)` inside the method. `GADR` adds `SBRG`, a GNVS field that
    the zero model returns as 0. The address (e.g. `0x6e0740` = port 0x6E, pad 0x74) is right for
    `SBRG = 0`.
  - **Why the judge flags them:** it only sees regions left in the namespace afterwards, and a
    method-local region is gone by then.
  - **The classifier change:** it now names these as "access through a method-local OperationRegion",
    with the candidate methods. It is applied identically to the stock column, whose figures are
    unchanged.
  - A later rung that seeds the model from a live NVS dump would give `SBRG` its real value.
- **Fails closed (LibUnimplemented):**
  - IndexField read 11, IndexField write 4 (HP, M910q, MSI);
  - long-buffer compare 2 (OptiPlex `PEPD._STA`) plus 4 Zenbook `_DSM` probes;
  - GenericSerialBus descriptor 2 (the Zenbook touchpad and touchscreen `_CRS` probes).

  Each is an OT-86 L4 item.
- **Zenbook device probes, newly reachable under Fix 1:**
  - the touchscreen `_HID` (`ELAN9008`) and `_CRS` evaluate;
  - all five GPIO controller `_CRS`s (`GPI0`-`GPI5`) evaluate and decode to an interrupt plus a 64 KiB
    MMIO window.

  The touchpad and touchscreen stay blocked on L4: the GenericSerialBus descriptor and the 16-byte UUID
  compare.

**Unchanged from stock (pre-existing, outside Fix 1):**
- **5 `LevelDoesNotExist` SSDT loads:** M910q ×2, MSI `ssdt4`, Zenbook `ssdt`/`ssdt18`. These are
  external scopes the load order never creates.
- **1 `NoCurrentOp`:** Zenbook `ssdt2`.

## 6. STOP-AND-REPORT items

1. **Root cause outside the list: `Acquire` returns nothing.** `Acquire` (`:835-849`) retires without
   contributing a result. §19.6 `Acquire` returns an Integer: Ones on timeout, Zero when acquired. Every
   `Local0 = Acquire (...)` therefore leaves `Local0` Uninitialized, and the next comparison fails with
   `InvalidOperationOnObject { LogicalOp, Uninitialized }`.
   - **Affected (6, identical on stock and Fix 1):** OptiPlex Linux `\_TZ.FN00`-`FN04._STA` (SSDT3), and
     Zenbook `EC0._INI` (through `REBC`, traced).
   - **Not fixed:** it is outside G1–G3 and (a)-(d), and no gate needs it.
   - **Recommendation:** add it to L4; it is a one-line `contribute_arg`.
2. **(c) needed the extended `AccessAs` encoding as well as `Connection`.** The OptiPlex field is
   `Connection (RES1), Offset (0x56), AccessAs (BufferAcc, AttribBytes (0x21)), CMD0, 16`. I parse the
   3-byte `ExtendedAccessField` the same parse-only, fail-closed way. This is my reading of "only as far
   as the OptiPlex DSDT loads". The plain `AccessAs` (`0x01`) stays `todo!()`: Fix 1 never reaches it.
3. **(d) unary: two value fixes beyond arity.** `Not` and `FindSetLeftBit` were computed wrongly (see the
   table). Fixing only the arity would have turned a panic into a wrong answer, so they are fixed in the
   same commit. FromBCD/ToBCD had the same arity defect and are fixed too; they are not reached in the
   corpus.
4. **Spec text was not available from this container.** The egress proxy blocks `uefi.org`.
   - **What that means for the citations:** they name the ACPI 6.5 section and operator from the spec's
     structure (§19.6 operator reference, §19.3.5 data types, §20.2.5.x encodings), not a quoted text.
   - **What was checked instead:** behaviour against ACPICA's source (acpica/acpica @ `e85aa3be`, read for
     behaviour only, nothing copied). Exact opcode bytes came from ACPICA's raw `amlcode.h`.
   - **Spec vs ACPICA:** no divergence was found in any implemented item. The one known divergence
     (static buffer stores, §2) sits in the item not implemented.
   - **Reviewer:** please check the section citations against the spec text.
5. **Analysis aids:**
   - I built ACPICA's host `iasl` from that checkout. I used it to disassemble corpus tables and to decode
     every KAT independently: each KAT disassembles to the ASL in its doc comment.
   - Temporary trace logging (not committed) produced the §4 traces.
   - Neither is part of the harness, the build or the kernel.
6. **XSDT:** unchanged. The 2 OptiPlex XSDT entries present in neither dump stay recorded as unaccounted.

## 7. Per-commit measurements (`acpi+fix1` column)

| Commit | Item | ONOET | QEMU wrong | panics | DSDT | real-hw clean |
|---|---|---|---|---|---|---|
| `d7569caf` | vendored, unpatched | 83 | 24 | 27 | 3/7 | 335/446 |
| `96a99ae8` | G1 operand sites | 65 | 24 | 27 | 3/7 | 352/446 |
| `22734d50` | (d) RefOf/Index store | 65 | 24 | 26 | 3/7 | 354/448 |
| `c14310f9` | G2/G3 | **0** | 24 | 33 | 3/7 | 561/612 |
| `87a291ac` | G1 Increment/Decrement | 0 | 24 | 33 | 3/7 | 561/612 |
| `a94d0964` | (d) unary/BCD arity | 0 | 24 | 29 | 3/7 | 565/612 |
| `3d50bcc0` | (a) PCI_Config base | 0 | **0** | 29 | 3/7 | 565/612 |
| `010356a7` | (c) + (d) fail closed | 0 | 0 | **0** | **5/7** | 565/612 |
| `4ece897f` | (b) unwind | 0 | 0 | 0 | 5/7 | **569/612** |

Panics rise mid-series (33) because G2/G3 reach pre-existing `todo!()` sites. The (d) fail-closed commit
takes them to 0. The SHAs are the commits on `claude/acpi-aml-fix1` as cut from `main` `ff25f384`. If the branch is
rebased again, the commit messages carry the same figures.

## 8. Reproduce

```
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade      # regenerates report/coverage.{json,md}
cargo test -p acpi-aml-grade                                     # gates, KATs, reproducibility
git diff 7645a1b3 -- third_party/acpi-loganfy/src                # the Fix 1 patch
```

The default kernel ELF built with this branch's `Cargo.toml`/`Cargo.lock` is identical to the one built
with `main`'s; the hashes are in the PR's "PM: ready" comment.
