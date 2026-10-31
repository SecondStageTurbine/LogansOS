# Phase 3 BUILD-4 Stale-Fsync Spin-Localization Findings

Date: 2026-05-18
Agent: Cerebro
Status: LOCALIZATION COMPLETE — contended lock + owner + mechanism identified. No mechanism fix landed, per dispatch.
Authority: `Plans/DISPATCH-2026-05-18-stale-fsync-spin-livelock-localization-cerebro.md`

The commandment held: no fix until lock and owner are identified. They are.

## 1. Worktree state

- HEAD: `e2b5c6949005ff50ac543c65d2a710229871306b` (main)
- Uncommitted diff: 893 insertions across 11 files (carries forward Fix 8/9/11/12/14/15 + the prior liveness-instrumentation pass + this localization pass)

```
 kernel/src/arch/x86_64/idt.rs          |   8 +-
 kernel/src/drivers/block/virtio_blk.rs |  58 ++-
 kernel/src/fs/mod.rs                   |   5 +
 kernel/src/fs/neofs/cache.rs           | 150 +++-
 kernel/src/fs/neofs/mod.rs             |   2 +-
 kernel/src/sched/mod.rs                |   4 +-
 kernel/src/sched/scheduler.rs          |  53 +
 kernel/src/sync/mod.rs                 |   2 +
 kernel/src/sync/switch_mutex.rs        |  77 ++-
 kernel/src/test_harness.rs             |  51 +
 tools/xtask/src/main.rs                | 529 ++++-
```

## 2. ADR-020 owner metadata reuse

**Yes, reused as-is.** `LockOwner` in `kernel/src/sync/lock_owner.rs` already provides:

- `cpu`/`thread_id`/`acquisition_site` AtomicU32 fields
- Seqlock-protected `read_consistent()` returning `Option<LockOwnerSnapshot>`
- `kernel-diagnostic`-gated `acquire(cpu, thread, site)` / `release()` hooks

`SwitchMutex` already calls these on every acquire and on Drop/`release_token_for_context_switch`. This pass added only the missing **contention-target** side.

## 3. Instrumentation diff summary

### 3.1 New module: `kernel/src/sync/spin_snapshot.rs` (kernel-diagnostic-gated)

Per-CPU `SpinSnapshotSlot` array of size `MAX_CPUS` (64). Each slot has:

- `seq: AtomicU32` (seqlock — even=stable, odd=mid-write)
- `occupied: AtomicBool`
- `cpu`, `thread_id`, `lock_addr`, `requester_site`, `owner_cpu`, `owner_thread_id`, `owner_site`, `owner_torn`, `spin_iterations`, `acquired`

Public API:
- `record_spin_begin(cpu, thread_id, lock_addr, requester_site, owner: Option<LockOwnerSnapshot>)` — seqlock-protected write
- `record_spin_iteration_batch(cpu, batch)` — relaxed counter, called every 1024 pauses
- `record_spin_acquired(cpu)` — clears slot
- `read_snapshot(cpu) -> Option<SpinSnapshot>` — seqlock-tolerant, NMI-safe
- `dump_all_to_serial()` — walks slots 0..MAX_CPUS, emits `NEO_DIAG/1 event=SPIN_SNAPSHOT` frames

### 3.2 `kernel/src/sync/switch_mutex.rs`: contention slow-path

Split `lock()` into fast and slow paths. **Fast path** (uncontended) is byte-identical to prior behavior. **Slow path** (cmpxchg failed at least once):

1. Compute `requester_site = hash_site(Location::caller().file(), .line())` via the existing FNV-1a from `lock_owner`
2. Call `spin_snapshot::record_spin_begin(...)` with `self.owner.read_consistent()` captured at begin
3. Enter spin loop with batched iteration counter (every 1024 `spin_loop()` calls, flush to atomic)
4. On successful acquire: `record_spin_acquired(cpu)` (clears slot), then `record_owner_acquire(Location::caller())`

Also: improved `record_owner_acquire` to take `Location::caller()` instead of the previous const SITE which always pointed at switch_mutex.rs:99. Owner site hashes are now **per-call-site**, not always the same.

### 3.3 `kernel/src/arch/x86_64/idt.rs`: NMI dump

The NMI handler now, under `kernel-diagnostic`, calls `crate::sync::spin_snapshot::dump_all_to_serial()` before `halt_forever()`. Atomics-only; no locks taken; no allocation.

### 3.4 `tools/xtask/src/main.rs`: NMI injection on timeout

New env `NEO_QEMU_TIMEOUT_NMI=1`. When set alongside `NEO_QEMU_MONITOR_PORT`, the timeout path:

1. Opens TCP to QEMU monitor
2. Issues `nmi\n` (not `nmi 0\n` — first attempt used the wrong syntax and was a no-op; corrected)
3. Sleeps 400ms for guest to print SPIN_SNAPSHOT frames over serial
4. If `NEO_QEMU_TIMEOUT_DUMP=1` is also set, issues `info status`/`info cpus`/`info registers -a`
5. SIGKILLs QEMU

Failure-mode evidence is preserved: when `nmi 0` was rejected by QEMU, that text appeared in the dump file. Diagnostic-of-the-diagnostic worked.

