// SPDX-License-Identifier: MPL-2.0
//! x86_64 cooperative context switch.
//!
//! `context_switch(prev_rsp_ptr, next_rsp)` saves the SysV-AMD64 callee-saved
//! register set onto the current stack, writes the resulting RSP into
//! `*prev_rsp_ptr`, then loads `next_rsp` into RSP, pops the callee-saved set
//! from the new stack, and returns. Control resumes at the new task's saved RIP.
//!
//! What we save / why this set is sufficient:
//!   SysV AMD64 callee-saved: RBX, RBP, R12, R13, R14, R15.
//!   The compiler treats `extern "C"` calls as opaque clobbers of caller-saved
//!   regs (RAX, RCX, RDX, RSI, RDI, R8-R11), so no need to save those — the
//!   caller already spilled anything it cared about.
//!
//! What the legacy default-off path deliberately does NOT save:
//!   - RFLAGS: caller-saved in SysV; would also resurrect a stale IF if threads
//!     differ in interrupt state. We rely on `without_interrupts` at the caller.
//!   - FS/GS base: no per-thread TLS yet.
//!   - MXCSR: callee-saved, and the kernel default on both sides of a switch
//!     because every entry that runs Rust on a saved user image loads it
//!     (CP-41). x87 control word: no kernel code uses x87.
//!   - XMM registers: caller-saved in SysV, and this routine is only reached by
//!     a `call` (voluntary switch) or from an interrupt stub that has already
//!     saved x87/SSE itself (the timer stub, `swapgs_irq_entry`). The kernel IS
//!     compiled with SSE, so every interrupt stub that runs Rust must keep
//!     doing so (CP-38).
//!
//! With `xsave-fpu` enabled, the switch shims also save/restore the per-thread
//! x87/SSE/AVX image through `arch::x86_64::fpu`.

use core::arch::global_asm;
use core::sync::atomic::AtomicBool;

#[cfg(not(feature = "xsave-fpu"))]
global_asm!(
    r#"
.global context_switch
.type context_switch, @function
context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp
    mov rsp, rsi
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch, . - context_switch

.global context_switch_with_release2
.type context_switch_with_release2, @function
context_switch_with_release2:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp
    mov rsp, rsi
    mov byte ptr [rdx], 0
    mov byte ptr [rcx], 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch_with_release2, . - context_switch_with_release2

.global context_switch_with_release1
.type context_switch_with_release1, @function
context_switch_with_release1:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp
    mov rsp, rsi
    mov byte ptr [rdx], 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch_with_release1, . - context_switch_with_release1
"#
);

#[cfg(feature = "xsave-fpu")]
global_asm!(
    r#"
.global context_switch
.type context_switch, @function
context_switch:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp

    # SysV: after six pushes the stack is still 8 mod 16. Align before the
    # helper calls without changing the saved thread RSP image.
    sub rsp, 8
    mov r8, rsi
    mov r9, rcx
    mov rdi, rdx
    call fpu_save_area_asm
    mov rdi, r9
    call fpu_restore_area_asm
    add rsp, 8

    mov rsp, r8
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch, . - context_switch

.global context_switch_with_release2
.type context_switch_with_release2, @function
context_switch_with_release2:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp

    sub rsp, 8
    mov r10, rsi
    mov r11, rcx
    mov rdi, rdx
    call fpu_save_area_asm
    mov rdi, r11
    call fpu_restore_area_asm
    add rsp, 8

    mov rsp, r10
    mov byte ptr [r8], 0
    mov byte ptr [r9], 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch_with_release2, . - context_switch_with_release2

.global context_switch_with_release1
.type context_switch_with_release1, @function
context_switch_with_release1:
    push rbx
    push rbp
    push r12
    push r13
    push r14
    push r15
    mov [rdi], rsp

    sub rsp, 8
    mov r10, rsi
    mov r11, rcx
    mov rdi, rdx
    call fpu_save_area_asm
    mov rdi, r11
    call fpu_restore_area_asm
    add rsp, 8

    mov rsp, r10
    mov byte ptr [r8], 0
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx
    ret
.size context_switch_with_release1, . - context_switch_with_release1
"#
);

