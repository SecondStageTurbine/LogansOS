# PM-NOTE: WEDGE-INV-5+6 outcomes, four-signature timing-shift picture, and the find-the-mechanism directive

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the outcomes of WEDGE-INV-5 (0/60 pre-flight stop) and WEDGE-INV-6 (1/200 `SMOKE_MID_PHASE`), the four-signature timing-shift project-state interpretation those measurements support, the project lead's explicit rejection of the regression-guard discharge path, and the launch of WEDGE-INV-7 as the next mechanism-localization attempt.
**Authority:** PM. Records investigation results and the project lead's directional decision. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` is reinforced here, not amended; project lead's directive is "we cannot ship it with errors" and Path C (commitment amendment with regression-guard discharge) is therefore off the table.

---

## 1. WEDGE-INV-5: stop at pre-flight

WEDGE-INV-5 attempted out-of-band observation of `POST_SMOKE_SMP_SCENARIO` via the QEMU GDB stub. Cerebro chose dispatch §3.1.2 (direct QEMU invocation, bypassing `cargo neo-test`) to avoid an `xtask` change. Pre-flight produced 0/60 in-kernel timeouts. The dispatch's §9 stop condition triggered; the 200-boot main loop did not run.

Cerebro flagged the confound: §3.1.2 conflated two changes from WEDGE-INV-3+4's substrate (driver path: cargo → direct; GDB stub: off → on). The 0/60 result could mean either "the GDB stub masks the bug" or "the cargo-vs-direct cadence change masks the bug" or both. The result was honest and the stop call was correct.

## 2. WEDGE-INV-6: disambiguation produced an unexpected finding

WEDGE-INV-6 isolated direct-QEMU-no-stub for 200 boots. Result: 1/200 = 0.5% in-kernel timeout. Case II marginal under the dispatch's matrix.

The signature was the unexpected finding. The single timeout was `SMOKE_MID_PHASE` — the same signature as PATCH1's runs 13/15 and H1's run-4 from earlier in the campaign. **Not** the `POST_SMOKE_SMP_SCENARIO` that cargo-driven WEDGE-INV-3+4 produced 4 times.

## 3. The four-signature timing-shift picture

Combining the three measurements:

| Configuration | Boots | Timeouts | Rate | Signatures observed |
|---|---:|---:|---:|---|
| Cargo, no stub (WEDGE-INV-3+4) | 150 | 4 | 2.67% | `POST_SMOKE_SMP_SCENARIO` ×4 |
| Direct QEMU, stub (WEDGE-INV-5) | 60 | 0 | 0.0% | none |
| Direct QEMU, no stub (WEDGE-INV-6) | 200 | 1 | 0.5% | `SMOKE_MID_PHASE` ×1 |

Same kernel binary (`05e15c9`, functionally equivalent to current `708223b`). Same scenario. Same `NEO_SMP=4`. Different host-side timing → different latent race surfaces. Combined with `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`'s three pre-substrate-shift signatures (`SMOKE_MID_PHASE`, `SMOKE_COMPLETION` literal, `POST_SMOKE_USER_PROCESS`) all going quiet on the post-ADR-015 substrate, we now have evidence of **at least four distinct latent partial-reap-class races**, each surfacing under its own timing profile.

**What this means for project-state interpretation:**

