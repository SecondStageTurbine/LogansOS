# Q9+ EXPLORATION — Phase 2 BUILD-3 remaining work synthesis

**Date:** 2026-05-11
**Author:** PM
**Cadence:** OBSERVE/THINK exploration per Bible Article 14. Smaller than SC-3C/SC-4 THINK because the design space is narrower (one canonical Q-item plus closure checklist remainder).
**Type:** Exploration artifact. Output is a PM-tier decision: does Q9 become SC-5 BUILD work, stay deferred indefinitely with explicit forward-marker discipline, or get partially retired as obsolete under SC-4's D8 CAS discipline?
**Authority:** PM operational call per PL clarification 2026-05-10. PL sign-off needed only on (a) any Bible amendment that flows from this exploration and (b) decision to dispatch a Cerebro deep-research follow-up.

---

## 1. Mandate

Q9+ identifies and scopes the remaining Phase 2 BUILD-3 work after SC-4 closure. The "9+" framing acknowledges: (a) ADR-013's Q9 (reaper distribution) is the canonical deferred item, and (b) the migration checklist has a final cross-cutting line item ("bare-metal multi-CPU verification at BUILD-3 close") that has not yet been formally exercised.

Output: PM decision on whether Q9 enters SC-5 BUILD scope, stays deferred indefinitely, or gets partially retired. Plus a brief scope statement for the bare-metal verification mandate.

## 2. Status snapshot — where Phase 2 BUILD-3 stands

### 2.1 What's closed

Per `docs/phases/PHASE-2.md` post-SC-4-closure-integration (commit `3367a4f`):

| Sub-commit | ADR-013 Q-items addressed | Status | Closure SHA |
|---|---|---|---|
| 0 | GLOBAL bit fixup (Q7.a partial) | Closed | (early BUILD-3) |
| 1a | Q1 (per-CPU state shape) | Closed | (per-CPU infra refactor) |
| 1b | Q2 (AP bring-up) | Closed | (Limine MP request) |
| 2 | Q3 (per-CPU schedulers), Q5 (deferred-quantum restored), Q9 **DEFERRED** | Closed with amendment | (per-CPU schedulers; reaper distribution deferred) |
| 3 (SC-3B) | Q4 (wake-on-completer-CPU), Q6 (block_current_into lock-order) | Closed | `e2b3f4f` |
| 4 (SC-3C) | Q7.b (per-CPU CR3 cache), Q7.c (IPI TLB shootdowns) | Closed | `ead8b19` (4c) |
| 5+6 (SC-4) | Q8.a/b/c (capability table + blocked-thread-kill) | Closed | `57f3cb1` (6) |

### 2.2 What's deferred

**Q9 — Reaper distribution.** Original design: per-CPU reapers; `Process.live_thread_count: AtomicU32` gate on `on_thread_reap` cross-CPU correctness. Sub-commit 2 attempted implementation; 100x `ipc_heap_kill_smoke` failed at run 81/100 with kernel hang in `reap()` after `reaped 'pong'`. Eight-experiment bisection (A-H per `docs/ADR-013-phase-2-smp-retrofit.md` BUILD-3 amendment "sub-commit 2 Q9 deferred with bisection evidence") localized to Q9-specific machinery.

Sub-commit 2 closed with the interim shape:
- Per-CPU scheduler containers retained (Q3 closed).
- AP-side reaper participation removed; APs do not reap.
- BSP `reap()` scans all initialized per-CPU schedulers' zombie queues under each scheduler's lock.
- `Process.live_thread_count` field removed.

Acceptability rationale at sub-commit 2 close: "no production thread is routed to APs yet; AP zombie queues are empty in the steady state, so the BSP cross-CPU scan is effectively a bounded no-op."

### 2.3 What's changed since sub-commit 2 close

Three substantive changes affect Q9's design space:

