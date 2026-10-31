# DISPATCH - Phase 3 BUILD-1 post-close docs/index integration and merge

**Date:** 2026-05-12
**To:** Jarvis
**From:** PM
**Status:** READY
**Branch:** `codex/phase-3-build-1-jarvis`

---

## 1. Mission

Perform the post-close integration lane for Phase 3 BUILD-1.

BUILD-1 is PM-ratified closed by:

`Plans/PM-DECISION-2026-05-12-phase-3-build-1-close-report-ratification.md`

Primary close report:

`Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`

Your job is documentation/index integration and local merge readiness. Do not
start BUILD-2 or NeoFS native work.

---

## 2. Inputs

Read these first:

- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-1-close-report-ratification.md`
- `docs/phases/PHASE-2.md`
- `Plans/INDEX.md`
- `docs/ADR-021-phase-3-design-substrate.md`

Treat Prometheus's close report as the canonical BUILD-1 evidence bundle.

---

## 3. Required Edits

### 3.1 Create `docs/phases/PHASE-3.md`

Create a normative Phase 3 phase-status page modeled on the style of
`docs/phases/PHASE-2.md`.

Required content:

- Status line: Phase 3 in progress, BUILD-1 closed, BUILD-2 not started.
- Started date: 2026-05-11 or the date already established by Phase 3 THINK
  artifacts if the docs say otherwise.
- ADRs landed/active: include ADR-021 as the Phase 3 design substrate.
- BUILD-1 section: block layer, DMA buffers, virtio-blk write-through raw R/W,
  minimal VFS/testfs/file syscalls, integration smokes, reboot persistence, and
  aarch64 barrier-exposure lane.
- Course corrections:
  - 1B scheduler lifetime blocker resolved by 1B0 `ThreadWakeToken`.
  - 1D aarch64 blocker resolved by `7f48c85` dedicated QEMU aarch64 lane.
- Verification section with the close report evidence, including the aarch64
  PASS frame facts: `runtime=aarch64`, `transport=virtio_mmio`,
  `event_idx_negotiated=true`, `publish_descriptors=2`,
  `before_notification_check=2`, `data_integrity=ok`,
  `compile_only=false`.
- Forward marker: BUILD-2 NeoFS native is not started and requires a separate
  PM dispatch.

Do not rewrite ADR-021 semantics here. This is a phase-status document, not a
design change.

### 3.2 Update `Plans/INDEX.md`

Add a compact Phase 3 BUILD-1 section or entries for the closeout artifacts.

At minimum include:

- `CLOSE-REPORT-2026-05-12-phase-3-build-1.md`
- `PM-DECISION-2026-05-12-phase-3-build-1-close-report-ratification.md`
- `DISPATCH-2026-05-12-phase-3-build-1-post-close-docs-and-merge-jarvis.md`
- `PHASE-3-BUILD-1-AARCH64-LANE-VERIFICATION-2026-05-12.md`
- 1A, 1B0, 1B, 1C, and 1D verification artifacts.
- aarch64 blocker and blocked disposition artifacts, marked resolved/superseded
  by the aarch64 lane and close report.
- Cerebro aarch64 feasibility research and Prometheus closure prep.

Respect the index's status legend. If the branch has not yet merged to `main`
at the time you edit the row, do not falsely mark implementation artifacts as
`closed` under that legend. If you complete the local merge as part of this
lane, final index wording may mark the intended BUILD-1 artifacts closed.

---

## 4. Commit Scope

Commit only the closeout/docs files required for this lane.

Allowed add/update set:

- `docs/phases/PHASE-3.md`
- `Plans/INDEX.md`
- `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`
- `Plans/PM-DECISION-2026-05-12-phase-3-build-1-close-report-ratification.md`
- `Plans/DISPATCH-2026-05-12-phase-3-build-1-post-close-docs-and-merge-jarvis.md`

Do not add unrelated untracked `Plans/` files. The working tree is known to
contain substantial pre-existing untracked plan/doc material.

Suggested commit message:

`docs: record Phase 3 BUILD-1 closure`

---

## 5. Verification Before Merge

Run after the docs/index commit:

- `git diff --check HEAD~1..HEAD`
- `cargo k`
- `cargo k --no-default-features`
- `cargo neo-test phase3_aarch64_virtio_barrier_lane`
- `cargo neo-test phase3_block_vfs_smoke`

If Docker/Limine is required on this host, use the same Docker/Limine path
recorded in the BUILD-1 verification artifacts. Record exact commands and
outcomes in your closeout note.

Global `cargo fmt --check` is not a merge blocker for this lane. If run, it
must be reported honestly as blocked/failing on pre-existing formatting drift
unless it actually passes globally.

---

## 6. Merge Authorization

After the docs/index commit and verification pass, locally merge
`codex/phase-3-build-1-jarvis` to `main`.

Do not push unless the project lead separately asks for a push.

After merge, produce a closure note with:

- final branch/HEAD before merge,
- merge commit or fast-forward result,
- files committed in the docs/index commit,
- verification commands and results,
- confirmation that BUILD-2 was not started,
- confirmation that unrelated untracked files were not added.

---

## 7. Stop Conditions

Stop and file a blocker instead of merging if:

- Any required close report evidence is missing or contradictory.
- The aarch64 barrier lane fails.
- `cargo k` or `cargo k --no-default-features` fails.
- The docs/index update requires ADR-021 semantic changes.
- Merge conflicts appear outside the BUILD-1 implementation/doc surface.
- Completing the lane would require adding unrelated untracked files.
