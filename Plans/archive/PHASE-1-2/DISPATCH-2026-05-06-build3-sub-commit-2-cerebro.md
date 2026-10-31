# Dispatch: BUILD-3 sub-commit 2 — Per-CPU schedulers + Q5 mechanism deletion

**To:** Cerebro (coder 1)
**From:** PM
**Date:** 2026-05-06
**Parent commit:** `503d3dc` (currently HEAD on `main`)
**ADR substrate:** ADR-013 §Q3 (per-CPU schedulers), §Q5 (Invariants 2/3/4 deletion), §Q9 (per-CPU reapers)
**Spec substrate:** Bible Article 8.3 (per-CPU run queues, no global locks in hot paths)
**Estimated surface:** Substantial — ~400–700 lines net across 4–6 files. Largest dispatch since sub-commit 1a. Includes the highest-conviction empirical bet in BUILD-3 (the §Q5 claim).

---

## 0. Standing instructions

These are the discipline rules from sub-commit 1a/1b. They are not optional.

1. **Commit on `main`, not detached HEAD.** Verify with `git status` and `git branch --show-current` before each commit.
2. **Verify boot-order claims via direct file read.** Two prior dispatches got boot-order wrong. Do not trust this dispatch's prose where it could disagree with reality; trust the file.
3. **Flag deviations explicitly** in an ADR-013 amendment subsection if implementation requires departing from this dispatch shape. Sub-commits 1a (Amendment 1, static TSS/IST storage) and 1b (Amendment 2, sequential cpu_id assignment + AP CR0/CR4 setup) are the right pattern.
4. **Run verification before reporting it.** Boot the kernel; capture serial output; quote literal lines.
5. **Diff warnings against baseline.** Current baseline at `503d3dc` is **18 warnings**, PM-verified. Surface every delta with one-line explanation.
6. **Pre-implementation OBSERVE pass.** Read every file you'll touch before writing the first line. The OBSERVE list in §3 is mandatory.

---

## 1. What sub-commit 2 does

**Three coupled changes that must land together:**

1. **Per-CPU schedulers** (Q3). Replace `static SCHED: Lazy<Mutex<Scheduler>>` with `[PerCpuScheduler; MAX_CPUS]` accessed via `PER_CPU[cpu_id].scheduler_ptr`. Each CPU has its own current/ready/zombies queues and its own scheduler lock. APs adopt their bootstrap thread and run their own scheduler loop. No global scheduler.

2. **Q5 deletion** — the deferred-quantum mechanism is removed entirely. `HANDOFF_PREEMPT_DEFER_TICKS`, `Thread.preempt_defer_ticks`, `consume_preempt_defer`, `clear_current_preempt_defer`, `defer_current_preempt_for_handoff` — all gone. Three call sites updated (one in `on_tick`, one in `syscall_dispatch`, one in `wake_thread_front`, one in IPC's `continue_current_after_receive`).

3. **Per-CPU reapers** (Q9). Each CPU's bootstrap thread runs its own reaper loop on its CPU-local zombie queue. `on_thread_reap` is restructured to use the existing `PROCESSES.lock()` (still a `Mutex` in sub-commit 2; Q8.a's `RwLock` migration is sub-commit 5) plus an atomic `live_thread_count` on Process so concurrent reaps from different CPUs don't double-deregister.

**What sub-commit 2 does NOT do:**

- **No cross-CPU IPC.** APs run their own scheduler but cross-CPU rendezvous returns `EAGAIN`. That's sub-commit 3a/3b.
- **No ManuallyDrop + asm release** in `block_current_into`. Sub-commit 3a.
- **No TLB shootdowns.** Sub-commit 4.
- **No IPI infrastructure.** Sub-commit 4.
- **No `RwLock` on PROCESSES.** Stays `Mutex` in sub-commit 2; Q8.a is sub-commit 5.
- **No `kill_thread_by_pid` blocked-thread walk.** Q8.c is sub-commit 5.

**Goal milestone:** Boot QEMU with `-smp 4`. APs adopt their bootstrap threads and run them as kernel-thread "main" loops (each running its own per-CPU reaper). The full default test suite passes under both `-smp 1` and `-smp 4`. **`ipc_heap_kill_smoke` runs 100x consecutively under `-smp 4` with zero flakes.** That last gate is the empirical test of the §Q5 claim.

---

## 2. The §Q5 claim (read this carefully)

This is the highest-conviction empirical bet in BUILD-3. ADR-013 §Q5 specifies:

> Sub-commit 2 deletes:
> - The `HANDOFF_PREEMPT_DEFER_TICKS` constant in `scheduler.rs`.
> - The `Thread.preempt_defer_ticks` field.
> - `consume_preempt_defer()` and its call from `on_tick()`.
> - `clear_current_preempt_defer()` and its call from `syscall_dispatch`.
> - `defer_current_preempt_for_handoff()` and its call from `wake_thread_front`.

