<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: OT-87 installer THINK final narrow check

**Relay:** #94. **Reviewer:** Chad, Astra review lane.
**Reviewed SHA:** `6f9ae4c851bb0aa3306f4c98a7a186a1983290d0` (revision 5 over `13df6360`).
**Worktree:** `E:\AI Automation\workers\chad`.
**Scope:** relay #90 items 5–6 and its wording notes, as bounded by the relay #94 dispatch.

**Overall verdict: PASS.** The new FAT-formatting conflict is resolved, and the requested scope and wording
clarifications are present. No outstanding finding remains from this narrow review. The PM can merge the
THINK under the dispatch's close condition; implementation gates remain the later rungs' responsibility.

Reviewed the dispatch from `origin/main`, the full revision diff and the affected sections with their
surrounding contracts. The upstream/license checks passed in relay #90 and were not reopened here.
RAG was not used or needed. No build or tests run (review-only dispatch; no build required).

## 1. Host-only fatfs integration — PASS

- `docs/THINK-2026-10-27-ot87-installer.md:337` places both geometry-specific ESP templates in the host
  xtask builder. At `:340`, the builder reads back the generated image, verifies every slot chain is
  contiguous, derives the slot map from that output, and fails the build on a non-contiguous chain.
- The templates and maps are parts of the bundle (`:335`, `:345`). The compiled-in bundle hash and
  per-part validation before writing (`:346`) cover them; the fatfs row states this explicitly (`:854`).
- W3 still copies the ESP template into partition 1 and writes slot A (`:514`). W4 still writes and grows
  the NeoFS seed in partition 2 (`:515`). There is no post-W3 FAT format to invalidate boot files or maps.
- The kernel never formats FAT (`:559`) and never links fatfs (`:869`). The R4 row (`:790`), §4.3 (`:337`),
  §11.2 (`:854`) and §11.4 (`:866`) now agree on the host-only borrow. Activation retains the narrow slot
  writer (`:854`). This closes relay #90's integration failure.

## 2. curtin mapping — PASS

`docs/THINK-2026-10-27-ot87-installer.md:842` now maps replacing NeoFS superblock copies to W4's
seed write and separately describes W3 as replacing the whole ESP from the template. This agrees with
the state machine at `:514` and `:515` and removes the former W3/NeoFS attribution.

## 3. Deferred encryption and Secure Boot scope — PASS

`docs/THINK-2026-10-27-ot87-installer.md:879` explicitly defers encrypted installs and recovery
credentials, naming the pre-mount secret entry, cryptographic storage layer and recovery flow that need
their own design/oracle/review. At `:884`, Secure Boot signing, shim and enrollment are a separate lane.
At `:887`, the Zenbook's Secure Boot-off setting is explicitly a test prerequisite rather than general-PC
support. The installer-documentation requirement at `:890` says the target needs Secure Boot off until
that lane lands. These are the requested scope dispositions, not claims that those features are implemented.

## 4. Wording notes — PASS

`docs/THINK-2026-10-27-ot87-installer.md:479` now qualifies the ordering claim as applying to a
**newly submitted** header and explains why an old header can temporarily mismatch its replacement
array while the other valid, durable GPT copy supplies recovery.

At `:547`, a failed W6 backup flush stops the installer and leaves the primary-selected INCOMPLETE
state. At `:549`, the restarted Finish installing/primary-update case is explicitly a separate attempt,
never continued submission after the failed flush. Both relay #90 wording notes are closed.

## 5. Diff scope — PASS

The complete `13df6360..6f9ae4c8` diff changes only
`docs/THINK-2026-10-27-ot87-installer.md`. Its hunks are the host-template contract (`:337`), the two
wording clarifications (`:479`, `:547`), the matching R4 entry (`:790`), the curtin and fatfs rows (`:842`,
`:854`), the survey outcome and deferrals (`:866`, `:875`), plus the revision-status note at `:12`.
There are no code changes or unrelated design changes. Previously passed sections remain outside this review.

**Disposition:** PASS; return to the PM for the THINK merge. No further revision requested by this review.
