# DISPATCH: CI workflow caching + path filter (Prometheus, small-coding trial)

- Author: PM (Cerebro/Claude)
- Date: 2026-05-23
- Agent: Prometheus (Qwen3.6-27B)
- PL ratification: yes (verbal, this session — "file it directly and lets fix it, send it to Prometheus")
- Scope class: small-coding trial (per roster update 2026-05-10)
- Bible amendment: none
- ADR: none required (infra-level workflow change, no architectural impact)

## 1. Problem

Current GitHub Actions CI workflow (`.github/workflows/ci.yml`) runs the full
QEMU integration test suite on every push to `main` and every PR, regardless
of what changed. Observed run times for the last five runs, all docs-only
commits to `Plans/`:

```
26345867576  22m21s  plans: file NVMe driver flush+completion review notes
26344453252  22m21s  plans: BUILD-5b execution guide revision
26343161927  19m51s  plans: dispatch Jarvis to add block-sink degradation visibility
26322660797  22m21s  kernel+tools: block-device log sink and framebuffer phase indicator
26322558589  21m5s   plans: file BUILD-5b operator execution guide
```

Three causes, all addressable in a single workflow file edit:

1. **No path filter** — markdown-only commits in `Plans/` and `docs/`
   trigger the full QEMU run. ~22 min wasted per docs commit. We commit
   docs heavily during PM workflow.
2. **No Docker layer cache** — `docker compose build neo` rebuilds the
   container from scratch every run. apt-install + rustup take ~5–10 min
   and rarely change.
3. **No cargo target cache** — `target/` and `.cargo-home/` live at the
   workspace root (bind-mounted into the container per `docker-compose.yml`),
   so they vanish between runs. Full kernel + dependency rebuild every
   time. ~5–8 min.

## 2. Deliverables

Modify exactly one file: `.github/workflows/ci.yml`.

Do NOT modify:
- `Dockerfile`
- `docker-compose.yml`
- Any cargo config, `Cargo.toml`, or `Cargo.lock`
- Any kernel or userland source

The full replacement contents of `ci.yml` are specified verbatim in §3
below. Copy them in, commit, push, observe two verification runs
(§5). If both verification runs behave as specified, the dispatch is
complete.

## 3. New `.github/workflows/ci.yml`

Replace the entire contents of the file with this:

```yaml
name: CI

on:
  pull_request:
    paths-ignore:
      - 'Plans/**'
      - 'docs/**'
      - '**.md'
      - 'LICENSE*'
      - '.gitignore'
  push:
    branches:
      - main
    paths-ignore:
      - 'Plans/**'
      - 'docs/**'
      - '**.md'
      - 'LICENSE*'
      - '.gitignore'

jobs:
  neo-test:
    name: QEMU Integration Tests
    runs-on: ubuntu-24.04
    timeout-minutes: 45

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      # The container's `neo` user is UID 1000, but the GHA runner checks
      # files out as UID 1001. Without this, the container can't write to
      # /workspace/.cargo-home or create cargo's atomic temp files at the
      # workspace root, and the build dies with EACCES.
      - name: Align workspace ownership with container user
        run: sudo chown -R 1000:1000 "${{ github.workspace }}"

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Cache Docker layers
        uses: actions/cache@v4
        with:
          path: /tmp/.buildx-cache
          key: buildx-${{ runner.os }}-${{ hashFiles('Dockerfile') }}-${{ github.sha }}
          restore-keys: |
            buildx-${{ runner.os }}-${{ hashFiles('Dockerfile') }}-
            buildx-${{ runner.os }}-

      - name: Build dev container (with buildx cache)
        uses: docker/build-push-action@v5
        with:
          context: .
          file: ./Dockerfile
          tags: neoplutarch-dev:latest
          load: true
          cache-from: type=local,src=/tmp/.buildx-cache
          cache-to: type=local,dest=/tmp/.buildx-cache-new,mode=max

      # Move the new cache over the old one to prevent unbounded growth.
      # docker/build-push-action writes to a separate dir to avoid the
      # known cache-overwrite issue.
      - name: Rotate buildx cache
        run: |
          rm -rf /tmp/.buildx-cache
          mv /tmp/.buildx-cache-new /tmp/.buildx-cache

      - name: Cache cargo registry, git, and target
        uses: actions/cache@v4
        with:
          path: |
            ${{ github.workspace }}/.cargo-home/registry
            ${{ github.workspace }}/.cargo-home/git
            ${{ github.workspace }}/target
          key: cargo-${{ runner.os }}-${{ hashFiles('**/Cargo.lock', 'rust-toolchain.toml') }}-${{ github.sha }}
          restore-keys: |
            cargo-${{ runner.os }}-${{ hashFiles('**/Cargo.lock', 'rust-toolchain.toml') }}-
            cargo-${{ runner.os }}-

      # Re-chown after cache restore: actions/cache restores files owned
      # by the runner user (1001), but the container needs UID 1000.
      - name: Re-align workspace ownership after cache restore
        run: sudo chown -R 1000:1000 "${{ github.workspace }}"

      - name: Run QEMU integration tests
        run: docker compose run --rm -e NEO_SMP=4 neo cargo neo-test
```

## 4. What each change does (for review purposes)

### Path filter (`paths-ignore`)

If every changed file in the push (or PR) matches one of the listed
patterns, the workflow does not trigger at all. Patterns chosen:

- `Plans/**` — PM dispatches, close-reports, NOTES, ratifications
- `docs/**` — bible, phase docs, ADRs, specs
- `**.md` — any markdown anywhere (catches a stray README edit)
- `LICENSE*` — license file
- `.gitignore` — tooling ignore