The argument that this is correct under SMP+per-CPU+wake-on-completer-CPU goes (paraphrased from §Q5):

1. CPU0's sender extracts CPU1's blocked receiver from the Channel mutex. (Q4 wake-on-completer-CPU; in sub-commit 2 cross-CPU IPC is EAGAIN'd, so the Q4 cross-CPU path doesn't exercise yet — but the *same-CPU* IPC path on each CPU still uses the same mechanism.)
2. CPU0 takes its own scheduler lock. IF=0 throughout (`without_interrupts` discipline, every IPC syscall wraps this).
3. Front-queues the receiver, yields. Atomic critical section under IF=0.
4. Context switch happens with IF=0. Receiver becomes current.
5. After the context switch, IF gets re-enabled when the IPC syscall's `without_interrupts` scope exits.

The race the deferred-quantum was patching: a stale PIT tick on the same CPU, fired in the narrow window between wake-of-peer (front-queue) and yield-by-completer. Under per-CPU schedulers + IF=0 across that window, the race window is closed by construction — no PIT tick can fire on this CPU while IF=0.

**This is the bet.** If wrong, `ipc_heap_kill_smoke` flakes under SMP. The 100x consecutive run is the test.

**What you need to do:**

- Implement Q5's deletions exactly as specified.
- Run `ipc_heap_kill_smoke` 100x consecutively on `NEO_SMP=4`. Zero flakes is the closure gate. Any flake means: **STOP, do not close, surface to PM.** The mitigation per §Q5 is to re-introduce the deferred-quantum mechanism (option b after all) — but that decision is PM's, not yours.

Note the asymmetry: a 100x clean run does not prove the race is gone, only that the test doesn't catch it at this scale. The closure gate accepts this — sub-commit 2 closes if 100x is clean; if a later sub-commit's testing reveals the race, that's an ADR amendment moment, not a sub-commit 2 reopen.

---

## 3. Pre-implementation OBSERVE pass (mandatory)

Before writing a single line, read these files in full. Note specific line numbers; the dispatch quotes some, but trust the file.

