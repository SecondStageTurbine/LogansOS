# PM-NOTE: WEDGE-INV-4 Case III verdict — commitment stays active, four-signature picture

**Date:** 2026-05-08
**Author:** PM
**Type:** Project-state record. Captures the Case III verdict from `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md`, the resulting disposition of the wedge-fix-before-SC-3B commitment, the four-signature picture that emerged when WEDGE-INV-4's residual timeouts were qualitatively classified, and the project-lead's decision to proceed with Option B (out-of-band GDB-stub observation) for the next investigation. Doc-only artifact; does not change project commitments — instead it documents that the commitment is NOT discharged by WEDGE-INV-4 data.
**Authority:** PM. Recording the case verdict and the resulting disposition is bookkeeping. The wedge-fix-before-SC-3B commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` remains active in its original form. The selection of Option B was the project lead's call (recorded here for traceability).

---

## 1. What WEDGE-INV-4 produced

Substrate: HEAD `05e15c9` after Cerebro committed `boot: skip Limine boot menu (timeout: 0) to eliminate H7 pre-kernel stall`.

- **M1 (Limine fix verification):** 30/30 PASS on `kaslr_smoke`, 0 H7 / `TIMEOUT_PRE_KERNEL` instances. Limine fix verified.
- **M2 (extended baseline):** 100 uninstrumented `smp_per_cpu_scheduler_smoke` boots at `NEO_SMP=4`. 96 PASS, 4 `TIMEOUT_IN_KERNEL_SMOKE` (runs 15, 28, 45, 90).
- **Combined with WEDGE-INV-3** (50/50 PASS): `4/150 = 2.67%` in-kernel timeout rate. Wilson 95% CI ≈ `1.04% – 6.66%`.

The dispatch's §5 thresholds:

| Case | Threshold | Verdict |
|---|---|---|
| I | 0/150 | NO |
| II | 1–3/150 | NO |
| III | 4+/150 | **YES** |

**Verdict: Case III.** The wedge is not at the historical 22–28% rate (or the H7-corrected 14% rate), but it is meaningfully present and reproducible. The "effectively absent" interpretation is off the table.

## 2. Disposition of the wedge-fix-before-SC-3B commitment

The commitment in `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` **remains active in its original form**. Specifically:

- It is NOT discharged by "addressed-by-substrate-shift-plus-regression-guard" — that path was contingent on Case I, which we did not reach.
- The commitment continues to gate sub-commit 3b: the wedge must be addressed before SC-3B ships.
- The PM cannot re-defer; only the project lead can re-defer via ADR or amendment.
- The commitment amendment that was queued in the handoff §5.2 is **not drafted**. Drafting it now would be premature: we don't yet know what mechanism we're addressing, and a meaningful amendment depends on what WEDGE-INV-5 finds.

## 3. The four-signature picture

Cerebro's findings report makes a discrimination that warrants explicit project-state recording. The literal `signature_classifier()` defined in DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md §4.2 returns `SMOKE_COMPLETION` for all four M2 timeouts because:

- Initial ping/pong/hog smoke completed.
- No `neo-init` or user worker process spawned.

But qualitative inspection of the four logs reveals a tighter pattern: the `smp_per_cpu_scheduler_smoke` scenario dispatch had begun (`NEO_TEST/1 event=BEGIN name=smp_per_cpu_scheduler_smoke phase=dispatch`), all four `smp-hog-{0..3}` workers had spawned, and a subset of those scenario workers had been reaped before the wedge fired. Cerebro reported this as a fourth signature, `POST_SMOKE_SMP_SCENARIO`, distinct from PATCH1's `SMOKE_COMPLETION` examples (where no scenario or user-process phase had begun).

This judgment call — refusing to let the coarser literal classifier hide a qualitatively distinct phase boundary — is the right one. It's the same discipline as `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md`'s separation of three signatures from the original "wedge" label. **The classifier in xtask should be amended in a future Jarvis dispatch to distinguish `POST_SMOKE_SMP_SCENARIO` from `SMOKE_COMPLETION` so future runs don't collapse them.**

The combined picture across PATCH1 and WEDGE-INV-4:

| Signature | PATCH1 (pre-substrate-shift) | WEDGE-INV-4 (post-substrate-shift) |
|---|---:|---:|
| `SMOKE_MID_PHASE` | 2 (runs 13, 15) | 0 |
| `SMOKE_COMPLETION` (literal) | 2 (runs 21, 40) | 0 |
| `POST_SMOKE_USER_PROCESS` | 3 (runs 11, 17, 42) | 0 |
| `POST_SMOKE_SMP_SCENARIO` (new) | 0 | 4 (runs 15, 28, 45, 90) |

PATCH1's three signatures all went quiet. The fourth signature, structurally adjacent to `SMOKE_MID_PHASE` (both involve partial reap during a worker-spawning phase), surfaced as the residual.

## 4. What this story implies (without overclaiming)

- The historical 3 signatures appear to have been addressed enough to be invisible at the 100-boot scale. We do not know mechanism. The convergence-note story (`Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10) — that ADR-015's heap shift disrupted memory-layout-sensitive timing windows — remains the leading hypothesis but is not proved.
- The `POST_SMOKE_SMP_SCENARIO` signature is structurally similar to `SMOKE_MID_PHASE` (partial reap during worker spawn). The substrate change may have *shifted* a partial-reap-class race from the initial smoke phase into the scheduler-stress scenario phase rather than eliminating the underlying class of races.
- 3 of 4 timeouts wedge after exactly `smp-hog-0` reaps; 1 wedges after `smp-hog-0` and `smp-hog-1` reap. The phase boundary is tight. This is a strong reproducer for targeted observation.
- Sub-commit 3a's pre-implementation analysis (`Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`) treats the lock-free `zombie_count == 0` short-circuit as composition-safe under H1's narrow refutation. The `POST_SMOKE_SMP_SCENARIO` signature does NOT immediately falsify that — H1's race window is BSP-vs-AP-zombie-production, and the new signature's zombie producers are scenario workers whose `sched::spawn()` enqueues on `current_scheduler()` (the BSP). But this should be re-examined when WEDGE-INV-5 names a mechanism.

