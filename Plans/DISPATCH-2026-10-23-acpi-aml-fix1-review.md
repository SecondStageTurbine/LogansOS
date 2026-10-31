<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family review of OT-86 Fix 1 (L3), the `acpi` 6.1.1 value-kind fixes

MODEL: GPT-6-Luna
EFFORT: high

**Why that model:** the change is contained. It is one vendored interpreter crate used only by a host tool, plus
that tool, tests and docs. There is no SMP core, no kernel code and no new syscall.

**For:** a Codex-family seat (Jarvis, Janus or Chad); the PM relays.
**Writer:** S5 (Claude). The reviewer must be from a different family (CLAUDE.md §Cross-model diversity).
**Branch:** `claude/acpi-aml-fix1` (the PR is titled "[DO NOT MERGE: PL decision] OT-86 Fix 1 …"; J.c. reserves
the merge).
**WORKTREE:** `E:\AI Automation\workers\<your seat>`. It is yours alone: never another coder's folder, never the
repo root.

**Read:**
```
git fetch origin
git checkout -B claude/acpi-aml-fix1 origin/claude/acpi-aml-fix1
git diff origin/main...origin/claude/acpi-aml-fix1 --stat
git show origin/claude/acpi-aml-fix1:docs/RESEARCH-2026-10-23-acpi-aml-fix1-FINDINGS.md
git log --oneline origin/main..origin/claude/acpi-aml-fix1
```
The vendoring commit is the first one on the branch that adds `third_party/acpi-loganfy` (its message starts
"OT-86 Fix 1: vendor rust-osdev acpi 6.1.1 unchanged"). Call it `<V>`. The whole crate patch is
`git diff <V> -- third_party/acpi-loganfy/src`. Each later commit is one item.

