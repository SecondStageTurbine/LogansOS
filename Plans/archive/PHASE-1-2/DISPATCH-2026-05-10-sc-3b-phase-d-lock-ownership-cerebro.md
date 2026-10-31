# Dispatch: SC-3B Phase D — lock-ownership probe + block-path narrow tracing

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-10
**Type:** Instrumentation-only investigation dispatch. Per Bible Article 14 — investigation phase, not BUILD. No commit lands. Substrate at exit identical to entry. Single-recipient. Targeted by Cerebro's Phase C "Recommended next dispatch" §Recommendation; PM ratifies and specifies.
**Authority:** `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md` §"Recommendation" (lock-ownership instrumentation directly identified by Cerebro as the next useful probe). `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 (hypothesis ranking framework). The Phase C capture's per-trace analysis (pair=3 receiver mid-block, channel ch=3 locked, CPU1 scheduler.inner locked, CPU3 with three workers Ready behind ap-3-main).
**Substrate:** Project HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0`. Tracked working tree clean per Cerebro's Phase C exit. Stash list `{0}` Fix 6.1, `{1}` NMI watchdog, `{2}` wedge-instr-2, `{3}` h1-experiment. Backup `Z:\AltimitNeo\2026-05-10-090353` captures clean post-Phase-C state.

---

## 0. Standing instructions

1. **No commit lands.** All instrumentation is investigation-only. Substrate at exit is identical to entry.
2. **Stash `{1}` is applied as base.** Phase D adds two instrumentation deltas on top: SwitchMutex owner metadata + narrow trace frames in `block_current_into_with_guard`.
3. **Stashes `{0}`, `{2}`, `{3}` not touched.** Any apply/pop affecting them is a halt-and-ping condition.
4. **Pre-run sanity required.** After applying stash + Phase D deltas: `cargo k` passes at 18-warning baseline (delta tolerable up to +5 for instrumentation). Five sibling SMP scenarios pass single-run sanity at NEO_SMP=4. Instrumented stress passes single-run sanity at NEO_SMP=4 (the PASS path must work).
5. **Backup runs after capture, before tree restore.**
6. **Stop conditions are honored** (§7).
7. **PM is on call.**

### 0.1 Honest framing

Phase D is data collection; the data answers a specific question: *who holds channel ch=3's state lock and CPU1's scheduler.inner at hang time, and where in the block-and-park sequence is the receiver actually stuck?* Three of four plausible mechanism classes resolve clean from this data:

- **Hd (deadlock-shape, AB/BA on channel + scheduler):** owner metadata shows two threads each holding one of the locks, each reaching for the other.
- **Hs (stuck holder, single-thread non-progress):** owner metadata shows one thread holding the lock and its narrow-trace position pinpoints the stuck site inside `block_current_into_with_guard`.
- **Hr (release-on-switch ordering bug):** owner metadata shows a thread that has *already context-switched* still recorded as owner — meaning the asm release didn't actually drop the lock byte, even though the switch happened. This is the most subtle and the one the Janus parallel research is targeting.
- **Hc (contention-only, NMI-fire timing artifact):** the owner changes between probe-1 and probe-2, demonstrating the lock isn't held forever — the Phase C snapshot just caught contended state.

Phase D is not expected to close the gate. It is expected to disambiguate Hd/Hs/Hr/Hc and feed the next dispatch (likely a fix-design dispatch on the post-Sub-step-5b substrate).

In parallel, a Janus second-pass deep-research dispatch goes out targeting prior art for "release-on-context-switch" lock primitives and the failure modes thereof. PM consumes both artifacts together for the next synthesis.

---

## 1. Goal

Capture lock ownership at hang time for the two locks observed held in Phase C — `Channel.state` (specifically channel ch=3's instance) and `PerCpuScheduler.inner` (specifically CPU1's instance) — plus narrow trace frames showing the receiver's progress through `block_current_into_with_guard`. Produce findings that disambiguate Hd / Hs / Hr / Hc.

