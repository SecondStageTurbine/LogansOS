# Close report: KASLR-Q1B (higher-N pre-existence diagnostic)

## Q1B result

- 50 runs on main HEAD `9505619`, `NEO_SMP=4` default suite.
- PANIC: 4.
- PASS: 31.
- TIMEOUT: 14.
- OTHER: 1.

PANIC serial logs:

- Run 1: `target/verify/KASLR-Q1B-main-run-1.slab_thread_migration.serial.log`
- Run 24: `target/verify/KASLR-Q1B-main-run-24.cap_derive_missing_meta.serial.log`
- Run 34: `target/verify/KASLR-Q1B-main-run-34.buddy_alloc_free_alloc_returns_same_frame.serial.log`
- Run 43: `target/verify/KASLR-Q1B-main-run-43.smep_policy.serial.log`

PANIC details:

- Run 1: `virt_base=0xffffffffff826000`; scenario `slab_thread_migration`; `kernel/src/mm/heap.rs:108 attempt to add with overflow`.
- Run 24: `virt_base=0xffffffffff607000`; scenario `cap_derive_missing_meta`; `attempt to add with overflow`.
- Run 34: `virt_base=0xffffffffffb55000`; scenario `buddy_alloc_free_alloc_returns_same_frame`; `attempt to add with overflow`.
- Run 43: `virt_base=0xffffffffff606000`; scenario `smep_policy`; `attempt to add with overflow`.

TIMEOUT scenario distribution:

- `buddy_alloc_free_alloc_returns_same_frame`: 1.
- `buddy_max_order`: 2.
- `buddy_split_coalesce`: 2.
- `cap_derive_subset`: 1.
- `cap_grant_missing_meta`: 1.
- `cap_mint_and_use`: 1.
- `cap_survive_exit`: 2.
- `expect_pass_but_alloc_fail`: 1.
- `ipc_heap_kill_smoke`: 1.
- `slab_local_to_global_promotion`: 1.
- `smap_policy`: 1.

OTHER:

- Run 32: `buddy_alloc_free_alloc_returns_same_frame` did not exit via debug success; serial shows allocator panic at `alloc/src/alloc.rs:573:9` after heap ready, not the `addr.rs`/heap-placement overflow.

## Conclusion

Pre-existing. The `attempt to add with overflow` panic reproduced 4 times on clean `main` HEAD under the requested 50-run `NEO_SMP=4` default-suite diagnostic. The panic occurs at high randomized executable bases and is independent of the sub-commit 2 working tree.

The `4/50` overflow rate is above the dispatch's "4+ surprising" note, so this should be treated as stronger-than-expected evidence that the KASLR heap-placement bug is a real mainline defect, not a sub-commit 2 regression.

## Binary size data

- main HEAD kernel ELF: 9,378,344 bytes.
- sub-commit-2 kernel ELF: 9,379,056 bytes.
- Absolute delta: 712 bytes.
- Percent delta: 0.00759%.

## Working tree state

- Restored to the pre-diagnostic post-EXP-H state.
- Tracked diff: 11 modified tracked files, 366 insertions, 112 deletions.
- Both stashes remain present:
- `stash@{0}: On main: kaslr-q1-higher-n-pre-diagnostic`
- `stash@{1}: On main: sub-commit-2-close-attempt-blocked-on-kaslr`

## Anything weird

- The timeout rate was 14/50, close to the prior 3/10 shape and still consistent with the known harness wedge.
- One non-overflow `OTHER` panic occurred after heap initialization at a high KASLR placement. This is not the `addr.rs` overflow, but it is adjacent enough to mention in the KASLR triage context.
- The first loop wrapper initially classified run 1 as `OTHER` because xtask stdout did not include the serial panic text. I corrected the classification after inspecting the scenario serial log and updated subsequent runs to inspect the failing scenario's serial log for the overflow signature.
