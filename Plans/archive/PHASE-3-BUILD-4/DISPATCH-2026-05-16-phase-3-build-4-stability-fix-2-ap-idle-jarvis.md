# Dispatch - Phase 3 BUILD-4 Stability Fix 2: AP Idle Halt And Residual I/O Timeout

Date: 2026-05-16
Agent: Jarvis
Status: AUTHORIZED IMPLEMENTATION
Authority:

- `Plans/PHASE-3-BUILD-4-STABILITY-FIX-BLOCKER-2026-05-16.md`
- `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-jarvis.md`
- `Plans/CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md`

Substrate:

```text
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
```

## 1. PM Disposition

The first stability lane is partially accepted but not closable.

Accepted:

- CI serial artifact upload is the right infrastructure fix.
- Cross-CPU `kill_thread_by_pid` fixed the `ipc_heap_kill_smoke` surface:
  30/30 PASS, 0 retry-pass.
- NeoFS writeback worker parking was necessary and should stay.

Still blocking:

- `phase3_sched_wake_token_abandon`: 30/30 final PASS but 5 retry-pass.
- `phase3_neofs_fsync_filecap_stale_regression`: stopped at 25/30 with
  23 PASS, 4 retry-pass, 2 final double-timeout failures.

The stale-fsync timeout serial tails show successful virtio completions with
`wake=Woken` and no `NEO_TEST` terminal frame. That means the kernel is not
obviously dead; progress is too slow or scheduling after I/O wake is delayed.

BUILD-4B remains blocked.

## 2. New Primary Hypothesis

The first stability fix removed one busy spinner: the NeoFS writeback worker.
It did not remove the AP bootstrap idle spin:

```rust
loop {
    yield_now();
    core::hint::spin_loop();
}
```

Under `NEO_SMP=4`, otherwise-idle APs can still consume host vCPU time while
the BSP is doing hundreds of virtio block operations. On the updated CI host
kernel/KVM timing, that can be enough to turn a valid but I/O-heavy scenario
into a 180-second wall-clock timeout.

This also explains why the failure appears as repeated successful completions:
the storage path is moving, but the guest is wasting host scheduling budget on
idle vCPUs instead of letting QEMU and the BSP make timely progress.

## 3. Scope

### Fix 6 - Race-safe AP idle halt

Replace the AP bootstrap busy-spin idle path with a halt-based idle path when
the AP has no ready work.

Required semantics:

- APs must not busy-spin when their local ready queue is empty.
- APs must still schedule ready work promptly when a remote enqueue sends a
  reschedule IPI.
- The idle halt must be race-safe against a wake arriving between the ready
  queue check and `hlt`.
- Do not hold `PerCpuScheduler.inner` across `hlt`.
- Do not use a raw `hlt` with interrupts enabled and a lost-wake window.

Preferred shape:

1. Add a scheduler helper for AP idle, for example
   `halt_current_cpu_if_idle()`.
2. In that helper:
   - disable local interrupts;
   - lock the current CPU scheduler and check whether `ready` is empty;
   - drop the scheduler lock;
   - if ready is non-empty, re-enable interrupts and return;
   - if ready is empty, execute an atomic enable-and-halt sequence
     (`sti; hlt`, e.g. `x86_64::instructions::interrupts::enable_and_hlt()`);
   - return with interrupts enabled.
3. Change `run_ap_bootstrap()` from `yield_now(); spin_loop();` to a loop that
   yields ready work and halts only when idle.

The `sti; hlt` shape is load-bearing: with interrupts disabled during the
ready-queue check, a remote IPI that arrives after the check remains pending
and wakes the CPU immediately after `sti`.

### Fix 7 - Same-CPU I/O wake is observation-only for this lane

Do not change `wake_thread_token` scheduling semantics in this commit unless
Fix 6 is proven insufficient and PM authorizes the follow-up.

Reason: changing same-CPU token wake behavior can affect storage, kthread join,
and scheduler wake-token tests at once. AP idle spin is the simpler proven
remaining busy-loop. Fix that first.

If Fix 6 does not clear the stale-fsync timeout, file a blocker with phase
markers that can distinguish:

- mount/free-map validation still too slow;
- seed write/fsync too slow;
- stale fsync gate stuck;
- refreshed fsync/drain stuck;
- same-CPU wake-to-run delay after virtio completion.

## 4. Required Instrumentation

