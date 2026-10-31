<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family review of OT-86 Fix 2, the panic-free `acpi-loganfy` AML interpreter

MODEL: GPT-6-Luna
EFFORT: high

**Why that model:** the change is contained. It touches one vendored interpreter crate (`src/aml` only), which
only a host tool uses, plus that tool, tests and docs. There is no SMP core, no kernel code and no new syscall.

**For:** a Codex-family seat (Jarvis, Janus or Chad); the PM relays.
**Writer:** S5 (Claude). The reviewer must be from a different family (CLAUDE.md §Cross-model diversity).
**Branch:** `claude/acpi-aml-fix1`. The PR is titled "[DO NOT MERGE: PL decision] OT-86 Fix 2 …", and J.c.
reserves the merge.
**WORKTREE:** `E:\AI Automation\workers\<your seat>`. It is yours alone: never another coder's folder, never the
repo root.

**Read:**
```
git fetch origin
git checkout -B claude/acpi-aml-fix1 origin/claude/acpi-aml-fix1
git diff origin/main...origin/claude/acpi-aml-fix1 --stat
git show origin/claude/acpi-aml-fix1:docs/RESEARCH-2026-10-23-acpi-aml-fix2-FINDINGS.md
git show origin/claude/acpi-aml-fix1:third_party/acpi-loganfy/PANIC-CENSUS.md
git log --oneline origin/main..origin/claude/acpi-aml-fix1
```
The crate patch for Fix 2 alone is `git diff origin/main...origin/claude/acpi-aml-fix1 -- third_party/acpi-loganfy/src`.
The whole patch against upstream is a diff against the crates.io copy Cargo downloads:
`diff -ru ~/.cargo/registry/src/*/acpi-6.1.1/src third_party/acpi-loganfy/src`.

