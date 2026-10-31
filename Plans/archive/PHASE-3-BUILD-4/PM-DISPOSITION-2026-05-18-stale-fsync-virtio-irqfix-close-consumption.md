# PM Disposition - Stale-Fsync Virtio IRQ-Fix Close Consumption

Date: 2026-05-18
Authority consumed:

- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-VIRTIO-IRQFIX-CLOSE-2026-05-18.md`
- `Plans/DISPATCH-2026-05-18-stale-fsync-virtio-irqsafe-completion-fix-cerebro.md`
- `Plans/PHASE-3-BUILD-4-STALE-FSYNC-SPIN-LOCALIZATION-FINDINGS-2026-05-18.md`
- Commit `7166128` (`fs/virtio-blk: hold device-inner lock with IF=0 in completion worker`)
- Commit `27d8c5a` (`tools/xtask: preserve per-attempt serial logs + timeout monitor dump`)

## Disposition

PM accepts the stale-fsync virtio IRQ-fix close artifact.

The BUILD-4 stale-fsync blocker is closed. The failure was root-caused as a
same-CPU `SwitchMutex` deadlock on `VirtioBlkDevice::inner`, caused by a
completion worker holding the device lock with IF=1 while a submitter later spun
on that same lock with IF=0. The accepted fix keeps the existing `SwitchMutex`
contract and masks interrupts only around the completion-worker lock hold.

## Accepted Evidence

Accepted evidence:

- `phase3_neofs_fsync_filecap_stale_regression` ran 50/50 PASS under `NEO_SMP=4`;
- stale-fsync had 0 retry-pass and 0 final double-timeouts after the mechanism fix;
- the final single stale-fsync smoke passed with the default 180s timeout, 128 MiB disk, and default writeback mode;
- final `NEO_SMP=4 cargo neo-test --no-meta` exited 0;
- `cargo build -p xtask`, `cargo k`, and `cargo k --no-default-features` passed;
- warning baseline stayed at 72 warnings for both kernel build gates;
- the simplify gate removed failed tuning and investigation-only scaffolding before commit;
- the retained xtask changes are accepted as durable diagnostic substrate, not as a behavioral workaround.

## Accepted Code Shape

Accepted kernel mechanism:

```rust
fn poll_completions(&self) {
    let completion = crate::arch::x86_64::without_interrupts(|| {
        let mut inner = self.inner.lock();
        inner.poll_used()
    });
    ...
}
```

This is the correct narrow fix for this blocker. It closes the exact
interrupt-state mismatch without broadening the scheduler release-on-switch
surface or changing `VirtioBlkDevice::inner` to a different mutex type.

Accepted durable tooling:

- attempt-numbered serial logs;
- `NEO_TEST_RUN_ID` suffix for outer-loop evidence preservation;
- `NEO_QEMU_TIMEOUT_DUMP` monitor register capture.

These are useful diagnostic substrate and do not change default test behavior.

## Non-Accepted As Closure Evidence For Other Lanes

The wake-token-abandon retry tail remains real and separate.

The close artifact's isolated wake-token-abandon rerun produced 30/30 final PASS
but 5 retry-pass attempts. The failed mixed-loop attempt and the isolated
retry-pass attempts showed no virtio deadlock signature, so they do not block the
stale-fsync IRQ-fix close. They also do not disappear just because the virtio
fix is accepted.

This disposition therefore closes only the stale-fsync virtio deadlock lane.
It does not authorize BUILD-4B.

## Next Authorized Lane

BUILD-4B remains paused.

The next authorized lane is a narrow root-cause investigation of
`phase3_sched_wake_token_abandon` under `NEO_SMP=4`, using the durable xtask
evidence substrate now available. The lane must classify the remaining
scheduler timing tail before any feature work resumes.

No merge. No push. No BUILD-4B until the wake-token-abandon lane is consumed or
PM explicitly reclassifies the risk.
