<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PL-DECISION: USB stack + USB Ethernet re-deferred from Phase 4 to Phase 7.5

**Date:** 2026-05-30
**From:** PL (decision recorded by PM/Cerebro)
**Type:** Article 24.1 re-deferral of a bundled phase deliverable
**Status:** DECIDED
**References:**
- docs/bible.md Article 13 phase ledger (Phase 4 / Phase 7.5 lines); Bible v1.8.0 → v1.9.0
- docs/ADR-017-wifi-support-strategy.md §D3 (USB Ethernet connectivity)
- Bible Articles 9.7, 9.9 (USB stack), 9.10 (USB Ethernet)
- docs/phases/PHASE-4.md (Phase 4 retrospective / formal close)
- PM-DECISION-2026-05-28-phase-4-opening.md (original bundling)

---

## Context

The Phase 4 opening scope and the Article 13 ledger bundled **USB stack landing
(Articles 9.7, 9.9)** and **USB Ethernet adapter support (Article 9.10,
ADR-017 §D3)** into Phase 4 as the bare-metal connectivity path. Phase 4
delivered the full network stack (BUILD-1 … BUILD-5) over **virtio-net**;
**no USB host stack or USB Ethernet driver was built** across BUILD-1–5. The
Phase 4 retrospective flagged this as an outstanding scope item.

Per Bible Article 24.1, re-deferring a deliverable the project has committed to
requires an explicit project-lead decision, recorded as an ADR or amendment.

## Decision

USB stack landing and USB Ethernet adapter support are **re-deferred from Phase 4
to the Phase 7.5 connectivity lane** (ADR-017 territory, alongside WiFi).

- Phase 4 closes on the virtio-net network stack; it is **not** held open for USB.
- The Article 13 ledger Phase 4 line is amended to mark Phase 4 Complete and to
  record the re-deferral; the Phase 7.5 line absorbs the USB deliverables. Bible
  bumped v1.8.0 → v1.9.0.
- ADR-017 §D3 remains the design reference for USB Ethernet; its scheduling moves
  to Phase 7.5.

## Rationale

- USB Ethernet is a **connectivity-driver** concern, naturally grouped with the
  Phase 7.5 connectivity work (WiFi / iwlwifi) under the same ADR-017 umbrella,
  rather than with the protocol-stack work Phase 4 delivered.
- Phase 4's value milestone — an end-to-end HTTPS fetch on the native ABI — was
  met over virtio-net and does not depend on the USB transport.
- Bundling held the protocol-stack phase hostage to a driver-class deliverable
  with no shared code path; unbundling lets each ship on its own merits.

## De-escalation rule

This is a re-deferral, not a cancellation. USB stack + USB Ethernet remain
committed deliverables at Phase 7.5. Any further slip requires a fresh Article
24.1 PL decision.
