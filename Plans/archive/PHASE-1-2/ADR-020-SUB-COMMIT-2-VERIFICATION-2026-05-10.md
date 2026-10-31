# ADR-020 Sub-commit 2 verification

**Branch:** `jarvis/kernel-rename-adr-2026-05-10`
**Sub-commit:** ADR-020 LockOwner foundation
**Date run:** 2026-05-11 EDT

## Diff stat

Staged code diff before this verification log:

```text
 kernel/Cargo.toml                 |   4 +-
 kernel/src/arch/x86_64/per_cpu.rs |  19 ++++-
 kernel/src/sched/scheduler.rs     |  12 +++-
 kernel/src/sync/lock_owner.rs     | 145 ++++++++++++++++++++++++++++++++++++++
 kernel/src/sync/mod.rs            |   1 +
 kernel/src/sync/switch_mutex.rs   |  56 ++++++++++++++-
 kernel/src/test_harness.rs        |  64 +++++++++++++++++
 tools/xtask/src/main.rs           |  42 +++++++++--
 8 files changed, 328 insertions(+), 15 deletions(-)
```

## Build gate

`cargo k`

- PASS.
- Kernel warning count: 18, matching the sub-commit 1 baseline.

`cargo k --no-default-features`

- PASS.
- Kernel warning count: 18.
- Confirms the `kernel-diagnostic` cfg-off path builds with `LockOwner` as a ZST and without lock-path owner argument evaluation.

`cargo fmt`

- Not run successfully: `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`.
- Formatting was kept manual.

`git diff --check`

- PASS for whitespace errors.
- Output contained only existing Windows line-ending conversion warnings for touched files.

## Integration gate

Host `cargo neo-test` is not usable in this workspace because the host is missing Limine/xorriso pieces. Per the project Docker path, QEMU gates were run in the dev container with `NEO_SMP=4`.

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test lock_owner_consistent`

- PASS.
- Serial frame:

```text
NEO_TEST/1 event=PASS name=lock_owner_consistent cpu=0 thread_id=0 site=0xad605c76
```

`docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all`

- PASS, command exit 0.
- `target/neo-test` summary after the run:
  - `logs=35`
  - `pass_logs=34`
  - `fail_logs=1`
- The single FAIL log is the expected `expect_pass_but_fail` meta-test, and xtask reported that deliberate failure was detected.

## Implementation notes

- `SwitchMutex` now records owner metadata after acquiring the lock and clears it before releasing the lock.
- The context-switch release path clears owner metadata through `release_token_for_context_switch()` before the guard is forgotten and the asm release token is handed off.
- `current_thread_id` is mirrored in per-CPU state so diagnostic reads do not need to lock scheduler state.
- Acquisition-site hashing uses `#[track_caller]` to preserve the caller's file/line through `SwitchMutex::lock()` and `try_lock()`, then hashes that static location with the ADR's FNV-style `hash_site` helper. The `lock_site!()` macro is also exported for future explicit acquisition sites, but the generic `SwitchMutex` API keeps existing callers unchanged.

## Result

ADR-020 sub-commit 2 gate PASS. Proceed to ADR-019 sub-phase A.
