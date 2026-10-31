# Dispatch: SC-3B Fix 6.1 — per-pair direction alternation in stress test (test-only)

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** BUILD-stage execution dispatch following project-lead ratification of `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §6.1 (Fix 6.1: per-pair direction alternation). Single-commit, test-only change. Implements the leading hypothesis fix (Hn — deterministic Q4 migration accumulation from constant pair-direction assignment) before any kernel-level instrumentation or change is contemplated. Per Bible Article 14, this re-opens the BUILD/VERIFY stage for SC-3B Sub-step 5's verification gate; Sub-step 6 (docs + close) remains blocked until 200/200 closes.
**Substrate:** Project HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` (SC-3B Sub-step 5 commit). Backup: `Z:\AltimitNeo\2026-05-09-144314`.

Three preserved stashes (unchanged across this dispatch):

- `stash@{0}: On main: wedge-investigation-8-nmi-watchdog`
- `stash@{1}: On main: wedge-investigation-2-instrumentation`
- `stash@{2}: On main: h1-experiment-instrumentation`

None of the stashes is applied during this dispatch. `stash@{0}` is reserved as candidate Phase B instrumentation only if Fix 6.1 fails to close the gate (see §6).

---

## 0. Standing instructions

1. **Single test-only commit.** This dispatch authorizes exactly one commit to `kernel/src/test_harness.rs`. No kernel source code is modified. No other files are touched.
2. **Build verification before commit.** `docker compose run --rm neo cargo k` must pass at the existing 18-warning baseline (no new warnings).
3. **No stash interactions.** All three stashes remain unmodified for the duration of this dispatch.
4. **Backup runs after the commit lands** via `Plans/backup-to-synology.ps1` per the standing 2026-05-08-refined three-stash mechanism.
5. **Stop conditions are honored** (§5). If a stop condition fires, halt, capture diagnostics, ping PM. Do not push through.
6. **The 200/200 closure gate stands.** No threshold relaxation. The wedge-fix-discharge precedent (`Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`) is the standing statistical framing.
7. **PM is on call.** Ping for any clarification during code change or gate execution.

---

## 1. Goal

Replace the deterministic `send_first` direction assignment in `run_smp_cross_cpu_rendezvous_stress` with per-pair alternation, breaking the constant-direction-per-pair pattern that drives Q4 migration accumulation onto a single CPU. After this commit, re-run the BUILD-3 sub-commit 3 verification gate's primary stress closure (Step 6: 200-run `smp_cross_cpu_rendezvous_stress` loop at NEO_SMP=4). On 200/200 PASS with zero classifier-signature timeouts, hand back to PM for Sub-step 6 (docs + close).

The change has no kernel impact. It is purely a redistribution of which direction (sender-first vs. receiver-first) each pair takes on each of its iterations, such that no single pair is locked into one direction across the run.

---

## 2. The fix

### 2.1 Background (anchored in source)

The current stress test orchestrator (verified at `kernel/src/test_harness.rs` `run_smp_cross_cpu_rendezvous_stress`) assigns the per-iteration direction as:

```rust
for seq in 1..=STRESS_ITERATIONS {
    let pair = ((seq - 1) as usize) % pairs;
    ...
    let send_first = (seq & 1) == 1;
    ...
}
```

For `pairs=4`:
- Pair 0 receives seq=1, 5, 9, 13, … — all odd. **Always `send_first=true`.**
- Pair 1 receives seq=2, 6, 10, 14, … — all even. **Always `send_first=false`.**
- Pair 2 receives seq=3, 7, 11, 15, … — all odd. **Always `send_first=true`.**
- Pair 3 receives seq=4, 8, 12, 16, … — all even. **Always `send_first=false`.**

