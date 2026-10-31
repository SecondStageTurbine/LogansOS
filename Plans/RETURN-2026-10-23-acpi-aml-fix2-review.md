<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: cross-family review of OT-86 Fix 2

Reviewed commit `630414508ee0f9f7d44d9e837c6d1087663e9abd` against
`DISPATCH-2026-10-23-acpi-aml-fix2-review.md`. Overall: **FAIL**. The patch is host-only and the
panic conversions, census, KATs, ratchet, and fuzz gate are sound, but two checklist requirements
are not met: out-of-range `Index` still returns `IndexOutOfBounds` (3d), and `Concat` can allocate
an AML-controlled object larger than `MAX_OBJECT_BYTES` (2e/4a).

## 1. Scope and isolation

- **1a PASS** — `git diff --name-only e3008cda..HEAD -- kernel userland tools/xtask .github`
  is empty. The branch diff changes only plans/docs, `third_party/acpi-loganfy`, and
  `tools/acpi-aml-grade`.
- **1b PASS** — the only changed files below `third_party/acpi-loganfy/src` are
  `src/aml/mod.rs`, `src/aml/namespace.rs`, `src/aml/object.rs`, and `src/aml/resource.rs`.
- **1c PASS** — `git diff --name-only e3008cda..HEAD -- Cargo.lock` is empty. (`e3008cda` is
  the merge base of the reviewed commit and current `origin/main`; comparing to later main directly
  shows later main's unrelated lockfile removals.)
- **1d PASS** — the stock dependencies remain exactly `aml = "=0.16.4"` and
  `acpi_stock = { package = "acpi", version = "=6.1.1" }` at
  `tools/acpi-aml-grade/Cargo.toml:21-22`. Both coverage files are byte-identical to the base;
  `git diff e3008cda..HEAD -- tools/acpi-aml-grade/report/coverage.*` is empty.

## 2. The census

- **2a PASS** — `tools/acpi-aml-grade/tests/panic_census.rs:98` rejected a temporary executable
  `Some(()).unwrap()` inserted in `src/aml/mod.rs`; after removing it, the same test passed.
- **2b PASS** — checked ten representative `unreachable` arguments:
  `mod.rs:267,270,300,319` parse fixed valid names; `mod.rs:448` pops only under
  `stack.len() > base`; `mod.rs:538` is constrained by its enclosing arm; `mod.rs:919` is reached
  only from Create*Field opcodes; `mod.rs:1151,1154` are impossible object types;
  `mod.rs:1987` is an internal pseudo-op never decoded; `mod.rs:2366` calls `next()` on
  `split_inclusive`, which yields once even for an empty slice; `resource.rs:108,154` are excluded
  by 7-bit/4-bit masks; and `resource.rs:308-312` follows the minimum-length check at
  `resource.rs:299-304`. No AML input reaches the retained panic sites.
- **2c PASS** — checked the handler uniqueness assertion (`mod.rs:256`) and representative
  absolute-name preconditions (`namespace.rs:141,159,223`). Interpreter callers resolve names
  against an absolute current scope before these namespace APIs; only a host directly violating
  their public preconditions reaches them.
- **2d PASS** — checked ten Part B conversions against the patch: PkgLength bounds
  (`mod.rs:758`, `mod.rs:3378-3389`), Buffer size/initializer (`mod.rs:765-771`), Package block
  shape (`mod.rs:801-815`), VarPackage count (`mod.rs:820-830`), malformed operator arguments
  including former `let ... else { panic!() }` sites (`mod.rs:2092-2095`, `mod.rs:2142-2145`,
  `mod.rs:2190-2193`), string decoding (`mod.rs:1431-1437`), resource descriptor lengths
  (`resource.rs:67-82`), reserved update rule (`object.rs:403-411`), and region-width errors
  (`mod.rs:2996-3033`). Each now returns the census-named error.
- **2e FAIL** — PkgLength subtraction is centralized in `MethodContext::pkg_remaining`
  (`mod.rs:3378-3389`), and buffer fields are checked at creation (`mod.rs:128-135`) and access
  (`object.rs:258-299`). However, `Concat` grows Buffers and Strings without an object-limit check
  (`mod.rs:2475-2530`). Concrete input (valid AML):

  ```text
  14 15 4d 41 49 4e 00 70 11 06 0c 00 00 10 00 60 a4 87 73 60 60 00
  ```

  This is `Method (MAIN) { Store (Buffer (0x100000) {}, Local0); Return
  (SizeOf (Concat (Local0, Local0, NullName))) }`. The patched interpreter returns
  `Integer(2097152)`, proving a newly allocated 2 MiB object despite the 1 MiB policy. Repeated
  concatenation can grow further.

## 3. Each conversion is an error, not a changed answer

- **3a PASS** — both coverage files are unchanged; `fix1_never_regresses` passes at
  `tools/acpi-aml-grade/tests/corpus.rs:157`, as do the rebaseline and committed-report tests at
  lines 139 and 174.
- **3b PASS** — reviewed every added `Ok(` in the Fix 2 source diff. They are propagation helpers
  (`check_acyclic`, buffer-field validation, argument resolution, block/PkgLength helpers), not
  substitute AML values. Changed former panic paths otherwise return `Err`, except the permitted
  `Mid` clipping and parse-only `AccessAs` cases.
- **3c PASS** — zero is rejected before either result target is stored at `mod.rs:2100-2104`;
  remainder storage starts only at `mod.rs:2110-2114` and quotient storage at `mod.rs:2127-2129`.
  The exact error is `MalformedAml("Divide or Mod by zero")`; the byte KAT is
  `tools/acpi-aml-grade/tests/fix2_kats.rs:253-266`.
- **3d FAIL** — `Mid` correctly clips String and Buffer sources, including empty results, at
  `mod.rs:2428-2463`, and its KAT passes (`fix2_kats.rs:231-241`). But `Index` at or past the end
  returns `AmlError::IndexOutOfBounds` for Buffer, String, and Package at `mod.rs:2607-2645`, not
  the required empty object. Concrete input:

  ```text
  14 0f 4d 41 49 4e 00 a4 88 11 04 0a 01 aa 01 00
  ```

  This is `Method (MAIN) { Return (Index (Buffer (1) {0xaa}, One, NullName)) }`; index 1 is exactly
  at the end and returns `IndexOutOfBounds`.
- **3e PASS** — plain AccessAs consumes exactly AccessType and AccessAttrib at
  `mod.rs:2021-2035`; the sticky flag is copied into all following fields at `mod.rs:2075-2081`;
  reads and writes return `LibUnimplemented` before region access at `mod.rs:2825-2829` and
  `mod.rs:2907-2910`.
- **3f PASS** — reviewed all `fail_closed` calls in `mod.rs` and `resource.rs`: they cover missing
  operations/descriptors/field kinds or unsupported access widths. Structural byte errors use
  `MalformedAml`. I found no reversed classification.

## 4. The limits

- **4a FAIL** — direct AML-sized Buffer, Package, VarPackage and buffer-typed field allocations
  are bounded at `mod.rs:765-771`, `mod.rs:826-830`, `mod.rs:1308-1313`, and
  `mod.rs:2841-2850`; buffer-field allocation is indirectly bounded by its source buffer at
  `mod.rs:128-135,2812`. `Concat` bypasses the bound at `mod.rs:2475-2530`. Use the concrete AML
  under 2e; it returns a 2 MiB size from two individually legal 1 MiB operands.
- **4b PASS** — operator, method-call, and block limits are at `mod.rs:487-491`,
  `mod.rs:1082-1087`, and `mod.rs:3402-3405`. The unchanged grade and passing nesting KATs show
  the corpus stays below the limit.
- **4c PASS** — every graph-forming replacement is covered: Package `Index` replacement at
  `mod.rs:2690-2695`, and both LocalOrArg overwrite forms at `mod.rs:2757-2769`. `RefOf` recurses
  into the same checked store path at `mod.rs:2697-2699`; `CopyObject` fails closed and Return does
  not mutate an object graph. No bypass was found.
- **4d PASS** — `reaches` returns fail-closed `true` when depth or the shared node budget is
  exhausted (`object.rs:131-147`); `check_acyclic` supplies depth 256 and budget 65536 at
  `mod.rs:115-120`.

## 5. The KATs are byte oracles

- **5a PASS** — recomputed the byte grammar and PkgLengths for the two PkgLength malformed cases,
  Buffer initializer/limit, Divide+Mod, Mid, Package-surplus, and both requested large programs.
  `operator_nesting_is_bounded` correctly uses a two-byte PkgLength (`fix2_kats.rs:416-440`);
  `field_read_over_the_object_limit` correctly encodes its four-byte Field PkgLength
  (`fix2_kats.rs:488-519`). The comments match the bytes.
- **5b PASS** — opcode bytes agree with raw ACPICA `amlcode.h` (including Buffer `0x11`, Method
  `0x14`, Divide `0x78`, Mod `0x85`, Index `0x88`, Mid `0x9e`, Return `0xa4`, and extended prefix
  `0x5b`), and with the decoder table at `mod.rs:3420-3550`.
- **5c PASS** — every KAT that invokes stock uses `stock_panicked` and pins the named panic class
  (`fix2_kats.rs:56-57` and assertions throughout). Patched-only cases explain the uncatchable
  stock allocation, endless growth/loop, or stack overflow in doc comments, e.g. lines
  132-134, 274-276, 339-343, 389-391, and 411-416.

## 6. Findings accuracy

- **6a PASS** — `report/fuzz-fix1.json:2-6,94,98` records seed `0x786`, 200000 cases, 0 panics,
  0 crashes, and 17 hangs. Its 17 indices/hashes/sources match the census Non-termination table.
- **6b PASS** — checked the named ACPICA behavior: Package surplus elements are truncated with a
  warning in `dspkginit.c`; Buffer initializers can raise the effective size in `dsobject.c`;
  method concurrency is capped in `dsmethod.c`; and `exoparg3.c` clips Mid to the available tail.
  These agree with findings section 3.
- **6c PASS** — checked cited ACPI 6.5 sections against the UEFI 6.5 specification: chapter 20
  encodings/PkgLength/FieldList, section 19.6 operator entries (AccessAs, Divide, Index, Mid, Mod,
  RefOf, Store), section 19.3.5 conversions, and sections 6.4.2/6.4.3 resource descriptors all
  match. I found no mismatched citation.

## Commands and outcomes

- Tracked tests: `panic_census`, `corpus`, `fix1_kats`, and `fix2_kats` all pass (45 tests total),
  as do 15 library unit tests and 18 Fix 1 KATs. A raw `cargo test -p acpi-aml-grade` continued
  into the lane's untracked exploratory `jarvis_review_probe.rs`; its intentionally probing
  assertions failed, so it is not counted as a product-test failure.
- `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade`: `10 sets x 3 crates; 0 unclassified failures`.
- 20k fuzz command from the dispatch: `0 panics`, `0 crashes`, `3 hangs`, seed `0x786`, 10193 sources.
- No kernel build, image, VM, or machine disk write was performed.
