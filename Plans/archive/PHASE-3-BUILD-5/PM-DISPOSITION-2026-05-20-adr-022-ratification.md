# PM Disposition - ADR-022 Ratification

Date: 2026-05-20
From: PM
Status: RATIFIED PER ARTICLE 27 PL SIGNOFF
Subject: ADR-022 (Fault-recovery direction — forward marker for Theseus-style cell-swap)

Authority:

- `docs/ADR-022-fault-recovery-direction-forward-marker.md`
- `Plans/PM-DISPOSITION-2026-05-20-prometheus-three-item-followup-consumption.md` §4.1
- Bible Article 27 (amendments and ADR ratification)
- PL signal 2026-05-20 directing option (a): "ratify as-is"

## 1. Disposition

ADR-022 is ratified as-is per Bible Article 27 and PL signoff.

The ADR's Status field already reads:

```text
Status: Accepted (forward marker; no architectural commitment)
```

PL signoff confirms this status. ADR-022 is now an authoritative ADR in
the project's normative ADR tree, with the specific semantics that:

- it records the consideration of Theseus-style cell-swap fault recovery;
- it explicitly defers the decision; it does NOT commit the project to
  adopting cell-swap;
- it names three concrete conditions under which the project would
  revisit (Phase 4+ network failure mode, Phase 6+ accelerator driver
  fault mode, equivalent intralingual fault-isolation pattern emerging);
- it does NOT propose a Bible amendment to Article 9.2; the Article 9.2
  tension is explicitly named and left unresolved as part of the
  deferral.

## 2. What This Ratification Does NOT Do

For clarity, the following are explicitly NOT effects of this
ratification:

- The project is NOT committed to adopting Theseus cell-swap in any
  phase.
- Bible Article 9.2 is NOT amended.
- No phase-roadmap change is introduced.
- No engineering work is scheduled toward fault recovery beyond what
  already exists in Bible Article 1.2 / 15.2 ("make the bug impossible").
- No subordinate ADR or ANSPEC is implicitly opened by ratifying this
  one.

The ADR documents a *deferral*. The ratification documents that the
*deferral itself* is now an authoritative project position.

## 3. Forward Triggers Now Active

The three "Conditions for revisit" named in ADR-022 are now the official
triggers for reopening the fault-recovery direction question. Should any
of these conditions materialize in Phase 4+ or Phase 6+, the next PM
seat (or whoever holds the seat then) should surface ADR-022 to PL and
discuss whether the trigger has been met and whether the consideration
should be reopened.

The conditions, restated for forward-reference convenience:

1. A Phase 4+ network-stack failure mode where a driver panic costs
   significantly more (lost connection state, in-flight request loss
   across tenants) than a graceful driver restart would.
2. A Phase 6+ accelerator driver fault mode where the alternative to
   graceful restart is killing in-flight inference contexts owned by
   other processes.
3. An equivalent fault-isolation design (Asterinas OSTD framekernel
   discipline, or another intralingual approach) becomes substantially
   easier to adopt than Theseus cell-swap and produces similar
   guarantees.

## 4. Documentation Lifecycle

ADR-022's status field already reflects its ratified state. No edit to
the ADR is required by this ratification.

PM will not generate further artifacts for the ADR-022 lifecycle unless
a forward trigger fires.

## 5. Lanes Status After This Disposition

This disposition closes the ADR-022 ratification loop opened by
`Plans/PM-DISPOSITION-2026-05-20-prometheus-three-item-followup-consumption.md`
§4.1.

Remaining forward items queued for PL:

- BUILD-4D (ext4 read-only) — authorization signaled by PL 2026-05-20;
  PM dispatching in
  `Plans/DISPATCH-2026-05-20-phase-3-build-4D-ext4-readonly-jarvis.md`.
- BUILD-5 bare-metal target — pending PL hardware commitment.

No other PM-side action triggered by this ratification.
