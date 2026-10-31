# Dispatch: SC-3B Phase C — instrumentation pass on post-IPI substrate

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-09
**Type:** Instrumentation-only investigation dispatch. Per Bible Article 14 — investigation work, not BUILD. No commit lands from this dispatch. The substrate (HEAD `2d8f65d`) returns clean post-investigation. Single-recipient. PM-decided design under the four-anchor delegation.
**Authority:** `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md` (Outcome B confirmed at run 052/200, MISSING_TERMINAL_FRAME, identical signature to runs 36/6/10). `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 fail-branch (Hp drops; H1-variant re-promotes; channel state must be captured). `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` (precedent for NMI patch + adaptation). `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md` §6.2 (Phase C contents specified at dispatch time as the Outcome B path).
**Substrate:** Project HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0` (Sub-step 5b, IPI infrastructure landed). Tracked working tree clean per Cerebro's Sub-step 5b exit. Stash list `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment. Backup `Z:\AltimitNeo\2026-05-09-224332` with verification artifacts under `artifacts/`.

---

## 0. Standing instructions

1. **No commit lands from this dispatch.** All instrumentation is investigation-only. The substrate at exit is identical to entry: HEAD `2d8f65d`, tracked tree clean, four stashes preserved.
2. **Stash `{1}` is applied for the NMI watchdog base.** Phase C adds three instrumentation deltas on top of the applied stash. Cerebro decides whether to stash the Phase C deltas separately or discard after capture.
3. **Stashes `{0}`, `{2}`, `{3}` are not touched.** Any apply/pop affecting them is a halt-and-ping condition.
4. **Pre-run sanity required.** After applying the stash + Phase C deltas, `cargo k` passes at the 18-warning baseline plus any warnings the instrumentation adds (document delta). All five sibling SMP scenarios pass single-run sanity at NEO_SMP=4. The instrumented `smp_cross_cpu_rendezvous_stress` passes single-run sanity at NEO_SMP=4 (the BEGIN→PASS transition still happens; instrumentation hasn't broken the test).
5. **Backup runs after capture, before tree restore.** The 2026-05-08-refined script handles four stashes. If Cerebro stashes Phase C deltas, the script handles five.
6. **Stop conditions are honored** (§7).
7. **PM is on call.** Mid-dispatch design ambiguity → ping.

### 0.1 Honest framing on what Phase C produces

Phase C is data collection, not fix design. The dispatch produces two artifacts:

- A run log of the 50-run loop with NMI armed (artifact directory below).
- A findings document at `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-09.md`.

PM consumes the findings, re-ranks hypotheses, and dispatches the next move. The next move is one of: targeted fix dispatch (if mechanism is now clear), second Janus deep-research pass (if Phase C produces a refined signature space outside our local search), or further instrumentation (if Phase C captures partial state and a specific extension is high-value). Phase C is not expected to close the gate by itself; it is expected to disambiguate H1-variant from the residual Hp / something-else cluster.

The Outcome B verification post-IPI showed the failure recurred at run 052 with identical signature. Hp at 75% → ~30–35%. H1-variant (channel-level lost wakeup) was at 5% specifically because the Outcome B NMI snapshot showed no parked threads in `EndpointState` — but channel state was *not* dumped in that snapshot. The Phase C channel state dump is the load-bearing addition; PROGRESS frames and cpu_id tracking are supporting evidence.

---

## 1. Goal

Capture state during a hang on the post-Sub-step-5b substrate sufficient to disambiguate three sub-hypotheses:

1. **Channel-level lost wakeup (H1-variant).** A worker is parked in `EndpointState::SendBlocked` or `RecvBlocked` and its peer never wakes it. Confirmable by NMI snapshot showing parked thread IDs in the channel state dump.
2. **Scheduler-progress residual (Hp-residual).** The same CPU2/CPU3 ap-N-main-running-with-workers-Ready pattern from Outcome B recurs even with IPI infrastructure live. Confirmable by NMI scheduler snapshot matching Outcome B run 026's layout. Would imply IPI not delivering or not effective; investigate APIC error register and vector dispatch.
3. **Mechanism not yet named.** Workers running but stuck in some new code path (IPC fastpath, atomic spin, etc.). Confirmable by NMI scheduler snapshot showing workers as Running on APs with no parked threads in channel state.

Plus: PROGRESS frames tell us *how far* into the iteration count the hang occurs (50/950/etc. — informative for whether the failure mode is iteration-count-dependent or arbitrary).

Plus: cpu_id tracking on workers tells us *which pair* hung and *where* their threads were last seen, narrowing the per-pair vs cross-pair question.

---

## 2. Design specifications (PM-decided, not negotiable absent ADR)

### 2.1 Apply stash `{1}` as the base

```text
git stash apply stash@{1}
```

Adapt as in Outcome B and Phase B:
- The stash calls `sched.inner.is_locked()`; current `SwitchMutex` lacks that method.
- Replace with `try_lock()`-based inference: `let inner = sched.inner.try_lock(); let locked = inner.is_none(); ... if let Some(inner) = inner { ... }`.

This is the third application of this adaptation pattern. The pattern is stable; if it diverges from prior application, halt and ping.

### 2.2 Channel state dump in NMI handler (highest-priority addition)

Extend the NMI watchdog handler (the patch from stash `{1}`) to walk known stress-fixture channel endpoints and dump their state. Specifically:

- For each channel that the stress fixture allocates, emit a frame:
  ```
  [ NMI ] channel cpu=N ch=K state={Idle|SendBlocked|RecvBlocked|Locked} parked_id=<id_or_none>
  ```
- Use `try_lock()` on the endpoint state mutex; on failure (lock held by another CPU), emit `state=Locked` rather than blocking. The NMI handler must never acquire a channel lock — that is a deadlock vector under the very condition we are diagnosing.
- The fixture currently allocates a known set of stress channels at `reset_stress_state()` time. The NMI handler should walk that set, not the global channel namespace.
- If the fixture's channel set is not directly addressable from the NMI handler's scope, expose a static or atomic-pointer registry of "stress channels" populated by the test setup. Cerebro decides the wiring; PM defers.
- Output ordering: emit channel state *after* the existing scheduler snapshot for each CPU, so a single NMI capture file pairs scheduler state with channel state for that snapshot.

Constraint: the channel state dump must complete in bounded time. NMI handlers are constrained — the handler should iterate at most O(stress_pair_count) channels and bail quickly on lock contention.

If the channel state walk cannot be done safely (e.g., the structure traversal would require a lock that an existing thread holds), surface and PM clarifies. Do not skip the channel dump silently — that defeats the dispatch's load-bearing purpose.

### 2.3 PROGRESS frames in stress orchestrator (test-only)

In `kernel/src/test_harness.rs`, inside `run_smp_cross_cpu_rendezvous_stress`, emit a PROGRESS frame every 50 iterations:

```rust
if seq % 50 == 0 && seq > 0 {
    serial_println!(
        "NEO_TEST/1 event=PROGRESS name=smp_cross_cpu_rendezvous_stress iteration={} pair_count={} last_blocked_cpu={} last_completer_cpu={}",
        seq, pair_count, last_blocked_cpu, last_completer_cpu
    );
}
```

Emission cadence: every 50 iterations, ~20 frames per 1000-iteration run. Negligible overhead vs the existing scenario emit pattern.

`last_blocked_cpu` and `last_completer_cpu` are tracked via small-state additions to the orchestrator. Cerebro decides the storage shape (atomic, per-pair array, etc.).

If `seq` is a per-pair counter rather than a global iteration counter, adjust the frame to emit `pair=K iteration=N` at the same 50-step cadence per-pair. PM defers naming/shape to Cerebro; what matters is having ~20 progress markers per 1000-iteration run.

### 2.4 cpu_id tracking on workers (test-only)

At each `block_current_into` call site inside the stress workers (sender-blocks-first and receiver-blocks-first paths), emit:

```
NEO_TEST/1 event=WORKER name=stress-{sender|receiver} pair=K iter=N action={pre_block|post_wake} cpu_id=X
```

The `pre_block` frame fires immediately before the worker enters `block_current_into`; the `post_wake` frame fires immediately after the block returns. This reveals the cpu_id at block time and after wake — directly testing whether Q4's wake-on-completer-CPU pattern is being honored under stress.

Volume concern: at 1000 iterations × ~4 worker emit points per iteration × ~K pairs, this is several thousand frames per run. If serial throughput becomes a bottleneck (i.e., emit overhead masks the bug timing), reduce to one emit pair per N iterations (cadence matched to PROGRESS).

PM lean: emit at full cadence first; if Cerebro observes runs taking materially longer than Phase B's 13s avg / 108s max, dial back. The criterion is "instrumentation must not change the failure-rate enough to mask the bug." If average run time exceeds ~30s, that's the dial-back threshold.

### 2.5 What does NOT change in this dispatch

- No kernel logic changes. No scheduler changes. No IPC changes. No APIC changes. The Sub-step 5b infrastructure is unchanged.
- No Fix 6.1 application. Stash `{0}` stays untouched.
- No commit lands. All deltas are working-tree-only.
- No NMI threshold change from Outcome B's 10s post-BEGIN silence. Same trigger, same monitor port pattern.
- No change to test scenario logic (orchestration, pair count, iteration count, direction assignment). Instrumentation is additive observation only.

---

## 3. Pre-run checklist

Before kicking the 50-run loop:

- [ ] `git stash apply stash@{1}` succeeds.
- [ ] NMI patch adapted (`is_locked()` → `try_lock()`) per Phase B / Outcome B precedent.
- [ ] Channel state dump extension added to NMI handler. Try-lock discipline confirmed: handler does not block on channel locks.
- [ ] PROGRESS frames added to stress orchestrator (~20 per run).
- [ ] cpu_id tracking added to worker block/wake points.
- [ ] `docker compose run --rm neo cargo k` passes. Document warning count delta from 18-baseline. Tolerable delta for instrumentation: up to +5; beyond that, halt and ping.
- [ ] All five sibling SMP scenarios pass single-run sanity at NEO_SMP=4 (instrumented stress is the sixth; tested separately below).
- [ ] Single-run `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 still produces a PASS frame at the end. The PASS path must work — instrumentation cannot break the success case.
- [ ] Captured single-run output shows ~20 PROGRESS frames + ~thousands of WORKER frames + a final PASS frame. Visual inspection confirms the new frames are well-formed.

If any of these fail, halt and ping before kicking the loop.

---

## 4. Run shape

50 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 with NMI armed at 10s post-BEGIN serial silence threshold. Match Outcome B's run shape:

- Artifact directory: `target/verify/sc3b-phase-c-nmi/neo-smp4/`.
- NMI monitor enabled with `NEO_QEMU_MONITOR_PORT`.
- Host runner sends HMP `cpu N` then `nmi` for CPUs 0..3 after 10s post-BEGIN silence.
- Per-run files: `run-NNN.serial.log`, `run-NNN.xtask.log`, `run-NNN.nmi.serial.log` (when NMI fires), `run-NNN.cargo.log`.
- Results index: `target/verify/sc3b-phase-c-nmi/neo-smp4/results.jsonl`.

**Early-stop discipline:**
- On first NMI capture, complete the run (let host wrapper kill QEMU after capture) and stop the loop. The capture is what matters; additional captures at this stage have diminishing returns and Phase C's analytical value comes from a clean reading of one well-instrumented capture.
- On any classifier-signature failure other than NMI capture (e.g., a kernel panic or scenario FAIL frame), stop and report.
- If 50 runs complete with zero NMI captures (variance sampled clean), stop. Report the negative result with timing distribution and let PM decide next move.

---

## 5. Findings document

Write `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-09.md` (or 05-10 if calendar advances) with structure mirroring `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`:

1. **Substrate.** HEAD, stashes, applied patches, build PASS state, warning count.
2. **Run summary.** 50 runs, classification breakdown, timing distribution.
3. **Per-trace analysis** for each NMI capture run:
   - Serial log up to and including BEGIN.
   - The PROGRESS frame trail — last few PROGRESS frames before silence.
   - The last WORKER frames before silence (especially: which pair, which action, what cpu_id, did a `pre_block` fire without a matching `post_wake`).
   - CPU0 NMI IP + symbol resolution (per Outcome B precedent).
   - Per-CPU scheduler snapshot.
   - **Per-channel state dump** (the load-bearing addition).
   - Comparison to Outcome B run 026's snapshot.
4. **Pattern across captures** if more than one fired.
5. **Comparison to closure-gate retry run 010 and Sub-step-5b run 052** by serial signature.
6. **Recommendation.** Per-hypothesis evidence breakdown:
   - H1-variant supported / disfavored / unresolved.
   - Hp-residual supported / disfavored / unresolved.
   - Mechanism-not-yet-named: any new pattern observed.
   - Specific next-step recommendation.
7. **Exit status.** Tree restore confirmation, stash state, backup hash.

---

## 6. Outcomes and forward path

### 6.1 Outcome C-A — channel state dump shows parked threads

H1-variant confirmed (or at least directly supported). PM produces a fix-design dispatch:
- Identify the lost-wakeup site by tracing from the parked thread ID + endpoint state back to the IPC code path that should have woken it.
- The fix is likely in the IPC retrofit (one of the four block sites — send, recv, grant, recv_grant — has a wake call that's missing or misordered).
- Fix lands as a kernel commit on top of `2d8f65d`. Re-run BUILD §9 Step 6 + Step 7.

### 6.2 Outcome C-B — scheduler shows new mechanism pattern

Workers running on APs but no parked threads in channel state, OR a layout pattern not seen in Outcome B run 026. The mechanism is somewhere we haven't named.

PM either dispatches a Janus second-pass (recurrence trigger fires post-Phase-C: same failure now recurred across two distinct fix attempts — Fix 6.1 attempt + Sub-step 5b IPI — with internal investigation visibly partial), or designs further targeted instrumentation, or both in parallel per Outcome B precedent.

Janus second-pass would target the new signature space specifically; the first-pass artifact at `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` becomes prior context.

### 6.3 Outcome C-C — same Outcome-B layout (CPU2/CPU3 ap-N-main running with workers ready)

The IPI is landed but isn't actually breaking the spin. This is a Sub-step 5b regression — the architectural fix didn't behave as designed.

PM dispatches an IPI delivery audit: read the LAPIC error register on each CPU at hang time, verify the ICR delivery status didn't stick, verify the vector 0xfd actually maps to the reschedule handler in the loaded IDT, verify the EOI mechanism works (a stuck pending-interrupt would bias against the IPI). The Sub-step 5b commit stays landed regardless; the audit determines whether the IPI infrastructure itself needs a follow-up patch.

### 6.4 Outcome C-D — no NMI captures across all 50 runs

Variance sampled clean. Combined with Phase B's 100/100 + post-IPI 51/52 PASS, this would be 200+ clean runs across multiple windows post-Fix-6.1-failure. Statistically uneven (the gate failure sequences imply a true rate above 0.5%), but informative.

PM either redesigns instrumentation to bias toward catching the hang (longer iteration count, higher pair count, deliberate host load), escalates to Janus for a second deep-research pass on the post-IPI signature, or both.

### 6.5 Forward path summary

```
Phase C run ──Outcome C-A──→ Fix-design dispatch (lost-wakeup site fix) → Cerebro
            │
            ──Outcome C-B──→ Janus second-pass + targeted instrumentation
            │
            ──Outcome C-C──→ IPI delivery audit dispatch → Cerebro
            │
            ──Outcome C-D──→ Instrumentation re-design + possible Janus second-pass
```

---

## 7. Stop conditions

Halt and ping PM if:

- The cargo k warning delta exceeds +5 above the 18-baseline.
- Pre-run single-run instrumented stress fails to produce a PASS frame (instrumentation broke the success path).
- Average run time during the 50-run loop exceeds ~30s (instrumentation overhead is masking the bug).
- Stash `{0}`, `{2}`, or `{3}` is touched accidentally.
- The NMI handler's channel state walk deadlocks or panics (channel locking discipline failed).
- A sibling SMP scenario regresses post-instrumentation.
- Any classifier-signature failure other than NMI capture during the loop (e.g., a kernel panic, FAIL frame from a PROGRESS-emit assertion, etc.).
- Backup script reports failure for two consecutive attempts.
- Tree restore at exit shows non-zero diff against HEAD `2d8f65d`.

If a stop condition fires, the dispatch's substrate guarantee (HEAD `2d8f65d` clean exit, four stashes preserved) holds — Cerebro restores tree state per §0.1 of the dispatch convention.

---

## 8. Estimated wall time

- Apply stash + adapt + Phase C deltas: ~60–90 minutes (channel state dump is the new work; PROGRESS frames and cpu_id tracking are mechanically straightforward).
- Pre-run sanity: ~30 minutes (cargo k + sibling scenarios + single-run stress).
- 50-run loop: ~10–60 minutes (matched to Phase B / Outcome B; varies with how early NMI fires or whether all 50 complete).
- Findings document: ~30–45 minutes.
- Tree restore + backup: ~10 minutes.

Total: ~2–4 hours from dispatch acceptance to findings-document delivery.

---

## 9. Cross-references

**Authority for this dispatch:**
- `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md` — Outcome B confirmed, run 052/200 missing-terminal-frame.
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 — hypothesis ranking framework; H1-variant identified as load-bearing-unresolved.
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` §"Recommendation" — PM-acknowledged gap: "channel endpoint state was not dumped"; Phase C is the closure of that gap.
- `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md` §6.2 — Phase C contents pre-specified at dispatch time.

**Substrate context:**
- HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0` — Sub-step 5b commit (IPI infrastructure landed; xAPIC enabled on all 4 CPUs verified per `apic: cpu=N lapic_id=N enabled xAPIC` boot lines).
- Backup `Z:\AltimitNeo\2026-05-09-224332` with `artifacts/` subdirectory containing the Step 6 verification logs.

**Investigation chain (Phase C is third in this arc):**
- Phase A — triple-parallel investigation: `Plans/SC-3B-INVESTIGATION-{cerebro,prometheus,claude}-2026-05-09.md` + synthesis at `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`.
- Phase B — NMI watchdog instrumentation pass: `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md`, `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`.
- Phase C — this dispatch: `Plans/DISPATCH-2026-05-09-sc-3b-phase-c-instrumentation-cerebro.md`.

**Stash usage:**
- `stash@{1}` (NMI watchdog) — applied as base. Adaptation pattern (`is_locked()` → `try_lock()`) per `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md` and `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md` precedent.
- `stash@{0}`, `stash@{2}`, `stash@{3}` — not touched. Disposition unchanged.

**Prior-art reference:**
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` §1.2 — Asterinas PR #2488 ("init not woken after dead children moved"), Linux `wake_q` discipline ("task must already be ready for an immediate wake at queueing point"). Both reinforce that lost-wakeup sites cluster around state-transition handoffs, which is what H1-variant predicts.

**Governing posture:**
- `docs/bible.md` Articles 1.2 (correctness > coherence > performance — Phase C produces evidence, not a fix; correctness of the eventual fix depends on Phase C's evidence quality), 14 (cadence — investigation phase between Outcome B and the next BUILD), 15.2 (SAFETY comments — the channel state dump's NMI-context access patterns must carry SAFETY comments justifying lock-free read discipline).
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — forward concern: H1-variant in IPC handoff would affect AI inference workloads under capability-passing IPC. Phase C's diagnostic value extends past SC-3B closure.

**Routing rule applies:**
- Per `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md` §2.4 — Phase C kernel/test instrumentation goes to Cerebro. If Phase C produces an Outcome C-B / C-D signature, a second Janus deep-research pass is the likely next move (recurrence trigger satisfied: same failure now across two fix attempts + Phase B null).

---

**End of dispatch.**

*Phase C: apply stash {1} + add channel state dump to NMI handler + PROGRESS frames every 50 iterations + cpu_id tracking on worker block/wake points. 50-run loop on post-Sub-step-5b substrate, NMI armed at 10s post-BEGIN silence, early-stop on first capture. No commit lands; substrate returns clean at HEAD `2d8f65d`. Findings document at `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-09.md`. Outcome C-A → fix-design (H1-variant confirmed). Outcome C-B → Janus second-pass + targeted instrumentation. Outcome C-C → IPI delivery audit. Outcome C-D → instrumentation redesign + possible Janus. Estimated 2–4 hours. PM is on call.*
