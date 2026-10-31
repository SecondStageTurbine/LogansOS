# Dispatch - Phase 3 BUILD-4 Stability Fix 4B: Writeback Backend Matrix

Date: 2026-05-17
Agent: Cerebro
Status: AUTHORIZED EXPERIMENT
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-4-BLOCKER-2026-05-16.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-4-qemu-storage-mode-cerebro.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

Substrate:

```text
dbe59a3 plans: authorize Phase 3 BUILD-4 stability fix 4
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
```

Active uncommitted substrate remains:

- Fix 8: per-attempt serial logs;
- Fix 9: narrow virtio-only `wake_thread_token_front`;
- Fix 11: `NEO_QEMU_STORAGE_MODE` env gate;
- Fix 12: `NEO_TEST_RUN_ID` suffix;
- stale-fsync progress frames.

## 1. PM Disposition

`direct-native` is rejected.

Observed:

- `direct-native`: 18 PASS / 1 FAIL / 2 retry-pass by run 19;
- final failure rate worse than Fix 9 writeback baseline;
- direct I/O did not localize the tail and likely removed host page-cache
  buffering that was helping.

Do not run direct-native regressions. Do not make direct-native default.

The remaining already-implemented low-blast-radius experiment is to compare the
writeback AIO backend variants:

1. `writeback-io-uring`
2. `writeback-threads`

After these two modes, stop. If neither beats the Fix 9 baseline, PM will choose
between targeted timeout policy, smaller valid NeoFS disk, or a real storage
architecture lane. Do not implement those here.

## 2. Scope

### Experiment A - `writeback-io-uring`

Run:

```text
NEO_QEMU_STORAGE_MODE=writeback-io-uring
NEO_SMP=4
```

Required:

- confirm QEMU accepts the exact drive string;
- capture any QEMU stderr;
- run `phase3_virtio_blk_flush_command_smoke`;
- run `phase3_neofs_fsync_issues_flush`;
- run stale-fsync loop up to 50, stopping on first final failure.

Each stale-fsync invocation must set unique `NEO_TEST_RUN_ID`, for example:

```text
NEO_TEST_RUN_ID=uring-stale-01
```

### Experiment B - `writeback-threads`

Run only if `writeback-io-uring` is rejected, fails, or produces retry/failure
rates not materially better than Fix 9 baseline.

Required shape mirrors Experiment A:

```text
NEO_QEMU_STORAGE_MODE=writeback-threads
NEO_SMP=4
```

Use unique `NEO_TEST_RUN_ID`, for example:

```text
NEO_TEST_RUN_ID=threads-stale-01
```

## 3. Baselines And Decision Rules

Baseline A - Fix 9 writeback:

```text
41/42 PASS, 1 final double-timeout, 5 retry-pass, retry-pass rate 12.2%.
```

Baseline B - direct-native:

```text
18/19 PASS, 1 final double-timeout, 2 retry-pass, rejected.
```

Candidate mode acceptance threshold:

- 50/50 stale-fsync final PASS;
- materially lower retry-pass rate than 12.2%;
- no fsync/flush semantic regression;
- no QEMU stderr warning indicating the requested backend was ignored or
  downgraded.

If a mode reaches 30/30 with zero retry-pass, report an interim PM check-in
before extending to 50. If it reaches 30/30 with retry-pass but no final
failures, continue to 50 but preserve retry evidence.

If both writeback backend variants fail to beat the baseline, file a blocker.
Recommended next PM choice should be one of:

- targeted timeout policy for I/O-heavy NeoFS scenarios;
- smaller valid NeoFS test disk after proving mkfs accepts it;
- storage architecture design lane for pipelined drain / multi-in-flight
  virtio;
- accept retry-pass with explicit caveat, only if PM decides the residual tail
  is environmental enough.

## 4. Verification

For each attempted mode, capture:

- exact QEMU drive string from xtask output;
- QEMU stderr, including empty/no-warning statement;
- `phase3_virtio_blk_flush_command_smoke` PASS/FAIL;
- `phase3_neofs_fsync_issues_flush` PASS/FAIL;
- stale-fsync loop table;
- every retry/failure attempt log path;
- last `event=PROGRESS` frame for any timed-out attempt.

Do not run the final full suite unless a candidate reaches the acceptance
threshold and PM explicitly authorizes full-suite verification.

## 5. Stop Conditions

Stop and report if:

- QEMU rejects a backend mode;
- QEMU silently ignores or downgrades the requested backend;
- stale-fsync gets a final double-timeout;
- a mode reaches a retry-pass rate at or above the Fix 9 baseline by run 30;
- flush-command or fsync-issues-flush semantics regress;
- any per-attempt log is missing despite `NEO_TEST_RUN_ID`.

## 6. Artifact

File:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-4B-BACKEND-MATRIX-2026-05-17.md
```

Include:

- mode matrix;
- exact command lines / env;
- drive strings;
- loop tables;
- retry/failure evidence;
- recommendation.

No commit unless PM asks. No push. No BUILD-4B.
