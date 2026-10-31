<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: CI gate scaling — the tiered-gate / suite-growth policy

**Date:** 2026-07-12
**Author:** Prometheus (Qwen)
**Dispatch:** `Plans/DISPATCH-2026-07-12-ci-gate-scaling-think-prometheus-and-cerebro.md`
**Status:** Design THINK, host-only. Independent of Cerebro's THINK — PM synthesizes.

---

## ISC-1: The cost model, from real data

### Primary evidence

**Jarvis B3 gate measurements** (`82f33270`, the incoming baseline):
- Host job: cold-cache measured **24–26 min** (fmt + workspace tests + shard-coverage + clippy workspace + clippy kernel + no_std guard). Old 30-min ceiling cancelled cold runs; raised to **60 min**.
- QEMU shards: old 70-min ceiling killed shards at **14–16 of 60–69 scenarios** (all passing, still compiling/running). Raised to **240 min** — covers measured slope.
- Staging: `neo-test` now `needs: host-checks` — QEMU shards wait on a green host job. Total gate = host (24–26 min cold) + QEMU shards (parallel, up to 240 min).

**Per-scenario cost distribution** (from xtask `main.rs`):
- Default timeout: `TEST_TIMEOUT = 120s` (2 min).
- Timeout-heavy scenarios (explicit overrides): `arthur_memory_encrypted`, `arthur_memory_delegation`, `arthur_memory_forgetting`, `arthur_memory` — each has `scenario_timeout() > TEST_TIMEOUT`. These are the CI-contention tail, deferred to shard tails via `scenario_shard_run_late()`.
- Real-brain scenarios (`arthur_real_brain_boot`) skip in CI (`.neom` models not in git).
- The interactive prompt-driven scenarios (A3 agentic, reversibility, multistep) are the new cost class: they drive serial input/output over QEMU's serial console, which is slower than the old semihost-only scenarios.

**Cold vs warm cache split** (from runbook + ci.yml):
- **Docker image build:** ~13 min cold (QEMU 10.2.3 source compile). Cached via buildx layer cache keyed on Dockerfile hash — warm hit is near-instant.
- **Cargo registry/git/target:** Restored from `actions/cache/restore` but **not saved** from QEMU jobs ("self-hosted target uploads can run for 30+ min"). This means every QEMU shard starts with at best a partial cargo cache hit (registry index, not compiled targets). The target directory — the compiled kernel + userland binaries per scenario feature-set — is rebuilt every cold run.
- **Estimated cold split:** From the 240-min ceiling covering "60/69-scenario shard slopes + honest interaction cost + cold dependency graph": I estimate ~30-40 min of a cold shard is build (cargo compile kernel variants + userland), and ~160-200 min is scenario execution. Warm cache could halve the build component to ~5-10 min if target artifacts were cached between runs.

**TCG multiplier:** Pure emulation (no KVM/HVT on WSL2 Docker containers). TCG is roughly **5-10x slower** than native for x86_64 code execution. The kernel boot + userland scenarios run entirely under TCG. This is a hard floor — WSL2 Docker containers can't expose `/dev/kvm`.

**Growth rate:** The dispatch states ~240+ scenarios across 4 shards. From the xtask source, I can count the scenario registry (`TEST_SCENARIOS` array). Every feature build adds scenarios, and the standing rule **forbids pruning** ("gate discriminators are never removed as test-only weight — they are paid-down debt and stay armed permanently"). The suite is monotonically increasing. At the current rate (B1→B2→B3 added ~20-30 scenarios each), expect +10-20 scenarios per kernel-touching build.

### Cost model summary

| Component | Cold (measured/estimated) | Warm (estimated) |
|-----------|--------------------------|------------------|
| Host job (fmt, test, clippy, no_std) | 24–26 min | 10–15 min (with cargo cache) |
| Docker image build | 13 min cold / ~0 min warm (buildx cache) | ~0 min |
| QEMU shard (60-69 scenarios) | ~200 min (30 min build + 170 min exec) | ~150 min (5 min build + 145 min exec) |
| **Full gate (host + 4 shards parallel)** | ~226 min (staging: 26 + 200) | ~165 min (15 + 150) |