1. **`kernel/src/sched/scheduler.rs`** — the entire file. The structure is:
   - Lines 1–47: module doc + imports.
   - Lines 49–67: constants and `ZOMBIE_COUNT` (the one atomic that already lets `reap()` skip the SCHED lock on empty steady state — Q9 carries this pattern forward per-CPU).
   - Lines 69–77: `Scheduler` struct + `SCHED` static.
   - Lines 80–104: `init()` and the `pit::init` + `pic::unmask_irq` ordering.
   - Lines 107–125: `spawn`, `push_ready`.
   - Lines 145–180: `rotate` — the queue-rotation primitive used by `yield_now` and `exit`.
   - Lines 182–210: `cross_process_prep`, `cross_thread_kstack`.
   - Lines 230–245: `on_tick`, `consume_preempt_defer`, `clear_current_preempt_defer`, `defer_current_preempt_for_handoff` — these are the §Q5 deletion targets.
   - Lines 280–340: `block_current_into` — the lock-order inversion comment is at lines 305–319; do not touch the implementation in sub-commit 2 (3a's territory).
   - Lines 363–395: `wake_thread`, `wake_thread_front`, `prepare_woken_thread` — `wake_thread_front` calls `defer_current_preempt_for_handoff` for the calling thread; that line goes in §Q5.
   - Lines 408–425: `kill_thread_by_pid` — has a Phase 1.5b limitation comment about Blocked threads. Q8.c's blocked-walk lifts it in sub-commit 5; sub-commit 2 leaves the limitation in place.
   - Lines 437–470: `reap` — the global zombie drain. Becomes per-CPU in Q9.

2. **`kernel/src/sched/thread.rs`** — `Thread` struct definition and `preempt_defer_ticks: u8` field at line 101. The field is initialized to 0 at three Thread-construction sites (lines 173, 191, 279). Q5 deletes the field; the three initializers go with it.

3. **`kernel/src/sched/process.rs`** — `on_thread_reap` at lines 659+. Reads `PROCESSES`, walks handle table, calls `close_channel_peers`, removes from registry. The "two PROCESSES.lock() acquisitions, intentional" comment at lines 648–658 names the BUILD-3 lesson; do not refight that battle. Q9's per-CPU reaper invokes `on_thread_reap` per-zombie just like the current global reaper does — the reaper distribution is in scheduler.rs, not here.

   **What to add to `Process`:** a `live_thread_count: AtomicU32` field (per Q9) initialized to 1 in `new_user` and `new_kernel`. Increment when a thread is spawned for that pid; decrement at the start of `on_thread_reap`. Only proceed with deregister if the post-decrement count is 0 with a double-check under the registry lock per Q9's pseudocode.

4. **`kernel/src/ipc/mod.rs`** — `continue_current_after_receive` at lines 466–467 calls `sched::defer_current_preempt_for_handoff()`. That call goes in §Q5; the function name itself becomes invalid. Replace with: nothing. The function body becomes just `sched::wake_thread_front(thread, return_value);` and the explanatory comment updates accordingly.

5. **`kernel/src/arch/x86_64/syscall.rs`** — find every call to `clear_current_preempt_defer`. There should be one (per ADR-013 §Q5 description) at the syscall entry path. That call goes in §Q5.

   ```
   Select-String -Path "kernel\src\arch\x86_64\syscall.rs" -Pattern "clear_current_preempt_defer|preempt_defer"
   ```

6. **`kernel/src/arch/x86_64/per_cpu.rs`** — confirm `PER_CPU[N].scheduler_ptr: AtomicPtr<PerCpuScheduler>` is the field reserved for this in sub-commit 1a. Confirm the type name is what's expected. The pointer is null until each CPU's scheduler is initialized.

7. **`kernel/src/arch/x86_64/smp.rs`** — `ap_entry` currently calls `halt_forever()` after slab init. Sub-commit 2 replaces that final `halt_forever()` with a call to `sched::run_ap_bootstrap(cpu_id)` (new function in scheduler.rs).

8. **`kernel/src/main.rs`** — current `_kernel_start` calls `sched::init()` at line ~293, then `sched::smoke_test()`. Sub-commit 2 changes `sched::init()` to per-CPU shape. The order remains: `mm::init` → `arch::x86_64::smp::bring_up_aps` → `sched::init_bsp()` → `sched::smoke_test()`. APs come up *before* BSP scheduler init because AP entry calls `sched::run_ap_bootstrap` which constructs the AP's scheduler — it does not need the BSP's scheduler to exist. (Verify this is true; if AP scheduler bring-up has any dependency on BSP scheduler state, the order has to flip and §3 of the close report names the dependency.)

9. **Test scenario harness:** `tools/xtask/src/main.rs`, scenarios array. Sub-commit 2 adds two new scenarios: `smp_per_cpu_scheduler_smoke` and `smp_same_cpu_ipc`. Look at the existing scenario shape to know what feature flags and what test bodies look like.

10. **`Plans/HANDOFF-2026-05-05-build3-sub-commit-1a-close.md`** §0.5 — the tool-access guide. Use background `cargo k` builds for repeated runs; use `Select-String` for grep operations.

If any of these contradicts this dispatch, **flag the contradiction in your close report** and either deviate (with ADR-013 amendment subsection) or surface to PM before continuing. Do not silently rewrite.

---

## 4. Implementation plan (suggested, not normative)

The shape that follows is what PM expects. If you find a better factoring during OBSERVE, deviate and document.

### 4.1 The new `PerCpuScheduler` type

In `kernel/src/sched/scheduler.rs`, replace `Scheduler` with:

```rust
/// Per-CPU scheduler state. One instance per CPU, heap-allocated at AP
/// bring-up (or at BSP scheduler init), pointer stored in
/// `PER_CPU[cpu_id].scheduler_ptr`. Never moved after construction.
pub struct PerCpuScheduler {
    pub cpu_id: u32,
    /// Lock protects current/ready/zombies. Per ADR-013 §Q3, no global
    /// scheduler lock; per-CPU only.
    inner: Mutex<PerCpuSchedulerInner>,
    /// Lock-free zombie count. Same fast-path pattern as old global
    /// `ZOMBIE_COUNT`, now per-CPU.
    zombie_count: AtomicUsize,
}

struct PerCpuSchedulerInner {
    current: Box<Thread>,
    ready: VecDeque<Box<Thread>>,
    zombies: VecDeque<Box<Thread>>,
}
```

The bootstrap thread for each CPU is constructed by `Thread::adopt_current(name)` where name is `"main"` for the BSP and `"ap-N-main"` for AP cpu_id N. (Concrete name format: see what `Thread::adopt_current` accepts; it takes `&'static str` so the AP names need static storage. Easiest: a small `match cpu_id { 0 => "main", 1 => "ap-1-main", 2 => "ap-2-main", 3 => "ap-3-main", _ => "ap-main" }` helper. Phase 5+ will need real per-CPU allocation; this is fine for sub-commit 2.)

### 4.2 The new init API

Replace:

```rust
pub fn init() { ... }
```

with:

```rust
/// Initialize the BSP's per-CPU scheduler. Programs the PIT and unmasks IRQ0.
/// Must be called exactly once on the BSP, after `mm::init()` and after
/// `arch::x86_64::smp::bring_up_aps()` (so APs are already running their
/// own schedulers via `run_ap_bootstrap`).
pub fn init_bsp() { ... }

/// Initialize an AP's per-CPU scheduler and start its bootstrap thread.
/// Called from `arch::x86_64::smp::ap_entry` as the AP's terminal step.
/// Never returns — the bootstrap thread runs the AP's main loop forever.
pub fn run_ap_bootstrap(cpu_id: u32) -> ! { ... }

/// Compatibility shim — alias for `init_bsp`. Existing callers in
/// `kernel/src/main.rs` may still call `sched::init()`. Decide:
/// - keep the shim for blast-radius safety, or
/// - update `main.rs` and remove the shim.
/// PM recommends removing the shim — the shim earned its keep in 1a/1b
/// when the surface was unstable; sub-commit 2 is the moment to clean up.
```

`run_ap_bootstrap` does:
1. Construct the AP's `PerCpuScheduler` on the heap, including the AP's bootstrap Thread (`Thread::adopt_current("ap-N-main")`).
2. Store the pointer in `PER_CPU[cpu_id].scheduler_ptr` via `AtomicPtr::store(Ordering::Release)`.
3. Start the AP's per-CPU LAPIC timer (or mask local timer for sub-commit 2 if APs don't yet need preemption — see §4.6 below).
4. Enter the bootstrap thread's main loop: a `loop { reap_local(); yield_now(); }` — same shape as BSP's main loop, except the AP has nothing else to do; just reap.

