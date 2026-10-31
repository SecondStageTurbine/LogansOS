# ADR-005: Ring-Transition Prologue for the Timer ISR

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-05-03 |
| **Authority** | Project Lead |
| **Supersedes** | — |
| **Superseded by** | — |
| **Related** | Bible Article 4.1 · ADR-001 (monolithic kernel) · `kernel/src/arch/x86_64/timer.rs` (Phase 1.3) · `kernel/src/arch/x86_64/syscall.rs` (Phase 1.4 SYSCALL prologue) |

---

## Context

The Phase 1.3 timer ISR was written when no ring-3 code existed. Its asm wrapper unconditionally treats the iretq frame's saved RSP (at `[rsp+24]` on entry to the ISR) as a kernel stack belonging to the interrupted thread, and switches to it for the rest of the ISR's work:

```
mov r10, [rsp + 24]    ; "thread RSP" (from iretq frame)
sub r10, 40            ; reserve 5 qwords for iretq frame copy
... copy frame ...
mov rsp, r10           ; switch to "thread stack"
... save GP regs, fxsave, call dispatcher ...
```

This was correct under Phase 1.3's assumption: every interrupted thread was a kernel thread, and "thread RSP" in the iretq frame was always a kernel stack pointer mid-execution.

Phase 1.4 introduced ring-3 user processes. The single short-lived `neo-init` rarely lived long enough for the 100 Hz timer to fire during its ring-3 execution, so the bug did not manifest. Phase 1.4's plan also did not include an explicit "ring-3 + timer preemption works" criterion (ISC-91 covered ring-0 preemption only — Phase 1.3 ping/pong/hog still passes), so the gap was not caught at planning time.

Phase 1.5 BUILD-2 introduced a longer-lived `neo-worker` process and the bug surfaces. Reproducing reliably:

```
[ kernel ] proc: spawned 'worker' (pid=2) entry=0x0000000000400000 user_rsp=0x00007fffffffeff8
... worker runs in ring 3 ...
[ trap  ] PAGE_FAULT (#PF) cr2=0x00000000004b5000
[ trap  ] PAGE_FAULT err = PageFaultErrorCode(0x0)
[ trap  ] PAGE_FAULT rip=0xffff80001ff9c8c8 cs=0x8 rflags=0x2 rsp=0x00007fffffffeb60 ss=0x0
```

