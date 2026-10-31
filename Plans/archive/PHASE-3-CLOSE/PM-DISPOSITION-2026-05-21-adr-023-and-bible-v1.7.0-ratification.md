# PM Disposition - ADR-023 + Bible v1.7.0 Ratification

Date: 2026-05-21
From: PM
Status: RATIFIED PER ARTICLE 27 PL SIGNOFF
Subject: ADR-023 (Linux binary compatibility via personality layer) + Bible v1.6.0 → v1.7.0 (Articles 3, 5.6, 28.2)

Authority:

- `Plans/ADR-023-DRAFT-2026-05-21-linux-personality-layer.md`
- `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-21-articles-3-and-5-6-and-28-2-personality-subsystems.md`
- `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md` (Option A/B/C surfacing; PL chose Option B)
- Bible Articles 24, 27 (decision-making, amendment procedure)
- PL signal 2026-05-21: "ratify"

## 1. Disposition

ADR-023 and the co-ratified Bible amendment (Articles 3 fourth bullet edit; new Article 5.6; Article 28.2 cross-reference update; footer bump v1.6.0 → v1.7.0) are RATIFIED as-is per Bible Article 27 and PL signoff.

ADR-023 enters the project's normative ADR tree with the specific semantics that:

- It establishes the architectural mechanism (Linux personality subsystem, kernel-resident, opt-in per process, isolated from the native ABI by construction) by which Bible Article 28.2's accessibility commitment is delivered for Linux software.
- It scopes the work into three syscall tiers (Tier 1 Phase 8, Tier 2 Phase 9, Tier 3 Phase 10+) with a bright-line exclusion list (namespaces, cgroups, eBPF, Linux `capabilities(7)`, perf_event, ptrace-beyond-minimal, kernel module loading).
- It commits one constraint on Phase 6 (accelerator drivers must remain *expressible* as Linux-DRM-compatible character devices) and one constraint on Phase 4 (sockets-equivalent API must be translatable from BSD sockets) — neither restructures earlier phases.
- It defers actual implementation to Phase 8+; PERSPEC-001 is the Phase 8 implementation contract and is not drafted by this ratification.

The co-ratified Bible amendment:

- Edits Article 3 fourth bullet to point at Article 5.6 for the foreign-OS-compatibility mechanism (replacing "POSIX compatibility shim" casual carve-out language with a defined-mechanism reference).
- Adds Article 5.6 formalizing personality subsystems as the architectural form of foreign-OS compatibility: kernel-resident, exec-bound per process, isolated from native ABI by construction, statically compiled (Article 9.2-compliant), each personality requires its own ADR.
- Updates Article 28.2's cross-reference from "the POSIX compatibility shim (Article 3)" to "personality subsystems (Articles 3, 5.6); the Linux personality (ADR-023)…" so the accessibility-goal article points at the mechanism article and at the first ADR delivering it.
- Bumps Bible version v1.6.0 → v1.7.0 (minor; structural addition of Article 5.6, lighter touches on 3 and 28.2).

## 2. What This Ratification Does

In one paragraph: Article 28.2's standing commitment ("the software ecosystem is reachable") acquires a Bible-level architectural mechanism (Article 5.6 personality subsystems) and a first concrete realization (ADR-023 Linux personality). Future ADRs proposing alternative foreign-OS-compatibility approaches are bounded by Article 5.6's specification; an ADR proposing a userspace-only shim, a VM-based hosting model, or POSIX-as-native-ABI is pre-rejected unless it supersedes Article 5.6 through the standard amendment process.

## 3. What This Ratification Does NOT Do

For clarity, the following are explicitly NOT effects of this ratification:

- **No Phase 8 work is opened.** Implementation lives in PERSPEC-001 which is drafted at Phase 8 THINK. This ratification does not authorize implementation work, nor does it shift the phase ledger.
- **No additional foreign-OS personality is authorized.** Article 5.6 establishes the *mechanism* as general; the *only* personality this ratification commits the project to is Linux (per ADR-023). A BSD, macOS, Plan 9, or other personality requires its own ADR.
- **No retroactive change to closed phases.** Phases 1.5 and 2 are closed; Phase 3 is substantively closed. The Process-struct personality byte, syscall-entry personality-dispatch byte, and CI personality-stub target are now Phase 8 retrofit items, recorded as forward markers in ADR-023's Phase-8 checklist.
- **No commitment to a graphics stack, audio personality, or input personality.** Those are Phase 9–10+ work, each requiring its own ADR when its phase opens. ADR-023 commits only to the accelerator-driver design constraint (DRM-compat-expressible) needed for Phase 6 to leave that option open.
- **No commitment to Linux ABI version chasing.** The personality pins to a Linux LTS baseline at Phase 8 launch time (Article 1.4 authority). Newer-Linux-only software may not work and that is acceptable.
- **No Windows personality is authorized or anticipated.** ADR-023 explicitly notes that Proton-on-Linux covers Windows games transitively; a Windows personality is not in scope without a superseding ADR.

