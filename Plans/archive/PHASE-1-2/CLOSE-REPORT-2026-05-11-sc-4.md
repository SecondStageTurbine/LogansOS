# CLOSE REPORT — SC-4 (BUILD-3 sub-commits 5+6): Q8.a/b capability table SMP + Q8.c blocked-thread-kill walk

**Date:** 2026-05-11
**Owner:** PM
**Status:** **CLOSED.**
**Type:** Phase 2 BUILD-3 sub-commits 5+6 closure report (matches SC-3C precedent `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md`). Consumes three Jarvis sub-commit verification reports. Single same-day cadence: THINK → Cerebro D5 dispatch + return + consumption → PLAN → BUILD all closed on 2026-05-11.

---

## 1. Closure summary

SC-4 closed on origin/main with three Jarvis sub-commits landing sequentially (5a → 5b → 6). One PL operational commit (`5a88925`) for CI reliability landed between SC-3C closure and SC-4 5a. All gates passed locally + GitHub CI green on all three sub-commits.

**SC-4 closes Q8 — the final capability-table + process-exit SMP correctness work Phase 2 BUILD-3 needed.** Combined with SC-3B (IPC Channel SwitchMutex retrofit) and SC-3C (CR3 cache + IPI TLB shootdowns), the kernel is now SMP-correct for: address-space mutations, IPC rendezvous, capability operations, and process-exit teardown across natural-exit and forced-kill paths.

**The architecturally-load-bearing contribution is sub-commit 5b: Phase A retirement of `cr3_for_thread`.** Cerebro D5 reframed PM's original D4 question from "what lock type for PROCESSES?" to "should PROCESSES be on the hot path at all?" Substrate audit confirmed: no. Thread now carries `pmap: Option<Arc<Pmap>>`; scheduler context-switch path accesses it directly; PROCESSES leaves the hot path entirely. Linux `loaded_mm` parallel; SC-3C's `active_pmap_cr3` per-CPU cache applied at the Thread layer.

## 2. Commits on origin/main

| SHA | Author | Type | Description |
|---|---|---|---|
| `5a88925` | PL (SecondStageTurbine) | Operational CI fix | tools/xtask: retry transient neo-test scenario failures. Landed between SC-3C closure integration and SC-4 5a; parallel to e7782fc pattern from SC-3C. |
| `c18646e` | Jarvis | SC-4 5a | kernel/sched: retrofit process handles for SMP (D1/D2/D3/D6; SwitchMutex on handles + derivations). |
| `c02abf7` | Jarvis | SC-4 5b | kernel/sched: retire cr3_for_thread for Thread pmap (D4 Phase A finalized + D7 exiting flag + D8 exit_state CAS). |
| `57f3cb1` | Jarvis | SC-4 6 | kernel/sched + ipc: handle blocked-thread kill walk (D9 walk + D10 close_peer my-side fix + D12 zombify_thread). |

## 3. Verification

### 3.1 Per-sub-commit gates (all PASS)

Three Jarvis verification reports filed:
- `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md` (2.6 KB)
- `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md` (2.9 KB)
- `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md` (2.4 KB)

### 3.2 Local + CI gates

- `cargo k` PASS (all three sub-commits)
- `cargo k --no-default-features` PASS (all three)
- `cargo check -p xtask` PASS
- Local Docker `cargo neo-test all` PASS (all three)
- **GitHub CI green on all three pushed sub-commits.** Latest run: 25705327371, QEMU Integration Tests success.

### 3.3 D14/D17 negative-feature scenarios

Per SC-3C 4c D14 missing-shootdown-detection precedent, SC-4 added equivalent negative-feature gates:
- `q8a_concurrent_install_revoke` (D16) — verified by 5a.
- `q8d_phase_a_coverage` (D18) — verified by 5b.
- `q8c_blocked_kill_walk` + `q8c_close_peer_my_side_extract` (D17) — verified by 6.

Detailed mechanism markers documented in the three sub-commit verification reports.

### 3.4 cargo fmt --check BLOCKED (tooling gap)

Same rustfmt-not-installed-for-pinned-nightly gap from SC-3C. Forward marker continues from `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` §6.1.

