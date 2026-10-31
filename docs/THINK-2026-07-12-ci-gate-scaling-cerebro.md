<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: CI gate scaling — the tiered-gate / suite-growth policy (Cerebro)

**Date:** 2026-07-12
**Author:** Cerebro (main-tier coder, Claude family)
**Dispatch:** `Plans/DISPATCH-2026-07-12-ci-gate-scaling-think-prometheus-and-cerebro.md`
**Status:** Design THINK — HOST-ONLY, no code, no QEMU, no CI runs fired. Independent (judge-panel; not read against Prometheus's THINK; the PM synthesizes; the PL ratifies the merge-gate contract).
**Incoming baseline:** Jarvis's B3 CI patch @ `82f33270` (host 30→60 min, QEMU 70→240 min, `neo-test needs: host-checks` staging). Treated as given — not relitigated. This THINK builds the tiered policy *on top of* it: **Jarvis's 240-min ceiling is the correct cost of the FULL/nightly tier; this design adds a cheap MERGE tier and the governance contract around it.**

---

## 0. The one-paragraph thesis

The gate's wall-clock is `waves × (cold_tax + (N/shards) × per_scenario)`, where `N` is the scenario count, `waves = ceil(shards / runners)`, and — measured below — `per_scenario ≈ 2 min` is **~65% per-scenario kernel recompile + ISO build and only ~35% the QEMU run**. Three of those four factors buy a **constant-factor, one-time** improvement: warm caches shrink `cold_tax`, the 4th box removes a wave, more shards shrink `N/shards`. Only `N` grows — **~2 scenarios/day, forever, with pruning FORBIDDEN**. Therefore the only lever that *scales with the problem* is one whose merge-time cost is **independent of `N`**: a merge-blocking tier sized by `core + diff-blast-radius`, with the full `N`-sized suite deferred to the nightly `schedule` that already exists. Everything else in this THINK (warm-cache levers, fleet, shard count) is the constant-factor runway that buys time to land the tier; the tier + its escaped-defect contract is the part that scales.

---

## 1. ISC-1 — the cost model, from real data

All numbers below are from the two dispatch-cited `workflow_dispatch` runs, read from primary job logs (`gh run view --job … --log`), not summarized.

### 1.1 The two runs

**Run `29168424137`** — `origin/main` config (pre-Jarvis-patch: 70-min QEMU, 30-min host, *not* staged), cold cache, 2026-07-11:

| Job | Wall | Verdict |
|---|---|---|
| SPDX License Headers | 33 s | ✅ |
| Monolithic Identity Guard | 50 s | ✅ |
| Host Tests, Format & Lint | 31 m 15 s | ❌ **cancelled** at the 30-min ceiling (cold clippy) |
| QEMU shard 2/4 | 71 m 26 s | ❌ cancelled (70-min ceiling) — wave 1 |
| QEMU shard 3/4 | 71 m 25 s | ❌ cancelled — wave 1 |
| QEMU shard 1/4 | 71 m 15 s | ❌ cancelled — wave 2 (started 22:25, after shard 2 died) |
| QEMU shard 4/4 | 71 m 15 s | ❌ cancelled — wave 2 (started 22:25, after shard 3 died) |

Run wall-clock **2 h 23 m 37 s, every shard cancelled, none finished.** The wave structure is the binding fact: SPDX+Monolithic freed their runners by 21:14:26, but **host-checks held the 3rd runner for its full 31 min**, so wave 1 ran only **2 QEMU shards** concurrently; shards 1 & 4 queued until shards 2 & 3 died. **4 shards on 3 runners = 2 waves of 2.**

**Run `29174345628`** — Jarvis's staged patch (`82f33270`: 240 QEMU / 60 host / `needs: host-checks`), cold, 2026-07-12:

| Job | Wall | Verdict |
|---|---|---|
| SPDX | 33 s | ✅ |
| Monolithic Identity | 53 s | ✅ |
| Host Tests, Format & Lint | **26 m 29 s** | ✅ (fits the new 60-min ceiling; would have died at the old 30) |
| QEMU shards 1–4 | started 01:17:09–12 | (post-host) — **staging confirmed:** every shard launched only after host-checks went green |

This is the evidence for two of Jarvis's baseline numbers: the **~26-min cold host cost** (his "measured 24–26") and that **staging works** (QEMU no longer burns runners on a tree that will fail lint).

### 1.2 Per-scenario cost anatomy (run `29168424137`, shard 2/4, cold — primary log)

Shard 2 was assigned **60 scenarios**. The log gives the exact decomposition:

- **Cold fixed setup, per shard:** job start `21:14:12` → xtask binary running `21:36:46` = **22 m 34 s**. Inside that: the **buildx layer cache MISSED** and rebuilt the QEMU-10.2.3 dev image (`21:22:09`→`~21:31`, **≈ 9 min**), plus checkout, docker plumbing, and the xtask/dependency compile (**≈ 4.5 min**).
- **First scenario `smep_catch` (cold):** xtask-start `21:36:46` → PASS `21:48:40` = **11 m 54 s**, of which the **QEMU run was 34.0 s**. The other ~11 min is the cold compile of the 4 userspace binaries + the first scenario kernel + the ISO build.
- **Warm steady-state (scenarios 2+):** the target cache is now warm within the shard, so each scenario is dominated by its *incremental* kernel rebuild (distinct cargo feature ⇒ real recompile) + ISO build + QEMU run:

  | Scenario | Wall | QEMU run | QEMU share |
  |---|---|---|---|
  | `kaslr_smoke` | 120 s | 38.1 s | 32% |
  | `bare_metal_hhdm_repair_path` | 124 s | 40.0 s | 32% |
  | `ai_latency_oracle` | 229 s | 126.3 s | 55% (a heavy one) |
  | `q7c_shootdown_timeout_panic` | 119 s | 36.1 s | 30% |

  **A light scenario ≈ 2 min; ~35% is the QEMU run and ~65% is the per-scenario kernel recompile + ISO build.**

### 1.3 The decomposition that drives every later recommendation

For a cold 4-shard full gate (`N ≈ 240` default-suite scenarios, per §1.5):

```
gate_wall ≈ waves × [ cold_tax + (N / shards) × per_scenario ]
          = ceil(4/3=2) × [ ~30 min + (240/4=60) × ~2 min ]        (cold)
          = 2 × [ 30 + 120 ] ≈ 300 min   →  the "≈ 4 fleet-hours"
```

- **`cold_tax ≈ 30 min/shard`** = ~9 min dev-image rebuild (recoverable: the cache *missed*) + ~4.5 min xtask compile + ~11 min first-scenario cold compile + checkout/docker. **Almost all of this is cache-recoverable** (ISC-5).
- **`per_scenario ≈ 2 min`, ~65% compile / ~35% QEMU.** This is **not** cache-recoverable across scenarios (each scenario is a distinct kernel feature) and is **largely TCG-independent** (the compile two-thirds does not care about emulation).
- **`waves = 2`** because 4 shards contend for 3 runners.

### 1.4 The TCG multiplier — smaller than it looks

Under pure TCG a light scenario *boots and runs* in ~35–40 s; the same boot on bare metal is ~2–3 s, so **TCG ≈ 10–15× on the QEMU-run third**. But because the QEMU run is only ~35% of per-scenario wall-clock, the **suite-level TCG tax is diluted** — roughly `0.35 × (1 − 1/12) ≈ 32%` of per-scenario time is "TCG overhead we can't buy back without KVM." KVM is unavailable (the runners are themselves inside WSL2; nested virt is off), so this third is a floor, not a lever. **The compile two-thirds is the larger and more actionable cost.**

### 1.5 Suite size and growth rate (measured)

Registry today: **264 `TestScenario` entries**; the default CI suite is **~240** after the `ignored`, 7 hardware-only (`is_hardware_only_scenario`), and 2 explicit-only (`is_explicit_only_scenario`) filters. Growth from `git` history of `tools/xtask/src/main.rs`:

| Date | `main~N` | Scenarios |
|---|---|---|
| 2026-06-07 | ~800 | 194 |
| 2026-06-17 | ~500 | 223 |
| 2026-06-23 | ~300 | 242 |
| 2026-07-07 | ~150 | 256 |
| 2026-07-11 | HEAD | 264 |

**+70 scenarios in 34 days = 2.06 scenarios/day (+36%/month), roughly linear, and monotone** (the standing rule forbids pruning — "discriminators stay armed permanently"). Each new scenario adds ~2 min warm to whichever shard it lands on, i.e. ~0.5 min to gate wall-clock. **The suite doubles within a quarter, and the full gate's cost doubles with it.** This is the fact that makes constant-factor fixes insufficient and tiering necessary.

### 1.6 Weight model (already cost-aware)

`scenario_shard_weight = scenario_timeout(name)` (default 180 s; heavies 420–900 s for the accel/usb/wifi/arthur-crypto scenarios; the real-brain 3600 s scenarios **skip in CI** and are weighted 1/nominal so they don't distort the balancer). `arthur_memory` and the reboot-persistence family are ×2 (two-boot). `shard_partition` is a **deterministic LPT greedy** cost-weighted bin-pack, conflict-group-aware (the `arthur_memory` family can't share a shard) with a run-late tail bucket, and it is already **N-parametric** (`shard_count_one_is_identity`, `shard_partition_handles_more_shards_than_scenarios`). `shard-coverage N` proves the partition is complete + pairwise-disjoint — the sharding analog of a byte-oracle. **This machinery already supports everything ISC-4 needs; 4→N is a matrix change, not new partition code.**

---

## 2. ISC-2 — the tiered gate (the core design)

Two tiers, both driven by the *same* xtask registry so they can never disagree about what a scenario is:

- **MERGE tier** (blocking, on `workflow_dispatch` — the coder/PM pre-merge gate): `core ∪ impacted-area`.
- **FULL tier** (the anti-rot net, on the existing `schedule` nightly + a `workflow_dispatch` full option): the entire ~240-scenario suite, at Jarvis's 240-min ceiling. Unchanged from today except that it stops being the *per-merge* gate.

### 2.1 Membership criterion — principled, deterministic, auditable (ISC-2a)

Membership is computed by xtask, not by a human eyeballing a diff:

**(1) The standing core — `scenario_is_core()`.** A *named, reviewed allowlist* of scenarios that run on **every** merge regardless of the diff. It is deliberately minimal-but-complete:
- every boot/smoke invariant (`ipc_heap_kill_smoke`, `smep_catch`, `smap_catch`, `kaslr_smoke`, `bare_metal_hhdm_repair_path`, the console/liveness pair);
- **every governance-invariant discriminator (I-1…I-15)** and **every I-14 fault-containment discriminator** (lock isolation, liveness-degradation, DMA-fence/VT-d) — these are load-bearing for the whole AI-as-kernel thesis and must never be scoped out;
- **every kill-switch equality regression** (the complexity-governance rule: disabled ≡ prior layer);
- `ot49_idle_entry_wake`, `ai_latency_oracle`, and the caps/revoke core (`cap_derive_missing_meta`, `cap_revoke_cross_process`, `file_cap_derive_grant`, `sys_inspect_rights_gate`).

The core is an *allowlist with a rationale per entry*, pinned by an xtask invariant test (see ISC-6). Size target: ~40–50 scenarios (it does **not** grow with `N` — it grows only when a genuinely new cross-cutting invariant is added).

**(2) The impacted-area selection.** Add one field to `TestScenario`:

```
areas: &'static [Area]     // Area ∈ { Boot, Mm, Sched, Ipc, Caps, Governance,
                           //          Neofs, Storage, Net, Usb, Wifi, Accel,
                           //          Arthur, Userland, Build }  (enum, exhaustive)
```

and a **diff→area map** — an ordered list of path-prefix rules (`kernel/src/mm/** → Mm`, `kernel/src/sched/** → Sched`, `kernel/src/net/** → Net`, `kernel/src/governance/** → Governance`, `userland/** → Userland`, `tools/xtask/** → Build`, …). Given the merge diff's changed paths, xtask computes `impacted = ⋃ areas(path)`; the merge tier is `core ∪ { s : areas(s) ∩ impacted ≠ ∅ }`.

**(3) Fail-safe, not fail-silent.** The map is **total and closed**: every kernel/userland top-level directory MUST map to ≥1 area (an xtask test asserts this), and **any changed path that matches no rule promotes to "impacts everything" ⇒ run the FULL suite at merge.** A coverage gap is therefore *loud and expensive*, never a silent green. Adding a new subsystem directory without a map entry does not skip its scenarios — it forces a full gate until someone adds the mapping. This is the same doctrine as the hermeticity invariant: a green must never come from an omission.

**Why this shape scales:** the core is fixed; the impacted set is bounded by the diff's blast radius, **not** by total suite size `N`. A typical single-subsystem diff touches 1–2 areas ⇒ merge tier ≈ `core (≈45) + one or two areas' scenarios (≈15–40)` ≈ **60–85 scenarios today, and *still ≈ 60–85 when N has doubled***, because doubling `N` mostly adds scenarios to areas a given diff doesn't touch. That `N`-independence is the whole point.

### 2.2 Everything stays armed somewhere (ISC-2b)

The FULL suite runs **nightly** on the existing `schedule` trigger (cron `0 8 * * *`, on `main`) at the 240-min ceiling. No scenario is ever removed; a scenario scoped out of a given *merge* is still run within ≤24 h on `main`. The anti-rot rule is satisfied by construction: the merge tier *selects*, it never *deletes*.

### 2.3 No misleading green (ISC-2c)

- The merge-tier CI job is **named for what it is** — e.g. `QEMU Merge Tier (core + impacted: mm, sched)` — so a green check never reads as "all tests passed."
- xtask prints the tier manifest at run start and the **PM's merge report states the tier explicitly**: `"merge-tier: core(46) + impacted[mm,sched](31) = 77/240 scenarios; full suite runs nightly (last green nightly: <run-url>)."`
- A `shard-coverage`-style gate proves the merge tier is a **superset of core** and that the printed manifest equals what actually ran (no drift between "what we said we ran" and "what ran"). Same falsifiable-property discipline as the existing complete+disjoint proof.

---

## 3. ISC-3 — the escaped-defect contract (the governance half — PL ratifies)

This is the part that changes what "merged" *means*, so it is stated precisely for PL ratification.

### 3.1 What a green MERGE tier means (and does not)

> **Green merge-tier ⇒** "every standing-core scenario, plus every scenario covering the kernel areas this diff touches, passed at this exact tree." **It does NOT mean** "the full suite passed." It is a **scoped** correctness proof: complete over `core ∪ impacted`, silent over the rest until the nightly.

The PM's merge report must carry the tier and the last-green-nightly URL (§2.3) so the reader knows exactly what was and wasn't proven.

### 3.2 When the nightly reddens `main` — revert-first, bounded fix-forward (the PL decision)

Because the merge tier is a subset, a defect *can* land on `main` and only redden the nightly. The contract:

1. **Localize.** The nightly runs on `main`; the culprit is in `git log <last-green-nightly>..<red-nightly>`. The red scenario names its area; the area names the merges in that range that touched it. This is usually a 1–3 commit range — `git bisect` over the nightly scenario closes it fast.
2. **Default = revert-first.** `main` must always be *return-to-green-able within one action*. The culprit merge is reverted unless (3) applies. Revert-first is the default because it protects the *next* coder branching off `main`.
3. **Bounded fix-forward escape hatch.** If the owning coder can land a rebased, gate-green fix within a **stated window (recommend: 4 working hours / same working day)**, fix-forward is allowed instead of revert. Past the window ⇒ revert; re-land with the fix. The window is a PL-set number.
4. **Ownership.** The coder whose merge introduced the escaping area owns the fix; the **PM triages "who owns" within the window** and notifies the PL on any `main`-red. A nightly red is not a fire drill for everyone — it is a named owner + a clock.
5. **Infra-red exception.** A nightly red from runner contention/OOM/disk (the runbook's INFRA class) is **not** an escaped defect — it's re-fired on a drained box, per OT-35 doctrine. The contract applies only to a *reproducible* scenario failure.

### 3.3 The promote-the-escaped-scenario feedback loop (anti-fragile)

Every escape is a **proof that the selection missed**, so the selection is tightened, permanently:

- When a defect escapes the merge tier and reddens nightly, its scenario is either (a) added to the **standing core**, or (b) the **diff→area map is corrected** so that the *class of diff* that introduced it would have selected it (the more common, better-targeted fix). Which one is a judgment call recorded in the fix commit.
- An xtask test pins the new core entry / map rule so the tightening cannot silently regress.
- **Consequence:** the merge tier monotonically converges toward "catches everything that has ever escaped," while staying `N`-bounded — it grows by *escapes*, not by *suite size*. This is the exact mirror of the byte-oracle/discriminator discipline: a miss becomes a permanent, armed check.

### 3.4 Kill-switch (governance requirement)

Per the complexity-governance kill-switch rule, ship a `merge_tier_enabled` flag whose **disabled state is provably ≡ the full gate, bit-for-bit** (when disabled, `merge_tier = full suite`). "Turn off tiering" is then a one-line revert that restores today's full-gate-per-merge exactly — the entire policy is one action away from the status quo the PL already trusts.

---

## 4. ISC-4 — parallelism + fleet

### 4.1 The free win hiding in the wave math

From §1.1: **4 shards on 3 runners = 2 waves.** Wall-clock is `2 × slowest_shard`. But:

```
4 shards on 3 runners:  2 waves × (60 scen × 2 min) = 2 × 120 = 240 min wall   (2 waves)
3 shards on 3 runners:  1 wave  × (80 scen × 2 min) = 1 × 160 = 160 min wall   (1 wave)
```

**Matching shard count to *runner* count (3, not 4) removes a wave and cuts full-gate wall-clock ~33%** — at the cost of fatter individual shards (160 min > some ceilings, but well under Jarvis's 240). This is a genuine fork (§7): over-sharding past runner count *re-serializes into waves*, which is what the current 4-on-3 does. The instinct "more shards = faster" is wrong once shards exceed runners.

### 4.2 The 4th box is the highest-ROI fleet action

The Ryzen M75q-1 is a **separate machine** ⇒ a true 4th concurrent runner (the current 3 all share one 8-core i9 and top out at ~3 smooth shards — runbook). Racking it turns **4 shards into 1 wave**, roughly halving the full-gate wall-clock, with **zero coverage change and zero code**. Recommendation: **rack the 4th box before adding shards.** With 4 real runners, 4 shards run in one wave; the shard count = runner count = 4 invariant is restored.

### 4.3 Per-runner-label guard thresholds

The Ryzen box has a different core count / RAM profile than the i9, so it needs its **own** pre-flight floor, not the global 32 GiB (the runbook already anticipates this — "a per-runner-label guard threshold, not the global 32 GiB floor"). Design: the RAM/disk pre-flight reads a **per-label** required value (a small `case "$RUNNER_LABELS"` table), so a smaller box asserts its own honest floor and a mis-sized shard on it fails fast with correct attribution instead of thrashing.

### 4.4 Should shard count auto-scale with the fleet?

**Recommend: shard count == count of runners that can hold a QEMU shard smoothly** — i.e. one wave, no queuing — surfaced as a *single* value that the CI matrix, the `shard-coverage N` gate, and the partition all read. It's 3 today (i9 caps at 3), 4 with the Ryzen, more when a second real machine lands. Do **not** auto-scale shards *above* runner count — that manufactures waves (the 4-on-3 bug). The partition is already N-safe, so changing the number is a one-line matrix edit + keeping the `shard-coverage` N in lockstep (an xtask test already guards drift).

### 4.5 Fleet target — holding the merge tier as `N` doubles

**Stated target: merge tier ≤ 45 min end-to-end.** Achievable math (warm caches per ISC-5, merge tier ≈ 40 scenarios on 3 shards, run **host ∥ merge-QEMU** rather than staged — see §4.6):

```
merge_tier_wall ≈ max( host_checks_warm≈10min , (40/3≈14 scen × 2min ≈ 28min) ) ≈ 28–30 min  (warm)
```

Crucially, **this target holds as the FULL suite doubles**, because the merge tier is `core + diff-blast-radius` (§2.1) — `N`-independent. The nightly full tier absorbs the growth (and is where the 4th box + warm caches keep it under the 240 ceiling for longer). The fleet needs to hold *the merge tier* under 45 min and *the nightly* under 240; only the nightly fights growth, and it fights it once per day on a drained box, which is exactly what a nightly is for.

### 4.6 Staging: keep for FULL, drop for MERGE

Jarvis's `needs: host-checks` staging is right for the FULL tier (don't spend scarce QEMU runners on a tree that fails lint — the waste is large when QEMU is 240 min). For the *small* merge tier, prefer **host-checks ∥ merge-QEMU** (parallel), so merge wall-clock is `max(host, qemu)` not `host + qemu`. The wasted-runner risk is small (merge-QEMU is short) and the latency win is large (this is what makes the ≤45-min target reachable). Not a relitigation of Jarvis's patch — a *tier-specific* choice layered on it.

---

## 5. ISC-5 — the warm-cache levers (minutes without touching coverage)

From §1.3, `cold_tax ≈ 30 min/shard` and it is *almost entirely cache-recoverable*. Do these **first** — they are pure speedups, reversible, no policy change, and they may buy enough runway that tiering is about *scaling* rather than *firefighting*:

1. **Prebuilt dev image, pulled not rebuilt (biggest, most deterministic).** The buildx cache **missed** in run `29168424137` and rebuilt QEMU-10.2.3 for ~9 min. Publish `neoplutarch-dev:<Dockerfile-hash>` to GHCR on Dockerfile change; CI `docker pull`s it. Deterministic hit, no per-run rebuild race. **~9 min/shard → ~30 s.** (The current Dockerfile-keyed local cache is the right idea but still loses on cold runners / cache eviction / a cancelled job skipping the save — a registry pull removes all three failure modes.)
2. **Persistent per-runner cargo `target/` volume (not restore-only-from-actions-cache).** Today the cargo cache is *restore-only* because self-hosted `target/` uploads run 30+ min after a passing shard. A **local named Docker volume** per `(runner, shard)` sidesteps the upload entirely: `target/` survives across runs on the box's own disk, so the **first scenario drops from ~12 min cold to ~2 min warm.** ~10 min/shard. (Disk hygiene: bound it with the existing prune discipline; it trades disk for minutes, which the runbook already manages.)
3. **`sccache` spike (the per-scenario third).** Each scenario recompiles the kernel under a distinct feature; a shared compilation cache *might* cache the common crate-dep objects across scenarios and shave the ~65% compile portion of `per_scenario`. Unknown whether feature-gating busts the cache too often to help — **worth a bounded spike**, not a commitment. This is the only lever that touches the per-scenario floor rather than the one-time cold tax.
4. **Staging (landed).** Already avoids QEMU spend on lint-failing trees — keep for FULL (§4.6).

Combined, (1)+(2) take `cold_tax` from ~30 min to **~8 min/shard**, which alone pulls a cold full gate from ~5 fleet-hours toward ~3 and buys months of growth runway. **All coverage-neutral.**

---

## 6. ISC-6 — rollout + guards (no flag-day)

Sequence so nothing is a cliff and every step is one revert from the last:

1. **Land the ISC-5 warm-cache levers.** Pure speedup, no policy change. Verifiable independently.
2. **Add the taxonomy to xtask** — the `areas` field, `scenario_is_core()`, the diff→area map, and the fail-safe. Ship with **xtask invariant tests** (Jarvis's `shard-coverage` precedent):
   - every scenario has ≥1 area; every kernel/userland top-level dir maps to ≥1 area (map totality);
   - `core` is non-empty and every core scenario exists;
   - `merge_tier(diff) ⊇ core` for all diffs; an unmapped path ⇒ full suite;
   - `merge_tier_enabled = false ⇒ merge_tier == full suite` (the kill-switch equality).
3. **Shadow mode (the no-flag-day).** Add the merge-tier job **alongside** the existing full gate for a burn-in (recommend ~2 weeks): the full gate stays blocking; the merge tier runs advisory. Measure the **escape rate** — how often would the merge tier have gone green while the full gate caught something. This produces the evidence the PL needs for ISC-3 ratification (an actual number, not a hope).
4. **Flip after burn-in.** If the escape rate is acceptable, make the merge tier blocking on `workflow_dispatch`; move the full suite to `schedule` (nightly) + a `workflow_dispatch` full option (§7.5). The nightly is already wired.
5. **Kill-switch stays armed.** `merge_tier_enabled=false` (or a CI-config revert) restores full-gate-per-merge, bit-for-bit (§3.4). The tier definitions are pinned by the step-2 tests so they cannot silently rot — the same "falsifiable maintainability property" discipline as the invariant ledger.

---

## 7. ISC-7 — honest forks + recommendation

| # | Fork | Options | Cerebro's call | Why |
|---|---|---|---|---|
| 7.1 | **Tier criterion shape** | (a) `core ∪ impacted-area`; (b) fixed core-only subset; (c) instrumented/coverage-guided selection | **(a)** | Principled + auditable + `N`-bounded. (b) can't adapt to where a diff actually lands (misses cross-area regressions or over-runs). (c) is brittle magic in a merge-blocking path — the runbook already rejected leaky path-detection for the main-push case. |
| 7.2 | **Nightly-red on `main`** | revert-first / bounded fix-forward / unbounded fix-forward | **revert-first default + bounded (≈4 h) fix-forward, culprit-owned** | `main` must be one action from green for the next brancher; the bounded window respects "forward momentum" without letting `main` sit red. **PL ratifies the window.** |
| 7.3 | **Merge-tier ceiling** | pick a target | **≤ 45 min end-to-end** (host ∥ merge-QEMU, warm) | Reachable per §4.5, holds as `N` doubles, keeps the coder's inner loop tight. **PL sets the exact number.** |
| 7.4 | **Shard count** | match-to-runners (fewer/fatter, 1 wave) / over-shard (more/thinner, multi-wave) | **match to runner count** (3 now → 4 with the Ryzen) | §4.1: over-sharding past runner count re-serializes into waves. Fatter shards need the 240 ceiling, which we already have. |
| 7.5 | **`workflow_dispatch` full-gate-per-merge for high-blast-radius** | keep / drop | **KEEP** | A change to a governance invariant / I-14 / an ABI / the scheduler should run the FULL gate at merge, not just core+impacted. Expose it as a `workflow_dispatch` `full` input (or a `[full-gate]` merge-commit trailer the PM sets). This *is* the kill-switch's manual, per-merge form — cheap insurance for the diffs where a subset is genuinely not enough. |
| 7.6 | **Standing-core size** | minimal-complete / broad ("all of phase X") | **minimal-complete** | Boot + every governance/I-14 discriminator + every kill-switch equality + caps core. Not phase-bucketed. Reviewed allowlist with per-entry rationale, pinned by test. |

**The single PL-ratification item is ISC-3** — the escaped-defect contract, because it redefines what "merged" means (scoped proof at merge, full coverage deferred ≤24 h to nightly, revert-first with a bounded owned window, and every escape permanently tightening the selection). Everything else is PM-synthesizable.

---

## 8. Convergence note (for the PM's cross-model synthesis)

This is the Claude-family half of a Qwen+Claude judge panel. The load-bearing claims a cross-model check should either corroborate or refute:
1. **per_scenario is ~65% compile / ~35% QEMU** (⇒ TCG and warm-caching are *diluted* levers; the compile-per-scenario is the real floor). — measured, §1.2.
2. **The merge tier's size is `N`-independent** (`core + diff-blast-radius`), which is *why* it scales where sharding/caching/fleet (all constant-factor) don't. — the central design claim, §2.1/§4.5.
3. **4-shards-on-3-runners is a self-inflicted extra wave**; match shards to runners. — §4.1.
4. **Fail-safe selection** (unmapped path ⇒ full suite) is what keeps "no silently-dropped coverage" honest. — §2.1.
5. **Every escape tightens the selection** (promote-to-core / fix-the-map), making the tier anti-fragile rather than a permanent coverage hole. — §3.3.

If Prometheus's independent THINK lands on (2) and (5) by a different route, that convergence is real cross-model evidence for the tier + contract; divergence on (1) or (3) is worth the PM digging into before synthesis.
