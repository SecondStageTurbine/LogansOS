# ADR-011: Single-CPU IPC Handoff Invariants and Phase 2 SMP Replacement Targets

| | |
|---|---|
| **Status** | Accepted (retroactive Phase 1.5c documentation) |
| **Date** | 2026-05-04 |
| **Authority** | Project Lead |
| **Supersedes** | — (amends ADR-004 with course corrections that landed in Phase 1.5c BUILD-2 and BUILD-4) |
| **Superseded by** | ADR-013 (Phase 2 SMP Scheduler & IPC Retrofit, 2026-05-05) |
| **Related** | ADR-004 (IPC channel design — base) · ADR-007 (process termination — depends on Invariant 3) · ADR-010 (capability system — `sys_grant`/`sys_recv_grant` use this same handoff mechanism) · `docs/phases/PHASE-1.5c.md` Course Corrections |

> **This ADR has a defined endpoint.** It documents single-CPU constraints that exist because Phase 1 ships single-CPU. The expected superseder is the Phase 2 SMP retrofit ADR (likely ADR-NNN, lands as part of Phase 2 BUILD-3 SMP foundation). At that point, every Invariant below is either replaced, removed, or repurposed per the migration checklist; ADR-011 transitions to historical-record status. Do not invest in extending this ADR — invest in the Phase 2 SMP design conversation that retires it.

---

## Context

ADR-004 specified the IPC channel rendezvous as "synchronous, anonymous handles, deterministic-ordering inheritance." It did NOT specify the precise scheduling discipline that connects rendezvous completion to the resumption of the previously-blocked peer. During Phase 1.5c BUILD-2 and BUILD-4, two production boot failures surfaced under real timer pressure that forced that discipline to be made explicit:

- **BUILD-2 finding (serial in the handoff window).** The Phase 1.5b smoke test had `sys_write` calls between the rendezvous-side `sys_recv` and the follow-on blocking `sys_recv`. Under timer interrupts in the host's QEMU instance, a tick could fire while the receiver was mid-`sys_write`, preempt to the just-woken peer (the previous sender), and let the sender's `sys_kill` observe the receiver as Ready instead of RecvBlocked — breaking the kill-EINVAL determinism the BUILD-6 test depended on. **Fix:** the smoke test was tightened to put the follow-on blocking syscall immediately after the rendezvous, with no intervening userland code (and certainly no syscalls).

- **BUILD-4 finding (stale PIT tick).** Even with the smoke tightened, a stale PIT tick could fire in the narrow window between the rendezvous-completer's wake-of-peer and the rendezvous-completer's eventual yield, leaving the woken peer scheduled but immediately preemptible. **Fix:** the wake path was changed to use a front-queue placement (`wake_thread_front`) AND grant the woken peer a bounded preempt-defer quantum, cleared at the peer's next syscall entry or when it blocks.

The combined mechanism is a **single-CPU direct-handoff approximation**. It is correct under one CPU plus `without_interrupts` discipline. It is **not** correct under SMP, where the woken peer may be on a different CPU and where a "front of the queue" has no global meaning.

This ADR records the invariants the current code relies on, names them so Phase 2 SMP work can find them, and pairs each with the SMP-replacement target.

ADR-011 is retroactive Phase 1.5c documentation. The code shipped at commits `8847fd4` (BUILD-1) → `6591ab9` (BUILD-2) → `3aa07d1` (BUILD-3) → `0531b3e` (BUILD-4). This ADR documents what those commits made true; it does not propose new mechanics.

---

## Invariants

The four invariants below are presented as a **checklist for Phase 2 SMP work**. Each names what the current code enforces, why FIFO/un-bounded alternatives would break the system, and what must replace the invariant under SMP.

### Invariant 1: Rendezvous-completer yields directly to the woken peer at the front of the ready queue

**Statement.** When `Channel::send` or `Channel::recv` finds the peer endpoint in `SendBlocked` or `RecvBlocked` and copies the user buffer, the completion path calls `handoff_to_rendezvous_peer(peer_thread, return_value)` (defined in `kernel/src/ipc/mod.rs`). That function does exactly two things:

```rust
sched::wake_thread_front(thread, return_value);
sched::yield_now();
```

