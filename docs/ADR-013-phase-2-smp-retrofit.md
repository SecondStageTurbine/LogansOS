# ADR-013: Phase 2 SMP Scheduler & IPC Retrofit

| | |
|---|---|
| **Status** | Accepted (Phase 2 BUILD-3 THINK; sub-commits 1-5 implement) |
| **Date** | 2026-05-05 |
| **Authority** | Project Lead |
| **Supersedes** | ADR-011 (single-CPU IPC handoff invariants) |
| **Superseded by** | — |
| **Related** | Bible Articles 4.5 (single-node scope), 7 (memory model), 8 (scheduling), 12 (capability security) · ADR-004 (IPC channel design — base) · ADR-007 (process termination — composes with the lock-order resolution) · ADR-010 (capability system foundation) · ADR-012 (buddy + slab allocators — slab cross-CPU free is a sub-commit-3b dependency) · `kernel/src/arch/x86_64/per_cpu.rs` (forward-designed for SMP retrofit; comments name the migration) · `kernel/src/sched/scheduler.rs` (lock-order inversion documented inline) · `kernel/src/mm/paging.rs` (GLOBAL bit warning) |

---

## Context

ADR-011 documented four single-CPU IPC handoff invariants and explicitly anticipated "a Phase 2 SMP retrofit ADR (likely ADR-NNN, lands as part of Phase 2 BUILD-3 SMP foundation)" as its superseder. That ADR is this one.

ADR-013 is the design substrate for Phase 2 BUILD-3: SMP foundation + IPC retrofit + advanced TLB management + capability-table multi-CPU composition. It makes nine load-bearing decisions (Q1-Q9 below), each with rationale and named alternatives. Sub-commits 1 through 5 of BUILD-3 implement these decisions; sub-commit 0 lands this ADR plus the GLOBAL-bit work that touches Q7's surface.

The kernel was forward-designed for this retrofit. `kernel/src/arch/x86_64/per_cpu.rs`, `kernel/src/sched/scheduler.rs`, `kernel/src/mm/paging.rs`, and `kernel/src/sched/process.rs` all carry inline comments naming "Phase 2 SMP must" migrations. ADR-013 makes the choices those comments anticipated and specifies the retrofit shape.

This ADR uses the **Q1/Q2/Q3 forward-looking shape** (matching ADR-004, ADR-006, ADR-010, ADR-012), not ADR-011's invariant-numbered retroactive shape. Per ADR-011's own scope note: "Q1/Q2/Q3 is the right shape for forward-looking design ADRs."

### Bible alignment

- Bible Article 4.5 (added 2026-05-05): "NeoPlutarch is single-node by design." This ADR concerns SMP within a single node; cross-node scope is explicitly excluded.
- Bible Article 8.3: "SMP is supported from Phase 2. Per-CPU state is real (per-CPU GDT/TSS/IST, per-CPU run queues, per-CPU current-thread). Global locks are forbidden in hot paths; we use per-CPU structures and lock-free or fine-grained locking." This ADR is the design that delivers on that mandate.
- Bible Article 12.1: "Capability-based security from Phase 2." Q8 below addresses how the capability table composes under multi-CPU access.

### Scope

Within scope:
- Per-CPU state replication and AP bring-up (Q1, Q2)
- Per-CPU schedulers, ADR-011 invariant resolutions (Q3, Q4, Q5)
- Lock-order resolution for `block_current_into` (Q6)
- TLB management: GLOBAL bit, CR3 cache, IPI-driven shootdowns (Q7)
- Capability-table SMP composition (Q8)
- Reaper distribution (Q9)

Out of scope (explicit anti-scope at end of document):
- Arc-aware slab integration (deferred per ADR-012 BUILD-2 post-close summary)
- Cross-CPU thread migration as a feature beyond rendezvous-driven migration
- Inference scheduling class (Phase 7)
- NUMA awareness
- PCID (Process Context Identifiers)
- Kernel RCU primitives
- Fairness shaping / vruntime / CFS

---

## Q1: Per-CPU state shape

### The question

`kernel/src/arch/x86_64/per_cpu.rs` currently defines a single `static PER_CPU: PerCpu`, with the module-level doc explicitly naming "Phase 2 SMP must replace `PER_CPU` with `[PerCpu; MAX_CPUS]` indexed by APIC ID." The migration is anticipated; the question is the precise shape.

Three sub-questions:

- **Q1.a:** What is `MAX_CPUS`?
- **Q1.b:** What fields does `PerCpu` carry post-retrofit?
- **Q1.c:** How does code identify "the current CPU"?

### Q1.a — MAX_CPUS = 64

Static cap. Each `PerCpu` is one cache line (64 bytes); the array is 4 KiB total. Trivial.

Considered alternatives:
- **8 / 16:** Too restrictive. Modern dev hardware (Threadripper, EPYC, recent server CPUs) routinely has more.
- **256:** Over-spec. 16 KiB array, no realistic Phase 2 hardware needs it.
- **Compile-time configurable:** Adds a build dimension we don't need. A constant change is cheap when it matters.

**Choice:** `MAX_CPUS = 64`. Linux's historical default. Matches realistic Phase 2 bare-metal targets (4-32 cores typical). Bumpable to 128 or 256 in a future BUILD if a real workload demands it.

The actual brought-up CPU count is `min(MAX_CPUS, what_limine_reports)`; we don't have to use the full array.

### Q1.b — PerCpu fields post-retrofit

Current PerCpu fields (per `per_cpu.rs`):
- `kernel_rsp` (offset 0, asm-load-bearing for SYSCALL entry)
- `user_rsp_save` (offset 8, asm-load-bearing for SYSCALL stack swap)
- `slab_state` (offset 16, BUILD-2 sub-commit 0 addition)
- `_padding: [u64; 5]` (pads to 64 bytes)

Post-retrofit additions:
- `cpu_id: u32` (offset 24) — sequential CPU index 0..N, used for indexing into the static array. Distinct from APIC ID.
- `active_pmap_cr3: AtomicU64` (offset 32) — current CR3 frame for the CR3 cache (Q7); 0 == kernel CR3 (no per-process Pmap loaded).
- `scheduler_ptr: AtomicPtr<PerCpuScheduler>` (offset 40) — pointer to this CPU's scheduler state, heap-allocated in the `PerCpuScheduler` struct (Q3). Set once during AP bring-up; read-only thereafter.

Total: 48 bytes used; 16 bytes padding remaining within the 64-byte cache line. Future BUILD-3 extensions (IPI deferred-work pointer, statistics counters) fit in the remaining padding without growing the cache footprint.

**Discipline rule:** PerCpu stays exactly one cache line. Anything bigger goes behind a pointer. This is what makes `gs:[OFFSET]` access cheap; we will not give that up for convenience.

Asm-load-bearing offsets stay at their established positions (0, 8) — `syscall_entry` asm references them by exact byte offset. Adding fields after them is ABI-safe.

### Q1.c — Identifying the current CPU

Two options exist:

- **Option A: GS-base + `gs:[OFFSET]`.** The current per-CPU access pattern. To find "the current PerCpu," compute the CPU's PerCpu address from `gs:[OFFSET]` (offset 0 holds `kernel_rsp`; we'd add `cpu_id` and read it via `gs:[24]`). Fast: one MOV.
- **Option B: APIC ID register read + lookup.** Read the local APIC's `APIC_ID` register and map to sequential index via a runtime-built table. Slower (LAPIC MMIO read or MSR read), but no GS-base dependency.

**Choice: Option A, with a fallback path.** GS-base is the established pattern; sub-commit 1 sets each AP's GS-base to its PerCpu slot during bring-up. `current_cpu_id()` reads `gs:[24]`. This is one instruction.

Fallback: a `cpu_id_from_apic_id()` function exists for boot-time use before GS-base is wired. Reads APIC ID, walks the (small) Limine-provided CPU list. Used in `ap_entry` exactly once per CPU before that CPU's GS-base is set.

### Choice summary (Q1)

```rust
const MAX_CPUS: usize = 64;

#[repr(C, align(64))]
pub struct PerCpu {
    pub kernel_rsp: UnsafeCell<u64>,        // offset 0  (asm)
    pub user_rsp_save: UnsafeCell<u64>,     // offset 8  (asm)
    pub slab_state: UnsafeCell<*mut ()>,    // offset 16
    pub cpu_id: UnsafeCell<u32>,            // offset 24
    _pad1: u32,                              // offset 28 (alignment)
    pub active_pmap_cr3: AtomicU64,         // offset 32
    pub scheduler_ptr: AtomicPtr<PerCpuScheduler>, // offset 40
    _padding: [u8; 16],                     // offset 48..64
}

pub static PER_CPU: [PerCpu; MAX_CPUS] = [const { PerCpu::EMPTY }; MAX_CPUS];

pub fn current_cpu_id() -> u32 {
    // Reads gs:[24]
    unsafe { core::arch::asm!(
        "mov {}, gs:[24]", out(reg) cpu_id, options(nostack, nomem)
    ); cpu_id }
}

pub fn current_per_cpu() -> &'static PerCpu {
    &PER_CPU[current_cpu_id() as usize]
}
```

(Pseudocode; exact form determined in implementation.)

---

## Q2: AP bring-up

### The question

How are secondary processors brought from firmware-parked state to running our scheduler?

### Limine MP request

Per the Limine boot protocol (v5.x and later), the bootloader supports `LIMINE_SMP_REQUEST`. When the kernel embeds this request, Limine bootstraps APs to a parked state. Each AP is described by a `limine_smp_info` struct with fields `lapic_id`, `processor_id`, `goto_address`, and `extra_argument`.

To start an AP, the BSP performs an atomic write to the AP's `goto_address`. The AP jumps to that address with:
- A pointer to its `limine_smp_info` in RDI.
- A 64 KiB stack provided by the bootloader.
- CPU state otherwise matching the BSP's at handoff (paging on, GDT loaded, etc.).

Per Limine spec: "Note: The presence of this request will prompt the bootloader to bootstrap the secondary processors. This will not be done if this request is not present."

### Choice: use Limine MP request

Bible Article 4.4: "The bootloader is Limine, not GRUB, not a custom one. Limine gives us a clean modern boot protocol with HHDM, framebuffer, modules, and SMP support without legacy BIOS baggage." Use what we said we'd use.

Considered alternative: roll our own INIT-SIPI-SIPI sequence. Rejected. More code, more testing surface, no Limine-relative benefit. The Limine MP path is well-tested and used by other production kernels.

### `ap_entry` Rust function

Each AP's startup sequence:

