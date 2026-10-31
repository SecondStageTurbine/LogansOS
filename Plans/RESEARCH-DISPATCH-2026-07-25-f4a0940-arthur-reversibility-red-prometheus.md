<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH DISPATCH: `f4a0940` Arthur reversibility gate RED

**Date:** 2026-07-25
**From:** PM
**To:** Prometheus
**Role:** Research and independent verification only
**Candidate:** `f4a0940872e865207128a240587f7cac036f0cf9`
**Comparator:** `625e531f353b417498a968dfa0b74f1d45c0e592`
**Failed gate:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30181808389`
**Failed job:** `https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/30181808389/job/89739709262`
**Status:** **DIAGNOSE ONLY. The candidate is frozen. No rerun token exists.**

Do not write production or test code. Do not commit, push, amend, rebase,
force-push, dispatch a workflow, build with credentials, boot hardware, reset a
device, open a PR, or merge.

## PM-verified failure facts

Run `30181808389` is `workflow_dispatch`, attempt 1, exact candidate SHA.
Shards 2-4, host checks, SPDX, and monolithic identity passed. Shard 1 failed
only because `arthur_reversibility` timed out after four internal 420-second
attempts.

The failing job ran on `ryzen-4` with 94.3 GiB visible memory and 1,345.1 GiB
free Docker-host disk, so the workflow's capacity preflights passed.

The four serial captures do not stop at one deterministic instruction:

- attempt 1 reaches the `write_file` agentic AST;
- attempts 2 and 3 emit successful `write_file` accounting and then stop;
- attempt 4 reaches the following model action and stops after its candidate
  and mask setup.

This is not evidence of an MT7601U-path failure.

The scenario is already known to be flaky:

- green gate `30135801447`, exact `625e531f...`, on `ryzen-3`: attempt 1 timed
  out at 420 seconds; attempt 2 passed in 10.8 seconds;
- green gate `30104558141`, exact `2d43670e...`, on `ryzen-5`: attempt 1 timed
  out at 420 seconds; attempt 2 passed in 14.6 seconds.

The artifact upload quota error occurred after the scenario failure. It did not
cause the timeout, but it prevented durable upload of the serial files.

These facts establish a strong pre-existing-flake hypothesis. They do not waive
the all-green gate or authorize a blind rerun.

## Research question

Does `f4a0940` change any byte or runtime input consumed by
`arthur_reversibility`, or did the existing Arthur/NeoFS/virtio scheduling
flake simply exhaust its four retries on `ryzen-4`?

## R1. Full log and history analysis

Use `gh` to inspect the exact jobs, not summaries. Return:

1. a four-row table of the last meaningful scenario breadcrumb in each failed
   attempt;
2. runner identity, visible memory, disk, QEMU elapsed time, retry count, and
   final status;
3. a table of at least the last five workflow-dispatch gates containing
   `arthur_reversibility`, showing runner, attempt-level timeout/pass sequence,
   and terminal elapsed time;
4. whether the signature correlates with a runner, first attempt, fixture
   reuse, polling/IRQ mode, or another observable variable.

Do not classify the post-failure artifact-quota error as the timeout cause.

## R2. Scenario-specific artifact parity

The prior generic `!mt7601u` equality rows are not enough. Build the exact
`arthur_reversibility` scenario from both comparator and candidate in clean,
detached worktrees and isolated target directories.

Use:

- the repository-pinned toolchain;
- `CARGO_INCREMENTAL=0`;
- one fixed neutral `NEO_BUILD_ID`;
- `RUSTFLAGS` including `-Z location-detail=none`;
- the same `NEO_SMP=4`;
- identical build and normalization commands.

Compare every runtime input that can affect this scenario:

- kernel ELF;
- `neo-init`, `neo-shell`, `neo-arthur`, and any other staged user module;
- staged `limine.conf`;
- the generated Arthur oracle model, vocabulary, and `danger.key`;
- the generated 768 MiB NeoFS scenario image;
- the scenario feature list and QEMU command line.

For each file, return both sizes, both SHA-256 values, and `cmp` result.
Normalize ELF files only when necessary, using an identified
ELF-preserving `llvm-objcopy --strip-all`; also report raw equality. Do not
compare digest envelopes or text surrogates.

If any runtime input differs, identify the producing source delta and explain
whether the difference can reach the observed stall. Do not repair it.

## R3. Controlled targeted reproduction

Run the targeted scenario outside workflow dispatch on the same healthy host
and toolchain:

```powershell
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test arthur_reversibility
```

Use fresh scenario outputs for each invocation. Execute a bounded paired
sequence:

1. comparator `625e531f...`;
2. candidate `f4a0940...`;
3. candidate `f4a0940...` once more only if the first candidate invocation
   differs materially from the comparator.

Do not increase the 420-second scenario budget or the built-in retry count.
Return every attempt's elapsed time, terminal result, and last meaningful
breadcrumb. Preserve local serial logs and hashes when an invocation fails.

If host contention or another environmental condition makes the comparison
invalid, stop and return `INCONCLUSIVE`; do not tune around it.

## Return contract

Return exactly one research verdict:

- `PREEXISTING-FLAKE`: scenario artifacts are byte-equivalent and the observed
  behavior reproduces independently of the candidate;
- `RUNNER-SPECIFIC`: evidence isolates the failure to `ryzen-4` or another
  runner condition;
- `CANDIDATE-RELATED`: a candidate-produced runtime difference reaches the
  scenario or only the candidate reproduces the failure;
- `INCONCLUSIVE`: evidence cannot distinguish the above.

Include the complete evidence tables and a recommendation. Do not fire or
recommend silently treating the RED run as green.

Only the PM may issue a new exact-SHA four-shard token after reviewing this
research. No code or state-changing repository action is authorized.
