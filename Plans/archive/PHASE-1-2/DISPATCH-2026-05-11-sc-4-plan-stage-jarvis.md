# DISPATCH — SC-4 PLAN-stage: Phase 2 BUILD-3 sub-commits 5a + 5b + 6 (Jarvis)

**Date filed:** 2026-05-11
**From:** PM
**To:** Jarvis (Codex-backed kernel agent per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1)
**Ferry:** PL ferries to Jarvis session.
**Status:** SC-4 PLAN-stage. Three sub-commits per D15 boundary: **5a** handles + derivations SwitchMutex retrofit, **5b** Phase A retirement of `cr3_for_thread` + exiting flag + exit_state CAS, **6** blocked-thread-kill walk + close_peer my-side fix + zombify_thread primitive. Phase 2 BUILD-3 sub-commit numbering continues from SC-3C (4a/4b/4c closed at `88c7bc2` / `3272840` / `ead8b19`).

---

## 1. Mandate summary

Implement three sub-commits closing Q8 (capability table SMP composition + blocked-thread-kill walk). Land sequentially on `origin/main` via clean fast-forward. Each sub-commit includes its own verification report; SC-4 closure docs land via a separate Prometheus integration dispatch after sub-commit 6 closes.

D-decisions are PM-tier finalized per `Plans/SC-4-THINK-2026-05-11.md` and `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`. **D5 returned Cerebro deep-research artifact `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (52.1 KB; primary-source citations from Linux/seL4/illumos/FreeBSD/Redox).** Citation-precision corrections from that artifact apply throughout this dispatch and must be reflected in code comments.

## 2. Pre-implementation context

### 2.1 Inputs to consume

**Required reading before sub-commit 5a:**
- `Plans/SC-4-THINK-2026-05-11.md` — full THINK artifact (39.3 KB). Sections §3 (R1-R11 race enumeration), §4 (Q8.a design space), §5 (Q8.b design space), §6 (Q8.c design space), §7 (D-table).
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md` — D4 finalized to Phase A; four THINK corrections folded; sub-commit 5b reframed.
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` — Cerebro citations. Sections §3.3 (Linux `pidmap_lock` vs `tasklist_lock`), §5.4 (`spin::RwLock` 5× regression), §6 (illumos doors model), §10 (Phase B `arc-swap` deferred path), §11 (architectural reframing).

**Substrate inventory:**
- `kernel/src/sched/process.rs` — capability infrastructure; 714 lines. Hot-path operations table in THINK §2.2.
- `kernel/src/sched/scheduler.rs` lines 45, 300, 306 — `cr3_for_thread(next.pid)` call sites for Phase A retirement.
- `kernel/src/sched/thread.rs` — Thread struct (60-80 lines header for context). Currently carries `pub(super) pid: ProcessId`; sub-commit 5b adds `pub(super) pmap: Option<Arc<Pmap>>`.
- `kernel/src/ipc/mod.rs` — Channel + EndpointState + close_peer; 504 lines. close_peer my-side drop bug at the `guard[my_side.idx()] = EndpointState::Closed;` line (sub-commit 6 fix).

**Ratified ADRs anchoring SC-4:**
- ADR-004 (anonymous-handle IPC + capability data model)
- ADR-007 (process termination discipline; lifts the explicit Phase 1.5b Blocked-in-IPC limitation that sub-commit 6 addresses)
- ADR-018 / ADR-019 (Hpw + IrqSafeMutex)
- ADR-020 (SwitchMutex LockOwner)

### 2.2 Critical Cerebro corrections to apply

Four PM THINK corrections per Cerebro D5 returns. Apply in code comments and verification reports wherever the relevant topic is touched:

| Topic | Correct citation |
|---|---|
| Linux PID-lookup synchronization | `pidmap_lock` (`kernel/pid.c:89`, `DEFINE_SPINLOCK`, `__cacheline_aligned_in_smp`) protects IDR mutations. **NOT** `tasklist_lock` — that's task-list / parent-child / signal topology serialization. |
| `spin::RwLock` upgrade path | **Documented-pathological at 4-64 CPU range.** 5× regression vs Mutex for short critical sections (parking_lot benchmarks per "Read Locks Are Not Your Friends"). The `spin` crate's own docs warn of writer starvation. Do NOT introduce. |
| `AtomicPtr<Arc<BTreeMap>>` design pattern | This is `arc-swap` crate (vorner; used in TiKV, signal-hook) — an **application-level** pattern, not OS-kernel prior art. **No production OS kernel uses arc-swap for process tables.** Linux RCU is different (RCU-protected linked-list with grace-period reclamation, not whole-structure CoW). |
| PROCESSES hot-path contention | **Architecturally avoidable**, not a sync-primitive choice. Linux `loaded_mm`, seL4 `current_thread()`, NeoPlutarch's already-deployed `active_pmap_cr3` — all publish running-entity CR3 data per-CPU at context-switch time. PM's D4 framing entered the wrong design subtree until Cerebro reframed. |

## 3. Sub-commit 5a — handles + derivations SwitchMutex retrofit (D1, D2, D3, D6)

### 3.1 Scope

Convert `Process.handles` and `Process.derivations` from `spin::Mutex` to `SwitchMutex` matching the SC-3B IPC channel retrofit discipline. Establish revoke-walk lock-ordering as a load-bearing invariant.

### 3.2 Files modified

- `kernel/src/sched/process.rs`:
  - `Process` struct: change `handles: Mutex<HandleTable>` → `handles: SwitchMutex<HandleTable>` and `derivations: Mutex<BTreeMap<CapId, Option<CapId>>>` → `derivations: SwitchMutex<BTreeMap<CapId, Option<CapId>>>`. Update both constructors (`new_kernel`, `new_user`) accordingly.
  - All call sites — `install_cap`, `derive_handle`, `revoke_handle`, `complete_grant`, `on_thread_reap`, `terminate`, `spawn_user`, `name_of`, `any_user_process`, plus any debug helpers. Existing `without_interrupts(|| ...)` wrapping stays; SwitchMutex acquisition composes inside.
- `kernel/src/ipc/mod.rs`: cross-references to `proc.handles.lock()` and `proc.derivations.lock()` already exist via `process::complete_grant`. SwitchMutex API surface is compatible; verify no direct `.lock()` calls on these fields exist outside process.rs (likely none).

`PROCESSES` and `COMPLETED_EXIT_CODES` remain `spin::Mutex<BTreeMap<...>>` in this sub-commit. They are addressed (or eliminated from hot-path) in sub-commit 5b.

### 3.3 Lock-ordering invariant (D6)

Document at the top of `revoke_handle`:

```rust
// Lock acquisition order under SMP per SC-4 D6:
//   1. PROCESSES (outermost; snapshot only — released before per-process locks)
//   2. Per-process locks acquired in ascending ProcessId.0 order
//   3. Within a single process: handles before derivations
//   4. Channel state (innermost; matches SC-3B SwitchMutex ordering)
//
// `processes_snapshot()` returns Vec<Arc<Process>> sorted by pid; iteration order
// is monotonically ascending which preserves the ordering when per-process locks
// are acquired during the walk. Concurrent revokes on different CPUs each follow
// the same ordering rule, so AB-BA deadlock is impossible.
//
// Citation: FreeBSD `kern/kern_proc.c` allproc-walk + per-proc lock pattern uses
// the same monotonic-pid ordering rule (see Plans/RESEARCH-2026-05-11-sc-4-d5-
// process-table-smp-cerebro.md §5.2).
```

The `processes_snapshot()` helper at `process.rs` line ~313 already returns the values cloned out under the PROCESSES lock. Verify the BTreeMap iteration order is by ascending key (BTreeMap guarantees this by Ord) — if `ProcessId` derives `Ord` as `pub struct ProcessId(pub u32)` with derived Ord then yes. The invariant doc above can cite the BTreeMap Ord guarantee.

### 3.4 Lock-ordering for `complete_grant`

`complete_grant` touches two processes (grantor + recipient). The strict-by-pid ordering rule means:

```rust
pub fn complete_grant(
    grantor_pid: ProcessId,
    source_cap_id: CapId,
    offered: CapRights,
    recipient_pid: ProcessId,
    max_rights: CapRights,
) -> Result<u32, GrantInstallError> {
    without_interrupts(|| {
        // Take grantor lock first if grantor_pid < recipient_pid; otherwise
        // take recipient lock first. Both paths acquire in ascending-pid order
        // to avoid AB-BA deadlock with another concurrent grant in the reverse
        // direction. (SC-4 D6 invariant.)
        // ...
    })
}
```

The existing implementation acquires grantor.handles lock briefly to read the source handle, releases, then calls `install_cap(&recipient, ...)` which takes recipient.handles + recipient.derivations. Sequencing is already release-before-reacquire on different processes, so the ordering rule applies trivially. Add comment documenting the discipline.

### 3.5 Verification gates (5a)

| Gate | Expected |
|---|---|
| `cargo k` | PASS |
| `cargo k --no-default-features` | PASS |
| `cargo neo-test all` | PASS |
| 200/200 `NEO_SMP=4 smp_cross_cpu_rendezvous_stress` | PASS (SC-3B/SC-3C baseline preserved) |
| New scenario `q8a_concurrent_install_revoke` (D16, see §6.1) | PASS |
| `q8a_concurrent_install_revoke` under `neo-test-missing-cap-lock-negative` feature | FAIL (the negative feature suppresses the SwitchMutex acquisition; revocation set must be detectable as incomplete or Vec corruption must be detectable) |
| `cargo fmt --check` | BLOCKED (rustfmt not installed; surface in verification report per SC-3C closure forward marker) |

### 3.6 Commit message (5a)

```
kernel/sched: retrofit Process.handles + derivations for SMP (Q8.a)