Add lightweight progress frames to
`phase3_neofs_fsync_filecap_stale_regression` before rerunning the targeted
loop. These frames may remain in the test scenario if they are stable and
useful.

Required frames:

```text
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=mount_done ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=seed_fsync_done ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=hold_set ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=mutation_dirty ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=stale_fsync_done ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=refreshed_fsync_begin ...
NEO_TEST/1 event=PROGRESS name=phase3_neofs_fsync_filecap_stale_regression stage=refreshed_fsync_done ...
```

Include cache stats where available:

- `dirty_blocks`;
- `drained_blocks`;
- `flush_required`;
- `cache_flushes`;
- virtio flush command/completion counters if cheap.

These frames are not a replacement for the final PASS frame. They exist so a
future timeout has a phase boundary instead of only a tail full of completion
lines.

## 5. Verification

Run from the dev container.

### Build gates

```text
cargo k
cargo k --no-default-features
```

Both must pass. Warning count must not increase over the current 72-warning
baseline unless the delta is explicitly justified.

### Targeted loops

Run under `NEO_SMP=4`:

```text
cargo neo-test phase3_neofs_fsync_filecap_stale_regression
cargo neo-test phase3_sched_wake_token_abandon
cargo neo-test ipc_heap_kill_smoke
```

Minimum counts:

- `phase3_neofs_fsync_filecap_stale_regression`: 50 runs.
- `phase3_sched_wake_token_abandon`: 30 runs.
- `ipc_heap_kill_smoke`: 20 runs.

Close target:

- zero final failures;
- zero retry-pass timeouts for `phase3_neofs_fsync_filecap_stale_regression`;
- materially reduced or zero retry-pass for `phase3_sched_wake_token_abandon`.

If stale-fsync produces any timeout, preserve the serial log and classify by
the last observed `event=PROGRESS` stage.

### Regression scenarios

Run once under `NEO_SMP=4`:

```text
cargo neo-test smp_per_cpu_scheduler_smoke
cargo neo-test smp_cross_cpu_migration_reap
cargo neo-test smp_cross_cpu_rendezvous_stress
cargo neo-test phase3_storage_completion_cross_cpu_wake
cargo neo-test phase3_sched_wake_token_round_trip
cargo neo-test phase3_sched_wake_token_stale_generation
cargo neo-test phase3_neofs_writeback_dirty_drain
cargo neo-test phase3_neofs_writeback_worker_stop_join
cargo neo-test phase3_neofs_fsync_coordinates_with_writeback_gc
cargo neo-test phase3_neofs_fsync_dirty_persists_after_crash
cargo neo-test phase3_neofs_fsync_issues_flush
cargo neo-test phase3_build1_reboot_persistence_smoke
```

### Final gate

Only after the targeted loops and regression scenarios pass:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

Required result: exit 0. Report any `PASS after retry` lines explicitly.

## 6. Stop Conditions

Stop and report before committing if:

- the AP idle implementation can miss a wake between ready-check and halt;
- an AP halts while its ready queue is non-empty;
- reschedule IPI delivery no longer wakes an idle AP;
- any scheduler test panics with `ready queue empty`;
- stale-fsync still double-times out after Fix 6;
- stale-fsync timeout occurs and no `event=PROGRESS` phase marker identifies
  the last reached stage;
- the proposed fix changes `wake_thread_token` same-CPU semantics without a
  separate PM authorization;
- any BUILD-4A readdir scenario regresses.

## 7. Close Or Blocker Artifact

If successful, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-2-CLOSE-EVIDENCE-2026-05-16.md
```

If blocked, file:

```text
Plans/PHASE-3-BUILD-4-STABILITY-FIX-2-BLOCKER-2026-05-16.md
```

The artifact must include:

- HEAD commit hash and `git log --oneline -8`;
- exact build-gate results;
- targeted-loop tables;
- retry-pass counts before and after Fix 6;
- literal stale-fsync PASS frame from at least one passing run;
- for any timeout, the last observed stale-fsync `event=PROGRESS` frame;
- final full-suite result if reached;
- explicit statement whether BUILD-4B remains blocked.

## 8. Commit Policy

Allowed implementation commits:

1. `sched: halt idle APs instead of spinning`
2. `tests: add stale fsync progress frames`
3. `docs: record Phase 3 BUILD-4 stability fix 2 evidence`

Do not push. Do not merge. Do not start BUILD-4B.
