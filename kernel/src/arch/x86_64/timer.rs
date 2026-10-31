// SPDX-License-Identifier: MPL-2.0
//! Timer interrupt — vector 32 (PIC IRQ0).
//!
//! This is the entry point for preemption. The CPU dispatches this handler
//! on TIMER_IST_INDEX (IST 2) so a stack-overflow in the interrupted thread
//! cannot wedge the timer. The asm wrapper then:
//!
//!   1. Saves the interrupted thread's r10/r11 to a per-CPU scratch area
//!      (TIMER_SCRATCH) — we need them as scratch during the stack-switch
//!      arithmetic, but the thread expected them intact across the interrupt.
//!   2. Reads the interrupted thread's RSP from the iretq frame on IST.
//!   3. Copies the 5-qword iretq frame from IST stack to thread_RSP - 40 on
//!      the thread's own stack, so iretq later finds it there. (Cooperative
//!      `context_switch` saves prev's RSP — that pointer must be on the
//!      *thread's* stack, not the per-CPU IST stack which is shared.)
//!   4. Switches RSP to the thread stack.
//!   5. Restores r10/r11 from TIMER_SCRATCH.
//!   6. Saves all 15 GP regs onto the thread stack.
//!   7. Anchors RSP via rbp, aligns down, reserves an FPU save area, saves it,
//!      and loads the kernel's default MXCSR (CP-41).
//!   8. Calls `timer_rust_handler` which EOIs the PIC, ticks, and may yield.
//!   9. On return: restore FPU state, restore RSP from rbp, pop GP regs, iretq.
//!
//! Single-CPU correctness: TIMER_SCRATCH is a static mut shared across all
//! preemptions. Safe because the IDT entry is an interrupt gate (IF cleared
//! on entry), so the timer cannot re-enter itself before iretq pops RFLAGS.
//! Phase 2+ SMP will need per-CPU scratch via GS-base.
//!
//! Ring-transition prologue (ADR-005): the asm wrapper branches on the saved
//! CS.RPL in the iretq frame. For ring-0 source the existing path is correct
//! (iretq frame's saved RSP IS the interrupted kernel thread's current RSP).
//! For ring-3 source we MUST NOT use that RSP — it's the user stack — so we
//! swapgs and load the destination kernel stack top from per-CPU
//! (`gs:[KERNEL_RSP_OFFSET]`, kept current by the scheduler), saving the
//! user RSP into per-CPU scratch (`gs:[USER_RSP_SAVE_OFFSET]`). On exit we
//! re-check CS.RPL and swapgs back only on the ring-3 return path.

use core::arch::global_asm;
use core::sync::atomic::{AtomicU64, Ordering};

#[cfg_attr(feature = "lapic-timer", allow(unused_imports))]
// the PIC EOI is in `lapic_timer::eoi`
use crate::arch::x86_64::pic;
use crate::sched;

/// Tick counter, incremented once per timer interrupt. 100 Hz nominal.
pub static TICKS: AtomicU64 = AtomicU64::new(0);

/// Single-CPU scratch slot for r10/r11 across the IST→thread stack switch.
/// Lives in .bss; declared from asm via `.comm` is annoying so we expose it
/// here and reference via `[rip + TIMER_SCRATCH]`.
#[unsafe(no_mangle)]
static mut TIMER_SCRATCH: [u64; 2] = [0; 2];

/// OT-61 C1.3b (CP-86): hand the timer body the interrupted CS (the iretq
/// frame copy sits 136 bytes above the `rbp` anchor in both stub variants:
/// the anchor slot, 15 saved registers, then RIP), so it can retire a thread of
/// a process being retired only when it interrupted ring 3. OT-93 reads it in
/// every build (a thread of a force-killed process ends there too).
macro_rules! timer_ring3_arg {
    () => {
        "    mov rdi, [rbp + 136]\n"
    };
}