Each pair is locked into one direction. Combined with Q4 wake-on-completer-CPU semantics (the completer wakes the parked peer on the completer's own CPU), the migration map is monotonic per pair — sender and receiver converge onto the same CPU after the first iteration of that pair, and stay there. Across many pairs, this accumulates threads on specific CPUs and decouples the orchestrator's CPU0 from the workers' CPUs.

The investigation synthesis (`Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §2.7) ranks this as the leading cause of the run-36 silent timeout (Hn at 50% confidence; corroborated independently by Claude-3 at 55%; Cerebro's primary investigation reached the same conclusion at 50% in the pre-synthesis pass).

### 2.2 The change

Add a new per-pair direction tracker, reset it in `reset_stress_state`, and consume it inside the iteration loop. The semantics: each pair alternates between `send_first=true` and `send_first=false` on its successive iterations, regardless of the global seq parity.

**File:** `kernel/src/test_harness.rs`. No other files.

**Approximate diff sketch (PM-authored; final naming and exact placement is Cerebro's call):**

(a) **Static declaration** — add alongside `STRESS_SENDER_GO`, `STRESS_RECEIVER_GO`, etc.:

```rust
#[cfg(feature = "neo-test-smp-cross-cpu-rendezvous-stress")]
static STRESS_PAIR_DIRECTION: [core::sync::atomic::AtomicBool; STRESS_MAX_PAIRS] =
    [const { core::sync::atomic::AtomicBool::new(false) }; STRESS_MAX_PAIRS];
```

(b) **Reset** — extend `reset_stress_state`:

```rust
for idx in 0..STRESS_MAX_PAIRS {
    STRESS_SENDER_GO[idx].store(0, Ordering::Release);
    STRESS_RECEIVER_GO[idx].store(0, Ordering::Release);
    STRESS_SENDER_DONE[idx].store(0, Ordering::Release);
    STRESS_RECEIVER_DONE[idx].store(0, Ordering::Release);
    STRESS_PAIR_DIRECTION[idx].store(false, Ordering::Release);  // NEW
}
```

(c) **Direction selection** — replace the global-parity line in `run_smp_cross_cpu_rendezvous_stress`:

```rust
// Before:
let send_first = (seq & 1) == 1;

// After:
let send_first = STRESS_PAIR_DIRECTION[pair].fetch_xor(true, Ordering::Relaxed);
```

`fetch_xor` returns the previous value. Starting from `false`, the sequence per pair is: returns `false` → stores `true`; returns `true` → stores `false`; returns `false` → stores `true`; … Each pair alternates between `send_first=false` (recv-first) and `send_first=true` (send-first) on each visit. The first iteration of each pair is recv-first; the second is send-first; and so on. This matches the synthesis's intent (per-pair alternation), and the resulting global counts of send-first vs. recv-first iterations remain balanced for the typical pair counts (NEO_SMP=2 → 250/250 per pair × 2 pairs = 500/500 total; NEO_SMP=4 → 125/125 per pair × 4 pairs = 500/500 total).

### 2.3 Naming and primitive choice

The synthesis used `AtomicBool` for the per-pair direction tracker. The existing per-pair statics in this scenario use `AtomicU32`. Both are correct. PM defers the choice to Cerebro:

- `AtomicBool` is the most natural fit for a binary alternation; `fetch_xor(true, Relaxed)` is idiomatic and one line.
- `AtomicU32` matches the surrounding pattern; would use `fetch_add(1, Relaxed) & 1 == 0` or similar.

`AtomicBool` is the recommended default. The constant of `false` for the initial stored value is the recommended default; this means each pair's first iteration is recv-first (the `else` branch of the existing `if send_first` block). The investigation does not depend on the starting direction; only on the alternation.

### 2.4 What does NOT change

- No kernel source. `kernel/src/sched/`, `kernel/src/ipc/`, `kernel/src/arch/`, `kernel/src/sync/` all untouched.
- No new tests, scenarios, or feature flags.
- No PASS frame schema change. The PASS frame still reports `iterations`, `send_first`, `recv_first`, `cpus` exactly as before. The values of `send_first` and `recv_first` will redistribute across pairs but the totals stay balanced.
- No `STRESS_ITERATIONS` change. Still 1000.
- No `STRESS_MAX_PAIRS` change. Still 4.

### 2.5 Diff size budget

The synthesis estimated 5–10 lines. PM's expectation:

- 3 lines for the static declaration (depending on formatting).
- 1 line added to `reset_stress_state`.
- 1 line replacing the existing `let send_first = (seq & 1) == 1;`.

Total ≈ 5 lines added + 1 line replaced. If the diff exceeds ~15 lines net, that is a signal something else is being changed; halt and reconcile with the dispatch before committing.

---

## 3. Pre-commit checklist

Before the commit lands:

- [ ] `docker compose run --rm neo cargo k` passes with the existing 18-warning baseline (no new warnings).
- [ ] `git diff --stat` shows changes confined to `kernel/src/test_harness.rs` exclusively.
- [ ] Diff line count is ≤15 lines net (per §2.5).
- [ ] No new `unsafe` blocks introduced. (None should be required for this change.)
- [ ] No new dependencies introduced.
- [ ] `cargo neo-test --features neo-test-smp-cross-cpu-rendezvous-stress` (or whatever the host invocation is at NEO_SMP=4) PASSes single-run sanity check before launching the gate loop.
- [ ] All five 50-run supplementary scenarios from the original gate (Step 5: `smp_block_release_smoke`, `smp_send_blocks_first_then_completes`, `smp_recv_blocks_first_then_completes`, `smp_grant_cross_cpu`, `smp_cross_cpu_migration_reap`) run a single PASS sanity check at NEO_SMP=4 — confirms the test-harness change introduced no compile or static-init regression for sibling scenarios. (One run each is sufficient sanity; full 50-run loops not required, since the change is scoped to the stress scenario's static block + one function.)
- [ ] `git status` clean before commit; clean after commit (no stragglers).

---

## 4. Verification gate

Re-run BUILD dispatch §9 Step 6 (the failed step) and Step 7 (the deferred NEO_SMP=2 smoke).

### 4.1 Required steps

1. **Single-boot sanity at NEO_SMP=4** of `smp_cross_cpu_rendezvous_stress`. PASS expected with PASS frame schema unchanged.
2. **200-run loop** of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4. **200/200 PASS, zero classifier-signature timeouts.**
3. **25-run loop** of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=2 (Step 7 from the original gate). 25/25 PASS expected.

### 4.2 PASS criteria per run

Each run must satisfy (unchanged from the original gate's §9.2):

- BEGIN frame observed.
- Exactly one terminal PASS or FAIL frame.
- Terminal frame is PASS.
- Debug-exit code matches PASS.
- No `BUG:` line.
- No `KERNEL PANIC` line.
- No timeout.
- PASS frame fields: `iterations=1000 send_first=<n> recv_first=<m> cpus=<pairs>`. The values of `n` and `m` will reflect the new alternation but should sum to 1000.

### 4.3 Statistical interpretation

The original gate observed 35/35 PASS before run 36's silent timeout at NEO_SMP=4. If Hn is the root cause, the per-pair alternation breaks the deterministic CPU-load accumulation, and 200/200 PASS is the closure signal.

P[200/200 PASS | underlying intermittent rate ≥ 0.5%] ≈ 0.45% — same statistical framing as the L wedge-fix discharge (`Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md`). If Fix 6.1 passes 200/200, the fix is the closure.

If even a single timeout matching the run-36 signature (BEGIN frame followed by silence, no PASS/FAIL/BUG/PANIC, QEMU 180s timeout) occurs in the 200-run loop, do NOT extend the loop to chase statistical zero. Stop, classify, report, and bring to PM.

### 4.4 Verification log

Produce `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` (or 2026-05-10 if the calendar advances during execution). Structure mirrors the existing `Plans/SC-3B-VERIFICATION-2026-05-09.md`:

```
# SC-3B Fix 6.1 verification log

## Substrate
[commit SHA, backup path]

## Pre-commit sanity
[cargo k result, single-boot result, sibling-scenario sanity]

## Gate steps
[Step 1: single-boot sanity — result + PASS frame]
[Step 2: 200-run NEO_SMP=4 — result + clean count + any failures]
[Step 3: 25-run NEO_SMP=2 — result + clean count]

## Statistical interpretation
[mirroring L's discharge framing]

## Conclusion
[pass / fail; recommendation for Sub-step 6 / Phase B]
```

This log is a working-tree artifact, not a commit. Captured by the post-commit backup.

### 4.5 Estimated wall time

- Code change: ~30 minutes (write, build, sanity-check).
- 200-run loop at NEO_SMP=4: ~30–60 minutes depending on host load (each run is ~5–15 seconds typical; failures take 180 seconds).
- 25-run loop at NEO_SMP=2: ~5–15 minutes.
- Verification log + commit + backup: ~15 minutes.

Total: ~1.5–2 hours from dispatch acceptance to handoff.

---

## 5. Stop conditions

Halt and ping PM if any of these fire:

- The diff exceeds ~15 lines net or strays outside `kernel/src/test_harness.rs`.
- `cargo k` introduces any new warning, even one.
- Single-boot sanity of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 fails before the gate loop starts.
- Any of the five sibling scenarios' single-run sanity fails (none should — the change is scoped to the stress scenario's static block + one function, with `#[cfg(feature = ...)]` gating).
- Step 2 (200-run NEO_SMP=4) records ANY failure run with the run-36 signature: BEGIN frame followed by silence, no terminal frame, QEMU 180s timeout.
- Step 2 records any other failure mode (panic, BUG, FAIL frame with a new reason, missing PASS frame for any other reason).
- Step 3 (25-run NEO_SMP=2) records any failure.
- Backup script reports failure for two consecutive attempts post-commit.

If a stop condition fires after the commit has already landed, the commit stands (do not amend, do not revert). Triage proceeds from the current substrate.

---

## 6. Outcomes and forward path

### 6.1 Outcome A — Gate passes (200/200 NEO_SMP=4 + 25/25 NEO_SMP=2)

This is the expected outcome.

Cerebro:
1. Land the verification log (`Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md` or dated equivalent) as a working-tree artifact.
2. Run backup.
3. Notify PM that the gate has closed.

PM then opens Sub-step 6 (docs + close) per BUILD dispatch §7, with these adjustments:

- Close report (`Plans/CLOSE-REPORT-2026-05-XX-sc-3b.md`) includes a summary of the three-way investigation (per `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §2.7) and Fix 6.1 verification evidence.
- ADR-013 §Q4 and §Q6 amendments unchanged from BUILD dispatch §7.1–7.2 — the kernel implementation of those questions is mechanically unchanged.
- The investigation may be cross-referenced from ANSPEC-001 §10.2 if PL wants the forward concern (Claude-3's note: deterministic Q4 migration patterns could plausibly affect production AI workloads under capability-passing IPC) noted for Phase 5 design. Optional, not blocking.

### 6.2 Outcome B — Gate fails (any timeout in 200-run NEO_SMP=4 OR any failure in NEO_SMP=2 smoke)

Phase B instrumentation opens.

Cerebro:
1. Capture the failure serial log(s) under `target/verify/sc3b-fix-6-1-gate/` (or equivalent path).
2. Land the verification log capturing whatever runs completed.
3. Run backup.
4. Stand by for PM's Phase B instrumentation dispatch.

PM then produces a Phase B instrumentation dispatch (`Plans/DISPATCH-2026-05-XX-sc-3b-phase-b-instrumentation-cerebro.md`) following the synthesis §3 structure:

- Apply `stash@{0}` (NMI watchdog from wedge investigation) for investigation only — not committed. NMI threshold: PM defers to Cerebro per the synthesis (5s per Claude-3 §3.1, or 10s per Cerebro §3.7; Cerebro's choice given Cerebro's deeper familiarity with typical iteration timing).
- Add per-iteration PROGRESS frames every 50 iterations (or 10 if Cerebro judges that better) inside the orchestrator loop, after the GO signals are set, before `wait_for_stress_iteration_done`.
- Run a smaller loop (50 runs at NEO_SMP=4) with the instrumentation. NMI captures stack traces on hang; PROGRESS frames identify the stuck iteration.
- Bring data back to PM. PM ranks-with-data and produces the next iteration of the fix design.

### 6.3 Forward path summary

```
Fix 6.1 commit → 200/200 NEO_SMP=4 + 25/25 NEO_SMP=2 ──Outcome A──→ Sub-step 6 (docs + close)
                                                       │
                                                       ──Outcome B──→ Phase B instrumentation (NMI + PROGRESS)
                                                                       │
                                                                       └→ Ranking-with-data → next fix iteration
```

---

## 7. Commit message template

```
test: SC-3B Fix 6.1 — per-pair direction alternation in stress test

Replaces the deterministic `let send_first = (seq & 1) == 1` with a
per-pair AtomicBool that alternates direction on each pair's successive
iterations. The original assignment locked each pair into one direction
(pair 0/2 always send_first; pair 1/3 always recv_first), which combined
with Q4 wake-on-completer-CPU produced deterministic Q4 migration
accumulation onto specific CPUs and a silent timeout at run 36/200 of
the BUILD-3 sub-commit 3 closure gate.

Test-only change. No kernel modification. No PASS frame schema change;
send_first and recv_first counts redistribute across pairs but remain
balanced.

Verification: re-runs BUILD dispatch §9 Step 6 (200-run NEO_SMP=4) +
Step 7 (25-run NEO_SMP=2). Log at Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md
(working-tree). [200/200 + 25/25 if outcome A; partial otherwise.]

Reference: Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md §6.1,
Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md §3.6.
```

If the gate passes 200/200, the bracketed clause in the verification line resolves to "200/200 + 25/25". If a partial result, capture what completed honestly and let the verification log carry the detail.

---

## 8. Cosmetic note on Sub-step 5 commit body

The Sub-step 5 commit (`a61e318`) body has literal `\n` separators due to PowerShell quoting at commit time. Per the standing "no amend without explicit request" rule, this is correctly left alone. The Fix 6.1 commit body should NOT be amended for cosmetic reasons either; if the literal `\n` issue recurs in the Fix 6.1 commit, leave it alone and surface the artifact in the close report's notes section if PL wants. PM's standing posture on cosmetic commit-body artifacts: tolerate, document if useful, do not amend.

---

## 9. Cross-references

**Authority for this dispatch:**
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md` §3.6, §4.1 — PL's ratification of Fix 6.1 + synthesis bookkeeping option (a).
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §6.1 — fix design (PM synthesis-of-record).

**The substrate this dispatch operates on:**
- HEAD `a61e318d93321c9e84732869f5de9da96abe3d5f` — SC-3B Sub-step 5 commit.
- `kernel/src/test_harness.rs` — the file being modified.

**The verification gate this dispatch re-runs:**
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md` §9 — original BUILD verification methodology.
- `Plans/SC-3B-VERIFICATION-2026-05-09.md` — the gate run that captured the run-36 failure.

**Statistical framing precedent:**
- `Plans/COMMITMENT-DISCHARGE-2026-05-08-wedge-fix-before-sc-3b.md` — L's wedge-fix discharge, P[200/200|rate>0.5%]≈0.45% framing.
- `Plans/WEDGE-FIX-L-VERIFICATION-2026-05-08.md` — L's empirical evidence (300/300).

**Investigation reports informing the fix:**
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md` — Cerebro's per-agent investigation.
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md` — Claude-3's investigation (independent corroboration of Hn).
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md` — Prometheus's investigation (calibration).

**Phase B fallback (if Outcome B):**
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md` §3 — instrumentation strategy.
- `stash@{0}` — NMI watchdog from wedge investigation.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance), 1.3 (every commit is intended to ship), 14 (OBSERVE → THINK → PLAN → BUILD/VERIFY).
- `docs/ADR-013-phase-2-smp-retrofit.md` §Q4, §Q6 — the kernel substrate this test exercises (mechanically unchanged by Fix 6.1).

---

**End of dispatch.**

*One commit, ~5 lines net, test-only, no kernel impact. 200-run NEO_SMP=4 + 25-run NEO_SMP=2 closure gate. Estimated 1.5–2 hours from dispatch acceptance to handoff. Outcome A → Sub-step 6 docs + close. Outcome B → Phase B instrumentation per synthesis §3. PM is on call.*