## 5. Next step — Option B selected

Two approaches were named for the next investigation:

- **Option A:** in-kernel instrumentation of the SMP scenario worker spawn/reap path. Faster but heisenbug risk (WEDGE-INV-2 demonstrated instrumentation can mask this bug class).
- **Option B:** out-of-band observation via QEMU's GDB stub. Slower per-boot but substrate-untouched.

**Project lead selected Option B** on 2026-05-08. The dispatch is `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md` (this PM-NOTE is being filed concurrently with the dispatch). The dispatch's design is:

- Pre-flight 30-boot confirmation that the bug still reproduces with the GDB stub enabled.
- Main loop of 200 boots, GDB capture on each timeout.
- State dumps capture per-CPU registers, backtraces, run queues, zombie state, `smp-hog-*` thread states.
- §6 decision matrix maps the cross-dump pattern into one of six cases (lock spin, missing wakeup, reap inconsistency, silent fault, all-CPUs-idle-with-work-outstanding, no convergent pattern).

Substrate stays at `05e15c9`. Stashes remain preserved.

## 6. What does NOT change with this note

- The Bible. No amendments.
- The wedge-fix-before-SC-3B commitment. Active in original form.
- ADR-015. No amendment from this analysis.
- Any other ADR.
- The two preserved stashes.
- INDEX.md's "Open Project Commitments" section (the wedge-fix commitment row stays as-is until WEDGE-INV-5 closes).

## 7. Cross-references

- `Plans/WEDGE-INV-4-FINDINGS-2026-05-08.md` — the findings being summarized.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-4-extended-baseline-cerebro.md` — the dispatch that produced them; §4.2 defines the literal classifier whose collapsing was identified.
- `Plans/DISPATCH-2026-05-08-wedge-investigation-5-gdb-capture-cerebro.md` — the next dispatch (concurrent with this note).
- `Plans/PM-NOTE-2026-05-08-patch1-signature-analysis.md` — the three-signature analysis; this note extends it to four.
- `Plans/PM-NOTE-2026-05-07-wedge-fix-commitment.md` — the active commitment whose disposition is recorded here as "stays active."
- `Plans/PM-NOTE-2026-05-08-wedge-and-q9-convergence.md` §10 — the convergence-note story; remains the leading hypothesis but not proved.
- `Plans/HANDOFF-2026-05-08-wedge-investigation-mid-campaign.md` §3.1 — the case-verdict matrix that this note executes against.
- `Plans/H7-INVESTIGATION-FINDINGS-2026-05-08.md` §5.1 — the canonical classifier; will need a future Jarvis dispatch to add `POST_SMOKE_SMP_SCENARIO` as a distinct bucket.

---

**End of note.**

*Case III. Commitment stays active. Investigation continues with out-of-band observation. The four-signature picture is the project-state context the next dispatch operates inside.*
