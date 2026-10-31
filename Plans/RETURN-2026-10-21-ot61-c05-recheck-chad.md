# OT-61 C0.5 browser re-check (CHAD)

**Date:** 2026-10-21  
**Reviewed head:** `c4765bebe7c870a234bccc164cb3e915c32f02b3` (`origin/claude/ot61-c05-std-overlay`)  
**Base diff:** `459d36e3..c4765beb` (`userland`, `tools`)  
**Source record:** `docs/RESEARCH-2026-10-21-ot61-c05-FINDINGS.md`, §§4.2 and 7.

## Verdicts

1. **PASS — every `syscall3` reference requires a `SYS_*` first argument.** `a4_gate.rs` G5 iterates every code-token occurrence of `syscall3`, allows only the trampoline declaration, and rejects references without a call or with a first argument that is not a whole `SYS_*` identifier (`userland/neo-std-pal/a4_gate.rs:221-245`). G2 then requires that argument name to be declared as a checked constant in that same file (`:252-280`). Qualified forms are not exempt. Chad’s method-qualified fixture is committed verbatim and asserts refusal under G3 and G5 (`tools/xtask/src/ot61_c05_tests.rs:217-224`); seven qualified call forms and pointer/alias/turbofish/wrapper cases are also fixtures (`:227-258`).

2. **PASS — exactly one trampoline per trampoline file; `asm!` only inside it.** G3 accepts the brace-matched `fn syscall3` only in the two listed trampoline files, reports a second declaration, and requires each such file to contain one (`userland/neo-std-pal/a4_gate.rs:137-163`). It rejects all asm macro forms outside the matched body and limits the sole in-body `asm!` template to exactly `"syscall"` (`:165-195`). The committed tests exercise a missing trampoline, second trampoline, renamed asm wrapper, and extra/multi-instruction asm (`tools/xtask/src/ot61_c05_tests.rs:267-284, 336-341`).

3. **PASS — comments/strings cannot fake code; side doors are refused.** The same-length lexer blanks line/block comments and literal contents while preserving code offsets (`userland/neo-std-pal/a4_gate.rs:361-482`); the committed string/comment fixtures check fake calls and fake declarations (`tools/xtask/src/ot61_c05_tests.rs:285-305`). G6 rejects `include!`, both inner/outer `#[path]`, `#[link]`, and extern blocks (`userland/neo-std-pal/a4_gate.rs:292-326`); committed fixtures cover each (`tools/xtask/src/ot61_c05_tests.rs:322-334`).

4. **PASS — both xtask backstops fail on the recorded sabotaged builds.** The installed-rust-src backstop byte-compares all four installed modules with scanned sources, diffs patched files against `.neo-pristine`, and runs the added lines through A4 (`tools/xtask/src/main.rs:5058-5134`). §4.2 records its sabotage outcome: an obfuscated `syscall3(41, ...)` inserted into the exit arm failed G5. The ELF backstop disassembles the produced ELF, rejects syscall-class instructions outside the two sanctioned symbols, and rejects multiple sites per symbol (`tools/xtask/src/main.rs:5138-5191`). §4.2 records its sabotage outcome: making the overlay trampoline `#[inline(always)]` produced a syscall in `map_region` and failed. This is supported by the review record’s reported real and negative runs; this re-check did not rerun those builds.

## Scope note

This is a source and recorded-evidence re-check of the specified head. The findings document records 30/30 A4 host fixtures and the two temporary backstop sabotage refusals (§4.2). No new build or tests were run for this return.