Specifically:

1. SwitchMutex owner metadata (instrumentation-only, set on lock acquire / cleared on release). Read by NMI handler on try_lock failure.
2. Narrow trace frames inside `block_current_into_with_guard` at the four sequence points Cerebro identified.
3. Multi-probe in NMI handler: try_lock the same locks twice with a brief delay to disambiguate held-forever from contention timing.
4. Locate the receiver thread (id=14 in Phase C; will likely be a different id this run) on whichever CPU is currently running it; confirm it is the channel-lock owner.

---

## 2. Design specifications

### 2.1 SwitchMutex owner metadata

Add three fields to the `SwitchMutex` type (instrumentation-only, behind the same feature gate or stash visibility — PM defers shape to Cerebro):

```
struct SwitchMutex<T> {
    // existing fields ...

    // Instrumentation only. Racy by design; written non-atomically inside
    // the lock-held section. Read non-atomically by the NMI handler. Stale
    // reads are tolerable — the NMI is read-only diagnostic context. The
    // NMI handler reads owner_seq before and after to detect torn writes
    // (mismatch ⇒ display "owner_data_torn").
    owner_cpu: AtomicU32,            // CPU that acquired the lock
    owner_thread_id: AtomicU32,      // thread id at acquisition time
    owner_site: AtomicPtr<u8>,       // &'static str pointer, source-site label
    owner_seq: AtomicU64,            // bumped on lock and on unlock
}
```

`owner_site` is a `&'static str` pointer; passed into the lock acquisition path as a parameter or via a thread-local "current site" if Cerebro prefers. PM lean: site label is per-call-site, supplied at the lock() call. Each call-site gets a distinct `&'static str` like `"channel.send.block_current_into_with_guard"` or `"scheduler.rotate.yield_now"`.

Owner update sequence:

```
// At lock acquisition (after the lock byte transitions to held):
owner_seq.fetch_add(1, Relaxed);   // odd value = transition in progress
owner_cpu.store(cur_cpu, Relaxed);
owner_thread_id.store(cur_tid, Relaxed);
owner_site.store(site_ptr, Relaxed);
owner_seq.fetch_add(1, Relaxed);   // even value = data settled

// At lock release (before the lock byte transitions to free):
owner_seq.fetch_add(1, Relaxed);
owner_cpu.store(0, Relaxed);
owner_thread_id.store(0, Relaxed);
owner_site.store(null, Relaxed);
owner_seq.fetch_add(1, Relaxed);
```

The seqlock-style discipline means the NMI reader can detect torn writes. SAFETY comments justify the racy access pattern: the NMI handler reads diagnostic-only data, no synchronization invariants depend on it.

The `Q6` asm `context_switch_with_release2` releases the channel lock from the new stack. **The release path inside the asm helper must also clear the owner metadata.** This is the load-bearing detail for detecting Hr (release-on-switch ordering bug). If the asm releases the lock byte but doesn't clear owner metadata, NMI sees stale owner — Cerebro should design the metadata clear to happen *before* the lock byte release in the release sequence, mirroring the dual-write seqlock at acquisition. In asm, this means an extra two stores plus the seq increments before the lock-byte release. Cerebro decides the asm sequencing; PM specifies the invariant: owner metadata cleared atomically (seqlock-wise) with lock byte release, including in the asm release path.

### 2.2 NMI handler: lock owner readout + multi-probe

Extend the Phase C NMI patch to:

1. **Lock owner readout on try_lock failure.** When `sched.inner.try_lock()` returns None or `channel.state.try_lock()` returns None, read the owner fields with seqlock discipline:

```
[ NMI ] sched cpu=1 inner_locked=true owner_cpu=0 owner_thread_id=14 owner_site="channel.recv.block_current_into_with_guard"
[ NMI ] channel cpu=N ch=3 state=Locked owner_cpu=0 owner_thread_id=14 owner_site="channel.recv.block_current_into_with_guard"
```

