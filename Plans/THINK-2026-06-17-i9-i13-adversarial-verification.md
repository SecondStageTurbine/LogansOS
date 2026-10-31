<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: adversarial-verification log — the I-9/I-13 refcount+generation core under attack

**Date:** 2026-06-17
**Author:** Claude (PM), from a PL adversarial-test session
**Status:** VERIFICATION RECORD (falsifiable; file:line grounded) + one standing rule
**Invariants exercised:** I-9 (production tensors), I-13 (slot-arena ABA / generational keys), I-14 (subsystem fault containment) — `docs/complexity-governance.md`
**Related:** ADR-039 (migration reservation), ADR-040 (residency daemon), ADR-042 (slot arena), ADR-043 (ownership conversion / revoke-wins), ADR-045 (fenced DMA arena), ADR-050 (VT-d); companion `Plans/THINK-2026-06-17-vtd-bringup-window-deny-by-default.md`

---

## 0. Why this exists

In a PL adversarial-test session (2026-06-17) five independent attacks were thrown at the kernel's
concurrency / security / device-integrity core. Each was answered **by reading the live code, not memory.**
Three (A, B, D) bottom out on the *same* mechanism — the generational slot arena's refcount+generation
discipline (I-9/I-13), keyed per-tensor with a strict total lock order; the VT-d bring-up probe (C) is also
recorded in its own companion THINK because it produced a real forward-looking finding; and the accel-ring
probe (E) exercises a *different* core — the kernel-owned-producer + exact-value monotonic seqno-fence
discipline. This log banks all five as a falsifiable record: the attack, the verdict, the code that answers
it, and the residual. It also records the standing rules that fell out.

The shared lemma, stated once: **a use is authorized by minting a short-lived `Arc<Tensor>` through a
generation-checked gate (`arena::mint` = the `fget`/inc-not-zero analogue, `ai/arena.rs:263`). The mint
pins the object (strong ≥ 2) for exactly the operation's duration; a structural mutation (migrate / free /
revoke) bumps the slot generation so a stale key can never re-resolve (I-13), and the object's physical
storage drains only when the last in-flight `Arc` drops (the RCU-like grace period). The coordination is
per-tensor (`TENSOR_SLOT` lock level 15) and per-process (the handle-table `SwitchMutex`) — never a global
lock and never an IPI.**

---

## 1. Probe A — use-after-migration (residency daemon vs. in-flight inference)

**Attack.** A thread is doing inference on a tensor via a legitimate handle; the background residency
daemon decides to migrate that same tensor CPU→GPU to relieve pressure. How is a use-after-migration
prevented — guaranteeing no racing thread reads the *old* physical address after the kernel reallocated
that page — **without a global lock** that would erase the performance gains?

**Verdict: DEFENDED. The mechanism is exactly the requested lock-free discipline.** Three interlocking
guarantees, all per-tensor:

1. **Refcount-gated claim — migration is *refused* while a reader holds the tensor.** The in-flight op
   holds a strong `Arc` for the whole op (`ai/exec.rs:91`, *"the I-9 pin — strong ≥ 2 for exactly the pass
   duration; the daemon skips it as referenced"*). The daemon's `try_migrate` reads the counts under the
   slot lock and runs `decide_claim` (`ai/reservation_logic.rs`): `if strong != 1 || weak != 0 →
   SkippedReferenced`. So an active op (strong ≥ 2) makes the claim skip — the daemon **cannot start**
   migrating a tensor a thread is reading. Atomic count-read + decision, not a held lock.
2. **Park/commit rendezvous — a reader arriving *during* a claimed migration parks, then re-resolves to
   the new location.** If the daemon claimed (genuine quiescent window), it swaps the slot to
   `SlotState::Reserved { tensor_id }` and takes the only `Arc` (`reservation.rs::try_migrate` Phase 1, all
   under the slot lock). A racing `mint → guarded_clone` (`reservation.rs`) sees `Reserved` and parks
   atomically on the reservation's waiter list (`block_current_with_token_and_guard` — token installed
   under the guard, so the commit's wake can't be lost); on commit it re-resolves to the **migrated**
   tensor. The slot lock serializes the two outcomes — no interleaving observes `strong==1` *and* clones.
