# Dispatch — CI SMP Race Triage

Date: 2026-05-16
Agent: Cerebro
Status: AUTHORIZED INVESTIGATION
Authority: PM verbal — "New investigation lane. Diagnose main-branch nondeterministic SMP CI failures."

Substrate:

```text
bb47d0a plans: authorize Phase 3 BUILD-4A readdir
```

## 1. Mission

Determine why docs-only commits are producing non-deterministic QEMU integration-test
failures under `NEO_SMP=4`. Three consecutive CI runs (2026-05-16) each failed on a
different scenario. No kernel code changed across these commits. The runner image
changed on 2026-05-13 from `20260413.86.1` to `20260513.135.3`, one day before the
failures began.

Pre-investigation triage from PM has already identified candidate mechanisms (see
§4). This dispatch authorizes Cerebro to gather evidence, run local reproduction,
classify failures, and produce a findings report.

## 2. Failing Runs

| Run ID     | Commit                                     | Failing scenario                        | Attempt 1         | Attempt 2           |
|------------|--------------------------------------------|-----------------------------------------|-------------------|---------------------|
| 25963226678| docs: update Phase 3 after BUILD-3 closure | `ipc_heap_kill_smoke`                   | exit status 35    | exit status 35      |
| 25963424647| plans: record Phase 3 BUILD-4 prereq audit | `cap_revoke_local`                      | exit status 35    | exit status 35      |
| 25963469513| plans: authorize Phase 3 BUILD-4A readdir  | `phase3_sched_wake_token_abandon`       | exit status 35    | **QEMU timeout 180s** |
| local only | BUILD-4A working tree (full --no-meta)     | `phase3_neofs_fsync_filecap_stale_regression` | QEMU timeout 180s | **QEMU timeout 180s** |

Note: `phase3_neofs_fsync_filecap_stale_regression` does not appear in CI failures because CI
fails at an earlier scenario before reaching it. It is only observable locally once the earlier
scenarios pass (different host timing on developer machine vs. CI runner).

Exit status 35 = `qemu_debug_exit_status(TEST_EXIT_FAILURE)` = `(0x11 << 1) | 1`.
The kernel reached an assertion, reported failure via the debug-exit port, and
exited cleanly. It is NOT a crash or panic — it is an explicit test failure.

The QEMU timeout on the second `phase3_sched_wake_token_abandon` attempt is an
escalation: the kernel hung completely on retry.

Cross-run evidence: `ipc_heap_kill_smoke` and `cap_revoke_local` PASS in the run
where `phase3_sched_wake_token_abandon` fails. Non-deterministic — timing-dependent.

## 3. Environment

| Property       | Last success (2026-05-12)  | First failure (2026-05-16) |
|----------------|----------------------------|----------------------------|
| Runner image   | ubuntu-24.04 v20260413.86.1| ubuntu-24.04 v20260513.135.3|
| QEMU (container)| 7.2.22 (Debian 1:7.2+dfsg-7+deb12u18) | same |
| NEO_SMP        | 4                          | 4                          |
| Kernel code    | 60319e3 (last code commit) | same                       |

The container ships its own QEMU binary (7.2.22). The host Linux kernel on the CI
runner changed between the two image versions. A newer host kernel can alter KVM VM
exit frequency, CPU pinning behavior, and preemption timing for QEMU vCPUs, which
directly affects the guest SMP race windows.

## 4. Candidate Mechanisms (pre-investigation)

### M1 — `kill_thread_by_pid` is CPU-local

`scheduler::kill_thread_by_pid` (scheduler.rs:1024) only searches the CALLING CPU's
ready queue:

```rust
pub fn kill_thread_by_pid(pid: ProcessId) -> bool {
    without_interrupts(|| {
        let sched = current_scheduler();  // CURRENT CPU ONLY
        let mut s = sched.inner.lock();
        let idx = s.ready.iter().position(|t| t.pid == pid);
```

If the target thread is on a DIFFERENT CPU's ready queue (possible when work hasn't
been scheduled yet or was moved by cross-CPU interactions), `kill_thread_by_pid`
returns false. `process::terminate()` falls through to the channel-walk and
wake-token-walk fallbacks, which only find BLOCKED threads, not READY threads on
remote CPUs. The fallback sequence:

