<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: `arthur_memory_forgetting`'s 900 s hangs are a same-CPU deadlock on the write guard's judgement

**Seat:** S4. **Trigger:** gate run 40174433840 (#119), shard 3, job 121112629726.
`arthur_memory_forgetting` timed out at the full 900 s budget twice: attempt 1 ran 08:22:15–08:37:15 UTC
and attempt 2 ran 08:37:16–08:52:16. Attempt 3 passed in 18.6 s. The kernel `.text` was identical to main
24ab67a0, so this is main's code.

## Evidence

**The CI attempts left no serial log.** The job ended green after xtask's retry, so the dump-on-failure step
never ran, and the run uploaded no artifacts. The job log has only the two `TIMEOUT budget=900s` lines.

**Local reproduction.** I ran `NEO_SMP=4` on a 4-CPU host, with 4 busy-loop CPU hogs and a 150 s per-attempt
timeout (a local, uncommitted xtask edit; a pass takes about 20–45 s under the hog).
- Run 1 hung on attempts 1 and 2, and attempt 3 passed. That is the CI pattern.
- Run 17 hung on attempt 1; run 19 hung on attempts 1 and 2, like run 1 and CI.
- Runs 2, 11–16, 18 and 20 passed cleanly.
- So 3 hung runs in 12 (5 hung attempts in 17). The hang is real but timing-dependent.
  (Runs 3–10 were not run: the first loop was cut short after run 2.)

**Both hung attempts stop at the same logical step.** The scenario's `io-trace` feature logs every block request.
1. Arthur's first memory write (`ot43 … event=key_write_begin kind=fact writes=1`) reaches the block write guard.
2. Because this is the device's first write, the guard judges it (`GuardedDevice::arming` →
   `spin::Once::call_once(judge)`).
3. The judgement reads the disk through `GuardReader`: three 4 KiB reads (sectors 0, 2048, 131072), then 512-byte
   reads of the same sectors.

| attempt | last lines |
|---|---|
| hung 1 | `submit req_id=176 op=Read sector=0 … block_len=512`, `inline-poll req_id=176 outcome=hit`, then nothing |
| hung 2 | `submit req_id=177 op=Read sector=0 … block_len=512`, `inline-poll req_id=177 outcome=hit`, then nothing |
| pass 3 | the same reads complete, then `block: writes armed device=0 driver=virtio-blk marker=neofs …`, then the writes |

Neither hung attempt ever prints `writes armed`: in both, the judgement never finished. And in both, an earlier
request of the same judgement had parked (`inline-poll … outcome=miss … action=park`, then woken). The passing
attempt never parked there.