## 4. Mechanism named

**None.** SC-4 is correctness-implementation work for ratified design per `Plans/SC-4-THINK-2026-05-11.md` D1-D18 + `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`. The Phase A retirement of `cr3_for_thread` is significant but it's an architectural simplification of existing infrastructure, not a new wedge mechanism requiring naming.

## 5. Course corrections during BUILD

**One operational adjustment:** PL landed `5a88925` (`tools/xtask: retry transient neo-test scenario failures`) between SC-3C closure (commit `165a2e2`) and SC-4 5a start (`c18646e`). Parallel to the e7782fc IrqSafeMutex bench CI fix from SC-3C. CI reliability discipline continues to evolve with the test surface; PM does not need to take action.

**No code course corrections in BUILD.** All three Jarvis sub-commits landed clean against the dispatch spec. The D4 Phase A architectural change in 5b is the SC-4 headline contribution; Jarvis rendered the Cerebro citations in the commit message and code comments correctly per the dispatch §2.2 four corrections.

**Pre-staging gap for closure docs:** SC-3C pre-staged closure documentation via Prometheus during BUILD (`Plans/CLOSURE-PREP-2026-05-11-sc-3c-phase2-and-index-additions.md`). SC-4 did not. Follow-up Prometheus dispatch (`Plans/DISPATCH-2026-05-11-sc-4-closure-integration-prometheus.md`) handles drafting + integration in one pass. PM operational note: future BUILD-stage dispatches should consider pre-staging in parallel — the SC-3C cadence with concurrent doc-prep was cleaner.

## 6. Forward markers

### 6.1 rustfmt tooling gap

Continues from SC-3C. No resolution this cycle.

### 6.2 SC-4-introduced forward markers (embedded inline per dispatch §8)

All embedded as code comments in `kernel/src/sched/process.rs` and `kernel/src/ipc/mod.rs` per dispatch §8:

- **FM-D5.1: spin::RwLock is documented-pathological at our scale.** 5× regression vs Mutex at 4-64 CPU range per Cerebro §5.4.
- **FM-D5.2: arc-swap Phase B fallback** deferred unless Phase A audit reveals a blocker.
- **FM-D5.3: illumos doors architectural reference** for cross-process IPC design language as Phase 3 approaches.
- **FM-D5.4: PROCESSES single-Mutex correctness boundary** validated by FreeBSD allproc + illumos pidlock at NeoPlutarch's scale.
- **FM-D13: Single-thread-per-process invariant** — Phase 2+ multi-thread requires iterating all threads of target_pid via a scheduler-side per-pid enumeration that doesn't exist yet. zombify_thread is reentrant; walk pattern generalizes.
- **FM-D14: COMPLETED_EXIT_CODES growth bound** — Phase 2+ sys_wait consumer deferred.

### 6.3 Inherited from SC-3C

Seven Phase 2+ TLB management optimizations cluster (PCID/ASID; T2 targeted shootdown with strictly-vs-loosely-accurate cpumask; lazy TLB freed_tables exception; shootdown batching; INVLPGB/RAR; Spectre-v2 BTB). All remain in PHASE-2.md "Forward reference: Phase 2+ TLB management optimizations" section per commit `165a2e2`.

### 6.4 Incidental observation surfaced

Jarvis closure report notes `tools/wedge-investigation/` untracked workspace content remains. PM note: not blocking; appears to be a local investigation directory parallel to Plans/ that didn't get formalized into a tracked artifact. PL operational call on whether to formalize, archive, or leave as-is.

## 7. Calibration

### 7.1 Cerebro D5: highest-value Cerebro contribution to date

Cerebro D5 (`Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md`, 52.1 KB) identified that PM was solving the wrong question. PM's THINK §5.1 D4 was framed as "what lock type for PROCESSES?"; Cerebro reframed to "does PROCESSES need to be on the hot path at all?" with substrate evidence (Linux `loaded_mm`, seL4 `current_thread()`, NeoPlutarch's own `active_pmap_cr3`) supporting Phase A retirement.