A commit that touches `Plans/foo.md` AND `kernel/src/lib.rs` still
runs CI, because not all files match the ignore list. This is the
correct safety semantics.

The current project does not require CI as a branch protection check,
so docs-only PRs that skip CI will still mergeable. If we later add
branch protection, we'll need a sibling "skip job" pattern; not for
this dispatch.

### Docker Buildx + layer cache

`docker/setup-buildx-action@v3` installs buildx as the default builder.
`docker/build-push-action@v5` builds the image and saves layers to
`/tmp/.buildx-cache-new`. The `actions/cache@v4` step persists that dir
across runs keyed on the Dockerfile hash.

The cache-rotate step (`rm -rf` + `mv`) works around a known
docker/buildx issue where writing cache to the same dir you're reading
from causes corruption.

After this step, the local docker daemon has `neoplutarch-dev:latest`
loaded, so the subsequent `docker compose run` finds it via the
`image:` field in `docker-compose.yml` and does not rebuild.

### Cargo cache

`target/` and `.cargo-home/{registry,git}` live on the host filesystem
because `docker-compose.yml` bind-mounts `.:/workspace`. Caching them
via `actions/cache@v4` keyed on `Cargo.lock` + `rust-toolchain.toml`
means dependency builds are reused across runs as long as the lock
file doesn't change.

The second `chown` step after cache restore is necessary: the cache
action restores files as the runner user (UID 1001), but the
container's `neo` user is UID 1000. Without the re-chown, the
container hits EACCES during the build.

## 5. Verification

Two test pushes after landing the change, in this order:

### V1 — docs-only push (must NOT trigger CI)

After the workflow change is on `main`, edit any file under `Plans/`
(e.g., add a trivial line to this dispatch document), commit, push.

**Expected:** No workflow run appears in `gh run list`. The branch
status stays clean. Total elapsed CI time: 0.

If a workflow run DOES appear, the path filter is misconfigured.

### V2 — kernel-touch push (must trigger CI, must complete faster)

After V1, make a trivial no-op edit to any file under `kernel/src/`
(e.g., add a blank line to a comment, then remove it in a follow-up
commit if you want a clean tree). Commit, push.

**Expected:**
- Workflow run triggers within 30s of push.
- First run after the cache changes is the "cold" run — still ~22 min
  because there's no cache to restore from yet. THIS IS EXPECTED.
- Second kernel-touch run after that (any further kernel edit) should
  complete in 7–12 min as both caches hit.

So the verification requires THREE commits total:
1. The workflow change itself (cold run, ~22 min — this populates the cache)
2. The V1 docs-only commit (no run)
3. The V2 kernel-touch commit (warm run, target < 12 min)

## 6. Acceptance criteria

- **C1.** `ci.yml` matches §3 verbatim (whitespace and ordering equivalent).
- **C2.** No other file in the repo is modified.
- **C3.** V1 verification: docs-only commit does not trigger any workflow run.
- **C4.** V2 verification: kernel-touch commit triggers CI, completes
  successfully (not just faster — must still PASS), and the second
  such commit's run completes in under 12 min wall-clock.
- **C5.** Cold cache run (the workflow-change commit itself) does not
  regress beyond ~25 min. If it does, the YAML has an error and the
  workflow should be reverted via `git revert`.
- **C6.** Close-report filed at
  `Plans/CLOSE-REPORT-2026-05-23-ci-caching-and-path-filter.md` with:
  - Commit hash of the YAML change
  - V1 verification: link to the commit that should have triggered but
    didn't, and screenshot or text proof from `gh run list`
  - V2 verification: link to the two kernel-touch runs with their wall-clock
    durations from `gh run list`
  - Notes on any deviations or unexpected behavior

## 7. Out of scope (do not do these in this dispatch)

- Adding branch protection rules.
- Adding a sibling "skip" job for required-check scenarios.
- Splitting the workflow into multiple jobs (build vs test).
- Caching the rustup toolchain installation (it's inside the Docker
  image layer; the buildx cache already covers it).
- Any change to the test matrix or QEMU smp count.
- Adding new workflows for lint, fmt, clippy, etc. (separate dispatch
  if/when desired).
- Anything in `Dockerfile`, `docker-compose.yml`, kernel source, or
  userland source.

## 8. Notes for Prometheus

This is your first coding-scope dispatch under the roster update of
2026-05-10. Scope is intentionally tight: one file, contents specified
verbatim, no synthesis required beyond pasting. Three verification
gates (C3, C4, C5) make pass/fail unambiguous.

If you encounter ambiguity, surface it to PM via a NOTES document
before pushing the YAML. Do not improvise on workflow syntax — if the
spec in §3 doesn't compile (YAML parse error), stop and flag.

If V2's second warm run does NOT complete in under 12 min, do not
attempt a tuning pass. File the close-report with the actual
duration observed, list it as a partial pass (C1–C3 met, C4 partial),
and PM will decide whether to revise on a follow-up dispatch.

## 9. References

- `.github/workflows/ci.yml` (current, ~30 lines)
- `Dockerfile` (current, single-stage Debian + Rust + QEMU)
- `docker-compose.yml` (defines `neoplutarch-dev:latest` image, bind-mounts
  workspace, sets `CARGO_HOME` to `/workspace/.cargo-home`)
- `docker/build-push-action` v5 docs:
  https://github.com/docker/build-push-action
- `actions/cache` v4 docs: https://github.com/actions/cache
- Roster update (memory, 2026-05-10): Prometheus = Qwen3.6-27B, scope
  includes "trial for small coding."
