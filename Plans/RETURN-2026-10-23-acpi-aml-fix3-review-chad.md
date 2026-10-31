<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN (relay #48): OT-86 Fix 3 cross-family review

**Reviewed SHA:** `4389b0627446d92e1a34d11d9b6aa8969015f5b8` (pinned per corrected dispatch)
**Branch:** `codex/acpi-aml-fix3-review-chad`
**Verdict:** FAIL — item 3.

RAG health check was healthy (`ollama=true`, `qdrant=true`). The requested semantic query was run. The required
checks also ran: `cargo test -p acpi-aml-grade` passed; the grade run with `CARGO_PROFILE_DEV_PANIC=unwind`
passed (10 sets × 3 crates, 0 unclassified failures); the short fuzz run (seed `0x0786`, 20,000 cases, 4 jobs)
reported 0 panics, 0 crashes, and 3 hangs.

## Checklist

1. **PASS — scope.** `git diff d604e839 4389b062 --name-only` has no `kernel/`, `userland/`, `tools/xtask/`, or
   `.github/` paths; the crate changes are confined to `third_party/acpi-loganfy/src/aml/`; `Cargo.lock` is
   unchanged. In `tools/acpi-aml-grade/report/coverage.json`, no `"stock":` column values differ from the base.
   The diff does change `fix1` results, as expected.

2. **PASS, with a scope note — Zenbook goal.** `tools/acpi-aml-grade/tests/zenbook_touchpad.rs:23,38-59,65-102`
   reads the kernel quirk read-only, evaluates `_CRS`, `_DSM` functions 1 and 0, and `_ADR`; `_CRS` bytes are
   checked against `q.crs_template`, decoded with the crate decoder, and decoded values (including interrupt
   values) are checked against quirk fields/template. `_DSM` fn 1 and `_ADR` are compared to quirk fields. Note:
   the fn-0 supported-functions mask is asserted as literal `[0x03]` at line 95 (not represented in `Quirk`),
   and resource-source text is also asserted literally at line 80, though `_CRS` byte equality already ties it to
   the quirk template.

3. **FAIL — right operand conversion is incomplete.** `third_party/acpi-loganfy/src/aml/mod.rs:2275-82` uses
   strict `as_integer()` and `as_string()` for Integer-left and String-left comparisons; those helpers reject
   other object types (`third_party/acpi-loganfy/src/aml/object.rs:205-224`). ACPICA converts the right operand
   to the left operand's type before comparison ([ACPICA `AcpiExDoLogicalOp`](https://github.com/open-acpica/acpica/blob/master/source/components/executer/exmisc.c)). Concrete AML method bytes for
   `Method (MIXI, 0) { Return (LEqual (One, Buffer (One) { 1 })) }`:
   `14 0D 4D 49 58 49 00 A4 93 01 11 03 01 01`. The RHS is a one-byte Buffer whose value converts to Integer 1;
   this interpreter instead returns an `ObjectNotOfExpectedType` error at the Integer-left comparison. The
   Buffer-left/String-right case does append NUL, but it does not cover the reverse direction or Integer-left
   coercion.

4. **PASS — I2C descriptor.** `third_party/acpi-loganfy/src/aml/resource.rs:568-606` checks common/type-data
   lengths and NUL-terminated UTF-8 source bounds, decodes the fields and vendor bytes, and fails closed for
   SPI/UART/CSI-2/vendor types. KATs cover vendor data and unsupported/truncated descriptors
   (`tools/acpi-aml-grade/tests/fix3_kats.rs:146-204`).

5. **PASS — GPIO descriptors.** `third_party/acpi-loganfy/src/aml/resource.rs:647-699` bounds-checks fixed size,
   offsets, pin-table parity, source range, and vendor range before slicing; KATs cover GpioIo, GpioInt, and a
   malformed descriptor (`tools/acpi-aml-grade/tests/fix3_kats.rs:405-488`).

6. **PASS — IndexField/BankField sequencing.** `third_party/acpi-loganfy/src/aml/mod.rs:2958-2977,3057-3101`
   aligns accesses to access width, loops across datums, writes IndexField offset before data access, and writes
   BankField selector before region access. The KATs assert ordered accesses and the 16-bit split at
   `tools/acpi-aml-grade/tests/fix3_kats.rs:293-330,340-363`; an overflowing index is rejected before access
   (`:372-397`).

7. **PASS — Acquire.** `third_party/acpi-loganfy/src/aml/mod.rs:1045-1072` maps acquisition to Zero and
   `MutexAcquireTimeout` to Ones, then contributes that integer as the result. The acquired result reaching
   Local0 is tested at `tools/acpi-aml-grade/tests/fix3_kats.rs:246-265`. Limitation: the grade handler always
   grants, so timeout is code-reviewed rather than exercised; this limitation is documented in findings.

8. **PASS — field read size.** `third_party/acpi-loganfy/src/aml/mod.rs:2921-2933` uses `bit_length.div_ceil(8)`
   and caps allocation. The 72-bit KAT expects 9 bytes (`tools/acpi-aml-grade/tests/fix3_kats.rs:222-239`).

9. **PASS — AccessAs.** `third_party/acpi-loganfy/src/aml/mod.rs:2058-2080` updates access-type bits in the
   running field flags; subsequent named fields receive the current flags at `:2115-2123`. The KAT checks
   pre-AccessAs and post-AccessAs widths (`tools/acpi-aml-grade/tests/fix3_kats.rs:496-524`).

10. **PASS — Store to Buffer/String.** `third_party/acpi-loganfy/src/aml/mod.rs:2836-2864` keeps non-empty
    Buffer size, zero-fills/truncates, grows an empty Buffer only under `STORE_GROWS_EMPTY_BUFFER`, appends NUL
    to String-to-Buffer, takes source length for String-to-String, and fails closed for other String sources.
    The main KAT and the added NUL KAT are at `tools/acpi-aml-grade/tests/fix3_kats.rs:651-742`.

11. **PASS — LNot grammar/behavior.** `third_party/acpi-loganfy/src/aml/mod.rs:1924-1931` parses standalone LNot
    with one TermArg and the prefixed comparison forms with two; runtime returns Ones at the table width
    (`:2224-2234`). KATs cover the If result, Store target behavior, revision-width Ones, and prefixed pairs
    (`tools/acpi-aml-grade/tests/fix3_kats.rs:538-621`). Checked against ACPI 6.5 §20.2.5.4 and ACPICA raw
    `acopcode.h` `ARGP_LNOT_OP`.

12. **PASS — KAT byte evidence.** Spot-checked comparison and Store packages/PkgLengths
    (`tools/acpi-aml-grade/tests/fix3_kats.rs:55-126`), I2C descriptor bytes (`:146-170`), GPIO descriptor
    templates (`:405-488`), and Index/Bank field packages (`:293-363`). The test module cites raw ACPICA
    `amlcode.h` for opcodes (`fix3_kats.rs:5`).

13. **PASS — VALUES/coverage agreement.** `Plans/VALUES-2026-10-23-acpi-aml-fix3.md:9-21` records exactly one
    LNot-only completion (`EC0._INI`), 97 total rows all from FAIL/not-evaluated to a result, and no changed
    previously completed values or newly failing evaluations. The committed coverage report agrees on its
    aggregate counts (`tools/acpi-aml-grade/report/coverage.json:3200-3210`).

14. **PASS — panic census.** `third_party/acpi-loganfy/PANIC-CENSUS.md:17-19` records 58 sites, 47 unreachable,
    11 host-API-only, and 0 unclassified; the panic-census test passed in the required cargo test run. I found
    no new unchecked slice access in the reviewed resource decoders. Field-offset arithmetic in
    `third_party/acpi-loganfy/src/aml/mod.rs:2048-49,2130,2961-62,2999-3000` is not individually checked;
    the enclosing AML block/PkgLength bounds and 64-bit host limits bound the values in this tool's supported
    environment, so I do not classify it as an additional failure.

15. **PASS — committed fuzz evidence.** `docs/RESEARCH-2026-10-23-acpi-aml-fix3-FINDINGS.md:17` matches
    `tools/acpi-aml-grade/report/fuzz-fix1.json:2-7,96-98`: seed `0x0786`, 200,000 cases, zero panics/crashes,
    17 hangs. The hang corpus is retained in the committed report; the requested short run was separately
    clean for panics/crashes (3 hangs in 20,000 cases).

16. **PASS, with an additional gap recorded.** Findings state the revision-1 comparison integer-width gap and
    lack of corpus probes for items 3, 9, 10 and Acquire timeout (`docs/RESEARCH-2026-10-23-acpi-aml-fix3-FINDINGS.md:75-84`),
    and disclose String conversion-on-Store failing closed (`:78`). This review adds the item-3 right-operand
    conversion defect above; it is not stated as a known gap in findings.

## Review boundary

Review only; no Zenbook disk was accessed or written. Existing unrelated `.gitignore` modification and `.ignore`
untracked file were preserved and excluded from this commit.
