# CLOSE-REPORT: SC-3B — Phase 2 BUILD-3 sub-commit 3 (Q4/Q6 IPC retrofit closure)

**Date filed:** 2026-05-10
**Author:** PM
**Status:** Final. SC-3B closed.
**Closure commit:** `e2b3f4f15a449ea828c1329e4182413cf6345a3a` — `build-3 sub-step 5c: enforce irq-safe acquisition for Channel.state`
**Verification log:** `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`
**Closure gate result:** 200/200 NEO_SMP=4 PASS, 25/25 NEO_SMP=2 PASS, `cargo k` PASS with 18 warnings (baseline, delta 0).

---

## 1. TL;DR

SC-3B is closed. The intermittent `smp_cross_cpu_rendezvous_stress` MISSING_TERMINAL_FRAME failure at NEO_SMP=4 — first surfaced after the Q4/Q6 IPC retrofit on 2026-05-09 — was caused by the **Hpw mechanism**: preempt-while-holding-a-spinlock combined with an IRQ-off same-CPU waiter, producing a single-CPU self-deadlock. The mechanism is now structurally foreclosed for all audited `Channel.state` call sites by wrapping process-context acquirers in `without_interrupts`. Five investigation passes (Phases A through D, with a fifth pass via Janus second-pass research) over two days were required to name the mechanism precisely. The fix is small (five call sites in `kernel/src/ipc/mod.rs`), surgical (no SwitchMutex changes, no scheduler changes), and verified by 200 consecutive PASS boots at the high-stress configuration.

---

## 2. Investigation arc

The arc spanned 2026-05-09 through 2026-05-10 across five distinct investigation passes, each adding visibility the prior couldn't reach.

### 2.1 Phase A — Three-way investigation (2026-05-09)

Parallel investigations by Cerebro (kernel-code mechanism hypotheses), Janus (silent-timeout prior art search), and PM (synthesis). Produced:
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` (Janus first-pass)

Hypothesis set produced: Hd (deadlock AB/BA), Hr (release-on-switch ordering), Hc (contention timing), Hs (stuck holder). None confirmed; all remained candidates.

### 2.2 Phase B — NMI watchdog (2026-05-09)

Added NMI watchdog instrumentation to capture the hang state at the failure moment, since the failure was silent (no panic, no log). Outcome B verified the watchdog fires on the silent-timeout failure mode but didn't immediately localize the mechanism — a re-arm issue had to be fixed before the watchdog could capture useful state.

Produced:
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md`

### 2.3 Phase C — NMI + instrumentation (2026-05-10 morning)

Added PROGRESS frames + cpu_id worker tracking + channel state dump in the NMI handler. At run 5/50 the watchdog fired with diagnostic capture: pair 3 receiver was in pre-block state, channel ch=3 was LOCKED, CPU1 scheduler.inner was LOCKED, CPU3 ap-3-main had 3 ready stress workers. The instrumentation captured the symptom precisely but the mechanism behind the lock state was still unnamed — Outcome C-B.

Produced:
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-c-instrumentation-cerebro.md`
- `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md`

### 2.4 Phase D — Lock-ownership probe (2026-05-10 midday)

Added SwitchMutex owner metadata (cpu/thread_id/site, seqlock discipline including the asm release path), multi-probe NMI lock readout, thread-locate walk, and BLOCK_TRACE emit points in the IPC block path. **Captured the smoking gun at run 43/50**: CPU0 was in `_mm_pause` with IF=0; channel ch=3 was locked across all three NMI probes; the owner was thread 0 (main) in `Ready` state on CPU0; the pair-3 receiver had emitted pre_block but no BLOCK_TRACE.

The picture this resolved to: the stress orchestrator's `wait_for_stress_endpoint_blocked` calls `debug_endpoint_is_recv_blocked` which acquires `Channel.state` via plain `self.state.lock()` with preemption ENABLED. The timer preempts main while main is holding the lock. The receiver (which arrives via IPC syscall, IF=0) tries to acquire the same lock, spins forever. Main can't run because the IF=0 spinner is on the same CPU. Single-CPU self-deadlock.

Mechanism named: **Hpw — preempt-while-holding-lock + irq-off same-CPU waiter.**

Produced:
- `Plans/DISPATCH-2026-05-10-sc-3b-phase-d-lock-ownership-cerebro.md`
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`