## 4. Build gates

| Gate | Command | Result | Warnings |
|---|---|---|---|
| xtask | `cargo build -p xtask` | OK | 0 |
| kernel default | `cargo k` | OK | 72 (baseline preserved) |
| kernel no-default | `cargo k --no-default-features` | OK | 72 (baseline preserved) |

## 5. Smoke evidence

### 5.1 First smoke (`spinloc-smoke`) — broken `nmi 0` syntax

Both attempts hit final-timeout. NMI command rejected by QEMU: `nmi: extraneous characters at the end of line`. Zero `SPIN_SNAPSHOT` frames produced — the kernel handler never fired. Per-CPU register dump confirmed identical convergent-PAUSE pattern from the prior lane (all 4 CPUs at the same `__mm_pause` RIP, CPU0 IF=0).

Evidence preserved under `spinloc-smoke-bad-nmi.*` filenames.

### 5.2 Second smoke (`spinloc-smoke2`) — corrected `nmi\n`

Both attempts hit final-timeout (consistent with prior tail rate under instrumentation). NMI handler fired both times; SPIN_SNAPSHOT frames captured.

| Attempt | Last fsync frame before NMI | SPIN_SNAPSHOT (only occupied slot) |
|---|---|---|
| 1 | `FSYNC_WRITE_BEGIN seq=0 block=16394 role=root dirty=7 drained=1 submissions=419 completions=419 in_flight_dma=0` | `slot=0 cpu=0 thread_id=0 lock_addr=0xffffffff8da002f0 requester_site=0xab5648df owner_cpu=0 owner_thread_id=7 owner_site=0xfe467900 owner_torn=false spin_iterations=944838656 acquired=false` |
| 2 | `FSYNC_WRITE_BEGIN seq=0 block=16399 role=root dirty=4 drained=1 submissions=435 completions=435 in_flight_dma=0` | `slot=0 cpu=0 thread_id=0 lock_addr=0xffffffffd68002f0 requester_site=0xab5648df owner_cpu=0 owner_thread_id=7 owner_site=0xfe467900 owner_torn=false spin_iterations=952172544 acquired=false` |

`SPIN_SNAPSHOT_SUMMARY occupied_slots=1 max_cpus=64` in both — **only CPU0 has an active SwitchMutex spin slot.** The other 3 CPUs are not spinning on any SwitchMutex; they're elsewhere in the scheduler/idle paths (which use `spin_loop()` directly without going through SwitchMutex).

## 6. Site resolution

FNV-1a `hash_site(file, line)` reversed by brute force over all `.rs` files in `kernel/`. Each target hash had **exactly one match**:

| Hash | Resolved | Role |
|---|---|---|
| `0xab5648df` (requester_site) | `kernel/src/drivers/block/virtio_blk.rs:521` | spinner is here |
| `0xfe467900` (owner_site) | `kernel/src/drivers/block/virtio_blk.rs:538` | holder acquired here |

### Site code

**Line 521** (spinner — `submit_pending_and_wait`):
```rust
fn submit_pending_and_wait(&self, pending: PendingIo) -> BlockResult<()> {
    ensure_completion_worker();
    let ret = crate::arch::x86_64::without_interrupts(|| {
        let guard = self.inner.lock();              // ← line 521 (spinner)
        if guard.in_flight.is_some() { return BLOCK_RET_UNSUPPORTED; }
        sched::block_current_with_token_and_guard(guard, ...)
    });
    ...
}
```

**Line 538** (holder — `poll_completions`):
```rust
fn poll_completions(&self) {
    let completion = {
        let mut inner = self.inner.lock();          // ← line 538 (holder)
        inner.poll_used()
    };
    if let Some(completion) = completion {
        ... sched::wake_thread_token_front(...) ...
    }
}
```

### Lock identity

Same lock (`VirtioBlkDevice::inner: SwitchMutex<VirtioBlkInner>`) across both runs. KASLR-relative offset `0x2f0` is consistent across the two attempts' lock addresses (`0xffffffff8da002f0` and `0xffffffffd68002f0`).

## 7. Mechanism

**Same-CPU SwitchMutex deadlock between the virtio completion-poll path and the submitter path.**

Trace:

1. Completion worker kthread (thread_id=7) on CPU 0 takes `self.inner.lock()` at line 538. **No `without_interrupts` wrapper**, so IF=1.
2. Timer IRQ fires on CPU 0 while the worker is inside the critical section (between `inner.lock()` and the implicit drop at `};`). Lock byte is still held; `LockOwner` metadata still points to thread 7.
3. Scheduler preempts the worker mid-critical-section, picks the submitter thread (thread_id=0) to run on CPU 0.
4. Submitter calls `submit_pending_and_wait` → `without_interrupts(|| { ... })` → IF=0 → `self.inner.lock()` at line 521.
5. Lock is held by the preempted worker. Submitter spins. Spin loop runs ~2.6M iterations/sec.
6. Because IF=0 on CPU 0, no timer IRQ can fire. The worker can never be rescheduled. The lock can never be released.
7. Other CPUs continue executing whatever non-virtio threads they have; they aren't blocked on this lock, but they have no work that resolves the deadlock either.
8. virtio completions stop being polled. Submitter's wake never arrives. Whole stale-fsync drain stalls.

