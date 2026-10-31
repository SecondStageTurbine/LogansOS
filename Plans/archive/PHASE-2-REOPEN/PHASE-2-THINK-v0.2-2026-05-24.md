# Phase 2 THINK v0.2 — Answered Design Questions for PL Ratification

**Date:** 2026-05-24
**Author:** PM (substance owner), Cerebro (recommendations), Prometheus (polish)
**Status:** Awaiting PL ratification
**Scope binding:** `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` (four scope-class ratifications); `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` (PM acceptance of Cerebro dispositions)
**Predecessor:** `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — the seven ratified questions; `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (`c5017bf`) — v0.1 batch ratification

---

## Preamble

Phase 2 PLAN cadence: OBSERVE closed, scope-class items ratified, THINK v0.1 question batch ratified. This THINK v0.2 presents the answered disposition for each of the seven questions. PL ratifies as a batch; the dispositions resolve to BUILD subphase scope or explicit anti-scope. PM concurs with all seven Cerebro recommendations per `PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md`.

---

## Q1 — `sys_close` use case and semantics

**Disposition: DEFER `sys_close`. Ship `sys_wait` + `sys_inspect`; CLOSE bit stays reserved.**

The use-case search found no Phase-2 driver: the only processes are short-lived test cohorts and `neo-init`, none of which hold many channels long enough that implicit-close-at-exit is a problem. No "I'm done with this channel" pattern appears in the project's accumulated design notes. The genuine driver is Phase 4 (network sockets) and Phase 7 (multi-tenant inference serving) — both out of scope.

ADR-010 note 8 frames CLOSE as "ships Phase 2 BUILD-2 or later, once a use case appears" — explicitly use-case-gated. Shipping with no consumer violates the project's own "no mechanism without a real correctness/security/accounting property" discipline. Deferral leaves the CLOSE bit reserved at zero ABI cost; future activation is non-breaking.

The semantics are already built: `sys_close(handle)` would gate on `semantic & CLOSE != 0`; route my-side endpoint through the unified `close_channel_peers` / `extract_parked_if_owned` machinery F-005 hardened, waking a blocked peer with `-EPIPE`; survive-exit is unaffected because close operates on this handle's endpoint and a third process's derived cap keeps the `Arc<Channel>` alive. Only the syscall wrapper + CLOSE gate would be new.

**Alternative considered:** ship now for ABI completeness. Ruled out: activating a reserved bit with no consumer adds permanent surface to test and maintain for zero current benefit.

**Revisit trigger:** Phase 4 network-socket handles, or any long-lived process whose channel count makes hold-until-exit a resource concern.

---

## Q2 — `sys_wait(handle)` semantics and FM-D14 retirement

The consumer surface already exists in source: `exit_state`, `exit_code`, `record_exit_code()`, `take_completed_exit_code(pid)`. `sys_wait` is the missing caller + blocking case.

**(a) Blocking semantics — blocking by default, with a `WNOHANG`-style non-blocking poll flag.** ADR-007 specifies "blocks until the target exits"; the canonical use is the supervisor (`neo-init` waits for a worker). The blocking case parks the caller's Thread via the established rendezvous-block mechanism; the reaper wakes it. The poll flag returns immediately with a "still running" indicator. *Alternative — poll-only:* ruled out because it forces busy-polling, contradicts ADR-007, and weakens FM-D14 bounding.

**(b) Already-exited target — return cached exit code immediately, free the slot via `take_completed_exit_code`.** This is the FM-D14 retirement mechanism: the consumer that frees `COMPLETED_EXIT_CODES` entries.

**(c) Multiple waiters — first waiter consumes; subsequent waiters get `-ESRCH`.** POSIX-faithful (`wait()` single-consumer), keeps `COMPLETED_EXIT_CODES` bounded. At reap, one parked waiter wakes with the code; others wake with `-ESRCH`. *Alternative — broadcast + handle-presence-gated slot lifetime:* cleaner full FM-D14 retirement but requires a per-target handle refcount the cap system doesn't currently expose. Deferred (see (e)).