### 2.5 Janus second-pass — release-on-switch prior art (2026-05-10, parallel to Phase D)

Dispatched in parallel with Phase D to look for prior art on release-on-context-switch lock primitives. Janus found Linux/FreeBSD/xv6/Zircon all have scheduler-special-case lock handoff disciplines; Redox is the closest Rust match via `switch_finish_hook`. No exact prior art for the two-lock asm release bug shape we were investigating, but the broader bug class (ownership-metadata-drift + IRQ-discipline-mismatch) is well-documented.

Janus's second-pass confirmed independently what Phase D was showing on instrumentation: this is a known bug class with known mitigation patterns. The mitigation that applies to NeoPlutarch is the IRQ-safe-acquisition discipline, not the release-on-switch primitive.

Produced:
- `Plans/DISPATCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`

---

## 3. Hypothesis tracking — final

| Hypothesis | State | Notes |
| --- | --- | --- |
| Hd — deadlock AB/BA | **Refuted** | All scheduler locks were unlocked at NMI capture; no two-lock deadlock pattern observable. |
| Hr — release-on-switch ordering | **Refuted** | The releaser was a non-blocked-yet thread; release ordering was not the failure path. |
| Hc — contention timing | **Refuted** | Lock state was stable across three NMI probes with the same owner — not transient contention. |
| Hs — stuck holder | **Adjacent** | The holder was not stuck *inside* the block path; it was *preempted while holding*. Same family of failure but different exact mechanism. |
| **Hpw — preempt-while-holding + irq-off same-CPU waiter** | **CONFIRMED** | Captured at Phase D run 43/50. Necessary conditions: (1) lock acquired in preemptible context, (2) same lock acquired in IF=0 context, (3) both contexts can run on the same CPU. All three conditions held for the affected `Channel.state` call sites. |

---

## 4. The fix

Sub-step 5c, landed as commit `e2b3f4f`. Five `Channel.state` call sites that acquired the lock in preemptible process context were wrapped in `without_interrupts(...)` to disable local interrupts across the critical section. Specifically:

- `Channel::fail_recv_grant`
- `Channel::debug_endpoint_is_grant_blocked`
- `Channel::debug_endpoint_is_send_blocked`
- `Channel::debug_endpoint_is_recv_blocked`
- `Channel::close_peer` (wake performed after guard drop)

Four sibling call sites (`Channel::send`, `Channel::recv`, `Channel::grant`, `Channel::recv_grant`) were already IRQ-off by their syscall/test callers; they received call-site discipline comments noting this invariant but no structural change.

**What the fix is NOT:**
- Not a SwitchMutex change. The Phase D ownership-metadata probe was instrumentation; the seqlock owner metadata it added was reverted after the gate (no production-stable diagnostic affordance landed yet — see carry-forwards).
- Not a scheduler change. Preemption discipline at the scheduler level was not modified.
- Not a SwitchMutex-by-construction change. The structural "all SwitchMutex acquisitions are IRQ-off" change is a carry-forward ADR, not part of Sub-step 5c.

The discipline established by Sub-step 5c is local: the audited `Channel.state` call sites obey IRQ-off-on-acquire. The structural enforcement of this discipline kernel-wide is the carry-forward.

---

## 5. Verification gate results

Reproduced from `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md`:

| Gate | Result | Notes |
| --- | --- | --- |
| `cargo k` pre-commit | PASS | 18 warnings, baseline delta 0, 0 new unsafe-op-in-unsafe-fn warnings |
| `cargo neo-test all` at NEO_SMP=4 | PASS | 34 serial logs; 33 PASS; 1 deliberate-FAIL meta-test frame |
| Pre-commit smoke (5 consecutive `smp_cross_cpu_rendezvous_stress` at NEO_SMP=4) | 5/5 PASS | Timings 65.1s – 76.3s |
| Closure gate at NEO_SMP=4 (200 boots) | **200/200 PASS** | Timings 62.8s min, 510.9s max, 112.9s avg |
| Closure gate at NEO_SMP=2 (25 boots) | **25/25 PASS** | Timings 78.4s min, 88.3s max, 82.4s avg |
| `cargo k` post-rename HEAD `cca6889` | PASS | 18 warnings, baseline delta 0 |
| Backup to Synology | PASS | `Z:\AltimitNeo\2026-05-10-121704` |

