# SC-4 Sub-Commit 5a Verification: Process Handles/Derivations SwitchMutex

Date: 2026-05-11
Branch: `jarvis/sc-4-5a-handles-derivations-switchmutex-2026-05-11`
Base: `origin/main` after SC-3C CI recovery commit `5a88925`

## Scope

Implemented SC-4 5a / Q8.a:

- Converted `Process.handles` and `Process.derivations` from `spin::Mutex` to `SwitchMutex`.
- Preserved `HandleTable` as `Vec<Option<Handle>>`.
- Enforced `handles` before `derivations` locking in derive/revoke paths.
- Documented the revoke walk ordering:
  - `PROCESSES` snapshot only.
  - Per-process locks in ascending `ProcessId` order.
  - `handles` before `derivations`.
  - Channel state innermost.
- Added positive and negative Q8.a gates:
  - `q8a_concurrent_install_revoke`
  - `q8a_concurrent_install_revoke_negative`

## Incident During Verification

The first Docker Q8.a positive run exposed a real derive-vs-revoke race:

- Symptom: `q8a_live_handles_after_revoke count=0x1`
- Cause: `revoke_handle` scanned the derivation closure before removing the root handle, allowing a concurrent derive to create one child after the scan but before final root removal.
- Fix: `revoke_handle` now validates and removes the caller's root handle under the caller handle lock before cross-process derivation scanning. `derive_handle` holds `handles` through child installation and derivation insertion.

## Verification Matrix

| Command | Result | Notes |
| --- | --- | --- |
| `cargo check -p xtask` | PASS | Host build check for xtask. |
| `cargo k` | PASS | Warnings only. |
| `cargo k --no-default-features` | PASS | Warnings only. |
| `cargo neo-test q8a_concurrent_install_revoke` | BLOCKED | Host Limine environment missing: `/home/neo/limine`; Docker path used instead. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q8a_concurrent_install_revoke` | PASS | Positive Q8.a concurrent derive/revoke gate. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q8a_concurrent_install_revoke_negative` | PASS | Negative feature gate detects missing cap-table lock path. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | Full Docker suite, including `smp_cross_cpu_rendezvous_stress` and meta-test deliberate failure detection. |
| `git diff --check` | PASS | CRLF working-copy warnings only. |
| `cargo fmt --check` | BLOCKED | `cargo-fmt.exe` is not installed for `nightly-2026-04-16-x86_64-pc-windows-msvc`. |

## Stress Note

The full suite exercised `smp_cross_cpu_rendezvous_stress` once under `NEO_SMP=4`. A separate 200/200 repeated stress loop was not run for this sub-commit verification.