If `owner_seq` is odd (transition in progress) on either read or the two reads disagree, emit `owner_data_torn` instead.

2. **Multi-probe sequence.** For each lock that fails the first try_lock, retry up to 3 times with a brief delay (e.g., a few hundred PAUSE iterations) between attempts. Emit each probe's result:

```
[ NMI ] channel cpu=N ch=3 probe=1 state=Locked owner_cpu=0 owner_thread_id=14 owner_site="..."
[ NMI ] channel cpu=N ch=3 probe=2 state=Locked owner_cpu=0 owner_thread_id=14 owner_site="..."
[ NMI ] channel cpu=N ch=3 probe=3 state=Locked owner_cpu=0 owner_thread_id=14 owner_site="..."
```

Or, if it transitions:

```
[ NMI ] channel cpu=N ch=3 probe=1 state=Locked owner_cpu=0 owner_thread_id=14 owner_site="..."
[ NMI ] channel cpu=N ch=3 probe=2 state=Idle parked_id=none
```

Three probes with low delay are sufficient to distinguish held-forever from contention. Probe spacing is not critical — what matters is that the NMI handler doesn't burn so long on probes that it itself becomes a confounding factor.

3. **Thread location walk.** After the per-CPU scheduler snapshot, scan all four scheduler ready/blocked queues looking for the recorded `owner_thread_id` from any locked channel. Emit:

```
[ NMI ] thread_locate id=14 found_on=cpu0_current state=Running rsp=0x...
```

Or, if the thread is in a ready queue:

```
[ NMI ] thread_locate id=14 found_on=cpu1_ready[2] state=Ready rsp=0x...
```

This pairs lock ownership with thread state. If the owner thread is in `current` for a CPU and that CPU's RIP is in `_mm_pause`, that's the stuck-holder signature (Hs / Hr).

### 2.3 Narrow trace frames in `block_current_into_with_guard`

Add four trace emit points along the receiver's block path in `kernel/src/sched/scheduler.rs::block_current_into_with_guard`:

```
NEO_TEST/1 event=BLOCK_TRACE phase=channel_guard_held channel_id=K thread_id=T cpu=N
NEO_TEST/1 event=BLOCK_TRACE phase=scheduler_lock_acquired channel_id=K thread_id=T cpu=N
NEO_TEST/1 event=BLOCK_TRACE phase=installer_complete channel_id=K thread_id=T cpu=N
NEO_TEST/1 event=BLOCK_TRACE phase=release_token_handoff channel_id=K thread_id=T cpu=N
```

The four phases match Cerebro's recommendation and the documented sub-step structure of `block_current_into_with_guard`. The `release_token_handoff` frame fires immediately before invoking `context_switch_with_release2`. After the asm switch returns control on the new stack to the *new* current thread, the parking thread doesn't emit further frames until it is woken and resumes — so a parked thread's visible trail ends at `release_token_handoff` followed (eventually) by a `post_wake` from the test harness.

If `release_token_handoff` fires but the thread never resumes (no `post_wake` follows), that's parked-and-not-woken. If `installer_complete` fires but `release_token_handoff` doesn't, that's stuck-before-asm. If `scheduler_lock_acquired` fires but `installer_complete` doesn't, that's stuck-mid-installer. And so on. The four-phase trail is a fingerprint.

Channel-id in the frame: pass the channel's identifier through to the block path. If `block_current_into_with_guard` doesn't currently know the channel ID, supply it via a parameter or a thread-local. PM defers wiring to Cerebro; what matters is that frames carry channel-id so the trace can be correlated with the locked channel from NMI.

Volume concern: at 1000 iterations × ~4 frames × multiple workers, this generates several thousand BLOCK_TRACE frames per run. If serial throughput is a bottleneck, conditional on `cfg!(feature = "neo-test-block-trace")` or a runtime sentinel and only enable for the targeted scenario.

### 2.4 What does NOT change