**1. SC-3B Q4 closed (wake-on-completer-CPU rendezvous migration).** This explicitly creates the cross-CPU thread movement the sub-commit 2 acceptability rationale said wasn't happening yet. Threads CAN now route to APs via IPC rendezvous on AP CPUs. Steady-state SMP workloads with cross-CPU IPC will accumulate threads on APs.

The current BSP-scans-all interim is still operationally OK because:
- Existing test scenarios don't drive enough cross-CPU IPC to fill AP zombie queues meaningfully.
- Phase 1.5b single-thread-per-process invariant (SC-4 D13) means each user process's zombie production is bounded by 1 thread.

But the rationale is weakening. Phase 2+ multi-threaded user processes + heavy cross-CPU IPC workloads (Phase 5+ inference scheduling per Bible Article 23) will break the "AP zombie queues are empty" assumption.

**2. SC-4 D8 added `Process.exit_state: AtomicU8` CAS-claim.** This provides natural-exit-vs-forced-kill mutual exclusion at the Process level. It is structurally similar to Q9's `live_thread_count` in one specific way: both serialize "who does the cleanup?" decisions. Under SC-4 D13's single-thread-per-process invariant, the `exit_state` CAS already gates Process deregistration — there is exactly one path to cleanup, and CAS picks the winner.

**Critical observation:** under single-thread-per-process, `live_thread_count` is redundant with `exit_state` CAS. The thread reap IS the process exit; the CAS already serializes. `live_thread_count` only earns its keep under multi-threaded user processes where multiple threads must coordinate "I'm the last one" decisions.

**3. SC-4 D13 made single-thread-per-process an explicit Phase 1.5b/Phase 2 invariant.** Phase 2+ multi-threaded user processes are formally forward-marker. The design that would need `live_thread_count` is not Phase 2 scope.

### 2.4 What's not done in the migration checklist

ADR-013 cross-cutting verification:
> - [ ] **Bare-metal multi-CPU verification at BUILD-3 close** (Bible Article 13 mandate).

This is a verification deliverable, not a code deliverable. Sub-commits 1b through SC-4 6 have all verified on QEMU with `-smp 4`; bare-metal multi-CPU has not been formally exercised since BUILD-1 (which was single-CPU on bare metal per the standard verification protocol).

Bible Article 13 mandates a bare-metal multi-CPU verification at BUILD-3 close. The work itself is straightforward (boot the existing kernel on a 4+ core bare-metal target and observe SMP correctness), but it requires hardware + a formal verification protocol.

## 3. Q9 design alternatives

Four shapes for Q9's resolution, given the changes in §2.3.

### Alternative (a) — Re-attempt original Q9 with hypothesis-derived precautions

Implement per-CPU reapers + reintroduce `Process.live_thread_count` with the precautions named in the sub-commit 2 amendment's "future work" guidance:
- Initialize the counter before any thread becomes runnable.
- Verify exactly-once decrement semantics under SMP.
- Avoid AP-side reaper participation until threads can actually route to APs.
- Use `SeqCst` until weaker ordering is proven.
- Add a regression test that exercises the counter under real multi-thread-per-process load.

The third point is the awkward one: AP-side reaper participation was specifically what the sub-commit 2 bisection deferred. If "until threads can actually route to APs" is the gate, that gate already opened with SC-3B Q4 rendezvous migration. Re-attempting needs an actual exit-path test that drives cross-CPU thread routing.

**Cost:** Re-investigates a known-flaky surface; requires Cerebro-tier prior-art research on Linux task lifecycle / kref discipline / RCU-protected task tear-down to choose memory ordering correctly.

**Benefit:** Phase 2 BUILD-3 closes with all Q-items addressed. Phase 2+ multi-threaded user processes have the infrastructure they need.

### Alternative (b) — Defer Q9 permanently; declare BUILD-3 substantially complete under D13

