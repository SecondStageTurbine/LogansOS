<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-86 Fix 2: panic-free AML interpretation in `acpi-loganfy`

Host only. The kernel is untouched. Fix 2 turns every AML-reachable panic in the vendored interpreter
(`third_party/acpi-loganfy/src/aml`) into an error. Fix 1 (L3, merged as #102 `7377a9b3`) is the base. The
per-site record is `third_party/acpi-loganfy/PANIC-CENSUS.md`.

## Summary: the gates

| Gate | Result | |
|---|---|---|
| 0 unclassified explicit panic sites in `src/aml` | 58 sites left: 47 proven unreachable, 11 host-API only, 0 unclassified (census Part A; `tests/panic_census.rs` enforces it by file, line and text) | PASS |
| every stock site accounted for | 181 stock sites: 124 converted to an error, 57 kept and classified (census Part B) | PASS |
| corpus-seeded fuzz, 0 panics in 200k cases | seed `0x0786`, 200 000 cases, debug build with overflow checks: **0 panics, 0 crashes**, 17 hangs (L7 data) (`report/fuzz-fix1.json`) | PASS |
| Fix 1 gates still hold | `report/coverage.json` and `coverage.md` are byte-identical to `main`'s; `fix1_gates_hold_against_the_rebaseline` passes | PASS |
| per-target ratchet (ported from the second Fix 1) | `report/ratchet.json`, floor set at `main` `7377a9b3`; `fix1_never_regresses` passes | PASS |
| one malformed-AML KAT per converted class | `tests/fix2_kats.rs`: 23 hand-laid programs (§3, §7) | PASS |
| kernel untouched | no file under `kernel/`, `userland/`, `tools/xtask/`, `.github/`; `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` unchanged, so every kernel build input is the same tree as `main`'s (§6) | PASS |

Stock `acpi` 6.1.1, same seed and pool, 200 000 cases: 62 793 cases panic (at 48 distinct sites), 48 crash, 36 hang.

## 1. What Fix 2 changes

**Scope:** `src/aml` only. `src/sdt`, `src/platform` and the rest of the crate parse static tables, not AML, and
are out of scope (census, "Out of scope"). The crate patch is
`git diff origin/main -- third_party/acpi-loganfy/src`; every change carries a `LOGANFY(Fix 2)` comment.

**The errors.** Two kinds, as approved:
- `AmlError::MalformedAml(&'static str)`, the one new variant. The string names the check (for example
  `"PkgLength runs past the end of its enclosing block"`). The grade's classifier files it as `parse-error`.
- `fail_closed(feature)` → `AmlError::LibUnimplemented` with a `LOGANFY fail-closed: <feature>` warn line, for AML
  that is well formed but uses something the crate does not build (Notify, Load, LoadTable, Match, CopyObject,
  IndexField/BankField access, 21 resource-template cases). None of these
  returns a made-up value.

**The classes** (stock site lines in census Part B):

| Class | Stock | Now |
|---|---|---|
| operator operands of an unexpected kind (`let [...] = … else { panic!() }`) | 49 sites | `MalformedAml("operator arguments of an unexpected kind")`. Defensive: the resolve behaviours normally fix each operand's kind, and no input found reaches most of them; `_STA` returning a non-Integer does |
| PkgLength past or short of its block | slicing and `pkg_length - consumed` | `pkg_remaining` / `start_new_block` checks |
| block structure (`block_stack.pop().unwrap()`, Package/VarPackage asserts and counts) | 16 sites | `MalformedAml` naming the check; surplus Package initializers are an error |
| unbuilt operations (`todo!`, `unimplemented!`, the in-flight catch-all) | 36 sites | fail closed (15 in the interpreter, 21 in the resource decoder; 3 of the 36 are the Fix 1 field-list parse-only items) |
| buffer and buffer-field bounds | indexing in `copy_bits`, `read/write_buffer_field` | `check_buffer_field` at creation; bounds checks on access |
| names (root as a name, non-normalised, empty) | 8 asserts/panics | `MalformedAml` |
| region access width / address overflow | 6 `panic!` + arithmetic | `MalformedAml` / checked arithmetic |
| reserved FieldFlags UpdateRule (3) | `unreachable!()` | `InvalidFieldFlags` |
| Divide/Mod by zero, reference cycles, nesting, allocation size | found by the fuzz (§2) | §3 |

The `unreachable!()` sites that are kept now say why (`unreachable!("descriptor_type is 7 bits")`).

## 2. Fuzz

`tools/acpi-aml-grade/src/fuzz.rs` + `src/bin/acpi-aml-fuzz.rs`. Deterministic (xorshift from the seed). The pool is
10 193 sources: every corpus AML table of at most 16 KiB, every Method/Device/ThermalZone/Processor/PowerResource
object of at most 4 KiB cut out of the corpus tables, and the Fix 1 KAT programs. Each case applies 1 to 4 byte
mutations, loads the result as a table, evaluates up to 16 of the methods it defines with Integer(0) arguments, and
decodes any returned Buffer as a resource template. Cases run in child processes (4 jobs): a panic is caught and
reported with its site; a worker death (stack overflow, allocation failure) is a crash; a case past 20 s is a hang.
The workers cap single allocations at 1 GiB. The build is the workspace `dev` profile (overflow checks and debug
assertions on, `panic = unwind` for the run), and no opt-level override applies to these crates.

| Run | Code | Cases | Panics | Crashes | Hangs |
|---|---|---|---|---|---|
| stock, seed `0x0786` | crates.io 6.1.1 | 2 000 | 622 | – | – |
| stock, seed `0x0786` | crates.io 6.1.1 | 200 000 | 62 793 (48 sites) | 48 | 36 |
| `fix1`, before Fix 2 | `7377a9b3` | 2 000 | 622 | – | – |
| after the first conversions | `382627c3` | 20 000 | 0 | – | 7 |
| 200k | `382627c3` | 200 000 | 2 (Divide/Mod by zero) | 24 (reference cycles; in-flight growth) | 44 |
| final, first round | `3433290e` | 200 000 | 0 | 0 | 17 |
| **after review round 1** | **`4b618826`** (crate tree `b47b0f3b`) | **200 000** | **0** | **0** | **17** (the same 17 hashes) |

Both classes from that 200k run were fixed in `d9f3b7c3` (Divide/Mod, cycles) and `2fb0dd5c` (in-flight growth); each of its 26 failing cases was re-run on the fix
and returns an error (`acpi-aml-fuzz case --seed 0x0786 --index <i>`). The hang count fell from 44 to 17 because
cases that used to run until an allocation failed now stop at a limit.

The 17 hangs are listed by input hash in the census ("Non-termination") as L7 data, not diagnosed one by one. Per
the approved scope the step budget stays in L7.

Reproduce: `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- run --seed 0x0786 --cases 200000 --jobs 4 --report tools/acpi-aml-grade/report/fuzz-fix1.json`
(about two minutes on 4 cores).

## 3. Decisions for J.c.: limits and divergences from ACPICA

Fix 2 fails closed in places where ACPICA tolerates the input, and sets limits that are policy. Each is a choice
that could go the other way, so each is listed. The ACPICA citations were read from the acpica/acpica checkout at
`e85aa3be`.

1. **`MAX_OBJECT_BYTES` = 1 MiB.** Caps every allocation whose size AML gives: `Buffer`, `Package`, `VarPackage`, a
   buffer-typed field read, and (since the first review round, §7) the result of an operator that grows its
   operands: `Concat`, `ConcatRes`, `ToHexString`, `ToDecimalString`, `ToBuffer` of a String. The corpus's largest object is a few KiB. Stock allocated whatever the AML asked
   (`Buffer (0xFFFFFFFF)`). KATs `buffer_size_over_the_object_limit`, `field_read_over_the_object_limit`.
2. **`MAX_NESTING` = 256.** Caps block nesting, method-call depth and in-flight operators. ACPICA's nearest limit is
   255 concurrent invocations of one method (`dsmethod.c`, AE_AML_METHOD_LIMIT). It costs nothing on the corpus (the
   grade is unchanged). KATs `method_recursion_is_bounded`, `operator_nesting_is_bounded` (300 legal nested `Add`s:
   stock returns 301, Fix 2 stops at the limit).
3. **A Package with more initializers than NumElements is an error.** ACPICA truncates to NumElements with a warning
   (`dspkginit.c` lines 265–296). KAT `package_with_surplus_initializers`.
4. **A Buffer initializer longer than BufferSize is an error.** ACPICA grows the buffer to the initializer
   (`dsobject.c` lines 219–222). KAT `buffer_initializer_longer_than_its_size`.
5. **Divide/Mod by zero is `MalformedAml("Divide or Mod by zero")`**, not a variant of its own, since one new
   variant was approved. ACPICA returns AE_AML_DIVIDE_BY_ZERO (`utmath.c`). KAT `divide_and_mod_by_zero`.
6. **A store that would make an object reach itself is refused** (`Store (RefOf (Local0), Local0)`, or a Package
   element given a reference to its own Package). Stock stores it, then overflows the stack the first time anything
   walks it. The check walks at most `MAX_NESTING` levels and 65 536 objects and refuses when it runs out. I did not
   establish what ACPICA does with these stores. KATs `store_that_makes_a_local_refer_to_itself`,
   `store_that_makes_a_package_contain_itself`.
7. **`Mid` past the end returns what exists**, as ACPICA does (`exoparg3.c`, `AcpiExOpcode_3A_1T_1R`). This is the
   one conversion that returns a value instead of an error. KAT `mid_past_the_end_returns_the_rest`.
8. **A plain `AccessAs` is parsed and not applied**, so the fields after it fail closed, as Fix 1 did for the extended
   form. Applying it is L4.
9. **Host-API asserts stay.** `install_region_handler` twice, and `is_absolute` on names passed to the namespace,
   can only fail when the host (the kernel, at L5) calls the API wrongly; the census lists the 11 sites.

## 4. STOP-AND-REPORT: a value defect found, not fixed

**A buffer-typed field read returns 8× too many bytes.** `do_field_read` sizes its output as
`vec![0; field.bit_length.next_multiple_of(8)]`: that is a bit count rounded to 8, used as a byte count. A 72-bit
field reads as a 72-byte Buffer whose first 9 bytes hold the value; it should be a 9-byte Buffer (ceil(72/8)).
Reproduced on both crates through `run_kat` with
`OperationRegion (REG0, SystemMemory, Zero, 0x10)  Field (REG0, ByteAcc, NoLock, Preserve) { FLD0, 72 }
Method (MAIN) { Return (FLD0) }`, bytes
`5b 80 52 45 47 30 00 00 0a 10  5b 81 0c 52 45 47 30 01 46 4c 44 30 48 04  14 0b 4d 41 49 4e 00 a4 46 4c 44 30`:
both return a 72-byte Buffer. This is
stock behaviour, outside the panic list, and it changes values, so Fix 2 leaves it alone (a comment marks it) and
only caps the size. Proposed for L4, with a KAT. No gate moves, because stock and `acpi+fix1` agree and the grade
has no oracle for a field read's length.

**`VENDORED.md` was stale.** It said the first commit adding the crate had the unpatched source; #102 was
squash-merged, so no commit on `main` does. It now says to diff against the crates.io registry copy.

## 5. What the census does not prove

- Implicit panics (slice indexing, arithmetic, allocation) are covered by class, with a check and a KAT per class,
  not line by line. The fuzz is the evidence that none is left on the paths it reaches; review item 2e asks for an
  independent look.
- The `unreachable` arguments are by reading. Rust does not check them. The census test only checks that every
  site is listed.
- The fuzz mutates corpus AML. Structure it never builds (deep Package/Reference graphs, a host `NativeMethod`)
  is covered only by the KATs and by reading.

## 6. Reproduce

```
CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade        # report/coverage.* (unchanged from main)
cargo test -p acpi-aml-grade                                       # gates, ratchet, Fix 1 + Fix 2 KATs, census
git diff origin/main -- third_party/acpi-loganfy/src               # the Fix 2 patch
diff -ru ~/.cargo/registry/src/*/acpi-6.1.1/src third_party/acpi-loganfy/src   # Fix 1 + Fix 2 vs upstream
```

Kernel identity: `git rev-parse origin/main:<p>` equals `HEAD:<p>` for `kernel`, `userland`, `tools/xtask`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` and `.github` (hashes in the PR's "PM: ready" comment). The
kernel is built from exactly the same inputs, so no ELF rebuild is needed (CLAUDE.md, the tree-identity rule).

Commits after the rebase onto `e3008cda` (old → new): `fea7e2d0` → `382627c3`, `3c7b67af` → `d9f3b7c3`,
`94895e4f` → `2fb0dd5c`, `b9270fdf` → `3433290e`, `e0d5f39f` → `01f9cd34`.

## 7. Review round 1 (Jarvis, GPT-6-Luna high, `5c9f9bfe`)

Every item passed except two, both fixed in `4b618826` on top of `63041450`, with no rebase:

- **2e/4a: `Concat` could build an object past `MAX_OBJECT_BYTES`.** The reviewer's input
  (`14 15 4d 41 49 4e 00 70 11 06 0c 00 00 10 00 60 a4 87 73 60 60 00`, the `Concat` of two 1 MiB Buffers) returned
  a 2 MiB object. I audited every operator that builds a result bigger than its operands, and found the same gap in:
  - `ConcatRes` (the sum plus 2 bytes);
  - `ToHexString` (about 5× a Buffer);
  - `ToDecimalString` (up to 4×);
  - `ToBuffer` of a String (plus 1 byte);
  - `Concat`'s String path, where `from_utf8_lossy` can triple a Buffer.

  `check_object_size` now bounds each result. Every operand is already within the limit, so the transient
  allocation before the check is a small multiple of it. `Mid` and `ToString` are bounded by their source, and a
  Store copies. KATs: `concat_result_over_the_object_limit` (the reviewer's bytes; stock returns 2 097 152) and
  `to_hex_string_result_over_the_object_limit` (256 KiB in, 1 310 719 bytes on stock).
- **3d: the dispatch item was wrong, not the code.** The `Index` operator at or past the end returns
  `IndexOutOfBounds`, and that is correct. ACPICA returns AE_AML_BUFFER_LIMIT, AE_AML_STRING_LIMIT or
  AE_AML_PACKAGE_LIMIT (`exoparg2.c` lines 382–411, `AcpiExOpcode_2A_1T_1R`). The ACPI 6.5 text was not reachable
  from the container. The "empty object" behaviour belongs to `Mid` with an `Index` past the end. The dispatch now
  says so. KAT `index_past_the_end_is_an_error` (the reviewer's bytes) pins the error on both crates.

After the fix, the gates were re-run:
- the corpus report is byte-identical to `main`;
- the ratchet holds;
- the census is regenerated (line numbers only; Part B is unchanged);
- the 200k fuzz has 0 panics and 0 crashes;
- the close-gate passes.