- No kernel logic changes. Owner metadata is observation-only; SwitchMutex behavior is unchanged. Block path behavior is unchanged.
- No commit lands.
- No NMI threshold change from Phase C (10s post-BEGIN silence, same monitor pattern).
- Sub-step 5b infrastructure (xAPIC, IPI handler, `place_thread_on_cpu` IPI send) is not modified.
- No PROGRESS / WORKER frame logic changes from Phase C — they continue to emit alongside BLOCK_TRACE frames.
- Stash `{0}` (Fix 6.1) stays untouched.

---

## 3. Pre-run checklist

- [ ] `git stash apply stash@{1}` succeeds; same `is_locked()` → `try_lock()` adaptation as Phase C.
- [ ] SwitchMutex owner metadata fields added; lock acquire/release paths updated (including the asm `context_switch_with_release2` release path).
- [ ] NMI handler extended to read owner metadata with seqlock discipline; multi-probe loop added; thread-locate walk added.
- [ ] Four BLOCK_TRACE emit points added to `block_current_into_with_guard`.
- [ ] `docker compose run --rm neo cargo k` passes. Document warning delta (tolerable up to +5).
- [ ] Five sibling SMP scenarios pass single-run at NEO_SMP=4 (instrumentation must not break them).
- [ ] Single-run instrumented `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4 still produces a PASS frame. Visual inspection confirms ~thousands of BLOCK_TRACE frames + ~20 PROGRESS frames + ~4000 WORKER frames + final PASS frame.
- [ ] No regression in average run time vs Phase C single-run sanity. If avg run time exceeds ~150s (vs Phase C's ~67s avg), instrumentation overhead is masking — halt and ping.

---

## 4. Run shape

Mirror Phase C exactly except artifact directory:

- Artifact directory: `target/verify/sc3b-phase-d-nmi/neo-smp4/`.
- 50 boots of `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4.
- NMI armed at 10s post-BEGIN serial silence threshold via `NEO_QEMU_MONITOR_PORT`.
- Host runner sends HMP `cpu N` then `nmi` for CPUs 0..3 after silence trigger.
- Per-run files: `run-NNN.serial.log`, `run-NNN.xtask.log`, `run-NNN.nmi.serial.log` on capture, `run-NNN.cargo.log`.
- Results index: `results.jsonl`.
- Early-stop on first NMI capture. If 50 runs complete clean, stop and report.

---

## 5. Findings document

Write `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md` with structure mirroring Phase C findings:

1. **Substrate.** HEAD, stashes, applied patches, build PASS state, warning delta.
2. **Run summary.** Count, classification, timing distribution, results.jsonl reference.
3. **Per-trace analysis** for each NMI capture:
   - Serial prefix.
   - PROGRESS frame trail (last few before silence).
   - WORKER frame trail (last few before silence).
   - **BLOCK_TRACE frame trail for the relevant receiver/sender thread** — load-bearing for identifying stuck site.
   - CPU0 NMI IP + symbol.
   - Per-CPU scheduler snapshot.
   - Per-channel state dump.
   - **Lock owner readouts** for every locked SwitchMutex — load-bearing.
   - **Multi-probe results** for every locked SwitchMutex — load-bearing for Hc disambiguation.
   - **Thread location walk** results for every distinct owner_thread_id.
4. **Pattern across captures** if more than one fired.
5. **Comparison to Phase C run 005's snapshot.**
6. **Mechanism classification.** Per-hypothesis evidence breakdown:
   - **Hd** (deadlock-shape): supported / disfavored / unresolved with cited frames.
   - **Hs** (stuck holder, single-thread): supported / disfavored / unresolved with cited frames.
   - **Hr** (release-on-switch ordering): supported / disfavored / unresolved with cited frames.
   - **Hc** (contention-only): supported / disfavored / unresolved with cited frames.
7. **Recommendation.** Specific next-dispatch recommendation. If a clean mechanism emerges, recommend a fix-design dispatch and identify the code-path target.
8. **Exit status.** Tree restore, stash state, backup hash.

---

## 6. Outcomes and forward path