The "AP runs the reaper loop forever" is fine for sub-commit 2 because APs have no user processes scheduled to them (cross-CPU IPC is EAGAIN'd, so no thread migrates, and `push_ready` from BSP doesn't yet route to APs). APs effectively idle. Sub-commits 3+ will route real work.

### 4.3 The current-CPU accessor

Add to `scheduler.rs`:

```rust
/// Get a reference to the current CPU's scheduler. Reads
/// `PER_CPU[current_cpu_id()].scheduler_ptr` via Acquire and dereferences.
/// PANICS if the scheduler hasn't been initialized for this CPU yet —
/// this should never happen in steady state because every CPU runs
/// `init_bsp` (BSP) or `run_ap_bootstrap` (AP) before any other code calls
/// scheduler functions.
fn current_scheduler() -> &'static PerCpuScheduler {
    let cpu_id = per_cpu::current_cpu_id();
    let ptr = PER_CPU[cpu_id as usize].scheduler_ptr.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "current_scheduler: cpu_id={} not initialized", cpu_id);
    // SAFETY: pointer is set exactly once per CPU during init and lives
    // for kernel lifetime (heap-allocated, intentionally leaked via
    // Box::into_raw, never freed). The lifetime of the returned reference
    // is bounded by the kernel's lifetime, hence 'static.
    unsafe { &*ptr }
}
```

Every existing function that did `SCHED.lock()` becomes `current_scheduler().inner.lock()`. The functions are:

- `spawn` / `push_ready` — push to current CPU's queue (sub-commit 2; sub-commit 3+ may route to the loaded least CPU but that's deferred).
- `rotate` — uses current CPU's scheduler.
- `yield_now` — current CPU.
- `on_tick` — current CPU.
- `exit` — current CPU.
- `current_pid` — current CPU.
- `block_current_into` — current CPU.
- `wake_thread` / `wake_thread_front` — sub-commit 2 routes to *current CPU*. This is consistent with the §Q4 wake-on-completer-CPU pattern even though sub-commit 2 doesn't exercise cross-CPU IPC (the rendezvous-completer is always on the current CPU because cross-CPU IPC is EAGAIN'd).
- `kill_thread_by_pid` — sub-commit 2 only handles ready-queue victims on the current CPU. Cross-CPU ready-queue search is sub-commit 5.
- `reap` — becomes `reap_local`, drains the current CPU's zombie queue only.

### 4.4 Q5 deletion — exact list of edits

**`kernel/src/sched/thread.rs`:**
- Delete the `preempt_defer_ticks: u8` field from `Thread` (line 101).
- Delete the three initializers (lines 173, 191, 279).

**`kernel/src/sched/scheduler.rs`:**
- Delete `const HANDOFF_PREEMPT_DEFER_TICKS: u8 = 32;` (line 60).
- Delete `pub fn clear_current_preempt_defer` (lines ~232–237).
- Delete `pub fn defer_current_preempt_for_handoff` (lines ~239–243).
- Delete `fn consume_preempt_defer` (lines ~245–253).
- In `on_tick`, remove the `if consume_preempt_defer() { return; }` line. Body becomes just `yield_now();`.
- In `prepare_woken_thread`, remove the `thread.preempt_defer_ticks = 0;` line.
- In `wake_thread_front`, remove the `thread.preempt_defer_ticks = HANDOFF_PREEMPT_DEFER_TICKS;` assignment and the explanatory comment about it.
- In `block_current_into`, remove the `prev.preempt_defer_ticks = 0;` line. (Three things to remove total in scheduler.rs, plus the constant and three function deletions.)

**`kernel/src/ipc/mod.rs`:**
- In `continue_current_after_receive` (lines 463–468), remove the `sched::defer_current_preempt_for_handoff();` call. The function body becomes just `sched::wake_thread_front(thread, return_value);` plus the explanatory comment, which gets updated.

