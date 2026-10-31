# ADR-012: Buddy + Slab Allocator Foundation

| | |
|---|---|
| **Status** | Accepted (Phase 2 THINK; BUILD-2 implements) |
| **Date** | 2026-05-04 |
| **Authority** | Project Lead |
| **Supersedes** | — (retires Phase 1's `BumpFrameAllocator` + intrusive free-list overlay) |
| **Superseded by** | — |
| **Related** | Bible Article 7.4 (Phase 2 buddy + slab mandate) · ANSPEC-001 §8.1 (tensor allocator separate, per-device) · ADR-010 (capability slot allocations migrate to slab) · ADR-011 (single-CPU IPC handoff invariants — slab cache lifetime composes with these) · `kernel/src/mm/frame.rs` (existing bump+free-list overlay being retired) · `kernel/src/mm/heap.rs` (linked_list_allocator that stays as variable-size fallback) |

---

## Context

Bible Article 7.4: "Physical memory is managed by a frame allocator. The bootstrap allocator is a bump allocator (Phase 1); a buddy allocator replaces it (Phase 2); a slab allocator sits atop the buddy for object pools (Phase 2). No kmalloc-style heap fragmentation games."

Phase 2 BUILD-1 (capability foundation, commit `d759cbd` + the implementation that landed against it) introduced fixed-size kernel objects with high-frequency mint/derive/grant/revoke patterns: `Handle` slots in process tables, `CapId` derivation log entries, `EndpointState` slots, `Process`/`Thread`/`Channel` allocations. All of these currently route through the kernel's `linked_list_allocator` (general-purpose kernel heap). That's a fragmentation hazard under sustained capability churn — a real Phase 2 concern, not a theoretical one.

Phase 2 BUILD-2 lands the buddy and slab allocators per Bible 7.4. ANSPEC-001 §8.1 explicitly carves the tensor allocator out of this scope as a per-device subsystem; Phase 5 owns it. BUILD-2 is purely the kernel-internal memory management substrate — physical frames + fixed-size kernel object pools.

The order matters: allocators-before-SMP per the Phase 2 plan and per the four reasons surfaced in the BUILD-2 design conversation. Specifically, BUILD-1's capability slots currently fragment the kernel heap; migrating them to slab cleans this up *before* SMP adds contention.

Three orthogonal design axes drive the implementation. Each is decided independently below.

## Decision

### Q1 — Buddy frame allocator: classic power-of-2, max order 14

The current `BumpFrameAllocator` (with intrusive free-list overlay added in Phase 1.5) is replaced by a classic power-of-2 buddy allocator covering orders 0..14 (4 KiB to 64 MiB).

```rust
// kernel/src/mm/frame.rs (BUILD-2)

pub struct BuddyAllocator {
    free_lists: [LinkedList<FreeBlock>; MAX_ORDER + 1],
    /// Tracks allocated/freed counts for the leak detector
    /// (compatible with mm::frames_in_use accessor).
    allocated_pages: AtomicU64,
    /// Lock for the free_lists. Single-CPU + without_interrupts callers
    /// in Phase 2 BUILD-2; SMP BUILD-3 adds the cross-CPU contention
    /// story (per-CPU magazines is one option, finer-grained per-order
    /// locking is another — Phase 2 SMP ADR decides).
    lock: Mutex<()>,
}

const MIN_ORDER: usize = 0;       // 4 KiB (= PhysFrame<Size4KiB>)
const MAX_ORDER: usize = 14;      // 64 MiB
```

**Why max_order = 14.** Phase 2's needs (kernel-image, slab back-stores, kernel-stack pages) max out around order 10 (4 MiB). Phase 5's tensor allocator (per ANSPEC §8.1) is a separate subsystem, but if/when it sources frames from the buddy, typical model-weight blocks land in the 16-64 MiB range. Setting max_order = 14 means the buddy CAN serve those allocations without a parallel large-block allocator. The cost is one extra bit in the order field; the benefit is a Phase 5 retrofit avoided. Phase 5 still owns the tensor *allocator*; max_order is just "what's the largest contiguous frame range buddy can hand out."

**Min order = 4 KiB.** Matches `PhysFrame<Size4KiB>` invariants throughout the kernel. 8 KiB minimum would force every allocator-touching code path to think about "is this one frame or two?" with no upside; slab handles small-object internal fragmentation by definition.

| Alternative | Rejected because |
|---|---|
| Buddy + region pinning (carve-out for SMP per-CPU + tensor up front) | Premature commitment to Phase 5 tensor design and Phase 2 SMP per-CPU layout. Boot-time reservations sit OUTSIDE the buddy's managed range — the buddy doesn't need hooks for them. ANSPEC §8.1 explicitly keeps tensor allocator separate. |
| Buddy + per-NUMA-node free lists | No NUMA in Phase 2 (single physical memory model under QEMU; bare-metal target TBD per ADR-009). Pure premature complexity. |

**seL4 reference.** seL4 ships no kernel buddy — userspace owns memory via untyped capabilities + retypes. NEO is monolithic; kernel-side buddy is appropriate. Linux's buddy is the canonical reference.

**Phase 5 forward-compat.** Tensor allocator is its own subsystem per ANSPEC §8.1. Buddy doesn't see tensor allocations and doesn't need to. Phase 5 may extend by allowing the tensor allocator to source bulk frames from buddy (max_order = 14 supports this); no Phase 2 change needed.

### Q2 — Slab allocator: per-CPU caches with global fallback, in BUILD-2

Slab caches use Linux-SLUB-style per-CPU magazines with global cache fallback. Per-CPU caches are lockless on the hot path; global cache acquires the per-cache global lock for batched transfers when local cache hits its high or low watermark.

```rust
// kernel/src/mm/slab.rs (BUILD-2)

pub struct Slab<T: Send> {
    name: &'static str,
    /// Per-CPU magazines. In single-CPU mode this is a 1-element array;
    /// SMP BUILD-3+ sizes it per actual CPU count. Access via per_cpu
    /// module's GS-base accessor — exercised in single-CPU so the
    /// access pattern is correct from day one.
    per_cpu: PerCpu<Magazine<T>>,
    /// Global free list. Acquired only on per-CPU cache miss/full.
    global: Mutex<GlobalCache<T>>,
    /// Configuration (per-cache tunables, see Q3 of opens).
    config: SlabConfig,
}

pub struct SlabConfig {
    /// Per-CPU magazine high-water mark — push to global on full.
    pub local_capacity: usize,
    /// Global cache high-water mark — return frames to buddy on overflow.
    pub global_capacity: usize,
    /// Batch size for local↔global transfer.
    pub batch_size: usize,
    /// Zero each object on free (before returning to the cache). Default
    /// `true`. Set `false` only when the cache type's constructor fully
    /// overwrites every field — see implementation note on per-cache
    /// zeroing policy below. New cache types default to `true` until
    /// proven otherwise.
    pub zero_on_free: bool,
}
```

**Why B (per-CPU + global) over A (single-global with SMP-friendly interface).** The decision was relitigated during BUILD-2 design conversation. Verbatim from the conversation record:

> Threshold and batching mechanics are exercised in single-CPU. The pull/push thresholds, the batch-transfer logic, the local→global slow-path lock acquisition all run on every alloc/free pattern that crosses the per-CPU cache's threshold, even with one CPU. BUILD-2 single-CPU exercises that mechanism in a controlled environment; BUILD-4 SMP debugging conflates "is this mechanism wrong?" with "is concurrent access wrong?"

This is **Principle 4** below in concrete form. The "interface stable, swap implementation later" approach (option A) was rejected because (a) cache-warmth semantics differ between single-global FIFO and per-CPU LIFO, so the interface contract is NOT identical, and (b) the "we'll do per-CPU later" pattern empirically becomes "we never did per-CPU and SMP debugging is harder than it needs to be."

| Alternative | Rejected because |
|---|---|
| Single global slab cache per object size, with SMP-friendly interface | Cache-warmth semantics aren't actually interface-equivalent (single-global FIFO vs per-CPU LIFO). Plus the "we'll add per-CPU later" risk: BUILD-4 retrofitting conflates mechanism design with SMP debugging. Per-CPU cache invariants are cheaper to surface in BUILD-2 single-CPU. |
| Bonwick-style slab with constructed/destructed objects | Pre-construct on slab fill, reuse on alloc without re-running ctor. Higher complexity; matters for objects with expensive constructors. Phase 2 cap slots, channels, etc. are cheap to construct (set a few fields). Not justified. Revisit if Phase 5 introduces a cache type with genuinely expensive construction. |

**seL4 reference.** seL4 ships no slab — kernel objects are typed by retype-from-untyped, sized at retype time. NEO's slab is for kernel-internal pools, not userspace-visible objects. Linux's SLUB is the canonical reference; NEO's design is a simplified subset (no NUMA, no debug instrumentation surfaces yet).

**Phase 5 forward-compat.** Tensor metadata may eventually warrant a slab cache; the per-cache configuration mechanism handles it without ADR amendment.

### Q3 — Migration: incremental, five sub-commits

Buddy + slab land in five sub-commits, each independently bisectable, with a simplify gate between each.

| # | Scope | Verification |
|---|---|---|
| 0 | Slab cache **interface only** (trait/struct API, no implementation, no callers). | Compile + existing test suite green (anti-regression). |
| 1 | Buddy replaces `BumpFrameAllocator`. Free-list overlay retires. | Boot test + existing test suite (frame leak audit, IPC, cap, security tests). |
| 2 | Slab implementation lands behind sub-commit-0's interface. `Handle` cache migrated. | Boot + cap_* tests + new slab-specific tests. |
| 3 | Channel + EndpointState + derivation log entry caches migrated. | Boot + ipc_* tests + cap_* tests. |
| 4 | Process + Thread caches migrated. | Boot + ipc_heap_kill_smoke + frame leak audit. |

Sub-commit 0 separates interface contract from implementation: bisecting "did buddy break?" doesn't get conflated with "did slab break?" — and bisecting "did slab break?" doesn't get conflated with "did the interface change?" The cost is one tiny commit; the benefit is bisectability hygiene at exactly the granularity Phase 1.5b/1.5c proved valuable.

| Alternative | Rejected because |
|---|---|
| Buddy-only in BUILD-2; slab deferred | Bible 7.4 explicitly mandates both in Phase 2. Splitting into BUILD-2 + BUILD-2.5 dilutes the BUILD's purpose. |
| Buddy + full slab as one commit | Big single commit, hard to bisect. The four+ sub-commits buy bisectability at trivial cost. |

## ABI / structural surface

### Buddy

`BuddyAllocator` implements `FrameAllocator<Size4KiB>` from the `x86_64` crate. The existing `mm::with_frame_alloc<F, R>` wrapper is unchanged; callers don't see the swap. `mm::frames_in_use()` accessor still works (now backed by `BuddyAllocator::allocated_pages` instead of `BumpFrameAllocator::FRAMES_IN_USE`). The Phase 1.5c BUILD-6 leak detector keeps working unchanged.

### Slab

```rust
pub trait SlabCache<T: Send> {
    fn alloc(&self) -> Option<Box<T>>;
    fn free(&self, obj: Box<T>);
    fn stats(&self) -> SlabStats;
}

pub struct SlabStats {
    pub local_in_use: usize,    // sum across per-CPU magazines
    pub global_in_use: usize,
    pub buddy_pages_held: usize,
}
```

Six BUILD-2 cache instances:
- `static PROCESS_CACHE: Slab<Process>`
- `static THREAD_CACHE: Slab<Thread>`
- `static CHANNEL_CACHE: Slab<Channel>`
- `static HANDLE_CACHE: Slab<Handle>`
- `static ENDPOINT_STATE_CACHE: Slab<EndpointState>`
- `static DERIVATION_ENTRY_CACHE: Slab<DerivationEntry>`

**Sub-decision (BUILD-2 implementer judgment):** whether `Handle::Channel` and `Handle::Process` variants share `HANDLE_CACHE` or get separate `CHANNEL_HANDLE_CACHE` + `PROCESS_HANDLE_CACHE` based on hot-path analysis. ADR-012 does not pre-commit; the implementer decides during BUILD-2 based on whether the variants have different lifecycle hot paths. Either choice satisfies the migration scope.

### Configuration (per-cache, tunable)

```rust
const PROCESS_CACHE_CONFIG:    SlabConfig = SlabConfig { local_capacity: 4,  global_capacity: 16, batch_size: 2,  zero_on_free: false };
const THREAD_CACHE_CONFIG:     SlabConfig = SlabConfig { local_capacity: 8,  global_capacity: 32, batch_size: 4,  zero_on_free: false };
const CHANNEL_CACHE_CONFIG:    SlabConfig = SlabConfig { local_capacity: 16, global_capacity: 64, batch_size: 8,  zero_on_free: false };
const HANDLE_CACHE_CONFIG:     SlabConfig = SlabConfig { local_capacity: 32, global_capacity: 128, batch_size: 16, zero_on_free: true };
const ENDPOINT_STATE_CONFIG:   SlabConfig = SlabConfig { local_capacity: 16, global_capacity: 64, batch_size: 8,  zero_on_free: true };
const DERIVATION_ENTRY_CONFIG: SlabConfig = SlabConfig { local_capacity: 32, global_capacity: 128, batch_size: 16, zero_on_free: true };
```

**These starting values are educated guesses, not measured optima.** The local:global ratio (1:4) and batch:local ratio (1:2) are held constant across all caches deliberately — *don't tune ratios per-cache without measurement*. Higher-frequency caches (Handle, DerivationEntry) get larger absolute magazines; lower-frequency (Process, Thread) get smaller. The `SlabConfig` struct itself is the **load-bearing contract**; the numbers inside it are tunable parameters expected to be revised under Phase 2 BUILD-4 SMP measurement and Phase 5 inference workload data.

**Discipline:** do not relitigate these numbers without data. Do relitigate them when data exists. The BUILD-2 implementer should not adjust them based on intuition; future contributors should not treat them as sacred either. (See Principle 5 in implementation notes below.)

### Co-existence with `linked_list_allocator` — permanent, not transitional

**Slab and the kernel heap (`linked_list_allocator`) coexist permanently.** Slab serves fixed-size object pools (Process, Thread, Channel, Handle, EndpointState, DerivationEntry, future Tensor metadata). The kernel heap serves variable-size allocations (`Vec<u8>` for inherited handle arrays, `Vec<PhysFrame>` in `Pmap::Drop`'s walker, `String`, `Box<dyn Trait>`, ad-hoc buffers). Code allocating a fixed-size kernel object uses slab; code allocating a variable-size buffer uses the kernel heap. Both source from buddy.

**Neither replaces the other.** ADR-012 does not anticipate a future BUILD that migrates kernel-heap allocations into slab — that would be a category error (slab is fundamentally for fixed-size pools). Phase 3+ may revisit `linked_list_allocator` *itself* (replacing it with a slab-of-slabs design or a region-based allocator), but that's a separate ADR with separate discipline. ADR-012's scope ends at "buddy replaces bump; slab handles fixed-size object pools; kernel heap handles variable-size as before."

**Three allocation classes (forward-compat note for Phase 5):** fixed-size kernel object → slab; variable-size kernel allocation → kernel heap; device-resident tensor (per ANSPEC §8.1) → tensor allocator (Phase 5+, distinct subsystem). The class determines the allocator; don't relitigate which allocation goes where. (See Principle 6 in implementation notes below.)

## Non-decisions (deferred)

- **Bonwick pre-construct semantics.** Deferred until a cache type with expensive construction appears. Lazy across all six BUILD-2 caches.
- **Per-NUMA-node free lists.** No NUMA in Phase 2.
- **Slab debug instrumentation (poison patterns, double-free detection beyond what the existing frame allocator provides).** Phase 1.5b's `FREE_POISON` debug check carries forward into the buddy's coalesce path. Slab-level double-free detection is implicit (`Box<T>` ownership prevents trivial double-free); deeper instrumentation is Phase 3+ if needed.
- **Slab shrinking under memory pressure.** Phase 1.5c didn't have memory pressure; Phase 2 doesn't either at QEMU scales. When real pressure arrives (Phase 5+ inference workloads), revisit.
- **`linked_list_allocator` replacement.** Phase 3+ may revisit the kernel heap allocator itself; out of scope here.

## Implementation notes (load-bearing — read before changing)

1. **Buddy uses single global lock for Phase 2.** Single-CPU `without_interrupts` callers in BUILD-2 mean no contention. SMP BUILD-3 hardens — possibly per-order locking, possibly per-CPU magazines for the buddy itself, possibly stays single-lock. Phase 2 SMP ADR decides. **Lock ordering: `Slab.per_cpu` (lockless) → `Slab.global` (per-cache) → `Buddy.lock` (global).** Strict; deeper locks acquired AFTER shallower ones; never reverse. Document everywhere a slab function calls into buddy.

2. **Per-CPU storage uses the existing `per_cpu` module / GS-base accessor.** Even in single-CPU, the per-CPU array has 1 element and access goes through GS-base. This exercises the access pattern from day one — Principle 4 in concrete form. SMP BUILD-3 expands the per-CPU array size; the access pattern is unchanged.

3. **Cache-warmth: per-CPU returns LIFO-local objects.** Per-CPU magazines push to / pop from the same end of the magazine ring. This means a freshly-freed object is the most likely to be reallocated. **This differs from FIFO single-global semantics.** Anti-regression target: any future code that develops dependencies on FIFO reuse (object-recently-freed observability, cache-line-bouncing hypotheses) must be checked against this. Most code shouldn't care; flagging anyway because Phase 5 inference latency is sensitive to allocator locality.

4. **Sub-commit 1 retires `BumpFrameAllocator` AND its intrusive free-list overlay.** The buddy's coalesce path is the sole free path. Two free paths that need to agree is a bug factory; the coalesce-disagrees-with-overlay failure mode would be subtle and hard to debug. Phase 1.5's `FREE_POISON` debug check moves into the buddy's coalesce path (a block found on the wrong-order free list during coalesce is a double-free signal). Frame leak audit migrates to the buddy's `allocated_pages: AtomicU64`; the `mm::frames_in_use()` accessor semantic is preserved. If a regression surfaces post-sub-commit-1 that the overlay would have caught, that's a bug in the buddy that needs fixing — not a reason to keep the overlay alive.

5. **`mm::frames_in_use()` accessor compatibility.** The Phase 1.5c BUILD-6 leak detector reads this. Keep it working — buddy's `allocated_pages: AtomicU64` is the new backing. Same semantics: incremented on alloc, decremented on free (including coalesce-into-larger-block returning sub-blocks to higher orders).

6. **Slab caches return memory to buddy on `global_capacity` overflow.** When a global cache exceeds `global_capacity`, return the oldest backing slab page (8 KiB, 4 cache slots typically) to the buddy. This is what makes the leak audit closeable — slab doesn't hold memory indefinitely after process exit drops the cache pressure.

7. **Process exit composes with slab.** When a process exits and `on_thread_reap` runs, the process's `Handle`/`EndpointState`/`DerivationEntry` allocations all flow back to their slab caches via `Box::drop` → `Slab::free`. The cache-ceiling logic above means memory eventually flows back to the buddy. No special process-exit hook needed; the existing Drop chain handles everything.

8. **Capability slot allocations migrate to `HANDLE_CACHE` in sub-commit 2.** Specifically: `HandleTable::install` no longer goes through `Vec::push`/`Vec::clone` for the entry; entries are slab-allocated `Box<Handle>` referenced by index in the table. The migration changes one allocation pattern per process. ADR-010 BUILD-1 amendments stay valid; the slab migration is an implementation detail of how Handle slots are stored.

9. **Per-cache zero-on-free policy.** The threat model: slab caches are kernel-global (cross-process reuse is the default). Cap-related data leaking via slab reuse violates capability discipline (caps are unforgeable and unobservable). IPC-pending payload data leaking via slab reuse violates IPC isolation. Specifically:
   - **`zero_on_free: true`** for `HANDLE_CACHE`, `DERIVATION_ENTRY_CACHE`, `ENDPOINT_STATE_CACHE`. These hold cap data (`cap_id`, `rights`) or IPC-pending payload (`waiter` thread refs, user-buffer VAs, pending grant rendezvous state).
   - **`zero_on_free: false`** for `PROCESS_CACHE`, `THREAD_CACHE`, `CHANNEL_CACHE`. Their constructors fully overwrite every field; explicit zero-on-free would be redundant work.
   - **New cache types default to `true`.** Adding a new cache type with `zero_on_free: false` requires explicit justification (constructor demonstrably overwrites every field). This is the **default-conservative discipline** (Principle 7 below).

10. **Principle 5 — Mechanism vs numbers.** The `SlabConfig` struct is the load-bearing contract; the specific local/global/batch numbers in BUILD-2 are tunable parameters expected to be revised under measurement. Do not relitigate the numbers without data; do relitigate them when data exists. Generalizes to any future tunable parameter (Phase 5 inference scheduler weights, future allocator thresholds, anything with a number that can be measured). Mechanism stays; numbers move.

11. **Principle 6 — Allocation class discipline.** Three allocation classes exist in the kernel: fixed-size kernel object pool → slab; variable-size kernel allocation → kernel heap; device-resident tensor → tensor allocator (Phase 5+, distinct subsystem). The class determines the allocator. Code allocating a fixed-size object MUST go through slab; code allocating variable-size MUST go through the kernel heap. Don't relitigate which allocation goes where — the class is the answer. Future ADRs adding new allocation classes (e.g., a Phase 7 inference-context allocator with deadline awareness) cite this principle and add a fourth class explicitly.

12. **Principle 7 — Default-conservative for new cache types.** New `SlabConfig` instances default to `zero_on_free: true`. Setting `false` requires written justification — typically that the cache type's constructor demonstrably overwrites every field. Future contributors adding a cache don't have to reason about whether their data is sensitive; the default is the safe choice. Generalizes beyond slab to any future kernel mechanism with a "this might leak" property: default to the safe behavior; explicit-unsafe requires justification.

13. **`per_cpu` module extension shape is a sub-commit-0 implementer sub-decision.** ADR-012 §Q2 mandates that slab caches use the existing `per_cpu` module / GS-base accessor pattern (Principle 4 in concrete form). The current `PerCpu` struct at `kernel/src/arch/x86_64/per_cpu.rs` has fixed fields (`kernel_rsp`, `user_rsp_save`) and is NOT a generic `PerCpu<T>` container. Sub-commit 0 (slab interface stub) must resolve the extension shape, choosing among three viable options:
    - **(a) Extend `PerCpu` with a slab-magazine slot.** Same struct, additional field. Lowest churn; `PER_CPU` static stays the canonical accessor; existing `KERNEL_RSP_OFFSET` / `USER_RSP_SAVE_OFFSET` constants unchanged. Risk: future per-CPU concerns (Phase 2 SMP run queues, Phase 7 inference scheduler state) all accrete fields here, growing the struct unbounded.
    - **(b) Build a parallel per-CPU mechanism for slab.** Separate `static SLAB_PER_CPU: ...` with its own GS-base offset (or its own MSR-base mechanism on a non-GS register). Risk: two per-CPU layouts to keep aligned with asm offsets; harder to audit.
    - **(c) Define a generic `PerCpu<T>` container that both fixed fields and slab magazines use.** Highest design churn now; cleanest long-term. Risk: ABI alignment with `syscall_entry` asm becomes type-parametric, which the asm cannot express — would need either a fixed prefix layout (kernel_rsp + user_rsp_save in the same offsets, regardless of the type parameter) or asm changes.

    The implementer chooses based on what other Phase 2 work (SMP foundation in BUILD-3, eventually Phase 5/7 per-CPU state) is expected to want from `per_cpu`. **Mirrors the `ProcessHandle`-vs-`Channel`-handle cache split sub-decision in §Q2:** ADR-012 does not pre-commit; the chosen shape lands in the BUILD-2 verification log so future readers find the rationale. The choice is constrained but not forced — read `per_cpu.rs` and `syscall_entry`'s asm offsets before deciding.

## Principles (additions to ADR-010's project-wide principle index)

ADR-010 lifted Principles 1–3 (defense-in-depth on cap-rights; reserved-bit pattern; errors at the causing actor). ADR-012 adds Principle 4 (below) plus three application-scoped principles in implementation notes 10–12:

- **Principle 5: Mechanism vs numbers** (note 10) — `SlabConfig` is the contract; magazine sizes are tunables.
- **Principle 6: Allocation class discipline** (note 11) — fixed-size → slab; variable-size → kernel heap; device-resident → tensor allocator (Phase 5+).
- **Principle 7: Default-conservative for new cache types** (note 12) — new caches default to `zero_on_free: true`.

Principles 5–7 are documented in implementation notes because they are context-of-application disciplines (specific to this ADR's mechanisms) but are numbered for cross-ADR citation. Future ADRs may cite "per Principle 5/6/7" without re-deriving.

### Principle 4: SMP-shaped mechanisms land at design time, not at SMP bring-up time

When a kernel mechanism has SMP-correct semantics that differ from naive single-CPU semantics, ship the SMP-correct version BEFORE SMP arrives if the mechanism's invariants can be partially exercised in single-CPU.

The "we'll add SMP-correctness later" pattern conflates mechanism design with concurrent debugging:
- Mechanism bugs become indistinguishable from SMP bugs.
- Pull/push thresholds, batch sizes, lock-ordering invariants all have to be designed under SMP pressure.
- The mechanism's interface contract is committed to in single-CPU; SMP-correct interfaces may differ subtly (cache-warmth, ordering guarantees).

Today's example: per-CPU slab caches in BUILD-2. The threshold/batching mechanics, lock-acquire pattern, and per-CPU access pattern (via GS-base) all exercise in single-CPU even though concurrent contention does not. BUILD-3 SMP work then debugs *concurrency*, not *mechanism*.

Future expected applications:
- SMP-correct lock primitives in BUILD-3 (designed once; not bolted on later).
- Per-CPU run-queue mechanics (threshold-based work-stealing logic exercises in single-CPU).
- Future Phase 5 inference scheduler if any of its mechanism logic exercises pre-SMP.

Does NOT apply to SMP-only concerns (IPI delivery, TLB shootdown, cross-CPU lock-fairness): those have no single-CPU exercise path. Apply Principle 4 only where the mechanism has structure that single-CPU can validate.

This is the Phase 1.5c lesson reapplied to allocators: implicit invariants get expensive when surfaced under simultaneous-bug conditions. Make them explicit early.

## Cross-references

- **Bible Article 7.4** — buddy + slab Phase 2 mandate satisfied by this ADR.
- **ANSPEC-001 §8.1** — tensor allocator stays separate per-device. Buddy max_order = 14 supports tensor allocator drawing bulk frames from buddy in Phase 5 if it wants to.
- **ADR-010** — capability slot migration to slab is a BUILD-2 sub-commit. ADR-010's Handle enum + BUILD-1 amendment are unchanged at the abstract level; only the storage layer migrates.
- **ADR-011** — slab cache lifetime composes with the IPC handoff invariants. `Channel` slab cache holds `Channel` allocations; the rendezvous mechanism uses those allocations; nothing in the rendezvous semantics depends on the underlying allocator.
- **`docs/HANDOFF.md`** — should be updated post-ADR-012 to reflect the sub-commit migration sequence as the BUILD-2 plan.

## Test scenarios (Phase 2 BUILD-2 verification)

Per-sub-commit anti-regression + new functional tests:

**Sub-commit 0 (interface stub).**
- Compiles. Existing test suite green.

**Sub-commit 1 (buddy replaces bump).**
- All Phase 1.5b/1.5c tests pass: `ipc_heap_kill_smoke`, `smep_catch`, `smap_catch`, `smep_policy`, `smap_policy`, `kaslr_smoke`, `expect_pass_but_fail`.
- Frame leak audit: `final == baseline` after `ipc_heap_kill_smoke`.
- New: `buddy_split_coalesce` — allocate at order 0, force splitting from a higher-order free block, then free and verify coalesce up to the original order. Falsifiable: if coalesce is broken, the same allocation pattern over many iterations leaks.
- New: `buddy_max_order` — allocate at order 14 (64 MiB) to verify the high end of the order range works. (Skip if the test VM doesn't have 64 MiB free; condition the test on memory availability.)
- New: **`expect_pass_but_alloc_fail`** — symmetric with Phase 1.5c's `expect_pass_but_fail`. Deliberately exhaust the buddy and verify (a) the allocation function returns the error variant rather than panicking, (b) the test runner correctly propagates the failure. Lands in sub-commit 1 alongside the buddy itself; the buddy's OOM-returns-error semantic is a property of the buddy, and verifying it requires the meta-test in the same commit. After the deliberate-OOM completes, the test frees the exhaustion allocations and verifies the buddy recovers to a usable state. Anti-regression on Bible Article 15.3 (no `unwrap()` in steady-state paths) — the meta-test makes the rule infrastructure-enforced, not just review-enforced.

**Sub-commit 2 (slab + Handle migration).**
- All previous tests pass.
- `cap_*` test suite from ADR-010 still passes (Handle slab allocation is transparent to cap-discipline tests).
- New: `slab_local_to_global_promotion` — alloc/free pattern that exceeds `local_capacity`, verify objects flow to global cache and the lock-acquire path runs.
- New: `slab_global_to_buddy_overflow` — alloc/free pattern that exceeds `global_capacity`, verify slab returns excess backing pages to buddy.

**Sub-commits 3 + 4.**
- All previous tests pass.
- IPC tests pass (Channel + EndpointState now slab-allocated).
- Frame leak audit clean (Process + Thread now slab-allocated).

## Future Phase 5+ extension

When tensor allocator (ANSPEC §8.1) lands:
- It may source bulk frames from buddy via `with_frame_alloc(|fa| fa.allocate_blocks(order))` for blocks up to order 14.
- It manages its own residency / migration / dedup logic above that.
- No buddy changes required; the max_order = 14 decision in this ADR enables the integration.

When new fixed-size kernel object types appear (e.g., `Tensor` metadata, `Model` graph nodes):
- Add a new `Slab<T>` instance with appropriate `SlabConfig`.
- No ADR amendment needed; `SlabCache<T>` trait and per-cache config mechanism handle it.

When SMP arrives (Phase 2 BUILD-3+):
- Per-CPU magazine count increases from 1 to actual CPU count.
- `per_cpu` module's GS-base accessor is unchanged.
- Buddy's single global lock revisited; replacement options: per-order locking, per-CPU magazines for the buddy itself, finer-grained shards. Phase 2 SMP ADR decides.
- Slab `local_capacity` / `batch_size` likely retuned under measurement.

---

## BUILD-2 amendment: cache list scope correction (pre-implementation)

Surfaced during the Pre-BUILD-2 Checkpoint round, before sub-commit 0 lands. The original ADR-012 cache list (Process, Thread, Channel, Handle, EndpointState, DerivationEntry) was specified ahead of reading the BUILD-1 code's actual allocation shapes. Cross-checking against the post-BUILD-1 code revealed two cache types that aren't separately allocated in current code, which makes "slab cache" a category mistake for them. Correcting before code lands.

### Removed: `Handle` cache

`Handle` is a Rust enum with `Channel { channel: Arc<Channel>, side, cap_id, rights }` and `Process { target, cap_id, rights }` variants. It is stored **inline** in `HandleTable.entries: Vec<Option<Handle>>` (defined in `kernel/src/sched/process.rs`). The Vec's backing buffer holds `Option<Handle>` slots directly; there is no `Box<Handle>` allocation per handle today.

A "Handle slab cache" would only make sense if Handles became `Box<Handle>` allocated separately. That requires a `Vec<Option<Box<Handle>>>` migration that adds an indirection per lookup with no measurable benefit — Handles in a single process's table benefit from being cache-line-adjacent in the Vec's contiguous backing buffer; cross-process slab-locality provides nothing because handle tables are accessed by their owning process's threads only. The migration is overhead without payoff.

**Resolution:** the actual fixed-size allocation underneath a `Handle::Channel` variant is the `Arc<Channel>` payload. The `Channel` cache (already in the list) covers that. The `Handle` enum itself stays inline in Vec slots, no slab. Phase 5+ Tensor / Model handle variants that genuinely require separate allocation can add their own slab caches when introduced; the trait API supports that without ADR-012 amendment.

### Removed: `EndpointState` cache

`EndpointState` is the per-channel-side state enum stored as `state: Mutex<[EndpointState; 2]>` inside `Channel` (defined in `kernel/src/ipc/mod.rs`). The two endpoint slots are inline in the Channel's allocation; there is no separate `EndpointState` allocation today. Same category mistake as Handle.

**Resolution:** `EndpointState` is a subfield of `Channel`'s allocation; the `Channel` cache (already in the list) covers it transitively. No separate `EndpointState` cache.

### Deferred: `DerivationEntry` cache shape

The per-process derivation log is `Mutex<BTreeMap<CapId, Option<CapId>>>`. Entries are `(CapId, Option<CapId>)` tuples stored in `BTreeMap`'s internal node type — opaque, not directly slab-allocatable without custom-allocator integration with `alloc::collections::BTreeMap`.

**Resolution:** defer the DerivationEntry slab decision to sub-commit 3 implementation analysis. Three options surface there: (a) replace `BTreeMap` with a slab-friendly per-process structure (linked list of fixed-size nodes, hash-chain, etc.); (b) keep `BTreeMap` and slab-allocate its internal node type via `Allocator` trait integration (unstable Rust feature; likely too aggressive for Phase 2); (c) accept that derivation log entries stay in the kernel heap (Principle 6: variable-size container → kernel heap; even though entries are fixed-size, the BTreeMap's internal allocations are managed by the BTreeMap, not by individual entries). Sub-commit 3 picks one with rationale.

### Final BUILD-2 cache list

Three cache types ship in sub-commit 0's interface stub and are filled in sub-commits 2–4:

| Cache | Allocation site (today) | Sub-commit |
|---|---|---|
| `Process` | `Box::new(Process { ... })` in `process::spawn_user` | sub-commit 4 |
| `Thread` | `Box::new(Self { ... })` in `Thread::new`, `Thread::adopt_current`, `Thread::new_user` | sub-commit 4 |
| `Channel` | `Arc::new(Self { ... })` in `Channel::new` | sub-commit 3 |

DerivationEntry decision lands in sub-commit 3. Handle and EndpointState are not slab-cached; they remain inline in their respective container types.

### Magazine size table (revised)

The Handle row is dropped from the magazine size table. Updated:

```rust
const PROCESS_CACHE_CONFIG: SlabConfig = SlabConfig { local_capacity: 4,  global_capacity: 16,  batch_size: 2,  zero_on_free: false };
const THREAD_CACHE_CONFIG:  SlabConfig = SlabConfig { local_capacity: 8,  global_capacity: 32,  batch_size: 4,  zero_on_free: false };
const CHANNEL_CACHE_CONFIG: SlabConfig = SlabConfig { local_capacity: 16, global_capacity: 64,  batch_size: 8,  zero_on_free: false };
```

EndpointState and Handle rows are gone. DerivationEntry config lands when sub-commit 3 picks an option.

Note: with Handle removed, no cache type currently has `zero_on_free: true`. The default-conservative discipline (Principle 7) is preserved by the trait's `SlabConfig` default; new cache types added in Phase 5+ for Tensor/Model/etc. will set `zero_on_free: true` per Principle 7.

### Rationale summary

The original cache list was specified ahead of reading the BUILD-1 code's actual allocation shapes. The Pre-BUILD-2 Checkpoint round caught the discrepancy before sub-commit 0 committed to an incompatible interface. Surfacing during checkpoint (vs. surfacing during sub-commit 2 implementation) follows the BUILD-1 amendment-subsection pattern from ADR-010.

---

## BUILD-2 amendment: sub-commit 1 invariants (post-implementation)

Surfaced during sub-commit 1 implementation and BUILD close at commit `b617acf`.
Three load-bearing invariants emerged that ADR-012's original text did not pre-specify
explicitly. Promoting to amendment text so future BUILDs can cite them by section
rather than reconstruct from chat history.

### Invariant 1: Split/coalesce never touch `allocated_pages`

The `BuddyAllocator::allocated_pages: AtomicU64` counter — the backing for
`mm::frames_in_use()` — is incremented by exactly 1 per `allocate_frame()` call
and decremented by exactly 1 per `free()` call, **regardless of the order at which
the allocation lands or the order at which coalescing terminates.**

Concretely:

- **Split path.** When `allocate_frame()` is satisfied by splitting a higher-order
  free block, the split returns one frame to the caller and pushes the remaining
  N split-products back onto lower-order free lists. `allocated_pages` increments
  by exactly 1 (for the returned frame). The split-products are still free and
  do not count.

- **Coalesce path.** When `free()` returns a frame and the buddy of that frame is
  also free, coalesce combines them into a higher-order block. This is purely
  free-list rearrangement; no caller is observing a free count change beyond the
  -1 the `free()` already accounted for. `allocated_pages` is unchanged by the
  coalesce step itself.

This invariant is what makes `frame_audit()`'s delta-tracking continue to function
unchanged across the bump→buddy migration. It is also what catches a class of
buddy bugs at test time: if a future change to the split path accidentally
double-counts (e.g., increments per split-product rather than per returned frame),
the leak audit in `spawn_neo_init` fires immediately.

**Verified at sub-commit 1 close:** baseline=final=1026 in production boot,
identical to bump-allocator behavior. Confirms the invariant holds.

**Implication for future work:** any change to `BuddyAllocator`'s split or coalesce
internals must preserve this invariant. Code review on such changes should
specifically check that only `allocate_frame()` and `free()` boundary functions
modify `allocated_pages`; internal helpers do not.

### Invariant 2: Sub-commit 1 lock baseline — outer Mutex only, no inner `Buddy.lock`

ADR-012 §implementation note 1 names `Buddy.lock` in the lock-ordering table
(`Slab.per_cpu` → `Slab.global` → `Buddy.lock`). Sub-commit 1 establishes that
this inner lock **does not exist as a code-level construct in sub-commit 1.**

Rationale: Principle 4 (SMP-shaped mechanisms land at design time) applies only
where the mechanism's invariants can be partially exercised in single-CPU. Inner
`Buddy.lock` has no caller in sub-commit 1 — slab does not exist yet. Adding the
inner lock without a caller would specify its ordering semantics against a
phantom use case, which is the opposite of letting the design crystallize against
real usage.

**Sub-commit 1 baseline:**

- `BuddyAllocator` is held inside the existing `static FRAME_ALLOC: Mutex<Option<BuddyAllocator>>`
  outer wrapper.
- All access to buddy state goes through `with_frame_alloc(|fa| ...)`, which holds
  the outer Mutex for the duration of the closure.
- No inner `Mutex<()>` field on `BuddyAllocator`.
- The `lock: Mutex<()>` field shown in ADR-012 §Q1's struct sketch is **not present**
  in sub-commit 1's actual implementation.

**When the inner lock arrives:** sub-commit 2 introduces slab caches that call into
buddy on per-CPU magazine refill (slab→buddy alloc) and on global cache overflow
(slab→buddy free in batches). At that point, the slab→buddy interaction
clarifies what the inner lock's semantics need to be — specifically, whether
slab acquires buddy through the outer wrapper (re-entering the same Mutex it
may already hold transitively) or through an interior buddy API that does not
require the outer Mutex.

Sub-commit 2's checkpoint conversation makes that decision with the slab caller
in view. Either outcome is consistent with this amendment: this amendment locks
in the sub-commit 1 baseline only, not the sub-commit 2 design.

### Invariant 3: HHDM-before-buddy-init ordering

`BuddyAllocator::new(memmap)` reads `HHDM_OFFSET` via `phys_to_virt` to install
`FreeBlock` headers in free frames at construction time. This is a new ordering
requirement that the bump allocator did not have (the bump allocator stored no
per-frame metadata at boot).

**Required ordering in `mm::init`:**

```
1. HHDM_OFFSET.call_once(|| hhdm_offset)
2. BuddyAllocator::new(memmap)
3. (subsequent mm::init steps)
```

If `BuddyAllocator::new` runs before `HHDM_OFFSET` is initialized, the kernel
panics at `[ kernel ] mm: hhdm_offset called before mm::init`. This was caught
during sub-commit 1's first production-boot attempt and fixed by reordering.
The fix is documented inline in `mm/mod.rs` near the new ordering.

**Forward implication for sub-commit 2:** slab init has a similar ordering
requirement — slab caches must initialize after buddy is operational and before
any thread allocates from a slab. The pattern generalizes:

```
mm::init ordering invariant:
  HHDM_OFFSET → buddy → slab → (anything that allocates)
```

Sub-commit 2 makes this concrete by adding `mm::slab::init()` in the right place.
A `debug_assert!` checking "slab not yet initialized" inside `Slab::alloc()` would
surface ordering bugs immediately — implementer judgment call whether to add
that or rely on the cleaner "slab static is `None` until init" approach.

This invariant is named here so that future allocator additions (e.g., Phase 5
tensor allocator integration with buddy) explicitly check their own ordering
requirements against `mm::init`.

### Closing note

These three invariants are sub-commit-1-scoped. Sub-commits 2, 3, and 4 may
add further amendment subsections as their own implementation surfaces new
load-bearing invariants. The amendment-subsection pattern (vs. silent code
drift or vs. premature ADR rewrite) continues from ADR-010 and ADR-011.

---

## BUILD-2 amendment: Channel migration revert (sub-commit 3 fix-up)

Surfaced during sub-commit 3 runtime verification at fix-up commit time.
Sub-commit 3 (commit `13d7a25`) migrated `Channel::new()` from
`Arc::new(Self { ... })` (heap) to slab-allocated via `Arc::from(slab_box)`.
The migration was reverted in the fix-up commit. CHANNEL_CACHE remains
declared and initialized; it is unused by production code.

### Root cause: Arc-vs-slab incompatibility

`Arc::from(Box<T>)` accepts a `Box<T>` and transfers its allocation into
Arc's internal storage. When the last `Arc` reference drops, Arc invokes
the **global allocator's** deallocator on the inner storage — not the
allocator the original `Box` came from. The slab's `free()` path is
never called. The freed memory returns to `linked_list_allocator` (the
kernel heap), bypassing the slab cache entirely.

This was caught by the `slab_channel_migration` test scenario's two-
invariant check:
- **Invariant 1 (alloc traverses slab):** PASS — `buddy_pages_held` went
  up on first `Channel::new()` call.
- **Invariant 2 (drop returns to slab):** FAIL — after dropping the
  `Arc<Channel>`, neither `local_in_use` nor `global_in_use` increased.

The kernel does not crash and `ipc_heap_kill_smoke` continues to PASS
because the storage *does* eventually return to buddy via the
heap→buddy path; it just bypasses the slab. The slab's accounting
silently bleeds entries that never come back.

### Implication for ADR-012's cache list

The original ADR-012 §Q3 cache list (Process, Thread, Channel) and the
BUILD-2 amendment cache list scope correction both assumed slab-vs-heap
was the only allocation-shape consideration. The Arc-vs-slab dimension
was not surfaced.

- **Channel** is the only currently Arc-wrapped type in the BUILD-2 cache
  list. Migration is reverted; CHANNEL_CACHE infrastructure remains for
  future Arc-aware integration.
- **Thread** returns `Box<Thread>` from `Thread::new`,
  `Thread::new_user`, and `Thread::adopt_current`. Sub-commit 4 can
  migrate it by routing both construction and the reaper free path
  through `THREAD_CACHE`.
- **Process** needs a separate sub-commit 4 scope correction. Its
  constructor returns `Process` by value and the registry insertion site
  wraps that value in `Arc::new(proc)`. See the "sub-commit 4 scope"
  amendment below.

### What sub-commit 4 should verify before migration

The Channel finding generalizes: sub-commit 4 must verify each candidate
type's actual construction and final-free path before migration. A type
whose last owner is `Arc<T>` cannot be slab-migrated by
`Arc::from(Box<T>)` without Arc-aware slab integration. Thread has a
direct `Box<Thread>` construction/free path; Process does not. The next
amendment records the corrected sub-commit 4 scope.

### Future-BUILD concern: Arc-aware slab integration

A complete Arc-vs-slab integration requires one of:

- **Custom Drop implementation on the wrapped type** that returns to the
  slab and uses interior raw pointers to avoid Arc's default deallocator.
- **`Arc::new_in()` with a custom `Allocator` impl** that delegates to
  the slab. Requires the unstable `allocator_api` feature; the project
  is on nightly so this is feasible but introduces a feature dependency.
- **Refactoring Channel away from `Arc<Channel>`** entirely. Significant
  IPC redesign; out of scope for Phase 2.

This decision is deferred to a future BUILD (Phase 2 BUILD-3+ or Phase
5 if tensor objects need similar Arc-wrapping). Sub-commit 3's
`slab_channel_migration` test scenario stays in-tree as a regression
pin: when Arc-aware integration lands, flipping `ignored: false` in
`tools/xtask/src/main.rs` re-enables the test, which should then PASS.

### Sub-commit 3 fix-up scope

The fix-up commit makes three changes:

1. `kernel/src/ipc/mod.rs`: `Channel::new()` reverted to `Arc::new(...)`.
   `slab` import removed. Comment block at the call site documents the
   deferral.
2. `tools/xtask/src/main.rs`: `slab_channel_migration` test scenario
   marked `ignored: true` with inline comment.
3. `kernel/src/test_harness.rs`: unchanged. The two-invariant test code
   stays as-is for future re-enablement.
4. `docs/ADR-012-...`: this amendment subsection.

CHANNEL_CACHE remains declared and initialized in `mm/slab.rs`. The
infrastructure exists, is exercised via TestObject test scenarios, and
is available for future Arc-aware migration.

---

## BUILD-2 amendment: sub-commit 4 scope (Thread migrated, Process deferred)

Surfaced during sub-commit 4 correction before commit. This amendment
corrects the Process rationale in the prior "Channel migration revert"
subsection and records the final sub-commit 4 scope.

### Migrated: Thread

Thread migrated to slab in production. The three Thread construction
sites all allocate through `THREAD_CACHE`:

- `Thread::new`
- `Thread::new_user`
- `Thread::adopt_current`

The reaper returns zombie thread storage through `THREAD_CACHE` after
dropping the owned kernel stack. The `slab_thread_migration` scenario
uses the same two-invariant pattern that caught the Channel issue:
allocation must traverse slab (`buddy_pages_held` increases or cached
entries decrease), and free must return storage to the cache
(`local_in_use` or `global_in_use` increases).

Sub-commit 4 course correction: `Thread` cache alloc/free must bracket
`THREAD_CACHE` access with `without_interrupts`. The slab per-CPU
magazine is single-CPU safe only while maskable interrupts are disabled;
the first full-suite verification exposed intermittent scheduler-smoke
timeouts when the Thread free path ran with the PIT still live. Thread
free now treats owned-stack drop plus cache return as one interrupt-masked
lifetime transition; stack allocation remains outside that critical
section because construction has not yet published the Thread to the
scheduler.

Related slab-body correction: per-CPU slab state now carries distinct
magazines for `TestObject`, `Thread`, `Process`, and `Channel`, selected
by a typed `SlabObject` trait. The earlier generic first-field cast would
have made production `THREAD_CACHE` frees share a ring with TestObject
scenarios and future Process/Channel caches. That was acceptable only
while caches were not production-owned; after Thread migration it became
a real type-isolation bug and was corrected before the full regression
sweep.

### Deferred: Process

Process did not migrate in sub-commit 4. The earlier rationale saying
Process was a direct `Box<Process>` construction path was wrong:
`Process::new_user` returns `Process` by value. The registry insertion
site then wraps that value with `Arc::new(proc)`. There is no
`Box<Process>` construction path to migrate.

Migrating Process to slab requires one of two out-of-scope changes:

- Refactor `Process::new_user` to return `Box<Process>`, then still
  solve the same Arc-vs-slab issue at the registry insertion site.
- Implement Arc-aware slab integration so the final `Arc<Process>` drop
  returns storage to `PROCESS_CACHE` rather than the global allocator.

This is the same architectural finding as the prior "Channel migration
revert" subsection, generalized to a second type: an `Arc<T>` final owner
cannot be made slab-correct by first allocating a `Box<T>` and then
converting it into `Arc<T>`.

### Cache status

`PROCESS_CACHE` remains declared and initialized in `mm/slab.rs`. The
slab machinery is exercised by the TestObject scenarios, but
`PROCESS_CACHE` is unused by production until Arc-aware slab integration
lands. This matches `CHANNEL_CACHE` after the sub-commit 3 fix-up:
infrastructure present, production migration deferred.

---

## BUILD-2 amendment: post-close summary

Surfaced after BUILD-2 closed at commit `f5a4984`. The four BUILD-2
amendment subsections above remain authoritative for their individual
findings. This subsection consolidates the load-bearing BUILD-2 lessons
into one citable summary for BUILD-3 planning.

### Consolidated finding 1: Arc-vs-slab is an allocator boundary

Sub-commit 3's Channel migration fix-up (`e754723`) proved that
`Arc::from(Box<T>)` is not slab integration. The allocation path can
traverse a slab and still fail the free-path invariant because the final
`Arc<T>` drop returns storage through the global allocator, not through
`Slab<T>::free`.

Sub-commit 4 (`f5a4984`) generalized that finding to Process. Process
never had a `Box<Process>` construction path: `Process::new_user`
returns `Process` by value, and the registry insertion site wraps it in
`Arc::new(proc)`. Even if Process construction were refactored to return
`Box<Process>`, the same Arc-vs-slab boundary would remain at registry
insertion.

BUILD-3 planning must treat Arc-aware slab integration as an explicit
architecture question, not as a call-site migration detail.

### Consolidated finding 2: cache candidates require allocation-shape audit

The original cache list was useful as a design target, but BUILD-2 proved
that a type name is not enough to justify a slab cache. The actual
question is whether the type has a direct fixed-size construction path
and a final-free path that can return storage to the same cache.

The resulting BUILD-2 status is:

| Cache | Production status after BUILD-2 | Reason |
|---|---|---|
| `THREAD_CACHE` | Live | `Thread::new`, `Thread::new_user`, and `Thread::adopt_current` allocate `Box<Thread>` storage; scheduler reaping returns zombies through `Thread::free`. |
| `PROCESS_CACHE` | Declared and initialized, unused by production | `Process::new_user` returns `Process` by value; registry ownership is `Arc<Process>`. Requires Arc-aware slab integration. |
| `CHANNEL_CACHE` | Declared and initialized, unused by production | `Channel::new` returns `Arc<Channel>`; `Arc::from(Box<Channel>)` failed the return-to-cache invariant. Requires Arc-aware slab integration. |

`PROCESS_CACHE` and `CHANNEL_CACHE` remain in-tree because the slab
mechanism is already implemented and their configs are part of the
BUILD-2 ABI surface. Their production migration is deferred, not
forgotten.

### Consolidated finding 3: `without_interrupts` is allocator discipline

Sub-commit 2 introduced the per-CPU magazine mechanism. Sub-commit 4's
full-suite verification turned the implicit single-CPU assumption into an
explicit rule: any current single-CPU caller that mutates per-CPU slab
magazine state must run with maskable interrupts disabled.

This is not Thread-specific. Thread was simply the first production
caller frequent enough to expose the race through scheduler-smoke
timeouts. The project-wide BUILD-2 rule is:

```
single-CPU per-CPU allocator state mutation => without_interrupts
```

For BUILD-3 SMP, this rule becomes the retrofit point. Disabling
maskable interrupts protects against same-CPU preemption; it does not
protect against other CPUs. SMP must add the real multi-CPU discipline
around per-CPU slab state before multiple cores can mutate allocator
state concurrently.

### BUILD-3 planning consequence

BUILD-3 must not assume "declared slab cache" means "production object
pool." After BUILD-2, the production object-pool surface is Thread only.
Process and Channel are infrastructure-present but architecturally
blocked on Arc-aware slab integration.

The open architectural question for BUILD-3 THINK is whether to:

- keep `Arc<T>`-owned objects on the global heap and remove/ignore their
  unused slab caches until a real workload justifies the complexity;
- implement Arc-aware slab allocation (`Arc::new_in` with a custom
  allocator, or an equivalent kernel-owned refcount/storage wrapper);
- refactor the affected objects away from `Arc<T>` ownership.

Until that question is answered, future code must preserve the current
status table: `THREAD_CACHE` live, `PROCESS_CACHE` and `CHANNEL_CACHE`
declared-but-unused.

### OT-8 amendment (2026-07-03): the open question is answered — removed

The fork the BUILD-3 planning consequence left open sat unresolved from
Phase 2 BUILD-2 until it became the oldest coder-actionable `mm` debt
(OT-8). The OT-8 build resolved it with the **first option: remove the
unused caches until a real workload justifies the complexity.** Evidence:

- **The wire-in was tried and failed structurally.** Sub-commit 3's
  `Arc::from(Box<Channel>)` migration bled cache entries (Arc's drop frees
  to the global allocator, never `slab::free`) and was reverted. Every
  *correct* route — `Arc::new_in` (unstable `allocator_api`, and
  `Arc<T, A>` is a different type that infects every `Arc<Process>` /
  `Arc<Channel>` signature), a kernel-owned refcount wrapper, or an
  ownership refactor away from `Arc` — is a cross-cutting change that
  cannot live inside `mm/slab`.
- **No allocation-pressure evidence.** Process and Channel construction
  are cold paths (spawn, channel setup). The hot spawn/reap type — Thread
  — is the one production cache, and it stayed.
- **The scaffolding had real carrying cost:** two dead per-CPU magazines
  allocated on every CPU at init, two `Once` statics, a permanently
  `ignored` QEMU scenario (`slab_channel_migration`) + its feature +
  registry entry, and a recurring audit/census tax.

Removed: `PROCESS_CACHE`, `CHANNEL_CACHE`, their `SlabConfig` consts, the
`SlabPerCpuState.process`/`.channel` magazines, the `SlabObject` impls for
`Process`/`Channel`, and the ignored `slab_channel_migration` regression
pin (scenario + `neo-test-slab-channel-migration` feature + xtask entry).
The status table above and the "remain in-tree" paragraph are superseded
by this amendment; the general `Slab<T>`/`Magazine<T>`/`GlobalCache<T>`
mechanism is unchanged and `THREAD_CACHE` remains live. Reversal is cheap
by construction: a config + static + `SlabObject` impl (~15 lines) plus
re-adding the scenario, if a slab-compatible refcount type ever lands.

The removal is equality-proven: none of the removed items sat on any
production path (the only readers were the ignored scenario and slab
internals), and the surviving slab surface — unit tests, the
`slab_local_to_global_promotion` / `slab_global_to_buddy_overflow` /
`slab_thread_migration` scenarios, and the host `slab-magazine-tests`
KATs — passes unchanged.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. Phase 2 BUILD-2 implements the decisions above; deviations during BUILD-2 either fold back into this ADR via amendment subsection or land as superseding ADRs.