### 6.1 Outcome D-A — Hd confirmed (deadlock-shape)

Owner metadata shows two distinct threads each holding one lock, multi-probe shows both held across the full probe interval, thread-locate places each owner on a different CPU in `_mm_pause` or similar.

**Forward:** PM produces fix-design dispatch. Likely fix shape: lock acquisition order audit at all four IPC retrofit sites; resolve any AB/BA pattern. May involve revisiting Q6's release order ratification (`SC-3B-PLAN-SYNTHESIS-2026-05-09.md` §3.2 ratified Channel-first, scheduler-second; Phase D may surface a path that violates this).

### 6.2 Outcome D-B — Hs confirmed (stuck holder)

Owner metadata shows one thread holding the lock; BLOCK_TRACE frames show it stuck at a specific phase (e.g., `installer_complete` emitted, `release_token_handoff` not).

**Forward:** PM produces fix-design dispatch targeting the specific stuck site. The BLOCK_TRACE phase pinpoints the code region; the fix narrows from there.

### 6.3 Outcome D-C — Hr confirmed (release-on-switch ordering bug)

Owner metadata shows a thread that has already been switched away from (no longer current on its original CPU) but is still recorded as owner. The asm release path didn't clear owner metadata atomically with the lock byte release — or, more concerningly, the asm release path didn't actually release the lock byte.

**Forward:** PM produces a kernel fix dispatch targeting the asm `context_switch_with_release2` release sequence. This is the highest-severity outcome — it would mean Q6's mechanism-correctness claim from the source audit was wrong, and a structural correction is needed. Janus second-pass output (parallel dispatch) is directly relevant here.

### 6.4 Outcome D-D — Hc supported (contention-only timing artifact)

Multi-probe shows lock state changing within probe sequence. The Phase C "channel locked" snapshot was a contention coincidence, not a deadlock or stuck holder.

**Forward:** Mechanism is elsewhere. The Phase C signature still includes "workers piling up on CPU3 behind ap-3-main" which Hc doesn't fully explain — that aspect of the snapshot remains unresolved. PM dispatches further targeted instrumentation focused on what makes workers yield-instantly when given CPU time on CPU3 (likely some other contended primitive or a state machine that's stuck in a re-yielding loop).

### 6.5 Outcome D-E — no NMI captures across all 50 runs

Variance sampled clean. Combined with prior data this would be a long string of clean runs post-Phase-C-style-instrumentation. PM either redesigns instrumentation, escalates iteration count / pair count to bias toward catching the hang, or treats Janus second-pass output as the load-bearing input for the next move.

### 6.6 Forward path summary

```
Phase D run ──D-A──→ fix-design dispatch (lock-order audit) → Cerebro
            │
            ──D-B──→ fix-design dispatch (stuck-site fix) → Cerebro
            │
            ──D-C──→ kernel fix dispatch (asm release path) → Cerebro
            │
            ──D-D──→ further instrumentation (CPU3 yield-instantly mechanism) → Cerebro
            │
            ──D-E──→ instrumentation redesign + lean on Janus second-pass output
```

In all branches, the parallel Janus second-pass artifact informs the synthesis.

---

## 7. Stop conditions

Halt and ping PM if:

- Cargo k warning delta exceeds +5 above the 18-baseline.
- Pre-run single-run instrumented stress fails to produce a PASS frame.
- Average run time during the 50-run loop exceeds ~150s (instrumentation overhead is masking the bug timing).
- Stash `{0}`, `{2}`, or `{3}` is touched accidentally.
- The NMI handler's owner readout deadlocks or panics (seqlock discipline failed).
- Asm `context_switch_with_release2` modifications break sibling SMP scenarios (instrumentation broke a load-bearing mechanism).
- Any classifier-signature failure other than NMI capture during the loop.
- Backup script reports failure for two consecutive attempts.
- Tree restore at exit shows non-zero diff against HEAD `2d8f65d`.

---

## 8. Estimated wall time

