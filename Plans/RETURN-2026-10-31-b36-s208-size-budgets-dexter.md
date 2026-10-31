<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — #260 size budgets: rebased, re-measured, downloads fail loudly (Section 208)

**Seat:** Dexter (DeepSeek family) · **Dispatch:** `Plans/DISPATCH-2026-10-31-b36-bounces.md`, Section 208
**Branch:** `dexter/size-budgets` · **Base:** `origin/main` = `20ca0480` (rebased; no `site/` path in the diff)
**Old head:** `e0b8121e` → **rebased head before this RETURN:** `0e1e2e39` → **head:** this commit
**Scope:** the CI workflow, the size-budget gate and this RETURN. No `kernel/`, `userland/`, `boot/`, `site/` or
QEMU-scenario change in this branch; the rebases carried main's changes under it.

## 1. Rebase and re-measure

`git rebase origin/main` moved e0b8121e's one commit from base `8d0b366b` to `20ca0480`. `git diff --name-only
origin/main...HEAD` has no `site/` path. `main` gained #246, the `site/` changes, #259 (SDK-ABI) and the docs-only
batch 38/39 commits since the old base. The code tree is unchanged from the previously gated `89e74721`: every
subtree hash matches — `.github` `aa747197`, `kernel` `df5e0405`, `userland` `e6be1a1c`, `boot` `2e9108c5`,
`tools` `083fc5c5`, `Cargo.toml` `5cf7c30c`, `Cargo.lock` `09759e58` — so the close-gate and measurement below
transfer to `0e1e2e39` by the tree-identity rule.

Measured with the exact command the CI step runs (`cargo run -p xtask -- size-budget`, dev container, head
`0e1e2e39`):

| Number | Measured | Ceiling | Headroom |
|---|---:|---:|---:|
| image A kernel `.text` | 1,483,727 B | 1,516,687 B | **32,960 B** (32 KiB + 192 B) |
| model-less ISO | 153,524,224 B | 154,572,800 B | **1,048,576 B** (1 MiB) |

`[xtask] size-budget PASS`; neither ceiling needed raising. `.text` is 192 B *below* the pre-rebase measurement
(1,483,919 B) — main's new code changed layout but did not grow the section — so its headroom is 32,960 B.
`readelf -S` independently confirms `.text` `sh_size = 0x16a3cf` = 1,483,727 B.

The ISO is sector-quantized (74,963 × 2,048 B sectors in this final-head run). A run on `89e74721` produced
74,964 sectors, so the ISO has a one-sector (2,048 B) run-to-run metadata variance; both are well inside the
1 MiB margin.

## 2. Failed downloads now fail the step loudly

`.github/workflows/ci.yml`, `Size budgets — image A .text + ISO (blocking)` step:

- `apt-get download xorriso` and every missing dependency download now print `::error::size-budget: ...` and
  `exit 1` on failure. The old `dpkg -s "$pkg" ... || apt-get download "$pkg" || true` is gone; an installed
  package is skipped with `continue`, and a missing one that cannot be downloaded fails the step.
- Limine is still cloned at the pinned `v8.6.0-binary` branch. The clone, `make`, and the final
  `xorriso --version` each have an explicit `::error::` + `exit 1` wrapper, so a broken download cannot be read
  as "no xorriso, carry on."
- Success path exercised in WSL with the same block: it downloaded `libburn4t64`, `libisoburn1t64`,
  `libisofs6t64` and `xorriso`, extracted them into the runner-local prefix, and `xorriso --version` printed
  `1.5.6`. The failure path is a plain non-zero `apt-get download` under the workflow's `bash -e -o pipefail`,
  with the explicit `::error::` line above it.

## 3. The job still fits its 30-minute timeout

The Product Feature Build job (`timeout-minutes: 30`) on CI run
[41162668349](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41162668349) / job
[124429036420](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41162668349/job/124429036420) spent:

- **Size budgets step:** 03:59:13Z → 03:59:58Z = **45 s**.
- **Whole job:** 03:57:53Z → 04:01:01Z = **3 min 08 s**.
- Other build steps in that job: kernel product build 33 s, shipped-kernel opt-level-3 build 44 s.

Those are step times from the same job definition and the same size-budget step. This dispatch adds only
`if ! ...` wrappers and `::error::` messages around downloads that already ran; it adds no build or download
work, so the 45-second step / 3-minute job remain the measured cost against a 30-minute ceiling.

The head-`0e1e2e39` `workflow_dispatch` run is
[41171463734](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41171463734). At filing its jobs were
still queued behind the self-hosted runners: all five report `busy` while the Actions API lists no in-progress
run (the runner wedge already recorded in the original #260 RETURN). That is the run to read for the
post-change step time once the runners drain; the step-time evidence above is from the prior run at the same
job definition.

## Evidence

- `scripts/worker-close-gate.sh` on `89e74721` (base `c5032e5`, code tree identical to `0e1e2e39`): **all four
  steps OK** — `cargo fmt --all --check`, workspace tests, `ai-ops-tests` (real-quant/AVX2 and soft-f32-only),
  the atrium feature tests, workspace clippy, blocking kernel clippy, the `!xsave-fpu` kernel build, and the
  full CI userland build/clippy list from `ci.yml`; SPDX OK; self-stat **3 files, +343/−4**
  (`.github/workflows/ci.yml`, `tools/size-budget.toml`, `tools/xtask/src/main.rs`).
- `git diff --stat origin/main...HEAD` shows no `site/` path and no `kernel/`, `userland/` or `boot/` change.
- Size-budget: `[xtask] size-budget PASS` at the ceilings above, run on `0e1e2e39`.
- CI: run 41171463734 is the post-change dispatch (queued at filing, runner wedge); the prior run's Product
  Feature Build was green with the step times given in §3.

## Notes

- The `tools/xtask/` change makes the close-gate's conservative `KERNEL_TOUCHING` detector say yes. The diff adds
  only a host command (`size-budget`) and a CI step; it changes no kernel, userland, boot or QEMU scenario code.
  The `workflow_dispatch` run above is the full workflow, including the 4-shard QEMU half, once the runners
  drain.
- The ceilings are unchanged: the new `.text` measurement is smaller, and the ISO's one-sector variance is inside
  the existing noise margin, so no budget raise is warranted.