3. **Old-page lifetime is a consequence of the claim, not a lock.** Phase 2 copies old→new while owning the
   only reference (`Arc::get_mut(&mut tensor)` succeeds *because* strong==1/weak==0 — the lemma). The old
   storage's `Arc` drops (page returns to the allocator) only **after** the copy+commit. Since the claim
   required sole ownership, **no other thread ever had a pointer to the old storage** — `Arc::get_mut`
   succeeding is the type-system *proof* of no-readers. That proof is what replaces the global lock.

**ABA cover (I-13).** `lookup` (`ai/arena.rs:234`) checks `key_matches(entry.generation, key, …)` first;
`free` bumps the generation; the returned `SlotRef` is a clone of the snapshotted `ManagedSlot`, so a
racing free-then-realloc *"can only drain the snapshotted slot, never alias the new occupant"*
(`arena.rs:237`).

**Self-instrumented (falsifiable).** `CLAIMS_ADMITTED / CLAIMS_CONTENDED / CLAIMS_REFERENCED /
ACCESSOR_PARKS / MIGRATIONS_COMMITTED / DELTA_VIOLATIONS` (`reservation.rs`) make the property measurable
in a gate run.

**Residual.** Soundness even covers a *misbehaving* accessor: *"a container whose accessor skips this
protocol can still never produce a use-after-migrate — it can only make the gate see strong > 1 and the
claim skip"* — i.e. the failure mode of a bug is a **missed migration, not a UAF** (fails in the safe
direction). The actual op path (`exec.rs`) is compliant (holds the minted `Arc` across execution).

## 2. Probe B — capability revocation in SMP (the "stale handle" gap)

**Attack.** Even if the kernel updates the master capability table, a thread that already loaded the
capability into a register / stack local still holds authority until that instruction completes. How is
**instantaneous** revocation achieved across all cores without a stop-the-world IPI that would violate the
AI-native latency requirements?

