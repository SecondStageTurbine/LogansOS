<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: CP-44 page-fault `swapgs` cross-family review (Jarvis / Codex)

**Date:** 2026-10-17

**Reviewed commit:** `735bea1bd84380f987792e2c4d139151feb641b7`

**Verdict:** **MERGE.** No CP-44 correctness finding survived re-verification. The merged change closes the
reported `page_fault`/observability recursion. The already-documented syscall-exit and fatal-exception GS
holes remain real follow-up work, not regressions introduced by this commit.

## Findings

No findings.

## Notes (non-blocking)

1. `page_fault`'s `swapgs` is the first source-level body operation, but not literally the first emitted
   instruction. The `extern "x86-interrupt"` prologue saves registers/XMM state and the CS predicate calls
   small `InterruptStackFrame::deref` helpers before the swap. Disassembly confirmed that none of this reads
   GS or per-CPU state, and the swap still precedes `record_trace` and every scheduler call.
2. A future alignment repair must own RSP in naked/global assembly. Applying `and rsp, -16` from inline asm
   inside an ordinary compiler-generated `extern "x86-interrupt"` function would invalidate the compiler's
   frame assumptions. A dedicated assembly wrapper that aligns, calls a diverging `extern "C"` body, and
   never attempts the generated epilogue is sound.

## Requested checks

### 1. `page_fault` never returns

I walked every configuration-dependent continuation from
`kernel/src/arch/x86_64/idt.rs:556-627`.

- With `neo-test`, `handle_page_fault` either returns only for `NO_EXPECTATION` or diverges through
  `pass`, `fail`, `fail_with_hex`, `debug_exit`, the q7c handler, or the atrium handlers. After a normal
  return, control continues to either the browser kill or the fatal path.
- With `browser-runtime`, a fault with `USER_MODE` calls `fault_kill_current_user -> scheduler::exit`; both
  are `-> !` (`kernel/src/sched/process.rs:2681-2726`, `scheduler.rs:1289-1359`).
- Every remaining cfg combination reaches `unexpected_trap -> debug_exit` under `neo-test`, or
  `halt_forever` without it. Both diverge.

Therefore no current build reaches the generated `iretq` with the kernel GS installed. A future recoverable
page-fault path must add a matching exit swap, as the source comment states.

### 2. Predicate and nested-entry analysis

The saved-CS predicate is the correct key for GS state. A ring-3 instruction can fault on an implicit
supervisor access and produce PFEC.U/S=0 while saved CS.RPL remains 3; checking `USER_MODE` at the swap would
miss that case. Conversely, PFEC.U/S=1 implies the faulting access was from CPL3, so the browser-runtime
kill gate cannot run without the top-of-handler swap having run first in the ordinary architectural path.

The known exception is a CPL0 interval in which GS is already the user base. Such intervals exist before an
entry swap and after an exit swap. A nested `#PF` there has saved CS.RPL=0 and this handler deliberately will
not swap. I checked the current instructions in those intervals: they touch registers, mapped kernel stacks,
or fixed `.bss`; a malformed `iretq` produces a protection exception rather than an ordinary page fault.
The documented bare-metal diagnostic path can still fault indirectly from another fatal handler while GS is
user, and remains a sibling hole.

NMI/`#MC` can arrive before the swap because they are not controlled by IF. Their current handlers only print
and halt and make no direct per-CPU read (`idt.rs:661-664,834-840`). They cannot cause a double swap in the
current flow. If such a nested handler itself page-faulted before the original swap, the nested `#PF` would
see RPL0/user-GS and would not swap; that is the same documented paranoid-entry residual, not a new CP-44
failure mode.

### 3. Switch-away state

After a user `#PF`, the new swap leaves `GS_BASE = &PER_CPU` and the prior user value in
`KERNEL_GS_BASE`. `fault_kill_current_user` retires the current thread through `scheduler::exit` without
changing either MSR. The next kernel context therefore starts with the normal kernel GS. Its eventual user
return performs the same exit swap as before and receives the user value from `KERNEL_GS_BASE`. This is
identical to the old browser-only swap, except that the invariant is established before observability and
test hooks.

CP-41's `load_kernel_fp_control` remains immediately inside the `USER_MODE` kill branch and executes before
`fault_kill_current_user` (`idt.rs:598-607`). The earlier GS swap neither removes nor bypasses it.

### 4. Sibling entry/exit holes

The exact entry-path audit found no additional normal path that performs a GS-relative per-CPU read before
its intended swap:

- `syscall_entry` swaps before its first `gs:` access (`syscall.rs:537-545,624-633`).
- Every MSI/IPI IRQ routes through `swapgs_irq_entry`, which checks saved CS before calling Rust
  (`idt.rs:278-360`).
- Both timer variants check saved CS, swap before per-CPU stack selection, and return while IF remains clear
  until `iretq` restores the frame (`timer.rs:60-182,194-310`).
