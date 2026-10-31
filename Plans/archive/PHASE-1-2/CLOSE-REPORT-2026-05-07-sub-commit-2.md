# Close report: sub-commit 2

## Commit

- Hash: `0b5ffb4b1bd7bf6478ad9e987c665ea2b6a9e2d4`
- Branch: `main`
- Lands at: 2026-05-07 ~14:56 -04:00
- Project is local-only; no push step.

## What was committed

- §Q3 per-CPU schedulers.
- §Q5 deferred-quantum mechanism, restored from the earlier failed deletion.
- §Q9 deferred, with the BSP-scans-all-CPUs fallback shipped instead.
- AP GS-base rewire amendment.
- ADR-013 amendments documenting all of the above.
- Two new test scenarios in `kernel/src/test_harness.rs`.

## Closure gate

EXP-H: 100/100 PASS on `ipc_heap_kill_smoke` at `NEO_SMP=4`. Gate met on 2026-05-06.

## What was NOT included

- §Q9 redo, deferred to ADR-016 / sub-commit 5+.
- KASLR heap-placement fix, separate triage; ADR-015 in draft/finalization artifacts.
- §Q9 redo design notes, in `Plans/`, not yet committed; awaits INDEX.md sweep.
- ADR-015, not included in this commit.

## Stashes

Both stashes dropped after successful commit. `git stash list` is empty.

## Next steps for PM

- ADR-015 ratification + KASLR triage dispatch.
- Sub-commit 3a checkpoint conversation, with pre-thinking already at `Plans/SC-3A-CHECKPOINT-PRETHINKING-2026-05-07.md`.
- Jarvis: INDEX.md sweep covering 2026-05-06 14:00 onward.