**(d) D8/D13 interaction — `sys_wait` observes `exit_state` externally; does NOT participate in the CAS.** D8's CAS serializes who performs cleanup (natural-exit vs forced-kill); `sys_wait` is a reader of the outcome, not a cleanup actor. Clean producer/consumer split: the reap path (CAS owner) is the sole producer; `sys_wait` is the consumer. Under D13, thread reap IS process exit — exactly one wake point.

**(e) FM-D14 retirement — `sys_wait` BOUNDS growth; does not fully retire it.** With first-consumer semantics, a code is freed when waited. But a code for an exited process whose handle-holder never calls `sys_wait` still accumulates — the residual. Full structural retirement requires tying slot lifetime to handle-presence (free when the last `ProcessHandle` drops). Phase 2 close ships the practical bound; full retirement is a future ADR triggered by `COMPLETED_EXIT_CODES` residual-growth measurement if it surfaces.

**Prior art:** POSIX `waitpid` (WNOHANG, single-consumer ECHILD); Plan 9 `await`/`wait`. The first-consumer + WNOHANG shape is the well-trodden one.

---

## Q3 — `sys_inspect(handle)` surface

**Disposition: SHIP `sys_inspect` in Phase 2, scoped to a thin scalar status snapshot.**

- **Inspectable-state set:** run-state (`exit_state` enum: Running / Exited / Zombie), cached `exit_code` if exited, `thread_count` (always 1 under D13 — future-proofs for multi-thread), open-channel-handle count, and `name`. Returned as a fixed value-type snapshot struct passed by value — never any `Handle`, `CapId`, or pointer field.
- **No-ambient-authority enforcement (Bible 12.2):** structurally enforced by the return type — only scalars (enums, counts, an i32 code, a `&'static str` name). Cap inventory identities are explicitly excluded; a count is observable, identities are not.
- **Phase-2-vs-Phase-4:** ship in Phase 2. The `ProcessHandle` minted at `sys_spawn` already carries INSPECT; the parent-inspects-child supervisor pattern works today (`neo-init` asks "is my worker still running? what code did it exit with?" without killing or blocking).

**Reasoning.** Activating INSPECT completes the reserved-bit ABI trio cheaply (read-only syscall, no new mechanism) and complements `sys_wait`: `sys_wait` blocks-and-consumes; `sys_inspect` is the non-blocking, non-consuming peek.

**Alternative considered:** defer alongside `sys_close`. Under D13, `thread_count` is always 1 and `exit_state`+`exit_code` overlap `sys_wait(WNOHANG)`, so the marginal Phase-2 surface is thin. PM concurred with shipping because the cost is near-zero and it gives the supervisor a clean non-consuming status read.

---

## Q4 — Bible Article 16.4 performance-regression harness

**Core insight:** separate regression-gating from absolute-characterization, and assign each to the environment that can actually serve it.

**(a) Reference environment — TIERED, not single.** Three environments, three roles: **QEMU TCG** runs deterministic structural proxies per-commit (context-switch counts, CR3-write counts, allocation counts, scheduling-decision counts for a fixed workload) — TCG absolute timing is fiction, but counters are deterministic and catch algorithmic regressions. **Bare-metal OptiPlex 7080** captures absolute-latency characterization (boot time, syscall latency ns, context-switch ns, fairness) during PL hardware sessions. **VMware-VM** is the reproducible middle tier for relative trends when bare-metal sessions aren't available. Load-bearing for gating = TCG proxies; load-bearing for absolute characterization = bare-metal. *PL operational confirmation needed — see §9 below.*

**(b) Methodology — n-trial median + IQR for absolute metrics; deterministic single-shot for TCG proxies.** Absolute metrics: report median over n trials with IQR as spread; flag a regression only when the median shift exceeds a margin tied to the measured spread. The TCG proxies are deterministic; single-shot suffices and any change is signal.

**(c) CI integration — proxies every push (gating); absolute characterization periodic (tracking, not gating).** Deterministic TCG proxies run on every push as part of existing CI (cheap, deterministic, no flake). Absolute-latency characterization runs nightly on a dedicated runner if available, otherwise at phase boundaries during PL bare-metal sessions. Per-push gates only on the proxies; absolute trends are tracked, not gated.