/// FXSAVE-format image of the architectural initial x87/SSE state, the state a
/// new thread must start from (CP-45): x87 as after FNINIT (FCW =
/// `KERNEL_X87_FCW` 0x037F, FSW=0, every tag empty — abridged FTW=0 —
/// FIP/FDP/FOP and ST0-7 zero), MXCSR = `KERNEL_MXCSR` 0x1F80 (all SIMD
/// exceptions masked, round-to-nearest, no FTZ/DAZ), xmm0-15 zero.
/// `fpu::FpuState::new_initial` uses it as the legacy region of a new thread's
/// save image; without `xsave-fpu` the ring-3 entry trampolines FXRSTOR it
/// directly.
pub(crate) const FXSAVE_INIT_IMAGE: [u8; 512] = {
    let mut image = [0u8; 512];
    // FCW at bytes 0..2, MXCSR at bytes 24..28 (little-endian).
    let fcw = super::KERNEL_X87_FCW.to_le_bytes();
    let mxcsr = super::KERNEL_MXCSR.to_le_bytes();
    image[0] = fcw[0];
    image[1] = fcw[1];
    image[24] = mxcsr[0];
    image[25] = mxcsr[1];
    image[26] = mxcsr[2];
    image[27] = mxcsr[3];
    image
};
// The byte oracle for the image: hand-laid-out, independent of the constants.
const _: () = assert!(
    FXSAVE_INIT_IMAGE[0] == 0x7f
        && FXSAVE_INIT_IMAGE[1] == 0x03
        && FXSAVE_INIT_IMAGE[24] == 0x80
        && FXSAVE_INIT_IMAGE[25] == 0x1f
        && FXSAVE_INIT_IMAGE[26] == 0
        && FXSAVE_INIT_IMAGE[27] == 0
);

#[cfg(not(feature = "xsave-fpu"))]
#[repr(C, align(16))]
struct FxsaveImage([u8; 512]);

/// FXRSTOR source for the ring-3 entry trampolines (16-byte aligned).
#[cfg(not(feature = "xsave-fpu"))]
#[unsafe(no_mangle)]
static USER_ENTRY_FXSAVE_INIT: FxsaveImage = FxsaveImage(FXSAVE_INIT_IMAGE);

// Without `xsave-fpu` there is no per-thread FPU image, so a new user thread
// would be entered on whatever x87/SSE state the CPU holds at its first
// switch-in: kernel values, or another process's registers. Reset it in the
// ring-3 entry trampolines. FNINIT also clears FIP/FDP/FOP, which FXRSTOR
// leaves untouched on AMD parts when no x87 exception is pending. With
// `xsave-fpu` the switch shims have already restored the thread's
// `FpuState::new_initial` image, so there is nothing to do here.
#[cfg(not(feature = "xsave-fpu"))]
macro_rules! user_entry_fpu_reset {
    () => {
        r#"
    fninit
    fxrstor [rip + USER_ENTRY_FXSAVE_INIT]
"#
    };
}

#[cfg(feature = "xsave-fpu")]
macro_rules! user_entry_fpu_reset {
    () => {
        ""
    };
}

global_asm!(concat!(
    r#"

# thread_start_trampoline — first thing executed by every spawned thread.
# `Thread::new` plants its address in the initial stack frame so the first
# context_switch's `ret` lands here. We `sti` to enable maskable interrupts
# (otherwise the cooperative switch leaves the new thread with IF=0 — Phase
# 1.3 preemption depends on IF=1 in user threads), then `ret` pops the real
# entry-fn pointer and jumps to it.
.global thread_start_trampoline
.type thread_start_trampoline, @function
thread_start_trampoline:
    sti
    ret
.size thread_start_trampoline, . - thread_start_trampoline

# enter_userspace_trampoline — first thing executed by every spawned USER thread.
# Phase 1.4. `Thread::new_user` plants its address in the initial kernel-stack
# frame, with a 5-word IRETQ frame (RIP, CS, RFLAGS, RSP, SS) directly above
# it. After context_switch's 6 callee-saved pops + ret, control lands here
# with RSP pointing at the IRETQ frame. We swapgs (kernel→user) and iretq —
# the CPU pops the frame, switches CS/SS to ring 3 selectors, restores user
# RFLAGS (with IF=1 baked in by Thread::new_user), loads user RIP, switches
# to user RSP.
#
# Per ADR-005 Part B: kernel mode always has GS_BASE = &PER_CPU (set by
# per_cpu::init). The kernel→user transition must swapgs to put the user's
# GS into GS_BASE (KERNEL_GSBASE holds it for first dispatch — initially 0,
# i.e., user starts with GS=0; refreshed on every SYSCALL/timer round-trip).
#
# CP-45: the callee-saved registers arrive zero from the initial frame, but the
# caller-saved ones still hold context_switch's arguments and scratch (kernel
# stack/heap pointers, the XSAVE mask). Zero them so none reaches ring 3.
.global enter_userspace_trampoline
.type enter_userspace_trampoline, @function
enter_userspace_trampoline:
"#,
    user_entry_fpu_reset!(),
    r#"
    xor eax, eax
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor edi, edi
    xor r8d, r8d
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    swapgs
    iretq
.size enter_userspace_trampoline, . - enter_userspace_trampoline
"#
));

