# DISPATCH — SC-3C PLAN-stage: Q7.b CR3 cache + Q7.c IPI TLB shootdowns (Jarvis)

**Date filed:** 2026-05-11
**From:** PM
**To:** Jarvis (Codex, kernel coding lane per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1)
**Ferry:** PL ferries to Jarvis session.
**Status:** PLAN-stage dispatch for Phase 2 BUILD-3 sub-commit 4 ("SC-3C"). Three sub-commits per D12: 4a (Q7.b CR3 cache) → 4b (Q7.c shootdown machinery, no production callers) → 4c (Q7.c call-site wiring). This is **Jarvis calibration dispatch 3 of 3** under the new agent-roster assignment per `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §3.1.

---

## 1. Context

SC-3C completes Phase 2 Q7 by addressing the two remaining sub-questions left after Q7.a (per-CPU Pmap binding) landed earlier:

- **Q7.b — CR3 cache.** Avoid redundant `Cr3::write` calls in the scheduler context-switch path. The per-CPU substrate exists at `kernel/src/arch/x86_64/per_cpu.rs:33` (`active_pmap_cr3: AtomicU64`) but is unused — Q7.b wires it.
- **Q7.c — IPI TLB shootdowns.** Add cross-CPU TLB invalidation via IPI. Currently the kernel has no cross-CPU TLB invalidation; safe under single-process SMP because all user code shares one CR3-loaded address space, but a correctness gate for any cross-process SMP work that follows (Q8.a/b/c sub-commits 5 and 6 depend on this).

Full design context lives in `Plans/SC-3C-THINK-2026-05-10.md`. **All fourteen design decisions (D1–D14) have been ratified.** Four (D3, D4, D9, D14) were provisionally ratified pending Cerebro confirmation and finalized 2026-05-11 by independent primary-source verification per `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`. The Cerebro deep-research artifact (`Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`) is the canonical prior-art reference; cite that artifact (not the superseded PM-preliminary) when prior-art evidence is invoked.

**Substrate baseline at dispatch time:** HEAD `origin/main` = `27d1b99` (kernel-rename bundle fully closed; sub-commits 1+2+3 landed). `IrqSafeMutex` is **available** in `kernel/src/sync/irqsafe_mutex.rs`; per D13 the bridge fallback to `without_interrupts(...)` is no longer needed. Sender-side shootdown coordination state should use `IrqSafeMutex` where mixed-context locking is required.

---

## 2. Inputs to consume before BUILD

Read in this order:

1. **`Plans/SC-3C-THINK-2026-05-10.md`** — full design space, D-table at §6, sub-commit boundary rationale at §4.6 and §5 R-X1. This is the canonical design document; this dispatch references it heavily rather than restating.
2. **`Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md`** — four FINALIZED D-revisions (D3 N=16→N=33, D4 API-takes-target-set + T2 upgrade, D9 FreeBSD overlap pattern, D14 missing-shootdown-detection scenario).
3. **`Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md`** — canonical prior-art reference. **Use this artifact (not PM-preliminary) for any citation of Linux/FreeBSD/seL4/Asterinas/AMD-INVLPGB behavior.**
4. **`Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md`** — seven citation/factual corrections to PM-preliminary; one Phase 2+ forward marker; the strictly-accurate-vs-loosely-accurate T2 cpumask PLAN-documentation requirement (see §6 below).
5. **`Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md`** — calibration patterns from dispatch 1 of 3. Strong stop discipline expected. Branching convention: agent-named branch off `main`.
6. **`Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md`** — perf-gate threshold environment-anchoring discipline. **Critical for SC-3C:** any perf gate added must be environment-anchored as a regression detector under QEMU TCG debug, not a theoretical native-hardware target.
7. **Substrate files at HEAD `27d1b99`** (Jarvis reads on its own):
   - `kernel/src/arch/x86_64/per_cpu.rs` (`active_pmap_cr3` field at line 33)
   - `kernel/src/arch/x86_64/apic.rs` (IPI infrastructure; pre-wired `APIC_ICR_LOW`/`APIC_ICR_HIGH`/`ICR_LEVEL_ASSERT` constants with `#[allow(dead_code)] // first production caller lands with Q7 TLB shootdowns`)
   - `kernel/src/arch/x86_64/idt.rs` (IDT handler hookup)
   - `kernel/src/arch/x86_64/mod.rs:142-149` (`flush_local_tlb()` existing primitive)
   - `kernel/src/sched/scheduler.rs` (context-switch flow; ~700 LOC)
   - `kernel/src/sched/process.rs:339-343` (`cr3_for_thread(pid)`)
   - `kernel/src/mm/pmap.rs` (Pmap; `unmap_page`, `map_page`; PCID-deferral comment at line 320)
   - `kernel/src/sync/irqsafe_mutex.rs` (now available; use for sender-side state)
   - `kernel/src/test_harness.rs` (scenario harness; SC-3B closure-gate machinery)

---

## 3. D-decisions JArvis must honor

Quick-reference summary. **All ratified per FINALIZED D-revisions.** Where Cerebro corrected PM-preliminary, the corrected fact is what to apply.