#[cfg(not(feature = "xsave-fpu"))]
global_asm!(concat!(
    r#"
.global timer_isr_entry
.type timer_isr_entry, @function
timer_isr_entry:
    # ENTRY: on TIMER_IST_INDEX (IST 2) stack.
    # CPU has pushed iretq frame: [rsp+0]=RIP, [rsp+8]=CS, [rsp+16]=RFLAGS, [rsp+24]=RSP_thread, [rsp+32]=SS.
    # IF=0 (interrupt gate), no nested timer possible.

    # Save interrupted thread's r10/r11 to scratch (we need them as scratch
    # below; thread expected them intact across the interrupt).
    mov [rip + TIMER_SCRATCH + 0], r10
    mov [rip + TIMER_SCRATCH + 8], r11

    # === ADR-005: ring-transition prologue =============================
    # Branch on saved CS.RPL (low 2 bits of CS at [rsp+8]).
    #   RPL=0 (ring-0 source): iretq frame's saved RSP is the kernel
    #     thread's current RSP — use it as today.
    #   RPL=3 (ring-3 source): saved RSP is the USER stack; swapgs and load
    #     the kernel stack top from per-CPU instead.
    test byte ptr [rsp + 8], 3
    jz 1f                              # 1f = ring-0 source

    # --- ring-3 source -------------------------------------------------
    swapgs                             # GS_BASE now = &PER_CPU (standard kernel state)
    mov r10, gs:[0]                    # kernel stack top (KERNEL_RSP_OFFSET=0)
    # User RSP is at IST [rsp+24] and gets copied into the destination iretq
    # frame at [r10+24] by the common copy block below; no per-CPU stash
    # needed (unlike the SYSCALL path where the CPU doesn't push user RSP).
    jmp 2f

1:  # --- ring-0 source: existing path ----------------------------------
    mov r10, [rsp + 24]                # interrupted kernel thread's RSP

2:  # r10 = destination kernel stack top. Reserve 5 qwords for iretq frame copy.
    sub r10, 40

    # Copy 5-qword iretq frame from IST [rsp..rsp+40] to dest [r10..r10+40].
    mov r11, [rsp +  0]; mov [r10 +  0], r11
    mov r11, [rsp +  8]; mov [r10 +  8], r11
    mov r11, [rsp + 16]; mov [r10 + 16], r11
    mov r11, [rsp + 24]; mov [r10 + 24], r11
    mov r11, [rsp + 32]; mov [r10 + 32], r11

    # Switch to destination kernel stack.
    mov rsp, r10

    # Restore thread's r10, r11 from scratch.
    mov r10, [rip + TIMER_SCRATCH + 0]
    mov r11, [rip + TIMER_SCRATCH + 8]

    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    # Interrupt delivery clears neither RFLAGS.DF nor RFLAGS.AC, and ring 3 can
    # set both (`std`, `popfq`). The SysV Rust body and compiler_builtins'
    # `rep movs`/`stos` memcpy/memset assume DF=0 (CP-39), and AC=1 suspends
    # SMAP for every supervisor access (CP-40). Clear both before any Rust
    # runs. A flags-mask is used instead of `clac`, which #UDs without SMAP.
    # The saved frame is untouched, so iretq restores the caller's DF/AC.
    pushfq
    and qword ptr [rsp], -263169        # ~(AC (1<<18) | DF (1<<10))
    popfq

    # Anchor pre-align RSP via rbp (callee-saved per SysV; the call below preserves it).
    push rbp
    mov rbp, rsp

    # ALIGNMENT INVARIANT (do not insert any push between here and `call`):
    # `and rsp, ~0xF` aligns RSP to 16. `sub rsp, 512` keeps it 16-aligned.
    # `call` pushes 8 → callee enters at RSP ≡ 8 mod 16 (SysV requirement).
    and rsp, 0xFFFFFFFFFFFFFFF0
    sub rsp, 512
    fxsave [rsp]
    # Never run the Rust body, or a thread it switches to, on the interrupted
    # context's MXCSR (CP-41). fxrstor puts it back. No fninit, as in
    # syscall_entry: the kernel has no x87 code.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]

"#,
    timer_ring3_arg!(),
    r#"
    call timer_rust_handler

    fxrstor [rsp]

    # Unwind: restore RSP from rbp anchor, pop the anchor, pop the 15 GP regs.
    mov rsp, rbp
    pop rbp

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    # === ADR-005: ring-transition epilogue =============================
    # RSP now points at the iretq frame copy. CS at [rsp+8].
    # Re-check CS.RPL: if source was ring 3, swapgs back to restore user GS.
    # If source was ring 0, leave GS alone (we never swapped in).
    test byte ptr [rsp + 8], 3
    jz 3f
    swapgs                             # restore user GS state
3:
    # iretq pops the 5-qword frame. For ring-3 source the saved RSP slot
    # holds the user RSP we copied through, so the CPU transitions back to
    # ring 3 with the correct user stack. For ring-0 source it's the
    # interrupted kernel thread's RSP and we resume on the same stack.
    iretq
.size timer_isr_entry, . - timer_isr_entry
"#
));

