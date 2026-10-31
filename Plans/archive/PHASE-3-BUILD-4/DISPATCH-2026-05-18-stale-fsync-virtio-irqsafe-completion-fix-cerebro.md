# Dispatch - Stale-Fsync Virtio Completion IRQ-Masking Fix

Date: 2026-05-18
Agent: Cerebro
Status: AUTHORIZED MECHANISM FIX

Authority:

- `Plans/PM-DISPOSITION-2026-05-18-stale-fsync-spin-localization-consumption.md`
- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md`

## 1. Purpose

Fix the root-caused stale-fsync SMP deadlock.

Root cause:

```text
completion worker takes VirtioBlkDevice::inner with IF=1
timer preempts worker while lock is held
submitter runs on same CPU inside without_interrupts
submitter spins on same lock with IF=0
timer cannot reschedule the holder
deadlock
```

This is not a storage timeout, not QEMU freeze, and not fsync architecture.
It is a local interrupt-state mismatch on the virtio device lock.

## 2. Authorized Code Change

Change only the completion-side critical section in:

```text
kernel/src/drivers/block/virtio_blk.rs
```

Required shape:

```rust
fn poll_completions(&self) {
    let completion = crate::arch::x86_64::without_interrupts(|| {
        let mut inner = self.inner.lock();
        inner.poll_used()
    });
    if let Some(completion) = completion {
        ...
    }
}
```

Add a concise comment explaining why this lock is IRQ-masked:

```text
submit_pending_and_wait may spin on this same lock with IF=0 while parking the
submitter; therefore the completion worker must not be timer-preempted while
holding it.
```

Do not change `VirtioBlkDevice::inner` to `IrqSafeMutex` in this lane.

## 3. Anti-Scope

Do not:

- redesign fsync;
- redesign scheduler wake tokens;
- change `block_current_with_token_and_guard`;
- add an irq-safe release-token lock abstraction;
- add retries or larger timeouts;
- shrink disk further;
- remove or weaken `phase3_neofs_fsync_filecap_stale_regression`;
- commit localization instrumentation before PM accepts it as durable.

## 4. Verification Before Simplify

Run with the current diagnostic WIP still present so the fix can be observed:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_RUN_ID=virtio-irqfix-smoke neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
```

Then run:

```text
50x NEO_SMP=4 phase3_neofs_fsync_filecap_stale_regression
```

Acceptance:

- 50/50 final PASS;
- 0 final double-timeouts;
- retry-pass count reported explicitly;
- if any retry-pass occurs, attempt logs must show no recurrence of the
  `VirtioBlkDevice::inner` IF=1-holder / IF=0-spinner signature;
- any semantic `event=FAIL` is a blocker.

Also run:

```text
30x NEO_SMP=4 phase3_sched_wake_token_abandon
20x NEO_SMP=4 ipc_heap_kill_smoke
```

These protect against accidentally regressing scheduler/wake-token behavior
while fixing virtio.

## 5. Simplify Gate

After the fix passes targeted loops, stop and simplify before committing.

Classify each active WIP component:

- **Keep:** the actual virtio IRQ-masking root fix.
- **Candidate keep:** per-attempt serial logs and timeout monitor dump if they
  remain low-risk and useful as durable diagnostic substrate.
- **Candidate remove:** stale-fsync liveness frames, spin snapshots, NMI timeout
  injection, failed timeout/disk/backend tuning, and any other investigation-only
  scaffolding.
- **Re-test required:** if a component is removed, re-run at least the targeted
  stale-fsync smoke before removing the next one. No phantom wins.

The simplify gate output must be included in the close artifact.

## 6. Final Verification After Simplify

After the final intended commit diff is reduced to shippable code:

```powershell
docker compose run --rm neo cargo build -p xtask
docker compose run --rm neo cargo k
docker compose run --rm neo cargo k --no-default-features
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test phase3_neofs_fsync_filecap_stale_regression
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test --no-meta
```

If the full suite exits nonzero, stop and file a blocker with the exact scenario
and serial evidence.

## 7. Required Artifact

Write:

```text
Plans/PHASE-3-BUILD-4-STALE-FSYNC-VIRTIO-IRQFIX-CLOSE-2026-05-18.md
```

Include:

- exact HEAD and worktree state;
- root cause summary;
- code diff summary;
- why `IrqSafeMutex` direct swap was rejected for this lane;
- targeted loop results;
- simplify gate table showing keep/remove decisions;
- final full-suite result;
- literal PASS lines for stale-fsync after the final intended diff;
- whether BUILD-4B remains blocked.

## 8. Commit Policy

Commit only after:

- mechanism fix is implemented;
- targeted stale-fsync 50x has 0 final double-timeouts;
- simplify gate is complete;
- final full `NEO_SMP=4 cargo neo-test --no-meta` exits 0;
- close artifact is written.

No push. No merge. No BUILD-4B until PM consumes the close artifact.
