<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# NOTE: tearing down a process whose thread token-parked corrupts memory (pre-existing; found by OT-22)

**Status:** ROOT-CAUSED and fixed as **CP-60** in PR #59 (`claude/token-park-teardown-fix`), and ported
into #58. `process::KERNEL_CR3` was `Lazy::new(Cr3::read)`. Its first read happened at the first switch from
a user-process thread to a kernel thread, so that process's PML4 became "the kernel's". When the process died,
the live page table was freed. The token park was only how the repro reached that switch first. Everything
below is the hunt as it happened, kept for the record.

**Original status:** open, unfixed. **Found by:** the OT-22 rung-1 `wait_any_core` scenario (branch
`claude/ot22-multiplex`). **Not caused by `wait_any`:** it reproduces with the bare scheduler token park.
**Severity:** high once anything kills token-parked threads. The browser-runtime futex already does, and
OT-22's `desktop-park` makes desktop clients token-park too.

## Symptom

`wait_any_core` (NEO_SMP=4) intermittently triple-faults (QEMU exits 0 under `-no-reboot`), or the kernel
reports `DOUBLE_FAULT`. It always happens shortly after a test process is torn down (`terminate` + `reap`)
and the next `Pmap::new` runs.

QEMU `-d int,cpu_reset` captured two shapes:

- A `#PF` with CR3 = `0x18100000`: the CPU was running on the previous test process's PML4 after that
  `Pmap` was dropped and its frame was handed out again. `Pmap::new` was zeroing that same frame, which was
  the live CR3.
- `Pmap::new` handed out `0x1fec8000`, the **kernel's own PML4**. The buddy allocator was corrupted.

The bug is timing-sensitive. Serial prints, a CR3 read per yield, or even a lock-free trace ring around CR3
writes and Pmap alloc/free made it vanish for 9 to 15 runs in a row. Without instrumentation it failed on
every attempt in one batch.

## Narrowing (all NEO_SMP=4, kernel feature `neo-test-wait-any-core`)

Each round: create a process (`Process::new_user` + `register`), run one
`Thread::new_kernel_for_process_test` thread on its Pmap, then `terminate` + `reap`.

| The thread before teardown | Result |
|---|---|
| yield-spins (`loop { yield_now() }`), killed from the ready queue | **clean**: 16 boots × 40 rounds |
| parks with `wait_any::sleep_ticks(1)` (token park, woken each tick) | **#DF / triple fault** on almost every boot |
| parks once with `block_current_with_token_and_guard` on a private `IrqSafeMutex`, never woken, zombified by `zombify_parked_thread_by_pid` | **#DF** on most boots |

In the full `wait_any_core`, the IPC leg's waiter token-parks, is woken, returns, and yield-spins. It is then
killed from the ready queue, and the crash follows. So having *ever* token-parked seems to be enough.

## Leads (unverified)

- Compare what `kill_thread_by_pid` and `zombify_parked_thread_by_pid` leave behind with what a
  token-parked-then-woken thread leaves behind:
  - token slot generation, which `take_token_thread_by_pid` does not bump;
  - the per-CPU `active_pmap_cr3` cache vs the hardware CR3 across `switch_context_with_release2`;
  - TSS.rsp0 / `kernel_rsp` left pointing at a reaped thread's stack (`cross_thread_kstack` is only called
    on cross-process switches).
- Check whether a reaped thread's kernel stack or Box can still be written after `Thread::free`, for
  example by a resume of a thread that was both zombified and readied.
- Possibly specific to kernel threads that carry a process Pmap (`new_kernel_for_process_test`). Check
  with a real user process that blocks in the futex or in `SYS_WAIT_ANY` and is then `sys_kill`ed.

## Repro

On `claude/ot22-multiplex`, replace the body of `run_wait_any_core` in `kernel/src/test_harness.rs` with:

```rust
static CHURN_TOKEN: crate::sync::IrqSafeMutex<Option<crate::sched::ThreadWakeToken>> =
    crate::sync::IrqSafeMutex::new(None);
extern "C" fn churn_park() -> ! {
    CHURN_RAN.store(true, Ordering::Release);
    crate::arch::x86_64::without_interrupts(|| {
        let _ = crate::sched::block_current_with_token_and_guard(CHURN_TOKEN.lock(), |s, t| *s = Some(t));
    });
    loop { crate::sched::yield_now(); }
}
// in run_wait_any_core:
for _ in 0..60 {
    let pid = wa_arm("churn");
    CHURN_RAN.store(false, Ordering::Release);
    crate::sched::scheduler::push_ready(
        crate::sched::thread::Thread::new_kernel_for_process_test("churn", pid, churn_park));
    wa_until("churn ran", || CHURN_RAN.load(Ordering::Acquire));
    for _ in 0..50 { crate::sched::yield_now(); }
    wa_retire(pid);
}
pass("churn_ok");
```

Run `LIMINE_DIR=… NEO_SMP=4 cargo xtask neo-test wait_any_core`. Expect `DOUBLE_FAULT` in the serial log, or
QEMU status 0. To capture the fault, add `-d int,cpu_reset -D <file>.%d.log` to the neo-test QEMU command.

## Impact on OT-22

- `wait_any_core` is not gate-reliable until this is fixed. Its legs pass when they survive, and the
  failure is memory corruption, not a logic verdict.
- `desktop_idle_parks` / `desktop_idle_spins` don't kill parked threads (Arthur and the WM exit naturally),
  and they pass repeatedly.
- `desktop-park` stays off in the product, as planned, so production is not exposed. But a `sys_kill` of a
  parked desktop client would take this path.
- Recommend a separate scheduler dispatch, with a cross-family lens, before OT-22 merges.