**(d) ADR-019 50ns native bench — SUBSUMED, not orthogonal.** The IrqSafeMutex lock/unlock-cycle micro-bench is exactly the kind of context-switch-adjacent measurement Bible 16.4 wants; fold it in as one of the harness's bare-metal micro-benchmarks, retiring the standalone ADR-019 deferral.

**(e) Regression policy — three tiers.** (i) Deterministic-proxy change = **CI BLOCKER** (deterministic, so any change is a real algorithmic change). (ii) Absolute-latency regression beyond noise margin on characterization tier = **WARNING + investigation** (environment-sensitive, not auto-blocker). (iii) Sustained/large regression across multiple characterization runs = **requires an ADR justifying the regressing change** (Bible 1.2: performance is third, so a regression accepted for correctness/coherence is fine — but must be justified in writing).

**Alternative considered:** single reference environment. Pure-bare-metal can't gate per-commit; pure-QEMU-TCG can't characterize absolute latency. Either fails half of 16.4's intent. The tiered approach gates continuously and characterizes truthfully.

---

## Q5 — ADR-019 sub-phases B/C scope and pairing with audit (b) tests

**(a) Pair sub-phase C with `INV-IPC-5` only; route `INV-SYNC-5` to Q7/A2.** The two audit (b) tests touch different lock surfaces: `INV-IPC-5` (`ipc_rendezvous_translate_fault`) is on the Channel.state / rendezvous surface — that IS sub-phase C's surface. `INV-SYNC-5` (`sched_ready_queue_capacity_pressure`) is on the scheduler ready-queue surface — that is Q7/A2's surface, and A2 is its structural fix. Pairing `INV-SYNC-5` with ADR-019 C would put a ready-queue test in a Channel.state subphase. So: ADR-019 B/C + `INV-IPC-5` = one defense-in-depth subphase; `INV-SYNC-5` rides the A2 subphase.

**(b) Per-sub-phase gate — C gets the SC-3B 200/200 `NEO_SMP=4` gate; B gets a lighter gate (full suite green + ADR-019 micro-bench).** Gate proportional to risk. Sub-phase C migrates `Channel.state` — the exact lock surface SC-3B's Hpw mechanism lived on; a regression there is catastrophic. Sub-phase B migrates non-Channel.state `SwitchMutex` callers at lower-risk sites; full-suite-green plus the IrqSafeMutex acquire/release micro-bench suffices.

**(c) B before C — confirmed.** B establishes `IrqSafeMutex` usage broadly at mixed-context `SwitchMutex` callers, proving the type in place; C then migrates the highest-risk `Channel.state` sites onto the now-established type and removes the manual wrappers. B and C touch disjoint locks, so there's no double-migration risk; the order is about coherence, and B-first is cleaner.

**Alternative considered:** single combined "lock-discipline" subphase. Ruled out: bundles catastrophic-risk Channel.state migration with lower-risk work under one gate, forcing the whole subphase to the 200/200 bar and conflating a ready-queue test with a Channel.state migration. Splitting B (light gate) from C (heavy gate) and routing INV-SYNC-5 to A2 keeps risk and surface aligned.

---

## Q6 — Bare-metal multi-CPU verification protocol

**(a) Acceptance bar — the SC-3B/3C/4 SMP-specific scenario set must PASS on real hardware:** `smp_cross_cpu_rendezvous_stress` (SC-3B 200/200 gate), `q7c_*` (TLB shootdown: missing-shootdown-detection + concurrent-munmap), `q8a/b/c_*` (cap-table-SMP + blocked-kill walk), the `cap_*` suite under SMP, and `ipc_heap_kill_smoke`. Bare-metal's value over QEMU is real LAPIC IPI delivery, real cache coherency, and real TSC — so the must-pass set targets exactly the scenarios whose correctness depends on those.

**(b) Core count — ≥4 logical CPUs minimum, plus the target's full count.** 4 gives apples-to-apples comparability with the established `NEO_SMP=4` dev gate; running at the hardware's full logical-CPU count validates `MAX_CPUS` scaling beyond 4 and surfaces SMT-specific issues.

**(c) Evidence shape — mirror the QEMU PASS vocabulary plus AP-bringup evidence:** full serial-log capture, frame-audit-clean post-run, zero `KERNEL PANIC` / `BUG:` lines, per-AP GS-base sentinel PASS, per-scenario PASS sentinels. Extended from the established PASS vocabulary (`PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md` §5) with per-AP sentinels.

