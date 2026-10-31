# ADR-026: Project license â€” MPL-2.0 for code, CC-BY-4.0 for documentation

**Date:** 2026-05-29
**Author:** PM (Cerebro), drafting
**Status:** Ratified 2026-05-29 by project-lead approval (JC). Binding.
**ADR Number:** ADR-026.
**Linked to:** ADR-025 (ratification/adoption pattern); resolves the open item in Bible Article 26.
**Bible articles affected:** Article 26 (License â€” previously "to be decided before public release"; this ADR makes the decision). A follow-up Bible amendment marks Article 26 decided and references this ADR.

---

## Decision

1. **All NeoPlutarch source code is licensed under the Mozilla Public License 2.0 (MPL-2.0).**
   Scope: `kernel/`, `userland/`, `tools/`, `tests/`, and all Rust sources and build scripts therein. Recorded in `./LICENSE` (exact canonical MPL-2.0 text) and marked per-file with `SPDX-License-Identifier: MPL-2.0`.

2. **All NeoPlutarch documentation and governance artifacts are licensed under Creative Commons Attribution 4.0 International (CC-BY-4.0).**
   Scope: `docs/` (the Bible, ANSPEC specs, ADRs, phase documents), `Plans/` (planning/dispatch/return/disposition/ratification artifacts), and project-authored `*.md` at the repo root. Marked where applied with `SPDX-License-Identifier: CC-BY-4.0`.

3. **Third-party dependencies retain their own upstream licenses.** Dependency caches (e.g., `.cargo-home/`) are not NeoPlutarch code and are not covered by `./LICENSE`.

GPL is rejected (consistent with Bible Article 26).

## Rationale

### Why MPL-2.0 over Apache-2.0 for code

- **The AI subsystem (Phases 5-7) is the core differentiator.** ANSPEC-001 territory â€” kernel-resident tensor/model/inference-context primitives and the multi-tenant accelerator scheduler â€” is where NeoPlutarch's distinctive value concentrates. MPL-2.0's **file-level copyleft** keeps improvements to those files in the commons: anyone who modifies an MPL file must make that modified file available under MPL. Apache-2.0 (permissive) would allow a downstream party to take the AI subsystem, improve it privately, and never contribute back. For the subsystem that justifies the project's existence, "improvements return to the commons" is the right default.

- **MPL keeps that copyleft without the GPL userland-boundary headache.** GPL's copyleft attaches to the whole linked work, which raises unresolved questions at the kernel/userland boundary (does a userland program built against the native ABI become a derivative of the kernel?). That is exactly the "distraction" Bible Article 26 calls out. **MPL-2.0 sidesteps it entirely: copyleft attaches per file, not per linked work.** A userland application can be under any license; only modifications to NeoPlutarch's own MPL files are copyleft. This gives commons-protection for our code without imposing license terms on applications that merely run on or link against the system.

- **Adoption-friendly.** MPL files can be combined with proprietary or differently-licensed code in a larger work, lowering the barrier for downstream use while still protecting the specific files we wrote. This matches the "permissive enough for adoption" criterion in Bible Article 26.

### Why CC-BY-4.0 for documentation

Documentation, specifications (the Bible, ANSPEC), and decision records (ADRs) are **prose, not code**. Software licenses including MPL-2.0 are poorly suited to prose and create ambiguity when applied to it. CC-BY-4.0 is the appropriate instrument: it keeps the specs and decision records **freely shareable and quotable with attribution**, which serves the project's documentation-as-first-class-artifact culture (Bible Article 18) and lets others cite ANSPEC/ADRs cleanly.

## SPDX convention

- Every source file carries `// SPDX-License-Identifier: MPL-2.0` (comment syntax appropriate to the file type) as the first line(s).
- Documentation files carry `SPDX-License-Identifier: CC-BY-4.0` where a header is practical; directory-level declaration via `NOTICE` covers the rest to avoid churning hundreds of prose files.
- Header rollout is executed by a dedicated dispatch (Prometheus, SPDX-header pass).

## Consequences

**Positive:**
- Commons-protection for the differentiating code (AI subsystem, kernel) by construction.
- No GPL boundary ambiguity; userland/application licensing is unconstrained.
- Specs/ADRs are cleanly shareable with attribution.
- SPDX headers make per-file licensing machine-checkable (REUSE-compatible).

**Negative / costs:**
- MPL file-level copyleft is a real obligation contributors must understand (modifying an MPL file = share that file under MPL).
- Two-license split (code vs docs) requires the SPDX discipline to be maintained as files are added.

**Neutral:**
- No runtime or build impact; licensing is metadata + the LICENSE/NOTICE files.

## Open questions (finalize before public release; not blockers to adoption)

- **Q1. Copyright holder identity.** `NOTICE` currently reads "NeoPlutarch project contributors." Decide whether copyright vests in an individual, an entity, or contributors collectively before public release.
- **Q2. Contributor terms.** DCO (sign-off) vs CLA. Leaning DCO for low-friction inbound=outbound under MPL-2.0; to be settled in its own ADR if needed.
- **Q3. Doc-license edge cases.** Generated artifacts, vendored doc snippets, and any embedded third-party prose need a per-case note.

## Artifacts produced with this ADR

- `./LICENSE` â€” exact canonical MPL-2.0 text (fetched from the authoritative Mozilla source).
- `./NOTICE` â€” licensing summary (code MPL-2.0, docs CC-BY-4.0, third-party retains upstream).
- `Plans/PL-RATIFICATION-2026-05-29-adr-026-license-mpl-2.0.md` â€” ratification record.
- `Plans/DISPATCH-2026-05-29-spdx-headers-prometheus.md` â€” SPDX header rollout dispatch.

---

**Status: RATIFIED
Supersedes: ADR-009 (MIT OR Apache-2.0, now superseded by this decision) 2026-05-29.** Binding project decision. A Bible amendment marking Article 26 decided (referencing this ADR) is the follow-up governance step.