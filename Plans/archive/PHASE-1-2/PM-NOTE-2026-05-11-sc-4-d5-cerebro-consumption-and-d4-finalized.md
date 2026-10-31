# PM-NOTE — SC-4 D5 Cerebro consumption + D4 finalized + sub-commit 5b reframed

**Date filed:** 2026-05-11
**Author:** PM
**Type:** PM-NOTE. Consumes `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (Cerebro deep-research artifact, 52.1 KB, 14 sections, primary-source citations). Finalizes D4 in `Plans/SC-4-THINK-2026-05-11.md`. Reframes sub-commit 5b scope per Cerebro's Phase A finding.
**Authority:** PM operational decision per PL clarification 2026-05-10 (PM owns operational + technical decisions; D-decisions are PM-tier). PL-tier sign-off not required for this consumption.

---

## 1. Headline finding

**PM was solving the wrong problem in THINK §5.1.** PM's D4 lean was "what lock type for `PROCESSES` registry?" Cerebro's research reframed the question to "**does `PROCESSES` need to be on the hot path at all?**" The substrate-audited answer is no. Phase A (retire `cr3_for_thread(pid)`; have scheduler context-switch path access `thread.pmap` directly) eliminates the contention before the lock-type choice matters.

This is the SC-3C 4a per-CPU CR3 cache pattern applied at a different layer: SC-3C cached the CR3 value per-CPU to skip redundant `Cr3::write` calls; SC-4 Phase A puts the `Arc<Pmap>` directly on the Thread so the scheduler skips the registry lookup entirely. Linux uses the same conceptual pattern via `cpu_tlbstate.loaded_mm` (per-CPU GS-relative pointer to the active `mm_struct`). NeoPlutarch's already-deployed `per_cpu.active_pmap_cr3` is conceptual kin.

## 2. PM substrate audit verifying Cerebro's Phase A assumption

Cerebro's Phase A recommendation depends on: *all callers of `cr3_for_thread(pid)` are in the scheduler context-switch path with a `&Thread` in scope.* PM verified:

```text
$ grep -rn "cr3_for_thread" kernel/src/
kernel/src/sched/process.rs:343:    pub fn cr3_for_thread(pid: ProcessId) -> (PhysFrame, Cr3Flags) { ... }
kernel/src/sched/scheduler.rs:45:   use super::process::{KERNEL_PID, ProcessId, cr3_for_thread};
kernel/src/sched/scheduler.rs:300:      let (frame, flags) = cr3_for_thread(next.pid);
kernel/src/sched/scheduler.rs:306:      let (frame, flags) = cr3_for_thread(next.pid);
```

Both call sites (`scheduler.rs:300` and `scheduler.rs:306`) are in the cross-process context-switch path with `next` (the Thread being switched to) in scope. `Thread` already carries `pub(super) pid: ProcessId`. Adding `pmap: Option<Arc<Pmap>>` to Thread (None for kernel-PID threads; Some(Arc::clone(&proc.pmap)) for user threads) is a mechanical extension; the scheduler then accesses `next.pmap` directly via the Thread reference it already has.

**Phase A is viable. PROCESSES leaves the hot path entirely.**

## 3. D4 finalized: Phase A only; Phase B contingent and deferred

**D4 (PM finalization, PM-tier):**

- **Phase A** lands in sub-commit 5b. Implementation:
  1. Add `pmap: Option<Arc<Pmap>>` field to `Thread` struct.
  2. Plumb the Pmap Arc from `Process::new_user` through `Thread::new_user`.
  3. Replace `let (frame, flags) = cr3_for_thread(next.pid);` at scheduler.rs:300 and :306 with `let (frame, flags) = next.pmap.as_ref().map(|p| p.cr3()).unwrap_or_else(kernel_cr3);` (or equivalent).
  4. Mark `cr3_for_thread` `#[deprecated]` or retire entirely after the scheduler stops calling it. PM operational call: **retire entirely** — dead code clarifies intent, and any future caller wanting CR3-by-PID should reconsider their access pattern.
  5. `PROCESSES` registry stays `spin::Mutex<BTreeMap<ProcessId, Arc<Process>>>` for the remaining cold-path operations (revoke, complete_grant, terminate, on_thread_reap, name_of, any_user_process). Cerebro's cross-validates §5.2 confirm this is the production pattern at NeoPlutarch's scale (FreeBSD `allproc`, illumos `pidlock`).

