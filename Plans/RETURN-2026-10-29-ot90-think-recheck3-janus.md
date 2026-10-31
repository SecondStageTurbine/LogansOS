<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-90 shutdown THINK third re-check (relay #120)

**MODEL:** GPT-6-Luna. **EFFORT:** medium. **Seat:** Janus.
**Reviewed:** `docs/THINK-2026-10-27-ot90-shutdown-restart.md` and `docs/THINK-2026-10-27-ot90-s5-walk.py` at `a6d9def1` (PR #153).
**Overall: FAIL.** The rerun and method-arity fixes pass; a non-`_S5` `Name` inside a conditional body leaks into the unconditional declaration set.

## Checklist

1. **Rerun — PASS.** I copied the pinned script to a scratch directory and ran it with `python -I` against `fixtures/acpi/`. It printed labeled per-table results and the totals line: `tables walked to end: 93/93; sets decoded: 10/10` (THINK:224, 720; script:190-224, 230-239).

2. **#119 probes and new arity probes — PASS.** The valid DSDT `_S5` followed by an unknown-opcode SSDT now fails closed on the SSDT (script:190-210). `Name (VAL0, CALL (Zero))` before `Method (CALL, 1)` decodes 5/0; `CALL`’s forward arity is collected. The forward `Method (CALL, 2)` with only one argument fails closed (`declarations do not settle`); `Store (UNKN (Zero), VAL0)` fails on unresolved `UNKN`; and a root `_S5` inside `If (One)` fails as conditional (THINK:180-195; script:85-94, 155-174, 201-224). These match the stated outcomes.

3. **Conditional-body walking and declaration leakage — FAIL.** The walk propagates the caller’s root scope into an `If` body and marks only `_S5` specially; every other `Name` is added to the shared declaration set with that root path (script:155-159, 168-174; `declare` at :63-67). A concrete probe was `If (Zero) { Name (VAL0, One) }; Store (VAL0, Zero); Name (_S5, Package (2) { ByteConst (5), ByteConst (0) })`. The prototype returns `OK` with 5/0. Removing the conditional `Name (VAL0, One)` makes the same table fail closed on unresolved `VAL0`. Thus a declaration in an unexecuted conditional body satisfies a later strict lookup as though it were unconditionally present. The `_S5` itself in this probe is a genuine root definition; the finding is that the namespace walk accepts the leaked declaration. The THINK says conditional bodies are walked because they contain `External` declarations (THINK:165-167), but it does not scope ordinary `Name` declarations to the condition. Track conditional declarations separately or reject their use outside the conditional, while retaining the needed `External` collection.

4. **Wrong declared arity limit — PASS (bounded in the checked cases).** I tried a method declared with arity two but called with one argument, followed by the only `_S5_` occurrence inside `Scope (\\_SB)`. It failed closed with `declarations do not settle`; it did not accept the nested `_S5`. The revised text explicitly says a wrong declared arity is not guaranteed to fail closed and limits acceptance to an unconditional root `Name` at the unique `_S5_` occurrence (THINK:214-219; script:201-224). I did not find a concrete wrong-arity input that bypasses that backstop.

## Review method

Read relay #120, RETURN #119, and the docs diff. RAG health was healthy (`ollama=true`, `qdrant=true`). Reran the prototype in a scratch directory and exercised the requested hand-laid AML cases against its `decode` function. No build or other tests were run.