Accept the interim shape (BSP scans all per-CPU zombie queues) as the Phase 2 BUILD-3 final answer. Q9 becomes a Phase 2+ forward marker that lifts only when multi-threaded user processes ship. The acceptability rationale: under D13 single-thread-per-process, the BSP scan is correctness-equivalent to per-CPU reapers (production thread routing to APs produces zombies on APs, but BSP drains them; the only "miss" is throughput, and at our scale BSP throughput is sufficient).

**Cost:** Phase 2 BUILD-3 doesn't formally close all ADR-013 Q-items. Need to write the deferral into PHASE-2.md as an explicit "Q9 stays deferred under single-thread-per-process invariant" forward marker.

**Benefit:** Zero risk; SC-4's exhaustive SMP-correctness story stays unblemished by the known-flaky Q9 surface.

### Alternative (c) — Different reaper distribution shape (no live_thread_count gate)

Per-CPU reapers, but the deregistration logic stays single-path: each per-CPU reaper drains its own zombie queue and calls a shared `on_thread_reap(pid)` function. That function uses SC-4's `exit_state` CAS to claim the cleanup; the loser bails. No `live_thread_count` field.

Under single-thread-per-process (D13), this is correct: there's exactly one thread per process, so the thread's reap IS the process's exit, and the CAS handles serialization. Multi-threaded user processes (D13 forward) would need additional discipline, but the per-CPU reaper containers and per-CPU draining are in place.

**Cost:** Different from original Q9; needs a new design walk-through. Single-thread-per-process invariant becomes load-bearing for correctness — if D13 is ever relaxed without addressing this, silent bugs.

**Benefit:** Per-CPU reapers ship now without the `live_thread_count` machinery that broke in sub-commit 2. SC-4's CAS becomes the unified Process-exit serialization primitive across natural-exit, forced-kill, and reap paths.

### Alternative (d) — Hybrid: per-CPU reapers drain own queue; BSP-only deregistration

Per-CPU reapers run on each CPU's bootstrap thread, drain that CPU's local zombie queue, free thread resources. But Process deregistration (removing from PROCESSES) stays BSP-only: when a per-CPU reaper would deregister, it instead queues a "deregister this pid" message to a BSP-handled queue; BSP's reaper does the actual deregistration under PROCESSES write lock.

**Cost:** Cross-CPU queue + handoff complexity. Two reaper types (per-CPU drain vs BSP deregistration).

**Benefit:** Per-CPU drain throughput; BSP-serialized deregistration sidesteps the `live_thread_count` race.

### Alternative comparison

| Alt | Per-CPU reapers? | live_thread_count? | exit_state CAS as primary serialization? | Multi-thread Phase 2+ ready? | Cost |
|---|---|---|---|---|---|
| (a) | Yes | Yes (re-attempted) | No (live_thread_count is primary) | Yes | High — re-investigates known-flaky |
| (b) | No (BSP scans all) | No | Yes (via D8 already) | No (needs future work) | Low — defer + document |
| (c) | Yes | No | Yes (D8 is unified) | Conditional (D13 invariant becomes load-bearing) | Medium — new design, single dispatch |
| (d) | Yes (drain only) | No | Mixed (BSP for deregister) | Conditional | High — cross-CPU queue infrastructure |

**PM lean:** Alternative (b). Defer Q9 permanently in current shape; declare BUILD-3 substantially complete under D13's single-thread-per-process invariant. Document the deferral as an explicit Phase 2+ forward marker. Reconsider when (and only when) Phase 2+ multi-threaded user processes enter scope.

Reasoning:
- The interim shape (BSP scans all) is provably correct under D13. The deferred work earns its keep only under multi-thread; multi-thread is explicit Phase 2+ forward marker.
- SC-4's `exit_state` CAS already provides the unified Process-exit serialization primitive that alternatives (a) and (c) reinvent.
- Sub-commit 2's bisection evidence is a clear signal that the `live_thread_count` design has a real race the project couldn't characterize. Re-attempting without that characterization is high-risk; commissioning a Cerebro deep-research pass on it for a deferred item is over-engineering.
- ALTIMIT-faithful posture per close-report §7.3: when a mechanism's original justification is removed (here, by D13), delete or defer the mechanism rather than carrying it forward as latent complexity.

