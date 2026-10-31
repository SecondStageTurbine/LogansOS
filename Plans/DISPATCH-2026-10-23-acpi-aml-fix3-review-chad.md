<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #48): cross-family review of OT-86 Fix 3 (PR #107) → Chad

**MODEL:** GPT-6-Luna. **EFFORT:** high. The change is contained: one vendored interpreter crate
(`third_party/acpi-loganfy/src/aml`), which only a host tool uses, plus that tool, its tests and docs. It touches no
kernel code, no SMP core and no syscall.

**Type:** review only. **Deliverable:** one pushed RETURN. **Reviewed SHA:** `4389b062` (pinned; draft PR #107,
branch `claude/acpi-aml-fix1`, base `main` after `d604e839`). The writer is S5 (Claude), so the reviewer must come
from a different model family. **J.c. reserves the merge.**

**What Fix 3 does:** it gets the Zenbook S14 touchpad's `_CRS` and HID `_DSM` evaluating through AML alone, and
checks the result against the kernel's `ZENBOOK_S14_QUIRK`. It also fixes one stock-crate defect found on the way:
`LNot` was parsed with two operands. The items are in `Plans/PROPOSAL-2026-10-23-acpi-aml-fix3-scope.md`
(items 1–3, 5–10; 4, 11 and 12 stay out).

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/acpi-aml-fix3-review-chad
          git fetch origin && git checkout -B codex/acpi-aml-fix3-review-chad 4389b062

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-23-acpi-aml-fix3-review-chad.md      # this file
git diff d604e839 4389b062 --stat                                                # whole PR vs the main it merged
git log --oneline origin/main..4389b062
git show 4389b062:Plans/PROPOSAL-2026-10-23-acpi-aml-fix3-scope.md
git show 4389b062:docs/RESEARCH-2026-10-23-acpi-aml-fix3-FINDINGS.md
git show 4389b062:Plans/VALUES-2026-10-23-acpi-aml-fix3.md
git diff d604e839 4389b062 -- third_party/acpi-loganfy/src                       # the crate patch for Fix 3
```

Per-item commits:
- `bf17f3a9` (1), `ba29ba8e` (2), `6158fafb` (3), `218d6510` (5/6), `eab0ebf6` (7), `1aa42a36` (9), `f8151a06` (10);
- **`b48a1a8d` LNot**, on its own;
- `0fc11f4e` (8, switch `STORE_GROWS_EMPTY_BUFFER`);
- `4389b062`: one more item-8 KAT (`string_stored_to_a_buffer_carries_its_nul`), test and findings only.

**Run** (from the repo root, inside your worktree):
- `cargo test -p acpi-aml-grade`;
- `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade` (PowerShell:
  `$env:CARGO_PROFILE_DEV_PANIC='unwind'; cargo run -p acpi-aml-grade`). Afterwards `git status` must be clean,
  because the committed report should already be current;
- a short fuzz: `CARGO_PROFILE_DEV_PANIC=unwind cargo run -p acpi-aml-grade --bin acpi-aml-fuzz -- run --seed 0x0786 --cases 20000 --jobs 4`.
  It must report 0 panics and 0 crashes.

RAG (semantic code search) is available. Use it for "where is X done?" questions; grep is better for exact symbols.
Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it is down, note that in the RETURN and use
grep/Read; the PL manages the RAG. Example query: `$body=@{query='AML IndexField BankField access sequence'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For a FAIL, give a concrete input (AML bytes, a fuzz seed and case,
or a command). For a PASS, say what you checked. An uncertain point is a note, not a FAIL.

**Scope**
1. No file under `kernel/`, `userland/`, `tools/xtask/` or `.github/` changed. Within the crate, only
   `src/aml/` changed. `Cargo.lock` is unchanged or changed only in `acpi-aml-grade`'s own entry. The stock
   (crates.io `acpi =6.1.1`) column of `report/coverage.json` is unchanged from `origin/main`.

**The goal**
2. `tools/acpi-aml-grade/tests/zenbook_touchpad.rs`:
   - evaluates ETPD `_CRS`, `_DSM` fn 0 and fn 1, and I2C5 `_ADR` through the patched interpreter on the Zenbook
     tables;
   - decodes `_CRS` with the crate's own decoder;
   - compares every field with `ZENBOOK_S14_QUIRK`, mounted read-only from
     `kernel/src/drivers/i2c_hid/quirk_logic.rs`.
   Check that no expected value is a constant copied into the test instead of read from the quirk.

**Each item**
3. **Item 1, String/Buffer compares.** They follow ACPICA `AcpiExDoLogicalOp`:
   - lexicographic, with the shorter-is-less tie-break;
   - the right operand converted to the left's type;
   - a String converted to Buffer carries its NUL.
   Integer compares are unchanged.
4. **Item 2, I2C serial-bus decode.** The 0x8E I2C decode matches ACPI 6.5 §6.4.3.8.2 field offsets and flags,
   including vendor data and the resource-source string. SPI, UART and CSI-2 still fail closed. A truncated
   descriptor returns an error, never a panic or an out-of-bounds read.
5. **Item 3, GpioInt/GpioIo decode.** The 0x8C decode matches §6.4.3.8.1 (pin table offset, resource-source
   offset, vendor data offset/length), and every offset from the bytes is bounds-checked.
6. **Items 5/6, IndexField and BankField.** For IndexField, check:
   - the index register is written before each data access;
   - the index value is the field's byte offset, aligned to the access width;
   - accesses wider than the data register split correctly.
   For BankField, the bank value is written before the region access. Compare the access-sequence KATs with
   ACPICA `exfldio.c`.
7. **Item 7, Acquire.** `Acquire` returns Zero when acquired and Ones on timeout, and the result reaches its target.
8. **Item 9, field read size.** A buffer-typed field read is `ceil(bits/8)` bytes, not `bits` rounded to 8.
   Confirm the 72-bit KAT.
9. **Item 10, AccessAs.** A plain `AccessAs` sets the access width for the fields after it, and only those.
10. **Item 8, Store to Buffer/String (ACPICA behaviour).** Check that:
    - a non-empty Buffer target keeps its size, zero-filled or truncated;
    - a zero-length Buffer target takes the source's length, and only that branch follows
      `STORE_GROWS_EMPTY_BUFFER`;
    - a String target takes the source's length;
    - a String source stored to a Buffer carries its NUL;
    - a store of any other type to a String fails closed.
    Compare with ACPICA `exstorob.c` `AcpiExStoreBufferToBuffer` / `AcpiExStoreStringToString` and `exconvrt.c`.
11. **LNot (`b48a1a8d`).** `LNot` starts with one TermArg. `LNotEqual`, `LLessEqual` and `LGreaterEqual` (the
    0x92-prefixed pairs) still take two. Check the KATs:
    - the `If (LNot (One))` case returns 2;
    - the Store case leaves `Local0 = 0` and `Local1 = 7`;
    - `LNot (Zero)` is Ones at the table's integer width (rev 1: 0xFFFFFFFF; rev 2: 64 bits).
    Compare with ACPI 6.5 §20.2.5.4 and ACPICA `acopcode.h` `ARGP_LNOT_OP`, read from raw source.

**Evidence**
12. Every KAT's bytes match its ASL comment under the AML grammar. Spot-check at least six, recomputing
    PkgLengths, including the I2C and GPIO descriptor templates. Opcode values come from a raw source (ACPICA
    `amlcode.h`).
13. `Plans/VALUES-2026-10-23-acpi-aml-fix3.md` agrees with the committed `report/coverage.json`:
    - 97 rows, each from FAIL or not-evaluated to a result;
    - 0 completed evaluations changed value;
    - 0 newly failing;
    - the LNot commit alone changes exactly one evaluation (Zenbook `EC0._INI`).
14. `PANIC-CENSUS.md` lists every new panic-capable site in the Fix 3 diff (0 unclassified), and its test passes.
    Note any slice index or arithmetic on AML-derived values in the new code that is neither checked nor guarded.
15. The fuzz numbers in the findings match `report/fuzz-fix1.json` (seed 0x0786, 200k, 0 panics, 0 crashes, 17
    hangs), and the hang list is unchanged from Fix 2.
16. The findings' known gaps are stated honestly:
    - comparison integer width in rev-1 tables;
    - no corpus probe reaching items 3, 9 and 10.
    Note any other gap you find.

A FAIL is reported only after you have re-checked it yourself.

## Close condition

- Commit `Plans/RETURN-2026-10-23-acpi-aml-fix3-review-chad.md` on your branch and **push it**, with the verdict
  (PASS, or FAIL with the item numbers). Paste the pushed SHA to the PL.
- **Never write the Zenbook's disk.**
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
