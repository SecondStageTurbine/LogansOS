# PM-DISPOSITION: Phase 2 OBSERVE close

- Author: PM
- Date: 2026-05-24
- Status: OBSERVE step CLOSED. Four scope-class items surfaced for PL ratification before THINK can be dispatched.
- Inputs consumed:
  - `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (Cerebro Day-1 deliverable, commit `97972e1`)
  - `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20.md`
  - `Plans/PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md`
  - `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` + git log filter
- Outputs filed: this disposition + `Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md`

---

## 1. Acceptance-criteria verification

Per `Plans/DISPATCH-2026-05-24-phase-2-observe-cerebro.md` §4:

| Criterion | Status | Evidence |
|---|---|---|
| **C1** Every §2 source scanned or marked unavailable with reason | ✓ | Cerebro §"Sources scanned" table: 12 sources COMPLETE, 4 SAMPLED, 2 DEFERRED with stated reason (§7.5 evidence docs — resolved by PM in §3 below) |
| **C2** §1–§4 inventory complete | ✓ | Landed-work inventory across BUILD-1/2/3; remaining surface decomposed per Article 13's four workstreams + cross-cutting; audit closure integrated per-finding; deferred ADRs dispositioned |
| **C3** §5 design questions enumerated, not answered, in Phase 4 THINK v0.2 shape, 8–15 count | ✓ | 14 questions, grouped by workstream, Q1+Q10 carrying the scope-class weight |
| **C4** §6 2-3 BUILD subphase candidates with scope sketch + dependencies | ✓ | Three candidates ranked by dependency-cleanness × impact; Arc-aware slab explicitly excluded as first-BUILD |
| **C5** Deliverable filed at specified path + pushed | ✓ | `97972e1` on origin/main |
| **C6** Cerebro stops at deliverable file (no audit amendments, no ADR/bible drafting, no THINK answers, no scope decisions) | ✓ | All §7 scope-class items surfaced to PM, none decided unilaterally |

OBSERVE deliverable accepted.

## 2. Acknowledgment — dispatch §1.2 factual inaccuracy

The OBSERVE dispatch's §1.2 framing — *"buddy+slab allocators are pending"* — was inaccurate. BUILD-2 landed at `f5a4984` with buddy live, slab live for THREAD_CACHE, and PROCESS_CACHE + CHANNEL_CACHE declared-but-unused pending Arc-aware integration. Cerebro caught this in the headline and §7.1.

The slip is the same pattern as Day-1 audit work: PM didn't have full Phase-2 BUILD history in session context, made an unjustified factual claim about landed status, worker caught and corrected. The dispatch's *instruction* to inventory landed work was correct in spirit; the *assertion* about specific landed status was wrong.

Going-forward correction: PM dispatches will frame "remaining work" as a question for the worker to answer against current state, not as an asserted gap list. The audit-Day-1 lesson (no PM-tier execution of worker-tier inventory) plus this (no PM-tier asserted landed-status when the project's BUILD history exceeds session context) are the same discipline expressed twice.

## 3. PM resolutions — items resolvable without PL judgment

### 3.1 §7.5 — Bare-metal multi-CPU verification status

**Definitively NOT satisfied.** Two evidence docs read:

- `PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20.md`: VMware Workstation Pro production-ISO smoke PASSes at 1, 4, and 8 vCPU. But the procedure doc explicitly self-classifies (L6): *"Explicitly labeled 'hypervisor-VM verification' not 'bare-metal verification'. The Bible Article 13 bare-metal mandate stays formally open for if/when actual bare hardware becomes available."* Project's own discipline closes the question.
- `PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md`: explicit `Status: PASS-WITH-SCOPE`. Verifies framebuffer boot reached on real OptiPlex 7080 hardware. Explicitly does NOT claim multi-CPU exercise, internal disk write, serial capture, or storage validation. Single-CPU UEFI boot smoke only.

**Implication:** A1 (bare-metal multi-CPU verification) is a genuine BUILD requirement, not a documentation-pass collapse. Phase 2's Article 13 close-gate remains open. The OBSERVE §6 candidate "Phase 2 formal-close verification BUILD" depends on a PL hardware session for real bare-metal multi-CPU exercise.

### 3.2 §7.8 — ADR-016 landing status

**A1 landed; A2 unimplemented with contested phase classification.**

- A1 (IRQ-safe heap wrapper, defense-in-depth) landed at commit `aac0c11` (2026-05-08).
- A2 (scheduler decoupling from global heap — replace `VecDeque<Box<Thread>>` ready/zombie queues with bounded/pre-allocated storage) was ratified at `8a2311d` (2026-05-08). Amendment `1dbbfa7` reframed A2 to "Phase 2 conformance work." No implementation commit visible in `git log` since amendment.
- Cerebro inferred A2 as Phase-3 substrate from HANDOFF-2026-05-11 §10.2; the ADR's own amendment text says "Phase 2 conformance." Project state contradicts itself.

**Implication:** ADR-016 A2 is a Phase-2-scope-not-yet-landed defense-in-depth item — same shape as ADR-019 B/C (cross-listed in OBSERVE §2.X2). It joins the §6 candidate-3 "defense-in-depth BUILD" or potentially the §6 candidate-2 "formal-close verification BUILD" depending on PL's §7.1 framing. **Worth surfacing as a documented-but-contested phase-classification:** PL may want to ratify which is current truth (the ADR amendment, the handoff §10.2, or revise both).

### 3.3 §7.3 — Copy/Move capability ops phase classification

**PM call: stay aligned with ADR-010 Q2 — copy/move is Phase 3+.** The dispatch §1.2's "capability system *full buildout*" framing was loose; ADR-010 Q2 explicitly defers copy/move to "Phase 3+" and that's the binding position. Stakes are low (move is a no-op at the flat-table layer) but the framing should be explicit: Phase 2 "full buildout" = reserved-bit activation (sys_close/sys_wait/sys_inspect), not copy/move.

PL may override if there's a use case for copy/move in Phase 2; absent that, no scope expansion.

### 3.4 §7.7 — Stale doc headers + ADR-013 Q8.a supersession note

**PM dispatches to Prometheus** for parallel doc-reconciliation work. Three items, none blocking:

- `docs/phases/PHASE-2.md` header reads "In progress (BUILD-3 active)" though BUILD-3 is substantially complete and Phase 3/4 came after.
- `docs/ADR-019-...md` header reads "Implementation status: PENDING" though sub-phase A landed at `27d1b99`.
- `docs/ADR-013-...md` Q8.a (RwLock) was superseded in implementation by SC-4 D5 (single Mutex per FM-D5.4). FM-D5.4 records the deviation; the Q8.a section text does not.

Dispatch filed at `Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md`. Prometheus's lane: documenting + amending + cleanup. Parallel to THINK; does not block.

## 4. PL-tier scope-class items — surfaced for ratification

THINK cannot be dispatched until these are resolved. Each carries PM's recommendation + rationale, but PL holds sign-off per the operational-posture memo.

### 4.1 §7.1 — THE framing question: close vs greenfield vs both

The single highest-leverage decision. Phase 2's core (cap foundation, allocators, SMP retrofit) is substantially landed since 2026-05-11. Options:

- **(a) Formal-completion BUILD set.** Bare-metal multi-CPU verification + cap reserved-bit activation (sys_close / sys_wait / sys_inspect) + audit (b) absorption + Bible 16.4 perf-regression harness. Closes Article 13 mandates cleanly. Roughly the scope of OBSERVE §6 candidates 1+2.
- **(b) Deferred-optimization BUILD set.** Pull Phase-2+ forward markers into Phase 2: Arc-aware slab, PCID/lazy-TLB, perhaps more. Larger scope, higher architectural risk (BUILD-3 already explicitly deferred Arc-aware slab as "BUILD-3 THINK question").
- **(c) Both, as ordered sub-phases.** Close-set (a) first, then optimization-set (b) as a follow-on or fold into Phase 3 substrate.

**PM recommendation: (c) with (a) first.** Rationale:
- Article 13 has an unsatisfied close-gate (bare-metal multi-CPU verification — §3.1). Shipping (a) discharges the gate and produces a clean Phase 2 close artifact.
- Cap reserved-bit syscalls are zero-ABI-break additive work (Principle 2) — high impact (retires FM-D14, completes ADR-010/ADR-007 "Future Phase 2 hardening") at low mechanism risk.
- Bible 16.4 perf-regression harness is mandated "from Phase 2 onward" and missing — landing it with the close anchors all forward measurement.
- Arc-aware slab is the BUILD-3 deferred architectural item that wants its own THINK pass, not a first subphase. Deferring it to (b) — or folding into Phase 3 substrate where the workload may justify it — preserves the close-cleanness of (a).

**(b) without (a) is incoherent** — would leave the Article 13 close-gate open while adding more in-flight Phase-2 work. (a) only is also viable if PL wants the smallest-possible Phase-2 close.

### 4.2 §7.2 — Phase 2 ledger semantics: close first or close-with-rest?

Follows from §7.1. If (a) or (c-a-first), Phase 2 closes formally on existing landed work + cap reserved bits + verification + perf harness. The deferred work (Arc-aware slab, ADR-019 B/C, ADR-016 A2 if Phase-2-classified) becomes either Phase-2.5 / a discrete defense-in-depth BUILD set / or folds into Phase 3 substrate. If (b), Phase 2 stays open until everything lands.

The queued `PM-NOTE-…-phase-2-build-3-substantially-complete.md` (`Q9-EXPLORATION` §7.2) was never filed. Whether to file it as part of (a)'s close, or skip it as superseded by this audit + OBSERVE chain, is a PL preference.

**PM recommendation: close first per (c-a-first). File a Phase 2 close artifact at the end of (a)'s last BUILD; supersede the unfiled PM-NOTE.**

### 4.3 §7.4 — "Advanced TLB management" interpretation

Bible Art 13 names "advanced TLB management" as Phase 2. The correctness baseline (GLOBAL bit, CR3 cache, IPI shootdown with INVLPG/full-flush hybrid) shipped. PCID, T2 targeted shootdown, lazy-TLB, batching, INVLPGB/RAR are project-self-classified as "Phase 2+ / Phase 3+" in `PHASE-2.md` §"Forward reference: Phase 2+ TLB management optimizations."

**PM recommendation: baseline satisfies Article 13's "advanced TLB management."** Rationale:
- The forward markers are explicitly *project-self-classified* as out-of-Phase-2. Pulling them forward overrides the project's own scoping discipline without a strong justification.
- "Advanced" is relative to Phase 1's "no SMP, no shootdown" state. The current baseline is dramatically more advanced than Phase 1 had.
- Optimization items (PCID etc.) are measurement-gated per ADR-013 anti-scope — pulling them forward without measurement evidence is performance-first, which violates Bible 1.2's priority order (correctness > coherence > performance).
- If PL disagrees and wants specific optimizations pulled forward, the right shape is a new ADR per item with measurement-trigger criteria — not a Phase 2 PLAN expansion.

### 4.4 §7.6 — PID recycling in Phase 2 scope?

ADR-007 Impl note 6 flags stale-handle-revocation as conditional on Phase 2 adding PID recycling. If Phase 2 does *not* add recycling, B5 / Q5 drop out of scope.

**PM recommendation: NOT in Phase 2 scope.** Rationale:
- `NEXT_PID: AtomicU64` is monotonic and at 64-bit width is functionally non-exhaustible for the project's foreseeable lifetime (even at a million spawns/sec it lasts ~292,000 years).
- Adding recycling pulls in cap-id-based handle identity + revoke-on-death machinery that complicates Phase 2 close.
- If PID exhaustion ever becomes a real concern, address it in the phase where the relevant workload (e.g., persistent-server workloads, container-host workloads) actually surfaces. Not Phase 2.

PL may override if there's a specific workload requirement; absent that, defer.

## 5. Open question 7.2 micro-decision (PM-tier, recorded for transparency)

Cerebro's §7.2 framed the phase-ledger question as PL-tier. On reflection it's a derived question — fully determined by §7.1's answer (per §4.2 above). Not a separate decision PL needs to make; PM-DISPOSITION captures the derivation. Recording here so the audit trail shows the four PL-tier items (§7.1, §7.4, §7.6, plus optional ADR-016-A2-phase-classification per §3.2) and the three PM-resolved items (§7.3, §7.5, §7.7) sum to seven of Cerebro's eight surfaced open questions. §7.8 was factual (PM resolved §3.2).

## 6. Net OBSERVE outcome

| Workstream | Landed | Remaining identified | Phase-2 disposition (pending PL §4.1) |
|---|---|---|---|
| SMP | Q1–Q8 (BUILD-3); Q9 deferred-permanent | Bare-metal multi-CPU verification (open close-gate); audit (b) `smp_concurrent_reap_after_terminate`; audit (b) `sched_ready_queue_capacity_pressure` | Phase 2 close (a) scope per §4.1 PM rec |
| Capability | Foundation (BUILD-1); rights vocabulary; survive-exit | sys_close / sys_wait / sys_inspect (reserved-bit activation); audit (b) `cap_revoke_spawn_inheritance_descendants`; CapId-SMP-banks (measurement-gated) | Phase 2 close (a) scope per §4.1 PM rec |
| Allocators | Buddy live; slab live for THREAD_CACHE (BUILD-2) | Arc-aware slab integration (the big deferred architectural Q); buddy SMP contention; DerivationEntry slab; config tuning | **§4.1 PM rec defers to (b) or Phase 3 substrate** |
| Advanced TLB | Correctness baseline shipped (Q7) | PCID, T2 targeted, lazy, batching, INVLPGB/RAR, Spectre-v2 IBPB | **§4.3 PM rec: baseline satisfies Art 13** |
| Cross-cutting | F-005 fix substrate (this session) | audit (b) `ipc_rendezvous_translate_fault`; ADR-019 B/C; ADR-016 A2 (contested); Bible 16.4 perf harness; rustfmt gap | Mix: B/C + A2 + perf-harness in (a) close per PM rec; INV-IPC-5 with cap-buildout; rustfmt is PL operational |

Pending PL resolution of §4.1–§4.4, the Phase 2 PLAN cadence enters THINK with a question batch derived from Cerebro's §5 (14 seeds) refined against the scope answers.

## 7. Next steps (gated)

1. **PL responds to §4** (§4.1–§4.4 + optional ADR-016 A2 classification ratification).
2. **PM drafts THINK dispatch** with the question batch refined against PL's scope answers. Questions that drop out of scope under PL's framing are explicitly removed; questions added by the framing answer are added. PL-rec-aligned: ~8 questions in the THINK batch if (c-a-first) is ratified.
3. **PM routes the THINK batch to PL for ratification** (Phase 4 THINK v0.2 Q1-Q12 pattern).
4. **Optionally — Prometheus reformats the THINK batch** into a clean PL-ratification-ready doc per PL's earlier preference for Prometheus polish after Cerebro synthesis. This step folds in if PL wants the polish; otherwise PM files the THINK dispatch directly.
5. **Prometheus doc-reconciliation runs in parallel** (filed alongside this disposition) — does not gate THINK.

## 8. References

- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` (`97972e1`) — OBSERVE deliverable
- `Plans/DISPATCH-2026-05-24-phase-2-observe-cerebro.md` (`df5bfb5`) — the OBSERVE dispatch
- `Plans/DISPATCH-2026-05-24-phase-2-doc-reconciliation-prometheus.md` — parallel doc-reconciliation dispatch (this commit)
- `Plans/PHASE-2-AND-3-VMWARE-HYPERVISOR-VM-VERIFICATION-2026-05-20.md` — §3.1 evidence
- `Plans/PHASE-3-BARE-METAL-OPTIPLEX-7080-BOOT-SMOKE-EVIDENCE-2026-05-22.md` — §3.1 evidence
- `docs/ADR-016-irq-safe-heap-and-scheduler-decoupling.md` — §3.2 evidence; A1 landed `aac0c11`, A2 amendment `1dbbfa7`
- `Plans/PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` (`199fb51`) + `Plans/PM-DISPOSITION-2026-05-24-f005-close.md` (`5884ffd`) — audit chain
- Bible Articles 13, 14, 16.4
- ADRs 007, 010, 012, 013, 016, 019
