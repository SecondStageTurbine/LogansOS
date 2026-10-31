# PM Disposition - Stale-Fsync Spin Localization Consumption

Date: 2026-05-18
Authority consumed:

- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md`
- `Plans/DISPATCH-2026-05-18-stale-fsync-spin-livelock-localization-cerebro.md`

## Disposition

PM accepts the localization findings.

The stale-fsync blocker is now root-caused:

```text
VirtioBlkDevice::inner is held by the completion worker with IF=1.
Timer preempts that worker while the lock is held.
The submitter later runs on the same CPU inside without_interrupts, spins on
the same lock with IF=0, and prevents the timer from rescheduling the holder.
```

Final classification: **Case C**. The completion path is preempted before the
wake can run.

## Accepted Evidence

Accepted findings:

- contended lock is `VirtioBlkDevice::inner: SwitchMutex<VirtioBlkInner>`;
- spinner site is `kernel/src/drivers/block/virtio_blk.rs:521`;
- owner site is `kernel/src/drivers/block/virtio_blk.rs:538`;
- owner is the virtio completion worker (`thread_id=7`);
- spinner is the submitter (`thread_id=0`);
- owner lock acquisition happens with IF=1;
- spinner lock acquisition happens inside `without_interrupts` with IF=0;
- both captured attempts converged on the same lock and same owner site with
  `owner_torn=false`.

## Correction To Proposed Fix

The proposed direct `IrqSafeMutex` swap is not accepted as a "single-file"
fix as written.

Reason: the submit path passes the virtio lock guard into
`block_current_with_token_and_guard`, which is coupled to
`SwitchMutexGuard::release_token_for_context_switch()` and the
`context_switch_with_release2` SwitchMutex lock-byte contract. Replacing
`SwitchMutex` with the current `IrqSafeMutex` would require a broader scheduler
release-on-switch abstraction or a kernel-owned irq-safe release-token lock.
That is not the smallest safe fix for this blocker.

## Authorized Fix

Authorize the narrow mechanism fix:

```text
Keep VirtioBlkDevice::inner as SwitchMutex.
Wrap the completion-side poll lock in without_interrupts.
```

Target:

```rust
fn poll_completions(&self) {
    let completion = crate::arch::x86_64::without_interrupts(|| {
        let mut inner = self.inner.lock();
        inner.poll_used()
    });
    ...
}
```

This closes the actual race window: the completion worker can no longer be
timer-preempted while holding `VirtioBlkDevice::inner`.

## Simplify Gate

After the fix is verified, run a simplify gate over the active WIP.

Do not ship investigation scaffolding or failed tuning knobs by inertia.

Specifically:

- the root fix must remain;
- stale-fsync scenario semantics must remain strict;
- durable evidence improvements may remain only if independently justified;
- temporary liveness/spin instrumentation should be removed unless PM accepts it
  as durable diagnostic substrate;
- timeout/disk/backend tuning that failed to close the blocker should be
  removed unless fresh post-fix evidence proves it is still required.

BUILD-4B remains blocked until the fix, simplify gate, and final full SMP gate
are accepted.
