<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: QEMU capacity trials on the M75q and M910q (2026-10-26)

Both trials ran `.github/workflows/qemu-trial.yml` (non-gating) at main `9a81203c`. The report came from
`scripts/qemu-trial-report.py` before #146. Each report's one `FAIL: expect_pass_but_fail` is xtask's deliberate-failure
meta-test, and both logs print `neo-test meta: deliberate failure was detected`, so it isn't a real failure.

| | M910q (`m910q-1`) | M75q (`m75q-1`) |
|---|---|---|
| Run | `40378947430` | `40378943109` |
| Shards | 1 of 4 | all 4, one after another |
| Threads / MemTotal | 4 / 15,881 MiB | 8 / 62,221 MiB |
| Scenarios | 93 (92 PASS first try + meta) | 369 (368 PASS first try + meta) |
| Retries / timeouts / under 1.5× | 0 / 0 / 0 | 0 / 0 / 0 |
| Suite wall clock | 1,871 s (31 min) | 8,284 s (138 min; QEMU time 3,791 s, the rest is per-scenario kernel builds) |
| Tightest run | `desktop_lens_boot` 73.5 of 180 s (2.45×) | `desktop_lens_fault_boot` 95.8 of 180 s (1.88×) |
| Peak QEMU RSS | 1,024 MiB | 1,049 MiB |
| Peak memory in use (whole host) | 1,865 MiB | 6,777 MiB, with `m75q-2` running light jobs at the same time |
| Lowest free disk | 178 GiB | 133 GiB (`target/` 63 GB at the end) |

For comparison, the batch-1 gate (`40363142380`, two shards at a time on the ryzen box) took 42.5, 42.9, 45.1 and
45.8 min per shard (176 min in total). In batch 2's re-run (`40383263248`, attempt 2), shards 1 and 4 alone on ryzen
took about 22 min each.

## What it means

- **M75q.** All four shards pass with room to spare (tightest 1.88×). At 62 GiB it clears the gate's existing 32 GiB
  memory pre-flight (`.github/workflows/ci.yml`, the `*) REQUIRED_GIB=32` arm) with no workflow change. It can take
  the `qemu-shard` label on one runner (`m75q-1`) as it is; `m75q-2` stays `light`. This supersedes
  `Plans/DECISION-2026-08-02-m75q-out-of-qemu-shard-pool.md`, which was about the 32 GB WSL2 setup (`ryzen-5`/`ryzen-6`,
  28 GiB floor). The box now runs Ubuntu on bare metal with 62 GiB.
- **M910q.** Shard 1 passes, and its memory need is small. The heaviest whole-suite host use measured anywhere was
  6.8 GiB, light jobs included, against its 15.5 GiB. It fails the gate's 32 GiB pre-flight, though: batch 2's shards 1
  and 4 landed on it at 19:26 UTC and went red as infra in under a second. To use it, `ci.yml` needs a host-specific
  floor for `m910q-1` (14 GiB leaves 2× the measured peak), shipped and gated like any `.github` change, before the
  label goes back on.
- **Adding labels is J.c.'s decision.**

## Outcome (2026-10-26/27)

- J.c. added `qemu-shard` to `m75q-1`. #147 (`9d4f2c69`) gave `m910q-1` a 14 GiB pre-flight floor, and J.c. re-added its
  `qemu-shard` label.
- #147's gate (`40415784574`) was the first with `m75q-1` taking a shard: shard 4 passed in 33.7 min, against
  19–22 min on ryzen.
- #148's gate (`40423706179`) was the first to run all four shards in one wave, on `ryzen-2`, `ryzen-1`, `m75q-1` and
  `m910q-1`. `m910q-1` passed the 14 GiB pre-flight. Test steps took 17.9, 17.6, 36.7 and 33.0 min, and the whole run
  took about 45 min from dispatch, against about 90–110 min with two shard runners.
- On 2026-10-26, crates.io download failures hit only the mini-PCs: `m910q-1` with an HTTP/2 framing error and `m75q-1`
  with a connection reset. #148 (`71dafaa0`) sets `CARGO_HTTP_MULTIPLEXING=false` and `CARGO_NET_RETRY=10` in every
  workflow job, and J.c. set the first on `m910q-1`'s runner `.env` as well. #148's own gate needed no re-runs.