**Strong alternative consideration:** (c). If PL prefers Phase 2 BUILD-3 to close all ADR-013 Q-items rather than leave Q9 explicitly deferred, alternative (c) is the cleanest path. The CAS-as-unified-serialization framing is architecturally appealing; it would land as a SC-5 sub-commit on the scale of SC-4 6 (~200-300 lines).

## 4. Q9 decision points

PM operational call per agent-roster lane discipline (PM owns operational + technical decisions; PL sign-off on legitimate PL-tier items). The PL-tier elements here:

**4.1 Does Phase 2 BUILD-3 close with all Q-items addressed, or with Q9 explicitly deferred?**

This is a Bible-Article-13-adjacent question because the phase ledger semantics matter. PM lean: deferral is acceptable; Bible Article 13's phase completion criteria don't require every ADR-013 sub-question to land — the phase milestone is "Phase 2 SMP works correctly," not "ADR-013 fully implemented."

**4.2 Should PM dispatch Cerebro for prior-art research on production-kernel reaper distribution?**

PM lean: no, not for alternative (b). If PL prefers alternative (c) or (a), a Cerebro dispatch would resolve the AcqRel-vs-SeqCst memory-ordering question and survey Linux kref / RCU task tear-down for prior art. SC-4 D5 pattern would apply.

**4.3 Forward marker discipline for the deferral.**