- SwitchMutex owner metadata + asm release path update: ~90–120 minutes (the asm release path update is the riskiest piece).
- NMI handler extensions (owner readout, multi-probe, thread locate): ~60–90 minutes.
- BLOCK_TRACE emit points: ~30 minutes.
- Pre-run sanity: ~30 minutes.
- 50-run loop: ~10–60 minutes (matched to Phase C; varies with how early NMI fires).
- Findings document: ~45–60 minutes (denser than Phase C since per-trace is richer).
- Tree restore + backup: ~10 minutes.

Total: ~4–6 hours from acceptance to findings delivery.

---

## 9. Cross-references

**Authority:**
- `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md` §"Recommendation" — direct identification of lock-ownership instrumentation as next probe.
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md` §3 — hypothesis ranking framework.

**Substrate context:**
- HEAD `2d8f65da77b4ea10f3d53226724e9381961c48c0` — Sub-step 5b (IPI infrastructure landed; xAPIC verified live on all 4 CPUs).
- Backup `Z:\AltimitNeo\2026-05-10-090353` (clean post-Phase-C exit).

**Investigation chain (Phase D is fourth in this arc):**
- Phase A — three-way investigation: `Plans/SC-3B-INVESTIGATION-{cerebro,prometheus,claude}-2026-05-09.md` + synthesis.
- Phase B — NMI watchdog: `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`.
- Outcome B / NMI re-arm: `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`.
- Phase C — NMI + instrumentation: `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md`.
- Phase D — this dispatch.

**Parallel work:**
- `Plans/DISPATCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus second-pass deep research, dispatched in parallel. Targeting prior art for "release-on-context-switch" lock primitives and known implementation pitfalls. Recurrence trigger satisfied per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §2.1.

**Stash usage:**
- `stash@{1}` (NMI watchdog) — applied as base. `is_locked()` → `try_lock()` adaptation per Phase C precedent.
- `stash@{0}`, `stash@{2}`, `stash@{3}` — not touched.

**Source files in scope:**
- `kernel/src/sync/switch_mutex.rs` — owner metadata addition.
- `kernel/src/sched/scheduler.rs::block_current_into_with_guard` — BLOCK_TRACE emit points.
- `kernel/src/arch/x86_64/context.rs:48-68` — `context_switch_with_release2` asm release sequence (owner metadata clear before lock byte release).
- NMI handler (in stash `{1}`) — owner readout, multi-probe, thread-locate extensions.

**Bible anchors:**
- Article 8.5.4 — no blocking while holding a spinlock. Phase D's load-bearing question is whether this invariant is being violated at runtime in some path (Hr) or whether an apparent violation is actually a release-ordering bug (also Hr).
- Article 14 — investigation-phase cadence; Phase D produces evidence, not fix.
- Article 15.2 — SAFETY comments on the racy owner-metadata access in NMI context.

**Forward concern:**
- `docs/altimitneo-ai-subsystem-spec.md` (ANSPEC-001) §3.7, §10.2 — the SwitchMutex+release-on-switch mechanism is on the critical path for AI inference workloads under capability-passing IPC. Phase D's findings (whichever outcome) inform that forward design.

---

**End of dispatch.**

*Phase D: SwitchMutex owner metadata + four BLOCK_TRACE emit points in block_current_into_with_guard + multi-probe NMI lock readout + thread-locate walk. Apply stash {1} as base; layer Phase D deltas on top. 50-run loop on post-Sub-step-5b substrate, NMI armed at 10s post-BEGIN silence, early-stop on first capture. No commit; substrate returns clean at HEAD `2d8f65d`. Findings at `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`. Outcomes D-A (Hd, deadlock) → lock-order fix dispatch. D-B (Hs, stuck holder) → site-targeted fix. D-C (Hr, release ordering bug) → asm release path fix. D-D (Hc, contention timing) → further instrumentation. D-E (no capture) → redesign + lean on Janus parallel output. Janus second-pass dispatched in parallel. Estimated 4-6 hours. PM is on call.*
