# PM Disposition - Stale-Fsync Substrate Audit Consumption

Date: 2026-05-18
Authority consumed:

- `Plans/AUDIT-2026-05-18-stale-fsync-liveness-substrate-survey.md`
- `Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`

## Disposition

The read-only substrate audit is accepted.

The audit correctly narrows the failure window to:

```text
mount_done emitted
seed_fsync_done not emitted
stall inside neofs_3e_fsync_or_fail(seed)
```

That places the timeout inside the first clean fsync drain after a tiny seed
write, not inside the stale-cap correctness logic. The next authorized step is
narrow liveness instrumentation, not a storage redesign and not more test
harness tuning.

## Audit Correction

One important correction before implementation:

A diagnostic frame emitted only before `write_fs_block()` cannot classify an
in-flight stall. If the system stops inside the blocking write path, the last
frame would say "about to write" and might still show no in-flight request.

Therefore the instrumentation must bracket blocking operations:

- emit before the block write is submitted;
- record virtio submission counters at submission time;
- emit after the block write returns;
- do the same for device flush.

Without submit/complete bracketing, Case B/C/D/F can still collapse into an
ambiguous "last frame before write" result.

## Decisions On Open Audit Questions

1. Feature gate: use the existing `neo-test` compile gate plus a runtime
   stale-fsync diagnostic gate. Do not add a new Cargo feature unless the
   existing test feature cannot cleanly carry this.

2. Frame rate: emit per blocking operation boundary, not periodic spam. Required
   frames are `FSYNC_DRAIN_ENTER`, `FSYNC_WRITE_BEGIN`, `FSYNC_WRITE_DONE`,
   `FSYNC_FLUSH_BEGIN`, `FSYNC_FLUSH_DONE`, `FSYNC_DRAIN_YIELD`, and
   `FSYNC_DRAIN_EXIT`. If a single fsync would emit more than 64 diagnostic
   frames, switch to a capped summary and state that explicitly in the findings.

3. Timeout dump: add `NEO_QEMU_TIMEOUT_DUMP=1` as the opt-in. It should do
   nothing unless `NEO_QEMU_MONITOR_PORT` is also set. Write monitor output to a
   sibling `.timeout-dump.log` file, not into the serial log.

4. Parallel loops: do not run instrumented and uninstrumented loops in parallel
   on the same host. Host contention is part of the suspected failure surface.
   Parallel runs would contaminate the measurement. Run sequential A/B samples
   instead.

## Required Instrumentation Shape

Add only the smallest state needed to classify the A-H cases:

- virtio monotonic submission counter;
- last submitted request type and block/LBA if available;
- last submitted token CPU/slot;
- existing completion/wake counters reused unchanged;
- NeoFS drain/write/flush begin/done frames;
- xtask timeout monitor dump gated by `NEO_QEMU_TIMEOUT_DUMP=1`.

Do not add new kthreads, new scheduler policy, new retries, new timeouts, or
new storage architecture behavior in the instrumentation pass.

## Authorization

Cerebro is authorized to implement the diagnostic instrumentation as a WIP
classification pass.

No mechanism fix is authorized until the first failing or retrying diagnostic
sample is classified against the dispatch's A-H table.

No commit is authorized for instrumentation-only work unless PM consumes the
findings and decides the instrumentation should remain as durable diagnostic
substrate.
