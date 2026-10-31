# F-005 Close Report: Forced-Kill Cross-Process Zombify

## Commits

- Base: `199fb51e75c0491e86f8997b22d871dc6f4e5cd8`
- Pattern reference: PR #6 / `4dfb13f33e23da05dd9a672af87a9e6fa08e24b3`
- Fix commit: `9a282f2e34b101171e7789150e97f9f9fd2f0749`

## Fix Shape

Chosen shape: unified pid-aware helper.

`Channel::extract_parked_if_owned(side, expected_pid)` is now the only public extraction primitive for a parked channel side. It delegates to the same private state helper used by `close_peer` before that side is closed. The forced-kill walk passes `target_pid`; `close_peer` passes `exiting_pid`.

This restores INV-IPC-8 lockstep structurally: a thread may be extracted for zombification only when the parked waiter pid matches the expected owner pid. Foreign waiters parked through grants are left in place until normal close semantics wake them with `-EPIPE`.

## Tests

- New scenario: `q8c_blocked_kill_foreign_parked`
  - Covers A force-killing B while foreign grantee C is parked on B's channel side through a grant and B's own thread is blocked elsewhere.
  - Observed: `sys_kill(B) == 0`, C wakes with `-EPIPE`, C is not zombified, B is reaped cleanly, and the post-reap frame audit is clean.
- Adjacent focused gates:
  - `q8c_blocked_kill_walk`
  - `q8c_close_peer_my_side_extract`
  - `cap_survive_exit`
  - `cap_grant_basic`
  - `cap_grant_missing_meta`
- Local full suite:
  - `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test`
  - Exit code: `0`
  - Log: `target/neo-test/full-suite-f005-20260524-160614.out.log`

## CI

- Workflow: `CI` / `QEMU Integration Tests`
- CI run id/hash: `26373931452`
- Head: `9a282f2e34b101171e7789150e97f9f9fd2f0749`
- URL: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/26373931452
- Result: `success`
- Evidence: workflow step `Run QEMU integration tests` completed with `success`; this step runs `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test`.

## INV-IPC-8

INV-IPC-8 is structurally guaranteed for this bug class by the unified helper. Both natural close extraction and forced-kill parked-waiter extraction share the same pid ownership predicate, so a granted foreign waiter cannot be selected for target-process zombification through either path.