**The binding constraint is QEMU scenario execution under TCG.** Build caching helps the host job meaningfully but saves only ~30 min per shard (15% of a cold shard). The 85% is QEMU execution time, which no cache touches.

---

## ISC-2: The tiered gate

### Core design: two tiers, one invariant

**The invariant:** every scenario is armed somewhere, always. A green merge-tier does not mean "the full suite passed" — it means "the merge-tier passed, and the full suite ran nightly on this tree within N hours." No coverage is ever dropped.

### Tier membership criterion: deterministic, principled, auditable

**Two-axis membership:**

| Axis | Definition | Scenarios included |
|------|-----------|-------------------|
| **Standing core** | Boot, caps, governance invariants + every I-14 discriminator + the per-merge safety floor | ~60-80 scenarios (estimated: all scenarios whose `name` matches boot, ipc, caps, spawn, kill, revoke, derive, grant, map, mount, observability, kernel_pass, init, shell, arthur_console, v1_console, input_path, memory_basic, grants_persist, heap_kill, page_fault, tss, smp, scheduler categories) |
| **Impacted-area** | Scenarios whose kernel source paths overlap the diff | Deterministic: `git diff --name-only main...HEAD \| grep '^kernel/src/'` → map to scenario via `scenario.kernel_paths()` (or a simpler heuristic: scenario name prefix matches touched module) |

**The merge-tier = standing core ∪ impacted-area.** This is:
- **Deterministic:** given a diff, the same scenarios are selected every time.
- **Auditable:** the xtask prints the tier label and scenario count at shard start.
- **Never silently incomplete:** the merge report says "MERGE-TIER: N scenarios (core: M, impacted: K)" — no misleading green.
- **Bounded:** even a diff touching everything falls back to the standing core + all impacted = the full suite (degenerate case, honest).

**Implementation in xtask:** A new `--tier merge` flag that filters `TEST_SCENARIOS` via the two-axis rule. The shard partition logic is unchanged — it partitions whatever subset `--tier` produces. The `shard-coverage` host-check extends to verify the merge-tier partition is also complete + disjoint.

**What defines the standing core?** A `#[scenario(tier = "core")]` attribute (or equivalent tag in the scenario struct) applied at scenario registration. The PM reviews and approves each tag. Existing I-14 discriminators are auto-tagged core. New scenarios default to **full-tier-only** until the PM promotes them to core (the feedback loop from ISC-3). This prevents the core from inflating by accident.

### Everything stays armed: the full tier

The **nightly `schedule` trigger** (cron `0 8 * * *`) already runs the full suite on main. It continues unchanged. Additionally, every `workflow_dispatch` gate runs **only the merge-tier** by default, but provides a `--tier full` option (or `full-gate: true` dispatch input) for high-blast-radius changes. This keeps the full-gate option one dispatch away.

### No misleading green

The workflow job name encodes the tier:
- Merge-tier: `QEMU Integration Tests (MERGE-TIER shard N/M)`
- Full-tier: `QEMU Integration Tests (FULL shard N/M)`

The serial log header prints: `[xtask] tier=merge core=68 impacted=12 total=80` (example). The merge report (close-gate) states which tier ran. A green merge-tier means "core + impacted areas are clean" — not "everything passed."

---

## ISC-3: The escaped-defect contract

### What a green merge-tier means

**Definition:** "The standing core invariants hold, and the kernel areas touched by this diff are exercised clean." It does NOT mean "every scenario passes." It DOES mean: if a core invariant is broken, the gate caught it. If only a non-core, non-impacted scenario would fail, the nightly will catch it.

### When nightly reddens main

