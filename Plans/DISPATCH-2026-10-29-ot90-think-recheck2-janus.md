<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #119): second re-check of the OT-90 THINK, item 2 only (#153 @ 8ecbde6b) → Janus

**MODEL:** GPT-6-Luna. **EFFORT:** medium. This re-checks one of your own findings (#118 item 2) on a design doc,
plus one prototype script to rerun.

**Type:** narrow design re-check. **Deliverable:** one pushed RETURN,
`Plans/RETURN-2026-10-29-ot90-think-recheck2-janus.md`. **Reviewed SHA:** `8ecbde6b` (PR #153). Your RETURN #118
reviewed `15379e82`. Diff: `git diff 15379e82 8ecbde6b -- docs/`.

**What changed.** S5 dropped the byte scan for a parse-only namespace walk (THINK §3.2), because treating data
containers as covering failed closed on 6 of the 10 real sets. The walk:
- **Enters:** Scope, Device, Processor, PowerResource and ThermalZone.
- **Skips by PkgLength:** Method (recording its arity), If/Else/While, Buffer/Package/VarPackage and the Field ops.
  Strings are skipped to their NUL.
- **Parses** every other table-level term through an opcode-shape table. Method arity comes from Method/External;
  an unknown name takes 0 args. Anything unknown fails closed.
- **Accepts** exactly one root `Name (\_S5)` with a valid package, and the `_S5_` bytes must occur exactly once.

The prototype is `docs/THINK-2026-10-27-ot90-s5-walk.py` on the PR branch.

## Checklist (pass/fail, plus `file:line` and a concrete input for any fail)

1. **The walk can't accept a non-root or non-static `_S5`.** Check these cases:
   - your #93 counterexample and the #118 Buffer payload;
   - the same bytes inside a String and inside a Package;
   - `\_SB._S5`;
   - `_S5` inside an If or a Device;
   - `_S5` after a Method whose arity the walk gets wrong. Can a call to an External with an undeclared arity throw the
     walk out of step, so it reads data as terms? If it can, does it fail closed, or can it land on a lookalike?
2. **It still accepts the legitimate cases:**
   - an absolute `Name (\_S5)` inside `Scope (\_SB)`;
   - an SSDT-root `_S5`;
   - the ten captured sets.
3. **Rerun the prototype.**
   ```
   git show 8ecbde6b:docs/THINK-2026-10-27-ot90-s5-walk.py > %TEMP%\s5walk.py
   python -I %TEMP%\s5walk.py
   ```
   Run it from a scratch folder, against the repo's `fixtures/acpi/` (pass the path the script asks for). Report what
   it prints: 93/93 walked and 10/10 decoded, as claimed, or what differs.
4. **The planned KATs include** the Buffer-payload case and an SSDT-root case.

```
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot90-think-recheck2-janus
          git fetch origin && git checkout -B codex/ot90-think-recheck2-janus 8ecbde6b

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot90-think-recheck2-janus.md
git show origin/main:Plans/RETURN-2026-10-28-ot90-think-recheck-janus.md
```

RAG (semantic code search) is available for this project. Health check first:
`Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that in your RETURN and continue with direct file
reads; it's never a blocker.

Commit and push the RETURN to your branch. Before ending, run `git status --porcelain` and `git log @{u}..`; both must
be empty.