Per SC-4 D1/D2/D3/D6:
- Process.handles: spin::Mutex<HandleTable> → SwitchMutex<HandleTable>.
- Process.derivations: spin::Mutex<BTreeMap<CapId, Option<CapId>>> →
  SwitchMutex<BTreeMap<CapId, Option<CapId>>>.
- HandleTable internals unchanged (Vec<Option<Handle>>; per D2 S1).
- Revoke-walk lock-ordering documented as strict-by-ascending-ProcessId
  per D6; complete_grant ordering documented to match.

Validated by Cerebro D5 research §5.1: seL4 BKL evaluation (Peters &
Heiser 2014; arXiv 1609.08372) shows coarse per-entity locks scale to
~14 cores within socket for short critical sections, well above our
4-64 CPU target. SwitchMutex per-process granularity is correct at
NeoPlutarch's scale.

PROCESSES and COMPLETED_EXIT_CODES remain spin::Mutex in this sub-commit;
addressed (or eliminated from hot path) in sub-commit 5b.

New negative-feature gate `q8a_concurrent_install_revoke` with
`neo-test-missing-cap-lock-negative` verifies the test harness detects
the failure mode (parallel to SC-3C 4c D14 missing-shootdown-detection).

See:
- Plans/SC-4-THINK-2026-05-11.md §4, §7 D1-D3, D6
- Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md §5.1
- Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md §5.1
```

### 3.7 Stop conditions (5a)

- 200/200 NEO_SMP=4 stress regresses → STOP, surface results.
- `q8a_concurrent_install_revoke` PASSES under both presence + absence of `neo-test-missing-cap-lock-negative` → the gate isn't actually detecting the failure mode. STOP, refine the scenario, surface.
- Any code path acquiring `SwitchMutex` without an enclosing `without_interrupts` → STOP, audit the path. The existing discipline (every cap-op wraps in `without_interrupts`) must be preserved; SwitchMutex does not provide IRQ-off by itself.
- Channel state mutex ordering with the new per-process locks introduces an apparent cycle → STOP. Channel state is innermost per the §3.3 ordering rule; if a code path acquires it outermost, that's a real ordering violation needing redesign.

## 4. Sub-commit 5b — Phase A retirement + exiting flag + exit_state CAS (D4, D7, D8, D18)

### 4.1 Scope

The D4 Phase A architectural change: `Thread` carries its own `pmap: Option<Arc<Pmap>>`, the scheduler accesses it directly during context switch, and `cr3_for_thread(pid)` is retired entirely. Removes PROCESSES from the cross-process context-switch hot path. Linux `loaded_mm` parallel; SC-3C `active_pmap_cr3` per-CPU cache applied at the Thread layer.

Plus: `Process.exiting: AtomicBool` for grant-vs-target-exit liveness check (illumos doors `DOOR_INVALID` parallel per Cerebro §6); `Process.exit_state: AtomicU8` for natural-exit-vs-forced-kill mutual exclusion.

### 4.2 Files modified

- `kernel/src/sched/thread.rs`:
  - Add `pub(super) pmap: Option<Arc<Pmap>>` field to `Thread` struct (visibility matches existing `pub(super) pid`).
  - Update `Thread::new_user(name, pid, entry, initial_user_rsp)` signature to take `pmap: Arc<Pmap>` and store `Some(pmap)`.
  - `Thread::new_kernel` (or equivalent kernel-thread constructor) stores `pmap: None`.
  - Verify no other call sites of `Thread::new_user` exist outside `process::spawn_user`.
- `kernel/src/sched/process.rs`:
  - `Process::new_user(name, pmap)` already wraps pmap in `Arc`; `spawn_user` already holds the Arc when constructing the Thread. Plumb the Arc clone through: `Thread::new_user(name, pid, entry, initial_user_rsp, Arc::clone(&process.pmap.as_ref().unwrap()))`. (Or restructure so spawn_user reads back the Arc from the Process after `register(process)`; see §4.3 implementation note.)
  - Add `Process.exiting: AtomicBool` field, initialized `false` in both constructors.
  - Add `Process.exit_state: AtomicU8` field, initialized to `EXIT_STATE_RUNNING` (0) in both constructors. Define constants: `EXIT_STATE_RUNNING = 0`, `EXIT_STATE_EXITING_NATURAL = 1`, `EXIT_STATE_EXITING_FORCED = 2`, `EXIT_STATE_EXITED = 3`.
  - `install_cap` checks `proc.exiting.load(Ordering::Acquire)` before installing; returns `GrantInstallError::RecipientGone` (or appropriate error for non-grant callers — derive_handle's error path is `EINVAL`-shaped per existing code) if target is exiting.
  - `on_thread_reap` CAS-claims `exit_state` from RUNNING → EXITING_NATURAL at the top; sets `exiting = true` immediately after winning the CAS; bail with no-op log if CAS loses (forced-kill won; nothing for natural path to do).
  - `terminate` CAS-claims `exit_state` from RUNNING → EXITING_FORCED at the top; sets `exiting = true` immediately after winning the CAS; bail with `ESRCH` if CAS loses (natural exit already in progress or completed).
  - Both paths transition to EXITED at end.
  - **Retire `pub fn cr3_for_thread(pid: ProcessId) -> (PhysFrame, Cr3Flags)` entirely.** Delete the function definition; delete the export.
- `kernel/src/sched/scheduler.rs`:
  - Line 45: remove `cr3_for_thread` from the import.
  - Lines 300 and 306: replace `let (frame, flags) = cr3_for_thread(next.pid);` with direct access:
    ```rust
    let (frame, flags) = next.pmap
        .as_ref()
        .map(|pmap| pmap.cr3())
        .unwrap_or_else(process::kernel_cr3);
    ```
    Preserve the kernel-PID fallback to `kernel_cr3()`. `process::kernel_cr3` is `pub` and stays.

### 4.3 Implementation note: Arc plumbing through spawn_user

The current `spawn_user` constructs `process` (which stores `Arc<Pmap>`), then calls `register(process)` (consumes Process by value, returns `Arc<Process>`), then constructs Thread. Two clean options:

**Option A — clone the pmap Arc before register:**
```rust
let pmap_for_thread = process.pmap.as_ref().map(Arc::clone);
let _arc = register(process);  // consumes process
let thread = Thread::new_user(name, pid, entry, initial_user_rsp, pmap_for_thread);
```

**Option B — read back from registered Arc:**
```rust
let arc = register(process);
let pmap_for_thread = arc.pmap.as_ref().map(Arc::clone);
let thread = Thread::new_user(name, pid, entry, initial_user_rsp, pmap_for_thread);
```

PM operational call: **Option A**. Cleaner ordering; the Arc clone happens once at thread creation; no extra `arc` binding lingering.

### 4.4 D18 — Phase A coverage gate

Add new test scenario `q8d_phase_a_coverage` (or similar; Jarvis chooses the canonical name matching the existing test-harness naming conventions) gated by feature `neo-test-d5-phase-a-coverage`:

```rust
#[cfg(feature = "neo-test-d5-phase-a-coverage")]
pub fn q8d_phase_a_coverage_scenario() {
    // Instrument: if `cr3_for_thread` is invoked during context switch,
    // the scenario panics with "Phase A regression: cr3_for_thread reached".
    // The scenario spawns a user process, forces context switches between it
    // and the kernel, and verifies CR3 reloads happen via thread.pmap path.
}
```

Since `cr3_for_thread` is being deleted, the literal instrumentation pattern is "verify the function doesn't exist at compile time". The scenario instead verifies positive coverage: that `thread.pmap.cr3()` is exercised on real cross-process context switches and produces correct CR3 frame/flags. PASS confirms Phase A retirement is functional.

### 4.5 Verification gates (5b)

| Gate | Expected |
|---|---|
| `cargo k` | PASS (verify `cr3_for_thread` references removed cleanly) |
| `cargo k --no-default-features` | PASS |
| `cargo neo-test all` | PASS |
| 200/200 `NEO_SMP=4 smp_cross_cpu_rendezvous_stress` | PASS |
| New scenario `q8d_phase_a_coverage` (D18) | PASS — verifies thread.pmap path exercised |
| `q7b_cr3_cache_skip_count` (SC-3C 4a regression check) | PASS — per-CPU CR3 cache continues to short-circuit same-process resumes |
| `q7c_process_exit_during_thread_run` (SC-3C 4c regression check) | PASS — exit teardown shootdown still works; `q8c_deferred=true` marker still present (sub-commit 6 removes it) |
| Grant rendezvous with target mid-exit (new scenario `q8b_grant_vs_exit_race`, see §6.2) | PASS — grant observes RecipientGone via exiting flag |
| Natural-exit + forced-kill race (new scenario `q8b_exit_state_cas`, see §6.3) | PASS — CAS winner proceeds; CAS loser observes EXITED and bails cleanly |
| `cargo fmt --check` | BLOCKED |

### 4.6 Commit message (5b)

```
kernel/sched: retire cr3_for_thread; Thread carries Arc<Pmap> (Q8.b Phase A)