**Run** (from the repo root):
- `cargo test -p acpi-aml-grade` (PowerShell: the same);
- `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (PowerShell:
  `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`). Afterwards `git status` must be clean,
  because the committed report is already current;
- a short fuzz run: `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- run --seed 0x0786 --cases 20000 --jobs 4`.
  It must report 0 panics and 0 crashes. The full 200k run is in the findings (§2); rerun it if you have time.

RAG (semantic code search) is available. Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health`.
If it is down, note that in your RETURN and use grep/Read; it is never a blocker. Example query:
`$body=@{query='AML interpreter PkgLength block bounds MalformedAml'; top_k=5}|ConvertTo-Json; Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

**How to answer.** For each item give **PASS** or **FAIL** and the `file:line` that shows it. For a FAIL, add the
concrete input (an AML byte sequence, a fuzz seed and case index, or a command). For every PASS, say what you
checked. An uncertain point is a note, not a FAIL.

**Delivery.** Commit your answers as `Plans/RETURN-2026-10-23-acpi-aml-fix2-review.md` on `claude/acpi-aml-fix1`
and push. A review delivered only in chat does not exist (CLAUDE.md §"Work is only real when it is on origin").

## 1. Scope and isolation

- [ ] **1a** No file under `kernel/`, `userland/`, `tools/xtask/` or `.github/` changed (`git diff --stat`).
- [ ] **1b** Within `third_party/acpi-loganfy/src`, only files under `src/aml/` changed on this branch.
- [ ] **1c** `Cargo.lock` is unchanged on this branch, or changed only in `acpi-aml-grade`'s own entry.
- [ ] **1d** The stock `acpi` and `aml` columns are still graded from crates.io (`=6.1.1`, `=0.16.4`), and the
  stock column of `report/coverage.json` is unchanged from `origin/main`.

## 2. The census

`third_party/acpi-loganfy/PANIC-CENSUS.md`, enforced by `tools/acpi-aml-grade/tests/panic_census.rs`:

- [ ] **2a** The test's scan finds what it should. Add a line containing `.unwrap()` to any `src/aml` file and check
  that the test fails, then remove it.
- [ ] **2b** Pick at least eight Part A rows marked `unreachable`, including `mod.rs` `split_inclusive`,
  `resource.rs` `address_fields.next().unwrap()` and one `unreachable!("…")`. For each, check that the argument
  in the row holds in the code: no AML input reaches the site.
- [ ] **2c** Pick at least three Part A rows marked `host-API`. For each, check that every interpreter call path
  satisfies the precondition, so only a host calling the public API another way can reach it.
- [ ] **2d** Part B: pick ten stock sites, at least three of them `panic!()` lines inside `let … else`. For each,
  check that the patched code at that place returns the error the row names.
- [ ] **2e** The "Implicit panics" section: check its claims for at least two classes (for example, every
  `pkg_length` subtraction goes through `pkg_remaining`; every buffer-field access goes through
  `check_buffer_field` or the bounds checks in `object.rs`). Note any slice index or arithmetic on AML-derived
  values that is neither checked nor guarded, with an input that reaches it if you can build one.

## 3. Each conversion is an error, not a changed answer

- [ ] **3a** The corpus report is unchanged by Fix 2: `report/coverage.json` and `coverage.md` on this branch equal
  `origin/main`'s (`git diff origin/main -- tools/acpi-aml-grade/report/coverage.*` is empty). The ratchet test
  `fix1_never_regresses` passes.
- [ ] **3b** No conversion returns a made-up value. Search the Fix 2 diff for new `Ok(` on the paths that used to
  panic; each should be an `Err(…)`, the clipped `Mid` (3d) or the parse-only `AccessAs` (3e).
- [ ] **3c** Divide and Mod by zero return `MalformedAml("Divide or Mod by zero")` and nothing is stored to either
  target first (ACPI 6.5 §19.6 `Divide`, `Mod`; ACPICA AE_AML_DIVIDE_BY_ZERO).
- [ ] **3d** `Mid` past the end returns the bytes that exist (ACPICA `exoparg3.c` `AcpiExOpcode_3A_1T_1R`) for
  both a String and a Buffer source, and an empty object when `Index` is at or past the end of the source.
  (Corrected after the first review round: the first version said the `Index` *operator* past the end returns an
  empty object. It does not, and should not: ACPICA returns AE_AML_BUFFER_LIMIT / AE_AML_STRING_LIMIT /
  AE_AML_PACKAGE_LIMIT, `exoparg2.c`, and the crate returns `IndexOutOfBounds`; KAT
  `index_past_the_end_is_an_error`.)
- [ ] **3e** A plain `AccessAs` in a field list is consumed exactly (two bytes after `0x01`). Every later field in
  that list is marked `access_unimplemented`, and its reads and writes return `LibUnimplemented` before touching a
  region.
- [ ] **3f** `fail_closed` is used only where the crate lacks the feature (unimplemented opcodes, descriptors,
  region widths), and `MalformedAml` only where the bytes are malformed. Note any site where the other one fits
  better.

## 4. The limits

- [ ] **4a** `MAX_OBJECT_BYTES` (1 MiB) bounds every AML-sized allocation: `Buffer`, `Package`, `VarPackage` and a
  buffer-typed field read. Look for any other `vec![…; n]`, `Vec::with_capacity(n)` or `resize` whose `n` comes
  from AML.
- [ ] **4b** `MAX_NESTING` (256) bounds block nesting (`start_new_block`), method-call depth (InternalMethodCall)
  and in-flight operators (the dispatch loop). Check that the corpus never comes near it: the grade is unchanged
  (3a).
- [ ] **4c** `check_acyclic` runs on every store that can make one object reach another: the `Index` element
  replace and both `LocalOrArg` overwrite paths in `do_store`. Look for a store path that skips it (for example a
  `RefOf` store into a Package, `CopyObject`, or a method `Return` of a reference to its own Local).
- [ ] **4d** `reaches` is bounded: `max_depth` and the shared `budget` stop it on a deep or wide graph, and it
  answers "reaches" (fail closed) when it runs out.

## 5. The KATs are byte oracles

`tools/acpi-aml-grade/tests/fix2_kats.rs`:

- [ ] **5a** Each program's bytes match its ASL comment under the AML grammar (ACPI 6.5 chapter 20). Spot-check at
  least six, recomputing the PkgLengths, including `operator_nesting_is_bounded` (2-byte PkgLength) and
  `field_read_over_the_object_limit` (4-byte PkgLength). Disassembling with `iasl -d`, after prepending a 36-byte
  DSDT header with a valid checksum, is acceptable evidence.
- [ ] **5b** Opcode values agree with a raw source, such as ACPICA `source/include/amlcode.h`, not a summarised one.
- [ ] **5c** Every KAT that runs the stock crate asserts the stock panic, and the panic is the class the KAT names.
  The patched-only KATs (stack overflow, allocation failure, endless loop on stock) say why in their doc comment.

## 6. Findings accuracy

- [ ] **6a** The fuzz numbers in findings §2 match `tools/acpi-aml-grade/report/fuzz-fix1.json`: seed, case count,
  0 panics, 0 crashes, and the hang count. The hang list in the census matches that file.
- [ ] **6b** The ACPICA comparisons in findings §3 (Package truncation, Buffer growth, the reentrancy limit, Mid
  clipping) match the ACPICA source at the files named.
- [ ] **6c** Note any ACPI 6.5 section citation that does not match the spec text. The writer could not reach the
  spec from the container.

## Close condition

The RETURN file is pushed with every item answered. The PM banks it and replies on the PR.
