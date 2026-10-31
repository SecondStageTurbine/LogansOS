<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-90 shutdown THINK second re-check (relay #119)

**MODEL:** GPT-6-Luna. **EFFORT:** medium. **Seat:** Janus.
**Reviewed:** `docs/THINK-2026-10-27-ot90-shutdown-restart.md` and `docs/THINK-2026-10-27-ot90-s5-walk.py` at `8ecbde6b` (PR #153).
**Overall: FAIL.** The listed namespace cases are addressed, but the prototype does not parse every table as claimed and its wrong-arity rationale is stronger than its behavior.

## Checklist

1. **Reject non-root/non-static `_S5`; handle method arity — FAIL (fail-closed claim not established).** The explicit rules reject nested scopes, conditional bodies and data payloads, and require a root `Name` with a package (THINK:152-180; walker `walk.py:120-127`, `:140-160`). The asked forward-call case shows the stated misalignment behavior is not necessarily fail-closed. Concrete AML terms: `Name (VAL0, CALL (Zero))`, followed by `Method (CALL, 1) { Return (Zero) }`, followed by a genuine root `Name (_S5, Package (2) { ByteConst (5), ByteConst (0) })`. The prototype sees `CALL` before its declaration, assumes zero arguments (`walk.py:67-72`, `:98-103`), then consumes the leftover `Zero` as a table-level term and continues. I ran this through `decode`; it returned `('OK', 'dsdt', '0x37', [5, 0])`. This input still contains a genuine root `_S5`, so it does not demonstrate a wrong SLP_TYP; it does demonstrate that the assertion “a misaligned walk … fails closed” (THINK:184-190) is not guaranteed. The requested check for a call whose arity is unavailable needs a KAT, or the design should state and enforce a narrower condition. I did not find a concrete non-root lookalike accepted among the listed cases.

2. **Accept legitimate cases — PASS, with evidence limitation.** The parser resolves absolute `Name (\\_S5)` to the root inside a named scope (`walk.py:107-113`, `:124-127`), and `decode` proceeds to a later SSDT if no root definition was found in the DSDT (`walk.py:144-160`). The rerun returned ten `OK` values: 7/0, 5/0 and 0/0. However, because it returns as soon as a table supplies `_S5`, those runs do not establish that the SSDTs in the captured sets parse successfully (see item 3).

3. **Rerun prototype — FAIL against the 93/93 claim.** I copied the pinned script to a scratch directory and ran `python -I <scratch>\\s5walk.py <repo>\\fixtures\\acpi`. It printed ten `OK` result lines, one per set, and no `93/93 walked` summary. The fixture tree has 93 DSDT/SSDT files. In `decode`, finding `_S5` returns immediately from inside the table loop (`walk.py:148-158`), so a DSDT-root result skips all following SSDTs. A direct probe with a valid DSDT root `_S5` followed by an SSDT containing an unknown opcode returned `OK` without parsing that SSDT. Thus the script's run does not support THINK:194's claim that all 93 tables walk to end. The output also leaves each set label blank (`walk.py:165`) because `os.path.basename` receives Windows paths with a trailing backslash; this makes the ten results harder to map back to fixtures.

4. **Planned KATs include Buffer and SSDT-root cases — PASS.** The revised KAT list includes the Buffer, String, Package, and SSDT-root cases (THINK:438-445). The KAT list does not currently include the forward-method-arity case above.

## Review method

Read relay #119, RETURN #118, and the docs diff. RAG health was healthy (`ollama=true`, `qdrant=true`). Reran the pinned prototype in a scratch directory and used small in-memory AML probes against its `decode` function. No build or tests were run.