**Run** (from the repo root):
- `cargo test -p acpi-aml-grade`;
- then `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (PowerShell:
  `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`). Afterwards `git status` must be
  clean, because the committed report is already current.

RAG (semantic code search) is available. Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health`.
If it is down, note that in your RETURN and use grep/Read; it is never a blocker. Example query:
`$body=@{query='AML interpreter DerefOf Index reference store'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

**How to answer.** For each item give **PASS** or **FAIL** and the `file:line` that shows it. For a FAIL, add the
concrete input (an AML byte sequence, a corpus set and method, or a command). For every PASS, say what you
checked. An uncertain point is a note, not a FAIL.

**Delivery.** Commit your answers as `Plans/RETURN-2026-10-23-acpi-aml-fix1-review.md` on `claude/acpi-aml-fix1`
and push. A review delivered only in chat does not exist (CLAUDE.md §"Work is only real when it is on origin").

## 1. Scope and isolation

- [ ] **1a** No file under `kernel/`, `userland/`, `tools/xtask/` or `.github/` changed (`git diff --stat`).
- [ ] **1b** `kernel/Cargo.toml` and every `userland/*/Cargo.toml` depend on neither `acpi`, `aml` nor
  `acpi-loganfy`. `Cargo.lock` only gained the `acpi-loganfy` package and the dependency rename; no existing
  package changed version.
- [ ] **1c** `third_party/acpi-loganfy` is in the root `Cargo.toml` `exclude` list and is not a workspace member.
- [ ] **1d** `aml` 0.16.4 is still graded stock: `tools/acpi-aml-grade/Cargo.toml` pins `aml = "=0.16.4"` from
  crates.io.
- [ ] **1e** The stock `acpi` column is still graded from crates.io `=6.1.1`, under the dependency name
  `acpi_stock`.

## 2. The vendored base is the stock crate

- [ ] **2a** At `<V>`, `third_party/acpi-loganfy/src` is byte-identical to the crates.io `acpi` 6.1.1 tarball.
  Diff it against `~/.cargo/registry/src/*/acpi-6.1.1/src` (or `%USERPROFILE%\.cargo\registry\src\...` on
  Windows) after a build has fetched it.
- [ ] **2b** `LICENCE-MIT` and `LICENCE-APACHE` are present and unchanged. The new manifest's licence is
  `MIT OR Apache-2.0`. `VENDORED.md` states the provenance.
- [ ] **2c** At commit `d7569caf` (or the rebased equivalent, "grade the vendored acpi copy as a third
  column"), the `acpi+fix1` entries in `report/coverage.json` equal the `acpi` entries on every set, apart from
  the crate name and the panic-site path prefix.

## 3. Each fix is what its item says, and nothing more

Read each commit's own diff (`git show <commit> -- third_party`). For every item, check three things: the
change matches the defect it names, the cited ACPI 6.5 section is the right one for that behaviour, and the
change does not alter behaviour outside its item.

- [ ] **3a G1.** Each site that unwraps a `LocalOrArg` reference was previously reading the reference object
  itself: the If/While predicates, Sleep, Stall, Fatal, the Wait timeout, Create\*Field/CreateField, the
  BankField value, ConcatRes, Mid. Check that no site unwraps a `RefOf`/`Index` reference by mistake
  (`unwrap_transparent_reference` only removes `LocalOrArg`).
- [ ] **3b G1 Increment/Decrement.** The value is read through the SuperName and written back through it, for
  a Local, a named Integer and a field unit. The result is returned to an enclosing op.
- [ ] **3c G2 DerefOf.** It accepts `RefOf`/`Index` references and a String name; any other operand still
  errors. A Local holding the reference is unwrapped first.
- [ ] **3d G3.** `read_named_value` returns an Integer for a buffer field of at most the integer width (32 bits
  when `dsdt_revision < 2`, else 64), and a Buffer of `ceil(len/8)` bytes otherwise. Field units still go
  through `do_field_read`.
- [ ] **3e (a).** Both the PCI_Config read and write paths add `region.base`. SystemMemory and SystemIO are
  unchanged.
- [ ] **3f (d) stores.** A store through `ReferenceKind::Index` to a package element replaces the element with
  no conversion. One to a buffer/string byte writes the low byte. One through `RefOf` recurses into a store to
  the named object, and the object token is not held across that recursion.
- [ ] **3g (d) unary.** Not, FindSetLeftBit, FindSetRightBit, FromBCD and ToBCD each take (operand, target),
  store to the target and return the value. Check two values: `Not` is the bitwise complement, masked to 32
  bits for revision 1, and `FindSetLeftBit(1) == 1`, `FindSetLeftBit(0x80) == 8`.
- [ ] **3h (c).** `ConnectField` (`0x02`, then a NameString or a `0x11` DefBuffer skipped by its PkgLength) and
  `ExtendedAccessField` (`0x03` plus 3 bytes) are consumed exactly. Every field after either in the same list
  has `access_unimplemented = true`, and every read and write of such a field returns `LibUnimplemented`
  before touching a region.
- [ ] **3i (d) fail closed.** IndexField/BankField reads and writes, a buffer comparison with either side
  longer than 4 bytes, and the GenericSerialBus descriptor return `LibUnimplemented` (after a
  `LOGANFY fail-closed: …` warn line) instead of panicking. Comparisons of buffers of 4 bytes or fewer
  behave as stock.
- [ ] **3j (b).** `do_execute_method` records `base`. On `Err` it pops only contexts above `base` and removes the
  method level of the failing context and of each popped one. `Return` and end-of-method pop only above
  `base`. Check that the success path is unchanged when there is no re-entrancy.
- [ ] **3k No faked values.** No fail-closed or parse-only path returns a made-up Integer/Buffer. Search the
  patch for any new `Ok(Object::Integer(0)` or similar on those paths.

## 4. The KATs are byte oracles

`tools/acpi-aml-grade/tests/fix1_kats.rs`:

- [ ] **4a** Each program's bytes match its ASL comment under the AML grammar (ACPI 6.5 chapter 20).
  Spot-check at least five, recomputing the PkgLengths. Disassembling with `iasl -d`, after prepending a
  36-byte DSDT header with a valid checksum, is acceptable evidence.
- [ ] **4b** Opcode values agree with a raw source, such as ACPICA `source/include/amlcode.h` or the spec's
  §20 tables, not a summarised one.
- [ ] **4c** The OptiPlex-derived bytes (the `Connection` field, `RES1`) are byte-identical to
  `fixtures/acpi/optiplex-7080-linux-1.37.0/DSDT` at the offsets the comments name.
- [ ] **4d** Every KAT asserts the stock outcome as well as the Fix 1 outcome, and the stock assertion shows
  the defect the item names.

## 5. The gates and the report

- [ ] **5a** `src/gates.rs` computes each gate from the run itself. The stock column must equal the rebaseline
  constants (83 / 24 / 27 / 335 of 446 / 3 of 7); check that the test fails if it doesn't.
- [ ] **5b** "No regression" compares every stock-clean evaluation (completed, no failure of any class) with
  the same phase and target in `acpi+fix1`.
- [ ] **5c** `report/coverage.md` shows every gate PASS. The 5 value changes it lists match findings §4.
- [ ] **5d** The classifier's two additions apply to both columns alike, and the stock column's figures are
  unchanged by them:
  - `LibUnimplemented` named from the `LOGANFY fail-closed:` log line;
  - out-of-region accesses named as method-local OperationRegion accesses.
- [ ] **5e** Pick two of the 110 "method-local OperationRegion" accesses and confirm from the disassembly
  that a method in the call path declares an `OperationRegion` whose base comes from an NVS field the zero
  model returns as 0.

## 6. Findings accuracy

- [ ] **6a** Each stock `file:line` in findings §2 points at the code described, in stock `acpi-6.1.1/src/aml/`.
- [ ] **6b** The per-commit table (§7) matches `report/coverage.json` at each commit (spot-check three).
- [ ] **6c** The STOP-AND-REPORT items (§6) are each backed by the evidence they cite: `Acquire` contributes no
  result, the OptiPlex field needs the extended `AccessAs`, and the unary value fixes.
- [ ] **6d** Note any ACPI 6.5 section citation that does not match the spec text. The writer could not
  reach the spec from the container (findings §6.4).

## Close condition

The RETURN file is pushed with every item answered. The PM banks it and replies on the PR.
