<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: narrow re-check of PR #104

Reviewed `c910b0b9e8fc85c4ab1819de10b7025d4c744421` against
`DISPATCH-2026-10-23-acpi-aml-fix2-recheck-jarvis.md`.

**Verdict: CLOSED.** Both findings from review round 1 are resolved, and the requested regression gates pass.

## 1. 2e/4a closed — PASS

`check_object_size` rejects a result longer than `MAX_OBJECT_BYTES` at
`third_party/acpi-loganfy/src/aml/mod.rs:109-113`. It is applied to every result-growing path named by the
dispatch:

- `ConcatRes`, after combining both templates and adding the two-byte end tag, at `mod.rs:574-592`;
- `ToBuffer` of a String, after adding its NUL terminator, at `mod.rs:2300-2308`;
- Buffer forms of `ToDecimalString` and `ToHexString`, at `mod.rs:2411-2429`;
- Buffer and String forms of `Concat`, before constructing the result object, at `mod.rs:2535-2548`.

The reviewer's exact AML input is now the KAT `concat_result_over_the_object_limit` at
`tools/acpi-aml-grade/tests/fix2_kats.rs:605-629`. Stock returns `Integer(2097152)`; the patched interpreter
returns `MalformedAml("result larger than the interpreter's object limit")` at lines 621-628. The KAT passed.

The author's remaining audit claim holds:

- `Mid` takes a slice no larger than its String/Buffer source at `mod.rs:2442-2477`.
- `ToString` takes at most a prefix of its Buffer source at `mod.rs:2368-2388`.
- `Store` copies or converts its source rather than growing it at `mod.rs:2674-2804`.

I reviewed every `Object::Buffer`, `Object::String`, and `Object::Package` construction and the associated
`Vec`/`String` growth in `src/aml`. I found no other operator that can build an object over
`MAX_OBJECT_BYTES` from legal operands. The checks occur after transient allocation in `ConcatRes`,
`ToBuffer`, and decimal/hex conversion; as documented at `mod.rs:105-108`, each operand is already bounded,
so those transients are bounded by a small constant multiple of 1 MiB.

## 2. 3d resolved — PASS

The earlier review interpreted the dispatch incorrectly. The authoritative ACPI 6.5 text defines `Index` as
returning a reference to the indexed Buffer byte, String character, or Package element; it does not prescribe an
empty object for an out-of-range index. ACPICA's `AcpiExOpcode_2A_1T_1R` returns
`AE_AML_BUFFER_LIMIT`, `AE_AML_STRING_LIMIT`, or `AE_AML_PACKAGE_LIMIT`, matching the crate's
`IndexOutOfBounds` branches at `third_party/acpi-loganfy/src/aml/mod.rs:2626-2664`.

The exact review input is pinned by `index_past_the_end_is_an_error` at
`tools/acpi-aml-grade/tests/fix2_kats.rs:655-673`; both stock and patched interpreters return
`IndexOutOfBounds`. The KAT passed.

The corrected original dispatch accurately distinguishes the cases at
`Plans/DISPATCH-2026-10-23-acpi-aml-fix2-review.md:84-89`: `Mid` with its Index argument past the source
returns an empty object, while the `Index` operator past the source returns an error. Findings section 7 states
the same rule at `docs/RESEARCH-2026-10-23-acpi-aml-fix2-FINDINGS.md:175-179`.

**Citation note:** the re-check dispatch asks for ACPI 6.5 §19.6.63, but in ACPI 6.5 the `Index` operator is
§19.6.62 and §19.6.63 is `IndexField`. `Index` became §19.6.63 in ACPI 6.6. This numbering error does not
change the semantic verdict. Source checked: UEFI Forum, ACPI 6.5 online specification,
`https://uefi.org/specs/ACPI/6.5/`.

## 3. Nothing else regressed — PASS

- `cargo test -p acpi-aml-grade` passed: 15 library unit tests, 6 corpus/gate tests, 18 Fix 1 KATs,
  23 Fix 2 KATs, and the panic-census test.
- The census scanner passed at `tools/acpi-aml-grade/tests/panic_census.rs:88-108`; every retained explicit
  site is classified `unreachable` or `host-API`, so there are 0 unclassified sites.
- `fix1_never_regresses` passed at `tools/acpi-aml-grade/tests/corpus.rs:157-170`.
- `committed_report_matches_the_harness` passed at `tools/acpi-aml-grade/tests/corpus.rs:173-187`, and
  `git diff origin/main -- tools/acpi-aml-grade/report/coverage.json
  tools/acpi-aml-grade/report/coverage.md` is empty.
- The review-only untracked probe was temporarily moved out of Cargo's integration-test discovery for the exact
  command, then restored byte-for-byte. It is not part of PR #104.
- `cargo clean` removed all artifacts produced by this re-check. No kernel build, image, VM, or machine-disk
  write was performed.
