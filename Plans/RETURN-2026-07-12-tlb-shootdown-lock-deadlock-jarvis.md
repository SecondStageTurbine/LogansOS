<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: TLB shootdown sender-lock deadlock and spawn-transition targeting

**Date:** 2026-07-12
**From:** Jarvis
**To:** PM / PL
**Branch:** `codex/tlb-shootdown-lock-deadlock-jarvis`
**Dispatch:** `Plans/DISPATCH-2026-07-12-b3-blocked-disposition-two-main-defects.md`, defect #1
**Status:** FIX PROVEN; full exact-head QEMU gate BLOCKED by separately assigned OT-59 defect #2

## Disposition

The defect is live on plain `main`, independent of OT-16 B3's heavy allocations. The plain-main discriminator timed out with CPU1 waiting for CPU0's TLB-shootdown ACK; its serial log SHA-256 was `0C9AEE22A5FED4C264F39DBFF93C7A372B8E14A30D39C717A065011C9EB408C6`.

Root cause #1 was an ABBA deadlock. CPU A held the global shootdown sender lock while waiting for CPU B's ACK. CPU B contended for that `IrqSafeMutex`, which masked B's interrupts, so B could not enter the TLB IPI handler and acknowledge A. The sender lock is now a `SwitchMutex`; a contending IF=0 caller temporarily enables interrupts only while waiting, then restores its original IF immediately after acquisition. The IPI handler still never takes the sender lock.

## Did We Fix The Bug Or The Test?

The original scenario used `any_user_process()` as if it meant "CPU0 has entered ring 3 and can service an IPI." That was a **test-precondition artifact**: process-registry publication deliberately precedes first ring-3 dispatch, so the predicate did not prove what the scenario assumed.

However, the corresponding production state was not dismissed as unreachable. A CPU can be registry-visible while `spawn_user` still has IF=0, and the old production broadcast selected every shootdown-ready CPU without regard to that spawn transition. Merely waiting for a ring-3 handshake would have hidden a real targetability hole. The production path now handles the reachable state:

1. While the CPU is still serviceable, it acquires the sender lock. Under that lock it masks IF and withdraws its shootdown-ready bit. Every sender that could have sampled the old bit has completed before withdrawal; no later sender can select the CPU.
2. The existing IRQ-off Pmap/ELF/process-publication transaction runs while the CPU is explicitly non-targetable.
3. Rejoin uses an explicit per-CPU timer-preemption guard, then reacquires the sender lock while allowing non-timer IPIs. Under the lock it performs a full local TLB flush and republishes readiness.
4. It releases the sender lock and immediately executes `sti`. A sender cannot pass the lock before release; an IPI sent in the release-to-`sti` interval remains pending and is serviced after IF becomes 1.

This ordering also closes the subtler flush/publication race: no remote mapping-update sender can interleave after the full local flush but before the ready-bit publication, because both occur under the same sender lock.

The revised scenario proves both sides independently. During the real IRQ-off publication window it observes `ready=false`, attempts a shootdown, and proves `ipi_delta=0`. After production rejoin, but before first ring-3 execution, it sends a real TLB shootdown and receives the ACK. A separate reschedule IPI then proves an interrupt entry from ring 3 before the 128-shot flood begins. The test handshake no longer creates safety; it observes the production invariant.

## Changes

- `kernel/src/mm/tlb_shootdown.rs`: interruptible sender serialization, locked readiness withdrawal/rejoin, full local flush, timeout breadcrumb, and deterministic contention probe.
- `kernel/src/sched/process.rs`: scopes the IRQ-off spawn transaction inside the local shootdown quiesce protocol.
- `kernel/src/sched/scheduler.rs`: nestable per-CPU timer-preemption guard; IF and non-timer interrupt delivery remain live.
- `kernel/src/arch/x86_64/per_cpu.rs`, `idt.rs`: readiness transition and ring-3 reschedule-entry observation.
- `kernel/src/test_harness.rs`, `tools/xtask/src/main.rs`: deterministic ABBA, publication-filter, pre-ring-3 ACK, ring-3 entry, 128-shot, IF-restoration, and EOI-progress oracles.
- `docs/ADR-013-phase-2-smp-retrofit.md`: records the sender-lock and temporarily-unserviceable-CPU protocol.

The earlier broad serial/block-sink/spawn experiment was removed. Against the rebased mainline, the final code changes are confined to the eight focused files above, plus this RETURN.

## AAG

**Concurrency:** PASS. The sender lock serializes pending-slot ownership. Readiness withdrawal and rejoin are ordered by that same lock. The deterministic two-CPU probe forces the former ABBA schedule rather than relying on stress probability.