```rust
extern "C" fn ap_entry(info: *const limine::SmpInfo) -> ! {
    // Step 1: Determine our cpu_id from the LAPIC ID in info.
    let lapic_id = unsafe { (*info).lapic_id };
    let cpu_id = cpu_id_from_apic_id(lapic_id);

    // Step 2: Wire GS-base to PER_CPU[cpu_id].
    let per_cpu_addr = &PER_CPU[cpu_id as usize] as *const PerCpu as u64;
    unsafe { GsBase::write(VirtAddr::new(per_cpu_addr)); }
    unsafe { KernelGsBase::write(VirtAddr::new(0)); }

    // Step 3: Verify GS-base wiring via sentinel.
    // PerCpu.cpu_id was pre-written by the BSP; AP reads via gs:[24] and checks.
    let read_cpu_id = current_cpu_id();
    if read_cpu_id != cpu_id {
        panic!("AP {}: GS-base sentinel mismatch (read {}, expected {})",
               cpu_id, read_cpu_id, cpu_id);
    }

    // Step 4: Per-AP arch init (GDT, IDT, IST stacks).
    // (GDT and IDT are shared with BSP; only IST stacks are per-AP.)
    arch::init_ap_arch(cpu_id);

    // Step 5: Initialize this AP's slab_state.
    mm::slab::init_per_cpu(cpu_id);

    // Step 6: Initialize this AP's scheduler.
    // Sub-commit 1 stops here (APs spin-wait).
    // Sub-commit 2 onward: each AP enters its own scheduler loop.
    sched::init_ap(cpu_id);

    // Step 7: Adopt the AP's bootstrap thread and start the per-CPU scheduler.
    sched::run_ap_bootstrap(cpu_id);
    // Never returns; the bootstrap thread runs the reaper loop forever.
}
```