unsafe extern "C" {
    #[cfg(not(feature = "xsave-fpu"))]
    /// Save the current task's callee-saved registers, write the resulting
    /// RSP to `*prev_rsp_ptr`, then load `next_rsp` and resume the next task.
    ///
    /// SAFETY: caller must (1) hold no locks across the call, (2) have
    /// interrupts disabled, (3) ensure `prev_rsp_ptr` points at a writable
    /// `u64`, and (4) ensure `next_rsp` is the saved RSP of a task whose
    /// stack contains a valid initial frame (six callee-saved slots followed
    /// by the entry-fn pointer to `ret` into).
    pub fn context_switch(prev_rsp_ptr: *mut u64, next_rsp: u64);

    #[cfg(feature = "xsave-fpu")]
    /// Save the current task's callee-saved registers and x87/SSE/AVX state,
    /// write the resulting RSP to `*prev_rsp_ptr`, then restore `next_fpu`,
    /// load `next_rsp`, and resume the next task.
    ///
    /// SAFETY: caller must satisfy the legacy `context_switch` contract, and
    /// `prev_fpu`/`next_fpu` must point at live per-thread FPU save areas sized
    /// and aligned per `arch::x86_64::fpu::save_area_size()`.
    pub fn context_switch(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        prev_fpu: *mut u8,
        next_fpu: *mut u8,
    );

    #[cfg(not(feature = "xsave-fpu"))]
    /// Save the current task's callee-saved registers, write the resulting
    /// RSP to `*prev_rsp_ptr`, load `next_rsp`, then release two SwitchMutex
    /// lock bytes on the new stack before resuming the next task.
    ///
    /// SAFETY: caller must satisfy `context_switch`'s safety contract plus:
    /// (1) `channel_lock` and `sched_lock` must point to lock bytes inside
    /// kernel-owned SwitchMutex instances, not third-party private layout;
    /// (2) both locks must be held by the calling CPU at entry; (3) caller
    /// must have intentionally forgotten both guards so Rust will not run
    /// their Drop implementations after this asm release; (4) release order
    /// is Channel first, scheduler second; (5) all release metadata is passed
    /// in registers, so the function reads no old-stack release list after
    /// `mov rsp, rsi`; and (6) interrupt state is the caller's contract.
    pub fn context_switch_with_release2(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        channel_lock: *const AtomicBool,
        sched_lock: *const AtomicBool,
    );

    #[cfg(feature = "xsave-fpu")]
    /// Save the current task's callee-saved registers and x87/SSE/AVX state,
    /// write the resulting RSP to `*prev_rsp_ptr`, restore `next_fpu`, load
    /// `next_rsp`, then release two SwitchMutex lock bytes on the new stack.
    ///
    /// SAFETY: caller must satisfy the legacy release2 contract, and
    /// `prev_fpu`/`next_fpu` must point at live per-thread FPU save areas sized
    /// and aligned per `arch::x86_64::fpu::save_area_size()`.
    pub fn context_switch_with_release2(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        prev_fpu: *mut u8,
        next_fpu: *mut u8,
        channel_lock: *const AtomicBool,
        sched_lock: *const AtomicBool,
    );

    #[cfg(not(feature = "xsave-fpu"))]
    /// Save the current task's callee-saved registers, write the resulting
    /// RSP to `*prev_rsp_ptr`, load `next_rsp`, then release ONE SwitchMutex
    /// lock byte on the new stack before resuming the next task.
    ///
    /// The single-lock sibling of `context_switch_with_release2`, used by
    /// `sched::exit` — which holds only the scheduler lock (no channel lock)
    /// yet must not let a concurrent `reap()` observe the dying thread as
    /// reap-able until this CPU is off its (zombie) stack.
    ///
    /// SAFETY: caller must satisfy `context_switch`'s safety contract plus:
    /// (1) `sched_lock` must point to a lock byte inside a kernel-owned
    /// SwitchMutex held by the calling CPU at entry; (2) caller must have
    /// intentionally forgotten the guard so Rust will not run its Drop after
    /// this asm release; (3) the release metadata is passed in a register, so
    /// the function reads no old-stack release list after `mov rsp`; and
    /// (4) interrupt state is the caller's contract.
    pub fn context_switch_with_release1(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        sched_lock: *const AtomicBool,
    );

    #[cfg(feature = "xsave-fpu")]
    /// Save the current task's callee-saved registers and x87/SSE/AVX state,
    /// write the resulting RSP to `*prev_rsp_ptr`, restore `next_fpu`, load
    /// `next_rsp`, then release ONE SwitchMutex lock byte on the new stack.
    ///
    /// SAFETY: caller must satisfy the legacy release1 contract, and
    /// `prev_fpu`/`next_fpu` must point at live per-thread FPU save areas sized
    /// and aligned per `arch::x86_64::fpu::save_area_size()`.
    pub fn context_switch_with_release1(
        prev_rsp_ptr: *mut u64,
        next_rsp: u64,
        prev_fpu: *mut u8,
        next_fpu: *mut u8,
        sched_lock: *const AtomicBool,
    );

    /// First-instruction trampoline for every spawned KERNEL thread.
    /// `Thread::new` installs its address in the initial frame so
    /// context_switch's `ret` lands here. The trampoline does `sti; ret`,
    /// which enables interrupts (so timer preemption can fire) and then
    /// jumps to the real entry fn.
    pub fn thread_start_trampoline();

    /// First-instruction trampoline for every spawned USER thread (Phase
    /// 1.4+). `Thread::new_user` plants its address in the initial frame
    /// followed by a 5-word IRETQ frame; this trampoline zeroes the
    /// caller-saved registers (and, without `xsave-fpu`, resets x87/SSE
    /// state) and `iretq`s to ring 3, so no kernel register contents reach
    /// the new thread (CP-45).
    pub fn enter_userspace_trampoline();

    /// OT-61 C1.1: like [`enter_userspace_trampoline`], but the frame carries
    /// one extra word (the `thread_create` `arg`) directly below the IRETQ
    /// frame, popped into RDI so the ring-3 entry receives it as its first
    /// SysV argument. The other caller-saved registers are zeroed so kernel
    /// register contents never leak to a fresh ring-3 thread.
    #[cfg(feature = "browser-runtime")]
    pub fn enter_userspace_arg_trampoline();
}