`wake_thread_front` places the woken thread at the **head** of `Scheduler.ready` (via `push_front`), not the tail. `yield_now` immediately rotates the rendezvous-completer to the back of the ready queue and switches to the front (i.e., to the just-woken peer). Together: the previously-blocked peer becomes the running thread before the rendezvous-completer's syscall returns to userspace.

**Rationale.** ADR-007's kill-on-Blocked test (BUILD-6) and the cap-grant rendezvous in ADR-010 both depend on the property that, under single-CPU, the just-completed-rendezvous's woken peer reaches a known scheduling state before any other thread runs. Under FIFO `wake_thread` (push_back), the woken peer would queue behind any other Ready thread; under no immediate yield, the rendezvous-completer would keep running and userland test code would observe non-deterministic interleavings.

**Phase 2 SMP replacement target.** Under SMP, "front of the ready queue" has no global meaning — each CPU has its own ready queue. Replacement options, none of which Phase 2 has decided yet:
- (a) Wake the peer on the same CPU as the rendezvous-completer; soft front-queue local; no IPI.
- (b) Wake the peer on its previously-running CPU via IPI; lose the synchronous "completer yields to peer" property.
- (c) Drop the ordering guarantee entirely and require explicit synchronization primitives (futex-style or barrier syscalls) where userspace cares.
The SMP ADR (deferred to Phase 2 BUILD-3) chooses one. Tests that depend on Invariant 1 either survive the choice or get rewritten to be timing-independent.

#### Phase 2 BUILD-1 amendment: consumer-first receive completions

BUILD-1 exposed an arrival-order gap in the original "blocked peer always runs first" wording. If the sender arrived first and blocked, then the receiver later completed the rendezvous, waking the blocked sender first let the sender observe the receiver as `Ready` before the receiver consumed the payload and entered its follow-on blocking syscall. Production boot surfaced this as `sys_kill` terminating `neo-worker` in the go-block window instead of returning `-EINVAL`.

The corrected single-CPU rule is **consumer-first**:

- If a sender/grantor completes a waiting receiver/recipient, the blocked consumer is front-queued and the completer yields immediately (`handoff_to_rendezvous_peer`).
- If a receiver/recipient completes a waiting sender/grantor, the producer is front-queued but the current consumer keeps running with a bounded handoff defer (`continue_current_after_receive`) until its next syscall entry or block.
- `sys_grant` / `sys_recv_grant` follow the same model: the recipient is the consumer, regardless of whether grantor or recipient arrived first.

This keeps the property tests actually depend on: the actor receiving data or authority reaches a stable post-rendezvous state before the producer can make follow-on observations. Phase 2 SMP must preserve or explicitly replace this consumer-first property, not just "wake the blocked peer."

---

### Invariant 2: The just-woken peer holds a bounded preempt-defer quantum

**Statement.** `wake_thread_front` sets the woken thread's `preempt_defer_ticks` to `HANDOFF_PREEMPT_DEFER_TICKS = 32` (defined in `kernel/src/sched/scheduler.rs`). When the timer interrupt's `on_tick` runs and finds `current.preempt_defer_ticks > 0`, it decrements the counter and returns WITHOUT yielding (`consume_preempt_defer`). The thread runs without timer-driven preemption for up to 32 ticks (~320ms at 100 Hz PIT).

**Rationale.** The BUILD-4 production-boot failure: even with `wake_thread_front`, a stale PIT tick could fire on the very first instruction the woken peer executes. Without the deferred quantum, the peer would be immediately preempted, returned to the back of the ready queue, and the rendezvous-completer (still in the ready queue from `yield_now`) would be picked next — defeating Invariant 1's "peer runs first" guarantee. The bounded quantum gives the woken peer enough cycles to make forward progress before the timer can take the CPU back.

The quantum is **bounded** (32 ticks, not unbounded): a thread that needs to be preempted for fairness MUST eventually be preempted. 32 ticks ≈ 320ms is enough to reach a syscall or block in any realistic Phase 1.5c workload but small enough that a runaway thread can't starve the system.

