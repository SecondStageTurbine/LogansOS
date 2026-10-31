<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-90 P1, the host-only power logic → S5

**Why.** OT-90 (clean shutdown and restart) is designed and decided:
- the THINK is `docs/THINK-2026-10-27-ot90-shutdown-restart.md`, revision 5, re-checked by Janus;
- J.c. decided §12 D1 and D2 on 2026-10-29.

P1 is the first rung (§11): pure logic, fully oracled, and nothing wired into boot. Its prerequisite, #180 (the
NeoFS barrier), merged as `c805f2b2`.

## Scope (§11, row P1)

`kernel/src/acpi/power_logic.rs`, a pure module with no MMIO, no port I/O and no allocation in its decision paths.
It contains:
1. **The FADT power-field parse:** PM1a/PM1b control blocks (legacy and X_ forms, with the X_ preference rule in §3.1),
   `RESET_REG` and `RESET_VALUE`, the reset-register-supported flag, `SLP_TYP` and `SLP_EN` positions.
2. **The static `\_S5` decode (§3.2):** a Rust port of the revision-5 walk in `docs/THINK-2026-10-27-ot90-s5-walk.py`.
   It includes the fixed-point arity pre-pass, the strict pass, and the conditional-scope rules the re-checks
   settled.
3. **PM1 value composition:** the PM1a/PM1b control writes for S5 from the decoded `SLP_TYPa/b`.
4. **The reset-chain plan (§4):** the ordered list of reset methods to try, and when each applies. It's a plan only;
   P2 executes it.

## The oracles

- **The 10-set corpus oracle.** For every set in `fixtures/acpi/`, the Rust decode equals the Python prototype's
  output, as committed in Appendix A. Generate the expected values by running the prototype on the raw fixture files
  (`python3 -I docs/THINK-2026-10-27-ot90-s5-walk.py fixtures/acpi`) and commit them as a fixture, never typed in by
  hand.
- **The 93-table walk:** every table in the corpus passes the strict pass, or is refused exactly where the prototype
  refuses it.
- **The §3.2 negatives**, as committed byte fixtures. These cover the false-accept cases from Janus's re-checks #118
  to #120, including a `Name` inside `If (Zero)`.
- **A hand-laid FADT byte example** (the byte-oracle rule): one small FADT laid out by hand in the test, with the
  parse asserted field by field. Take the offsets from the ACPI spec or the raw fixture bytes, never from a
  summarizing fetch.

## Kill-switch and size

P1 is not called from the kernel. The module is either `#[cfg(feature = "power-control")]` (default off, the
THINK's feature), or mounted by `#[path]` from a host test crate only. Either way, show that kernel `.text` with
`power-control` off is identical to main's, measured with `NEO_BUILD_ID` pinned.

## Prior art

THINK §7 has the survey: licences, last commits, and what is taken as principle only. P1 adds nothing. If you read
another implementation's AML walker (ACPICA is dual-licensed; Linux is GPL), it's for principle only, and you say so
in the PR body.

## Rules

- `scripts/worker-close-gate.sh` and one PR. It's host-only, so the gate is push CI; the PM adds it to a batch anyway
  if it touches `kernel/`.
- PR body: the oracle results per set, the kill-switch `.text` table, and the prior-art line.
- Review after merge: Janus (the reviewer of the THINK's walk), GPT-6-Luna high, a contained check that the Rust
  port matches the revision-5 walk.
- Commit and push at every checkpoint. Send the PM the head SHA from `git ls-remote`.

```
WORKTREE: cloud session S5 (your own clone)
BRANCH:   claude/ot90-p1-power-logic
          git fetch origin && git checkout -B claude/ot90-p1-power-logic origin/main
git show origin/main:Plans/DISPATCH-2026-10-29-ot90-p1-power-logic-s5.md
git show origin/main:docs/THINK-2026-10-27-ot90-shutdown-restart.md
```

RAG isn't available in the cloud session. Before ending, `git status --porcelain` and `git log @{u}..` are both empty.