Diagnosis: timer fires while worker is in ring 3. CPU switches to IST 2 (correct), pushes iretq frame on IST 2 with `[rsp+24]` = user RSP (`0x7fff_ffff_eff8`-ish). Asm wrapper does `sub r10, 40` and switches `rsp` to user-stack memory. The kernel then runs `timer_rust_handler` *with `RSP` pointing into a user page*. Subsequent kernel work (calls, register spills, allocations) writes to user memory. Eventually a stack-frame access crosses into unmapped territory (cr2=0x4b5000 is in worker's user half but outside its 16 KiB stack mapping) and the kernel page-faults from inside its own ISR path.

The non-determinism (different runs trap at different RIPs / CR2 values) is the bug telling us "I am sensitive to interleaving" — a different timer/syscall race produces a different corruption pattern. This is exactly the signal to stop chasing prints and fix the foundation.

## Decision

**Adopt the standard ring-transition prologue for the timer ISR, AND adopt the standard GS-base convention for kernel mode.** Both changes land together because the prologue's correctness depends on the convention.

### Part A — the prologue

On entry, branch on the saved CS in the iretq frame:

- **CS.RPL == 0 (ring-0 source):** the existing path is correct. The iretq frame's saved RSP is the interrupted kernel thread's current RSP; it lives below the kernel stack top with the rest of the stack free below. `[rsp+24] - 40` is a safe destination.

- **CS.RPL == 3 (ring-3 source):** the iretq frame's saved RSP is the user RSP and **must not be used as a kernel stack**. Instead:
  1. `swapgs` to bring the per-CPU area into `GS_BASE`.
  2. Load the destination kernel stack top from per-CPU (`gs:[KERNEL_RSP_OFFSET]`), set by the scheduler's `cross_thread_kstack` whenever it switches into a thread.
  3. Reserve 40 bytes for the iretq frame copy as before. The user RSP is preserved in the iretq frame copy itself (no need for a per-CPU stash slot, unlike the SYSCALL path where the CPU doesn't push it).
  4. Continue with the existing GP-save / FXSAVE / dispatcher path.

On exit, before `iretq`: re-check the saved CS. If RPL == 3, `swapgs` to restore user GS state. If RPL == 0, do not swapgs.

### Part B — the GS convention

The Phase 1.4 `per_cpu::init` writes `GS_BASE = 0` and `KERNEL_GSBASE = &PER_CPU`, treating "kernel mode with GS=0" as the resting state and `swapgs`-ing to `&PER_CPU` only inside the SYSCALL body. This is the inverse of the convention every standard x86_64 kernel uses, and it works only as long as no other kernel-mode code (outside SYSCALL) does `gs:[..]` access. The Part A prologue introduces exactly that — a `gs:[..]` read in the timer ISR — and surfaces a cross-thread leak: when the timer's ring-3 prologue `swapgs`'s, the body runs with `GS = &PER_CPU`. If the body context-switches into a brand-new user thread via `enter_userspace_trampoline` (which does not `swapgs`), that thread enters ring 3 inheriting `GS = &PER_CPU` instead of `GS = user_gs`. Its first SYSCALL then `swapgs`'s `KERNEL_GSBASE = 0` into `GS_BASE`, and the very next `mov gs:[8], rsp` faults at virtual address `0x8`. Reproduces deterministically once `neo-worker` exists.

**Adopt the standard convention to break the leak:**

- `per_cpu::init` writes `GS_BASE = &PER_CPU` and `KERNEL_GSBASE = 0`. Kernel mode now ALWAYS has `GS_BASE = &PER_CPU`.
- `enter_userspace_trampoline` adds `swapgs` before `iretq`, completing the kernel→user transition in the standard way.
- Existing `swapgs` placements in `syscall_entry` and (Part A) the timer prologue/epilogue are unchanged — they already match the standard convention; only the boot-time direction was inverted.

Net effect: every transition between kernel and user is paired by exactly one `swapgs` on each side. Kernel-mode code can read `gs:[..]` from any context without first checking which convention is active. Task switches inside the kernel never touch GS state.

This is catch-up to standard kernel practice. The fix is not novel design; it adopts what every modern x86_64 kernel (Linux, FreeBSD, Windows, illumos, the hobbyist OSes) does on every interrupt vector that can fire from ring 3, AND for the kernel-mode GS convention.

### Scope: timer only, for now

The `extern "x86-interrupt"` exception vectors (#PF, #GP, #UD, etc.) handle the ring transition correctly already — the CPU itself switches to TSS.rsp0 on a ring-3 → ring-0 fault, and rustc's `extern "x86-interrupt"` ABI generates a prologue that doesn't touch RSP further.

The IST-using exception vectors (#DF on IST 0, NMI on IST 1) are also fine: they unconditionally use a dedicated IST stack regardless of source ring, and their handlers don't try to switch off it.

Only the timer asm wrapper is broken because it manually re-switches stacks based on the iretq frame's RSP — assuming that RSP is always a kernel stack. This ADR fixes only the timer.

If Phase 1.5+ adds another asm-level IDT handler that does manual stack-switching (e.g., for IPC fast-path), the same fix-pattern applies. The asm helper extracted by this ADR (the prologue + epilogue branches) becomes the canonical primitive.

## Consequences

### Positive

- **The bug is fixed at its root.** No more user-stack-as-kernel-stack corruption. The non-determinism observed in BUILD-2 disappears.
- **Phase 1.5b unblocks.** IPC, signals, and the userspace heap all rely on the scheduler being able to preempt long-lived ring-3 work safely. With the prologue fix, those subsystems can land without re-litigating this issue.
- **Catch-up to standard.** NeoPlutarch's interrupt handling is now what every contributor would expect from an x86_64 kernel.
- **Future ASM IDT handlers reuse the pattern.** The CS.RPL-branch + swapgs structure becomes the template.

### Negative

- **Modest asm complexity increase.** Adds ~25 lines: CS.RPL test, two swapgs instructions, two stack-loading paths. The prologue is no longer a single linear sequence.
- **Slight perf hit on ring-3 timer interrupts.** swapgs + per-CPU memory access + extra branch ≈ ~15 cycles. Imperceptible at 100 Hz timer rate.
- **The TIMER_SCRATCH static is still required** (single-CPU `r10`/`r11` save during the stack switch). Phase 2 SMP migration replaces it with per-CPU storage; that's already the migration target documented in `per_cpu.rs`.
- **One extra `swapgs` per user-thread first dispatch.** `enter_userspace_trampoline` now does `swapgs; iretq` instead of just `iretq`. ~10 cycles, paid once per process.

### Neutral

- **Phase 1.5 re-splits.** Phase 1.5a = BUILD-1 (Pmap::Drop walker) + this fix + sys_spawn + neo-worker scaffold. Phase 1.5b = IPC + signals + userspace heap. The split point is the architectural boundary the bug just told us about (ring-3 preemption works *or* it doesn't; everything in 1.5b assumes it works).

## Alternatives Considered

### Alternative 1: Drop IST 2, use TSS.rsp0 for the timer like exception vectors do

Rejected. The IST was added in Phase 1.3 specifically to defend against stack-overflow in the interrupted thread. Removing it trades simplicity for less defense-in-depth. The IST-plus-prologue-fix combination keeps both properties.

### Alternative 2: Patch the timer asm to detect ring-3 source and just halt with a diagnostic

Rejected. "Detect and halt" is a debugger pattern, not a fix. ring-3 preemption is required for Phase 1.5; halting on it defeats the phase.

### Alternative 3: Defer the fix and ship Phase 1.5a without it

Rejected. The bug exists in `main` right now. It does not manifest reliably until a long-lived ring-3 process exists, which Phase 1.5a's neo-worker creates. Shipping Phase 1.5a without the fix means main is in a known-broken state for any user who runs the smoke test long enough. Bible Article 1.3 ("every commit is intended to ship") cuts both ways — landing a broken commit because "Phase 1.5b will fix it" is the toy-code anti-pattern in a different costume.

### Alternative 4: Always-swapgs (drop the CS.RPL branch)

Rejected. Always-swapgs would force the ring-0 timer path to use `gs:[KERNEL_RSP_OFFSET]` (= kernel stack top) as the destination, which would *overwrite the in-use portion* of the interrupted kernel thread's stack (since `top - 40 < current_rsp` whenever the interrupted thread has used any stack). The CS.RPL branch is not optional.

## Implementation

This ADR is implemented by the Phase 1.5a commit landing alongside it. Three concrete files change:

### 1. `kernel/src/arch/x86_64/per_cpu.rs` — adopt standard GS convention

```rust
pub fn init() {
    KernelGsBase::write(VirtAddr::new(0));
    GsBase::write(VirtAddr::new(per_cpu_addr));   // was: GsBase=0, KernelGsBase=&PER_CPU
}
```

### 2. `kernel/src/arch/x86_64/context.rs` — `enter_userspace_trampoline`

```
enter_userspace_trampoline:
    swapgs                              ; was missing — kernel→user transition needs it
    iretq
```

### 3. `kernel/src/arch/x86_64/timer.rs` — ring-transition prologue/epilogue

```
timer_isr_entry:
    # Save scratch r10/r11 to TIMER_SCRATCH (single-CPU; per-CPU migration: Phase 2).
    mov [rip + TIMER_SCRATCH + 0], r10
    mov [rip + TIMER_SCRATCH + 8], r11

    # Branch on saved CS.RPL (CS at [rsp+8], low 2 bits = RPL).
    test byte ptr [rsp + 8], 3
    jz .from_kernel

.from_user:
    swapgs                              ; GS_BASE now = &PER_CPU (standard kernel state)
    mov r10, gs:[0]                     ; kernel stack top from per-CPU
    jmp .common

.from_kernel:
    mov r10, [rsp + 24]                 ; interrupted kernel thread's RSP

.common:
    sub r10, 40                         ; reserve 5 qwords for iretq frame copy
    ; ... copy iretq frame from [rsp..rsp+40] to [r10..r10+40] ...
    mov rsp, r10                        ; switch to destination kernel stack
    mov r10, [rip + TIMER_SCRATCH + 0]  ; restore user/kernel r10
    mov r11, [rip + TIMER_SCRATCH + 8]  ; restore user/kernel r11
    ; ... existing GP push / fxsave / call timer_rust_handler / fxrstor / pops ...
    ; After all pops, RSP points at iretq frame.

    # On return, branch on saved CS again to decide whether to swapgs back.
    test byte ptr [rsp + 8], 3
    jz .ret_to_kernel
    swapgs                              ; restore user GS state
.ret_to_kernel:
    iretq
```

The user RSP is NOT stashed in per-CPU scratch (unlike the SYSCALL path) — the iretq frame copy on the destination kernel stack already preserves it at offset +24.

Verification (all confirmed in the Phase 1.5a commit's boot trace):

- [x] BUILD-1 (Pmap::Drop walker) re-verified: walker logs `pmap: dropped — freed 12 frames + 1 PML4` for both `neo-init` and `neo-worker` after exit.
- [x] BUILD-2-without-IPC re-verified: `neo-init` spawns `neo-worker`, both print, both reaped, both Pmaps freed. No `SS=0`, no spurious page faults.
- [x] **Preemption-heavy verification:** `neo-worker` runs a tight ring-3 loop (~6M iterations) printing four "preemption-loop tick" lines via `sys_write`, then exits cleanly. The loop spans many 100 Hz timer ticks (each forcing the new ring-transition prologue path) with zero faults.
- [x] Phase 1.3 ping/pong/hog smoke still passes (anti-regression for ring-0 preemption — `ticks=8`).

## Notes

This bug was not caught earlier because the Phase 1.4 plan did not include an explicit "what existing tests must continue to pass under the new functionality" criterion that covered preemption + ring 3. ISC-91 covered "Phase 1.3 ping/pong/hog still passes" (ring-0 preemption only). The Plan-agent review of Phase 1.4 focused on the new SYSCALL/IRETQ asm and did not flag the timer asm as needing a similar review for the new ring-3 case.

**Future Phase plans should include a "preemption-cross-product" criterion**: for any new execution context introduced (ring 3, threaded user, async syscalls, etc.), explicitly enumerate which existing preemption sources (timer, IPI, NMI) must continue to work in that context, and add an ISC for each. This catches contract-gap-under-preemption bugs at planning time rather than at boot-test time.

This is the same lesson Phase 1.4 absorbed for cooperative `context_switch`'s IF preservation (Phase 1.2 contract was "saves callee-saved + RIP" — not "preserves IF" — and Phase 1.3 preemption needed IF=1 in user threads). Generalizing: every primitive's runtime-state contract must be re-examined when a new contract requirement arises.

---

**Decision recorded.** Per Bible Article 27, this ADR is amendable by superseding ADR.