- The convergence-note story (`Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10) is strongly supported. ADR-015's substrate change appears to have damped — not eliminated — multiple distinct memory-layout-sensitive timing windows. Different damping per signature per timing profile.
- Mechanism localization on any single signature does not address the others. They are distinct races, not one root cause.
- Statistical disposition framing ("the wedge happens at rate X under conditions Y") is incomplete — the rate is conditional on the timing profile, and the population that surfaces is also conditional. There is no single rate to track.

## 4. Project lead's directive: find the mechanism

The PM presented three paths in light of the four-signature finding:

- **A1** — cargo+stub via `xtask` env-var gate, ~50 min, dispositive on whether stub masks under cargo.
- **B2** — kernel-side scenario watchdog in stash, more invasive, lower-priority fallback.
- **C** — accept the four-signature picture, amend the wedge-fix commitment to add a regression-guard discharge path, draft an ADR for multi-signature regression guard.

PM leaned C with A1 as defensible alternative.

**Project lead's response (2026-05-08):** "I want to find the source of the error, we cannot ship it with errors."

This is a directional decision with two clear consequences:

1. **Path C is rejected.** The regression-guard discharge path is off the table. The wedge-fix-before-SC-3B commitment stays active in its original form: actual mechanism, actual fix, before SC-3b ships. PM does not draft the commitment amendment; PL has explicitly rejected the framing.
2. **Path A1 proceeds.** WEDGE-INV-7 dispatch drafted: cargo+stub via small `NEO_QEMU_GDB_PORT` env-var gate in `xtask`. Pre-flight 30 (extending to 60 if 0/30) before the 200-boot main loop with GDB capture. If pre-flight produces 0/60, stop and ping; the next dispatch pivots to a different observation strategy (likely B2 territory).

The standing directive is that we keep investigating until the mechanism is found. The investigation may take additional dispatches if A1 produces another inconclusive result; that is acceptable cost relative to shipping a kernel with a known-reproducible wedge.

## 5. Disposition of the wedge-fix-before-SC-3B commitment

**Active in original form.** The commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` requires:

- Test-runner intermittent wedge fixed
- Before sub-commit 3b ships
- De-escalation: project lead via ADR or amendment only

WEDGE-INV-4 did not discharge it (Case III verdict). WEDGE-INV-5 did not discharge it (no measurement). WEDGE-INV-6 did not discharge it (Case II marginal, signature shift confirmed bug class). The project lead's directive forecloses regression-guard-as-discharge.

The only paths to discharge remain:

1. **Mechanism + fix path.** Localize at least the `POST_SMOKE_SMP_SCENARIO` mechanism (the strongest cargo-driven reproducer), commit a real fix, verify the fix at sufficient sample size that the rate is statistically zero (or close to it). Other signatures may need their own fixes; the commitment specifies "the test-runner intermittent wedge" without disambiguating four distinct races, but PL's "cannot ship with errors" framing implies *all* identified bugs need addressing before SC-3b ships.
2. **PL re-deferral via ADR or amendment.** Always available, but not currently on the table.

The PM's role from here is to execute the investigation cascade until mechanism is named, then to coordinate the fix dispatch + verification, then to mark the commitment met.

## 6. WEDGE-INV-7 launched

Dispatch at `Plans/DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md`. Highlights:

- One tracked code change permitted: minimal `NEO_QEMU_GDB_PORT` env-var gate in `xtask`. Default OFF. Single small commit before investigation runs.
- Pre-flight 30 boots (extend to 60 if 0/30). If pre-flight 0/60, STOP — GDB stub masks under cargo too; next dispatch pivots.
- If pre-flight reproduces, main loop 200 boots with GDB capture per WEDGE-INV-5 §3.3 wrapper.
- §6 decision matrix: 7 cases (the 6 from WEDGE-INV-5 plus a Case G for the pre-flight stop).
- Substrate after: HEAD = the new xtask commit. Stashes preserved (verified by the 2026-05-08-refined backup script).

## 7. What does NOT change with this note

- The Bible. No amendments.
- ADR-015. No amendment.
- Any other ADR.
- The two preserved stashes.
- The "Open Project Commitments" section of `Plans/INDEX.md` — the wedge-fix-commitment row stays as-is.
- The wedge-fix commitment's source artifact (`Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md`) — no change to its text.

## 8. Cross-references

- `Plans/WEDGE-INV-5-FINDINGS-2026-05-08.md` — the 0/60 pre-flight result with confound.
- `Plans/WEDGE-INV-6-FINDINGS-2026-05-08.md` — the 1/200 `SMOKE_MID_PHASE` result that produced the timing-shift evidence.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-7-cargo-stub-cerebro.md` — the next dispatch.
- `Plans/PM-NOTE-2026-05-08-wedge-inv-4-case-iii-verdict.md` — the prior verdict note; this note extends its four-signature framing with timing-shift evidence.
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md` — three signatures pre-substrate-shift; together with WEDGE-INV-4's `POST_SMOKE_SMP_SCENARIO` and WEDGE-INV-6's surfacing of `SMOKE_MID_PHASE` post-substrate-shift, this is the source of the four-signature picture.
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — the timing-window-disruption hypothesis, which the WEDGE-INV-6 timing shift strongly supports.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment.

---

**End of note.**

*The investigation continues. The project lead has rejected the regression-guard discharge path. The cost of additional observation dispatches is paid by the SC-3b gate; that is acceptable cost relative to shipping kernel with a known-reproducible wedge. WEDGE-INV-7 is the next attempt.*