**(d) Hardware target — OptiPlex 7080 required; AMD cross-vendor a non-blocking stretch.** *(PL operational ratification — see §9 below.)* OptiPlex 7080 is the established bare-metal target and satisfies Bible Article 13's "at least one bare-metal target." Cross-vendor AMD would validate vendor-specific APIC/INVLPG behavior but is not mandated.

**(e) Procedure shape — formal procedure document modeled on the VMware procedure, authored pre-BUILD, executed in-BUILD.** The protocol (boot steps, scenario list, evidence-capture instructions) is pre-BUILD planning; execution + evidence capture is the BUILD's verification deliverable. Matches the VMware precedent: PL executes a written procedure; PM consumes the evidence.

---

## Q7 — ADR-016 A2 implementation shape

ADR-016 §Decision 1 defers the A2 implementation structure. The hard constraints: spawn fails gracefully at a configurable per-CPU limit (provisional default 64); Box-pointer-stability invariant preserved (`rsp` slot must survive enqueue/dequeue); scheduler ops reach no `alloc`/`dealloc` while holding `PerCpuScheduler.inner`.

**(a) Structure — fixed-capacity per-CPU ring of `Box<Thread>`.** Two rings per CPU (ready, zombies), each `[Option<Box<Thread>>; N]` with head/tail indices, pre-allocated at scheduler init. Enqueue/dequeue = move a `Box` into/out of a slot + index arithmetic (O(1) both ends). Arbitrary removal for `kill_thread_by_pid` is O(N≤64) under a briefly-held lock.

**(b) Bounded capacity — per-CPU limit equal to the ring size, default 64, configurable at scheduler init.** `spawn`/`push_ready` check the count and return `-EAGAIN` when full. Per-CPU, not global: matches ADR-013's per-CPU scheduler topology and composes with FM-D5's retirement of `PROCESSES` from the hot path.

**(c) Composes with `INV-SYNC-5` — SAME BUILD; A2 IS the structural fix.** Today the `VecDeque` (READY_INITIAL_CAPACITY=8) can reallocate under the scheduler lock when >8 threads are ready — the `INV-SYNC-5` release-build hazard. The fixed ring cannot reallocate; over-capacity returns `-EAGAIN`. `sched_ready_queue_capacity_pressure` becomes A2's acceptance test: drive threads to the per-CPU bound and assert graceful `-EAGAIN` with no realloc/alloc-under-lock/panic.

**(d) Sequencing — A2 before ADR-019 B/C.** A2 settles the structure the scheduler lock protects; ADR-019 B/C migrates the lock type. Landing A2 first means the lock-type migration rebases onto a settled queue structure (one re-touch). They're otherwise orthogonal; the constraint is "do the bigger structural change first."

**Reasoning — why the ring over the canonical intrusive list.** The textbook answer is an intrusive doubly-linked list via per-Thread `next`/`prev` fields (Linux `list_head`, FreeBSD `TAILQ`). It needs zero container pre-allocation and preserves pointer-stability. But it forces `unsafe` raw-pointer plumbing into scheduler control flow — and Bible Article 4.2 confines `unsafe` to hardware boundaries (asm, MMIO, page tables, MSRs), reviewed line-by-line. A scheduler-data-structure intrusive list is not a hardware boundary. The fixed-capacity ring achieves every ADR-016 constraint entirely in safe Rust. The ring's cost is ~1 KB/CPU of pre-allocated slots (~64 KB total at MAX_CPUS), trivially within budget. Structure-over-discipline is ADR-016's own thesis; the ring delivers it without spending the project's tightly-rationed `unsafe` budget on scheduler logic.

**Alternatives considered:**
- **Intrusive doubly-linked list:** canonical and zero-pre-alloc, but injects `unsafe` into scheduler logic against Bible 4.2. Revisit only if a future phase needs unbounded ready queues or O(1) arbitrary-position removal the ring can't give.
- **Pre-reserved `VecDeque<Box<Thread>>` + spawn-time bound check:** keep existing container, `reserve` to the bound, refuse spawn past the bound. Ruled out because it's disciplinary, not structural — a missed bound check silently reallocates under the lock (the exact INV-SYNC-5 hazard). ADR-016's point is to make heap-independence structural; the fixed array makes "cannot grow" a type property.