PM operational call (no PL signoff): if alternative (b), write the deferral into PHASE-2.md "Forward reference: Phase 2+ capability + process-exit optimizations" subsection (where SC-4's similar markers live). Format parallel to FM-D14 (COMPLETED_EXIT_CODES growth bound — also single-thread-per-process gated).

## 5. Other remaining Phase 2 BUILD-3 work

Beyond Q9:

### 5.1 Bare-metal multi-CPU verification

ADR-013 cross-cutting checklist requires bare-metal multi-CPU verification at BUILD-3 close. This is the only un-checked item in the verification checklist post-SC-4.

PM lean: schedule as a non-blocking deliverable. Required for BUILD-3 close per Bible Article 13, but not blocking on Q9 disposition or any other work. The verification protocol is:
1. Boot current `origin/main` (HEAD `3367a4f`) on a bare-metal 4+ core x86_64 system via Limine.
2. Observe BSP + AP startup logs, GS-base sentinels, scheduler bootstrap.
3. Run a sampling of the test-harness scenarios that exercise SMP code paths (`smp_cross_cpu_rendezvous_stress`, `q7c_*`, `q8a/b/c/d_*`).
4. Verify frame audit clean, no `KERNEL PANIC`, no `BUG:`.
5. File verification artifact `Plans/PHASE-2-BUILD-3-BARE-METAL-VERIFICATION-2026-05-MM.md`.

Hardware dependency: PL needs to identify a target system. PM does not have visibility into PL's hardware availability.

### 5.2 Other Phase 2+ forward markers (no SC-4 BUILD action)

All of these are explicit Phase 2+ markers landed in PHASE-2.md per SC-4 closure integration (commit `165a2e2` + `3367a4f`):

- PCID/ASID enablement
- T2 targeted shootdown with strictly-vs-loosely-accurate cpumask
- Lazy TLB freed_tables exception
- Shootdown batching / mmu_gather analogue
- INVLPGB/RAR hardware-broadcast
- Spectre-v2 BTB flush on context switch
- `spin::RwLock` pathological warning
- `arc-swap` Phase B fallback
- illumos doors architectural reference
- PROCESSES single-Mutex correctness boundary
- COMPLETED_EXIT_CODES growth bound

None require BUILD-3 action; all are explicit Phase 2+ or Phase 3+ work.

### 5.3 ADR-019 bench bare-metal characterization

Per `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`, the IrqSafeMutex bench threshold (6000 ns under local Docker QEMU TCG) is environment-anchored and the 50 ns native-hardware target is deferred to Phase 2+ bare-metal CI. The bare-metal verification in §5.1 could opportunistically capture bare-metal IrqSafeMutex bench numbers for the ADR-019 record; PM operational call to fold that into the verification protocol.

### 5.4 rustfmt tooling gap

The pinned nightly `nightly-2026-04-16-x86_64-pc-windows-msvc` doesn't have rustfmt installed. `cargo fmt --check` BLOCKED across SC-3C, SC-4. Forward marker from `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` §6.1.

PL operational decision when convenient. Three options previously enumerated (install rustfmt for pinned nightly, pin different nightly that ships rustfmt, accept gap with alternative linting).

## 6. PM operational decision

Recording PM dispositions per PL clarification 2026-05-10 ("make the call, document reasoning in the artifact, ferry on"):

### 6.1 Q9 resolution: Alternative (b) — defer permanently in current shape

Q9 stays deferred. The current interim shape (BSP scans all per-CPU schedulers' zombie queues; APs do not reap) is the Phase 2 BUILD-3 final answer under D13's single-thread-per-process invariant. SC-4's `exit_state` AtomicU8 CAS already provides Process-exit serialization; `Process.live_thread_count` is not needed.

Implementation: zero code changes. PM authors a follow-up dispatch (PM cadence work, likely Prometheus for doc-only execution) to:
1. Add a new Phase 2+ forward-marker entry to PHASE-2.md "Forward reference: Phase 2+ capability + process-exit optimizations" subsection: **FM-Q9: Per-CPU reaper distribution with multi-thread support deferred to Phase 2+ multi-threaded user processes per ADR-013 §Q9 sub-commit-2 amendment.**
2. Amend ADR-013's Q9 section to note the deferral is the final answer for Phase 2 BUILD-3 (not a temporary state pending re-attempt). Reference SC-4 D8 as the unified Process-exit serialization primitive that obsoletes Q9's `live_thread_count` design under single-thread-per-process.
3. No Bible amendment needed — Phase 13 ledger does not require all sub-questions to land.

### 6.2 Bare-metal multi-CPU verification: scheduled, non-blocking

PM authors a verification dispatch when PL signals hardware availability. Protocol per §5.1. ADR-019 bare-metal bench characterization folded into the same verification run per §5.3.

### 6.3 No Cerebro dispatch needed

Q9's deferral does not require deep-research. The hypothesis recorded in the sub-commit 2 amendment is sufficient as a Phase 2+ forward marker; precise resolution waits for multi-threaded user processes to actually need it.

### 6.4 Phase 2 BUILD-3 closure framing

After §6.1's forward-marker dispatch lands, Phase 2 BUILD-3 is "substantially complete" with:
- All ADR-013 Q-items either implemented or explicitly deferred with documented rationale.
- Bare-metal multi-CPU verification pending (non-blocking; required for formal BUILD-3 close per Bible Article 13 but not in critical-path for Phase 5+ AI subsystem work).

The next major PM cadence work is Phase 5 (AI subsystem foundation) preparation OR Phase 3 (NeoFS) preparation OR Phase 4 (network stack) preparation. PM lean: Phase 5 next, matching the project's AI-native premise and ANSPEC-001's existing scope. But Phase 5 prep is its own THINK-stage exercise; out of scope for this Q9+ exploration.

## 7. Recommended next dispatches

Three follow-up dispatches queued, in priority order:

### 7.1 Prometheus: Q9 deferral documentation (PM-tier, ready now)

Single doc-only commit:
- Add FM-Q9 forward marker to PHASE-2.md per §6.1 step 1.
- Amend ADR-013 §Q9 per §6.1 step 2. Bible Article 27 applies if the amendment is substantive enough; PM lean: this is a Q9-section amendment recording the BUILD-3 final disposition, parallel to the existing sub-commit-2 amendment. No ADR version bump needed (ADRs don't have versions in this project's convention; amendments append).
- INDEX.md entry for this exploration artifact.

Estimated 50-80 line touches across two files. ~1-2 hour Prometheus session.

### 7.2 PM operational: phase-completion framing decision

Not a dispatch — a PM PM-NOTE recording the Phase 2 BUILD-3 closure framing:
- All code work complete.
- Bare-metal verification pending as scheduled non-blocking item.
- Phase 2 BUILD-3 is "substantially complete" for purposes of moving to Phase 3 / Phase 5 prep work.

PM authors at next session bandwidth. Output: `Plans/PM-NOTE-2026-05-MM-phase-2-build-3-substantially-complete.md`.

### 7.3 PM operational: Phase 5 (or Phase 3) prep THINK

When PL signals readiness, PM authors prep THINK for the next phase. ANSPEC-001 already exists as the Phase 5 design substrate (60+ page spec at `docs/specs/neoplutarch-ai-subsystem-spec.md` — formerly `altimitneo-ai-subsystem-spec.md` per the rename audit). The Phase 5 THINK would walk the existing ANSPEC-001 design and identify the first sub-commit cluster.

Alternatively: Phase 3 (NeoFS) or Phase 4 (network stack) may be PL preference. PL signals which phase to prep next.

## 8. Cross-references

**ADR-013 sections consumed:**
- §Q9 (lines 904-958) — original design.
- BUILD-3 amendment "sub-commit 2 Q9 deferred with bisection evidence" (lines 1170-1187).
- BUILD-3 amendment "sub-commit 2 Q9 hypothesis (informational, not investigated)" (lines 1188-1199).

**Phase 2 BUILD-3 closure artifacts:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` — SC-3C closure.
- `Plans/CLOSE-REPORT-2026-05-11-sc-4.md` — SC-4 closure.
- `docs/phases/PHASE-2.md` post-`3367a4f` — integrated phase log.

**SC-4 D-decisions relevant to Q9 analysis:**
- D8 (`Process.exit_state: AtomicU8` CAS) — obsoletes `live_thread_count` under D13.
- D13 (single-thread-per-process invariant) — load-bearing for alternative (b) acceptability.
- D14 (COMPLETED_EXIT_CODES growth bound) — parallel forward-marker format precedent.

**Process improvement precedent:**
- ADR-013 BUILD-3 amendment "sub-commit 2 Q5 bet failed" — discipline lesson on "is this mechanism still needed?" vs "are we changing the container shape?" separation. Q9 deferral applies the same discipline (the container shape changed correctly via Q3 per-CPU schedulers; the live_thread_count mechanism inside the container is what needs separate consideration).

---

## 9. Summary

PM operational decision: Q9 stays deferred permanently in current shape under D13's single-thread-per-process invariant. SC-4's D8 `exit_state` CAS obsoletes the `live_thread_count` design for Phase 2 BUILD-3 scope. Future Phase 2+ multi-threaded user processes will lift the deferral; the design space at that point can include Cerebro deep-research on Linux kref / RCU task tear-down memory ordering.

Phase 2 BUILD-3 is substantially complete. Remaining items: (a) Q9 deferral documentation via Prometheus dispatch, (b) bare-metal multi-CPU verification scheduled non-blocking, (c) rustfmt tooling gap pending PL operational call.

Next major PM cadence work is Phase 5 (AI subsystem foundation) preparation per ANSPEC-001, OR Phase 3 (NeoFS) preparation, per PL preference.

---

**End of Q9+ exploration.**

*PM decision: Q9 deferred permanently; FM-Q9 forward marker dispatch queued for Prometheus. Phase 2 BUILD-3 substantially complete. No Cerebro dispatch needed. PL signals next phase prep direction.*