**The hung CPU, from a QEMU monitor dump.** Repro runs 17 and 19 hung the same way: the judgement was
unfinished and there was no `writes armed` line. All three of their hung attempts were dumped, and all three
show the same CPU 0 stack (below; each boot's KASLR slide derived from its own `_mm_pause` RIP). xtask's `NEO_QEMU_TIMEOUT_DUMP=1` captured `info registers
-a` and each CPU's stack.
- **All four vCPUs** were at RIP `0xfffffffff7efbcd2`, the `ret` after `pause` in the out-of-line
  `core::arch::x86_64::_mm_pause` of the debug kernel. The KASLR slide is `0x77d5f000`, derived from that
  `_mm_pause` and page-aligned.
- **CPUs 1–3** were idle APs in their busy-yield loop: `RFL=0x282` (IF=1), or, for one AP caught inside
  `yield_now`'s `without_interrupts`, IF=0 in `scheduler::rotate` from `ap_entry`. None ran a thread.
- **CPU 0** had `RFL=0x83`: **IF=0, spinning**. Its stack, symbolized:

```
spin::once::Once<write_guard_logic::Arming>::poll        (the spin on an unfinished Once)
  ← try_call_once_slow ← try_call_once ← call_once
  ← GuardedDevice::arming                                 drivers/block/mod.rs:168
  ← GuardedDevice::write_range_allowed ← write_allowed
  ← <GuardedDevice as BlockDevice>::write_block           drivers/block/mod.rs:292
  ← neofs::inode::write_device_block_with_retry ← write_bytes_at ← write_fs_block
  ← neofs::cache::drain_pass
```

So a NeoFS writeback drain, running at IF=0, spins forever on the guard's `Once` while the judgement it waits
for never completes. `drain_pass` calls `write_fs_block` with the cache lock released (it snapshots the dirty
block under the lock, then writes), so the IF=0 is the caller's syscall state, not a held lock. The judging
thread is not on CPUs 1–3, which sit idle, so it is on CPU 0: runnable or woken, but behind the IF=0 spinner
that never yields.

## Hypothesis

**A second caller spins at IF=0 on the unfinished judgement, on the judging thread's CPU.**

- `GuardedDevice::arming` was `self.arming.call_once(|| self.judge())`. A `spin::Once` whose initializer is
  running makes every other caller spin, without yielding, until it finishes.
- The judgement does block I/O, so the judging thread parks while a read is in flight, and it can be
  timer-preempted. NeoFS's own comment says its writeback runs on "the preemptible `neofs-writeback`
  kthread".
- `GuardedDevice::submit` checks `write_allowed`, and so calls `arming()`, before the virtio-blk I/O gate. The
  gate's acquire forces interrupts on (CP-2), but `arming()` runs at the caller's IF. An **fsync** syscall arrives
  at IF=0: `sys_fsync_impl` (`syscall.rs:6075`) calls `file.fsync()` without `with_interrupts_enabled`, unlike
  `sys_read`/`sys_write`, which OT-74 wrapped IF-on. `fsync_regular` then takes `entries` and runs `drain_all` →
  `drain_pass` → `write_fs_block` → `write_block` → `arming()`, all at IF=0. (An earlier draft of this NOTE said
  a write syscall; `sys_write` runs IF-on. The dumps' 2 KiB stack window ends at `drain_pass`, and the IF=0 way
  into `drain_pass` is `sys_fsync`.)
- So, when the judge (thread A, the writeback kthread or a syscall thread) parks or is preempted inside the
  judgement, an fsync syscall (thread B, IF=0) can take its CPU and spin in `call_once`. B never yields and is
  never preempted. A is runnable, or woken by its completion, but queued behind B on that CPU, so it never runs
  and the `Once` never completes. On one CPU that is permanent: the 900 s timeout.

This is **CP-37**: a non-IRQ-safe spin lock taken from both preemptible and IF=0 contexts, an unbreakable
same-CPU deadlock. A `spin::Once` with a preemptible initializer is such a lock. It is the case **CP-56** warns
about: OT-74 fixed the NeoFS cache lock for exactly this shape ("a timer preemption of the worker … left the
lock held while an IF=0 syscall spun on it — on one CPU, forever"), but the write guard's `Once`, added later
(#85/#88), sits one step further down the same path and was not audited. It is banked as
**CP-82** in `docs/concurrency-pitfalls.md`.

It fits the evidence:
- it only happens on the first write of a boot, at the judgement;
- it needs the judge to be descheduled mid-judgement, which is likelier under host load (more QEMU vCPU
  descheduling, longer I/O waits, more parks), hence "passes in 18 s, otherwise hangs for 900 s";
- the hung attempts are the ones whose judgement parked.

Silicon would see it too. It needs only a preemptible judge and an IF=0 fsync meeting on one CPU.

## Falsification test

1. **Mechanism (deterministic): the `block_write_guard` judge-contention leg (this fix's PR).**
   - A kthread starts judging disk 1, and a test hook holds it inside the judgement's first read for 20 ticks.
   - The scenario thread then asks for the same judgement with interrupts OFF, as an fsync syscall does.
   - A witness counts how the waiter waited.
   - With the fix, it must wait yielding with interrupts on (`waited_yielding=1 spun_if_off=0`).
   - The control `neo-test-write-guard-judge-spin-control` restores the pre-fix `call_once` wait, and must FAIL
     `judge_waiter_spun_with_interrupts_off`, or hang if the waiter and the judge share a CPU.
   - **If the control passes, the leg cannot see the mechanism.**
   - **Result (local, `NEO_SMP=4`).** With the fix: PASS, `judge_contention waiter=Some(true) waited_yielding=1
     spun_if_off=0`. The spin control hung on all 4 attempts (180 s each), every one right after
     `enqueued kthread 'guard-judge'`: the waiter spun at IF=0 on the parked judge's CPU, which is the CI hang
     made deterministic. The guard-off control `neo-test-block-write-guard-control` still FAILs
     `unmarked_disk_written`.
2. **Effect (statistical): the repro loop on the fix.** `arthur_memory_forgetting`, `NEO_SMP=4`, 4 host CPU hogs,
   150 s per attempt.
   - Main hung 3 runs in 12 here (5 attempts in 17).
   - **If the fix branch still times out with the judgement unfinished (no `writes armed` line), the hypothesis
     is wrong.** In that case the next step is the QEMU monitor dump (`NEO_QEMU_TIMEOUT_DUMP=1
     NEO_QEMU_MONITOR_PORT=…`), which captures every vCPU's registers and stack at the timeout.
   - A hang elsewhere, with `writes armed` printed, would be a different bug.
   - **Result.** The fix branch, same loop and host load: **0 hung runs in 12**, every run passing on its first
     attempt (38–60 s) and printing `writes armed`. Main: 3 hung runs in 12. At main's rate, 12 clean runs in a
     row would happen by chance about 3% of the time (0.75^12), so this supports the fix but is not proof by
     itself; the deterministic leg above is the stronger test.

## Fix

`GuardedDevice::arming`:
- The first caller wins a `judging` flag, and only it enters `call_once`.
- Every other caller waits on `arming.get()` inside `with_interrupts_enabled`, yielding each round. That is the
  virtio-blk I/O gate's discipline (CP-2). A waiter can no longer hold the judge's CPU.
- The judgement itself is unchanged, so a device is still judged once, by its first writer, with the same
  reads and the same result.

## Lock audit (PM question on #120)

Turning interrupts on in `wait_for_judgement`: which locks can be held at that point? The full table is on PR #120
(issue comment 6396227883). In short:
- No path reaches `arming()` holding an `IrqSafeMutex` or any lock an ISR takes. The cache's `IrqSafeMutex` is
  released before `write_fs_block`.
- The block sink judges its device in `install()` with no lock held, before `SINK` is set.
- The NeoFS `SwitchMutex`es (`entries`, `metadata`, the inode locks) can be held. On virtio, the same chains
  already turn IF on, under the same locks, in the `io_gate` acquire and the completion park, so this adds
  nothing there.
- On AHCI/NVMe (synchronous polling) the window is new, but the hazard it could expose already exists on main:
  `sys_write` holds `entries` IF-on and preemptible, while `sys_fsync`/`sys_open`/`sys_readdir` take it at IF=0.
- **Follow-up:** run the NeoFS bodies of `sys_fsync`, `sys_open` and `sys_readdir` IF-on, as OT-74 did for
  read/write.