---

## Cross-Question Synthesis

Three interactions collapse the seven answers into a clean two-subphase shape.

**1. Q5 ↔ Q7 (surface-matching correction + sequencing).** The audit (b) tests split by lock surface: `INV-IPC-5` rides ADR-019 C (Channel.state); `INV-SYNC-5` rides A2 (ready-queue) — A2 is its structural fix. Both touch scheduler/lock surfaces, so A2 lands before ADR-019 B/C (structure before lock-type). Defense-in-depth sequences: **A2 (+INV-SYNC-5) → ADR-019 B → ADR-019 C (+INV-IPC-5).**

**2. Q1 ↔ Q2 ↔ Q3 (the capability reserved-bit trio).** Ship `sys_wait` + `sys_inspect`, defer `sys_close`. These form one capability-reserved-bit-activation BUILD subphase, which also absorbs audit (b) `INV-CAP-11` since it's the cap-revoke surface. `sys_wait` retires the practical half of FM-D14.

**3. Q4 ↔ Q6 (shared hardware session).** Both involve the bare-metal / VMware / QEMU environment triplet. The Q6 bare-metal multi-CPU verification session is the natural opportunity to also capture Q4's bare-metal absolute-latency baseline and the folded-in ADR-019 50ns micro-bench — one PL hardware session, three deliverables.

**Resulting BUILD-subphase shape (input to PLAN, not a PLAN decision):**
- **Subphase 1 — Capability reserved-bit activation:** `sys_wait` (Q2) + `sys_inspect` (Q3); `sys_close` deferred (Q1); absorbs `INV-CAP-11`.
- **Subphase 2 — Formal close + defense-in-depth:** A2 (Q7) + `INV-SYNC-5`; ADR-019 B/C (Q5) + `INV-IPC-5`; perf-regression harness (Q4); bare-metal multi-CPU verification (Q6); remaining audit (b) `F-004`. Q4 + Q6 share a hardware session.

---

## Items for PL Operational Ratification

**Q4(a) — performance harness reference environment.** Cerebro recommends a tiered split: QEMU-TCG deterministic structural proxies gate per-commit; bare-metal OptiPlex 7080 characterizes absolute latency periodically during PL hardware sessions; VMware-VM is the reproducible middle tier. PM recommends ratifying this as proposed — it's the only design that simultaneously honors Bible 16.4's "from Phase 2 onward" mandate and the operational reality that bare-metal can't run per-commit and TCG absolute numbers are fiction. PL confirms: is the tiered split acceptable, and is per-commit gating on TCG deterministic proxies acceptable as Bible 16.4 compliance?

**Q6(d)/(e) — bare-metal multi-CPU hardware target + executor.** Cerebro recommends OptiPlex 7080 required (≥4 cores, full logical-CPU count); AMD cross-vendor a non-blocking stretch; PL executes a written procedure document modeled on `PROCEDURE-2026-05-11-vmware-hypervisor-vm-verification.md`. PM recommends ratifying as proposed. PL confirms: OptiPlex 7080 as the target, AMD cross-vendor disposition (stretch/deferred/excluded), and PL hardware session availability.

---

## Follow-on Flags

**Q2(e) full FM-D14 retirement.** If `COMPLETED_EXIT_CODES` residual-growth measurement surfaces a real concern, handle-presence-gated slot lifetime warrants a dedicated ADR. Not Phase-2-close blocking; the practical bound (first-consumer-wins `sys_wait`) ships in close.

---

## References

- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-think-v0.2.md` (`eac6737`) — Cerebro's seven recommended dispositions
- `Plans/PM-DISPOSITION-2026-05-24-phase-2-think-v0.2-substance-close.md` — PM acceptance of Cerebro substance + PL-operational items
- `Plans/PHASE-2-THINK-v0.1-2026-05-24.md` (`46426d9`) — the seven ratified questions
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-think-v0.1.md` (`c5017bf`) — v0.1 batch ratification
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — the four binding scope constraints