| # | Decision | Apply as |
|---|---|---|
| D1 | Q7.b is B2.a — compare-and-skip with CR3-write-then-field-store ordering | Sub-commit 4a |
| D2 | PCID deferred to future sub-commit | Comment in `pmap.rs:320` may be extended; no code change |
| D3 | Q7.c granularity G3 hybrid, **N=33** (Linux measured threshold) | Sub-commit 4b INVLPG-vs-full-flush threshold constant |
| D4 | Q7.c target selection: T1 broadcast for SC-3C BUILD; **API takes target-set as input parameter**; T2 (per-mm cpumask) is Phase 2+ forward optimization | Sub-commit 4b API design |
| D5 | Q7.c synchrony: S3 synchronous with 1ms bounded wait + panic-on-timeout | Sub-commit 4b sender API |
| D6 | Lazy TLB: out | No code change; flag as forward marker in `tlb_shootdown.rs` comment |
| D7 | Shootdown batching: out | No code change; flag as forward marker in `tlb_shootdown.rs` comment |
| D8 | IPI vector for shootdown: `0xfc` | Sub-commit 4b |
| D9 | Self-invalidation runs **synchronously, after IPI dispatch and before the scoreboard wait loop** (the FreeBSD `curcpu_cb` pattern, per Cerebro-corrected mechanical sequencing) | Sub-commit 4b sender API |
| D10 | ACK mechanism: per-shootdown `pending: AtomicU32`; sender spins | Sub-commit 4b |
| D11 | Per-CPU shootdown slot: α (single slot, `AtomicPtr<ShootdownPayload>`) | Sub-commit 4b |
| D12 | Sub-commit order: 4a → 4b → 4c | This dispatch |
| D13 | `IrqSafeMutex` is **available** (landed at `27d1b99`); use directly for sender-side state | Sub-commit 4b |
| D14 | Test gate scenarios: SC-3B closure-gate discipline (200/200 NEO_SMP=4) + **missing-shootdown-detection scenario per FreeBSD-EN-18:07** | Sub-commit 4c verification |

**Cerebro corrections to keep in mind (citation precision for code comments and verification reports):**

- Linux's per-mm `mm_cpumask` is **eventually-consistent**, not strictly-accurate. `switch_mm_irqs_off` adds the incoming CPU's bit but never removes the outgoing CPU's bit; stale bits are scrubbed lazily by IPI receivers via `flush_tlb_func`. Relevant for Phase 2+ T2 PLAN-documentation requirement (§6 below).
- FreeBSD API signature is **`smp_targeted_tlb_shootdown_native(pmap_t, addr1, addr2, curcpu_cb, enum invl_op_codes op)`** — target-set derived from `pmap` internally; no explicit `cpuset_t mask` or `u_int vector` parameters. D4's API design takes target-set as input parameter is **structurally different** from FreeBSD's pmap-derived approach but **architecturally equivalent** (both factor target-set derivation out of every caller). Code comments should describe the API as "target-set passed as input parameter; derivation logic lives in the caller" — do not claim it mimics the FreeBSD signature.
- The `curcpu_cb` runs **after IPI dispatch and before the scoreboard wait loop** (not "during the wait loop" as PM-preliminary said). D9's self-invalidation timing follows this corrected sequencing.
- The Linux function name is **`broadcast_tlb_flush`**, not `broadcast_flush_tlb_all`. If code comments cite Linux INVLPGB integration as forward-marker context, use the correct function name.
- seL4 mailing list note date is **November 2018** if cited.
- FreeBSD-EN-18:07 verbatim wording is **"resulted in"** (not "resulting in") if quoted directly.

---

## 4. Sub-commit 4a — Q7.b CR3 cache (compare-and-skip)

### 4a.1 Scope

Wire the existing `per_cpu.active_pmap_cr3` field. The scheduler's context-switch path currently calls `Cr3::write(target_cr3)` unconditionally; change it to compare-and-skip per D1 / B2.a.

### 4a.2 Files to modify

- `kernel/src/arch/x86_64/per_cpu.rs` — may add a helper method on the per-CPU state for encoding/decoding the cached CR3 value if cleaner than open-coding at the call site. The field itself already exists; only access patterns are added.
- `kernel/src/sched/scheduler.rs` — the actual compare-and-skip logic. Identify the call site(s) where `Cr3::write` is currently invoked unconditionally and gate it on `target != active`.
- Possibly `kernel/src/sched/process.rs` — if `cr3_for_thread(pid)` is the natural point at which the encoded `u64` target is constructed, the encoding helper may live nearby. Jarvis's discretion on factoring.

**Do not touch:** `kernel/src/mm/pmap.rs` (PCID deferral comment may be extended for forward-marker tracking but is otherwise unchanged), `kernel/src/arch/x86_64/apic.rs`, `kernel/src/arch/x86_64/idt.rs`. Sub-commit 4a is scheduler-local.

### 4a.3 Behavioral specification

Implement per THINK §3.2 pseudocode pattern (`B2.a` ordering: CR3 write first, field store second; atomic ordering `Relaxed` for per-CPU field). Concrete invariants:

- Encoding `(frame, flags) → u64`: the encoded value must be reversible and unique per `(frame, flags)` pair. Jarvis's discretion on encoding scheme; recommend `frame.start_address().as_u64() | flags.bits() as u64` if no high bits collide (verify with the `x86_64` crate's Cr3Flags representation), else a tagged encoding.
- Compare-and-skip happens on **every** context-switch path that loads CR3 (including kernel-thread → user-thread, user-thread → kernel-thread, and user-thread-A → user-thread-B same-process). The win is dominantly on same-process context switches; cross-process switches always write CR3.
- If the encoded value is `0` (initial state, no Pmap yet loaded on this CPU), treat as cache-miss and write unconditionally. This handles AP bring-up cleanly.
- The post-write `store` of the new encoded value uses `Ordering::Relaxed`. The cross-CPU read concern flagged in THINK §3.3 R-B4 does not apply to sub-commit 4a (T3 active-only shootdown is deferred to Phase 2+); `Relaxed` is correct for SC-3C scope.