Four PM THINK corrections folded; D1 (SwitchMutex) and D7 (exiting flag) cross-validated by Cerebro citations (seL4 Peters/Heiser BKL evaluation; illumos doors `DOOR_INVALID` parallel).

**PM operational discipline updated:** future PM cadence should incorporate a "have I framed the right question?" gate before dispatching Cerebro. Cerebro is the safety net when PM misses it; the dispatch question-framing itself is now a PM-level checkpoint.

### 7.2 Jarvis post-calibration discipline

Three SC-4 sub-commits landed clean using the post-calibration dispatch shape per `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` §5.1-5.3. Dispatch omitted explicit calibration framing, branching strategy, verification report format, hybrid-git posture; emphasized D-decision references, Cerebro-corrected citations, sub-commit boundaries, Phase 2+ forward markers. Jarvis rendered all correctly without surfacing scope or design ambiguities mid-flight.

Particularly strong: the D4 Phase A retirement in 5b required embedding substantial Cerebro citations in the commit message and as inline code comments; Jarvis applied the four PM corrections per dispatch §2.2 without dropping or paraphrasing.

### 7.3 PM operational performance

One process improvement noted: SC-4 closure doc pre-staging gap per §5 above. Future Phase 2 BUILD-3 sub-commits should consider parallel Prometheus pre-staging during BUILD wall-clock, matching the SC-3C cadence pattern.

No agent-roster amendments triggered.

## 8. What Q9+ looks like

Phase 2 BUILD-3 has now closed Q4 (IPC retrofit via SC-3B), Q6 (IPC retrofit completion via SC-3B), Q7.b/c (CR3 cache + IPI TLB shootdowns via SC-3C), Q8.a/b (capability table SMP composition via SC-4 5a/5b), and Q8.c (blocked-thread-kill walk via SC-4 6).

**Remaining Phase 2 BUILD-3 critical-path candidates:**
- Q9+ work has not been pre-scoped in PM cadence. PM THINK-stage exploration needed to identify the next sub-commit cluster.
- Likely candidates per ANSPEC-001 § Phase 5 foundation deliverables: AI subsystem foundation work begins after Phase 2 BUILD-3 closes. The remaining Phase 2 work may be smaller cleanup items or PM may declare Phase 2 BUILD-3 substantially complete and pivot to Phase 2 BUILD-4 / Phase 3 NeoFS preparation.

PM authors Q9+ THINK-stage exploration on next session bandwidth.

## 9. Cross-references

**Verification reports consumed:**
- `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-2026-05-11.md`
- `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-2026-05-11.md`
- `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-2026-05-11.md`

**Design + research artifacts (canonical):**
- `Plans/SC-4-THINK-2026-05-11.md` — 18 D-decisions (D1-D18).
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` — Cerebro D5 prior-art research (52.1 KB).
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` — D4 finalized; sub-commit 5b reframed; four PM THINK corrections folded.

**Dispatches:**
- `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md`
- `Plans/DISPATCH-2026-05-11-sc-4-plan-stage-jarvis.md`

**Precedent:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` — SC-3C closure (the precedent this report follows).

**Pending follow-up:**
- `Plans/DISPATCH-2026-05-11-sc-4-closure-integration-prometheus.md` — SC-4 closure integration into PHASE-2.md + INDEX.md (Prometheus, ready to ferry).

---

## 10. Closure statement

SC-4 closed. Phase 2 BUILD-3 sub-commits 5+6 complete. Capability table SMP composition + blocked-thread-kill walk + process-exit sequencing all SMP-correct. cr3_for_thread retired entirely; Thread carries Arc<Pmap>; PROCESSES off the hot path. Cerebro D5 reframing was the architecturally-decisive contribution. Same-day cadence on THINK → Cerebro dispatch + return + consumption → PLAN → BUILD all on 2026-05-11.

Next critical-path: PM authors Q9+ THINK-stage exploration to identify next sub-commit cluster.

---

**End of close report.**

*SC-4 closed at commit `57f3cb1` (sub-commit 6). Q8.a/b/c complete. Phase 2 BUILD-3 SMP correctness substantially complete.*