The ratification documents architectural intent, the bounding mechanism, and one concrete deliverable (Linux personality). The implementation horizon is Phase 8+.

## 4. Forward Triggers Now Active

Three triggers are now the authoritative reopen points for further architectural work in this area:

1. **Phase 4 THINK opens.** Phase 4's sockets-equivalent API design must be translatable from BSD sockets; this constraint is now Bible-level via Article 5.6's "translate foreign syscalls to native kernel operations" specification. No specific Phase 4 design choice is mandated, but the translatability property is a checkable constraint at Phase 4 ratification.
2. **Phase 6 design substrate begins.** Accelerator drivers per ANSPEC-001 §9 must be designable such that a Linux-DRM-compatible character-device shim can be added without restructuring the driver internals. ANSPEC-001 §9 may need a Phase-6-time amendment to make the constraint explicit.
3. **Phase 8 THINK opens.** PERSPEC-001 (Linux Personality Specification) is drafted; the Phase 8 checklist in ADR-023 §"Forward markers" becomes the planning substrate; LTP subset is selected; the Linux LTS baseline is pinned.

ADR-023 also records five Open Questions (Q-Open-1 through Q-Open-5) that resolve at Phase 8 THINK. They are not ratification blockers; they are deferred-by-design items named so they are not rediscovered.

## 5. Documentation Lifecycle

ADR-023's DRAFT in `Plans/` retains its DRAFT status; the ratified copy lives at `docs/ADR-023-linux-personality-layer.md` with Status flipped from "DRAFT — awaiting PL ratification" to "Accepted". The DRAFT remains in `Plans/` as historical record per the established pattern (compare `Plans/ADR-016-DRAFT-2026-05-08-irq-safe-heap-and-scheduler-decoupling.md` retained alongside `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md`).

The Bible amendment DRAFT in `Plans/` retains its DRAFT status; the amended text lands in `docs/bible.md` with version bump v1.6.0 → v1.7.0. The amendment DRAFT remains in `Plans/` as historical record per the established pattern (compare `Plans/BIBLE-AMENDMENT-DRAFT-2026-05-08-article-8-5-critical-section-discipline.md` retained alongside the v1.0.5 Bible edits).

Both file moves and the Bible edits are mechanical work suitable for Prometheus. PM will produce a dispatch immediately following this disposition.

`Plans/INDEX.md` will be updated by the same dispatch to record:

- Both DRAFTs flip from `draft` to `closed` with the commit hash recorded.
- The Bible amendment row indicates the v1.6.0 → v1.7.0 version transition.
- The ADR-023 entry follows the format established by prior ADR DRAFT → docs/ transitions.

## 6. Lanes Status After This Disposition

This disposition closes the ADR-023 + Bible v1.7.0 ratification loop opened by `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md`. The prior (rescinded) dispatch's stale-context errors are fully discharged; the corrected work product is the artifact pair ratified here.

Remaining forward items queued for PL (unchanged from 2026-05-20 EOD handoff):

- BUILD-5b bare-metal target — pending PL T480s 20L8 hardware acquisition.
- VMware hypervisor-VM verification procedure — pending PL execution.
- Optional Phase 5 AI subsystem OBSERVE pass — pending PL signal.

No other PM-side action triggered by this ratification beyond the Prometheus dispatch.

## 7. PM Process Note

PM's recommendation in the rescission PM-NOTE was Option A (forward-marker ADR shape only; no Bible amendment). PL chose Option B (full ADR + Bible amendment). PM produced Option B work product at full quality; the disagreement is recorded in ADR-023 §"Alternative G" as institutional memory.

This is the correct disposition of a PM-recommends-X-PL-chooses-Y situation per the 2026-05-10 PL clarification on PM/PL roles: PM owns operational + technical decisions and surfaces options with recommendations; PL signs off on legitimately PL-tier calls (Bible amendments are squarely PL-tier); both viewpoints are recorded.

PM also notes that the live-Bible-as-first-action rule (filed in `Plans/PM-NOTE-2026-05-20-prometheus-dispatch-rescission-stale-context-error.md` §3 and reinforced in `Plans/PM-NOTE-2026-05-21-prometheus-dispatch-rescission-bible-ratification-stale-context.md` §4) was honored on this second pass. The corrected ADR-023 was drafted against ground-truth Bible v1.6.0, against the actual ADR ledger (ADR-022 most recent), against actual Phase state (Phase 3 substantively closed), and against the actual project name (NeoPlutarch). No stale-context errors carried into the ratified artifacts.

Filed.
