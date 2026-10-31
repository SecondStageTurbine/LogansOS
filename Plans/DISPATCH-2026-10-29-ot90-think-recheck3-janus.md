<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #120): third re-check of the OT-90 THINK, your #119 items only (#153 @ a6d9def1) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** medium. This is a narrow re-check of your own two #119 findings, plus a rerun of
the prototype.

**Deliverable:** one pushed RETURN, `Plans/RETURN-2026-10-29-ot90-think-recheck3-janus.md`.
**Reviewed SHA:** `a6d9def1` (PR #153). Your #119 reviewed `8ecbde6b`. Diff: `git diff 8ecbde6b a6d9def1 -- docs/`.

**What changed (S5's summary; check it, don't take it):**
1. The prototype walks every table of every set to its end, then decodes the set. It prints one OK/FAIL line per
   table, with the set name, then the totals. The THINK's Appendix A quotes "tables walked to end: 93/93; sets
   decoded: 10/10". Your probe (a valid DSDT root `_S5`, then an SSDT with an unknown opcode) now fails closed.
2. There is now a fixed-point pre-pass over all tables. It collects every Method/External arity and every declared
   name, for at most 8 passes, else it fails closed. Then a strict pass resolves each table-level name by §5.3
   search rules. A name that resolves to nothing fails the whole decode closed. So do conflicting arities, and so does
   a strict pass whose set of declarations differs.
   - If/Else/While bodies are now walked, not skipped, because the Zenbook PEP SSDT's `External (\ADBG, MethodObj, 1)`
     sits inside `If (Zero)`. An `_S5` inside one fails closed.
   - An unresolvable `External` declares nothing, as ACPICA's loader does.
   - The stated remaining limit: a table whose *declared* arity is wrong isn't guaranteed to fail closed, so the
     `_S5_` single-occurrence guard stays as the backstop.

## Checklist (pass/fail, plus `file:line`, and a concrete input for any fail)

1. **Rerun:** `git show a6d9def1:docs/THINK-2026-10-27-ot90-s5-walk.py`, run with `python -I` from a scratch folder
   against `fixtures/acpi/`. Do you get 93/93 walked and 10/10 decoded, with labels? Report the totals line.
2. **Your two probes:** the DSDT `_S5` plus a bad-opcode SSDT, and `Name (VAL0, CALL (Zero))` before
   `Method (CALL, 1)`. Plus S5's: a forward `Method (CALL, 2)` called with one argument, and `Store (UNKN (Zero), VAL0)`
   with UNKN undeclared. Do they behave as the THINK says?
3. **Walking If/Else/While bodies.** Does it open a new false-accept? For example, an `_S5` defined inside `If (One)`
   at the root must still fail closed. And does a `Name` inside such a body leak into the root?
4. **The stated limit (a wrong declared arity).** Is it honest and bounded? Can you build a table where a wrong
   declared arity plus a *single* `_S5_` occurrence that isn't the root `Name` gets accepted? If you can, that's a
   FAIL with the input.

```
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot90-think-recheck3-janus
          git fetch origin && git checkout -B codex/ot90-think-recheck3-janus a6d9def1

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot90-think-recheck3-janus.md
git show origin/main:Plans/RETURN-2026-10-29-ot90-think-recheck2-janus.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in your RETURN; it's
never a blocker. Docs only, no gate. Push the RETURN. Before ending, run `git status --porcelain` and
`git log @{u}..`; both must be empty.