This pattern is consistent with **every prior observation** from the liveness-instrumentation lane:

- All 4 CPUs converged on the same `__mm_pause` RIP in debug builds — the spinner is at SwitchMutex's pause; the other 3 CPUs are at scheduler/idle's pause (same `__mm_pause` symbol).
- CPU0 IF=0 (RFL=0x00000002), other CPUs IF=1 — submitter under `without_interrupts`; others in normal kernel code.
- Stall is not block-specific — any drain iteration can hit the race window of "timer fires while worker holds the lock."
- Retry-pass on the second attempt — the race window opens or closes by chance on a clean QEMU boot.
- Observer effect not exhibited — the instrumentation reproduces the bug, doesn't mask it.

## 8. Classification (A-H) after localization

| Case | Definition | Final classification |
|---|---|---|
| A | Guest stopped executing | **NO** — VM running, NMI delivered, kernel printed snapshots |
| B | Virtio submitted, never completed | **NO** — submissions=completions on the WRITE_BEGIN immediately before the stall |
| C | Completed but waiter not woken | **YES (as effect)** — the wake never happens because the completion worker can't drain past `inner.lock()` (preempted with lock held) |
| D | Waiter woken but did not run | **NO** — wake was never invoked; the deadlock is upstream of `wake_thread_token_front` |
| E | Waiter ran but no fsync progress | **NO** |
| F | Steady but too slow | **NO** |
| G | Semantic invariant failure | **NO** |
| H | Insufficient instrumentation | **NO** — instrumentation pinpointed the lock and owner |

**Primary classification: Case C, with the specific mechanism being a same-CPU SwitchMutex deadlock between the timer-preempted completion-poll path (line 538) and the `without_interrupts`-wrapped submitter path (line 521).**

The "C-vs-D leaning" the dispatch asked for: **C**. The wake handoff never gets a chance to run because the completion worker is stuck holding the very lock the wake path indirectly needs (`poll_used` runs under `inner.lock()`).

## 9. Recommended mechanism fix lane

Two correct options; the choice is a discipline call that affects every SwitchMutex caller.

**Option A — Promote `virtio_blk.inner` to `IrqSafeMutex`.** Narrowly scoped: only the virtio_blk lock changes. The completion worker would then acquire with interrupts disabled at line 538, eliminating the timer-preempt window. The submitter's existing `without_interrupts` wrapper remains; both paths now hold the lock with IF=0 (so timer can't preempt the holder), and both paths release before any context switch (so the lock is never held across yield).

**Option B — Require `without_interrupts` around any `self.inner.lock()` in virtio_blk.** Narrower change but more fragile: relies on every caller remembering to wrap. Equivalent effect to A but enforced by convention rather than the lock type. Not recommended.

**Option C (broader, not for this lane)** — Audit every `SwitchMutex` use across the kernel for "held across yield/preempt" anti-pattern. The spin_snapshot machinery added in this lane is the right tool: a stress-test under SMP that exercises every lock would reveal the next instance. Out of scope here.

**Recommendation: Option A.** Single-file change (`kernel/src/drivers/block/virtio_blk.rs`: switch `SwitchMutex` import to `IrqSafeMutex`), localized blast radius, kernel-diagnostic substrate already proves the diagnosis. No fsync redesign, no scheduler redesign — surgical mechanism fix per the dispatch's constraint.

## 10. Stop-condition compliance

Dispatch §5 stop conditions:

- ✅ "first retry-pass or final-fail sample identifies a contended lock and owner" — both attempts of `spinloc-smoke2` identified the same lock + owner with `owner_torn=false`. Two samples converge identically.
- (Not triggered: NMI capture failed, instrumentation masked failure, semantic FAIL, 30-attempt cap.)

## 11. BUILD-4B status

**Remains blocked**, per dispatch §1 ("No mechanism fix is authorized until this lane produces a lock/site localization finding"). The localization is now complete; PM consumption of these findings is the next gate. The mechanism fix (Option A above) is the suggested next lane, also under no-commit-yet policy.

## 12. Preserved evidence

| Location | Contents |
|---|---|
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.spinloc-smoke-bad-nmi.*` | First smoke with broken `nmi 0` — proves NMI didn't reach guest; per-CPU register dumps still consistent with convergent-PAUSE |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.spinloc-smoke2.attempt{1,2}.serial.log` | Working SPIN_SNAPSHOT capture, both attempts |
| `target/neo-test/phase3_neofs_fsync_filecap_stale_regression.spinloc-smoke2.attempt{1,2}.serial.timeout-dump.log` | xtask-side monitor capture (nmi + info registers -a) |
| `Plans/preserved-evidence/2026-05-18-t7-diag-01/` | Prior lane's retry-pass evidence (kept intact) |

## 13. Commit policy reminder

No commit for localization-only instrumentation until PM consumes findings. No push. No merge. No BUILD-4B. No scheduler redesign. No fsync redesign. No mechanism fix.
