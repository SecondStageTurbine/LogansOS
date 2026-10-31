# PM-DISPOSITION: Phase 2 THINK v0.2 substance — Cerebro return consumed

- Author: PM
- Date: 2026-05-24
- Status: Cerebro v0.2 substance ACCEPTED. PM judgment applied on §9 items; two PL-operational items surfaced for ratification before polish ships.
- Inputs consumed:
  - `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — Cerebro's recommendations across Q1–Q7
- Outputs filed: this disposition + `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md` (Prometheus polish dispatch)

---

## 1. Acceptance-criteria verification

Per `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-cerebro.md` §4:

| Criterion | Status | Evidence |
|---|---|---|
| **C1** Each Q1–Q7 has a recommended disposition with reasoning + alternatives + (where applicable) open dependencies | ✓ | All seven sections carry choice/why/alternative-ruled-out; open dependencies routed to §9 |
| **C2** All sub-questions called out in dispatch §2 addressed within each question's section | ✓ | Q2 covers (a)–(e); Q4 covers (a)–(e); Q5 covers (a)–(c); Q6 covers (a)–(e); Q7 covers (a)–(d). Q1 and Q3 are single-thrust questions, addressed |
| **C3** §8 cross-question synthesis names interactions | ✓ | Three interactions: Q5↔Q7 (surface-matching + sequencing), Q1↔Q2↔Q3 (reserved-bit trio), Q4↔Q6 (shared hardware session) |
| **C4** §9 open-questions-for-PM catalogs items needing PM/PL resolution | ✓ | Six items; chief PL-operational are Q4(a) reference-environment and Q6(d)/(e) hardware target + executor |
| **C5** Deliverable filed at specified path + pushed | ✓ | `eac6737` on `origin/main` |
| **C6** No final ratified calls, no audit artifact amendments, no ADR drafting, no bible amendments | ✓ | All recommendations explicitly marked as recommendations; Q2(e) full FM-D14 retirement flagged as possible follow-on ADR but not drafted; no bible amendment named as warranted |

All six criteria met. Recommendation density follows dispatch direction (committed choices, alternatives ruled out, hedging minimal).

## 2. PM rulings on Cerebro's recommendations

I concur with all seven of Cerebro's recommended dispositions. Explicit on the two load-bearing calls Cerebro flagged:

### 2.1 Q5(a) test re-routing — PM concedes; Cerebro's read stands

Cerebro routes `INV-SYNC-5` (`sched_ready_queue_capacity_pressure`) to Q7/A2 rather than to ADR-019 C alongside `INV-IPC-5`. My OBSERVE-era read grouped both audit (b) tests with ADR-019 B/C because they "touch Channel.state lock surface." That was wrong: only `INV-IPC-5` touches Channel.state. `INV-SYNC-5` is on the **ready-queue** surface, which A2 replaces with a fixed-capacity ring. A2 is therefore `INV-SYNC-5`'s structural fix — the test becomes A2's acceptance gate by construction (drive threads to the per-CPU bound; assert graceful `-EAGAIN` with no realloc/alloc-under-lock/panic).

Routing `INV-SYNC-5` to A2 keeps each (b) test with the BUILD that actually touches its lock surface and avoids the awkwardness of testing a structure (`VecDeque`) that A2 is replacing.

**Ruling: confirmed. INV-IPC-5 pairs with ADR-019 C. INV-SYNC-5 pairs with A2.**

### 2.2 Q7 — concur with fixed-capacity per-CPU ring of `Box<Thread>`

Cerebro recommends a ring (circular array deque of `Box<Thread>`) over the canonical intrusive linked list. The decisive argument is Bible Article 4.2 (`unsafe` confined to hardware boundaries — asm, MMIO, page tables, MSRs, reviewed line-by-line). An intrusive list with embedded `next`/`prev` raw-pointer plumbing forces `unsafe` into scheduler logic that is *not* a hardware boundary. The ring achieves every ADR-016 constraint (bounded by construction, pre-allocated, zero-alloc-under-lock, fallible spawn, Box-pointer-stability) entirely in safe Rust for ~1 KB/CPU.

This is project-specific constraint correctly overriding the canonical kernel-implementation choice. ADR-016's own thesis is "structure over discipline" — the ring delivers structural heap-independence; the intrusive list would have spent the project's tightly-rationed `unsafe` budget on scheduler data-structure plumbing. The ring's trade-offs (fixed capacity → `-EAGAIN` at limit; O(N≤64) arbitrary removal under brief lock) are acceptable and align with ADR-016's existing "spawn fails gracefully" mandate.

**Ruling: confirmed. A2 implementation shape = fixed-capacity per-CPU ring of `Box<Thread>`, default 64/CPU, configurable at init, `-EAGAIN` at limit.**

### 2.3 §9 items — PM judgment calls

| §9 item | Cerebro rec | PM ruling |
|---|---|---|
| **§9.3** Q5(a) re-routing | Re-route `INV-SYNC-5` to A2 | Confirmed per §2.1 above |
| **§9.4** Q2 blocking-vs-poll scope-density | Blocking + WNOHANG (default blocking) | Concur. ADR-007 names the supervisor case explicitly; poll-only contradicts design intent and weakens FM-D14 bounding. The "one new mechanism" cost (per-Process parking + wake from `on_thread_reap`) is small, well-defined, and rides infrastructure already present (parked-Thread mechanism is the rendezvous primitive) |
| **§9.5** Q3 ship-vs-defer `sys_inspect` | Ship thin | Concur. Cost is near-zero (read-only syscall, no new mechanism). Supervisor non-consuming peek is real even pre-Phase-4 via spawn-minted INSPECT. Completes the reserved-bit trio cleanly |
| **§9.6** Q2(e) full FM-D14 retirement via handle-presence-gated slot lifetime | Flagged as possible follow-on ADR; not Phase-2-close blocker | Concur. Phase 2 close ships the practical bound (first-consumer-wins `sys_wait`); full structural retirement is a future ADR triggered by `COMPLETED_EXIT_CODES` residual-growth measurement if it surfaces |

## 3. Items for PL operational ratification

Two §9 items are PL-operational per the dispatch's explicit directive and Cerebro's framing. Cerebro recommends; PL ratifies the operational specifics.

### 3.1 Q4(a) — performance harness reference environment

Cerebro recommends a **tiered split**: QEMU-TCG deterministic structural proxies gate per-commit (counters are deterministic under TCG even though absolute timing isn't); bare-metal OptiPlex 7080 characterizes absolute latency periodically during PL hardware sessions; VMware-VM is the reproducible middle tier when bare-metal sessions aren't available.

PL operational confirmation needed:
- Is the tiered split acceptable, or should one environment be designated "the" reference?
- Is per-commit gating on TCG deterministic proxies (rather than on absolute numbers) acceptable as Bible 16.4 compliance?

PM recommendation: ratify the tiered split as Cerebro proposes. It's the only design that simultaneously honors Bible 16.4's "from Phase 2 onward" mandate AND the operational reality that bare-metal can't run per-commit and TCG absolute numbers are fiction.

### 3.2 Q6(d)/(e) — bare-metal multi-CPU hardware target + executor

Cerebro recommends: **OptiPlex 7080 required** (established Intel bare-metal target, ≥4 cores, full logical-CPU count); **AMD cross-vendor a non-blocking stretch** (would additionally validate vendor-specific APIC/INVLPG behavior relevant to Q7.c shootdown, but not mandated); **PL executes a written procedure document** modeled on `PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` (procedure-doc-then-execute split — PM authors the procedure pre-BUILD, PL executes in-BUILD, PM consumes evidence).

PL operational confirmation needed:
- OptiPlex 7080 as the bare-metal target (confirming what Cerebro recommends).
- AMD cross-vendor — in scope as stretch goal, deferred, or excluded?
- PL hardware session availability for executing the procedure.

PM recommendation: ratify Cerebro's recommendation as proposed. OptiPlex required, AMD as future-stretch (no Phase-2-close blocking), PL session scheduled when convenient post-PLAN.

## 4. Net state and next steps

**Phase 2 PLAN cadence after this turn:**
- OBSERVE closed (`255372a`)
- Scope-class items ratified (`ee5055e` + `7040a49`)
- THINK v0.1 (question batch) authored + polished + ratified (`ee5055e` / `46426d9` / `c5017bf`)
- **THINK v0.2 substance returned by Cerebro and accepted by PM (this disposition)**
- Doc-reconciliation closed (`a796303` + `dbef8d9`)

**Next steps:**
1. **PL ratifies the two operational items in §3** above (Q4(a) tiered split, Q6(d)/(e) hardware target + executor). PM's recommendation defaults are noted; "yes to both" closes this gate.
2. **Prometheus polishes Cerebro v0.2 substance** into PL-ratification-ready form (dispatch filed alongside this disposition). The polish includes the PM rulings from §2 and §9 dispositions — i.e., the polished doc presents committed Phase-2-close answers, not Cerebro's "recommended" framing. Q4(a) and Q6(d)/(e) remain marked as PL-operational-pending in the polished output unless PL ratifies §3 before polish lands.
3. **PL ratifies the polished THINK v0.2.** Same shape as Phase 4 v0.2 Q1-Q12 batch ratification.
4. **PM enters PLAN sub-step** — translate the answered questions into BUILD subphase enumeration. Cerebro's §8 synthesis gives the shape (two subphases: cap reserved-bit activation; formal-close + defense-in-depth with A2 → ADR-019 B → C sequencing and Q4+Q6 sharing a hardware session). PLAN adds goal milestones, dependencies, anti-scope, /simplify gates.

**No active workstreams beyond these.** Adapter for serial-console diagnosis still slipped to 2026-05-25; that resumes BUILD-5b context when arrives.

## 5. References

- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — the substance accepted in this disposition
- `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-cerebro.md` (`c5017bf`) — the dispatch
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (`c5017bf`) — question-batch ratification
- `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — ratified question text
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (`ee5055e`+`7040a49`) — the four scope ratifications constraining answer space
- `Plans/DISPATCH-2026-05-24-phase-2-think-v0.2-polish-prometheus.md` (this commit) — Prometheus polish dispatch
- ADR-007, ADR-010, ADR-013 (FM-D5/D8/D13/D14), ADR-016 (Q7 substrate), ADR-019 (Q5)
- Bible Articles 4.2 (Q7 decisive), 8.3/8.5.3 (Q7), 12.2 (Q3), 13 (Q6), 16.4 (Q4)
- Audit chain: `199fb51` audit close, `5884ffd` F-005 close; (b) tests F-004 / INV-IPC-5 / INV-CAP-11 / INV-SYNC-5