**Verdict: DEFENDED — by recognizing the false premise.** Instantaneous mid-instruction revocation is
neither necessary nor achievable on real silicon (no cap system — seL4, Linux fd-close, CHERI — yanks
another core's register). The correct, achievable property is **monotone revocation**: (a) no *new*
authorization on any core after revoke, and (b) the one in-flight use completes safely and cannot be
leveraged to re-acquire or escalate. LogansOS provides exactly that:

- **(a) No new authority — capabilities are re-resolved per-use, never ambient.** Every syscall re-enters
  the per-process handle table under its lock: `proc.handles.lock().get(h)` with the USE-rights gate
  re-checked (`syscall.rs:862`). Revocation (`process::revoke_handle`) is a single-writer mutation under
  that **same** `SwitchMutex<HandleTable>`, plus a transitive cascade walking the derivation log
  (`BTreeMap<CapId, Option<CapId>>`) across **all** processes (a grant could have spread it), with
  monotonic-ProcessId lock ordering (no AB-BA). **The lock release *is* the cross-core happens-before**:
  release-store on revoke, acquire-load on the next `.get()` — scoped to exactly the threads that share the
  cap (same process), via a lock they already pay every syscall. No IPI; no global barrier; other
  processes untouched.
- **(b) Safe bounded drain — the same I-9/I-13 core.** `remove_cap_ids → release_tensor → arena::release`
  (`process.rs:673`) drops the *table's* logical reference and bumps the generation. An in-flight minted
  `Arc` keeps the object alive until the pass completes (no UAF); the generation bump makes the stale key
  un-re-resolvable (no re-acquire). The revoke-vs-resolve race has an explicit verdict — **"the revoke now
  wins"** (`syscall.rs:921-923`, ADR-043).

**Granularity (the honest answer to "stale register").** The stale capability is valid for **exactly one
operation — the one already resolved — then it's gone.** On the inference hot path that op is *short*: the
weight `Arc` is re-minted **per forward pass** (`exec.rs:91`) and the handle re-resolved **per syscall**
(`syscall.rs:862`), so revocation latency is **≤ one forward pass** (one token's compute), not a whole
generation — the decode loop (`generate.rs:61`) re-crosses the gate every token.

**Why no IPI is the *right* design, not a compromise.** The only cross-core coordination needed is the
per-process lock's release/acquire (already paid per syscall) plus the refcount drain. A stop-the-world IPI
would be both *unnecessary* (the refcount already guarantees no UAF) and a governance-latency violation
(I-2/I-14).

## 3. Probe C — the VT-d/IOMMU bring-up window

**Attack.** Between hardware power-on and the IOMMU page tables being constructed+enabled for a device's
domain, a malicious/buggy driver could DMA over kernel structures *before the fence exists.*

**Verdict: DEFENDED for the actual connectivity lane; one real forward-looking finding.** Full record in
the companion `Plans/THINK-2026-06-17-vtd-bringup-window-deny-by-default.md`. Summary:
- The Phase-7.5 connectivity drivers (MT7601U, CDC-ECM) are **USB devices, not bus masters** — they have
  no DMA engine; xHCI is the only master, and the **software DMA arena** (`dma_arena::fence`, BUILD-7.5-0)
  gates *address-minting* (not bus access), so it has **no bring-up window** and predates VT-d by design.
  Verified type-enforced down to the xHCI bootstrap: every controller DMA-pointer register
  (DCBAAP/CRCR/ERSTBA/ERDP) is programmed with a `fence_buffer()` `FencedDmaAddr`; the pointer-writers take
  `FencedDmaAddr` *by type*; arena-init is a hard precondition; Run/Stop is set last (§2C of the companion).
- **Real finding:** for a *future untrusted PCIe bus master* (the accel-driver class), `pci.rs:529` sets
  bus-master-enable unconditionally during probe, all before `iommu::init()` (`main.rs:449-460`), and VT-d
  enables identity-for-all with **no deny-by-default**. Harmless today (all masters are trusted TCB) but a
  live surface the instant an untrusted master is built. Fix banked: deny-by-default VT-d ordering + make
  BME consume proof-of-fencing + a CI grep-guard — a mandatory pre-condition ADR on that lane.

## 3b. Probe D — P4b daemon priority-inversion / swap-spike deadlock

**Attack.** Under a massive influx of inference requests (the P1 placement phase), can a priority inversion
deadlock form? If the P4b residency daemon (the kswapd-model background demote/prefetch) is starved by the
volume of incoming interrupts/scheduling, and the primary allocator (P2) hits a hard limit and falls back
to synchronous blocking reclaim, can the system wedge — the work thread blocked on pressure that only the
starved daemon can clear? And can the swapping thread and the work thread deadlock on the same
`TENSOR_SLOT` lock?

**Verdict: DEFENDED — the deadlock requires a work→daemon dependency that does not exist.** This is the
Linux kswapd/direct-reclaim split, built on purpose:

- **No wait-for edge work→daemon.** Reactive reclaim runs **inline in the allocating thread's own context**
  — the executor *"carries the whole policy arc per op — placement (§9.9), eviction under pressure
  (§9.10)"* (`ai/exec.rs`). When a placement under pressure hits the limit, the work thread evicts a victim
  itself and proceeds; it never enqueues work for the daemon and blocks. The P4b daemon is **strictly
  additive** (`residency_daemon.rs:13-14`): *"the synchronous P2/P3 placement path is the always-complete
  direct-reclaim backstop… dormant is bit-for-bit P3; awake, it can only make placements cheaper, never
  make one fail."* `kick()` is fire-and-forget (`residency_daemon.rs:265`: *"Non-blocking; the synchronous
  backstop never waits on it"*). So starving the daemon to zero CPU degrades **throughput**, never
  **liveness** — the I-14(b) guarantee (degrade to the synchronous backstop; correctness preserved,
  governance only slows).
- **No `TENSOR_SLOT` deadlock**, four structural reasons: (1) **strict total lock order**, lockdep-asserted
  (`sync/mod.rs`: `ADMISSION_QUEUES=10 < ARENA=12 < TENSOR_SLOT=15 < RESERVATION=20 < ACCT=28`) ⇒ no AB-BA
  cycle; (2) **the slot lock is never held across a block** — `try_migrate` holds it only for the brief
  claim/commit, `guarded_clone` drops it before parking (Probe A) ⇒ no hold-and-wait; (3) **contention
  resolves by skip, not block** — `try_migrate` is claim-or-skip (`SkippedReferenced`/`SkippedContended`
  return immediately; the daemon *"never blocks, never touches what a pass holds"*, `residency_daemon.rs:11`);
  (4) **the spinlock priority-inversion variant was hit and fixed** — `SwitchMutex` is a spin mutex
  (`switch_mutex.rs:2,45`), so the one real hazard is spinning IRQ-off on a preempted holder's lock;
  `guarded_clone`'s "gate run 4" comment records catching exactly this, and the design rule (don't spin
  IRQ-off on the slot lock) keeps a preempted holder reschedulable ⇒ bounded spin latency, never deadlock.

**Residual — measured, not assumed.** Under sustained pressure where the daemon never runs, all reclaim
falls to the inline backstop and inference latency rises to the backstop-only cost. The design treats this
as a falsifiable number: the latency oracle's baseline phase *"pauses the daemon to measure the
backstop-only cost"* (`residency_daemon.rs:70`). The priority-inversion worst case is the explicitly
measured floor of the latency discipline, not a hope.

## 3c. Probe E — descriptor-shadowing / ring-buffer desync (accel doorbell race)

**Attack.** The accel driver manages a ring buffer for hardware (e.g. Intel UHD 630): the kernel owns
head/tail, the device reports progress via DMA writeback/IRQ. In a high-frequency stream there is a window
between writing a descriptor and updating the doorbell/tail register. If an interrupt for a *previous*
command arrives during that write cycle, can a descriptor-shadowing error occur — could a malformed or
adversarial device trick the kernel into believing a descriptor completed before it was committed,
advancing the tail over uninitialized/stale command space?

**Verdict: DEFENDED, fail-safe — the attack mis-models who owns the tail.** This probe exercises a
*different* core than A–D: the **kernel-owned-producer + exact-value monotonic seqno-fence** discipline.

- **The tail/doorbell is kernel-owned producer state; the device cannot move it.** `submit_compute`
  (`accel_intel.rs`) writes the descriptor into the GGTT-mapped ring first, then rings the doorbell
  (`write32("ring-tail-write", RING_TAIL, command_len)`). The device only *reads* tail. "Advancing tail
  over uninitialized space" would be a kernel producer bug, not a device-inducible state.
- **Completion is gated on a self-planted exact-value seqno, never a device pointer.** The decisive line
  (`IntelGpuInner::poll`): `let seqno = read_volatile(pending.seqno.as_ptr()); if seqno != token.sequence {
  return Pending }`. The kernel plants `MI_STORE_DWORD_IMM(seqno_page, token.sequence)` as the batch's last
  command; the device writes that value only after executing everything before it; the kernel accepts
  `Complete` only on reading back *exactly* `token.sequence`. `RING_HEAD`/`RING_TAIL` are read for
  diagnostics only — they play no part in the completion decision. A device that doesn't run the batch, or
  writes a wrong value, stays `Pending` → `timeout`. **A `Complete` verdict is proof the device executed the
  descriptor; it cannot be forged via head/tail.** Worst case for a malformed/adversarial device is
  non-completion, never a false positive.
- **The interrupt-mid-write is harmless** because the completion decision is **level-triggered and
  idempotent**: poll or IRQ, the handler reads the seqno page and compares to `token.sequence`. There is no
  edge-state to consume, and the seqno page (device-written) does not alias the tail register
  (kernel-written) — no shared cell to tear.
- **Ordering + isolation:** the pre-doorbell MMIO read (`read_gt_status`) drains the posted-write buffer on
  x86 UC mapping (ADR-025 typed MMIO), so the descriptor is visible before the doorbell; `token.sequence`
  is monotonic per submission, so a *stale* seqno writeback (literal descriptor-shadowing) reads
  `!= token.sequence` → `Pending` (replay defeated); the ring + seqno page are kernel GGTT, tenant-isolated
  by per-tenant PPGTT (BUILD-4X) and arena/VT-d-bounded DMA (I-14).

**Residual.** Today's path is **single-in-flight bring-up** (`self.in_flight = Some(pending)`; a second
submit returns `InvalidRequest`). The attack's pipelined high-frequency multi-descriptor ring is not built
yet. The seqno-fence principle generalizes, but the pipelined build must observe the standing rule in §4.2.