**Phase 2 SMP replacement target.** Under SMP, the failure mode this addresses (stale PIT tick on the same CPU as the just-woken thread) does not exist in the same form — each CPU has its own timer interrupt and its own scheduler state. The deferred quantum becomes either:
- (a) Removed entirely as SMP-irrelevant, with the SMP scheduler's same-CPU wake (Invariant 1 option a) providing the equivalent guarantee.
- (b) Repurposed as a per-CPU "this thread just got woken; give it priority for one quantum" scheduling hint, kept for fairness-shaping purposes orthogonal to the original IPC handoff bug.
Phase 2 SMP ADR decides. The constant `HANDOFF_PREEMPT_DEFER_TICKS = 32` itself is a tuning knob; the mechanism it gates is the load-bearing piece.

---

### Invariant 3: The deferred quantum is cleared on the next syscall entry

**Statement.** `syscall_dispatch` (in `kernel/src/arch/x86_64/syscall.rs`) calls `clear_current_preempt_defer()` immediately on entry, BEFORE matching on the syscall number. The just-woken peer's deferred quantum lasts exactly until the peer's next syscall entry, at which point preemption resumes normally.

**Rationale.** The deferred quantum exists to bridge the userland gap between rendezvous-resumption and the peer's next syscall (which is typically the follow-on blocking call). Past the syscall boundary, the peer is in kernel mode, IF=0, and the timer cannot preempt anyway; the quantum no longer protects anything. Holding the quantum past that point would mean a thread that does many short syscalls in a row stays preemption-deferred indefinitely, defeating the bounded-ness of Invariant 2.

The clear-on-entry is also what makes the kill-EINVAL determinism work: after the woken peer runs userland and reaches a syscall, the quantum is gone; the timer can preempt; the rendezvous-completer (still in the ready queue) gets scheduled and reaches its next blocking syscall, observing the correct system state.

**Phase 2 SMP replacement target.** If Invariant 2 is removed (per (a) above), Invariant 3 is removed with it — there is no quantum to clear. If Invariant 2 is repurposed (per (b) above), the clear-on-syscall-entry semantic stays as a property of the per-CPU scheduling-hint mechanism. The Phase 2 SMP ADR makes both decisions together.

---

### Invariant 4: The deferred quantum is cleared when a thread blocks

**Statement.** `rotate()` (the shared queue-rotation primitive in `kernel/src/sched/scheduler.rs`) sets `prev.preempt_defer_ticks = 0` when moving a thread to either the ready or zombie queue. A thread that blocks via `block_current_into` does not carry its deferred quantum into its eventual resumption — when it's later resumed via `wake_thread` (without `_front`) or `wake_thread_front` (with a fresh quantum), its quantum is determined by the wake path, not by what it had before blocking.

**Rationale.** A thread's deferred quantum reflects "you were just woken from a rendezvous and need protection until you reach a syscall." When the thread itself blocks again, that justification is gone — the next thread to run is whoever the new wake path designates, with whatever quantum that wake path grants. Carrying stale quantum across block/wake cycles would cause threads to accumulate preempt-defer credit improperly.

**Phase 2 SMP replacement target.** Same as Invariant 2 + 3: if the quantum mechanism is removed under SMP, Invariant 4 is removed; if repurposed, the clear-on-block semantic stays as a property of the scheduling hint.

---

## Tests that depend on these invariants

The Phase 1.5c default `cargo neo-test` suite has scenarios that depend on the invariants above. Each is an **anti-regression target** for the Phase 2 SMP retrofit:

- **`ipc_heap_kill_smoke`** — the BUILD-6 kill-on-Blocked-EINVAL determinism depends on Invariants 1, 2, and 3 jointly. Under SMP without an Invariant-1 replacement, this test becomes timing-dependent and must be either rewritten with explicit synchronization or deleted.

- **`expect_pass_but_fail`** — the meta-test that verifies the runner catches red tests is independent of these invariants but verifies that the suite as a whole stays honest. Critical anti-regression. Under SMP, the test runner itself may need adjustment (concurrent serial frame parsing, multi-CPU sentinel handling, etc.); the meta-test ensures red tests are still caught regardless of those runner changes.

- **(future) `cap_grant_*` tests from ADR-010** — the cap-grant rendezvous uses the same handoff mechanism. Tests that observe ordering between grantor and recipient depend on Invariants 1–3. Phase 2 BUILD-1 should add the `cap_grant_*` tests with explicit awareness of this dependency, so the SMP retrofit knows what to either preserve or rewrite.

---

