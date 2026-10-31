# DISPATCH - Phase 3 BUILD-2 close SMP sweep gate

**Date:** 2026-05-14
**Owner:** Jarvis
**Status:** DISPATCHED
**Mode:** verification-only closure gate
**Base:** `f6532cb7dabb5f94adc1134b5cfd2fdd6b2ee6be` (`kernel/fs: enforce NeoFS typed FileCaps`)
**Authority:** `Plans/PM-DISPOSITION-2026-05-14-phase-3-build-2-2e-consumption.md`

---

## 1. Mission

Close the only remaining BUILD-2 close blocker:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

This is verification-only. Do not implement new features, do not start BUILD-3,
do not file a close report, do not merge, and do not push.

---

## 2. Required Run

From branch `codex/phase-3-build-2-2e-typed-inode-filecap` at commit
`f6532cb7dabb5f94adc1134b5cfd2fdd6b2ee6be`, run:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

If practical, begin from a quiet Docker state with no leftover test container
from the prior failed sweep. Do not delete unrelated data or images.

---

## 3. If The Sweep Passes

Create:

`Plans/PHASE-3-BUILD-2-CLOSE-SMP-SWEEP-ADDENDUM-2026-05-14.md`

The addendum must include:

- branch and HEAD commit;
- exact command;
- exit status;
- explicit PASS statement for the full non-explicit `NEO_SMP=4` sweep;
- note that prior `phase3_virtio_blk_abandoned_io_no_leak` failure did not
  reproduce in the full rerun;
- confirmation that no code, close report, BUILD-3 work, merge, or push was
  done.

The addendum may be committed as a Plans-only evidence commit.

Do not file the BUILD-2 close report unless PM separately authorizes it after
consuming the addendum.

---

## 4. If The Sweep Fails

Create one blocker/failure artifact:

`Plans/BLOCKER-2026-05-14-phase-3-build-2-full-smp-sweep-jarvis.md`

Include:

- failing scenario name;
- serial PASS/FAIL excerpt;
- command and exit status;
- whether the failure occurs before or during NeoFS 2E scenarios;
- targeted rerun result for the failing scenario on the 2E branch under
  `NEO_SMP=4`;
- targeted rerun result on base `a2f1e49` if the failure may be unrelated to
  2E;
- whether any container/test residue remained after the failure;
- recommendation: rerun, investigate, or PM waiver.

Do not fix code unless PM explicitly authorizes a follow-up implementation
dispatch. If the failure is clearly caused by 2E, say so plainly.

---

## 5. Non-Scope

Do not do any of the following:

- BUILD-3 planning or code;
- close report;
- phase docs, index, ADR, or Bible updates;
- merge or push;
- broad rustfmt cleanup;
- new NeoFS features;
- changes to virtio/barrier/block driver code unless PM authorizes a separate
  bug-fix dispatch.
