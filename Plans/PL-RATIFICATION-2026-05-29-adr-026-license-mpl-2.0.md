# PL-RATIFICATION: ADR-026 — project license MPL-2.0 (code) + CC-BY-4.0 (docs)

**Date:** 2026-05-29
**From:** PL (JC) — confirmed on mobile
**To:** Project record

---

## Ratification

ADR-026 is RATIFIED and adopted as `docs/ADR-026-license-mpl-2.0.md`.

**Decision:**
- Source code (kernel/, userland/, tools/, tests/) -> MPL-2.0. `./LICENSE` holds the exact canonical text.
- Documentation + governance (docs/, Plans/, ADRs, project *.md) -> CC-BY-4.0.
- Third-party dependencies retain upstream licenses; dependency caches are not AltimitNeo code.
- GPL rejected (per Bible Article 26).

## Rationale of record

MPL-2.0 chosen over Apache-2.0 because the AI subsystem (Phases 5-7) is the core differentiator: MPL's file-level copyleft keeps kernel and AI-subsystem improvements in the commons, while avoiding the GPL userland-boundary headache (MPL copyleft attaches per file, not per linked work, so application/userland licensing is unconstrained). Documentation is prose and takes CC-BY-4.0 (freely shareable with attribution), not MPL.

## Bible status

This ADR resolves the open decision in Bible Article 26 ("to be decided before public release"). Follow-up governance step: a Bible amendment marking Article 26 decided and referencing ADR-026 (PL-tier; tracked).

## Open questions carried (pre-public-release, non-blocking)

Q1 copyright holder identity; Q2 contributor terms (DCO vs CLA, leaning DCO); Q3 doc-license edge cases. None block adoption.

## Execution authorized

PM to roll out SPDX-License-Identifier headers via Prometheus dispatch (Plans/DISPATCH-2026-05-29-spdx-headers-prometheus.md).

---

**ADR-026 is binding as of 2026-05-29.**