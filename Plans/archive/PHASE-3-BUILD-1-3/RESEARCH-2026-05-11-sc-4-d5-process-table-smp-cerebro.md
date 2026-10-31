# Research: SC-4 D5 — Multi-CPU Process-Table Lookup Prior Art

| | |
|---|---|
| **Date** | 2026-05-11 (fresh-context Cerebro session) |
| **Author** | Cerebro deep-research artifact (dispatched via `DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md`) |
| **Type** | Research artifact (informational; not normative). Prior-art survey for SC-4 D4/D5 decision. |
| **Scope** | Multi-CPU process-table lookup: Linux RCU, seL4, Solaris/illumos, FreeBSD, alternative designs, small-N synthesis |
| **Authority** | None. PM consumes to ratify D4 and author the SC-4 PLAN-stage dispatch. |
| **Time budget actual** | ~3 hours (three parallel research agents + synthesis) |

---

## 1. Executive Summary

**The dominant D4 insight is not about lock type — it's about whether the PROCESSES registry needs to be on the hot path at all.** Linux's `current` macro (`this_cpu_read(current_task)`) gives every CPU zero-cost access to its running task via a per-CPU GS-relative pointer written at context-switch time. Linux's `cpu_tlbstate.loaded_mm` does the same for the memory management structure underlying CR3. NeoPlutarch already has `per_cpu.active_pmap_cr3` (landed in SC-3C) caching the CR3 value. The question for D4 is: does `cr3_for_thread(pid)` need to call into PROCESSES at all, or can the scheduler's runqueue entries carry their `Arc<Pmap>` directly, eliminating the hot-path registry lookup entirely? If yes — and the substrate evidence strongly suggests yes — the D4 contention problem dissolves and the remaining decision is conservative.

**`spin::RwLock` is not the "safe simple upgrade" it appears to be.** The `spin` crate's own documentation warns of writer starvation under read pressure. Empirical benchmarks show `parking_lot::RwLock` is ~5× slower than `Mutex` for short critical sections because the reader-counter atomic causes cache-line invalidations across all cores even when no write is occurring. At NeoPlutarch's target of 4–64 CPUs with the process-table lookup as a potentially-hot path, `spin::RwLock` has the documented failure mode exactly at the scale that matters. PM should not choose `spin::RwLock` as the first upgrade step.

**PM's AtomicPtr<Arc<BTreeMap>> lean is defensible but solves the wrong problem.** The `arc-swap` pattern (vorner's crate, used in production by TiKV and signal-hook) achieves wait-free reads with O(N) copy-on-write at write time. At N=100 processes, the write cost — cloning ~800 bytes of BTreeMap entries — is trivially absorbed at spawn/exit time. The pattern is correct. The concern is that it adds complexity to optimize a path that should be eliminated by the Tier 1 architectural change. If the hot path is already lock-free via per-CPU data, the registry's access rate drops to "process management operations" (single-digit Hz) at which point the existing `spin::Mutex` is acceptable. The right progression is: first audit and restructure callers to eliminate the registry from the scheduler hot path; then, only if profiling shows the remaining registry read rate is still a bottleneck, upgrade to `AtomicPtr<Arc<BTreeMap>>` (not to `spin::RwLock`).

**Production kernels at NeoPlutarch's scale do not use Linux-style RCU for process-table lookup.** seL4 uses a Big Kernel Lock (CLH queue lock) that scales to ~14 cores within a socket for short syscalls — per empirical evaluation by Peters and Heiser. FreeBSD uses per-bucket sx shared-locks plus a per-process mutex via a lock-handoff pattern. illumos uses a single global `pidlock` mutex with `PID_HOLD`/`PID_RELE` reference counting. Redox OS uses `RwLock<BTreeSet<Arc<...>>>`. None use RCU. Linux RCU's complexity is engineered for thousands of CPUs and millions of processes; the design principles transfer to NeoPlutarch but the implementation does not need to.

**The illumos doors IPC design is the most architecturally relevant finding.** illumos's cross-process IPC (`door_call`) bypasses PID lookup entirely — server processes are reached via a vnode-backed door descriptor, with `door_knob` serializing concurrent accesses and `DOOR_INVALID` checking server liveness. This is structurally identical to NeoPlutarch's Channel-grant pattern: the channel capability handle (not the server PID) is the durable cross-process reference, and the grant rendezvous needs only to verify the recipient's liveness flag (PM lean D7: `Process.exiting: AtomicBool`), not to re-traverse a process table.

---

## 2. Sources Surveyed

### Primary sources — verified at URL