- **Phase B** is contingent. If Phase A's audit surfaces a `cr3_for_thread`-equivalent caller PM missed (e.g., a future syscall handler that doesn't have `&Thread` in scope but needs CR3-by-PID), introduce `AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` for CR3 data ONLY using the `arc-swap` pattern documented in Cerebro §10. Keep `spin::Mutex` for the metadata registry. **Phase B is deferred to "only if Phase A reveals a blocker"**; not part of SC-4 baseline plan.

- **Phase B negative space:** **Do NOT use `spin::RwLock`** under any circumstance. Cerebro §5.4 documents the 5× regression vs `Mutex` for short critical sections from cache-line reader-counter ping-pong, with the failure mode appearing precisely at the 4-64 CPU scale that matters for NeoPlutarch. The `spin` crate's own documentation warns of writer starvation under read pressure. This is recorded as a Phase 2+ forward marker (§7 below) so future contributors don't reach for it as a default upgrade.

## 4. Four PM THINK corrections (Cerebro §3-§5 cited)

Folding back into SC-4 THINK at next revision:

| # | PM THINK assumption | Cerebro correction (citation) |
|---|---|---|
| 1 | `tasklist_lock` is Linux's PID-lookup serialization (implied in §3 R7) | **`tasklist_lock` is task-list (parent/child/signal topology) serialization, NOT PID-lookup. The PID IDR uses a separate `pidmap_lock` (`kernel/pid.c:89`, `DEFINE_SPINLOCK`, `__cacheline_aligned_in_smp`).** Linux's two-lock decomposition pays off only when (a) many CPUs race on fork/exit AND (b) for-each-process iteration doesn't need PID lookup. NeoPlutarch's single-lock model is equivalent in practice at our scale. (Cerebro §3.3) |
| 2 | `spin::RwLock` is a safe upgrade path from `spin::Mutex` (implied alternative in §5.1 L1') | **Documented 5× regression vs Mutex for short critical sections** (parking_lot benchmarks per "Read Locks Are Not Your Friends" empirical results). Cache-line invalidations from reader-counter atomic dominate even when no write is occurring. `spin::RwLock`'s own crate docs warn of writer starvation under read pressure. Failure mode appears at 4-64 CPUs — exactly NeoPlutarch's target range. (Cerebro §1, §5.4, §10) |
| 3 | PM's `AtomicPtr<Arc<BTreeMap>>` lean cited Linux RCU task_struct as direct prior art | **PM's lean was technically defensible but is an application-level pattern (vorner's `arc-swap` crate, used in production by TiKV and signal-hook), not an OS-kernel-level pattern. No production OS kernel uses `arc-swap`-style AtomicPtr<Arc<BTreeMap>> for process tables.** Linux RCU is different (RCU-protected linked list with grace-period reclamation, not whole-structure copy-on-write); citing Linux RCU as direct prior art for an arc-swap design is misleading. (Cerebro §1, §10, §11) |
| 4 | The PROCESSES contention is a synchronization-primitive choice (D4 framing in §5.1) | **The PROCESSES hot-path contention is architecturally avoidable, not a synchronization-primitive choice.** Linux's `loaded_mm` per-CPU pointer, seL4's `current_thread()` per-CPU pointer, and NeoPlutarch's already-deployed `active_pmap_cr3` per-CPU cache are all the same pattern: the running entity's CR3-relevant data is published per-CPU at context-switch time, removing the registry from the hot path. PM's D4 framing entered the wrong design subtree. (Cerebro §1, §11) |

PM acknowledges all four corrections. Folding into SC-4 THINK v2 (revision pending; for now this PM-NOTE serves as the authoritative D4 disposition).

## 5. Cerebro cross-validates on other D-decisions

Cerebro surfaced cross-validates for two PM THINK leans not directly in scope of D5:

### 5.1 D1 validated (SwitchMutex for `Process.handles` and `Process.derivations`)

Cerebro §4 (seL4 BKL evaluation per Peters & Heiser 2014, plus arXiv 1609.08372 Redis macrobenchmark): a single coarse lock scales to ~14 cores within a socket for short syscalls, with ARM quad-core scaling perfectly when critical sections are short (<500 cycles). NeoPlutarch's per-process handle table operations are short (Vec lookup, BTreeMap insert/remove) and well within this regime.

**D1 stands as ratified:** `SwitchMutex<HandleTable>` and `SwitchMutex<BTreeMap<CapId, Option<CapId>>>` for `Process.handles` and `Process.derivations`. The per-process lock granularity is correct; SwitchMutex composes with the SC-3B/SC-3C discipline; the scale Peters/Heiser demonstrate is well above our 4-64 CPU target.

### 5.2 D7 validated (`Process.exiting: AtomicBool` for grant-vs-target-exit race)

Cerebro §6 (illumos doors model from `usr/src/uts/common/fs/doorfs/door_sys.c`): illumos's cross-process IPC bypasses PID lookup entirely. Server processes are reached via a vnode-backed door descriptor; `door_knob` serializes concurrent accesses and `DOOR_INVALID` checks server liveness atomically. This is **structurally identical to NeoPlutarch's Channel-grant pattern**: the channel capability handle (not the server PID) is the durable cross-process reference, and the grant rendezvous needs only to verify the recipient's liveness flag (PM's D7 lean: `Process.exiting: AtomicBool`), not to re-traverse a process table.

**D7 stands as ratified.** Cerebro's framing is sharper than PM's: the AtomicBool is not a "race fix" but the canonical liveness primitive that mirrors illumos's `DOOR_INVALID`. PM updates D7 wording in THINK v2 to reflect this.

### 5.3 FreeBSD confirms no-SMR design at NeoPlutarch's scale

Cerebro §5 (FreeBSD `kern/kern_proc.c` `_pfind` body): FreeBSD's per-bucket `sx` shared-locks plus lock-handoff to `PROC_LOCK(p)` is the production pattern at comparable scale. **No SMR (Safe Memory Reclamation, FreeBSD's RCU-equivalent) is used for the `allproc` list.** Locked-handoff is the production design.