Per SC-4 D4 finalized in Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-
consumption-and-d4-finalized.md §3:

The Phase A architectural change eliminates PROCESSES from the cross-
process context-switch hot path entirely. Thread now carries its own
Option<Arc<Pmap>>, plumbed at thread creation from Process::new_user
through spawn_user. Scheduler context-switch path at scheduler.rs:300
and :306 accesses next.pmap directly instead of cr3_for_thread(next.pid).
cr3_for_thread is deleted.

Pattern parallel: Linux cpu_tlbstate.loaded_mm (per-CPU GS-relative
pointer to active mm_struct), seL4 current_thread() per-CPU pointer.
NeoPlutarch's already-deployed per_cpu.active_pmap_cr3 (SC-3C 4a) is
conceptual kin. Cerebro D5 §11 reframed PM's original D4 lock-type-
choice question to this architectural alternative.

Also per SC-4 D7 (illumos doors `DOOR_INVALID` parallel per Cerebro §6):
- Process.exiting: AtomicBool field added; checked at install_cap site
  before installing capabilities into a process mid-tear-down.

And per SC-4 D8:
- Process.exit_state: AtomicU8 CAS-claim discipline at on_thread_reap
  and terminate top. CAS-winner proceeds; CAS-loser observes EXITED and
  bails. Closes the natural-exit-vs-forced-kill race on the same pid.

