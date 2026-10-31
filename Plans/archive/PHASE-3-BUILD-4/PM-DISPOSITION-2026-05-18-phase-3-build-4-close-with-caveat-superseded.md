# PM Disposition - BUILD-4 Close-With-Caveat Superseded

Date: 2026-05-18

Supersedes:

- `Plans/PM-DISPOSITION-2026-05-18-phase-3-build-4-stability-fix-6-consumption.md`
- `Plans/DISPATCH-2026-05-18-phase-3-build-4-stability-close-with-caveat-cerebro.md`

## Disposition

The close-with-caveat path is rejected.

The diagnostic substrate from Fixes 8/9/11/12/14/15 remains useful, but BUILD-4
does not close while `phase3_neofs_fsync_filecap_stale_regression` has a known
final double-timeout tail under the required `NEO_SMP=4` gate.

Project rule: **we do not ship broken code and we do not ship a known flaky CI
gate as "closed."**

## Rationale

The previous disposition was defensible only under an "accept residual
environmental risk" posture. That is not the project posture.

The evidence still matters:

- more timeout did not fix the tail;
- smaller disk did not fix the tail;
- storage backend variants did not fix the tail;
- front-of-queue virtio wake helped but did not close the tail.

That evidence now means the work must move down a layer. The remaining question
is not "which harness knob absorbs this?" The remaining question is:

```text
During stale-fsync timeout, is the kernel making forward progress, blocked on a
specific storage/scheduler primitive, or has the host/QEMU substrate stopped
executing the guest?
```

BUILD-4 cannot proceed until that question is answered and the appropriate
mechanism fix lands.

## New Authorized Path

The next authorized lane is:

```text
Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md
```

## Guardrails

Do not:

- close BUILD-4 with residual stale-fsync double-timeout;
- start BUILD-4B;
- increase timeout again;
- add retries;
- shrink disk again;
- remove, ignore, or weaken the stale-fsync scenario;
- claim host/environment cause without out-of-band evidence;
- implement pipelined fsync without a design checkpoint if root-cause
  classification points at storage architecture.

Accepted outcome is either:

- a kernel/test-runner/environment mechanism fix that makes the required gate
  clean; or
- a PM-consumed blocker proving the current local substrate cannot execute the
  gate reliably, with a replacement verification substrate proposed.
