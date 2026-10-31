# SC-4 Sub-commit 6 Verification - Blocked-thread Kill Walk

Date: 2026-05-11
Branch: `jarvis/sc-4-6-blocked-kill-zombify-2026-05-11`

## Scope

Implemented SC-4 Q8.c:

- Added `sched::zombify_thread(Box<Thread>)` for forced retirement of parked IPC threads.
- Added `Channel::extract_my_side_if_blocked` for terminate's drained-handle walk.
- Changed `Channel::close_peer` to extract both peer-side and my-side endpoint states; peer waiters wake with `EPIPE`, my-side waiters are zombified.
- Updated `process::terminate` to handle Ready targets through `kill_thread_by_pid` and Blocked-in-IPC targets through the drained channel handle walk.
- Removed the `q8c_deferred=true` marker from `q7c_process_exit_during_thread_run`.
- Updated the userland `ipc_heap_kill_smoke` expectation: `sys_kill` on a blocked worker now succeeds and stale kill returns `ESRCH`.

## Verification

| Gate | Result |
|---|---|
| `cargo check -p xtask` | PASS |
| `cargo k` | PASS |
| `cargo k --no-default-features` | PASS |
| `docker compose run --rm -e NEO_SMP=4 neo sh -lc 'cargo neo-test q8c_blocked_kill_walk --no-meta && cargo neo-test q8c_close_peer_my_side_extract --no-meta && cargo neo-test q7c_process_exit_during_thread_run --no-meta'` | PASS |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke --no-meta` | PASS |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS |
| `rg -n "q8c_deferred\|kill-on-Blocked.*EINVAL\|kill-EINVAL\|did NOT return EINVAL\|expect EINVAL\|target.*ready queue.*Phase 1.5b" kernel userland` | PASS, no matches |

## Scenario Evidence

- `q8c_blocked_kill_walk` PASS line included `mechanism=q8c_blocked_kill_walk`, `detection=parked_thread_zombified`, `channel_closed_both_sides=true`, and `frame_audit_clean=true`.
- `q8c_close_peer_my_side_extract` PASS line included `mechanism=close_peer_my_side_extract`, `detection=parked_thread_zombified`, `channel_closed_both_sides=true`, and `frame_audit_clean=true`.
- `q7c_process_exit_during_thread_run` PASS line now includes `q8c_blocked_kill_walk_active=true`; the old `q8c_deferred=true` marker is absent.
- Full-suite `ipc_heap_kill_smoke` initially failed because userland still expected blocked kill to return `EINVAL`; after updating the smoke expectation to SC-4 Q8.c semantics, the targeted scenario and full suite passed.

## Formatting

`cargo fmt --check` remains blocked locally because `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`.