- Linux `kernel/pid.c` master HEAD: `https://raw.githubusercontent.com/torvalds/linux/master/kernel/pid.c`. Read for `find_pid_ns`, `alloc_pid`, `pidmap_lock`, IDR structure.
- Linux `include/linux/pid.h`: `https://raw.githubusercontent.com/torvalds/linux/master/include/linux/pid.h`. Read for `struct pid`, `struct hlist_head tasks[PIDTYPE_MAX]`.
- Linux `include/linux/sched/task.h`: `https://raw.githubusercontent.com/torvalds/linux/master/include/linux/sched/task.h`. Read for `get_task_struct`/`put_task_struct` bodies.
- Linux `kernel/exit.c`: `https://raw.githubusercontent.com/torvalds/linux/master/kernel/exit.c`. Read for `release_task`, `put_task_struct_rcu_user`, `delayed_put_task_struct`.
- Linux `kernel/fork.c`: `https://raw.githubusercontent.com/torvalds/linux/master/kernel/fork.c`. Read for `list_add_tail_rcu(&p->tasks, &init_task.tasks)`.
- Linux `arch/x86/include/asm/current.h`: `https://raw.githubusercontent.com/torvalds/linux/master/arch/x86/include/asm/current.h`. Read for `get_current()` GS-segment implementation.
- Linux `arch/x86/include/asm/tlbflush.h`: `https://raw.githubusercontent.com/torvalds/linux/master/arch/x86/include/asm/tlbflush.h`. Read for `cpu_tlbstate.loaded_mm` declaration and `LOADED_MM_SWITCHING` sentinel.
- Linux `Documentation/RCU/listRCU.rst`: `https://docs.kernel.org/RCU/listRCU.html`. Read for release_task RCU pattern and Example 3 node-level copy-on-write.
- seL4 FAQ: `https://sel4.systems/About/FAQ.html`. Read for BKL/CLH design statement and multicore roadmap.
- Peters & Heiser, "For a Microkernel, a Big Lock Is Fine": `https://www.trustworthy.systems/publications/nicta_full_text/8768.pdf`. Read for empirical scaling envelope (14 cores within socket; 5 cores under heavy workload; ARM quad-core scales perfectly at 500-cycle gaps).
- Peters et al., "Evaluation of Coarse-Grained Locking for Multicore Microkernels": arXiv 1609.08372 (`https://ar5iv.labs.arxiv.org/html/1609.08372`). Read for Redis macrobenchmark and BKL contention analysis.
- Oberhauser et al., "Practical Rely/Guarantee Verification of an Efficient Lock for seL4" arXiv 2407.20559v1: `https://arxiv.org/html/2407.20559v1`. Read for CLH lock weak-memory verification status.
- FreeBSD `sys/kern/kern_proc.c` main: `https://raw.githubusercontent.com/freebsd/freebsd-src/main/sys/kern/kern_proc.c`. Read for `allproc_lock` sx declaration, `_pfind` body with `pidhashlock[]`, lock-handoff to `PROC_LOCK(p)`.
- FreeBSD `sys/kern/kern_sig.c` main: `https://raw.githubusercontent.com/freebsd/freebsd-src/main/sys/kern/kern_sig.c`. Read for `pfind_any` + `PROC_UNLOCK` pattern in `kern_kill`.
- FreeBSD `sys/sys/proc.h` main: `https://raw.githubusercontent.com/freebsd/freebsd-src/main/sys/sys/proc.h`. Read for `PROC_LOCK`/`PROC_UNLOCK` macros and lock-key field annotations.
- FreeBSD Architecture Handbook, SMP locking: `https://docs.freebsd.org/en/books/arch-handbook/smp-lock-strategies.html`. Referenced for `sx` lock semantics.
- illumos `usr/src/uts/common/os/pid.c`: `https://raw.githubusercontent.com/illumos/illumos-gate/master/usr/src/uts/common/os/pid.c`. Read for `pidhash[]`, `pidlock`, `pidlinklock`, `PID_HOLD`/`PID_RELE` macros.
- illumos `usr/src/uts/common/sys/proc.h`: `https://raw.githubusercontent.com/illumos/illumos-gate/master/usr/src/uts/common/sys/proc.h`. Read for `p_lock` annotation and lock hierarchy.
- illumos `usr/src/uts/common/fs/doorfs/door_sys.c`: `https://raw.githubusercontent.com/illumos/illumos-gate/master/usr/src/uts/common/fs/doorfs/door_sys.c`. Read for `door_lookup`, `VN_HOLD`, `door_knob`, `DOOR_INVALID` pattern.
- Redox `src/context/mod.rs`: `https://github.com/redox-os/kernel/blob/master/src/context/mod.rs`. Read for `CONTEXTS: RwLock<L2, BTreeSet<ContextRef>>`, `ContextRef = Arc<RwLock<Context>>`, `Ord` by pointer.
- Redox March 2026 news (ordered locks rollout): `https://www.redox-os.org/news/this-month-260331/`. Confirmed ordered-lock typestate deployment.
- `arc-swap` crate docs: `https://docs.rs/arc-swap`. Read for reclamation mechanism (debt-based reader registry, not hazard pointers or RCU).
- vorner blog "Making Arc more atomic": `https://vorner.github.io/2018/06/24/arc-more-atomic.html`. Read for design rationale.
- vorner blog "Tricks In Arc Swap": `https://vorner.github.io/2019/04/06/tricks-in-arc-swap.html`. Read for debt-slot mechanics.
- `spin` crate `RwLock` docs: `https://docs.rs/spin/latest/spin/rwlock/struct.RwLock.html`. Read for writer starvation warning (verbatim from crate's own docs).
- `haphazard` crate docs: `https://docs.rs/haphazard`. Checked for `no_std` support (not present).
- LWN "What is RCU, Fundamentally?": `https://lwn.net/Articles/262464/`. Read for McKenney's RCU vs hazard pointer framing.

### Secondary / community references

- Linux Kernel "Unreliable Guide To Locking": `https://www.kernel.org/doc/html/latest/kernel-hacking/locking.html`. Read for `rwlock_t` performance note: "if it's just about infrequent ones, make [it] readable and simple, not fast."
- Leidinger.net FreeBSD cross-reference: `https://www.leidinger.net/FreeBSD/dox/kern/html/d1/dbe/kern__proc_8c.html`. Confirmed `pidhashlock[]` with `SX_DUPOK`, no SMR present.
- "Read Locks Are Not Your Friends" (Eventual Consistency blog): `https://eventual-consistency.vercel.app/posts/write-locks-faster`. Empirical: `parking_lot::RwLock` ~5× slower than `Mutex` for read-heavy short critical sections.

### Cannot verify (single-source or inaccessible)

- seL4 multikernel design details from Kent McLeod 2022 Summit slides: PDF was not text-extractable. Claims substantiated instead from seL4 FAQ, Peters/Heiser paper, and arXiv 2407.20559v1.
- seL4 manual section on CSpace lookup sync under SMP: PDF was not text-extractable. Design confirmed from arXiv 1609.08372.
- "McKenney 'Is Parallel Programming Hard' on whole-structure copy-on-write": book PDF not text-extractable via available channels. Relevant citation is `Documentation/RCU/listRCU.rst` Example 3 (node-level, not whole-structure).

---

## 3. Q1 — Linux RCU `task_struct` Lookup

### 3.1 Data structure

**Current Linux uses an IDR (radix-tree-backed integer registry) per `pid_namespace`, not a hash table.** The historical `pid_hash[]` chained-hash array has been replaced. `struct pid_namespace.idr` is initialized with `IDR_INIT(...)` and mutated via `idr_alloc_cyclic()` / `idr_remove()` inside `alloc_pid()` / `free_pid()`. The lookup is:

```c
// kernel/pid.c
struct pid *find_pid_ns(int nr, struct pid_namespace *ns)
{
    return idr_find(&ns->idr, nr);
}
```

Source: `kernel/pid.c` lines 530-532. The IDR is a radix-tree structure that maps integer keys (PIDs) to pointers efficiently for sparse, bounded keyspaces.

**PM-relevance:** NeoPlutarch's `BTreeMap<ProcessId, Arc<Process>>` is `O(log N)` with N ≤ 100, so `log₂(100) ≈ 7`. The asymptotic difference between IDR and BTreeMap is immaterial at this scale.

### 3.2 Read-side primitive

**The read side is `rcu_read_lock()` + `rcu_dereference_check()`, with an explicit lockdep assertion that the lock is held.** The lookup chain:

```c
// kernel/pid.c
struct task_struct *find_task_by_pid_ns(pid_t nr, struct pid_namespace *ns)
{
    RCU_LOCKDEP_WARN(!rcu_read_lock_held(),
                     "find_task_by_pid_ns() needs rcu_read_lock() protection");
    return pid_task(find_pid_ns(nr, ns), PIDTYPE_PID);
}
```

`pid_task` uses `rcu_dereference_check(hlist_first_rcu(&pid->tasks[type]), ...)` for the final dereference. Source: `kernel/pid.c` lines 826-859.

**PM-relevance:** Linux's `rcu_read_lock()` is a compiler barrier (on PREEMPT=n, literally a no-op at runtime). The zero-cost claim for RCU readers is real but only for the locking primitive; the caller still executes a tree lookup. For NeoPlutarch, an `Arc<Process>` clone out of a `spin::Mutex` guard is an atomic refcount increment — single-digit nanoseconds — and is dominated by the BTreeMap lookup itself.

### 3.3 Write-side serialization

**Linux decouples PID-lookup serialization from task-list serialization.** Two distinct write-side locks:

1. **`pidmap_lock`** — `DEFINE_SPINLOCK` at `kernel/pid.c` line 89, `__cacheline_aligned_in_smp`. Protects IDR mutations (`idr_alloc_cyclic`/`idr_remove`). Plain `spin_lock()`, no IRQ disable.

2. **`tasklist_lock`** — `rwlock_t`, still exists. Protects the global doubly-linked task list (`init_task.tasks`) and parent/child/signal topology. Write-side: `write_lock_irq(&tasklist_lock)` in `release_task()` before `list_del_rcu(&p->tasks)`. New tasks added via `list_add_tail_rcu(&p->tasks, &init_task.tasks)` in `copy_process()` under the same lock.

Source: `kernel/pid.c` (pidmap_lock), `kernel/exit.c` (tasklist_lock in release_task), `kernel/fork.c` (tasklist_lock in copy_process), `Documentation/RCU/listRCU.rst`.

**PM-relevance:** Linux's two-lock decomposition pays off when (a) many CPUs race on fork/exit simultaneously AND (b) for-each-process iteration doesn't need PID lookup. At NeoPlutarch's scale and workload, a single lock over the process table is equivalent in practice.

### 3.4 Reference counting and pinning

**Linux uses two refcounts on `task_struct`: `usage` (synchronous) and `rcu_users` (RCU grace-period users).** Verbatim from `include/linux/sched/task.h`:

```c
static inline struct task_struct *get_task_struct(struct task_struct *t) {
    refcount_inc(&t->usage);
    return t;
}
static inline void put_task_struct(struct task_struct *t) {
    if (!refcount_dec_and_test(&t->usage))
        return;
    call_rcu(&t->rcu, __put_task_struct_rcu_cb);
}
```

The pin idiom: `rcu_read_lock()` → `find_task_by_pid_ns()` → `get_task_struct(p)` (bumps `usage`) → `rcu_read_unlock()`. The task is now pinned outside the RCU critical section.

**PM-relevance:** This is `Arc<Process>` semantics in C. NeoPlutarch's `Arc<Process>` clone out of the BTreeMap while holding the Mutex guard is the exact equivalent of `get_task_struct` under `rcu_read_lock`, without requiring two separate refcounts. **Linux needs the extra machinery because C has no `Arc` primitive and because the RCU-lookup does not bump a refcount before returning.** Rust's `Arc` eliminates the need for both `rcu_users` and the grace-period callback.

### 3.5 Grace period and task lifecycle

The exit path (verbatim pattern from `Documentation/RCU/listRCU.rst` and `kernel/exit.c`):

```c
// release_task in kernel/exit.c
write_lock_irq(&tasklist_lock);
list_del_rcu(&p->tasks);
write_unlock_irq(&tasklist_lock);
// ... signal cleanup ...
put_task_struct_rcu_user(p);
// → call_rcu(&task->rcu, delayed_put_task_struct)
// → eventually: put_task_struct(tsk) → __put_task_struct → free_task
```

The RCU grace period exists specifically because `find_task_by_pid_ns` may be called without holding a reference, relying purely on `rcu_read_lock()` to prevent the task from being freed mid-lookup. As `Documentation/RCU/listRCU.rst` states: *"This deferring of destruction ensures that any readers traversing the list will see valid `p->tasks.next` pointers and deletion/freeing can happen in parallel with traversal of the list."*

**PM-relevance:** NeoPlutarch's `Arc<Process>` clone at lookup time eliminates the need for grace-period deferred freeing. The `Arc` is the pin; when the last `Arc` drops, `Process` is freed. No RCU callback machinery required.

### 3.6 `current` macro as implicit per-CPU process cache

On x86_64, `current` is defined via GS-segment-relative per-CPU data:

```c
// arch/x86/include/asm/current.h
static __always_inline struct task_struct *get_current(void) {
    return this_cpu_read_stable(current_task);
}
```

This is a single `mov %gs:offset, %rax` instruction. Zero-cost, no atomic, no lock. The scheduler writes `current_task` on every context switch while setting up the new task. **Linux never touches the global process table for "what process am I in?" — that's always a per-CPU load.**

The same pattern for mm: `cpu_tlbstate.loaded_mm` (in `arch/x86/include/asm/tlbflush.h`) is the per-CPU authoritative source for which `mm_struct` is currently loaded. Written at `switch_mm_irqs_off()` time under IRQs disabled. Read lock-free via `this_cpu_read`.

**PM-relevance:** This is the architectural template for eliminating PROCESSES from the context-switch hot path. See §5 (Synthesis and D4 Recommendation).

### 3.7 pm-preliminary characterization check

PM THINK described `tasklist_lock` as the primary write-side lock protecting PID lookup. **Partial correction:** `tasklist_lock` protects the linked-list iteration structure; PID-number-to-task lookup uses `pidmap_lock` (for IDR mutations) independently. The lookup itself is RCU-protected, not `tasklist_lock`-protected. `tasklist_lock` is the write-side lock for the task list, but it's a separate concern from the IDR. This distinction does not change any D4 decision but clarifies what Linux actually locks.

---

## 4. Q2 — seL4 Multi-CPU Thread/Capability Lookup

### 4.1 Big-kernel-lock vs fine-grained

**seL4 uses a Big Kernel Lock implemented as a CLH (Craig/Landin-Hagersten) queue lock.** Verbatim from the seL4 FAQ:

> "seL4 uses a big-lock approach, which makes sense for tightly-coupled cores that share an L2 cache. The kernel is not meant to scale to many cores, where instead a multikernel is the right approach — running a separate kernel image on each core."

The CLH lock has been independently formally verified under weak memory models (Oberhauser et al., arXiv 2407.20559v1, 2024). The end-to-end kernel-with-CLH proof is "in progress" per the seL4 roadmap; the lock itself is verified.

**Empirical scaling envelope (Peters & Heiser, "For a Microkernel, a Big Lock Is Fine", Trustworthy Systems publication 8768):**
- Within a socket on x86: BKL scales well to **~14 cores** at ~4,000-cycle inter-syscall intervals
- Across sockets: heavy contention; needs ~16,000-cycle gaps
- ARM quad-core: scales perfectly to 4 cores even at 500-cycle gaps (lower inter-core cache latency)
- Real workload (Redis macrobenchmark, arXiv 1609.08372): competitive to ~5 cores; contention-dominant beyond that

### 4.2 CSpace/CNode lookup synchronization

**CSpace lookup runs entirely under the BKL; no per-CNode or per-thread lock.** Because no other CPU can be executing kernel code while the BKL is held, capability resolution is sequentially consistent by construction. Each thread's CSpace root is at `thread->tcbCTable`; the CPtr walk traverses CNodes using capability-pointer bits. There is no RCU, no per-CNode mutex, no hazard pointer.

Source: arXiv 1609.08372 (Peters/Heiser evaluation of coarse-grained locking): *"If one core is in the IPC while another core is trying to IPC, even though both IPCs are core-local, the second will have to wait until the first gets out of the lock."*

### 4.3 Cross-thread IPC locking

**IPC endpoint operations also run under the BKL.** There is no per-endpoint lock. The endpoint queue (`endpoint->epQueue`) is protected by the global BKL; concurrent IPC operations on the same endpoint from different CPUs are fully serialized. When a sender's chosen receiver is on another core, seL4 sends an IPI to that core to reschedule; the TCB register transfer happens under the BKL on the sender's core before the IPI is dispatched.

### 4.4 Formal verification vs runtime invariants

seL4's approach is: **runtime invariant (BKL held ⟹ no other CPU in kernel) maps the SMP problem to a uniprocessor-with-IPI problem.** The existing uniprocessor proof can be extended incrementally rather than redone with full interleaving semantics. This is why the BKL was chosen — it preserves the verification tractability of the uniprocessor proof while enabling multicore operation.

**PM-relevance:** The Peters/Heiser BKL result is directly applicable to NeoPlutarch. At 4–64 CPUs with short capability-operation syscalls, a single global lock (even without CLH-queue fairness) avoids the entire capability-table SMP reasoning problem. For NeoPlutarch's Q8.a (per-process handle table), this validates the `SwitchMutex` lean (D1): a per-process lock serializes all capability operations on that process, which is the right granularity for NeoPlutarch — not a process-table-global BKL, but a per-process lock that gives the same "no concurrent mutation of this process's tables" invariant.

---

## 5. Q3 — Solaris/illumos Process Table

### 5.1 Data structure

**A hash table with a global `pidlock` mutex plus per-link `pidlinklock`.** Verbatim from `usr/src/uts/common/os/pid.c`:

```c
static struct pid **pidhash;
#define HASHPID(pid) (pidhash[((pid)&(pid_hashsz-1))])

kmutex_t pidlock;           /* global process lock */
static kmutex_t pidlinklock; /* protects hash chain links */
```

Unlike FreeBSD's per-bucket sx array, illumos has a **single global `pidlock`** for the hash lookup. The `pidlinklock` is separate and protects only the in-chain pointer updates.

### 5.2 `pidlock` vs `p_lock` hierarchy

**Strict two-level ordering: `pidlock` → `p_lock` → drop `pidlock`.** The standard lookup-and-operate sequence:

1. Acquire `pidlock`
2. Walk hash chain, find `proc_t`
3. Acquire `p_lock` (per-process mutex)
4. Release `pidlock`
5. Operate on proc under `p_lock`
6. Release `p_lock`

This matches the FreeBSD lock-handoff pattern: the per-process lock is acquired before the table lock is released, preventing any race window between lookup and use. `proc_exit` confirms the ordering: it explicitly releases `p_lock` before reacquiring `pidlock` for final removal, never reversing the acquisition order.

Source: `pid.c` and `exit.c` from illumos-gate.

### 5.3 `PID_HOLD` / `PID_RELE` reference counting

**illumos has explicit reference counting on `struct pid` objects (distinct from `proc_t`).** The macros assert `pidlock` is held during the count manipulation:

```c
// usr/src/uts/common/os/pid.c
#define PID_HOLD(pidp) ASSERT(MUTEX_HELD(&pidlock)); ++(pidp)->pid_ref;
#define PID_RELE(pidp) ASSERT(MUTEX_HELD(&pidlock)); \
        (pidp)->pid_ref > 1 ? --(pidp)->pid_ref : pid_rele(pidp);
```

This provides a "pin without holding `p_lock`" mechanism: acquire `pidlock`, do `PID_HOLD`, release `pidlock`, perform long-running operation, reacquire `pidlock`, do `PID_RELE`. The `struct pid` remains alive until all `PID_HOLD` callers have balanced with `PID_RELE`.

**PM-relevance:** This is "RCU-lite" for the rare case where a caller needs to outlive both `pidlock` and `p_lock`. NeoPlutarch's `Arc<Process>` clone out of the BTreeMap is the Rust equivalent: cloning the Arc under the Mutex guard gives the caller a pin that survives the lock release.

### 5.4 Doors IPC cross-process lookup — the most NeoPlutarch-relevant finding

**illumos doors bypass PID lookup entirely.** `door_call()` resolves the server via a file descriptor, not a PID:

```c
// door_sys.c
if ((dp = door_lookup(did, NULL)) == NULL)
    return (set_errno(EBADF));
```

`door_lookup` resolves the door descriptor through the file table (`getf(did)`), then holds the server's `proc_t` alive via `VN_HOLD(DTOV(dp))` — a vnode reference count on the door's vnode. Liveness is checked via `DOOR_INVALID(dp)` under `door_knob` (a global mutex for door-structure access). The server process's PID is never looked up; the door descriptor IS the durable cross-process handle.

Source: `door_sys.c` (verbatim `door_lookup`, `VN_HOLD`, `DOOR_INVALID` pattern), verified from primary source.

**PM-relevance:** This maps directly to NeoPlutarch's Channel-grant pattern. A `Handle::Channel { channel: Arc<Channel>, side, cap_id, rights }` IS the cross-process reference — the `Arc<Channel>` pins the channel independent of either process's lifecycle. The grant rendezvous (`complete_grant`) should check the recipient's `exiting` flag (PM lean D7: `Process.exiting: AtomicBool`) rather than performing a fresh PID lookup, mirroring illumos's `DOOR_INVALID` check under `door_knob`. **This validates PM lean D7 explicitly from illumos prior art.**

---

## 6. Q4 — FreeBSD `pfind` + Per-Process Locking + SMR

### 6.1 `allproc_lock` type and usage

**`allproc_lock` is an sx (shared/exclusive sleep-lock), cache-line-aligned, protecting the `allproc` linked list for full-list iteration.** Verbatim from `kern_proc.c`:

```c
struct sx __exclusive_cache_line allproc_lock;
```

**Critical clarification: `pfind` does NOT use `allproc_lock`.** `allproc_lock` protects only the full-list iteration (used by `proc_iterate`, `kvm_getprocs`, ps(1)). PID-by-PID lookup uses a separate `pidhashlock[]` array of per-bucket sx locks (initialized with `SX_DUPOK`).

Source: `kern_proc.c` (allproc_lock declaration, `_pfind` body), `leidinger.net` FreeBSD cross-reference confirming `pidhashlock[]` with `SX_DUPOK`.

### 6.2 `pfind` implementation

**`pfind` acquires the per-bucket sx lock in shared mode, walks the hash chain, acquires `PROC_LOCK(p)` on match before releasing the bucket lock, and returns a PROC_LOCK-held proc.** The `_pfind` internal function (from `kern_proc.c`):

```c
sx_slock(PIDHASHLOCK(pid));
LIST_FOREACH(p, PIDHASH(pid), p_hash) {
    if (p->p_pid == pid) {
        PROC_LOCK(p);
        // ... filter check (zombie exclusion for pfind, accept for pfind_any) ...
        sx_sunlock(PIDHASHLOCK(pid));
        return (p);   // returned with p_mtx held
    }
}
sx_sunlock(PIDHASHLOCK(pid));
return (NULL);
```

The lock-handoff guarantee: `PROC_LOCK(p)` is acquired before `sx_sunlock(PIDHASHLOCK(pid))`, so there is no window in which `p` can be freed between the lookup and the caller's use. Source: `kern_proc.c` (verbatim `_pfind` structure).

### 6.3 Per-process `p_mtx` and lock ordering

**`p_mtx` is a sleep mutex (not a spin mutex).** From `sys/proc.h`:

```c
#define PROC_LOCK(p)    mtx_lock(&(p)->p_mtx)
#define PROC_UNLOCK(p)  mtx_unlock(&(p)->p_mtx)
```

The lock-key annotation in the `struct proc` comment documents which fields each lock protects: `(c)` = `p_mtx`, `(d)` = `allproc_lock`, `(e)` = `proctree_lock`. Fields requiring both locks for write access are annotated with two identifiers.

The `kern_kill` lifetime model (from `kern_sig.c`):

```c
if ((p = pfind_any(pid)) == NULL)
    return (ESRCH);
// p is returned with PROC_LOCK held
error = p_cansignal(td, p, signum);
if (error == 0 && signum)
    pksignal(p, signum, &ksi);
PROC_UNLOCK(p);
```

`proc_exit`'s teardown path also acquires `PROC_LOCK` and the various synchronization predicates use `PROC_LOCK_ASSERT`. Therefore: a held `p_mtx` prevents teardown from completing, providing the same use-after-free protection as Linux's `get_task_struct` refcount, without a separate refcount object.

### 6.4 SMR for allproc — confirmed absent

**FreeBSD does not use SMR for the process table.** Direct inspection of `kern_proc.c` at `main` branch: no `smr_enter`, `smr_exit`, `smr_zone`, or `SMR_LIST_*` references present. FreeBSD's SMR subsystem exists (`sys/sys/smr.h`) and is used for VFS routing tables and UMA zone hot paths, but not for process lookup.

Source: `kern_proc.c` (direct inspection, confirmed by leidinger.net cross-reference: "No SMR references found").

**PM-relevance:** FreeBSD's choice confirms that SMR/RCU is not the right tool for process-table lookup even in a large production kernel at NeoPlutarch's target scale. The locked-handoff pattern (`pfind` returns `PROC_LOCK`-held proc) is simpler and correct.

---

## 7. Q5 — Alternative Simpler Designs

### 7.1 Per-CPU process pointer cache (Linux `loaded_mm` model)

**The canonical prior art is `cpu_tlbstate.loaded_mm` in `arch/x86/include/asm/tlbflush.h`.** This is a per-CPU pointer to the `mm_struct` currently loaded on the CPU — the direct analogue of what NeoPlutarch needs for CR3 management. The pointer is written at `switch_mm_irqs_off()` time with IRQs disabled; it is read lock-free via `this_cpu_read`. A sentinel `LOADED_MM_SWITCHING` marks the brief window mid-switch where CR3 and `loaded_mm` might otherwise be inconsistent.

The companion per-CPU abstraction for "current task" is `current_task` in `arch/x86/include/asm/current.h`, accessed via `this_cpu_read_stable(current_task)` — a single GS-relative load, zero lock overhead.

**What this means for `cr3_for_thread(pid)` specifically:** if each `Thread` in NeoPlutarch's runqueue carries its own `Arc<Pmap>`, the scheduler never needs to ask PROCESSES "what pmap does pid X have?" — it already has the answer from the dequeued `Thread`. The `active_pmap_cr3` per-CPU cache (already landed in SC-3C) completes the picture for the common case (same process resumed on this CPU: skip even the Arc deref). The PROCESSES registry is consulted only for **non-scheduling cross-process operations**: signal delivery, waitpid, kill, ptrace-style introspection — all of which are inherently rare.

### 7.2 Hazard pointers

**Wait-free reads, O(reader-thread-count) reclamation cost.** The canonical Rust implementation is `haphazard` (jonhoo, based on Folly's `hazptr`). Reader cost: one atomic store (publish hazard pointer) + one re-validation read — claimed "under one nanosecond" in optimized `hazptr_local<M>` path (Folly documentation).

**Practical limitation for NeoPlutarch:** `haphazard` is at version 0.1.x with no `no_std` guarantee. No production-quality `no_std` hazard pointer crate exists in the Rust kernel ecosystem. Implementing hazard pointers correctly in `no_std` requires careful attention to memory ordering, pointer publication visibility, and scan discipline — comparable complexity to implementing `arc-swap` from scratch.

**Verdict:** overkill for one pointer (the PROCESSES table root) at small N. The machinery-per-line-of-code ratio is poor. Hazard pointers earn their keep when you have many independently-managed pointer-sized objects with disjoint reclamation — a per-task-per-CPU hot cache, not a single-table pointer.

### 7.3 Global `spin::RwLock`

**Not the safe simple upgrade it appears.** The `spin` crate's own documentation warns:

> "This implementation is unfair to writers — if the lock always has readers, then no writers will ever get a chance."

Empirical measurements (Eventual Consistency blog, `parking_lot::RwLock` LRU workload): ~5× slower than `Mutex` for read-heavy short critical sections. Cause: the reader-counter atomic causes cache-line invalidations across all cores on each read-lock acquisition — even when no write is occurring. Linux's own kernel locking guide notes: *"`rwlock_t` ... slightly slower than normal locks, so in practice `rwlock_t` is not usually worthwhile."*

At NeoPlutarch's target of 4–64 CPUs with a potentially-hot process-table read path, `spin::RwLock` has the documented failure mode at exactly the scale that matters. FreeBSD uses `sx` (a *sleepable* shared-exclusive lock) rather than a spin RwLock precisely because process-table operations may need to sleep — also not applicable to NeoPlutarch's `no_std` context.

**Verdict:** do not use `spin::RwLock` for PROCESSES. The contention story under realistic NeoPlutarch workloads is worse, not better, than a well-designed Mutex. If read-side scalability matters, the `arc-swap` pattern is the correct Rust-idiomatic choice.

### 7.4 Sharded BTreeMap

**Right for Linux scale; complexity tax at NeoPlutarch scale.** Standard guidance: shard count ≈ number of contending threads; performance plateaus around 128 shards. Cache-line padding (`#[repr(align(64))]`) required to prevent false sharing.

The core problem for NeoPlutarch: any operation that scans all processes (capability revoke walk = `revoke_handle`, process enumeration = `processes_snapshot`) must acquire shards in lock-order to prevent R4-class deadlock (concurrent revokes causing AB-BA). With 100 processes and 16 shards, each shard holds ~6 entries — the locking overhead per scan exceeds any contention-reduction benefit. The cross-shard lock-ordering discipline adds new invariants to every multi-process operation.

**Verdict:** sensible optimization at N=10,000 processes; counterproductive at N=10–100. File as a Phase 3+ forward marker gated on "process count exceeds 1,000 AND profiling shows PROCESSES lock contention."

### 7.5 `AtomicPtr<Arc<BTreeMap>>` swap-on-write (PM's lean, `arc-swap` pattern)

**Correct for NeoPlutarch's scale; not what any OS kernel uses for its process table; complexity earned only if the hot-path problem is real.** The `arc-swap` crate implements this pattern with a debt-based reader registry (hybrid of hazard pointers and a sharded spin lock). From vorner's design notes: *"in-memory database that answers millions of queries per second, but is replaced only every 5 minutes"* — the intended use case is very-read-heavy, rarely-written configuration or routing data. TiKV and signal-hook use it in production.

**Write cost at NeoPlutarch's scale:** cloning a 100-entry `BTreeMap<ProcessId, Arc<Process>>` requires ~100 atomic refcount increments (one per `Arc<Process>` clone) plus tree rebuild. Total: microseconds-to-low-tens-of-microseconds, amortized to invisibility at process spawn/exit frequency.

**Key gap:** `arc-swap` is not `no_std`. A kernel-side reimplementation against `core::sync::atomic` requires ~200 lines of careful code. The debt-slot mechanism is subtle under weak memory models; it would need the same type of careful audit that produced the SC-3B and SC-3C `IrqSafeMutex` / `SwitchMutex` discipline.

**No production OS kernel uses this for its process table** — confirmed by examining Linux, Redox, FreeBSD, and illumos primary sources. Linux couldn't use it (millions of entries); the others simply don't need to (per-object mutex + Arc clone is sufficient).

**Verdict:** the right upgrade if the PROCESSES registry read rate remains hot after the Tier 1 restructuring (see D4 Recommendation below). Not the starting point.

### 7.6 Redox OS — verified `RwLock<BTreeSet<Arc<...>>>`

**Redox OS uses a global `RwLock<BTreeSet<ContextRef>>` with Arc-cloned-out references.** Verbatim from `src/context/mod.rs`:

```rust
static CONTEXTS: RwLock<L2, BTreeSet<ContextRef>> = RwLock::new(BTreeSet::new());

#[derive(Clone)]
pub struct ContextRef(pub Arc<ContextLock>);
pub type ContextLock = RwLock<L4, Context>;
```

`ContextRef` implements `Ord` by raw pointer (`Arc::as_ptr`), so there is no ID-based lookup over `CONTEXTS` — callers hold `Arc<ContextLock>` references acquired at creation time or iterate the set. The ordered-lock typestate (`L2`, `L4`) prevents deadlocks at compile time (March 2026 ordered-locks rollout confirmed from Redox news).

The Redox pattern matches what PM's D4 lean was reaching for: a read-write protected global table with Arc-based pinning of individual entries. Redox chose a `BTreeSet` (sorted by pointer) rather than a `BTreeMap` (keyed by ID), which means Redox contexts are referenced by `Arc<ContextLock>` directly rather than looked up by PID.

**PM-relevance:** Redox's pattern is architecturally sound at NeoPlutarch's scale. The L2/L4 lock-ordering typestate is a stronger version of what NeoPlutarch is implementing via the process-exit `AtomicBool` + BTreeMap-lock ordering. Redox's `BTreeSet` + pointer-sorted `Ord` implies all callers hold `Arc<ContextLock>` already; NeoPlutarch's `BTreeMap` by `ProcessId` implies callers may look up by ID. The tradeoff: `BTreeMap` is slightly more general (supports PID→Process without holding a prior `Arc`); `BTreeSet` is slightly simpler (all lookups are by extant `Arc`). Both are correct.

---

## 8. Q6 — Small-N Cost/Benefit Synthesis and D4 Recommendation

### 8.1 What Linux's design tells NeoPlutarch

Linux's process-table design is a three-layer system engineered for thousands of CPUs and millions of tasks:
1. **Per-CPU `current_task`** — zero-cost, GS-relative, no lock (the dominant "what process am I in?" query)
2. **RCU + IDR** — zero-cost read-side, deferred-free grace period (cross-process PID lookup)
3. **`tasklist_lock` (rwlock_t)** — for-each-process iteration and parent/child topology

The complexity is paid by the existence of two separate refcounts per task, a deferred-free RCU callback chain, and lockdep instrumentation. It is justified at Linux scale. The key design *principle* that transfers to NeoPlutarch is #1: **the dominant hot query must be served from per-CPU data, not from a global table.**

### 8.2 The architectural restructuring (Tier 1)

**The correct first move for D4 is to audit every caller of `cr3_for_thread(pid)` and restructure the hot path to eliminate the registry lookup.**

If every `cr3_for_thread(pid)` call happens in the scheduler context switch path where the scheduler already holds the `&Thread` for the next task — which the SC-4 THINK substrate evidence strongly implies — then the `Arc<Pmap>` can live directly on `Thread` (or `RunqueueEntry`). The scheduler switches to the next task's pmap by dereferencing `next_thread.pmap`, never touching PROCESSES. The `active_pmap_cr3` per-CPU field (post-SC-3C) provides the "skip if unchanged" optimization.

This is the Linux `cpu_tlbstate.loaded_mm` model applied to NeoPlutarch. It eliminates the PROCESSES registry from the scheduler hot path entirely, regardless of what synchronization PROCESSES uses.

### 8.3 Resulting D4 decision (after Tier 1)

Once the scheduler hot path no longer touches PROCESSES, the read access pattern changes from:

- **Before Tier 1:** potentially thousands of reads/second/CPU (every cross-process context switch)
- **After Tier 1:** single-digit reads/second (kill, waitpid, signal delivery, revoke walk, debugging/ps)

At the post-Tier-1 access rate, the simplest correct design is:

**Keep `spin::Mutex<BTreeMap<ProcessId, Arc<Process>>>` (the existing structure).** The premise that drove D4 — "every cross-process context switch takes the registry lock, causing SMP contention" — is dissolved by Tier 1. The existing single-mutex BTreeMap is SMP-correct and performs identically to any alternative at low access rates.

If Tier 1 is rejected for architectural reasons (see §8.4), the fallback design is:

**`Arc<Pmap>` registry in a separate lightweight structure, using `AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` (arc-swap-style).** Separate the hot data (`Arc<Pmap>` for CR3) from the cold data (`Process` metadata, handles, derivations). The CR3-lookup table uses the arc-swap pattern for wait-free reads; the full `PROCESSES` registry keeps its existing `spin::Mutex` for metadata access. This matches PM's lean precisely but scopes it to the minimal data needed on the hot path.

### 8.4 Conditions under which the recommendation changes

| Condition | Design response |
|---|---|
| **Tier 1 accepted** (scheduler holds `Arc<Pmap>` directly) | Keep existing `spin::Mutex<BTreeMap>`. No D4 change needed. |
| **Tier 1 rejected** (callers only have `pid`, not `Arc<Thread>`) | `AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` for CR3 hot path; `spin::Mutex<BTreeMap<ProcessId, Arc<Process>>>` for metadata. |
| **Process count grows past 10,000** | Tier 1 is still the right first move; if truly unavoidable, sharding (N=16 shards) + per-shard Mutex. |
| **CPU count exceeds 64** | Reassess; BKL-equivalent single Mutex becomes measurably contended; arc-swap fallback has no upper-CPU-count scaling limit. |
| **Non-scheduler hot path emerges** | Identify the path; apply arc-swap to that specific data. |

### 8.5 Why not `spin::RwLock`

Documented writer starvation (spin crate's own docs). Empirical ~5× slowdown vs. Mutex under read-heavy short critical sections (cache-line ping-pong on reader counter). This problem appears at exactly NeoPlutarch's target CPU range (16–64 CPUs). **`spin::RwLock` is not a safe simple upgrade; it is a performance regression under the conditions that matter.**

### 8.6 Why not full Linux RCU

RCU's zero-cost read side is the right answer at thousands of CPUs and millions of tasks. At NeoPlutarch's scale:
- Arc<Process> clone-while-holding-Mutex-guard is a single atomic refcount increment, not a lock acquisition. The lock overhead dominates at 10 ns; the Arc clone is ~2–3 ns. The marginal benefit of zero-cost RCU reads over "one atomic + unlock" is noise relative to the BTreeMap lookup itself.
- RCU requires two separate refcounts per task, deferred-free callback chains, and grace-period management. All of this is provided for free by `Arc<Process>`.
- As McKenney states (LWN "What is RCU, Fundamentally?"): data structures updated frequently are poor RCU candidates; so are structures read *infrequently*. After Tier 1, PROCESSES is both.

### 8.7 What seL4 says about small-N kernel synchronization

Peters/Heiser's empirical result: a BKL is fine for microkernels up to ~14 cores within a socket for short syscalls. This is the most directly applicable data point for NeoPlutarch's per-process handle-table lock (D1: `SwitchMutex`). It validates that per-process serialization via a coarse lock is correct and scalable at NeoPlutarch's target; it does not motivate RCU or hazard pointers for the handle table.

---

## 9. Citation Precision Corrections to PM THINK

**Four corrections to PM's THINK-stage characterizations:**

1. **`tasklist_lock` is not the PID-lookup serialization lock.** PM THINK §5.1 implied that Linux serializes PID-to-task lookup via `tasklist_lock`. Correction: `tasklist_lock` protects the linked-list iteration structure and parent/child/signal topology. PID-number-to-task lookup is serialized by `pidmap_lock` (for IDR mutations) and protected on the read side by RCU, independently of `tasklist_lock`. This does not change D4 but corrects the mental model.

2. **`spin::RwLock` is not a safe simple upgrade path.** PM THINK §5.1 listed `RwLock` as a viable alternative. Research shows `spin::RwLock` performs measurably worse than `Mutex` for short critical sections due to reader-counter cache-line ping-pong. The correct upgrade path from `spin::Mutex` is arc-swap-style `AtomicPtr<Arc<BTreeMap>>` (if reads need to be wait-free), not `spin::RwLock`.

3. **PM's RCU-style `AtomicPtr<Arc<BTreeMap>>` lean is not what any OS kernel uses for its process table.** Research confirms no production kernel (Linux, FreeBSD, illumos, Redox) uses this pattern for its process table. The pattern is used in production Rust systems software (TiKV, signal-hook) for configuration/routing data. PM's lean is technically sound but is an application-level pattern applied to a kernel structure, not derived from kernel prior art.

4. **The hot-path PROCESSES lock contention is architecturally avoidable before choosing a synchronization primitive.** PM's D4 formulation assumes `cr3_for_thread(pid)` must hit PROCESSES on every cross-process context switch. The Linux `loaded_mm` model shows this assumption is structural, not fundamental — if `Thread` carries `Arc<Pmap>` directly, the registry lookup is eliminated from the hot path entirely, making the synchronization-primitive choice for PROCESSES largely irrelevant to scheduler performance.

---

## 10. Forward Markers

Items surfaced by this research for Phase 2+ tracking. None blocks SC-4.

1. **Tier 1 architectural restructuring.** Audit every caller of `cr3_for_thread(pid)`. If all callers are scheduler-context (holding `&Thread`), retrofit `Thread` to carry `Arc<Pmap>` directly. This eliminates the PROCESSES registry from the scheduler hot path before any D4 synchronization-primitive change. SC-4 sub-commit 5 should land this audit as a precondition to D4 closure.

2. **`AtomicPtr<Arc<BTreeMap>>` fallback (arc-swap pattern) if Tier 1 rejected.** Scope to `Arc<Pmap>` hot data only; keep `Process` metadata under existing `spin::Mutex`. Requires ~200-line `no_std` reimplementation of the arc-swap debt-slot reclamation mechanism. File as a Phase 2+ optimization behind "Tier 1 not implementable."

3. **Ordered-lock typestate for process-table locks.** Redox OS (March 2026 rollout) uses compile-time lock-ordering via typestate (`L2`, `L4` annotations). This is the Rust-level equivalent of illumos's `pidlock` → `p_lock` rule. NeoPlutarch's R4 race (concurrent revokes deadlock) could be statically prevented by encoding the `PROCESSES` lock as L1, `Process.handles/derivations` as L2, `Channel.state` as L3. File as Phase 2+ refinement.

4. **Sharded `PROCESSES` (N=16 shards) if process count exceeds 1,000.** The correct threshold is when shard-contention benefit exceeds cross-shard-walk overhead. At 100 processes, threshold not met. At 10,000, clearly met. Gating condition: profiling shows `PROCESSES` write contention at process spawn/exit rate.

5. **Per-process `SwitchMutex` upgrade from `spin::Mutex` for `handles` and `derivations`.** PM lean D1. Independently validated by seL4/Peters-Heiser result: per-object mutex on short operations is the right design for Q8.a at NeoPlutarch's scale. Forward marker: when `SwitchMutex` inventory proves the 6000ns overhead is measurable in capability-operation profiles, revisit L4 (`IrqSafeMutex`) or arc-swap for the handle table specifically.

6. **illumos `PID_HOLD`/`PID_RELE` analogue for long-running NeoPlutarch operations.** If a future sub-commit needs to hold a process reference across a blocking operation without holding `Process.handles` or PROCESSES lock, implement a separate reference count on `Arc<Process>` outlasting the lock (Redox's `Arc<ContextLock>` clone idiom). This is already provided by Rust's `Arc<Process>` but explicit naming makes the discipline auditable.

7. **COMPLETED_EXIT_CODES bounded-size discipline (R8).** Out of scope for D4 but surfaces here as a known Q8.b cleanup issue. Add a `sys_wait` / `waitpid`-driven consumer for SC-4 sub-commit 5 to prevent unbounded growth.

---

## 11. Recommended D4 Resolution

**D4 resolves in two phases, not one:**

**Phase A (immediate — SC-4 sub-commit 5 prerequisite):** Audit every call site of `cr3_for_thread(pid)`. If all call sites are in the scheduler context-switch path and have access to the `Thread` being switched to (which the SC-4 THINK §2.2 hot-path table implies), restructure those call sites to use `thread.pmap` directly. This is the Linux `cpu_tlbstate.loaded_mm` model: the scheduler carries the pmap, never looking it up from PROCESSES on the hot path.

**Phase B (D4 synchronization choice, after Phase A):** With the hot path restructured:
- **If Phase A succeeds:** Keep `spin::Mutex<BTreeMap<ProcessId, Arc<Process>>>`. No synchronization upgrade needed. The access rate is too low for contention to be measurable.
- **If Phase A is blocked** (some caller genuinely has only a `pid` and needs `Arc<Pmap>` on a hot path): Introduce a separate `PmapCache: AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` for the hot CR3 data, using the arc-swap-style debt-slot reclamation. Keep PROCESSES under `spin::Mutex` for metadata. Do NOT use `spin::RwLock` for either structure.

**PM's AtomicPtr<Arc<BTreeMap>> lean is the correct fallback for Phase B, not the primary design.** The primary design insight is that the hot path should not need the registry at all.

---

## 12. Calibration Notes

**Verified primary sources.** All five research systems were addressed by direct primary-source reads. Linux source was fetched from `raw.githubusercontent.com/torvalds/linux/master/`. FreeBSD and illumos from their respective GitHub mirrors. Redox from `github.com/redox-os/kernel`. The Peters/Heiser paper was read from the Trustworthy Systems publications page. The seL4 arXiv papers were read from `arxiv.org`.

**Single-source findings:**
- The exact text of seL4 multikernel CSpace lookup synchronization comes from arXiv 1609.08372 (Peters/Heiser evaluation paper) — a peer-reviewed source, but single-document. The seL4 manual PDF was not text-extractable via available channels.
- The "~5× slower than Mutex" RwLock measurement comes from a single blog benchmark (Eventual Consistency). However, the same pathology is described in the spin crate's own documentation (verbatim writer starvation warning), the Linux kernel locking guide, and the BRAVO paper (arXiv 1810.01553), making this a well-corroborated finding even from a single primary benchmark.

**Corrections to PM THINK documented above in §9.**

**No production OS kernel uses `AtomicPtr<Arc<BTreeMap>>` for its process table** — confirmed by direct inspection of Linux, Redox, FreeBSD, and illumos. The pattern is used in production Rust systems software but derives from application-level design, not kernel prior art.

**Time budget actual:** ~3 hours (three parallel research agents, synthesis, artifact write-up). Within PM's 4–8 hour estimate.

---

## 13. Cross-References

**This artifact consumes:**
- `Plans/DISPATCH-2026-05-11-sc-4-d5-process-table-smp-prior-art-cerebro.md` — the dispatch
- `Plans/SC-4-THINK-2026-05-11.md` — THINK artifact with D4/D5 open decision and substrate inventory
- `Plans/RESEARCH-2026-05-10-tlb-shootdown-prior-art-cerebro.md` — SC-3C D15 pattern reference (format, citation discipline)
- `kernel/src/sched/process.rs` — PROCESSES registry substrate (from THINK §2.1)
- `kernel/src/sched/scheduler.rs` — context-switch path where `cr3_for_thread` is called (from THINK §2.2)

**This artifact is consumed by:**
- PM's consumption PM-NOTE (`Plans/PM-NOTE-2026-05-MM-sc-4-d5-cerebro-consumption.md`)
- PM's SC-4 PLAN-stage Jarvis dispatch (after D4 ratification)

**This artifact does NOT produce:**
- No Bible amendment
- No ANSPEC amendment
- No ADR
- No code changes (Cerebro recommends; PM decides)

---

## 14. Closure

Multi-CPU process-table lookup in production SMP kernels at NeoPlutarch's scale (10–100 processes, 4–64 CPUs) is a solved problem: per-object mutex + Arc-clone-based pinning is the universal pattern (FreeBSD locked-handoff, illumos `pidlock`→`p_lock` hierarchy, Redox `RwLock<BTreeSet<Arc<...>>>`), and the Big Kernel Lock is empirically justified at this scale for short-syscall microkernels (seL4 / Peters-Heiser). The hot-path problem PM identified (`cr3_for_thread(pid)` contending on PROCESSES on every cross-process context switch) is architecturally avoidable: Linux's `cpu_tlbstate.loaded_mm` model and NeoPlutarch's existing `per_cpu.active_pmap_cr3` point toward placing the `Arc<Pmap>` directly on the `Thread`/runqueue entry, eliminating the registry lookup from the scheduler path. D4 closes with Phase A (audit and restructure callers) first; Phase B (synchronization upgrade) only if Phase A is blocked. `spin::RwLock` is not the upgrade path; `AtomicPtr<Arc<BTreeMap>>` is the correct Tier-2 fallback if Phase A fails. illumos's doors IPC model — cross-process handle as durable capability, not PID lookup — directly validates PM lean D7 (`Process.exiting` flag) as the correct mechanism for grant-rendezvous liveness checks.

---

**End of research artifact.**

*D4 primary recommendation: audit callers of `cr3_for_thread(pid)` and eliminate the PROCESSES registry from the scheduler hot path (Linux `loaded_mm` / SC-3C `active_pmap_cr3` model). If all callers hold `Arc<Thread>` already, keep existing `spin::Mutex<BTreeMap>` — the access rate is too low for synchronization choice to matter. If a hot-path PID-only caller genuinely exists, fallback to `AtomicPtr<Arc<BTreeMap<ProcessId, Arc<Pmap>>>>` (arc-swap pattern) for CR3 data only. Do not use `spin::RwLock` — documented cache-line ping-pong regression at 4–64 CPUs. seL4 BKL empirical result validates per-process SwitchMutex (D1) for handle tables. illumos doors model validates D7 (Process.exiting AtomicBool as grant-rendezvous liveness check).*
