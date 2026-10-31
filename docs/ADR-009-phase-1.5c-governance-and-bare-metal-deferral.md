# ADR-009: Phase 1.5c Governance Cleanup and Bare-Metal Deferral

| | |
|---|---|
| **Status** | Accepted (license portion superseded by ADR-026) |
| **Date** | 2026-05-04 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | ADR-026 (license: MPL-2.0 replaces MIT/Apache-2.0, 2026-05-29) |
| **Related** | Bible Articles 13, 16.3, 17, 18.1, 26, 27 · ADR-002 · Phase 1.5c BUILD-4 |

---

## Context

The Phase 1.5c close-out audit found several governance/documentation drifts
that are not kernel-feature work but still matter before Phase 2:

- Bible Article 16.3 requires every phase to end with QEMU and bare-metal boot
  verification. Phase 1.5c has QEMU verification only; no hardware target is in
  front of this development environment.
- Bible Article 13's phase ledger still described Phase 1.4 as active.
- Bible Article 18.1 and the Article 17 tree diagram said ADRs live in
  `docs/adr/`, but the ratified in-tree convention is flat
  `docs/ADR-NNN-*.md`.
- Bible Article 26 said the license was undecided, while the workspace has
  shipped with `MIT OR Apache-2.0`.

This ADR records the cleanup instead of letting the drift become folklore.

## Decision

### Q1 — Bare-Metal Verification Deferral

Phase 1.5c is allowed to close with QEMU-only boot verification.

This is a documented deviation from Bible Article 16.3, not a weakening of the
rule. The revisit point is Phase 2 hardware bring-up: Phase 2 may develop in
QEMU, but its phase close-out must either verify on a named bare-metal target or
land a superseding ADR explaining why hardware remains unavailable.

Rationale: faking bare-metal verification is worse than naming the absence.
QEMU verification remains mandatory and was run for BUILD-4. The phase artifact
must state the hardware gap explicitly.

### Q2 — Phase Ledger

Bible Article 13 is amended to reflect the actual state at Phase 1.5c close:
Phase 1.4 and Phase 1.5 are complete; Phase 2 is planned next but not started
by this BUILD.

### Q3 — ADR Path

Bible Article 17 and Article 18.1 are amended to ratify the actual ADR layout:
ADRs live as flat files named `docs/ADR-NNN-*.md`.

Rejected alternative: moving all ADRs into `docs/adr/`. That would churn every
cross-reference for no architectural value. The flat convention is already
readable and grep-friendly.

### Q4 — License

Bible Article 26 is amended to ratify the existing workspace license:
`MIT OR Apache-2.0`.

Rationale: this is the standard Rust ecosystem dual-license pattern, it matches
the current `Cargo.toml`, and it keeps adoption friction low without GPL-style
boundary ambiguity. Future imported third-party code may carry different
licenses, but project-authored code and docs use `MIT OR Apache-2.0` unless a
file says otherwise.

## Consequences

### Positive

- Phase 2 starts with the Bible matching the actual repo instead of a stale
  phase ledger and stale ADR path.
- The bare-metal gap is visible to future contributors and has a concrete
  revisit point.
- The license field in `Cargo.toml` is no longer an undocumented governance
  choice.

### Negative

- Phase 1.5c still did not satisfy the strongest reading of Article 16.3.
  That remains a real verification gap until a hardware target is tested.

## Implementation

- [x] `docs/bible.md` Article 13 phase ledger updated.
- [x] `docs/bible.md` Article 17 and Article 18.1 updated to flat ADR paths.
- [x] `docs/bible.md` Article 26 updated to `MIT OR Apache-2.0`.
- [x] `docs/bible.md` footer bumped to version 1.0.2.
- [x] `docs/phases/PHASE-1.5c.md` records QEMU verification and the bare-metal
  deferral.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by
superseding ADR.
