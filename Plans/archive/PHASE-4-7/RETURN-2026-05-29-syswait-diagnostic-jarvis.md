# RETURN: sys_wait_multiple_waiters sweep-vs-standalone diagnostic

**Date:** 2026-05-29  
**From:** Jarvis  
**To:** PM (Cerebro)  
**Branch:** `codex/syswait-diagnostic-jarvis`  
**Diagnostic base:** current branch fast-forwarded to `origin/main` HEAD `497ef80` after the run; diagnostic began from `e2b7ad5` / `a4d37f2` lineage.  
**Scope:** Diagnostic only. No kernel, test, harness, or `jira-dispatcher.py` edits.

## Commands

Exp A:

```powershell
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test sys_wait_multiple_waiters --no-meta
```

repeated 20 times.

Exp B:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all --no-meta
```

Exp D:

```powershell
git worktree add --detach <temp-worktree> 8b5e7f6
docker compose run --rm -e NEO_SMP=4 -e NEO_TEST_ATTEMPTS=1 neo cargo neo-test sys_wait_multiple_waiters --no-meta
```

repeated 20 times at `8b5e7f6` (`plans: close phase 3 - storage stack complete, verified bare metal through build5b`), the closest pre-Phase-4 close point.

Raw command logs were kept outside the repo under:

```text
C:\Users\<user>\AppData\Local\Temp\syswait-diagnostic-20260529-141337
```

## Exp A - standalone repeat-stress, current main

Result: **9/20 PASS, 11/20 FAIL** under `NEO_SMP=4`, `NEO_TEST_ATTEMPTS=1`.

Fail detail:

- 9 runs failed with:
  `xtask: sys_wait_multiple_waiters did not exit via debug success; status=exit status: 35`
- 2 runs timed out with:
  `xtask: QEMU timed out after 180s for sys_wait_multiple_waiters`

Tally:

```text
run 01 FAIL status=35
run 02 FAIL timeout
run 03 PASS
run 04 PASS
run 05 PASS
run 06 PASS
run 07 FAIL status=35
run 08 FAIL status=35
run 09 PASS
run 10 FAIL timeout
run 11 PASS
run 12 FAIL status=35
run 13 FAIL status=35
run 14 FAIL status=35
run 15 PASS
run 16 FAIL status=35
run 17 FAIL status=35
run 18 PASS
run 19 FAIL status=35
run 20 PASS
```

Interpretation: standalone failure reproduces under SMP=4, so this is **not sweep-only** by the dispatch rule.

## Exp B - full sweep, default attempts

Result: **PASS**, command exit `0`.

Relevant sweep observations:

- `sys_wait_multiple_waiters` passed on attempt 1.
- `sys_wait_blocking` had one transient timeout and then passed on retry attempt 2/4:
  - `sys_wait_blocking attempt 1/4 failed: QEMU timed out after 180s`
  - `sys_wait_blocking PASS after retry attempt 2/4`

Interpretation: the full sweep can go green under default retry behavior, and this specific `sys_wait_multiple_waiters` run did not need a retry. Exp A still shows the standalone scenario is intrinsically flaky/racy under SMP=4.

## Exp C - code=6 assertion semantics

Source: `userland/neo-init/src/main.rs`, `sys_wait_multiple_waiters()`.

The scenario does this:

1. Spawn a `neo-worker` with a primary process handle.
2. Derive `second` from the same process handle with `WAIT` rights.
3. Release the worker and wait until `sys_inspect(handle)` reports the expected exited snapshot.
4. First waiter: `sys_wait(handle, 0)` must return `Ok(WAIT_MULTIPLE_CODE)`. Wrong result returns code `5`.
5. Second waiter: `sys_wait(second, 0)` must return `Err(ESRCH)`. Anything else returns code `6`.

Exact assertion behind code `6`:

```rust
match sys_wait(second, 0) {
    Err(e) if e == ESRCH => 0,
    _ => 6,
}
```

Failing waiter semantics: **the second derived process handle observes a wrong wait result after the first waiter has consumed the exit completion**. The user process does not log the actual returned value, so code `6` only narrows it to "not `Err(ESRCH)`"; that includes a duplicate successful reap, another errno, or a path that fails to reach the expected stale-handle result before debug failure.

## Exp D - pre-Phase-4 / Phase-3-close standalone repeat-stress

Commit tested: `8b5e7f6` (`plans: close phase 3 - storage stack complete, verified bare metal through build5b`).

Result: **13/20 PASS, 7/20 FAIL** under `NEO_SMP=4`, `NEO_TEST_ATTEMPTS=1`.

All 7 failures had the same explicit failure signature:

```text
xtask: `sys_wait_multiple_waiters` did not exit via debug success; status=exit status: 35
```

Tally:

```text
run 01 PASS
run 02 FAIL status=35
run 03 PASS
run 04 PASS
run 05 PASS
run 06 FAIL status=35
run 07 PASS
run 08 FAIL status=35
run 09 PASS
run 10 FAIL status=35
run 11 PASS
run 12 FAIL status=35
run 13 PASS
run 14 PASS
run 15 FAIL status=35
run 16 PASS
run 17 PASS
run 18 PASS
run 19 FAIL status=35
run 20 PASS
```

Interpretation: the race/flakiness is **pre-existing at Phase-3 close**. It was not introduced by Phase 4 or by the recent perf-proxy SMP-gate fix.

## Verdict

**B: intrinsic SMP race/flakiness, pre-existing at Phase-3 close.** The full sweep is retry-maskable at default attempts, but standalone `NEO_SMP=4` repeat-stress fails on current main and also fails at `8b5e7f6`; the failing assertion is the second derived waiter not getting `ESRCH` after the first waiter consumes the target exit.
