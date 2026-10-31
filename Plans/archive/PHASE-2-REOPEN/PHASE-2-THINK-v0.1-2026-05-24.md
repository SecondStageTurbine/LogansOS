# Phase 2 THINK v0.1 — Design Questions for PL Ratification

**Date:** 2026-05-24
**Author:** PM (substance), Prometheus (polish)
**Status:** Awaiting PL ratification
**Scope binding:** `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md`

---

## Preamble

Per the four ratified scope-class items (`PL-RATIFICATION-…-phase-2-observe-scope-class-items.md`): Phase 2 PLAN proceeds with a formal-completion BUILD set (close-first). Closing the open Article 13 mandate, activating ADR-010's three reserved-bit syscalls, standing up Bible 16.4's mandated perf-regression harness, absorbing the audit (b)-disposition tests, and shipping the defense-in-depth items (ADR-019 B/C, ADR-016 A2) constitute the close. Optimization items (Arc-aware slab, PCID/lazy-TLB) defer to follow-on or Phase 3 substrate.

The seven questions below are the design questions PLAN must resolve before BUILD subphases can be enumerated. PL ratifies as a batch; questions resolving to "do X" become BUILD subphase scope; questions resolving to "defer Y" become explicit anti-scope.

---

## Active Questions

**Q1 — `sys_close` use case and semantics.** ADR-010 reserved the CLOSE semantic bit pending a use case. Today, channel close is implicit at process exit via the `close_channel_peers` shared helper (now structurally pid-aware per F-005's unified `extract_parked_if_owned`). Does Phase 2 surface an explicit `sys_close(handle)` use case distinct from process-exit cleanup — for example, application-level "I'm done with this channel" semantics for long-lived processes that hold many channels — and if so, what are its semantics? Specifically: does `sys_close` wake the peer with `-EPIPE` mirroring exit-time cleanup, how does it interact with the survive-exit cap lifetime invariant (a closed channel cap still held by a third process), and is the CLOSE semantic bit gated as "enforcement, not theatre" per ADR-010 Principle 2? If no use case surfaces, the disposition is: ship `sys_wait` and `sys_inspect`; defer `sys_close` to the phase where a use case emerges, leaving the CLOSE bit reserved.

**Q2 — `sys_wait(handle)` semantics and FM-D14 retirement.** ADR-007's "Future Phase 2 hardening" names `sys_wait` returning the target process's exit code, and FM-D14 records that `COMPLETED_EXIT_CODES` grows unbounded without a wait/consume consumer. The design surface: (a) blocking semantics — does the caller block until the target exits, or return immediately with a "still running" indicator for polling? (b) handling targets that have already exited — does the call return the cached exit code directly and free the COMPLETED_EXIT_CODES slot? (c) multiple waiters — what happens if two processes hold ProcessHandles to the same target and both call `sys_wait` (one consumes, the other gets -ESRCH, or both get the cached code)? (d) interaction with D13 single-thread-per-process and D8 `exit_state` CAS — does `sys_wait` participate in the CAS dance or observe it externally? (e) FM-D14 retirement: does landing `sys_wait` fully retire the unbounded-growth concern, or only bound it (a code can still accumulate if no process holds a ProcessHandle at exit time)?

**Q3 — `sys_inspect(handle)` surface.** The INSPECT semantic bit is reserved per ADR-010. What process or thread state is inspectable through it, and how does the design stay inside Bible 12.2's no-ambient-authority discipline? Candidate inspectable state: target's exit_state (Running / Exited / Zombie), cached exit code if Exited, cap inventory (with explicit anti-leak discipline — inspecting target's caps must not authorize using them), thread count (currently always 1 per D13), open channel handle count. What does INSPECT on a ProcessHandle authorize observing without leaking another process's capabilities — i.e., is the rights-narrowing principle "INSPECT lets you observe state but never derive a usable cap from observation" structurally enforceable? Is INSPECT useful enough without cap-passing (Phase 4) to justify shipping in Phase 2 close, or does it await Phase 4's cap-passing-over-IPC to become operationally meaningful?

**Q4 — Bible Article 16.4 performance-regression harness.** Article 16.4 mandates per-commit tracking of boot time, syscall latency, context-switch cost, and scheduling fairness (Redox `schedrs`-adapted) "from Phase 2 onward." No such harness currently exists. Phase 2 close stands one up. The design questions: (a) reference environment — bare-metal OptiPlex, VMware VM, or QEMU TCG, and which numbers are load-bearing for regression detection (per the three-environment discipline)? (b) measurement methodology — single-shot, n-trial median, or distribution comparison; what statistical noise floor distinguishes signal from variance? (c) integration with CI — runs every push, PR-only, or nightly on a dedicated runner? (d) composition with ADR-019's deferred 50ns native bench — does the perf harness subsume it, or are they orthogonal? (e) regression policy — what delta triggers what action (warning, blocker, ADR for the regressing change)?

**Q5 — ADR-019 sub-phases B/C scope and pairing with audit (b) tests.** ADR-019 sub-phase A landed at `27d1b99` (IrqSafeMutex type at the wrapper layer). Sub-phase B migrates mixed-context `SwitchMutex` callers to `IrqSafeMutex`; sub-phase C migrates `Channel.state` `without_interrupts(...)` sites and removes the manual wrappers. The questions: (a) is B/C a standalone defense-in-depth BUILD subphase, or does it pair structurally with the audit (b) tests that touch the same Channel.state lock surface (INV-IPC-5 `ipc_rendezvous_translate_fault`, INV-SYNC-5 `sched_ready_queue_capacity_pressure`)? (b) what is the per-sub-phase verification gate — the SC-3B 200/200 NEO_SMP=4 standard, or something looser given the type-enforcement nature of the change? (c) is B before C the right order (caller migration before site migration), or does C before B avoid migrating call sites twice?

**Q6 — Bare-metal multi-CPU verification protocol.** Bible Article 13's bare-metal multi-CPU mandate is the open Phase 2 close-gate. The OptiPlex 7080 boot smoke is single-CPU UEFI boot only and does not satisfy the mandate. The questions: (a) acceptance bar — which SMP scenarios MUST PASS on real hardware (the SC-3B suite at minimum: `smp_cross_cpu_rendezvous_stress`, `q7c_*` shootdown, `q8a/b/c_*` cap-table-SMP)? (b) core count requirement — does 4 cores satisfy, or 8, or must it span both? (c) evidence shape — frame-audit-clean post-run, no PANIC/BUG kernel messages, GS-base sentinel checks PASS, serial-log capture of the full run? (d) hardware target — OptiPlex 7080 (current bare-metal target) or additional machines for cross-vendor coverage? (e) procedure shape — formal protocol document for the OptiPlex bare-metal path, and is the procedure itself part of the BUILD or pre-BUILD planning?

**Q7 — ADR-016 A2 implementation shape.** Per §1.4 of the PL ratification, ADR-016 A2 (decouple scheduler from global heap; replace `VecDeque<Box<Thread>>` ready/zombie queues with bounded/pre-allocated storage) is Phase 2 conformance work and folds into the close's defense-in-depth subset. ADR-016 specifies the architectural goal but defers the implementation structure to a follow-up design dispatch. The questions: (a) intrusive linked list via per-Thread next/prev fields, fixed-capacity per-CPU ring buffer, or another structure — what's the right shape given ADR-013's per-CPU scheduler topology and the D13 single-thread-per-process invariant? (b) where does the bounded capacity come from — `MAX_CPUS=64` × some per-CPU thread cap, or a global ready-queue cap that survives the FM-D5 hot-path-PROCESSES retirement? (c) does this work compose with audit (b)'s `sched_ready_queue_capacity_pressure` test (INV-SYNC-5) — are they the same BUILD or sequential? (d) sequencing relative to ADR-019 B/C — does A2 land before or after the lock-type migration, and is there an ordering constraint from the per-CPU scheduler's lock surface?

---

## Q11 Disposition

PL ratified deferral of Q11 (Spectre-v2 / IBPB on context switch) to a dedicated ADR per `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` §1.5. Q11 is NOT in this THINK batch. Future trigger for revisit: a CPU-vulnerability event that elevates BTI urgency, a security ADR that takes IBPB-on-context-switch as part of broader CPU-vuln mitigation, or measurement showing the unmitigated BTI surface is a near-term workload concern.

---

## References

- `Plans/PHASE-2-THINK-v0.1-2026-05-24-DRAFT.md` — predecessor draft (PM-authored substance).
- `Plans/PL-RATIFICATION-2026-05-24-phase-2-observe-scope-class-items.md` — scope binding and ratified dispositions.
- `Plans/CEREBRO-OUTPUT-2026-05-24-phase-2-observe.md` §5 — origin of the 14 question seeds (4 closed by ratification, 3 deferred as measurement-gated, 7 retained as active questions).
