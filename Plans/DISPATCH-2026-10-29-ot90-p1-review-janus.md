<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# REVIEW (relay #140) → Janus: OT-90 P1, the Rust port of the _S5 walk you re-checked

**MODEL:** GPT-6-Luna. **EFFORT:** high. Contained code: one host-only module, not yet called by the kernel.

#188 merged after batch 14b (run `40817887458`, green at `67f023bc`) as `5c03fe01`. Review it on `main`:
`git show 5c03fe01 --stat` and `git show 5c03fe01`. You reviewed the THINK's walk three times (#118–#120). The design is
`docs/THINK-2026-10-27-ot90-shutdown-restart.md`, now revision 6. One pushed RETURN, a plain checklist (pass or fail,
the enforcing `file:line`, a concrete input for any fail): `Plans/RETURN-2026-10-29-ot90-p1-review-janus.md`.

1. **The port.** Does `kernel/src/acpi/power_logic.rs`'s `\_S5` walk do what the revision-5 prototype
   (`docs/THINK-2026-10-27-ot90-s5-walk.py`) does? Check the fixed-point arity pre-pass, the strict pass, and the
   conditional-scope rules, including your #120 case (a `Name` inside `If (Zero)`).
2. **The corpus oracle.** `tools/acpi-power-logic-tests/fixtures/prototype-walk.txt` is the prototype's own output on
   the raw fixtures. Does the KAT check every table's result, the SLP_TYPs, and the `\_S5` offset against it?
3. **The negatives** (`fixtures/*.asl` and `*.aml`, and the hand-laid probes for #119 and #120). Is each refused for
   the right reason?
4. **The four deliberate differences** (THINK §3.1, steps 6 and 7, and the hardware-reduced row):
   - the 10 s retry repeats the full second PM1 write;
   - hardware-reduced machines need `SLEEP_STATUS` as well as `SLEEP_CONTROL`;
   - a GAS width of 0 takes the FADT length byte;
   - PM1b's base value is its own read.

   Is each one right against the ACPI spec?
5. **The FADT parse.** Is the hand-laid byte example correct field by field, and is the X_ preference rule right?
6. **The P2 rule** (THINK §3.2): the decode runs once at ACPI init, and the S5 path uses only the cached plan. Does
   anything in P1's API make that hard, or let the power-off path allocate?
7. **No kernel change.** Nothing in `kernel/src` declares the module, so kernel `.text` is unchanged.

```
MODEL: GPT-6-Luna. EFFORT: high. (Contained code: a host-only module.)
WORKTREE: E:\AI Automation\workers\janus   (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/ot90-p1-review-janus
          git fetch origin && git checkout -B codex/ot90-p1-review-janus origin/main

git fetch origin
git show origin/main:Plans/DISPATCH-2026-10-29-ot90-p1-review-janus.md
git show origin/main:docs/THINK-2026-10-27-ot90-shutdown-restart.md
```

RAG health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down, note that and read the files
directly. Push the RETURN. Before ending, run `git status --porcelain` and `git log @{u}..`; both must be empty.
