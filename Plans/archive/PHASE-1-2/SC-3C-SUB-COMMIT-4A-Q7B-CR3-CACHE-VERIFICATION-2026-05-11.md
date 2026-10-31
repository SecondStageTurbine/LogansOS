# SC-3C Sub-Commit 4a Verification: Q7.b CR3 Cache

Date: 2026-05-11
Owner: Jarvis
Status: PASS

## Scope

Implemented Q7.b per-CPU active CR3 caching for scheduler address-space switches:

- Added `PerCpu::active_pmap_cr3` compare-and-skip use in the scheduler switch path.
- Encoded `(PhysFrame, Cr3Flags)` as a reversible CR3 value via `per_cpu::encode_cr3`.
- Treated cache value `0` as a compulsory miss.
- Kept the required ordering: `Cr3::write(frame, flags)` occurs before the relaxed per-CPU cache store.
- Added instrumentation-gated CR3 write counters.
- Added the `q7b_cr3_cache_skip_count` synthetic same-Pmap scheduler scenario.
- Left PCID and cross-CPU shootdown behavior deferred to later SC-3C work.

## Gates

| Gate | Result | Notes |
| --- | --- | --- |
| `cargo k` | PASS | Host PowerShell build completed. Existing warnings only. |
| `cargo k --no-default-features` | PASS | Host PowerShell build completed. Existing warnings only. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test q7b_cr3_cache_skip_count` | PASS | Q7.b scenario completed under the Docker CI-like environment. |
| `docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all` | PASS | Full neo-test suite completed, including deliberate-failure meta-test detection. |

Direct host `cargo neo-test q7b_cr3_cache_skip_count` was not a valid gate on this machine because the host PowerShell environment is missing the Limine/xorriso test-runner dependencies (`/home/neo/limine` was not present). QEMU scenario gates were therefore run through Docker.

`cargo fmt` was attempted but could not run because `rustfmt` is not installed for the pinned `nightly-2026-04-16-x86_64-pc-windows-msvc` toolchain.

## Q7.b Scenario Evidence

Final `NEO_SMP=4` serial PASS line:

```text
NEO_TEST/1 event=PASS name=q7b_cr3_cache_skip_count workers=4 yields_per_worker=64 skipped=193 performed=138 threshold_ratio_num=1 threshold_ratio_den=2
```

Threshold required `skipped > performed / 2`.

Observed:

- skipped: 193
- performed: 138
- skipped/performed: 1.398

The observed skip count exceeds the required threshold.

## Deferred Gates

The SC-3B 200/200 `NEO_SMP=4` stress loop is not required for sub-commit 4a per the SC-3C dispatch. It remains deferred until after the shootdown machinery and call-site wiring sub-commits.