(Sub-commit 1 implements through Step 6's stub; sub-commit 2 fills in Steps 6 and 7.)

### Per-AP arch state

- **GDT:** Shared with BSP (the BSP's GDT is correct for any CPU). No per-AP GDT.
- **IDT:** Shared with BSP. No per-AP IDT.
- **IST stacks:** Per-AP. Each AP allocates its own IST stack(s) for NMI/MCE/double-fault. The TSS is per-AP because TSS.rsp0 must point to *this* AP's kernel stack.
- **TSS:** Per-AP. Allocated during `init_ap_arch`.
- **LAPIC:** Per-AP, but its register block is at a fixed physical address mapped via HHDM; no per-AP allocation.

### Discipline rule

APs do not call into any subsystem they haven't been explicitly cleared for. Until sub-commit 2, APs spin-wait at the end of `ap_entry`. Sub-commit 2 enables scheduler entry. Sub-commit 3 enables IPC. Each capability is enabled in order, so a bug in one doesn't propagate.

---

## Q3: Scheduler architecture

### The question

How do per-CPU schedulers compose? Single global queue, per-CPU + global rebalance, or per-CPU only?

### Choice: per-CPU run queues, no global

Each CPU has its own `PerCpuScheduler` struct, heap-allocated, pointer-stored in `PerCpu.scheduler_ptr`. Contents:

```rust
struct PerCpuScheduler {
    cpu_id: u32,
    current: Box<Thread>,           // running thread on this CPU
    ready: VecDeque<Box<Thread>>,   // ready queue, this CPU only
    zombies: VecDeque<Box<Thread>>, // zombies, this CPU only
    lock: spin::Mutex<()>,          // per-CPU lock
}
```

The current `static SCHED: Lazy<Mutex<Scheduler>>` is removed. There is no global scheduler lock; cross-CPU operations (rendezvous handoff, cross-CPU kill) take target-CPU locks explicitly.

### Considered alternatives

- **Single global queue (BFS-style):** Maximum simplicity. High contention on every scheduling decision. Rejected: defeats SMP.
- **Per-CPU + global rebalance queue:** Adds load balancing. Adds contention on the global. The "CPUs idle while another CPU's queue grows" problem requires it; we don't have that problem yet. Rejected for BUILD-3; deferrable.
- **Work stealing:** Each CPU steals from another's queue when idle. Real-system-grade but significant added complexity. Rejected for BUILD-3; deferrable.

### Anti-scope: no cross-CPU thread migration as a feature

The OBSERVE anti-scope said threads stick to their CPU once placed. Q4 (Invariant 1 resolution) creates the *one* exception: rendezvous-completers cause peer-migration to the completer's CPU. That migration is bounded — only on rendezvous, never on load balance.

Load balancing, work stealing, NUMA-aware placement: all deferred to future BUILDs. BUILD-3's correctness goal is "multi-CPU works"; performance optimization comes later.

### Per-CPU bootstrap thread

Sub-commit 2's `run_ap_bootstrap(cpu_id)` adopts a per-AP bootstrap thread (`Thread::adopt_current("ap-N-main")`) and runs the per-CPU reaper loop on it (Q9). Same mechanism as the BSP's `Thread::adopt_current("main")`, replicated per-AP.

### Test scenarios that anchor this choice

Two new scenarios in sub-commit 2:
- `smp_per_cpu_scheduler_smoke` — `-smp 4`, each AP spawns and runs a thread, all four threads complete, all reap correctly.
- `smp_same_cpu_ipc` — IPC test confined to a single CPU on each AP independently.

These verify per-CPU schedulers work in isolation before cross-CPU IPC enters the picture in sub-commit 3.

---

## Q4: ADR-011 Invariant 1 resolution — wake strategy under SMP

### The question

ADR-011 Invariant 1 specifies the single-CPU rendezvous handoff: `wake_thread_front` places the woken peer at the head of the (single) ready queue, then `yield_now` rotates the completer to the back and switches to the front. Under SMP, "front of the ready queue" has no global meaning — each CPU has its own.

ADR-011 enumerated three replacement options:
- **(a)** Wake the peer on the same CPU as the rendezvous-completer; soft front-queue local; no IPI.
- **(b)** Wake the peer on its previously-running CPU via IPI; lose the synchronous "completer yields to peer" property.
- **(c)** Drop the ordering guarantee entirely and require explicit synchronization primitives in userspace.

### Choice: (a) wake-on-completer-CPU

When a sender on CPU0 completes a rendezvous with a receiver in `RecvBlocked` previously running on CPU1:
1. The receiver's `Box<Thread>` is taken out of the EndpointState.
2. The `Box<Thread>` is enqueued at the front of CPU0's local scheduler ready queue.
3. The completer yields locally — the receiver runs on CPU0, on CPU0's local schedule.

Cross-CPU thread migration happens; it is bounded (rendezvous-driven only, not load-driven).

### Rationale

**Preserves the property tests depend on.** Consumer-first handoff (per ADR-011's BUILD-1 amendment) is a property production boot proved was needed — without it, `sys_kill` racing with rendezvous completion produces non-deterministic behavior visible to userspace. Option (a) preserves this property *locally*: on the completer's CPU, the woken peer reaches a stable post-rendezvous state before any other thread runs, exactly as in single-CPU.

**No IPI dependency.** Sub-commit 2 (per-CPU schedulers) ships before sub-commit 4 (IPI infrastructure for TLB shootdowns). Option (b) requires IPI for every cross-CPU rendezvous, which would force IPI infrastructure into sub-commit 2 or 3. Option (a) avoids this dependency cycle.

**Aligns with anti-scope.** The single bounded exception to "threads stick to their CPU" is rendezvous-driven migration. This is a controlled escape valve, not a generalized cross-CPU thread movement mechanism.

**Bounded cost.** Thread migration follows rendezvous frequency. Phase 1.5c kernel doesn't have server-shaped workloads that would make this hot. By Phase 5+ (inference syscalls) or Phase 4 (network handlers) when migration cost might matter, cache-locality optimization is its own conversation.

### Costs accepted

**Cross-CPU slab traffic.** A Thread allocated from CPU0's `THREAD_CACHE` may be freed on CPU1 (after migration + exit). BUILD-2's slab assumed single-CPU; sub-commit 3b ships the slab cross-CPU free path. See Q6's discipline notes.

**Cache-line bouncing.** A Thread's working set follows it; migration causes L1/L2 misses on the destination CPU. Acceptable given rendezvous frequency in current workloads.

**Thread.cpu_id tracking.** A new field. Update sites: thread allocation, migration, statistics. Bounded.

### Considered alternatives — why rejected

**Option (b) IPI to peer's previous CPU.** Pulls IPI infrastructure forward. Adds ~1µs latency to every cross-CPU rendezvous (IPI delivery). Loses synchronous handoff property — tests that depended on it need rewriting. Lower steady-state thread-migration cost once it's working, but BUILD-3's correctness goal doesn't justify the complexity.

**Option (c) drop ordering guarantee.** Maximum kernel simplicity, maximum userspace complexity. Every existing test that depended on Invariant 1 breaks; rewriting them with futex-style barriers requires futex infrastructure. Loses the consumer-first property the BUILD-1 amendment specifically added because production boot showed it was needed. ALTIMIT-faithful in spirit (less in the kernel) but breaks established userspace contracts.

### Consumer-first under SMP

ADR-011's BUILD-1 amendment specified consumer-first handoff. Translating to SMP under Option (a):

- **Sender completes a waiting receiver (consumer is the receiver).** Receiver's `Box<Thread>` migrates to sender's CPU. Sender front-queues the receiver and yields. Receiver runs first on sender's CPU. *Same as single-CPU semantics, on the sender's CPU.*

- **Receiver completes a waiting sender (consumer is the receiver — still running).** Sender's `Box<Thread>` migrates to receiver's CPU. Receiver front-queues the sender but does NOT yield — receiver continues running. Sender waits in receiver's CPU's ready queue. *Same as single-CPU consumer-first semantics, on the receiver's CPU.*

- **Grant rendezvous (recipient is the consumer per ADR-010).** Same shape as receive: recipient consumer keeps running; grantor migrates and queues.

In all cases, consumer-first is preserved on a per-rendezvous basis. No cross-CPU IPC operation breaks the property.

### Implementation note: the migration mechanic

When CPU0's IPC syscall finds CPU1's blocked Thread, the Thread is parked inside an EndpointState variant (e.g., `RecvBlocked { ..., waiter: Box<Thread> }`). Taking it out is just `mem::replace`. No migration concern at this stage — `Box<Thread>` is a heap pointer that any CPU can hold.

The migration happens at the enqueue step: CPU0 acquires CPU1's scheduler lock briefly only if the Thread tracked which CPU "owns" it for accounting. We don't track this — once a Thread is in an EndpointState, no scheduler queue references it. The Thread is genuinely homeless during the block. Sub-commit 3b enqueues it on CPU0's queue, period.

The Thread's `cpu_id` field is updated at enqueue time to record its new home. Future operations on this Thread (kill, wake, etc.) check `cpu_id` to find the right scheduler.

### Test scenarios anchoring Q4

- `smp_cross_cpu_ipc_smoke` (sub-commit 3b) — sender BSP, receiver AP1; rendezvous succeeds; receiver runs on BSP after.
- `smp_cross_cpu_kill_blocked` (sub-commit 3b) — receiver blocked on AP1, sender BSP issues `sys_kill`; cross-CPU yank works.
- Anti-regression: `ipc_heap_kill_smoke` PASSes with sender BSP and receiver AP1.

**Closed 2026-05-10** via SC-3B (Phase 2 BUILD-3 sub-commit 3). Closure commit: `e2b3f4f`. Sub-step 5c — IRQ-safe acquisition for `Channel.state`. Investigation arc and structural lessons recorded in `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`.

---

## Q5: ADR-011 Invariants 2/3/4 disposition

### The question

Invariants 2, 3, 4 collectively define the deferred-quantum mechanism (`HANDOFF_PREEMPT_DEFER_TICKS`, `Thread.preempt_defer_ticks`, `consume_preempt_defer`, `clear_current_preempt_defer`). Under SMP, do we remove or repurpose?

### Choice: remove entirely

Sub-commit 2 deletes:
- The `HANDOFF_PREEMPT_DEFER_TICKS` constant in `scheduler.rs`.
- The `Thread.preempt_defer_ticks` field.
- `consume_preempt_defer()` and its call from `on_tick()`.
- `clear_current_preempt_defer()` and its call from `syscall_dispatch`.
- `defer_current_preempt_for_handoff()` and its call from `wake_thread_front`.

Net: ~30 lines deleted across `scheduler.rs`, `thread.rs`, `syscall.rs`. Three call sites updated. Net code reduction.

### Rationale

The deferred-quantum mechanism patched a specific single-CPU race (BUILD-4 finding: stale PIT tick on the same CPU as the just-woken peer, fired in the narrow window between wake-of-peer and yield-by-completer). Under SMP with Q3's per-CPU schedulers + Q4's wake-on-completer-CPU:

1. CPU0's sender finds CPU1's receiver in `RecvBlocked`. (Critical section under CPU1's scheduler lock to extract; not relevant to the race.)
2. CPU0 takes its own scheduler lock. IF=0 (per `block_current_into` discipline). Front-queues the receiver, yields. Atomic critical section.
3. Context switch to receiver on CPU0. Receiver is current; sender is back-queued on CPU0.

Steps 2-3 happen with IF=0. CPU0's local PIT cannot fire between step 2 (front-queue) and step 3 (yield) because IF=0. After step 3, the receiver is current; CPU0's PIT may fire and preempt normally — but that's just standard preemption of a normally-running thread, no race. The race the deferred quantum patched does not exist in this architecture.

CPU1's PIT can fire concurrently with steps 2-3 because CPU1 is independent. But CPU1's PIT preempts CPU1's current thread (which is whatever CPU1 is running, not the migrating receiver). Independent. No race.

### Considered alternative — repurpose

ADR-011's option (b): repurpose the mechanism as a per-CPU "newly-woken thread gets bounded preempt-immunity for fairness shaping." Rejected because:

- The current mechanism patches a specific race; repurposing for vague future need is over-fit.
- Future fairness work (vruntime, deficit round-robin, etc.) will want different mechanisms. Locking in `preempt_defer_ticks` semantics now constrains future design without benefit.
- Code-debt: the mechanism's name and existence document a single-CPU race that no longer exists. Future readers would have to read SMP-era code to understand a single-CPU patch's repurposing.

**Discipline.** When a mechanism's original justification disappears, delete it. Repurposing is a doc-debt trap.

### Risk

I claimed the race the deferred quantum patched does not exist under Q3+Q4. If wrong, `ipc_heap_kill_smoke` fails non-deterministically under SMP — exactly the same way it failed before BUILD-4 fixed it.

**Mitigation.** Sub-commit 2 closure gate: `ipc_heap_kill_smoke` runs 100x consecutively on `-smp 4` QEMU; flake rate of 0 across all runs is required for sub-commit 2 to close. If any flake observed, the claim is wrong and we either re-introduce the deferred-quantum mechanism (option b after all) or find what we missed.

### Test scenarios anchoring Q5

- Sub-commit 2 verification: `ipc_heap_kill_smoke` runs 100x on `-smp 4` post-deletion. Zero flakes required.
- Sub-commit 2 verification: All Phase 1.5b/1.5c scenarios PASS unchanged. They depended on Invariants 1-4 jointly under single-CPU; under SMP, Invariant 1 is replaced by Q4 and Invariants 2-4 are removed; the scenarios should still PASS because the architectural change preserves the semantics they actually depended on (consumer-first ordering, deterministic kill-on-Blocked).

---

## Q6: Lock-order resolution for `block_current_into`

### The question

`kernel/src/sched/scheduler.rs:block_current_into` carries an inline comment that explicitly names the SMP lock-order inversion problem:

> The matching *wake* path (`wake_thread` called from inside an IPC syscall) takes the holder's lock first, then SCHED. That is a classic lock-order inversion and would deadlock on SMP.

ADR-011 implicitly endorsed two named resolutions:
- **(a)** Acquire both locks in a fixed order with a per-channel-then-SCHED rule and a parking-without-SCHED-held primitive.
- **(b)** Adopt a release-on-the-new-stack pattern (ManuallyDrop + asm release) so the SCHED-held window is closed before any other CPU can reach it.

### Choice: (b) ManuallyDrop + asm release

Sub-commit 3a restructures `block_current_into` so the per-CPU scheduler lock is **released on the new stack**, after `context_switch` completes, before control returns into the new thread's code.

### Mechanic

```rust
// Conceptual; exact form determined in implementation review.
pub fn block_current_into<F>(installer: F) -> i64
where F: FnOnce(Box<Thread>),
{
    without_interrupts(|| {
        let sched = current_scheduler();
        let guard = sched.lock.lock();

        let next = sched.ready.pop_front()
            .expect("block_current_into: no ready thread");
        let mut prev = mem::replace(&mut sched.current, next);
        prev.state = ThreadState::Blocked;
        sched.current.state = ThreadState::Running;
        let next_rsp = sched.current.rsp;

        // Cross-CPU bookkeeping (unchanged from sub-commit 2).
        if prev.pid != sched.current.pid {
            cross_process_prep(&sched.current);
        } else if sched.current.pid != KERNEL_PID {
            cross_thread_kstack(&sched.current);
        }

        // Capture rsp slot before installer takes ownership.
        let prev_rsp_ptr: *mut u64 = &mut prev.rsp;

        // Hand the box to the installer; do NOT drop the guard.
        installer(prev);

        // Convert the guard to a raw lock-release token.
        // This is a `&AtomicBool` or `&Mutex<()>` reference whose
        // unlock semantic is a single store. The asm wrapper performs
        // that store after switching stacks.
        let lock_addr = guard.into_raw_lock_ptr();
        mem::forget(guard); // explicit: prevent normal drop

        // SAFETY: see context_switch_with_release contract.
        // The new thread's stack is established by context_switch;
        // immediately on the new stack, the lock release fires;
        // only then does the new thread's code resume.
        unsafe { context_switch_with_release(prev_rsp_ptr, next_rsp, lock_addr); }

        // Resumed here. Our blocked_return slot was written by the waker.
        current_scheduler().current_blocked_return()
    })
}
```

The asm wrapper `context_switch_with_release` is the load-bearing piece. Its contract:

1. Save current thread's callee-saved registers + flags + rsp into `*prev_rsp_ptr`.
2. Load `next_rsp` into RSP. Stack is now the new thread's.
3. **First action on the new stack:** release the lock at `lock_addr` (single store, e.g., `mov [lock_addr], 0`).
4. Pop callee-saved registers + flags from the new stack.
5. Return into the new thread's code.

Step 3 is the critical addition. Before the new thread's code can run, the old thread's scheduler lock is released — meaning any other CPU waiting on that lock can now make progress. Crucially, this happens after the stack switch, so the old thread is no longer using the old stack; another CPU can wake the old thread (push it back onto a ready queue, write to its `blocked_return` slot) without racing on the old thread's stack memory.

### Multiple-lock release

The conceptual sketch above shows a single lock release. In practice, IPC syscalls hold **two** locks when entering `block_current_into`:

- The Channel's `state: Mutex<[EndpointState; 2]>` (acquired by the IPC syscall before the rendezvous-state read).
- The per-CPU scheduler lock (acquired inside `block_current_into`).

The installer closure mutates the EndpointState (inserting the parked `Box<Thread>` into the relevant `*Blocked.waiter` slot), so the Channel mutex must be held across the closure call. Both locks must be released on the new stack — the Channel mutex first (so wakers on other CPUs can complete the rendezvous), then the scheduler lock.

The asm wrapper takes a small fixed-size release list:

```rust
unsafe fn context_switch_with_release(
    prev_rsp_ptr: *mut u64,
    next_rsp: u64,
    locks_to_release: &[*const AtomicBool], // released in order, max 4 entries
) -> i64;
```

Realistic upper bound is 4 release entries; future syscalls inheriting this primitive get the same bound. Sub-commit 3a's pre-implementation checkpoint specifies the exact ABI.

### Rationale

**Isolated change.** Resolution (a) requires modifying every IPC syscall site: `sys_send`, `sys_recv`, `sys_grant`, `sys_recv_grant`, plus future ones. Each gets restructured to take the holder's lock first, do all combined work, drop the scheduler lock early. That's a large fan-out of careful changes; one missed site breaks SMP.

Resolution (b) is one place: `block_current_into` plus its asm helper. IPC syscalls don't change. Future syscalls using the same primitive inherit the resolution automatically.

**Well-known pattern.** ManuallyDrop + asm release is a standard kernel technique. L4 family, Mach, seL4 all use a variant of "release the lock after switching stacks." We are not inventing.

**Composes with Q4.** Cross-CPU rendezvous under Q4 (wake-on-completer-CPU) involves the *completer's* CPU acquiring the *Channel mutex* to extract the blocked Thread from its EndpointState. The Channel mutex is the single point of synchronization for the rendezvous itself; the per-CPU scheduler locks remain CPU-local. Q6's resolution applies to `block_current_into` (the path that puts threads to sleep), and the released locks include both the scheduler lock and any mutex held across the installer call — typically the Channel mutex.

### Risk

The asm release is the highest-correctness-risk work in BUILD-3. Lock release on the new stack is a load-bearing invariant; getting it wrong means lost wakeups or deadlocks under SMP. Mitigations:

- Pre-implementation checkpoint conversation specifically focused on the asm contract.
- Explicit code review by project lead before merge.
- The asm is short (~30 lines after expansion to handle multiple locks).
- Sub-commit 3a enables only same-CPU testing; sub-commit 3b enables cross-CPU. Two-stage rollout exposes bugs at a smaller surface first.

### Considered alternative — why (a) was rejected

Resolution (a) "acquire both locks in fixed order, park without SCHED held" requires:
1. Every IPC syscall acquires Channel mutex first.
2. While Channel mutex is held, IPC syscall does the rendezvous-state read, decides to block, releases Channel mutex, *then* enters `block_current_into`.
3. `block_current_into` re-acquires whatever locks the new state needs, parks the thread, switches.
4. The wake path must coordinate: before waking, the thread state must already be in EndpointState, but the parking happens after the EndpointState mutation — so there's a window where the thread is "logically blocked" but not yet "physically parked."

The "logically blocked, physically running" window is where the bugs hide. It's a known-ugly pattern. Resolution (b) avoids it by keeping both locks held across the switch.

### Considered alternative — why not redesign IPC

A more radical option: redesign IPC entirely so the rendezvous machinery is lock-free or uses different primitives (channels-as-MPMC-queues, futex-style userspace cooperation, etc.). Rejected because BUILD-3's job is the SMP retrofit, not IPC redesign. ADR-004 + ADR-010's IPC design is what we're keeping; this resolution is the smallest change that makes that design SMP-correct.

### Test scenarios anchoring Q6

- `smp_block_release_smoke` (sub-commit 3a) — same-CPU rendezvous in tight loop on each CPU; verifies the new pattern doesn't introduce regressions in single-CPU semantics.
- All same-CPU IPC tests (BUILD-2's full suite) PASS under sub-commit 3a — the lock-order resolution is invisible to single-CPU correctness.
- `smp_cross_cpu_ipc_smoke` (sub-commit 3b) — cross-CPU rendezvous works without deadlock; verifies the multi-lock release fires in correct order.
- `smp_cross_cpu_block_release_stress` (sub-commit 3b) — high-rate cross-CPU rendezvous loop on `-smp 4`; verifies no lock leaks or deadlocks under contention.

**Closed 2026-05-10** via SC-3B (Phase 2 BUILD-3 sub-commit 3). Closure commit: `e2b3f4f`. Sub-step 5c — IRQ-safe acquisition for `Channel.state`. Investigation arc and structural lessons recorded in `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`.

---

## Q7: TLB management — GLOBAL bit, CR3 cache, IPI shootdowns

### The question

Three independent but related TLB concerns:
- **Q7.a:** GLOBAL bit on kernel mappings.
- **Q7.b:** Per-CPU "last loaded CR3" cache to skip no-op CR3 writes.
- **Q7.c:** Cross-CPU TLB shootdowns when one CPU mutates a Pmap active on another.

### Q7.a — GLOBAL bit on kernel mappings

#### Background

Per `kernel/src/mm/paging.rs:sanity_check`:

> WARNING - GLOBAL bit not set on kernel .text. CR3 reload on cross-process switch will flush kernel TLB entries. Phase 2 should set GLOBAL on kernel mappings (Limine config or post-boot fixup).

The x86_64 GLOBAL bit, when set on a leaf page-table entry, exempts that mapping from being flushed when CR3 is reloaded. PCIDE/PCID interactions aside (we don't enable PCID in BUILD-3), GLOBAL is the standard mechanism for keeping kernel TLB entries valid across user-process switches.

Without GLOBAL, every cross-process CR3 reload flushes the entire TLB, including kernel mappings. The kernel's working set has to be re-walked. Under SMP this gets compounded by cross-CPU TLB shootdowns affecting kernel-mapped pages too.

#### Limine reality

Per Limine boot protocol v5.x-v9.x (researched 2026-05-05 via the Limine repo on GitHub), kernel mappings are set up by Limine with "supervisor, read, write, execute" permissions. The protocol does not specify GLOBAL, and based on the protocol's flag enumeration, Limine does not set GLOBAL on kernel mappings.

This means BUILD-3 must do post-boot fixup. The fixup runs once during `mm::init`, after Limine's page tables are in place but before any cross-process CR3 reload happens.

#### Choice: post-boot fixup walking PML4 entries 256-511

Sub-commit 0 ships a function in `mm/paging.rs` that:

1. Reads the active PML4 (kernel's PML4) via the existing `active_level_4_table(hhdm_offset)` helper.
2. Iterates entries 256-511 (the kernel half of the 48-bit canonical address space, per Bible Article 7.2).
3. For each PRESENT entry, recurses into the PDPT and walks down to PD/PT level (or huge-page leaves at PD level for 2MiB pages, PDPT level for 1GiB pages).
4. On every PRESENT leaf entry, sets the GLOBAL bit. PML4 entries themselves are not leaves; GLOBAL applies to leaves only.
5. After the walk, executes `mov cr4, cr4` (or equivalent) to ensure the GLOBAL bit's effects are observed; specifically, set CR4.PGE if not already set (it should be — it's required for GLOBAL to mean anything).

The fixup runs on the BSP only, before any AP is brought up. APs inherit the same kernel-half PML4 (per Pmap construction's "PML4-entry copy with shared lower tables" — see `kernel/src/mm/pmap.rs`); the GLOBAL bits on leaf entries are visible to every CPU through the shared sub-tables.

**Critical ordering:** the fixup must happen before the first `Pmap::new()` is called, because Pmap construction copies the kernel-half PML4 entries into each new user Pmap. If GLOBAL bits are set after some Pmaps already exist, those Pmaps' kernel-half views are stale. Sub-commit 0 inserts the fixup early in `mm::init`, before any process is spawned.

#### Eligibility — what gets GLOBAL

All kernel-half mappings get GLOBAL because the kernel half is shared across every user Pmap by construction. Specifically:

- Kernel `.text`, `.data`, `.bss` (Limine-mapped at the requested high-half virtual address).
- HHDM region (Limine-provided direct map of physical memory at the HHDM offset).
- IST stacks.
- Kernel heap region.
- Any future kernel-half mapping created by `mm::init`.

#### What does NOT get GLOBAL

User-half mappings (entries 0-255). User mappings are per-process; setting GLOBAL on them would retain stale entries across process switches.

#### Risk

If a kernel-half mapping is *not* actually stable across processes (e.g., a per-CPU MMIO mapping that should differ per-CPU), GLOBAL on it retains a stale TLB entry.

**Mitigation.** No such mappings currently exist. The fixup logs each leaf it processes; review the log on first SMP boot to verify no unexpected leaves are present. If Phase 5+ adds per-CPU kernel mappings (e.g., per-CPU LAPIC MMIO), this fixup must be revisited — but those mappings should be per-AP-allocated, not in the shared kernel half, to avoid this concern entirely.

#### Test scenarios

- Sub-commit 0 verification: post-fixup, `paging::sanity_check` reports `global=true` for the kernel `.text` sample.
- New scenario `tlb_kernel_global_smoke` — boot with the fixup, perform a cross-process CR3 reload, verify a known kernel page hit doesn't trigger a TLB miss (via QEMU's `info tlb` or a microbenchmark).

### Q7.b — Per-CPU CR3 cache

#### Mechanism

Per `kernel/src/sched/scheduler.rs:cross_process_prep`:

> Unconditional CR3 write. `cr3_for_thread` returns kernel CR3 for KERNEL_PID, the user process's Pmap CR3 otherwise — caller doesn't branch. Forces a TLB flush either way; Phase 1.5 can optimize by skipping no-op writes ("last loaded CR3" cache).

`PerCpu.active_pmap_cr3: AtomicU64` (per Q1.b) tracks this CPU's currently loaded CR3 frame. Before `Cr3::write` in `cross_process_prep`:

```rust
let (frame, flags) = cr3_for_thread(next.pid);
let new_cr3 = frame.start_address().as_u64();
let cur_cr3 = current_per_cpu().active_pmap_cr3.load(Ordering::Relaxed);
if new_cr3 != cur_cr3 {
    unsafe { Cr3::write(frame, flags); }
    current_per_cpu().active_pmap_cr3.store(new_cr3, Ordering::Relaxed);
}
```

#### Rationale

Same-process thread switches (multiple threads in one user process — Phase 1.5+ has these) skip the CR3 write and the associated TLB flush. The check is one PerCpu field read + one comparison.

#### Risk — debug-assert paranoia

If the cache says "this CR3 is current" but it's not (e.g., due to a missed update), we skip a write we shouldn't have. Stale TLB entries follow.

**Mitigation.** Under `cfg!(debug_assertions)`, before skipping the write, read CR3 directly and compare to the cached value. Panic on mismatch. This catches cache-coherence bugs in development without paying the read cost in release builds.

#### Test scenarios

- New scenario `smp_cr3_cache` (sub-commit 4) — instrumentation counts CR3 writes; same-process thread switches don't increment.
- Anti-regression: cross-process switches still work correctly (verified by all existing IPC tests under `-smp 4`).

### Q7.c — Cross-CPU TLB shootdowns

#### The problem

When CPU0 mutates a Pmap that is currently active on CPU1, CPU1's TLB has stale entries. Phase 1.5+ multi-threaded user processes make this real: thread T1 on CPU0 and thread T2 on CPU1 share a Pmap; T1 calls `sys_map_region` which mutates the Pmap; T2's TLB now has stale entries.

Without shootdown, T2 continues to access stale mappings. This is silent corruption — wrong memory reads with no crash.

#### Mechanism

**Per-Pmap active-CPUs bitmap.** Each Pmap carries an `AtomicU64` (one bit per CPU, up to 64 — matches MAX_CPUS). The bit is set when a CPU loads this Pmap's CR3; cleared when it switches to a different Pmap.

```rust
pub struct Pmap {
    // ... existing fields ...
    pub active_on_cpus: AtomicU64,
}
```

Update sites:
- `cross_process_prep` sets `active_on_cpus |= (1 << current_cpu_id())` *after* `Cr3::write`.
- Same function clears the bit on the previous Pmap *before* the write: `prev_pmap.active_on_cpus &= !(1 << current_cpu_id())`.

Order matters: clear the old bit first (so a concurrent shootdown doesn't think we're on the old Pmap when we've moved off it), then write CR3, then set the new bit. There's a small window between the clear and the new-set where the CPU is "transitioning" — both Pmaps see this CPU as not-active. Conservative: a shootdown sender that finds itself in this window should over-IPI rather than under-IPI. Implementation detail for sub-commit 4.

**Mutator path.** Any function that modifies a Pmap (`map_user`, `unmap_user`, `allocate_user_region`, future Phase 5+ tensor mapping) does, after the modification:

1. Read `target_pmap.active_on_cpus` atomically.
2. For each set bit other than `current_cpu_id()`, publish a shootdown request to that CPU and send a shootdown IPI.
3. Wait for ack from each target CPU before returning. If a target has not consumed its pending request after the first 1 ms window, re-send the IPI once; panic only if the final bounded wait still leaves ACKs pending.

**Receiver path.** Each CPU has an IPI handler (new IDT vector, dedicated to TLB shootdowns) that:

1. Reads the shootdown request from its per-CPU pending-shootdown slot. The current SC-3C implementation serializes senders behind a global shootdown lock, so a single slot plus a per-shootdown ACK counter is sufficient; a future batching implementation may replace this with the originally sketched bounded ring.
2. For specific-page shootdowns: `INVLPG [addr]`. For full flushes: `MOV CR3, CR3` (which flushes everything except GLOBAL — exactly what we want).
3. Acks via an atomic counter the sender polls.

#### IPI infrastructure

This is the first IPI consumer in BUILD-3 (sub-commit 4). Components:

- **APIC IPI delivery.** Write to LAPIC's ICR (Interrupt Command Register) to send an IPI to a specific destination CPU's vector.
- **New IDT vector for shootdown.** Allocated from the available range. Sub-commit 4 picks one (likely `0xFB`, leaving room above for future per-CPU IPIs).
- **IPI handler.** Runs on the standard kernel stack. Disables interrupts on entry (handled by the IDT gate), processes pending shootdowns, EOIs the LAPIC, returns.
- **Per-CPU pending-shootdown slot.** Current implementation uses one slot per CPU and a global sender lock. Senders publish the stack-local payload pointer, send an IPI, and wait for the target to swap the slot to null and ACK. This keeps the correctness baseline small; batching/ring overflow semantics remain deferred optimization work.

#### Considered alternatives

**Synchronous shootdown without ack.** Sender IPIs and continues without waiting. Reduces latency but loses correctness — the mutator might continue and assume the mutation has taken effect on all CPUs when CPU1 hasn't yet processed the IPI. Rejected.

**Lazy shootdown (delay until next page fault).** When CPU1 takes a page fault on a stale-TLB-induced wrong access, fault handler refreshes TLB. Significantly more complex; introduces fault-driven correctness; only safe under specific conditions. Deferred.

**Deferred batching.** Multiple shootdowns from the same mutator coalesce. Optimization. Sub-commit 4 ships synchronous shootdowns first; batching is a later improvement if benchmarks justify.

**PCID (Process Context Identifiers).** x86_64 feature for fine-grained TLB tagging. Allows multiple Pmaps' TLB entries to coexist without flushes. Real win but significant complexity; explicit anti-scope per BUILD-3 plan. Future BUILD.

**Blind timeout bump.** Raising the panic deadline without changing the delivery protocol was rejected after CI exposed QEMU/TCG SMP ACK-latency flakiness on 2026-06-02. The accepted policy keeps the original 1 ms window as an intermediate re-poke point, then uses a larger final deadline only after the target has been re-signaled. This preserves the missing-ACK panic for genuinely non-responsive CPUs while tolerating emulated-vCPU scheduling tails.

#### Test scenarios

- `smp_ipi_smoke` (sub-commit 4) — BSP sends IPI to AP1, AP1's handler counts it; verify count.
- `smp_tlb_shootdown_smoke` (sub-commit 4) — multi-threaded process shares Pmap across CPUs; one thread mutates Pmap (`sys_map_region`); the other thread's next access to the affected page sees the new mapping.
- `smp_tlb_shootdown_stress` (sub-commit 4) — high-rate Pmap mutation under `-smp 4`; verifies no shootdown loss, no ring overflow.

#### Risk

**Missed shootdown is silent corruption.** Stale TLB → wrong memory read, no crash. Mitigation: conservative bitmap tracking — when in doubt, IPI everyone; explicit test that reads after mutate cross-CPU.

**IPI handler correctness.** New IDT vector entry; new handler. Standard kernel stack; must EOI the LAPIC; must not block. Mitigation: short, well-reviewed handler; explicit test for IPI delivery latency and correctness.

**ACK-latency tails under emulated SMP.** QEMU/TCG can leave exactly one target AP late enough to miss a 1 ms sender spin deadline even though the CPU is not wedged. Mitigation: one intermediate re-poke after 1 ms, a final 50 ms deadline with panic-on-timeout, and instrumentation that records target ACK latency plus retry sends in the q7c synthetic round-trip scenario.

**Sender-lock waits and temporarily unserviceable CPUs.** The global lock serializes senders so each target's single pending slot cannot be overwritten, but the TLB IPI handler does not acquire that lock. The sender lock therefore uses a plain `SwitchMutex`; a contending caller temporarily enables interrupts while waiting and restores its original IF state immediately after acquisition. An IRQ-disabling wait is forbidden: CPU A can hold the lock and wait for CPU B's ACK while CPU B waits for the same lock; if B masks interrupts before spinning, B cannot enter the IPI handler and the cycle ends in `tlb_shootdown_timeout`. A CPU entering `spawn_user` first acquires the sender lock while it is still serviceable, then masks IF and withdraws its shootdown-ready bit under that lock. This drains every sender that could have sampled the old bit and prevents a new sender from observing a ready-but-unserviceable target. The existing IRQ-off ELF/Pmap/publication transaction then runs unchanged. Rejoin also occurs under the sender lock: with IF still masked, the CPU performs a full local TLB flush covering every mapping change made while it was absent, republishes readiness, releases the lock, and immediately enables interrupts. A sender can only proceed after the release, so an IPI sent in the release-to-`sti` interval remains pending and is serviced once IF becomes 1; an explicit per-CPU preemption guard prevents timer-driven context switches while an IRQ-off caller temporarily enables IF to acquire the rejoin lock. Thus the registry-visible-but-IF=0 state is production-reachable, but it is not a legal remote target; after readiness is republished and before first ring-3 execution, the CPU is targetable and handles shootdowns normally. `swapgs_irq_ring3` separately proves IF=0 sender-lock contention and restoration, zero IPI delivery during the quiesced publication window, a real ACK immediately after readiness rejoin but before ring 3, a reschedule IPI observed from ring 3, and 128 repeated ring-3 TLB IPIs with EOI progress.

**Slot pressure.** Current SC-3C code serializes senders behind a global shootdown lock, so the single pending slot cannot overflow under the shipped protocol. A future high-rate batching design may reintroduce a bounded ring; its overflow mitigation remains a full-flush sentinel.

**Closed 2026-05-10** via SC-3B (Phase 2 BUILD-3 sub-commit 3). Closure commit: `e2b3f4f`. Sub-step 5c — IRQ-safe acquisition for `Channel.state`. Investigation arc and structural lessons recorded in `Plans/CLOSE-REPORT-2026-05-10-sc-3b.md`.

---

## Q8: Capability-table SMP composition

### The question

The cross-process revoke walk in `kernel/src/sched/process.rs` currently iterates the `PROCESSES` registry while holding per-process derivation locks. Under SMP, this walk acquires multiple per-process locks across CPUs. How do these compose?

Three sub-questions:
- **Q8.a:** Lock structure on the `PROCESSES` registry.
- **Q8.b:** Lock-order rule for the cross-process revoke walk.
- **Q8.c:** Cross-CPU `kill_thread_by_pid` blocked-thread walk.

### Q8.a — `RwLock` on PROCESSES registry

#### Choice

Replace `static PROCESSES: Lazy<Mutex<BTreeMap<ProcessId, Arc<Process>>>>` with `static PROCESSES: Lazy<RwLock<BTreeMap<ProcessId, Arc<Process>>>>`.

Read paths (process lookup by pid — the common path) take the read lock. Write paths (register / deregister) take the write lock.

#### Superseded at SC-4 (commit `57f3cb1`)

Q8.a's `RwLock` selection was superseded by FM-D5.4's single-`Mutex` approach. Implementation at SC-4 D5 refactored `Thread` to carry `Arc<Pmap>` directly, removing PROCESSES from the SMP hot path entirely — the read-heavy contention that motivated `RwLock` no longer exists. PROCESSES remains correct with a single `Mutex` at NeoPlutarch's scale. See FM-D5.4 in `docs/phases/PHASE-2.md` and SC-4 close commit `57f3cb1` for the implementation truth.

#### Rationale

The registry is **read-heavy, write-light**:
- Read on every cap operation, IPC syscall, scheduler cross-process switch.
- Write only on `process::register` (spawn) and `process::on_thread_reap`'s deregister path.

`RwLock` allows concurrent reads from all CPUs without contention. Writes are rare and bounded.

#### Considered alternative — kernel RCU

RCU would let readers proceed without any lock at all, with deferred reclamation of removed entries on writer side. Real win for read-mostly structures. Rejected for BUILD-3:

- No existing RCU primitive in the kernel. Implementing minimal kernel RCU is substantial new mechanism.
- `RwLock` on a registry that's read in every IPC syscall is unlikely to become a bottleneck at Phase 2 scale. By Phase 5+ when it might, RCU can replace it transparently.
- Bible Article 4.3: "Every dependency is a liability." Adding kernel RCU is a real dependency.

If contention shows up in benchmarks, RCU becomes its own ADR + BUILD.

#### Considered alternative — `Mutex` (status quo)

Single mutex on PROCESSES. Every cap op serializes on it. Under `-smp 4` with multiple processes doing concurrent cap operations, this becomes the bottleneck. Rejected.

### Q8.b — Cross-process revoke walk lock-order rule

#### The walk

`sys_revoke(cap_id)` must:
1. Find the cap's derivation tree (descendants of `cap_id` across all processes).
2. For each affected process, mark its handle slots as revoked.
3. Free affected slots.

Implementation:

```rust
fn revoke_walk(cap_id: CapId) {
    // Phase 1: collect all CapIds to revoke (the descendants).
    // This needs access to every process's derivation log.
    let mut to_revoke = BTreeSet::new();
    to_revoke.insert(cap_id);
    
    // Read-lock the registry.
    let processes = PROCESSES.read();
    
    // Iterate processes in pid-order (deterministic).
    let mut changed = true;
    while changed {
        changed = false;
        for (_pid, proc) in processes.iter() {
            // Per-process write-lock on derivation log.
            let mut deriv = proc.derivations.lock();
            for (&child, &parent) in deriv.iter() {
                if let Some(p) = parent {
                    if to_revoke.contains(&p) && !to_revoke.contains(&child) {
                        to_revoke.insert(child);
                        changed = true;
                    }
                }
            }
            // Drop deriv lock before next iteration.
        }
    }
    
    // Phase 2: free handle slots and remove derivation entries.
    for (_pid, proc) in processes.iter() {
        let mut handles = proc.handles.lock();
        let mut deriv = proc.derivations.lock();
        // Both held simultaneously; this is a per-process scope, no cross-process inversion.
        handles.remove_cap_ids(&to_revoke);
        deriv.retain(|cap_id, _| !to_revoke.contains(cap_id));
    }
    
    drop(processes); // release registry read-lock.
}
```

#### Lock-order rule

**Order: registry read → per-process locks (handles before derivations within a process).**

Per-process locks are released before moving to the next process during the descendants-collection loop.

In the Phase 2 freeing pass, both per-process locks are held simultaneously. This is a per-process scope; no cross-process lock acquired while another process's locks are held. No cross-process inversion possible.

#### Discipline

All cap-table-touching paths follow the same order. Derivation logs are never acquired before the registry-read; handles are never acquired before derivations within the same process scope. This is the project-wide rule:

> Lock order for cap-table operations:
>   PROCESSES (read or write) → process.handles → process.derivations
> Within a single process scope, handles and derivations may be held jointly.

Sub-commit 5's audit verifies all cap-touching call sites obey this order.

### Q8.c — Cross-CPU `kill_thread_by_pid` blocked-thread walk

#### The current state

`kernel/src/sched/scheduler.rs:kill_thread_by_pid` per its doc:

> Phase 1.5b limitation (ADR-007 §Q3): only handles threads in the Ready state. A thread parked inside a Channel `EndpointState` (Blocked) is invisible to the scheduler queues and is NOT yanked.

Under SMP, this becomes more important: a target thread blocked in IPC on AP1 must still be killable from BSP. The walk must:
1. Find the target Process by pid.
2. Walk the Process's handle table (which includes Channel handles).
3. For each Channel handle, inspect the Channel's `state: Mutex<[EndpointState; 2]>`.
4. If either endpoint contains a `Blocked.waiter` whose Thread.pid matches the target pid, yank it.
5. Mark the yanked Thread as zombie and route to *the killer's CPU's* zombie queue.

#### Cross-CPU semantics

The blocked Thread's `Box<Thread>` is parked in a Channel EndpointState somewhere. That Channel's `state` mutex lives in a Channel struct that's `Arc`-shared across processes. Acquiring the Channel mutex is the entry point.

Once yanked, the Thread is homeless: not in any scheduler queue, no longer in any EndpointState. Routing to the killer's CPU's zombie queue (rather than the target's last-known-CPU) is consistent with Q4's wake-on-completer-CPU pattern: the actor doing the operation gets the resulting state on its own CPU.

The Thread's slab free will go cross-CPU (allocated on the target's CPU originally; freed on the killer's CPU now). Sub-commit 3b's slab cross-CPU free path (Q6 implicit dependency) handles this.

#### Lock-order

```
PROCESSES (read) → target.handles → channel.state → (move waiter Box) → drop channel.state → drop handles → killer's PerCpuScheduler
```

Critical: drop the Channel mutex before acquiring the killer's scheduler lock. Channel-then-scheduler order matches the established discipline.

#### Implementation

Sub-commit 5 ships the new walk in `kill_thread_by_pid`. The function becomes:

```rust
pub fn kill_thread_by_pid(pid: ProcessId) -> bool {
    // First try the local ready queue (fast path, single-CPU compatible).
    if try_kill_in_local_ready_queue(pid) { return true; }
    
    // Then try per-CPU ready queues on other CPUs.
    for cpu_id in 0..current_num_cpus() {
        if cpu_id == current_cpu_id() { continue; }
        if try_kill_in_remote_ready_queue(cpu_id, pid) { return true; }
    }
    
    // Finally, walk the target's handle table for blocked-in-IPC.
    let processes = PROCESSES.read();
    let target = match processes.get(&pid) {
        Some(p) => p.clone(),
        None => return false,
    };
    drop(processes); // we have the Arc; release registry early.
    
    let handles = target.handles.lock();
    for handle in handles.iter() {
        if let Handle::Channel { channel, .. } = handle {
            if try_yank_blocked_waiter(channel, pid) { return true; }
        }
    }
    
    false
}
```

`try_kill_in_remote_ready_queue` acquires the remote CPU's scheduler lock to scan; finding a match yanks the Box<Thread>, marks zombie, routes to killer's CPU. Lock acquisition is the standard local-then-other pattern.

#### Considered alternative — IPI-driven kill

Send an IPI to the target's last-known CPU asking it to do the kill locally. Adds IPI infrastructure dependency (now Q7's IPI machinery is needed for Q8 too — fine, since Q7 ships in sub-commit 4 anyway, and Q8 ships in sub-commit 5). Lower contention (the target's CPU does its own scan) but adds latency and complexity.

Rejected: the direct-walk is simpler and works without IPI. If contention becomes a problem, IPI-driven kill is a future optimization.

### Test scenarios anchoring Q8

- `smp_cap_revoke_cross_cpu` (sub-commit 5) — grantor BSP, recipient AP1; revoke from BSP correctly invalidates AP1's handle.
- `smp_kill_blocked_cross_cpu` (sub-commit 5) — target thread blocked in IPC on AP1; killer BSP issues `sys_kill`; target reaped correctly.
- All `cap_*` tests PASS under `-smp 4` (sub-commit 5 anti-regression gate).

---

## Q9: Reaper distribution

### The question

Currently, the reaper runs from the bootstrap "main" thread on the single CPU, draining the global zombie queue. Under SMP with per-CPU schedulers, where does reaping happen?

### Choice: per-CPU reapers

Each CPU's bootstrap thread runs its own reaper loop, draining its CPU's local `PerCpuScheduler.zombies` queue. No cross-CPU zombie traffic.

### Rationale

Zombies are produced per-CPU (a thread that exits on CPU0 ends up in CPU0's zombie queue). Reaping locally avoids cross-CPU traffic. Each CPU's bootstrap thread has nothing else to do; the reaper loop is its purpose.

### `on_thread_reap` semantics

The current `on_thread_reap(pid)` checks if this was the last thread of a user process and, if so, deregisters the Process. Under SMP, multiple CPUs may concurrently reap threads from the same process (last thread on CPU0 reaped, last thread on CPU1 reaped — but one of them is *actually* last).

Resolution: `on_thread_reap` takes the registry write lock for the deregistration. The "is this the last thread" check is the actual deregistration: try to remove the Process from PROCESSES; if the Process still has live threads, the removal is conditioned on a thread count.

**Implementation:** add a `live_thread_count: AtomicU32` to Process. Increment on thread spawn (atomic); decrement on thread reap (atomic). When a reaper decrements and the result is 0, attempt deregistration: take the registry write lock, double-check count is still 0, remove from registry. The double-check handles the rare race where a new thread is spawned between the decrement and the lock acquisition.

```rust
pub fn on_thread_reap(pid: ProcessId) {
    let processes = PROCESSES.read();
    let proc = match processes.get(&pid) {
        Some(p) => p.clone(),
        None => return, // already deregistered
    };
    drop(processes);
    
    let new_count = proc.live_thread_count.fetch_sub(1, Ordering::AcqRel) - 1;
    if new_count == 0 {
        // Last thread; attempt deregister.
        let mut processes = PROCESSES.write();
        // Double-check under write lock.
        if proc.live_thread_count.load(Ordering::Acquire) == 0 {
            processes.remove(&pid);
            // Drop the Arc; Process resources released when refcount hits 0.
        }
    }
}
```

### Considered alternative — single reaper

One CPU (BSP's main thread) drains all CPUs' zombies via cross-CPU traffic. Simpler structurally; higher cross-CPU traffic cost; reaping latency depends on BSP load. Rejected: per-CPU is the SMP-natural pattern.

### Test scenarios

- `smp_per_cpu_reaper_smoke` (sub-commit 2) — each CPU spawns and exits a thread; verify each CPU's reaper drains its own zombies.
- `smp_concurrent_process_exit` (sub-commit 5) — multi-threaded user process, threads exit concurrently on different CPUs; verify Process is deregistered exactly once.

---

## Anti-scope

What ADR-013 explicitly does not address; out of scope for BUILD-3:

- **Arc-aware slab integration.** Per ADR-012 BUILD-2 post-close summary, three known paths exist (custom Drop, `Arc::new_in()` with custom allocator, refactor away from Arc final-owner). BUILD-3 does NOT pick one. Process and Channel allocations remain heap-backed; PROCESS_CACHE and CHANNEL_CACHE remain declared-but-unused infrastructure. If BUILD-3's slab cross-CPU free audit (sub-commit 3b) reveals deeper Arc-vs-slab coupling than expected, BUILD-3 stops at sub-commit 3a and a separate Arc-aware-slab BUILD opens.

- **Cross-CPU thread migration as a feature.** Q4's wake-on-completer-CPU is the *one* form of cross-CPU thread movement. Load balancing, work stealing, periodic rebalancing — all explicitly deferred to future BUILDs.

- **Inference scheduling class.** Bible Article 8.1's "Inference" priority class is implemented in Phase 7. BUILD-3 implements per-CPU scheduling but does not add inference-aware mechanics.

- **NUMA awareness.** NeoPlutarch treats all CPUs as equally-distant from all memory in BUILD-3. NUMA-aware allocation, memory locality scheduling, and NUMA-aware accelerator placement (Phase 5+ relevance) are all Phase 8+.

- **PCID.** Process Context Identifiers (x86_64 feature for tagging TLB entries by process) is a real performance win that avoids most TLB flushes on process switch. Significant added complexity. Deferred to a future BUILD; the CR3 cache (Q7.b) provides the cheap subset benefit.

- **Kernel RCU primitives.** `RwLock` is the discipline for read-mostly structures (Q8.a). RCU-style readers may become necessary at Phase 5+ scale; deferred.

- **Fairness shaping / vruntime / CFS.** Q5 explicitly removes the deferred-quantum mechanism. Future fairness work (vruntime-based scheduling, deficit round-robin, proportional-share) is its own design conversation.

- **Lock-free queues / MCS locks.** `spin::Mutex` and the new `RwLock` are the synchronization primitives. Lock-free MPSC for IPI shootdown rings is the one exception (Q7.c). General lock-free data structures, MCS locks, etc. — out of scope.

- **Userspace SMP-awareness.** libneo and userland processes are oblivious to which CPU they run on. The kernel handles all SMP concerns. Userspace pinning APIs, NUMA hints, etc. — Phase 8+.

- **APIC / xAPIC vs x2APIC distinction.** BUILD-3 uses xAPIC mode. x2APIC support deferred.

- **Hot-plug CPUs.** Adding/removing CPUs at runtime is out of scope. CPU count is determined at boot.

---

## Migration checklist

ADR-011's migration checklist, expanded with BUILD-3 specifics:

### Sub-commit 0 — ADR-013 + GLOBAL bit
- [ ] ADR-013 lands at `docs/ADR-013-phase-2-smp-retrofit.md`.
- [ ] ADR-011 acquires `Superseded by: ADR-013` metadata.
- [ ] Post-boot fixup walks PML4 entries 256-511, sets GLOBAL on kernel-half leaves.
- [ ] `paging::sanity_check` reports `global=true` on kernel `.text` sample.

### Sub-commit 1 — Per-CPU state + AP bring-up
- [ ] `MAX_CPUS = 64` constant defined.
- [ ] `PerCpu` array exists with 64 elements; Q1.b fields all present.
- [ ] `current_per_cpu()` accessor uses GS-base.
- [ ] Limine MP request configured; AP count detected at boot.
- [ ] Each AP's `ap_entry` sets GS-base, verifies via sentinel, panics on mismatch.
- [ ] Per-AP TSS + IST stacks allocated.
- [ ] APs spin-wait at end of `ap_entry` (no scheduler entry yet).
- [ ] Single-CPU boot still works (one CPU = BSP only).
- [ ] All BUILD-2 anti-regression tests PASS.

### Sub-commit 2 — Per-CPU schedulers
- [ ] Global `SCHED` removed; `PerCpuScheduler` heap-allocated, pointer in `PerCpu.scheduler_ptr`.
- [ ] Each AP runs its own bootstrap thread + reaper loop.
- [ ] **Invariant 2/3/4 mechanism deleted** (Q5): `HANDOFF_PREEMPT_DEFER_TICKS`, `Thread.preempt_defer_ticks`, `consume_preempt_defer`, `clear_current_preempt_defer`, `defer_current_preempt_for_handoff` all removed.
- [ ] Same-CPU IPC works on each CPU independently. Cross-CPU IPC returns EAGAIN cleanly.
- [ ] Per-CPU reaper distributes correctly; `on_thread_reap` takes registry write lock.
- [ ] **`ipc_heap_kill_smoke` runs 100x consecutive on `-smp 4`; zero flakes.** This is the Q5 correctness gate.
- [ ] All BUILD-2 anti-regression tests PASS.

### Sub-commit 3a — ManuallyDrop + asm release
- [ ] `block_current_into` uses ManuallyDrop + asm release pattern (Q6).
- [ ] `context_switch_with_release` asm wrapper handles multiple lock releases on new stack.
- [ ] All same-CPU IPC tests PASS.
- [ ] Cross-CPU IPC still EAGAIN.
- [ ] **Project lead code review of asm changes before merge.**

### Sub-commit 3b — Cross-CPU IPC + slab cross-CPU free
- [ ] Cross-CPU IPC succeeds (Q4 wake-on-completer-CPU implemented).
- [ ] Consumer-first handoff property preserved (per Q4's per-rendezvous semantics).
- [ ] `mm/slab.rs` handles cross-CPU free correctly (audit + fix per BUILD-3 plan).
- [ ] `ipc_heap_kill_smoke` PASSes with sender BSP and receiver AP1.
- [ ] `cap_grant_*` tests PASS with grantor and recipient on different CPUs.
- [ ] **Slab audit checkpoint:** if the audit reveals Arc-vs-slab forcing beyond cross-CPU free, BUILD-3 stops here and a separate Arc-aware-slab BUILD opens.

### Sub-commit 4 — TLB shootdowns + IPI infra + CR3 cache
- [ ] IPI infrastructure works (Q7.c).
- [ ] Per-Pmap `active_on_cpus` bitmap correct on CR3 reload.
- [ ] Pmap mutation triggers shootdowns to all other active CPUs; mutator waits for ack.
- [ ] CR3 cache (Q7.b) skips no-op writes; debug-assert verifies cache coherence under `cfg!(debug_assertions)`.
- [ ] Multi-threaded user process across CPUs works correctly (TLB stays coherent).

### Sub-commit 5 — Capability-table SMP composition
- [ ] `PROCESSES` is `RwLock`-protected (Q8.a).
- [ ] Cross-process revoke walk obeys lock-order rule (Q8.b).
- [ ] `kill_thread_by_pid` walks blocked threads via channel-handle inspection; cross-CPU blocked targets killed correctly (Q8.c).
- [ ] All `cap_*` tests PASS under `-smp 4`.

### Cross-cutting verification
- [ ] `cargo k` green at every sub-commit close.
- [ ] All prior tests PASS — anti-regression.
- [ ] Frame leak audit clean for every multi-CPU test run.
- [ ] No `KERNEL PANIC`, no `BUG:`.
- [ ] Multi-CPU QEMU verification: `-smp 4` is standard for sub-commits 1+.
- [ ] **Bare-metal multi-CPU verification at BUILD-3 close** (Bible Article 13 mandate).

---

## Tests retired or rewritten

Per ADR-011's tests-that-depend list:

- **`ipc_heap_kill_smoke`** — preserved, not rewritten. Q5's claim is that the test's correctness property (deterministic kill-on-Blocked-EINVAL) holds under SMP without the deferred-quantum mechanism. Sub-commit 2 verifies this empirically (100x flake-free run on `-smp 4`).

- **`expect_pass_but_fail`** — preserved unchanged. Independent of the invariants. Critical anti-regression for the test runner.

- **`cap_grant_*` tests (BUILD-1)** — preserved, with sub-commit 3b verifying they PASS cross-CPU. Q4's wake-on-completer-CPU preserves the consumer-first property they depend on.

No tests retired. Sub-commit 2's flake-free 100x gate replaces ADR-011's softer "preserve or rewrite" guidance.

---

## Cross-references

- **ADR-011** — superseded. ADR-011 transitions to historical-record status as of ADR-013's commit. Future searches for "Phase 2 SMP IPC handoff" find ADR-013 first; ADR-011 remains in tree as the trail of how we got here.
- **ADR-004** — base IPC design. Composes unchanged with ADR-013. The rendezvous semantics ADR-004 specified are preserved on a per-rendezvous basis under SMP.
- **ADR-007** — process termination. The kill-on-Blocked test composes with Q8.c's blocked-thread walk.
- **ADR-010** — capability system foundation. Q8 is the SMP composition layer atop ADR-010's cap-table design.
- **ADR-012** — buddy + slab allocators. ADR-013's Q4 (wake-on-completer-CPU) creates the slab cross-CPU free dependency; ADR-012's BUILD-2 post-close summary names the open Arc-vs-slab question that BUILD-3 explicitly does NOT solve but does intersect with.
- **Bible Article 4.5** — single-node scope. ADR-013 is SMP within a single node; cross-node distribution is Bible-foreclosed.
- **Bible Article 8.3** — "SMP is supported from Phase 2. Per-CPU state is real (per-CPU GDT/TSS/IST, per-CPU run queues, per-CPU current-thread). Global locks are forbidden in hot paths; we use per-CPU structures and lock-free or fine-grained locking." ADR-013 is the design that delivers on this mandate.
- **Bible Article 12.1** — "Capability-based security from Phase 2." Q8 addresses SMP composition.
- **`kernel/src/arch/x86_64/per_cpu.rs`** — module-level doc explicitly anticipated this retrofit ("Phase 2 SMP must replace `PER_CPU` with `[PerCpu; MAX_CPUS]`").
- **`kernel/src/sched/scheduler.rs`** — `block_current_into`'s lock-order inversion comment names the Q6 problem and its two resolutions. ADR-013 picks (b).
- **`kernel/src/mm/paging.rs`** — `sanity_check` GLOBAL bit warning becomes Q7.a's directive.
- **`kernel/src/sched/process.rs`** — `kill_thread_by_pid`'s "Phase 2 SMP work (or a smaller follow-on) lifts the restriction" becomes Q8.c.
- **Limine boot protocol v5.x-v9.x** — `LIMINE_SMP_REQUEST` for AP bring-up (Q2). Researched 2026-05-05 against the Limine repository.

---

## Notes

This ADR is forward-looking design (Q1/Q2/Q3 shape per ADR-011's own preference) rather than retroactive documentation. Sub-commits 1-5 of BUILD-3 implement the choices; ADR-013 amendments (following BUILD-2's amendment-subsection pattern) record findings that emerge during BUILD.

The design intentionally keeps anti-scope explicit and broad. ALTIMIT-faithful posture per the research artifact at `docs/research/ALTIMIT-real-world-implementation.md` is "do less in the kernel, more rigorously." BUILD-3's job is "Phase 2 SMP works correctly," not "Phase 2 SMP is fully optimized." Performance optimization comes in later BUILDs once correctness is established.

The single biggest risk surface in ADR-013 is Q6's ManuallyDrop + asm release. Mitigation is layered: pre-implementation checkpoint, project-lead review of asm before merge, two-stage rollout (3a same-CPU first, 3b cross-CPU second), explicit stress tests. This ADR commits to the choice; sub-commit 3a is where that commitment gets tested.

The second-biggest risk is the Q5 claim that removing the deferred-quantum mechanism is correct under Q3+Q4's SMP architecture. The 100x flake-free gate on sub-commit 2 closure is the empirical test; if the claim is wrong, the gate catches it before sub-commit 3a starts.

---

## BUILD-3 amendment: sub-commit 1a scope and per-AP GDT

### Amendment 1 — Per-AP GDT selected

BUILD-3 sub-commit 1a chooses per-AP GDTs (Option A) over a shared GDT with N TSS slots (Option B).

The original Q2 design treated the BSP GDT as shareable because code/data descriptors are CPU-invariant. The OBSERVE pass on `gdt.rs`, `idt.rs`, and `per_cpu.rs` found the missing load-bearing detail: the TSS descriptor is CPU-specific, and each CPU needs its own TSS plus its own IST stack set. Keeping a shared GDT would require preallocating one TSS descriptor per CPU in a single table and selecting the right descriptor at `ltr` time. That works mechanically, but it makes the shared table carry per-CPU state and couples every CPU's trap setup to a global descriptor-layout budget.

Per-AP GDTs keep the invariant local: each CPU owns one GDT, one TSS descriptor, one TSS, and three IST stacks. Kernel/user segment selectors remain identical across CPUs because the descriptor ordering is identical. The TSS selector is per-CPU and must be accessed through a CPU-indexed helper when needed.

Sub-commit 1a implements this by replacing the single `Lazy<(GlobalDescriptorTable, Selectors)>` with a `[Once<PerCpuGdt>; MAX_CPUS]` array. Each CPU initializes and loads its own GDT during trap init. The BSP path initializes CPU 0 only; AP paths are defined but not called until sub-commit 1b.

Implementation correction: the initial BUILD-3 dispatch assumed per-CPU TSS and IST storage could be heap-allocated during trap initialization. The current boot order initializes traps before `mm::init()`, so sub-commit 1a uses static per-CPU TSS and IST storage instead. The invariant is unchanged: each CPU owns one GDT, one TSS, and three writable IST stacks for the kernel lifetime. If a later BUILD wants heap-backed TSS/IST storage, it must first change the boot order explicitly rather than hiding allocation inside early trap init.

### Amendment 2 — Sub-commit 1 split into 1a and 1b

BUILD-3 sub-commit 1 is split into:

- **Sub-commit 1a:** refactor per-CPU infrastructure for `MAX_CPUS=64` without bringing up APs.
- **Sub-commit 1b:** add the Limine SMP request, AP entry path, AP startup sequencing, and actual AP bring-up.

The split is intentional. Per-CPU `PerCpu`, GDT, TSS, IST, syscall-MSR, IDT-load, and slab-state initialization can be made SMP-shaped while the kernel still boots as a single-CPU system. That gives a clean falsifier: after 1a, the single-CPU kernel must behave exactly as before, with only the per-CPU initialization log lines added.

Sub-commit 1b then tests AP bring-up against infrastructure that already compiles, boots, and passes the BUILD-2 anti-regression suite on the BSP. If AP startup fails, the failure surface is AP discovery/entry/init ordering rather than a mixed refactor-and-bring-up change.

No APs are brought up by sub-commit 1a. No Limine SMP request is added. Scheduler, IPC, TLB shootdown, and capability-table SMP composition remain later sub-commits.

---

## BUILD-3 amendment: sub-commit 1b AP bring-up corrections

Sub-commit 1b implements Limine MP bring-up, but two OBSERVE-pass corrections refine the dispatch shape.

First, CPU IDs are allocated by the BSP as NEO-local sequential IDs: BSP is always `cpu_id=0`, and APs receive `cpu_id=1..N` via Limine's `extra_argument`. The dispatch suggested passing the raw Limine response index. That is correct only if Limine lists the BSP at index 0. Sequential assignment preserves ADR-013 §Q1's invariant that `cpu_id` is a NEO-local dense index distinct from LAPIC ID, even if a future firmware or bootloader reports CPUs in a different order.

Second, APs run CPU-local CR0/CR4 setup before per-CPU trap initialization. The BSP already called `enable_sse()` early because rustc may emit SSE2 instructions and because SMEP/SMAP are CR4 policy bits. CR0/CR4 are per-CPU state, so every AP must do the same before it executes meaningful Rust code or relies on the SMEP/SMAP policy model. This is not scheduler bring-up; it is CPU-local architectural initialization required before the AP can safely spin-wait.

APs still do not enter the scheduler in sub-commit 1b. They wire GS-base, sentinel-check `gs:[24]`, load their per-AP GDT/TSS and the shared IDT, write per-CPU syscall MSRs, initialize per-CPU slab state, print readiness, and halt.

---

## BUILD-3 amendment: sub-commit 2 AP GS-base rewire after GDT load

Sub-commit 2's per-CPU scheduler bootstrap exposed an AP-only ordering bug in the sub-commit 1b AP bring-up sequence. APs wired `IA32_GS_BASE` via `per_cpu::init_ap(cpu_id)`, successfully passed the initial `gs:[24]` sentinel, then loaded their per-AP GDT. The GDT path loaded the visible GS selector as part of segment setup, which cleared the effective GS base on APs. The next per-CPU access inside scheduler bootstrap faulted at `gs:[24]` (`cr2=0x18`).

The corrected invariant is: AP trap initialization must rewire `IA32_GS_BASE` after loading the per-AP GDT and before any subsystem uses GS-relative per-CPU accessors again. `init_traps_ap(cpu_id)` now performs `gdt::init_ap(cpu_id)`, rewires `per_cpu::init_ap(cpu_id)`, loads the shared IDT, writes AP-local syscall MSRs, and sentinel-checks `current_cpu_id() == cpu_id`.

This does not change ADR-013's Q1 addressing model or Q2 AP bring-up model. It refines the order inside AP trap initialization so per-CPU scheduler bootstrap can safely read `PER_CPU[cpu_id].scheduler_ptr`, `slab_state`, and future per-CPU scheduler fields through GS.

The extra AP `per_cpu:` log line is intentional evidence of the rewire. Future AP init changes must preserve the same invariant: any code path that reloads GS after `per_cpu::init_ap()` must either avoid clearing GS-base or immediately rewire and sentinel-check before returning to common kernel code.

---

## BUILD-3 amendment: sub-commit 2 Q5 bet failed; deferred quantum stays

Sub-commit 2's 100x `ipc_heap_kill_smoke` closure gate failed at run 24 under `NEO_SMP=4`. The `[ neo-init ] BUG: sys_kill on Blocked did NOT return EINVAL` line appeared exactly when Q5 said it might.

The §Q5 argument relied on a claim that does not hold:

> the front-queue + yield window is closed by construction — no PIT tick can fire on this CPU while IF=0.

This is true for the kernel-side critical section. It is not true for the end-to-end semantic the deferred-quantum mechanism actually protected: after `sys_send` to a `RecvBlocked` peer returns, the peer has reached its next blocking syscall. Once `block_current_into`'s `without_interrupts` scope exits and the resumed thread runs in userspace, IF is re-enabled and the next PIT tick is on the CPU's hardware clock, independent of when the thread became current. Under QEMU SMP, host scheduling jitter makes the inter-tick gap small enough that the resumed thread can be preempted before reaching its next syscall.

Resolution: the deferred-quantum mechanism is restored in per-CPU form. `Thread.preempt_defer_ticks`, `HANDOFF_PREEMPT_DEFER_TICKS`, `consume_preempt_defer`, `clear_current_preempt_defer`, and `defer_current_preempt_for_handoff` all return. Reads and writes of `preempt_defer_ticks` go through the per-CPU scheduler's locked inner state, the same way sub-commit 2 routes the rest of `current`, `ready`, and `zombies` access.

Sub-commit 2's actual scope is therefore: §Q3 (per-CPU schedulers), §Q9 (per-CPU reapers + `live_thread_count`), the AP GS-base rewire amendment, and the two new test scenarios. The §Q5 deletion is reverted and will not be retried; the mechanism is small, correct, and load-bearing for several Phase 1.5b smoke-test semantics.

The §Q5 idea was wrong-shaped: it bundled a cleanup attempt with the per-CPU scheduler refactor. Future cleanup proposals must cleanly separate "is this mechanism still needed?" from "are we changing the container shape?" The container shape changed correctly; the mechanism inside the container should not have been touched in the same commit.

---

## BUILD-3 amendment: sub-commit 2 Q9 deferred with bisection evidence

Sub-commit 2's first closure attempt included Q9's per-CPU reaper distribution and `Process.live_thread_count` gate. The `NEO_SMP=4` 100x `ipc_heap_kill_smoke` gate failed at run 81 with a kernel hang in `sched::smoke_test`'s `reap()` path after `[ kernel ] sched: reaped 'pong' (id=5)`. Adjacent runs passed cleanly; the observed failure rate was approximately 1.25%.

Restoring the Q5 deferred-quantum mechanism fixed the earlier `sys_kill` semantic race but did not eliminate the hang. A non-mutating bisection then narrowed the trigger:

- **A:** `NEO_SMP=1` 100x `ipc_heap_kill_smoke` passed 100/100, so the hang requires SMP.
- **B:** `NEO_SMP=4` bare boot passed 100/100, so the hang requires the test-harness configuration.
- **D:** bare-boot kernel under test-harness QEMU flags passed 100/100, so QEMU flags alone are not the trigger.
- **E:** `kaslr_smoke` under `NEO_SMP=4` passed 100/100, so generic `mod test_harness` presence is not sufficient.
- **F:** `cap_mint_and_use` under `NEO_SMP=4` passed 100/100, so generic `spawn_neo_init` presence is not sufficient.
- **G:** instrumented `reap()` on `ipc_heap_kill_smoke` failed at run 10 after `[ DBG ] reap: post-on_thread_reap id=8` with no loop-end line, amplifying the hang rather than cleanly localizing it.
- **H:** sub-commit 2 minus Q9 passed 100/100 under `NEO_SMP=4`, implicating Q9.

Resolution: Q9 is deferred. Sub-commit 2 keeps Q3's per-CPU scheduler containers but removes Q9's AP-side reaper participation and `Process.live_thread_count` gate. BSP `reap()` now scans initialized per-CPU schedulers, drains each zombie queue under that scheduler's lock, and processes zombies locally. APs do not reap. This is option (b) from `Plans/DISPATCH-2026-05-06-sub-commit-2-bisect-step1-cerebro.md`.

This shape is acceptable for sub-commit 2 because no production thread is routed to APs yet; AP zombie queues are empty in the steady state, so the BSP cross-CPU scan is effectively a bounded no-op beyond checking initialized scheduler pointers. Any later sub-commit that introduces cross-CPU thread movement must revisit reaper distribution instead of assuming this interim shape is final.

## BUILD-3 amendment: sub-commit 2 Q9 hypothesis (informational, not investigated)

The precise Q9 failure mechanism was not investigated after EXP-H passed. Drilling further would have required additional bisection rounds without changing the closure decision: sub-commit 2 closes with Q9 deferred.

The leading hypothesis is informational, not proven. Q9's modified `on_thread_reap` decremented `proc_arc.live_thread_count` with `fetch_sub(1, AcqRel)` and returned early when the previous count was not 1. In Phase 1.5b's one-thread-per-process model, that previous count should always be 1. If it is ever 0 or greater than 1, process cleanup is skipped: the process remains in `PROCESSES`, `any_user_process()` can keep returning true, and the kernel can wait forever.

Plausible causes include a counter-initialization visibility race, an exactly-once violation from AP-side reaper participation, or a memory-ordering assumption that `AcqRel` did not actually justify. EXP-G's serial instrumentation amplifying the failure rate is consistent with a timing-sensitive race in these paths, but it does not prove which path is responsible.

Future work that reintroduces `live_thread_count` or an equivalent multi-thread-per-process readiness gate should initialize the counter before any thread becomes runnable, verify exactly-once decrement semantics under SMP, avoid AP-side reaper participation until threads can actually route to APs, use `SeqCst` until weaker ordering is proven, and add a regression test that exercises the counter under real multi-thread-per-process load. These are recommendations for the future Q9 replacement, not requirements imposed on sub-commit 2.

## BUILD-3 amendment: Q9 deferred permanently under D13 invariant

Sub-commit 2 deferred Q9 with bisection evidence and an unverified hypothesis. SC-4 (BUILD-3 sub-commits 5+6, closed 2026-05-11 at commit `57f3cb1`) added `Process.exit_state: AtomicU8` CAS-claim discipline (D8) and ratified the single-thread-per-process invariant (D13).

Under D13, the thread reap IS the process exit: there is exactly one thread per user process, and that thread's `on_thread_reap` path uniquely triggers Process deregistration. D8's CAS already serializes natural-exit-vs-forced-kill at the Process level. `live_thread_count` was Q9's mechanism for multi-thread "is this the last thread?" serialization — under D13 there is no such ambiguity, and the CAS handles single-thread serialization.

Q9's interim shape from this amendment cluster (BSP scans all per-CPU schedulers' zombie queues; APs do not reap; `live_thread_count` absent) is therefore the Phase 2 BUILD-3 final disposition. The deferral becomes permanent under D13.

If/when Phase 2+ multi-threaded user processes enter scope (currently a Phase 2+ forward marker per SC-4 D13), the Q9 design space reopens. Recommended starting point: `Plans/Q9-EXPLORATION-2026-05-11.md` §3 alternative (c) — per-CPU reapers without `live_thread_count`, using D8's CAS as the unified Process-exit serialization primitive. The `live_thread_count` original design (alternative a) carries the sub-commit 2 hypothesis-unverified race surface; alternative (c) avoids it by composing with D8 instead of duplicating its serialization function.

This amendment closes Q9 for Phase 2 BUILD-3. No code changes; the interim shape from "sub-commit 2 Q9 deferred with bisection evidence" stands as the final disposition.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR. Phase 2 BUILD-3 implements the decisions above; deviations during BUILD-3 either fold back into this ADR via amendment subsection (per ADR-012's amendment-subsection pattern) or land as superseding ADRs.