**`kernel/src/arch/x86_64/syscall.rs`:**
- Remove the call to `clear_current_preempt_defer()` at the syscall entry. Surface where exactly it is in your OBSERVE pass and delete that line. There should be exactly one such call.

**Update comments where needed.** The `wake_thread_front` doc-comment names "the documented single-CPU handoff invariant" and "deferred quantum"; that paragraph should be replaced with one that explains the new semantics (front-queue + per-CPU IF=0 discipline preserves consumer-first; no deferred quantum needed under SMP).

### 4.5 Per-CPU reapers (Q9)

The current reaper:
- `reap()` is called from BSP's bootstrap "main" thread loop.
- Drains global `s.zombies`.
- Calls `Thread::free(zombie)` and `process::on_thread_reap(pid)` for each.

The new shape:
- Each CPU's bootstrap thread runs `reap_local()` in its main loop.
- `reap_local()` accesses `current_scheduler().inner.zombies` and `.zombie_count`.
- Drain pattern unchanged otherwise.

The `on_thread_reap` change is in `kernel/src/sched/process.rs`:

```rust
// In Process struct:
pub live_thread_count: AtomicU32,

// In Process::new_kernel and Process::new_user:
live_thread_count: AtomicU32::new(1),

// New helper, called wherever a Thread is constructed for an existing
// process pid (today: only spawn_user, which constructs Process+Thread
// together so the initial count of 1 is correct; sub-commit 2 doesn't
// add multi-thread-per-process spawn paths, so this helper exists for
// future sub-commits — surface its presence in your close report).
pub fn proc_on_thread_spawn(pid: ProcessId) {
    if pid == KERNEL_PID { return; }
    if let Some(p) = get(pid) {
        p.live_thread_count.fetch_add(1, Ordering::AcqRel);
    }
}

// Updated on_thread_reap:
pub fn on_thread_reap(pid: ProcessId) {
    if pid == KERNEL_PID { return; }

    let proc_arc = match without_interrupts(|| PROCESSES.lock().get(&pid).cloned()) {
        Some(p) => p,
        None => return,
    };

    // Decrement live_thread_count. If post-decrement is 0, this thread
    // was the last; proceed with deregistration. The double-check under
    // the registry lock handles the (rare) race where another thread is
    // spawned between decrement and lock acquisition.
    let prev_count = proc_arc.live_thread_count.fetch_sub(1, Ordering::AcqRel);
    if prev_count != 1 {
        return; // not the last thread; another reaper or this CPU later finishes
    }

    let exit_code = proc_arc.exit_code.load(Ordering::Acquire);
    let drained = without_interrupts(|| proc_arc.handles.lock().drain());
    close_channel_peers(&drained);

    without_interrupts(|| {
        let mut processes = PROCESSES.lock();
        // Double-check under the lock that count is still 0 (no concurrent
        // spawn raced in). Under sub-commit 2's mutex this is somewhat
        // redundant since fetch_sub already serialized, but it makes the
        // RwLock migration in sub-commit 5 simpler.
        if proc_arc.live_thread_count.load(Ordering::Acquire) == 0 {
            processes.remove(&pid);
        }
    });

    if exit_code != EXIT_CODE_RUNNING {
        without_interrupts(|| {
            COMPLETED_EXIT_CODES.lock().insert(pid, exit_code);
        });
    }

    let name = proc_arc.name;
    let pid_for_log = pid.0;
    drop(drained);
    drop(proc_arc);

    serial_println!(
        "[ kernel ] proc: deregistered '{}' (pid={})",
        name,
        pid_for_log
    );
    crate::mm::frame_audit("post-natural-exit");
}
```

Note: Phase 1.5b has at most one thread per user process, so `live_thread_count` is always 1 when `on_thread_reap` is called. The change is forward-compatible (multi-thread-per-process arrives in Phase 5+) without behavioral change today. The `terminate` path in `process.rs` does NOT need the same restructure — it's a single-thread-per-process forced kill, and the existing structure is fine. (Surface in your close report if your OBSERVE pass finds this assertion wrong.)

### 4.6 AP timer setup — explicit anti-scope choice

APs need preemption to run multi-thread workloads. Sub-commit 2 has two choices:

**Option A — APs do not enable preemption.** AP local APIC timer remains masked. AP runs only its bootstrap thread (which calls `reap_local + yield_now` in a loop). Since `yield_now` no-ops when the ready queue is empty, AP idles efficiently. Cross-CPU IPC is EAGAIN'd in sub-commit 2, so no thread is ever queued on an AP — APs have nothing to preempt to. **This is the correct choice for sub-commit 2.**

**Option B — APs enable LAPIC timer.** Each AP programs its local APIC timer at TIMER_HZ. Adds infrastructure (LAPIC timer registers, calibration, IDT vector for AP timer interrupt). Defer to sub-commit 4 when IPI infrastructure is being added anyway.