- The user-entry trampolines are reached by context switches performed under `without_interrupts`; they do
  not execute `sti` before `swapgs; iretq` (`context.rs:218-236`).

The CP-44 text's syscall-exit residual is real. `spawn_user` explicitly resumes TLB-shootdown readiness by
enabling IF (`process.rs:2075-2079,2176-2183`; `tlb_shootdown.rs:39-50`). Both syscall stubs then reach
`swapgs; iretq` without `cli` (`syscall.rs:613-614,704-705`). A maskable interrupt in that one-instruction
window sees saved CS.RPL=0 with the user GS live, skips its entry swap, and can fault on its first per-CPU
read. Adding `cli` immediately before the syscall exit swap is the correct follow-up; `iretq` restores the
user IF.

Moving GS establishment into assembly is the better long-term home, but not as one undifferentiated CP-40
shim. Error-code and no-error-code vectors have different saved-CS offsets, and any returning handler needs a
symmetric exit swap. A page-fault-specific naked/global-assembly wrapper can establish GS before any compiler
prologue and fix the error-code stack alignment at the same time.

### 5. Discriminator

The discriminator cannot pass vacuously:

- The original userspace battery still requires the guarded worker to die with exit 139 and the scheduler to
  continue.
- The handler independently derives the CPU slot from `IA32_GS_BASE`, compares it with the cpu id written by
  `record_trace` through `gs:[24]`, and increments `RING3_PF_TRACED_ON_KERNEL_GS` only on a match.
- The post-battery check requires that counter to be nonzero (`test_harness.rs:3677-3727`).

`latest_trace_cpu_id`'s overwrite guard is sound for this x86-only target. Writers claim a sequence with the
global `fetch_add` before touching a slot, publish the slot sequence with Release, and the reader rejects a
candidate if `TRACE_NEXT_SEQ` has advanced past `seq + N` after its field loads
(`observability.rs:399-413,465-499`). On x86 TSO, observing an overwrite's fields cannot precede observing
the writer's earlier sequence claim. A false failure would require all 16 slots to be claimed between the
local `record_page_fault` and the immediate hook while IF is clear. The scenario has one ring-3 guard fault
and idle peer CPUs, so no same-`cr2` writer can spuriously supply another CPU id in the measured lane.

Widening the CP-40 atomics and assertions to the observability feature preserves the original battery's
requirements: saved-frame AC must have been armed and live handler AC must be zero. The scenario feature
compiled successfully at `NEO_SMP=4`.

### 6. Alignment note and additional structural checks

The pinned compiler's linked-kernel disassembly reproduces the note exactly:

- `page_fault` starts with ten pushes and `sub rsp, 0x278`; its body issues calls with RSP eight bytes away
  from the SysV caller alignment.
- `recompute_earliest` subtracts `0xb8` and emits `movaps [rsp+0x90], xmm2`. Reached through
  `fault_kill_current_user -> futex::purge_pid`, that aligned move faults when the dying process has a timed
  futex waiter.

The note is therefore factual. The sound repair is a page-fault assembly entry that preserves the hardware
frame, establishes the kernel GS, aligns RSP, and calls a diverging C-ABI body. It should be gated with a
timed-futex sibling so the current `recompute_earliest` instruction is executed, not merely inspected.

I additionally checked the inline-asm options: `swapgs` neither uses the stack nor changes RFLAGS, so
`nostack, preserves_flags` is accurate. Omitting `nomem` is conservative and ensures the compiler cannot move
the subsequent per-CPU/trace access before the GS transition.

## Build, codegen, and environment evidence

- Compiler: `rustc 1.97.0-nightly (e8e4541ff 2026-04-15)`, LLVM 22.1.2.
- The fixed `threads_guard_fault_observability` userspace and kernel builds succeeded.
- Fixed disassembly: saved-CS test -> `swapgs` -> AC counters -> `record_trace` -> test hook ->
  `load_kernel_fp_control` -> `fault_kill_current_user`.
- Prescribed negative control compiled successfully in a separate target directory. Its disassembly placed
  `record_trace` and the test hook before the sole `swapgs` in the browser kill branch, confirming the
  discriminator's targeted ordering difference. The temporary source edit was restored before this RETURN.
- QEMU execution was unavailable on this Windows seat because xtask could not find the required Limine tree
  (`/home/neo/limine`, `$LIMINE_DIR` unset). I do not claim a runtime PASS or negative-control timeout.
- RAG health was `healthy` with Ollama and Qdrant available, but all three path-filtered conceptual queries
  returned empty results. Load-bearing conclusions therefore use direct source reads, exact searches, and
  linked-kernel disassembly.
- Both review target directories were reclaimed with `cargo clean`. The pre-existing untracked `graft/`
  directory was left untouched.
