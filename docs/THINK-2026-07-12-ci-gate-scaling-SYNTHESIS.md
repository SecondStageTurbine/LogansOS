<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: CI gate scaling — SYNTHESIS of the two independent designs (Prometheus × Cerebro → the ratifiable policy)

**Date:** 2026-07-12
**Author:** PM (Claude)
**Inputs:** `docs/THINK-2026-07-12-ci-gate-scaling-prometheus.md` (panel A, Qwen, `5ea9034`) · `docs/THINK-2026-07-12-ci-gate-scaling-cerebro.md` (panel B, Codex-seat/Claude-family, `0252cc4`) — produced under judge-panel independence (neither read the other), from the shared measured baseline (the B3 gate saga runs + Jarvis's `82f33270` CI patch).
**Status:** PM synthesis. The convergent core is **cross-model evidence** (Qwen + Claude-family, PM-synthesized) and is treated as settled. **One divergence — the escaped-defect window — is the single PL-ratification item.**

---

## 0. One breath
Two independent designs converged on the entire architecture: **a two-tier gate driven by one xtask registry — a merge-blocking tier (`standing core ∪ diff-impacted-area`) + the existing nightly full suite — where the merge tier's size is bounded by the diff's blast radius, not by total suite count `N`.** Both derived the same membership criterion, the same fail-safe (an unmapped path forces the full suite — "a green must never come from an omission"), the same escaped-scenario→core feedback loop, the same warm-cache-first ordering, and the same auto-tag-new-scenarios-full-until-promoted guard against core inflation. **They diverge on exactly one number: the fix-forward window (Cerebro 4h→revert-default; Prometheus 12h→revert-fallback).** That, and only that, is what the PL ratifies.

## 1. The convergent core (settled)
| # | Both independently designed | Note |
|---|---|---|
| C1 | **Two tiers, one registry:** merge-blocking (`core ∪ impacted`) + nightly full — the full suite never stops running, it stops being the *per-merge* gate | The anti-rot rule preserved: everything armed somewhere |
| C2 | **`N`-independence is the whole point:** the merge tier is bounded by diff blast radius; doubling `N` mostly adds scenarios to areas a given diff doesn't touch | This is the only lever that *scales with the problem*; everything else is constant-factor runway |
| C3 | **Membership = a reviewed standing core + a deterministic diff→area/path selection**, computed by xtask, printed at shard start, auditable | Both: core ≈ 60–80 (incl. every I-14 discriminator + kill-switch equality + boot/caps floor) |
| C4 | **Fail-safe, not fail-silent:** a changed path matching no area/scenario mapping **promotes to the full suite** — a coverage gap is loud and expensive, never a silent green | Both independently reached this — the strongest corroboration; it's the hermeticity doctrine applied to tiering |
| C5 | **No misleading green:** tier encoded in the job name + serial header + merge report ("MERGE-TIER: N scenarios") | A green merge-tier means "core + impacted clean," stated, never "everything passed" |
| C6 | **Escaped-scenario → core promotion** (self-healing: if a scenario fails independent of the diff's area, it IS a core invariant), PM-reviewed to prevent one-off inflation | Both; new scenarios default full-tier-only until promoted |
| C7 | **Warm-cache levers first, policy second:** the measured cold cost is dominated by compile/rebuild, largely cache-recoverable; do the cheap infra minutes before spending policy | (see §3 — the one place their cost models differ, reconciled below) |
| C8 | **Shard count scales with the fleet** (xtask partition already proven complete+disjoint to 8; cost-weighted); the 4th box removes a wave; merge-tier target ~45 min | Both |

## 2. The divergence — the escaped-defect window (the PL decision)
Both agree on revert-first-vs-fix-forward as the axis and on named-owner + a clock; they differ on the number:
- **Cerebro (panel B): revert-first is the DEFAULT; fix-forward is the escape hatch within ~4 working hours**, else revert. Rationale: `main` must be one action from green to protect the *next* coder branching off it.
- **Prometheus (panel A): fix-forward within a 12-hour bounded window is the default; revert is the fallback at 12h.** Rationale: revert burns the merge and loses context; the defect is already caught by nightly, not silently accumulating.

**PM recommendation: adopt Cerebro's revert-first default with a *shortened* fix-forward hatch (recommend 4 working hours), for a reason neither fully weighed — the N=1 branching topology.** Coders branch off `main` constantly; a red `main` taxes *every* seat that rebases during the window (the stale-base tax this project already fights). Revert-first keeps `main` continuously branchable — the cheaper default in a many-coders-one-main shop. Prometheus's "revert loses context" cost is real but smaller: the reverted work sits on its branch, re-landed with the fix; nothing is lost but a merge/re-merge cycle. **So: revert-first default; fix-forward only if the owning coder lands a rebased, gate-green fix within 4 working hours; past that, revert-and-reland. Infra-reds (runbook class) are exempt — re-fired, never counted as escapes.** This is the ratifiable contract; the 4h number is the PL's dial.

## 3. Reconciling the one factual difference (cost model)
The panels' cost models agree on structure and diverge on one figure worth recording honestly, because it changes the *expected payoff*:
- **Cerebro:** per-scenario cost is **~65% compile / ~35% QEMU**; suite-level TCG tax diluted to ~32%; "**the compiler, not the emulator, is the expensive part**" ⇒ warm caches buy a lot.
- **Prometheus:** the binding constraint is **QEMU execution (~85% of a cold shard)**; caching saves only ~15%.

These are measured from *different cuts* (per-scenario vs whole-cold-shard incl. the one-time image/xtask build), so they're not strictly contradictory, but the payoff of the warm-cache levers hinges on which dominates. **This is the single load-bearing empirical uncertainty in the whole policy, and it is cheaply resolvable:** instrument one shard to emit per-phase timing (compile-seconds vs QEMU-run-seconds per scenario) on the next full nightly and settle it with data. **That instrumentation is the recommended FIRST build slice of the eventual OT** — measure before investing in caching vs sharding. (This is itself the project's own discipline: don't optimize on an un-measured model.)

## 4. The buildable shape (what the OT dispatch specifies)
Merged from both, smallest-first:
- **Slice 0 (measure):** per-phase shard timing instrumentation → one nightly → settle §3. Cheap, decides the ordering of everything after.
- **Slice 1 (tier plumbing):** `tier`/`areas` fields on the scenario struct + `scenario_is_core()` allowlist (I-14 discriminators auto-core) + the total-and-closed diff→area map + `--tier merge` in xtask + the merge-tier `shard-coverage` complete+disjoint check. Host-testable.
- **Slice 2 (workflow):** `workflow_dispatch` runs merge-tier by default, `full-gate:true` option retained, nightly `schedule` = full unchanged; tier in job name + serial header + merge report; xtask invariant tests pin the tier definitions + the map's totality (Jarvis's precedent).
- **Slice 3 (contract):** the escaped-defect contract in the runbook + CLAUDE.md (revert-first / 4h fix-forward / promote-escaped-to-core), and the nightly-red owner-triage flow.
- **Slice 4 (warm-cache, ordered by Slice 0's data):** persistent cargo target/registry caches, prebuilt dev image; shard-count-scales-with-fleet.
- Kill-switch: the full-gate-on-every-kernel-merge behavior stays one revert/`full-gate:true` away.

## 5. Status
This synthesis is the ratifiable policy. **The PL ratifies exactly one thing now: the escaped-defect contract (revert-first + a fix-forward window — PM recommends 4 working hours).** Everything in §1 is cross-model-convergent and settled; §3's cost question is settled by Slice-0 measurement, not by decree. Bank as a new OT (the CI-gate-scaling policy + its build slices), sequenced after B3 clears the runners — and note that Jarvis's 240-min patch is the correct interim tourniquet that this policy retires.