**The contract:** an escaped defect (green merge-tier → red nightly) triggers a **bounded fix-forward window**:

| Timeline | Action |
|----------|--------|
| **0–4 hours after nightly red** | The merge author (or on-call coder) triages: is it a real regression or a flaky infra failure? Pre-flight guards (RAM, disk) already filter infra. If the scenario failed with a deterministic assertion, it's a real regression. |
| **4–12 hours** | **Fix-forward window.** The author lands a fix on main without re-running the full merge-tier gate (the nightly already proved the defect; the fix is bounded to the escaped area). The fix MUST include the impacted-area scenarios + the escaped scenario. |
| **12 hours — no fix landed** | **Revert.** The original merge is reverted. The revert commit cites the escaped scenario. The author re-merges after fixing + full gate. |

**Rationale for fix-forward (not revert-first):** Revert-first is the safe default but has a hidden cost: it burns the merge, loses context, and forces a restart. A 12-hour bounded window gives the author time to fix the known defect while containing blast radius (the defect was already caught by nightly; it's not silently on main for days). The 12-hour deadline is tight enough that the defect doesn't accumulate downstream merges.

**Who owns it?** The merge author. The nightly failure annotation tags the PR/commit author. If the author is unavailable after 4 hours, PM assigns.

### The feedback loop: promote escaped scenarios to core

When a scenario escapes (passed merge-tier, failed nightly), it is **automatically promoted to the standing core** (`tier = "core"` tag added). The rationale: if a scenario can fail independently of the diff's impacted area, it's a core invariant and belongs in the merge-tier. This is the self-healing mechanism — the core grows toward real discriminators, not by PM intuition.

**Guard against core inflation:** The PM reviews each promotion. If the escape was a one-off (timing-sensitive, hardware-dependent misclassification), the PM can reject the promotion and document the reason. But the default is promote — the core should reflect what actually matters.

---

## ISC-4: Parallelism + fleet

### Current fleet

- **3 self-hosted runners** on one i9-9900K box (8C/16T, 128 GB RAM, WSL2 capped at 96 GB).
- Each runner = 1 concurrent GitHub job. 3 runners × 4 vCPU (`NEO_SMP=4`) = 12 vCPU oversubscribing 8 physical cores (2× SMT).
- **Box top-out:** ~3 smooth shards max (the runbook confirms: "this box tops out at ~3 smooth shards regardless of RAM"). The 4th shard waits for a slot.
- **4th box procured** (Ryzen M75q-1) — not yet racked. This is the key leverage point.

### Shard rebalance: 4 → N, cost-weighted

The xtask partition is already cost-aware (`scenario_shard_run_late` defers timeout-heavy scenarios to shard tails). The partition supports any count (proven by the `shard_partition_is_complete_and_disjoint_for_all_counts` test up to 8). **Recommendation: scale shard count with runner count.**

| Fleet size | Shard count | Rationale |
|-----------|-------------|-----------|
| 3 runners (current) | 4 shards | 3 run parallel, 1 queued. Cost-weighted partition keeps the queued shard light enough to start quickly when a slot opens. |
| 4 runners (4th box racked) | 6 shards | 4 run parallel, 2 queued. More parallelism = less per-shard scenario count = shorter wall-clock per shard. |
| 5+ runners | 8 shards | Diminishing returns past 8 (each shard has fewer scenarios, and the partition overhead grows). |

**Auto-scaling shard count:** The workflow determines shard count from the number of available runners at dispatch time. GitHub doesn't expose runner count directly, but we can use a fixed mapping: `workflow_dispatch` → read a `FLEET_SIZE` env var or config file that the PM updates when the fleet changes. Simpler: **hard-code shard count in ci.yml** and update it when the fleet changes. The xtask partition handles any count correctly (proven to 8).

### Merge-tier time target

**Target: merge-tier ≤ 45 min end-to-end** (host + QEMU shards). This is the "don't make me wait for lunch" threshold — fast enough that a coder can gate, grab coffee, and come back to a verdict.

**How to hit it:**
- Standing core: ~60-80 scenarios → at ~2-3 min/scenario average under TCG → ~120-240 min serial. Sharded 4 ways on 3 runners: ~40-80 min wall-clock for the heaviest shard. With impacted-area overlap, some shards finish early.
- With 6 shards on 4 runners (4th box): ~30-60 min wall-clock.
- **The 45-min target is achievable with 4+ runners and a core of ~70 scenarios.** With 3 runners, expect 60-80 min for the merge-tier — still better than 240 min full-suite.

### Per-runner-label guard thresholds

When the 4th box (Ryzen M75q-1) is slower than the i9, it needs a **per-runner-label guard**: label it `self-hosted,slower` and give it a higher timeout or fewer scenarios. The xtask could read `RUNNER_NAME` and adjust per-scenario timeouts proportionally. **Simpler approach:** don't shard differently — just accept that the slower box's shard takes longer, and set the workflow timeout to accommodate the slowest runner. The merge-tier's smaller scenario count makes this tolerable.

---

## ISC-5: Warm-cache levers

### Lever 1: Save cargo target cache from QEMU shards (highest impact)

**Current state:** QEMU jobs restore cargo cache but **never save** ("self-hosted target uploads can run for 30+ min"). This means every cold run recompiles all kernel variants + userland binaries. The measured ~30 min build time per cold shard is this recompilation.

**Fix:** Add a `post: always()` save step for the cargo target cache, but **only save on success** (a failed shard's target dir may be corrupt). Use `actions/cache/save@v4` in a post-step conditional on `!failure()`. The upload time concern is real — but self-hosted runners don't upload to GitHub's cache server; they use the local filesystem. The `actions/cache` on self-hosted runners with a local cache dir is instant (local copy, not network upload).

**Expected savings:** ~25 min per cold shard (build drops from 30 min to 5 min). **Total gate savings: ~100 min** (4 shards × 25 min). This alone brings a warm full-suite gate from ~226 min to ~126 min.

### Lever 2: Prebuilt dev image (already largely solved)

The buildx layer cache keyed on Dockerfile hash is already effective — warm hits are near-instant. The 13-min cold build only happens when the Dockerfile changes (rare). **No action needed.**

### Lever 3: Persistent per-runner cargo registry cache

The registry cache (downloaded crate source) is restored but not saved. On self-hosted runners, this should be a **persistent volume** outside the workflow workspace — mounted at `.cargo-home/registry`. GitHub Actions' workspace is ephemeral, but `/tmp` or a dedicated `/gha-cache` path persists between runs on self-hosted runners.

**Fix:** Mount a persistent registry cache path. Key on `Cargo.lock` hash for invalidation. Expected savings: ~2-3 min per shard (crate download → instant).

### Lever 4: Staging (already landed)

Jarvis's `needs: host-checks` on `neo-test` means QEMU shards don't start until the host job passes. If the host job fails (clippy, fmt), QEMU never runs — saving ~200 min of wasted shard time on a lint failure. **Already implemented, no action needed.**

### Warm-cache summary

| Lever | Effort | Savings (per full gate) | Priority |
|-------|--------|------------------------|----------|
| Save cargo target cache from QEMU shards | Low (add post-step) | ~100 min (4 × 25 min) | **HIGHEST** |
| Persistent registry cache | Low (mount persistent path) | ~8 min (4 × 2 min) | Medium |
| Prebuilt dev image | Already solved | 0 (done) | — |
| Staging | Already landed | Variable (saves full gate on lint failure) | — |

---

## ISC-6: Rollout + guards

### Sequencing (no flag-day)

| Phase | Change | Risk | Rollback |
|-------|--------|------|----------|
| **P1** | Save cargo target cache from QEMU shards (ISC-5 Lever 1) | Low — cache is advisory; miss = cold run as before | Remove post-step |
| **P2** | Add `--tier merge` to xtask + `tier` dispatch input; default all `workflow_dispatch` to merge-tier | Medium — wrong tier selection could drop coverage | Default back to full-tier; the xtask invariant test catches partition errors |
| **P3** | Tag standing-core scenarios (`tier = "core"`); extend `shard-coverage` to verify core partition | Low — tagging is metadata-only | Remove tags, revert to full-tier default |
| **P4** | Add impacted-area selection (diff-driven); wire into merge-tier | Medium — diff-to-scenario mapping must be correct | Disable impacted-area; merge-tier = core only |

Each phase is independently useful and reversible. P1 delivers cache savings immediately. P2+P3 deliver the tiered gate. P4 adds the diff-driven precision.

### xtask invariant tests

Extend the existing shard-coverage test (`cmd_shard_coverage`) to:
1. Verify the **core-tier partition** is complete + disjoint for the configured shard count.
2. Verify **every scenario has a tier tag** (core or full) — no untagged scenarios slip through.
3. Verify the **impacted-area selector** is deterministic (same diff → same selection).

These run in the host-checks job, blocking QEMU. A broken tier definition fails the host job before any shard starts.

### Kill-switch: full-gate-on-every-merge stays one dispatch away

The `workflow_dispatch` input accepts `full-gate: true`. When set, the gate runs the full suite regardless of tier. This is for:
- High-blast-radius changes (consolidation builds, architecture shifts).
- PM discretion before merging a batch of changes.
- Debugging a suspected tier-selection bug.

The workflow job name reflects the tier: `QEMU Integration Tests (FULL shard N/M)` vs `(MERGE-TIER shard N/M)`.

---

## ISC-7: Honest forks + recommendations

### Fork 1: Tier criterion shape — core-only vs core + impacted-area

| Option | Pros | Cons |
|--------|------|------|
| **Core-only** (fixed set, no diff analysis) | Simple, deterministic, fast to implement. Core size is stable. | Misses defects in non-core areas that the diff touches. The merge-tier becomes a "boot smoke test" rather than a meaningful gate. |
| **Core + impacted-area** (diff-driven) | Catches defects in touched code. Scales with diff size — small diffs = fast gate. | More complex. Diff-to-scenario mapping can be wrong (false negatives: touched code not mapped to its scenario). Core can inflate if impacted-area promotes too aggressively. |

**Recommendation: Core + impacted-area.** The core-only option is too narrow — merging a change to the scheduler that breaks a non-core scheduling scenario would pass the gate and fail the nightly, creating the exact escaped-defect workflow we're trying to minimize. The impacted-area adds meaningful coverage for the cost of diff analysis (which xtask already does for `--filter`).

**Mitigation for mapping errors:** Start with a conservative mapping (scenario name prefix → kernel module path). Over-include rather than under-include. A scenario that runs when it doesn't need to costs time; a scenario that doesn't run when it should costs trust.

### Fork 2: Escaped defect — revert-first vs bounded fix-forward

| Option | Pros | Cons |
|--------|------|------|
| **Revert-first** | Maximum safety. No risk of stacking merges on a broken tree. | Burns the merge. Loses context. Forces restart. Slow for the team. |
| **Bounded fix-forward (12h)** | Author fixes the known defect quickly. No lost context. Nightly already contained the blast radius. | Risk: author lands a wrong fix in the hurry. 12-hour window requires someone to be available. |

**Recommendation: Bounded fix-forward, 12-hour window.** The nightly already caught the defect — it's not silently propagating. The author has the most context and can fix it fastest. The 12-hour deadline prevents indefinite deferral. If the author is unavailable (off-hours, travel), the PM can revert on behalf of the team.

### Fork 3: Merge-tier time target

| Target | Feasibility | Fleet dependency |
|--------|-------------|-----------------|
| ≤ 30 min | Aggressive — requires 4+ runners + warm cache + small core (~50 scenarios) | 4th box mandatory |
| ≤ 45 min | Achievable with 4 runners + warm cache + ~70 scenario core | 4th box + cache save |
| ≤ 60 min | Achievable now with 3 runners + merge-tier (~70 scenarios, no cache save needed) | Current fleet |

**Recommendation: Target ≤ 45 min.** This is the "coffee and back" threshold. Achievable after the 4th box lands + cache save (P1). Interim target with current fleet: ≤ 60 min for merge-tier.

### Fork 4: Shard count — fixed vs auto-scaled

| Option | Pros | Cons |
|--------|------|------|
| **Fixed** (update ci.yml when fleet changes) | Simple. No runtime discovery. PM controls the number. | Manual update per fleet change. Easy to forget. |
| **Auto-scaled** (discover runner count at dispatch) | Always optimal. Adapts to fleet changes. | GitHub doesn't expose runner count cleanly. Complex. |

**Recommendation: Fixed, updated by PM.** The fleet changes infrequently. Hard-coding `matrix.shard: [1,2,3,4,5,6]` when the 4th box lands is simpler than runtime discovery. The xtask handles any count correctly (proven to 8).

### Fork 5: Full-gate option per merge — always available or PM-only?

| Option | Pros | Cons |
|--------|------|------|
| **Always available** (any dispatch can set `full-gate: true`) | Coder discretion. No PM bottleneck for high-blast-radius changes. | Coder might skip full-gate when they shouldn't. Extra fleet load if overused. |
| **PM-only** (full-gate requires PM approval or a separate workflow) | Controlled fleet usage. PM judges blast radius. | Bottleneck. PM might not be available. |

**Recommendation: Always available.** The `full-gate: true` dispatch input is self-documenting — the job name says "FULL" and takes longer. A coder who knows their change is high-blast-radius will use it. Fleet load is bounded by the concurrency group (cancels stale runs on the same branch). No PM bottleneck.

---

## Synthesis: the recommended design

1. **Tiered gate:** Merge-tier = standing core (~70 scenarios, tagged `tier = "core"`) + impacted-area (diff-driven). Full-tier = everything. Merge-tier is the default for `workflow_dispatch`; full-tier is `full-gate: true`.
2. **Escaped-defect contract:** 12-hour bounded fix-forward window. Escaped scenario auto-promoted to core (PM-reviewed).
3. **Fleet/shard:** 6 shards when 4th box lands (4 runners). Fixed count, PM-updated. Target: merge-tier ≤ 45 min.
4. **Warm cache:** Save cargo target cache from QEMU shards (P1, ~100 min savings). Persistent registry cache (P2, ~8 min savings).
5. **Rollout:** 4-phase, no flag-day. Each phase independently useful and reversible. xtask invariant tests block bad tier definitions.
6. **Kill-switch:** Full-gate always one dispatch away. Job name encodes tier. Serial log prints tier + scenario count.

---

## Close-gate note

**Docs-only design THINK — no CI run exists.** Paths-ignored per dispatch. `scripts/worker-close-gate.sh` host set locally (skip-with-note). No code, no QEMU, no CI runs fired — pointedly avoiding adding gate load while the gate is the subject.

---

## RETURN

**Artifact:** `docs/THINK-2026-07-12-ci-gate-scaling-prometheus.md` (this file)
**Deliverables:**
1. Cost model from real run data (cold/warm split, TCG multiplier, growth rate) — ISC-1
2. Tiered gate design: core + impacted-area criterion, deterministic and auditable — ISC-2
3. Escaped-defect contract: 12h bounded fix-forward, auto-promote to core — ISC-3
4. Fleet/shard plan: 6 shards at 4 runners, ≤ 45 min merge-tier target — ISC-4
5. Warm-cache levers: save cargo target cache (~100 min savings) — ISC-5
6. Rollout: 4-phase, xtask invariant tests, full-gate always available — ISC-6
7. Forks + recommendations (7 forks resolved) — ISC-7