1. Channel handle walk — finds IPC-blocked threads only
2. `zombify_parked_thread_by_pid` — finds wake-token parked threads only
3. `debug_assert!(found_blocked, ...)` — FIRES if thread was Ready on another CPU

A debug_assert panic routes through the test-harness panic handler →
`debug_exit(EXIT_FAILURE)` → exit status 35.

### M2 — `ipc_heap_kill_smoke` is documented as single-CPU sensitive

Source comment (neo-init/src/main.rs:205–209):

```
// Single-CPU rendezvous: the worker transitions directly from receiving
// this message into sys_recv(chan_main). There must be no worker-side
// diagnostic syscall in between; a timer tick in that window would make
// the blocked-kill assertion scheduler-timing-sensitive.
```

The test sends "go-block" synchronously (IPC rendezvous), then immediately calls
`sys_kill`. With SMP=4, the worker can be on a different CPU. The rendezvous
completing on CPU 0 does not guarantee the worker on CPU 1 has already called
`sys_recv(chan_main)`. If `sys_kill` fires in that window, the worker is Ready
(not Blocked), and M1 applies.

### M3 — `phase3_sched_wake_token_abandon` deadlock escalation

The 180s QEMU timeout on the second attempt indicates a kernel hang, not just an
assertion failure. Candidates:

