<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Self-Hosted CI Runner Runbook

## Current Topology

CI is self-hosted only. Do not add GitHub-hosted runners or `ubuntu-latest`;
the project has zero hosted-minute budget.

As built on 2026-06-16, the runner pool is:

| Runner name | Role |
| --- | --- |
| `altimitneo-wsl` | Primary OptiPlex WSL runner |
| `gaming-pc-2` | Gaming PC runner |
| `gaming-pc-3` | Gaming PC runner |

All workflow jobs use `runs-on: self-hosted`. GitHub schedules each job onto
any online runner in the pool.

Check live state:

```powershell
gh api repos/SecondStageTurbine/NeoPlutarch/actions/runners `
  --jq '.runners[] | [.name, .status, .busy] | @tsv'
```

Expected operating state: all three runners are `online` unless the Gaming PC
runners were intentionally stopped while gaming.

## Workflow Shape

`.github/workflows/ci.yml` intentionally keeps one workflow with job-level
gating:

- `host-checks` runs the host gate: `cargo fmt --all --check`,
  `cargo test --workspace --exclude neoplutarch-kernel`,
  `cargo run -p xtask -- shard-coverage 4`, and host clippy.
- `spdx-lint` checks source SPDX headers.
- `neo-test` runs the full QEMU matrix:
  `cargo neo-test --shard N/4` for shards `1..4`.

`neo-test` runs only for:

- `workflow_dispatch`
- nightly `schedule`
- `push` to `main`

Routine non-main branch pushes stay cheap. The required pre-merge QEMU proof
for kernel/scenario branches is a manual `workflow_dispatch` run on that branch
ref.

The concurrency group is:

```yaml
ci-${{ github.workflow }}-${{ github.ref }}-${{ github.event_name }}
```

That keeps stale runs on the same branch/event self-draining, while a manual
`workflow_dispatch` QEMU gate is not cancelled by an ordinary branch push.

## Pre-Merge QEMU Close Gate

Chosen policy: documented `workflow_dispatch` on the branch.

Reason: it gives the PM/coder a full branch-QEMU proof before merge without
burning all three self-hosted runners on every intermediate kernel push. An
auto-trigger for `codex/**` kernel branches can be added later as an explicit
opt-in, but this runbook keeps the current check set unchanged.

This close gate is mandatory before PM merge when the branch changes any of:

- `kernel/**`
- `tools/xtask/**`
- scenario definitions or the QEMU harness
- `Cargo.toml`, `Cargo.lock`, or crate `Cargo.toml` files

Run it after the branch is rebased and pushed:

```powershell
git fetch origin
git rebase origin/main
$branch = git branch --show-current
git push -u origin $branch

gh workflow run ci.yml --ref $branch
Start-Sleep -Seconds 10
$run = gh run list --workflow ci.yml --branch $branch --event workflow_dispatch --limit 1 --json databaseId --jq '.[0].databaseId'
gh run watch $run --exit-status
gh run view $run --json status,conclusion,event,headBranch,url --jq '.'
```

Close only after the run is green and includes:

- `Host Tests, Format & Lint`
- `SPDX License Headers`
- all four `QEMU Integration Tests (shard N/4)` jobs

If a dispatch needs to cite the gate, use this form:

```text
Pre-merge QEMU gate: trigger `gh workflow run ci.yml --ref <branch>` after
rebase/push; require the workflow_dispatch run green, including all four QEMU
shards, before PM `pm-merge-check` and `--ff-only`.
```

## Cache Isolation

Buildx cache is per runner:

```text
/tmp/.buildx-cache-${{ runner.name }}
/tmp/.buildx-cache-new-${{ runner.name }}
```

This avoids the root-owned/shared-cache collision that appears when several
self-hosted runners rotate the same `/tmp/.buildx-cache` path.

Sanity check after workflow edits:

```powershell
rg -n "/tmp/\\.buildx-cache" .github\workflows\ci.yml
```

Every real cache path must include `${{ runner.name }}`. Diagnostic wildcard
echoes are allowed only when they do not read, write, move, or delete the shared
base path.

Cargo, Rustup, Docker config, and HOME state are workspace-local:

```text
${{ github.workspace }}/.docker
${{ github.workspace }}/.home
${{ github.workspace }}/.rustup-home
${{ github.workspace }}/.cargo-home
${{ github.workspace }}/target
```

QEMU shard target caches are restore-only and shard-local. Do not turn QEMU
shard target-cache saves back on without checking runner disk and upload time.

## Runner Service Model

Runner services should run as a normal WSL user, not as root. Use `sudo` only
for service installation/control:

```bash
cd ~/actions-runner-<name>
sudo ./svc.sh install "$USER"
sudo ./svc.sh start
sudo ./svc.sh status
```

Do not run `sudo ./run.sh` for normal operation. If a runner was accidentally
installed root-owned, stop it, uninstall it, fix ownership, then reinstall it
for the unprivileged user:

```bash
cd ~/actions-runner-<name>
sudo ./svc.sh stop || true
sudo ./svc.sh uninstall || true
sudo chown -R "$USER:$USER" .
sudo ./svc.sh install "$USER"
sudo ./svc.sh start
```

Each runner instance needs its own install directory and work directory. Do not
point two runner services at the same `_work`.

## Stop While Gaming

On the Gaming PC, stop `gaming-pc-2` and `gaming-pc-3` before gaming if CI load
would interfere. Stopping the service is enough; do not remove the runner from
GitHub.

```bash
cd ~/actions-runner-gaming-pc-2 && sudo ./svc.sh stop
cd ~/actions-runner-gaming-pc-3 && sudo ./svc.sh stop
```

Bring them back afterward:

```bash
cd ~/actions-runner-gaming-pc-2 && sudo ./svc.sh start
cd ~/actions-runner-gaming-pc-3 && sudo ./svc.sh start
```

Verify:

```powershell
gh api repos/SecondStageTurbine/NeoPlutarch/actions/runners `
  --jq '.runners[] | [.name, .status, .busy] | @tsv'
```

## Failure Triage

Use these checks before changing workflow behavior:

```powershell
gh run list --workflow ci.yml --limit 10
gh run view <run-id> --log-failed
gh api repos/SecondStageTurbine/NeoPlutarch/actions/runners `
  --jq '.runners[] | [.name, .status, .busy] | @tsv'
```

If QEMU shards fail on a branch, inspect the failed shard logs and the uploaded
`neo-test-serial-logs` artifact before rerunning. If only the host shard-coverage
gate fails, fix the scenario selection or matrix count mismatch before trusting
any QEMU green.
