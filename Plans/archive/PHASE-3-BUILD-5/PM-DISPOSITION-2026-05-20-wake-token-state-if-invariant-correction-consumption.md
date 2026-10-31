# PM Disposition - WAKE_TOKEN_STATE IF-Invariant Correction Consumption

Date: 2026-05-20
Authority consumed:

- `Plans/PHASE-3-BUILD-4-WAKE-TOKEN-STATE-IF-INVARIANT-CORRECTION-CLOSE-2026-05-19.md`
- `Plans/DISPATCH-2026-05-19-wake-token-state-if-invariant-correction-cerebro.md`
- Commit `0682d24` (`test/wake-token: wrap remaining WAKE_TOKEN_STATE writes in without_interrupts`)

## Disposition

PM accepts the correction and closes the BUILD-4 wake-token stability blocker.

The previous close artifact correctly identified a same-CPU `SwitchMutex`
deadlock caused by mixed IF=0 and IF=1 holders of `WAKE_TOKEN_STATE`, but PM
blocked closure because three sibling post-wake write paths still violated the
same lock-level invariant. Commit `0682d24` completes the invariant.

Accepted evidence:

- `12/12` `WAKE_TOKEN_STATE.lock()` acquisitions are now inside
  `without_interrupts`;
- build gates preserve the established 72-warning baseline;
- `phase3_sched_wake_token_abandon` passed 50/50 under `NEO_SMP=4`;
- `phase3_sched_wake_token_round_trip` passed 50/50 under `NEO_SMP=4`;
- `phase3_sched_wake_token_stale_generation` passed 50/50 under `NEO_SMP=4`;
- guardrails passed 80/80 under `NEO_SMP=4`;
- total targeted and guardrail result is 230/230 PASS with zero retries;
- final `NEO_SMP=4 cargo neo-test --no-meta` exited 0.

This is sufficient to unblock the next BUILD-4 lane.

## Carry-Forward

The broader mixed-IF `SwitchMutex` audit remains real follow-up work. This
disposition closes the known `WAKE_TOKEN_STATE` violation; it does not claim that
every `SwitchMutex` in the kernel has been audited.

That audit is not a blocker for starting BUILD-4B, but BUILD-4B must not add new
mixed-IF lock sites. Any new or refactored `SwitchMutex` holder introduced by
BUILD-4B must document its interrupt-state discipline at the call site or use an
IRQ-safe wrapper where appropriate.

## Authorized Next Lane

BUILD-4B may proceed:

```text
Plans/DISPATCH-2026-05-20-phase-3-build-4B-mount-table-and-mount-syscalls-cerebro.md
```

No FAT32, ext4, mount namespace, rootfs pivot, symlink traversal, broad POSIX
surface, BUILD-3F CAS/dedup, or BUILD-5 storage work is authorized by this
disposition.
