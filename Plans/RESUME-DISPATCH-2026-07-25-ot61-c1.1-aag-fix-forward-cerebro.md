<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESUME DISPATCH: OT-61 browser C1.1 AAG fix-forward

**Date:** 2026-07-25
**From:** PM
**To:** Cerebro
**Program:** OT-61 capability-confined native browser
**Current `origin/main`:** `62d743da8c1a8a4205ea6399df6d7010e11de123`
**Rejected candidate:** `d5e79fc7ce0f66ad8fb2e804b696b7c9a8c0d6e1`
**Existing fix-forward:** `Plans/FIX-FORWARD-2026-07-23-ot61-c1.1-aag-blockers-cerebro.md`
**Status:** **RESUMED FOR ONE CODER. Build and push now. No workflow-dispatch
token, merge, PR, or hardware action.**

## PM disposition

The PL has lifted the browser-lane hold for exactly one parallel coder while
the WiFi candidate awaits its final independent review. Cerebro owns this
single additional lane. Do not open, delegate, or coordinate another new
implementation lane from this authorization.

OT-61 is a LogansOS 1.0 ship gate. This is the bounded native Servo +
SpiderMonkey host-runtime ladder described by ADR-078; it does not add a
general POSIX or Linux personality.

## Fresh-branch requirement

Preserve the rejected branch and exact SHA. Do not rebase, amend, or
force-push it.

Create:

```text
codex/ot61-c1.1-aag-fix-forward-cerebro
```

from the exact current `origin/main`. The PM independently verified that all
nineteen main-side changes since the rejected candidate's merge-base are
governance documents only; there is no intervening production-code delta.

Replay the C1.1 code content from these commits, preserving stable patch IDs
where the current base permits:

```text
807b7293  C1.1 threads + futex implementation
375b1ac6  teardown Arc-leak and fault-kill repair
b1b969c5  formatting and clippy correction
```

Do not replay the stale `dd4eff9e` RETURN commit. Do not blindly replay the
`d5e79fc7` pitfall-document commit: carry its CP-30 substance into the final
accurate pitfall bank, with statuses matching the corrected exact candidate.

Then append the independently owned repair. A clean reconstruction or a
small number of coherent repair commits is acceptable; return the complete
old-to-new patch map and own-stat.

## Required repair

Read and execute the existing fix-forward in full. All six blockers are
mandatory:

1. token wake remains owned and non-panicking under a full ready ring;
2. successful created-thread stacks unmap/free exactly once before process
   exit, without touching the initial or a live stack;
3. ring-3 fault-kill drains siblings from every owning wait queue, including
   IPC and process-wait;
4. `BrowserRuntime` authority composes deterministically with `NetDial` and
   other root capabilities;
5. the TLS-free child bootstrap initializes bounds, reserved TCB/key state,
   and isolated per-thread errno before application entry;
6. delayed join uses the frozen per-thread done word through the futex
   wait/wake path, with no private join-token protocol.

Preserve:

- the frozen `0x44..0x4D` ABI;
- physical-address futex keys;
- absolute monotonic deadlines and wake-count semantics;
- `thread_get_info` bounds validity;
- single-joiner, detach, exact exit-code, and no-lost-wakeup behavior;
- capability confinement and no userspace self-mint;
- the default-off `browser-runtime` kill switch and OFF equality;
- CP-30 blocking-syscall ownership discipline;
- the merged SharedPixelBuffer shared-XOR-mutable prerequisite;
- native libneo capability semantics, with no exported POSIX personality.

Do not weaken the production contract, reduce capacities to make tests pass,
or replace a non-vacuous discriminator with a model-only assertion.

## Build-now gates

Before RETURN:

- run every focused discriminator required by the existing fix-forward;
- run the C1.1 host/KAT suite and both existing QEMU scenarios locally;
- add and run the new capacity, stack-reclamation, all-queue fault-kill,
  composed-capability, bootstrap, and futex-join scenarios;
- run format, workspace tests, blocking workspace/kernel clippy, SPDX, and
  one uninterrupted `scripts/worker-close-gate.sh`;
- prove representative `browser-runtime`-OFF artifact equality;
- push the fresh branch and return the exact automatic push-CI URL.

Push CI skips QEMU. Do not fire `workflow_dispatch`; the PM will independently
review the returned SHA and decide the exact-SHA four-shard token after the
current WiFi review/gate boundary permits it.

## RETURN

Return:

- branch, exact tip/parents/base, merge-base, remote parity, and clean
  worktree;
- replay map with stable patch IDs;
- exact own-stat and touched paths;
- blocker-to-repair and discriminator table;
- focused and complete test commands/counts;
- pitfall-bank updates;
- kill-switch/OFF equality evidence;
- close-gate result;
- exact-SHA automatic push-CI URL.

No workflow dispatch, hardware boot, credential-filled build, merge, PR, or
new coder delegation is authorized.
