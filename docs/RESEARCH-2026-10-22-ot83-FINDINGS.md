<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# OT-83: a ring-3 CPU exception kills the process, not the machine (FINDINGS)

**Date:** 2026-10-22 · **Seat:** S3 (Claude family) · **Branch:** `claude/ot83-ring3-exception-kill`
**Base:** `origin/main` @ `0c501085`
**Origin:** logged as OT-83 by the OT-61 C0.5 lane (PR #86). The PL asked why we don't fix it; this lane fixes
it. It is a normal PM merge.

## 0. Result

Before this change, every CPU exception handler did `dump(); halt_forever()` whatever privilege level the
exception came from. Only the ring-3 page fault was contained (OT-61 C1.1), and only in builds with
`browser-runtime`.

Any user program could halt the whole machine. It took one of these:
- `ud2`;
- an integer divide by zero;
- a privileged instruction;
- setting RFLAGS.TF;
- an unmasked floating-point error;
- in the default and product builds, a stray pointer, since the `#PF` containment was not compiled in.

Now:
- **A CPL-3 exception kills only the faulting process,** with exit code 128 + the signal Linux delivers for
  it. The kernel, scheduler and governance stay live, and the next process runs.
- **A CPL-0 exception halts exactly as before.**
- **The containment is unconditional:** it is no longer behind `browser-runtime`.

| Vector | Exit | Reachable from ring 3 here? | Proof |
|---|---|---|---|
| `#UD` | 132 (SIGILL) | yes (`ud2`) | `ring3_fault_ud` |
| `#DE` | 136 (SIGFPE) | yes (`div` by 0) | `ring3_fault_de` |
| `#GP` | 139 (SIGSEGV) | yes (`hlt` at CPL 3) | `ring3_fault_gp` |
| `#DB` | 133 (SIGTRAP) | yes (RFLAGS.TF via `popfq`) | `ring3_fault_db` |
| `#MF` | 136 (SIGFPE) | yes, once `CR0.NE=1` (§2) | `ring3_fault_mf` |
| `#XM` | 136 (SIGFPE) | yes on hardware (`CR4.OSXMMEXCPT=1`) | `ring3_fault_xm`, **ignored in the QEMU gate** (§3) + host KAT |
| `#SS` | 135 (SIGBUS) | yes on hardware (a non-canonical stack access) | `ring3_fault_ss`, **ignored in the QEMU gate** (§3) + host KAT |
| `#PF` | 139 (SIGSEGV) | yes | existing `threads_guard_fault` etc.; now also without `browser-runtime` |
| `#NP` | 135 (SIGBUS) | not in practice (every user descriptor is present) | host KAT |
| `#AC` | 135 (SIGBUS) | no: `CR0.AM=0` is forced at boot (CP-40) | host KAT |
| `#NM` | 136 (SIGFPE): a **Neo defensive fallback**, not Linux-derived (Linux repairs `#NM`, or dies with SIGSEGV; it never signals it) | no: `CR0.EM=0`, `CR0.TS` never set | host KAT |
| `#OF`, `#BR` | 139 (SIGSEGV) | no: `into` and `bound` are `#UD` in 64-bit mode | host KAT |

Control: `ring0_fault_halts` raises `#UD` in ring 0. It reaches `halt_kernel_fault` and never the kill path.

Left unrouted on purpose, because each is a kernel or hardware problem whichever ring was running: `#TS`
(a TSS defect), `#DF`, NMI, `#MC`, and `#BP` (which already returns).

## 1. What changed

- **`kernel/src/user_fault_logic.rs` (new, pure).** `route(vector, saved_cs)` returns `KillProcess { exit_code }`
  when `CS & 3 == 3`, otherwise `Halt`. It also holds the vector → signal table. It is host-tested by
  `tools/xtask/src/ot83_tests.rs` through `#[path]` against a hand-written table (5 tests: every vector at
  both user selectors, ring 0 and null CS, all 65,536 selector values, table completeness, and the C1.1 139
  unchanged).
- **`kernel/src/arch/x86_64/idt.rs`.**
  - `contain_user_fault(vector, frame, detail)` is the first call in the `#DE`, `#DB`, `#OF`, `#BR`, `#UD`,
    `#NM`, `#NP`, `#SS`, `#GP`, `#MF`, `#AC` and `#XM` handlers. In `#GP` it runs after the CP-47/48 test
    hook, which always diverges.
  - On CPL 3 it does `swapgs` (CP-44: before any per-CPU access), then `load_kernel_fp_control` (CP-41),
    then the kill.
  - `halt_kernel_fault(vector)` is the unchanged CPL-0 halt, with the control scenario's hook.
  - The `#PF` kill lost its `#[cfg(feature = "browser-runtime")]`.
- **`kernel/src/sched/process.rs`.** `fault_kill_current_user_exception(vector, rip, detail)` is the C1.1
  `fault_kill_current_user`, generalized to any vector and made unconditional. Only `fault_killing`, the
  futex purge and `wait_any` stay behind their features, because only `browser-runtime` creates second
  threads. `fault_kill_current_user(cr2, rip)` remains as the `#PF` wrapper, and `FAULT_KILL_EXIT_CODE` is
  still 139.
- **`kernel/src/arch/x86_64/mod.rs`.** `CR0.NE = 1` is set in `enable_sse`, which runs on the BSP and every AP
  (§2).
- **`userland/fault-probe` (new).** One binary per reachable vector, plus `fault-probe-ok`. xtask builds and
  stages them only for their scenario (`/modules/neo-fault-probe`, `/modules/neo-fault-probe-ok`).
- **Scenarios:** `ring3_fault_{ud,de,gp,db,mf}` and `ring0_fault_halts`. All are built **without**
  `browser-runtime`, so they prove the default feature set.

## 2. Finding: x87 errors were silently lost (`CR0.NE`)

The `#MF` probe first *survived*. It unmasked divide-by-zero in the x87 control word, divided by zero, and
`fwait` returned normally. The kernel never set `CR0.NE` (it sets EM, MP and AM only), so NE was whatever the
bootloader left. With NE=0 an unmasked x87 error is signalled on the legacy FERR# → IRQ13 path, which nothing
services, so the error vanished and the program carried on with a wrong result.

The kernel now sets `CR0.NE = 1`, the native mode Linux requires. After that the same probe dies with 136.
The kernel itself uses SSE, not waiting x87 instructions, and its FP paths are FXSAVE/XSAVE/FNINIT, none of
which wait; so NE=1 adds no ring-0 `#MF` source.

## 3. `#SS` and `#XM` under QEMU TCG

Both probes are correct for hardware. QEMU's TCG emulator, the only CPU the gate has, does not model either
exception:

| Probe | QEMU 8.2.2 (local) | QEMU 10.2.3 (CI's version, built locally from the v10.2.3 tag) | Real hardware (SDM) |
|---|---|---|---|
| `push` with RSP = `0x8000_0000_0000_0008` (writes a non-canonical address) | `#PF`, CR2 `0x8000000000000000` → killed 139 | `#GP(0)` → killed 139 | `#SS(0)` → 135 |
| `divss` 1.0 / 0.0 with MXCSR.ZM cleared | no exception; the probe survives (99) | no exception (99) | `#XM` → 136 |

So `ring3_fault_ss` and `ring3_fault_xm` are registered with `ignored: true` and stay runnable with
`--include-ignored` under KVM or on bare metal. They are kept out of the QEMU gate, where they could only
fail. Two things still hold for these vectors:
- Their **routing** is proven by the host KAT.
- Their **kill path** is the same `contain_user_fault` → `fault_kill_current_user_exception` path that the
  other five scenarios run. The `#SS` probe shows it directly: under both QEMUs the mis-vectored fault
  (`#PF`/`#GP`) still killed only the process, with 139, and the next process ran.

## 4. Pitfall-bank consultation (`docs/concurrency-pitfalls.md`)

This is a forced-termination path entered from exception context, on any CPU, at IF=0. For each class the
change touches:

| Class | Introduced / guarded / N-A | How | Discriminator |
|---|---|---|---|
| CP-44 (per-CPU read through the user GS) | **guarded** | `swapgs` is the first act on the CPL-3 branch, before any print or per-CPU read. The only earlier code is `syscall_msr_trap`, which swaps itself and diverges. | every `ring3_fault_*` scenario: the kill path's scheduler calls read per-CPU state, and a wrong GS would nest-fault into `#DF` |
| CP-46 (misaligned Rust from error-code vectors) | **guarded (existing)** | `#GP`, `#SS`, `#NP` and `#AC` enter through `set_error_code_entry!` (realigned); the others are no-error-code vectors | `ring3_fault_gp` |
| CP-40, CP-39 (AC/DF in handlers) | **guarded (existing)** | every vector's shim clears AC and DF | existing `exception_entry_flags` |
| CP-41 (ring-3 MXCSR/FCW following the kernel) | **guarded** | `load_kernel_fp_control` before the kill; FNINIT does not wait, so a pending x87 error cannot re-fault | `ring3_fault_mf`, `ring3_fault_xm`: the kernel and the next process run after an unmasked FP fault |
| CP-2, CP-3, CP-37, CP-64 (IF=0 spins, shootdown readiness) | **was EXPOSED (F1, §7) → guarded** | The kill's locks are the C1.1 set: futex, `wait_any`, admission `QUEUES`, per-CPU scheduler locks. It sends no shootdown and waits for no peer; the `Pmap` drop happens later in the reaper. The first version also waited on the log sink (`SINK.lock()` from the admission withdrawal trace's flush, `QUEUES` held, IF=0): that is F1, now CP-71. The kill now runs in a stage-only log section and the traces never flush. | `ring3_fault_*`; `admission_park_kill_sink_held` (+ control) |
| CP-30, CP-36 (owned frames across a kill, the Ready→Running window) | **N-A for the faulting thread; CP-36's gap inherited** | It was in ring 3, so its kernel stack holds only the exception frame. Siblings use the C1.1 scans, which are unchanged. **Limitation, kept stated:** the scans retire Ready and parked siblings only. A sibling *running* on another CPU (e.g. a register-only loop) is not retired by them and can outlive the fault kill until it next enters the kernel. Process anchors prevent a UAF; they do not prove prompt whole-process termination. The single-threaded probes do not test this. | `threads_guard_fault` (existing) |
| CP-63, CP-65, CP-66, CP-67 (kill-protected sections) | **unchanged** | The faulting thread cannot be in one, since those are kernel-mode states. Siblings in admission are cancelled (CP-66 `cancel_pid`); scans skip protected threads. | existing admission and IPC kill scenarios |
| Fault on any CPU | **guarded** | per-CPU state is reached only after `swapgs`; the kill uses the current CPU's scheduler | scenarios run on whichever CPU the probe lands on |
| Nested fault inside the kill path | **fail-loud** | That fault is at CPL 0, so it routes to the halt and never recurses into a kill | `ring0_fault_halts` (the CPL-0 routing) |

New entries: **CP-70** (a ring-3-reachable vector that halts the machine), added to the bank with the guard and
discriminators above (CP-69 had just been taken by #90); and **CP-71** (a log flush that waits on the sink
lock at IF=0 or under `QUEUES`), from F1 (§7).

## 5. Evidence

- **Local QEMU 8.2.2, rebased tree** (`origin/main` @ `0c501085`). Every scenario printed the line below:

  | Scenario | Result |
  |---|---|
  | `ring3_fault_ud` | process_killed_exit=132, second_process_exit=0 |
  | `ring3_fault_de` | 136, then 0 |
  | `ring3_fault_gp` | 139, then 0 |
  | `ring3_fault_db` | 133, then 0 |
  | `ring3_fault_mf` | 136, then 0 (it failed with 99 before `CR0.NE`) |
  | `ring0_fault_halts` | reached=halt_kernel_fault, kill_path=not_taken |

  Serial excerpt (`#UD`):
  ```
  [ fault-probe-ud ] raising #UD in ring 3
  [ kernel ] proc: ring-3 #UD pid=1 rip=0x0000000000400015 detail=0x0 — fault-killing process, exit 132 (I-14b)
  [ kernel ] proc: deregistered 'fault-probe' (pid=1)
  [ ring3-fault ] fault-probe exit code 132
  [ fault-probe-ok ] second process ran after the fault-kill
  NEO_TEST/1 event=PASS name=ring3_fault_ud ring3_fault=#UD process_killed_exit=132 kernel_alive=true second_process_exit=0 opens=ot83
  ```
- **Host KAT:** `cargo test -p xtask ot83`, 5/5 pass.
- **4-shard `workflow_dispatch` gate on `92bf59a5`: GREEN.** Run
  [39763749653](https://github.com/SecondStageTurbine/LogansOS/actions/runs/39763749653) passed all 4 QEMU
  shards plus host, SPDX and feature-gated coverage. It includes `ring3_fault_{ud,de,gp,db,mf}` and
  `ring0_fault_halts` on CI's QEMU 10.2.3. That head is `origin/main` @ `4e9ea28c` merged in; the
  close-gate passes on it.
- **Push-CI:** green on `f06dfd45` (run 39705896422), after one re-run of two jobs that failed on a
  crates.io download.

## 6. Notes

- **No kill-switch.** This is a containment fix. The disabled state is the bug (the machine halts), so there
  is no previous layer to preserve. The default kernel changes on purpose; everything else in the diff is
  test-only.
- **OPEN-THREADS.** The OT-83 row lives on PR #86's branch, not on `main`. Once both PRs have merged, the PM
  should mark it RESOLVED with this lane's evidence.

## 7. F1 (Jarvis review): the cancellation trace waited on the log sink at IF=0

**The finding.** The exception kill calls `admission::cancel_pid` at IF=0. With a matching pending request,
`trace_withdrawn` ran with `QUEUES` held and called `log::flush()`, which takes `SINK` with a blocking spin. If
a kernel logger holding `SINK` (taken at IF=1 by `block_sink::write_fmt`) was preempted on the same CPU, the
kill spun forever: the holder can never run there again. A peer shootdown targeting that CPU then times out
(CP-64).

**The fix.**
- `log::block_sink` has a per-CPU **stage-only section** (`enter_stage_only` / `leave_stage_only`, nestable).
  Inside it a line is staged under `try_lock` of both sink locks, or dropped and counted
  (`DROPPED_CONTENDED_LOGS`). It never waits, never flushes, and does no block I/O. When staging is full the
  line is dropped, not flushed. It sets `FLUSH_PENDING`.
- `log::flush_pending()` is the durable write for normal context. It takes `SINK` and the staging lock only
  by `try_lock`; if either is held, the flag stays set for the next call.
- `fault_kill_current_user_exception` runs from its first print through the sibling scans inside a stage-only
  section, which ends just before `scheduler::exit()`. So the fault print, the withdrawal traces and anything
  else it logs never wait or do I/O. The suffix after the section (`scheduler::exit()`) was inspected in the
  re-check and logs nothing that waits.
- Every admission trace (`ADMITTED`, `DEFERRED`, `WITHDRAWN`) is now stage-only on every path, so no trace
  flushes while `QUEUES` is held. `submit`, `submit_and_park` and `complete` call `flush_pending()` once
  `QUEUES` is released. The reaper calls it after each reap. `cancel_pid` itself never flushes.
- The CP-66 order is unchanged: `QUEUES`, then the scheduler wake with `ECANCELED`.

**Wording qualified.** The general `block_sink::write_fmt` path (outside a stage-only section) still flushes
inline when its 4 KiB staging fills (`append` → `flush_locked`, `block_sink.rs` ~212). That is block I/O, and
it can run at IF=0 (e.g. from `sys_write`). It is pre-existing and not on the exception-kill path any more.
"Allocation-free, no I/O" in this lane's docs means the kill path, inside its stage-only section; it is not a
property of every logging call.

**Discriminator: `admission_park_kill_sink_held`** (feature `neo-test-admission-park-kill-sink-held`, which
implies `-threaded`).
- A kernel thread on CPU 0 takes `SINK` and holds it across yields: a logger preempted mid-section.
- With it held, a sibling of P2 (whose caller is parked in admission) runs on CPU 0, masks interrupts, and
  calls `fault_kill_current_user_exception(#UD)`.
- Required: the kill progresses (P2's caller is cancelled at the boundary and P2 is reaped). Then, with
  `SINK` still held, a full `tlb_shootdown::shootdown` to every ready CPU must be ACKed (it panics on
  timeout). Then the holder is released. The rest of the threaded scenario follows.
- **Control** `neo-test-admission-park-kill-sink-held-control` restores the pre-fix trace (print, then the
  blocking `log::flush()`). It must hang.

**Local evidence** (QEMU 8.2.2, `NEO_SMP=4`):

| Run | Result |
|---|---|
| `admission_park_kill_sink_held` | PASS (12.5 s): `sink_held fault_sibling_exited=true shootdown=acked holder_released=true` |
| control | **TIMEOUT** at 180 s. The serial log's last line is P3's `WITHDRAWN — process torn down`: the kill wedged in the trace's flush, as F1 predicted. |