---

## 4. The standing rules that fell out

### 4.1 Long-running operations re-validate their capability at internal boundaries (from Probes A/B)

> **Long-running operations must re-validate their capability at internal boundaries.** A use is
> authorized by a generation-checked `mint`; an op that resolves a cap *once* and then runs a long internal
> loop *without* re-minting extends its revocation latency (Probe B) and its migration-skip window (Probe
> A) to the op's entire runtime. The inference path already satisfies this for free — it re-mints per
> forward pass (`exec.rs:91`) and re-resolves per syscall (`syscall.rs:862`), so its revocation/migration
> granularity is one token, not one generation. Any **new** long-running kernel op over governed tensors
> (a future fused multi-step kernel, an in-kernel generation syscall that loops internally, a long
> accel-batch) MUST re-validate per step the same way, or document why its op-granular latency is
> acceptable. For *memory safety* the refcount covers it regardless; this rule is about *authority and
> revocation latency*.

This rule is cheap to honor (it is the existing `mint` discipline applied per-step) and converts an
implicit property of today's code into an explicit gate for tomorrow's.

### 4.2 Pipelined accel-ring slot reclaim is gated by the per-slot seqno fence, never the device HEAD (from Probe E)

> When the accel ring goes from single-in-flight to **pipelined multi-descriptor** submission, a producer
> slot may be reclaimed/recycled **only** once its own planted seqno fence has fired (`seqno ==
> token.sequence` for that slot), **never** merely because the device's `RING_HEAD` register has advanced
> past it. Trusting the device-advanced head to free slots is exactly the "advance over stale/uninitialized
> space" hole the descriptor-shadowing attack imagines; gating reclaim on the per-slot seqno fence keeps
> the "validate, don't trust" property under pipelining. The completion *principle* already exists
> (`accel_intel.rs` `poll`: exact-value compare); this rule binds the *reclaim* path to it when that build
> lands. Monotonic per-submission seqnos (already the case) additionally defeat stale-writeback replay.

