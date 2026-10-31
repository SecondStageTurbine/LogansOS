# DISPATCH - Phase 3 BUILD-2 post-close docs/index and local merge

**Date:** 2026-05-14
**Owner:** Jarvis
**Status:** DISPATCHED
**Mode:** post-close documentation/integration only
**Base branch:** `codex/phase-3-build-2-2e-typed-inode-filecap`
**Base HEAD:** `d49f9ab95c2941bde7e499157228c0e9a26a067e`
**Authority:** `Plans/PM-DECISION-2026-05-14-phase-3-build-2-close-report-ratification.md`

---

## 1. Mission

Record Phase 3 BUILD-2 closure in the phase docs and Plans index, commit the
allowed closeout artifacts, and locally fast-forward `main`.

This is documentation/integration only. Do not start BUILD-3.

---

## 2. Allowed Files

Edit:

- `docs/phases/PHASE-3.md`
- `Plans/INDEX.md`

Include these untracked closeout artifacts in the docs commit:

- `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md`
- `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-2-2e-consumption.md`
- `Plans/DISPATCH-2026-05-14-phase-3-build-2-close-smp-sweep-jarvis.md`
- `Plans/PM-DECISION-2026-05-14-phase-3-build-2-close-report-ratification.md`
- `Plans/DISPATCH-2026-05-14-phase-3-build-2-post-close-docs-and-merge-jarvis.md`

Do not stage or commit the unrelated pre-existing untracked Plans/docs files,
`AGENTS.md`, or `tools/wedge-investigation/`.

---

## 3. Required Phase Doc Update

Update `docs/phases/PHASE-3.md` so it no longer says BUILD-2 has not started.

Required facts:

- Phase 3 status: BUILD-1 and BUILD-2 closed; BUILD-3 not started.
- BUILD-1 close report remains
  `Plans/CLOSE-REPORT-2026-05-12-phase-3-build-1.md`.
- BUILD-2 close report is
  `Plans/CLOSE-REPORT-2026-05-14-phase-3-build-2.md`.
- BUILD-2 implementation series:
  - `f781957` 2A format/mkfs;
  - `309f28e` 2B read-only mount;
  - `d2c8810` 2C create/write;
  - `a2f1e49` 2D CoW/fsync/recovery;
  - `f6532cb` 2E typed/FileCap;
  - `d49f9ab` close SMP sweep evidence.
- BUILD-2 delivered NeoFS native v1, narrow Regular-file create/write/readback,
  CoW recovery, `SYS_FSYNC = 15` no-op semantics, typed-inode executable policy
  helper, and FileCap stale-generation rejection.
- BUILD-2 did not start BUILD-3 CAS, dedup, write-back cache, FLUSH
  negotiation, GC, real file-backed mmap/exec, runtime mkfs, retype, or broader
  POSIX work.

Keep the document concise. Do not rewrite ADR-021, the Bible, or
`docs/specs/neofs-format.md`.

---

## 4. Required Plans Index Update

Append a targeted "Phase 3 BUILD-2 artifacts (2026-05-13 to 2026-05-14)"
section to `Plans/INDEX.md`.

At minimum, include rows for:

- 2A verification artifact;
- 2B verification artifact;
- 2C verification artifact;
- 2D verification artifact;
- 2E verification artifact;
- close SMP sweep addendum;
- BUILD-2 close report;
- Prometheus close-evidence prep;
- Cerebro 2E policy/generation review;
- PM close report ratification;
- this post-close dispatch.

Use `closed` for landed/closed artifacts and `historical` for prep/research
sidecars where appropriate. Keep this as a targeted section; do not perform a
broad index archaeology pass.

---

## 5. Verification

Before committing:

```text
git diff --check
cargo k
cargo k --no-default-features
```

Runtime re-runs are not required for this docs-only post-close commit because
`Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md` already records
the closing full sweep. If Jarvis chooses to run a targeted smoke anyway, record
it in the verification note.

After staging only the allowed files:

```text
git diff --cached --check
```

Commit message:

```text
docs: record Phase 3 BUILD-2 closure
```

---

## 6. Local Merge

After the docs commit succeeds and verification passes:

1. Record the branch and HEAD before merge.
2. Switch to `main`.
3. Fast-forward merge the BUILD-2 branch.
4. Record the resulting `main` HEAD.

Do not push.

If `main` cannot fast-forward cleanly, stop and file a blocker. Do not perform
a non-fast-forward merge without PM authorization.

---

## 7. Verification Artifact

Create:

`Plans/PHASE-3-BUILD-2-POST-CLOSE-DOCS-VERIFICATION-2026-05-14.md`

Include:

- files changed;
- commit hash for the docs/index commit;
- verification commands and results;
- pre-merge branch/HEAD;
- local `main` fast-forward result;
- explicit statement that no BUILD-3 work, push, ADR/spec/Bible update, or
  unrelated cleanup was done.

This verification artifact is allowed in the docs commit if created before
commit; otherwise it may be committed as a tiny follow-up Plans-only artifact.

---

## 8. Stop Conditions

Stop and file a blocker if:

- verification fails;
- local `main` cannot fast-forward;
- staging would require unrelated untracked files;
- docs wording would imply BUILD-3 has started;
- the task begins to require ADR/Bible/spec changes.
