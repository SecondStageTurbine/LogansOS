<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: cross-family review of OT-86 Fix 1 (L3)

Reviewed PR commit `1be753d3` and the dispatch on `origin/claude/acpi-aml-fix1`.

## 1. Scope and isolation

- **1a PASS** — `git diff --stat origin/main...1be753d3` has no paths under `kernel/`, `userland/`, `tools/xtask/`, or `.github/`.
- **1b PASS** — kernel and userland manifests have no ACPI interpreter dependencies. The `Cargo.lock` change adds `acpi-loganfy` and its package/dependency entries; no existing package version changes.
- **1c PASS** — root `Cargo.toml:49-59` excludes `third_party/acpi-loganfy`; it is not listed among workspace members (`Cargo.toml:4-47`).
- **1d PASS** — `tools/acpi-aml-grade/Cargo.toml:20` pins `aml = "=0.16.4"` from crates.io.
- **1e PASS** — `tools/acpi-aml-grade/Cargo.toml:21` grades crates.io `acpi = "=6.1.1"` under `acpi_stock`.

## 2. Vendored base

- **2a PASS** — at vendoring commit `7645a1b3`, the source is the unmodified stock copy; the current edits are the subsequent Fix 1 patches plus the final SPDX-only commit. The registry `acpi-6.1.1` source fetched for this review matches the base described by `third_party/acpi-loganfy/VENDORED.md:15-29`.
- **2b PASS** — `LICENCE-MIT`, `LICENCE-APACHE`, and the `MIT OR Apache-2.0` manifest licence are present at the vendoring commit; `VENDORED.md:1-13` records provenance.
- **2c PASS** — the report at `d7569caf` records stock and `acpi+fix1` as equal on each of the ten corpus sets. It uses the third column name and its panic path prefix; no outcome difference is described.

## 3. Fix review

- **3a PASS** — `third_party/acpi-loganfy/src/aml/mod.rs:516-517, 571, 639, 772, 809-835, 884, 907, 913, 1042, 1063, 2213-2216` unwrap Local/Arg references for ConcatRes, Wait, Fatal, If/While, Create*Field/CreateField, dereference, Sleep/Stall, BankField, and Mid. `unwrap_transparent_reference` only unwraps `LocalOrArg` (`aml/object.rs:139-154`), leaving `RefOf`/`Index` intact.
- **3b PASS** — `aml/mod.rs:467-494` reads the operand value, stores the incremented/decremented value through the original SuperName and contributes the store result to the enclosing op. KAT `g1_while_increment_decrement_on_locals` covers Local behavior; other covered use is in the same generic store path.
- **3c PASS** — `aml/mod.rs:872-904` accepts only RefOf/Index references and strings, unwraps a Local/Arg first, reads the referenced/named value, and returns `ObjectNotOfExpectedType` for other types.
- **3d PASS** — `aml/mod.rs:2553-2574` returns field-unit reads through `do_field_read`; buffer fields become an Integer at or below the DSDT integer width (32 bits for revision <2, otherwise 64) and a `ceil(len/8)` Buffer above that width. `do_field_read` starts at `:2576`.
- **3e PASS** — `aml/mod.rs:2720-2760` and `2790-2830` add `region.base` in PCI_Config read and write branches. SystemMemory and SystemIO branches remain unchanged. ACPI 6.5 §19.6.99 defines the OperationRegion offset as the starting offset in the selected space.
- **3f PASS** — `aml/mod.rs:2330-2448` handles Index package assignment by replacing the element without conversion, buffer/string byte assignment with the low byte, and RefOf by recursively storing to the named object. The object token is dropped before recursive RefOf storage (`:2380-2401`).
- **3g PASS** — `aml/mod.rs:1927-1987` gives Not, FindSetLeftBit/RightBit, FromBCD and ToBCD source plus target, stores their value, and contributes the value. The same block masks Not to the revision-1 integer width and computes one-based left-bit positions. `fix1_kats.rs:326-367` checks `Not` and the requested `FindSetLeftBit` values.
- **3h PASS** — `aml/mod.rs:1848-1875` consumes ConnectField NameString or PkgLength-delimited BufferData, and ExtendedAccessField plus its three bytes; `:1818-1824, 1877-1895` marks subsequent fields. Reads and writes reject marked fields before region access (`:2575-2584`, `:2646-2655`).
- **3i PASS** — IndexField/BankField accesses fail closed at `aml/mod.rs:2600-2615, 2660-2672`; long-buffer comparisons fail closed at `:2016-2028`; GenericSerialBus descriptors fail closed at `aml/resource.rs:90-94`. These paths use `fail_closed` (`aml/mod.rs:93-96`) before returning `LibUnimplemented`. The short-buffer comparison implementation is unchanged. KATs cover all cases.
- **3j PASS** — `aml/mod.rs:382-414` records `base`, unwinds only contexts above it, and removes method levels for the failing and popped contexts on error. Return/end-of-method paths call `pop_caller_context(base)` (`:982-994, 1080-1103`); the helper only pops above base (`:1801-1806`). No-reentrancy normal completion follows the prior branch behavior. Both error and reentrant KATs pass.
- **3k PASS** — searching the source patch for new `Ok(Object::Integer(0` or similar found no made-up success values on parse-only/fail-closed paths. They return `LibUnimplemented`.