**Choose Option A.** Sub-commit 2's AP main loop is `loop { reap_local(); yield_now(); }` — and `yield_now` on an empty ready queue (always, in sub-commit 2) does nothing. The AP busy-loops harmlessly. If you want to be fancy, drop in a `hlt` instruction inside the empty-queue branch of `yield_now` — but that's a §Q3 optimization, not sub-commit 2 scope.

### 4.7 New test scenarios

Two new scenarios in `tools/xtask/src/main.rs`:

```
TestScenario {
    name: "smp_per_cpu_scheduler_smoke",
    feature: "neo-test-smp-per-cpu-scheduler-smoke",
    user_feature: None,
    ignored: false,
    limine_kaslr: None,
},
TestScenario {
    name: "smp_same_cpu_ipc",
    feature: "neo-test-smp-same-cpu-ipc",
    user_feature: None,
    ignored: false,
    limine_kaslr: None,
},
```

`smp_per_cpu_scheduler_smoke`: under `NEO_SMP=4`, the BSP spawns 4 kernel threads (`hog0`, `hog1`, `hog2`, `hog3`); after a fixed number of ticks each thread reports completion via a shared atomic counter; test passes when counter reaches 4. Under sub-commit 2's policy (no work routes to APs), all 4 threads run on BSP — that's still a valid test of per-CPU scheduling shape because the BSP's scheduler is also a per-CPU scheduler. The AP existence is verified by the boot log already emitting the AP-ready lines from sub-commit 1b.

`smp_same_cpu_ipc`: under `NEO_SMP=4`, an IPC ping-pong is performed between two threads, both on the BSP (same-CPU IPC). Verifies that IPC continues to work with the §Q5 mechanism deleted. Essentially a copy of `ipc_heap_kill_smoke`'s IPC-only scenario sans kill, kept as a separate scenario so the test runner verifies "same-CPU IPC works under SMP" as a property.

The actual test bodies live in `kernel/src/test_harness/` (or wherever the existing scenarios are; check the existing `ipc_heap_kill_smoke` body for the pattern). Construct them by analogy.

### 4.8 What you do NOT touch

- `kernel/src/sched/scheduler.rs::block_current_into`'s lock acquisition pattern. Same lock-order inversion as before; sub-commit 3a fixes it. Leave the existing comment.
- `kernel/src/sched/process.rs::PROCESSES` — stays `Mutex<BTreeMap<...>>`. Sub-commit 5 makes it `RwLock`.
- `kernel/src/sched/process.rs::terminate` — already documented above; minimal change.
- `kernel/src/sched/process.rs::kill_thread_by_pid`'s blocked-walk limitation — stays. Sub-commit 5.
- IPC body of `send`/`recv`/`grant`/`recv_grant`. Only `continue_current_after_receive`'s body changes per Q5.
- `kernel/src/arch/x86_64/smp.rs::ap_entry`'s structure. Only the final `halt_forever()` becomes `sched::run_ap_bootstrap(cpu_id)`.

---

## 5. Verification gates

All gates are PM-verifiable from your output. Run them; paste literal output; do not paraphrase.

### 5.1 Build green

```
docker compose run --rm neo cargo k
```

Expected: green build. **Warning count: 18 ± delta**, with each delta line-explained.

Likely deltas:
- `Thread.preempt_defer_ticks` field deletion may retire a "field is never read" warning if one existed.
- New `PerCpuScheduler` struct, new functions, etc. may introduce new dead-code or unused-variable warnings — surface and either remove the dead code or add `#[allow(dead_code)]` with a comment explaining what's load-bearing.

### 5.2 Single-CPU boot anti-regression

```
NEO_SMP=1 docker compose run --rm neo cargo neo-tty
```

Expected: identical-shape output to sub-commit 1b's `NEO_SMP=1` boot log. Specifically:

- The existing per_cpu / gdt / paging / smp lines are unchanged.
- New line: `[ kernel ] sched: BSP per-CPU scheduler ready (cpu=0, ready_queue empty)` (or similar — the message text is at your discretion, just make it informative).
- `[ kernel ] frame: leak audit clean (baseline=N final=N)`.
- `[ kernel ] phase 0 boot complete`.
- No `BUG:`. No `KERNEL PANIC`.

### 5.3 Multi-CPU boot

```
NEO_SMP=4 docker compose run --rm neo cargo neo-tty
```

Expected: BSP output as above, PLUS for each AP:
- `[ kernel ] sched: AP per-CPU scheduler ready (cpu=N, bootstrap='ap-N-main')` (or similar).
- The AP serial output should show its scheduler init lines after the existing AP-ready line from sub-commit 1b.

Frame leak audit: still clean.

### 5.4 Test suite SMP=1

```
NEO_SMP=1 docker compose run --rm neo cargo neo-test
```

Expected: 23/23 PASS (sub-commit 1b baseline) + 2 new scenarios PASS = **25/25 PASS** + meta-FAIL `expect_pass_but_fail`. Total scenarios in default suite goes from 24 to 26 (with one ignored — `slab_channel_migration` — staying ignored).