### 4a.4 Verification gates for sub-commit 4a

All must PASS before commit:

1. `cargo k` — kernel build with default features.
2. `cargo k --no-default-features` — kernel build with minimal features.
3. `cargo neo-test all` — full scenario suite must not regress.
4. **New scenario:** `cargo neo-test q7b_cr3_cache_skip_count` — synthetic same-process context-switch workload that verifies the cache hit rate is non-zero. Implementation: instrument the scheduler with a per-CPU counter (`cr3_writes_skipped: AtomicU64` and `cr3_writes_performed: AtomicU64`) on a feature flag; the scenario runs a same-process tight context-switch loop and asserts `skipped > performed * threshold_ratio`. Threshold ratio: `>= 0.5` (at least 50% of context switches in the same-process loop skip the CR3 write). **This is a regression detector, not an absolute-performance gate** — anchored to behavior under QEMU TCG debug, not theoretical native hardware. The counters may be `#[cfg(feature = "neo_test_instrumentation")]` gated so they don't ship in release builds.
5. **Existing scenario set:** the SC-3B closure-gate equivalent — `200/200 NEO_SMP=4` boot-stress must continue to pass. Run this **after** sub-commits 4b and 4c land (it's expensive to run between every sub-commit); for 4a alone, the standard `cargo neo-test all` is sufficient.

### 4a.5 Commit message (sub-commit 4a)

Suggested title:

```
kernel/sched: cache active CR3 per CPU; skip redundant CR3 writes (Q7.b)
```

Suggested body (substance; Jarvis may revise wording):

```
Wires the existing per_cpu.active_pmap_cr3 field (substrate at per_cpu.rs:33,
unused since AP bring-up landed). The scheduler context-switch path now
compares the target CR3 encoding against the per-CPU cached value and skips
the Cr3::write when they match.

Ordering follows SC-3C D1 / B2.a (THINK artifact §3.2): CR3 write first,
field store second. If an interrupt fires between the write and the store,
the next context-switch sees stale-low and writes CR3 redundantly — one
extra write, no correctness loss.

PCID-aware CR3 caching (D2) is deferred to a future ADR-driven sub-commit
post-measurement.

New scenario q7b_cr3_cache_skip_count instruments the scheduler under
neo_test_instrumentation feature and asserts the same-process context-
switch loop achieves >= 50% cache-hit rate. Threshold is an environment-
anchored regression detector under QEMU TCG debug per the perf-gate
discipline in Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-
clarification.md.

See Plans/SC-3C-THINK-2026-05-10.md §3, D1.
```

### 4a.6 Stop conditions for sub-commit 4a

- Any verification gate fails → STOP, file failure report, surface to PM. Do not commit.
- Scheduler test scenarios show ANY regression (existing passing scenario newly fails) → STOP, file failure report. The B2.a ordering should be a pure performance win; any correctness regression means the change is wrong.
- Cache hit rate in `q7b_cr3_cache_skip_count` is below 50% → file investigation note before committing. Either the threshold is wrong for the QEMU TCG environment (in which case it's a perf-gate-environment issue per the ADR-019 lesson and PM resolves), or the implementation has a bug that's causing cache misses on legitimately-equal targets (in which case the implementation is wrong).

---

## 5. Sub-commit 4b — Q7.c TLB shootdown machinery (no production callers)

### 5b.1 Scope

Lands the full TLB shootdown machinery as a standalone, testable subsystem. **No production callers** at the end of this sub-commit — `pmap.rs` mutation sites still don't invoke shootdown. The intent is to land the machinery, gate it with a synthetic test, then wire call sites in sub-commit 4c with confidence the underlying machinery is correct.

### 5b.2 Files to create

- **`kernel/src/mm/tlb_shootdown.rs` (NEW)** — the shootdown machinery: payload struct, sender API, receiver logic, per-CPU shootdown slot, the strictly-vs-loosely-accurate T2 forward-marker documentation block (§6 below).

### 5b.3 Files to modify

- `kernel/src/arch/x86_64/apic.rs` — add `send_tlb_shootdown_ipi(target_cpu)` function using the pre-wired `APIC_ICR_LOW`/`APIC_ICR_HIGH`/`ICR_LEVEL_ASSERT` constants (the `#[allow(dead_code)]` markers come off these constants as part of this sub-commit, since they now have production callers).
- `kernel/src/arch/x86_64/idt.rs` — register the vector-0xfc IDT entry pointing at the shootdown handler.
- `kernel/src/arch/x86_64/mod.rs` — add the `invlpg(virt_addr: VirtAddr)` inline helper. Use the asm pattern from THINK §4.3 with `nostack, preserves_flags` options and a SAFETY comment explaining the CPL=0 + canonical-address precondition.
- `kernel/src/arch/x86_64/per_cpu.rs` — add the `pending_shootdown: AtomicPtr<ShootdownPayload>` field per D11.
- `kernel/src/mm/mod.rs` — `pub mod tlb_shootdown;` if not already a re-exports module.
- `kernel/src/test_harness.rs` — new scenario `q7c_shootdown_synthetic_round_trip` (see §5b.5 below).

### 5b.4 API design (per D4 ratification + Cerebro corrections)

**Sender API:**

```rust
/// Invalidate `range` on the target CPU set.
///
/// `range == None` requests a full local TLB flush on each target.
/// `range == Some((start, end))` requests INVLPG for each page in the range
/// if the page count is <= INVLPG_THRESHOLD (33, per Linux measured threshold;
/// see SC-3C D3 ratification), else a full local TLB flush on each target.
///
/// `targets` is the explicit CPU bitmask of receivers. For SC-3C, callers pass
/// `other_cpus_mask()` (broadcast to all CPUs except self) per D4 / T1; future
/// Phase 2+ T2 work will derive a narrower target-set from per-mm cpumask
/// state and pass it here.
///
/// Self-invalidation runs synchronously after IPI dispatch and before the
/// scoreboard wait loop, per SC-3C D9 (FreeBSD curcpu_cb pattern). Returns
/// when all targets ACK or when the 1ms bounded wait expires (panic on
/// timeout per D5).
pub fn shootdown(range: Option<(VirtAddr, VirtAddr)>, targets: CpuMask);
```

The `CpuMask` type is Jarvis's discretion — a newtype around `u64` is sufficient for SC-3C; cleaner than a raw `u64` at the API boundary.

**Receiver:** the IDT handler at vector 0xfc dispatches to a function in `tlb_shootdown.rs` that consumes the per-CPU pending payload, invalidates per the payload's range, and ACKs by decrementing `pending`.

**Memory ordering on the payload:**

- `pending.fetch_sub(1, Ordering::Release)` on the receiver side (the receiver's invalidation work must be observable before the ACK).
- `pending.load(Ordering::Acquire)` on the sender side (sender must see the receivers' work effects when ACK is observed).
- `targets` and `range` fields on the payload are written by the sender once before publishing; receivers read them after the per-CPU slot is populated. The `AtomicPtr` publish-via-store on the per-CPU slot provides the synchronization edge; the payload fields themselves can be plain (non-atomic) reads on the receiver after the AtomicPtr read returns non-null.

### 5b.5 Verification gates for sub-commit 4b

All must PASS before commit:

1. `cargo k` — kernel build with default features.
2. `cargo k --no-default-features` — kernel build with minimal features.
3. `cargo neo-test all` — full scenario suite must not regress.
4. **New scenario:** `cargo neo-test q7c_shootdown_synthetic_round_trip` — a synthetic test that, from CPU 0, invokes `shootdown(None, other_cpus_mask())` and verifies (a) all CPUs in `other_cpus_mask()` receive the IPI (per-CPU instrumentation counters), (b) the sender returns within the 1ms bounded wait, (c) the local TLB on CPU 0 is also invalidated (self-invalidation per D9), (d) no panic occurs. Variants:
   - 4a-1: `range = None` (full flush on each target).
   - 4a-2: `range = Some((va, va + 4*4096))` (4-page INVLPG range; below threshold).
   - 4a-3: `range = Some((va, va + 64*4096))` (64-page range; above N=33 threshold; falls back to full flush per D3).
5. **New scenario:** `cargo neo-test q7c_shootdown_timeout_panic` — a synthetic test that simulates a stuck receiver (one CPU is forced into a long IF=0 critical section exceeding 1ms before its handler can run) and verifies the sender panics with the diagnostic per D5. This is a **panic-expected** scenario; the test harness's expect-panic mechanism wraps it (same mechanism that handles the `expect_pass_but_fail` meta-test that confirmed harness behavior in `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md`).

**No `200/200 NEO_SMP=4` run for sub-commit 4b in isolation.** That stress runs after sub-commit 4c lands (the production wiring); running it on 4b without callers wouldn't exercise the shootdown path.

### 5b.6 Commit message (sub-commit 4b)

Suggested title:

```
kernel/mm: introduce TLB shootdown machinery (Q7.c machinery; no callers)
```

Suggested body (substance):

```
Introduces the cross-CPU TLB invalidation machinery per SC-3C D3/D4/D5/D8/
D9/D10/D11. No production callers at this commit; pmap.rs mutation sites
still don't invoke shootdown. Sub-commit 4c wires the call sites.

Components:
- mm/tlb_shootdown.rs: ShootdownPayload struct (range + targets + pending
  AtomicU32 ACK counter), sender API shootdown(range, targets), receiver
  logic dispatched from vector 0xfc IDT entry.
- arch/x86_64/apic.rs: send_tlb_shootdown_ipi(target_cpu) using the pre-
  wired APIC_ICR_LOW/APIC_ICR_HIGH/ICR_LEVEL_ASSERT constants.
- arch/x86_64/idt.rs: vector 0xfc handler registration.
- arch/x86_64/mod.rs: invlpg(virt_addr) inline helper (first INVLPG
  callsite in the kernel).
- arch/x86_64/per_cpu.rs: pending_shootdown AtomicPtr field.

Design ratified in Plans/SC-3C-THINK-2026-05-10.md and finalized in
Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md:
- D3: G3 hybrid; INVLPG if <= 33 pages (Linux measured threshold), else
  full local flush per target.
- D4: T1 broadcast for SC-3C BUILD; API takes target-set as input
  parameter. T2 (per-mm cpumask) is Phase 2+ forward optimization.
- D5: synchronous with 1ms bounded wait; panic on timeout.
- D8: vector 0xfc (adjacent to reschedule IPI 0xfd).
- D9: self-invalidation synchronously after IPI dispatch and before
  scoreboard wait (the FreeBSD curcpu_cb pattern; mechanical sequencing
  corrected per Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-
  and-d-revisions-finalized.md §3.3).
- D10: per-shootdown AtomicU32 ACK counter; sender spins.
- D11: per-CPU single-slot AtomicPtr<ShootdownPayload>.

Sender-side coordination state uses IrqSafeMutex (landed at 27d1b99);
the D13 fallback to without_interrupts(...) is unused.

New scenarios q7c_shootdown_synthetic_round_trip (variants for None range,
small INVLPG range, large-falls-back-to-full range) and q7c_shootdown_
timeout_panic verify the machinery in isolation. 200/200 NEO_SMP=4
stress runs at sub-commit 4c after production wiring lands.

Phase 2+ forward markers documented inline in tlb_shootdown.rs:
- Strictly-accurate vs loosely-accurate per-mm cpumask design choice for
  T2 migration (Cerebro-corrected understanding of Linux's eventually-
  consistent mm_cpumask).
- Lazy-TLB page-table-teardown exception (Linux pattern: should_flush_
  tlb bypass when freed_tables=true).
- Linux global-ASID promotion's sampling-gate-plus-rising-floor pattern
  for future INVLPGB/RAR work (Cerebro-corrected understanding;
  consider_global_asid is statistical not deterministic).

See:
- Plans/SC-3C-THINK-2026-05-10.md
- Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md (FINALIZED)
- Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md
- Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-
  finalized.md
```

### 5b.7 Stop conditions for sub-commit 4b

- Any verification gate fails → STOP, file failure report, surface to PM.
- `q7c_shootdown_synthetic_round_trip` shows any target CPU not receiving the IPI → STOP. This is a correctness gate; missing IPI delivery is the precise failure mode FreeBSD-EN-18:07 describes.
- `q7c_shootdown_timeout_panic` shows the sender hangs past the 1ms bound without panicking → STOP. The bounded wait is a load-bearing safety property of D5.

---

## 6. Sub-commit 4c — Q7.c call-site wiring + D14 verification

### 6c.1 Scope

Wires the shootdown machinery into `pmap.rs` (and any other cross-CPU-visible page-table mutation sites identified during BUILD). Sub-commit 4c is where SC-3C becomes correctness-load-bearing — the missing-shootdown-detection scenario (D14, per FreeBSD-EN-18:07) gates this commit.

### 6c.2 Files to modify

- `kernel/src/mm/pmap.rs` — `unmap_page` (primary call site), `map_page` (only if the existing mapping is replacing a non-zero PTE — verify during BUILD), any other Pmap-mutating method that may affect cross-CPU-visible state.
- `kernel/src/mm/paging.rs` — if kernel-half mapping changes can affect cross-CPU TLBs (heap regions, MMIO regions). Likely lower-priority than user-half but verify.
- `kernel/src/mm/heap.rs` — only if the heap implementation mutates page tables in ways visible to other CPUs after AP bring-up (ADR-015 KASLR-tolerant heap may or may not be quiescent post-init; verify).
- `kernel/src/test_harness.rs` — D14 verification scenarios (see §6c.4).

### 6c.3 Wiring pattern

At each cross-CPU-visible page-table mutation site:

1. Perform the mutation (write the PTE).
2. Issue a memory barrier (`core::sync::atomic::fence(Ordering::SeqCst)` or equivalent — the goal is to ensure the PTE write is visible to other CPUs before the shootdown IPI fires).
3. Call `tlb_shootdown::shootdown(Some((start, end)), other_cpus_mask())` for the affected virtual address range.

For ranges affecting only one page, pass `Some((va, va + 4096))`. For full-Pmap teardown, pass `None` (full local flush on each target).

**Memory-ordering note for Jarvis to verify during BUILD:** x86_64 has strong total-store-order semantics for writes, so a plain `mfence` should be sufficient. However, the safe-Rust-via-`core::sync::atomic::fence(SeqCst)` is the idiomatic abstraction; verify the generated assembly matches the intent (a single `mfence` or equivalent serializing instruction). The SAFETY comment on the fence should reference R-C2 from THINK §4.5.

### 6c.4 Verification gates for sub-commit 4c

All must PASS before commit:

1. `cargo k` — kernel build with default features.
2. `cargo k --no-default-features` — kernel build with minimal features.
3. `cargo neo-test all` — full scenario suite must not regress.
4. **Existing-substrate stress:** `200/200 NEO_SMP=4` — SC-3B closure-gate equivalent must continue to pass. This is the load-bearing gate that confirms the new shootdown integration hasn't broken existing SMP behavior.
5. **New scenario per D14:** `cargo neo-test q7c_concurrent_munmap_smp` — CPU A munmaps a region while CPU B's thread is actively reading from the region; expected behavior: CPU B's read either succeeds (TLB still valid) or faults (TLB invalidated, mapping gone) — no use-after-free, no stale-data read. Run at NEO_SMP=4.
6. **New scenario per D14:** `cargo neo-test q7c_process_exit_during_thread_run` — Process P has threads on CPU A and CPU B; CPU A initiates process exit while CPU B's thread is in user mode; expected behavior: CPU B's thread is reaped cleanly via the existing Q8.c-deferred mechanism (Q8.c is a separate future sub-commit; for SC-3C, this scenario tests that the shootdown primitive doesn't introduce regressions in the existing exit path, not that Q8.c is complete). Run at NEO_SMP=4.
7. **NEW: Missing-shootdown-detection scenario per D14 + FreeBSD-EN-18:07** (the load-bearing D14 verification): `cargo neo-test q7c_missing_shootdown_detection` — a deliberately-broken Pmap mutation site (gated behind a `#[cfg(feature = "neo_test_missing_shootdown_negative")]` flag) that performs PTE invalidation **without** calling `shootdown(...)`. The scenario then attempts a read from the invalidated address on a different CPU than the one that performed the invalidation; the test harness expects the test to detect the stale-TLB-based incorrect behavior (i.e., the other CPU still successfully reads through the stale TLB entry — this is the FreeBSD-EN-18:07 failure mode). **This is a negative-expected scenario** — passing the test means the harness correctly detected the failure mode that the absence-of-shootdown would cause. The feature flag is OFF in production builds; ON only for this scenario. This is the explicit "we verified that omitting shootdown is detectable" gate per D14 ratification.

### 6c.5 Commit message (sub-commit 4c)

Suggested title:

```
kernel/mm: wire TLB shootdown into Pmap mutation sites (Q7.c call sites)
```

Suggested body (substance):

```
Wires the SC-3C shootdown machinery from sub-commit 4b into the cross-CPU-
visible page-table mutation sites in pmap.rs (and paging.rs/heap.rs where
applicable). SC-3C becomes correctness-load-bearing at this commit; before
this, the kernel had no cross-CPU TLB invalidation and was safe only under
single-process SMP.

Wiring pattern at each site: (1) PTE write, (2) SeqCst fence per THINK
§4.5 R-C2, (3) shootdown(range, other_cpus_mask()) per D4 broadcast
target selection.

Verification gates:
- 200/200 NEO_SMP=4 SC-3B closure-gate equivalent: PASS, no regressions.
- q7c_concurrent_munmap_smp: PASS.
- q7c_process_exit_during_thread_run: PASS (Q8.c future work; this
  scenario only verifies no regressions in the existing exit path).
- q7c_missing_shootdown_detection: PASS — the negative-expected scenario
  per D14 verifies the harness detects the stale-TLB failure mode that
  FreeBSD-EN-18:07 (2018) documented as the canonical missing-shootdown
  silent-data-corruption anchor.

See:
- Plans/SC-3C-THINK-2026-05-10.md §4, §5, D14
- Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-
  finalized.md (Cerebro-confirmed; FreeBSD-EN-18:07 verbatim "resulted in")
- Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md
```

### 6c.6 Stop conditions for sub-commit 4c

- Any verification gate fails → STOP, file failure report, surface to PM.
- `200/200 NEO_SMP=4` stress regresses (passes < 200/200) → STOP. SC-3B's closure gate was 200/200; SC-3C must hold it.
- `q7c_missing_shootdown_detection` fails to detect the stale-TLB failure mode → STOP. **This is the load-bearing gate for D14 ratification.** Failure means the verification harness cannot catch the FreeBSD-EN-18:07-class failure mode, which means future regressions of the shootdown wiring would slip through undetected.
- Any test scenario that previously passed now fails → STOP. SC-3C is meant to be a correctness gain plus a performance preservation; correctness regressions are not acceptable.

---

## 7. PLAN-stage documentation requirements (inline in code)

Three Phase 2+ forward markers from Cerebro D15 consumption must be encoded as inline comments in the new `tlb_shootdown.rs` file. These are forward-documentation requirements, not implementation work; capture them once and they're discoverable when the relevant Phase 2+ work surfaces.

### 7.1 Strictly-accurate vs loosely-accurate per-mm cpumask (T2 design choice)

In `tlb_shootdown.rs`, near the API documentation for `shootdown(range, targets)`, include a block comment that says (substance; Jarvis may refine wording):

```rust
/// Phase 2+ T2 migration note: per-mm cpumask design choice
///
/// SC-3C ships T1 broadcast (D4): the `targets` parameter is set to
/// `other_cpus_mask()` at every production call site. Phase 2+ T2 work
/// will derive a narrower target-set from per-mm cpumask state, reducing
/// IPI count to CPUs that have actually loaded the mm's CR3.
///
/// When that work lands, the per-mm cpumask discipline must choose between:
///
/// - Strictly-accurate: remove-on-switch-out + add-on-switch-in. CPU mask
///   reflects current truth at all times. Higher cost on context-switch
///   hot path (extra atomic on every switch-out).
///
/// - Loosely-accurate: add-only on switch-in; never remove on switch-out;
///   stale bits scrubbed lazily by IPI receivers self-clearing on receipt.
///   This is the Linux model (verified at primary source per Plans/
///   RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md). Cheaper
///   context-switch path; more IPIs delivered to CPUs that don't actually
///   need them (they self-scrub on receipt).
///
/// Linux's choice (loosely-accurate) is informed by context-switch path
/// being hotter than IPI path. NeoPlutarch's choice depends on workload
/// mix; the inference scheduler holding threads on accelerators for long
/// stretches may favor strictly-accurate. ADR-tier decision required
/// before T2 implementation begins.
///
/// See Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-
/// revisions-finalized.md §3.1.
```

### 7.2 Lazy-TLB page-table-teardown exception (forward marker)

In `tlb_shootdown.rs`, near the receiver handler or near the `range` field on the payload, include a block comment that says (substance):

```rust
/// Phase 2+ lazy-TLB note: page-table-teardown exception
///
/// SC-3C does not implement lazy-TLB (D6: out of scope). If Phase 2+
/// work introduces lazy-TLB optimization (where CPUs in idle or kernel-
/// only execution skip user-mm TLB invalidation IPIs), the Linux pattern
/// requires an explicit exception: the lazy filter MUST be bypassed
/// unconditionally when the shootdown is for page-table teardown (freed
/// upper-level paging structures). A CPU with stale upper-level paging
/// caches can fault into freed memory after the teardown completes.
///
/// Linux encodes this as a `freed_tables: bool` flag on the flush request
/// that bypasses `should_flush_tlb`'s lazy filter unconditionally. If
/// NeoPlutarch lands lazy-TLB later, the same exception must be encoded
/// in the shootdown API.
///
/// See Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-
/// revisions-finalized.md §4.
```

### 7.3 Linux global-ASID promotion mechanism (forward marker)

In `tlb_shootdown.rs`, near the documentation about INVLPG threshold, include a one-paragraph block comment:

```rust
/// Phase 2+ INVLPGB/RAR note: global-ASID promotion mechanism
///
/// SC-3C's broadcast model (D4 / T1) and the N=33 INVLPG threshold (D3)
/// match the conservative single-target shootdown pattern of pre-INVLPGB
/// Linux. AMD INVLPGB (Linux 6.15 mainline, Feb 2025) introduces hardware-
/// broadcast TLB invalidation for global-ASID-tagged mms. The Linux
/// promotion mechanism (`consider_global_asid` in arch/x86/mm/tlb.c) uses
/// a 1-in-32 statistical sampling gate plus a rising floor as the global-
/// ASID bitmap fills — NOT a simple deterministic 3-CPU threshold.
///
/// If/when NeoPlutarch designs a similar promotion logic (Phase 2+
/// INVLPGB/RAR work), the sampling-gate-plus-rising-floor pattern is the
/// precedent to study, not the simplified 3-CPU model.
///
/// Linux function name: broadcast_tlb_flush (the INVLPGB integration
/// point, callable from flush_tlb_mm_range when mm_global_asid(mm) is
/// set).
///
/// See Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-
/// revisions-finalized.md §3.5.
```

Total forward-marker comment volume in `tlb_shootdown.rs` is approximately 60 lines. This is appropriate for a foundational SMP-correctness module; future engineers reading the file get the Phase 2+ design context inline without needing to re-find the PM-NOTEs.

---

## 8. Branching, commit, push protocol

**Branch naming convention:** `jarvis/sc-3c-build-2026-05-11` (same defensive pattern as the kernel-rename bundle).

**Commit cadence:** one commit per sub-commit (three commits total: 4a, 4b, 4c). Each commit's verification gates pass before the next sub-commit begins.

**Push timing:** after each sub-commit's verification gates pass AND the verification report for that sub-commit is filed in `Plans/`. Push command: `git push origin HEAD:main` from the branch tip. Should be clean fast-forward; if rejected by upstream advancement (parallel work landing on `origin/main`), STOP and surface to PL for guidance — the kernel-rename bundle precedent rebased cleanly because the parallel work touched only README, but a parallel kernel-touching commit would require more careful handling.

**Verification reports:** file three reports, one per sub-commit:

- `Plans/SC-3C-SUB-COMMIT-4A-Q7B-CR3-CACHE-VERIFICATION-2026-05-MM.md`
- `Plans/SC-3C-SUB-COMMIT-4B-Q7C-MACHINERY-VERIFICATION-2026-05-MM.md`
- `Plans/SC-3C-SUB-COMMIT-4C-Q7C-CALL-SITES-VERIFICATION-2026-05-MM.md`

Format/style: match `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md` as the voice anchor. Document verbatim gate output where representative; document any incidental observations; cite commit SHAs after push.

**The verification reports are committed alongside the code commits**, parallel to how `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` and `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md` were tracked alongside their commits in the prior bundle.

---

## 9. General stop conditions

Beyond per-sub-commit stop conditions in §4–§6, also STOP if:

- Any commit's `cargo k` or `cargo k --no-default-features` fails. The kernel must build cleanly at every commit boundary.
- Any pre-existing scenario that was passing on `origin/main` HEAD `27d1b99` newly fails. The full-suite `cargo neo-test all` gate enforces this.
- A perf gate threshold is encoded with a theoretical native-hardware target (e.g., specifying nanosecond targets that imply native-hardware perf). All perf gates in SC-3C must be regression detectors anchored to QEMU TCG debug environment per the ADR-019 bench lesson. Sub-commit 4a's `q7b_cr3_cache_skip_count` is the only sub-commit-level perf gate; it's a ratio-based regression detector, not a time-based gate.
- Cerebro citation precision is violated in code comments or verification reports (e.g., citing `broadcast_flush_tlb_all` instead of `broadcast_tlb_flush`, claiming the FreeBSD API takes `cpuset_t mask`, claiming `curcpu_cb` runs during the wait loop). The Cerebro D15 corrections are non-negotiable; if Jarvis is uncertain whether a citation is precise, consult `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` directly.
- Any sub-commit produces verification gate output that suggests the implementation is correct but the gate is environment-fragile (e.g., the q7b cache-hit rate is borderline at 49-51% under QEMU TCG run-to-run variance). File investigation note, surface to PM for threshold-revision decision before committing — same pattern as the ADR-019 bench threshold environment-clarification.

---

## 10. Calibration framing (Jarvis dispatch 3 of 3)

This is Jarvis calibration dispatch 3 of 3 under the new agent-roster assignment. Two prior dispatches landed strong:

- **Dispatch 1 (kernel-rename bundle, sub-commits 1+2):** clean landings, exemplary stop discipline at sub-commit 3 gate (no force-through despite environment-fragile bench threshold), three optimization iterations documented before stop.
- **Dispatch 2 (sub-commit 3 finalize):** clean landing at `27d1b99` with PM-revised threshold; verification report comprehensive; orphan-container incidental observation surfaced.

**Forward calibration interest for this dispatch:** how Jarvis handles a dispatch where the implementation requires making non-trivial architectural choices mid-flight. SC-3C is more architecturally rich than the prior two — three sub-commits, multiple D-decisions in play, Cerebro-corrected citations to apply precisely, Phase 2+ forward-marker documentation embedded inline. If Jarvis encounters ambiguity in any D-decision interpretation, surface to PM before committing rather than guessing. PM-tier decisions are operational and can be resolved fast; bad calls baked into commits cost more to unwind.

After this dispatch closes, PM produces a Jarvis voice profile artifact (equivalent to a Janus voice profile) capturing observed dispatch-handling patterns across all three calibration dispatches.

---

## 11. Closure protocol

After all three sub-commits land + verification reports filed + branch pushed to `origin/main`:

1. Jarvis reports back to PL with: (a) three commit SHAs (4a, 4b, 4c), (b) verification report filenames, (c) any incidental observations across the bundle, (d) wall-clock time actual.
2. PL ferries the report back to PM.
3. PM consumes the report and ratifies SC-3C closure via a PM-NOTE (parallel to the kernel-rename bundle closure pattern). PHASE-2.md gets a SC-3C closure marker; PCID deferral, lazy-TLB, T2 cpumask migration, INVLPGB/RAR all enter the Phase 2+ forward-marker registry.
4. SC-3C is closed. Sub-commit 5 (Q8.a/b — capability table SMP composition) becomes the next candidate; sub-commit 6 (Q8.c — blocked-thread-kill walk + process-exit sequencing) follows that.

If Jarvis stops at any sub-commit gate during BUILD, the closure protocol pauses at that point: failure report filed, PM resolves, follow-up dispatch (parallel to the sub-commit 3 finalize pattern) re-engages from the stop point. No requirement to land all three sub-commits in a single dispatch wall-clock.

---

## 12. Estimated wall-clock and stop budget

**Estimated end-to-end wall-clock for Jarvis:**
- Sub-commit 4a: 1–2 hours (scheduler-local change; small surface).
- Sub-commit 4b: 4–6 hours (new module; multiple subsystems touched; synthetic test scenarios).
- Sub-commit 4c: 2–4 hours (wiring + new D14 scenarios; the missing-shootdown-detection scenario is the most novel test code).
- Verification gates between sub-commits: ~30 min each.
- Total: 8–14 hours wall-clock.

**Stop budget guidance:** if any sub-commit's wall-clock exceeds 2x the upper estimate (i.e., 4a takes >4h, 4b takes >12h, 4c takes >8h), pause and surface to PM. This may indicate a design ambiguity worth resolving via PM consultation rather than continuing to implement against an unclear spec. Calibration dispatches are not the place to push through 4x overruns.

---

## 13. Cross-references

**Design inputs:**
- `Plans/SC-3C-THINK-2026-05-10.md` — canonical design (PM cadence work, Bible 14).
- `Plans/PM-NOTE-2026-05-10-sc-3c-d-revisions-ratified.md` — D-revisions FINALIZED.
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — canonical prior art.
- `Plans/PM-NOTE-2026-05-11-sc-3c-d15-cerebro-consumption-and-d-revisions-finalized.md` — corrections + Phase 2+ markers.

**Calibration inputs:**
- `Plans/PM-NOTE-2026-05-10-agent-roster-update.md` §2.1 (Jarvis kernel lane), §3.1 (calibration discipline).
- `Plans/PM-NOTE-2026-05-11-jarvis-first-dispatch-calibration.md` — Jarvis calibration 1 of 3 record.
- `Plans/PM-NOTE-2026-05-11-adr-019-bench-threshold-environment-clarification.md` — perf-gate threshold environment-anchoring discipline.

**Voice anchors:**
- `Plans/KERNEL-RENAME-SUB-COMMIT-1-VERIFICATION-2026-05-10.md` — verification report format.
- `Plans/ADR-020-SUB-COMMIT-2-VERIFICATION-2026-05-10.md` — verification report format.
- `Plans/IRQSAFE-MUTEX-SUB-COMMIT-3-VERIFICATION-2026-05-11.md` — verification report format (most recent).

**Architectural anchors:**
- `docs/adr/ADR-018-...` (Hpw discipline — applies to shootdown IPI handler running at IF=0).
- `docs/adr/ADR-019-switchmutex-irq-off-by-construction.md` (IrqSafeMutex; available at `27d1b99` for sender-side state).
- `docs/adr/ADR-020-owner-metadata-as-production-diagnostic.md` (LockOwner; carries through any IrqSafeMutex used in SC-3C).

**Already landed on `origin/main`:**
- `27d1b99 kernel/sync: introduce IrqSafeMutex (ADR-019 sub-phase A)` — sub-commit 4b uses this directly.
- `de8965c kernel/sync: add LockOwner field to SwitchMutex (ADR-020 foundation)`.
- `053a5bc kernel: rename AltimitNeo → NeoPlutarch; A3 boot identity`.

---

**End of dispatch.**

*SC-3C BUILD: three sub-commits per D12 (4a Q7.b CR3 cache → 4b Q7.c machinery → 4c Q7.c call sites). All fourteen D-decisions finalized via Cerebro D15 confirmation; corrected citation precision applied throughout. Phase 2+ forward markers (strictly-vs-loosely-accurate T2 cpumask, lazy-TLB page-table-teardown exception, INVLPGB/RAR global-ASID promotion mechanism) documented inline in tlb_shootdown.rs. D14 missing-shootdown-detection scenario gates sub-commit 4c per FreeBSD-EN-18:07. Calibration dispatch 3 of 3 under new Jarvis-under-Codex assignment.*