(a) `wait_for_primary_token()` polls 100,000 × (yield_now + reap). If the worker
    never successfully installs its wake token (e.g., because
    `block_current_with_token_and_guard`'s `pop_front().expect()` panics due to
    an empty ready queue on the worker's CPU), the token is never set and the loop
    runs to exhaustion. The timeout suggests the kernel hit this and then hung in
    cleanup.

(b) The zombie-reap loop at the end of the test (another 100,000 × yield+reap)
    could hang if `terminate()` succeeded in zombifying the thread but the zombie
    counter never decrements (reaper bug or reference count cycle).

(c) `block_current_with_token_and_guard` panics with "ready queue empty", which
    under some panic-handler paths could cause a non-terminating loop.

### M4 — `cap_revoke_local` failure mechanism (less clear)

`cap_revoke_local` is purely sequential, single-process, no spawning. The most
likely exit code from the userland function is `return 4` (sys_send on revoked
handle did NOT return EBADF). Possible causes:

(a) A background kthread (e.g., NeoFS writeback worker if it was started) interferes
    with handle table state — but `cap_revoke_local` does not mount NeoFS, so the
    writeback worker is not started for this scenario.

(b) The AP schedulers have some idle-loop behavior that contaminates shared state
    between the userland syscall path and the handle table.

(c) An earlier part of the boot sequence (before the test scenario runs) leaves
    some state inconsistent under SMP that affects handle revocation lookup.

(d) The fast consistent failure in ~1.7 seconds across both attempts suggests this
    may be a more deterministic race than the others.

This mechanism requires serial log evidence to classify precisely.

## 5. First-Pass Investigation Protocol

### 5.1 Serial log capture (P0 — currently missing)

The CI job does NOT upload serial logs as artifacts. The `NEO_TEST/1 event=FAIL
reason=...` frame in the serial output would immediately identify which assertion
failed in each scenario. This is the highest-priority gap.

Action: Add artifact upload to `.github/workflows/ci.yml` for the serial log
path `/workspace/target/neo-test/*.serial.log` on failure. This unblocks all
future triage.

### 5.2 Local reproduction

From clean main (`bb47d0a`), in the dev container with `NEO_SMP=4`:

```bash
# Targeted loop runs — each scenario, 50 iterations
for i in $(seq 1 50); do
  docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke && echo "PASS $i" || echo "FAIL $i"
done

for i in $(seq 1 50); do
  docker compose run --rm -e NEO_SMP=4 neo cargo neo-test cap_revoke_local && echo "PASS $i" || echo "FAIL $i"
done

for i in $(seq 1 50); do
  docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_sched_wake_token_abandon && echo "PASS $i" || echo "FAIL $i"
done
```

Then NEO_SMP=1 control runs (20 iterations each) to confirm SMP sensitivity:

```bash
for scenario in ipc_heap_kill_smoke cap_revoke_local phase3_sched_wake_token_abandon; do
  for i in $(seq 1 20); do
    docker compose run --rm -e NEO_SMP=1 neo cargo neo-test $scenario && echo "PASS $i" || echo "FAIL $i"
  done
done
```

### 5.3 Classification gate

For each failure, classify as exactly one of:
- `ASSERT_35`: exit status 35, assertion failure (kernel-reported via debug-exit port)
- `TIMEOUT`: QEMU timeout (kernel hung / deadlock)
- `PANIC_35`: exit status 35 from panic handler (not test assertion)
- `INFRA`: docker/build failure unrelated to kernel

The serial log (once uploaded) tells you which path: `ASSERT_35` shows
`NEO_TEST/1 event=FAIL reason=...` frames; `PANIC_35` shows a different
serial pattern.

### 5.4 Mechanism confirmation

If M1 is the cause for `ipc_heap_kill_smoke` and `phase3_sched_wake_token_abandon`:
- Serial log will show `reason=neo_init_exit_nonzero code=5` (kill returned nonzero)
  for ipc_heap_kill_smoke, or a panic frame from debug_assert in terminate()
- Serial log will show `reason=wake_token_abandon_not_zombified` or a panic frame
  for phase3_sched_wake_token_abandon

If M2 is confirmed: NEO_SMP=1 runs will PASS; NEO_SMP=4 runs will fail at some
non-trivial rate.

If M3 (deadlock) is confirmed: a 180s timeout will sometimes reproduce under
NEO_SMP=4 for `phase3_sched_wake_token_abandon`.

### M5 — NeoFS writeback kthread busy-spin under `writeback_hold = true`

`phase3_neofs_fsync_filecap_stale_regression` sets `writeback_hold = true` (test_harness.rs:6469)
and **never releases it**. The NeoFS writeback kthread in `spawn_writeback_worker` (cache.rs:295)
loops: `drain_worker_cycle() → no-op (hold set) → yield_now() → no-op (idle AP) → repeat`.

Under SMP=4 with one otherwise-idle AP, this is a continuous 100% AP busy-spin. Under the new
host kernel's KVM scheduling, this reduces the host CPU time available for virtio-blk interrupt
delivery, degrading I/O throughput. The 128 MB NeoFS disk (`capacity_sectors=262144`) requires
hundreds of sector reads during mount validation. Under degraded throughput, total test time
exceeds 180 seconds.

Mechanism confirmed from serial log: I/O completes with `status=0` and `wake=Woken` but the
stream never terminates. No `NEO_TEST/1 event=PASS` frame appears.

This is **not** a NeoFS/fsync logic bug and is **not** BUILD-4A-induced. It is a pre-existing
timing sensitivity in the writeback kthread's idle behavior, first exposed locally when the
host KVM timing changed (same root cause as CI failures).

## 6. No-Code-Change Scope

This dispatch authorizes investigation only. No production code changes.
Findings report is the sole deliverable.

If reproduction is confirmed and root cause is identified, a separate fix
dispatch will be opened. Expected fixes:

- `kill_thread_by_pid`: extend to scan all CPUs' ready queues (or implement
  a cross-CPU kill request via IPI)
- `ipc_heap_kill_smoke`: ensure worker is deterministically Blocked before
  sys_kill is called (e.g., add a synchronization barrier after go-block)
- CI: add `--upload-artifact` for serial logs on failure

## 7. Required Output

```text
E:\AI Automation\AltimitNeo\Plans\CI-SMP-RACE-TRIAGE-FINDINGS-2026-05-16.md
```

Minimum required content:
- Environment evidence with QEMU version, runner delta, local reproduction status
- Classification of each failing scenario (ASSERT_35 / TIMEOUT / PANIC_35 / INFRA)
- Identified assertions per scenario (from serial log or code analysis)
- SMP-sensitivity determination (NEO_SMP=1 vs NEO_SMP=4)
- Confirmed or refuted mechanism per scenario
- Recommended fix per scenario
- Stop conditions for the fix dispatch