#[cfg(feature = "xsave-fpu")]
global_asm!(concat!(
    r#"
.global timer_isr_entry
.type timer_isr_entry, @function
timer_isr_entry:
    # ENTRY: on TIMER_IST_INDEX (IST 2) stack.
    # CPU has pushed iretq frame: [rsp+0]=RIP, [rsp+8]=CS, [rsp+16]=RFLAGS, [rsp+24]=RSP_thread, [rsp+32]=SS.
    # IF=0 (interrupt gate), no nested timer possible.

    # Save interrupted thread's r10/r11 to scratch (we need them as scratch
    # below; thread expected them intact across the interrupt).
    mov [rip + TIMER_SCRATCH + 0], r10
    mov [rip + TIMER_SCRATCH + 8], r11

    # === ADR-005: ring-transition prologue =============================
    # Branch on saved CS.RPL (low 2 bits of CS at [rsp+8]).
    #   RPL=0 (ring-0 source): iretq frame's saved RSP is the kernel
    #     thread's current RSP — use it as today.
    #   RPL=3 (ring-3 source): saved RSP is the USER stack; swapgs and load
    #     the kernel stack top from per-CPU instead.
    test byte ptr [rsp + 8], 3
    jz 1f                              # 1f = ring-0 source

    # --- ring-3 source -------------------------------------------------
    swapgs                             # GS_BASE now = &PER_CPU (standard kernel state)
    mov r10, gs:[0]                    # kernel stack top (KERNEL_RSP_OFFSET=0)
    jmp 2f

1:  # --- ring-0 source: existing path ----------------------------------
    mov r10, [rsp + 24]                # interrupted kernel thread's RSP

2:  # r10 = destination kernel stack top. Reserve 5 qwords for iretq frame copy.
    sub r10, 40

    # Copy 5-qword iretq frame from IST [rsp..rsp+40] to dest [r10..r10+40].
    mov r11, [rsp +  0]; mov [r10 +  0], r11
    mov r11, [rsp +  8]; mov [r10 +  8], r11
    mov r11, [rsp + 16]; mov [r10 + 16], r11
    mov r11, [rsp + 24]; mov [r10 + 24], r11
    mov r11, [rsp + 32]; mov [r10 + 32], r11

    # Switch to destination kernel stack.
    mov rsp, r10

    # Restore thread's r10, r11 from scratch.
    mov r10, [rip + TIMER_SCRATCH + 0]
    mov r11, [rip + TIMER_SCRATCH + 8]

    push rax
    push rcx
    push rdx
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15

    # Interrupt delivery clears neither RFLAGS.DF nor RFLAGS.AC, and ring 3 can
    # set both (`std`, `popfq`). The SysV Rust body and compiler_builtins'
    # `rep movs`/`stos` memcpy/memset assume DF=0 (CP-39), and AC=1 suspends
    # SMAP for every supervisor access (CP-40). Clear both before any Rust
    # runs. A flags-mask is used instead of `clac`, which #UDs without SMAP.
    # The saved frame is untouched, so iretq restores the caller's DF/AC.
    pushfq
    and qword ptr [rsp], -263169        # ~(AC (1<<18) | DF (1<<10))
    popfq

    # Anchor pre-align RSP via rbp (callee-saved per SysV; the call below preserves it).
    push rbp
    mov rbp, rsp

    # XSAVE requires 64-byte alignment. FPU_SAVE_AREA_SIZE is CPUID.0xD-sized
    # and 64-byte aligned by arch::x86_64::fpu.
    and rsp, 0xFFFFFFFFFFFFFFC0
    sub rsp, qword ptr [rip + FPU_SAVE_AREA_SIZE]
    mov rdi, rsp
    call fpu_save_area_asm
    # Never run the Rust body, or a thread it switches to, on the interrupted
    # context's MXCSR (CP-41). The restore puts it back. No fninit, as in
    # syscall_entry: the kernel has no x87 code.
    ldmxcsr [rip + KERNEL_DEFAULT_MXCSR]

"#,
    timer_ring3_arg!(),
    r#"
    call timer_rust_handler

    mov rdi, rsp
    call fpu_restore_area_asm

    # Unwind: restore RSP from rbp anchor, pop the anchor, pop the 15 GP regs.
    mov rsp, rbp
    pop rbp

    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rax

    # === ADR-005: ring-transition epilogue =============================
    # RSP now points at the iretq frame copy. CS at [rsp+8].
    # Re-check CS.RPL: if source was ring 3, swapgs back to restore user GS.
    # If source was ring 0, leave GS alone (we never swapped in).
    test byte ptr [rsp + 8], 3
    jz 3f
    swapgs                             # restore user GS state
3:
    iretq
.size timer_isr_entry, . - timer_isr_entry
"#
));

