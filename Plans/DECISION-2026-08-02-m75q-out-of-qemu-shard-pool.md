<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DECISION: the M75q leaves the QEMU shard pool — **PL-ratified 2026-08-02**

**Directive (PL, verbatim):** *"taking the M75q out of the pool for QEMU shards"*

## The evidence that forced it (Oracle, OT-74 — measured, not inferred)

`sys_generate_kv_persist_equivalence`, 25 runs / 58 shard-jobs / 284 scenarios:

| host | attempt-1 failures | ever completed? |
|---|---|---|
| main Ryzen 9 box (`ryzen-1..4`) | **0 / 5** | yes, 84.5 → 116.6 s |
| **M75q** (`ryzen-5/6`, `<runner-host>`) | **4 / 4** | **never — 0 of 16 attempts** |

**On the M75q this is not flaky. It is deterministic failure.** Mechanism is capacity, not a race: main-box completions spread 84.5–116.6 s against a **180 s** budget — only **1.5–2.1× headroom** — and the M75q is roughly **2× slower**. It doesn't *sometimes* miss; it *cannot fit*. The memory fix did not change this (it failed again post-fix on `ryzen-6`, 2026-08-02) because memory was never the binding constraint.

## Why removal rather than a budget increase

A runner that deterministically fails a gate scenario is **not adding capacity — it is adding a 4-in-N chance of a false red on every gate**. That is strictly worse than not having it: each false red costs a ~35-minute re-fire on the binding constraint, and it trains everyone to read reds as noise, which is how a real escape gets waved through.

Raising the budget instead would make **every** gate wait longer to accommodate a half-speed box — paying the tax on all runs to rescue one runner.

**Note the contrast that made this decidable**, and why the two flaky scenarios must not be treated as one problem: `arthur_reversibility` is a **race** (19 attempts, completions 7.1–16.8 s, failures pinned at *exactly* 420 s, zero intermediate values — a 25× gap, and 3/11 hangs on the *good* box), so a budget increase there is **useless by construction**. `sys_generate_kv_persist_equivalence` is **capacity** — a real distribution against thin headroom. Same symptom, opposite remedies. Oracle established the distinction by **killing his own bimodality claim** for the second scenario after asserting it from n=2.

## The change — label-gated, in this order

**The M75q stays in the fleet.** It has been reliably green on SPDX, monolithic-identity and host-checks; it is excluded from **QEMU shards only**.

**Step 1 — PL, on the runner hosts (MUST come first).** Add the label **`qemu-shard`** to `ryzen-1`, `ryzen-2`, `ryzen-3`, `ryzen-4` **only**. Do **not** add it to `ryzen-5` or `ryzen-6`. Easiest via GitHub → *Settings → Actions → Runners → \<runner\> → add label*.

**Step 2 — repo, one line.** `.github/workflows/ci.yml:63`, the `neo-test` job:
```yaml
-    runs-on: self-hosted
+    runs-on: [self-hosted, qemu-shard]
```
**Leave `runs-on: self-hosted` unchanged at lines 336 and 383** (SPDX, monolithic-identity) so the M75q keeps serving those.

**⚠ Order is load-bearing.** If step 2 lands before step 1, **no runner carries the label and every QEMU shard queues indefinitely** — a self-inflicted total gate outage. Labels first, then the workflow change.

## What this does not fix
- **`arthur_reversibility` is untouched** — it is a race on the *good* hardware (**37.5%** attempt-1 failure there) and removing the M75q does not address it. **OT-74 stays open.**
- **Six other scenarios fail attempt 1 on the main box with zero M75q involvement** — `arthur_multistep_agentic`, `arthur_memory`, `virtio_accel_identity_roundtrip`, `sched_spawn_reap_spawn_heavy`, `phase3_kthread_detached_worker_smoke`, `arthur_real_brain_boot_ot16_walker`. Capacity cannot explain those.
- **`threads_futex` is the worst measured at 2/3 = 66.7% with 2 reds** — and it is the **open C1.1 lane's own scenario**, so that lane is currently gating against a test that fails attempt 1 two thirds of the time.

## Fleet capacity consequence, stated plainly
QEMU shard capacity drops from **6 runners to 4**. Since a 4-shard gate wants 4 concurrent slots, **one gate now saturates the pool** and concurrent gates serialize completely. That is the honest cost, and it is accepted: 4 runners that produce trustworthy verdicts beat 6 that produce a false red on a coin flip.