## 4. KAT review

- **4a PASS** — inspected the AML comments and package lengths for the Local predicate, DerefOf Index, BufferField, PCI_Config, and Connection/ExtendedAccess KATs (`fix1_kats.rs:70-155, 174-225, 370-415, 486-518`). The packages close at the comment-marked boundaries, and all 18 KATs execute successfully. The test run does not itself disassemble AML; the findings report says ACPICA `iasl` independently decoded the KATs.
- **4b PASS** — opcode values in the KATs agree with the crate opcode table (`aml/mod.rs:3150-3210`) and the AML §20 tables, including IndexField `0x5B86`, FindSetLeftBit `0x81`, and Connection/ExtendedAccess prefixes `0x02`/`0x03`.
- **4c PASS** — `fix1_kats.rs:486-518` identifies the OptiPlex DSDT offset `0x4ab9c` for the Connection field; `:565-601` identifies the `RES1` GenericSerialBus descriptor bytes and corpus source. The tests compare the verbatim bytes to those expected.
- **4d PASS** — every KAT uses the dual runner and asserts both stock and Fix 1 outcomes; the stock assertions reproduce the named panic, wrong result, or missing behavior (see `fix1_kats.rs:46-68` and the 18 test bodies).

## 5. Gates and report

- **5a PASS** — `src/gates.rs:16-20, 104-143, 221-227` computes the measures from the `Run` and checks rebaseline constants 83 / 24 / 27 / 335 of 446 / 3 of 7. `tests/corpus.rs:139-153` asserts the gate and committed-report checks; `cargo test -p acpi-aml-grade` passes.
- **5b PASS** — `src/gates.rs:106-138` compares each stock-clean evaluation against the same set, phase, and target in Fix 1 and records failure or missing evaluations as regressions.
- **5c PASS** — `report/coverage.md:50-60` shows all gates PASS. Its five value changes match findings §4. `cargo run -p acpi-aml-grade` regenerated no report diff.
- **5d PASS** — `src/classify.rs:562-576` classifies fail-closed log names, and `:337-386` classifies method-local region accesses. Both use the shared classifier for both crate columns. Current stock totals remain the pinned figures, and the report has 110 method-local accesses only in Fix 1.
- **5e PASS, with evidence note** — the report's example at `coverage.md:75` names `_INI \_SB_.PCI0.I2C0.TPD0._INI` and candidates `SHPO`/`SRXO`; findings §5 (`:138-149`) describes `PDW0`, `GADR`, and GNVS `SBRG`, whose zero-model value yields the reported runtime address. The findings say these were disassembled/traced. I did not independently rerun `iasl` against the source DSDTs during this review, so this item relies on that recorded analysis.

## 6. Findings accuracy

- **6a PASS** — spot-checked the cited stock panic/type sites in the pinned `acpi-6.1.1/src/aml/` source, including `mod.rs:813`, `1741`, `1889`, `2318`, `2387`, `2447`, and `resource.rs:89`; they point to the described unwrap/todo/panic sites.
- **6b PASS** — spot-checked three entries from findings §7 against report snapshots at their commits: `d7569caf` (baseline), `c14310f9` (G2/G3), and `010356a7` (c + fail closed). Their recorded ONOET, QEMU wrong-access, panic, DSDT, and real-hardware figures agree.
- **6c PASS** — `aml/mod.rs:834-851` confirms Acquire retires without contributing a result. The ACPI 6.5 §19.6.2 signature is Boolean (represented in AML as Zero/Ones); the traced Uninitialized Local and subsequent comparison failure are consistent with that. OptiPlex's `Connection (RES1)` is followed by `AccessAs (BufferAcc, AttribBytes ...)` (`fix1_kats.rs:486-518`), so parsing only Connection would leave the extended field-list element unconsumed. The unary fixes match the operator syntax/results and the KAT expectations (`:326-367`).
- **6d PASS** — checked the citations in findings §2 against the ACPI 6.5 pages on uefi.org. §19.6 operator citations (If/While, DerefOf, Increment/Decrement, Not/FindSetLeftBit, OperationRegion, etc.) match the named operators; §20.2.5.2 is the AML FieldList encoding and includes `ConnectField`/`ExtendedAccessField`; §20.2.5.4 is the operator encoding table. §19.3.5 is the ASL Data Types section; the more precise Local/Arg read rules are §19.3.5.8.1 and §19.3.5.8.2, so the findings' citation is broad but not wrong. No citation was found to name a different clause or contradict its cited behavior. Sources checked: [ACPI 6.5 ASL Reference](https://uefi.org/specs/ACPI/6.5/19_ASL_Reference.html) and [ACPI 6.5 AML Specification](https://uefi.org/specs/ACPI/6.5/20_AML_Specification.html).

## Verification and worktree

`cargo test -p acpi-aml-grade` passed (9 unit tests, 5 corpus/gate tests, 18 KATs). With `CARGO_PROFILE_DEV_PANIC=unwind`, `cargo run -p acpi-aml-grade` graded 10 sets × 3 crates with 0 unclassified failures and made no report changes. `cargo clean` removed the generated build artifacts. Pre-existing `.gitignore` modification and untracked `.ignore` were present before review and were left untouched and uncommitted.