unsafe extern "C" {
    /// Symbol provided by the global_asm! block above. Address is taken in
    /// `idt::init` to register vector 32 with `set_handler_addr`.
    pub fn timer_isr_entry();
}

/// Called from the asm wrapper between FPU save and restore. RSP is 16-aligned
/// here per SysV; safe to call other Rust fns. RBP holds the pre-align RSP
/// anchor — we must not clobber it (Rust's epilogue won't, since rbp is
/// callee-saved and we don't take its address).
#[unsafe(no_mangle)]
extern "C" fn timer_rust_handler(interrupted_cs: u64) {
    // irq_xmm_preserve: the entry state this stub hands its body (CP-39/40/41).
    #[cfg(feature = "neo-test-irq-xmm-preserve")]
    {
        crate::arch::x86_64::idt::TIMER_BODY_FLAGS.note();
        // ...and make its x87/SSE save load-bearing, as the test-vector body does.
        crate::arch::x86_64::idt::irq_test_clobber_xmm();
    }
    TICKS.fetch_add(1, Ordering::Relaxed);
    // OT-61 C1.1 hang discriminator: count ticks PER CPU. The expectation is
    // that every AP reads 0 — this is PIC IRQ0 (vector 32), delivered to the
    // BSP only, and there is no LAPIC timer — which means APs have no
    // preemption point and an EOI-only reschedule IPI cannot force one
    // (CP-15/CP-21). That is load-bearing for this hunt, so the capture
    // measures it instead of inferring it from a comment.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::note_tick();
    // OT-63 Rung 1 Slice 1: the timer stands in for the Gen9 vblank ISR in QEMU
    // (no GPU) — drain pending hardware-flip present-fences. Only fires in the
    // HardwareFlip backend (an atomic load; no lock, no work in the Software /
    // accel-off kill-switch path). Slice 4 replaces this trigger with the real
    // display-engine vblank/flip-done ISR; the drained `on_present_vblank` body
    // is identical (lock-free per buffer, I-14a).
    #[cfg(feature = "atrium-compositor-g1")]
    if crate::atrium::present_backend::mode()
        == crate::atrium::present_backend::PresentMode::HardwareFlip
    {
        crate::atrium::present_backend::on_present_vblank();
    }
    // EOI before sched::on_tick: the PIC is now armed for the next IRQ, but
    // IF stays 0 until iretq pops RFLAGS — so no nested entry. If on_tick
    // yields, we may not iretq for many ticks, and the PIC may collapse the
    // backlog to one pending IRQ. Acceptable: 100 Hz preemption tolerates
    // missed ticks.
    #[cfg(feature = "lapic-timer")]
    crate::arch::x86_64::lapic_timer::eoi();
    #[cfg(not(feature = "lapic-timer"))]
    pic::end_of_interrupt(0);
    // OT-61 C1.1: wake expired futex waiters BEFORE the preemption rotate —
    // on_tick can switch away, and the expiry scan must run on every tick.
    // Fast path is one relaxed atomic load when no timed waiter exists.
    #[cfg(feature = "browser-runtime")]
    crate::sched::futex::on_tick_expire();
    // OT-22: wake expired multiplex waiters (and input waiters, whose
    // readiness is pulled once per tick). Same fast path: one relaxed load.
    #[cfg(feature = "wait-multiplex")]
    crate::sched::wait_any::on_tick_expire();
    // CP-81: wake expired kernel-thread sleepers. Same fast path.
    #[cfg(feature = "tick-sleep")]
    crate::sched::tick_sleep::on_tick_expire();
    // OT-61 C1.1: the silent-hang watchdog. Runs AFTER the expiry scan (so a
    // waiter that was merely due-to-time-out is already gone) and BEFORE
    // `on_tick`, which may switch away. Cheap: one tick-delta compare on all
    // but every ~256th tick, and it prints only when the futex queue has
    // genuinely frozen — converting a 180 s blind timeout into a captured,
    // self-describing cross-CPU state dump.
    #[cfg(feature = "neo-test")]
    crate::sched::wake_trace::on_tick_watchdog(TICKS.load(Ordering::Relaxed));
    // OT-61 C1.3b (CP-86): a ring-3 thread of a process being retired ends
    // here instead of being preempted back into a ready queue (it holds no
    // kernel lock: it was in ring 3). Before the EOI'd `on_tick`, which may
    // switch away.
    #[cfg(feature = "browser-runtime")]
    if interrupted_cs & 3 == 3 {
        crate::sched::process::retire_current_if_dying();
    }
    // OT-93: likewise for a thread of a force-killed process.
    if interrupted_cs & 3 == 3 {
        crate::sched::exit_current_if_force_killed();
    }
    sched::on_tick();
}