### 5.5 Test suite SMP=4

```
NEO_SMP=4 docker compose run --rm neo cargo neo-test
```

Expected: same 25/25 PASS + meta-FAIL.

If any scenario PASSes on SMP=1 but FAILs on SMP=4, that's a sub-commit 2 bug. Likely candidates:

- `ipc_heap_kill_smoke` flaking — that's the §Q5 bet failing. Surface immediately; do not close.
- A `cap_*` scenario flaking — cap operations involve PROCESSES.lock; under SMP the same Mutex is contended cross-CPU, but Mutex correctness shouldn't depend on contention level. If this happens, surface; it likely points to a real bug in the per-CPU scheduler code.

### 5.6 The §Q5 100x flake-free gate

This is the closure gate. Run `ipc_heap_kill_smoke` 100x consecutively under `NEO_SMP=4`:

```
for ($i=1; $i -le 100; $i++) {
  Write-Output "=== Run $i ==="
  $env:NEO_SMP=4
  $r = docker compose run --rm neo cargo neo-test ipc_heap_kill_smoke 2>&1
  if ($r -match "PASS") { Write-Output "Run $i: PASS" }
  else { Write-Output "Run $i: FAIL"; Write-Output $r; break }
}
```

Adjust the script to whatever shape fits the harness — the point is 100 consecutive runs, capture each, and stop on the first non-PASS. Report the count of consecutive PASSes; "100/100 PASS" is the gate.

