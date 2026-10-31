# Dispatch - Phase 3 BUILD-4 Stability Close With Caveat

Date: 2026-05-18
Agent: Cerebro
Status: SUPERSEDED

**SUPERSEDED:** Do not execute this dispatch. It is superseded by
`Plans/DISPATCH-2026-05-18-phase-3-build-4-stale-fsync-liveness-root-cause-cerebro.md`.
BUILD-4 does not close while the stale-fsync double-timeout tail remains known.

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-phase-3-build-4-stability-fix-6-consumption.md`
- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-6-BLOCKER-2026-05-17.md`
- `Plans/RESEARCH-2026-05-16-phase-3-build-4-virtio-kvm-timing-prometheus.md`

Substrate:

```text
d4e2aa4 plans: authorize Phase 3 BUILD-4 smaller NeoFS test disk
8f1a4b2 plans: authorize Phase 3 BUILD-4 targeted timeout policy
2377d4d plans: authorize Phase 3 BUILD-4 storage backend matrix
dbe59a3 plans: authorize Phase 3 BUILD-4 stability fix 4
014e5d7 plans: authorize Phase 3 BUILD-4 stability fix 3
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
```

## 1. PM Decision

The stability lane is no longer chasing statistical zero for
`phase3_neofs_fsync_filecap_stale_regression`.

Fixes 9, 14, and 15 each improved or exposed a different part of the same
tail, but none eliminated the final double-timeout condition. The residual
tail is accepted as a documented substrate caveat for BUILD-4 close. The
accepted result is not "stale-fsync is fixed." The accepted result is:

```text
BUILD-4 stability substrate is instrumented, narrowed, and diagnosable.
Residual I/O-heavy NeoFS double-timeout remains a known host/QEMU/serialized-I/O caveat.
Storage architecture follow-up is required if the project later needs this tail truly closed.
```

## 2. Scope

Package and commit only the accepted diagnostic/stability substrate:

- Fix 8: per-attempt serial logs.
- Fix 9: narrow virtio-only `wake_thread_token_front`.
- Fix 11: `NEO_QEMU_STORAGE_MODE` diagnostic env gate, default writeback.
- Fix 12: `NEO_TEST_RUN_ID` suffix for repeated-loop log preservation.
- Fix 14: targeted 360s timeout for the I/O-heavy NeoFS allowlist.
- Fix 15: targeted 72 MiB NeoFS image for the same I/O-heavy allowlist.
- Stale-fsync `NEO_TEST/1 event=PROGRESS` frames.

Do not commit or reintroduce Fix 6/AP idle halt.

Do not implement pipelined fsync, multi-in-flight virtio, storage scheduler
changes, sys_mount, sys_umount, FAT32, ext4, BUILD-4B, BUILD-5, timeout bumps,
extra retries, scenario removal, or scenario ignore markers.

## 3. Required Code Review Checkpoints

Before committing, explicitly confirm in the close artifact:

- `wake_thread_token_front` is used only by the virtio-blk completion path.
- Existing general wake-token paths still use fair `push_back` semantics.
- `NEO_QEMU_STORAGE_MODE` default is unchanged from historical writeback mode.
- `NEO_TEST_RUN_ID` is filename-only and sanitized; it must not alter scenario behavior.
- Per-attempt logs are the load-bearing evidence; canonical serial logs are compatibility copies.
- The 360s timeout and 72 MiB disk policies apply only to the named I/O-heavy NeoFS allowlist.
- Non-allowlisted NeoFS scenarios keep the 128 MiB default image and 180s timeout.
- Stale-fsync PROGRESS frames are diagnostic only and do not change test semantics.

## 4. Verification Gates

Run these gates before committing:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
git diff --check
```

Run targeted scenario checks:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_sched_wake_token_abandon
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=close-stale-single neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=close-mount-default neo cargo neo-test phase3_neofs_mount_readonly_root
```

Expected policy observations:

- `ipc_heap_kill_smoke` keeps the 180s timeout.
- `phase3_sched_wake_token_abandon` keeps the 180s timeout.
- `phase3_neofs_fsync_filecap_stale_regression` uses 72 MiB and 360s.
- `phase3_neofs_mount_readonly_root` uses default 128 MiB and 180s.

Then run one final full-suite attempt:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Interpretation:

- If the wrapper exits 0, record clean close evidence.
- If the wrapper exits non-zero solely because `phase3_neofs_fsync_filecap_stale_regression`
  double-times out with preserved attempt logs and no `event=FAIL` frame, record it
  as the accepted residual caveat, not as a new blocker.
- Any nonzero result in any other scenario is a blocker. Stop, preserve logs, and file
  a blocker.
- Any `event=FAIL` frame in stale-fsync is a blocker. Accepted residual covers
  double-timeout only, not semantic failure.

Do not rerun the full suite repeatedly to hunt for a green sample. One final
attempt is evidence; repeated attempts would hide the operating envelope.

## 5. Close Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-STABILITY-CLOSE-WITH-CAVEAT-2026-05-18.md
```

The artifact must include:

- exact HEAD and `git log --oneline -8`;
- `git diff --stat` for the committed stability substrate;
- build-gate output and warning count;
- targeted scenario output with literal PASS lines or timeout evidence;
- final full-suite wrapper result;
- stale-fsync residual table summarizing Fix 9, Fix 14, and Fix 15;
- evidence paths for preserved attempt logs;
- the code-review checkpoints from section 3;
- explicit statement that residual double-timeout is accepted only for the
  known stale-fsync host/QEMU/serialized-I/O tail;
- explicit statement that the storage architecture lane remains open for
  pipelined fsync / multi-in-flight design;
- explicit statement that BUILD-4B remains blocked until PM consumes this
  close artifact.

Required caveat language:

```text
The stability lane does not claim to eliminate the I/O-heavy NeoFS timing tail.
It commits a diagnostic substrate and narrows the residual to a known
double-timeout class in phase3_neofs_fsync_filecap_stale_regression. Passing
attempts continue to verify ESTALE, durability state preservation, and refreshed
capability fsync drain. Semantic FAIL frames remain blockers.
```

## 6. Commit Policy

Commit the accepted substrate in logical commits. Suggested grouping:

1. `test: preserve neo-test attempt serial logs`
2. `sched: prioritize serialized virtio block completions`
3. `test: add NeoFS I/O-heavy timing policy`
4. `docs: close BUILD-4 stability lane with residual caveat`

If the final grouping differs, explain why in the close artifact.

Do not amend existing commits unless explicitly authorized. Do not push. Do not
merge. Do not start BUILD-4B.

## 7. Storage Architecture Follow-Up

If the close artifact is accepted, the next non-BUILD-4B planning lane should be
a design-only storage architecture dispatch for pipelined fsync drain. That lane
must answer crash-recovery ordering, flush semantics, concurrent writeback/GC
interaction, and rollback tests before any code changes.
