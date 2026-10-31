<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PLAN: OpenCode open-weight-coder A/B — task-fill + scorecard (PM complement to JC's handoff)

**Date:** 2026-06-19
**From:** PM (Claude)
**Complements:** JC's "AltimitOS — Open-Weight Coder A/B Test" handoff. That handoff is the authoritative spec (models, wiring, controls, rubric). This fills the two PM-shaped gaps: **§4 (real repo tasks)** and the **fillable scorecard**, plus one test-design improvement and the execution/scoring boundary.

## 0. Boundary (who does what)
- **JC / operator:** subscribe to OpenCode Go; confirm the model IDs in the picker; run every task × model × run (fresh side branch off the fixed base, OpenAI-compatible path, constant reasoning preset); keep prompting style constant (one operator).
- **PM (Claude):** authored the tasks + scorecard (this doc); **does the post-run scoring** of the candidate diffs — `Unsafe correctness`, `no_std discipline`, `Diff quality` are exactly the verify-before-merge review the PM already does on every worker return. Hand me each model's final diff + turn log per run and I score it against the rubric and the ground-truth.
- **Not in scope mid-task:** no Claude/Jarvis assistance *during* a run (handoff §6). Designing the harness and scoring the outputs is the evaluation, not assistance.

## 1. The one test-design improvement: use KNOWN-ANSWER tasks
The handoff's task *shapes* are right. The improvement: make **B and C reproduce real, already-solved repo work from a base commit *before* the solution landed.** Two payoffs:
1. **Objective scoring.** We have the *real* diff/fix as ground truth, so "did the model get it right" is a comparison, not a judgment call.
2. **No critical-path risk.** The real work is already merged; these are throwaway eval branches. A model's wrong answer costs nothing.
**Hard control:** the operator must **not load the solution commit (or anything downstream of it) into context** — run on the pinned pre-fix base, answer withheld. Leaking the fix invalidates the run.

## 2. Proposed tasks (PM-proposed — JC confirm or swap)

### Slot A — self-contained primitive (algorithmic precision + `unsafe` in isolation) → expected to favor DeepSeek
**Task:** Implement an **AVX2 SIMD path** for a scalar `ai/ops` routine that lacks one (recommend the **q8_K quantizer** or a `soft_f32` hot routine), behind a **CPUID + XCR0 runtime gate**, validated **bit-identical (integer) / within-2⁻¹² (float) against the existing scalar implementation as the oracle.**
- **Base:** current `main`. **Ground-truth/quality reference:** the merged 9.3 `qk_matmul` AVX2 build (`16c7ffb`) shows exactly what a correct CPUID/XCR0-gated, `target_feature`, SAFETY-documented AVX2 op looks like — score against that pattern.
- **Scoring anchors:** the bit-identical/within-tol KAT is the objective gate; `unsafe` axis = the intrinsics + the OS-feature gate (the classic trap is checking the AVX2 CPUID bit but **not** XCR0 — 9.3 got it right); `no_std` = stays kernel-pure.
- *Alt:* reproduce the **9.1 scalar q4_K×q8_K integer-dot** from spec (base before `c7f7e5d`; ground-truth = 9.1) — lighter on `unsafe`, heavier on the i32-overflow/scale-depack precision.

### Slot B — cross-file refactor (engineering coherence) → expected to favor GLM / Kimi
**Task:** Reproduce the **Phase-9.2 quantized-matmul wiring**: add a `matmul_vec_weight()` that selects an optional quantized-weight path behind the `phase9-quantized-matmul` kill-switch, and **thread the optional raw weight through every per-layer matmul call site** (attn q/k/v/o, ffn gate/up/down, output) in `kernel/src/ai/forward_logic.rs` **without changing the f32 path.**
- **Base:** `89f134e` (the commit before the 9.2 merge). **Ground-truth:** the real 9.2 diff (`93822e6`).
- **Scoring anchors:** the **kill-switch invariant is the objective gate** — feature-OFF must be the f32 path **bit-for-bit** (the 8.5 KATs unchanged); did it thread **all** call sites; did it break any caller; diff surgical vs sprawling. This is the coherence axis: the same invariant held consistently across N edit sites.

### Slot C — long debug chain (long-horizon persistence) → expected to favor GLM / Kimi
**Task:** Given the symptom — *"the P4b residency daemon's `hot`/`prefetched` two-atomic tenant↔daemon protocol intermittently demotes-and-miscounts a just-consumed tensor"* — **diagnose the root cause and propose the fix.**
- **Base:** the commit before `5c0b3bb` ("Fix CI QEMU shard 3 daemon flake"). **Ground-truth:** `5c0b3bb` — the two atomics used `Release`/`Acquire`, which orders each atomic only with itself ⇒ **no total order across the two**, so the rollback can observe "prediction withdrawn" without "prefetch consumed." Fix = **`SeqCst` total-order on both flags + publish `prefetched` before the migration is visible.**
- **Scoring anchors:** did it reach *"no total order across the two atomics"* (the crux), and propose `SeqCst`/total-ordering; turns-to-green; did it avoid plausible-but-wrong fixes (e.g., a coarser lock that "works" but isn't the real bug). A subtle concurrency race from a one-line symptom is the persistence axis.
- *Alt:* the BUILD-7.5-B AX88179 ECM-TX blind-debug (ground-truth: wrong native-ASIX TX header + all-zero BOOTP `chaddr`) — a longer multi-iteration chain.

### Bonus probe (UNSCORED — no ground-truth yet): OT-24
Optionally point each model at the **live SMP frame-allocator reentrancy race** (OT-24, `kernel/src/mm/mod.rs:397` `with_frame_alloc` reentered during AP bring-up) from the actual QEMU panic logs. A model that genuinely cracks an *open* bug is strong real signal — and we'd get candidate diagnoses to compare against the real coder's fix. Keep it **out of the scored rubric** (we can't score against an unknown answer), but log whether any model nails it.

## 3. Scorecard (fillable — one row per task × model × run)

| Task | Model | Run | Compiles 1st (×2) | Turns-to-green | Unsafe-correct (×2) | no_std | Diff quality | Quota burned | Wall-clock | Weighted total | Notes |
|------|-------|-----|-------------------|----------------|---------------------|--------|--------------|--------------|------------|----------------|-------|
| A | GLM-5.2 | 1 | | | | | | | | | |
| A | DeepSeek V4-Flash | 1 | | | | | | | | | |
| A | Kimi K2.7 Code | 1 | | | | | | | | | |
| A | **Prometheus** (Qwen 27B, incumbent) | 1 | | | | | | local⇒n/a | | | |
| B | GLM-5.2 | 1 | | | | | | | | | |
| B | DeepSeek V4-Flash | 1 | | | | | | | | | |
| B | Kimi K2.7 Code | 1 | | | | | | | | | |
| B | **Prometheus** (Qwen 27B, incumbent) | 1 | | | | | | local⇒n/a | | | |
| C | GLM-5.2 | 1 | | | | | | | | | |
| C | DeepSeek V4-Flash | 1 | | | | | | | | | |
| C | Kimi K2.7 Code | 1 | | | | | | | | | |
| C | **Prometheus** (Qwen 27B, incumbent) | 1 | | | | | | local⇒n/a | | | |

*(Add rows for runs 2–3 where quota allows. Weighted total = `Compiles×2 + Unsafe×2 + (Turns-to-green, no_std, Diff)` on the 1–5 scale; Quota + Wall-clock are the cost columns, reported separately for the cost-vs-quality call.)*

**Quota pacing (handoff §2):** GLM (~880) and Kimi (~1,150) exhaust the window ~36× faster than V4-Flash (~31,650). Run the premium pair **first** while quota's fresh; spend variance reruns (2–3×) on whichever model is leading after run 1, not uniformly.

## 4. Deliverable (PM → JC)
Filled scorecard → **recommended default executor + fallback** → the **cost call**: does the premium model's quality on *kernel* work (not the Python/web benchmarks, §7) justify ~36× the quota of V4-Flash. PM does the per-run unsafe/no_std/diff scoring; JC makes the final pick.

## 5. Prometheus (Qwen 27B) as the incumbent baseline (competitor #4 — PL-requested)
**Prometheus is Qwen 27B run in the OpenCode CLI** — our *current* open-weight executor. Running it through the same A/B/C tasks makes this a clean four-way **incumbent-vs-challengers** bake-off: the decision isn't "best open-weight," it's **"does GLM/DeepSeek/Kimi beat the Qwen 27B we already run for kernel work?"** Same harness (OpenCode), same task class — the only variable is the model.

**Two mandatory controls (or the comparison is invalid):**
- **No-peek (Prometheus is repo-native).** The challengers run on a pinned pre-fix base with no future history; Prometheus works our *live* repo and *can* see the merged solution (9.2, the SeqCst fix). So the operator gives Prometheus a worktree **checked out at the pinned base commit**, and the prompt **explicitly forbids consulting any commit after the base** (no `git show` of the solution, no reading the later diff). Same answer-withheld condition as the others. Scorecard note: Prometheus's runs carry a no-peek-honor caveat (higher leak risk than the air-gapped challenger runs).
- **Same standalone condition.** Solo run — no PM/cross-coder help mid-task (handoff §6), identical prompt + pre-loaded context, fresh side branch.

**Evaluator neutrality (state it in the writeup, but it's a *plus* now):** the PM doing the scoring is **Claude** — a *different* model lineage from all four competitors (Qwen / GLM / DeepSeek / Kimi). So there's **no same-lineage bias** toward any of them, including the incumbent; the PM is a neutral third party. The decision still rides the **objective double-weighted axes** (compiles Y/N; the bit-identical/within-tol KAT; the off≡f32-bit-for-bit gate; reached-the-known-root-cause) — those are falsifiable regardless of who scores — with `diff-quality` the only judgment call.

**Quota:** if Qwen-27B-Prometheus is **self-hosted/local**, it costs no OpenCode Go quota (and runs all three tasks + variance reruns free of the §2 pacing). If it runs **through the OpenCode Go subscription**, log its usage units too — it's part of the same cost picture. Operator marks whichever applies.

**Scorecard:** add a **Prometheus (Qwen 27B, incumbent)** row under each task (A/B/C) alongside the three challengers in §3.

---

## 6. Round-1 results (PRELIMINARY — scored from model self-reports, not yet from diffs)

**Binding caveat:** only **Slot C** is scored bindingly (the ground-truth — `5c0b3bb`, the P4b SeqCst race — is documented and I judged both write-ups against it). A/B are **self-reported** (the models' own "bit-identical / N tests pass" claims); they need the actual branches to bind the `unsafe-correctness` and `diff-quality` axes and verify the gates. Per verify-before-merge, a model's green-claim is not a green until checked.

### Control issues found (gate validity, not just score)
1. **🔴 Prometheus/Qwen Slot B = INVALID (no-peek violated).** It reported "9.2 wiring already present in the working tree… no edits needed, zero diff" — i.e. it ran on a *post-9.2* tree, not the pinned pre-fix base `89f134e`. It found the answer in the repo rather than reproducing it. Cell void; re-run on a worktree actually checked out at `89f134e`.
2. **🟡 Slot A not head-to-head (spec defect, mine).** Slot A offered a *choice* ("q8_K quantizer **or** a soft_f32 routine"), so DeepSeek did `quantize_block_q8_k` and Qwen did `soft_f32 add/mul/relu` — different targets, not comparable. Round 2 pins one target.
3. **🟡 One run per cell, each model only on its predicted-favorable slot** (DeepSeek→A, Kimi→B, GLM→C). Confirms the directional priors; gives **no off-axis or variance data**. A default executor must be good at all three slots.

### Preliminary scorecard (round 1)
| Slot | Model | Objective gate | Cost | Status |
|---|---|---|---|---|
| C (debug) | GLM-5.2 | ✅ reached exact ground-truth (no-total-order crux + SeqCst+publish fix + rejected coarse-lock/timeout wrong-fixes) | $0.13 / 5% ctx | **BINDING PASS** |
| C (debug) | Qwen 27B (incumbent) | ✅ reached exact ground-truth + caught the clear-on-failure third element | $0.00 (local) | **BINDING PASS — matched the premium model** |
| B (refactor) | Kimi K2.7 | ⚠️ self-reports byte-identical to `93822e6`; 130 KATs both feature states | **$2.52 / 63% ctx** | self-reported — verify the branch |
| B (refactor) | Qwen 27B | ❌ ran on post-9.2 tree | $0.00 | **INVALID — re-run on `89f134e`** |
| A (primitive) | DeepSeek V4-Flash | ⚠️ complete AVX2 q8_K quantizer, bit-identical claimed, 138 tests | $0.47 | self-reported — verify the branch |
| A (primitive) | Qwen 27B | ⚠️ add/mul/relu bit-identical/within-tol; *honestly punted exp* (correct — hw-exp ≠ soft-float-exp, §9.4) | $0.00 | self-reported; different target ⇒ not vs DeepSeek |

### Headline
**"Does any challenger clearly beat the incumbent Qwen 27B?" — not answered yet, and the incumbent is holding.** On Slot C (hardest axis — long-horizon concurrency debug) Qwen **matched GLM exactly at $0**, even surfacing one detail GLM missed. Challengers looked strong only on their *predicted-strength* slots; nobody's been tested off-axis. Cost spread is already informative: **Kimi $2.52 (63% ctx) ≫ DeepSeek $0.47 > GLM $0.13 > Qwen $0 (local).**

## 7. Round-2 spec (tightened — answers the default-vs-fallback question)

Fixes the three control issues + binds the scoring. **Goal of round 2: a full, comparable matrix + the diffs, so the default/fallback call is decision-grade.**

1. **Pin Slot A to ONE target:** `quantize_block_q8_k` AVX2 (the DeepSeek target), all four models head-to-head, gated **bit-identical** vs the scalar oracle. (Drop the "or soft_f32" choice.)
2. **Enforce Prometheus no-peek mechanically:** operator creates the Qwen worktree by `git checkout -b <eval> 89f134e` (B) / `git checkout -b <eval> <pre-5c0b3bb>` (C) / current `main` (A), and **verifies `git log -1` shows the base before the run** — the solution commit must not be reachable. Prompt forbids `git show` of later history.
3. **Full off-axis matrix:** run **every model on all three slots** (A/B/C), not just its favorite — the default executor must win across the board. 2–3× variance reruns where quota allows; premium models (GLM/Kimi) first per §2 pacing.
4. **Hand the PM the branches/diffs per run** (not just summaries) — so the gates are *verified* and `unsafe-correctness` + `diff-quality` get real scores. Binding scores require this.
5. **Objective gates per slot (deterministic — restate so scoring isn't judgment):** A = bit-identical-vs-scalar KAT + correct CPUID+XCR0 gate; B = compiles + off≡f32 *bit-for-bit* + all 8 call sites threaded + (bonus) byte-identical to `93822e6`; C = reached "no total order across the two atomics" + proposed SeqCst-total-order + publish-before-migrate, and rejected the coarse-lock/timeout wrong-fixes.

**Then:** PM scores the full verified matrix → recommended **default + fallback** + the cost call (does any challenger's quality justify its quota over the free local incumbent). Round 1 says the incumbent isn't beaten yet; round 2 is what proves or overturns that.

## 8. Round-2 execution list (operator checklist — the MINIMAL set)

Full matrix = 4 models × 3 slots = 12 cells. **4 are already done in round 1** (don't re-run): DeepSeek-A, Kimi-B (valid — *bind from diff*), GLM-C, Qwen-C (binding pass). **8 cells remain.** Bases are per-slot: **A → `main` (HEAD); B → `89f134e`; C → `ecae749` (=`5c0b3bb^`).**

### Bind-now (NO run — just hand the PM the branch/diff)
- **DeepSeek Slot-A** (q8_K AVX2) — verify bit-identical + score unsafe/diff-quality.
- **Kimi Slot-B** (9.2 repro) — verify byte-identical to `93822e6` + score.
- *How:* push the two eval side-branches to `origin` and give me the names, or paste `git diff`. No quota.

### The 8 runs, cost-ordered (run top-down; if quota dies, the dropped cells are the expensive-fallback ones)
| # | Model | Slot | Base | Control / gate |
|---|-------|------|------|----------------|
| 1 | **Qwen 27B** (incumbent) | A | `main` | q8_K AVX2; bit-identical vs scalar oracle. **Free (local).** |
| 2 | **Qwen 27B** | B | `89f134e` | **No-peek: `git checkout -b ev89 89f134e`, verify `git log -1`, forbid viewing `93822e6`.** Gate: off≡f32 bit-for-bit + all 8 sites. Free. |
| 3 | **DeepSeek V4-Flash** | B | `89f134e` | no-peek; same B gate. Cheap (~31.6k allowance). |
| 4 | **DeepSeek V4-Flash** | C | `ecae749` | diagnose; gate = reach "no total order across the two atomics" + SeqCst+publish fix + reject coarse-lock/timeout. Cheap. |
| 5 | **GLM-5.2** | A | `main` | q8_K AVX2; bit-identical gate. Premium (~880) — but GLM ran cheap in R1. |
| 6 | **GLM-5.2** | B | `89f134e` | no-peek; B gate. Premium. |
| 7 | **Kimi K2.7** | A | `main` | q8_K AVX2; bit-identical gate. **Premium + ran expensive in R1 ($2.52/63%ctx) — run last.** |
| 8 | **Kimi K2.7** | C | `ecae749` | diagnose; C gate. Premium — run last; **skippable if premium quota's gone** (Kimi's cost already makes it a fallback-at-best). |

### Controls (all runs)
Fresh side branch off the listed base; OpenAI-compatible path; constant reasoning preset per model; identical prompt + pre-loaded context; **one operator**; **no PM/cross-coder help mid-task**. For B and C the solution commit (`93822e6` / `5c0b3bb`) must be **unreachable/un-viewed** — air-gapped for the open-weight models, enforced-by-checkout for the repo-native Qwen. **Hand the PM every run's branch/diff + turn log** (binding scores need the diff, not the summary).

### Variance
Single run per cell first. Spend 2–3× reruns **only on the eventual leader** (after the matrix fills), not uniformly — premium quota won't cover uniform variance.

### On return → PM produces the decision
PM binds every cell (verify the gates + score unsafe/diff-quality on the 1–5 rubric, double-weighting Compiles + Unsafe) → fills §3 → **recommended default + fallback + the cost call** (does any challenger's cross-the-board quality justify its quota over the free local incumbent). Round 1: incumbent un-beaten; round 2's off-axis cells (1–8) decide it.

## 9. Round-2 binds (as branches land)
- **Kimi Slot-B (`4fa3f20`): ✅ BINDING PASS — byte-identical.** PM-verified `forward_logic.rs` is byte-for-byte the real 9.2 (`93822e6`); `matmul_vec` untouched (off≡f32 holds). Flawless cross-file reproduction. Locked.
- **`8956eec` (Slot-A q8_K AVX2): ✗ INVALID cell — contaminated base.** Built *stacked* on Kimi's Slot-B off the pre-9.2 base `89f134e` (no 9.3 AVX2 present), so it rebuilt the whole AVX2 dot+quantizer instead of adding only the q8_K quantizer on current `main`. Wrong base, wrong (bigger) task, unattributed model. Code craft is sound (correct CPUID+XCR0 gate / target_feature / SAFETY / bit-identical KATs) — positive signal, not a scoreable cell. **Re-run Slot A clean.**

### Operator rule (prevents the recurrence)
**Every run is its OWN fresh branch off the slot's base — never reuse a branch name, never stack one run on another.** A=`main`, B=`89f134e`, C=`ecae749`. Distinct names: `ab-A-<model>`, `ab-B-<model>`, `ab-C-<model>`. The Slot-A redo must branch off `main` (so 9.3's AVX2 dot is already present and the model adds *only* the q8_K quantizer).

## 10. Round-2 binds (clean, novel/un-gameable tasks)
- **DeepSeek OT-24 (diagnosis): ✅ PASS (PM-verified).** Correctly identified the single-global `#[cfg(debug_assertions)]` `IN_FRAME_ALLOC` guard false-positiving on cross-AP contention; real allocator Mutex-safe; release unaffected. Fix branch `ot24-deepseek` not pushed (described: per-CPU guard) — diagnosis bound, fix-code unverified.
- **Kimi OT-24 (fix `ot24-kimi`, `34843d0`): ✅ BINDING PASS (PM-verified correct + sound).** Per-CPU `[AtomicBool; MAX_CPUS]` guard indexed by `current_cpu_id()`; verified `current_cpu_id()` is valid at the call site (AP wires GS-base at `smp.rs:139` before the `with_frame_alloc` paths at 151/152); same-CPU/NMI reentry still caught; no deadlock (Mutex is the serialization); `frame.rs` comment corrected. **Promotable as the real OT-24 fix** (run the close-gate + QEMU gate, then merge — also de-flakes the debug-build gate).
- **Qwen3.6-27B / Prometheus OT-24 (fix `ot24-qwen`, `27408d9`): ✅ BINDING PASS (PM-verified correct + sound) — the INCUMBENT nailed it.** Same correct root cause and per-CPU `[AtomicBool; MAX_CPUS]` fix as Kimi (+22/−8, `mm/mod.rs` only), and **marginally the better base**: adds an explicit `cpu_id < MAX_CPUS` bounds assert before indexing (fails loud, not OOB, if the id is ever stale), a clearer rewritten doc comment, and a correct deadlock-freedom argument (`current_cpu_id()` read under `without_interrupts` ⇒ stable id; single lock under IRQ-off; guard panics before the second non-reentrant `Mutex::lock`). The free, local incumbent matched the premium challengers on the un-gameable cell.
- **DeepSeek q8_K (`q8k-deepseek`): branch empty (== main, uncommitted) — re-push with commit.** Design (scalar tie-break + SIMD quantize) sidesteps the sign-tie trap, but code unverifiable until pushed.

**Headline (un-gameable cell):** on OT-24 — the only cell with no copyable answer — **all three of {Qwen-27B incumbent, DeepSeek, Kimi} produced correct work** (DeepSeek a verified diagnosis; Kimi and the Qwen-27B incumbent each a verified, promotable fix). The earlier "reproduce-solved-work" cells proved nothing (answers were in the repo); this one proves capability — and yielded *two* real fixes for OT-24. **Decision implication:** the incumbent (free, local Qwen-27B / Prometheus) matched the premium challengers (Kimi $2.52/run, DeepSeek $0.47) on the one task that can't be gamed — so **no challenger has beaten the incumbent on capability**, only on price (none cheaper than free). Provisional read: **keep Qwen-27B as the default executor**; a paid challenger earns the slot only if it wins a cell the incumbent fails, which hasn't happened. Promotion of the OT-24 fix uses **`ot24-qwen` (`27408d9`)** as the cleaner base (bounds assert).

## 11. CLOSED — final scorecard + roster outcome (2026-06-19)
Scored on **verified diffs only** (prose/self-report does not bind):

| Model | Cost/run | Verified cells | Result |
|---|---|---|---|
| **Qwen-27B** (Prometheus, incumbent) | **$0 local** | Slot C (P4b SeqCst race) · OT-24 fix `ot24-qwen` `27408d9` | **2× BINDING PASS** — OT-24 fix **merged to `main` as `3724ad1`** |
| **Kimi K2.7** | ~$2.52 | Slot B `4fa3f20` (byte-identical) · OT-24 fix `ot24-kimi` `34843d0` | **2× BINDING PASS** |
| **DeepSeek V4-Flash** | ~$0.47 | OT-24 diagnosis | **1× PASS** (q8_K never validly pushed — `q8k-deepseek` `10055fc` is an OT-24-guard copy, not q8_K) |
| **GLM-5.2** (→ Genesis) | ~$0.13 | Slot C (P4b SeqCst race) | **1× BINDING PASS** |

**Invalid/unscored:** `8956eec` (contaminated base); Qwen & DeepSeek q8_K (never validly pushed); remaining off-axis matrix cells (not run — judged non-decision-changing).

**Verdict:** no model failed a cell it genuinely attempted; **no challenger beat the incumbent.** On the two un-gameable / ground-truth cells (Slot-C race, OT-24) the free local Qwen-27B matched or led every paid model, and its OT-24 fix is the one that shipped.
- **Default executor:** keep **Qwen-27B / Prometheus** ($0, 2 verified passes, one merged).
- **Paid fallback:** **GLM-5.2** (cheapest paid pass, $0.13) → adopted as a **standing coder seat: Genesis** (see CLAUDE.md roster, trust-ramp on bounded/verifiable tasks first). **DeepSeek** ($0.47) second; **Kimi** capable but 5–20× the cost with no capability edge — reserve.
- **Budget:** further paid runs are variance-only; **bake-off CLOSED.**
