<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #109: K1a-0 capability graph review

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. **Seat:** Janus.
**Reviewed:** `149d4a1990f77fd61c8eb1fa55e736188707618c` (PR #166 squash).
**Branch:** `codex/k1a0-cap-graph-review-janus`.
**Overall verdict: FAIL. Request revert-first of #166 by the PM under the dispatch's post-merge rule.**
The blocking findings are F1 and F2 below. No implementation changes or revert were made by this review.
The lock-order wording and advisory-set discrepancies are distinguished from those defects.
All source line numbers refer to the reviewed SHA, not today's `origin/main`.

## Findings

- [ ] **FAIL F1 — P1: a graph snapshot can become the final Process/Pmap owner and wait for a shootdown while holding CAP_GRAPH.**
  `kernel/src/sched/process.rs:1735` owns snapshot Arcs inside `retire`; the per-iteration Arc drops at
  `:1742`. `cap_is_live` similarly destroys its temporary snapshot at `:1779`–`:1782`, and
  `revoke_handle`'s `processes` vector (`:2039`) is destroyed before its earlier-declared graph guard
  (`:2026`). The natural reaper unregisters at `:2872` and drops its local Process Arc at `:2880`
  **outside** CAP_GRAPH. That can make any of those snapshots the last owner.

  Concrete interleaving, default build, three online shootdown-ready CPUs:
  1. CPU 0 reaps the only thread of ordinary user process P. `scheduler.rs:2716` calls
     `Thread::free` before `on_thread_reap` at `:2723`; `thread.rs:232` and `:235` clear that thread's
     Pmap and Process anchors. P has no other borrowed Process/Pmap references. CPU 0 completes P's
     graph teardown (`process.rs:2858`), releases CAP_GRAPH, and pauses before registry removal.
  2. CPU 1 closes an unrelated intermediate cap with a child. It holds CAP_GRAPH from
     `process.rs:1793`, enters `retire`, and snapshots the still-registered P at `:1735`.
  3. CPU 0 removes P from PROCESSES at `:2872` and drops its cleanup Arc at `:2880`. The empty P
     remains alive solely through CPU 1's snapshot. No live thread or active CR3 still names P.
  4. CPU 2 enters an ordinary derive/close/revoke, takes its ticket, and spins at
     `sync/ticket_mutex.rs:46` with IF=0 waiting for CPU 1's CAP_GRAPH. It remains marked
     `tlb_shootdown_ready`; these ordinary operations do not quiesce their CPUs.
  5. CPU 1 finishes P's iteration and drops the final snapshot Arc at `process.rs:1742`.
     `Process::pmap` is an `Option<Arc<Pmap>>` (`:396`); its final drop enters `Pmap::drop`, whose
     active-CR3 check passes because P's thread is already freed. `mm/pmap.rs:1082` calls the
     shootdown path while CPU 1 still owns CAP_GRAPH.
  6. `mm/pmap.rs:1150` targets `other_cpus_mask`; `mm/tlb_shootdown.rs:348` and `:352` use online
     readiness, not whether a CPU ran P. CPU 2 is included. The sender publishes/IPIs/waits at
     `tlb_shootdown.rs:236`–`:249`; CPU 2 cannot handle that IPI until CAP_GRAPH is released,
     and CPU 1 cannot release it until CPU 2 ACKs.

  The result is the configured **50 ms shootdown timeout and panic**, not a claim of an infinite
  wait (`tlb_shootdown.rs:21`, `:390`, `:405`). A contended SHOOTDOWN_LOCK can also temporarily
  enable interrupts under CAP_GRAPH (`:264`), violating the holder's IF discipline. The uncontended
  sender path above suffices. Spawn's local quiesce does not protect an ordinary close/revoke or its
  waiting peer. The snapshot existed before this PR in revoke, but the global CAP_GRAPH dependency
  and the new `retire`/liveness snapshots are new.

  Required discriminator for a subsequent fix: pause natural reap after graph teardown, let an
  unrelated graph scan pin P, finish registry/local-Arc release, put a second CPU into graph
  contention, then release the scan. Require completion, no shootdown timeout, and no Pmap drop or
  interrupt-enable window under CAP_GRAPH. This interleaving was source-traced, not run in QEMU here.

- [ ] **FAIL F2 — P1: last-tensor removal can park while holding CAP_GRAPH.**
  Returning drained Handle objects for later drop is insufficient: removal has synchronous release
  effects. `process.rs:1765` calls `HandleTable::drain`; `:1435` releases every tensor before returning.
  Close and revoke do the same at `:1374` and `:1421`. `release_tensor` at `:1289` calls
  `ai/arena.rs:402`; a last release removes the occupant and calls `drain` at `:430`.
  `arena.rs:505` invokes `reservation::guarded_clone`, which actually parks at
  `ai/reservation.rs:450` when the slot is Reserved.

  Concrete interleaving:
  1. CPU 1's migration worker claims a managed tensor with only its resting physical Arc, installs
     the Reserved marker (`reservation.rs:301`–`:311`), and copies it with the slot/reservation
     locks released. A live Tensor handle holds a slot key/logical arena count, not another physical
     Arc (`process.rs:1275`, `:1282`), so it does not prevent this claim.
  2. CPU 0 closes that last Tensor handle (or drains/revokes it), holding CAP_GRAPH and the handles
     lock. Arena release sees `free_now`, takes the occupant, and reaches the Reserved slot's
     `guarded_clone`. CPU 0 parks at `reservation.rs:450` with its outer graph/handles guards still
     on the suspended stack.
  3. Another ready thread on CPU 0 enters an unrelated cap operation and spins for CAP_GRAPH at
     IF=0. On CPU 1, migration commits and wakes the parked closer; if a previously ready cap caller
     is scheduled ahead of that closer, it too spins for CAP_GRAPH at IF=0. The owner is now Ready
     behind non-preemptible graph waiters and cannot resume to release the graph.

  Even without step 3, step 2 directly disproves the mandatory no-park premise. This release path
  already ran under a handles lock before #166; **the newly added exposure is suspending the global
  CAP_GRAPH owner and stopping unrelated processes**, not a claim that arena drain was introduced
  here. A deterministic Reserved-slot last-close/drain test must witness that no park occurs while
  CAP_GRAPH is held. The existing seven legs do not reserve a tensor during removal.

- [ ] **FAIL F3 — P2, reusable ticket primitive: the guard inherits Sync for T: Send without requiring T: Sync.**
  `sync/ticket_mutex.rs:28` makes `TicketMutex<T>` Sync for T: Send; the guard at `:30` contains only
  `&TicketMutex<T>`, so it automatically becomes Sync for the same bound. Its Deref at `:80` exposes
  `&T`. Safe scoped threads can consequently share `&TicketMutexGuard<Cell<u32>>` and concurrently
  call `Cell::set`, although Cell is not Sync. CPU A's `set(1)` and CPU B's `set(2)` access the same
  cell concurrently while the single guard holds the ticket: mutual exclusion between guards does
  not protect those shared references. A compile-only probe importing the exact new source accepted
  both `assert_sync::<TicketMutexGuard<'static, Cell<u32>>>()` and the two scoped closures. It was
  not executed. The current CAP_GRAPH payload, BTreeSet<CapId>, is Sync; this is a defect in the new
  generic safe API, separate from the current CAP_GRAPH runtime failures F1/F2.

## Dispatch checklist

1. [ ] **FAIL — lock order as literally specified; documented implementation order otherwise checks out.**
   - [ ] FAIL: CAP_GRAPH is not outermost. CPU 0 executing `Channel::grant` acquires Channel.state
     at `ipc/mod.rs:322`, then CAP_GRAPH through `complete_grant` at `:330` /
     `process.rs:2299`; recv_grant does likewise at `ipc/mod.rs:386` / `:393`. This is an explicit
     exception in the implementation/PR/ADR (`process.rs:1670`, `docs/ADR-010-capability-system-foundation.md:280`),
     not evidence by itself of a reversed-lock cycle. Correct the dispatch's wording.
   - [x] PASS: each production acquisition precedes PROCESSES/handles/derivations and scheduler
     acquisition. The PR caller rows are present at `process.rs:1793`, `:1837`, `:1890`, `:1923`,
     `:1962`, `:2026`, `:2299`, `:2648`, `:2858`, `:3718`. Exact-symbol searches found no additional
     production lock acquisitions outside those paths. Syscall installations include the extra
     root-cap constructors, file-open helper, AI constructors and GPU constructors; they use the
     same wrappers. GPU-fence installation (`syscall.rs:7673`) retains pixel-buffer guards, so
     "no lock held" in the caller table is too broad; no reverse acquisition from the reviewed
     graph paths to those pixel guards was found.
   - [x] PASS: the reaper releases the scheduler guard at `scheduler.rs:2693` before freeing the
     zombie at `:2716` and invoking process cleanup at `:2723` / `:2725`. Registry snapshotting
     releases PROCESSES before process locks (`process.rs:1625`). Wait notifications take no
     Channel.state: `wait_any.rs:260`, `:283`, `:297`; explicit peer-close occurs after graph
     release (`syscall.rs:6318`, `process.rs:2861`).

2. [ ] **FAIL — interrupts-off ticket-lock obligations, because of F1/F2 and F3.**
   - [x] PASS: the listed production entry sites acquire inside without_interrupts. The public
     `lock_cap_graph` API itself only documents the obligation (`process.rs:1697`); callers enforce
     it. IRQ/fault paths defer graph teardown to the reaper; no direct IRQ/exception acquisition
     was found among the exhaustive lock-symbol sites.
   - [ ] FAIL: no park/block/wait under the guard is false: see F1's final-Arc shootdown and F2's
     reservation park, including their concrete CPU/lock instruction points.
   - [x] PASS, conditional primitive property: tickets are FIFO, using relaxed fetch_add and an
     acquire load (`ticket_mutex.rs:45`–`:47`), with release on unlock (`:95`). A new waiter cannot
     overtake a queued revoke. The actual runtime completion bound fails when a holder encounters
     F1/F2; FIFO alone does not prove liveness or a short maximum IF-off duration.
   - [x] PASS for the current kernel's counter use: a normal IF-off CPU can have only one outstanding
     blocking acquisition. With MAX_CPUS=64 (`arch/x86_64/per_cpu.rs:15`), far fewer than 2^32 tickets
     are outstanding, and both counters wrap consistently. The hook's try_lock is used while its
     own guard already holds the lock (`process.rs:2207`), so the holder cannot advance during that
     check. Total lifetime acquisitions passing 2^32 is not itself a false grant.
   - [ ] FAIL: the generic guard's auto-Sync soundness is F3. This does not invalidate the concrete
     BTreeSet's exclusive access, but must be repaired in the newly exported primitive.
   - [x] PASS for finite graph work, not a latency measurement: revoke adds at least one previously
     unmarked cap per successful outer iteration (`process.rs:2048`–`:2053`); a fixed snapshot has
     finite edges. `cap_graph_logic.rs:19` bounds ancestor resolution by dying.len()+1. Scans and
     allocation scale with processes/caps, rather than a fixed short duration; no runtime latency
     bound was measured. The unbounded-progress cases are F1/F2.

3. [x] **PASS — revoke versus install, as a graph-serialization property.**
   `revoke_handle` keeps its guard from `process.rs:2026` through walk/removal/forget at `:2069`.
   Derive/net-dial checks and insertions are under one guard (`:1923`–`:1947`, `:1962`–`:2000`).
   Grant keeps source validation through recipient install under one guard (`:2299`–`:2332`).
   The local-parent fallback drops only handles (`:1866`), calls `cap_is_live(&g, p)` (`:1867`),
   then retakes handles and checks exiting (`:1870`–`:1872`); excluding-install matches this at
   `:1900`–`:1906`. Teardown cannot remove graph edges while another holder executes this section.
   Exiting can be published independently, but the recipient check handles that case. F1 concerns
   lifetime side effects of this otherwise serialized scan, not an observed install-after-walk escape.

4. [ ] **FAIL — HAS_CHILDREN exactness; PASS for ancestry preservation.**
   - [x] PASS: close/exit/kill move each surviving child's edge to the nearest non-dying ancestor
     under CAP_GRAPH (`process.rs:1753`–`:1757`, `:1765`–`:1771`, `:3718`–`:3722`;
     `cap_graph_logic.rs:18`, `:29`). The old edge is physically taken out before reparenting,
     but no graph operation can observe that intermediate state. Concurrent revoke sees one
     completed transaction or the other. Adjacent dying nodes and a dying root are handled.
   - [ ] FAIL: the set is deliberately "may have children," not exact (`process.rs:1677`). Concrete
     input on CPU 0 under successive graph sections: create root R; derive its only child C
     (`note_parent` inserts R at `:1710`); close C. `retire` returns at `:1733` because C has no
     children; nothing removes R from the set. R now has zero children and remains recorded.
     Closing R later causes an unnecessary scan. `forget` likewise removes only revoked IDs
     (`:1719`), not surviving parents that lose their last child. This is a checklist/optimization
     discrepancy, not a missed revoke: reparent inserts any new surviving parent at `:1739` and
     removes dying entries at `:1744`, so no false negative was found in the enforced paths.

5. [x] **PASS — spawn publication and failed-parent cleanup.**
   `syscall.rs:1731` clones inherited handles before spawn. `process.rs:2648` takes the graph,
   checks every inherited parent at `:2652`, installs at `:2665`, registers at `:2676`, then drops
   the graph at `:2677` before constructing/enqueueing the thread. ParentGone returns before
   registration (`:2654`) and maps to EBADF (`syscall.rs:1766`). Guard drop precedes the earlier
   constructed Process/Pmap locals on that error path. A child refused for a dead parent is never
   half registered. `unregister_unstarted` (`process.rs:1583`, used at `:2707`) does remove a
   scheduler-refused child outside CAP_GRAPH: that child has never run or exported a descendant,
   so no concrete lost-ancestry case follows; its parent can retain an advisory stale set entry.

6. [ ] **FAIL — seven-leg mechanism sensitivity is weaker than claimed.**
   - [x] PASS, source reading plus author-reported result: the combined unlocked control executes
     operations inside the windows (`process.rs:2226`, `:2245`) and disables lock, parent checks,
     and reparenting. The seven leak bits sum to 127, so `10 + leaked` is 137
     (`userland/neo-init/src/main.rs:6620`, `:6792`). PR #166 reports all seven leaking. That
     control execution was not independently rerun here.
   - [ ] FAIL: the normal race hook does not start an install on another CPU and witness it block.
     It checks try_lock and **manually defers** the operation (`process.rs:2217`–`:2224`), then
     invokes it after removal (`:2257`–`:2270`). For race-grant, the deferred `install_now` first
     re-reads the already removed source at `:2166` and returns EBADF at `:2175` without calling
     complete_grant. Thus it can pass even if complete_grant loses its continuous graph section.
     Concrete selective regression: release the graph after copying the grant source, reacquire
     immediately before `install_minted_locked`. CPU A copies the source, CPU B walks/removes its
     ancestor, CPU A reacquires and installs the copied child with parent_in_proc=false. The real
     interleaving leaks; this hook still runs the grant only after removal and reports success.
     The source gates merely recognize an allowed function containing lock/guard text
     (`tools/cap-graph-logic-tests/src/lib.rs:123`), not continuous ownership across that window.
   - [ ] FAIL: derive has the same forced sequencing limitation. Spawn genuinely retains an
     inherited copy across the window and therefore checks parent refusal, but treats every
     negative spawn result as absence of leakage (`neo-init/src/main.rs:6646`–`:6650`), allowing
     an unrelated spawn failure to pass. These are test discrimination gaps, not timing luck
     in the current hook. Close and share-close directly test reparenting; exit/kill test the
     final rewritten ancestry but similarly simulate the contending revoke by deferral at
     `process.rs:2243`. They do not prove that another CPU was actually prevented from entering.

7. [ ] **FAIL — no regression overall (F1/F2); the requested fixture coverage is present.**
   - [x] PASS: PR #166 lists a 45-scenario sweep including `wait_any_core`; its cross-process
     fixture is `test_harness.rs:24876`–`:24900`, with install naming the supplied ancestor's cap.
     The global parent scan permits that fixture (`process.rs:1867`). Existing wait removal
     notifications remain at `:1299`–`:1301`. The run/sweep evidence distinction is below.
   - [x] PASS: behavioral changes were identified: a tensor share, TCP accept or GPU fence whose
     captured parent closes/revokes before install now fails (current syscall mappings return
     EINVAL: `syscall.rs:2122`, `:4884`, `:7675`); spawn losing an inherited parent returns EBADF
     (`:1766`). Intermediate close/exit preserves ancestor revocability while recipients continue
     to survive donor exit (ADR Q4). Root close reparents its descendants to roots. Socket close
     now retires its graph edge before closing network state (`syscall.rs:5419`, `:5430`), keeping
     potentially broader object cleanup outside CAP_GRAPH. F1/F2 add unintended global liveness
     failures, which are the reason this item cannot pass overall.

8. [x] **PASS — source attribution of the reported .text delta, with measurement limits.**
   The full 20-file squash diff was read. Production additions are the lock, parent/edge checks,
   reparenting/teardown and related error handling; the remainder is host tests, scenario/hook
   wiring and documentation. Test hooks compile only for the scenario (`process.rs:2095`,
   `syscall.rs:1231`); no unrelated production feature was added. `kernel/build.rs:24` honors a
   supplied NEO_BUILD_ID, so pinning removes HEAD variation. PR #166 reports
   `NEO_BUILD_ID=kill-switch-fixed`: default +9600 B, desktop +9536 B, usb-host +9600 B versus
   main `785d5478`. These numeric/section-hash measurements were **not reproduced** here; no local
   binary comparison is claimed. Attribution is consistent with the source diff, not a claim that
   every added byte has been independently mapped to a symbol.

9. [ ] **FAIL — required concurrency prior-art section exists, but its no-wait claims do not hold.**
   PR #166 includes the CP table, and `docs/concurrency-pitfalls.md:3935` / `:3962` record CP-87/88.
   CP-37's IF discipline, CP-56's hidden Drop/outer-lock lesson and CP-63's parked-holder lesson
   were consulted (current bank read via `git show origin/main:docs/concurrency-pitfalls.md`).
   Direct call sites obey masking; the "nothing under it waits on another CPU" statement at
   `docs/concurrency-pitfalls.md:3951` is false by F1, and the no-park premise is false by F2.
   CP-61's existing removal wake mechanism is retained. No new device/protocol or vendored
   unfamiliar feature is introduced, so a separate driver survey/license comparison is N-A.

## Evidence and limits

- [x] PASS: started on the requested branch at exact reviewed SHA with a clean tree. Read the
  authoritative dispatch from origin/main, local AGENTS/CLAUDE routing and concurrency rules, full
  squash diff, caller table and call/drop/lock continuations. RAG health/query succeeded during
  setup; its other-checkout results were discovery hints only. Exact reviewed source governs this return.
- [x] PASS: independently queried CI run
  [40484657565](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40484657565): conclusion
  success, all 22 jobs success including four QEMU shards; run head is
  `30ef0049cd99e76d903f6472df7bf627ac8f37b6`. This is not the squash SHA. The dispatch/PR associate
  it with the merged code; local comparison to that exact run object was unavailable (`bad object`).
  The PR's separate 45-scenario sweep, seven-bit control run and section measurements are author
  reports, not new independent local executions.
- [x] PASS: actual host KAT source compiled directly with pinned rustc nightly-2026-04-16 (1.97):
  all eight tests passed (source gates, graph cases, ticket exclusion/FIFO). Cargo's offline route
  stopped at missing cached `gpt` in unrelated block-guard workspace resolution; it ran no tests.
  Compile-only guard-Sync probe accepted the unsafe-to-share Cell guard as described in F3.
- [x] PASS: no local QEMU gate, kernel implementation edit, main checkout, merge or revert was
  performed. Review findings F1/F2 are concrete static interleavings, with proposed discriminators;
  they are not presented as reproduced guest failures. Request remains **revert-first**, then repair
  and rerun a deterministic lifetime/drop/Reserved-slot gate before re-landing.
