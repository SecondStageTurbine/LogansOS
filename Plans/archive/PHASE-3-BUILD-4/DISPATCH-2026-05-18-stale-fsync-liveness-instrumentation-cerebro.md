# Dispatch - Stale-Fsync Liveness Instrumentation Pass

Date: 2026-05-18
Agent: Cerebro
Status: AUTHORIZED WIP INSTRUMENTATION / NO FIX YET

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-stale-fsync-substrate-audit-consumption.md`
- `Plans/AUDIT-2026-05-18-stale-fsync-liveness-substrate-survey.md`
- `Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`

## 1. Purpose

Classify the stale-fsync timeout before fixing anything.

The target is still only:

```text
NEO_SMP=4 cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

The working question:

```text
During seed_fsync, does the kernel stop at virtio submit/completion, wake-token
handoff, scheduler run, NeoFS drain state, or host/QEMU execution?
```

## 2. Instrumentation Scope

Implement WIP instrumentation only. No mechanism fix yet.

### 2.1 Virtio state

Add `neo-test`-gated debug counters/accessors for:

- monotonic submission count;
- last submitted request type;
- last submitted block/LBA if available at that layer;
- last submitted token CPU/slot;
- current in-flight request type if available.

Reuse existing counters/accessors for:

- completions;
- flush commands;
- flush completions;
- in-flight DMA buffers;
- waiting-thread status;
- last wake result;
- last completed request type;
- completion CPU/token CPU/token slot.

### 2.2 NeoFS fsync drain frames

Add runtime-gated `NEO_DIAG/1` frames around the blocking fsync drain
operations:

```text
NEO_DIAG/1 event=FSYNC_DRAIN_ENTER ...
NEO_DIAG/1 event=FSYNC_WRITE_BEGIN ...
NEO_DIAG/1 event=FSYNC_WRITE_DONE ...
NEO_DIAG/1 event=FSYNC_FLUSH_BEGIN ...
NEO_DIAG/1 event=FSYNC_FLUSH_DONE ...
NEO_DIAG/1 event=FSYNC_DRAIN_YIELD ...
NEO_DIAG/1 event=FSYNC_DRAIN_EXIT ...
```

The key correction from PM: do not rely on a single pre-write frame. The
blocking write and flush paths must be bracketed so a timeout has a meaningful
last-observed state.

Each frame should include only compact classification fields:

- scenario or trace name;
- drain sequence number;
- block/LBA if applicable;
- role if applicable;
- dirty block count;
- drained block count;
- flush-required;
- flush count;
- virtio submissions;
- virtio completions;
- in-flight DMA buffers;
- last wake result.

### 2.3 Runtime gate

Use the existing `neo-test` compile feature and a runtime stale-fsync diagnostic
gate. Do not emit these frames for every NeoFS test by default.

Acceptable shape:

- `debug_set_fsync_liveness_trace(true)` before seed fsync in the stale-fsync
  scenario;
- `debug_set_fsync_liveness_trace(false)` after the diagnostic region returns;
- if the kernel times out before disabling the gate, the next QEMU boot starts
  from clean state, so no persistent cleanup problem exists.

### 2.4 Xtask timeout dump

Add opt-in timeout dump support:

```text
NEO_QEMU_TIMEOUT_DUMP=1
NEO_QEMU_MONITOR_PORT=<port>
```

On timeout, before killing QEMU:

- connect to the monitor with short connect/read timeouts;
- issue a small state query such as `info status`, `info cpus`/`info threads`,
  and register state if supported by the monitor mode in use;
- write output to a sibling `.timeout-dump.log`;
- then kill QEMU as before.

If monitor capture fails, record the failure in the dump file or xtask stderr.
The failure itself is classification evidence.

## 3. Measurement Plan

Do not run parallel instrumented and uninstrumented loops on the same host.

Run in this order:

1. Build gates:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
```

2. Single instrumented smoke:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=diag-smoke -e NEO_QEMU_TIMEOUT_DUMP=1 -e NEO_QEMU_MONITOR_PORT=45555 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

3. Sequential diagnostic loop:

```text
Run up to 30 instrumented stale-fsync attempts.
Stop early on the first final double-timeout.
Stop early after 3 retry-pass attempts if their attempt-1 logs classify cleanly.
```

4. If instrumentation masks the failure:

```text
Record 30/30 or equivalent as observer-effect evidence.
Do not call that a fix.
Return to PM with the findings.
```

## 4. Classification Requirement

Every timeout/retry-pass attempt must be classified against the A-H table in
`Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`.

Especially distinguish:

- submitted not completed;
- completed not woken;
- woken not scheduled;
- scheduled but no NeoFS progress;
- steady progress but too slow;
- QEMU/host not responding to monitor capture.

## 5. Stop Conditions

Stop and file findings if:

- a final double-timeout is captured and classified;
- three retry-pass attempt-1 logs classify the same mechanism;
- any `event=FAIL` appears;
- timeout dump support cannot be made reliable with the existing QEMU monitor;
- instrumentation prevents reproduction across 30 attempts.

Do not proceed from classification to mechanism fix without PM consumption
unless the fix is a one-line obvious instrumentation bug.

## 6. Required Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-STALE-FSYNC-LIVENESS-INSTRUMENTATION-FINDINGS-2026-05-18.md
```

Include:

- exact worktree and HEAD;
- instrumentation diff summary;
- build-gate results;
- attempt table;
- last diagnostic frame for every timeout/retry-pass attempt;
- timeout dump path and result, if any;
- A-H classification;
- whether instrumentation appears to perturb the tail;
- recommended next fix lane.

## 7. Commit Policy

No commit for instrumentation-only work until PM consumes the findings.

No push. No merge. No BUILD-4B. No storage redesign.