If you observe even one FAIL: **STOP, do not commit, surface to PM.** A flake in this gate means the §Q5 claim is wrong; PM may decide to re-introduce the deferred-quantum mechanism (§Q5's deferred option b). That decision is not yours.

### 5.7 Frame leak audit clean across all boots

Every boot above must end with `frame: leak audit clean (baseline=N final=N)` where baseline equals final. Variations in N across boots are fine (KASLR, AP init detail) as long as baseline == final on each boot.

### 5.8 No regression in 1b boot logs

The sub-commit 1b ADR-013 amendments (sub-commit 1b corrections — sequential cpu_id, AP CR0/CR4 setup) must remain accurate. If your sub-commit 2 work changes the AP entry path semantics in a way that affects those amendments, the amendment text needs updating. (Likely it doesn't — sub-commit 2 only replaces `halt_forever()` at the END of `ap_entry` — but verify in your OBSERVE pass.)

---

## 6. Commit shape

Single commit on `main`. Suggested commit message:

```
build-3 sub-commit 2: per-CPU schedulers + Q5 mechanism deletion

Per ADR-013 §Q3 (per-CPU schedulers), §Q5 (Invariants 2/3/4 deletion),
§Q9 (per-CPU reapers).

- Replace `static SCHED: Lazy<Mutex<Scheduler>>` with `PerCpuScheduler`
  per CPU, accessed via `PER_CPU[cpu_id].scheduler_ptr`. Each CPU has
  its own current/ready/zombies queues + scheduler lock. No global
  scheduler.
- APs run their bootstrap thread (`ap-N-main`) in a per-CPU reaper +
  yield loop via `sched::run_ap_bootstrap`. AP LAPIC timer remains
  masked in sub-commit 2; APs idle harmlessly because cross-CPU IPC
  still returns EAGAIN.
- Deleted: `HANDOFF_PREEMPT_DEFER_TICKS`, `Thread.preempt_defer_ticks`,
  `consume_preempt_defer`, `clear_current_preempt_defer`,
  `defer_current_preempt_for_handoff`. Three call sites updated:
  `on_tick` simplified to just `yield_now()`, `wake_thread_front`
  drops the deferred-quantum assignment, `continue_current_after_receive`
  in ipc/mod.rs drops its `defer_current_preempt_for_handoff` call.
- `Process` gains `live_thread_count: AtomicU32`. `on_thread_reap`
  decrements; only proceeds with deregister if post-decrement is 0
  with a double-check under the registry lock. Phase 1.5b's
  one-thread-per-process invariant means this always proceeds today;
  the change is forward-compatible.
- Two new scenarios: `smp_per_cpu_scheduler_smoke`, `smp_same_cpu_ipc`.
- The §Q5 closure gate: `ipc_heap_kill_smoke` runs 100x consecutively
  under NEO_SMP=4 with zero flakes. [include actual count: e.g.,
  "Verified: 100/100 PASS"].

Sub-commit 2 does NOT include cross-CPU IPC (sub-commit 3), the
ManuallyDrop+asm release in block_current_into (sub-commit 3a), TLB
shootdowns (sub-commit 4), or the PROCESSES RwLock migration
(sub-commit 5). Anti-scope per ADR-013.
```

---

## 7. Risks (read before implementing)

### 7.1 The §Q5 race claim being wrong

Already covered in §2 and §5.6. The 100x gate catches it. If wrong, stop.

### 7.2 Per-CPU scheduler init ordering

The order BSP-runs-`bring_up_aps` → APs-run-`run_ap_bootstrap` → BSP-runs-`init_bsp` has a subtle property: APs construct their schedulers concurrently with BSP returning from `bring_up_aps` (BSP doesn't wait for APs). The BSP then runs `init_bsp` and `smoke_test`. If `smoke_test` does anything cross-CPU (it shouldn't — sub-commit 2 anti-scope is no cross-CPU work), it could race AP scheduler init.

**Mitigation:** ensure `smoke_test` only touches BSP state. Verify by reading it (`Select-String -Path "kernel\src\sched\*.rs" -Pattern "smoke_test"`). If it touches AP state, surface in close report.

Alternative mitigation: insert a barrier in `bring_up_aps` that waits for each started AP to set its `scheduler_ptr` before returning. But that adds infrastructure not strictly needed in sub-commit 2; defer.

### 7.3 `current_scheduler()` panic if cpu_id == 0 before BSP init

`init_bsp` is called from `_kernel_start` *after* `bring_up_aps`. So between `bring_up_aps` returning and `init_bsp` being called, the BSP has no scheduler. Anything that tries to access `current_scheduler()` in that window panics.

Today's `_kernel_start` doesn't call any scheduler function in that window. Verify by reading `main.rs` and confirming the order of calls between `bring_up_aps()` and `init_bsp()`. If anything between them calls a scheduler function, surface and either reorder or pre-init the BSP scheduler before `bring_up_aps`.

### 7.4 The bootstrap thread's `cpu_id` field

`Thread::adopt_current("main")` constructs a Thread with no specific CPU affinity. Under per-CPU schedulers, each AP's `adopt_current("ap-N-main")` becomes that AP's bootstrap. Make sure the Thread struct doesn't have any "cpu_id this thread belongs to" field that needs initialization — and if Q4's anti-scope-mentioned `Thread.cpu_id` field was added in sub-commit 1a (it wasn't, but verify), it has to be set per AP.

If `Thread` does need a cpu_id for sub-commit 2 (it shouldn't — the only place that matters is the future Q4 cross-CPU rendezvous, which is sub-commit 3b), surface and propose a one-field-add for Q4 forward-compat.

### 7.5 Cross-CPU push/wake from `kill_thread_by_pid`

Existing `kill_thread_by_pid` walks the BSP's ready queue. Under per-CPU it walks the *current* CPU's ready queue. A target on another CPU's ready queue won't be found — it'll EAGAIN'd to the caller. That's fine for sub-commit 2; sub-commit 5 lifts it via Q8.c.

The single relevant test scenario is `cap_revoke_cross_process` — it kills the target's process. Under SMP=4 with target spawned on BSP, the target's thread sits on BSP's ready queue and BSP's `kill_thread_by_pid` finds it. Should still work; verify in §5.5.

If `cap_revoke_cross_process` flakes under SMP=4 because the target is somehow on an AP's queue (which shouldn't happen given sub-commit 2's "no cross-CPU thread movement" policy, but verify), that's a sub-commit 2 routing bug — `push_ready` is supposed to put threads on the current CPU only.

### 7.6 IPI / cross-CPU work introduced by accident

Sub-commit 2 anti-scope includes "no IPI". If your implementation accidentally calls a cross-CPU function (e.g., a hypothetical `wake_thread_on(cpu_id, thread)`), surface it. The current scheduler doesn't have any such function, but if you find yourself wanting to add one, that's a §Q4 sub-commit 3b concern.

---

## 8. If something blocks you

Same discipline as 1a/1b. If something here is wrong, **stop, surface to PM, do not push through**. Specifically:

- The §Q5 100x gate fails → stop, surface, PM decides.
- A pre-existing test scenario flakes under SMP=4 → stop, surface, investigate.
- The AP-then-BSP init ordering surfaces a bug → stop, surface, propose alternative.
- OBSERVE pass reveals contradictions with this dispatch's prose → flag in close report; deviate via ADR-013 amendment subsection.

The 1a Amendment 1 ("Implementation correction" subsection — static TSS/IST storage) and 1b sub-commit-1b-corrections amendment are the canonical patterns.

---

## 9. Why this matters

Sub-commit 2 is the actual SMP work. 1a built the per-CPU containers. 1b lit them up. 2 makes the kernel scheduler-aware of the per-CPU shape. Sub-commits 3+ add cross-CPU functionality on top.

The §Q5 deletion is the hardest part because it's an empirical bet. ADR-013 made the bet by removing the mechanism rather than repurposing it; sub-commit 2's 100x gate is what tests the bet. Get this right and everything downstream is incremental SMP feature work. Get it wrong and we re-introduce the deferred-quantum mechanism (with proper SMP semantics, presumably) before sub-commit 3.

PM's expectation: this dispatch closes within one or two coder sessions. The §Q5 gate runs in the background for hours if needed; that's fine — just keep the 100x ipc_heap_kill_smoke loop running and check it before close.

---

**End of dispatch.**