// OT-61 C1.1 (`browser-runtime` only — the default build carries no extra
// asm, the Rule-1 kill-switch): the arg-passing ring-3 entry trampoline.
// Frame at entry (RSP →): [arg][RIP][CS][RFLAGS][RSP][SS].
#[cfg(feature = "browser-runtime")]
global_asm!(concat!(
    r#"
.global enter_userspace_arg_trampoline
.type enter_userspace_arg_trampoline, @function
enter_userspace_arg_trampoline:
    pop rdi
"#,
    user_entry_fpu_reset!(),
    r#"
    xor eax, eax
    xor ecx, ecx
    xor edx, edx
    xor esi, esi
    xor r8d, r8d
    xor r9d, r9d
    xor r10d, r10d
    xor r11d, r11d
    swapgs
    iretq
.size enter_userspace_arg_trampoline, . - enter_userspace_arg_trampoline
"#
));

/// Trampoline that catches a thread `entry` returning unexpectedly.
///
/// Phase 1.2 thread entries are all `extern "C" fn() -> !`, so this should be
/// unreachable. We install its address on the initial stack frame in the slot
/// directly above the entry-fn pointer; if `entry` ever does `ret`, control
/// lands here and we panic loudly instead of wandering into unmapped memory.
#[unsafe(no_mangle)]
pub extern "C" fn thread_exit_trampoline() -> ! {
    panic!("thread returned from `entry: extern \"C\" fn() -> !` — exit_trampoline reached");
}