**Interrupt / EOI:** PASS. IF=0 contention restores IF=0 after acquiring the lock; the holder's IPI is acknowledged while the waiter is interruptible. The scenario proves continued EOI progress and 128 completed TLB IPIs from ring 3.

**I-14 lock census:** PASS. No allocation, blocking I/O, or nested lock was added to the TLB IPI handler. The handler remains lock-free with respect to sender serialization. The new preemption guard is a per-CPU atomic depth counter and does not mask interrupts.

**Completeness:** PASS. The test's old precondition is identified as invalid, but the reachable production state is handled rather than waved away. Filtering, flush-before-rejoin, pre-ring-3 serviceability, ring-3 entry, repeated delivery, IF restoration, and EOI progress each have a distinct oracle.

**Failure behavior:** PASS. A shootdown that exceeds the existing final deadline still panics; it now prints sender CPU/IF, targets, pending count, unacked slots, retry mask, and cycle budget before panic. There is no fail-continue path.

**Scope / regressions:** PASS. No inference, B3, NeoFS, serial, or block-sink behavior remains changed. The scenario still requires natural user exit and exact frame-accounting recovery.

## Local Evidence

Host checks:

```text
cargo fmt --all -- --check                                      PASS
cargo check -p neoplutarch-kernel --features
  neo-test-swapgs-irq-ring3,neo-test-instrumentation            PASS
cargo test -p xtask --bin xtask                                 PASS (15/15)
git diff --check                                                PASS
```

Final local QEMU oracle before the closing rebase:

```text
shootdown_lock_probe=ok waiter_cpu=0 if0_contention=ok if_preserved=true ipi_ack=ok eoi_progress=ok
shootdown_publication_filter_probe=ok ready=false ipi_delta=0
shootdown_pre_ring3_rejoin_probe=ok ready=true ipi_ack=ok
shootdown_publication_probe=ok filtered_while_if0=ok pre_ring3_ack=ok
swapgs_ring3_ready_probe=ok reschedule_ipi_ring3_entries=2
NEO_TEST/1 event=PASS name=swapgs_irq_ring3 user_exit=0 lock_contention_probe=ok publication_probe=ok ring3_ready_probe=ok if_preserved=true eoi_progress=ok ipis_sent=128 cpu0_ipis_received=130 tlb_ipi_ring3_entries=121
```

Serial SHA-256: `06636119CA94C8E9D96A6E9555799173A3C6CF62F3097E99F93168E4A4B8CBC6`.

The locked-withdrawal/rejoin design also passed three consecutive QEMU runs before the explicit preemption guard replaced the bounded handoff credit. Their serial SHA-256 values were `2ECCF4EB6D2E0A77E867DC83CEB96D1A870683B4FCAAAB06AA0740201C547630`, `2BF3AD66E7B55E1B148FC3BC606F5DCBF775AC066E07B99AD99733F407CEFC02`, and `3CC6FAE1FB80DFA4C20913767FE87296F28B197B695144957693DC40BC5AF485`. The final explicit-guard design then passed the oracle above.

## Close Evidence

- Rebased on: `origin/main@e350ac4`
- Exact implementation/RETURN head: `9233745accf58bb56b83c38794cc07f30a9687bc`
- Host close gate: PASS, including fmt, host workspace tests, workspace clippy `-D warnings`, blocking kernel clippy, SPDX, and self-inspection
- Push CI: GREEN on exact head: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29214929089
- Manual QEMU gate: RED on exact head: https://github.com/SecondStageTurbine/NeoPlutarch/actions/runs/29215422018

The manual gate is a real red and is not attributed to infrastructure:

- Host, SPDX, and monolithic-identity jobs were green.
- Shard 1: `swapgs_irq_ring3` PASS on exact head in 90.5 seconds. The strengthened xtask oracle means this pass includes the deterministic lock-contention, filtered-publication, pre-ring-3 ACK, ring-3 entry, IF-restoration, EOI-progress, and 128-shot proofs. The shard was cancelled later at its 70-minute job ceiling.
- Shard 2: `arthur_memory` timed out on all four 180-second attempts.
- Shard 3: `arthur_reversibility` timed out on all four 420-second attempts.
- Shard 4: `arthur_multistep_agentic` timed out on attempts 1, 3, and 4; attempt 2 failed its missing `agentic_model_mode=graph-fixture` oracle.

The three Arthur failures are the separately dispatched OT-59 defect #2 family, not this branch's shootdown scenario. They are genuine scenario failures, not preflight, Docker, memory, or disk failures. I did not rerun the same full gate: it would consume the runner fleet while the named upstream defect remains reproducibly red. After defect #2 lands on `main`, this branch must rebase and obtain a fully green four-shard workflow-dispatch URL before merge or OT-16 B3 resume.

This RETURN proves defect #1's fix and records the remaining close-gate blocker; it is not a merge-ready completion claim.