## 5. Net

All four probes bottom out on the same small, falsifiable core: **a generation-checked refcount gate (the
claim-or-skip `mint`), keyed per-tensor and per-process, under a strict total lock order, with no global
lock and no IPI.** Use-after-migration (A), SMP capability revocation (B), connectivity DMA containment
(C), and the P4b priority-inversion/swap-spike (D) are the *same mechanism* viewed from four angles — the
claim-or-skip refcount gate, the strict lock order (`ARENA < TENSOR_SLOT < RESERVATION < ACCT`), the
never-hold-a-lock-across-a-block rule, and the additive-daemon/synchronous-backstop split. The accel-ring
probe (E) is a *second* small core — kernel-owned producer state + an exact-value monotonic seqno fence,
where a `Complete` verdict is proof of execution and a misbehaving device can only time out. Across both
cores the pattern is identical: **validate against something the kernel itself planted/owns; never trust a
device- or caller-supplied pointer.** That is exactly the legibility property Rule-3/Rule-4 governance is
meant to produce: **two cores to verify, not five.** Forward work surfaced, all banked and non-blocking:
the VT-d deny-by-default pre-condition (Probe C, companion THINK), the per-step cap re-validation rule
(§4.1), and the pipelined-ring seqno-gated-reclaim rule (§4.2). Probes A, D, and E surfaced no live gap —
their worst cases are a missed migration (A), a measured latency floor (D), and a device timeout (E),
each failing in the safe direction.
