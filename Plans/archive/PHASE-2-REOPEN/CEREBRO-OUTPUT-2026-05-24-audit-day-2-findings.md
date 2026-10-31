# CEREBRO-OUTPUT: pre-Phase-2 audit Day 2 findings

- Author: Cerebro (Claude — deep research + synthesis lane)
- Date: 2026-05-24
- Scope binding: `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md` (PL-ratified). 5-step method §4, 4-way disposition split §6.
- Dispatch: `Plans/DISPATCH-2026-05-24-audit-day-2-cerebro.md`.
- Input: `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` (Day 1 partial, commit `c2a4411`). This deliverable resolves the 10 PENDING invariants from Day 1 §6 and completes the §3 DEFERRED source scans. It does NOT amend the Day 1 artifact (per dispatch C6); PM consumes this.
- Headline: **one new DEFECT found (F-005), of the same shape as the close_peer seed bug, in the forced-kill path PR #6 did not touch.** Recommended disposition (a) immediate fix. All other PENDING invariants disposition to PASS or (b)/(d) non-blocking. Phase 2 PLAN is gated only on F-005's (a) fix landing.

---

## 1. Sources scanned (Day 1 §3 DEFERRED rows resolved)

| Source | Day 1 status | Day 2 status | Notes |
|---|---|---|---|
| `docs/bible.md` Article 8 (8.5.1–8.5.5) | PARTIAL | **COMPLETE** | 8.5.1 ↔ INV-SYNC-1/2; **8.5.2 + 8.5.3 are new invariants** (ISR no-alloc; scheduler heap-independence) — see §3 |
| `docs/bible.md` Article 12 (Security Model) | PARTIAL | **COMPLETE** | 12.1 ↔ INV-CAP-1/INV-NEG-1; 12.3 ↔ SMEP/SMAP/NX/KASLR/W^X (TESTED, tangential — §3 INV-SEC-1). No new cap invariants beyond ADR-010 rows; bible carries them at lower resolution |
| `docs/phases/PHASE-1.5c.md` | DEFERRED | **COMPLETE** | Confirms INV-IPC-10/12 lineage (Course Corrections §147, §161-165). **New: transition-note constraint "do not add locks to `Pmap::translate`"** → INV-IPC-13 (§3) |
| `kernel/src/ipc/mod.rs` doc-comments | PARTIAL | **COMPLETE** | EFAULT rendezvous handling confirmed (l.173-175, 242-243); `extract_my_side_if_blocked` (l.468) is the F-005 site |
| `kernel/src/sched/scheduler.rs` doc-comments | DEFERRED | **COMPLETE** | `kill_thread_by_pid` (l.1024) ready-queue-only; blocked handled by `terminate`. Fixed-capacity queues enforce INV-SYNC-5 via debug_assert |
| `kernel/src/sched/process.rs` doc-comments | DEFERRED | **COMPLETE** | `terminate` blocked-walk (l.952-976); rights checks (derive/grant/revoke) at l.507/549/615 — cleanly separate from cleanup |
| `kernel/src/sched/thread.rs` doc-comments | DEFERRED | **COMPLETE** | `Thread::pid()` accessor (added PR #6); `new_kernel_for_process_test` used by q8c test |
| `kernel/src/sched/mod.rs` doc-comments | DEFERRED | **COMPLETE** | Re-export surface; no new invariant claims |
| `kernel/src/sched/kthread.rs` | (lower priority) | SAMPLED | Phase-3 kthread join/stop; out of IPC/sched/cap audit lens — no 1.5b invariants |
| Git: INV-SYNC-2 IrqSafeMutex | DEFERRED | **COMPLETE** | Implemented `27d1b99` (ADR-019 sub-phase A). **Only sub-phase A landed**; B/C (call-site migration) not present. Adoption: `tlb_shootdown.rs` only — NOT the Channel.state sites |
| Git: INV-PROC-7/NEG-3 Q8.c walk | DEFERRED | **COMPLETE** | Blocked-kill implemented in `terminate` (not `kill_thread_by_pid`). Restriction LIFTED + TESTED for same-process case |
| CI: INV-IPC-11/12 regression | DEFERRED | **COMPLETE** | Run `26364516256` (today, full suite): `ipc_heap_kill_smoke PASS`. Deferred-quantum removal did not break the smoke |

All §2 dispatch sources scanned. No source could not be completed.

---

## 2. Method note

Day 1 established two lenses: **Lens A** (1.5b leak-audit vocabulary blind spots) and **Lens B** (ADR-013 SMP-retrofit invariants newly-reachable under SMP + capability grants). The close_peer seed bug was a Lens-B hit. Day 2's highest-value move was to apply the close_peer shape — *exit/kill-side cleanup acting on a parked thread without a pid check* — to every adjacent code path. That move found F-005. The other PENDING invariants resolved through direct source + git + CI verification.

---

## 3. New invariants found (augment Day 1 inventory; do not replace)

| ID | Invariant (verbatim/close) | Source | Evidence | Lens |
|---|---|---|---|---|
| INV-SYNC-4 | No heap allocation in interrupt context: ISR handlers and transitive callees may not allocate/deallocate from the global heap; ISR paths use pre-allocated/fixed/stack-bounded structures. | bible 8.5.2 | CLAIMED — discipline rule; enforced by review + Article 15.2 SAFETY comments. No runtime lockdep-equivalent. | A |
| INV-SYNC-5 | Scheduler heap-independence: any code path holding a per-CPU scheduler lock must not depend on the heap allocator; scheduler structures use bounded/pre-allocated storage so scheduling never triggers heap alloc. | bible 8.5.3 | **TESTED-by-debug-assert (partial)** — `rotate`/`wake_thread`/`zombify_thread` carry `debug_assert!(capacity > len)` guarding against scheduler-held `push_back` reallocation. Fixed capacities: `READY_INITIAL_CAPACITY=8`, `ZOMBIE_INITIAL_CAPACITY=4`. | A |
| INV-IPC-13 | The IPC rendezvous translate path (`Pmap::translate`) is lock-free; no lock may be added to it. Receiver translates sender's user buffer at rendezvous without acquiring a Pmap lock. | PHASE-1.5c transition note (l.265-267); ADR-004 rendezvous design | CLAIMED — load-bearing transition constraint; preserved by current code (translate is a page-walk read, no lock). | B |
| INV-SEC-1 | SMEP, SMAP, NX, KASLR, W^X mandatory from Phase 1. | bible 12.3 | TESTED — `smep_catch`, `smap_catch`, `smep_policy`, `smap_policy`, `kaslr_smoke`, `kaslr_heap_contract` all PASS (CI `26364516256`). | A (tangential to IPC/sched/cap lens) |

INV-SEC-1 is recorded for inventory completeness but is outside the audit's close_peer lens and needs no failure-mode analysis here (well-TESTED, bible-backed, Phase-1-closed).

---

## 4. Failure-mode analyses + 5. Proposed dispositions

Combined per invariant for readability. Dispositions are **recommendations**; PM ratifies. Tags per scope §6: PASS / PARTIAL→(a/b/c/d) / DEFECT→(a/b/c/d).

### F-005 — INV-PROC-7 / INV-NEG-3 implementation: cross-process zombify in the forced-kill blocked-walk (NEW DEFECT)

**The lift is real and tested.** `process::terminate` (process.rs:909) lifts the Phase-1.5b -EINVAL-on-Blocked restriction: when `kill_thread_by_pid` (ready-queue-only, all CPUs) returns false, terminate walks the target's drained channel handles via `Channel::extract_my_side_if_blocked(side)` → `zombify_thread`, with a `zombify_parked_thread_by_pid(target_pid)` fallback. Tested: kernel `q8c_blocked_kill_walk` (PASS, CI `26364516256`) and userland `sys_kill on Blocked worker (expect 0)` (neo-init main.rs:70-72). So INV-PROC-7/INV-NEG-3-as-lifted = **PASS for the same-process case.**

**The defect.** `extract_my_side_if_blocked` (ipc/mod.rs:468) extracts whatever thread is parked on the target's side and returns it **with no pid check** — identical to the pre-PR-#6 `close_peer` `extracted_my` flaw. The caller's assumption is stated verbatim at process.rs:955-958: *"Phase 1.5b has one thread per process. If the target was not Ready, the first parked thread found through its channel handles is the target."* PR #6 established that this assumption is false under SMP + capability grant: a **foreign** process's thread can be parked on the target's side via a granted derivation. The close_peer fix (commit `4dfb13f`) only fixed the natural-exit path (`close_channel_peers` → `close_peer`, now pid-aware). The forced-kill path's `extract_my_side_if_blocked` walk is a **separate, parallel extraction that PR #6 did not touch.**

**Failure modes not detected by existing tests:**
1. **Wrong-process zombify (forced kill).** A force-kills B; B's own thread is blocked elsewhere; a grantee thread C (pid≠B) is parked on B's channel side. `terminate(B)` walks B's handles, finds C parked, zombifies C, sets `found_blocked=true`, and `break`s — C is wrongly terminated and B's own thread is never reaped via this branch.
2. **Untested.** `q8c_blocked_kill_walk` spawns the blocker with the *target's own pid* (`new_kernel_for_process_test("q8c-recv-blocker", pid, …)`); the userland scenario kills a worker blocked on its *own* channel. Neither sets up a foreign grantee parked on the killed process's side. The cross-process forced-kill case has zero coverage — same "passes for the wrong reason / coverage shape too narrow" pattern as F-002.

**Reachability.** Requires (i) a ProcessHandle to force-kill the donor, (ii) the donor's own thread not Ready, (iii) a grantee parked (SendBlocked/RecvBlocked) on the donor's side via a granted derivation. Preconditions (iii) are exactly the close_peer scenario that `cap_survive_exit` proved constructible; (i)+(ii) add forced kill of a blocked donor. Constructible in principle today; more naturally reachable as Phase 2 grant/kill surface widens.

**Proposed disposition: DEFECT → (a) immediate kernel fix.** Localized, mirrors PR #6, no Phase-2-cap-ABI surface touched (scope §6 keeps it at (a)). **Fix shape:** thread `target_pid` into the blocked-walk so `extract_my_side_if_blocked` only extracts a waiter whose `waiter.pid() == target_pid`; a foreign waiter is left parked and gets woken with -EPIPE by the subsequent `close_channel_peers(&drained, target_pid)` (already pid-aware post-#6). Two viable implementations: (a1) add an `expected_pid: ProcessId` parameter to `extract_my_side_if_blocked` and check before `Some(waiter)`; (a2) return the waiter but have `terminate` check `parked.pid() == target_pid` before zombify, re-park-or-defer otherwise. (a1) is closer to PR #6's chosen shape. **Dispatch shape:** a Codex/Jarvis kernel-fix dispatch citing F-005, touching `kernel/src/ipc/mod.rs` + `kernel/src/sched/process.rs`, with a new test `q8c_blocked_kill_foreign_parked` (or userland equivalent) asserting the foreign grantee is woken with -EPIPE and not zombified, and the donor's own thread is the one reaped. Per dispatch §5, I do not write the dispatch — PM files it.

**Blocking for Phase 2 PLAN: YES** (per scope §10, (a)-disposition immediate fixes must land before Phase 2 PLAN). This is the audit's one blocking item.

### INV-IPC-5 — page-translate fault in rendezvous

**Code present, both arrival orders.** ipc/mod.rs:173-175 (first arrival / blocker wakes with -EFAULT via `handoff_to_rendezvous_peer(recv_thread, EFAULT)`); 242-243 (second arrival returns -EFAULT directly via `continue_current_after_receive(send_thread, EFAULT)`). The "no partial copies" claim is upheld by structure (translate-then-copy; fault aborts before copy).

**Failure modes not detected by existing evidence (CLAIMED — no fault-injection test):** (i) blocker-wakes-with-EFAULT path never exercised; (ii) second-arrival-returns-EFAULT path never exercised; (iii) a regression that introduces a partial copy before fault detection would not be caught.

**Proposed disposition: PARTIAL → (b) BUILD-N test work.** Add an IPC fault-injection scenario passing an unmapped/protected user pointer during rendezvous; assert -EFAULT on both arrival orders and no partial copy (e.g., destination buffer unmodified). Not blocking — code is correct on review; this closes a coverage gap. Recommended test: `ipc_rendezvous_translate_fault`.

### INV-IPC-11 / INV-IPC-12 — BUILD-2/4 handoff amendments (REPLACED/REMOVED under ADR-013 Q5)

**Verified.** ADR-013 Q5 removed the bounded deferred-quantum mechanism (INV-IPC-12 / INV-HANDOFF-2/3/4). The dependent test `ipc_heap_kill_smoke` **PASSes** in current full-suite CI (run `26364516256`, today). The removal did not break the smoke. INV-IPC-11 (no slow diagnostic syscalls in handoff window) is a test-design discipline, not a runtime check; it remains encoded in test structure.

**Proposed disposition: PASS.** The REMOVED/REPLACED mechanisms' dependent test survives their removal. No failure mode reachable from the deletion. No action.

### INV-PROC-2 — self-kill -EINVAL

**CLAIMED, guard present, runtime test deferred to Phase 4.** ADR-007 Implementation note 1 documents the explicit `sys_kill_impl` self-kill guard; the runtime test is deferred because constructing it requires cap-passing-over-IPC (Phase 4). Source confirms `sys_kill` rejects self upstream (the `kill_thread_by_pid` doc-comment notes "Caller must ensure `pid != current_pid()` … which `sys_kill` rejects upstream").

**Proposed disposition: PARTIAL → (d) documented deferral (already in ADR-007).** The deferral is pre-existing and ADR-recorded; no new artifact needed. **Not 1.5b-audit-blocking** — it's a Phase-4-reachable test gap, not a current correctness defect. PM may simply note it carries forward.

### INV-PROC-5 under SMP — reaper double-cleanup benign (Day 1 F-004, strengthened)

**Day 2 source check.** `terminate`'s sequencing is: `kill_thread_by_pid` → blocked-walk → `PROCESSES.remove` (registry-first, with `debug_assert` confirming absence) → drain handles → `close_channel_peers` → push to zombies. `on_thread_reap`'s early-return guard (`PROCESSES.get(&pid)` → None) depends on this ordering. Source confirms the ordering holds in current code; the F-004 risk is that future refactoring reorders it silently. The full suite (incl. forced-kill + natural-exit paths) PASSes under `NEO_SMP=4` in CI, so the invariant holds empirically.

**Proposed disposition: PARTIAL → (b) BUILD-N test work** (carries Day 1 F-004 unchanged). Recommended test: `smp_concurrent_reap_after_terminate` asserting no double `close_peer` when terminate(CPU0) races on_thread_reap(CPU1). Not blocking.

### INV-CAP-11 — spawn-inheritance derived-copy descendants in revoke walk

**A descendants-walk test exists and passes** (`cap_revoke_descendants_walk`, CI `26364516256`). The Day 1 concern stands: it is not confirmed that the test builds its derivation chain via **spawn-time inheritance** (sys_spawn passing handles → fresh parent-linked child CapIds) rather than via in-process explicit derive. The revoke walk's correctness for spawn-inheritance descendants (cross-process parent→child links) is the cap-mediated, cross-process case the audit lens flags.

**Proposed disposition: PARTIAL → (b) BUILD-N test work.** Either confirm the existing test exercises a spawn-inheritance chain, or add `cap_revoke_spawn_inheritance_descendants` asserting that revoking a root cap held by a parent reaches a grandchild cap inherited by a spawned descendant. Not blocking — the general descendants walk is TESTED; this closes the spawn-specific corner. **Open question for PM (§7).**

### INV-CAP-12 — CLOSE bit enforcement (sys_close not shipped)

**Not yet reachable.** `sys_close` does not exist; the CLOSE semantic bit is reserved (ADR-010 Principle 2). There is no code to audit for enforcement, and INV-CAP-13 (below) already confirms kernel cleanup paths are exempt by construction.

**Proposed disposition: PASS (deferred-correctly).** No 1.5b code violates it because the gated operation does not exist. Re-audit when `sys_close` lands (Phase 2+). Note carried, not blocking.

### INV-CAP-13 — kernel-cleanup paths exempt from rights checks

**Confirmed by source.** Rights checks (`has_meta(DERIVE/GRANT/REVOKE)`, `is_subset_of`) live exclusively in the capability operations in `process.rs` (derive l.507-510, revoke l.549, grant l.615). The cleanup paths — `close_peer`, `close_channel_peers`, `extract_my_side_if_blocked` — operate directly on `Channel`/`EndpointState` and never call any rights-checking function (grep: no `rights_check`/`has_meta`/`is_subset_of` in `ipc/mod.rs`). The exemption is structural: cleanup manipulates kernel object state, not capabilities.

**Proposed disposition: PASS.** Invariant holds by construction. **Caveat note:** this same structural exemption is *why* F-005 is reachable — cleanup bypasses the cap layer, so the only protection against operating on the wrong process's thread is an explicit pid check, which `extract_my_side_if_blocked` lacks. The exemption is correct; F-005 is the missing complementary guard.

### INV-SYNC-2 — IrqSafeMutex type-level enforcement

**Implemented (sub-phase A only); adoption minimal.** `kernel/src/sync/irqsafe_mutex.rs` exists (commit `27d1b99`, ADR-019 sub-phase A), correctly disabling IF before lock and restoring prior IF state on drop via `ManuallyDrop` ordering. **Only sub-phase A landed** — no sub-phase B/C migration commits. Adoption is limited to `mm/tlb_shootdown.rs`; the five Channel.state mixed-context call sites that INV-SYNC-1 covers still use `SwitchMutex` + manual `without_interrupts(...)`.

**Failure-mode analysis:** the invariant as stated ("IrqSafeMutex provides type-level enforcement of INV-SYNC-1 for mixed-IRQ-context locks") is *true for the mechanism* but the *type-level enforcement is not yet applied* where the Hpw risk actually lives. Current safety at those sites rests on INV-SYNC-1's manual discipline, which is **TESTED** (SC-3B sub-step 5c, 200/200 PASS at `NEO_SMP=4`). So there is no live correctness gap — there is a defense-in-depth gap (manual discipline instead of type-enforced).

**Proposed disposition: PARTIAL → (d) documented known-issue, already tracked in ADR-019.** ADR-019 records sub-phases A/B/C; B/C (migrate mixed-context sites to IrqSafeMutex) are the remaining work and are Phase-2-appropriate. No new artifact needed beyond confirming ADR-019's B/C status reflects "A landed, B/C pending." **Not blocking** — INV-SYNC-1 manual discipline is TESTED and holds.

### INV-NEG-3 — blocked-thread kill lifted under SMP

Same code + tests as INV-PROC-7. **PASS for the lift** (TESTED same-process). The cross-process foreign-parked corner is **F-005** (DEFECT → (a)). Recorded once, under F-005.

---

## 6. Updated net findings table

Combines Day 1 §7 (F-001..F-004) with Day 2 resolutions.

| Disposition | Count | Findings |
|---|---|---|
| PASS | 6 | F-003 (INV-IPC-8, Day 1); INV-IPC-11/12; INV-CAP-12 (deferred-correctly); INV-CAP-13; INV-NEG-3/INV-PROC-7 *lift*; INV-SEC-1 (new, TESTED) |
| PARTIAL → (a) immediate fix | 0 | — |
| PARTIAL → (b) BUILD-N test | 4 | F-004 (INV-PROC-5 SMP); INV-IPC-5 (fault-injection); INV-CAP-11 (spawn-inheritance descendants); INV-SYNC-5 (capacity-bound, see open Q) |
| PARTIAL → (c) bible amendment | 0 | — |
| PARTIAL → (d) documented deferral | 2 | INV-PROC-2 (Phase 4, ADR-007); INV-SYNC-2 (ADR-019 B/C) |
| **DEFECT → (a) immediate fix** | **1** | **F-005 (cross-process zombify in forced-kill blocked-walk) — BLOCKING** |
| DEFECT → (a) COMPLETE (Day 1) | 2 | F-001 (INV-IPC-7), F-002 (INV-CAP-6) — fixed in PR #6 |

**Net:** one open DEFECT (F-005, blocking, recommended (a)). Two DEFECTs already closed (PR #6). Four (b) test-work items absorbable into Phase 2 BUILD. Two (d) deferrals already ADR-tracked. No (c) bible amendments warranted.

**Load-bearing result for Phase 2 PLAN:** Phase 2 PLAN is gated on exactly one item — F-005's (a) fix landing (per scope §10, (a) fixes block; (b)/(d) do not). Once F-005 is fixed and CI green, the foundation's correctness boundary is characterized and Phase 2 PLAN may start.

---

## 7. Open questions for PM

1. **F-005 fix shape (a1 vs a2).** Recommend (a1) — add `expected_pid` to `extract_my_side_if_blocked`, mirroring PR #6's `close_peer` signature change — for consistency with the already-shipped fix. PM to confirm before filing the Codex/Jarvis dispatch. *Decision needed before the fix dispatch.*

2. **INV-CAP-11 — does `cap_revoke_descendants_walk` already exercise a spawn-inheritance chain?** I confirmed the test exists and passes but did not fully trace its userland body (feature-gated in `neo-init`) to determine whether the derivation chain is built via `sys_spawn` handle inheritance or via in-process derive. If the former, INV-CAP-11 upgrades to PASS and the (b) item drops. PM (or a follow-on read) to resolve. *Affects whether one (b) item exists.*

3. **INV-SYNC-5 capacity bound — is fixed `READY_INITIAL_CAPACITY=8` a Phase-2 risk?** Bible 8.5.3 forbids heap allocation under the scheduler lock. The ready queue is a fixed-capacity `VecDeque` (cap 8); a `push_back` beyond capacity reallocates *under the scheduler lock*, violating 8.5.3. Today a `debug_assert!` catches this — **but only in debug builds**; a release build silently allocates. Phase 1.5b/1.5c never exceed a handful of ready threads per CPU, so it has never fired. Phase 2 (more processes, SMP load-balancing onto AP run-queues) could plausibly exceed 8 ready threads on one CPU. **Recommend (b)/(d):** either a capacity-pressure test (`sched_ready_queue_capacity_pressure`) asserting behavior at/above cap, or a Phase-2 ADR deciding the bound's growth policy (grow-and-accept-one-alloc-outside-lock, vs. larger fixed cap, vs. intrusive list). *Is this in scope for this audit's findings, or a fresh Phase-2-OBSERVE item?* I have tagged it (b) provisionally; PM decides whether it belongs to this audit or Phase 2.

4. **Does F-005 warrant re-examining whether PR #6's close_peer fix and the F-005 forced-kill fix should share a single pid-aware extraction helper?** Both paths extract a parked thread on a process's side and must respect pid ownership. A unified `extract_parked_if_owned(side, pid)` primitive would prevent a *third* instance of this bug class from reappearing. This is a (a)-fix design choice / light refactor — flagging for PM as a possible scope note on the F-005 dispatch.

---

## 8. Close

Deliverable filed at `Plans/CEREBRO-OUTPUT-2026-05-24-audit-day-2-findings.md`. Per dispatch §4 C5/C6: pushed to `origin/main`; Day 1 PM-AUDIT artifact NOT amended (PM's consumption step). Commit hash reported in the return note accompanying this file.

Per dispatch §6, PM next: consume → ratify/revise dispositions → amend Day 1 PM-AUDIT with final findings → file `PM-DISPOSITION-2026-05-24-pre-phase-2-audit-close.md` naming F-005 as the blocking item for Phase 2 PLAN → file the F-005 Codex/Jarvis fix dispatch (and resolve open questions 1-4).

## 9. References

- `Plans/PM-AUDIT-SCOPE-2026-05-24-pre-phase-2-1.5b-invariants.md` — binding scope.
- `Plans/PM-AUDIT-2026-05-24-pre-phase-2-1.5b-invariants.md` — Day 1 inventory (commit `c2a4411`).
- `Plans/DISPATCH-2026-05-24-audit-day-2-cerebro.md` — this dispatch.
- `Plans/CLOSE-REPORT-2026-05-24-ci-caching-and-cap-survive-exit.md` — close_peer seed (PR #6).
- Commit `4dfb13f` — PR #6 close_peer pid-aware fix (natural-exit path).
- Commit `27d1b99` — IrqSafeMutex (ADR-019 sub-phase A).
- Commit `e2b3f4f` — SC-3B sub-step 5c (ADR-013 Q4/Q6/Q7 closure, INV-SYNC-1/3 tests).
- CI run `26364516256` (2026-05-24) — full `cargo neo-test` suite PASS (source for INV-IPC-11/12, INV-PROC-7, INV-SEC-1 evidence).
- Source: `kernel/src/ipc/mod.rs:468` (`extract_my_side_if_blocked`), `kernel/src/sched/process.rs:909-977` (`terminate`), `kernel/src/sched/scheduler.rs:1024` (`kill_thread_by_pid`), `kernel/src/sync/irqsafe_mutex.rs`.
- bible Articles 8 (8.5.1-8.5.5), 12; `docs/phases/PHASE-1.5c.md`.