## Phase 2 SMP migration checklist

When Phase 2 BUILD-3 (SMP foundation) lands, the engineer doing the work should walk this checklist invariant-by-invariant:

- [ ] **Invariant 1** — decide same-CPU-wake vs IPI-wake vs drop-ordering-guarantee. Document the choice in the SMP ADR. Update `wake_thread_front` and `handoff_to_rendezvous_peer` to match. Identify tests that depend on the original ordering and either preserve, rewrite, or delete.
- [ ] **Invariant 2** — decide remove vs repurpose. If removed, delete `HANDOFF_PREEMPT_DEFER_TICKS`, the `preempt_defer_ticks` field on `Thread`, and `consume_preempt_defer`. If repurposed, document the new semantics.
- [ ] **Invariant 3** — follow Invariant 2's decision. If removed, delete `clear_current_preempt_defer` and its caller in `syscall_dispatch`. If repurposed, audit the call site for SMP correctness.
- [ ] **Invariant 4** — follow Invariant 2's decision. Update `rotate()` accordingly.
- [ ] **Cap-grant tests** — review every `cap_grant_*` test added in Phase 2 BUILD-1 for SMP-correctness. Add explicit synchronization (futex, barrier, or equivalent Phase 2 primitive) where the test originally relied on Invariant 1's ordering.
- [ ] **kill-on-Blocked test** — same review for `ipc_heap_kill_smoke`. Likely needs explicit synchronization to remain deterministic under SMP.

This checklist is the deliverable shape this ADR is optimized for. SMP retrofitting is exactly the kind of work where "did we miss something?" is the dominant risk; an invariant-numbered list lets the engineer mechanically check each item.

---

## Cross-references

- **ADR-004** — base IPC design. ADR-011 amends it: the rendezvous handoff scheduling discipline is fully specified here, not in ADR-004's "Process-exit cleanup" or "User→user copy" sections.
- **ADR-007** — kill-on-Blocked test depends on Invariants 1–3.
- **ADR-010** — cap-grant rendezvous reuses `handoff_to_rendezvous_peer`. The Phase 2 SMP migration of these invariants is therefore **also** the Phase 2 SMP migration of cap-grant. Same retrofit, same time.
- **`docs/phases/PHASE-1.5c.md` Course Corrections** — the original record of the BUILD-2 and BUILD-4 failures that surfaced these invariants. ADR-011 is the design-ADR distillation of those entries.
- **`kernel/src/sched/scheduler.rs`** — `wake_thread_front`, `clear_current_preempt_defer`, `consume_preempt_defer`, `HANDOFF_PREEMPT_DEFER_TICKS`. The implementation surface.
- **`kernel/src/ipc/mod.rs`** — `handoff_to_rendezvous_peer`. The IPC consumer of the mechanism.

---

## Notes

This ADR is shorter than ADR-010 because it documents existing mechanics rather than proposing new ones. Its job is to make the load-bearing single-CPU constraints **findable** under their own ADR number. Future contributors searching for "what assumptions does the current IPC code make that I need to dismantle for SMP?" find ADR-011 as the canonical answer rather than archaeology through Phase 1.5c course corrections and inline comments.

The invariant-numbered structure is a deliberate departure from ADR-004/006/007/010's Q1/Q2/Q3 shape. Q1/Q2/Q3 is the right shape for a forward-looking design ADR ("which option do we pick"). Numbered invariants is the right shape for a retroactive constraints ADR ("what must Phase 2 dismantle"). Future retroactive ADRs (e.g., a Phase 2 SMP locking-discipline ADR) should follow this shape.

**Scope note for future cap operations:** Any future syscall that uses `handoff_to_rendezvous_peer` — `sys_grant` and `sys_recv_grant` from ADR-010 are the immediate examples; Phase 5 tensor-handle and model-handle cross-process operations will follow — inherits these invariants and their Phase 2 SMP migration. Single ADR, single retrofit, same time. Future ADRs introducing new rendezvous-using syscalls do NOT need to redocument these constraints; they cite ADR-011 and inherit. If Phase 5 surfaces a rendezvous-shaped operation that needs different scheduling discipline, that's a superseding ADR, not an extension here.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. The expected superseder is the Phase 2 SMP retrofit ADR that addresses each invariant in the migration checklist above.