This validates PM's general operational frame: at NeoPlutarch's scale, the right design is conservative locking with carefully-bounded critical sections, not RCU/SMR/epoch-reclamation machinery. RCU's complexity is engineered for thousand-CPU machines with millions of processes; we are two orders of magnitude smaller and our patterns should reflect that.

## 6. Sub-commit 5b scope reframed

D15 in SC-4 THINK originally specified sub-commit 5b as: "PROCESSES retrofit + exiting flag + exit_state CAS."

**With D4 Phase A, the PROCESSES retrofit is ELIMINATED from sub-commit 5b.** Revised 5b scope:

- Add `pmap: Option<Arc<Pmap>>` field to `Thread`.
- Plumb the Arc through `Process::new_user` → `Thread::new_user`.
- Replace `cr3_for_thread(next.pid)` call sites at `scheduler.rs:300, :306` with direct `next.pmap` access.
- Retire `pub fn cr3_for_thread` from `process.rs` (dead code after step 3).
- Add `Process.exiting: AtomicBool` field; check at install_cap site (per D7).
- Add `Process.exit_state: AtomicU8` field; CAS-claim at top of `on_thread_reap` and `terminate` (per D8); CAS-loser observes EXITED and bails.

Sub-commit 5b is now substantially smaller and architecturally cleaner. Estimated diff: ~200-300 lines (vs PM's original estimate of 500-800 for the full PROCESSES retrofit).

**Sub-commits 5a and 6 are unchanged.** 5a: handles + derivations SwitchMutex retrofit. 6: blocked-thread-kill walk + close_peer my-side fix + `sched::zombify_thread` primitive.

## 7. New Phase 2+ forward markers (from Cerebro consumption)

Carry forward into PHASE-2.md at the next forward-markers integration:

- **FM-D5.1: `spin::RwLock` is documented-pathological at our scale.** Future contributors should not reach for `spin::RwLock` as a "safe upgrade" from `spin::Mutex`. Use case for RwLock-shape needs is the SC-3B SwitchMutex pattern (per-process / per-channel) or the Phase A per-Thread direct-access pattern. Document in a new ADR if the discipline needs to be normative.

- **FM-D5.2: `arc-swap` pattern is the Phase B fallback if Phase A fails.** PM Phase A audit is complete; Phase B is not currently triggered. If a future syscall handler needs CR3-by-PID without `&Thread` access (theoretically possible: a sys_inspect-style RPC), the `arc-swap` pattern in Cerebro §10 is the canonical solution. Do NOT introduce until concretely needed.

- **FM-D5.3: illumos doors design is the architectural reference for NeoPlutarch's cross-process IPC.** Per Cerebro §6.4: durable cross-process references via capability handles (not PIDs); per-target liveness flag (AtomicBool/AtomicU8); short locked critical sections at the rendezvous point. This is the design language we should formalize as Phase 2 BUILD-3 closes and Phase 3 (NeoFS / cross-machine IPC) approaches.

- **FM-D5.4: PROCESSES registry remains single-lock `spin::Mutex` at SC-4 close.** This is correct at NeoPlutarch's scale (validated by FreeBSD `allproc`, illumos `pidlock`, seL4 BKL within socket). If profiling at Phase 3+ shows process-management operations as a bottleneck (sys_spawn / sys_kill / sys_wait throughput under high process-churn workloads), reconsider per Cerebro §10's Phase B framing.

## 8. Verification scenarios (D16 + D17) unchanged

D16 (`q8a_concurrent_install_revoke` with `neo-test-missing-cap-lock-negative`) and D17 (`q8c_blocked_kill_walk`) per SC-4 THINK §7.1 stand as ratified. Cerebro consumption does not affect verification scope.

Adding one new Phase A verification gate:

- **D18 (PM operational, NEW post-Cerebro consumption):** Phase A regression test. Spawn user process; verify `cr3_for_thread` is not called during normal context switches (via instrumentation feature `neo-test-d5-phase-a-coverage` that panics if `cr3_for_thread` is invoked); verify CR3 reload still works correctly via existing SC-3C 4a coverage tests. Test PASSES when the per-Thread access path is exercised and the legacy registry-lookup path is confirmed dead.

## 9. Calibration data: Cerebro D5 dispatch

Cerebro D5 calibration capture per voice-profile discipline:

**Dispatch quality:** Strong. Three parallel research agents synthesized; primary-source citations throughout (Linux source URLs, illumos source URLs, FreeBSD source URLs, Peters/Heiser paper PDF, arXiv 2407.20559v1, vorner blogs, RFC-style discipline on the four PM THINK corrections).

**Citation precision:** Four PM corrections were specific and well-cited. Cerebro distinguished between "what NeoPlutarch should do" recommendations and "what production kernels actually do" findings — the latter is the durable contribution.

**Architectural reframing:** The highest-value finding (Phase A reframing) was not requested in PM's dispatch §3 questions. Cerebro identified that the dispatch's question framing was itself a layer below the right one, and surfaced the right one. This is the kind of value PM cannot generate from training-data heuristics alone.

**Voice profile firming continues:** Cerebro is now established as the agent that catches PM-framing errors when they occur. Future PM cadence should incorporate a "have I framed the right question?" gate before dispatching Cerebro — but Cerebro is the safety net when PM misses it.

**No follow-up Cerebro dispatch needed for SC-4.** Remaining D-decisions (D1-D3, D6, D7, D8, D9-D12, D13-D17) are PM-tier with prior art Jarvis can cite at BUILD time. PLAN-stage dispatch authoring is the next operational step.

## 10. Operational next steps

1. **Update SC-4 THINK** (revision v2) to fold in: D4 finalization per §3, four corrections per §4, D7 wording refinement per §5.2, sub-commit 5b reframing per §6, new forward markers per §7, new D18 per §8. Defer to PLAN-stage authoring time — the consumption PM-NOTE is the authoritative interim record.
2. **Author SC-4 PLAN-stage dispatch** for Jarvis. Three sub-commits (5a, 5b, 6) with the reframed 5b scope. Estimated dispatch size ~40 KB matching SC-3C PLAN precedent.
3. **Coordinate Prometheus** for any parallel doc work (SC-4 closure prep pre-stage, etc.) — same pattern as SC-3C dispatch interleave.

PL action items: none. PM proceeds to PLAN-stage authoring at next session bandwidth.

## 11. Cross-references

**Consumed artifact:**
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` — 52.1 KB Cerebro deep-research artifact.

**Triggering dispatch:**
- `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` — PM dispatch §3 six questions.

**Updated by this consumption:**
- `Plans/SC-4-THINK-2026-05-11.md` D4 disposition (PM-NOTE is authoritative until THINK v2).

**Pattern precedent:**
- `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` — SC-3C D15 Cerebro consumption (21 KB; seven citation corrections + six forward markers; D-revisions finalized pattern).

**Substrate references verified during consumption:**
- `kernel/src/sched/process.rs:343` — `cr3_for_thread(pid)` definition (Phase A retirement target).
- `kernel/src/sched/scheduler.rs:45, :300, :306` — call sites (both have `&Thread` in scope, Phase A audit confirms).
- `kernel/src/sched/thread.rs:60-80` — `Thread` struct (already carries `pub(super) pid: ProcessId`; gains `pmap: Option<Arc<Pmap>>`).

**Cerebro voice profile context:**
- Two prior Cerebro dispatches consumed cleanly (`Plans/PM-NOTE-2026-05-11-ai-native-os-field-survey-consumption.md`, `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`). D5 continues the established pattern with the bonus contribution of identifying a PM-framing error.

---

## 12. Closure

D4 / D5 finalized. PROCESSES retrofit ELIMINATED from sub-commit 5b. Phase A retirement of `cr3_for_thread(pid)` is the architectural change. Phase B (`arc-swap` AtomicPtr) deferred to "only if Phase A reveals a blocker"; not triggered. `spin::RwLock` recorded as pathological at our scale and added to Phase 2+ forward markers. D1 and D7 cross-validated by Cerebro. Sub-commit 5b scope substantially reduced (~200-300 lines vs original ~500-800).

Calibration: Cerebro D5 is the highest-value Cerebro contribution to NeoPlutarch to date. PM-framing-error catch is the kind of substrate awareness that justifies the deep-research role.

Next operational step: PM authors SC-4 PLAN-stage dispatch for Jarvis covering revised 5a/5b/6 scope.

---

**End of PM-NOTE.**

*SC-4 D5 Cerebro consumed; D4 finalized to Phase A approach (retire cr3_for_thread; Thread carries Arc<Pmap>); sub-commit 5b substantially reduced; four PM THINK corrections folded; D1+D7 cross-validated; four new Phase 2+ forward markers captured. PLAN-stage dispatch next.*