PROCESSES remains spin::Mutex<BTreeMap> for the cold-path cross-process
operations (revoke, complete_grant, terminate, on_thread_reap, name_of,
any_user_process). Cerebro §5.2 confirms this is the production pattern
at NeoPlutarch's scale: FreeBSD `allproc` uses sx shared-lock + locked-
handoff, NOT SMR/RCU; illumos uses single `pidlock`. RCU's complexity is
engineered for thousand-CPU machines with millions of processes, two
orders of magnitude beyond our target.

Forward marker (Phase 2+): `arc-swap`-style AtomicPtr<Arc<BTreeMap>> for
CR3-data-only registry is the documented Phase B fallback per Cerebro
§10. Not triggered by this sub-commit's PM audit (all cr3_for_thread
callers are scheduler-context-switch with &Thread in scope, verified
2026-05-11). Reconsider only if a future syscall handler needs
CR3-by-PID without &Thread access.

Forward marker (Phase 2+): spin::RwLock is documented-pathological at
4-64 CPU range per Cerebro §5.4 (5× regression vs Mutex for short
critical sections; spin crate's own docs warn of writer starvation).
Recorded here so future contributors do not reach for it as a default
upgrade. ADR may follow if normative discipline needed.

New gate `q8d_phase_a_coverage` (D18) verifies thread.pmap path is
exercised on real cross-process context switches.

See:
- Plans/SC-4-THINK-2026-05-11.md §5, §7 D4-D8, D18
- Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md
- Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md §1, §6, §10, §11
```

### 4.7 Stop conditions (5b)

- `cr3_for_thread` deletion surfaces a caller PM's audit missed → STOP, surface. Phase B (`arc-swap`) is the contingent fallback per Cerebro §10; do NOT implement Phase B without PM consultation — it's deferred unless concretely needed.
- Thread struct size grows by more than `sizeof::<Option<Arc<Pmap>>>()` (one word for discriminant + one word for Arc pointer = 16 bytes on x86_64) → STOP. Unexpected growth indicates struct-packing or alignment surprise; surface.
- `q7b_cr3_cache_skip_count` regression on the SC-3C per-CPU cache → STOP. The cache should still short-circuit same-process resumes; if the cache effectiveness drops, Phase A's `thread.pmap.cr3()` is being called when it shouldn't, indicating a scheduler-path bug.
- `exit_state` CAS introduces a regression on `q7c_process_exit_during_thread_run` → STOP. The SC-3C scenario's `exit_reaped=true teardown_shootdown=true` should still PASS; the `q8c_deferred=true` marker stays (sub-commit 6 removes it).
- Grant rendezvous integration with exiting flag breaks existing `q4_grant_rendezvous_smp` or similar SC-3B scenario → STOP.

## 5. Sub-commit 6 — Blocked-thread-kill walk + close_peer my-side fix + zombify_thread (D9, D10, D12, D17)

### 5.1 Scope

Lift the Phase 1.5b limitation in `terminate` ("only Ready-state threads can be killed") and fix the latent `close_peer` my-side `Box<Thread>` drop bug. Introduce `sched::zombify_thread(Box<Thread>)` primitive for forced-kill of parked threads.

Removes the `q8c_deferred=true` marker from `q7c_process_exit_during_thread_run` (sub-commit 4c's SC-3C scenario) and replaces it with actual blocked-thread-kill walk implementation.

### 5.2 Files modified

- `kernel/src/sched/scheduler.rs` or `kernel/src/sched/mod.rs`:
  - Add `pub fn zombify_thread(thread: Box<Thread>)`. Pushes the thread directly onto the zombie queue without invoking the wake path. The thread's state is set to `ThreadState::Zombie`. The reaper drops the Box<Thread> on its next pass.
  - Implementation parallel to existing `wake_thread` / `wake_thread_on_current_cpu` but bypassing the ready queue and the syscall-return-value mechanism.

- `kernel/src/ipc/mod.rs`:
  - Fix `close_peer` per D10 (M1 two-stage close):
    ```rust
    pub fn close_peer(&self, my_side: EndpointSide) {
        crate::arch::x86_64::without_interrupts(|| {
            let peer_idx = my_side.other().idx();
            let my_idx = my_side.idx();
            let mut guard = self.state.lock();
            let extracted_peer = core::mem::replace(&mut guard[peer_idx], EndpointState::Closed);
            let extracted_my = core::mem::replace(&mut guard[my_idx], EndpointState::Closed);
            drop(guard);
            // Peer side: wake with EPIPE if blocked (existing semantics).
            match extracted_peer {
                EndpointState::SendBlocked { waiter, .. }
                | EndpointState::RecvBlocked { waiter, .. }
                | EndpointState::GrantBlocked { waiter, .. }
                | EndpointState::RecvGrantBlocked { waiter, .. } => {
                    sched::wake_thread(waiter, EPIPE);
                }
                EndpointState::Idle | EndpointState::Closed => {}
            }
            // My side: if my own process is being torn down and a thread is
            // parked here, zombify the thread (do not wake; it never runs again).
            // SC-4 D10/D12. The previous implementation silently dropped this
            // Box<Thread> — latent thread-leak bug surfaced under Phase 2+
            // multi-threaded user processes or under forced-kill of a process
            // with a thread blocked on its own channel side.
            match extracted_my {
                EndpointState::SendBlocked { waiter, .. }
                | EndpointState::RecvBlocked { waiter, .. }
                | EndpointState::GrantBlocked { waiter, .. }
                | EndpointState::RecvGrantBlocked { waiter, .. } => {
                    sched::zombify_thread(waiter);
                }
                EndpointState::Idle | EndpointState::Closed => {}
            }
        });
    }
    ```

- `kernel/src/sched/process.rs`:
  - Update `terminate(target_pid)` to handle Blocked-in-IPC targets per D9 (F1 handle-table walk):
    ```rust
    pub fn terminate(target_pid: ProcessId) -> i64 {
        // ... existing kernel-PID check ...
        // ... existing CAS exit_state claim (added in sub-commit 5b) ...

        // Phase 1.5b limitation lifted per SC-4 D9: if the target's thread
        // is Blocked-in-IPC, walk the handle table to find the parked
        // Box<Thread> and zombify directly. The `kill_thread_by_pid` path
        // remains for Ready-state targets; this is the Blocked fallback.
        let kill_result = scheduler::kill_thread_by_pid(target_pid);
        // kill_result is now true (Ready-state target found and yanked) OR
        // false (target may be Blocked; walk the handle table).

        // ... existing PROCESSES.remove() ...
        let proc_arc = match ... ;

        let drained = without_interrupts(|| proc_arc.handles.lock().drain());
        if !kill_result {
            // Target was not in Ready state. Walk drained handles for a
            // Channel handle whose MY-side endpoint state contains a
            // parked Box<Thread> belonging to target_pid. The first one
            // found IS the blocked thread (Phase 1.5b: single thread per
            // process; multi-thread is Phase 2+ forward marker per D13).
            for handle in &drained {
                if let Handle::Channel { channel, side, .. } = handle {
                    if let Some(parked) = channel.extract_my_side_if_blocked(*side) {
                        scheduler::zombify_thread(parked);
                        break;  // single-thread-per-process invariant
                    }
                }
            }
        }
        // close_channel_peers now extracts both sides per close_peer fix;
        // if MY-side already extracted above (by the kill walk), close_peer
        // observes Idle/Closed and no-ops on MY-side.
        close_channel_peers(&drained);
        // ... rest of existing terminate flow ...
    }
    ```

- Add `Channel::extract_my_side_if_blocked(my_side: EndpointSide) -> Option<Box<Thread>>` method to `kernel/src/ipc/mod.rs`:
  ```rust
  pub fn extract_my_side_if_blocked(&self, my_side: EndpointSide) -> Option<Box<Thread>> {
      crate::arch::x86_64::without_interrupts(|| {
          let my_idx = my_side.idx();
          let mut guard = self.state.lock();
          let extracted = core::mem::take(&mut guard[my_idx]);
          match extracted {
              EndpointState::SendBlocked { waiter, .. }
              | EndpointState::RecvBlocked { waiter, .. }
              | EndpointState::GrantBlocked { waiter, .. }
              | EndpointState::RecvGrantBlocked { waiter, .. } => {
                  // Leave my-side as Idle (mem::take set it). Caller (terminate)
                  // will subsequently call close_peer which will set both sides
                  // to Closed atomically.
                  Some(waiter)
              }
              other => {
                  guard[my_idx] = other;  // restore
                  None
              }
          }
      })
  }
  ```

- `kernel/src/test_harness.rs` (the 137 KB file):
  - Locate `q7c_process_exit_during_thread_run` scenario (sub-commit 4c added the `q8c_deferred=true` marker). Remove the `q8c_deferred=true` from the mechanism string; the scenario now exercises the actual implementation.
  - Add new scenario `q8c_blocked_kill_walk` per D17 (see §6.4).

### 5.3 Verification gates (6)

| Gate | Expected |
|---|---|
| `cargo k` | PASS |
| `cargo k --no-default-features` | PASS |
| `cargo neo-test all` | PASS |
| 200/200 `NEO_SMP=4 smp_cross_cpu_rendezvous_stress` | PASS |
| `q7c_process_exit_during_thread_run` | PASS — mechanism marker `q8c_deferred=true` now absent; replaced with `q8c_blocked_kill_walk_active=true` or equivalent |
| New scenario `q8c_blocked_kill_walk` (D17) | PASS |
| New scenario for `close_peer` my-side fix (`q8c_close_peer_my_side_extract`, see §6.5) | PASS — verifies the previously-leaked Box<Thread> is zombified |
| Frame audit after blocked-thread-kill | CLEAN (no leaked frames from Box<Thread> drop) |
| `cargo fmt --check` | BLOCKED |

### 5.4 Commit message (6)

```
kernel/sched + ipc: blocked-thread-kill walk + close_peer my-side fix (Q8.c)

Lifts the Phase 1.5b limitation in terminate ("only Ready-state threads
can be killed; Blocked-in-IPC returns EINVAL" — ADR-007 §Q3 deferred to
Phase 2 work) and fixes the latent close_peer my-side Box<Thread> drop
bug.

Per SC-4 D9 (F1 handle-table walk):
- terminate now handles Blocked-in-IPC targets. When kill_thread_by_pid
  returns false (target not Ready), terminate walks the drained handle
  table; for each Channel handle, calls channel.extract_my_side_if_blocked
  to find a parked Box<Thread>; zombifies the first one found (Phase 1.5b
  single-thread-per-process invariant; multi-thread is Phase 2+ forward
  marker per SC-4 D13).
- Channel::extract_my_side_if_blocked(my_side) added: atomically extracts
  a parked Box<Thread> from the endpoint state, leaving the state Idle.

Per SC-4 D10 (M1 two-stage close_peer):
- close_peer now extracts BOTH peer-side AND my-side endpoint states
  before mem::replace to Closed. Peer-side blocked thread wakes with
  EPIPE (existing semantics preserved). My-side blocked thread, if
  present, is zombified via the new sched::zombify_thread primitive
  rather than being silently dropped.
- The previous implementation's `guard[my_side.idx()] = EndpointState::
  Closed;` overwrote my-side without capturing what it replaced.  Under
  Phase 1.5b single-threaded user processes this never fired in practice;
  under Phase 2+ multi-threaded user processes or under forced-kill of a
  process with a thread blocked on its own channel side, the
  Box<Thread> would be leaked.

Per SC-4 D12 (W2 zombify-don't-wake):
- New primitive sched::zombify_thread(Box<Thread>) parallel to
  wake_thread / wake_thread_on_current_cpu but bypassing the ready queue
  and the syscall-return-value mechanism. The thread is pushed directly
  onto the zombie queue; the reaper drops the Box on its next pass.
  Distinct from wake_thread which implies "will run again with this
  return value"; zombify implies "will not run; free its resources".

The SC-3C 4c scenario q7c_process_exit_during_thread_run had a mechanism
marker `q8c_deferred=true` flagging that natural-exit with active
shootdown was correct but blocked-thread-kill was deferred. That marker
is now removed; the scenario continues to PASS with the active
implementation.

New gates per D17:
- q8c_blocked_kill_walk: spawn child, block in IPC, parent sys_kills,
  verify zombification + frame-leak-clean + channel state Closed both
  sides.
- q8c_close_peer_my_side_extract: forced-kill of process with thread
  parked on own channel side, verify the previously-leaked Box is
  zombified.

See:
- Plans/SC-4-THINK-2026-05-11.md §6, §7 D9-D12, D17
- Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md
  (q8c_deferred marker context)
- ADR-007 §Q3 (Phase 1.5b limitation lifted)
```

### 5.5 Stop conditions (6)

- `q7c_process_exit_during_thread_run` regresses post-implementation → STOP. The SC-3C scenario's exit-teardown-shootdown path must remain functional; the q8c implementation extends rather than replaces.
- Frame audit shows leaks after `q8c_blocked_kill_walk` → STOP. The zombified Box<Thread> must be fully reclaimed via the reaper's existing Drop path; no special handling required, but verify.
- Multi-thread case surfaces unexpectedly (target has more than one thread blocked) → STOP, surface. Phase 1.5b invariant is single-thread-per-process; if the kernel state has drifted, the assumption breakdown is a bug.
- `close_peer` two-stage extract introduces a subtle ordering issue with the rendezvous Drop chain (load-bearing comment on `on_thread_reap` cites historical deadlock in this area) → STOP, surface for PM consultation.
- `zombify_thread` interacts with the per-CPU zombie queue in a way that the existing reaper can't drain → STOP. The reaper should handle zombify-pushed threads transparently because they have the standard ThreadState::Zombie marker.

## 6. Verification scenarios — new gate specifications

### 6.1 `q8a_concurrent_install_revoke` (D16, sub-commit 5a)

Scenario: two threads on two CPUs operate on the same user process. Thread A repeatedly calls `derive_handle` to create new caps. Thread B repeatedly calls `revoke_handle` on a root cap that's the parent of all of A's derivations. Without proper locking, B's revocation set is incomplete or A's Vec corruption is detectable.

Pass condition: under presence of SwitchMutex, the revocation set is complete (no orphaned children) and Vec integrity is preserved across all iterations.

Negative-feature `neo-test-missing-cap-lock-negative`: suppresses the SwitchMutex acquisition on `Process.handles`. Under this feature, the scenario must DETECT the failure mode (incomplete revocation OR Vec corruption) and report. Test PASSES when the detection fires; this confirms the gate works against deliberately-broken implementations.

Event labels (parallel to SC-3C 4c style):
- Mechanism: `q8a_install_revoke_concurrent`
- Detection: `cap_revocation_incomplete` or `handle_table_vec_corruption`
- Negative feature: `neo-test-missing-cap-lock-negative`

### 6.2 `q8b_grant_vs_exit_race` (sub-commit 5b verification)

Scenario: process P holds a grant cap for process Q. Thread A in P initiates `complete_grant` to Q. Concurrently, thread B (e.g., parent's reaper) initiates `terminate(Q)`. Without the `exiting` flag, A might install the cap on a Process Q that's mid-tear-down. With the flag, A observes `RecipientGone`.

Pass: grant returns `RecipientGone` when target is `exiting=true`; cap is not installed.

### 6.3 `q8b_exit_state_cas` (sub-commit 5b verification)

Scenario: process P's last thread calls `sys_exit`, triggering `on_thread_reap` on CPU A. Concurrently, parent calls `sys_kill(P)` on CPU B. Both reach `exit_state` CAS. CAS-winner proceeds; CAS-loser observes EXITED state and bails. No double-cleanup, no missed cleanup.

Pass: exactly one path completes cleanup; the other observes EXITED and returns cleanly (`ESRCH` for the forced-kill loser, no-op for the natural-exit loser).

### 6.4 `q8c_blocked_kill_walk` (D17, sub-commit 6)

Scenario: parent spawns worker child with a channel handle inherited. Child blocks on `sys_recv` (RecvBlocked state on its endpoint side). Parent calls `sys_kill(worker)`. The blocked-thread-kill walk extracts the parked Box<Thread>, zombifies it, and the reaper drops the Box. Channel state is Closed on both sides. Frame audit clean.

Pass: worker thread enters zombie queue (not ready queue, not woken); channel state observable post-kill is Closed on both sides; frame audit post-reap is clean.

Event labels:
- Mechanism: `q8c_blocked_kill_walk`
- Detection: `parked_thread_zombified`, `channel_closed_both_sides`, `frame_audit_clean`

### 6.5 `q8c_close_peer_my_side_extract` (sub-commit 6 verification)

Scenario: target test for the close_peer my-side fix. Construct a state where a Channel endpoint has my-side in SendBlocked state with a parked Box<Thread>. Trigger close_peer (e.g., by closing the peer side or terminating the peer process). Verify the my-side parked Box<Thread> is zombified, not silently dropped.

Pass: zombified Box appears in zombie queue; no frame leak; channel state Closed on both sides.

This scenario specifically targets the previously-latent leak. Without the fix, the scenario fails as a thread-leak (detectable via frame audit). With the fix, it passes.

## 7. D-decisions applied (summary table)

| # | Decision | Sub-commit | Implementation |
|---|---|---|---|
| D1 | `Process.handles` lock type | 5a | SwitchMutex<HandleTable> |
| D2 | Handle table structure | 5a | Vec<Option<Handle>> unchanged |
| D3 | `Process.derivations` lock type | 5a | SwitchMutex<BTreeMap<CapId, Option<CapId>>> |
| D4 | PROCESSES retrofit | 5b (Phase A only) | Thread.pmap direct; cr3_for_thread retired |
| D5 | Cerebro D-research dispatch | — | Returned 52.1 KB artifact; consumed |
| D6 | Revoke walk lock-ordering | 5a | Strict by ascending ProcessId; documented inline |
| D7 | Grant vs target-exit race | 5b | Process.exiting: AtomicBool, checked at install_cap |
| D8 | Natural-exit vs forced-kill mutex | 5b | Process.exit_state: AtomicU8 CAS-claim |
| D9 | Blocked-thread-kill strategy | 6 | F1 handle-table walk via extract_my_side_if_blocked |
| D10 | close_peer my-side awareness | 6 | M1 two-stage close; my-side zombify |
| D11 | (rolled into D10) | — | — |
| D12 | Blocked-thread wake mechanism | 6 | W2 sched::zombify_thread(Box<Thread>) primitive |
| D13 | Multi-thread scope | (all) | Single-thread-per-process invariant; multi-thread is Phase 2+ forward marker |
| D14 | COMPLETED_EXIT_CODES growth | (none) | Phase 2+ forward marker (sys_wait consumer) |
| D15 | Sub-commit boundaries | (this dispatch) | 5a / 5b / 6 per §3-§5 |
| D16 | Q8.a verification gate | 5a | q8a_concurrent_install_revoke with neo-test-missing-cap-lock-negative |
| D17 | Q8.c verification gate | 6 | q8c_blocked_kill_walk |
| D18 | Phase A coverage gate | 5b | q8d_phase_a_coverage |

## 8. Phase 2+ forward markers to embed inline

Document these forward markers as code comments at relevant call sites. SC-3C dispatch §7 established the discipline; SC-4 continues it.

In `kernel/src/sched/process.rs`:

```rust
// SC-4 forward marker (Phase 2+): if a future syscall handler needs
// CR3-by-PID without &Thread access (theoretical case: sys_inspect-style
// RPC), the Phase B fallback is AtomicPtr<Arc<BTreeMap<ProcessId,
// Arc<Pmap>>>> using the arc-swap crate pattern. Cerebro D5 §10 documents
// this as the deferred path. Do NOT introduce until concretely needed;
// Phase A (thread.pmap direct) is sufficient for SC-4-era workloads.
```

```rust
// SC-4 forward marker (Phase 2+): spin::RwLock is documented-pathological
// at 4-64 CPU range. 5x regression vs Mutex for short critical sections
// (parking_lot benchmarks per "Read Locks Are Not Your Friends"). The
// spin crate's own docs warn of writer starvation under read pressure.
// Do NOT reach for spin::RwLock as a "safe upgrade" from spin::Mutex on
// PROCESSES or any other hot-path lock. Use SwitchMutex (per-entity) or
// the Phase A per-Thread direct-access pattern instead. Cerebro D5 §5.4.
```

In `kernel/src/sched/process.rs` near COMPLETED_EXIT_CODES:

```rust
// SC-4 forward marker (Phase 2+, D14): COMPLETED_EXIT_CODES growth is
// currently unbounded. Phase 1.5b has at most a handful of processes;
// growth is not a concern. Phase 2+ multi-process workloads with high
// process churn need a sys_wait-driven consumer. Not addressed by SC-4.
```

In `kernel/src/sched/process.rs` near the multi-thread invariant in `terminate`:

```rust
// SC-4 forward marker (Phase 2+, D13): single-thread-per-process invariant.
// terminate's blocked-thread-kill walk per D9 finds the FIRST blocked
// thread via handle-table iteration and zombifies it. Phase 2+ multi-
// threaded user processes will require iterating all threads of target_pid
// (via a scheduler-side per-pid thread enumeration that doesn't exist yet).
// The discipline composes: zombify_thread is reentrant; the walk pattern
// generalizes.
```

In `kernel/src/ipc/mod.rs` near close_peer:

```rust
// SC-4 forward marker (Phase 2+): illumos doors model (Cerebro D5 §6)
// is the architectural reference for cross-process IPC at NeoPlutarch's
// scale. Durable cross-process references via capability handles (not
// PIDs); per-target liveness flag (Process.exiting AtomicBool, added in
// SC-4 sub-commit 5b, parallel to illumos DOOR_INVALID); short locked
// critical sections at the rendezvous point. As Phase 2 BUILD-3 closes
// and Phase 3 approaches, this design language should formalize.
```

## 9. Push protocol

Three sub-commits land sequentially on `origin/main`:

1. Sub-commit 5a — branch `jarvis/sc-4-5a-handles-derivations-switchmutex-2026-05-11`. Push to `origin/main` via clean fast-forward. File verification report `Plans/SC-4-SUB-COMMIT-5A-HANDLES-DERIVATIONS-SWITCHMUTEX-VERIFICATION-<date>.md`.
2. Sub-commit 5b — branch `jarvis/sc-4-5b-phase-a-retire-cr3-for-thread-2026-05-MM`. Push. File verification report `Plans/SC-4-SUB-COMMIT-5B-PHASE-A-RETIRE-CR3-FOR-THREAD-VERIFICATION-<date>.md`.
3. Sub-commit 6 — branch `jarvis/sc-4-6-blocked-thread-kill-walk-2026-05-MM`. Push. File verification report `Plans/SC-4-SUB-COMMIT-6-BLOCKED-THREAD-KILL-WALK-VERIFICATION-<date>.md`.

Each sub-commit rebases over any parallel landings on `origin/main` (likely Prometheus parallel work, similar to SC-3C). Standard hybrid-git posture; Plans/ artifacts stay local except for the verification reports referenced in commits.

## 10. Closure protocol

After all three sub-commits land + three verification reports filed:

1. Jarvis reports back to PL with: (a) three commit SHAs, (b) three verification report filenames, (c) any incidental observations, (d) confirmation that `q8c_deferred=true` marker is removed from `q7c_process_exit_during_thread_run`.
2. PL ferries to PM.
3. PM consumes; files SC-4 close report `Plans/CLOSE-REPORT-2026-05-MM-sc-4.md` matching SC-3C precedent.
4. Separate Prometheus dispatch integrates SC-4 closure section into `docs/phases/PHASE-2.md` and entries into `Plans/INDEX.md` (parallel to SC-3C closure integration pattern).

## 11. Cross-references

**THINK + Cerebro consumption inputs:**
- `Plans/SC-4-THINK-2026-05-11.md` (39.3 KB)
- `Plans/PM-NOTE-2026-05-11-sc-4-d5-cerebro-consumption-and-d4-finalized.md`
- `Plans/RESEARCH-2026-05-11-sc-4-d5-process-table-smp-cerebro.md` (52.1 KB; Cerebro citations)

**SC-3C precedent:**
- `Plans/CLOSE-REPORT-2026-05-11-sc-3c.md` — SC-3C closure (commits attribution, D14 results, forward markers; SC-4 follows same pattern).
- `Plans/DISPATCH-2026-05-11-sc-3c-plan-stage-jarvis.md` — SC-3C PLAN dispatch (template for sub-commit structure, verification gate format, commit message style).
- `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-11.md` — `q8c_deferred=true` marker context.

**Substrate (verify before BUILD):**
- `kernel/src/sched/process.rs` lines 305-308 (PROCESSES), 343 (cr3_for_thread retire target), 165-172 (Process struct field additions), 380-end (revoke_handle / complete_grant / terminate / on_thread_reap).
- `kernel/src/sched/scheduler.rs` lines 45, 300, 306 (cr3_for_thread call sites).
- `kernel/src/sched/thread.rs` lines 60-80 (Thread struct field additions).
- `kernel/src/ipc/mod.rs` close_peer (~line 350 onwards).
- `kernel/src/test_harness.rs` `q7c_process_exit_during_thread_run` scenario (search for `q8c_deferred`).

**Discipline anchors:**
- `Plans/PM-NOTE-2026-05-11-jarvis-voice-profile.md` — Jarvis calibration culmination; future-dispatch-shape recommendations.
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — perf-gate environment-anchoring (no perf-gates this dispatch; SC-4 is correctness work).

---

## 12. Operational notes

- **rustfmt tooling gap** continues from SC-3C — `cargo fmt --check` BLOCKED. Surface in verification report consistently; no resolution this dispatch.
- **Parallel Prometheus dispatches likely** during SC-4 BUILD wall-clock window. Standard hybrid-git rebase pattern; SC-3C precedent confirms clean fast-forward works.
- **PL operational fixes possible** (parallel to e7782fc during SC-3C). If a CI environment-fragility issue surfaces with the new q8a/q8b/q8c/q8d scenarios, PL can land a fix between sub-commits per the established pattern.
- **Sub-commit 5b is the architecturally-load-bearing one.** The Phase A retirement of cr3_for_thread is the SC-4 headline contribution. Take care; the Cerebro citations are load-bearing in the commit message and code comments.

---

**End of dispatch.**

*SC-4 PLAN-stage: three sub-commits (5a handles+derivations SwitchMutex; 5b Phase A retirement of cr3_for_thread + exiting flag + exit_state CAS; 6 blocked-thread-kill walk + close_peer my-side fix + zombify_thread primitive). 18 D-decisions applied per SC-4 THINK + Cerebro D5 consumption. Four PM THINK corrections folded throughout. Five new verification scenarios (q8a_concurrent_install_revoke + neo-test-missing-cap-lock-negative; q8b_grant_vs_exit_race; q8b_exit_state_cas; q8c_blocked_kill_walk; q8c_close_peer_my_side_extract; q8d_phase_a_coverage). Single-thread-per-process invariant preserved; multi-thread is Phase 2+ forward marker.*
