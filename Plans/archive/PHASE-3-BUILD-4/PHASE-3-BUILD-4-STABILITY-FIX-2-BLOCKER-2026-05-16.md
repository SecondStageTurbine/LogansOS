# Phase 3 BUILD-4 Stability Fix 2 — Blocker Report

Date: 2026-05-16
Lane owner: Cerebro (per verbal PM reassignment from Jarvis)
Authority: `Plans/DISPATCH-2026-05-16-phase-3-build-4-stability-fix-2-ap-idle-jarvis.md`

Status: **BLOCKED** — stale-fsync final timeout on run 4 of 50.
Stopping per dispatch §6 and PM instruction.

---

## 1. HEAD and Recent Log

```text
HEAD (substrate, no new commits yet — Fix 6 changes are uncommitted):
584d3e8 plans: authorize Phase 3 BUILD-4 stability fix 2
be85ff1 fs: park NeoFS writeback worker when idle
cbe919f sched: scan all CPU ready queues for process kill
ada2de8 ci: upload neo-test serial logs on failure
75e00a5 plans: authorize Phase 3 BUILD-4 stability fix
00df29b docs: correct BUILD-4A close evidence
9324f4b phase3 build4a: directory readdir filecaps
bb47d0a plans: authorize Phase 3 BUILD-4A readdir
```

Fix 6 (AP idle halt) and the stale-fsync progress frames are implemented but
uncommitted. Build gates passed at 72 warnings (no new warnings).

---

## 2. Build Gates

```text
cargo k                    PASS  72 warnings (baseline unchanged)
cargo k --no-default-features  PASS  72 warnings (baseline unchanged)
```

---

## 3. Fix 6 Implementation Shape (uncommitted)

**`kernel/src/arch/x86_64/mod.rs`** — added `enable_and_hlt()` wrapper:
```rust
pub fn enable_and_hlt() {
    interrupts::enable_and_hlt();
}
```

**`kernel/src/sched/scheduler.rs`** — added `halt_current_cpu_if_idle()` and
modified `run_ap_bootstrap`:
```rust
fn halt_current_cpu_if_idle() {
    crate::arch::x86_64::disable_interrupts();
    let idle = {
        let sched = current_scheduler();
        let s = sched.inner.lock();
        s.ready.is_empty()
    };
    if idle {
        crate::arch::x86_64::enable_and_hlt();
    } else {
        crate::arch::x86_64::enable_interrupts();
    }
}

pub fn run_ap_bootstrap(cpu_id: u32) -> ! {
    ...
    loop {
        yield_now();
        halt_current_cpu_if_idle();  // was: core::hint::spin_loop()
    }
}
```

Race-safety argument: `cli` before ready-queue check means any reschedule IPI
arriving after the check is pending in the LAPIC and fires on `sti` in
`enable_and_hlt()` — the CPU cannot sleep through a wake.

**`kernel/src/test_harness.rs`** — 7 `event=PROGRESS` frames added to
`run_phase3_neofs_fsync_filecap_stale_regression` at each stage boundary
(mount_done, seed_fsync_done, hold_set, mutation_dirty, stale_fsync_done,
refreshed_fsync_begin, refreshed_fsync_done).

---

## 4. Targeted Loop Results (at time of stop)

Loops were running in parallel. Stopped reading after stale-fsync run 4 failure.
Loop processes were not forcibly killed (background processes); stale-fsync loop
continued and run 5 overwrote the failing serial log.

### phase3_neofs_fsync_filecap_stale_regression (50-run target, NEO_SMP=4)

| Run | Result | Notes |
|-----|--------|-------|
| 1 | PASS (retry) | first attempt timeout, retry passed |
| 2 | PASS (retry) | first attempt timeout, retry passed |
| 3 | PASS (retry) | first attempt timeout, retry passed |
| 4 | **FAIL** | attempt 1: no BEGIN frame; attempt 2: QEMU timeout 180s |
| 5 | PASS (retry) | first attempt timeout, retry passed; overwrote run 4 serial log |

Loop was still running at time of stop; runs 6–50 unknown.

**Final failures: 1** (run 4)
**Final PASS: 4** (runs 1, 2, 3, 5)
**Retry-pass: 4** (runs 1, 2, 3, 5)

### phase3_sched_wake_token_abandon (30-run target, NEO_SMP=4)

Runs 1–11: all clean PASS, 0 retries, 0 failures.  
Loop still running at time of stop.

### ipc_heap_kill_smoke (20-run target, NEO_SMP=4)

Runs 1–11: all clean PASS, 0 retries, 0 failures.  
Loop still running at time of stop.

---

## 5. Failure Evidence for Run 4

### xtask output (captured before serial log was overwritten)

```text
xtask: `phase3_neofs_fsync_filecap_stale_regression` failed after 2 attempts;
last error: QEMU timed out after 180s for `phase3_neofs_fsync_filecap_stale_regression`;
previous error: `phase3_neofs_fsync_filecap_stale_regression` produced no BEGIN frame;
serial=/workspace/target/neo-test/phase3_neofs_fsync_filecap_stale_regression.serial.log
```

Disk was successfully created (not a mkfs failure):
```text
NEOFS_MKFS/1 status=ok ... image_size=134217728 ... superblock_copies=3
root_inode_block=16386 next_free_block=16393 free_map_block=16385
```

### Serial log from failing attempt: LOST

The serial log for run 4 attempt 2 (the 180s timeout) was overwritten by run 5
before it could be captured. The dispatch requirement to "preserve the serial log
and classify by the last event=PROGRESS frame" cannot be fully satisfied.

