# SC-3C Sub-Commit 4c Verification: Q7.c Pmap Call-Site Wiring

Date: 2026-05-11
Owner: Jarvis
Status: PASS

## Scope

Sub-commit 4c wires the Q7.c shootdown machinery into production Pmap mutation sites that can make stale TLB entries visible across CPUs.

- `map_user_locked` now detects replacement of an existing user leaf PTE and issues the required post-write fence plus ranged shootdown.
- `unmap_user` provides the production user-page unmap path and issues the same fence plus ranged shootdown after clearing the leaf.
- `Pmap::Drop` issues a full-address-space shootdown after page-table leaf collection and before freeing collected page-table pages.
- The call-site pattern is PTE write, `SeqCst` fence per R-C2, then `shootdown(range, other_cpus_mask())`.
- The D14 negative feature `neo-test-missing-shootdown-negative` intentionally suppresses the call-site shootdown so the stale-TLB detector can prove the scenario gate is live.

This work was finalized on top of `f44bf45` (`origin/main`), which already contains sub-commit 4b (`3272840`) plus the parallel rename-audit documentation commit. No unrelated untracked Plan or investigation files were staged.

## Gates

| Gate | Result | Notes |
| --- | ---: | --- |
| `cargo k` | PASS | Existing warnings only. |
| `cargo k --no-default-features` | PASS | Existing warnings only. |
| `cargo fmt --check` | BLOCKED | `cargo-fmt.exe` is not installed for pinned toolchain `nightly-2026-04-16-x86_64-pc-windows-msvc`. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_concurrent_munmap_smp` | PASS | `NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown` |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_process_exit_during_thread_run` | PASS | `NEO_TEST/1 event=PASS name=q7c_process_exit_during_thread_run exit_reaped=true teardown_shootdown=true q8c_deferred=true` |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7c_missing_shootdown_detection` | PASS | `NEO_TEST/1 event=PASS name=q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7` |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | Full suite passed, including the deliberate-failure meta-test. |
| `200/200 NEO_SMP=4 smp_cross_cpu_rendezvous_stress` | PASS | Results stored under `target/verify/sc3c-4c-stress-neo-smp4-20260511-181228`; `results.csv` shows `PASS=200`. |

Direct host `cargo neo-test` is not a valid gate in this environment because the host is missing the Limine/xorriso test-runner dependencies; Docker compose was used for QEMU scenario gates.

## Scenario Evidence

Final `q7c_concurrent_munmap_smp` PASS line:

```text
NEO_TEST/1 event=PASS name=q7c_concurrent_munmap_smp trap=page_fault mechanism=q7c_munmap_shootdown
```

Final `q7c_missing_shootdown_detection` PASS line:

```text
NEO_TEST/1 event=PASS name=q7c_missing_shootdown_detection detected=stale_tlb_read_after_unmap negative_feature=neo-test-missing-shootdown-negative value=0xa7
```

Final `q7c_process_exit_during_thread_run` PASS line:

```text
NEO_TEST/1 event=PASS name=q7c_process_exit_during_thread_run exit_reaped=true teardown_shootdown=true q8c_deferred=true
```

Representative final `200/200` stress PASS line:

```text
NEO_TEST/1 event=PASS name=smp_cross_cpu_rendezvous_stress iterations=1000 send_first=500 recv_first=500 cpus=4
```

## Notes

- An initial `q7c_concurrent_munmap_smp` attempt faulted before the intended stale-TLB check because the harness read a user page from kernel mode with SMAP still enforced. The scenario was corrected to use the existing `with_user_access` helper for deliberate kernel-mode user-page reads, then rerun successfully. This was test choreography, not a shootdown failure.
- The process-exit scenario deliberately marks `q8c_deferred=true`: SC-3C adds the teardown shootdown, while Q8.c remains responsible for the stronger stop-all-threads-before-teardown rule.
- `git diff --check` was clean apart from expected Windows LF-to-CRLF warnings.
