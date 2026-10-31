# PL-RATIFICATION: Lean Patch Rule

- Recorded by: PM
- Ratification received: 2026-05-26
- Ratifying authority: Project Lead
- Ratification form: "Lets make that a real rule."
- Status: RATIFIED and applied locally
- Normative target: `docs/bible.md` Article 15.7, Bible v1.7.1

---

## 1. Disposition

The Project Lead ratified the lean-patch principle as a project rule.

The rule is now encoded in Bible Article 15.7:

> Every byte must buy correctness, observability, or a required capability.

Code paths, abstractions, dependencies, configuration switches, and test scaffolding that do not earn one of those roles are bloat and must be removed before merge.

## 2. Review Meaning

This ratification makes the following review posture binding:

- Patches must justify added machinery against correctness, observability, or a required capability.
- Speculative future-proofing is not enough.
- Convenience abstractions are rejected when the narrow direct fix preserves the same invariant and verification strength.
- Test scaffolding is welcome only when it buys real evidence or catches a real regression class.
- Dependencies remain liabilities and must clear the same bar as code.

## 3. Boundary

The "tight-machine" language is a review heuristic, not a new hardware target. NeoPlutarch remains governed by the architecture and hardware scope already ratified in Bible Articles 9 and 28.

The rule also does not authorize deleting correctness, observability, required hardware handling, or safety checks in the name of minimalism. Lean means earned surface area, not underbuilt surface area.

## 4. Applied Change

PM applied the rule directly to `docs/bible.md`:

- Added Article 15.7, "Lean patch rule."
- Bumped the Bible footer from v1.7.0 to v1.7.1.
- Prepended the 2026-05-26 amendment note to the footer history.

No kernel code changed.
