# SC-4 Sub-Commit 5b Verification: Phase A cr3_for_thread Retirement

Date: 2026-05-11
Branch: `jarvis/sc-4-5b-thread-pmap-exit-state-2026-05-11`
Base: `origin/main` at SC-4 5a commit `c18646e`

## Scope

Implemented SC-4 5b / Q8.b Phase A:

- Added `Thread.pmap: Option<Arc<Pmap>>`.
- Plumbed user thread pmap ownership through `spawn_user`.
- Retired `process::cr3_for_thread(pid)` from kernel sources.
- Changed scheduler CR3 prep to read `next.pmap` directly.
- Added `Process.exiting: AtomicBool` and checked it during capability install under the recipient handle lock.
- Added `Process.exit_state: AtomicU8` and CAS-claim discipline for natural exit and forced kill.
- Added Q8.b/Q8.d scenarios:
  - `q8d_phase_a_coverage`
  - `q8b_grant_vs_exit_race`
  - `q8b_exit_state_cas`

## Incident During Verification

The first full Docker suite failed in `ipc_heap_kill_smoke` with a final frame leak:

- Symptom: `frame leak: baseline=1027 final=1246`
- Cause: `Thread` storage is slab-backed and `Thread::free` manually drops owned fields before returning raw storage. The new `Thread.pmap` Arc was not cleared, so killed/reaped user threads kept their `Pmap` alive.
- Fix: `Thread::free` now sets `thread.pmap = None` alongside `thread.stack = None`.

## Verification Matrix

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p xtask` | PASS | Host xtask check. |
| `cargo k` | PASS | Warnings only. |
| `cargo k --no-default-features` | PASS | Warnings only. |
| `rg -n "cr3_for_thread" kernel/src` | PASS | No matches; command exits 1 because no references remain. |
| `docker compose run --rm -e NEO_SMP=4 neo sh -lc 'cargo neo-test q8d_phase_a_coverage --no-meta && cargo neo-test q8b_grant_vs_exit_race --no-meta && cargo neo-test q8b_exit_state_cas --no-meta && cargo neo-test q7b_cr3_cache_skip_count --no-meta && cargo neo-test q7c_process_exit_during_thread_run --no-meta'` | PASS | New 5b gates plus SC-3C q7b/q7c regressions. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test ipc_heap_kill_smoke --no-meta` | PASS | Re-run after fixing the `Thread.pmap` slab-drop leak. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | Full Docker suite, including new q8b/q8d gates, `smp_cross_cpu_rendezvous_stress`, and meta-test deliberate failure detection. |
| `docker compose run --rm -e NEO_SMP=4 neo sh -lc 'cargo neo-test ipc_heap_kill_smoke --no-meta && cargo neo-test q8b_exit_state_cas --no-meta'` | PASS | Final targeted rerun after dropping the temporary forced-kill `Arc<Process>` clone before registry removal. |
| `git diff --check` | PASS | CRLF working-copy warnings only. |
| `cargo fmt --check` | BLOCKED | `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`. |

## Stress Note

The full suite exercised `smp_cross_cpu_rendezvous_stress` once under `NEO_SMP=4`. A separate 200/200 repeated stress loop was not run for this sub-commit verification.