### 5.1 Run 82 measurement-infrastructure event

The raw `results.csv` for the NEO_SMP=4 gate contains one unscored `run=82 exit_code=137 classification=NO_SERIAL` row. Cerebro's verification log explains: the host command timed out after run 81, leaving an orphan `docker compose run` for run 82. Cerebro stopped the orphan before it reached a serial terminal frame and reran run 82 cleanly. The rerun passed and is the scored attempt. The scored gate count is 200 completed PASS boots.

This handling is the correct discipline. The exit_code=137 NO_SERIAL row is a measurement-infrastructure event (host process killed orphan container), not a kernel event. Pulling it out of the scored count and re-running cleanly preserved the gate's signal — pretending the row was a kernel failure would have been false-negative noise; pretending it didn't exist would have been dishonest. The explicit disclosure in the verification log is a positive calibration data point for Cerebro.

---

## 6. Janus dispatches and calibration

SC-3B used Janus across two dispatches plus the broader project-state TM pre-flight:

| Janus dispatch | Topic | Calibration verdict |
| --- | --- | --- |
| First-pass (2026-05-09) | Silent-timeout prior art across Linux/QEMU/Asterinas/Theseus | Spot-checked at high fidelity per `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`. Verbatim numerical match (Richard Jones's 292,612 boots, QEMU 8.0.0 ~1-in-300 rate) was a strong positive signal. |
| Second-pass (2026-05-10) | Release-on-switch prior art for Hpw-adjacent bug class | Consumed at somewhat-elevated review intensity (per the calibration step-down from spot-check). No anomaly observed. Asterinas + Redox + xv6 + Zircon citations consistent with substrate evidence. |
| Rename TM pre-flight (2026-05-10) | NeoPlutarch trademark and namespace cleanliness | Returned "Proceed with caveats" per `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`. Primary government TM databases handled honestly with explicit access-control caveats. |

Across three dispatches, Janus's calibration profile has firmed up from "elevated" through "somewhat-elevated" toward normal trust. Per `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md` §3.3, the post-2-3-dispatches calibration prediction tracks accurately. Future Janus dispatches consume at normal review intensity unless an anomaly surfaces.

---

## 7. Coordination outcome — Cerebro × Prometheus parallel execution

Sub-step 5c is the first time the project ran Cerebro and Prometheus in true parallel: Cerebro's kernel gate (200 boots at NEO_SMP=4, ~112s avg) was a 6+ hour wall-clock operation, and during that window Prometheus's three-commit rename dispatch landed bible v1.2.0, ADR-017, and the rename on the same local repo.

This worked because the dispatch I gave Prometheus had a **negative-list discipline**: `kernel/`, `boot/`, and `tools/` were explicitly off-limits, and `Plans/` historical artifacts were preserved unchanged. Prometheus touched only `docs/` and `README.md`. The verification log confirms the diff from `e2b3f4f` to `cca6889` touched only `README.md` and `docs/**`; no kernel/userland/xtask source changed. The gate's validity is unaffected — it validates `e2b3f4f`, the parent ancestor of the rename HEAD.

**Positive pattern to internalize:** disjoint file-set parallelism between coding and docs agents is a real productivity multiplier. The dispatch design (explicit negative list, explicit Plans/ preservation) made it safe by construction. Future dispatches with long-running coder gates should adopt the same pattern.

---

## 8. Carry-forwards (queued for Sub-step 6 docs dispatch)

Sub-step 6 is the documentation closure for SC-3B and will be dispatched to Prometheus after this CLOSE-REPORT lands. The following items are queued:

### 8.1 ADR proposals (three new ADRs)

**ADR proposal A: Bible Article 8.5.4 clarification — preempt-while-holding case.** The current 8.5.4 ("No blocking while holding a spinlock") conceptually covers Hpw but doesn't name the preempt-while-holding case explicitly. Proposed clarification: spinlocks acquired in preemptible context where the same lock may be acquired in IF=0 context on the same CPU MUST be acquired with local interrupts disabled. This is the discipline Sub-step 5c implemented locally; the ADR generalizes it kernel-wide.

**ADR proposal B: SwitchMutex IRQ-off by construction.** Type-level distinction between IRQ-safe and preemptible lock primitives. `SwitchMutex` (or a renamed variant) becomes IRQ-off by type — the API forces local interrupts off on acquire and restores prior state on drop. Plain `spin::Mutex` remains for exclusively-process-context or exclusively-ISR-context locks. This eliminates the class of bug Sub-step 5c fixed by making it un-expressible in the type system.

**ADR proposal C: Owner metadata as production-stable diagnostic affordance.** The Phase D ownership-metadata probe (cpu/thread_id/site, seqlock discipline) was instrumentation that Sub-step 5c reverted. The diagnostic value was real — it was the key signal for naming Hpw. Proposed: a feature-gated or always-on lightweight owner-metadata field on SwitchMutex (and possibly other lock types) with seqlock-protected reads from interrupt context. Cost is small (one cache line per lock); diagnostic value on future similar bugs is high.

### 8.2 ADR amendments

**ADR-011 supersession.** ADR-011 (single-CPU IPC handoff invariants) predates Sub-step 5c and assumes pre-SMP IPC discipline. Sub-step 5c's IRQ-safe acquisition discipline strictly extends ADR-011's model. Sub-step 6 should mark ADR-011 as superseded by the new ADR (proposal A above) once that ADR lands.

**ADR-013 §Q4/§Q6/§Q7 amendments.** ADR-013 (Phase 2 SMP retrofit) documents the retrofit plan. Sub-step 5c is the closure of SC-3B which is the closure of Q4/Q6 (per the original investigation framing). ADR-013 needs amendments to §Q4/§Q6 noting closure and to §Q7 noting the Hpw discovery as a structural lesson for future SMP retrofit steps.

### 8.3 Bible amendment proposal

**Article 16.4 bench surface expansion.** Per the Redox DWRR PM-NOTE prior-art triage (queued separately at `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` — currently not yet on disk; Prometheus has a queued dispatch to file it), Article 16.4's enumerated benches gain a scheduling-fairness benchmark item. Small addition to Article 16.4's existing enumeration; minor bible version bump (or none if treated as enumeration-extension housekeeping).

### 8.4 Phase document updates

**PHASE-2.md update.** Mark SC-3B and sub-step 5c complete. Include verification log reference, gate scores, and the Hpw mechanism finding. Link the three ADR proposals from §8.1.

### 8.5 INDEX.md sweep

Many new artifacts landed in `Plans/` during the SC-3B investigation. INDEX.md needs a sweep to incorporate them under the appropriate sections.

### 8.6 Outside-Sub-step-6 carry-forwards

**Kernel-side rename completion.** A separate small dispatch (Cerebro + Prometheus joint) covering `kernel/Cargo.toml` package name (`altimitneo` → `neoplutarch`), kernel source comments referencing AltimitNeo, `boot/limine.cfg` boot title if applicable, and `tools/xtask/` references. Now unblocked since SC-3B is closed. Estimated 30-45 minutes of execution.

**Push completion.** As noted at the top of the conversation that produced this CLOSE-REPORT: the four commits (`e2b3f4f`, `04f4a95`, `57439a6`, `cca6889`) exist on Claude Code worktree branches but main has not been advanced and origin has not received them. Project lead's fast-forward + push is the next operational step. Not blocking the CLOSE-REPORT itself; flagged as the outstanding state-of-git action.

**Redox DWRR PM-NOTE dispatch.** The Prometheus dispatch to file `Plans/PM-NOTE-2026-05-10-redox-dwrr-prior-art.md` (given inline during conversation) appears not yet executed. Small follow-up; Prometheus can pick up alongside Sub-step 6.

---

## 9. Cost and timing summary

- **Investigation duration:** ~36 wall-clock hours (2026-05-09 morning through 2026-05-10 afternoon).
- **Investigation passes:** 5 (Phases A, B, C, D, plus the Janus second-pass parallel).
- **Cerebro dispatches:** 14+ across the SC-3B arc (audit, build, fix attempts, NMI capture, instrumentation, Phase D, sub-step 5c).
- **Janus dispatches:** 2 within SC-3B (first-pass, second-pass) plus the rename TM pre-flight.
- **PM-NOTEs and findings:** 20+ artifacts in `Plans/` documenting investigation state.
- **Fix size:** five wrapped call sites in `kernel/src/ipc/mod.rs` plus discipline comments on four already-correct sites.
- **Gate cost:** ~6 hours wall-clock for the 200-boot NEO_SMP=4 gate alone.

The investigation-to-fix ratio (36 hours to land 5 wrapped call sites) is high in raw terms but appropriate for the failure class. Hpw is a single-CPU self-deadlock that surfaces intermittently under SMP stress; the failure was silent (no panic, no log) and not deterministically reproducible. Naming the mechanism precisely required watchdog-grade instrumentation that didn't exist at the start. Each investigation pass added the visibility the prior couldn't reach.

The alternative — patching by mechanical "add IRQ-safety everywhere" without naming the mechanism — would have landed a similar code change but without the structural lessons that drive carry-forwards §8.1 and §8.2. The 36 hours bought the *structural* discipline that prevents the entire bug class, not just one bug.

---

## 10. Cross-references

### 10.1 Verification log
- `Plans/SC-3B-SUB-STEP-5C-VERIFICATION-2026-05-10.md` — Cerebro's verification log; canonical record of gate results.

### 10.2 Investigation arc dispatches and findings
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-cerebro.md`
- `Plans/SC-3B-PLAN-CHECKPOINT-2026-05-09-prometheus.md`
- `Plans/SC-3B-PLAN-SYNTHESIS-2026-05-09.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-cerebro.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-prometheus.md`
- `Plans/SC-3B-INVESTIGATION-2026-05-09-claude.md`
- `Plans/SC-3B-INVESTIGATION-SYNTHESIS-2026-05-09.md`
- `Plans/SC-3B-SOURCE-AUDIT-2026-05-09.md`
- `Plans/SC-3B-PHASE-B-FINDINGS-2026-05-09.md`
- `Plans/SC-3B-OUTCOME-B-NMI-FINDINGS-2026-05-09.md`
- `Plans/SC-3B-PHASE-C-FINDINGS-2026-05-10.md`
- `Plans/SC-3B-PHASE-D-FINDINGS-2026-05-10.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09.md`
- `Plans/SC-3B-VERIFICATION-2026-05-09-fix-6-1.md`
- `Plans/SC-3B-SUB-STEP-5B-VERIFICATION-2026-05-09.md`

### 10.3 PM notes and decisions
- `Plans/PM-NOTE-2026-05-09-sc-3b-think-stage.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-stress-gate-failure-triage.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-phase-b-outcome-and-closure-options.md`
- `Plans/PM-NOTE-2026-05-09-sc-3b-outcome-b-synthesis.md`
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture.md`
- `Plans/PM-NOTE-2026-05-09-agent-roster-posture-janus-addendum.md`
- `Plans/PM-NOTE-2026-05-09-janus-recurrence-trigger.md`

### 10.4 Cerebro dispatches (SC-3B arc)
- `Plans/DISPATCH-2026-05-09-sc-3b-source-audit-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-build-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-fix-6-1-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-closure-gate-retry-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-investigation-parallel.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-plan-stage-parallel.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-nmi-capture-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-b-instrumentation-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-phase-c-instrumentation-cerebro.md`
- `Plans/DISPATCH-2026-05-09-sc-3b-sub-step-5b-ipi-infrastructure-cerebro.md`
- `Plans/DISPATCH-2026-05-10-sc-3b-phase-d-lock-ownership-cerebro.md`
- `Plans/DISPATCH-2026-05-10-sc-3b-sub-step-5c-irq-safe-lock-discipline-cerebro.md`

### 10.5 Janus dispatches and research
- `Plans/DISPATCH-2026-05-09-sc-3b-outcome-b-janus-deep-research-janus.md`
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md`
- `Plans/DISPATCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`
- `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md`

### 10.6 Bible and ADRs landed during the SC-3B period
- `docs/bible.md` v1.0.5 → v1.1.0 → v1.2.0 → v1.3.0 (rename)
- `docs/ADR-017-wifi-support-strategy.md`
- `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md`
- `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md`

### 10.7 Handoffs
- `Plans/HANDOFF-2026-05-09-end-of-day-sc-3b-investigation-complete.md`
- `Plans/HANDOFF-2026-05-09-sc-3b-sub-step-5b-in-flight.md`

---

**End of CLOSE-REPORT.**

*Per Bible Article 13's phase-completion criteria, SC-3B's closure is recorded by this CLOSE-REPORT, the verification log, and commit `e2b3f4f`. Phase 2 BUILD-3 sub-commit 3 is complete.*