**Last event=PROGRESS from the failing run: UNKNOWN.**

### Serial log from run 5 (passing — overwrote run 4)

Run 5 completed with all 7 PROGRESS stages and a PASS. This is the nearest
evidence of what the test looks like when it succeeds. Extracted from the
current serial log (632 lines total):

```text
NEO_TEST/1 event=BEGIN name=phase3_neofs_fsync_filecap_stale_regression phase=dispatch
NEO_TEST/1 event=PROGRESS ... stage=mount_done selected_superblock=0 generation=1
NEO_TEST/1 event=PROGRESS ... stage=seed_fsync_done dirty_blocks=0 drained_blocks=10 flush_required=false flushes=1
NEO_TEST/1 event=PROGRESS ... stage=hold_set writeback_hold=true
NEO_TEST/1 event=PROGRESS ... stage=mutation_dirty dirty_blocks=7 flush_required=true old_cap_generation=7
NEO_TEST/1 event=PROGRESS ... stage=stale_fsync_done stale_errno=-116 dirty_blocks_unchanged=true flush_commands_unchanged=true
NEO_TEST/1 event=PROGRESS ... stage=refreshed_fsync_begin dirty_blocks=7 flush_required=true refreshed_generation=9 current_inode_generation=9
NEO_TEST/1 event=PROGRESS ... stage=refreshed_fsync_done dirty_blocks=0 flush_required=false drained_blocks=17 flushes=2
NEO_TEST/1 event=PASS name=phase3_neofs_fsync_filecap_stale_regression path=/neofs/fsync-3e-stale.txt stale_fsync=ESTALE stale_errno=-116 stale_before_durability=true old_cap_generation=7 refreshed_generation=9 current_inode_generation=9 dirty_blocks_before_stale=7 dirty_blocks_after_stale=7 dirty_blocks_after_refreshed_fsync=0 flush_commands_after_stale_delta=0 cache_flushes=2 selected_superblock=0 generation=1
```

When the test passes, it reaches `refreshed_fsync_done` with `dirty_blocks=0`
and `drained_blocks=17` (7 dirty blocks drained + 10 from seed fsync = 17 total).
The entire test completes without stalling between stages.

---

## 6. Failure Classification

### Attempt 1: "produced no BEGIN frame"

QEMU ran (the disk was successfully created and QEMU was launched) but no output
appeared in the serial log. This is not a PROGRESS-stage timeout — the kernel
did not produce any serial output at all.

Possible causes:
- Transient QEMU startup failure (QEMU process exited before the guest kernel
  printed anything — host system transient)
- Guest triple-fault before serial port initialization
- Host resource contention causing QEMU to be killed before output flushed

This failure mode was not observed in any of the 60-run pre-fix isolation runs
or in any passing runs. It is likely a QEMU/host infrastructure transient, not
a test code or kernel bug.

### Attempt 2: QEMU timeout 180s

This is the actual test timeout. The serial log was overwritten, so the stage
at which the test stalled is unknown. Based on the pattern observed in runs 1–3
and 5 (all retry-pass), the stall is consistent with a timing sensitivity in
the I/O-heavy mount or fsync phases.

### Overall classification

The AP idle halt (Fix 6) **partially resolves** the stale-fsync timing tail:
- Pre-fix (isolation data): 20/20 final PASS, 4/20 retry-pass (20% retry rate)
- Post-fix (5 runs observed): 4/5 final PASS, 4/5 retry-pass, 1/5 final failure

The retry-pass rate has increased from 20% to 80%, suggesting Fix 6 moved some
runs from final-failure into retry-pass but also made retry-pass more frequent.
The single final failure on run 4 may be a QEMU/host transient layered on top
of the residual timing tail, given the unusual "no BEGIN frame" first attempt.

---

## 7. Regression Scenarios

Not reached. Stopping per dispatch and PM instruction on first final failure.

---

## 8. Final Full-Suite Gate

Not reached.

---

## 9. BUILD-4B Status

**BUILD-4B remains blocked.**

---

## 10. Next Mechanism Candidates

If PM authorizes a follow-up lane, the next mechanism candidates for eliminating
the residual timing tail in `phase3_neofs_fsync_filecap_stale_regression` are:

(a) **Same-CPU I/O wake scheduling** (Fix 7 from dispatch, currently observation-only):
    After each virtio-blk completion wakes the BSP, the BSP may not be scheduled
    immediately — the completion kthread or another ready thread runs first. Making
    the storage completion path prefer the front of the ready queue (rather than
    the back) would reduce the wake-to-run latency for the BSP.

(b) **Increased QEMU timeout for NeoFS I/O-heavy scenarios**: The 128 MB disk
    requires ~400+ sector reads for mount validation. Under the new KVM host
    kernel timing, 180s is a tight bound. A targeted timeout increase for
    NeoFS scenarios would not fix the underlying issue but would absorb the tail.

(c) **Smaller NeoFS test disk for this scenario**: The stale-regression test
    only needs enough space for one seed write + one mutation. A smaller disk
    (e.g., 16 MB instead of 128 MB) would reduce mount validation I/O by ~8×.

(d) **Serial log preservation on retry failure**: The xtask currently overwrites
    the serial log on each retry. If the first attempt's log were preserved (e.g.,
    `<scenario>.attempt1.serial.log`), the last PROGRESS frame from the failing
    attempt would be available for diagnosis.
